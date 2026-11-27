//! Shellumi RDP sound redirection handler and native audio player integration (`MS-RDPEA`).
//!
//! Provides live audio streaming from the remote Windows session to the local host
//! using pure Rust `ironrdp-rdpsnd` and `ironrdp-rdpsnd-native` (`cpal`).
//! Handles volume scaling, muting, and real-time audio telemetry reporting.

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ironrdp_rdpsnd::client::RdpsndClientHandler;
use ironrdp_rdpsnd::pdu::{AudioFormat, AudioFormatFlags, PitchPdu, VolumePdu, WaveFormat};
use shellumi_remote_desktop::SessionEvent;
use tracing::{debug, trace};

/// Throttle interval between successive audio status telemetry events to avoid saturating UI channels.
const AUDIO_STATUS_THROTTLE: Duration = Duration::from_millis(500);

/// Thread-safe control and metrics state for active RDP audio redirection.
#[derive(Clone, Debug)]
pub struct ShellumiRdpsndControl {
    is_muted: Arc<AtomicBool>,
    volume: Arc<AtomicU16>,
    audio_active: Arc<AtomicBool>,
    bytes_received: Arc<AtomicU64>,
    packets_received: Arc<AtomicU64>,
    format_name: Arc<Mutex<String>>,
    sample_rate: Arc<AtomicU32>,
    channels: Arc<AtomicU16>,
}

impl Default for ShellumiRdpsndControl {
    fn default() -> Self {
        Self::new(false, 100)
    }
}

impl ShellumiRdpsndControl {
    /// Create a new `ShellumiRdpsndControl` with initial mute and volume (0..=100) state.
    pub fn new(muted: bool, volume: u16) -> Self {
        Self {
            is_muted: Arc::new(AtomicBool::new(muted)),
            volume: Arc::new(AtomicU16::new(volume.min(100))),
            audio_active: Arc::new(AtomicBool::new(false)),
            bytes_received: Arc::new(AtomicU64::new(0)),
            packets_received: Arc::new(AtomicU64::new(0)),
            format_name: Arc::new(Mutex::new("Unnegotiated".to_string())),
            sample_rate: Arc::new(AtomicU32::new(0)),
            channels: Arc::new(AtomicU16::new(0)),
        }
    }

    /// Update the active negotiated audio format metadata.
    pub fn set_negotiated_format(
        &self,
        format: impl Into<String>,
        sample_rate: u32,
        channels: u16,
    ) {
        if let Ok(mut g) = self.format_name.lock() {
            *g = format.into();
        }
        self.sample_rate.store(sample_rate, Ordering::Relaxed);
        self.channels.store(channels, Ordering::Relaxed);
    }

    /// Build a `SessionEvent::AudioStatus` snapshot using the actual negotiated format.
    pub fn current_status_snapshot(&self) -> SessionEvent {
        let fmt = self
            .format_name
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "Unknown".to_string());
        self.status_snapshot(
            &fmt,
            self.sample_rate.load(Ordering::Relaxed),
            self.channels.load(Ordering::Relaxed),
        )
    }

    /// Check if audio output is currently muted.
    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Relaxed)
    }

    /// Set mute status.
    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::Relaxed);
    }

    /// Get current volume level (0..=100 percentage).
    pub fn volume(&self) -> u16 {
        self.volume.load(Ordering::Relaxed)
    }

    /// Set volume level (0..=100 percentage).
    pub fn set_volume(&self, vol: u16) {
        self.volume.store(vol.min(100), Ordering::Relaxed);
    }

    /// Check if active audio packets are currently streaming from the remote host.
    pub fn is_active(&self) -> bool {
        self.audio_active.load(Ordering::Relaxed)
    }

    /// Set active audio streaming state.
    pub fn set_active(&self, active: bool) {
        self.audio_active.store(active, Ordering::Relaxed);
    }

    /// Cumulative audio bytes received over the `RDPSND` virtual channel.
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }

    /// Cumulative audio packets (Wave PDUs) received over the `RDPSND` virtual channel.
    pub fn packets_received(&self) -> u64 {
        self.packets_received.load(Ordering::Relaxed)
    }

    /// Record a newly received audio wave packet.
    pub fn record_wave(&self, byte_count: usize) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received
            .fetch_add(byte_count as u64, Ordering::Relaxed);
    }

    /// Build a `SessionEvent::AudioStatus` snapshot.
    pub fn status_snapshot(&self, format: &str, sample_rate: u32, channels: u16) -> SessionEvent {
        SessionEvent::AudioStatus {
            active: self.is_active(),
            muted: self.is_muted(),
            volume: self.volume(),
            format: format.to_string(),
            sample_rate,
            channels,
            bytes_received: self.bytes_received(),
        }
    }
}

/// Scale 16-bit signed PCM audio samples by a percentage (0..=100).
pub fn scale_pcm_16bit(data: &[u8], volume_percent: u16) -> Vec<u8> {
    if volume_percent == 0 {
        return vec![0u8; data.len()];
    }
    if volume_percent >= 100 {
        return data.to_vec();
    }

    let mut out = Vec::with_capacity(data.len());
    let (chunks, remainder) = data.as_chunks::<2>();

    for chunk in chunks {
        let sample = i16::from_le_bytes(*chunk);
        let scaled = ((sample as i32 * volume_percent as i32) / 100) as i16;
        out.extend_from_slice(&scaled.to_le_bytes());
    }

    out.extend_from_slice(remainder);
    out
}

/// Shellumi RDP sound client handler implementing `RdpsndClientHandler`.
///
/// Wraps native `cpal::RdpsndBackend` to route decoded audio to the system default
/// sound output device while providing live volume adjustment, muting, and UI telemetry.
pub struct ShellumiRdpsndHandler {
    inner: ironrdp_rdpsnd_native::cpal::RdpsndBackend,
    control: ShellumiRdpsndControl,
    worker_tx: async_channel::Sender<SessionEvent>,
    current_format_name: String,
    current_sample_rate: u32,
    current_channels: u16,
    last_status_sent: Instant,
}

impl std::fmt::Debug for ShellumiRdpsndHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellumiRdpsndHandler")
            .field("control", &self.control)
            .field("current_format_name", &self.current_format_name)
            .field("current_sample_rate", &self.current_sample_rate)
            .field("current_channels", &self.current_channels)
            .finish()
    }
}

impl ShellumiRdpsndHandler {
    /// Create a new `ShellumiRdpsndHandler` tied to a `ShellumiRdpsndControl` and UI event sender.
    pub fn new(
        control: ShellumiRdpsndControl,
        worker_tx: async_channel::Sender<SessionEvent>,
    ) -> Self {
        Self {
            inner: ironrdp_rdpsnd_native::cpal::RdpsndBackend::new(),
            control,
            worker_tx,
            current_format_name: "PCM".to_string(),
            current_sample_rate: 44100,
            current_channels: 2,
            last_status_sent: Instant::now() - AUDIO_STATUS_THROTTLE,
        }
    }
}

impl RdpsndClientHandler for ShellumiRdpsndHandler {
    fn get_flags(&self) -> AudioFormatFlags {
        self.inner.get_flags()
    }

    fn get_formats(&self) -> &[AudioFormat] {
        self.inner.get_formats()
    }

    fn wave(&mut self, format_no: usize, ts: u32, data: Cow<'_, [u8]>) {
        self.control.record_wave(data.len());
        if !self.control.is_active() {
            self.control.set_active(true);
        }

        // Cache format parameters if available
        let mut is_pcm_16bit = false;
        let format_info = self.get_formats().get(format_no).map(|fmt| {
            is_pcm_16bit = fmt.format == WaveFormat::PCM && fmt.bits_per_sample == 16;
            (
                format!("{}", fmt.format),
                fmt.n_samples_per_sec,
                fmt.n_channels,
            )
        });
        if let Some((name, rate, channels)) = format_info {
            self.current_format_name = name.clone();
            self.current_sample_rate = rate;
            self.current_channels = channels;
            self.control.set_negotiated_format(name, rate, channels);
        }

        // Throttle UI status updates
        if self.last_status_sent.elapsed() >= AUDIO_STATUS_THROTTLE {
            self.last_status_sent = Instant::now();
            let _ = self.worker_tx.try_send(self.control.status_snapshot(
                &self.current_format_name,
                self.current_sample_rate,
                self.current_channels,
            ));
        }

        // Handle muting and volume scaling before passing to cpal output
        if self.control.is_muted() {
            trace!("RDP audio muted; dropping wave packet");
            return;
        }

        let volume = self.control.volume();
        if volume >= 100 || !is_pcm_16bit {
            self.inner.wave(format_no, ts, data);
        } else {
            let scaled = scale_pcm_16bit(&data, volume);
            self.inner.wave(format_no, ts, Cow::Owned(scaled));
        }
    }

    fn set_volume(&mut self, volume: VolumePdu) {
        debug!(?volume, "Remote server set RDPSND volume");
        let max_val = volume.volume_left.max(volume.volume_right);
        let pct = ((max_val as u32 * 100) / 0xFFFF) as u16;
        self.control.set_volume(pct);

        let _ = self.worker_tx.try_send(self.control.status_snapshot(
            &self.current_format_name,
            self.current_sample_rate,
            self.current_channels,
        ));

        self.inner.set_volume(volume);
    }

    fn set_pitch(&mut self, pitch: PitchPdu) {
        debug!(?pitch, "Remote server set RDPSND pitch");
        self.inner.set_pitch(pitch);
    }

    fn close(&mut self) {
        debug!("Closing RDPSND audio backend");
        self.control.set_active(false);
        let _ = self.worker_tx.try_send(self.control.status_snapshot(
            &self.current_format_name,
            self.current_sample_rate,
            self.current_channels,
        ));
        self.inner.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_pcm_16bit_volume_scaling() {
        let sample1: i16 = 10000;
        let sample2: i16 = -20000;
        let mut data = Vec::new();
        data.extend_from_slice(&sample1.to_le_bytes());
        data.extend_from_slice(&sample2.to_le_bytes());

        // 100% volume
        let scaled_100 = scale_pcm_16bit(&data, 100);
        assert_eq!(scaled_100, data);

        // 50% volume
        let scaled_50 = scale_pcm_16bit(&data, 50);
        let res1 = i16::from_le_bytes([scaled_50[0], scaled_50[1]]);
        let res2 = i16::from_le_bytes([scaled_50[2], scaled_50[3]]);
        assert_eq!(res1, 5000);
        assert_eq!(res2, -10000);

        // 0% volume
        let scaled_0 = scale_pcm_16bit(&data, 0);
        assert_eq!(scaled_0, vec![0, 0, 0, 0]);
    }

    #[test]
    fn test_control_state_and_snapshot() {
        let control = ShellumiRdpsndControl::new(false, 80);
        assert!(!control.is_muted());
        assert_eq!(control.volume(), 80);
        assert!(!control.is_active());

        control.set_muted(true);
        assert!(control.is_muted());

        control.set_volume(60);
        assert_eq!(control.volume(), 60);

        control.record_wave(512);
        assert_eq!(control.packets_received(), 1);
        assert_eq!(control.bytes_received(), 512);

        control.set_active(true);
        let snapshot = control.status_snapshot("PCM", 44100, 2);
        match snapshot {
            SessionEvent::AudioStatus {
                active,
                muted,
                volume,
                format,
                sample_rate,
                channels,
                bytes_received,
            } => {
                assert!(active);
                assert!(muted);
                assert_eq!(volume, 60);
                assert_eq!(format, "PCM");
                assert_eq!(sample_rate, 44100);
                assert_eq!(channels, 2);
                assert_eq!(bytes_received, 512);
            }
            _ => panic!("Expected SessionEvent::AudioStatus"),
        }
    }
}

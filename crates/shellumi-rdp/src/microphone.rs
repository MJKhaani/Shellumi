//! Shellumi RDP Microphone and Audio Input Redirection (`MS-RDPEAI`) integration.
//!
//! Provides live low-latency microphone capture redirection from the local host
//! to remote Windows RDP sessions over the `AUDIO_INPUT` Dynamic Virtual Channel (DVC).
//! Uses `cpal` for cross-platform hardware audio capture, supports dynamic format
//! negotiation (16-bit PCM 44.1/48 kHz stereo/mono), volume gain scaling, muting,
//! and real-time audio telemetry reporting.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ironrdp_client::rdp::RdpInputEvent;
use ironrdp_core::{
    ensure_size, impl_as_any, DecodeResult, Encode, EncodeResult, ReadCursor, WriteCursor,
};
use ironrdp_dvc::{encode_dvc_messages, DvcEncode, DvcMessage, DvcProcessor};
use ironrdp_pdu::{pdu_other_err, PduResult};
use ironrdp_svc::{ChannelFlags, SvcMessage};
use shellumi_remote_desktop::SessionEvent;
use tracing::{debug, error, info, trace, warn};

/// Dynamic Virtual Channel listener name for MS-RDPEAI.
pub const AUDIO_INPUT_CHANNEL_NAME: &str = "AUDIO_INPUT";

/// Message IDs defined in [MS-RDPEAI] § 2.2.1.
pub const MSG_SNDIN_VERSION: u8 = 0x01;
pub const MSG_SNDIN_FORMATS: u8 = 0x02;
pub const MSG_SNDIN_OPEN: u8 = 0x03;
pub const MSG_SNDIN_OPEN_REPLY: u8 = 0x04;
pub const MSG_SNDIN_DATA_INCOMING: u8 = 0x05;
pub const MSG_SNDIN_DATA: u8 = 0x06;
pub const MSG_SNDIN_FORMATCHANGE: u8 = 0x07;

/// WAVE format tag for uncompressed linear PCM.
pub const WAVE_FORMAT_PCM: u16 = 0x0001;

/// Minimum throttle duration between successive UI telemetry updates.
const TELEMETRY_THROTTLE_INTERVAL: Duration = Duration::from_millis(500);

/// Audio format entry conforming to [MS-RDPEA] § 2.2.2.1.1 and [MS-RDPEAI] § 2.2.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormatEntry {
    pub format_tag: u16,
    pub channels: u16,
    pub samples_per_sec: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub cb_size: u16,
}

impl AudioFormatEntry {
    /// Fixed part size for standard WAVEFORMATEX with empty extra data.
    pub const PCM_SIZE: usize = 18;

    /// Construct a standard 16-bit linear PCM format definition.
    pub fn pcm(samples_per_sec: u32, channels: u16) -> Self {
        let block_align = channels * 2; // 16-bit = 2 bytes per sample per channel
        let avg_bytes_per_sec = samples_per_sec * block_align as u32;
        Self {
            format_tag: WAVE_FORMAT_PCM,
            channels,
            samples_per_sec,
            avg_bytes_per_sec,
            block_align,
            bits_per_sample: 16,
            cb_size: 0,
        }
    }

    /// Encode this audio format into a binary cursor.
    pub fn encode(&self, dst: &mut WriteCursor<'_>) -> EncodeResult<()> {
        ensure_size!(in: dst, size: Self::PCM_SIZE);
        dst.write_u16(self.format_tag);
        dst.write_u16(self.channels);
        dst.write_u32(self.samples_per_sec);
        dst.write_u32(self.avg_bytes_per_sec);
        dst.write_u16(self.block_align);
        dst.write_u16(self.bits_per_sample);
        dst.write_u16(self.cb_size);
        Ok(())
    }

    /// Decode an audio format from a binary cursor.
    pub fn decode(cursor: &mut ReadCursor<'_>) -> DecodeResult<Self> {
        ensure_size!(in: cursor, size: Self::PCM_SIZE);
        let format_tag = cursor.read_u16();
        let channels = cursor.read_u16();
        let samples_per_sec = cursor.read_u32();
        let avg_bytes_per_sec = cursor.read_u32();
        let block_align = cursor.read_u16();
        let bits_per_sample = cursor.read_u16();
        let cb_size = cursor.read_u16();

        // Skip any extra codec-specific bytes
        if cb_size > 0 {
            ensure_size!(in: cursor, size: cb_size as usize);
            cursor.advance(cb_size as usize);
        }

        Ok(Self {
            format_tag,
            channels,
            samples_per_sec,
            avg_bytes_per_sec,
            block_align,
            bits_per_sample,
            cb_size,
        })
    }
}

/// Outgoing RDPEAI Protocol Data Unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RdpeaiPdu {
    Version(u32),
    Formats(Vec<AudioFormatEntry>),
    OpenReply(u32),
    IncomingData,
    Data(Vec<u8>),
    FormatChange(u32),
}

impl Encode for RdpeaiPdu {
    fn encode(&self, dst: &mut WriteCursor<'_>) -> EncodeResult<()> {
        match self {
            Self::Version(v) => {
                ensure_size!(in: dst, size: 5);
                dst.write_u8(MSG_SNDIN_VERSION);
                dst.write_u32(*v);
            }
            Self::Formats(formats) => {
                let total_size = 1 + 4 + 4 + formats.len() * AudioFormatEntry::PCM_SIZE;
                ensure_size!(in: dst, size: total_size);
                dst.write_u8(MSG_SNDIN_FORMATS);
                dst.write_u32(formats.len() as u32);
                dst.write_u32(total_size as u32);
                for fmt in formats {
                    fmt.encode(dst)?;
                }
            }
            Self::OpenReply(result) => {
                ensure_size!(in: dst, size: 5);
                dst.write_u8(MSG_SNDIN_OPEN_REPLY);
                dst.write_u32(*result);
            }
            Self::IncomingData => {
                ensure_size!(in: dst, size: 1);
                dst.write_u8(MSG_SNDIN_DATA_INCOMING);
            }
            Self::Data(data) => {
                ensure_size!(in: dst, size: 1 + data.len());
                dst.write_u8(MSG_SNDIN_DATA);
                dst.write_slice(data);
            }
            Self::FormatChange(idx) => {
                ensure_size!(in: dst, size: 5);
                dst.write_u8(MSG_SNDIN_FORMATCHANGE);
                dst.write_u32(*idx);
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "RDPEAI_PDU"
    }

    fn size(&self) -> usize {
        match self {
            Self::Version(_) => 5,
            Self::Formats(f) => 1 + 4 + 4 + f.len() * AudioFormatEntry::PCM_SIZE,
            Self::OpenReply(_) => 5,
            Self::IncomingData => 1,
            Self::Data(d) => 1 + d.len(),
            Self::FormatChange(_) => 5,
        }
    }
}

impl DvcEncode for RdpeaiPdu {}

/// Parsed incoming server RDPEAI message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerRdpeaiMessage {
    Version(u32),
    Formats(Vec<AudioFormatEntry>),
    Open {
        frames_per_packet: u32,
        initial_format: u32,
        format: Option<AudioFormatEntry>,
    },
    FormatChange(u32),
    Unknown(u8),
}

/// Parse an incoming server payload into a `ServerRdpeaiMessage`.
pub fn parse_server_rdpeai_message(payload: &[u8]) -> Result<ServerRdpeaiMessage, String> {
    if payload.is_empty() {
        return Err("Empty RDPEAI payload".into());
    }

    let mut cursor = ReadCursor::new(payload);
    let msg_id = cursor.read_u8();

    match msg_id {
        MSG_SNDIN_VERSION => {
            if cursor.len() < 4 {
                return Err("Truncated MSG_SNDIN_VERSION PDU".into());
            }
            let version = cursor.read_u32();
            Ok(ServerRdpeaiMessage::Version(version))
        }
        MSG_SNDIN_FORMATS => {
            if cursor.len() < 8 {
                return Err("Truncated MSG_SNDIN_FORMATS PDU header".into());
            }
            let num_formats = cursor.read_u32() as usize;
            let _cb_size = cursor.read_u32();
            let mut formats = Vec::with_capacity(num_formats.min(32));
            for _ in 0..num_formats {
                if cursor.is_empty() {
                    break;
                }
                match AudioFormatEntry::decode(&mut cursor) {
                    Ok(fmt) => formats.push(fmt),
                    Err(e) => {
                        debug!(
                            "Skipping unsupported audio format in server formats: {:?}",
                            e
                        );
                        break;
                    }
                }
            }
            Ok(ServerRdpeaiMessage::Formats(formats))
        }
        MSG_SNDIN_OPEN => {
            if cursor.len() < 8 {
                return Err("Truncated MSG_SNDIN_OPEN PDU header".into());
            }
            let frames_per_packet = cursor.read_u32();
            let initial_format = cursor.read_u32();
            let format = if cursor.len() >= AudioFormatEntry::PCM_SIZE {
                AudioFormatEntry::decode(&mut cursor).ok()
            } else {
                None
            };
            Ok(ServerRdpeaiMessage::Open {
                frames_per_packet,
                initial_format,
                format,
            })
        }
        MSG_SNDIN_FORMATCHANGE => {
            if cursor.len() < 4 {
                return Err("Truncated MSG_SNDIN_FORMATCHANGE PDU".into());
            }
            let new_format = cursor.read_u32();
            Ok(ServerRdpeaiMessage::FormatChange(new_format))
        }
        other => Ok(ServerRdpeaiMessage::Unknown(other)),
    }
}

/// Thread-safe control, settings, and telemetry state for RDP audio input redirection.
#[derive(Clone, Debug)]
pub struct MicrophoneControlState {
    channel_id: Arc<AtomicU32>,
    ready: Arc<AtomicBool>,
    recording: Arc<AtomicBool>,
    revoked: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
    volume: Arc<AtomicU16>, // 0..=100 input volume/gain
    bytes_sent: Arc<AtomicU64>,
    packets_sent: Arc<AtomicU64>,
    sample_rate: Arc<AtomicU32>,
    channels: Arc<AtomicU16>,
    input_sender: Arc<std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<RdpInputEvent>>>>,
    active_stop_signal: Arc<std::sync::Mutex<Option<Arc<AtomicBool>>>>,
}

impl Default for MicrophoneControlState {
    fn default() -> Self {
        Self::new(false, 100)
    }
}

impl MicrophoneControlState {
    /// Create a new `MicrophoneControlState` with initial mute and volume percentage.
    pub fn new(muted: bool, volume: u16) -> Self {
        Self {
            channel_id: Arc::new(AtomicU32::new(0)),
            ready: Arc::new(AtomicBool::new(false)),
            recording: Arc::new(AtomicBool::new(false)),
            revoked: Arc::new(AtomicBool::new(false)),
            muted: Arc::new(AtomicBool::new(muted)),
            volume: Arc::new(AtomicU16::new(volume.min(100))),
            bytes_sent: Arc::new(AtomicU64::new(0)),
            packets_sent: Arc::new(AtomicU64::new(0)),
            sample_rate: Arc::new(AtomicU32::new(44100)),
            channels: Arc::new(AtomicU16::new(2)),
            input_sender: Arc::new(std::sync::Mutex::new(None)),
            active_stop_signal: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Register the unbounded input sender used to transmit DVC messages to the IronRDP event loop.
    pub fn set_input_sender(&self, sender: tokio::sync::mpsc::UnboundedSender<RdpInputEvent>) {
        if let Ok(mut guard) = self.input_sender.lock() {
            *guard = Some(sender);
        }
    }

    /// Check if the input sender has been registered.
    pub fn has_input_sender(&self) -> bool {
        if let Ok(guard) = self.input_sender.lock() {
            guard.is_some()
        } else {
            false
        }
    }

    /// Dispatch DVC messages to the IronRDP client runner.
    pub fn send_dvc_messages(&self, channel_id: u32, messages: Vec<SvcMessage>) -> bool {
        if let Ok(guard) = self.input_sender.lock() {
            if let Some(sender) = guard.as_ref() {
                return sender
                    .send(RdpInputEvent::SendDvcMessages {
                        channel_id,
                        messages,
                    })
                    .is_ok();
            }
        }
        false
    }

    /// Negotiated dynamic virtual channel ID.
    pub fn channel_id(&self) -> Option<u32> {
        let id = self.channel_id.load(Ordering::Relaxed);
        if id != 0 {
            Some(id)
        } else {
            None
        }
    }

    /// Set dynamic virtual channel ID.
    pub fn set_channel_id(&self, id: u32) {
        self.channel_id.store(id, Ordering::Relaxed);
    }

    /// Check if the remote host has negotiated and opened the audio input channel.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Set channel readiness.
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Relaxed);
    }

    /// Check if audio recording is currently active.
    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::Relaxed)
    }

    /// Set recording state; disabling signals any active native capture stream to stop immediately.
    pub fn set_recording(&self, recording: bool) {
        self.recording.store(recording, Ordering::Release);
        if !recording {
            self.signal_stop_active_capture();
        }
    }

    /// Revoke microphone capture permission and stop any active native capture stream.
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
        self.recording.store(false, Ordering::Release);
        self.signal_stop_active_capture();
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked.load(Ordering::Acquire)
    }

    pub fn register_stop_signal(&self, sig: Arc<AtomicBool>) {
        if let Ok(mut guard) = self.active_stop_signal.lock() {
            *guard = Some(sig);
        }
    }

    fn signal_stop_active_capture(&self) {
        if let Ok(mut guard) = self.active_stop_signal.lock() {
            if let Some(sig) = guard.take() {
                sig.store(true, Ordering::Release);
            }
        }
    }

    /// Check if microphone capture is currently muted.
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// Set mute status.
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    /// Get current input volume/gain (0..=100%).
    pub fn volume(&self) -> u16 {
        self.volume.load(Ordering::Relaxed)
    }

    /// Set input volume/gain (0..=100%).
    pub fn set_volume(&self, vol: u16) {
        self.volume.store(vol.min(100), Ordering::Relaxed);
    }

    /// Negotiated sample rate (e.g. 44100 or 48000 Hz).
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    /// Set negotiated sample rate.
    pub fn set_sample_rate(&self, rate: u32) {
        self.sample_rate.store(rate, Ordering::Relaxed);
    }

    /// Negotiated channel count (1 for mono, 2 for stereo).
    pub fn channels(&self) -> u16 {
        self.channels.load(Ordering::Relaxed)
    }

    /// Set negotiated channel count.
    pub fn set_channels(&self, channels: u16) {
        self.channels.store(channels, Ordering::Relaxed);
    }

    /// Total audio bytes transmitted to the remote host.
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }

    /// Total audio packets transmitted to the remote host.
    pub fn packets_sent(&self) -> u64 {
        self.packets_sent.load(Ordering::Relaxed)
    }

    /// Record a transmitted audio chunk in telemetry.
    pub fn record_sent(&self, byte_count: usize) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent
            .fetch_add(byte_count as u64, Ordering::Relaxed);
    }

    /// Build a `SessionEvent::MicrophoneStatus` snapshot for UI presentation.
    pub fn status_snapshot(&self) -> SessionEvent {
        SessionEvent::MicrophoneStatus {
            active: self.is_recording(),
            muted: self.is_muted(),
            volume: self.volume(),
            format: format!("16-bit PCM {}Hz {}ch", self.sample_rate(), self.channels()),
            sample_rate: self.sample_rate(),
            channels: self.channels(),
            bytes_sent: self.bytes_sent(),
            packets_sent: self.packets_sent(),
        }
    }
}

/// Scale 16-bit signed PCM audio samples by volume percentage (0..=100) and mute state.
///
/// Guaranteed panic-free and immune to integer overflow.
pub fn scale_pcm_16bit(samples: &mut [i16], volume: u16, muted: bool) {
    if muted || volume == 0 {
        samples.fill(0);
        return;
    }
    if volume == 100 {
        return;
    }

    let vol = volume as i32;
    for sample in samples.iter_mut() {
        let scaled = (*sample as i32 * vol) / 100;
        *sample = scaled.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }
}

/// Convert an array of 16-bit signed integer samples to little-endian bytes.
pub fn samples_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// Active audio capture controller and background stream handle.
pub struct AudioCaptureHandle {
    _stream: Option<cpal::Stream>,
    stop_signal: Arc<AtomicBool>,
}

impl AudioCaptureHandle {
    /// Stop active audio recording.
    pub fn stop(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }
}

/// Start background audio capture using `cpal`.
///
/// Discovers the default audio input device, builds a capture stream,
/// applies volume scaling and muting, and transmits encoded `RdpeaiPdu::Data`
/// messages to the IronRDP input channel.
pub fn start_audio_capture(
    control_state: MicrophoneControlState,
    event_tx: async_channel::Sender<SessionEvent>,
    channel_id: u32,
    sample_rate: u32,
    channels: u16,
) -> Option<AudioCaptureHandle> {
    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(dev) => dev,
        None => {
            warn!("No default audio input device found on host; audio input capture in standby");
            return None;
        }
    };

    #[allow(deprecated)]
    let device_name = device.name().unwrap_or_else(|_| "Default Device".into());
    info!(
        device = %device_name,
        sample_rate,
        channels,
        "Initializing native audio input capture via cpal"
    );

    let config = cpal::StreamConfig {
        channels,
        sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_signal_clone = stop_signal.clone();
    let control_clone = control_state.clone();
    let event_tx_clone = event_tx.clone();

    let mut last_telemetry = Instant::now();

    let err_fn = |err| {
        error!("Error occurred on cpal input audio stream: {:?}", err);
    };

    let stream_result = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if stop_signal_clone.load(Ordering::Acquire)
                || !control_clone.is_recording()
                || control_clone.is_revoked()
            {
                return;
            }

            // Convert f32 samples (-1.0..=1.0) to i16 PCM
            let mut i16_samples: Vec<i16> = data
                .iter()
                .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect();

            let vol = control_clone.volume();
            let muted = control_clone.is_muted();
            scale_pcm_16bit(&mut i16_samples, vol, muted);

            let pcm_bytes = samples_to_le_bytes(&i16_samples);
            let byte_count = pcm_bytes.len();

            // Encode RDPEAI Data PDU
            let pdu = RdpeaiPdu::Data(pcm_bytes);
            if let Ok(svc_messages) =
                encode_dvc_messages(channel_id, vec![Box::new(pdu)], ChannelFlags::empty())
            {
                let _ = control_clone.send_dvc_messages(channel_id, svc_messages);
                control_clone.record_sent(byte_count);
            }

            // Throttle UI telemetry updates
            if last_telemetry.elapsed() >= TELEMETRY_THROTTLE_INTERVAL {
                last_telemetry = Instant::now();
                let _ = event_tx_clone.try_send(control_clone.status_snapshot());
            }
        },
        err_fn,
        None,
    );

    match stream_result {
        Ok(stream) => {
            if let Err(e) = stream.play() {
                error!("Failed to start cpal input stream: {:?}", e);
                return None;
            }
            debug!("cpal input audio stream started successfully");
            Some(AudioCaptureHandle {
                _stream: Some(stream),
                stop_signal,
            })
        }
        Err(e) => {
            warn!(
                "Failed to build cpal audio input stream: {:?}; operating in standby mode",
                e
            );
            None
        }
    }
}

/// Dynamic Virtual Channel client processor for `MS-RDPEAI` (`AUDIO_INPUT`).
pub struct ShellumiRdpeaiClient {
    state: MicrophoneControlState,
    event_tx: async_channel::Sender<SessionEvent>,
    supported_formats: Vec<AudioFormatEntry>,
    capture_handle: Option<AudioCaptureHandle>,
}

impl ShellumiRdpeaiClient {
    /// Create a new `ShellumiRdpeaiClient`.
    pub fn new(
        state: MicrophoneControlState,
        event_tx: async_channel::Sender<SessionEvent>,
    ) -> Self {
        let supported_formats = vec![
            AudioFormatEntry::pcm(44100, 2),
            AudioFormatEntry::pcm(44100, 1),
            AudioFormatEntry::pcm(48000, 2),
            AudioFormatEntry::pcm(48000, 1),
        ];

        Self {
            state,
            event_tx,
            supported_formats,
            capture_handle: None,
        }
    }

    /// Access control state.
    pub fn state(&self) -> &MicrophoneControlState {
        &self.state
    }

    /// Manually inject a PCM audio buffer for deterministic testing without hardware microphones.
    pub fn inject_test_audio_chunk(&self, chunk: &[i16]) -> Result<usize, String> {
        let channel_id = self
            .state
            .channel_id()
            .ok_or_else(|| "Channel ID not established".to_string())?;

        let mut samples = chunk.to_vec();
        scale_pcm_16bit(&mut samples, self.state.volume(), self.state.is_muted());
        let pcm_bytes = samples_to_le_bytes(&samples);
        let byte_count = pcm_bytes.len();

        let pdu = RdpeaiPdu::Data(pcm_bytes);
        let svc_messages =
            encode_dvc_messages(channel_id, vec![Box::new(pdu)], ChannelFlags::empty())
                .map_err(|e| format!("Failed to encode DVC messages: {:?}", e))?;

        self.state.record_sent(byte_count);
        let _ = self.event_tx.try_send(self.state.status_snapshot());
        let _ = self.state.send_dvc_messages(channel_id, svc_messages);

        Ok(byte_count)
    }
}

impl_as_any!(ShellumiRdpeaiClient);

impl DvcProcessor for ShellumiRdpeaiClient {
    fn channel_name(&self) -> &str {
        AUDIO_INPUT_CHANNEL_NAME
    }

    fn start(&mut self, channel_id: u32) -> PduResult<Vec<DvcMessage>> {
        debug!(channel_id, "MS-RDPEAI AUDIO_INPUT dynamic channel created");
        self.state.set_channel_id(channel_id);
        Ok(Vec::new())
    }

    fn process(&mut self, channel_id: u32, payload: &[u8]) -> PduResult<Vec<DvcMessage>> {
        self.state.set_channel_id(channel_id);

        let parsed = parse_server_rdpeai_message(payload).map_err(|e| {
            warn!(error = %e, "Failed to parse incoming RDPEAI message");
            pdu_other_err!("ShellumiRdpeaiClient", "malformed RDPEAI message")
        })?;

        trace!(?parsed, "Received RDPEAI message from server");

        match parsed {
            ServerRdpeaiMessage::Version(ver) => {
                info!(server_version = ver, "Negotiated RDPEAI protocol version");
                // Reply with our supported version (1)
                let reply = RdpeaiPdu::Version(1);
                Ok(vec![Box::new(reply)])
            }
            ServerRdpeaiMessage::Formats(formats) => {
                info!(
                    count = formats.len(),
                    "Received supported audio input formats from server"
                );
                // Reply with our supported format list
                let reply = RdpeaiPdu::Formats(self.supported_formats.clone());
                Ok(vec![Box::new(reply)])
            }
            ServerRdpeaiMessage::Open {
                frames_per_packet,
                initial_format,
                format,
            } => {
                let chosen_fmt = format.unwrap_or_else(|| {
                    let idx = (initial_format as usize).min(self.supported_formats.len() - 1);
                    self.supported_formats[idx]
                });

                info!(
                    frames_per_packet,
                    initial_format,
                    sample_rate = chosen_fmt.samples_per_sec,
                    channels = chosen_fmt.channels,
                    "Remote server opened audio input channel; activating microphone recording"
                );

                if self.state.is_revoked() {
                    self.capture_handle = None;
                    self.state.set_recording(false);
                    let _ = self.event_tx.try_send(self.state.status_snapshot());
                    return Ok(vec![Box::new(RdpeaiPdu::OpenReply(0x8000_4005))]);
                }

                self.state.set_sample_rate(chosen_fmt.samples_per_sec);
                self.state.set_channels(chosen_fmt.channels);
                self.state.set_ready(true);

                // Start native cpal audio capture BEFORE marking recording or replying S_OK
                self.capture_handle = start_audio_capture(
                    self.state.clone(),
                    self.event_tx.clone(),
                    channel_id,
                    chosen_fmt.samples_per_sec,
                    chosen_fmt.channels,
                );
                if let Some(ref handle) = self.capture_handle {
                    self.state.register_stop_signal(handle.stop_signal.clone());
                }

                let open_ok = self.capture_handle.is_some();
                self.state.set_recording(open_ok);
                let _ = self.event_tx.try_send(self.state.status_snapshot());

                let reply_code: u32 = if open_ok { 0 } else { 0x8000_4005 }; // S_OK or E_FAIL
                let reply = RdpeaiPdu::OpenReply(reply_code);
                Ok(vec![Box::new(reply)])
            }
            ServerRdpeaiMessage::FormatChange(new_format) => {
                info!(new_format, "Server requested RDPEAI format change");
                let idx = (new_format as usize).min(self.supported_formats.len() - 1);
                let chosen_fmt = self.supported_formats[idx];
                let was_recording = self.state.is_recording();
                if let Some(handle) = self.capture_handle.take() {
                    handle.stop();
                }
                self.state.set_sample_rate(chosen_fmt.samples_per_sec);
                self.state.set_channels(chosen_fmt.channels);
                if was_recording {
                    self.capture_handle = start_audio_capture(
                        self.state.clone(),
                        self.event_tx.clone(),
                        channel_id,
                        chosen_fmt.samples_per_sec,
                        chosen_fmt.channels,
                    );
                    self.state.set_recording(self.capture_handle.is_some());
                }
                let _ = self.event_tx.try_send(self.state.status_snapshot());

                let reply = RdpeaiPdu::FormatChange(new_format);
                Ok(vec![Box::new(reply)])
            }
            ServerRdpeaiMessage::Unknown(msg_id) => {
                debug!(msg_id, "Ignored unhandled server RDPEAI message");
                Ok(Vec::new())
            }
        }
    }

    fn close(&mut self, channel_id: u32) {
        info!(channel_id, "Closing RDPEAI audio input channel");
        if let Some(handle) = self.capture_handle.take() {
            handle.stop();
        }
        self.state.set_recording(false);
        self.state.set_ready(false);
        let _ = self.event_tx.try_send(self.state.status_snapshot());
    }
}

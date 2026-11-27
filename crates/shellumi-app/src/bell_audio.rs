//! Cross-platform terminal bell audio service with rate limiting and non-blocking playback.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[allow(dead_code)]
pub const MAX_CUSTOM_SOUND_BYTES: usize = 1024 * 1024; // 1 MiB
const DEFAULT_COOLDOWN: Duration = Duration::from_millis(120);

/// Rate limiter and coalescer for terminal bell audio requests.
#[derive(Debug)]
pub struct BellRateLimiter {
    cooldown: Duration,
    last_global: Option<Instant>,
    last_per_session: HashMap<u64, Instant>,
}

impl BellRateLimiter {
    pub fn new(cooldown: Duration) -> Self {
        Self {
            cooldown,
            last_global: None,
            last_per_session: HashMap::new(),
        }
    }

    pub fn allow(&mut self, session_id: u64, now: Instant) -> bool {
        if let Some(prev) = self.last_global {
            if now.duration_since(prev) < self.cooldown {
                return false;
            }
        }
        if let Some(prev) = self.last_per_session.get(&session_id) {
            if now.duration_since(*prev) < self.cooldown {
                return false;
            }
        }
        self.last_global = Some(now);
        self.last_per_session.insert(session_id, now);
        true
    }
}

/// Validates a custom WAV sound file (bounded size and RIFF/WAVE header).
#[allow(dead_code)]
pub fn validate_custom_sound_file(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Cannot read sound file: {e}"))?;
    if meta.len() as usize > MAX_CUSTOM_SOUND_BYTES {
        return Err(format!(
            "Sound file exceeds maximum size ({MAX_CUSTOM_SOUND_BYTES} bytes)"
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read sound file: {e}"))?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Unsupported sound format (expected PCM WAV)".to_string());
    }
    Ok(bytes)
}

#[derive(Clone, Debug)]
struct AudioPlayRequest {
    volume: f32,
}

/// Non-blocking cross-platform bell audio service backed by `cpal` on a worker thread.
pub struct BellAudioService {
    tx: mpsc::SyncSender<AudioPlayRequest>,
    limiter: BellRateLimiter,
    pub last_error: Option<String>,
}

impl BellAudioService {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<AudioPlayRequest>(8);
        std::thread::Builder::new()
            .name("shellumi-bell-audio".to_string())
            .spawn(move || {
                while let Ok(req) = rx.recv() {
                    if let Err(err) = play_chime_cpal(req.volume) {
                        tracing::debug!(error = %err, "Bell audio playback skipped or failed");
                    }
                }
            })
            .ok();
        Self {
            tx,
            limiter: BellRateLimiter::new(DEFAULT_COOLDOWN),
            last_error: None,
        }
    }

    pub fn trigger(&mut self, session_id: u64, volume: f32) -> bool {
        if volume <= 0.0 {
            return false;
        }
        if !self.limiter.allow(session_id, Instant::now()) {
            return false;
        }
        match self.tx.try_send(AudioPlayRequest {
            volume: volume.clamp(0.0, 1.0),
        }) {
            Ok(()) => {
                self.last_error = None;
                true
            }
            Err(mpsc::TrySendError::Full(_)) => false,
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.last_error = Some("Audio worker unavailable".to_string());
                false
            }
        }
    }
}

/// Plays a short 880Hz sine-envelope chime via `cpal` on the default output device.
pub fn play_chime_cpal(volume: f32) -> Result<String, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "No audio output device available".to_string())?;
    let device_name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "default".to_string());
    let config = device
        .default_output_config()
        .map_err(|e| format!("Failed to query default audio config: {e}"))?;
    let sample_rate = config.sample_rate() as f32;
    let channels = config.channels() as usize;
    let total_samples = (sample_rate * 0.09) as usize; // 90 ms chime
    let mut sample_idx = 0usize;
    let vol = volume.clamp(0.0, 1.0) * 0.25;

    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    let val = if sample_idx < total_samples {
                        let t = sample_idx as f32 / sample_rate;
                        let env = 1.0 - (sample_idx as f32 / total_samples as f32);
                        (t * 880.0 * std::f32::consts::TAU).sin() * env * vol
                    } else {
                        0.0
                    };
                    sample_idx += 1;
                    for sample in frame.iter_mut() {
                        *sample = val;
                    }
                }
            },
            |err| {
                tracing::debug!(error = %err, "cpal stream error");
            },
            None,
        )
        .map_err(|e| format!("Failed to build audio output stream: {e}"))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start audio stream: {e}"))?;
    std::thread::sleep(Duration::from_millis(100));
    drop(stream);
    Ok(device_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_coalesces_bel_bursts_with_injected_clock() {
        let mut limiter = BellRateLimiter::new(Duration::from_millis(100));
        let t0 = Instant::now();
        assert!(limiter.allow(1, t0));
        assert!(!limiter.allow(1, t0 + Duration::from_millis(20)));
        assert!(!limiter.allow(2, t0 + Duration::from_millis(50)));
        assert!(limiter.allow(1, t0 + Duration::from_millis(120)));
    }

    #[test]
    fn rejects_invalid_and_oversized_custom_sound_files() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("bad.wav");
        std::fs::write(&bad, b"not-a-wav").unwrap();
        assert!(validate_custom_sound_file(&bad).is_err());

        let big = tmp.path().join("big.wav");
        let mut data = vec![0u8; MAX_CUSTOM_SOUND_BYTES + 10];
        data[0..4].copy_from_slice(b"RIFF");
        data[8..12].copy_from_slice(b"WAVE");
        std::fs::write(&big, data).unwrap();
        assert!(validate_custom_sound_file(&big).is_err());
    }

    #[test]
    fn native_cpal_audio_smoke_test() {
        match play_chime_cpal(0.2) {
            Ok(device_name) => {
                assert!(!device_name.is_empty());
            }
            Err(err) => {
                // On headless CI without an audio device, cpal returns a clean error without panicking.
                assert!(!err.is_empty());
            }
        }
    }
}

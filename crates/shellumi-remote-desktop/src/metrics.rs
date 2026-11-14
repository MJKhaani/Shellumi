//! Real protocol metrics collector for frame rates, throughput, and decode latency.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Sliding window duration for FPS calculation.
const FPS_WINDOW: Duration = Duration::from_millis(1000);
/// Maximum samples retained for rolling average decode latency.
const MAX_DECODE_SAMPLES: usize = 60;

/// Snapshot of measured session metrics at a point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionMetricsSnapshot {
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub frames_decoded: u64,
    pub fps: Option<u32>,
    pub avg_decode_ms: Option<f64>,
    pub latency_ms: Option<u32>,
}

/// Rolling metrics collector for active remote desktop sessions.
///
/// Strictly computes metrics from observed protocol frames, byte streams, and timing.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionMetrics {
    bytes_received: u64,
    bytes_sent: u64,
    frames_decoded: u64,
    frame_timestamps: VecDeque<Instant>,
    decode_durations: VecDeque<Duration>,
    last_latency_ms: Option<u32>,
}

impl Default for SessionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMetrics {
    /// Initialize empty metrics with unmeasured status.
    pub fn new() -> Self {
        Self {
            bytes_received: 0,
            bytes_sent: 0,
            frames_decoded: 0,
            frame_timestamps: VecDeque::new(),
            decode_durations: VecDeque::new(),
            last_latency_ms: None,
        }
    }

    /// Record newly received network bytes.
    pub fn record_bytes_received(&mut self, count: u64) {
        self.bytes_received = self.bytes_received.saturating_add(count);
    }

    /// Record newly sent network bytes.
    pub fn record_bytes_sent(&mut self, count: u64) {
        self.bytes_sent = self.bytes_sent.saturating_add(count);
    }

    /// Record a successfully decoded and rendered frame update.
    pub fn record_frame_decoded(&mut self, decode_duration: Duration, now: Instant) {
        self.frames_decoded = self.frames_decoded.saturating_add(1);
        self.frame_timestamps.push_back(now);
        self.decode_durations.push_back(decode_duration);

        if self.decode_durations.len() > MAX_DECODE_SAMPLES {
            self.decode_durations.pop_front();
        }

        self.prune_stale_frames(now);
    }

    /// Record a round-trip network ping measurement.
    pub fn record_latency_sample(&mut self, latency_ms: u32) {
        self.last_latency_ms = Some(latency_ms);
    }

    /// Total bytes received over the wire.
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received
    }

    /// Total bytes transmitted over the wire.
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent
    }

    /// Total number of frame updates processed.
    pub fn frames_decoded(&self) -> u64 {
        self.frames_decoded
    }

    /// Compute currently measured frames per second over the active sliding window.
    ///
    /// Returns `None` if fewer than 2 frames have arrived in the window or the session has not started.
    pub fn fps(&self, now: Instant) -> Option<u32> {
        let cutoff = now.checked_sub(FPS_WINDOW).unwrap_or(now);
        let count = self
            .frame_timestamps
            .iter()
            .filter(|&&t| t >= cutoff)
            .count();

        if count >= 2 {
            Some(count as u32)
        } else {
            None
        }
    }

    /// Compute average decode duration in milliseconds over recent frames.
    pub fn avg_decode_ms(&self) -> Option<f64> {
        if self.decode_durations.is_empty() {
            return None;
        }

        let total_us: u128 = self.decode_durations.iter().map(|d| d.as_micros()).sum();
        let avg_us = total_us as f64 / self.decode_durations.len() as f64;
        Some(avg_us / 1000.0)
    }

    /// Capture an immutable telemetry snapshot.
    pub fn snapshot(&mut self, now: Instant) -> SessionMetricsSnapshot {
        self.prune_stale_frames(now);
        SessionMetricsSnapshot {
            bytes_received: self.bytes_received,
            bytes_sent: self.bytes_sent,
            frames_decoded: self.frames_decoded,
            fps: self.fps(now),
            avg_decode_ms: self.avg_decode_ms(),
            latency_ms: self.last_latency_ms,
        }
    }

    fn prune_stale_frames(&mut self, now: Instant) {
        if let Some(cutoff) = now.checked_sub(FPS_WINDOW) {
            while let Some(&front) = self.frame_timestamps.front() {
                if front < cutoff {
                    self.frame_timestamps.pop_front();
                } else {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_metrics_unmeasured_baseline() {
        let mut metrics = SessionMetrics::new();
        let now = Instant::now();
        let snap = metrics.snapshot(now);

        assert_eq!(snap.bytes_received, 0);
        assert_eq!(snap.bytes_sent, 0);
        assert_eq!(snap.frames_decoded, 0);
        assert_eq!(snap.fps, None);
        assert_eq!(snap.avg_decode_ms, None);
        assert_eq!(snap.latency_ms, None);
    }

    #[test]
    fn verifies_frame_recording_and_fps_calculation() {
        let mut metrics = SessionMetrics::new();
        let start = Instant::now();

        // Feed 10 frames spaced 50ms apart (20 FPS rate)
        for i in 0..10 {
            let frame_time = start + Duration::from_millis(i * 50);
            metrics.record_frame_decoded(Duration::from_millis(5), frame_time);
        }

        let at_500ms = start + Duration::from_millis(500);
        let snap = metrics.snapshot(at_500ms);
        assert_eq!(snap.frames_decoded, 10);
        assert_eq!(snap.fps, Some(10));
        assert!(snap.avg_decode_ms.is_some());
        let avg = snap.avg_decode_ms.unwrap();
        assert!((avg - 5.0).abs() < 0.1);
    }

    #[test]
    fn verifies_latency_recording() {
        let mut metrics = SessionMetrics::new();
        metrics.record_latency_sample(24);
        let snap = metrics.snapshot(Instant::now());
        assert_eq!(snap.latency_ms, Some(24));
    }
}

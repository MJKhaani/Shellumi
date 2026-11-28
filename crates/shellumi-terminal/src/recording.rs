//! Terminal session recording and Asciicast v2 serialization engine.
//!
//! Implements streaming session recording conforming to the Asciicast v2 specification,
//! supporting timestamped I/O event capturing, incremental UTF-8 decoding across chunk
//! boundaries, pause/resume, opt-in input capture with sensitive suppression, explicit
//! bounds/truncation, and strict deserialization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Default per-recording limits.
pub const DEFAULT_MAX_RECORDING_BYTES: usize = 50 * 1024 * 1024; // 50 MB
pub const DEFAULT_MAX_RECORDING_EVENTS: usize = 500_000;
pub const DEFAULT_MAX_RECORDING_DURATION_SECS: f64 = 12.0 * 3600.0; // 12 hours
pub const MAX_CAST_PARSE_BYTES: usize = 64 * 1024 * 1024; // 64 MB

/// Reason a recording stopped accepting new events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingTruncationReason {
    MaxBytes,
    MaxEvents,
    MaxDuration,
}

/// Configurable bounds for a recording session.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordingLimits {
    pub max_bytes: usize,
    pub max_events: usize,
    pub max_duration_secs: f64,
}

impl Default for RecordingLimits {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_RECORDING_BYTES,
            max_events: DEFAULT_MAX_RECORDING_EVENTS,
            max_duration_secs: DEFAULT_MAX_RECORDING_DURATION_SECS,
        }
    }
}

/// Type of recorded terminal event in Asciicast v2 format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CastEventType {
    /// Terminal output emitted by the process or remote SSH stream.
    #[serde(rename = "o")]
    Output,
    /// User input sent into the terminal / PTY.
    #[serde(rename = "i")]
    Input,
    /// Terminal screen resize notification (`"<cols>x<rows>"`).
    #[serde(rename = "r")]
    Resize,
}

impl CastEventType {
    pub fn as_code(&self) -> &'static str {
        match self {
            Self::Output => "o",
            Self::Input => "i",
            Self::Resize => "r",
        }
    }
}

/// A single timestamped terminal I/O event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CastEvent {
    /// Elapsed time in seconds since the start of recording.
    pub time_offset_secs: f64,
    /// Event stream classification (output, input, or resize).
    pub event_type: CastEventType,
    /// Event string payload (VT/ANSI escape sequences, text, or dimensions).
    pub data: String,
}

/// Header metadata for an Asciicast v2 recording file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsciicastHeader {
    /// Asciicast specification version (strictly 2).
    pub version: u32,
    /// Initial terminal column width.
    pub width: usize,
    /// Initial terminal row height.
    pub height: usize,
    /// Epoch timestamp when the recording commenced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
    /// Optional title or session descriptor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Total duration of the recording in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// Environment variables captured at recording start.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
}

impl Default for AsciicastHeader {
    fn default() -> Self {
        Self {
            version: 2,
            width: 80,
            height: 24,
            timestamp: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            ),
            title: Some("Shellumi Session Recording".to_string()),
            duration: None,
            env: HashMap::new(),
        }
    }
}

/// An immutable completed terminal session recording.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionRecording {
    /// Asciicast header metadata.
    pub header: AsciicastHeader,
    /// Chronological list of recorded events.
    pub events: Vec<CastEvent>,
    /// Whether the recording hit a configured limit and was truncated.
    #[serde(default)]
    pub truncated: bool,
    /// Reason for truncation, if truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_reason: Option<RecordingTruncationReason>,
}

impl SessionRecording {
    /// Exports the recording to standard Asciicast v2 format (`.cast`).
    pub fn to_asciicast_v2(&self) -> Result<String, String> {
        let mut out = String::new();
        let header_json = serde_json::to_string(&self.header)
            .map_err(|e| format!("Failed to serialize asciicast header: {e}"))?;
        out.push_str(&header_json);
        out.push('\n');

        for event in &self.events {
            let event_tuple = (
                (event.time_offset_secs * 1000.0).round() / 1000.0,
                event.event_type.as_code(),
                &event.data,
            );
            let event_json = serde_json::to_string(&event_tuple)
                .map_err(|e| format!("Failed to serialize asciicast event: {e}"))?;
            out.push_str(&event_json);
            out.push('\n');
        }

        Ok(out)
    }

    /// Parses an Asciicast v2 formatted string into a `SessionRecording`,
    /// validating version, dimensions, timestamps, and size bounds.
    pub fn parse_asciicast_v2(input: &str) -> Result<Self, String> {
        if input.len() > MAX_CAST_PARSE_BYTES {
            return Err(format!(
                "Asciicast input exceeds maximum allowed size ({} bytes)",
                MAX_CAST_PARSE_BYTES
            ));
        }
        let mut lines = input.lines();
        let first_line = lines
            .next()
            .ok_or_else(|| "Empty asciicast input".to_string())?;

        let header: AsciicastHeader = serde_json::from_str(first_line)
            .map_err(|e| format!("Invalid asciicast header JSON: {e}"))?;

        if header.version != 2 {
            return Err(format!(
                "Unsupported asciicast version: {}, expected 2",
                header.version
            ));
        }
        if header.width == 0 || header.height == 0 || header.width > 2000 || header.height > 2000 {
            return Err(format!(
                "Invalid terminal dimensions {}x{} in asciicast header",
                header.width, header.height
            ));
        }
        if let Some(d) = header.duration {
            if !d.is_finite() || d < 0.0 {
                return Err("Invalid negative or non-finite duration in header".to_string());
            }
        }

        let mut events = Vec::new();
        for (idx, line) in lines.enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (time_offset, type_code, data): (f64, String, String) =
                serde_json::from_str(trimmed)
                    .map_err(|e| format!("Invalid asciicast event line {}: {e}", idx + 2))?;

            if !time_offset.is_finite() || time_offset < 0.0 {
                return Err(format!(
                    "Invalid timestamp {time_offset} on line {}",
                    idx + 2
                ));
            }

            let event_type = match type_code.as_str() {
                "o" => CastEventType::Output,
                "i" => CastEventType::Input,
                "r" => CastEventType::Resize,
                other => {
                    return Err(format!(
                        "Unknown asciicast event type: '{other}' on line {}",
                        idx + 2
                    ))
                }
            };

            events.push(CastEvent {
                time_offset_secs: time_offset,
                event_type,
                data,
            });
        }

        Ok(Self {
            header,
            events,
            truncated: false,
            truncation_reason: None,
        })
    }

    /// Extracts all terminal output as plain text without ANSI escape sequences.
    pub fn plain_text_output(&self) -> String {
        let mut out = String::new();
        for event in &self.events {
            if event.event_type == CastEventType::Output {
                out.push_str(&strip_ansi_escapes(&event.data));
            }
        }
        out
    }

    /// Returns total recorded duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        if let Some(d) = self.header.duration {
            d
        } else if let Some(last) = self.events.last() {
            last.time_offset_secs
        } else {
            0.0
        }
    }

    /// Returns total number of recorded events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

/// Incremental UTF-8 decoder that buffers incomplete multibyte sequences across
/// arbitrary chunk boundaries and replaces invalid byte sequences with U+FFFD.
#[derive(Clone, Debug, Default)]
pub struct Utf8StreamDecoder {
    pending: [u8; 4],
    pending_len: usize,
}

impl Utf8StreamDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn decode_chunk(&mut self, input: &[u8]) -> String {
        let mut combined = Vec::with_capacity(self.pending_len + input.len());
        combined.extend_from_slice(&self.pending[..self.pending_len]);
        combined.extend_from_slice(input);
        self.pending_len = 0;

        let mut out = String::with_capacity(combined.len());
        let mut pos = 0;
        while pos < combined.len() {
            match std::str::from_utf8(&combined[pos..]) {
                Ok(valid) => {
                    out.push_str(valid);
                    break;
                }
                Err(err) => {
                    let valid_up_to = err.valid_up_to();
                    if valid_up_to > 0 {
                        let valid = std::str::from_utf8(&combined[pos..pos + valid_up_to])
                            .unwrap_or_default();
                        out.push_str(valid);
                        pos += valid_up_to;
                    }
                    match err.error_len() {
                        Some(invalid_len) => {
                            out.push('\u{FFFD}');
                            pos += invalid_len;
                        }
                        None => {
                            // Incomplete multibyte sequence at the end of the chunk
                            let rem = combined.len() - pos;
                            self.pending[..rem].copy_from_slice(&combined[pos..]);
                            self.pending_len = rem;
                            break;
                        }
                    }
                }
            }
        }
        out
    }

    pub fn flush(&mut self) -> String {
        if self.pending_len > 0 {
            self.pending_len = 0;
            "\u{FFFD}".to_string()
        } else {
            String::new()
        }
    }
}

/// Active in-memory session recorder.
pub struct SessionRecorder {
    header: AsciicastHeader,
    events: Vec<CastEvent>,
    start_time: Instant,
    paused_duration: Duration,
    pause_started_at: Option<Instant>,
    active: bool,
    paused: bool,
    input_recording_enabled: bool,
    limits: RecordingLimits,
    current_bytes: usize,
    flushed_event_count: usize,
    truncated: bool,
    truncation_reason: Option<RecordingTruncationReason>,
    output_utf8: Utf8StreamDecoder,
    input_utf8: Utf8StreamDecoder,
}

impl SessionRecorder {
    /// Creates a new inactive session recorder.
    pub fn new(width: usize, height: usize, title: Option<String>) -> Self {
        let header = AsciicastHeader {
            width,
            height,
            title,
            ..Default::default()
        };

        Self {
            header,
            events: Vec::new(),
            start_time: Instant::now(),
            paused_duration: Duration::ZERO,
            pause_started_at: None,
            active: false,
            paused: false,
            input_recording_enabled: false,
            limits: RecordingLimits::default(),
            current_bytes: 0,
            flushed_event_count: 0,
            truncated: false,
            truncation_reason: None,
            output_utf8: Utf8StreamDecoder::new(),
            input_utf8: Utf8StreamDecoder::new(),
        }
    }

    pub fn set_limits(&mut self, limits: RecordingLimits) {
        self.limits = limits;
    }

    pub fn set_input_recording_enabled(&mut self, enabled: bool) {
        self.input_recording_enabled = enabled;
    }

    pub fn input_recording_enabled(&self) -> bool {
        self.input_recording_enabled
    }

    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    pub fn truncation_reason(&self) -> Option<RecordingTruncationReason> {
        self.truncation_reason
    }

    /// Starts recording session events.
    pub fn start(&mut self) {
        self.start_time = Instant::now();
        self.paused_duration = Duration::ZERO;
        self.pause_started_at = None;
        self.active = true;
        self.paused = false;
        self.events.clear();
        self.current_bytes = 0;
        self.flushed_event_count = 0;
        self.truncated = false;
        self.truncation_reason = None;
        self.output_utf8 = Utf8StreamDecoder::new();
        self.input_utf8 = Utf8StreamDecoder::new();
        self.header.timestamp = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
    }

    /// Pauses recording. Elapsed recording time does not advance while paused.
    pub fn pause(&mut self) {
        if self.active && !self.paused {
            self.paused = true;
            self.pause_started_at = Some(Instant::now());
        }
    }

    /// Resumes recording after a pause.
    pub fn resume(&mut self) {
        if self.active && self.paused {
            if let Some(started) = self.pause_started_at.take() {
                self.paused_duration += started.elapsed();
            }
            self.paused = false;
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Returns true if recording is currently active (even if paused).
    pub fn is_active(&self) -> bool {
        self.active
    }

    fn effective_elapsed(&self) -> Duration {
        let total = self.start_time.elapsed();
        let current_pause = self
            .pause_started_at
            .map(|p| p.elapsed())
            .unwrap_or(Duration::ZERO);
        total.saturating_sub(self.paused_duration + current_pause)
    }

    fn can_accept_event(&mut self, payload_bytes: usize, elapsed_secs: f64) -> bool {
        if !self.active || self.paused || self.truncated {
            return false;
        }
        if elapsed_secs > self.limits.max_duration_secs {
            self.truncated = true;
            self.truncation_reason = Some(RecordingTruncationReason::MaxDuration);
            return false;
        }
        if self.events.len() >= self.limits.max_events {
            self.truncated = true;
            self.truncation_reason = Some(RecordingTruncationReason::MaxEvents);
            return false;
        }
        if self.current_bytes.saturating_add(payload_bytes) > self.limits.max_bytes {
            self.truncated = true;
            self.truncation_reason = Some(RecordingTruncationReason::MaxBytes);
            return false;
        }
        true
    }

    /// Records output bytes received from the terminal or remote host.
    pub fn record_output(&mut self, bytes: &[u8]) {
        if !self.active || self.paused || self.truncated {
            return;
        }
        let data = self.output_utf8.decode_chunk(bytes);
        if data.is_empty() {
            return;
        }
        let elapsed = self.effective_elapsed().as_secs_f64();
        if !self.can_accept_event(data.len(), elapsed) {
            return;
        }
        self.current_bytes += data.len();
        self.events.push(CastEvent {
            time_offset_secs: elapsed,
            event_type: CastEventType::Output,
            data,
        });
    }

    /// Records user keystroke input sent to the PTY (subject to opt-in and sensitivity policy).
    pub fn record_input(&mut self, bytes: &[u8]) {
        self.record_input_with_policy(bytes, false);
    }

    /// Records input only when input recording is enabled AND `sensitive` is false.
    pub fn record_input_with_policy(&mut self, bytes: &[u8], sensitive: bool) {
        if !self.active
            || self.paused
            || self.truncated
            || !self.input_recording_enabled
            || sensitive
        {
            return;
        }
        let data = self.input_utf8.decode_chunk(bytes);
        if data.is_empty() {
            return;
        }
        let elapsed = self.effective_elapsed().as_secs_f64();
        if !self.can_accept_event(data.len(), elapsed) {
            return;
        }
        self.current_bytes += data.len();
        self.events.push(CastEvent {
            time_offset_secs: elapsed,
            event_type: CastEventType::Input,
            data,
        });
    }

    /// Records a terminal window resize event (enforcing limits before insertion).
    pub fn record_resize(&mut self, cols: usize, rows: usize) {
        if !self.active || self.paused || self.truncated {
            return;
        }
        let elapsed = self.effective_elapsed().as_secs_f64();
        let data = format!("{cols}x{rows}");
        if !self.can_accept_event(data.len(), elapsed) {
            return;
        }
        self.current_bytes += data.len();
        self.events.push(CastEvent {
            time_offset_secs: elapsed,
            event_type: CastEventType::Resize,
            data,
        });
    }

    /// Stops recording and returns the completed `SessionRecording`.
    pub fn stop(&mut self) -> SessionRecording {
        let elapsed = self.effective_elapsed().as_secs_f64();
        let rem_out = self.output_utf8.flush();
        if !rem_out.is_empty() && self.can_accept_event(rem_out.len(), elapsed) {
            self.current_bytes += rem_out.len();
            self.events.push(CastEvent {
                time_offset_secs: elapsed,
                event_type: CastEventType::Output,
                data: rem_out,
            });
        }
        let rem_in = self.input_utf8.flush();
        if !rem_in.is_empty()
            && self.input_recording_enabled
            && self.can_accept_event(rem_in.len(), elapsed)
        {
            self.current_bytes += rem_in.len();
            self.events.push(CastEvent {
                time_offset_secs: elapsed,
                event_type: CastEventType::Input,
                data: rem_in,
            });
        }

        self.active = false;
        self.paused = false;
        self.pause_started_at = None;
        let mut header = self.header.clone();
        header.duration = Some(elapsed);

        SessionRecording {
            header,
            events: std::mem::take(&mut self.events),
            truncated: self.truncated,
            truncation_reason: self.truncation_reason,
        }
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    pub fn byte_count(&self) -> usize {
        self.current_bytes
    }

    pub fn header(&self) -> &AsciicastHeader {
        &self.header
    }

    /// Returns any newly-captured events since the last flush without stopping the recorder.
    pub fn peek_unflushed_events(&self) -> (usize, Vec<CastEvent>) {
        if self.flushed_event_count >= self.events.len() {
            return (0, Vec::new());
        }
        (
            self.events.len(),
            self.events[self.flushed_event_count..].to_vec(),
        )
    }

    /// Marks events up to `new_flushed_count` as durably persisted.
    pub fn mark_events_flushed(&mut self, new_flushed_count: usize) {
        self.flushed_event_count = new_flushed_count.min(self.events.len());
    }

    /// Returns the number of bytes in unflushed events.
    pub fn unflushed_bytes(&self) -> usize {
        self.events
            .get(self.flushed_event_count..)
            .map(|slice| slice.iter().map(|e| e.data.len()).sum())
            .unwrap_or(0)
    }

    pub fn elapsed_duration(&self) -> Duration {
        if self.active {
            self.effective_elapsed()
        } else {
            Duration::ZERO
        }
    }
}

fn strip_ansi_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
            continue;
        }
        if in_escape {
            if c.is_ascii_alphabetic() || c == '~' {
                in_escape = false;
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_serializes_asciicast_v2() {
        let mut recorder = SessionRecorder::new(80, 24, Some("Test Session".to_string()));
        recorder.set_input_recording_enabled(true);
        assert!(!recorder.is_active());

        recorder.start();
        assert!(recorder.is_active());

        recorder.record_output(b"Hello ");
        recorder.record_output(b"World!\r\n");
        recorder.record_input(b"ls\r");
        recorder.record_resize(120, 36);

        assert_eq!(recorder.event_count(), 4);

        let recording = recorder.stop();
        assert!(!recorder.is_active());
        assert_eq!(recording.events.len(), 4);
        assert_eq!(recording.header.width, 80);
        assert_eq!(recording.header.height, 24);
        assert_eq!(recording.header.title.as_deref(), Some("Test Session"));

        let cast_str = recording.to_asciicast_v2().expect("should serialize");
        let parsed = SessionRecording::parse_asciicast_v2(&cast_str).expect("should parse");
        assert_eq!(parsed.header.version, 2);
        assert_eq!(parsed.events.len(), 4);
        assert_eq!(parsed.events[2].event_type, CastEventType::Input);
        assert_eq!(parsed.events[3].data, "120x36");
        assert_eq!(recording.plain_text_output(), "Hello World!\r\n");
    }

    #[test]
    fn utf8_split_at_every_byte_boundary_and_invalid_bytes() {
        let mut recorder = SessionRecorder::new(80, 24, None);
        recorder.start();
        let emoji = "🚀".as_bytes();
        assert_eq!(emoji.len(), 4);
        for &b in emoji {
            recorder.record_output(&[b]);
        }
        recorder.record_output(&[0xff, 0xfe]);
        let rec = recorder.stop();
        let joined: String = rec.events.iter().map(|e| e.data.as_str()).collect();
        assert_eq!(joined, "🚀\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn input_disabled_by_default_and_sensitive_input_suppressed() {
        let mut recorder = SessionRecorder::new(80, 24, None);
        recorder.start();
        recorder.record_input(b"secret1");
        assert_eq!(recorder.event_count(), 0);

        recorder.set_input_recording_enabled(true);
        recorder.record_input_with_policy(b"my-password", true);
        assert_eq!(recorder.event_count(), 0);
        recorder.record_input_with_policy(b"ls\r", false);
        assert_eq!(recorder.event_count(), 1);
    }

    #[test]
    fn enforces_size_event_and_resize_limits_explicitly() {
        let mut recorder = SessionRecorder::new(80, 24, None);
        recorder.set_limits(RecordingLimits {
            max_bytes: 16,
            max_events: 100,
            max_duration_secs: 60.0,
        });
        recorder.start();
        recorder.record_output(b"1234567890");
        recorder.record_resize(100, 40); // "100x40" is 6 bytes -> total 16
        assert!(!recorder.is_truncated());
        recorder.record_resize(120, 40); // exceeds 16 bytes
        assert!(recorder.is_truncated());
        assert_eq!(
            recorder.truncation_reason(),
            Some(RecordingTruncationReason::MaxBytes)
        );
        let rec = recorder.stop();
        assert!(rec.truncated);
    }

    #[test]
    fn strips_ansi_escapes_correctly() {
        let colored = "\x1b[31mRed Text\x1b[0m and \x1b[1;32mBold Green\x1b[0m";
        let stripped = strip_ansi_escapes(colored);
        assert_eq!(stripped, "Red Text and Bold Green");
    }

    #[test]
    fn rejects_invalid_asciicast_versions() {
        let invalid = "{\"version\":1,\"width\":80,\"height\":24}\n[0.1, \"o\", \"hi\"]\n";
        let res = SessionRecording::parse_asciicast_v2(invalid);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .contains("Unsupported asciicast version: 1"));
    }
}

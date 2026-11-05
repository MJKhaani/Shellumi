//! Terminal session recording and Asciicast v2 serialization engine.
//!
//! Implements streaming session recording conforming to the Asciicast v2 specification,
//! supporting timestamped I/O event capturing, terminal window resizing, duration tracking,
//! plaintext log extraction, and deserialization for frame-by-frame replay.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
}

impl SessionRecording {
    /// Exports the recording to standard Asciicast v2 format (`.cast`).
    ///
    /// Header is formatted as a single JSON object on line 1, followed by
    /// JSON array entries `[time, type, data]` on subsequent lines.
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

    /// Parses an Asciicast v2 formatted string into a `SessionRecording`.
    pub fn parse_asciicast_v2(input: &str) -> Result<Self, String> {
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

        let mut events = Vec::new();
        for (idx, line) in lines.enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (time_offset, type_code, data): (f64, String, String) =
                serde_json::from_str(trimmed)
                    .map_err(|e| format!("Invalid asciicast event line {}: {e}", idx + 2))?;

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

        Ok(Self { header, events })
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

/// Active in-memory session recorder.
pub struct SessionRecorder {
    header: AsciicastHeader,
    events: Vec<CastEvent>,
    start_time: Instant,
    active: bool,
    max_bytes: usize,
    current_bytes: usize,
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
            active: false,
            max_bytes: 50 * 1024 * 1024, // 50 MB safety cap per recording
            current_bytes: 0,
        }
    }

    /// Starts recording session events.
    pub fn start(&mut self) {
        self.start_time = Instant::now();
        self.active = true;
        self.events.clear();
        self.current_bytes = 0;
        self.header.timestamp = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
    }

    /// Returns true if recording is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Records output bytes received from the terminal or remote host.
    pub fn record_output(&mut self, bytes: &[u8]) {
        if !self.active || self.current_bytes >= self.max_bytes {
            return;
        }
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let data = String::from_utf8_lossy(bytes).to_string();
        self.current_bytes += data.len();
        self.events.push(CastEvent {
            time_offset_secs: elapsed,
            event_type: CastEventType::Output,
            data,
        });
    }

    /// Records user keystroke input sent to the PTY.
    pub fn record_input(&mut self, bytes: &[u8]) {
        if !self.active || self.current_bytes >= self.max_bytes {
            return;
        }
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let data = String::from_utf8_lossy(bytes).to_string();
        self.current_bytes += data.len();
        self.events.push(CastEvent {
            time_offset_secs: elapsed,
            event_type: CastEventType::Input,
            data,
        });
    }

    /// Records a terminal window resize event.
    pub fn record_resize(&mut self, cols: usize, rows: usize) {
        if !self.active {
            return;
        }
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let data = format!("{cols}x{rows}");
        self.events.push(CastEvent {
            time_offset_secs: elapsed,
            event_type: CastEventType::Resize,
            data,
        });
    }

    /// Stops recording and returns the completed `SessionRecording`.
    pub fn stop(&mut self) -> SessionRecording {
        self.active = false;
        let mut header = self.header.clone();
        let duration = self.start_time.elapsed().as_secs_f64();
        header.duration = Some(duration);

        SessionRecording {
            header,
            events: std::mem::take(&mut self.events),
        }
    }

    /// Returns the number of events captured so far.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Returns elapsed duration since recording started.
    pub fn elapsed_duration(&self) -> Duration {
        if self.active {
            self.start_time.elapsed()
        } else {
            Duration::ZERO
        }
    }
}

/// Helper function to strip ANSI escape codes from terminal text.
fn strip_ansi_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
            continue;
        }
        if in_escape {
            // End of ANSI CSI sequence typically terminating with ASCII letter or ~
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
        assert!(cast_str.contains("\"version\":2"));
        assert!(cast_str.contains("\"width\":80"));
        assert!(cast_str.contains("\"height\":24"));
        assert!(cast_str.contains("\"o\""));
        assert!(cast_str.contains("Hello "));
        assert!(cast_str.contains("\"i\""));
        assert!(cast_str.contains("\"r\""));
        assert!(cast_str.contains("120x36"));

        // Test round-trip deserialization
        let parsed = SessionRecording::parse_asciicast_v2(&cast_str).expect("should parse");
        assert_eq!(parsed.header.version, 2);
        assert_eq!(parsed.header.width, 80);
        assert_eq!(parsed.header.height, 24);
        assert_eq!(parsed.events.len(), 4);
        assert_eq!(parsed.events[0].event_type, CastEventType::Output);
        assert_eq!(parsed.events[0].data, "Hello ");
        assert_eq!(parsed.events[2].event_type, CastEventType::Input);
        assert_eq!(parsed.events[2].data, "ls\r");
        assert_eq!(parsed.events[3].event_type, CastEventType::Resize);
        assert_eq!(parsed.events[3].data, "120x36");

        // Plain text stripping
        let plain = recording.plain_text_output();
        assert_eq!(plain, "Hello World!\r\n");
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

//! Terminal session audit recording models, sensitive data redaction, and cast playback engine.
//!
//! Provides decoupled domain structures for session recording metadata, automated
//! redaction of classified secrets (private keys, passwords, bearer tokens, API keys, MFA codes),
//! and a frame-by-frame Asciicast playback state machine supporting variable speed playback and seeking.

use serde::{Deserialize, Serialize};

/// Metadata describing a recorded terminal session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionRecordingMetadata {
    /// Unique identifier for this recording (UUID or timestamp-based key).
    pub id: String,
    /// Associated live session identifier.
    pub session_id: String,
    /// Optional foreign key to the host in inventory.
    pub host_id: Option<i64>,
    /// User-friendly title or description for the recording.
    pub title: String,
    /// Total duration of the recording in seconds.
    pub duration_secs: f64,
    /// Total number of I/O and resize events recorded.
    pub event_count: usize,
    /// Size of the raw Asciicast payload in bytes.
    pub byte_size: usize,
    /// ISO-8601 or RFC-3339 timestamp when recording occurred.
    pub recorded_at: String,
    /// Timestamp when record was saved in database.
    pub created_at: String,
    /// Arbitrary tags for filtering and categorization.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Brief summary of a session recording for listings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionRecordingSummary {
    pub id: String,
    pub session_id: String,
    pub host_id: Option<i64>,
    pub title: String,
    pub duration_secs: f64,
    pub event_count: usize,
    pub byte_size: usize,
    pub recorded_at: String,
}

impl SessionRecordingSummary {
    /// Formats the duration into a human-readable `MM:SS` or `HH:MM:SS` string.
    pub fn formatted_duration(&self) -> String {
        format_duration_hms(self.duration_secs)
    }

    /// Formats byte size into human-readable representation (`KB`, `MB`).
    pub fn formatted_size(&self) -> String {
        if self.byte_size < 1024 {
            format!("{} B", self.byte_size)
        } else if self.byte_size < 1024 * 1024 {
            format!("{:.1} KB", self.byte_size as f64 / 1024.0)
        } else {
            format!("{:.2} MB", self.byte_size as f64 / (1024.0 * 1024.0))
        }
    }
}

/// Playback speed multiplier options.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum PlaybackSpeed {
    Half,
    #[default]
    Normal,
    Double,
    Quad,
}

impl PlaybackSpeed {
    pub fn multiplier(&self) -> f64 {
        match self {
            Self::Half => 0.5,
            Self::Normal => 1.0,
            Self::Double => 2.0,
            Self::Quad => 4.0,
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Half => "0.5x",
            Self::Normal => "1.0x",
            Self::Double => "2.0x",
            Self::Quad => "4.0x",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Half => Self::Normal,
            Self::Normal => Self::Double,
            Self::Double => Self::Quad,
            Self::Quad => Self::Half,
        }
    }
}

/// Current status of the playback engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
    Completed,
}

/// A single timestamped playback frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaybackFrame {
    pub time_offset_secs: f64,
    pub event_type: String,
    pub data: String,
}

/// State machine driving Asciicast playback with time scrubbing and speed control.
#[derive(Clone, Debug, PartialEq)]
pub struct CastPlaybackEngine {
    frames: Vec<PlaybackFrame>,
    duration_secs: f64,
    current_time_secs: f64,
    status: PlaybackStatus,
    speed: PlaybackSpeed,
    next_frame_index: usize,
}

impl CastPlaybackEngine {
    /// Creates a new playback engine from a list of frames and total duration.
    pub fn new(mut frames: Vec<PlaybackFrame>, duration_override: Option<f64>) -> Self {
        frames.sort_by(|a, b| {
            a.time_offset_secs
                .partial_cmp(&b.time_offset_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let duration = duration_override
            .or_else(|| frames.last().map(|f| f.time_offset_secs))
            .unwrap_or(0.0)
            .max(0.0);

        Self {
            frames,
            duration_secs: duration,
            current_time_secs: 0.0,
            status: PlaybackStatus::Stopped,
            speed: PlaybackSpeed::Normal,
            next_frame_index: 0,
        }
    }

    /// Creates an engine by parsing an Asciicast v2 formatted string.
    pub fn from_asciicast_v2(input: &str) -> Result<Self, String> {
        let mut lines = input.lines();
        let header_line = lines.next().ok_or_else(|| "Empty cast input".to_string())?;

        let header: serde_json::Value = serde_json::from_str(header_line)
            .map_err(|e| format!("Failed to parse header JSON: {e}"))?;

        let duration_override = header.get("duration").and_then(|v| v.as_f64());

        let mut frames = Vec::new();
        for (idx, line) in lines.enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let val: serde_json::Value = serde_json::from_str(trimmed)
                .map_err(|e| format!("Invalid cast event on line {}: {e}", idx + 2))?;

            if let Some(arr) = val.as_array() {
                if arr.len() >= 3 {
                    let time = arr[0].as_f64().unwrap_or(0.0);
                    let event_type = arr[1].as_str().unwrap_or("o").to_string();
                    let data = arr[2].as_str().unwrap_or("").to_string();
                    frames.push(PlaybackFrame {
                        time_offset_secs: time,
                        event_type,
                        data,
                    });
                }
            }
        }

        Ok(Self::new(frames, duration_override))
    }

    pub fn status(&self) -> PlaybackStatus {
        self.status
    }

    pub fn speed(&self) -> PlaybackSpeed {
        self.speed
    }

    pub fn set_speed(&mut self, speed: PlaybackSpeed) {
        self.speed = speed;
    }

    pub fn cycle_speed(&mut self) -> PlaybackSpeed {
        self.speed = self.speed.next();
        self.speed
    }

    pub fn current_time_secs(&self) -> f64 {
        self.current_time_secs
    }

    pub fn duration_secs(&self) -> f64 {
        self.duration_secs
    }

    pub fn total_frames(&self) -> usize {
        self.frames.len()
    }

    pub fn play(&mut self) {
        if self.status == PlaybackStatus::Completed {
            self.seek(0.0);
        }
        self.status = PlaybackStatus::Playing;
    }

    pub fn pause(&mut self) {
        if self.status == PlaybackStatus::Playing {
            self.status = PlaybackStatus::Paused;
        }
    }

    pub fn toggle_play_pause(&mut self) {
        match self.status {
            PlaybackStatus::Playing => self.pause(),
            PlaybackStatus::Paused | PlaybackStatus::Stopped | PlaybackStatus::Completed => {
                self.play()
            }
        }
    }

    pub fn stop(&mut self) {
        self.status = PlaybackStatus::Stopped;
        self.current_time_secs = 0.0;
        self.next_frame_index = 0;
    }

    /// Advances playback by real elapsed time `delta_real_secs`, applying the current speed multiplier.
    /// Returns any frames that occurred during this time increment.
    pub fn advance(&mut self, delta_real_secs: f64) -> Vec<PlaybackFrame> {
        if self.status != PlaybackStatus::Playing || delta_real_secs <= 0.0 {
            return Vec::new();
        }

        let scaled_delta = delta_real_secs * self.speed.multiplier();
        let target_time = (self.current_time_secs + scaled_delta).min(self.duration_secs);

        let mut emitted = Vec::new();
        while self.next_frame_index < self.frames.len() {
            let frame = &self.frames[self.next_frame_index];
            if frame.time_offset_secs <= target_time {
                emitted.push(frame.clone());
                self.next_frame_index += 1;
            } else {
                break;
            }
        }

        self.current_time_secs = target_time;
        if self.current_time_secs >= self.duration_secs {
            self.status = PlaybackStatus::Completed;
        }

        emitted
    }

    /// Seeks directly to a target time offset in seconds.
    ///
    /// Returns all cumulative frames from time 0 up to `target_time_secs` so
    /// the virtual terminal screen can be rebuilt accurately to the seek point.
    pub fn seek(&mut self, target_time_secs: f64) -> Vec<PlaybackFrame> {
        let clamped = target_time_secs.clamp(0.0, self.duration_secs);
        self.current_time_secs = clamped;

        // Find index of first frame beyond target_time_secs
        let mut idx = 0;
        while idx < self.frames.len() && self.frames[idx].time_offset_secs <= clamped {
            idx += 1;
        }
        self.next_frame_index = idx;

        if self.current_time_secs >= self.duration_secs && self.duration_secs > 0.0 {
            self.status = PlaybackStatus::Completed;
        } else if self.status == PlaybackStatus::Completed {
            self.status = PlaybackStatus::Paused;
        }

        self.frames[..self.next_frame_index].to_vec()
    }

    /// Returns fractional progress between 0.0 and 1.0.
    pub fn progress_fraction(&self) -> f32 {
        if self.duration_secs <= 0.0 {
            0.0
        } else {
            ((self.current_time_secs / self.duration_secs) as f32).clamp(0.0, 1.0)
        }
    }

    /// Formats current position and duration as `MM:SS / MM:SS`.
    pub fn formatted_time(&self) -> String {
        format!(
            "{} / {}",
            format_duration_hms(self.current_time_secs),
            format_duration_hms(self.duration_secs)
        )
    }
}

/// Helper function to format seconds into `MM:SS` or `HH:MM:SS`.
pub fn format_duration_hms(total_secs: f64) -> String {
    let secs = total_secs.max(0.0).round() as u64;
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let remainder = secs % 60;

    if hours > 0 {
        format!("{hours:02}:{mins:02}:{remainder:02}")
    } else {
        format!("{mins:02}:{remainder:02}")
    }
}

/// Sensitive data redaction engine for session recordings and terminal audit logs.
///
/// Implements pattern-based scrubbing to mask confidential materials such as
/// SSH/TLS private keys, classified passwords, bearer tokens, AWS credentials,
/// GitHub tokens, and one-time multi-factor authentication codes.
pub struct SessionRedactor;

impl SessionRedactor {
    /// Redacts sensitive credentials from arbitrary text or terminal escape stream.
    pub fn redact_text(input: &str) -> String {
        let mut text = input.to_string();

        // 1. Redact PEM private keys (RSA, OPENSSH, EC, DSA, PRIVATE KEY)
        text = redact_private_keys(&text);

        // 2. Redact Bearer and authorization tokens
        text = redact_bearer_tokens(&text);

        // 3. Redact AWS access keys (e.g. AKIA...)
        text = redact_aws_keys(&text);

        // 4. Redact GitHub / GitLab personal access tokens
        text = redact_git_tokens(&text);

        // 5. Redact key-value assignments (password=..., api_key=..., secret=...)
        text = redact_assignments(&text);

        // 6. Redact 6-digit MFA codes
        text = redact_mfa_codes(&text);

        text
    }

    /// Redacts all events in an Asciicast v2 formatted string while preserving
    /// JSON syntax, time offsets, and window resize operations.
    pub fn redact_asciicast_v2(cast_data: &str) -> Result<String, String> {
        let mut lines = cast_data.lines();
        let header_line = lines.next().ok_or_else(|| "Empty cast data".to_string())?;

        // Verify header is valid JSON
        let _header_val: serde_json::Value = serde_json::from_str(header_line)
            .map_err(|e| format!("Invalid asciicast header: {e}"))?;

        let mut output = String::with_capacity(cast_data.len());
        output.push_str(header_line);
        output.push('\n');

        for (idx, line) in lines.enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let val: serde_json::Value = serde_json::from_str(trimmed)
                .map_err(|e| format!("Invalid cast event on line {}: {e}", idx + 2))?;

            if let Some(arr) = val.as_array() {
                if arr.len() >= 3 {
                    let time = arr[0].clone();
                    let ev_type = arr[1].as_str().unwrap_or("o");
                    let data = arr[2].as_str().unwrap_or("");

                    let redacted_data = if ev_type == "o" || ev_type == "i" {
                        Self::redact_text(data)
                    } else {
                        data.to_string()
                    };

                    let sanitized_event = serde_json::json!([time, ev_type, redacted_data]);
                    let event_line = serde_json::to_string(&sanitized_event)
                        .map_err(|e| format!("Failed to serialize event: {e}"))?;
                    output.push_str(&event_line);
                    output.push('\n');
                } else {
                    output.push_str(trimmed);
                    output.push('\n');
                }
            } else {
                output.push_str(trimmed);
                output.push('\n');
            }
        }

        Ok(output)
    }
}

/// Redacts PEM private key blocks.
fn redact_private_keys(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remainder = text;

    let begin_marker = "-----BEGIN ";
    let end_marker = "-----END ";

    while let Some(start_idx) = remainder.find(begin_marker) {
        result.push_str(&remainder[..start_idx]);
        let after_begin = &remainder[start_idx..];

        // Check if this begins a PRIVATE KEY
        if let Some(header_end) = after_begin.find("-----") {
            let header_tail = &after_begin[header_end + 5..];
            if let Some(first_newline) = header_tail.find('\n') {
                let first_line = &after_begin[..header_end + 5 + first_newline];
                if first_line.contains("PRIVATE KEY") {
                    // Find the end marker
                    if let Some(end_idx) = after_begin.find(end_marker) {
                        let after_end = &after_begin[end_idx..];
                        if let Some(trailing_dashes) = after_end[9..].find("-----") {
                            let block_end = end_idx + 9 + trailing_dashes + 5;
                            result.push_str("[REDACTED_PRIVATE_KEY]");
                            remainder = &after_begin[block_end..];
                            continue;
                        }
                    }
                }
            }
        }

        // Not a matched private key, advance past marker
        result.push_str(begin_marker);
        remainder = &remainder[start_idx + begin_marker.len()..];
    }

    result.push_str(remainder);
    result
}

/// Redacts Bearer tokens (e.g. `Bearer eyJ...`).
fn redact_bearer_tokens(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remainder = text;
    let target = "Bearer ";

    while let Some(idx) = remainder.find(target) {
        result.push_str(&remainder[..idx]);
        result.push_str("Bearer ");
        let token_part = &remainder[idx + target.len()..];

        // Read token characters: alphanumeric, ., -, _
        let token_len = token_part
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
            .count();

        if token_len >= 12 {
            result.push_str("[REDACTED_TOKEN]");
            remainder = &token_part[token_len..];
        } else {
            result.push_str(&token_part[..token_len]);
            remainder = &token_part[token_len..];
        }
    }

    result.push_str(remainder);
    result
}

/// Redacts AWS Access Key IDs (`AKIA...` or `ASIA...`).
fn redact_aws_keys(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remainder = text;

    while let Some(pos) = remainder.find("AKIA").or_else(|| remainder.find("ASIA")) {
        result.push_str(&remainder[..pos]);
        let candidate = &remainder[pos..];
        if candidate.len() >= 20 {
            let key_str = &candidate[..20];
            let all_upper_or_digit = key_str[4..].chars().all(|c| c.is_ascii_alphanumeric());
            if all_upper_or_digit {
                result.push_str("[REDACTED_AWS_KEY]");
                remainder = &candidate[20..];
                continue;
            }
        }
        result.push_str(&candidate[..4]);
        remainder = &candidate[4..];
    }

    result.push_str(remainder);
    result
}

/// Redacts GitHub tokens (`ghp_...`) and GitLab tokens (`glpat-...`).
fn redact_git_tokens(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remainder = text;

    while let Some(pos) = remainder.find("ghp_").or_else(|| remainder.find("glpat-")) {
        result.push_str(&remainder[..pos]);
        let candidate = &remainder[pos..];
        if candidate.starts_with("ghp_") && candidate.len() >= 40 {
            let token_body = &candidate[4..40];
            if token_body.chars().all(|c| c.is_ascii_alphanumeric()) {
                result.push_str("[REDACTED_GIT_TOKEN]");
                remainder = &candidate[40..];
                continue;
            }
        } else if candidate.starts_with("glpat-") && candidate.len() >= 26 {
            let token_body = &candidate[6..26];
            if token_body
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                result.push_str("[REDACTED_GIT_TOKEN]");
                remainder = &candidate[26..];
                continue;
            }
        }

        result.push_str(&candidate[..4]);
        remainder = &candidate[4..];
    }

    result.push_str(remainder);
    result
}

/// Redacts common key-value sensitive parameters like `password=xyz`, `api_key: "..."`.
fn redact_assignments(text: &str) -> String {
    let keywords = [
        "password=",
        "password:",
        "passwd=",
        "passwd:",
        "api_key=",
        "api_key:",
        "apikey=",
        "apikey:",
        "secret=",
        "secret:",
        "token=",
        "token:",
        "secret_token=",
        "secret_token:",
        "client_secret=",
        "client_secret:",
        "auth_token=",
        "auth_token:",
    ];

    let mut out = text.to_string();

    for kw in keywords {
        let mut cur = 0;
        while let Some(found_rel) = out[cur..].to_lowercase().find(kw) {
            let start = cur + found_rel;
            let val_start = start + kw.len();

            // Skip leading whitespace or quotes
            let remaining = &out[val_start..];
            let mut val_offset = 0;
            let mut quote_char = None;

            for c in remaining.chars() {
                if c == ' ' || c == '\t' {
                    val_offset += c.len_utf8();
                } else if c == '"' || c == '\'' {
                    quote_char = Some(c);
                    val_offset += c.len_utf8();
                    break;
                } else {
                    break;
                }
            }

            let value_content = &remaining[val_offset..];
            let mut val_end = 0;

            if let Some(q) = quote_char {
                // Read until closing quote
                if let Some(close_q) = value_content.find(q) {
                    val_end = val_offset + close_q;
                } else {
                    val_end = remaining.len();
                }
            } else {
                // Read until whitespace, comma, semicolon, newline
                for c in value_content.chars() {
                    if c.is_whitespace()
                        || c == ','
                        || c == ';'
                        || c == '&'
                        || c == '\r'
                        || c == '\n'
                    {
                        break;
                    }
                    val_end += c.len_utf8();
                }
                val_end += val_offset;
            }

            if val_end > val_offset && (val_end - val_offset) > 0 {
                let prefix = &out[..val_start + val_offset];
                let suffix = &out[val_start + val_end..];
                out = format!("{prefix}[REDACTED_SECRET]{suffix}");
                cur = val_start + val_offset + "[REDACTED_SECRET]".len();
            } else {
                cur = val_start;
            }

            if cur >= out.len() {
                break;
            }
        }
    }

    out
}

/// Redacts 6-digit MFA / TOTP codes following common 2FA verification prompts.
fn redact_mfa_codes(text: &str) -> String {
    let triggers = [
        "verification code:",
        "mfa code:",
        "2fa code:",
        "totp code:",
        "code: ",
    ];
    let lower = text.to_lowercase();

    for trigger in triggers {
        if let Some(pos) = lower.find(trigger) {
            let after = &text[pos + trigger.len()..];
            let trimmed = after.trim_start();
            let leading_spaces = after.len() - trimmed.len();

            if trimmed.len() >= 6 {
                let code_part = &trimmed[..6];
                if code_part.chars().all(|c| c.is_ascii_digit()) {
                    let end_bound = if trimmed.len() > 6 {
                        let next_char = trimmed.chars().nth(6).unwrap();
                        next_char.is_whitespace() || next_char == '\r' || next_char == '\n'
                    } else {
                        true
                    };

                    if end_bound {
                        let prefix = &text[..pos + trigger.len() + leading_spaces];
                        let suffix = &trimmed[6..];
                        return format!("{prefix}[REDACTED_MFA]{suffix}");
                    }
                }
            }
        }
    }

    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_pem_private_keys() {
        let sample = "Connecting to host...\n-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAABlwAAAAdzc2gtcn\nNhAAAAAwEAAQAAAYEAv778=\n-----END OPENSSH PRIVATE KEY-----\nConnection established.";
        let redacted = SessionRedactor::redact_text(sample);
        assert!(!redacted.contains("b3BlbnNzaC"));
        assert!(redacted.contains("[REDACTED_PRIVATE_KEY]"));
        assert!(redacted.contains("Connecting to host..."));
        assert!(redacted.contains("Connection established."));
    }

    #[test]
    fn redacts_bearer_tokens_and_aws_and_github_keys() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9\nexport AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE\nexport GITHUB_TOKEN=ghp_1234567890abcdefghijklmnopqrstuvwxyz";
        let redacted = SessionRedactor::redact_text(input);
        assert!(!redacted.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
        assert!(redacted.contains("Bearer [REDACTED_TOKEN]"));
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(redacted.contains("[REDACTED_AWS_KEY]"));
        assert!(!redacted.contains("ghp_1234567890abcdefghijklmnopqrstuvwxyz"));
        assert!(redacted.contains("[REDACTED_GIT_TOKEN]"));
    }

    #[test]
    fn redacts_password_and_secret_assignments() {
        let input = "curl -u admin:secret123 https://example.com/api?password=SuperSecretPassword&other=ok\nDB_PASS=\"MyPassphrase#99\"\nAPI_KEY: 'sk_live_998877665544'";
        let redacted = SessionRedactor::redact_text(input);
        assert!(!redacted.contains("SuperSecretPassword"));
        assert!(redacted.contains("password=[REDACTED_SECRET]&other=ok"));
        assert!(!redacted.contains("sk_live_998877665544"));
        assert!(redacted.contains("API_KEY: '[REDACTED_SECRET]'"));
    }

    #[test]
    fn redacts_mfa_codes_following_prompts() {
        let input = "Enter verification code: 654321\r\nAccess granted.";
        let redacted = SessionRedactor::redact_text(input);
        assert!(!redacted.contains("654321"));
        assert!(redacted.contains("Enter verification code: [REDACTED_MFA]\r\nAccess granted."));
    }

    #[test]
    fn redacts_full_asciicast_v2_stream() {
        let cast = "{\"version\":2,\"width\":80,\"height\":24,\"title\":\"Auth Test\"}\n[0.1, \"o\", \"Enter verification code: 123456\\r\\n\"]\n[0.5, \"i\", \"export SECRET=my_secret_token\\r\"]\n[1.0, \"r\", \"100x30\"]\n";
        let redacted = SessionRedactor::redact_asciicast_v2(cast).expect("should redact");
        assert!(redacted.contains("\"version\":2"));
        assert!(redacted.contains("[REDACTED_MFA]"));
        assert!(!redacted.contains("123456"));
        assert!(redacted.contains("[REDACTED_SECRET]"));
        assert!(!redacted.contains("my_secret_token"));
        assert!(redacted.contains("\"100x30\"")); // Resize event preserved
    }

    #[test]
    fn playback_engine_advances_and_seeks_accurately() {
        let cast = "{\"version\":2,\"width\":80,\"height\":24,\"duration\":5.0}\n[1.0, \"o\", \"frame 1\"]\n[2.0, \"o\", \"frame 2\"]\n[3.0, \"o\", \"frame 3\"]\n[4.0, \"o\", \"frame 4\"]\n";
        let mut engine = CastPlaybackEngine::from_asciicast_v2(cast).expect("valid cast");

        assert_eq!(engine.status(), PlaybackStatus::Stopped);
        assert_eq!(engine.duration_secs(), 5.0);
        assert_eq!(engine.formatted_time(), "00:00 / 00:05");

        engine.play();
        assert_eq!(engine.status(), PlaybackStatus::Playing);

        // Advance 1.5 seconds (at 1x speed)
        let frames1 = engine.advance(1.5);
        assert_eq!(frames1.len(), 1);
        assert_eq!(frames1[0].data, "frame 1");
        assert_eq!(engine.current_time_secs(), 1.5);

        // Advance another 1.0 second (to 2.5s)
        let frames2 = engine.advance(1.0);
        assert_eq!(frames2.len(), 1);
        assert_eq!(frames2[0].data, "frame 2");
        assert_eq!(engine.current_time_secs(), 2.5);

        // Toggle pause
        engine.toggle_play_pause();
        assert_eq!(engine.status(), PlaybackStatus::Paused);
        let paused_frames = engine.advance(1.0);
        assert!(paused_frames.is_empty());

        // Resume play at 2.0x speed
        engine.set_speed(PlaybackSpeed::Double);
        engine.play();
        assert_eq!(engine.speed().multiplier(), 2.0);

        // Advance 1.0 real second -> 2.0 scaled seconds (from 2.5s to 4.5s)
        let fast_frames = engine.advance(1.0);
        assert_eq!(fast_frames.len(), 2);
        assert_eq!(fast_frames[0].data, "frame 3");
        assert_eq!(fast_frames[1].data, "frame 4");
        assert_eq!(engine.current_time_secs(), 4.5);

        // Seek backward to 2.0s
        let cumulative = engine.seek(2.0);
        assert_eq!(cumulative.len(), 2);
        assert_eq!(engine.current_time_secs(), 2.0);
        assert_eq!(engine.progress_fraction(), 0.4);

        // Advance to end
        let end_frames = engine.advance(2.0);
        assert_eq!(end_frames.len(), 2);
        assert_eq!(engine.status(), PlaybackStatus::Completed);
    }
}

//! Terminal session audit recording models, sensitive data redaction, and cast playback engine.
//!
//! Provides decoupled domain structures for session recording metadata, automated
//! redaction of classified secrets (private keys, passwords, bearer tokens, API keys, MFA codes),
//! and a frame-by-frame Asciicast playback state machine supporting variable speed playback and seeking.

use crate::sync::EncryptedEnvelope;
use crate::vault::VaultId;
use crate::vault_cipher::VaultCipher;
use crate::vault_key::VaultKey;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Lifecycle state of an active or finalizing session recording.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RecordingLifecycleState {
    #[default]
    Idle,
    Recording,
    Paused,
    Finalizing,
    Saved {
        recording_id: String,
    },
    FailedRecoverable {
        recording_id: String,
        error: String,
    },
}

/// Formats a `SystemTime` as an RFC-3339 UTC timestamp string (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn format_rfc3339_utc(time: SystemTime) -> String {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let second = rem % 60;

    // Howard Hinnant's civil-from-days algorithm
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!("{year:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Encrypted crash-recovery checkpoint for an active or failed-to-commit recording.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordingCheckpoint {
    pub recording_id: String,
    pub vault_id: String,
    pub session_id: String,
    pub pane_id: u64,
    pub host_id: Option<i64>,
    pub protocol: String,
    pub title: String,
    pub started_at: String,
    pub duration_secs: f64,
    pub event_count: usize,
    pub byte_size: usize,
    pub cast_data: String,
    pub truncated: bool,
}

const MAX_RECOVERY_CHECKPOINT_BYTES: usize = 64 * 1024 * 1024;
pub const JOURNAL_SCHEMA_VERSION: u32 = 1;

/// A single event stored in an incremental encrypted recording journal chunk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JournalCastEvent {
    pub time_offset_secs: f64,
    pub event_type: String,
    pub data: String,
}

/// Authenticated manifest for an incremental crash-safe recording journal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordingJournalManifest {
    pub schema_version: u32,
    pub recording_id: String,
    pub vault_id: String,
    pub session_id: String,
    pub pane_id: u64,
    pub host_id: Option<i64>,
    pub protocol: String,
    pub title: String,
    pub started_at: String,
    pub width: usize,
    pub height: usize,
    pub last_durable_seq: u64,
    pub duration_secs: f64,
    pub event_count: usize,
    pub byte_size: usize,
    pub truncated: bool,
    pub paused: bool,
    pub finalized: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct JournalChunkPayload {
    seq: u64,
    events: Vec<JournalCastEvent>,
}

/// Encrypted on-disk recovery journal for active and recoverable session recordings.
/// Never writes plaintext cast data or secret material to disk.
pub struct RecordingRecoveryJournal {
    dir: PathBuf,
}

impl RecordingRecoveryJournal {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn validate_id(id: &str) -> Result<&str, String> {
        let clean = id.trim();
        if clean.is_empty()
            || clean.starts_with('.')
            || clean.contains('/')
            || clean.contains('\\')
            || clean.contains("..")
        {
            return Err("Invalid identifier for recovery journal".to_string());
        }
        Ok(clean)
    }

    fn vault_dir(&self, vault_id: &VaultId) -> Result<PathBuf, String> {
        let clean_v = Self::validate_id(vault_id.as_str())?;
        Ok(self.dir.join(clean_v))
    }

    fn recording_dir(&self, vault_id: &VaultId, recording_id: &str) -> Result<PathBuf, String> {
        let clean_r = Self::validate_id(recording_id)?;
        Ok(self.vault_dir(vault_id)?.join(clean_r))
    }

    fn ensure_owner_dir(path: &Path) -> Result<(), String> {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create recovery journal directory: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
        }
        Ok(())
    }

    /// Initializes an incremental encrypted journal for a newly started recording.
    pub fn init_incremental_journal(
        &self,
        manifest: &RecordingJournalManifest,
        vault_id: &VaultId,
        vault_key: &VaultKey,
    ) -> Result<(), String> {
        if manifest.vault_id != vault_id.as_str() {
            return Err("Vault ID mismatch initializing recording journal".to_string());
        }
        let rec_dir = self.recording_dir(vault_id, &manifest.recording_id)?;
        Self::ensure_owner_dir(&rec_dir)?;
        self.write_manifest_atomic(&rec_dir, manifest, vault_id, vault_key)
    }

    fn write_manifest_atomic(
        &self,
        rec_dir: &Path,
        manifest: &RecordingJournalManifest,
        vault_id: &VaultId,
        vault_key: &VaultKey,
    ) -> Result<(), String> {
        let plaintext = serde_json::to_vec(manifest)
            .map_err(|e| format!("Failed to serialize journal manifest: {e}"))?;
        let cipher = VaultCipher::new(vault_key, vault_id);
        let tag = format!(
            "recording_manifest:v{}:{}:{}",
            JOURNAL_SCHEMA_VERSION,
            vault_id.as_str(),
            manifest.recording_id
        );
        let envelope = cipher
            .encrypt_record(&tag, &plaintext)
            .map_err(|e| format!("Failed to encrypt journal manifest: {e}"))?;
        let encoded = serde_json::to_vec(&envelope)
            .map_err(|e| format!("Failed to encode journal manifest envelope: {e}"))?;
        let dest = rec_dir.join("manifest.enc");
        let tmp = rec_dir.join(format!(".manifest.{}.tmp", std::process::id()));
        write_restricted_atomic(&tmp, &dest, &encoded)
    }

    /// Appends an encrypted chunk of events and atomically updates the journal manifest.
    pub fn append_journal_chunk(
        &self,
        manifest: &RecordingJournalManifest,
        seq: u64,
        events: &[JournalCastEvent],
        vault_id: &VaultId,
        vault_key: &VaultKey,
    ) -> Result<(), String> {
        if manifest.vault_id != vault_id.as_str() {
            return Err("Vault ID mismatch appending journal chunk".to_string());
        }
        if manifest.byte_size > MAX_RECOVERY_CHECKPOINT_BYTES {
            return Err("Recording journal exceeded maximum size bound".to_string());
        }
        let rec_dir = self.recording_dir(vault_id, &manifest.recording_id)?;
        Self::ensure_owner_dir(&rec_dir)?;

        if !events.is_empty() {
            let chunk = JournalChunkPayload {
                seq,
                events: events.to_vec(),
            };
            let plaintext = serde_json::to_vec(&chunk)
                .map_err(|e| format!("Failed to serialize journal chunk: {e}"))?;
            let cipher = VaultCipher::new(vault_key, vault_id);
            let tag = format!(
                "recording_chunk:v{}:{}:{}:{}",
                JOURNAL_SCHEMA_VERSION,
                vault_id.as_str(),
                manifest.recording_id,
                seq
            );
            let envelope = cipher
                .encrypt_record(&tag, &plaintext)
                .map_err(|e| format!("Failed to encrypt journal chunk: {e}"))?;
            let encoded = serde_json::to_vec(&envelope)
                .map_err(|e| format!("Failed to encode journal chunk envelope: {e}"))?;
            let dest = rec_dir.join(format!("{seq:08}.chunk.enc"));
            let tmp = rec_dir.join(format!(".{seq:08}.{}.tmp", std::process::id()));
            write_restricted_atomic(&tmp, &dest, &encoded)?;
        }

        self.write_manifest_atomic(&rec_dir, manifest, vault_id, vault_key)
    }

    /// Lists recording IDs that have pending incremental journals in `vault_id` ONLY.
    pub fn list_pending_journals_for_vault(&self, vault_id: &VaultId) -> Vec<String> {
        let Ok(vdir) = self.vault_dir(vault_id) else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(&vdir) else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !name.starts_with('.') && path.join("manifest.enc").exists() {
                        ids.push(name.to_string());
                    }
                }
            }
        }
        ids.sort();
        ids
    }

    /// Loads and authenticates an incremental journal for `vault_id`.
    /// Never deletes corrupt or unknown-version journals; returns `Err` so they remain quarantined.
    pub fn load_incremental_journal(
        &self,
        recording_id: &str,
        vault_id: &VaultId,
        vault_key: &VaultKey,
    ) -> Result<(RecordingJournalManifest, Vec<JournalCastEvent>), String> {
        let rec_dir = self.recording_dir(vault_id, recording_id)?;
        let manifest_path = rec_dir.join("manifest.enc");
        let manifest_bytes = std::fs::read(&manifest_path)
            .map_err(|e| format!("Failed to read journal manifest: {e}"))?;
        if manifest_bytes.len() > 1024 * 1024 {
            return Err("Journal manifest exceeds maximum size".to_string());
        }
        let manifest_env: EncryptedEnvelope = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| format!("Corrupt journal manifest envelope: {e}"))?;
        let cipher = VaultCipher::new(vault_key, vault_id);
        let manifest_tag = format!(
            "recording_manifest:v{}:{}:{}",
            JOURNAL_SCHEMA_VERSION,
            vault_id.as_str(),
            recording_id
        );
        let manifest_plain = cipher
            .decrypt_record(&manifest_tag, &manifest_env)
            .map_err(|e| format!("Failed to decrypt journal manifest: {e}"))?;
        let manifest: RecordingJournalManifest = serde_json::from_slice(&manifest_plain)
            .map_err(|e| format!("Corrupt journal manifest payload: {e}"))?;

        if manifest.schema_version != JOURNAL_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported journal schema version {}",
                manifest.schema_version
            ));
        }
        if manifest.vault_id != vault_id.as_str() || manifest.recording_id != recording_id {
            return Err("Journal manifest identity mismatch".to_string());
        }

        let mut all_events = Vec::new();
        let mut total_bytes = 0usize;
        for seq in 1..=manifest.last_durable_seq {
            let chunk_path = rec_dir.join(format!("{seq:08}.chunk.enc"));
            let chunk_bytes = std::fs::read(&chunk_path)
                .map_err(|e| format!("Missing or unreadable journal chunk {seq}: {e}"))?;
            total_bytes = total_bytes.saturating_add(chunk_bytes.len());
            if total_bytes > MAX_RECOVERY_CHECKPOINT_BYTES * 2 {
                return Err("Journal chunks exceed maximum recovery size".to_string());
            }
            let chunk_env: EncryptedEnvelope = serde_json::from_slice(&chunk_bytes)
                .map_err(|e| format!("Corrupt journal chunk {seq} envelope: {e}"))?;
            let chunk_tag = format!(
                "recording_chunk:v{}:{}:{}:{}",
                JOURNAL_SCHEMA_VERSION,
                vault_id.as_str(),
                recording_id,
                seq
            );
            let chunk_plain = cipher
                .decrypt_record(&chunk_tag, &chunk_env)
                .map_err(|e| format!("Failed to decrypt journal chunk {seq}: {e}"))?;
            let chunk: JournalChunkPayload = serde_json::from_slice(&chunk_plain)
                .map_err(|e| format!("Corrupt journal chunk {seq} payload: {e}"))?;
            if chunk.seq != seq {
                return Err(format!(
                    "Journal chunk sequence mismatch: expected {seq}, got {}",
                    chunk.seq
                ));
            }
            all_events.extend(chunk.events);
        }

        Ok((manifest, all_events))
    }

    /// Reconstructs an Asciicast v2 string from a recovered manifest and its events.
    pub fn build_asciicast_v2(
        manifest: &RecordingJournalManifest,
        events: &[JournalCastEvent],
    ) -> Result<String, String> {
        let mut out = String::new();
        let header = serde_json::json!({
            "version": 2,
            "width": manifest.width.max(1),
            "height": manifest.height.max(1),
            "title": manifest.title,
            "duration": manifest.duration_secs,
        });
        out.push_str(
            &serde_json::to_string(&header)
                .map_err(|e| format!("Failed to serialize recovered header: {e}"))?,
        );
        out.push('\n');
        for ev in events {
            let t = (ev.time_offset_secs * 1000.0).round() / 1000.0;
            let row = serde_json::json!([t, ev.event_type, ev.data]);
            out.push_str(
                &serde_json::to_string(&row)
                    .map_err(|e| format!("Failed to serialize recovered event: {e}"))?,
            );
            out.push('\n');
        }
        Ok(out)
    }

    /// Removes an incremental journal after DB commit verification.
    pub fn remove_incremental_journal(
        &self,
        recording_id: &str,
        vault_id: &VaultId,
    ) -> Result<(), String> {
        let rec_dir = self.recording_dir(vault_id, recording_id)?;
        if rec_dir.exists() {
            std::fs::remove_dir_all(&rec_dir)
                .map_err(|e| format!("Failed to remove incremental journal: {e}"))?;
        }
        Ok(())
    }

    fn checkpoint_path(&self, recording_id: &str) -> Result<PathBuf, String> {
        let clean = recording_id.trim();
        if clean.is_empty() || clean.contains('/') || clean.contains('\\') || clean.contains("..") {
            return Err("Invalid recording ID for recovery checkpoint".to_string());
        }
        Ok(self.dir.join(format!("{clean}.rec.enc")))
    }

    pub fn save_checkpoint(
        &self,
        checkpoint: &RecordingCheckpoint,
        vault_id: &VaultId,
        vault_key: &VaultKey,
    ) -> Result<PathBuf, String> {
        if checkpoint.vault_id != vault_id.as_str() {
            return Err("Vault ID mismatch when saving recovery checkpoint".to_string());
        }
        let plaintext = serde_json::to_vec(checkpoint)
            .map_err(|e| format!("Failed to serialize recovery checkpoint: {e}"))?;
        if plaintext.len() > MAX_RECOVERY_CHECKPOINT_BYTES {
            return Err("Recovery checkpoint exceeds maximum allowed size".to_string());
        }

        let cipher = VaultCipher::new(vault_key, vault_id);
        let entity_tag = format!("recording_recovery:{}", checkpoint.recording_id);
        let envelope = cipher
            .encrypt_record(&entity_tag, &plaintext)
            .map_err(|e| format!("Failed to encrypt recovery checkpoint: {e}"))?;
        let encoded = serde_json::to_vec(&envelope)
            .map_err(|e| format!("Failed to encode encrypted envelope: {e}"))?;

        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("Failed to create recovery journal directory: {e}"))?;
        let dest = self.checkpoint_path(&checkpoint.recording_id)?;
        let tmp = self.dir.join(format!(
            ".{}.{}.tmp",
            checkpoint.recording_id,
            std::process::id()
        ));
        write_restricted_atomic(&tmp, &dest, &encoded)?;
        Ok(dest)
    }

    pub fn load_checkpoint(
        &self,
        recording_id: &str,
        vault_id: &VaultId,
        vault_key: &VaultKey,
    ) -> Result<RecordingCheckpoint, String> {
        let path = self.checkpoint_path(recording_id)?;
        let bytes =
            std::fs::read(&path).map_err(|e| format!("Failed to read recovery checkpoint: {e}"))?;
        if bytes.len() > MAX_RECOVERY_CHECKPOINT_BYTES * 2 {
            return Err("Encrypted recovery checkpoint exceeds size limit".to_string());
        }
        let envelope: EncryptedEnvelope = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Corrupt recovery checkpoint envelope: {e}"))?;
        let cipher = VaultCipher::new(vault_key, vault_id);
        let entity_tag = format!("recording_recovery:{recording_id}");
        let plaintext = cipher
            .decrypt_record(&entity_tag, &envelope)
            .map_err(|e| format!("Failed to decrypt recovery checkpoint: {e}"))?;
        let checkpoint: RecordingCheckpoint = serde_json::from_slice(&plaintext)
            .map_err(|e| format!("Corrupt recovery checkpoint payload: {e}"))?;
        if checkpoint.vault_id != vault_id.as_str() {
            return Err("Recovery checkpoint vault mismatch".to_string());
        }
        Ok(checkpoint)
    }

    pub fn list_checkpoint_ids(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(id) = name.strip_suffix(".rec.enc") {
                    if !id.starts_with('.') {
                        ids.push(id.to_string());
                    }
                }
            }
        }
        ids
    }

    pub fn remove_checkpoint(&self, recording_id: &str) -> Result<(), String> {
        let path = self.checkpoint_path(recording_id)?;
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove recovery checkpoint: {e}"))?;
        }
        Ok(())
    }
}

fn write_restricted_atomic(tmp: &Path, dest: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(tmp)
            .map_err(|e| format!("Failed to open temp checkpoint: {e}"))?;
        f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|e| {
            let _ = std::fs::remove_file(tmp);
            format!("Failed to write temp checkpoint: {e}")
        })?;
    }
    #[cfg(not(unix))]
    {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(tmp)
            .map_err(|e| format!("Failed to open temp checkpoint: {e}"))?;
        f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|e| {
            let _ = std::fs::remove_file(tmp);
            format!("Failed to write temp checkpoint: {e}")
        })?;
    }
    std::fs::rename(tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(tmp);
        format!("Failed to commit recovery checkpoint: {e}")
    })?;
    Ok(())
}

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
    /// Also scans across adjacent `"o"` or `"i"` event boundaries so secrets
    /// split across chunks are redacted.
    pub fn redact_asciicast_v2(cast_data: &str) -> Result<String, String> {
        let mut lines = cast_data.lines();
        let header_line = lines.next().ok_or_else(|| "Empty cast data".to_string())?;

        let header_val: serde_json::Value = serde_json::from_str(header_line)
            .map_err(|e| format!("Invalid asciicast header: {e}"))?;
        if header_val.get("version").and_then(|v| v.as_u64()) != Some(2) {
            return Err("Unsupported or missing asciicast v2 version".to_string());
        }

        let mut parsed_events: Vec<(serde_json::Value, String, String)> = Vec::new();
        for (idx, line) in lines.enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let val: serde_json::Value = serde_json::from_str(trimmed)
                .map_err(|e| format!("Invalid cast event on line {}: {e}", idx + 2))?;
            let arr = val
                .as_array()
                .ok_or_else(|| format!("Cast event on line {} is not an array", idx + 2))?;
            if arr.len() < 3 {
                return Err(format!(
                    "Cast event on line {} has fewer than 3 items",
                    idx + 2
                ));
            }
            let time = arr[0].clone();
            let ev_type = arr[1]
                .as_str()
                .ok_or_else(|| format!("Cast event type on line {} is not a string", idx + 2))?
                .to_string();
            let data = arr[2]
                .as_str()
                .ok_or_else(|| format!("Cast event data on line {} is not a string", idx + 2))?
                .to_string();
            parsed_events.push((time, ev_type, data));
        }

        // Chunk-aware redaction across consecutive same-stream ("o" or "i") events:
        // Coalesce contiguous same-type text runs, redact the joined run, and redistribute
        // or emit on the first event of the run with empty strings on subsequent merged chunks
        // only when a secret actually spanned a boundary; otherwise preserve per-chunk boundaries.
        let mut i = 0;
        while i < parsed_events.len() {
            let stream = parsed_events[i].1.clone();
            if stream != "o" && stream != "i" {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < parsed_events.len() && parsed_events[j].1 == stream {
                j += 1;
            }
            if j - i == 1 {
                parsed_events[i].2 = Self::redact_text(&parsed_events[i].2);
            } else {
                let mut combined = String::new();
                let mut offsets = Vec::with_capacity(j - i);
                for ev in &parsed_events[i..j] {
                    let start = combined.len();
                    combined.push_str(&ev.2);
                    offsets.push((start, combined.len()));
                }
                let redacted_combined = Self::redact_text(&combined);
                // Check if per-chunk redaction matches combined redaction
                let per_chunk: String = parsed_events[i..j]
                    .iter()
                    .map(|ev| Self::redact_text(&ev.2))
                    .collect();
                if per_chunk == redacted_combined {
                    for ev in &mut parsed_events[i..j] {
                        ev.2 = Self::redact_text(&ev.2);
                    }
                } else {
                    // A secret spanned a chunk boundary: store redacted combined in first chunk
                    // and clear continuation chunks to avoid leaking half of the secret.
                    parsed_events[i].2 = redacted_combined;
                    for ev in &mut parsed_events[(i + 1)..j] {
                        ev.2.clear();
                    }
                }
            }
            i = j;
        }

        let mut output = String::with_capacity(cast_data.len());
        output.push_str(header_line);
        output.push('\n');
        for (time, ev_type, data) in parsed_events {
            let sanitized_event = serde_json::json!([time, ev_type, data]);
            let event_line = serde_json::to_string(&sanitized_event)
                .map_err(|e| format!("Failed to serialize event: {e}"))?;
            output.push_str(&event_line);
            output.push('\n');
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

            let val_slice = &remaining[val_offset..val_end];
            if val_slice.starts_with("[REDACTED_") {
                cur = val_start + val_end;
                continue;
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

    #[test]
    fn redacts_secrets_split_across_asciicast_chunks() {
        let split_cast = "{\"version\":2,\"width\":80,\"height\":24}\n[0.1, \"o\", \"token=ghp_\"]\n[0.2, \"o\", \"1234567890abcdef1234567890abcdef1234\\r\\n\"]\n";
        let redacted = SessionRedactor::redact_asciicast_v2(split_cast).expect("should redact");
        assert!(!redacted.contains("ghp_1234567890abcdef"));
        assert!(
            redacted.contains("[REDACTED_GIT_TOKEN]") || redacted.contains("[REDACTED_SECRET]")
        );
    }

    #[test]
    fn encrypted_recovery_journal_roundtrips_and_rejects_wrong_key_or_tamper() {
        let tmp = tempfile::tempdir().unwrap();
        let journal = RecordingRecoveryJournal::new(tmp.path());
        let vault_id = VaultId::new("vault-a").unwrap();
        let other_vault = VaultId::new("vault-b").unwrap();
        let key = VaultKey::from_bytes([0x42; 32]);
        let wrong_key = VaultKey::from_bytes([0x99; 32]);

        let cp = RecordingCheckpoint {
            recording_id: "11111111-2222-4333-8444-555555555555".into(),
            vault_id: vault_id.as_str().to_string(),
            session_id: "sess-1".into(),
            pane_id: 7,
            host_id: Some(42),
            protocol: "ssh".into(),
            title: "prod".into(),
            started_at: format_rfc3339_utc(SystemTime::now()),
            duration_secs: 1.5,
            event_count: 2,
            byte_size: 48,
            cast_data: "{\"version\":2,\"width\":80,\"height\":24}\n[0.1,\"o\",\"UNIQUE_PLAINTEXT_SECRET_12345\"]\n".into(),
            truncated: false,
        };

        let path = journal.save_checkpoint(&cp, &vault_id, &key).unwrap();
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains("prod"));
        assert!(!on_disk.contains("UNIQUE_PLAINTEXT_SECRET_12345"));

        let loaded = journal
            .load_checkpoint(&cp.recording_id, &vault_id, &key)
            .unwrap();
        assert_eq!(loaded, cp);

        // Wrong key and wrong vault must fail closed
        assert!(journal
            .load_checkpoint(&cp.recording_id, &vault_id, &wrong_key)
            .is_err());
        assert!(journal
            .load_checkpoint(&cp.recording_id, &other_vault, &key)
            .is_err());

        // Corrupt journal must fail closed
        std::fs::write(&path, b"{corrupt").unwrap();
        assert!(journal
            .load_checkpoint(&cp.recording_id, &vault_id, &key)
            .is_err());
    }

    #[test]
    fn incremental_journal_chunks_recover_across_restart_and_quarantine_corrupt() {
        let tmp = tempfile::tempdir().unwrap();
        let journal = RecordingRecoveryJournal::new(tmp.path());
        let vault_a = VaultId::new("vault-alpha").unwrap();
        let vault_b = VaultId::new("vault-beta").unwrap();
        let key_a = VaultKey::from_bytes([0x11; 32]);
        let key_b = VaultKey::from_bytes([0x22; 32]);
        let rec_id = "22222222-3333-4444-8555-666666666666".to_string();

        let mut manifest = RecordingJournalManifest {
            schema_version: JOURNAL_SCHEMA_VERSION,
            recording_id: rec_id.clone(),
            vault_id: vault_a.as_str().to_string(),
            session_id: "sess-a".into(),
            pane_id: 1,
            host_id: Some(9),
            protocol: "Ssh".into(),
            title: "Alpha Prod".into(),
            started_at: format_rfc3339_utc(SystemTime::now()),
            width: 80,
            height: 24,
            last_durable_seq: 0,
            duration_secs: 0.0,
            event_count: 0,
            byte_size: 0,
            truncated: false,
            paused: false,
            finalized: false,
        };
        journal
            .init_incremental_journal(&manifest, &vault_a, &key_a)
            .unwrap();

        manifest.last_durable_seq = 1;
        manifest.event_count = 1;
        manifest.byte_size = 12;
        manifest.duration_secs = 0.5;
        journal
            .append_journal_chunk(
                &manifest,
                1,
                &[JournalCastEvent {
                    time_offset_secs: 0.5,
                    event_type: "o".into(),
                    data: "UNIQUE_CHUNK_1".into(),
                }],
                &vault_a,
                &key_a,
            )
            .unwrap();

        manifest.last_durable_seq = 2;
        manifest.event_count = 2;
        manifest.byte_size = 24;
        manifest.duration_secs = 1.2;
        journal
            .append_journal_chunk(
                &manifest,
                2,
                &[JournalCastEvent {
                    time_offset_secs: 1.2,
                    event_type: "r".into(),
                    data: "100x30".into(),
                }],
                &vault_a,
                &key_a,
            )
            .unwrap();

        // Vault B must NOT see Vault A's pending journals
        assert!(journal.list_pending_journals_for_vault(&vault_b).is_empty());
        assert_eq!(
            journal.list_pending_journals_for_vault(&vault_a),
            vec![rec_id.clone()]
        );

        // Wrong key fails closed
        assert!(journal
            .load_incremental_journal(&rec_id, &vault_a, &key_b)
            .is_err());

        // Correct vault & key recovers all durable chunks in order
        let (rec_m, rec_evs) = journal
            .load_incremental_journal(&rec_id, &vault_a, &key_a)
            .unwrap();
        assert_eq!(rec_m.last_durable_seq, 2);
        assert_eq!(rec_evs.len(), 2);
        assert_eq!(rec_evs[0].data, "UNIQUE_CHUNK_1");
        assert_eq!(rec_evs[1].data, "100x30");

        // Corrupt chunk 2 -> fails closed AND does NOT delete the journal
        let chunk2_path = tmp
            .path()
            .join(vault_a.as_str())
            .join(&rec_id)
            .join("00000002.chunk.enc");
        std::fs::write(&chunk2_path, b"tampered").unwrap();
        assert!(journal
            .load_incremental_journal(&rec_id, &vault_a, &key_a)
            .is_err());
        assert!(
            chunk2_path.exists(),
            "Corrupt journal must remain quarantined"
        );
    }
}

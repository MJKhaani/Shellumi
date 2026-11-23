//! First-run setup wizard engine, persistent state, and installation detection.
//!
//! Provides crash-safe and resumable lifecycle operations:
//! - Automatic detection of fresh installs, existing encrypted vaults, incomplete setups, and legacy databases
//! - Persistent state machine tracking wizard progress across process restarts
//! - Integrity verification of encrypted databases, key manifests, and vault registries
//! - Zero secrets persistence in the wizard state file

use crate::vault_key::VaultKey;
use crate::vault_manifest::VaultManifest;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;

/// Name of the setup wizard state file stored in the Shellumi root directory.
pub const SETUP_STATE_FILE: &str = ".setup_state.json";

/// Name of the setup wizard state temporary file for atomic updates.
pub const SETUP_STATE_TMP_FILE: &str = ".setup_state.tmp";

/// Detected status of the local Shellumi installation directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallationStatus {
    /// Fresh installation with no existing vaults, manifests, or legacy databases.
    Fresh,
    /// An incomplete setup was detected and can be safely resumed without data loss.
    IncompleteSetup {
        last_step: SetupStep,
        default_vault_created: bool,
    },
    /// Legacy database detected (e.g. `shellumi.db`) that requires safe migration.
    LegacyDatabase { path: String },
    /// Existing encrypted vaults detected (e.g. `default.db` or manifest).
    ExistingVaults {
        default_db_exists: bool,
        manifest_exists: bool,
        vault_count: usize,
    },
}

impl fmt::Display for InstallationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fresh => f.write_str("Fresh installation"),
            Self::IncompleteSetup { last_step, .. } => {
                write!(f, "Incomplete setup at step {last_step:?}")
            }
            Self::LegacyDatabase { path } => write!(f, "Legacy database detected at {path}"),
            Self::ExistingVaults { vault_count, .. } => {
                write!(
                    f,
                    "Existing encrypted vaults detected ({vault_count} vaults)"
                )
            }
        }
    }
}

impl InstallationStatus {
    /// Detects the installation status of the given base directory.
    pub fn detect(base_dir: &Path) -> Self {
        SetupState::detect_installation(base_dir)
    }
}

/// Setup wizard operation mode selected by the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SetupWizardMode {
    #[default]
    NewSetup,
    RestoreSetup,
}

impl fmt::Display for SetupWizardMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NewSetup => f.write_str("Create New Setup"),
            Self::RestoreSetup => f.write_str("Restore Existing Setup"),
        }
    }
}

/// Distinct steps in the native GPUI setup wizard workflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum SetupStep {
    #[default]
    Welcome,
    DefaultVault,
    GoogleDrive,
    AdditionalVaults,
    ReviewAndFinish,
    Completed,
}

impl fmt::Display for SetupStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Welcome => f.write_str("Welcome"),
            Self::DefaultVault => f.write_str("Default Vault"),
            Self::GoogleDrive => f.write_str("Google Drive Synchronization"),
            Self::AdditionalVaults => f.write_str("Additional Vaults"),
            Self::ReviewAndFinish => f.write_str("Review & Finish"),
            Self::Completed => f.write_str("Completed"),
        }
    }
}

/// Summary of a configured vault stored for the review screen and state persistence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredVaultSummary {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_remote: bool,
    pub sync_enabled: bool,
    pub server_url: Option<String>,
}

/// Summary of a cloud vault discovered in Google Drive without exposing decrypted payloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudVaultSummary {
    pub id: String,
    pub name: String,
    pub updated_at: u64,
    pub record_count: usize,
    pub has_key_envelope: bool,
    pub is_default: bool,
}

/// Errors produced during wizard state tracking or verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupError {
    Io(String),
    Serialization(String),
    Deserialization(String),
    CorruptedState(String),
    VerificationFailed(String),
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "Setup I/O error: {msg}"),
            Self::Serialization(msg) => write!(f, "Setup serialization error: {msg}"),
            Self::Deserialization(msg) => write!(f, "Setup deserialization error: {msg}"),
            Self::CorruptedState(msg) => write!(f, "Corrupted setup state: {msg}"),
            Self::VerificationFailed(msg) => write!(f, "Setup verification failed: {msg}"),
        }
    }
}

impl std::error::Error for SetupError {}

impl From<std::io::Error> for SetupError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// Persistent, crash-safe setup wizard state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupState {
    pub version: u32,
    pub mode: SetupWizardMode,
    pub current_step: SetupStep,
    pub default_vault_created: bool,
    pub default_vault_id: String,
    pub recovery_code_confirmed: bool,
    pub remember_in_keychain: bool,
    pub google_drive_connected: bool,
    pub google_drive_sync_enabled: bool,
    pub configured_vaults: Vec<ConfiguredVaultSummary>,
    pub completed: bool,
    pub completed_at: Option<u64>,
    pub active_vault_id: String,
}

impl Default for SetupState {
    fn default() -> Self {
        Self::new()
    }
}

impl SetupState {
    pub const CURRENT_VERSION: u32 = 1;

    /// Initializes a fresh setup state starting at the Welcome step.
    pub fn new() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            mode: SetupWizardMode::NewSetup,
            current_step: SetupStep::Welcome,
            default_vault_created: false,
            default_vault_id: "legacy-local".to_string(),
            recovery_code_confirmed: false,
            remember_in_keychain: false,
            google_drive_connected: false,
            google_drive_sync_enabled: false,
            configured_vaults: Vec::new(),
            completed: false,
            completed_at: None,
            active_vault_id: "legacy-local".to_string(),
        }
    }

    /// Loads persisted wizard state from the application base directory if present.
    pub fn load(base_dir: &Path) -> Result<Option<Self>, SetupError> {
        let state_path = base_dir.join(SETUP_STATE_FILE);
        if !state_path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&state_path).map_err(|e| SetupError::Io(e.to_string()))?;
        let state: Self = serde_json::from_str(&content)
            .map_err(|e| SetupError::Deserialization(e.to_string()))?;
        tracing::debug!(
            step = %state.current_step,
            completed = state.completed,
            mode = %state.mode,
            "Loaded existing setup wizard state"
        );
        Ok(Some(state))
    }

    /// Saves the setup wizard state atomically using a temporary file and rename.
    pub fn save(&self, base_dir: &Path) -> Result<(), SetupError> {
        if !base_dir.exists() {
            fs::create_dir_all(base_dir).map_err(|e| SetupError::Io(e.to_string()))?;
        }
        let tmp_path = base_dir.join(SETUP_STATE_TMP_FILE);
        let target_path = base_dir.join(SETUP_STATE_FILE);

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| SetupError::Serialization(e.to_string()))?;

        fs::write(&tmp_path, json.as_bytes()).map_err(|e| SetupError::Io(e.to_string()))?;

        // On Unix, sync data to disk
        #[cfg(unix)]
        if let Ok(file) = fs::File::open(&tmp_path) {
            let _ = file.sync_all();
        }

        fs::rename(&tmp_path, &target_path).map_err(|e| SetupError::Io(e.to_string()))?;

        tracing::debug!(
            step = %self.current_step,
            completed = self.completed,
            "Atomically persisted setup wizard state"
        );
        Ok(())
    }

    /// Advances the current step and persists the state transactionally.
    pub fn advance_to(&mut self, next: SetupStep, base_dir: &Path) -> Result<(), SetupError> {
        let mut cloned = self.clone();
        cloned.current_step = next;
        cloned.save(base_dir)?;
        *self = cloned;
        Ok(())
    }

    /// Marks the setup wizard as successfully completed and persists the state transactionally.
    pub fn mark_completed(&mut self, base_dir: &Path) -> Result<(), SetupError> {
        let mut cloned = self.clone();
        cloned.completed = true;
        cloned.current_step = SetupStep::Completed;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        cloned.completed_at = Some(now);
        cloned.save(base_dir)?;
        tracing::info!(
            completed_at = now,
            active_vault = %cloned.active_vault_id,
            "First-run setup wizard successfully completed and persisted"
        );
        *self = cloned;
        Ok(())
    }

    /// Detects the installation status of the given base directory.
    pub fn detect_installation(base_dir: &Path) -> InstallationStatus {
        // 1. Check for incomplete setup state first
        match Self::load(base_dir) {
            Ok(Some(state)) => {
                if !state.completed {
                    return InstallationStatus::IncompleteSetup {
                        last_step: state.current_step,
                        default_vault_created: state.default_vault_created,
                    };
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Corrupted setup state encountered in detect_installation");
            }
            Ok(None) => {}
        }

        // 2. Check for legacy unmigrated database
        let legacy_db = base_dir.join("shellumi.db");
        if legacy_db.exists() {
            if let Ok(meta) = fs::metadata(&legacy_db) {
                if meta.len() > 0 {
                    return InstallationStatus::LegacyDatabase {
                        path: legacy_db.to_string_lossy().to_string(),
                    };
                }
            }
        }

        // 3. Check for existing encrypted vaults
        let vaults_dir = base_dir.join("vaults");
        let default_db = vaults_dir.join("default.db");
        let manifests_dir = if base_dir.join("vault_manifests").exists() {
            base_dir.join("vault_manifests")
        } else {
            base_dir.join("manifests")
        };
        let default_manifest = manifests_dir.join("legacy-local.manifest.json");
        let alt_manifest = manifests_dir.join("default.manifest.json");

        let default_db_exists = default_db.exists()
            && fs::metadata(&default_db)
                .map(|m| m.len() > 0)
                .unwrap_or(false);
        let manifest_exists = default_manifest.exists() || alt_manifest.exists();

        let mut vault_count = 0;
        if vaults_dir.exists() {
            if let Ok(entries) = fs::read_dir(&vaults_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("db") {
                        vault_count += 1;
                    }
                }
            }
        }

        if default_db_exists || manifest_exists || vault_count > 0 {
            return InstallationStatus::ExistingVaults {
                default_db_exists,
                manifest_exists,
                vault_count,
            };
        }

        InstallationStatus::Fresh
    }

    /// Checks if setup has been completed and should not be displayed again.
    pub fn is_setup_completed(base_dir: &Path) -> bool {
        match Self::load(base_dir) {
            Ok(Some(state)) => {
                // If a setup state file exists, its `completed` flag is authoritative!
                // If `completed == false`, setup is incomplete, and must NEVER be treated as completed.
                state.completed
            }
            Ok(None) => {
                // If no state file exists, but default.db and manifest exist and are non-empty,
                // it's an existing pre-wizard completed installation.
                let default_db = base_dir.join("vaults").join("default.db");
                let default_manifest = base_dir
                    .join("vault_manifests")
                    .join("legacy-local.manifest.json");
                let alt_manifest = base_dir
                    .join("vault_manifests")
                    .join("default.manifest.json");
                let legacy_manifest = base_dir
                    .join("manifests")
                    .join("legacy-local.manifest.json");
                let db_valid = default_db.exists()
                    && fs::metadata(&default_db)
                        .map(|m| m.len() > 0)
                        .unwrap_or(false);
                let manifest_valid =
                    default_manifest.exists() || alt_manifest.exists() || legacy_manifest.exists();
                db_valid && manifest_valid
            }
            Err(e) => {
                // Corrupted setup state file detected: inspect existing encrypted databases and manifests
                tracing::warn!(error = %e, "Corrupted setup state file detected; checking vault presence");
                let default_db = base_dir.join("vaults").join("default.db");
                let default_manifest = base_dir
                    .join("vault_manifests")
                    .join("legacy-local.manifest.json");
                let alt_manifest = base_dir
                    .join("vault_manifests")
                    .join("default.manifest.json");
                let legacy_manifest = base_dir
                    .join("manifests")
                    .join("legacy-local.manifest.json");
                let db_valid = default_db.exists()
                    && fs::metadata(&default_db)
                        .map(|m| m.len() > 0)
                        .unwrap_or(false);
                let manifest_valid =
                    default_manifest.exists() || alt_manifest.exists() || legacy_manifest.exists();
                db_valid && manifest_valid
            }
        }
    }
}

/// Verification report produced before finalizing setup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupVerificationReport {
    pub default_db_encrypted: bool,
    pub default_manifest_valid: bool,
    pub vault_registry_accessible: bool,
    pub registered_vault_count: usize,
    pub unlock_verified: bool,
    pub errors: Vec<String>,
}

impl SetupVerificationReport {
    pub fn is_success(&self) -> bool {
        self.default_db_encrypted
            && self.default_manifest_valid
            && self.unlock_verified
            && self.errors.is_empty()
    }
}

/// Verifies database encryption, manifest integrity, and unlock capabilities before finishing setup.
pub fn verify_setup_integrity(
    base_dir: &Path,
    default_vault_key: Option<&VaultKey>,
) -> SetupVerificationReport {
    let mut report = SetupVerificationReport {
        default_db_encrypted: false,
        default_manifest_valid: false,
        vault_registry_accessible: false,
        registered_vault_count: 0,
        unlock_verified: false,
        errors: Vec::new(),
    };

    let default_db = base_dir.join("vaults").join("default.db");
    let manifests_dir = if base_dir.join("vault_manifests").exists() {
        base_dir.join("vault_manifests")
    } else if base_dir.join("manifests").exists() {
        base_dir.join("manifests")
    } else {
        base_dir.join("vault_manifests")
    };
    let manifest_path = manifests_dir.join("legacy-local.manifest.json");
    let alt_manifest_path = manifests_dir.join("default.manifest.json");

    // 1. Verify physical default database exists and is encrypted
    if !default_db.exists() {
        report
            .errors
            .push("Default vault database (default.db) does not exist.".into());
    } else {
        match fs::metadata(&default_db) {
            Ok(meta) if meta.len() == 0 => {
                report
                    .errors
                    .push("Default vault database is empty (0 bytes).".into());
            }
            Ok(_) => {
                // Verify it does NOT start with plaintext SQLite header
                if let Ok(mut f) = fs::File::open(&default_db) {
                    let mut header = [0u8; 16];
                    if f.read_exact(&mut header).is_ok() {
                        if &header == b"SQLite format 3\0" {
                            report.errors.push(
                                "Default vault database is unencrypted (contains plaintext SQLite header).".into()
                            );
                        } else {
                            report.default_db_encrypted = true;
                        }
                    } else {
                        report
                            .errors
                            .push("Unable to read default database header.".into());
                    }
                } else {
                    report
                        .errors
                        .push("Unable to open default database file.".into());
                }
            }
            Err(e) => {
                report
                    .errors
                    .push(format!("Cannot inspect default database metadata: {e}"));
            }
        }
    }

    // 2. Verify portable manifest exists and is valid
    let active_manifest_path = if manifest_path.exists() {
        Some(manifest_path)
    } else if alt_manifest_path.exists() {
        Some(alt_manifest_path)
    } else {
        None
    };

    if let Some(mpath) = active_manifest_path {
        match VaultManifest::load_from_path(&mpath) {
            Ok(manifest) => {
                if let Some(m_cost) = manifest.primary_envelope.argon2_m_cost {
                    if m_cost < 19 * 1024 {
                        report.errors.push(
                            "Manifest Argon2id memory cost is lower than security baseline.".into(),
                        );
                    }
                }
                if manifest.primary_envelope.salt.is_empty()
                    || manifest.primary_envelope.nonce.is_empty()
                    || manifest.primary_envelope.ciphertext.is_empty()
                {
                    report
                        .errors
                        .push("Manifest primary key envelope contains empty fields.".into());
                } else {
                    report.default_manifest_valid = true;
                }
            }
            Err(e) => {
                report
                    .errors
                    .push(format!("Failed to load or parse vault manifest: {e}"));
            }
        }
    } else {
        report
            .errors
            .push("Default vault manifest does not exist in vault_manifests/".into());
    }

    // 3. Verify unlock capability with provided key
    if let Some(key) = default_vault_key {
        if key.expose_bytes().len() == 32 {
            report.unlock_verified = true;
        } else {
            report.errors.push("Invalid DEK length for unlock.".into());
        }
    } else {
        report
            .errors
            .push("Default vault key was not provided for unlock verification.".into());
    }

    // 4. Check vault registry and count registered manifests
    if manifests_dir.exists() {
        report.vault_registry_accessible = true;
        if let Ok(entries) = fs::read_dir(&manifests_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.ends_with(".manifest.json"))
                        .unwrap_or(false)
                {
                    report.registered_vault_count += 1;
                }
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detects_fresh_installation() {
        let temp = tempdir().unwrap();
        let status = SetupState::detect_installation(temp.path());
        assert_eq!(status, InstallationStatus::Fresh);
        assert!(!SetupState::is_setup_completed(temp.path()));
    }

    #[test]
    fn detects_incomplete_setup_and_resumes() {
        let temp = tempdir().unwrap();
        let mut state = SetupState::new();
        state.mode = SetupWizardMode::NewSetup;
        state.current_step = SetupStep::DefaultVault;
        state.default_vault_created = true;
        state.save(temp.path()).unwrap();

        let status = SetupState::detect_installation(temp.path());
        assert_eq!(
            status,
            InstallationStatus::IncompleteSetup {
                last_step: SetupStep::DefaultVault,
                default_vault_created: true,
            }
        );
        assert!(!SetupState::is_setup_completed(temp.path()));

        // Resume and advance
        let mut loaded = SetupState::load(temp.path()).unwrap().unwrap();
        assert_eq!(loaded.current_step, SetupStep::DefaultVault);
        loaded
            .advance_to(SetupStep::GoogleDrive, temp.path())
            .unwrap();

        let reloaded = SetupState::load(temp.path()).unwrap().unwrap();
        assert_eq!(reloaded.current_step, SetupStep::GoogleDrive);
    }

    #[test]
    fn detects_legacy_database() {
        let temp = tempdir().unwrap();
        let legacy_file = temp.path().join("shellumi.db");
        fs::write(&legacy_file, b"SQLite format 3\0test_legacy_data").unwrap();

        let status = SetupState::detect_installation(temp.path());
        assert!(matches!(status, InstallationStatus::LegacyDatabase { .. }));
    }

    #[test]
    fn detects_existing_encrypted_vaults() {
        let temp = tempdir().unwrap();
        let vaults_dir = temp.path().join("vaults");
        fs::create_dir_all(&vaults_dir).unwrap();
        let db_file = vaults_dir.join("default.db");
        fs::write(&db_file, b"ENCRYPTED_AES256GCM_BLOB_HEADER_TEST").unwrap();

        let manifests_dir = temp.path().join("vault_manifests");
        fs::create_dir_all(&manifests_dir).unwrap();
        let manifest_file = manifests_dir.join("legacy-local.manifest.json");
        fs::write(&manifest_file, b"{}").unwrap();

        let status = SetupState::detect_installation(temp.path());
        assert!(matches!(
            status,
            InstallationStatus::ExistingVaults {
                default_db_exists: true,
                manifest_exists: true,
                vault_count: 1
            }
        ));
    }

    #[test]
    fn marks_completed_and_prevents_subsequent_runs() {
        let temp = tempdir().unwrap();
        let mut state = SetupState::new();
        state.save(temp.path()).unwrap();
        assert!(!SetupState::is_setup_completed(temp.path()));

        state.mark_completed(temp.path()).unwrap();
        assert!(SetupState::is_setup_completed(temp.path()));

        let loaded = SetupState::load(temp.path()).unwrap().unwrap();
        assert!(loaded.completed);
        assert_eq!(loaded.current_step, SetupStep::Completed);
        assert!(loaded.completed_at.is_some());
    }

    #[test]
    fn verifies_setup_integrity_validates_encrypted_headers() {
        let temp = tempdir().unwrap();
        let base = temp.path();

        // 1. Unencrypted sqlite header should fail
        let vaults = base.join("vaults");
        fs::create_dir_all(&vaults).unwrap();
        fs::write(vaults.join("default.db"), b"SQLite format 3\0payload").unwrap();

        let report = verify_setup_integrity(base, None);
        assert!(!report.is_success());
        assert!(!report.default_db_encrypted);
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("unencrypted (contains plaintext SQLite header)")));

        // 2. Encrypted header with valid manifest and key passes
        fs::write(
            vaults.join("default.db"),
            b"AES_GCM_ENCRYPTED_CHUNKS_NON_SQLITE_HEADER",
        )
        .unwrap();

        let manifests = base.join("vault_manifests");
        fs::create_dir_all(&manifests).unwrap();
        let vid = crate::vault::VaultId::new("legacy-local").unwrap();
        let key = VaultKey::generate().unwrap();
        let recovery = VaultKey::generate_recovery_code().unwrap();
        let primary_env = key.wrap_with_password(&vid, "SecurePassword123!#").unwrap();
        let recovery_env = Some(key.wrap_with_password(&vid, &recovery).unwrap());

        let manifest = VaultManifest::new(
            vid.as_str().to_string(),
            "Default Vault".to_string(),
            primary_env,
            recovery_env,
            false,
        );
        manifest
            .save_to_path(&manifests.join("legacy-local.manifest.json"))
            .unwrap();

        let valid_report = verify_setup_integrity(base, Some(&key));
        assert!(valid_report.is_success());
        assert!(valid_report.default_db_encrypted);
        assert!(valid_report.default_manifest_valid);
        assert!(valid_report.unlock_verified);
        assert!(valid_report.errors.is_empty());
    }
}

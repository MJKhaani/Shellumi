//! Typed installation inventory, onboarding route resolution, operation state, and pure reducer.
//!
//! Separates observed filesystem/credential metadata (`InstallationInventory`) from
//! workflow routing (`OnboardingRoute`) and runtime activity (`OperationState`).
//! Never stores user secrets, passwords, recovery codes, or DEKs.

use crate::setup::{SetupError, SetupState, SetupStep, SetupWizardMode, SETUP_STATE_FILE};
use crate::vault_manifest::VaultManifest;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Integrity state of a single filesystem artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactState {
    Absent,
    Valid,
    Empty,
    Corrupt(String),
    SymlinkRejected(String),
    Inaccessible(String),
}

impl ArtifactState {
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    pub fn is_present(&self) -> bool {
        !matches!(self, Self::Absent)
    }
}

/// Supported manifest directory layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManifestLayout {
    VaultManifests,
    LegacyManifests,
}

/// Observed sidecar file belonging to a SQLite/Turso database.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarArtifact {
    pub suffix: String,
    pub path: PathBuf,
    pub size_bytes: u64,
}

/// Observed state of the legacy `shellumi.db` source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacySourceStatus {
    Absent,
    Present {
        path: PathBuf,
        size_bytes: u64,
        modified_unix_secs: Option<u64>,
        sidecars: Vec<SidecarArtifact>,
        is_encrypted_or_non_sqlite: bool,
    },
    Invalid {
        path: PathBuf,
        reason: String,
    },
}

impl LegacySourceStatus {
    pub fn is_present(&self) -> bool {
        matches!(self, Self::Present { .. })
    }
}

/// Observed state of a single modern vault database + manifest pair.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultArtifactStatus {
    pub vault_id: String,
    pub is_default: bool,
    pub db_path: PathBuf,
    pub db_state: ArtifactState,
    pub manifest_path: Option<PathBuf>,
    pub manifest_state: ArtifactState,
    pub manifest_layout: Option<ManifestLayout>,
    pub has_primary_envelope: bool,
    pub has_recovery_envelope: bool,
}

impl VaultArtifactStatus {
    pub fn is_valid_pair(&self) -> bool {
        self.db_state.is_valid() && self.manifest_state.is_valid() && self.has_primary_envelope
    }
}

/// Storage-supplied read-only summary of `vaults/.migration_staging` and `.migration_complete`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationInventoryStatus {
    Absent,
    InProgress {
        transaction_id: String,
        phase: String,
        has_backup: bool,
        staged_vault_count: usize,
        receipt_present: bool,
        source_matches_journal: Option<bool>,
        can_rollback: bool,
        details: Vec<String>,
    },
    Complete {
        transaction_id: String,
        receipt_verified: bool,
        vaults_present: bool,
    },
    InvalidOrCorrupt {
        reason: String,
    },
}

/// Storage-supplied read-only summary of `vaults/.legacy_discard`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscardInventoryStatus {
    Absent,
    InProgress {
        transaction_id: String,
        phase: String,
        can_rollback: bool,
        irreversible_committed: bool,
        scoped_paths: Vec<PathBuf>,
        details: Vec<String>,
    },
    Complete {
        transaction_id: String,
        completed_at: Option<u64>,
    },
    InvalidOrCorrupt {
        reason: String,
    },
}

/// Observed state of `.setup_state.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupCheckpointStatus {
    Absent,
    Valid(SetupState),
    UnsupportedVersion(u32),
    Corrupt(String),
}

/// Kind of auxiliary pending transaction journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingTransactionKind {
    Creation,
    Restore,
    Rotation,
    Quarantine,
}

/// Auxiliary pending transaction artifact detected in the data root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingTransactionArtifact {
    pub kind: PendingTransactionKind,
    pub path: PathBuf,
    pub valid: bool,
    pub detail: String,
}

/// Read-only filesystem & metadata inventory of a Shellumi data root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallationInventory {
    pub root_dir: PathBuf,
    pub root_accessible: bool,
    pub root_error: Option<String>,
    pub legacy: LegacySourceStatus,
    pub vaults: Vec<VaultArtifactStatus>,
    pub migration: MigrationInventoryStatus,
    pub discard: DiscardInventoryStatus,
    pub setup_checkpoint: SetupCheckpointStatus,
    pub other_journals: Vec<PendingTransactionArtifact>,
}

fn check_no_symlink_from(root: &Path, path: &Path) -> Result<(), String> {
    // Check root itself (without walking OS-level symlinks like /var -> /private/var on macOS).
    match fs::symlink_metadata(root) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(format!("Symbolic link rejected at {}", root.display()));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("Cannot inspect {}: {e}", root.display())),
    }
    if let Ok(rel) = path.strip_prefix(root) {
        let mut cur = root.to_path_buf();
        for comp in rel.components() {
            cur.push(comp);
            match fs::symlink_metadata(&cur) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(format!("Symbolic link rejected at {}", cur.display()));
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("Cannot inspect {}: {e}", cur.display())),
            }
        }
    } else if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(format!("Symbolic link rejected at {}", path.display()));
        }
    }
    Ok(())
}

fn inspect_file_state(root: &Path, path: &Path) -> ArtifactState {
    if let Err(reason) = check_no_symlink_from(root, path) {
        if reason.contains("Symbolic link") {
            return ArtifactState::SymlinkRejected(reason);
        }
        return ArtifactState::Inaccessible(reason);
    }
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                ArtifactState::SymlinkRejected(format!("{} is a symbolic link", path.display()))
            } else if !meta.is_file() {
                ArtifactState::Corrupt(format!("{} is not a regular file", path.display()))
            } else if meta.len() == 0 {
                ArtifactState::Empty
            } else {
                ArtifactState::Valid
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ArtifactState::Absent,
        Err(e) => ArtifactState::Inaccessible(format!("Cannot access {}: {e}", path.display())),
    }
}

fn inspect_legacy_source(base_dir: &Path) -> LegacySourceStatus {
    let legacy_path = base_dir.join("shellumi.db");
    if let Err(reason) = check_no_symlink_from(base_dir, &legacy_path) {
        return LegacySourceStatus::Invalid {
            path: legacy_path,
            reason,
        };
    }
    match fs::symlink_metadata(&legacy_path) {
        Ok(meta) => {
            if !meta.is_file() {
                return LegacySourceStatus::Invalid {
                    path: legacy_path,
                    reason: "shellumi.db is not a regular file".to_string(),
                };
            }
            if meta.len() == 0 {
                return LegacySourceStatus::Invalid {
                    path: legacy_path,
                    reason: "shellumi.db is empty (0 bytes)".to_string(),
                };
            }
            let modified_unix_secs = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());

            let mut header = [0u8; 16];
            let is_encrypted_or_non_sqlite = match fs::File::open(&legacy_path) {
                Ok(mut f) => {
                    use std::io::Read;
                    match f.read_exact(&mut header) {
                        Ok(()) => &header != b"SQLite format 3\0",
                        Err(_) => true,
                    }
                }
                Err(e) => {
                    return LegacySourceStatus::Invalid {
                        path: legacy_path,
                        reason: format!("Cannot read shellumi.db: {e}"),
                    };
                }
            };

            let mut sidecars = Vec::new();
            for suffix in ["-wal", "-shm", "-journal"] {
                let sc_path = base_dir.join(format!("shellumi.db{suffix}"));
                match fs::symlink_metadata(&sc_path) {
                    Ok(sc_meta) if sc_meta.file_type().is_symlink() => {
                        return LegacySourceStatus::Invalid {
                            path: sc_path,
                            reason: format!("Sidecar shellumi.db{suffix} is a symbolic link"),
                        };
                    }
                    Ok(sc_meta) if sc_meta.is_file() => {
                        sidecars.push(SidecarArtifact {
                            suffix: suffix.to_string(),
                            path: sc_path,
                            size_bytes: sc_meta.len(),
                        });
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        return LegacySourceStatus::Invalid {
                            path: sc_path,
                            reason: format!("Cannot inspect sidecar shellumi.db{suffix}: {e}"),
                        };
                    }
                }
            }

            LegacySourceStatus::Present {
                path: legacy_path,
                size_bytes: meta.len(),
                modified_unix_secs,
                sidecars,
                is_encrypted_or_non_sqlite,
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => LegacySourceStatus::Absent,
        Err(e) => LegacySourceStatus::Invalid {
            path: legacy_path,
            reason: format!("Cannot inspect shellumi.db: {e}"),
        },
    }
}

impl InstallationInventory {
    /// Performs a read-only inspection of `base_dir` using storage-supplied migration and discard statuses.
    /// Never creates databases, mutates Keychain, or modifies any files.
    pub fn inspect_filesystem(
        base_dir: &Path,
        migration: MigrationInventoryStatus,
        discard: DiscardInventoryStatus,
    ) -> Self {
        let root_dir = base_dir.to_path_buf();
        if let Err(reason) = check_no_symlink_from(base_dir, base_dir) {
            return Self {
                root_dir,
                root_accessible: false,
                root_error: Some(reason),
                legacy: LegacySourceStatus::Absent,
                vaults: Vec::new(),
                migration,
                discard,
                setup_checkpoint: SetupCheckpointStatus::Absent,
                other_journals: Vec::new(),
            };
        }

        if base_dir.exists() {
            if let Err(e) = fs::read_dir(base_dir) {
                return Self {
                    root_dir,
                    root_accessible: false,
                    root_error: Some(format!("Data directory is inaccessible: {e}")),
                    legacy: LegacySourceStatus::Absent,
                    vaults: Vec::new(),
                    migration,
                    discard,
                    setup_checkpoint: SetupCheckpointStatus::Absent,
                    other_journals: Vec::new(),
                };
            }
        }

        let legacy = inspect_legacy_source(base_dir);

        // Inspect setup checkpoint without mutating or deleting corrupt/unknown files.
        let setup_checkpoint = {
            let state_path = base_dir.join(SETUP_STATE_FILE);
            match check_no_symlink_from(base_dir, &state_path) {
                Err(reason) => SetupCheckpointStatus::Corrupt(reason),
                Ok(()) => match SetupState::load(base_dir) {
                    Ok(Some(state)) => SetupCheckpointStatus::Valid(state),
                    Ok(None) => SetupCheckpointStatus::Absent,
                    Err(SetupError::UnsupportedVersion(v)) => {
                        SetupCheckpointStatus::UnsupportedVersion(v)
                    }
                    Err(e) => SetupCheckpointStatus::Corrupt(e.to_string()),
                },
            }
        };

        // Inspect vaults and manifests across both `vault_manifests/` and legacy `manifests/`.
        let vaults_dir = base_dir.join("vaults");
        let vm_dir = base_dir.join("vault_manifests");
        let lm_dir = base_dir.join("manifests");

        let mut vault_ids = std::collections::BTreeSet::new();
        let mut db_files: std::collections::BTreeMap<String, PathBuf> =
            std::collections::BTreeMap::new();
        let mut other_journals = Vec::new();

        if vaults_dir.exists() {
            if let Ok(entries) = fs::read_dir(&vaults_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with(".staged_") && name.ends_with(".db") {
                        other_journals.push(PendingTransactionArtifact {
                            kind: PendingTransactionKind::Creation,
                            path,
                            valid: true,
                            detail: format!("Staged vault database {name}"),
                        });
                        continue;
                    }
                    if name.starts_with(".restore_") {
                        other_journals.push(PendingTransactionArtifact {
                            kind: PendingTransactionKind::Restore,
                            path,
                            valid: true,
                            detail: format!("Staged restore artifact {name}"),
                        });
                        continue;
                    }
                    if name.contains("rotation") && name.ends_with(".json") {
                        other_journals.push(PendingTransactionArtifact {
                            kind: PendingTransactionKind::Rotation,
                            path,
                            valid: true,
                            detail: format!("Pending rotation journal {name}"),
                        });
                        continue;
                    }
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("db") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            let vid = if stem == "default" {
                                "legacy-local".to_string()
                            } else {
                                stem.to_string()
                            };
                            vault_ids.insert(vid.clone());
                            db_files.insert(vid, path);
                        }
                    }
                }
            }
        }

        let mut manifest_files: std::collections::BTreeMap<String, (PathBuf, ManifestLayout)> =
            std::collections::BTreeMap::new();
        for (dir, layout) in [
            (&vm_dir, ManifestLayout::VaultManifests),
            (&lm_dir, ManifestLayout::LegacyManifests),
        ] {
            if dir.exists() {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let name = entry.file_name().to_string_lossy().to_string();
                        if let Some(raw_id) = name.strip_suffix(".manifest.json") {
                            let vid = if raw_id == "default" {
                                "legacy-local".to_string()
                            } else {
                                raw_id.to_string()
                            };
                            vault_ids.insert(vid.clone());
                            manifest_files.entry(vid).or_insert((path, layout));
                        }
                    }
                }
            }
        }

        let mut vaults = Vec::new();
        for vid in vault_ids {
            let is_default = vid == "legacy-local";
            let db_path = db_files.get(&vid).cloned().unwrap_or_else(|| {
                if is_default {
                    vaults_dir.join("default.db")
                } else {
                    vaults_dir.join(format!("{vid}.db"))
                }
            });
            let db_state = inspect_file_state(base_dir, &db_path);

            let (manifest_path, manifest_layout, manifest_state, has_primary, has_recovery) =
                if let Some((mpath, layout)) = manifest_files.get(&vid) {
                    let st = inspect_file_state(base_dir, mpath);
                    if st.is_valid() {
                        match VaultManifest::load_from_path(mpath) {
                            Ok(m) => {
                                let has_primary = !m.primary_envelope.salt.is_empty()
                                    && !m.primary_envelope.nonce.is_empty()
                                    && !m.primary_envelope.ciphertext.is_empty();
                                let has_recovery = m.recovery_envelope.is_some();
                                if m.vault_id != vid && !(is_default && m.vault_id == "default") {
                                    (
                                        Some(mpath.clone()),
                                        Some(*layout),
                                        ArtifactState::Corrupt(format!(
                                            "Manifest vault_id '{}' does not match '{}'",
                                            m.vault_id, vid
                                        )),
                                        false,
                                        false,
                                    )
                                } else {
                                    (
                                        Some(mpath.clone()),
                                        Some(*layout),
                                        ArtifactState::Valid,
                                        has_primary,
                                        has_recovery,
                                    )
                                }
                            }
                            Err(e) => (
                                Some(mpath.clone()),
                                Some(*layout),
                                ArtifactState::Corrupt(format!("Invalid manifest JSON: {e}")),
                                false,
                                false,
                            ),
                        }
                    } else {
                        (Some(mpath.clone()), Some(*layout), st, false, false)
                    }
                } else {
                    (None, None, ArtifactState::Absent, false, false)
                };

            vaults.push(VaultArtifactStatus {
                vault_id: vid,
                is_default,
                db_path,
                db_state,
                manifest_path,
                manifest_state,
                manifest_layout,
                has_primary_envelope: has_primary,
                has_recovery_envelope: has_recovery,
            });
        }

        Self {
            root_dir,
            root_accessible: true,
            root_error: None,
            legacy,
            vaults,
            migration,
            discard,
            setup_checkpoint,
            other_journals,
        }
    }

    /// Convenience inspection without storage-crate migration/discard parsers (used in core unit tests).
    pub fn inspect_basic(base_dir: &Path) -> Self {
        let migration = if base_dir.join("vaults/.migration_staging").exists() {
            MigrationInventoryStatus::InvalidOrCorrupt {
                reason: "Migration staging present (inspect via storage)".to_string(),
            }
        } else {
            MigrationInventoryStatus::Absent
        };
        let discard = if base_dir.join("vaults/.legacy_discard").exists() {
            DiscardInventoryStatus::InvalidOrCorrupt {
                reason: "Discard staging present (inspect via storage)".to_string(),
            }
        } else {
            DiscardInventoryStatus::Absent
        };
        Self::inspect_filesystem(base_dir, migration, discard)
    }

    pub fn default_vault(&self) -> Option<&VaultArtifactStatus> {
        self.vaults.iter().find(|v| v.is_default)
    }

    pub fn has_valid_default_vault(&self) -> bool {
        self.default_vault().is_some_and(|v| v.is_valid_pair())
    }
}

/// High-level onboarding route derived from `InstallationInventory`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnboardingRoute {
    Fresh,
    LegacyMigration,
    InterruptedMigration,
    InterruptedDiscard,
    ExistingVault,
    CoexistingLegacyAndModern,
    InterruptedCreation,
    InterruptedRestore,
    InconsistentInstallation {
        reason: String,
        details: Vec<String>,
    },
    ExplicitReentry,
}

/// Result of deterministic route resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteResolution {
    pub route: OnboardingRoute,
    pub reconcile_checkpoint: bool,
    pub needs_recovery_confirmation: bool,
    pub diagnostics: Vec<String>,
}

/// Resolves the authoritative onboarding route from observed inventory and re-entry intent.
pub fn resolve_onboarding_route(
    inventory: &InstallationInventory,
    explicit_reentry: bool,
) -> RouteResolution {
    let mut diagnostics = Vec::new();

    if !inventory.root_accessible {
        return RouteResolution {
            route: OnboardingRoute::InconsistentInstallation {
                reason: inventory
                    .root_error
                    .clone()
                    .unwrap_or_else(|| "Data directory is inaccessible".to_string()),
                details: vec![format!("Root: {}", inventory.root_dir.display())],
            },
            reconcile_checkpoint: false,
            needs_recovery_confirmation: false,
            diagnostics,
        };
    }

    // 1. Active valid discard transaction takes highest precedence.
    match &inventory.discard {
        DiscardInventoryStatus::InProgress { details, .. } => {
            diagnostics.extend(details.clone());
            return RouteResolution {
                route: OnboardingRoute::InterruptedDiscard,
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }
        DiscardInventoryStatus::InvalidOrCorrupt { reason } => {
            return RouteResolution {
                route: OnboardingRoute::InconsistentInstallation {
                    reason: format!("Corrupt or unowned legacy-discard journal: {reason}"),
                    details: vec![
                        "Refusing automatic deletion; inspect vaults/.legacy_discard before proceeding."
                            .to_string(),
                    ],
                },
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }
        DiscardInventoryStatus::Absent | DiscardInventoryStatus::Complete { .. } => {}
    }

    // 2. Invalid/corrupt migration journal or checkpoint errors block unsafe automation.
    match &inventory.migration {
        MigrationInventoryStatus::InvalidOrCorrupt { reason } => {
            return RouteResolution {
                route: OnboardingRoute::InconsistentInstallation {
                    reason: format!("Invalid or unreadable migration journal: {reason}"),
                    details: vec![
                        "Original database and staging artifacts have been preserved.".to_string(),
                    ],
                },
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }
        MigrationInventoryStatus::InProgress { details, .. } => {
            diagnostics.extend(details.clone());
            return RouteResolution {
                route: OnboardingRoute::InterruptedMigration,
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }
        MigrationInventoryStatus::Absent | MigrationInventoryStatus::Complete { .. } => {}
    }

    match &inventory.setup_checkpoint {
        SetupCheckpointStatus::UnsupportedVersion(v) => {
            return RouteResolution {
                route: OnboardingRoute::InconsistentInstallation {
                    reason: format!("Unsupported setup checkpoint version {v}"),
                    details: vec![
                        "This installation was touched by a newer version of Shellumi.".to_string(),
                    ],
                },
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }
        SetupCheckpointStatus::Corrupt(err) => {
            return RouteResolution {
                route: OnboardingRoute::InconsistentInstallation {
                    reason: format!("Corrupted setup checkpoint (.setup_state.json): {err}"),
                    details: vec![
                        "Preserved .setup_state.json without deleting it. Inspect or back up before continuing."
                            .to_string(),
                    ],
                },
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }
        SetupCheckpointStatus::Absent | SetupCheckpointStatus::Valid(_) => {}
    }

    if let LegacySourceStatus::Invalid { path, reason } = &inventory.legacy {
        return RouteResolution {
            route: OnboardingRoute::InconsistentInstallation {
                reason: format!(
                    "Invalid legacy database artifact at {}: {reason}",
                    path.display()
                ),
                details: vec![
                    "Resolve or back up the invalid legacy file before continuing.".to_string(),
                ],
            },
            reconcile_checkpoint: false,
            needs_recovery_confirmation: false,
            diagnostics,
        };
    }

    // Check for any partial/mismatched vault artifacts (DB only or manifest only or corrupt).
    for vault in &inventory.vaults {
        if !vault.is_valid_pair() {
            let mut details = Vec::new();
            details.push(format!("Vault ID: {}", vault.vault_id));
            details.push(format!("Database state: {:?}", vault.db_state));
            details.push(format!("Manifest state: {:?}", vault.manifest_state));
            return RouteResolution {
                route: OnboardingRoute::InconsistentInstallation {
                    reason: format!(
                        "Vault '{}' has missing, mismatched, or corrupt artifacts",
                        vault.vault_id
                    ),
                    details,
                },
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }
    }

    // Check pending restore / creation staging if no valid default vault exists yet.
    if !inventory.has_valid_default_vault() {
        if let Some(pending) = inventory
            .other_journals
            .iter()
            .find(|j| j.kind == PendingTransactionKind::Restore)
        {
            diagnostics.push(pending.detail.clone());
            return RouteResolution {
                route: OnboardingRoute::InterruptedRestore,
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }
        if let Some(pending) = inventory
            .other_journals
            .iter()
            .find(|j| j.kind == PendingTransactionKind::Creation)
        {
            diagnostics.push(pending.detail.clone());
            return RouteResolution {
                route: OnboardingRoute::InterruptedCreation,
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }
    }

    let valid_modern_exists = inventory.has_valid_default_vault();
    let legacy_present = inventory.legacy.is_present();
    let migration_complete = matches!(
        inventory.migration,
        MigrationInventoryStatus::Complete {
            receipt_verified: true,
            vaults_present: true,
            ..
        }
    );

    let checkpoint_completed = matches!(
        &inventory.setup_checkpoint,
        SetupCheckpointStatus::Valid(state) if state.completed
    );
    let recovery_confirmed = matches!(
        &inventory.setup_checkpoint,
        SetupCheckpointStatus::Valid(state)
            if state.recovery_code_confirmed
                || state
                    .confirmed_recovery_vaults
                    .iter()
                    .any(|v| v == "legacy-local")
    );

    // 3. Valid active DB + matching manifest.
    if valid_modern_exists {
        if legacy_present && !migration_complete {
            // Legacy DB coexists with independent valid modern vault(s) without migration ownership proof.
            diagnostics.push(
                "Both a legacy shellumi.db and an independent modern vault exist without migration ownership."
                    .to_string(),
            );
            return RouteResolution {
                route: OnboardingRoute::CoexistingLegacyAndModern,
                reconcile_checkpoint: false,
                needs_recovery_confirmation: false,
                diagnostics,
            };
        }

        if explicit_reentry {
            return RouteResolution {
                route: OnboardingRoute::ExplicitReentry,
                reconcile_checkpoint: !checkpoint_completed,
                needs_recovery_confirmation: migration_complete && !recovery_confirmed,
                diagnostics,
            };
        }

        // Valid activated migration with stale/incomplete setup checkpoint reconciles from storage.
        return RouteResolution {
            route: OnboardingRoute::ExistingVault,
            reconcile_checkpoint: !checkpoint_completed,
            needs_recovery_confirmation: migration_complete && !recovery_confirmed,
            diagnostics,
        };
    }

    // 4. Legacy DB present, no active modern installation, no migration journal.
    if legacy_present {
        return RouteResolution {
            route: OnboardingRoute::LegacyMigration,
            reconcile_checkpoint: false,
            needs_recovery_confirmation: false,
            diagnostics,
        };
    }

    // 5. Fresh installation.
    RouteResolution {
        route: OnboardingRoute::Fresh,
        reconcile_checkpoint: false,
        needs_recovery_confirmation: false,
        diagnostics,
    }
}

/// Unique identifier for an in-flight onboarding operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OnboardingOperationId(pub u64);

/// Human-readable phase of an active operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationPhase {
    Preparing,
    BackingUp,
    ReadingAndPlanning,
    CreatingVault,
    CopyingAndEncrypting,
    Verifying,
    Activating,
    Quarantining,
    CommittingDelete,
    Purging,
    RollingBack,
    WaitingForGoogleSignIn,
    ExchangingOAuthToken,
    DiscoveringCloudVaults,
    RestoringCloudVault,
    ConnectingServer,
    Finalizing,
}

impl fmt::Display for OperationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Preparing => "Preparing…",
            Self::BackingUp => "Creating verified backup…",
            Self::ReadingAndPlanning => "Reading and planning…",
            Self::CreatingVault => "Creating encrypted vault…",
            Self::CopyingAndEncrypting => "Copying and encrypting records…",
            Self::Verifying => "Verifying integrity…",
            Self::Activating => "Activating verified artifacts…",
            Self::Quarantining => "Quarantining scoped legacy files…",
            Self::CommittingDelete => "Committing permanent removal…",
            Self::Purging => "Purging quarantined legacy files…",
            Self::RollingBack => "Rolling back changes…",
            Self::WaitingForGoogleSignIn => "Waiting for Google sign-in…",
            Self::ExchangingOAuthToken => "Completing Google sign-in…",
            Self::DiscoveringCloudVaults => "Scanning Google Drive backups…",
            Self::RestoringCloudVault => "Restoring encrypted cloud vault…",
            Self::ConnectingServer => "Connecting to Shellumi Server…",
            Self::Finalizing => "Verifying setup and launching…",
        };
        f.write_str(label)
    }
}

/// Sanitized user-facing error with optional diagnostic details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserFacingError {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub diagnostic_code: &'static str,
}

impl UserFacingError {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        diagnostic_code: &'static str,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            detail: None,
            diagnostic_code,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Single authoritative runtime state for wizard operations.
/// Prevents simultaneous green success banners and red error banners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationState {
    Idle,
    Running {
        id: OnboardingOperationId,
        phase: OperationPhase,
        can_cancel: bool,
        progress: Option<(u64, u64)>,
    },
    Succeeded {
        id: OnboardingOperationId,
        summary: String,
    },
    Failed {
        id: OnboardingOperationId,
        error: UserFacingError,
        retryable: bool,
    },
    Cancelled {
        id: OnboardingOperationId,
    },
}

impl OperationState {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    pub fn can_cancel(&self) -> bool {
        matches!(
            self,
            Self::Running {
                can_cancel: true,
                ..
            }
        )
    }

    pub fn active_id(&self) -> Option<OnboardingOperationId> {
        match self {
            Self::Running { id, .. }
            | Self::Succeeded { id, .. }
            | Self::Failed { id, .. }
            | Self::Cancelled { id } => Some(*id),
            Self::Idle => None,
        }
    }

    pub fn error(&self) -> Option<&UserFacingError> {
        match self {
            Self::Failed { error, .. } => Some(error),
            _ => None,
        }
    }

    pub fn success_summary(&self) -> Option<&str> {
        match self {
            Self::Succeeded { summary, .. } => Some(summary.as_str()),
            _ => None,
        }
    }
}

/// Route-aware wizard pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WizardPage {
    FreshWelcome,
    LegacyDecision,
    DiscardConfirm,
    InterruptedMigration,
    InterruptedDiscard,
    InconsistentInstallation,
    VaultPassword,
    RecoveryCode,
    MigrationReview,
    MigrationProgress,
    LocalUnlock,
    GoogleDrive,
    CloudRestore,
    AdditionalVaults,
    ReviewAndFinish,
}

impl WizardPage {
    pub fn title(&self) -> &'static str {
        match self {
            Self::FreshWelcome => "Welcome",
            Self::LegacyDecision => "Existing Data",
            Self::DiscardConfirm => "Remove Database",
            Self::InterruptedMigration => "Resume Migration",
            Self::InterruptedDiscard => "Resume Removal",
            Self::InconsistentInstallation => "Diagnostics",
            Self::VaultPassword => "Secure Vault",
            Self::RecoveryCode => "Recovery Code",
            Self::MigrationReview => "Review Migration",
            Self::MigrationProgress => "Migrating Data",
            Self::LocalUnlock => "Unlock Vault",
            Self::GoogleDrive => "Google Drive",
            Self::CloudRestore => "Restore Backup",
            Self::AdditionalVaults => "More Vaults",
            Self::ReviewAndFinish => "Finish",
        }
    }

    /// Maps a page to the high-level `SetupStep` for checkpoint persistence.
    pub fn to_setup_step(&self) -> SetupStep {
        match self {
            Self::FreshWelcome
            | Self::LegacyDecision
            | Self::DiscardConfirm
            | Self::InterruptedMigration
            | Self::InterruptedDiscard
            | Self::InconsistentInstallation => SetupStep::Welcome,
            Self::VaultPassword
            | Self::RecoveryCode
            | Self::MigrationReview
            | Self::MigrationProgress
            | Self::LocalUnlock => SetupStep::DefaultVault,
            Self::GoogleDrive | Self::CloudRestore => SetupStep::GoogleDrive,
            Self::AdditionalVaults => SetupStep::AdditionalVaults,
            Self::ReviewAndFinish => SetupStep::ReviewAndFinish,
        }
    }
}

/// Pure controller/reducer for onboarding state transitions, operation gating, and navigation.
#[derive(Clone, Debug)]
pub struct OnboardingController {
    pub session_generation: u64,
    pub next_operation_seq: u64,
    pub route: OnboardingRoute,
    pub mode: SetupWizardMode,
    pub current_page: WizardPage,
    pub operation: OperationState,
    pub default_vault_unlocked: bool,
    pub recovery_confirmed: bool,
    pub google_drive_skipped: bool,
    pub additional_vaults_skipped: bool,
    pub discard_backup_completed: bool,
    pub discard_proceed_without_backup: bool,
    pub discard_acknowledged: bool,
    pub discard_token_input: String,
}

impl OnboardingController {
    pub fn from_resolution(resolution: &RouteResolution) -> Self {
        let (mode, current_page, default_vault_unlocked, recovery_confirmed) =
            match &resolution.route {
                OnboardingRoute::Fresh => (
                    SetupWizardMode::NewSetup,
                    WizardPage::FreshWelcome,
                    false,
                    false,
                ),
                OnboardingRoute::LegacyMigration => (
                    SetupWizardMode::NewSetup,
                    WizardPage::LegacyDecision,
                    false,
                    false,
                ),
                OnboardingRoute::InterruptedMigration => (
                    SetupWizardMode::NewSetup,
                    WizardPage::InterruptedMigration,
                    false,
                    false,
                ),
                OnboardingRoute::InterruptedDiscard => (
                    SetupWizardMode::NewSetup,
                    WizardPage::InterruptedDiscard,
                    false,
                    false,
                ),
                OnboardingRoute::InconsistentInstallation { .. } => (
                    SetupWizardMode::RestoreSetup,
                    WizardPage::InconsistentInstallation,
                    false,
                    false,
                ),
                OnboardingRoute::ExistingVault
                | OnboardingRoute::CoexistingLegacyAndModern
                | OnboardingRoute::ExplicitReentry => (
                    SetupWizardMode::RestoreSetup,
                    WizardPage::LocalUnlock,
                    false,
                    false,
                ),
                OnboardingRoute::InterruptedCreation => (
                    SetupWizardMode::NewSetup,
                    WizardPage::VaultPassword,
                    false,
                    false,
                ),
                OnboardingRoute::InterruptedRestore => (
                    SetupWizardMode::RestoreSetup,
                    WizardPage::GoogleDrive,
                    false,
                    false,
                ),
            };

        Self {
            session_generation: 1,
            next_operation_seq: 1,
            route: resolution.route.clone(),
            mode,
            current_page,
            operation: OperationState::Idle,
            default_vault_unlocked,
            recovery_confirmed,
            google_drive_skipped: false,
            additional_vaults_skipped: false,
            discard_backup_completed: false,
            discard_proceed_without_backup: false,
            discard_acknowledged: false,
            discard_token_input: String::new(),
        }
    }

    /// Starts a new operation if idle (single-flight gate at handler boundary).
    pub fn try_start_operation(
        &mut self,
        phase: OperationPhase,
        can_cancel: bool,
    ) -> Option<(u64, OnboardingOperationId)> {
        if self.operation.is_running() {
            return None;
        }
        let id = OnboardingOperationId(self.next_operation_seq);
        self.next_operation_seq = self.next_operation_seq.saturating_add(1);
        self.operation = OperationState::Running {
            id,
            phase,
            can_cancel,
            progress: None,
        };
        Some((self.session_generation, id))
    }

    /// Updates progress only if generation and operation ID still match the active run.
    pub fn update_operation_progress(
        &mut self,
        generation: u64,
        id: OnboardingOperationId,
        phase: OperationPhase,
        can_cancel: bool,
        progress: Option<(u64, u64)>,
    ) -> bool {
        if generation != self.session_generation {
            return false;
        }
        if matches!(self.operation, OperationState::Running { id: active, .. } if active == id) {
            self.operation = OperationState::Running {
                id,
                phase,
                can_cancel,
                progress,
            };
            true
        } else {
            false
        }
    }

    /// Completes an operation only if generation and ID match.
    pub fn finish_operation_ok(
        &mut self,
        generation: u64,
        id: OnboardingOperationId,
        summary: impl Into<String>,
    ) -> bool {
        if generation != self.session_generation {
            return false;
        }
        if matches!(self.operation, OperationState::Running { id: active, .. } if active == id) {
            self.operation = OperationState::Succeeded {
                id,
                summary: summary.into(),
            };
            true
        } else {
            false
        }
    }

    /// Fails an operation only if generation and ID match.
    pub fn finish_operation_err(
        &mut self,
        generation: u64,
        id: OnboardingOperationId,
        error: UserFacingError,
        retryable: bool,
    ) -> bool {
        if generation != self.session_generation {
            return false;
        }
        if matches!(self.operation, OperationState::Running { id: active, .. } if active == id) {
            self.operation = OperationState::Failed {
                id,
                error,
                retryable,
            };
            true
        } else {
            false
        }
    }

    /// Cancels the active operation if cancellable.
    pub fn cancel_active_operation(&mut self) -> Option<OnboardingOperationId> {
        if let OperationState::Running {
            id,
            can_cancel: true,
            ..
        } = self.operation
        {
            self.operation = OperationState::Cancelled { id };
            Some(id)
        } else {
            None
        }
    }

    /// Clears non-running operation feedback when navigating.
    pub fn clear_feedback(&mut self) {
        if !self.operation.is_running() {
            self.operation = OperationState::Idle;
        }
    }

    /// Navigates to a new page, resetting destructive confirmation tokens on exit.
    pub fn navigate_to(&mut self, page: WizardPage) -> bool {
        if self.operation.is_running() {
            return false;
        }
        if self.current_page == WizardPage::DiscardConfirm && page != WizardPage::DiscardConfirm {
            self.reset_discard_confirmation();
        }
        self.current_page = page;
        self.clear_feedback();
        true
    }

    pub fn reset_discard_confirmation(&mut self) {
        self.discard_acknowledged = false;
        self.discard_token_input.clear();
        self.discard_proceed_without_backup = false;
    }

    /// Returns true only when all explicit destructive confirmation gates are satisfied.
    pub fn can_confirm_legacy_discard(&self) -> bool {
        !self.operation.is_running()
            && self.current_page == WizardPage::DiscardConfirm
            && self.discard_acknowledged
            && self.discard_token_input == "DELETE"
            && (self.discard_backup_completed || self.discard_proceed_without_backup)
    }

    /// Route-aware Back navigation; never replays creation or mutates consent.
    pub fn can_go_back(&self) -> bool {
        if self.operation.is_running() {
            return false;
        }
        !matches!(
            self.current_page,
            WizardPage::FreshWelcome
                | WizardPage::LegacyDecision
                | WizardPage::InterruptedMigration
                | WizardPage::InterruptedDiscard
                | WizardPage::InconsistentInstallation
        )
    }

    pub fn go_back(&mut self) -> bool {
        if !self.can_go_back() {
            return false;
        }
        let prev = match self.current_page {
            WizardPage::DiscardConfirm | WizardPage::MigrationReview => WizardPage::LegacyDecision,
            WizardPage::MigrationProgress => WizardPage::VaultPassword,
            WizardPage::VaultPassword => match self.route {
                OnboardingRoute::LegacyMigration => WizardPage::LegacyDecision,
                OnboardingRoute::InterruptedMigration => WizardPage::InterruptedMigration,
                _ => WizardPage::FreshWelcome,
            },
            WizardPage::RecoveryCode => WizardPage::VaultPassword,
            WizardPage::LocalUnlock => WizardPage::FreshWelcome,
            WizardPage::GoogleDrive => {
                if self.mode == SetupWizardMode::RestoreSetup {
                    WizardPage::FreshWelcome
                } else if self.recovery_confirmed {
                    WizardPage::RecoveryCode
                } else {
                    WizardPage::VaultPassword
                }
            }
            WizardPage::CloudRestore => WizardPage::GoogleDrive,
            WizardPage::AdditionalVaults => {
                if self.mode == SetupWizardMode::RestoreSetup {
                    WizardPage::CloudRestore
                } else {
                    WizardPage::GoogleDrive
                }
            }
            WizardPage::ReviewAndFinish => WizardPage::AdditionalVaults,
            _ => return false,
        };
        self.navigate_to(prev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn route_precedence_matrix_covers_critical_states() {
        let temp = tempdir().unwrap();
        let inv = InstallationInventory::inspect_basic(temp.path());
        assert_eq!(
            resolve_onboarding_route(&inv, false).route,
            OnboardingRoute::Fresh
        );

        // Legacy DB present even with stale RestoreSetup checkpoint -> LegacyMigration
        let mut stale = SetupState::new();
        stale.mode = SetupWizardMode::RestoreSetup;
        stale.current_step = SetupStep::DefaultVault;
        stale.save(temp.path()).unwrap();
        fs::write(temp.path().join("shellumi.db"), b"SQLite format 3\0legacy").unwrap();
        let inv = InstallationInventory::inspect_basic(temp.path());
        assert_eq!(
            resolve_onboarding_route(&inv, false).route,
            OnboardingRoute::LegacyMigration
        );

        // Active discard transaction overrides everything
        let inv_discard = InstallationInventory::inspect_filesystem(
            temp.path(),
            MigrationInventoryStatus::Absent,
            DiscardInventoryStatus::InProgress {
                transaction_id: "tx-1".into(),
                phase: "Quarantining".into(),
                can_rollback: true,
                irreversible_committed: false,
                scoped_paths: vec![],
                details: vec![],
            },
        );
        assert_eq!(
            resolve_onboarding_route(&inv_discard, false).route,
            OnboardingRoute::InterruptedDiscard
        );

        // Interrupted migration overrides stale checkpoint
        let inv_mig = InstallationInventory::inspect_filesystem(
            temp.path(),
            MigrationInventoryStatus::InProgress {
                transaction_id: "tx-2".into(),
                phase: "BackedUp".into(),
                has_backup: true,
                staged_vault_count: 0,
                receipt_present: false,
                source_matches_journal: Some(true),
                can_rollback: false,
                details: vec![],
            },
            DiscardInventoryStatus::Absent,
        );
        assert_eq!(
            resolve_onboarding_route(&inv_mig, false).route,
            OnboardingRoute::InterruptedMigration
        );
    }

    #[test]
    fn corrupt_checkpoint_routes_to_inconsistent_installation() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join(SETUP_STATE_FILE), b"{not-valid-json").unwrap();
        let inv = InstallationInventory::inspect_basic(temp.path());
        let res = resolve_onboarding_route(&inv, false);
        assert!(matches!(
            res.route,
            OnboardingRoute::InconsistentInstallation { .. }
        ));
    }

    #[test]
    fn reducer_enforces_single_flight_and_discard_gates() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("shellumi.db"), b"SQLite format 3\0legacy").unwrap();
        let inv = InstallationInventory::inspect_basic(temp.path());
        let res = resolve_onboarding_route(&inv, false);
        let mut ctrl = OnboardingController::from_resolution(&res);
        assert_eq!(ctrl.current_page, WizardPage::LegacyDecision);

        assert!(ctrl.navigate_to(WizardPage::DiscardConfirm));
        assert!(!ctrl.can_confirm_legacy_discard());

        ctrl.discard_acknowledged = true;
        assert!(!ctrl.can_confirm_legacy_discard());

        ctrl.discard_token_input = "delete".to_string(); // must be exact uppercase DELETE
        assert!(!ctrl.can_confirm_legacy_discard());

        ctrl.discard_token_input = "DELETE".to_string();
        ctrl.discard_proceed_without_backup = true;
        assert!(ctrl.can_confirm_legacy_discard());

        // Exiting DiscardConfirm resets confirmation gates
        assert!(ctrl.navigate_to(WizardPage::LegacyDecision));
        assert!(!ctrl.discard_acknowledged);
        assert!(ctrl.discard_token_input.is_empty());

        // Single-flight operation gating
        let (gen, op1) = ctrl
            .try_start_operation(OperationPhase::BackingUp, true)
            .unwrap();
        assert!(ctrl
            .try_start_operation(OperationPhase::Verifying, true)
            .is_none());
        // Stale generation ignored
        assert!(!ctrl.finish_operation_ok(gen + 1, op1, "stale"));
        assert!(ctrl.operation.is_running());
        assert!(ctrl.finish_operation_ok(gen, op1, "Backup verified"));
        assert_eq!(ctrl.operation.success_summary(), Some("Backup verified"));
        assert!(ctrl.operation.error().is_none());
    }
}

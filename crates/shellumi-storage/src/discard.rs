//! Crash-safe, explicitly authorized legacy database removal (`LegacyDiscardEngine`).
//!
//! Removes only the canonical `shellumi.db` and its verified local sidecars
//! (`-wal`, `-shm`, `-journal`) after explicit user confirmation (`DELETE` + checkbox).
//! Never touches other vaults, portable manifests, user backups, Keychain secrets,
//! or remote cloud snapshots.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use shellumi_core::onboarding::DiscardInventoryStatus;
use shellumi_core::setup::{SetupState, SetupStep, SetupWizardMode};
use shellumi_core::CredentialStore;
use uuid::Uuid;

use crate::migration::MigrationEngine;
use crate::StorageError;

const DISCARD_ROOT: &str = "vaults/.legacy_discard";
const DISCARD_COMPLETE_MARKER: &str = "vaults/.legacy_discard_complete";
const JOURNAL_FILE: &str = "journal.json";
const JOURNAL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscardPhase {
    Prepared,
    Quarantining,
    QuarantinedVerified,
    DeleteCommitted,
    Purging,
    Completed,
    RollingBack,
    RolledBack,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedFileRecord {
    pub suffix: String,
    pub original_path: PathBuf,
    pub quarantine_path: PathBuf,
    pub size_bytes: u64,
    pub sha256: String,
    pub moved_to_quarantine: bool,
    pub purged: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscardJournal {
    pub version: u32,
    pub transaction_id: String,
    pub phase: DiscardPhase,
    pub confirmed_at: u64,
    pub confirmation_token_verified: bool,
    pub files: Vec<ScopedFileRecord>,
    pub completed_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardConfirmation {
    pub acknowledged: bool,
    pub token: String,
    pub expected_source_fingerprints: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardBackupReport {
    pub backup_dir: PathBuf,
    pub files_copied: Vec<(String, PathBuf, String)>,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardReport {
    pub transaction_id: String,
    pub removed_files: Vec<PathBuf>,
    pub rolled_back_migration: bool,
    pub completed_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscardResumeOutcome {
    NothingPending,
    RolledBack { transaction_id: String },
    CompletedPurge(DiscardReport),
}

type Fault<'a> = dyn FnMut(&str) -> Result<(), StorageError> + Send + 'a;

fn failure(msg: impl Into<String>) -> StorageError {
    let msg = msg.into();
    tracing::error!(reason = %msg, "Legacy discard stopped; data preserved");
    StorageError::Serialization(msg)
}

fn io_err<T>(res: std::io::Result<T>, op: &str) -> Result<T, StorageError> {
    res.map_err(|e| failure(format!("{op}: {e}")))
}

fn no_symlink(path: &Path) -> Result<(), StorageError> {
    // Canonicalize the deepest existing ancestor so OS-level symlinks (e.g. /var -> /private/var on macOS)
    // do not trigger false positives, while still rejecting any symlink at `path` or its trailing components.
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(failure(format!(
                "Refusing discard operation through symbolic link {}",
                path.display()
            )));
        }
    }
    if let Some(parent) = path.parent() {
        if parent.exists() {
            if let Ok(meta) = fs::symlink_metadata(parent) {
                if meta.file_type().is_symlink() {
                    return Err(failure(format!(
                        "Refusing discard operation through symbolic link {}",
                        parent.display()
                    )));
                }
            }
        }
    }
    Ok(())
}

fn ensure_private_dir(path: &Path) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            ensure_private_dir(parent)?;
        }
    }
    no_symlink(path)?;
    if !path.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        io_err(builder.create(path), "Create private discard directory")?;
        if let Some(parent) = path.parent() {
            sync_dir(parent)?;
        }
    }
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), StorageError> {
    #[cfg(unix)]
    if path.exists() {
        io_err(
            File::open(path).and_then(|f| f.sync_all()),
            "Sync discard directory",
        )?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn atomic_write_private(path: &Path, data: &[u8]) -> Result<(), StorageError> {
    no_symlink(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| failure("Discard file has no parent"))?;
    ensure_private_dir(parent)?;
    let tmp = parent.join(format!(".write-{}", Uuid::new_v4()));
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = io_err(opts.open(&tmp), "Create temporary discard file")?;
    io_err(
        file.write_all(data).and_then(|()| file.sync_all()),
        "Flush discard file",
    )?;
    drop(file);
    io_err(fs::rename(&tmp, path), "Publish discard file")?;
    sync_dir(parent)
}

fn sha256_file(path: &Path) -> Result<String, StorageError> {
    no_symlink(path)?;
    let mut f = io_err(File::open(path), "Open file for digest")?;
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buf = [0u8; 65536];
    loop {
        let n = io_err(f.read(&mut buf), "Read file for digest")?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    Ok(hex::encode(ctx.finish()))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn save_journal(tx_dir: &Path, journal: &DiscardJournal) -> Result<(), StorageError> {
    let bytes = serde_json::to_vec_pretty(journal)
        .map_err(|e| failure(format!("Serialize discard journal: {e}")))?;
    atomic_write_private(&tx_dir.join(JOURNAL_FILE), &bytes)?;
    tracing::debug!(
        transaction_id = %journal.transaction_id,
        phase = ?journal.phase,
        "Legacy discard checkpoint persisted"
    );
    Ok(())
}

fn load_journal(tx_dir: &Path) -> Result<DiscardJournal, StorageError> {
    let jpath = tx_dir.join(JOURNAL_FILE);
    no_symlink(&jpath)?;
    let bytes = io_err(fs::read(&jpath), "Read legacy discard journal")?;
    let journal: DiscardJournal = serde_json::from_slice(&bytes)
        .map_err(|e| failure(format!("Invalid legacy discard journal: {e}")))?;
    if journal.version != JOURNAL_VERSION || Uuid::parse_str(&journal.transaction_id).is_err() {
        return Err(failure(
            "Unsupported legacy discard journal version or transaction ID",
        ));
    }
    if !journal.confirmation_token_verified {
        return Err(failure(
            "Legacy discard journal lacks verified confirmation token",
        ));
    }
    Ok(journal)
}

fn find_active_tx_dir(base: &Path) -> Result<Option<PathBuf>, StorageError> {
    let root = base.join(DISCARD_ROOT);
    no_symlink(&root)?;
    if !root.exists() {
        return Ok(None);
    }
    let mut active = Vec::new();
    for entry in io_err(fs::read_dir(&root), "Inspect discard staging directory")? {
        let entry = io_err(entry, "Inspect discard entry")?;
        let path = entry.path();
        if path.is_dir() && path.join(JOURNAL_FILE).exists() {
            active.push(path);
        }
    }
    if active.len() > 1 {
        return Err(failure(
            "Multiple legacy discard transactions exist; refusing automatic action",
        ));
    }
    Ok(active.into_iter().next())
}

fn move_to_quarantine(src: &Path, dst: &Path) -> Result<(), StorageError> {
    no_symlink(src)?;
    no_symlink(dst)?;
    if dst.exists() {
        return Err(failure(format!(
            "Quarantine destination already exists: {}",
            dst.display()
        )));
    }
    if let Some(parent) = dst.parent() {
        ensure_private_dir(parent)?;
    }
    match fs::rename(src, dst) {
        Ok(()) => {
            if let Some(p) = dst.parent() {
                sync_dir(p)?;
            }
            if let Some(p) = src.parent() {
                sync_dir(p)?;
            }
            Ok(())
        }
        Err(_) => {
            // Cross-filesystem fallback: verified copy + fsync + create-only publish + remove source.
            let expected = sha256_file(src)?;
            let data = io_err(fs::read(src), "Read source for cross-device quarantine")?;
            let mut opts = OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut file = io_err(opts.open(dst), "Create quarantine copy")?;
            io_err(
                file.write_all(&data).and_then(|()| file.sync_all()),
                "Flush quarantine copy",
            )?;
            drop(file);
            if sha256_file(dst)? != expected {
                let _ = fs::remove_file(dst);
                return Err(failure("Cross-device quarantine copy failed digest check"));
            }
            io_err(fs::remove_file(src), "Remove source after verified copy")?;
            if let Some(p) = dst.parent() {
                sync_dir(p)?;
            }
            if let Some(p) = src.parent() {
                sync_dir(p)?;
            }
            Ok(())
        }
    }
}

fn verify_modern_vault_absent(base: &Path) -> Result<(), StorageError> {
    let default_db = base.join("vaults/default.db");
    let vm_manifest = base.join("vault_manifests/legacy-local.manifest.json");
    let alt_manifest = base.join("vault_manifests/default.manifest.json");
    let lm_manifest = base.join("manifests/legacy-local.manifest.json");
    if default_db.exists()
        && (vm_manifest.exists() || alt_manifest.exists() || lm_manifest.exists())
    {
        return Err(failure(
            "An active modern Default Vault already exists; legacy discard cannot delete or overwrite modern vaults",
        ));
    }
    Ok(())
}

/// Service managing crash-safe, explicitly authorized legacy database removal.
pub struct LegacyDiscardEngine;

impl LegacyDiscardEngine {
    /// Computes SHA-256 fingerprints of `shellumi.db` and its scoped sidecars for confirmation binding.
    pub fn compute_source_fingerprints(
        base_dir: &Path,
    ) -> Result<BTreeMap<String, String>, StorageError> {
        let base = io_err(base_dir.canonicalize(), "Locate Shellumi data directory")?;
        no_symlink(&base)?;
        let mut fps = BTreeMap::new();
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let src = base.join(format!("shellumi.db{suffix}"));
            no_symlink(&src)?;
            if src.exists() {
                fps.insert(suffix.to_string(), sha256_file(&src)?);
            }
        }
        if !fps.contains_key("") {
            return Err(failure("Legacy shellumi.db is absent"));
        }
        Ok(fps)
    }

    /// Read-only inspection of `vaults/.legacy_discard` and completion marker.
    pub fn inspect_state(base_dir: &Path) -> DiscardInventoryStatus {
        let root = base_dir.join(DISCARD_ROOT);
        if let Err(e) = no_symlink(&root) {
            return DiscardInventoryStatus::InvalidOrCorrupt {
                reason: e.to_string(),
            };
        }
        if !root.exists() {
            let marker = base_dir.join(DISCARD_COMPLETE_MARKER);
            if marker.exists() {
                if let Ok(bytes) = fs::read(&marker) {
                    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        let tx = v
                            .get("transaction_id")
                            .and_then(|s| s.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let ts = v.get("completed_at").and_then(|n| n.as_u64());
                        return DiscardInventoryStatus::Complete {
                            transaction_id: tx,
                            completed_at: ts,
                        };
                    }
                }
            }
            return DiscardInventoryStatus::Absent;
        }

        match find_active_tx_dir(base_dir) {
            Ok(Some(tx_dir)) => match load_journal(&tx_dir) {
                Ok(journal) => {
                    if journal.phase == DiscardPhase::Completed {
                        DiscardInventoryStatus::Complete {
                            transaction_id: journal.transaction_id,
                            completed_at: journal.completed_at,
                        }
                    } else if journal.phase == DiscardPhase::RolledBack {
                        DiscardInventoryStatus::Absent
                    } else {
                        let irreversible = matches!(
                            journal.phase,
                            DiscardPhase::DeleteCommitted | DiscardPhase::Purging
                        );
                        let can_rollback = matches!(
                            journal.phase,
                            DiscardPhase::Prepared
                                | DiscardPhase::Quarantining
                                | DiscardPhase::QuarantinedVerified
                        );
                        let scoped_paths = journal
                            .files
                            .iter()
                            .map(|f| f.original_path.clone())
                            .collect();
                        let details = vec![
                            format!("Transaction ID: {}", journal.transaction_id),
                            format!("Phase: {:?}", journal.phase),
                            format!("Scoped files: {}", journal.files.len()),
                        ];
                        DiscardInventoryStatus::InProgress {
                            transaction_id: journal.transaction_id,
                            phase: format!("{:?}", journal.phase),
                            can_rollback,
                            irreversible_committed: irreversible,
                            scoped_paths,
                            details,
                        }
                    }
                }
                Err(e) => DiscardInventoryStatus::InvalidOrCorrupt {
                    reason: e.to_string(),
                },
            },
            Ok(None) => DiscardInventoryStatus::InvalidOrCorrupt {
                reason: "Discard directory exists without a valid transaction journal".to_string(),
            },
            Err(e) => DiscardInventoryStatus::InvalidOrCorrupt {
                reason: e.to_string(),
            },
        }
    }

    /// Creates a verified offline backup of `shellumi.db` and its scoped sidecars in `dest_dir`.
    /// Rejects destinations inside `base_dir` or through symlinks.
    pub fn create_backup(
        base_dir: &Path,
        dest_dir: &Path,
    ) -> Result<DiscardBackupReport, StorageError> {
        let base = io_err(base_dir.canonicalize(), "Locate source data directory")?;
        no_symlink(&base)?;
        no_symlink(dest_dir)?;

        if dest_dir.exists() {
            if let Ok(canon_dest) = dest_dir.canonicalize() {
                if canon_dest.starts_with(&base) {
                    return Err(failure(
                        "Backup destination cannot be inside the Shellumi data directory targeted for removal",
                    ));
                }
            }
        } else if let Some(parent) = dest_dir.parent() {
            if parent.exists() {
                if let Ok(canon_parent) = parent.canonicalize() {
                    if canon_parent.starts_with(&base) {
                        return Err(failure(
                            "Backup destination cannot be inside the Shellumi data directory targeted for removal",
                        ));
                    }
                }
            }
        }

        ensure_private_dir(dest_dir)?;

        let mut files_copied = Vec::new();
        let mut total_bytes = 0u64;
        let mut found_main = false;

        for suffix in ["", "-wal", "-shm", "-journal"] {
            let src = base.join(format!("shellumi.db{suffix}"));
            no_symlink(&src)?;
            if !src.exists() {
                continue;
            }
            if suffix.is_empty() {
                found_main = true;
            }
            let digest = sha256_file(&src)?;
            let data = io_err(fs::read(&src), "Read legacy file for backup")?;
            let target = dest_dir.join(format!("shellumi.db{suffix}"));
            if target.exists() {
                return Err(failure(format!(
                    "Backup destination already contains {}",
                    target.display()
                )));
            }
            atomic_write_private(&target, &data)?;
            if sha256_file(&target)? != digest {
                return Err(failure(format!(
                    "Backup verification failed for {}",
                    target.display()
                )));
            }
            total_bytes += data.len() as u64;
            files_copied.push((suffix.to_string(), target, digest));
        }

        if !found_main {
            return Err(failure("No legacy shellumi.db found to back up"));
        }

        tracing::info!(
            backup_dir = %dest_dir.display(),
            file_count = files_copied.len(),
            total_bytes,
            "Verified offline legacy database backup created"
        );

        Ok(DiscardBackupReport {
            backup_dir: dest_dir.to_path_buf(),
            files_copied,
            total_bytes,
        })
    }

    /// Executes explicitly confirmed legacy database discard.
    pub fn execute_discard(
        base_dir: &Path,
        confirmation: &DiscardConfirmation,
        store: &dyn CredentialStore,
    ) -> Result<DiscardReport, StorageError> {
        Self::execute_discard_with_fault(base_dir, confirmation, store, &mut |_| Ok(()))
    }

    pub fn execute_discard_with_fault(
        base_dir: &Path,
        confirmation: &DiscardConfirmation,
        _store: &dyn CredentialStore,
        fault: &mut Fault<'_>,
    ) -> Result<DiscardReport, StorageError> {
        if !confirmation.acknowledged || confirmation.token != "DELETE" {
            return Err(failure(
                "Explicit confirmation (acknowledgement and exact 'DELETE' token) is required",
            ));
        }

        let base = io_err(base_dir.canonicalize(), "Locate Shellumi data directory")?;
        no_symlink(&base)?;
        no_symlink(&base.join(".migration.lock"))?;

        let lock = io_err(
            OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(base.join(".migration.lock")),
            "Open exclusive onboarding lock",
        )?;
        lock.try_lock().map_err(|e| {
            failure(format!(
                "Another storage operation is running; retry after it exits: {e}"
            ))
        })?;

        verify_modern_vault_absent(&base)?;

        // If an uncommitted migration exists, roll back owned outputs safely first.
        let rolled_back_migration = MigrationEngine::rollback_if_uncommitted(&base)?;
        verify_modern_vault_absent(&base)?;

        // Check if a discard transaction is already in progress.
        if let Some(tx_dir) = find_active_tx_dir(&base)? {
            let mut journal = load_journal(&tx_dir)?;
            if matches!(
                journal.phase,
                DiscardPhase::DeleteCommitted | DiscardPhase::Purging | DiscardPhase::Completed
            ) {
                return Self::drive_journal(
                    &base,
                    &tx_dir,
                    &mut journal,
                    rolled_back_migration,
                    fault,
                );
            }
        }

        // Build fresh scope from canonical legacy files.
        let tx_id = Uuid::new_v4().to_string();
        let tx_dir = base.join(DISCARD_ROOT).join(&tx_id);
        let quarantine_dir = tx_dir.join("quarantine");

        let mut files = Vec::new();
        let mut current_fps = BTreeMap::new();
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let src = base.join(format!("shellumi.db{suffix}"));
            no_symlink(&src)?;
            if !src.exists() {
                continue;
            }
            let meta = io_err(fs::symlink_metadata(&src), "Inspect legacy file metadata")?;
            if !meta.is_file() {
                return Err(failure(format!("{} is not a regular file", src.display())));
            }
            let sha = sha256_file(&src)?;
            current_fps.insert(suffix.to_string(), sha.clone());
            files.push(ScopedFileRecord {
                suffix: suffix.to_string(),
                original_path: src,
                quarantine_path: quarantine_dir.join(format!("shellumi.db{suffix}")),
                size_bytes: meta.len(),
                sha256: sha,
                moved_to_quarantine: false,
                purged: false,
            });
        }

        if !files.iter().any(|f| f.suffix.is_empty()) {
            return Err(failure("Legacy shellumi.db is absent; nothing to remove"));
        }

        if let Some(expected) = &confirmation.expected_source_fingerprints {
            if expected != &current_fps {
                return Err(failure(
                    "Legacy database files changed since confirmation was prepared; reconfirm before removing",
                ));
            }
        }

        ensure_private_dir(&quarantine_dir)?;
        let mut journal = DiscardJournal {
            version: JOURNAL_VERSION,
            transaction_id: tx_id,
            phase: DiscardPhase::Prepared,
            confirmed_at: now_secs(),
            confirmation_token_verified: true,
            files,
            completed_at: None,
        };
        save_journal(&tx_dir, &journal)?;
        fault("prepared")?;

        Self::drive_journal(&base, &tx_dir, &mut journal, rolled_back_migration, fault)
    }

    /// Resumes or rolls back an interrupted discard transaction on startup.
    pub fn resume_or_rollback(
        base_dir: &Path,
        _store: &dyn CredentialStore,
    ) -> Result<DiscardResumeOutcome, StorageError> {
        Self::resume_or_rollback_with_fault(base_dir, &mut |_| Ok(()))
    }

    pub fn resume_or_rollback_with_fault(
        base_dir: &Path,
        fault: &mut Fault<'_>,
    ) -> Result<DiscardResumeOutcome, StorageError> {
        let base = io_err(base_dir.canonicalize(), "Locate Shellumi data directory")?;
        no_symlink(&base)?;
        let Some(tx_dir) = find_active_tx_dir(&base)? else {
            return Ok(DiscardResumeOutcome::NothingPending);
        };
        no_symlink(&base.join(".migration.lock"))?;
        let lock = io_err(
            OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(base.join(".migration.lock")),
            "Open exclusive onboarding lock",
        )?;
        lock.try_lock().map_err(|e| {
            failure(format!(
                "Another storage operation is running; retry after it exits: {e}"
            ))
        })?;

        let mut journal = load_journal(&tx_dir)?;
        match journal.phase {
            DiscardPhase::Prepared
            | DiscardPhase::Quarantining
            | DiscardPhase::QuarantinedVerified
            | DiscardPhase::RollingBack => {
                journal.phase = DiscardPhase::RollingBack;
                save_journal(&tx_dir, &journal)?;
                fault("rollback_started")?;
                for idx in (0..journal.files.len()).rev() {
                    if journal.files[idx].moved_to_quarantine {
                        let q = &journal.files[idx].quarantine_path;
                        let orig = &journal.files[idx].original_path;
                        if orig.exists() {
                            return Err(failure(format!(
                                "Cannot roll back legacy discard: {} already exists",
                                orig.display()
                            )));
                        }
                        if q.exists() {
                            if sha256_file(q)? != journal.files[idx].sha256 {
                                return Err(failure(
                                    "Quarantined file digest mismatch during rollback",
                                ));
                            }
                            move_to_quarantine(q, orig)?;
                        }
                        journal.files[idx].moved_to_quarantine = false;
                        save_journal(&tx_dir, &journal)?;
                        fault(&format!("rollback_file:{idx}"))?;
                    }
                }
                journal.phase = DiscardPhase::RolledBack;
                save_journal(&tx_dir, &journal)?;
                let tx_id = journal.transaction_id.clone();
                let _ = fs::remove_dir_all(&tx_dir);
                let root = base.join(DISCARD_ROOT);
                if root.exists()
                    && fs::read_dir(&root)
                        .map(|mut d| d.next().is_none())
                        .unwrap_or(false)
                {
                    let _ = fs::remove_dir(&root);
                }
                Ok(DiscardResumeOutcome::RolledBack {
                    transaction_id: tx_id,
                })
            }
            DiscardPhase::DeleteCommitted | DiscardPhase::Purging | DiscardPhase::Completed => {
                let report = Self::drive_journal(&base, &tx_dir, &mut journal, false, fault)?;
                Ok(DiscardResumeOutcome::CompletedPurge(report))
            }
            DiscardPhase::RolledBack => Ok(DiscardResumeOutcome::NothingPending),
        }
    }

    fn drive_journal(
        base: &Path,
        tx_dir: &Path,
        journal: &mut DiscardJournal,
        rolled_back_migration: bool,
        fault: &mut Fault<'_>,
    ) -> Result<DiscardReport, StorageError> {
        if journal.phase == DiscardPhase::Prepared {
            journal.phase = DiscardPhase::Quarantining;
            save_journal(tx_dir, journal)?;
        }

        if journal.phase == DiscardPhase::Quarantining {
            for idx in 0..journal.files.len() {
                if !journal.files[idx].moved_to_quarantine {
                    let orig = &journal.files[idx].original_path;
                    let q = &journal.files[idx].quarantine_path;
                    if orig.exists() {
                        if sha256_file(orig)? != journal.files[idx].sha256 {
                            return Err(failure(
                                "Legacy source changed during quarantine; aborting discard",
                            ));
                        }
                        move_to_quarantine(orig, q)?;
                    } else if !q.exists() {
                        return Err(failure(format!(
                            "Scoped file {} vanished before quarantine",
                            orig.display()
                        )));
                    }
                    journal.files[idx].moved_to_quarantine = true;
                    save_journal(tx_dir, journal)?;
                    fault(&format!("quarantined:{idx}"))?;
                }
            }
            for f in &journal.files {
                if f.original_path.exists() {
                    return Err(failure("Source file still exists after quarantine"));
                }
                if !f.quarantine_path.exists() || sha256_file(&f.quarantine_path)? != f.sha256 {
                    return Err(failure("Quarantine verification failed"));
                }
            }
            journal.phase = DiscardPhase::QuarantinedVerified;
            save_journal(tx_dir, journal)?;
            fault("quarantined_verified")?;
        }

        if journal.phase == DiscardPhase::QuarantinedVerified {
            verify_modern_vault_absent(base)?;
            journal.phase = DiscardPhase::DeleteCommitted;
            save_journal(tx_dir, journal)?;
            fault("delete_committed")?;
        }

        if journal.phase == DiscardPhase::DeleteCommitted {
            journal.phase = DiscardPhase::Purging;
            save_journal(tx_dir, journal)?;
        }

        if journal.phase == DiscardPhase::Purging {
            for idx in 0..journal.files.len() {
                if !journal.files[idx].purged {
                    let q = &journal.files[idx].quarantine_path;
                    no_symlink(q)?;
                    if q.exists() {
                        if sha256_file(q)? != journal.files[idx].sha256 {
                            return Err(failure(
                                "Quarantined file digest mismatch before purge; refusing deletion",
                            ));
                        }
                        io_err(fs::remove_file(q), "Purge quarantined legacy file")?;
                        if let Some(p) = q.parent() {
                            sync_dir(p)?;
                        }
                    }
                    journal.files[idx].purged = true;
                    save_journal(tx_dir, journal)?;
                    fault(&format!("purged:{idx}"))?;
                }
            }

            // Postcondition verification: canonical source and sidecars must be absent.
            for f in &journal.files {
                if f.original_path.exists() || f.quarantine_path.exists() {
                    return Err(failure(
                        "Legacy discard postcondition failed: scoped file still exists",
                    ));
                }
            }

            let completed_at = now_secs();
            journal.completed_at = Some(completed_at);
            journal.phase = DiscardPhase::Completed;
            save_journal(tx_dir, journal)?;
            fault("purged_verified")?;
        }

        let completed_at = journal.completed_at.unwrap_or_else(now_secs);
        let marker_payload = serde_json::json!({
            "version": JOURNAL_VERSION,
            "transaction_id": journal.transaction_id,
            "completed_at": completed_at,
        });
        atomic_write_private(
            &base.join(DISCARD_COMPLETE_MARKER),
            marker_payload.to_string().as_bytes(),
        )?;

        // Reconcile setup checkpoint to Fresh.
        let mut fresh_state = SetupState::new();
        fresh_state.mode = SetupWizardMode::NewSetup;
        fresh_state.current_step = SetupStep::Welcome;
        fresh_state.last_transaction_id = Some(journal.transaction_id.clone());
        fresh_state.discard_completed_at = Some(completed_at);
        fresh_state
            .save(base)
            .map_err(|e| failure(format!("Reconcile setup checkpoint after discard: {e}")))?;

        let removed_files = journal
            .files
            .iter()
            .map(|f| f.original_path.clone())
            .collect();
        let tx_id = journal.transaction_id.clone();

        // Clean up empty staging directory after completion receipt is durable.
        let _ = fs::remove_dir_all(tx_dir);
        let root = base.join(DISCARD_ROOT);
        if root.exists()
            && fs::read_dir(&root)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false)
        {
            let _ = fs::remove_dir(&root);
        }

        tracing::info!(
            transaction_id = %tx_id,
            completed_at,
            "Legacy database discard completed and setup checkpoint reconciled to Fresh"
        );

        Ok(DiscardReport {
            transaction_id: tx_id,
            removed_files,
            rolled_back_migration,
            completed_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shellumi_core::EphemeralCredentialStore;
    use tempfile::tempdir;

    fn seed_legacy_root(base: &Path) {
        fs::write(base.join("shellumi.db"), b"SQLite format 3\0legacy-main").unwrap();
        fs::write(base.join("shellumi.db-wal"), b"wal-bytes").unwrap();
        fs::write(base.join("shellumi.db-shm"), b"shm-bytes").unwrap();
        fs::write(base.join("shellumi.db-journal"), b"journal-bytes").unwrap();
        // Unrelated files that must survive
        fs::create_dir_all(base.join("ssh_keys")).unwrap();
        fs::write(base.join("ssh_keys/id_ed25519"), b"private-key-data").unwrap();
        fs::create_dir_all(base.join("recordings")).unwrap();
        fs::write(base.join("recordings/session.cast"), b"cast-data").unwrap();
    }

    #[test]
    fn rejects_invalid_confirmations_and_preserves_all_files() {
        let temp = tempdir().unwrap();
        seed_legacy_root(temp.path());
        let store = EphemeralCredentialStore::new();

        for conf in [
            DiscardConfirmation {
                acknowledged: false,
                token: "DELETE".into(),
                expected_source_fingerprints: None,
            },
            DiscardConfirmation {
                acknowledged: true,
                token: "delete".into(),
                expected_source_fingerprints: None,
            },
            DiscardConfirmation {
                acknowledged: true,
                token: "".into(),
                expected_source_fingerprints: None,
            },
        ] {
            let err = LegacyDiscardEngine::execute_discard(temp.path(), &conf, &store).unwrap_err();
            assert!(err.to_string().contains("confirmation"));
            assert!(temp.path().join("shellumi.db").exists());
            assert!(temp.path().join("shellumi.db-wal").exists());
        }
    }

    #[test]
    fn exact_confirmation_removes_only_scoped_files_and_reconciles_fresh() {
        let temp = tempdir().unwrap();
        seed_legacy_root(temp.path());
        let store = EphemeralCredentialStore::new();
        let conf = DiscardConfirmation {
            acknowledged: true,
            token: "DELETE".into(),
            expected_source_fingerprints: None,
        };

        let report = LegacyDiscardEngine::execute_discard(temp.path(), &conf, &store).unwrap();
        assert_eq!(report.removed_files.len(), 4);
        assert!(!temp.path().join("shellumi.db").exists());
        assert!(!temp.path().join("shellumi.db-wal").exists());
        assert!(!temp.path().join("shellumi.db-shm").exists());
        assert!(!temp.path().join("shellumi.db-journal").exists());

        // Unrelated artifacts preserved
        assert!(temp.path().join("ssh_keys/id_ed25519").exists());
        assert!(temp.path().join("recordings/session.cast").exists());

        // Setup checkpoint reconciled to Fresh
        let state = SetupState::load(temp.path()).unwrap().unwrap();
        assert!(!state.completed);
        assert_eq!(state.current_step, SetupStep::Welcome);
        assert!(state.discard_completed_at.is_some());
    }

    #[test]
    fn backup_verifies_files_and_rejects_inside_data_root() {
        let temp = tempdir().unwrap();
        seed_legacy_root(temp.path());

        let inside_err =
            LegacyDiscardEngine::create_backup(temp.path(), &temp.path().join("backup"))
                .unwrap_err();
        assert!(inside_err.to_string().contains("inside"));

        let backup_root = tempdir().unwrap();
        let report = LegacyDiscardEngine::create_backup(temp.path(), backup_root.path()).unwrap();
        assert_eq!(report.files_copied.len(), 4);
        assert!(backup_root.path().join("shellumi.db").exists());
        assert!(backup_root.path().join("shellumi.db-wal").exists());
    }

    #[test]
    fn crash_before_commit_rolls_back_and_after_commit_resumes_purge() {
        let store = EphemeralCredentialStore::new();
        let conf = DiscardConfirmation {
            acknowledged: true,
            token: "DELETE".into(),
            expected_source_fingerprints: None,
        };

        // Pre-commit crash points must roll back cleanly and restore source files.
        for point in ["prepared", "quarantined:0", "quarantined_verified"] {
            let temp = tempdir().unwrap();
            seed_legacy_root(temp.path());
            let mut hit = false;
            let err = LegacyDiscardEngine::execute_discard_with_fault(
                temp.path(),
                &conf,
                &store,
                &mut |ev| {
                    if ev == point && !hit {
                        hit = true;
                        Err(StorageError::Serialization("simulated crash".into()))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            assert!(err.to_string().contains("simulated crash"));
            assert!(matches!(
                LegacyDiscardEngine::inspect_state(temp.path()),
                DiscardInventoryStatus::InProgress {
                    irreversible_committed: false,
                    ..
                }
            ));

            let outcome = LegacyDiscardEngine::resume_or_rollback(temp.path(), &store).unwrap();
            assert!(matches!(outcome, DiscardResumeOutcome::RolledBack { .. }));
            assert!(temp.path().join("shellumi.db").exists());
            assert!(temp.path().join("shellumi.db-wal").exists());
        }

        // Post-commit crash points must resume purge idempotently.
        for point in ["delete_committed", "purged:0", "purged_verified"] {
            let temp = tempdir().unwrap();
            seed_legacy_root(temp.path());
            let mut hit = false;
            let _ = LegacyDiscardEngine::execute_discard_with_fault(
                temp.path(),
                &conf,
                &store,
                &mut |ev| {
                    if ev == point && !hit {
                        hit = true;
                        Err(StorageError::Serialization("simulated crash".into()))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            if point == "purged_verified" {
                assert!(matches!(
                    LegacyDiscardEngine::inspect_state(temp.path()),
                    DiscardInventoryStatus::Complete { .. }
                ));
            } else {
                assert!(matches!(
                    LegacyDiscardEngine::inspect_state(temp.path()),
                    DiscardInventoryStatus::InProgress {
                        irreversible_committed: true,
                        ..
                    }
                ));
            }

            let outcome = LegacyDiscardEngine::resume_or_rollback(temp.path(), &store).unwrap();
            assert!(matches!(outcome, DiscardResumeOutcome::CompletedPurge(_)));
            assert!(!temp.path().join("shellumi.db").exists());
            assert!(!temp.path().join("shellumi.db-wal").exists());
        }
    }

    #[test]
    fn blocks_discard_when_modern_default_vault_exists() {
        let temp = tempdir().unwrap();
        seed_legacy_root(temp.path());
        fs::create_dir_all(temp.path().join("vaults")).unwrap();
        fs::create_dir_all(temp.path().join("vault_manifests")).unwrap();
        fs::write(temp.path().join("vaults/default.db"), b"encrypted").unwrap();
        fs::write(
            temp.path()
                .join("vault_manifests/legacy-local.manifest.json"),
            b"{}",
        )
        .unwrap();

        let store = EphemeralCredentialStore::new();
        let conf = DiscardConfirmation {
            acknowledged: true,
            token: "DELETE".into(),
            expected_source_fingerprints: None,
        };
        let err = LegacyDiscardEngine::execute_discard(temp.path(), &conf, &store).unwrap_err();
        assert!(err.to_string().contains("modern Default Vault"));
        assert!(temp.path().join("shellumi.db").exists());
        assert!(temp.path().join("vaults/default.db").exists());
    }
}

//! Transactional vault password rotation coordinator and crash-recovery engine.
//!
//! Coordinates atomic-like staged updates across all four vault persistence stores:
//! 1. Vault manifest (`<base>/manifests/<vault_id>.manifest.json`)
//! 2. Target encrypted vault database (`<base>/vaults/<vault_id>.db`)
//! 3. Registry database (`<base>/vaults/default.db`)
//! 4. OS credential store (`CredentialStore`)
//!
//! Never persists plaintext passwords, recovery codes, or raw DEKs in journals or logs.

use crate::data_path::{
    self, is_safe_vault_id, write_durable_file, PasswordRotationJournal, PasswordRotationStage,
};
use shellumi_core::{
    validate_vault_password, CredentialStore, VaultId, VaultKey, VaultKeyManager, VaultManifest,
};
use shellumi_storage::{HostStore, LEGACY_LOCAL_VAULT_ID};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// Process-wide exclusive lock per vault ID to prevent concurrent password rotations.
static ACTIVE_ROTATIONS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

struct RotationLockGuard {
    vault_id: String,
}

impl RotationLockGuard {
    fn acquire(vault_id: &str) -> Result<Self, String> {
        let mut guard = ACTIVE_ROTATIONS
            .lock()
            .map_err(|_| "Password rotation lock poisoned".to_string())?;
        if guard.contains(vault_id) {
            return Err(format!(
                "A password rotation is already in progress for vault '{vault_id}'."
            ));
        }
        guard.insert(vault_id.to_string());
        Ok(Self {
            vault_id: vault_id.to_string(),
        })
    }
}

impl Drop for RotationLockGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = ACTIVE_ROTATIONS.lock() {
            guard.remove(&self.vault_id);
        }
    }
}

/// Deterministic fault-injection points for testing crash and rollback recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationFaultPoint {
    BeforeJournalCreate,
    AfterJournalCreate,
    BeforeManifestUpdate,
    AfterManifestUpdate,
    BeforeTargetDbUpdate,
    AfterTargetDbUpdate,
    BeforeRegistryUpdate,
    AfterRegistryUpdate,
    BeforeCredentialStoreUpdate,
    AfterCredentialStoreUpdate,
    BeforeCommitVerify,
    AfterCommitVerify,
    BeforeJournalComplete,
    AfterJournalComplete,
    BeforeRollback,
    AfterRollback,
}

pub type FaultInjector = dyn Fn(RotationFaultPoint) -> Result<(), String> + Send + Sync;

/// Input parameters for rotating an existing vault's password.
pub struct PasswordRotationRequest<'a> {
    pub base_path: &'a Path,
    pub vault_id: &'a str,
    pub old_password: &'a str,
    pub new_password: &'a str,
    pub expected_in_memory_key: Option<&'a VaultKey>,
    pub default_vault_key: Option<&'a VaultKey>,
    pub cred_store: &'a dyn CredentialStore,
    pub fault_hook: Option<&'a FaultInjector>,
}

#[allow(dead_code)]
/// Outcome of a successful password rotation.
#[derive(Clone)]
pub struct PasswordRotationOutcome {
    pub vault_id: String,
    pub key: VaultKey,
}

/// Describes an interrupted password rotation that requires user authentication or manual inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRotationRecovery {
    pub vault_id: String,
    pub journal_path: PathBuf,
    pub is_corrupted: bool,
    pub stage: Option<PasswordRotationStage>,
    pub reason: String,
}

#[allow(dead_code)]
/// Outcome of reconciling a pending password rotation.
#[derive(Clone)]
pub struct RotationReconciliationOutcome {
    pub vault_id: String,
    pub key: VaultKey,
    pub committed_new_password: bool,
}

fn check_fault(hook: Option<&FaultInjector>, point: RotationFaultPoint) -> Result<(), String> {
    if let Some(f) = hook {
        f(point)?;
    }
    Ok(())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn rotation_journal_path(base_path: &Path, vault_id: &str) -> PathBuf {
    data_path::resolve_manifest_dir(base_path).join(format!("{vault_id}.rotation_journal.json"))
}

fn reject_if_symlink(path: &Path) -> Result<(), String> {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(format!(
                "Refusing to operate on symlinked path: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

async fn verify_encrypted_records_readable(db_path: &Path, key: &VaultKey) -> Result<(), String> {
    let path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid UTF-8 database path".to_string())?;
    let store = HostStore::open_with_key(path_str, Some(key))
        .await
        .map_err(|e| {
            format!(
                "Failed to open encrypted vault database {}: {e}",
                db_path.display()
            )
        })?;
    // Query representative encrypted tables to verify DEK readability.
    let _ = store
        .list()
        .await
        .map_err(|e| format!("Failed to verify encrypted host records: {e}"))?;
    let _ = store
        .list_groups()
        .await
        .map_err(|e| format!("Failed to verify encrypted group records: {e}"))?;
    Ok(())
}

async fn write_db_envelope(
    db_path: &Path,
    db_key: Option<&VaultKey>,
    vault_id: &str,
    envelope_json: &str,
    require_row: bool,
) -> Result<(), String> {
    let path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid UTF-8 database path".to_string())?;
    let mut store = HostStore::open_with_key(path_str, db_key)
        .await
        .map_err(|e| format!("Failed to open database {}: {e}", db_path.display()))?;
    let updated = store
        .save_vault_recovery_envelope(vault_id, envelope_json)
        .await
        .map_err(|e| {
            format!(
                "Failed to save recovery envelope in {}: {e}",
                db_path.display()
            )
        })?;
    if require_row && !updated {
        return Err(format!(
            "Vault context row for '{vault_id}' missing in {}",
            db_path.display()
        ));
    }
    Ok(())
}

async fn read_db_envelope(
    db_path: &Path,
    db_key: Option<&VaultKey>,
    vault_id: &str,
) -> Result<Option<String>, String> {
    let path_str = db_path
        .to_str()
        .ok_or_else(|| "Invalid UTF-8 database path".to_string())?;
    let store = HostStore::open_with_key(path_str, db_key)
        .await
        .map_err(|e| format!("Failed to open database {}: {e}", db_path.display()))?;
    store
        .get_vault_recovery_envelope(vault_id)
        .await
        .map_err(|e| {
            format!(
                "Failed to read recovery envelope from {}: {e}",
                db_path.display()
            )
        })
}

/// Executes a full transactional vault password rotation across manifest, target DB,
/// registry DB, and OS credential store, with durable journaling and staged rollback.
pub async fn rotate_vault_password(
    req: PasswordRotationRequest<'_>,
) -> Result<PasswordRotationOutcome, String> {
    if !is_safe_vault_id(req.vault_id) {
        return Err(format!("Invalid vault ID '{}'.", req.vault_id));
    }
    let vid = VaultId::new(req.vault_id).map_err(|e| e.to_string())?;
    let _lock = RotationLockGuard::acquire(req.vault_id)?;

    let journal_path = rotation_journal_path(req.base_path, req.vault_id);
    if journal_path.exists() {
        return Err(format!(
            "An unfinished password rotation journal already exists for vault '{}'. Reconcile recovery first.",
            req.vault_id
        ));
    }

    let manifest_path = data_path::resolve_manifest_file(req.base_path, req.vault_id);
    let target_db_path = data_path::resolve_vault_db_file(req.base_path, req.vault_id);
    let registry_db_path = data_path::resolve_vault_db_file(req.base_path, LEGACY_LOCAL_VAULT_ID);

    reject_if_symlink(&manifest_path)?;
    reject_if_symlink(&target_db_path)?;
    reject_if_symlink(&registry_db_path)?;
    reject_if_symlink(&journal_path)?;

    if !manifest_path.exists() {
        return Err(format!(
            "Vault manifest not found for '{}'; cannot rotate password.",
            req.vault_id
        ));
    }

    // 1. Read and validate existing manifest + old password
    let original_manifest_backup = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read vault manifest: {e}"))?;
    let original_manifest: VaultManifest = serde_json::from_str(&original_manifest_backup)
        .map_err(|e| format!("Corrupted vault manifest: {e}"))?;
    if original_manifest.vault_id != req.vault_id {
        return Err(format!(
            "Manifest vault ID mismatch: expected '{}', found '{}'.",
            req.vault_id, original_manifest.vault_id
        ));
    }

    let dek = original_manifest
        .unlock(req.old_password)
        .map_err(|_| "Current password is incorrect.".to_string())?;

    if let Some(in_mem) = req.expected_in_memory_key {
        if in_mem.expose_bytes() != dek.expose_bytes() {
            return Err("Current password does not match the active unlocked vault key.".into());
        }
    }

    // 2. Validate new password
    validate_vault_password(req.new_password)
        .map_err(|e| format!("New password does not meet security requirements: {e}"))?;
    if req.new_password == req.old_password {
        return Err("New password must differ from the current password.".into());
    }

    // 3. Verify target DB is readable with DEK & capture original envelopes
    verify_encrypted_records_readable(&target_db_path, &dek).await?;

    let is_same_db = target_db_path == registry_db_path;
    let reg_key = if is_same_db {
        Some(&dek)
    } else {
        req.default_vault_key
    };

    let original_target_db_envelope = if !is_same_db {
        read_db_envelope(&target_db_path, Some(&dek), req.vault_id).await?
    } else {
        None
    };

    let original_registry_envelope =
        read_db_envelope(&registry_db_path, reg_key, req.vault_id).await?;

    // 4. Inspect OS credential store state
    let keychain_result = VaultKeyManager::retrieve_key(req.cred_store, &vid);
    let had_keychain_secret = match keychain_result {
        Ok(Some(existing_kc_key)) => {
            if existing_kc_key.expose_bytes() != dek.expose_bytes() {
                return Err(
                    "OS credential store key does not match the unlocked vault DEK.".into(),
                );
            }
            true
        }
        Ok(None) => false,
        Err(e) => {
            if original_manifest.remember_in_keychain {
                return Err(format!("OS credential store is unavailable: {e}"));
            }
            tracing::warn!(
                vault_id = %req.vault_id,
                error = %e,
                "Credential store unavailable during preflight for non-remembered vault"
            );
            false
        }
    };

    // 5. Prepare and cryptographically verify new manifest
    let mut new_manifest = original_manifest.clone();
    new_manifest
        .change_password(&dek, req.new_password)
        .map_err(|e| format!("Failed to wrap vault key with new password: {e}"))?;

    let verified_new_dek = new_manifest.unlock(req.new_password).map_err(|_| {
        "Verification failed: new password did not unlock new manifest.".to_string()
    })?;
    if verified_new_dek.expose_bytes() != dek.expose_bytes() {
        return Err(
            "Critical verification failure: new password unwrapped a different DEK.".into(),
        );
    }
    if new_manifest.recovery_envelope != original_manifest.recovery_envelope {
        return Err("Verification failed: recovery envelope mutated unexpectedly.".into());
    }
    if let Some(ref hist) = new_manifest.key_revision {
        hist.verify(req.vault_id, "key", &dek)
            .map_err(|e| format!("Verification failed: key revision invalid: {e}"))?;
    }

    let new_primary_envelope_json = serde_json::to_string(&new_manifest.primary_envelope)
        .map_err(|e| format!("Failed to serialize new envelope: {e}"))?;
    let staged_manifest_json = serde_json::to_string_pretty(&new_manifest)
        .map_err(|e| format!("Failed to serialize new manifest: {e}"))?;

    // 6. Write durable operation journal (Stage: Prepared)
    check_fault(req.fault_hook, RotationFaultPoint::BeforeJournalCreate)?;
    if let Some(parent) = journal_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create manifest directory: {e}"))?;
    }

    let mut journal = PasswordRotationJournal {
        journal_version: 2,
        vault_id: req.vault_id.to_string(),
        stage: PasswordRotationStage::Prepared,
        original_manifest_backup: Some(original_manifest_backup.clone()),
        staged_manifest_json: Some(staged_manifest_json.clone()),
        new_primary_envelope_json: Some(new_primary_envelope_json.clone()),
        original_registry_envelope: original_registry_envelope.clone(),
        original_target_db_envelope: original_target_db_envelope.clone(),
        target_db_path: Some(target_db_path.to_string_lossy().into_owned()),
        registry_db_path: Some(registry_db_path.to_string_lossy().into_owned()),
        manifest_path: Some(manifest_path.to_string_lossy().into_owned()),
        had_keychain_secret,
        remember_in_keychain: original_manifest.remember_in_keychain,
        updated_at: now_secs(),
    };
    write_rotation_journal_file(&journal_path, &journal)?;

    // 7. Execute staged writes; roll back on any failure
    let commit_res: Result<(), String> = (async {
        check_fault(req.fault_hook, RotationFaultPoint::AfterJournalCreate)?;
        // Stage: ManifestSaved
        check_fault(req.fault_hook, RotationFaultPoint::BeforeManifestUpdate)?;
        write_durable_file(&manifest_path, &staged_manifest_json)?;
        journal.stage = PasswordRotationStage::ManifestSaved;
        journal.updated_at = now_secs();
        write_rotation_journal_file(&journal_path, &journal)?;
        check_fault(req.fault_hook, RotationFaultPoint::AfterManifestUpdate)?;

        // Stage: TargetDbUpdated
        check_fault(req.fault_hook, RotationFaultPoint::BeforeTargetDbUpdate)?;
        if !is_same_db {
            write_db_envelope(
                &target_db_path,
                Some(&dek),
                req.vault_id,
                &new_primary_envelope_json,
                false,
            )
            .await?;
        }
        journal.stage = PasswordRotationStage::TargetDbUpdated;
        journal.updated_at = now_secs();
        write_rotation_journal_file(&journal_path, &journal)?;
        check_fault(req.fault_hook, RotationFaultPoint::AfterTargetDbUpdate)?;

        // Stage: RegistryUpdated
        check_fault(req.fault_hook, RotationFaultPoint::BeforeRegistryUpdate)?;
        write_db_envelope(
            &registry_db_path,
            reg_key,
            req.vault_id,
            &new_primary_envelope_json,
            original_registry_envelope.is_some(),
        )
        .await?;
        journal.stage = PasswordRotationStage::RegistryUpdated;
        journal.updated_at = now_secs();
        write_rotation_journal_file(&journal_path, &journal)?;
        check_fault(req.fault_hook, RotationFaultPoint::AfterRegistryUpdate)?;

        // Stage: CredentialStoreUpdated
        check_fault(
            req.fault_hook,
            RotationFaultPoint::BeforeCredentialStoreUpdate,
        )?;
        if original_manifest.remember_in_keychain {
            VaultKeyManager::provision_key(req.cred_store, &vid, &dek)
                .map_err(|e| format!("Failed to update OS credential store: {e}"))?;
            let check = VaultKeyManager::retrieve_key(req.cred_store, &vid)
                .map_err(|e| format!("Failed to verify OS credential store after update: {e}"))?;
            if check
                .as_ref()
                .map(|k| k.expose_bytes() != dek.expose_bytes())
                .unwrap_or(true)
            {
                return Err("OS credential store verification mismatch after rotation.".into());
            }
        }
        journal.stage = PasswordRotationStage::CredentialStoreUpdated;
        journal.updated_at = now_secs();
        write_rotation_journal_file(&journal_path, &journal)?;
        check_fault(
            req.fault_hook,
            RotationFaultPoint::AfterCredentialStoreUpdate,
        )?;

        // Stage: Verified
        check_fault(req.fault_hook, RotationFaultPoint::BeforeCommitVerify)?;
        let reloaded_manifest = VaultManifest::load_from_path(&manifest_path)
            .map_err(|e| format!("Commit verification failed loading manifest: {e}"))?;
        let reloaded_dek = reloaded_manifest
            .unlock(req.new_password)
            .map_err(|_| "Commit verification failed: new password cannot unlock disk manifest.".to_string())?;
        if reloaded_dek.expose_bytes() != dek.expose_bytes() {
            return Err("Commit verification failed: DEK mismatch on disk manifest.".into());
        }
        verify_encrypted_records_readable(&target_db_path, &dek).await?;
        if original_registry_envelope.is_some() {
            let reg_env = read_db_envelope(&registry_db_path, reg_key, req.vault_id).await?;
            if reg_env.as_deref() != Some(new_primary_envelope_json.as_str()) {
                return Err(
                    "Commit verification failed: registry envelope does not match committed envelope."
                        .into(),
                );
            }
        }
        journal.stage = PasswordRotationStage::Verified;
        journal.updated_at = now_secs();
        write_rotation_journal_file(&journal_path, &journal)?;
        check_fault(req.fault_hook, RotationFaultPoint::AfterCommitVerify)?;

        // Stage: Completed
        check_fault(req.fault_hook, RotationFaultPoint::BeforeJournalComplete)?;
        journal.stage = PasswordRotationStage::Completed;
        journal.updated_at = now_secs();
        write_rotation_journal_file(&journal_path, &journal)?;
        std::fs::remove_file(&journal_path)
            .map_err(|e| format!("Failed to remove completed rotation journal: {e}"))?;
        check_fault(req.fault_hook, RotationFaultPoint::AfterJournalComplete)?;

        Ok(())
    })
    .await;

    match commit_res {
        Ok(()) => {
            tracing::info!(vault_id = %req.vault_id, "Vault password rotation committed and verified");
            Ok(PasswordRotationOutcome {
                vault_id: req.vault_id.to_string(),
                key: dek,
            })
        }
        Err(orig_err) => {
            tracing::error!(
                vault_id = %req.vault_id,
                error = %orig_err,
                "Vault password rotation failed; initiating staged rollback"
            );
            let rb_res = rollback_rotation(
                &journal_path,
                &mut journal,
                &manifest_path,
                &target_db_path,
                &registry_db_path,
                &dek,
                reg_key,
                req.cred_store,
                req.fault_hook,
            )
            .await;

            match rb_res {
                Ok(()) => Err(orig_err),
                Err(rb_err) => Err(format!(
                    "{orig_err} (Rollback also encountered an error; journal preserved for recovery: {rb_err})"
                )),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn rollback_rotation(
    journal_path: &Path,
    journal: &mut PasswordRotationJournal,
    manifest_path: &Path,
    target_db_path: &Path,
    registry_db_path: &Path,
    dek: &VaultKey,
    reg_key: Option<&VaultKey>,
    cred_store: &dyn CredentialStore,
    fault_hook: Option<&FaultInjector>,
) -> Result<(), String> {
    check_fault(fault_hook, RotationFaultPoint::BeforeRollback)?;

    journal.stage = PasswordRotationStage::RollingBack;
    journal.updated_at = now_secs();
    write_rotation_journal_file(journal_path, journal)?;

    // 1. Restore manifest
    let orig_manifest_str = journal
        .original_manifest_backup
        .as_deref()
        .ok_or_else(|| "Missing original manifest backup in journal".to_string())?;
    write_durable_file(manifest_path, orig_manifest_str)?;

    // 2. Restore target DB envelope if modified/separate
    if target_db_path != registry_db_path {
        if let Some(ref orig_target_env) = journal.original_target_db_envelope {
            write_db_envelope(
                target_db_path,
                Some(dek),
                &journal.vault_id,
                orig_target_env,
                false,
            )
            .await?;
        }
    }

    // 3. Restore registry DB envelope
    if let Some(ref orig_reg_env) = journal.original_registry_envelope {
        write_db_envelope(
            registry_db_path,
            reg_key,
            &journal.vault_id,
            orig_reg_env,
            false,
        )
        .await?;
    }

    // 4. Restore OS credential store
    let vid = VaultId::new(&journal.vault_id).map_err(|e| e.to_string())?;
    if journal.remember_in_keychain {
        if journal.had_keychain_secret {
            VaultKeyManager::provision_key(cred_store, &vid, dek).map_err(|e| {
                format!("Failed to restore credential store key during rollback: {e}")
            })?;
        } else {
            VaultKeyManager::delete_key(cred_store, &vid).map_err(|e| {
                format!("Failed to clean credential store key during rollback: {e}")
            })?;
        }
    }

    // 5. Verify restored manifest parses & records are readable
    let restored_manifest = VaultManifest::load_from_path(manifest_path)
        .map_err(|e| format!("Rollback verification failed loading restored manifest: {e}"))?;
    if restored_manifest.vault_id != journal.vault_id {
        return Err("Rollback verification failed: vault ID mismatch".into());
    }
    verify_encrypted_records_readable(target_db_path, dek).await?;

    check_fault(fault_hook, RotationFaultPoint::AfterRollback)?;

    // Only remove journal after verified rollback
    std::fs::remove_file(journal_path)
        .map_err(|e| format!("Failed to remove journal after rollback: {e}"))?;
    tracing::info!(vault_id = %journal.vault_id, "Password rotation rolled back and verified");
    Ok(())
}

fn write_rotation_journal_file(
    path: &Path,
    journal: &PasswordRotationJournal,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(journal)
        .map_err(|e| format!("Failed to serialize rotation journal: {e}"))?;
    write_durable_file(path, &json)
}

fn collect_rotation_journal_paths(base_path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    for dir in [
        data_path::resolve_manifest_dir(base_path),
        data_path::resolve_vault_dir(base_path),
    ] {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(format!(
                    "Cannot inspect rotation journals in {}: {e}",
                    dir.display()
                ))
            }
        };
        for entry in entries {
            let path = entry
                .map_err(|e| format!("Cannot inspect rotation journal entry: {e}"))?
                .path();
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".rotation_journal.json"))
            {
                found.push(path);
            }
        }
    }
    Ok(found)
}

fn infer_vault_id_from_journal_path(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .and_then(|s| s.strip_suffix(".rotation_journal.json").map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Discovers and reconciles any interrupted password rotation journals at startup.
///
/// - Automatically recovers stages that do not require database decryption keys (`Prepared`,
///   `ManifestSaved`, `Completed`) or when keychain keys are available.
/// - Returns `PendingRotationRecovery` entries for any vaults that require user authentication
///   or have corrupted journals, allowing unrelated vaults to remain accessible.
pub async fn reconcile_pending_rotations_at_startup(
    base_path: &Path,
    cred_store: &dyn CredentialStore,
) -> Result<Vec<PendingRotationRecovery>, String> {
    let paths = collect_rotation_journal_paths(base_path)?;
    let mut pending = Vec::new();

    for jpath in paths {
        let inferred_vid = infer_vault_id_from_journal_path(&jpath);
        let raw = match std::fs::read_to_string(&jpath) {
            Ok(s) => s,
            Err(e) => {
                pending.push(PendingRotationRecovery {
                    vault_id: inferred_vid,
                    journal_path: jpath,
                    is_corrupted: true,
                    stage: None,
                    reason: format!("Cannot read rotation journal: {e}"),
                });
                continue;
            }
        };

        let mut journal: PasswordRotationJournal = match serde_json::from_str(&raw) {
            Ok(j) => j,
            Err(e) => {
                pending.push(PendingRotationRecovery {
                    vault_id: inferred_vid,
                    journal_path: jpath,
                    is_corrupted: true,
                    stage: None,
                    reason: format!("Corrupted rotation journal: {e}"),
                });
                continue;
            }
        };

        if !is_safe_vault_id(&journal.vault_id) {
            pending.push(PendingRotationRecovery {
                vault_id: inferred_vid,
                journal_path: jpath,
                is_corrupted: true,
                stage: Some(journal.stage),
                reason: format!(
                    "Unsafe vault ID '{}' in rotation journal.",
                    journal.vault_id
                ),
            });
            continue;
        }

        let manifest_path = journal
            .manifest_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| data_path::resolve_manifest_file(base_path, &journal.vault_id));

        match journal.stage {
            PasswordRotationStage::Completed => {
                if VaultManifest::load_from_path(&manifest_path).is_ok() {
                    let _ = std::fs::remove_file(&jpath);
                } else {
                    pending.push(PendingRotationRecovery {
                        vault_id: journal.vault_id.clone(),
                        journal_path: jpath,
                        is_corrupted: false,
                        stage: Some(journal.stage),
                        reason: "Completed rotation journal found but manifest is unreadable."
                            .into(),
                    });
                }
            }
            PasswordRotationStage::Prepared => {
                // No store was mutated yet. Verify manifest is intact, then remove journal.
                if VaultManifest::load_from_path(&manifest_path).is_ok() {
                    std::fs::remove_file(&jpath)
                        .map_err(|e| format!("Failed to clean Prepared journal: {e}"))?;
                } else if let Some(ref backup) = journal.original_manifest_backup {
                    write_durable_file(&manifest_path, backup)?;
                    std::fs::remove_file(&jpath)
                        .map_err(|e| format!("Failed to clean Prepared journal: {e}"))?;
                } else {
                    pending.push(PendingRotationRecovery {
                        vault_id: journal.vault_id.clone(),
                        journal_path: jpath,
                        is_corrupted: false,
                        stage: Some(journal.stage),
                        reason: "Manifest missing and no backup in journal.".into(),
                    });
                }
            }
            PasswordRotationStage::ManifestSaved => {
                // Only manifest was touched; DBs and credential store were not modified.
                if let Some(ref backup) = journal.original_manifest_backup {
                    write_durable_file(&manifest_path, backup)?;
                    VaultManifest::load_from_path(&manifest_path)
                        .map_err(|e| format!("Restored manifest invalid: {e}"))?;
                    std::fs::remove_file(&jpath)
                        .map_err(|e| format!("Failed to remove reconciled journal: {e}"))?;
                    tracing::info!(
                        vault_id = %journal.vault_id,
                        "Startup recovery restored original manifest from ManifestSaved stage"
                    );
                } else {
                    pending.push(PendingRotationRecovery {
                        vault_id: journal.vault_id.clone(),
                        journal_path: jpath,
                        is_corrupted: false,
                        stage: Some(journal.stage),
                        reason: "Original manifest backup missing in journal.".into(),
                    });
                }
            }
            PasswordRotationStage::TargetDbUpdated
            | PasswordRotationStage::RegistryUpdated
            | PasswordRotationStage::CredentialStoreUpdated
            | PasswordRotationStage::Verified
            | PasswordRotationStage::RollingBack => {
                // Try automatic reconciliation if keys are in OS credential store
                let vid = VaultId::new(&journal.vault_id).map_err(|e| e.to_string())?;
                let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).map_err(|e| e.to_string())?;
                let vault_key = VaultKeyManager::retrieve_key(cred_store, &vid)
                    .ok()
                    .flatten();
                let default_key = if journal.vault_id == LEGACY_LOCAL_VAULT_ID {
                    vault_key.clone()
                } else {
                    VaultKeyManager::retrieve_key(cred_store, &default_vid)
                        .ok()
                        .flatten()
                };

                if let Some(ref vkey) = vault_key {
                    match reconcile_with_keys(
                        base_path,
                        &jpath,
                        &mut journal,
                        vkey,
                        default_key.as_ref(),
                        cred_store,
                    )
                    .await
                    {
                        Ok(_) => {
                            tracing::info!(
                                vault_id = %journal.vault_id,
                                "Startup recovery reconciled interrupted rotation using keychain key"
                            );
                        }
                        Err(err) => {
                            pending.push(PendingRotationRecovery {
                                vault_id: journal.vault_id.clone(),
                                journal_path: jpath,
                                is_corrupted: false,
                                stage: Some(journal.stage),
                                reason: err,
                            });
                        }
                    }
                } else {
                    pending.push(PendingRotationRecovery {
                        vault_id: journal.vault_id.clone(),
                        journal_path: jpath,
                        is_corrupted: false,
                        stage: Some(journal.stage),
                        reason: "Password or recovery code required to reconcile interrupted password rotation."
                            .into(),
                    });
                }
            }
        }
    }

    Ok(pending)
}

/// Authenticates a pending rotation recovery using the user's password (old or new) or recovery code,
/// reconciles all four stores, verifies encrypted records, and removes the journal.
pub async fn reconcile_rotation_with_password(
    base_path: &Path,
    vault_id: &str,
    passphrase: &str,
    default_vault_key: Option<&VaultKey>,
    cred_store: &dyn CredentialStore,
) -> Result<RotationReconciliationOutcome, String> {
    if !is_safe_vault_id(vault_id) {
        return Err(format!("Invalid vault ID '{vault_id}'."));
    }
    let _lock = RotationLockGuard::acquire(vault_id)?;

    let jpath = rotation_journal_path(base_path, vault_id);
    if !jpath.exists() {
        return Err(format!(
            "No pending rotation journal found for vault '{vault_id}'."
        ));
    }

    let raw = std::fs::read_to_string(&jpath)
        .map_err(|e| format!("Cannot read rotation journal: {e}"))?;
    let mut journal: PasswordRotationJournal = serde_json::from_str(&raw)
        .map_err(|e| format!("Rotation journal is corrupted and cannot be parsed: {e}"))?;

    if journal.vault_id != vault_id {
        return Err(format!(
            "Journal vault ID mismatch: expected '{vault_id}', found '{}'.",
            journal.vault_id
        ));
    }

    // Try unlocking DEK from original_manifest_backup or staged_manifest_json or disk manifest
    let mut dek_opt: Option<VaultKey> = None;
    if let Some(ref orig_str) = journal.original_manifest_backup {
        if let Ok(m) = serde_json::from_str::<VaultManifest>(orig_str) {
            if let Ok(k) = m.unlock(passphrase) {
                dek_opt = Some(k);
            }
        }
    }
    if dek_opt.is_none() {
        if let Some(ref staged_str) = journal.staged_manifest_json {
            if let Ok(m) = serde_json::from_str::<VaultManifest>(staged_str) {
                if let Ok(k) = m.unlock(passphrase) {
                    dek_opt = Some(k);
                }
            }
        }
    }
    if dek_opt.is_none() {
        let mpath = data_path::resolve_manifest_file(base_path, vault_id);
        if let Ok(m) = VaultManifest::load_from_path(&mpath) {
            if let Ok(k) = m.unlock(passphrase) {
                dek_opt = Some(k);
            }
        }
    }

    let dek = dek_opt.ok_or_else(|| {
        "Incorrect password or recovery code for reconciling this vault rotation.".to_string()
    })?;

    let reg_key = if vault_id == LEGACY_LOCAL_VAULT_ID {
        Some(&dek)
    } else {
        default_vault_key
    };

    reconcile_with_keys(base_path, &jpath, &mut journal, &dek, reg_key, cred_store).await
}

async fn reconcile_with_keys(
    base_path: &Path,
    journal_path: &Path,
    journal: &mut PasswordRotationJournal,
    dek: &VaultKey,
    reg_key: Option<&VaultKey>,
    cred_store: &dyn CredentialStore,
) -> Result<RotationReconciliationOutcome, String> {
    let manifest_path = journal
        .manifest_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| data_path::resolve_manifest_file(base_path, &journal.vault_id));
    let target_db_path = journal
        .target_db_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| data_path::resolve_vault_db_file(base_path, &journal.vault_id));
    let registry_db_path = journal
        .registry_db_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| data_path::resolve_vault_db_file(base_path, LEGACY_LOCAL_VAULT_ID));

    let is_same_db = target_db_path == registry_db_path;
    let effective_reg_key = if is_same_db { Some(dek) } else { reg_key };

    match journal.stage {
        PasswordRotationStage::Verified | PasswordRotationStage::Completed => {
            // Forward-complete: commit staged manifest & envelopes
            let staged_manifest_str = journal
                .staged_manifest_json
                .as_deref()
                .ok_or_else(|| "Staged manifest missing from Verified journal".to_string())?;
            let new_env = journal
                .new_primary_envelope_json
                .as_deref()
                .ok_or_else(|| "New primary envelope missing from Verified journal".to_string())?;

            write_durable_file(&manifest_path, staged_manifest_str)?;
            if !is_same_db {
                write_db_envelope(
                    &target_db_path,
                    Some(dek),
                    &journal.vault_id,
                    new_env,
                    false,
                )
                .await?;
            }
            write_db_envelope(
                &registry_db_path,
                effective_reg_key,
                &journal.vault_id,
                new_env,
                journal.original_registry_envelope.is_some(),
            )
            .await?;
            if journal.remember_in_keychain {
                let vid = VaultId::new(&journal.vault_id).map_err(|e| e.to_string())?;
                VaultKeyManager::provision_key(cred_store, &vid, dek)
                    .map_err(|e| format!("Failed to reconcile OS credential store: {e}"))?;
            }
            verify_encrypted_records_readable(&target_db_path, dek).await?;
            std::fs::remove_file(journal_path)
                .map_err(|e| format!("Failed to remove reconciled journal: {e}"))?;

            Ok(RotationReconciliationOutcome {
                vault_id: journal.vault_id.clone(),
                key: dek.clone(),
                committed_new_password: true,
            })
        }
        PasswordRotationStage::Prepared
        | PasswordRotationStage::ManifestSaved
        | PasswordRotationStage::TargetDbUpdated
        | PasswordRotationStage::RegistryUpdated
        | PasswordRotationStage::CredentialStoreUpdated
        | PasswordRotationStage::RollingBack => {
            // Rollback to original consistent state
            rollback_rotation(
                journal_path,
                journal,
                &manifest_path,
                &target_db_path,
                &registry_db_path,
                dek,
                effective_reg_key,
                cred_store,
                None,
            )
            .await?;

            Ok(RotationReconciliationOutcome {
                vault_id: journal.vault_id.clone(),
                key: dek.clone(),
                committed_new_password: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shellumi_core::{
        CredentialError, CredentialStore, EphemeralCredentialStore, SecretString, SshDestination,
        VaultContext, VaultLocation,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    struct FaultyCredentialStore {
        inner: EphemeralCredentialStore,
        fail_get: AtomicBool,
        fail_set: AtomicBool,
        fail_delete: AtomicBool,
    }

    impl FaultyCredentialStore {
        fn new() -> Self {
            Self {
                inner: EphemeralCredentialStore::new(),
                fail_get: AtomicBool::new(false),
                fail_set: AtomicBool::new(false),
                fail_delete: AtomicBool::new(false),
            }
        }
    }

    impl CredentialStore for FaultyCredentialStore {
        fn set_secret(
            &self,
            service: &str,
            account: &str,
            secret: &str,
        ) -> Result<(), CredentialError> {
            if self.fail_set.load(Ordering::SeqCst) {
                return Err(CredentialError::Storage(
                    "Simulated OS keychain write failure".into(),
                ));
            }
            self.inner.set_secret(service, account, secret)
        }

        fn get_secret(
            &self,
            service: &str,
            account: &str,
        ) -> Result<Option<SecretString>, CredentialError> {
            if self.fail_get.load(Ordering::SeqCst) {
                return Err(CredentialError::Storage(
                    "Simulated OS keychain unavailable".into(),
                ));
            }
            self.inner.get_secret(service, account)
        }

        fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError> {
            if self.fail_delete.load(Ordering::SeqCst) {
                return Err(CredentialError::Storage(
                    "Simulated OS keychain delete failure".into(),
                ));
            }
            self.inner.delete_secret(service, account)
        }
    }

    async fn setup_two_vault_fixture(
        base_dir: &Path,
        cred_store: &dyn CredentialStore,
        sec_id: &str,
        sec_remember: bool,
    ) -> (VaultKey, VaultKey, String, String, String) {
        let def_pass = "DefaultPass123!@#";
        let sec_pass = "SecondaryPass123!@#";
        let sec_rec = VaultKey::generate_recovery_code().unwrap();

        let def_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
        let (def_dek, def_env, _) =
            VaultKeyManager::provision_new_vault(cred_store, &def_vid, def_pass, true, None)
                .unwrap();
        let def_manifest = VaultManifest::new(
            def_vid.as_str().to_string(),
            "Default".into(),
            def_env.clone(),
            None,
            true,
        );
        let def_mpath = data_path::resolve_manifest_file(base_dir, LEGACY_LOCAL_VAULT_ID);
        std::fs::create_dir_all(def_mpath.parent().unwrap()).unwrap();
        def_manifest.save_to_path(&def_mpath).unwrap();

        let def_db_path = data_path::resolve_vault_db_file(base_dir, LEGACY_LOCAL_VAULT_ID);
        std::fs::create_dir_all(def_db_path.parent().unwrap()).unwrap();
        let mut reg_store = HostStore::open_with_key(def_db_path.to_str().unwrap(), Some(&def_dek))
            .await
            .unwrap();
        reg_store
            .save_vault_recovery_envelope(
                LEGACY_LOCAL_VAULT_ID,
                &serde_json::to_string(&def_env).unwrap(),
            )
            .await
            .unwrap();

        // Secondary vault
        let sec_vid = VaultId::new(sec_id).unwrap();
        let (sec_dek, sec_env, sec_rec_env) = VaultKeyManager::provision_new_vault(
            cred_store,
            &sec_vid,
            sec_pass,
            sec_remember,
            Some(&sec_rec),
        )
        .unwrap();
        let sec_manifest = VaultManifest::new(
            sec_id.into(),
            "Secondary".into(),
            sec_env.clone(),
            sec_rec_env,
            sec_remember,
        );
        let sec_mpath = data_path::resolve_manifest_file(base_dir, sec_id);
        sec_manifest.save_to_path(&sec_mpath).unwrap();

        let sec_env_json = serde_json::to_string(&sec_env).unwrap();
        reg_store
            .register_vault_context(
                "Secondary",
                &VaultContext::new(sec_vid.clone(), VaultLocation::Local),
            )
            .await
            .unwrap();
        reg_store
            .save_vault_recovery_envelope(sec_id, &sec_env_json)
            .await
            .unwrap();

        let sec_db_path = data_path::resolve_vault_db_file(base_dir, sec_id);
        let mut sec_store = HostStore::open_with_key(sec_db_path.to_str().unwrap(), Some(&sec_dek))
            .await
            .unwrap();
        sec_store
            .register_vault_context(
                "Secondary",
                &VaultContext::new(sec_vid, VaultLocation::Local),
            )
            .await
            .unwrap();
        sec_store
            .save_vault_recovery_envelope(sec_id, &sec_env_json)
            .await
            .unwrap();
        sec_store
            .add(
                "Prod DB",
                &SshDestination::new("10.0.0.5", "ops", 22).unwrap(),
                None,
                &[],
                None,
                None,
            )
            .await
            .unwrap();

        (def_dek, sec_dek, def_pass.into(), sec_pass.into(), sec_rec)
    }

    #[tokio::test]
    async fn test_full_password_rotation_preserves_dek_records_recovery_and_revision_history() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shellumi_rot_ok_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let cred_store = FaultyCredentialStore::new();
        let sec_id = "vlt-sec-ok";
        let (def_dek, sec_dek, _def_pass, sec_pass, sec_rec) =
            setup_two_vault_fixture(&temp_dir, &cred_store, sec_id, true).await;

        let new_pass = "RotatedStrongPass999!@#";
        let outcome = rotate_vault_password(PasswordRotationRequest {
            base_path: &temp_dir,
            vault_id: sec_id,
            old_password: &sec_pass,
            new_password: new_pass,
            expected_in_memory_key: Some(&sec_dek),
            default_vault_key: Some(&def_dek),
            cred_store: &cred_store,
            fault_hook: None,
        })
        .await
        .expect("Rotation should succeed");

        assert_eq!(outcome.key.expose_bytes(), sec_dek.expose_bytes());
        assert!(!rotation_journal_path(&temp_dir, sec_id).exists());

        // Verify manifest unlocks with new password, fails with old password, preserves recovery code & revision
        let mpath = data_path::resolve_manifest_file(&temp_dir, sec_id);
        let loaded = VaultManifest::load_from_path(&mpath).unwrap();
        assert!(loaded.unlock(&sec_pass).is_err());
        assert_eq!(
            loaded.unlock(new_pass).unwrap().expose_bytes(),
            sec_dek.expose_bytes()
        );
        assert_eq!(
            loaded
                .unlock_with_recovery(&sec_rec)
                .unwrap()
                .expose_bytes(),
            sec_dek.expose_bytes()
        );
        loaded
            .key_revision
            .as_ref()
            .expect("Authenticated key revision history must be present")
            .verify(sec_id, "key", &sec_dek)
            .expect("Key revision history must verify with original DEK");

        // Verify encrypted records are still readable
        let sec_db = data_path::resolve_vault_db_file(&temp_dir, sec_id);
        let store = HostStore::open_with_key(sec_db.to_str().unwrap(), Some(&sec_dek))
            .await
            .unwrap();
        let hosts = store.list().await.unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "Prod DB");

        // Verify registry and target DB envelopes match new primary envelope
        let new_env_json = serde_json::to_string(&loaded.primary_envelope).unwrap();
        assert_eq!(
            store.get_vault_recovery_envelope(sec_id).await.unwrap(),
            Some(new_env_json.clone())
        );
        let def_db = data_path::resolve_vault_db_file(&temp_dir, LEGACY_LOCAL_VAULT_ID);
        let reg_store = HostStore::open_with_key(def_db.to_str().unwrap(), Some(&def_dek))
            .await
            .unwrap();
        assert_eq!(
            reg_store.get_vault_recovery_envelope(sec_id).await.unwrap(),
            Some(new_env_json)
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_fault_injection_before_and_after_all_stages_rolls_back_cleanly() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shellumi_rot_faults_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let cred_store = FaultyCredentialStore::new();
        let sec_id = "vlt-sec-faults";
        let (def_dek, sec_dek, _def_pass, sec_pass, _sec_rec) =
            setup_two_vault_fixture(&temp_dir, &cred_store, sec_id, true).await;

        let fault_points = [
            RotationFaultPoint::BeforeJournalCreate,
            RotationFaultPoint::AfterJournalCreate,
            RotationFaultPoint::BeforeManifestUpdate,
            RotationFaultPoint::AfterManifestUpdate,
            RotationFaultPoint::BeforeTargetDbUpdate,
            RotationFaultPoint::AfterTargetDbUpdate,
            RotationFaultPoint::BeforeRegistryUpdate,
            RotationFaultPoint::AfterRegistryUpdate,
            RotationFaultPoint::BeforeCredentialStoreUpdate,
            RotationFaultPoint::AfterCredentialStoreUpdate,
            RotationFaultPoint::BeforeCommitVerify,
            RotationFaultPoint::AfterCommitVerify,
            RotationFaultPoint::BeforeJournalComplete,
        ];

        let new_pass = "FaultTestNewPass123!@#";
        for fp in fault_points {
            let hook = move |pt: RotationFaultPoint| -> Result<(), String> {
                if pt == fp {
                    Err(format!("Injected fault at {fp:?}"))
                } else {
                    Ok(())
                }
            };
            let res = rotate_vault_password(PasswordRotationRequest {
                base_path: &temp_dir,
                vault_id: sec_id,
                old_password: &sec_pass,
                new_password: new_pass,
                expected_in_memory_key: Some(&sec_dek),
                default_vault_key: Some(&def_dek),
                cred_store: &cred_store,
                fault_hook: Some(&hook),
            })
            .await;
            assert!(res.is_err(), "Expected fault error at {fp:?}");

            // After rollback, old password still unlocks original DEK, journal is cleaned up, records readable
            let mpath = data_path::resolve_manifest_file(&temp_dir, sec_id);
            let loaded = VaultManifest::load_from_path(&mpath).unwrap();
            assert_eq!(
                loaded.unlock(&sec_pass).unwrap().expose_bytes(),
                sec_dek.expose_bytes(),
                "Old password must still unlock after rollback at {fp:?}"
            );
            assert!(
                loaded.unlock(new_pass).is_err(),
                "New password must NOT unlock after rollback at {fp:?}"
            );
            assert!(
                !rotation_journal_path(&temp_dir, sec_id).exists(),
                "Journal should be removed after verified rollback at {fp:?}"
            );
        }

        // Test failure during rollback itself -> journal MUST be preserved
        let rollback_fault = |pt: RotationFaultPoint| -> Result<(), String> {
            if pt == RotationFaultPoint::AfterRegistryUpdate
                || pt == RotationFaultPoint::BeforeRollback
            {
                Err(format!("Simulated failure at {pt:?}"))
            } else {
                Ok(())
            }
        };
        let res = rotate_vault_password(PasswordRotationRequest {
            base_path: &temp_dir,
            vault_id: sec_id,
            old_password: &sec_pass,
            new_password: new_pass,
            expected_in_memory_key: Some(&sec_dek),
            default_vault_key: Some(&def_dek),
            cred_store: &cred_store,
            fault_hook: Some(&rollback_fault),
        })
        .await;
        assert!(res.is_err());
        assert!(
            rotation_journal_path(&temp_dir, sec_id).exists(),
            "Journal MUST be preserved if rollback fails"
        );

        // Now startup reconciliation recovers cleanly using keychain key across restarts
        let pending = reconcile_pending_rotations_at_startup(&temp_dir, &cred_store)
            .await
            .unwrap();
        assert!(pending.is_empty());
        assert!(!rotation_journal_path(&temp_dir, sec_id).exists());
        let mpath = data_path::resolve_manifest_file(&temp_dir, sec_id);
        assert_eq!(
            VaultManifest::load_from_path(&mpath)
                .unwrap()
                .unlock(&sec_pass)
                .unwrap()
                .expose_bytes(),
            sec_dek.expose_bytes()
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_restart_recovery_without_keychain_requires_auth_and_preserves_other_vaults() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shellumi_rot_authrec_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let cred_store = FaultyCredentialStore::new();
        let sec_id = "vlt-sec-authrec";
        // Secondary vault has remember_in_keychain = false
        let (def_dek, sec_dek, _def_pass, sec_pass, sec_rec) =
            setup_two_vault_fixture(&temp_dir, &cred_store, sec_id, false).await;

        let new_pass = "VerifiedCrashNewPass123!@#";
        let mpath = data_path::resolve_manifest_file(&temp_dir, sec_id);
        let orig_manifest_str = std::fs::read_to_string(&mpath).unwrap();
        let orig_manifest: VaultManifest = serde_json::from_str(&orig_manifest_str).unwrap();
        let mut new_manifest = orig_manifest.clone();
        new_manifest.change_password(&sec_dek, new_pass).unwrap();
        let new_env_json = serde_json::to_string(&new_manifest.primary_envelope).unwrap();
        let staged_json = serde_json::to_string_pretty(&new_manifest).unwrap();

        // Check journal contains NO plaintext passwords or recovery codes or raw DEK
        let jpath = rotation_journal_path(&temp_dir, sec_id);
        let journal = PasswordRotationJournal {
            journal_version: 2,
            vault_id: sec_id.into(),
            stage: PasswordRotationStage::Verified,
            original_manifest_backup: Some(orig_manifest_str),
            staged_manifest_json: Some(staged_json),
            new_primary_envelope_json: Some(new_env_json),
            original_registry_envelope: Some(
                serde_json::to_string(&orig_manifest.primary_envelope).unwrap(),
            ),
            original_target_db_envelope: Some(
                serde_json::to_string(&orig_manifest.primary_envelope).unwrap(),
            ),
            target_db_path: Some(
                data_path::resolve_vault_db_file(&temp_dir, sec_id)
                    .to_string_lossy()
                    .into_owned(),
            ),
            registry_db_path: Some(
                data_path::resolve_vault_db_file(&temp_dir, LEGACY_LOCAL_VAULT_ID)
                    .to_string_lossy()
                    .into_owned(),
            ),
            manifest_path: Some(mpath.to_string_lossy().into_owned()),
            had_keychain_secret: false,
            remember_in_keychain: false,
            updated_at: 1000,
        };
        write_rotation_journal_file(&jpath, &journal).unwrap();
        let raw_journal = std::fs::read_to_string(&jpath).unwrap();
        assert!(!raw_journal.contains(&sec_pass));
        assert!(!raw_journal.contains(new_pass));
        assert!(!raw_journal.contains(&sec_rec));

        // Repeated restarts without authentication keep journal intact and report pending recovery ONLY for sec_id
        for _ in 0..2 {
            let pending = reconcile_pending_rotations_at_startup(&temp_dir, &cred_store)
                .await
                .unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].vault_id, sec_id);
            assert!(!pending[0].is_corrupted);
            assert!(jpath.exists());
        }

        // Unrelated default vault remains completely accessible
        let def_db = data_path::resolve_vault_db_file(&temp_dir, LEGACY_LOCAL_VAULT_ID);
        assert!(
            HostStore::open_with_key(def_db.to_str().unwrap(), Some(&def_dek))
                .await
                .is_ok()
        );

        // User authenticates with new password -> reconciles Verified stage to committed state
        let outcome = reconcile_rotation_with_password(
            &temp_dir,
            sec_id,
            new_pass,
            Some(&def_dek),
            &cred_store,
        )
        .await
        .unwrap();
        assert!(outcome.committed_new_password);
        assert_eq!(outcome.key.expose_bytes(), sec_dek.expose_bytes());
        assert!(!jpath.exists());
        assert_eq!(
            VaultManifest::load_from_path(&mpath)
                .unwrap()
                .unlock(new_pass)
                .unwrap()
                .expose_bytes(),
            sec_dek.expose_bytes()
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_corrupted_journal_unavailable_keychain_invalid_old_pass_and_concurrency() {
        let temp_dir = std::env::temp_dir().join(format!(
            "shellumi_rot_edge_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let cred_store = FaultyCredentialStore::new();
        let sec_id = "vlt-sec-edge";
        let (def_dek, sec_dek, _def_pass, sec_pass, _sec_rec) =
            setup_two_vault_fixture(&temp_dir, &cred_store, sec_id, true).await;

        // 1. Invalid old password fails preflight cleanly without creating journal
        let err = rotate_vault_password(PasswordRotationRequest {
            base_path: &temp_dir,
            vault_id: sec_id,
            old_password: "WrongOldPassword1!",
            new_password: "ValidNewPassword123!",
            expected_in_memory_key: Some(&sec_dek),
            default_vault_key: Some(&def_dek),
            cred_store: &cred_store,
            fault_hook: None,
        })
        .await;
        assert!(err.is_err());
        assert!(!rotation_journal_path(&temp_dir, sec_id).exists());

        // 2. Unavailable OS keychain fails when remember_in_keychain = true
        cred_store.fail_get.store(true, Ordering::SeqCst);
        let kc_err = rotate_vault_password(PasswordRotationRequest {
            base_path: &temp_dir,
            vault_id: sec_id,
            old_password: &sec_pass,
            new_password: "ValidNewPassword123!",
            expected_in_memory_key: Some(&sec_dek),
            default_vault_key: Some(&def_dek),
            cred_store: &cred_store,
            fault_hook: None,
        })
        .await;
        assert!(kc_err.is_err());
        cred_store.fail_get.store(false, Ordering::SeqCst);

        // 3. Concurrent rotation lock prevents simultaneous rotations on same vault
        {
            let _lock = RotationLockGuard::acquire(sec_id).unwrap();
            let conc_err = rotate_vault_password(PasswordRotationRequest {
                base_path: &temp_dir,
                vault_id: sec_id,
                old_password: &sec_pass,
                new_password: "ValidNewPassword123!",
                expected_in_memory_key: Some(&sec_dek),
                default_vault_key: Some(&def_dek),
                cred_store: &cred_store,
                fault_hook: None,
            })
            .await;
            assert!(conc_err.is_err());
        }

        // 4. Truncated/corrupted journal is never deleted on startup
        let jpath = rotation_journal_path(&temp_dir, sec_id);
        std::fs::write(&jpath, b"{corrupted").unwrap();
        let pending = reconcile_pending_rotations_at_startup(&temp_dir, &cred_store)
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert!(pending[0].is_corrupted);
        assert!(jpath.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

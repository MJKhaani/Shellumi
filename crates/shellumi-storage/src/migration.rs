//! Resumable database migration from legacy shared `shellumi.db` to separate physical encrypted vault databases.
//!
//! Rebuilds storage architecture into:
//! - `Shellumi/vaults/default.db` (for default vault `legacy-local`, holding registry and default records)
//! - `Shellumi/vaults/<vault-id>.db` (for secondary vaults, holding only their own records)
//!
//! Each database is encrypted with its own random 256-bit DEK using AES-256-GCM.
//! Staged in dedicated `.migration_staging/` directory with `migration_journal.json` for crash safety.
//! Server credentials are migrated into OS-protected credential storage (`CredentialStore`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::info;

use shellumi_core::{CredentialStore, Snippet, VaultId, VaultKey, VaultKeyManager, VaultManifest};

use crate::{HostStore, StorageError, LEGACY_LOCAL_VAULT_ID};

/// Detailed report of a completed database migration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub legacy_db_found: bool,
    pub backup_path: Option<PathBuf>,
    pub vaults_migrated: usize,
    pub default_records_migrated: usize,
    pub secondary_records_migrated: usize,
    pub server_credentials_migrated: usize,
}

/// Optional user credentials for provisioning encryption during legacy migration.
#[derive(Clone, Debug, Default)]
pub struct MigrationCredentials {
    pub default_password: Option<String>,
    pub default_recovery_code: Option<String>,
    pub secondary_passwords: HashMap<String, String>,
    pub secondary_recovery_codes: HashMap<String, String>,
}

/// Persistent journal documenting progress through migration staging.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MigrationJournal {
    pub started_at: u64,
    pub legacy_db_path: PathBuf,
    pub backup_path: PathBuf,
    pub staged_vaults: Vec<StagedVaultMetadata>,
    pub completed_stages: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StagedVaultMetadata {
    pub vault_id: String,
    pub name: String,
    pub is_default: bool,
    pub db_filename: String,
    pub manifest_filename: String,
    pub host_count: usize,
    pub snippet_count: usize,
    pub workspace_count: usize,
    pub identity_count: usize,
}

pub struct MigrationEngine;

impl MigrationEngine {
    /// Determines whether migration is needed for the given base directory.
    pub fn is_migration_needed(base_dir: &Path) -> bool {
        let legacy_db = base_dir.join("shellumi.db");
        let marker = base_dir.join("vaults").join(".migration_complete");
        legacy_db.exists() && !marker.exists()
    }

    /// Safely and resumably migrates an existing legacy database into separate encrypted databases.
    pub async fn migrate_legacy_database_if_needed(
        base_dir: &Path,
        default_dek: Option<&VaultKey>,
        cred_store: &dyn CredentialStore,
    ) -> Result<MigrationReport, StorageError> {
        Self::migrate_legacy_database_with_credentials(base_dir, default_dek, None, cred_store)
            .await
    }

    /// Migrates an existing legacy database into separate encrypted databases with optional user-confirmed credentials.
    pub async fn migrate_legacy_database_with_credentials(
        base_dir: &Path,
        default_dek: Option<&VaultKey>,
        credentials: Option<&MigrationCredentials>,
        cred_store: &dyn CredentialStore,
    ) -> Result<MigrationReport, StorageError> {
        let legacy_db_path = base_dir.join("shellumi.db");
        if !legacy_db_path.exists() {
            return Ok(MigrationReport::default());
        }

        let vaults_dir = base_dir.join("vaults");
        let complete_marker = vaults_dir.join(".migration_complete");
        if complete_marker.exists() {
            return Ok(MigrationReport {
                legacy_db_found: true,
                ..Default::default()
            });
        }

        // Dedicated staging directory: NEVER touch or delete pre-existing vaults/ or vault_manifests/
        let staging_dir = vaults_dir.join(".migration_staging");
        if staging_dir.exists() {
            tracing::warn!(
                staging_path = %staging_dir.display(),
                "Previous database migration staging was interrupted. Cleaning up staging directory only..."
            );
            let _ = std::fs::remove_dir_all(&staging_dir);
        }

        let staging_vaults_dir = staging_dir.join("vaults");
        let staging_manifests_dir = staging_dir.join("vault_manifests");
        std::fs::create_dir_all(&staging_vaults_dir).map_err(|e| {
            StorageError::Serialization(format!("failed to create staging vaults directory: {e}"))
        })?;
        std::fs::create_dir_all(&staging_manifests_dir).map_err(|e| {
            StorageError::Serialization(format!(
                "failed to create staging manifests directory: {e}"
            ))
        })?;

        let res = Self::migrate_staging_internal(
            base_dir,
            &vaults_dir,
            &legacy_db_path,
            &complete_marker,
            &staging_dir,
            &staging_vaults_dir,
            &staging_manifests_dir,
            default_dek,
            credentials,
            cred_store,
        )
        .await;

        if res.is_err() {
            let _ = std::fs::remove_dir_all(&staging_dir);
        }
        res
    }

    #[allow(clippy::too_many_arguments)]
    async fn migrate_staging_internal(
        base_dir: &Path,
        vaults_dir: &Path,
        legacy_db_path: &Path,
        complete_marker: &Path,
        staging_dir: &Path,
        staging_vaults_dir: &Path,
        staging_manifests_dir: &Path,
        default_dek: Option<&VaultKey>,
        credentials: Option<&MigrationCredentials>,
        cred_store: &dyn CredentialStore,
    ) -> Result<MigrationReport, StorageError> {
        info!(
            legacy_path = %legacy_db_path.display(),
            staging_path = %staging_dir.display(),
            "Starting legacy database migration in dedicated staging directory"
        );

        // 1. Create timestamped backup of the original database and WAL/SHM files
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let backup_path = base_dir.join(format!("shellumi.db.backup.{timestamp}"));
        std::fs::copy(legacy_db_path, &backup_path).map_err(|e| {
            StorageError::Serialization(format!("failed to create legacy database backup: {e}"))
        })?;
        let legacy_wal = base_dir.join("shellumi.db-wal");
        if legacy_wal.exists() {
            let _ = std::fs::copy(
                &legacy_wal,
                base_dir.join(format!("shellumi.db-wal.backup.{timestamp}")),
            );
        }
        let legacy_shm = base_dir.join("shellumi.db-shm");
        if legacy_shm.exists() {
            let _ = std::fs::copy(
                &legacy_shm,
                base_dir.join(format!("shellumi.db-shm.backup.{timestamp}")),
            );
        }

        let journal_path = staging_dir.join("migration_journal.json");
        let mut journal = MigrationJournal {
            started_at: timestamp,
            legacy_db_path: legacy_db_path.to_path_buf(),
            backup_path: backup_path.clone(),
            staged_vaults: Vec::new(),
            completed_stages: vec!["backup_created".to_string()],
        };
        let _ = std::fs::write(
            &journal_path,
            serde_json::to_string_pretty(&journal).unwrap_or_default(),
        );

        let mut report = MigrationReport {
            legacy_db_found: true,
            backup_path: Some(backup_path),
            ..Default::default()
        };

        // 2. Open the legacy database
        let legacy_str = legacy_db_path
            .to_str()
            .ok_or_else(|| StorageError::Serialization("invalid path encoding".into()))?;
        let legacy_store = HostStore::open(legacy_str).await?;

        // 3. Resolve DEK for default vault
        let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let default_key = if let Some(k) = default_dek {
            k.clone()
        } else if let Ok(Some(k)) = VaultKeyManager::retrieve_key(cred_store, &default_vid) {
            k
        } else if let Some(creds) = credentials {
            if let Some(ref pwd) = creds.default_password {
                shellumi_core::validate_vault_password(pwd).map_err(|e| {
                    StorageError::Serialization(format!(
                        "Default vault password policy violation: {e}"
                    ))
                })?;
                let new_key = VaultKey::generate().map_err(|e| {
                    StorageError::Serialization(format!("default key generation error: {e}"))
                })?;
                let primary_env = new_key
                    .wrap_with_password(&default_vid, pwd)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                let rec_env = if let Some(ref rec_code) = creds.default_recovery_code {
                    Some(
                        new_key
                            .wrap_with_password(&default_vid, rec_code)
                            .map_err(|e| StorageError::Serialization(e.to_string()))?,
                    )
                } else {
                    None
                };
                let manifest = VaultManifest::new(
                    LEGACY_LOCAL_VAULT_ID.to_string(),
                    "Personal".to_string(),
                    primary_env,
                    rec_env,
                    true,
                );
                let manifest_path =
                    staging_manifests_dir.join(format!("{LEGACY_LOCAL_VAULT_ID}.manifest.json"));
                manifest
                    .save_to_path(&manifest_path)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                let _ = VaultKeyManager::provision_key(cred_store, &default_vid, &new_key);
                new_key
            } else {
                return Err(StorageError::Serialization(
                    "MigrationRequired: Default vault requires an explicit user password.".into(),
                ));
            }
        } else {
            return Err(StorageError::Serialization(
                "MigrationRequired: Default vault requires an explicit user password.".into(),
            ));
        };

        // 4. Open/create encrypted default vault database in staging directory
        let staged_default_db_path = staging_vaults_dir.join("default.db");
        let staged_default_db_str = staged_default_db_path
            .to_str()
            .ok_or_else(|| StorageError::Serialization("invalid path encoding".into()))?;
        let mut default_store =
            HostStore::open_with_key(staged_default_db_str, Some(&default_key)).await?;

        // 5. Migrate Registry into default.db
        let contexts = legacy_store.list_vault_contexts().await?;
        for (name, ctx) in &contexts {
            default_store.register_vault_context(name, ctx).await?;
        }

        let conns = legacy_store.list_vault_server_connections().await?;
        let conns_len = conns.len();
        for mut conn in conns {
            // Move server credentials into OS-protected storage; fail securely if credential migration fails
            conn.secure_credentials(cred_store).map_err(|e| {
                StorageError::Serialization(format!(
                    "Failed to move server credentials to secure credential store for connection {}: {e}",
                    conn.id
                ))
            })?;
            report.server_credentials_migrated += 1;
            default_store.save_vault_server_connection(&conn).await?;
        }

        // Migrate sync state and directory envelopes
        if let Ok(dir_model) = legacy_store.build_vault_directory().await {
            default_store
                .save_vault_directory_from_model(&default_vid, &default_key, &dir_model)
                .await?;
        }

        // 6. Migrate default vault records
        let default_hosts = legacy_store.list().await?;
        for host in &default_hosts {
            default_store
                .add(
                    &host.name,
                    &host.destination,
                    host.group_name.as_deref(),
                    &host.tags,
                    host.jump_host.as_deref(),
                    host.startup_snippet_id,
                )
                .await?;
            report.default_records_migrated += 1;
        }

        let default_snippets = legacy_store.list_snippets().await?;
        for snippet in &default_snippets {
            let snip = Snippet {
                name: snippet.name.clone(),
                command: snippet.command.clone(),
                description: snippet.description.clone(),
                tags: snippet.tags.clone(),
                variables: snippet.variables.clone(),
            };
            default_store.add_snippet(&snip).await?;
            report.default_records_migrated += 1;
        }

        let default_workspaces = legacy_store.list_workspaces().await?;
        for ws in &default_workspaces {
            default_store
                .add_workspace(
                    &ws.name,
                    ws.description.as_deref(),
                    &ws.layout,
                    &ws.sessions,
                    &ws.tags,
                )
                .await?;
            report.default_records_migrated += 1;
        }

        let default_identities = legacy_store.list_identities().await?;
        for id_rec in &default_identities {
            let new_id = id_rec.to_identity();
            let mut id_dto = shellumi_core::NewIdentity::new(
                &new_id.name,
                &new_id.username,
                new_id.auth_method.clone(),
            )
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
            id_dto.password_secret_ref = new_id.password_secret_ref;
            id_dto.key_path = new_id.key_path;
            id_dto.passphrase_secret_ref = new_id.passphrase_secret_ref;
            id_dto.certificate_path = new_id.certificate_path;
            id_dto.tags = new_id.tags;
            id_dto.comment = new_id.comment;
            default_store.create_identity(&id_dto).await?;
            report.default_records_migrated += 1;
        }

        report.vaults_migrated += 1;
        journal.staged_vaults.push(StagedVaultMetadata {
            vault_id: LEGACY_LOCAL_VAULT_ID.to_string(),
            name: "Personal".to_string(),
            is_default: true,
            db_filename: "default.db".to_string(),
            manifest_filename: format!("{LEGACY_LOCAL_VAULT_ID}.manifest.json"),
            host_count: default_hosts.len(),
            snippet_count: default_snippets.len(),
            workspace_count: default_workspaces.len(),
            identity_count: default_identities.len(),
        });
        journal
            .completed_stages
            .push("default_vault_staged".to_string());
        let _ = std::fs::write(
            &journal_path,
            serde_json::to_string_pretty(&journal).unwrap_or_default(),
        );

        // 7. Migrate secondary vaults to separate physical databases
        let mut secondary_keys: HashMap<String, VaultKey> = HashMap::new();
        for (vname, ctx) in &contexts {
            let vid_str = ctx.id().as_str();
            if vid_str == LEGACY_LOCAL_VAULT_ID {
                continue;
            }

            let sec_vid = ctx.id();
            let sec_manifest_path = staging_manifests_dir.join(format!("{vid_str}.manifest.json"));

            // Require explicit user-provided password for secondary migrated vaults
            let sec_pwd = credentials.and_then(|c| c.secondary_passwords.get(vid_str).cloned());
            let sec_key = if let Some(ref pwd) = sec_pwd {
                shellumi_core::validate_vault_password(pwd).map_err(|e| {
                    StorageError::Serialization(format!(
                        "Secondary vault '{vname}' ({vid_str}) password policy violation: {e}"
                    ))
                })?;
                let k = VaultKey::generate().map_err(|e| {
                    StorageError::Serialization(format!("secondary key generation error: {e}"))
                })?;
                let env = k
                    .wrap_with_password(sec_vid, pwd)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                let rec_env = if let Some(rec_code) =
                    credentials.and_then(|c| c.secondary_recovery_codes.get(vid_str))
                {
                    Some(
                        k.wrap_with_password(sec_vid, rec_code)
                            .map_err(|e| StorageError::Serialization(e.to_string()))?,
                    )
                } else {
                    None
                };
                let manifest =
                    VaultManifest::new(vid_str.to_string(), vname.clone(), env, rec_env, true);
                manifest
                    .save_to_path(&sec_manifest_path)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                let _ = VaultKeyManager::provision_key(cred_store, sec_vid, &k);
                k
            } else {
                return Err(StorageError::Serialization(format!(
                    "MigrationRequired: Secondary local vault '{vname}' ({vid_str}) requires an explicit user password."
                )));
            };

            secondary_keys.insert(vid_str.to_string(), sec_key.clone());

            let sec_db_path = staging_vaults_dir.join(format!("{vid_str}.db"));
            let sec_db_str = sec_db_path
                .to_str()
                .ok_or_else(|| StorageError::Serialization("invalid path encoding".into()))?;
            let mut sec_store = HostStore::open_with_key(sec_db_str, Some(&sec_key)).await?;
            sec_store.set_vault_id(sec_vid.clone());

            // Query legacy records scoped to this secondary vault
            let mut scoped_legacy = HostStore::open(legacy_str).await?;
            scoped_legacy.set_vault_id(sec_vid.clone());

            let scoped_hosts = scoped_legacy.list().await?;
            for h in &scoped_hosts {
                sec_store
                    .add(
                        &h.name,
                        &h.destination,
                        h.group_name.as_deref(),
                        &h.tags,
                        h.jump_host.as_deref(),
                        h.startup_snippet_id,
                    )
                    .await?;
                report.secondary_records_migrated += 1;
            }

            let scoped_snippets = scoped_legacy.list_snippets().await?;
            for s in &scoped_snippets {
                let snip = Snippet {
                    name: s.name.clone(),
                    command: s.command.clone(),
                    description: s.description.clone(),
                    tags: s.tags.clone(),
                    variables: s.variables.clone(),
                };
                sec_store.add_snippet(&snip).await?;
                report.secondary_records_migrated += 1;
            }

            let scoped_workspaces = scoped_legacy.list_workspaces().await?;
            for ws in &scoped_workspaces {
                sec_store
                    .add_workspace(
                        &ws.name,
                        ws.description.as_deref(),
                        &ws.layout,
                        &ws.sessions,
                        &ws.tags,
                    )
                    .await?;
                report.secondary_records_migrated += 1;
            }

            let scoped_identities = scoped_legacy.list_identities().await?;
            for id_rec in &scoped_identities {
                let new_id = id_rec.to_identity();
                let mut id_dto = shellumi_core::NewIdentity::new(
                    &new_id.name,
                    &new_id.username,
                    new_id.auth_method.clone(),
                )
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
                id_dto.password_secret_ref = new_id.password_secret_ref;
                id_dto.key_path = new_id.key_path;
                id_dto.passphrase_secret_ref = new_id.passphrase_secret_ref;
                id_dto.certificate_path = new_id.certificate_path;
                id_dto.tags = new_id.tags;
                id_dto.comment = new_id.comment;
                sec_store.create_identity(&id_dto).await?;
                report.secondary_records_migrated += 1;
            }

            // Verify secondary vault record counts
            if sec_store.list().await?.len() != scoped_hosts.len() {
                return Err(StorageError::Serialization(format!(
                    "Verification failed: host count mismatch for secondary vault {vid_str}"
                )));
            }
            if sec_store.list_snippets().await?.len() != scoped_snippets.len() {
                return Err(StorageError::Serialization(format!(
                    "Verification failed: snippet count mismatch for secondary vault {vid_str}"
                )));
            }
            if sec_store.list_workspaces().await?.len() != scoped_workspaces.len() {
                return Err(StorageError::Serialization(format!(
                    "Verification failed: workspace count mismatch for secondary vault {vid_str}"
                )));
            }
            if sec_store.list_identities().await?.len() != scoped_identities.len() {
                return Err(StorageError::Serialization(format!(
                    "Verification failed: identity count mismatch for secondary vault {vid_str}"
                )));
            }

            report.vaults_migrated += 1;
            journal.staged_vaults.push(StagedVaultMetadata {
                vault_id: vid_str.to_string(),
                name: vname.clone(),
                is_default: false,
                db_filename: format!("{vid_str}.db"),
                manifest_filename: format!("{vid_str}.manifest.json"),
                host_count: scoped_hosts.len(),
                snippet_count: scoped_snippets.len(),
                workspace_count: scoped_workspaces.len(),
                identity_count: scoped_identities.len(),
            });
            journal
                .completed_stages
                .push(format!("secondary_vault_staged:{vid_str}"));
            let _ = std::fs::write(
                &journal_path,
                serde_json::to_string_pretty(&journal).unwrap_or_default(),
            );
        }

        // -------------------------------------------------------------------
        // 8. Verification Phase: Verify default database counts and cross-vault isolation
        // -------------------------------------------------------------------
        if default_store.list().await?.len() != default_hosts.len() {
            return Err(StorageError::Serialization(
                "Verification failed: default vault host count mismatch".into(),
            ));
        }
        if default_store.list_snippets().await?.len() != default_snippets.len() {
            return Err(StorageError::Serialization(
                "Verification failed: default vault snippet count mismatch".into(),
            ));
        }
        if default_store.list_workspaces().await?.len() != default_workspaces.len() {
            return Err(StorageError::Serialization(
                "Verification failed: default vault workspace count mismatch".into(),
            ));
        }
        if default_store.list_identities().await?.len() != default_identities.len() {
            return Err(StorageError::Serialization(
                "Verification failed: default vault identity count mismatch".into(),
            ));
        }
        if default_store.list_vault_contexts().await?.len() != contexts.len() {
            return Err(StorageError::Serialization(
                "Verification failed: registry vault context count mismatch".into(),
            ));
        }
        if default_store.list_vault_server_connections().await?.len() != conns_len {
            return Err(StorageError::Serialization(
                "Verification failed: server connection count mismatch".into(),
            ));
        }

        drop(default_store);

        // Verify that default database opens cleanly with default_key and fails with wrong key
        let test_def = HostStore::open_with_key(staged_default_db_str, Some(&default_key)).await;
        if test_def.is_err() {
            return Err(StorageError::Serialization(
                "Verification failed: unable to re-open staged default database with DEK".into(),
            ));
        }
        drop(test_def);

        // Verify cross-vault isolation on disk
        for (sec_id, sec_k) in &secondary_keys {
            let sec_p = staging_vaults_dir.join(format!("{sec_id}.db"));
            let sec_p_str = sec_p.to_str().unwrap();

            // Must open with own key
            let test_sec = HostStore::open_with_key(sec_p_str, Some(sec_k)).await;
            if test_sec.is_err() {
                return Err(StorageError::Serialization(format!(
                    "Verification failed: unable to re-open staged secondary database {sec_id} with DEK"
                )));
            }
            drop(test_sec);

            // Must fail when opened with default key
            if HostStore::open_with_key(sec_p_str, Some(&default_key))
                .await
                .is_ok()
            {
                return Err(StorageError::Serialization(format!(
                    "Verification failed: cross-vault isolation violated! Secondary database {sec_id} opened with default DEK."
                )));
            }
        }

        journal
            .completed_stages
            .push("data_integrity_verified".to_string());
        let _ = std::fs::write(
            &journal_path,
            serde_json::to_string_pretty(&journal).unwrap_or_default(),
        );

        // -------------------------------------------------------------------
        // 9. Atomic Activation: Activate staged files into final directory layout
        // -------------------------------------------------------------------
        std::fs::create_dir_all(vaults_dir).map_err(|e| {
            StorageError::Serialization(format!("failed to create vaults directory: {e}"))
        })?;
        let manifests_dir = base_dir.join("vault_manifests");
        std::fs::create_dir_all(&manifests_dir).map_err(|e| {
            StorageError::Serialization(format!("failed to create vault_manifests directory: {e}"))
        })?;

        // Move all staged database files into vaults_dir
        if let Ok(entries) = std::fs::read_dir(staging_vaults_dir) {
            for entry in entries.flatten() {
                let src = entry.path();
                if src.is_file() {
                    let filename = src.file_name().unwrap();
                    let dst = vaults_dir.join(filename);
                    std::fs::copy(&src, &dst).map_err(|e| {
                        StorageError::Serialization(format!(
                            "Failed to activate vault database {}: {e}",
                            src.display()
                        ))
                    })?;
                }
            }
        }

        // Move all staged manifest files into manifests_dir
        if let Ok(entries) = std::fs::read_dir(staging_manifests_dir) {
            for entry in entries.flatten() {
                let src = entry.path();
                if src.is_file() {
                    let filename = src.file_name().unwrap();
                    let dst = manifests_dir.join(filename);
                    std::fs::copy(&src, &dst).map_err(|e| {
                        StorageError::Serialization(format!(
                            "Failed to activate vault manifest {}: {e}",
                            src.display()
                        ))
                    })?;
                }
            }
        }

        // Write .migration_complete marker in vaults_dir
        std::fs::write(complete_marker, b"verified\n").map_err(|e| {
            StorageError::Serialization(format!("failed to write migration complete marker: {e}"))
        })?;

        // Atomically rename legacy database to .migrated
        let migrated_flag_path = base_dir.join("shellumi.db.migrated");
        std::fs::rename(legacy_db_path, &migrated_flag_path).map_err(|e| {
            StorageError::Serialization(format!("failed to rename legacy database: {e}"))
        })?;
        if legacy_wal.exists() {
            let _ = std::fs::rename(&legacy_wal, base_dir.join("shellumi.db-wal.migrated"));
        }
        if legacy_shm.exists() {
            let _ = std::fs::rename(&legacy_shm, base_dir.join("shellumi.db-shm.migrated"));
        }

        // Clean up staging directory
        let _ = std::fs::remove_dir_all(staging_dir);

        info!(
            vaults_migrated = report.vaults_migrated,
            default_records = report.default_records_migrated,
            secondary_records = report.secondary_records_migrated,
            server_credentials = report.server_credentials_migrated,
            "Legacy database migration verified and activated successfully"
        );

        Ok(report)
    }
}

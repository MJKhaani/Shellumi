use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
#[allow(dead_code)] // All variants are exercised in cross-platform path tests.
pub(crate) enum Platform {
    Mac,
    Windows,
    Linux,
}

pub(crate) fn resolve_data_dir(
    platform: Platform,
    home: Option<&OsStr>,
    app_data: Option<&OsStr>,
    xdg_data: Option<&OsStr>,
) -> Result<PathBuf, String> {
    let home_dir = || {
        home.filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| {
                "A valid home directory is required to locate Shellumi's data.".to_owned()
            })
    };
    let root = match platform {
        Platform::Mac => home_dir()?.join("Library/Application Support"),
        Platform::Windows => app_data
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| "APPDATA is unavailable; cannot locate Shellumi's data.".to_owned())?,
        Platform::Linux => match xdg_data
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
        {
            Some(path) => path,
            None => home_dir()?.join(".local/share"),
        },
    };
    Ok(root.join("Shellumi"))
}

pub(crate) fn default_data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    let platform = Platform::Mac;
    #[cfg(target_os = "windows")]
    let platform = Platform::Windows;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let platform = Platform::Linux;
    resolve_data_dir(
        platform,
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("APPDATA").as_deref(),
        std::env::var_os("XDG_DATA_HOME").as_deref(),
    )
}

pub(crate) fn is_default_vault(vault_id: &str) -> bool {
    vault_id.is_empty() || vault_id == "default" || vault_id == "legacy-local"
}

pub(crate) fn resolve_root_dir(base_path: &Path) -> PathBuf {
    let has_extension = base_path.extension().is_some();
    if has_extension {
        if let Some(parent) = base_path.parent() {
            if parent.file_name() == Some(OsStr::new("vaults"))
                || parent.file_name() == Some(OsStr::new("vault_manifests"))
            {
                parent.parent().unwrap_or(parent).to_path_buf()
            } else {
                parent.to_path_buf()
            }
        } else {
            PathBuf::from(".")
        }
    } else if base_path.file_name() == Some(OsStr::new("vaults"))
        || base_path.file_name() == Some(OsStr::new("vault_manifests"))
    {
        base_path.parent().unwrap_or(base_path).to_path_buf()
    } else {
        base_path.to_path_buf()
    }
}

pub(crate) fn resolve_vault_dir(base_path: &Path) -> PathBuf {
    let root = resolve_root_dir(base_path);
    root.join("vaults")
}

pub(crate) fn resolve_vault_db_file(base_path: &Path, vault_id: &str) -> PathBuf {
    let vaults_dir = resolve_vault_dir(base_path);
    if is_default_vault(vault_id) {
        vaults_dir.join("default.db")
    } else {
        let sanitized: String = vault_id
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        vaults_dir.join(format!("{sanitized}.db"))
    }
}

pub(crate) fn resolve_manifest_dir(base_path: &Path) -> PathBuf {
    let root = resolve_root_dir(base_path);
    root.join("vault_manifests")
}

pub(crate) fn resolve_manifest_file(base_path: &Path, vault_id: &str) -> PathBuf {
    let manifests_dir = resolve_manifest_dir(base_path);
    let vid = if is_default_vault(vault_id) {
        shellumi_storage::LEGACY_LOCAL_VAULT_ID
    } else {
        vault_id
    };
    manifests_dir.join(format!("{vid}.manifest.json"))
}

#[allow(dead_code)]
pub(crate) fn resolve_legacy_db_file(base_path: &Path) -> PathBuf {
    let root = resolve_root_dir(base_path);
    root.join("shellumi.db")
}

#[allow(dead_code)]
pub(crate) fn resolve_data_file(
    platform: Platform,
    home: Option<&OsStr>,
    app_data: Option<&OsStr>,
    xdg_data: Option<&OsStr>,
) -> Result<PathBuf, String> {
    let dir = resolve_data_dir(platform, home, app_data, xdg_data)?;
    Ok(resolve_vault_db_file(&dir, "default"))
}

#[allow(dead_code)]
pub(crate) fn resolve_legacy_data_file(
    platform: Platform,
    home: Option<&OsStr>,
    app_data: Option<&OsStr>,
    xdg_data: Option<&OsStr>,
) -> Result<PathBuf, String> {
    let dir = resolve_data_dir(platform, home, app_data, xdg_data)?;
    Ok(dir.join("shellumi.db"))
}

pub(crate) fn default_data_file() -> Result<PathBuf, String> {
    let dir = default_data_dir()?;
    Ok(resolve_vault_db_file(&dir, "default"))
}

pub(crate) fn prepare_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Database path has no parent directory.".to_owned())?;
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Cannot create Shellumi data directory {}: {error}",
            parent.display()
        )
    })
}

pub(crate) fn resolve_quarantine_dir(base_path: &Path) -> PathBuf {
    let root = resolve_root_dir(base_path);
    root.join("quarantine")
}

pub(crate) fn is_safe_vault_id(vault_id: &str) -> bool {
    !vault_id.is_empty()
        && !vault_id.contains("..")
        && !vault_id.contains('/')
        && !vault_id.contains('\\')
        && vault_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum QuarantineStage {
    Initiated,
    WalShmStaged,
    DatabaseStaged,
    ManifestStaged,
    Completed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct VaultQuarantineJournal {
    pub journal_version: u32,
    pub vault_id: String,
    pub original_db_path: String,
    pub original_manifest_path: Option<String>,
    pub original_wal_path: Option<String>,
    pub original_shm_path: Option<String>,
    pub quarantined_db_path: Option<String>,
    pub quarantined_manifest_path: Option<String>,
    pub quarantined_wal_path: Option<String>,
    pub quarantined_shm_path: Option<String>,
    pub stage: QuarantineStage,
    pub quarantined_at: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct VaultQuarantineRecord {
    pub vault_id: String,
    pub original_db_path: String,
    pub original_manifest_path: Option<String>,
    pub quarantined_at: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PasswordRotationStage {
    Initiated,
    ManifestSaved,
    TargetDbUpdated,
    RegistryUpdated,
    CredentialStoreUpdated,
    Verified,
    Completed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PasswordRotationJournal {
    pub journal_version: u32,
    pub vault_id: String,
    pub stage: PasswordRotationStage,
    pub original_manifest_backup: Option<String>,
    pub original_registry_envelope: Option<String>,
    pub original_target_db_envelope: Option<String>,
    pub updated_at: u64,
}

pub(crate) fn write_durable_file(path: &Path, content: &str) -> Result<(), String> {
    use std::io::Write;
    let tmp = path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4().as_simple()));
    {
        let mut file = std::fs::File::create(&tmp)
            .map_err(|e| format!("Failed to create durable file {}: {e}", tmp.display()))?;
        file.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write durable file {}: {e}", tmp.display()))?;
        file.sync_all()
            .map_err(|e| format!("Failed to fsync durable file {}: {e}", tmp.display()))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        format!(
            "Failed to atomic rename durable file to {}: {e}",
            path.display()
        )
    })?;
    Ok(())
}

fn write_durable_journal(path: &Path, journal: &VaultQuarantineJournal) -> Result<(), String> {
    let json = serde_json::to_string_pretty(journal)
        .map_err(|e| format!("Failed to serialize quarantine journal: {e}"))?;
    write_durable_file(path, &json)
}

pub(crate) fn write_rotation_journal(
    path: &Path,
    journal: &PasswordRotationJournal,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(journal)
        .map_err(|e| format!("Failed to serialize rotation journal: {e}"))?;
    write_durable_file(path, &json)
}

pub(crate) fn detect_and_recover_interrupted_rotation(
    base_path: &Path,
) -> Result<Vec<String>, String> {
    let root = resolve_root_dir(base_path);
    let manifests_dir = resolve_manifest_dir(base_path);
    let vaults_dir = root.join("vaults");

    let mut search_dirs = Vec::new();
    if manifests_dir.exists() {
        search_dirs.push(manifests_dir);
    }
    if vaults_dir.exists() && vaults_dir != root.join("vault_manifests") {
        search_dirs.push(vaults_dir);
    }

    let mut recovered = Vec::new();
    for dir in search_dirs {
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("Failed to read directory {}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.to_string_lossy().ends_with(".rotation_journal.json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(journal) = serde_json::from_str::<PasswordRotationJournal>(&content) {
                        if journal.stage != PasswordRotationStage::Completed {
                            tracing::warn!(
                                vault_id = %journal.vault_id,
                                stage = ?journal.stage,
                                "Recovering interrupted password rotation"
                            );
                            if let Some(backup_content) = &journal.original_manifest_backup {
                                let manifest_path =
                                    resolve_manifest_file(base_path, &journal.vault_id);
                                let _ = write_durable_file(&manifest_path, backup_content);
                            }
                            if !recovered.contains(&journal.vault_id) {
                                recovered.push(journal.vault_id);
                            }
                        }
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }
    Ok(recovered)
}

pub(crate) fn quarantine_vault_files(
    base_path: &Path,
    vault_id: &str,
    reason: &str,
) -> Result<PathBuf, String> {
    if !is_safe_vault_id(vault_id) {
        return Err(format!("Invalid or unsafe vault ID '{vault_id}'"));
    }
    if is_default_vault(vault_id) {
        return Err("Cannot quarantine default vault".into());
    }

    let target_db = resolve_vault_db_file(base_path, vault_id);
    let target_manifest = resolve_manifest_file(base_path, vault_id);
    let wal = PathBuf::from(format!("{}-wal", target_db.display()));
    let shm = PathBuf::from(format!("{}-shm", target_db.display()));

    // Verify paths are not symlinks (prevent symlink redirection attacks)
    for p in [&target_db, &target_manifest, &wal, &shm] {
        if let Ok(meta) = std::fs::symlink_metadata(p) {
            if meta.file_type().is_symlink() {
                return Err(format!(
                    "Refusing to quarantine symlinked file: {}",
                    p.display()
                ));
            }
        }
    }

    let quarantine_root = resolve_quarantine_dir(base_path);
    std::fs::create_dir_all(&quarantine_root)
        .map_err(|e| format!("Failed to create quarantine root directory: {e}"))?;

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let q_folder_name = format!("{vault_id}_{now_secs}_{}", uuid::Uuid::new_v4().as_simple());
    let q_dir = quarantine_root.join(&q_folder_name);
    std::fs::create_dir_all(&q_dir)
        .map_err(|e| format!("Failed to create quarantine vault folder: {e}"))?;

    // Canonical path validation: verify q_dir stays within quarantine_root
    if let (Ok(canon_root), Ok(canon_q)) = (quarantine_root.canonicalize(), q_dir.canonicalize()) {
        if !canon_q.starts_with(&canon_root) {
            return Err("Quarantine folder escapes quarantine root directory".into());
        }
    }

    let journal_path = q_dir.join("journal.json");
    let mut journal = VaultQuarantineJournal {
        journal_version: 1,
        vault_id: vault_id.to_string(),
        original_db_path: target_db.to_string_lossy().to_string(),
        original_manifest_path: if target_manifest.exists() {
            Some(target_manifest.to_string_lossy().to_string())
        } else {
            None
        },
        original_wal_path: if wal.exists() {
            Some(wal.to_string_lossy().to_string())
        } else {
            None
        },
        original_shm_path: if shm.exists() {
            Some(shm.to_string_lossy().to_string())
        } else {
            None
        },
        quarantined_db_path: None,
        quarantined_manifest_path: None,
        quarantined_wal_path: None,
        quarantined_shm_path: None,
        stage: QuarantineStage::Initiated,
        quarantined_at: now_secs.to_string(),
        reason: reason.to_string(),
    };

    // Stage 1: Persist durable operation journal with fsync BEFORE moving any files
    write_durable_journal(&journal_path, &journal)?;

    // Stage 2: Quarantine WAL / SHM files
    if wal.exists() {
        let q_wal = q_dir.join(format!("{vault_id}.db-wal"));
        std::fs::rename(&wal, &q_wal)
            .or_else(|_| {
                std::fs::copy(&wal, &q_wal)?;
                std::fs::remove_file(&wal)
            })
            .map_err(|e| format!("Failed to quarantine WAL file: {e}"))?;
        journal.quarantined_wal_path = Some(q_wal.to_string_lossy().to_string());
    }
    if shm.exists() {
        let q_shm = q_dir.join(format!("{vault_id}.db-shm"));
        std::fs::rename(&shm, &q_shm)
            .or_else(|_| {
                std::fs::copy(&shm, &q_shm)?;
                std::fs::remove_file(&shm)
            })
            .map_err(|e| format!("Failed to quarantine SHM file: {e}"))?;
        journal.quarantined_shm_path = Some(q_shm.to_string_lossy().to_string());
    }
    journal.stage = QuarantineStage::WalShmStaged;
    write_durable_journal(&journal_path, &journal)?;

    // Stage 3: Quarantine database file
    let q_db = q_dir.join(format!("{vault_id}.db"));
    if target_db.exists() {
        std::fs::rename(&target_db, &q_db)
            .or_else(|_| {
                std::fs::copy(&target_db, &q_db)?;
                std::fs::remove_file(&target_db)
            })
            .map_err(|e| format!("Failed to quarantine database file: {e}"))?;
        journal.quarantined_db_path = Some(q_db.to_string_lossy().to_string());
    }
    journal.stage = QuarantineStage::DatabaseStaged;
    write_durable_journal(&journal_path, &journal)?;

    // Stage 4: Quarantine manifest file
    let q_manifest = q_dir.join(format!("{vault_id}.manifest.json"));
    if target_manifest.exists() {
        std::fs::rename(&target_manifest, &q_manifest)
            .or_else(|_| {
                std::fs::copy(&target_manifest, &q_manifest)?;
                std::fs::remove_file(&target_manifest)
            })
            .map_err(|e| format!("Failed to quarantine manifest file: {e}"))?;
        journal.quarantined_manifest_path = Some(q_manifest.to_string_lossy().to_string());
    }
    journal.stage = QuarantineStage::ManifestStaged;
    write_durable_journal(&journal_path, &journal)?;

    // Stage 5: Finalize metadata & mark Completed
    let record = VaultQuarantineRecord {
        vault_id: vault_id.to_string(),
        original_db_path: target_db.to_string_lossy().to_string(),
        original_manifest_path: journal.original_manifest_path.clone(),
        quarantined_at: now_secs.to_string(),
        reason: reason.to_string(),
    };
    let meta_path = q_dir.join("quarantine_meta.json");
    let record_json = serde_json::to_string_pretty(&record)
        .map_err(|e| format!("Failed to serialize quarantine metadata: {e}"))?;
    write_durable_file(&meta_path, &record_json)?;

    journal.stage = QuarantineStage::Completed;
    write_durable_journal(&journal_path, &journal)?;

    tracing::info!(
        vault_id = %vault_id,
        quarantine_dir = %q_dir.display(),
        "Quarantined vault files securely with durable journal"
    );

    Ok(q_dir)
}

pub(crate) fn restore_quarantined_vault(q_dir: &Path) -> Result<(), String> {
    let journal_path = q_dir.join("journal.json");
    let meta_path = q_dir.join("quarantine_meta.json");

    let record = if journal_path.exists() {
        let content = std::fs::read_to_string(&journal_path)
            .map_err(|e| format!("Failed to read quarantine journal: {e}"))?;
        let journal: VaultQuarantineJournal = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse quarantine journal: {e}"))?;
        VaultQuarantineRecord {
            vault_id: journal.vault_id,
            original_db_path: journal.original_db_path,
            original_manifest_path: journal.original_manifest_path,
            quarantined_at: journal.quarantined_at,
            reason: journal.reason,
        }
    } else if meta_path.exists() {
        let content = std::fs::read_to_string(&meta_path)
            .map_err(|e| format!("Failed to read quarantine metadata: {e}"))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse quarantine metadata: {e}"))?
    } else {
        return Err("Neither quarantine journal nor metadata file found".into());
    };

    let target_db = PathBuf::from(&record.original_db_path);
    let target_manifest = record.original_manifest_path.as_ref().map(PathBuf::from);

    // Overwrite prevention: refuse to overwrite existing files!
    if target_db.exists() {
        return Err(format!(
            "Cannot restore vault '{}': destination database already exists at '{}'",
            record.vault_id,
            target_db.display()
        ));
    }
    if let Some(ref tm) = target_manifest {
        if tm.exists() {
            return Err(format!(
                "Cannot restore vault '{}': destination manifest already exists at '{}'",
                record.vault_id,
                tm.display()
            ));
        }
    }

    let q_db = q_dir.join(format!("{}.db", record.vault_id));
    if q_db.exists() {
        if let Some(parent) = target_db.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::rename(&q_db, &target_db)
            .or_else(|_| {
                std::fs::copy(&q_db, &target_db)?;
                std::fs::remove_file(&q_db)
            })
            .map_err(|e| format!("Failed to restore database from quarantine: {e}"))?;
    }

    if let Some(target_manifest) = target_manifest {
        let q_manifest = q_dir.join(format!("{}.manifest.json", record.vault_id));
        if q_manifest.exists() {
            if let Some(parent) = target_manifest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::rename(&q_manifest, &target_manifest)
                .or_else(|_| {
                    std::fs::copy(&q_manifest, &target_manifest)?;
                    std::fs::remove_file(&q_manifest)
                })
                .map_err(|e| format!("Failed to restore manifest from quarantine: {e}"))?;
        }
    }

    // Restore WAL / SHM if present
    let q_wal = q_dir.join(format!("{}.db-wal", record.vault_id));
    let orig_wal = PathBuf::from(format!("{}-wal", target_db.display()));
    if q_wal.exists() {
        let _ = std::fs::rename(&q_wal, &orig_wal);
    }
    let q_shm = q_dir.join(format!("{}.db-shm", record.vault_id));
    let orig_shm = PathBuf::from(format!("{}-shm", target_db.display()));
    if q_shm.exists() {
        let _ = std::fs::rename(&q_shm, &orig_shm);
    }

    // Remove quarantine folder
    let _ = std::fs::remove_dir_all(q_dir);
    tracing::info!(
        vault_id = %record.vault_id,
        "Restored quarantined vault files safely"
    );
    Ok(())
}

pub(crate) fn detect_and_recover_interrupted_quarantine(
    base_path: &Path,
) -> Result<Vec<String>, String> {
    let q_root = resolve_quarantine_dir(base_path);
    if !q_root.exists() {
        return Ok(Vec::new());
    }
    let mut recovered = Vec::new();
    let entries =
        std::fs::read_dir(&q_root).map_err(|e| format!("Failed to read quarantine root: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let journal_file = path.join("journal.json");
        if !journal_file.exists() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&journal_file) {
            if let Ok(mut journal) = serde_json::from_str::<VaultQuarantineJournal>(&content) {
                if journal.stage != QuarantineStage::Completed {
                    tracing::warn!(
                        vault_id = %journal.vault_id,
                        stage = ?journal.stage,
                        dir = %path.display(),
                        "Recovering interrupted vault quarantine"
                    );
                    match journal.stage {
                        QuarantineStage::Initiated => {
                            let _ = std::fs::remove_dir_all(&path);
                        }
                        QuarantineStage::WalShmStaged => {
                            if let (Some(q_wal), Some(orig_wal)) =
                                (&journal.quarantined_wal_path, &journal.original_wal_path)
                            {
                                let _ = std::fs::rename(q_wal, orig_wal);
                            }
                            if let (Some(q_shm), Some(orig_shm)) =
                                (&journal.quarantined_shm_path, &journal.original_shm_path)
                            {
                                let _ = std::fs::rename(q_shm, orig_shm);
                            }
                            let _ = std::fs::remove_dir_all(&path);
                        }
                        QuarantineStage::DatabaseStaged | QuarantineStage::ManifestStaged => {
                            if let Some(orig_manifest) = &journal.original_manifest_path {
                                let orig_m = Path::new(orig_manifest);
                                let q_m = path.join(format!("{}.manifest.json", journal.vault_id));
                                if orig_m.exists() && !q_m.exists() {
                                    let _ = std::fs::rename(orig_m, &q_m);
                                    journal.quarantined_manifest_path =
                                        Some(q_m.to_string_lossy().to_string());
                                }
                            }
                            let record = VaultQuarantineRecord {
                                vault_id: journal.vault_id.clone(),
                                original_db_path: journal.original_db_path.clone(),
                                original_manifest_path: journal.original_manifest_path.clone(),
                                quarantined_at: journal.quarantined_at.clone(),
                                reason: format!(
                                    "{} (recovered after interrupted operation)",
                                    journal.reason
                                ),
                            };
                            if let Ok(meta_json) = serde_json::to_string_pretty(&record) {
                                let _ =
                                    std::fs::write(path.join("quarantine_meta.json"), meta_json);
                            }
                            journal.stage = QuarantineStage::Completed;
                            let _ = write_durable_journal(&journal_file, &journal);
                        }
                        QuarantineStage::Completed => {}
                    }
                    recovered.push(journal.vault_id);
                }
            }
        }
    }
    Ok(recovered)
}

#[allow(dead_code)]
pub(crate) fn purge_quarantined_vault(q_dir: &Path) -> Result<(), String> {
    if !q_dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(q_dir).map_err(|e| format!("Failed to purge quarantine directory: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{
        prepare_parent, resolve_data_file, resolve_legacy_data_file, resolve_manifest_file,
        resolve_vault_db_file, Platform,
    };
    use std::ffi::OsStr;

    #[test]
    fn macos_uses_application_support() {
        let home = std::env::temp_dir().join("alice");
        assert_eq!(
            resolve_data_file(Platform::Mac, Some(home.as_os_str()), None, None).unwrap(),
            home.join("Library/Application Support/Shellumi/vaults/default.db")
        );
        assert_eq!(
            resolve_legacy_data_file(Platform::Mac, Some(home.as_os_str()), None, None).unwrap(),
            home.join("Library/Application Support/Shellumi/shellumi.db")
        );
    }

    #[test]
    fn windows_uses_roaming_app_data() {
        let appdata = std::env::temp_dir().join("Roaming");
        assert_eq!(
            resolve_data_file(Platform::Windows, None, Some(appdata.as_os_str()), None).unwrap(),
            appdata.join("Shellumi/vaults/default.db")
        );
    }

    #[test]
    fn linux_uses_absolute_xdg_or_home_fallback() {
        let home = std::env::temp_dir().join("alice");
        let xdg = std::env::temp_dir().join("data");
        assert_eq!(
            resolve_data_file(
                Platform::Linux,
                Some(home.as_os_str()),
                None,
                Some(xdg.as_os_str())
            )
            .unwrap(),
            xdg.join("Shellumi/vaults/default.db")
        );
        assert_eq!(
            resolve_data_file(
                Platform::Linux,
                Some(home.as_os_str()),
                None,
                Some(OsStr::new("relative"))
            )
            .unwrap(),
            home.join(".local/share/Shellumi/vaults/default.db")
        );
    }

    #[test]
    fn resolves_separate_vault_databases_and_manifests() {
        let base = std::path::Path::new("/Users/alice/Library/Application Support/Shellumi");
        assert_eq!(
            resolve_vault_db_file(base, "default"),
            base.join("vaults/default.db")
        );
        assert_eq!(
            resolve_vault_db_file(base, "legacy-local"),
            base.join("vaults/default.db")
        );
        assert_eq!(
            resolve_vault_db_file(base, "vlt-finances-2026"),
            base.join("vaults/vlt-finances-2026.db")
        );
        assert_eq!(
            resolve_manifest_file(base, "vlt-finances-2026"),
            base.join("vault_manifests/vlt-finances-2026.manifest.json")
        );
        assert_eq!(
            resolve_manifest_file(base, "default"),
            base.join("vault_manifests/legacy-local.manifest.json")
        );

        // Also works when passed a specific database file path
        let db_file = base.join("vaults/default.db");
        assert_eq!(
            resolve_vault_db_file(&db_file, "vlt-cloud"),
            base.join("vaults/vlt-cloud.db")
        );
        assert_eq!(
            resolve_manifest_file(&db_file, "vlt-cloud"),
            base.join("vault_manifests/vlt-cloud.manifest.json")
        );
    }

    #[test]
    fn missing_platform_directory_is_an_error() {
        assert!(resolve_data_file(Platform::Mac, None, None, None).is_err());
        assert!(resolve_data_file(Platform::Windows, None, None, None).is_err());
        assert!(resolve_data_file(Platform::Linux, None, None, None).is_err());
    }

    #[test]
    fn prepares_a_missing_data_directory_without_replacing_existing_data() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Shellumi").join("shellumi.db");
        prepare_parent(&path).unwrap();
        std::fs::write(&path, b"existing database").unwrap();
        prepare_parent(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"existing database");
    }

    #[test]
    fn initializes_database_in_new_directory_and_preserves_saved_hosts() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Application Support/Shellumi/shellumi.db");
        prepare_parent(&path).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut store = shellumi_storage::HostStore::open(path.to_str().unwrap())
                .await
                .unwrap();
            let destination =
                shellumi_core::SshDestination::new("example.org", "alice", 22).unwrap();
            store
                .add("Test host", &destination, None, &[], None, None)
                .await
                .unwrap();
            drop(store);
            prepare_parent(&path).unwrap();
            let store = shellumi_storage::HostStore::open(path.to_str().unwrap())
                .await
                .unwrap();
            let hosts = store.list().await.unwrap();
            assert_eq!(hosts.len(), 1);
            assert_eq!(hosts[0].destination, destination);
        });
    }
}

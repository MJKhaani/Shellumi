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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct VaultQuarantineRecord {
    pub vault_id: String,
    pub original_db_path: String,
    pub original_manifest_path: Option<String>,
    pub quarantined_at: String,
    pub reason: String,
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

    // Verify paths are not symlinks (prevent symlink redirection attacks)
    if let Ok(meta) = std::fs::symlink_metadata(&target_db) {
        if meta.file_type().is_symlink() {
            return Err(format!(
                "Refusing to quarantine symlinked database file: {}",
                target_db.display()
            ));
        }
    }
    if let Ok(meta) = std::fs::symlink_metadata(&target_manifest) {
        if meta.file_type().is_symlink() {
            return Err(format!(
                "Refusing to quarantine symlinked manifest file: {}",
                target_manifest.display()
            ));
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

    let mut moved_db = false;
    let mut moved_manifest = false;

    let q_db = q_dir.join(format!("{vault_id}.db"));
    if target_db.exists() {
        std::fs::rename(&target_db, &q_db)
            .or_else(|_| {
                std::fs::copy(&target_db, &q_db)?;
                std::fs::remove_file(&target_db)
            })
            .map_err(|e| format!("Failed to quarantine database file: {e}"))?;
        moved_db = true;
    }

    let q_manifest = q_dir.join(format!("{vault_id}.manifest.json"));
    if target_manifest.exists() {
        std::fs::rename(&target_manifest, &q_manifest)
            .or_else(|_| {
                std::fs::copy(&target_manifest, &q_manifest)?;
                std::fs::remove_file(&target_manifest)
            })
            .map_err(|e| format!("Failed to quarantine manifest file: {e}"))?;
        moved_manifest = true;
    }

    // Quarantine any SQLite WAL / SHM files
    let wal = PathBuf::from(format!("{}-wal", target_db.display()));
    if wal.exists() {
        let q_wal = q_dir.join(format!("{vault_id}.db-wal"));
        let _ = std::fs::rename(&wal, &q_wal);
    }
    let shm = PathBuf::from(format!("{}-shm", target_db.display()));
    if shm.exists() {
        let q_shm = q_dir.join(format!("{vault_id}.db-shm"));
        let _ = std::fs::rename(&shm, &q_shm);
    }

    let record = VaultQuarantineRecord {
        vault_id: vault_id.to_string(),
        original_db_path: target_db.to_string_lossy().to_string(),
        original_manifest_path: if moved_manifest {
            Some(target_manifest.to_string_lossy().to_string())
        } else {
            None
        },
        quarantined_at: now_secs.to_string(),
        reason: reason.to_string(),
    };
    let meta_path = q_dir.join("quarantine_meta.json");
    let record_json = serde_json::to_string_pretty(&record)
        .map_err(|e| format!("Failed to serialize quarantine metadata: {e}"))?;
    std::fs::write(&meta_path, record_json)
        .map_err(|e| format!("Failed to write quarantine metadata: {e}"))?;

    tracing::info!(
        vault_id = %vault_id,
        quarantine_dir = %q_dir.display(),
        moved_db = moved_db,
        moved_manifest = moved_manifest,
        "Quarantined vault files securely"
    );

    Ok(q_dir)
}

pub(crate) fn restore_quarantined_vault(q_dir: &Path) -> Result<(), String> {
    let meta_path = q_dir.join("quarantine_meta.json");
    if !meta_path.exists() {
        return Err("Quarantine metadata file not found".into());
    }
    let record_str = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read quarantine metadata: {e}"))?;
    let record: VaultQuarantineRecord = serde_json::from_str(&record_str)
        .map_err(|e| format!("Failed to parse quarantine metadata: {e}"))?;

    let q_db = q_dir.join(format!("{}.db", record.vault_id));
    let target_db = PathBuf::from(&record.original_db_path);
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

    if let Some(orig_manifest) = record.original_manifest_path {
        let q_manifest = q_dir.join(format!("{}.manifest.json", record.vault_id));
        let target_manifest = PathBuf::from(orig_manifest);
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

    // Remove quarantine folder
    let _ = std::fs::remove_dir_all(q_dir);
    tracing::info!(
        vault_id = %record.vault_id,
        "Restored quarantined vault files"
    );
    Ok(())
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

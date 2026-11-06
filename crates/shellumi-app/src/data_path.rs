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

pub(crate) fn resolve_data_file(
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
    Ok(root.join("Shellumi").join("shellumi.db"))
}

pub(crate) fn default_data_file() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    let platform = Platform::Mac;
    #[cfg(target_os = "windows")]
    let platform = Platform::Windows;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let platform = Platform::Linux;
    resolve_data_file(
        platform,
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("APPDATA").as_deref(),
        std::env::var_os("XDG_DATA_HOME").as_deref(),
    )
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

#[cfg(test)]
mod tests {
    use super::{prepare_parent, resolve_data_file, Platform};
    use std::ffi::OsStr;

    #[test]
    fn macos_uses_application_support() {
        let home = std::env::temp_dir().join("alice");
        assert_eq!(
            resolve_data_file(Platform::Mac, Some(home.as_os_str()), None, None).unwrap(),
            home.join("Library/Application Support/Shellumi/shellumi.db")
        );
    }

    #[test]
    fn windows_uses_roaming_app_data() {
        let appdata = std::env::temp_dir().join("Roaming");
        assert_eq!(
            resolve_data_file(Platform::Windows, None, Some(appdata.as_os_str()), None).unwrap(),
            appdata.join("Shellumi/shellumi.db")
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
            xdg.join("Shellumi/shellumi.db")
        );
        assert_eq!(
            resolve_data_file(
                Platform::Linux,
                Some(home.as_os_str()),
                None,
                Some(OsStr::new("relative"))
            )
            .unwrap(),
            home.join(".local/share/Shellumi/shellumi.db")
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

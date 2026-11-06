//! Local desktop operations. Call from a background worker, never the UI thread.
use crate::{HostStore, SavedSnippet, SavedWorkspace};
use serde::{Deserialize, Serialize};
use shellumi_core::{Snippet, WorkspaceSession};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub terminal_font_size: u16,
    pub scrollback_lines: usize,
    pub ui_font_size: u16,
    pub show_hidden_files: bool,
    pub startup_overview: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            terminal_font_size: 14,
            scrollback_lines: 5000,
            ui_font_size: 13,
            show_hidden_files: false,
            startup_overview: false,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if !(10..=24).contains(&self.terminal_font_size) || !(11..=18).contains(&self.ui_font_size)
        {
            return Err("Font size is outside the supported range.".into());
        }
        if !(100..=50000).contains(&self.scrollback_lines) {
            return Err("Scrollback must be between 100 and 50,000 lines.".into());
        }
        Ok(())
    }
    fn path(database: &Path) -> PathBuf {
        database.with_extension("preferences.json")
    }
    pub fn load(database: &Path) -> Result<Self, String> {
        match std::fs::read(Self::path(database)) {
            Ok(bytes) => {
                let prefs: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                prefs.validate()?;
                Ok(prefs)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(&self, database: &Path) -> Result<(), String> {
        self.validate()?;
        let path = Self::path(database);
        let temporary = path.with_extension("json.tmp");
        std::fs::write(
            &temporary,
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(temporary, path).map_err(|e| e.to_string())
    }
}
#[derive(Clone, Debug)]
pub enum Command {
    Refresh,
    ExportBackup {
        path: PathBuf,
        passphrase: shellumi_core::SecretString,
    },
    RestoreBackup {
        path: PathBuf,
        passphrase: shellumi_core::SecretString,
    },
    SaveGroup(shellumi_core::HostGroup),
    ImportHosts {
        config: String,
        default_user: String,
    },
    RevokeKnownHost {
        path: PathBuf,
        expected: String,
    },
    SaveSnippet {
        id: Option<i64>,
        snippet: Snippet,
    },
    DeleteSnippet(i64),
    SaveWorkspace {
        id: Option<i64>,
        name: String,
        description: String,
        layout: String,
        sessions: Vec<WorkspaceSession>,
    },
    DeleteWorkspace(i64),
    DuplicateWorkspace {
        id: i64,
        name: String,
    },
    SavePreferences(Preferences),
    ImportSnippets(String),
}
pub struct Snapshot {
    pub identities: Vec<crate::SavedIdentity>,
    pub hosts: Vec<crate::SavedHost>,
    pub groups: Vec<shellumi_core::HostGroup>,
    pub snippets: Vec<SavedSnippet>,
    pub workspaces: Vec<SavedWorkspace>,
    pub preferences: Preferences,
}
pub async fn execute(database: &str, command: Command) -> Result<Snapshot, String> {
    let mut store = HostStore::open(database).await.map_err(|e| e.to_string())?;
    match command {
        Command::Refresh => {}
        Command::ExportBackup { path, passphrase } => {
            if passphrase.expose_secret().chars().count() < 12 {
                return Err("Use a backup passphrase with at least 12 characters.".into());
            }
            let vault = store
                .export_vault(passphrase.expose_secret(), "shellumi-desktop")
                .await
                .map_err(|e| e.to_string())?;
            let data = serde_json::to_vec_pretty(&vault).map_err(|e| e.to_string())?;
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| {
                    format!("Could not create backup (existing files are never overwritten): {e}")
                })?;
            file.write_all(&data).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        Command::RestoreBackup { path, passphrase } => {
            let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.len() > 50_000_000 {
                return Err("Backup must be a file smaller than 50 MB.".into());
            }
            let vault: shellumi_core::SyncVault =
                serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            store
                .connection
                .execute("BEGIN IMMEDIATE", ())
                .await
                .map_err(|e| e.to_string())?;
            let result = store.import_vault(passphrase.expose_secret(), &vault).await;
            match result {
                Ok(summary) if summary.errors.is_empty() => {
                    store
                        .connection
                        .execute("COMMIT", ())
                        .await
                        .map_err(|e| e.to_string())?;
                }
                Ok(summary) => {
                    let _ = store.connection.execute("ROLLBACK", ()).await;
                    return Err(format!(
                        "Backup could not be restored: {}",
                        summary.errors.join("; ")
                    ));
                }
                Err(error) => {
                    let _ = store.connection.execute("ROLLBACK", ()).await;
                    return Err(error.to_string());
                }
            }
        }
        Command::SaveGroup(group) => {
            if store
                .get_group(&group.name)
                .await
                .map_err(|e| e.to_string())?
                .is_some()
            {
                store
                    .update_group(&group)
                    .await
                    .map_err(|e| e.to_string())?;
            } else {
                store.add_group(&group).await.map_err(|e| e.to_string())?;
            }
        }
        Command::ImportHosts {
            config,
            default_user,
        } => {
            let entries = shellumi_core::parse_ssh_config(&config, Some(&default_user));
            if entries.is_empty() {
                return Err("No valid named hosts found. Include Host, HostName and User, or supply a default username.".into());
            }
            store
                .connection
                .execute("BEGIN IMMEDIATE", ())
                .await
                .map_err(|e| e.to_string())?;
            for entry in entries {
                if let Err(error) = store
                    .add(&entry.name, &entry.destination, None, &[], None, None)
                    .await
                {
                    let _ = store.connection.execute("ROLLBACK", ()).await;
                    return Err(error.to_string());
                }
            }
            store
                .connection
                .execute("COMMIT", ())
                .await
                .map_err(|e| e.to_string())?;
        }
        Command::RevokeKnownHost { path, expected } => {
            let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("Refusing to rewrite a symlink or non-file trust store.".into());
            }
            let original = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            if !original.lines().any(|line| line == expected) {
                return Err("The trust record changed. Refresh before revoking it.".into());
            }
            if expected.starts_with('@') {
                return Err(
                    "This record already has a trust marker. Edit it with OpenSSH tools.".into(),
                );
            }
            let updated = original
                .lines()
                .map(|line| {
                    if line == expected {
                        format!("@revoked {line}")
                    } else {
                        line.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            let temporary = path.with_extension("shellumi.tmp");
            std::fs::write(&temporary, updated).map_err(|e| e.to_string())?;
            std::fs::set_permissions(&temporary, metadata.permissions())
                .map_err(|e| e.to_string())?;
            std::fs::rename(temporary, path).map_err(|e| e.to_string())?;
        }
        Command::SaveSnippet { id, snippet } => {
            if let Some(id) = id {
                store
                    .update_snippet(id, &snippet)
                    .await
                    .map_err(|e| e.to_string())?;
            } else {
                store
                    .add_snippet(&snippet)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        Command::DeleteSnippet(id) => {
            store.delete_snippet(id).await.map_err(|e| e.to_string())?;
        }
        Command::SaveWorkspace {
            id,
            name,
            description,
            layout,
            sessions,
        } => {
            if shellumi_core::WorkspaceLayout::parse(&layout).is_none() {
                return Err("Invalid workspace layout.".into());
            }
            if let Some(id) = id {
                store
                    .update_workspace(id, &name, Some(&description), &layout, &sessions, &[])
                    .await
                    .map_err(|e| e.to_string())?;
            } else {
                store
                    .add_workspace(&name, Some(&description), &layout, &sessions, &[])
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        Command::DeleteWorkspace(id) => {
            store
                .delete_workspace(id)
                .await
                .map_err(|e| e.to_string())?;
        }
        Command::DuplicateWorkspace { id, name } => {
            store
                .duplicate_workspace(id, &name)
                .await
                .map_err(|e| e.to_string())?;
        }
        Command::SavePreferences(prefs) => prefs.save(Path::new(database))?,
        Command::ImportSnippets(json) => {
            let snippets: Vec<Snippet> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            // Validate the entire package before mutating storage.
            let snippets = snippets
                .into_iter()
                .map(|s| Snippet::new(s.name, s.command, s.description, &s.tags))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            store
                .connection
                .execute("BEGIN IMMEDIATE", ())
                .await
                .map_err(|e| e.to_string())?;
            for snippet in snippets {
                if let Err(error) = store.add_snippet(&snippet).await {
                    let _ = store.connection.execute("ROLLBACK", ()).await;
                    return Err(error.to_string());
                }
            }
            store
                .connection
                .execute("COMMIT", ())
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(Snapshot {
        identities: store.list_identities().await.map_err(|e| e.to_string())?,
        hosts: store.list().await.map_err(|e| e.to_string())?,
        groups: store.list_groups().await.map_err(|e| e.to_string())?,
        snippets: store.list_snippets().await.map_err(|e| e.to_string())?,
        workspaces: store.list_workspaces().await.map_err(|e| e.to_string())?,
        preferences: Preferences::load(Path::new(database))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn desktop_edits_survive_reopen_and_invalid_packages_do_not_partially_import() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shellumi.db");
        let path = path.to_str().unwrap();
        let snippet = Snippet::new("Inspect", "echo {{name:-world}}", None, &[]).unwrap();
        let state = execute(path, Command::SaveSnippet { id: None, snippet })
            .await
            .unwrap();
        let id = state.snippets[0].id;
        let updated = Snippet::new("Updated", "uptime", None, &[]).unwrap();
        execute(
            path,
            Command::SaveSnippet {
                id: Some(id),
                snippet: updated,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            execute(path, Command::Refresh).await.unwrap().snippets[0].command,
            "uptime"
        );
        let invalid = r#"[{"name":"Valid","command":"pwd","description":null,"tags":[],"variables":[]},{"name":"","command":"pwd","description":null,"tags":[],"variables":[]}]"#;
        assert!(execute(path, Command::ImportSnippets(invalid.into()))
            .await
            .is_err());
        assert_eq!(
            execute(path, Command::Refresh)
                .await
                .unwrap()
                .snippets
                .len(),
            1
        );
        let preferences = Preferences {
            terminal_font_size: 18,
            show_hidden_files: true,
            ..Preferences::default()
        };
        execute(path, Command::SavePreferences(preferences.clone()))
            .await
            .unwrap();
        assert_eq!(
            execute(path, Command::Refresh).await.unwrap().preferences,
            preferences
        );
        execute(path, Command::DeleteSnippet(id)).await.unwrap();
        assert!(execute(path, Command::Refresh)
            .await
            .unwrap()
            .snippets
            .is_empty());
    }
    #[test]
    fn invalid_preferences_are_rejected_without_replacing_existing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        Preferences::default().save(&db).unwrap();
        assert!(Preferences {
            scrollback_lines: usize::MAX,
            ..Preferences::default()
        }
        .save(&db)
        .is_err());
        assert_eq!(Preferences::load(&db).unwrap(), Preferences::default());
    }
    #[tokio::test]
    async fn workspace_editor_supports_reopen_rename_duplicate_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("local.db");
        let db = db.to_str().unwrap();
        let sessions = vec![WorkspaceSession {
            name: "Local".into(),
            kind: shellumi_core::WorkspaceSessionKind::Local,
            destination: None,
            host_id: None,
            startup_command: None,
            working_directory: None,
            pane_index: 0,
            excluded_from_broadcast: false,
        }];
        let snapshot = execute(
            db,
            Command::SaveWorkspace {
                id: None,
                name: "Operations".into(),
                description: "Local sessions".into(),
                layout: "vertical".into(),
                sessions: sessions.clone(),
            },
        )
        .await
        .unwrap();
        let id = snapshot.workspaces[0].id;
        execute(
            db,
            Command::SaveWorkspace {
                id: Some(id),
                name: "Renamed".into(),
                description: "Edited".into(),
                layout: "grid".into(),
                sessions,
            },
        )
        .await
        .unwrap();
        let snapshot = execute(
            db,
            Command::DuplicateWorkspace {
                id,
                name: "Copy".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(snapshot.workspaces.len(), 2);
        assert!(snapshot
            .workspaces
            .iter()
            .any(|w| w.name == "Renamed" && w.layout == "grid"));
        execute(db, Command::DeleteWorkspace(id)).await.unwrap();
        let reopened = execute(db, Command::Refresh).await.unwrap();
        assert_eq!(reopened.workspaces.len(), 1);
        assert_eq!(reopened.workspaces[0].name, "Copy");
    }
    #[tokio::test]
    async fn trust_revocation_rejects_stale_records_and_marks_matching_key() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("local.db");
        let db = db.to_str().unwrap();
        let trust = dir.path().join("known_hosts");
        let original = "example.org ssh-ed25519 AAAATEST";
        std::fs::write(&trust, format!("{original}\n")).unwrap();
        assert!(execute(
            db,
            Command::RevokeKnownHost {
                path: trust.clone(),
                expected: "stale record".into()
            }
        )
        .await
        .is_err());
        assert_eq!(
            std::fs::read_to_string(&trust).unwrap(),
            format!("{original}\n")
        );
        execute(
            db,
            Command::RevokeKnownHost {
                path: trust.clone(),
                expected: original.into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(trust).unwrap(),
            format!("@revoked {original}\n")
        );
    }
    #[tokio::test]
    async fn host_import_and_group_defaults_are_persistent() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("local.db");
        let db = db.to_str().unwrap();
        let group = shellumi_core::HostGroup::new(
            "Production",
            Some("deploy"),
            Some(2222),
            &["production".into()],
        )
        .unwrap();
        execute(db, Command::SaveGroup(group)).await.unwrap();
        let config = "Host api\n HostName 192.0.2.1\n Port 2200\n";
        execute(
            db,
            Command::ImportHosts {
                config: config.into(),
                default_user: "ubuntu".into(),
            },
        )
        .await
        .unwrap();
        let reopened = execute(db, Command::Refresh).await.unwrap();
        assert_eq!(reopened.hosts.len(), 1);
        assert_eq!(reopened.hosts[0].destination.port().get(), 2200);
        assert_eq!(
            reopened.groups[0].default_username.as_deref(),
            Some("deploy")
        );
        let updated =
            shellumi_core::HostGroup::new("Production", Some("admin"), None, &[]).unwrap();
        let snapshot = execute(db, Command::SaveGroup(updated)).await.unwrap();
        assert_eq!(snapshot.groups.len(), 1);
        assert_eq!(
            snapshot.groups[0].default_username.as_deref(),
            Some("admin")
        );
    }
    #[tokio::test]
    async fn encrypted_desktop_backup_roundtrips_without_plaintext_or_overwriting_files() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("source.db");
        let db = db.to_str().unwrap();
        execute(
            db,
            Command::SaveSnippet {
                id: None,
                snippet: Snippet::new("Inspect", "echo backup-test-value", None, &[]).unwrap(),
            },
        )
        .await
        .unwrap();
        let backup = dir.path().join("backup.json");
        let secret = shellumi_core::SecretString::new("desktop-test-passphrase".into());
        execute(
            db,
            Command::ExportBackup {
                path: backup.clone(),
                passphrase: secret.clone(),
            },
        )
        .await
        .unwrap();
        let encrypted = std::fs::read_to_string(&backup).unwrap();
        assert!(!encrypted.contains("backup-test-value"));
        assert!(execute(
            db,
            Command::ExportBackup {
                path: backup.clone(),
                passphrase: secret.clone()
            }
        )
        .await
        .is_err());
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), encrypted);
        let restored = dir.path().join("restored.db");
        let restored = restored.to_str().unwrap();
        assert!(execute(
            restored,
            Command::RestoreBackup {
                path: backup.clone(),
                passphrase: shellumi_core::SecretString::new("wrong secret".into())
            }
        )
        .await
        .is_err());
        assert!(execute(restored, Command::Refresh)
            .await
            .unwrap()
            .snippets
            .is_empty());
        let state = execute(
            restored,
            Command::RestoreBackup {
                path: backup,
                passphrase: secret,
            },
        )
        .await
        .unwrap();
        assert_eq!(state.snippets[0].command, "echo backup-test-value");
    }
}

#[derive(Clone, Debug)]
pub struct PublicSshRecord {
    pub name: String,
    pub key_type: String,
    pub public_key: String,
    pub fingerprint: String,
    pub path: PathBuf,
    pub trust_record: Option<String>,
}
/// Reads public metadata only. Private key material is never loaded.
pub fn public_ssh_records(home: &Path, keys: bool) -> Result<Vec<PublicSshRecord>, String> {
    let ssh = home.join(".ssh");
    if keys {
        let entries = match std::fs::read_dir(&ssh) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        let mut records = Vec::new();
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|ext| ext != "pub") {
                continue;
            }
            let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.len() > 65536 {
                continue;
            }
            let public_key = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let output = std::process::Command::new("ssh-keygen")
                .arg("-l")
                .arg("-f")
                .arg(&path)
                .output()
                .map_err(|e| e.to_string())?;
            if !output.status.success() {
                continue;
            }
            records.push(PublicSshRecord {
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                key_type: public_key
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_owned(),
                fingerprint: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
                public_key,
                path,
                trust_record: None,
            });
        }
        records.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(records)
    } else {
        let path = ssh.join("known_hosts");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        Ok(shellumi_core::parse_known_hosts(&text)
            .into_iter()
            .map(|entry| PublicSshRecord {
                name: entry.hosts.join(", "),
                key_type: entry.key_type.clone(),
                public_key: format!("{} {}", entry.key_type, entry.key_base64),
                fingerprint: format!(
                    "{} · {}",
                    shellumi_core::fingerprint_public_key(&format!(
                        "{} {}",
                        entry.key_type, entry.key_base64
                    ))
                    .unwrap_or_else(|_| "Invalid key encoding".into()),
                    if entry.marker == Some(shellumi_core::HostKeyMarker::Revoked) {
                        "Revoked"
                    } else {
                        "Stored trust record"
                    }
                ),
                path: path.clone(),
                trust_record: text
                    .lines()
                    .nth(entry.line_number.saturating_sub(1))
                    .map(str::to_owned),
            })
            .collect())
    }
}

/// Resolve metadata on a background task. Explicit host identity takes precedence
/// over group inheritance; deleted references fail instead of selecting another account.
pub async fn resolve_connection(
    database: &str,
    host_id: i64,
) -> Result<shellumi_core::connection::ConnectionPlan, String> {
    let store = HostStore::open(database).await.map_err(|e| e.to_string())?;
    let host = store
        .get_host(host_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Host no longer exists.")?;
    if !host.is_ssh() {
        return Err("The selected host is no longer an SSH host.".into());
    }
    let group = if let Some(name) = &host.group_name {
        store.get_group(name).await.map_err(|e| e.to_string())?
    } else {
        None
    };
    let identity_id = host
        .identity_id
        .or_else(|| group.as_ref().and_then(|group| group.identity_id));
    let mut identity = if let Some(id) = identity_id {
        Some(
            store
                .get_identity(id)
                .await
                .map_err(|e| e.to_string())?
                .ok_or("Selected identity no longer exists.")?
                .to_identity(),
        )
    } else {
        None
    };
    fn absolute(path: &str) -> Result<String, String> {
        if path.trim().is_empty() || path.chars().any(char::is_control) {
            return Err("Invalid identity file path.".into());
        }
        let path = if let Some(rest) = path.strip_prefix("~/") {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .ok_or("Cannot expand identity path without a home directory.")?
                .join(rest)
        } else {
            PathBuf::from(path)
        };
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(path)
        };
        path.into_os_string()
            .into_string()
            .map_err(|_| "Identity path is not valid UTF-8.".into())
    }
    if let Some(identity) = &mut identity {
        if let Some(path) = &identity.key_path {
            identity.key_path = Some(absolute(path)?);
        }
        if let Some(path) = &identity.certificate_path {
            identity.certificate_path = Some(absolute(path)?);
        }
        match &mut identity.auth_method {
            shellumi_core::AuthMethod::PublicKey { key_name }
                if identity.key_path.is_none()
                    && (key_name.contains('/') || key_name.contains('\\')) =>
            {
                identity.key_path = Some(absolute(key_name)?);
            }
            shellumi_core::AuthMethod::Certificate {
                cert_path,
                key_path,
            } => {
                *cert_path = absolute(cert_path)?;
                if let Some(path) = key_path {
                    *path = absolute(path)?;
                }
            }
            _ => {}
        }
    }
    let destination = shellumi_core::resolve_effective_destination_with_identity(
        host.destination.host(),
        Some(host.destination.username()),
        Some(host.destination.port().get()),
        group.as_ref(),
        identity.as_ref(),
    )
    .map_err(|e| e.to_string())?;
    shellumi_core::connection::ConnectionPlan::new(
        destination,
        identity.as_ref(),
        host.jump_host.as_deref(),
    )
}

#[cfg(test)]
mod connection_tests {
    use super::*;
    use shellumi_core::{AuthMethod, HostGroup, NewIdentity, SshDestination};

    #[tokio::test]
    async fn connection_resolution_reopens_database_and_applies_host_over_group_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("connections.db");
        let path = path.to_str().unwrap();
        let mut store = HostStore::open(path).await.unwrap();
        let group_identity = store
            .create_identity(&NewIdentity::new("Group", "group-user", AuthMethod::Agent).unwrap())
            .await
            .unwrap();
        let mut identity = NewIdentity::new(
            "Host",
            "host-user",
            AuthMethod::PublicKey {
                key_name: "description".into(),
            },
        )
        .unwrap();
        identity.key_path = Some("keys/private key".into());
        identity.passphrase_secret_ref = Some("env://SHELLUMI_TEST_PASSPHRASE".into());
        let host_identity = store.create_identity(&identity).await.unwrap();
        store
            .add_group(
                &HostGroup::new("Fleet", None, None, &[])
                    .unwrap()
                    .with_identity_id(Some(group_identity)),
            )
            .await
            .unwrap();
        let dest = SshDestination::new("2001:db8::1", "original", 2200).unwrap();
        let inherited = store
            .add_full(
                "Inherited",
                &dest,
                Some("Fleet"),
                &[],
                Some("bastion.test"),
                None,
                "ssh",
                None,
                None,
            )
            .await
            .unwrap();
        let explicit = store
            .add_full(
                "Explicit",
                &dest,
                Some("Fleet"),
                &[],
                None,
                None,
                "ssh",
                None,
                Some(host_identity),
            )
            .await
            .unwrap();
        drop(store);
        let inherited_plan = resolve_connection(path, inherited).await.unwrap();
        assert_eq!(inherited_plan.destination.username(), "group-user");
        assert!(inherited_plan
            .ssh_arguments
            .contains(&"bastion.test".into()));
        assert!(inherited_plan
            .sftp_arguments
            .contains(&"group-user@[2001:db8::1]".into()));
        let plan = resolve_connection(path, explicit).await.unwrap();
        assert_eq!(plan.destination.username(), "host-user");
        assert_eq!(plan.destination.port().get(), 2200);
        assert!(Path::new(plan.identity_file.as_ref().unwrap()).is_absolute());
        assert!(plan
            .identity_file
            .as_ref()
            .unwrap()
            .ends_with("keys/private key"));
        assert!(plan.uses_askpass());
        assert!(plan
            .ssh_arguments
            .contains(&"StrictHostKeyChecking=yes".into()));
        assert!(!plan.ssh_arguments.iter().any(|arg| arg.contains("env://")));
        // Refresh returns identities, so changes made by the CLI can be selected without restart.
        assert_eq!(
            execute(path, Command::Refresh)
                .await
                .unwrap()
                .identities
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn missing_hosts_and_invalid_bindings_are_rejected_and_defaults_remain_available() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("connections.db");
        let path = path.to_str().unwrap();
        let mut store = HostStore::open(path).await.unwrap();
        let dest = SshDestination::new("example.test", "user", 22).unwrap();
        let normal = store
            .add_full("Default", &dest, None, &[], None, None, "ssh", None, None)
            .await
            .unwrap();
        let broken = store
            .add_full(
                "Missing",
                &dest,
                None,
                &[],
                None,
                None,
                "ssh",
                None,
                Some(999),
            )
            .await;
        assert!(
            broken.is_err(),
            "Foreign keys must reject a nonexistent identity binding"
        );
        let telnet = store
            .add_full("Telnet", &dest, None, &[], None, None, "telnet", None, None)
            .await
            .unwrap();
        drop(store);
        assert!(resolve_connection(path, -1).await.is_err());
        assert!(resolve_connection(path, telnet)
            .await
            .unwrap_err()
            .contains("no longer an SSH host"));
        let plan = resolve_connection(path, normal).await.unwrap();
        assert_eq!(plan.ssh_arguments, dest.openssh_arguments());
        assert!(!plan.uses_askpass());
    }
}

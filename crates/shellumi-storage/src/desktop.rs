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
    execute_scoped(database, None, command).await
}

pub async fn execute_scoped(
    database: &str,
    vault_id: Option<&str>,
    command: Command,
) -> Result<Snapshot, String> {
    execute_scoped_with_key(database, vault_id, command, None).await
}

pub async fn execute_scoped_with_key(
    database: &str,
    vault_id: Option<&str>,
    command: Command,
    key: Option<&shellumi_core::VaultKey>,
) -> Result<Snapshot, String> {
    let resolved_key = if let Some(k) = key {
        Some(k.clone())
    } else if !crate::is_protected_vault_path(database) {
        None
    } else if let Some(vid) = vault_id {
        if let Ok(v) = shellumi_core::VaultId::new(vid) {
            let cred_store = shellumi_core::default_credential_store();
            shellumi_core::VaultKeyManager::retrieve_key(&*cred_store, &v)
                .ok()
                .flatten()
        } else {
            None
        }
    } else {
        let cred_store = shellumi_core::default_credential_store();
        if let Ok(v) = shellumi_core::VaultId::new(crate::LEGACY_LOCAL_VAULT_ID) {
            shellumi_core::VaultKeyManager::retrieve_key(&*cred_store, &v)
                .ok()
                .flatten()
        } else {
            None
        }
    };

    let mut store = HostStore::open_with_key(database, resolved_key.as_ref())
        .await
        .map_err(|e| e.to_string())?;
    let vid_str = if let Some(vid) = vault_id {
        if let Ok(v) = shellumi_core::VaultId::new(vid) {
            store.set_vault_id(v);
            Some(vid.to_string())
        } else {
            None
        }
    } else {
        None
    };

    let is_remote = if let Some(ref vid) = vid_str {
        if let Ok(Some(ctx)) = store.get_vault_context(vid).await {
            matches!(ctx.location(), shellumi_core::VaultLocation::Remote { .. })
        } else {
            false
        }
    } else {
        false
    };
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
            if is_remote {
                if let Some(ref vid) = vid_str {
                    let payload = crate::SyncSnippetPayload {
                        name: snippet.name.clone(),
                        command: snippet.command.clone(),
                        description: snippet.description.clone(),
                        tags: snippet.tags.clone(),
                    };
                    let json = serde_json::to_string(&payload).ok();
                    let _ = store
                        .record_pending_mutation(
                            vid,
                            &snippet.name,
                            "snippet",
                            0,
                            false,
                            json.as_deref(),
                        )
                        .await;
                }
            }
        }
        Command::DeleteSnippet(id) => {
            let snippet_name = store.get_snippet(id).await.ok().flatten().map(|s| s.name);
            let deleted = store.delete_snippet(id).await.map_err(|e| e.to_string())?;
            if is_remote && deleted {
                if let (Some(ref vid), Some(sname)) = (&vid_str, snippet_name) {
                    let _ = store
                        .record_pending_mutation(vid, &sname, "snippet", 0, true, None)
                        .await;
                }
            }
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
            if is_remote {
                if let Some(ref vid) = vid_str {
                    let payload = crate::SyncWorkspacePayload {
                        name: name.clone(),
                        description: if description.is_empty() {
                            None
                        } else {
                            Some(description.clone())
                        },
                        layout: layout.clone(),
                        sessions: sessions.clone(),
                        tags: vec![],
                    };
                    let json = serde_json::to_string(&payload).ok();
                    let _ = store
                        .record_pending_mutation(vid, &name, "workspace", 0, false, json.as_deref())
                        .await;
                }
            }
        }
        Command::DeleteWorkspace(id) => {
            let ws_name = store.get_workspace(id).await.ok().flatten().map(|w| w.name);
            let deleted = store
                .delete_workspace(id)
                .await
                .map_err(|e| e.to_string())?;
            if is_remote && deleted {
                if let (Some(ref vid), Some(wname)) = (&vid_str, ws_name) {
                    let _ = store
                        .record_pending_mutation(vid, &wname, "workspace", 0, true, None)
                        .await;
                }
            }
        }
        Command::DuplicateWorkspace { id, name } => {
            store
                .duplicate_workspace(id, &name)
                .await
                .map_err(|e| e.to_string())?;
            if is_remote {
                if let Some(ref vid) = vid_str {
                    if let Ok(Some(ws)) = store.get_workspace_by_name(&name).await {
                        let payload = crate::SyncWorkspacePayload {
                            name: ws.name.clone(),
                            description: ws.description.clone(),
                            layout: ws.layout.clone(),
                            sessions: ws.sessions.clone(),
                            tags: ws.tags.clone(),
                        };
                        let json = serde_json::to_string(&payload).ok();
                        let _ = store
                            .record_pending_mutation(
                                vid,
                                &name,
                                "workspace",
                                0,
                                false,
                                json.as_deref(),
                            )
                            .await;
                    }
                }
            }
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
            for snippet in &snippets {
                if let Err(error) = store.add_snippet(snippet).await {
                    let _ = store.connection.execute("ROLLBACK", ()).await;
                    return Err(error.to_string());
                }
            }
            store
                .connection
                .execute("COMMIT", ())
                .await
                .map_err(|e| e.to_string())?;
            if is_remote {
                if let Some(ref vid) = vid_str {
                    for snippet in snippets {
                        let payload = crate::SyncSnippetPayload {
                            name: snippet.name.clone(),
                            command: snippet.command.clone(),
                            description: snippet.description.clone(),
                            tags: snippet.tags.clone(),
                        };
                        let json = serde_json::to_string(&payload).ok();
                        let _ = store
                            .record_pending_mutation(
                                vid,
                                &snippet.name,
                                "snippet",
                                0,
                                false,
                                json.as_deref(),
                            )
                            .await;
                    }
                }
            }
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
            let output = shellumi_core::process::background_command("ssh-keygen")
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
    resolve_connection_with_key(database, host_id, None, None).await
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedAuthSource {
    SavedIdentity(i64),
    HostSpecific,
    InheritedGroup(i64),
    AskOnConnect,
    NoAuthentication,
}

#[derive(Clone, Debug)]
pub struct ResolvedProtocolAuth {
    pub source: ResolvedAuthSource,
    pub protocol: shellumi_core::ConnectionProtocol,
    pub identity: Option<shellumi_core::Identity>,
    pub group: Option<shellumi_core::HostGroup>,
}

/// Shared typed authentication resolver used across SSH, SFTP, Mosh, RDP, VNC, Telnet, Serial, and Local.
pub async fn resolve_protocol_auth(
    store: &HostStore,
    host: &crate::SavedHost,
    cred_store: &dyn shellumi_core::CredentialStore,
) -> Result<ResolvedProtocolAuth, String> {
    let proto = host.protocol();
    if host.auth_dependency_unresolved {
        return Err(format!(
            "Host '{}' has an unresolved synchronized identity dependency ({}). Sync or assign an identity before connecting.",
            host.name,
            host.unresolved_identity_name
                .as_deref()
                .unwrap_or("pending")
        ));
    }
    let caps = shellumi_core::ProtocolCapabilities::for_protocol(proto);
    let group = if let Some(name) = &host.group_name {
        store.get_group(name).await.map_err(|e| e.to_string())?
    } else {
        None
    };

    let cfg_map = host
        .protocol_config
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(s).ok())
        .unwrap_or_default();
    let cfg_str = |k: &str| {
        cfg_map
            .get(k)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
    };
    let auth_mode = cfg_str("auth_mode");
    let host_secret_ref = cfg_str("host_secret_ref");
    let host_key_path = cfg_str("host_key_path");
    let host_passphrase_ref = cfg_str("host_passphrase_ref");
    let vault_store = shellumi_core::VaultCredentialStore::new(cred_store, &store.host_vault_id);

    let validate_identity = |ident: &shellumi_core::Identity| -> Result<(), String> {
        if !caps.is_auth_method_compatible(&ident.auth_method) {
            return Err(format!(
                "Identity '{}' ({}) is not compatible with {}.",
                ident.name, ident.auth_method, proto
            ));
        }
        for r in [&ident.password_secret_ref, &ident.passphrase_secret_ref]
            .into_iter()
            .flatten()
        {
            if r.starts_with("keychain://") {
                vault_store
                    .validate_reference(r)
                    .map_err(|e| format!("Invalid identity credential reference: {e}"))?;
            }
        }
        Ok(())
    };

    if let Some(id) = host.identity_id {
        let ident = store
            .get_identity(id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Selected identity no longer exists.")?
            .to_identity();
        validate_identity(&ident)?;
        return Ok(ResolvedProtocolAuth {
            source: ResolvedAuthSource::SavedIdentity(id),
            protocol: proto,
            identity: Some(ident),
            group,
        });
    }

    if host_secret_ref.is_some() || host_key_path.is_some() || host_passphrase_ref.is_some() {
        for r in [&host_secret_ref, &host_passphrase_ref]
            .into_iter()
            .flatten()
        {
            vault_store
                .validate_reference(r)
                .map_err(|e| format!("Invalid host credential reference: {e}"))?;
        }
        let auth_method = if host_key_path.is_some() {
            shellumi_core::AuthMethod::PublicKey {
                key_name: host.name.clone(),
            }
        } else {
            shellumi_core::AuthMethod::Password
        };
        if !caps.is_auth_method_compatible(&auth_method) {
            return Err(format!(
                "Host-specific {} credential is not compatible with {}.",
                auth_method, proto
            ));
        }
        return Ok(ResolvedProtocolAuth {
            source: ResolvedAuthSource::HostSpecific,
            protocol: proto,
            identity: Some(shellumi_core::Identity {
                id: 0,
                name: format!("{} (host-specific)", host.name),
                username: host.destination.username().to_string(),
                auth_method,
                password_secret_ref: host_secret_ref,
                key_path: host_key_path,
                passphrase_secret_ref: host_passphrase_ref,
                certificate_path: None,
                tags: Vec::new(),
                comment: None,
                created_at: None,
                updated_at: None,
            }),
            group,
        });
    }

    if auth_mode.as_deref() == Some("ask") {
        // Explicit Ask on Connect must never silently fall back to group inheritance.
        return Ok(ResolvedProtocolAuth {
            source: ResolvedAuthSource::AskOnConnect,
            protocol: proto,
            identity: None,
            group,
        });
    }

    if auth_mode.as_deref() == Some("no_auth") {
        if !caps.allows_no_auth {
            return Err(format!("{proto} does not allow No Authentication."));
        }
        return Ok(ResolvedProtocolAuth {
            source: ResolvedAuthSource::NoAuthentication,
            protocol: proto,
            identity: None,
            group,
        });
    }

    if cfg_map
        .get("local_credentials_required")
        .and_then(|v| v.as_bool())
        == Some(true)
        && host_secret_ref.is_none()
        && host_key_path.is_none()
        && host_passphrase_ref.is_none()
    {
        return Err(format!(
            "Device-local credentials are required for synchronized host '{}'. Open Host Settings to configure credentials on this device.",
            host.name
        ));
    }

    if let Some(ref g) = group {
        if store
            .is_group_auth_dependency_unresolved(&g.name)
            .await
            .map_err(|e| e.to_string())?
        {
            return Err(format!(
                "Group '{}' has an unresolved synchronized identity dependency. Sync or assign an identity before connecting.",
                g.name
            ));
        }
    }

    if let Some(group_identity_id) = group.as_ref().and_then(|g| g.identity_id) {
        if caps.allows_saved_identity {
            let ident = store
                .get_identity(group_identity_id)
                .await
                .map_err(|e| e.to_string())?
                .ok_or("Group identity no longer exists.")?
                .to_identity();
            validate_identity(&ident)?;
            return Ok(ResolvedProtocolAuth {
                source: ResolvedAuthSource::InheritedGroup(group_identity_id),
                protocol: proto,
                identity: Some(ident),
                group,
            });
        }
    }

    Ok(ResolvedProtocolAuth {
        source: if !caps.allows_ask_on_connect && caps.allows_no_auth {
            ResolvedAuthSource::NoAuthentication
        } else {
            ResolvedAuthSource::AskOnConnect
        },
        protocol: proto,
        identity: None,
        group,
    })
}

/// Resolve saved/inherited RDP or VNC credentials through the unified protocol auth resolver.
pub async fn resolve_remote_desktop_auth_with_key(
    database: &str,
    host_id: i64,
    active_vault_id: Option<&str>,
    vault_key: Option<&shellumi_core::VaultKey>,
) -> Result<(Option<String>, Option<String>, ResolvedAuthSource), String> {
    let resolved_key = if let Some(k) = vault_key {
        Some(k.clone())
    } else if let Some(vid) = active_vault_id {
        if let Ok(v) = shellumi_core::VaultId::new(vid) {
            let cred_store = shellumi_core::default_credential_store();
            shellumi_core::VaultKeyManager::retrieve_key(&*cred_store, &v)
                .ok()
                .flatten()
        } else {
            None
        }
    } else {
        None
    };

    let mut store = HostStore::open_with_key(database, resolved_key.as_ref())
        .await
        .map_err(|e| e.to_string())?;
    if let Some(vid) = active_vault_id {
        if let Ok(v) = shellumi_core::VaultId::new(vid) {
            store.set_vault_id(v);
        }
    }
    let host = store
        .get_host(host_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Host no longer exists.")?;
    let proto = host.protocol();
    if proto != shellumi_core::ConnectionProtocol::Rdp
        && proto != shellumi_core::ConnectionProtocol::Vnc
    {
        return Err("The selected host is not a remote desktop host.".into());
    }
    let cred_store = shellumi_core::default_credential_store();
    let resolved = resolve_protocol_auth(&store, &host, cred_store.as_ref()).await?;
    let vault_store = shellumi_core::VaultCredentialStore::new(&*cred_store, &store.host_vault_id);
    if let Some(ident) = resolved.identity {
        let uname = if ident.username.is_empty() || ident.username == "-" {
            None
        } else {
            Some(ident.username.clone())
        };
        let pwd = match ident.password_secret_ref.as_deref() {
            Some(r) => vault_store
                .resolve(r)
                .map_err(|e| e.to_string())?
                .map(|s| s.expose_secret().to_string()),
            None => None,
        };
        Ok((uname, pwd, resolved.source))
    } else {
        Ok((None, None, resolved.source))
    }
}

pub async fn resolve_connection_with_key(
    database: &str,
    host_id: i64,
    active_vault_id: Option<&str>,
    key: Option<&shellumi_core::VaultKey>,
) -> Result<shellumi_core::connection::ConnectionPlan, String> {
    let resolved_key = if let Some(k) = key {
        Some(k.clone())
    } else if !crate::is_protected_vault_path(database) {
        None
    } else if let Some(vid) = active_vault_id {
        if let Ok(v) = shellumi_core::VaultId::new(vid) {
            let cred_store = shellumi_core::default_credential_store();
            shellumi_core::VaultKeyManager::retrieve_key(&*cred_store, &v)
                .ok()
                .flatten()
        } else {
            None
        }
    } else {
        let cred_store = shellumi_core::default_credential_store();
        if let Ok(v) = shellumi_core::VaultId::new(crate::LEGACY_LOCAL_VAULT_ID) {
            shellumi_core::VaultKeyManager::retrieve_key(&*cred_store, &v)
                .ok()
                .flatten()
        } else {
            None
        }
    };

    let mut store = HostStore::open_with_key(database, resolved_key.as_ref())
        .await
        .map_err(|e| e.to_string())?;
    if let Some(vid) = active_vault_id {
        if let Ok(v) = shellumi_core::VaultId::new(vid) {
            store.set_vault_id(v);
        }
    }
    let host = store
        .get_host(host_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Host no longer exists.")?;
    let proto = host.protocol();
    if proto != shellumi_core::ConnectionProtocol::Ssh
        && proto != shellumi_core::ConnectionProtocol::Mosh
    {
        return Err("The selected host does not use SSH authentication.".into());
    }
    let cred_store = shellumi_core::default_credential_store();
    let resolved = resolve_protocol_auth(&store, &host, cred_store.as_ref()).await?;
    let group = resolved.group;
    let mut identity = resolved.identity;
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
            .contains("does not use SSH authentication"));
        let plan = resolve_connection(path, normal).await.unwrap();
        assert_eq!(plan.ssh_arguments, dest.openssh_arguments());
        assert!(!plan.uses_askpass());
    }

    #[tokio::test]
    async fn resolves_host_specific_credentials_mosh_and_explicit_ask_without_group_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("host_auth.db");
        let path_str = path.to_str().unwrap();
        let mut store = HostStore::open(path_str).await.unwrap();
        let vid = shellumi_core::VaultId::new(crate::LEGACY_LOCAL_VAULT_ID).unwrap();
        store.set_vault_id(vid.clone());

        let cred_store = shellumi_core::EphemeralCredentialStore::new();
        let vault_store = shellumi_core::VaultCredentialStore::new(&cred_store, &vid);
        let group_pw_ref = vault_store
            .store_unique("identity-password", "group-pw")
            .unwrap();
        let group_ident_id = store
            .create_identity(&NewIdentity {
                name: "Group Ident".into(),
                username: "groupuser".into(),
                auth_method: AuthMethod::Password,
                password_secret_ref: Some(group_pw_ref),
                key_path: None,
                passphrase_secret_ref: None,
                certificate_path: None,
                tags: vec![],
                comment: None,
            })
            .await
            .unwrap();
        store
            .add_group(&HostGroup {
                name: "Prod".into(),
                default_username: Some("groupuser".into()),
                default_port: Some(std::num::NonZeroU16::new(22).unwrap()),
                identity_id: Some(group_ident_id),
                tags: vec![],
            })
            .await
            .unwrap();

        let dest = SshDestination::new("app.example.test", "deploy", 2222).unwrap();

        // 1. Explicit Ask on Connect must NOT inherit group identity
        let ask_host_id = store
            .add_full(
                "AskHost",
                &dest,
                Some("Prod"),
                &[],
                None,
                None,
                "ssh",
                Some(r#"{"auth_mode":"ask"}"#),
                None,
            )
            .await
            .unwrap();
        let ask_host = store.get_host(ask_host_id).await.unwrap().unwrap();
        let ask_resolved = resolve_protocol_auth(&store, &ask_host, &cred_store)
            .await
            .unwrap();
        assert_eq!(ask_resolved.source, ResolvedAuthSource::AskOnConnect);
        assert!(ask_resolved.identity.is_none());

        // 2. Host-specific SSH key + passphrase in protocol_config
        let key_file = dir.path().join("id with spaces");
        std::fs::write(&key_file, "dummy").unwrap();
        let pass_ref = vault_store.store_unique("host-passphrase", "kpw").unwrap();
        let host_cfg = serde_json::json!({
            "host_key_path": key_file.to_str().unwrap(),
            "host_passphrase_ref": pass_ref,
        })
        .to_string();
        let mosh_host_id = store
            .add_full(
                "MoshHost",
                &dest,
                None,
                &[],
                None,
                None,
                "mosh",
                Some(&host_cfg),
                None,
            )
            .await
            .unwrap();
        let mosh_host = store.get_host(mosh_host_id).await.unwrap().unwrap();
        let mosh_resolved = resolve_protocol_auth(&store, &mosh_host, &cred_store)
            .await
            .unwrap();
        assert_eq!(mosh_resolved.source, ResolvedAuthSource::HostSpecific);
        let mosh_ident = mosh_resolved.identity.unwrap();
        assert_eq!(mosh_ident.key_path.as_deref(), key_file.to_str());
        assert_eq!(mosh_ident.passphrase_secret_ref, Some(pass_ref));

        // 3. Cross-vault host_secret_ref must be rejected
        let other_vid = shellumi_core::VaultId::new("other-vault").unwrap();
        let other_store = shellumi_core::VaultCredentialStore::new(&cred_store, &other_vid);
        let foreign_ref = other_store.store_unique("host-password", "x").unwrap();
        let bad_cfg = serde_json::json!({ "host_secret_ref": foreign_ref }).to_string();
        let bad_host_id = store
            .add_full(
                "ForeignRefHost",
                &dest,
                None,
                &[],
                None,
                None,
                "ssh",
                Some(&bad_cfg),
                None,
            )
            .await
            .unwrap();
        let bad_host = store.get_host(bad_host_id).await.unwrap().unwrap();
        assert!(resolve_protocol_auth(&store, &bad_host, &cred_store)
            .await
            .is_err());

        // 4. RDP group identity inheritance + explicit Ask/NoAuth blocking fallback
        let rdp_group_pass_ref = vault_store
            .store_unique("identity-password", "rdp-secret")
            .unwrap();
        let rdp_ident_id = store
            .create_identity(&shellumi_core::NewIdentity {
                name: "RDP Corp".into(),
                username: "corpuser".into(),
                auth_method: shellumi_core::AuthMethod::Password,
                password_secret_ref: Some(rdp_group_pass_ref),
                key_path: None,
                passphrase_secret_ref: None,
                certificate_path: None,
                tags: Vec::new(),
                comment: None,
            })
            .await
            .unwrap();
        store
            .add_group(&shellumi_core::HostGroup {
                name: "WindowsFleet".into(),
                default_username: Some("corpuser".into()),
                default_port: Some(std::num::NonZeroU16::new(3389).unwrap()),
                identity_id: Some(rdp_ident_id),
                tags: Vec::new(),
            })
            .await
            .unwrap();
        let rdp_dest = SshDestination::new("10.0.0.88", "corpuser", 3389).unwrap();
        let rdp_inherited_id = store
            .add_full(
                "RDP Inherited",
                &rdp_dest,
                Some("WindowsFleet"),
                &[],
                None,
                None,
                "rdp",
                None,
                None,
            )
            .await
            .unwrap();
        let rdp_inherited = store.get_host(rdp_inherited_id).await.unwrap().unwrap();
        let rdp_resolved = resolve_protocol_auth(&store, &rdp_inherited, &cred_store)
            .await
            .unwrap();
        assert_eq!(
            rdp_resolved.source,
            ResolvedAuthSource::InheritedGroup(rdp_ident_id)
        );
        let rdp_ident = rdp_resolved.identity.unwrap();
        assert_eq!(rdp_ident.username, "corpuser");
        let rdp_pwd = vault_store
            .resolve(rdp_ident.password_secret_ref.as_deref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(rdp_pwd.expose_secret(), "rdp-secret");

        // Explicit Ask on Connect blocks group fallback for RDP
        let rdp_ask_id = store
            .add_full(
                "RDP Ask",
                &rdp_dest,
                Some("WindowsFleet"),
                &[],
                None,
                None,
                "rdp",
                Some(r#"{"auth_mode":"ask"}"#),
                None,
            )
            .await
            .unwrap();
        let rdp_ask = store.get_host(rdp_ask_id).await.unwrap().unwrap();
        let rdp_ask_resolved = resolve_protocol_auth(&store, &rdp_ask, &cred_store)
            .await
            .unwrap();
        assert_eq!(rdp_ask_resolved.source, ResolvedAuthSource::AskOnConnect);
        assert!(rdp_ask_resolved.identity.is_none());
    }
}

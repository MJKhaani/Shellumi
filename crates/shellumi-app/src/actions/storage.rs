//! Storage actions.
use super::*;

impl Shellumi {
    /// Returns the database path for the vault registry (always default.db).
    pub(crate) fn registry_db_path(&self) -> Option<String> {
        let base = self.database.as_deref()?;
        let path = data_path::resolve_vault_db_file(
            std::path::Path::new(base),
            shellumi_storage::LEGACY_LOCAL_VAULT_ID,
        );
        path.to_str().map(str::to_owned)
    }

    /// Returns the database path for the active vault.
    pub(crate) fn active_vault_db_path(&self) -> Option<String> {
        let base = self.database.as_deref()?;
        let path =
            data_path::resolve_vault_db_file(std::path::Path::new(base), &self.active_vault_id);
        path.to_str().map(str::to_owned)
    }

    /// Returns the database path for a given vault ID.
    #[allow(dead_code)]
    pub(crate) fn vault_db_path(&self, vault_id: &str) -> Option<String> {
        let base = self.database.as_deref()?;
        let path = data_path::resolve_vault_db_file(std::path::Path::new(base), vault_id);
        path.to_str().map(str::to_owned)
    }

    /// Returns the active vault's DEK from memory or the OS Keychain.
    pub(crate) fn active_vault_key(&self) -> Option<shellumi_core::VaultKey> {
        self.vault_key_for(&self.active_vault_id)
    }

    /// Returns the DEK for the specified vault from memory or OS credential storage.
    pub(crate) fn vault_key_for(&self, vault_id: &str) -> Option<shellumi_core::VaultKey> {
        if let Some(key) = self.active_vault_keys.get(vault_id) {
            return Some(key.clone());
        }
        if let Ok(vid) = shellumi_core::VaultId::new(vault_id) {
            let cred_store = shellumi_core::default_credential_store();
            shellumi_core::VaultKeyManager::retrieve_key(&*cred_store, &vid)
                .ok()
                .flatten()
        } else {
            None
        }
    }

    pub(crate) fn start_storage(&mut self, create_parent: bool, cx: &mut Context<Self>) {
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let Some(base_db) = self.database.clone() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let in_memory_key = self.active_vault_key();
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        self.hosts = HostState::Loading;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let base_path = std::path::Path::new(&base_db);
            let base_dir = if base_path.is_dir() {
                base_path
            } else if let Some(parent) = base_path.parent() {
                if parent.file_name() == Some(std::ffi::OsStr::new("vaults")) {
                    parent.parent().unwrap_or(parent)
                } else {
                    parent
                }
            } else {
                std::path::Path::new(".")
            };

            // Run migration from legacy shellumi.db if needed
            let cred_store = shellumi_core::default_credential_store();
            if shellumi_storage::MigrationEngine::is_migration_needed(base_dir) {
                if default_key.is_none() {
                    return Err(
                        "MigrationRequired: Legacy data detected. Migration requires vault credentials.".to_string(),
                    );
                }
                shellumi_storage::MigrationEngine::migrate_legacy_database_if_needed(
                    base_dir,
                    default_key.as_ref(),
                    &*cred_store,
                )
                .await
                .map_err(|e| format!("MigrationRequired: {e}"))?;
            }

            if create_parent {
                data_path::prepare_parent(std::path::Path::new(&path))?;
            }
            let key = match in_memory_key {
                Some(k) => Some(k),
                None => {
                    if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id) {
                        shellumi_core::VaultKeyManager::retrieve_key(&*cred_store, &vid)
                            .ok()
                            .flatten()
                    } else {
                        None
                    }
                }
            };
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, key.as_ref())
                    .await
                    .map_err(|error| error.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id) {
                    store.set_vault_id(vid);
                }
                let hosts = store.list().await.map_err(|error| error.to_string())?;
                let snippets = store
                    .list_snippets()
                    .await
                    .map_err(|error| error.to_string())?;
                let workspaces = store
                    .list_workspaces()
                    .await
                    .map_err(|error| error.to_string())?;
                let identities = store
                    .list_identities()
                    .await
                    .map_err(|error| error.to_string())?;
                let tunnels = store.list_tunnels().await.unwrap_or_default();
                Ok::<_, String>((hosts, snippets, workspaces, identities, tunnels, key))
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok((hosts, snippets, workspaces, identities, tunnels, key)) => {
                        if let Some(k) = key {
                            app.active_vault_keys.insert(app.active_vault_id.clone(), k);
                        }
                        app.hosts = HostState::Loaded(hosts);
                        app.snippets = snippets;
                        app.workspaces = workspaces;
                        app.identities = identities;
                        app.tunnels = tunnels;
                        app.desktop_command(shellumi_storage::desktop::Command::Refresh, cx);
                        app.load_vault_list(cx);
                    }
                    Err(error) => {
                        if error.starts_with("MigrationRequired:") {
                            app.hosts = HostState::MigrationRequired(error.clone());
                        } else {
                            app.hosts = HostState::Error(error.clone());
                            if error.contains("encryption")
                                || error.contains("key")
                                || error.contains("locked")
                            {
                                let vid = app.active_vault_id.clone();
                                let vname = app.active_vault.clone();
                                app.open_unlock_vault_modal(vid, vname, cx);
                            }
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn load_identities(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let in_memory_key = self.active_vault_key();
        let task = cx.background_executor().spawn(async move {
            let key = match in_memory_key {
                Some(k) => Some(k),
                None => {
                    if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id) {
                        let cred_store = shellumi_core::default_credential_store();
                        shellumi_core::VaultKeyManager::retrieve_key(&*cred_store, &vid)
                            .ok()
                            .flatten()
                    } else {
                        None
                    }
                }
            };
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id) {
                    store.set_vault_id(vid);
                }
                // Auto-discover existing OpenSSH keys from ~/.ssh and ensure they exist as saved identities
                if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
                    let ssh_dir = home.join(".ssh");
                    if let Ok(entries) = std::fs::read_dir(&ssh_dir) {
                        let current_user =
                            std::env::var("USER").unwrap_or_else(|_| "root".to_string());
                        let existing = store.list_identities().await.unwrap_or_default();
                        for entry in entries.flatten() {
                            let p = entry.path();
                            if p.extension().and_then(|s| s.to_str()) == Some("pub") {
                                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                                    let priv_path = ssh_dir.join(stem);
                                    let priv_str = priv_path.to_string_lossy().to_string();
                                    let already_exists = existing.iter().any(|id| {
                                        id.name == stem || id.key_path.as_deref() == Some(&priv_str)
                                    });
                                    if !already_exists && priv_path.exists() {
                                        if let Ok(mut new_id) = shellumi_core::NewIdentity::new(
                                            stem,
                                            &current_user,
                                            shellumi_core::AuthMethod::PublicKey {
                                                key_name: stem.to_string(),
                                            },
                                        ) {
                                            new_id.key_path = Some(priv_str);
                                            new_id.comment =
                                                Some("Auto-discovered from ~/.ssh".to_string());
                                            let _ = store.create_identity(&new_id).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                store.list_identities().await.map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Ok(ids) = res {
                    app.identities = ids;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

//! Storage actions.
use super::*;

impl Shellumi {
    pub(crate) fn start_storage(&mut self, create_parent: bool, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        self.hosts = HostState::Loading;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            if create_parent {
                data_path::prepare_parent(std::path::Path::new(&path))?;
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path)
                    .await
                    .map_err(|error| error.to_string())?;
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
                Ok::<_, String>((hosts, snippets, workspaces, identities))
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok((hosts, snippets, workspaces, identities)) => {
                        app.hosts = HostState::Loaded(hosts);
                        app.snippets = snippets;
                        app.workspaces = workspaces;
                        app.identities = identities;
                        app.desktop_command(shellumi_storage::desktop::Command::Refresh, cx);
                    }
                    Err(error) => app.hosts = HostState::Error(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn load_identities(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
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

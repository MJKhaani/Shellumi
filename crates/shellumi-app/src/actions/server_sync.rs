//! Server synchronization actions and vault management for Shellumi Server v1.
use super::*;
use shellumi_core::server::ShellumiServerClient;
use shellumi_storage::ServerSyncEngine;

/// Summary metadata for a registered vault context in the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VaultContextSummary {
    pub id: String,
    pub name: String,
    pub is_remote: bool,
    pub server_url: Option<String>,
    pub server_vault_id: Option<String>,
}

impl Shellumi {
    /// Returns true if the active vault is bound to a remote Shellumi Server.
    pub(crate) fn is_active_vault_remote(&self) -> bool {
        self.active_vault_id != shellumi_storage::LEGACY_LOCAL_VAULT_ID
            && !self.active_vault_id.is_empty()
    }

    /// Loads the list of available vaults from storage and refreshes sync status.
    pub(crate) fn load_vault_list(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let contexts = store
                    .list_vault_contexts()
                    .await
                    .map_err(|e| e.to_string())?;
                let conns = store
                    .list_vault_server_connections()
                    .await
                    .map_err(|e| e.to_string())?;

                let mut summaries = Vec::new();
                summaries.push(VaultContextSummary {
                    id: shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(),
                    name: "Personal".to_string(),
                    is_remote: false,
                    server_url: None,
                    server_vault_id: None,
                });

                for (name, ctx) in contexts {
                    let id = ctx.id().as_str().to_string();
                    if id == shellumi_storage::LEGACY_LOCAL_VAULT_ID {
                        continue;
                    }
                    match ctx.location() {
                        shellumi_core::VaultLocation::Local => {
                            summaries.push(VaultContextSummary {
                                id,
                                name,
                                is_remote: false,
                                server_url: None,
                                server_vault_id: None,
                            });
                        }
                        shellumi_core::VaultLocation::Remote {
                            server,
                            server_vault_id,
                        } => {
                            let srv_id = server.as_str();
                            let srv_url = conns
                                .iter()
                                .find(|c| c.id == srv_id)
                                .map(|c| c.base_url.clone());
                            summaries.push(VaultContextSummary {
                                id,
                                name,
                                is_remote: true,
                                server_url: srv_url,
                                server_vault_id: Some(server_vault_id.as_str().to_string()),
                            });
                        }
                    }
                }

                Ok::<_, String>(summaries)
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Ok(summaries) = result {
                    app.available_vaults = summaries;
                    if !app.is_active_vault_remote() {
                        app.sync_status = AppSyncStatus::LocalOnly;
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Switches the active vault scope and reloads hosts and sync state.
    pub(crate) fn switch_vault(
        &mut self,
        vault_id: String,
        vault_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_vault_id = vault_id;
        self.active_vault = vault_name;
        self.vault_dropdown_open = false;
        self.navigate_page("Hosts", window, cx);

        if self.is_active_vault_remote() {
            self.sync_status = AppSyncStatus::Syncing;
            self.load_hosts(cx);
            self.sync_active_vault(cx);
        } else {
            self.sync_status = AppSyncStatus::LocalOnly;
            self.load_hosts(cx);
        }
        cx.notify();
    }

    /// Triggers a delta synchronization with the remote Shellumi Server for the active vault.
    pub(crate) fn sync_active_vault(&mut self, cx: &mut Context<Self>) {
        if !self.is_active_vault_remote() {
            self.sync_status = AppSyncStatus::LocalOnly;
            cx.notify();
            return;
        }

        if matches!(self.sync_status, AppSyncStatus::Syncing) {
            return;
        }

        let Some(path) = self.database.clone() else {
            self.sync_status = AppSyncStatus::Error("Database not open.".into());
            cx.notify();
            return;
        };

        self.sync_status = AppSyncStatus::Syncing;
        cx.notify();

        let active_vault_id = self.active_vault_id.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let contexts = store
                    .list_vault_contexts()
                    .await
                    .map_err(|e| e.to_string())?;
                let target_ctx = contexts
                    .into_iter()
                    .find(|(_, ctx)| ctx.id().as_str() == active_vault_id)
                    .ok_or_else(|| "Active vault context not found in storage".to_string())?;

                let (server_conn_id, remote_vault_id) = match target_ctx.1.location() {
                    shellumi_core::VaultLocation::Remote {
                        server,
                        server_vault_id,
                    } => (
                        server.as_str().to_string(),
                        server_vault_id.as_str().to_string(),
                    ),
                    shellumi_core::VaultLocation::Local => {
                        return Err(
                            "Cannot perform remote server sync on a local vault".to_string()
                        );
                    }
                };

                let conn_record = store
                    .get_vault_server_connection(&server_conn_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| {
                        format!("Server connection '{server_conn_id}' record not found")
                    })?;

                let token = conn_record.auth_token.clone().ok_or_else(|| {
                    "Server connection is not authenticated (missing access token)".to_string()
                })?;

                let mut client = ShellumiServerClient::new(&conn_record.base_url);
                client.set_token(token);
                if let Some(dev_id_str) = &conn_record.device_id {
                    if let Ok(dev_uuid) = uuid::Uuid::parse_str(dev_id_str) {
                        client.set_device_id(dev_uuid);
                    }
                }

                let remote_uuid = uuid::Uuid::parse_str(&remote_vault_id)
                    .map_err(|e| format!("Invalid remote vault UUID: {e}"))?;

                let passphrase = std::env::var("SHELLUMI_VAULT_PASSPHRASE")
                    .unwrap_or_else(|_| format!("passphrase-{remote_vault_id}"));

                let vault_id_obj = shellumi_core::VaultId::new(&active_vault_id)
                    .map_err(|e| format!("Invalid local vault ID: {e}"))?;

                let report = ServerSyncEngine::sync_vault(
                    &mut store,
                    &vault_id_obj,
                    remote_uuid,
                    &client,
                    &passphrase,
                )
                .await
                .map_err(|e| e.to_string())?;

                Ok::<_, String>(report)
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok(_report) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        app.sync_status = AppSyncStatus::UpToDate {
                            last_sync_time: now,
                        };
                        app.load_hosts(cx);
                    }
                    Err(error) => {
                        app.sync_status = AppSyncStatus::Error(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

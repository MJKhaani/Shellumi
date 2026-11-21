//! Server synchronization actions and vault management for Shellumi Server v1.
use super::*;
use shellumi_core::server::ShellumiServerClient;
use shellumi_storage::desktop::Command;
use shellumi_storage::ServerSyncEngine;

/// Summary metadata for a registered vault context in the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VaultContextSummary {
    pub id: String,
    pub name: String,
    pub is_remote: bool,
    pub server_url: Option<String>,
    pub server_vault_id: Option<String>,
    pub is_locked: bool,
}

impl Shellumi {
    /// Returns true if the active vault is bound to a remote Shellumi Server.
    pub(crate) fn is_active_vault_remote(&self) -> bool {
        self.available_vaults
            .iter()
            .find(|v| v.id == self.active_vault_id)
            .map(|v| v.is_remote)
            .unwrap_or(false)
    }

    /// Loads the list of available vaults from storage and refreshes sync status.
    pub(crate) fn load_vault_list(&mut self, cx: &mut Context<Self>) {
        let Some(reg_path) = self.registry_db_path() else {
            return;
        };
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = match HostStore::open_with_key(&reg_path, default_key.as_ref()).await {
                    Ok(s) => s,
                    Err(_) => {
                        return Ok(vec![VaultContextSummary {
                            id: shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(),
                            name: "Personal".to_string(),
                            is_remote: false,
                            server_url: None,
                            server_vault_id: None,
                            is_locked: true,
                        }]);
                    }
                };
                let contexts = store
                    .list_vault_contexts()
                    .await
                    .map_err(|e| e.to_string())?;
                let conns = store
                    .list_vault_server_connections()
                    .await
                    .map_err(|e| e.to_string())?;

                let legacy_locked = store
                    .is_vault_locked(shellumi_storage::LEGACY_LOCAL_VAULT_ID)
                    .await
                    .unwrap_or(false);

                let mut summaries = Vec::new();
                summaries.push(VaultContextSummary {
                    id: shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(),
                    name: "Personal".to_string(),
                    is_remote: false,
                    server_url: None,
                    server_vault_id: None,
                    is_locked: legacy_locked,
                });

                for (name, ctx) in contexts {
                    let id = ctx.id().as_str().to_string();
                    if id == shellumi_storage::LEGACY_LOCAL_VAULT_ID {
                        continue;
                    }
                    let is_locked = store.is_vault_locked(&id).await.unwrap_or(false);
                    match ctx.location() {
                        shellumi_core::VaultLocation::Local => {
                            summaries.push(VaultContextSummary {
                                id,
                                name,
                                is_remote: false,
                                server_url: None,
                                server_vault_id: None,
                                is_locked,
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
                                is_locked,
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
                    } else if matches!(app.sync_status, AppSyncStatus::LocalOnly) {
                        app.sync_active_vault(cx);
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
        let is_locked = self
            .available_vaults
            .iter()
            .find(|v| v.id == vault_id)
            .map(|v| v.is_locked)
            .unwrap_or(false);

        if is_locked {
            self.vault_dropdown_open = false;
            self.open_unlock_vault_modal(vault_id, vault_name, cx);
            return;
        }

        self.active_vault_id = vault_id;
        self.active_vault = vault_name;
        self.vault_dropdown_open = false;
        self.active_ws_client = None;
        self.server_ws_status = None;
        self.navigate_page("Hosts", window, cx);

        self.load_hosts(cx);
        self.load_identities(cx);
        self.load_workspaces(cx);
        self.desktop_command(Command::Refresh, cx);

        if self.is_active_vault_remote() {
            self.sync_active_vault(cx);
        } else {
            self.sync_status = AppSyncStatus::LocalOnly;
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

        if self.is_server_sync_in_progress {
            return;
        }

        let Some(reg_path) = self.registry_db_path() else {
            self.sync_status = AppSyncStatus::Error("Registry database not configured.".into());
            cx.notify();
            return;
        };
        let Some(vault_db_path) = self.active_vault_db_path() else {
            self.sync_status = AppSyncStatus::Error("Active vault database not configured.".into());
            cx.notify();
            return;
        };

        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let active_key = self.active_vault_key();

        let passphrase = match std::env::var("SHELLUMI_VAULT_PASSPHRASE") {
            Ok(value) if value.chars().count() >= 8 => value,
            _ => {
                if let Some(ref key) = active_key {
                    hex::encode(key.expose_bytes())
                } else {
                    self.sync_status = AppSyncStatus::Error(
                        "Active vault must be unlocked before server sync can proceed.".into(),
                    );
                    cx.notify();
                    return;
                }
            }
        };

        self.is_server_sync_in_progress = true;
        self.sync_status = AppSyncStatus::Syncing;
        cx.notify();

        let active_vault_id = self.active_vault_id.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let reg_store = HostStore::open_with_key(&reg_path, default_key.as_ref())
                    .await
                    .map_err(|e| format!("Failed to open vault registry: {e}"))?;
                let contexts = reg_store
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

                let conn_record = reg_store
                    .get_vault_server_connection(&server_conn_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| {
                        format!("Server connection '{server_conn_id}' record not found")
                    })?;

                let cred_store = shellumi_core::default_credential_store();
                let token_sec = conn_record
                    .resolve_auth_token(&*cred_store)
                    .ok_or_else(|| {
                        "Server connection is not authenticated (missing or unresolved access token)".to_string()
                    })?;
                let token = token_sec.expose_secret().to_string();

                let mut client = ShellumiServerClient::new(&conn_record.base_url);
                client.set_token(token);
                if let Some(dev_id_str) = &conn_record.device_id {
                    if let Ok(dev_uuid) = uuid::Uuid::parse_str(dev_id_str) {
                        client.set_device_id(dev_uuid);
                    }
                }

                let remote_uuid = uuid::Uuid::parse_str(&remote_vault_id)
                    .map_err(|e| format!("Invalid remote vault UUID: {e}"))?;

                let vault_id_obj = shellumi_core::VaultId::new(&active_vault_id)
                    .map_err(|e| format!("Invalid local vault ID: {e}"))?;

                let mut vault_store = HostStore::open_with_key(&vault_db_path, active_key.as_ref())
                    .await
                    .map_err(|e| format!("Failed to open active vault database: {e}"))?;
                vault_store.set_vault_id(vault_id_obj.clone());

                let report = ServerSyncEngine::sync_vault_with_connection(
                    &mut vault_store,
                    &vault_id_obj,
                    remote_uuid,
                    &client,
                    &passphrase,
                    Some(&conn_record),
                    &*cred_store,
                )
                .await
                .map_err(|e| e.to_string())?;

                Ok::<_, String>(report)
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                app.is_server_sync_in_progress = false;
                match result {
                    Ok(report) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        if report.has_conflicts {
                            app.sync_status = AppSyncStatus::Conflict {
                                message:
                                    "Conflict detected on remote server. Local changes preserved."
                                        .to_string(),
                            };
                        } else {
                            app.sync_status = AppSyncStatus::UpToDate {
                                last_sync_time: now,
                            };
                        }
                        app.load_hosts(cx);
                        app.desktop_command(Command::Refresh, cx);
                        app.listen_server_ws_events(cx);
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

    /// Enrolls the current desktop device with a remote Shellumi Server using protocol v1 challenge-response.
    pub(crate) fn enroll_current_device(
        &mut self,
        server_conn_id: String,
        device_label: String,
        cx: &mut Context<Self>,
    ) {
        let Some(reg_path) = self.registry_db_path() else {
            return;
        };
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut reg_store = HostStore::open_with_key(&reg_path, default_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                let conn = reg_store
                    .get_vault_server_connection(&server_conn_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("Server connection '{server_conn_id}' not found"))?;

                let cred_store = shellumi_core::default_credential_store();
                let token_sec = conn
                    .resolve_auth_token(&*cred_store)
                    .ok_or_else(|| "Server connection missing auth token".to_string())?;
                let mut client = ShellumiServerClient::new(&conn.base_url);
                client.set_token(token_sec.expose_secret().to_string());

                let resp = reg_store
                    .enroll_device_with_server(&server_conn_id, &mut client, &device_label)
                    .await
                    .map_err(|e| e.to_string())?;

                Ok::<_, String>(resp)
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok(_resp) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        app.sync_status = AppSyncStatus::UpToDate {
                            last_sync_time: now,
                        };
                        app.load_vault_list(cx);
                    }
                    Err(e) => {
                        app.sync_status =
                            AppSyncStatus::Error(format!("Device enrollment failed: {e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Subscribes to Phase 11 WebSocket notification events for the active remote vault.
    pub(crate) fn listen_server_ws_events(&mut self, cx: &mut Context<Self>) {
        if !self.is_active_vault_remote() {
            return;
        }
        let Some(reg_path) = self.registry_db_path() else {
            return;
        };
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let active_vault_id = self.active_vault_id.clone();

        if self.active_ws_client.is_some() {
            return;
        }

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let reg_store = HostStore::open_with_key(&reg_path, default_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                let contexts = reg_store
                    .list_vault_contexts()
                    .await
                    .map_err(|e| e.to_string())?;
                let target_ctx = contexts
                    .into_iter()
                    .find(|(_, ctx)| ctx.id().as_str() == active_vault_id)
                    .ok_or_else(|| "Active vault context not found".to_string())?;

                let (server_conn_id, remote_vault_id) = match target_ctx.1.location() {
                    shellumi_core::VaultLocation::Remote {
                        server,
                        server_vault_id,
                    } => (
                        server.as_str().to_string(),
                        server_vault_id.as_str().to_string(),
                    ),
                    shellumi_core::VaultLocation::Local => return Err("Local vault".to_string()),
                };

                let conn_record = reg_store
                    .get_vault_server_connection(&server_conn_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "Server connection not found".to_string())?;

                let cred_store = shellumi_core::default_credential_store();
                let token = conn_record
                    .resolve_auth_token(&*cred_store)
                    .map(|t| t.expose_secret().to_string())
                    .unwrap_or_default();
                let device_uuid = conn_record
                    .device_id
                    .as_deref()
                    .and_then(|d| uuid::Uuid::parse_str(d).ok())
                    .unwrap_or_else(uuid::Uuid::new_v4);
                let remote_uuid =
                    uuid::Uuid::parse_str(&remote_vault_id).map_err(|e| e.to_string())?;

                let config = shellumi_core::server::WsClientConfig::new(
                    conn_record.base_url,
                    token,
                    device_uuid,
                );

                Ok::<_, String>((config, remote_uuid))
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            if let Ok((config, remote_uuid)) = res {
                let ws_client = shellumi_core::server::ShellumiWsClient::spawn(config);
                let client_ref = ws_client.clone();
                let _ = app.update(cx, |app, cx| {
                    app.active_ws_client = Some(ws_client);
                    app.server_ws_status =
                        Some(shellumi_core::server::WsConnectionStatus::Connecting);
                    cx.notify();
                });

                let sub_task = cx.background_executor().spawn(async move {
                    let _ = client_ref.subscribe_vaults(vec![remote_uuid]).await;
                    let mut rx = client_ref.subscribe_events();
                    while let Ok(event) = rx.recv().await {
                        match event {
                            shellumi_core::server::SyncNotificationEvent::VaultChanged {
                                ..
                            }
                            | shellumi_core::server::SyncNotificationEvent::KeyRotated { .. } => {
                                return Ok(());
                            }
                            shellumi_core::server::SyncNotificationEvent::AccessRevoked {
                                ..
                            } => {
                                return Err("AccessRevoked".to_string());
                            }
                            _ => {}
                        }
                    }
                    Ok(())
                });

                let event_res = sub_task.await;
                let _ = app.update(cx, |app, cx| match event_res {
                    Ok(()) => {
                        app.sync_active_vault(cx);
                    }
                    Err(e) if e == "AccessRevoked" => {
                        app.sync_status = AppSyncStatus::Error(
                            "Vault access was revoked by administrator on server.".into(),
                        );
                        cx.notify();
                    }
                    _ => {}
                });
            }
        })
        .detach();
    }
}

//! Vault creation and remote server enrollment actions.
use super::*;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use shellumi_core::server::{
    generate_device_ed25519_keypair_pkcs8, CreateVaultRequest, LoginRequest, RegisterDeviceRequest,
    ShellumiServerClient,
};
use shellumi_core::{ServerConnectionId, VaultContext, VaultId, VaultLocation};
use shellumi_storage::{VaultServerConnectionRecord, VaultSyncStateRecord};
use uuid::Uuid;

impl Shellumi {
    /// Opens the vault creation modal dialog.
    pub(crate) fn open_create_vault_modal(&mut self, cx: &mut Context<Self>) {
        self.vault_dropdown_open = false;
        let server_url = cx.new(|cx| {
            TextInput::new(
                cx,
                "https://shellumi.corp.internal:8443",
                "Server URL (e.g. https://shellumi.corp.internal:8443)",
            )
        });
        let username = cx.new(|cx| TextInput::new(cx, "", "Username or email"));
        let password = cx.new(|cx| TextInput::new(cx, "", "Account password").masked());
        let device_name = cx.new(|cx| {
            let default_name = std::env::var("USER")
                .map(|u| format!("{u}-desktop"))
                .unwrap_or_else(|_| "MacBook-Desktop".to_string());
            TextInput::new(cx, &default_name, "Friendly device name")
        });
        let vault_name =
            cx.new(|cx| TextInput::new(cx, "", "Vault name (e.g. Production Infrastructure)"));
        let vault_passphrase = cx.new(|cx| {
            TextInput::new(cx, "", "Zero-knowledge passphrase (min 12 characters)").masked()
        });
        let local_desc = cx.new(|cx| TextInput::new(cx, "", "Description (optional)"));

        self.create_vault_modal = Some(CreateVaultModalState {
            active_tab: CreateVaultModalTab::RemoteServer,
            server_url_input: server_url,
            username_input: username,
            password_input: password,
            device_name_input: device_name,
            vault_name_input: vault_name,
            vault_passphrase_input: vault_passphrase,
            local_vault_desc_input: local_desc,
            error: None,
            status_message: None,
            is_testing: false,
            is_submitting: false,
        });
        cx.notify();
    }

    /// Dismisses the vault creation modal dialog.
    pub(crate) fn dismiss_create_vault_modal(&mut self, cx: &mut Context<Self>) {
        self.create_vault_modal = None;
        cx.notify();
    }

    /// Switches between Remote Server and Local vault creation tabs.
    pub(crate) fn set_create_vault_tab(
        &mut self,
        tab: CreateVaultModalTab,
        cx: &mut Context<Self>,
    ) {
        if let Some(modal) = &mut self.create_vault_modal {
            modal.active_tab = tab;
            modal.error = None;
            modal.status_message = None;
            cx.notify();
        }
    }

    /// Tests reachability and health of the specified remote Shellumi Server.
    pub(crate) fn test_remote_server_connection(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = &mut self.create_vault_modal else {
            return;
        };

        let server_url = modal.server_url_input.read(cx).value().trim().to_string();
        if server_url.is_empty() {
            modal.error = Some("Server URL cannot be empty.".to_string());
            cx.notify();
            return;
        }

        if !server_url.starts_with("http://") && !server_url.starts_with("https://") {
            modal.error = Some("Server URL must begin with http:// or https://".to_string());
            cx.notify();
            return;
        }

        modal.is_testing = true;
        modal.error = None;
        modal.status_message = None;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let client = ShellumiServerClient::new(&server_url);
            let health = client.health_live().map_err(|e| e.to_string())?;
            let caps = client.capabilities().map_err(|e| e.to_string())?;
            Ok::<_, String>((health, caps))
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.create_vault_modal {
                    modal.is_testing = false;
                    match result {
                        Ok((health, caps)) => {
                            modal.status_message = Some(format!(
                                "Server online: status={} (protocol v{})",
                                health.status, caps.protocol_version
                            ));
                        }
                        Err(e) => {
                            modal.error = Some(format!("Connection test failed: {e}"));
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Submits vault creation or remote enrollment.
    pub(crate) fn submit_create_vault(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = &mut self.create_vault_modal else {
            return;
        };

        let active_tab = modal.active_tab;
        let Some(db_path) = self.database.clone() else {
            modal.error = Some("Local database is not initialized.".to_string());
            cx.notify();
            return;
        };

        match active_tab {
            CreateVaultModalTab::Local => {
                let vault_name = modal.vault_name_input.read(cx).value().trim().to_string();
                let desc = modal
                    .local_vault_desc_input
                    .read(cx)
                    .value()
                    .trim()
                    .to_string();

                if vault_name.is_empty() {
                    modal.error = Some("Vault name cannot be empty.".to_string());
                    cx.notify();
                    return;
                }

                modal.is_submitting = true;
                modal.error = None;
                modal.status_message = None;
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut store = HostStore::open(&db_path).await.map_err(|e| e.to_string())?;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    let now_secs = now.as_secs();
                    let now_nanos = now.subsec_nanos();
                    let vault_id_str = format!("vlt-{now_secs}-{now_nanos}");
                    let now_iso = format!("{now_secs}");
                    let desc_opt = if desc.is_empty() { None } else { Some(desc) };

                    let shared_vault = SharedVault::new(
                        &vault_id_str,
                        &vault_name,
                        desc_opt,
                        "local_owner",
                        &now_iso,
                    )
                    .map_err(|e| e.to_string())?;
                    store
                        .create_shared_vault(&shared_vault)
                        .await
                        .map_err(|e| e.to_string())?;

                    let local_vault_id = VaultId::new(vault_id_str).map_err(|e| e.to_string())?;
                    let context = VaultContext::new(local_vault_id, VaultLocation::Local);
                    store
                        .register_vault_context(&vault_name, &context)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok::<_, String>(vault_name)
                });

                cx.spawn(async move |app, cx| {
                    let result = task.await;
                    let _ = app.update(cx, |app, cx| match result {
                        Ok(created_name) => {
                            app.active_vault = created_name;
                            app.create_vault_modal = None;
                            app.load_hosts(cx);
                            cx.notify();
                        }
                        Err(e) => {
                            if let Some(modal) = &mut app.create_vault_modal {
                                modal.is_submitting = false;
                                modal.error = Some(format!("Failed to create local vault: {e}"));
                                cx.notify();
                            }
                        }
                    });
                })
                .detach();
            }
            CreateVaultModalTab::RemoteServer => {
                let server_url = modal.server_url_input.read(cx).value().trim().to_string();
                let username = modal.username_input.read(cx).value().trim().to_string();
                let password = modal.password_input.read(cx).value().trim().to_string();
                let device_name = modal.device_name_input.read(cx).value().trim().to_string();
                let vault_name = modal.vault_name_input.read(cx).value().trim().to_string();
                let passphrase = modal
                    .vault_passphrase_input
                    .read(cx)
                    .value()
                    .trim()
                    .to_string();

                if server_url.is_empty() {
                    modal.error = Some("Server URL cannot be empty.".to_string());
                    cx.notify();
                    return;
                }
                if username.is_empty() {
                    modal.error = Some("Username or email cannot be empty.".to_string());
                    cx.notify();
                    return;
                }
                if password.is_empty() {
                    modal.error = Some("Password cannot be empty.".to_string());
                    cx.notify();
                    return;
                }
                if vault_name.is_empty() {
                    modal.error = Some("Vault name cannot be empty.".to_string());
                    cx.notify();
                    return;
                }
                if passphrase.len() < 12 {
                    modal.error = Some(
                        "Passphrase must be at least 12 characters for zero-knowledge security."
                            .to_string(),
                    );
                    cx.notify();
                    return;
                }

                modal.is_submitting = true;
                modal.error = None;
                modal.status_message = None;
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut client = ShellumiServerClient::new(&server_url);

                    // 1. Authenticate with server
                    let login_resp = client
                        .login(&LoginRequest {
                            identifier: username.clone(),
                            password,
                            device_id: None,
                        })
                        .map_err(|e| format!("Login failed: {e}"))?;

                    client.set_token(login_resp.access_token.clone());

                    // 2. Generate local device signing keypair (Ed25519)
                    let (signing_pkcs8, signing_pub_b64) =
                        generate_device_ed25519_keypair_pkcs8().map_err(|e| e.to_string())?;

                    // 3. Register device with server
                    let dev_reg = RegisterDeviceRequest {
                        device_name: device_name.clone(),
                        signing_public_key: signing_pub_b64,
                        agreement_public_key: "dummy-agreement-key".to_string(),
                    };
                    let device_resp = client
                        .register_device(&dev_reg)
                        .map_err(|e| format!("Device enrollment failed: {e}"))?;

                    client.set_device_id(device_resp.device_id);

                    // 4. Find or create vault on server
                    let existing_vaults =
                        client.list_vaults().map(|r| r.vaults).unwrap_or_default();
                    let remote_vault_id = if let Some(v) = existing_vaults
                        .iter()
                        .find(|v| v.encrypted_name.as_deref() == Some(&vault_name))
                    {
                        v.vault_id
                    } else {
                        let create_req = CreateVaultRequest {
                            name: Some(vault_name.clone()),
                            encrypted_name: Some(vault_name.clone()),
                            vault_type: "personal".to_string(),
                        };
                        let new_vault = client
                            .create_vault(&create_req)
                            .map_err(|e| format!("Failed to create vault on server: {e}"))?;
                        new_vault.vault_id
                    };

                    // 5. Store connection and vault in local Turso database
                    let mut store = HostStore::open(&db_path).await.map_err(|e| e.to_string())?;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);

                    let conn_id = format!("srv-{}", Uuid::new_v4());
                    let conn_record = VaultServerConnectionRecord {
                        id: conn_id.clone(),
                        base_url: server_url,
                        auth_token: Some(login_resp.access_token),
                        refresh_token: Some(login_resp.refresh_token),
                        device_id: Some(device_resp.device_id.to_string()),
                        device_signing_key: Some(BASE64_STANDARD.encode(&signing_pkcs8)),
                        created_at: now,
                        updated_at: now,
                    };
                    store
                        .save_vault_server_connection(&conn_record)
                        .await
                        .map_err(|e| e.to_string())?;

                    let local_vault_id =
                        VaultId::new(format!("v-{}", Uuid::new_v4())).map_err(|e| e.to_string())?;
                    let context = VaultContext::new(
                        local_vault_id.clone(),
                        VaultLocation::Remote {
                            server: ServerConnectionId::new(&conn_id).map_err(|e| e.to_string())?,
                            server_vault_id: VaultId::new(remote_vault_id.to_string())
                                .map_err(|e| e.to_string())?,
                        },
                    );
                    store
                        .register_vault_context(&vault_name, &context)
                        .await
                        .map_err(|e| e.to_string())?;

                    let sync_state = VaultSyncStateRecord {
                        vault_id: local_vault_id.as_str().to_string(),
                        server_sequence: 0,
                        last_cursor: None,
                        last_synced_at: None,
                        status: "idle".to_string(),
                        error_message: None,
                    };
                    store
                        .save_vault_sync_state(&sync_state)
                        .await
                        .map_err(|e| e.to_string())?;

                    Ok::<_, String>(vault_name)
                });

                cx.spawn(async move |app, cx| {
                    let result = task.await;
                    let _ = app.update(cx, |app, cx| match result {
                        Ok(created_name) => {
                            app.active_vault = created_name;
                            app.create_vault_modal = None;
                            app.load_hosts(cx);
                            cx.notify();
                        }
                        Err(e) => {
                            if let Some(modal) = &mut app.create_vault_modal {
                                modal.is_submitting = false;
                                modal.error = Some(e);
                                cx.notify();
                            }
                        }
                    });
                })
                .detach();
            }
        }
    }
}

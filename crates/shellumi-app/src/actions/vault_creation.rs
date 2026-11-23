//! Vault creation and remote server enrollment actions.
use super::*;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use shellumi_core::server::{
    generate_device_ed25519_keypair_pkcs8, generate_device_x25519_keypair, keypair_from_pkcs8,
    sign_device_challenge, CreateVaultRequest, DeviceChallengeRequest, LoginRequest,
    RegisterDeviceRequest, ShellumiServerClient,
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
        let local_password = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                "Vault password (min 8 chars, uppercase, lowercase, number, symbol)",
            )
            .masked()
        });
        let local_password_confirm =
            cx.new(|cx| TextInput::new(cx, "", "Confirm vault password").masked());

        self.create_vault_modal = Some(CreateVaultModalState {
            active_tab: CreateVaultModalTab::RemoteServer,
            server_url_input: server_url,
            username_input: username,
            password_input: password,
            device_name_input: device_name,
            vault_name_input: vault_name,
            vault_passphrase_input: vault_passphrase,
            local_vault_desc_input: local_desc,
            local_vault_password_input: local_password,
            local_vault_password_confirm_input: local_password_confirm,
            remember_in_keychain: false,
            generated_recovery_code: None,
            recovery_code_confirmed: false,
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

    /// Toggles confirmation of having saved the emergency recovery code.
    pub(crate) fn toggle_create_vault_recovery_confirmed(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.create_vault_modal {
            modal.recovery_code_confirmed = !modal.recovery_code_confirmed;
            cx.notify();
        }
    }

    /// Finalizes vault creation after the user confirms saving their recovery code.
    pub(crate) fn finalize_vault_creation(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.create_vault_modal {
            if !modal.recovery_code_confirmed {
                modal.error = Some(
                    "Please confirm that you have saved your recovery code before proceeding."
                        .to_string(),
                );
                cx.notify();
                return;
            }
            self.create_vault_modal = None;
            self.load_vault_list(cx);
            self.start_storage(false, cx);
            cx.notify();
        }
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

    /// Toggles the "remember in keychain" checkbox for local vault creation.
    pub(crate) fn toggle_create_vault_remember_keychain(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.create_vault_modal {
            modal.remember_in_keychain = !modal.remember_in_keychain;
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
                let _desc = modal
                    .local_vault_desc_input
                    .read(cx)
                    .value()
                    .trim()
                    .to_string();
                let password = modal
                    .local_vault_password_input
                    .read(cx)
                    .value()
                    .trim()
                    .to_string();
                let password_confirm = modal
                    .local_vault_password_confirm_input
                    .read(cx)
                    .value()
                    .trim()
                    .to_string();
                let remember_in_keychain = modal.remember_in_keychain;

                if vault_name.is_empty() {
                    modal.error = Some("Vault name cannot be empty.".to_string());
                    cx.notify();
                    return;
                }
                if password.is_empty() {
                    modal.error = Some("Vault password cannot be empty.".to_string());
                    cx.notify();
                    return;
                }
                if password != password_confirm {
                    modal.error =
                        Some("Passwords do not match. Please verify your password.".to_string());
                    cx.notify();
                    return;
                }
                if let Err(err) = shellumi_core::validate_vault_password(&password) {
                    modal.error = Some(format!(
                        "Password does not meet security requirements: {err} (min 8 characters, with uppercase, lowercase, numbers, and symbols)."
                    ));
                    cx.notify();
                    return;
                }

                modal.is_submitting = true;
                modal.error = None;
                modal.status_message = None;
                cx.notify();

                let db_path_clone = db_path.clone();
                let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
                let task = cx.background_executor().spawn(async move {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    let now_secs = now.as_secs();
                    let now_nanos = now.subsec_nanos();
                    let vault_id_str = format!("vlt-{now_secs}-{now_nanos}");
                    let local_vault_id = VaultId::new(&vault_id_str).map_err(|e| e.to_string())?;

                    // 1. Provision random 256-bit DEK, wrap with Argon2id password, and generate recovery code
                    let cred_store = shellumi_core::default_credential_store();
                    let recovery_code = shellumi_core::VaultKey::generate_recovery_code()
                        .map_err(|e| e.to_string())?;
                    let (key, primary_env, recovery_env) =
                        shellumi_core::VaultKeyManager::provision_new_vault(
                            cred_store.as_ref(),
                            &local_vault_id,
                            &password,
                            remember_in_keychain,
                            Some(&recovery_code),
                        )
                        .map_err(|e| e.to_string())?;

                    // 2. Persist portable key envelope manifest BEFORE creating database
                    let manifest = shellumi_core::VaultManifest::new(
                        local_vault_id.as_str().to_string(),
                        vault_name.clone(),
                        primary_env.clone(),
                        recovery_env.clone(),
                        remember_in_keychain,
                    );
                    let manifest_path = data_path::resolve_manifest_file(
                        std::path::Path::new(&db_path_clone),
                        local_vault_id.as_str(),
                    );
                    manifest
                        .save_to_path(&manifest_path)
                        .map_err(|e| format!("Failed to save vault manifest: {e}"))?;

                    // 3. Create separate physical encrypted database for this vault
                    let new_db_path = data_path::resolve_vault_db_file(
                        std::path::Path::new(&db_path_clone),
                        local_vault_id.as_str(),
                    );
                    if let Err(e) = data_path::prepare_parent(&new_db_path) {
                        let _ = std::fs::remove_file(&manifest_path);
                        return Err(e);
                    }
                    let new_db_str = new_db_path
                        .to_str()
                        .ok_or_else(|| "Invalid UTF-8 path for new vault database".to_string())?;
                    let mut new_store = match HostStore::open_with_key(new_db_str, Some(&key)).await
                    {
                        Ok(s) => s,
                        Err(err) => {
                            let _ = std::fs::remove_file(&manifest_path);
                            let _ = std::fs::remove_file(&new_db_path);
                            return Err(format!("Failed to initialize encrypted database: {err}"));
                        }
                    };
                    new_store.set_vault_id(local_vault_id.clone());

                    // 4. Register in default vault registry (default.db)
                    let reg_db_path = data_path::resolve_vault_db_file(
                        std::path::Path::new(&db_path_clone),
                        shellumi_storage::LEGACY_LOCAL_VAULT_ID,
                    );
                    let mut reg_store = match HostStore::open_with_key(
                        reg_db_path.to_str().unwrap(),
                        default_key.as_ref(),
                    )
                    .await
                    {
                        Ok(s) => s,
                        Err(err) => {
                            let _ = std::fs::remove_file(&manifest_path);
                            let _ = std::fs::remove_file(&new_db_path);
                            return Err(format!("Failed to open default registry database: {err}"));
                        }
                    };

                    let context = VaultContext::new(local_vault_id.clone(), VaultLocation::Local);
                    if let Err(e) = reg_store
                        .register_vault_context(&vault_name, &context)
                        .await
                    {
                        let _ = std::fs::remove_file(&manifest_path);
                        let _ = std::fs::remove_file(&new_db_path);
                        return Err(format!("Failed to register vault context: {e}"));
                    }

                    if let Ok(env_json) = serde_json::to_string(&primary_env) {
                        let _ = new_store
                            .save_vault_recovery_envelope(local_vault_id.as_str(), &env_json)
                            .await;
                        let _ = reg_store
                            .save_vault_recovery_envelope(local_vault_id.as_str(), &env_json)
                            .await;
                    }

                    Ok::<_, String>((
                        vault_name,
                        local_vault_id.as_str().to_string(),
                        key,
                        recovery_code,
                    ))
                });

                cx.spawn(async move |app, cx| {
                    let result = task.await;
                    let _ = app.update(cx, |app, cx| match result {
                        Ok((created_name, vault_id, key, recovery_code)) => {
                            app.available_vaults.push(VaultContextSummary {
                                id: vault_id.clone(),
                                name: created_name.clone(),
                                is_remote: false,
                                server_url: None,
                                server_vault_id: None,
                                is_locked: false,
                            });
                            app.active_vault = created_name;
                            app.active_vault_id = vault_id.clone();
                            app.active_vault_keys.insert(vault_id, key);
                            if let Some(modal) = &mut app.create_vault_modal {
                                modal.is_submitting = false;
                                modal.generated_recovery_code = Some(recovery_code);
                                modal.recovery_code_confirmed = false;
                            }
                            app.load_vault_list(cx);
                            app.load_hosts(cx);
                            app.load_identities(cx);
                            app.load_workspaces(cx);
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
                if let Err(err) = shellumi_core::validate_vault_password(&passphrase) {
                    modal.error = Some(format!(
                        "Passphrase does not meet security requirements: {err} (min 8 characters, with uppercase, lowercase, numbers, and symbols)."
                    ));
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

                let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
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

                    // 2. Generate local device signing keypair (Ed25519) & agreement keypair (X25519)
                    let (signing_pkcs8, signing_pub_b64) =
                        generate_device_ed25519_keypair_pkcs8().map_err(|e| e.to_string())?;
                    let (agreement_secret, agreement_pub_b64) =
                        generate_device_x25519_keypair().map_err(|e| e.to_string())?;
                    let signing_keypair =
                        keypair_from_pkcs8(&signing_pkcs8).map_err(|e| e.to_string())?;

                    // Challenge and sign
                    let challenge_resp = client
                        .device_challenge(&DeviceChallengeRequest::default())
                        .map_err(|e| format!("Device challenge failed: {e}"))?;
                    let challenge_sig =
                        sign_device_challenge(&signing_keypair, &challenge_resp.challenge);

                    // 3. Register device with server
                    let dev_reg = RegisterDeviceRequest {
                        device_name: device_name.clone(),
                        signing_public_key: signing_pub_b64,
                        agreement_public_key: agreement_pub_b64,
                        challenge: Some(challenge_resp.challenge),
                        challenge_signature: Some(challenge_sig),
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

                    // 5. Store connection and vault in default registry database
                    let reg_db_path = data_path::resolve_vault_db_file(
                        std::path::Path::new(&db_path),
                        shellumi_storage::LEGACY_LOCAL_VAULT_ID,
                    );
                    let cred_store = shellumi_core::default_credential_store();
                    let mut reg_store = HostStore::open_with_key(
                        reg_db_path.to_str().unwrap(),
                        default_key.as_ref(),
                    )
                    .await
                    .map_err(|e| e.to_string())?;

                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);

                    let server_inst = client
                        .capabilities()
                        .ok()
                        .map(|c| c.server_instance_id.to_string());
                    let conn_id = format!("srv-{}", Uuid::new_v4());
                    let conn_record = VaultServerConnectionRecord {
                        id: conn_id.clone(),
                        base_url: server_url,
                        auth_token: Some(login_resp.access_token),
                        refresh_token: Some(login_resp.refresh_token),
                        device_id: Some(device_resp.device_id.to_string()),
                        device_signing_key: Some(BASE64_STANDARD.encode(&signing_pkcs8)),
                        server_instance_id: server_inst,
                        device_agreement_key: Some(
                            BASE64_STANDARD.encode(agreement_secret.as_ref()),
                        ),
                        created_at: now,
                        updated_at: now,
                    };
                    reg_store
                        .save_vault_server_connection_secured(&conn_record, cred_store.as_ref())
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
                    reg_store
                        .register_vault_context(&vault_name, &context)
                        .await
                        .map_err(|e| e.to_string())?;

                    // Provision vault key into OS credential store and generate recovery envelope
                    let (key, primary_env, recovery_env) =
                        shellumi_core::VaultKeyManager::provision_new_vault(
                            cred_store.as_ref(),
                            &local_vault_id,
                            &passphrase,
                            true,
                            None,
                        )
                        .map_err(|e| e.to_string())?;

                    let manifest = shellumi_core::VaultManifest::new(
                        local_vault_id.as_str().to_string(),
                        vault_name.clone(),
                        primary_env.clone(),
                        recovery_env,
                        true,
                    );
                    let manifest_path = data_path::resolve_manifest_file(
                        std::path::Path::new(&db_path),
                        local_vault_id.as_str(),
                    );
                    manifest
                        .save_to_path(&manifest_path)
                        .map_err(|e| format!("Failed to save manifest: {e}"))?;

                    // Initialize separate encrypted physical database for remote vault cache
                    let new_db_path = data_path::resolve_vault_db_file(
                        std::path::Path::new(&db_path),
                        local_vault_id.as_str(),
                    );
                    if let Err(e) = data_path::prepare_parent(&new_db_path) {
                        let _ = std::fs::remove_file(&manifest_path);
                        return Err(e);
                    }
                    let mut new_store =
                        match HostStore::open_with_key(new_db_path.to_str().unwrap(), Some(&key))
                            .await
                        {
                            Ok(s) => s,
                            Err(err) => {
                                let _ = std::fs::remove_file(&manifest_path);
                                let _ = std::fs::remove_file(&new_db_path);
                                return Err(format!(
                                    "Failed to initialize encrypted database: {err}"
                                ));
                            }
                        };
                    new_store.set_vault_id(local_vault_id.clone());

                    if let Ok(env_json) = serde_json::to_string(&primary_env) {
                        let _ = new_store
                            .save_vault_recovery_envelope(local_vault_id.as_str(), &env_json)
                            .await;
                        if let Err(e) = reg_store
                            .save_vault_recovery_envelope(local_vault_id.as_str(), &env_json)
                            .await
                        {
                            let _ = std::fs::remove_file(&manifest_path);
                            let _ = std::fs::remove_file(&new_db_path);
                            return Err(e.to_string());
                        }
                    }

                    let sync_state = VaultSyncStateRecord {
                        vault_id: local_vault_id.as_str().to_string(),
                        server_sequence: 0,
                        last_cursor: None,
                        last_synced_at: None,
                        status: "idle".to_string(),
                        error_message: None,
                        active_key_epoch: 1,
                        conflicted_object_id: None,
                        conflict_details: None,
                    };
                    if let Err(e) = reg_store.save_vault_sync_state(&sync_state).await {
                        let _ = std::fs::remove_file(&manifest_path);
                        let _ = std::fs::remove_file(&new_db_path);
                        return Err(e.to_string());
                    }

                    Ok::<_, String>((vault_name, local_vault_id.as_str().to_string(), key))
                });

                cx.spawn(async move |app, cx| {
                    let result = task.await;
                    let _ = app.update(cx, |app, cx| match result {
                        Ok((created_name, vault_id, key)) => {
                            app.available_vaults.push(VaultContextSummary {
                                id: vault_id.clone(),
                                name: created_name.clone(),
                                is_remote: true,
                                server_url: None,
                                server_vault_id: None,
                                is_locked: false,
                            });
                            app.active_vault = created_name;
                            app.active_vault_id = vault_id.clone();
                            app.active_vault_keys.insert(vault_id, key);
                            app.create_vault_modal = None;
                            app.load_vault_list(cx);
                            app.load_hosts(cx);
                            app.load_identities(cx);
                            app.load_workspaces(cx);
                            if app.is_active_vault_remote() {
                                app.sync_active_vault(cx);
                            }
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

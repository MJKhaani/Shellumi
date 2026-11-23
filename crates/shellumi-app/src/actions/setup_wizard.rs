//! First-run setup wizard actions and background workflows.
use super::*;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use shellumi_core::server::{
    generate_device_ed25519_keypair_pkcs8, generate_device_x25519_keypair, keypair_from_pkcs8,
    sign_device_challenge, CreateVaultRequest, DeviceChallengeRequest, LoginRequest,
    RegisterDeviceRequest, ShellumiServerClient,
};
use shellumi_core::setup::{
    verify_setup_integrity, ConfiguredVaultSummary, InstallationStatus, SetupState, SetupStep,
    SetupWizardMode,
};
use shellumi_core::sync::{
    build_google_drive_auth_url, generate_pkce_challenge, GoogleDriveSyncClient,
    OAuthCallbackListener, SystemHttpTransport,
};
use shellumi_core::{
    ServerConnectionId, VaultContext, VaultId, VaultKey, VaultKeyManager, VaultLocation,
};
use shellumi_storage::VaultServerConnectionRecord;
use std::path::{Path, PathBuf};
use uuid::Uuid;

impl Shellumi {
    /// Opens the native GPUI first-run setup wizard.
    pub(crate) fn open_setup_wizard(&mut self, cx: &mut Context<Self>) {
        let base_dir = if let Some(base_str) = self.database.as_deref() {
            let p = Path::new(base_str);
            data_path::resolve_root_dir(p)
        } else {
            data_path::default_data_dir().unwrap_or_else(|_| PathBuf::from("."))
        };

        let installation_status = SetupState::detect_installation(&base_dir);
        let loaded_state = SetupState::load(&base_dir).ok().flatten();
        let mut state = loaded_state.unwrap_or_default();

        // If resuming an incomplete setup, resume at that step; otherwise start at Welcome
        if let InstallationStatus::IncompleteSetup { last_step, .. } = &installation_status {
            state.current_step = *last_step;
        }

        let default_password = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                "Strong vault password (min 8 chars, uppercase, lowercase, number, symbol)",
            )
            .masked()
        });
        let default_password_confirm =
            cx.new(|cx| TextInput::new(cx, "", "Confirm vault password").masked());
        let legacy_password =
            cx.new(|cx| TextInput::new(cx, "", "Legacy vault password (if encrypted)").masked());
        let existing_unlock =
            cx.new(|cx| TextInput::new(cx, "", "Vault password or recovery code").masked());

        let google_client_id = cx.new(|cx| {
            TextInput::new(
                cx,
                "shellumi-desktop",
                "Google OAuth Client ID (optional, defaults to standard Shellumi client)",
            )
        });

        let sec_vault_name =
            cx.new(|cx| TextInput::new(cx, "", "Vault name (e.g. Work Infrastructure)"));
        let sec_vault_password = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                "Secondary vault password (min 8 chars, uppercase, lowercase, digit, symbol)",
            )
            .masked()
        });
        let sec_vault_password_confirm =
            cx.new(|cx| TextInput::new(cx, "", "Confirm secondary password").masked());

        let server_url = cx.new(|cx| {
            TextInput::new(
                cx,
                "https://shellumi.corp.internal:8443",
                "Server URL (e.g. https://shellumi.corp.internal:8443)",
            )
        });
        let server_user = cx.new(|cx| TextInput::new(cx, "", "Username or email"));
        let server_pass = cx.new(|cx| TextInput::new(cx, "", "Account password").masked());
        let server_device_name = cx.new(|cx| {
            let default_name = std::env::var("USER")
                .map(|u| format!("{u}-desktop"))
                .unwrap_or_else(|_| "Desktop-Client".to_string());
            TextInput::new(cx, &default_name, "Friendly device name")
        });

        self.setup_wizard_modal = Some(SetupWizardModalState {
            state,
            installation_status,
            base_dir,
            default_password_input: default_password,
            default_password_confirm_input: default_password_confirm,
            default_recovery_code: None,
            recovery_code_confirmed: false,
            remember_in_keychain: false,
            legacy_password_input: legacy_password,
            existing_unlock_input: existing_unlock,
            unlock_with_recovery_code: false,
            google_client_id_input: google_client_id,
            is_connecting_google: false,
            google_auth_url: None,
            cloud_snapshots: Vec::new(),
            is_fetching_snapshots: false,
            sync_default_opt_in: false,
            additional_tab: AdditionalVaultsTab::CreateLocal,
            sec_vault_name_input: sec_vault_name,
            sec_vault_password_input: sec_vault_password,
            sec_vault_password_confirm_input: sec_vault_password_confirm,
            sec_vault_keychain: false,
            sec_vault_drive_opt_in: false,
            sec_recovery_code: None,
            sec_recovery_confirmed: false,
            server_url_input: server_url,
            server_user_input: server_user,
            server_pass_input: server_pass,
            server_device_name_input: server_device_name,
            is_testing_server: false,
            verification_report: None,
            is_busy: false,
            error: None,
            status_message: None,
        });

        cx.notify();
    }

    /// Selects the wizard mode (NewSetup vs RestoreSetup) and advances to DefaultVault step.
    pub(crate) fn wizard_select_mode(&mut self, mode: SetupWizardMode, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.state.mode = mode;
            let _ = wizard.state.save(&wizard.base_dir);
            wizard.state.current_step = SetupStep::DefaultVault;
            let _ = wizard.state.save(&wizard.base_dir);
            wizard.error = None;
            wizard.status_message = None;
            cx.notify();
        }
    }

    /// Navigates back to the previous step in the wizard.
    pub(crate) fn wizard_prev_step(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.error = None;
            wizard.status_message = None;
            let prev = match wizard.state.current_step {
                SetupStep::DefaultVault => SetupStep::Welcome,
                SetupStep::GoogleDrive => SetupStep::DefaultVault,
                SetupStep::AdditionalVaults => SetupStep::GoogleDrive,
                SetupStep::ReviewAndFinish => SetupStep::AdditionalVaults,
                _ => SetupStep::Welcome,
            };
            wizard.state.current_step = prev;
            let _ = wizard.state.save(&wizard.base_dir);
            cx.notify();
        }
    }

    /// Advances directly to the next step, persisting state.
    pub(crate) fn wizard_advance_to_step(&mut self, next: SetupStep, cx: &mut Context<Self>) {
        let key = if next == SetupStep::ReviewAndFinish {
            self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID)
        } else {
            None
        };
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.error = None;
            wizard.status_message = None;
            wizard.state.current_step = next;
            let _ = wizard.state.save(&wizard.base_dir);

            if next == SetupStep::ReviewAndFinish {
                // Run verification check
                wizard.verification_report =
                    Some(verify_setup_integrity(&wizard.base_dir, key.as_ref()));
            }

            cx.notify();
        }
    }

    /// Toggles the OS Keychain checkbox for the default vault.
    pub(crate) fn wizard_toggle_keychain(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.remember_in_keychain = !wizard.remember_in_keychain;
            wizard.state.remember_in_keychain = wizard.remember_in_keychain;
            cx.notify();
        }
    }

    /// Toggles confirmation that the emergency recovery code has been saved.
    pub(crate) fn wizard_toggle_recovery_confirmed(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.recovery_code_confirmed = !wizard.recovery_code_confirmed;
            wizard.state.recovery_code_confirmed = wizard.recovery_code_confirmed;
            cx.notify();
        }
    }

    /// Toggles recovery code vs password toggle for unlocking existing vaults.
    pub(crate) fn wizard_toggle_unlock_mode(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.unlock_with_recovery_code = !wizard.unlock_with_recovery_code;
            cx.notify();
        }
    }

    /// Generates and creates the encrypted default vault.
    pub(crate) fn wizard_create_default_vault(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = &mut self.setup_wizard_modal else {
            return;
        };

        let password = wizard
            .default_password_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let confirm = wizard
            .default_password_confirm_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let remember_in_keychain = wizard.remember_in_keychain;

        if password.is_empty() {
            wizard.error = Some("Vault password cannot be empty.".to_string());
            cx.notify();
            return;
        }
        if password != confirm {
            wizard.error = Some("Passwords do not match. Please re-enter.".to_string());
            cx.notify();
            return;
        }
        if let Err(err) = shellumi_core::validate_vault_password(&password) {
            wizard.error = Some(format!(
                "Password requirements not met: {err} (min 8 chars, uppercase, lowercase, number, symbol)."
            ));
            cx.notify();
            return;
        }

        wizard.is_busy = true;
        wizard.error = None;
        wizard.status_message =
            Some("Generating random DEK and Argon2id key manifest…".to_string());
        cx.notify();

        let base_dir = wizard.base_dir.clone();
        let task = cx.background_executor().spawn(async move {
            let default_vid =
                VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID).map_err(|e| e.to_string())?;

            // 1. Provision random DEK and emergency recovery code
            let cred_store = shellumi_core::default_credential_store();
            let recovery_code = VaultKey::generate_recovery_code().map_err(|e| e.to_string())?;
            let (key, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
                cred_store.as_ref(),
                &default_vid,
                &password,
                remember_in_keychain,
                Some(&recovery_code),
            )
            .map_err(|e| e.to_string())?;

            // 2. Persist wrapped key manifest BEFORE creating database
            let manifests_dir = base_dir.join("vault_manifests");
            data_path::prepare_parent(&manifests_dir.join("placeholder"))?;
            let manifest_path =
                manifests_dir.join(format!("{}.manifest.json", default_vid.as_str()));
            let manifest = shellumi_core::VaultManifest::new(
                default_vid.as_str().to_string(),
                "Default Vault".to_string(),
                primary_env.clone(),
                recovery_env.clone(),
                remember_in_keychain,
            );
            manifest
                .save_to_path(&manifest_path)
                .map_err(|e| format!("Failed to write vault manifest: {e}"))?;

            // 3. Create encrypted database default.db
            let vaults_dir = base_dir.join("vaults");
            data_path::prepare_parent(&vaults_dir.join("placeholder"))?;
            let db_path = vaults_dir.join("default.db");
            let db_str = db_path
                .to_str()
                .ok_or_else(|| "Invalid UTF-8 database path".to_string())?;

            let mut store = match HostStore::open_with_key(db_str, Some(&key)).await {
                Ok(s) => s,
                Err(err) => {
                    let _ = std::fs::remove_file(&manifest_path);
                    let _ = std::fs::remove_file(&db_path);
                    return Err(format!("Failed to initialize encrypted database: {err}"));
                }
            };
            store.set_vault_id(default_vid.clone());

            // 4. Register default vault in registry
            let context = VaultContext::new(default_vid.clone(), VaultLocation::Local);
            store
                .register_vault_context("Default Vault", &context)
                .await
                .map_err(|e| format!("Failed to register default vault context: {e}"))?;

            if let Ok(env_json) = serde_json::to_string(&primary_env) {
                let _ = store
                    .save_vault_recovery_envelope(default_vid.as_str(), &env_json)
                    .await;
            }

            Ok::<_, String>((recovery_code, key))
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(wizard) = &mut app.setup_wizard_modal {
                    wizard.is_busy = false;
                    match result {
                        Ok((recovery_code, key)) => {
                            wizard.default_recovery_code = Some(recovery_code);
                            wizard.state.default_vault_created = true;
                            wizard.state.configured_vaults.push(ConfiguredVaultSummary {
                                id: shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(),
                                name: "Default Vault".to_string(),
                                is_default: true,
                                is_remote: false,
                                sync_enabled: false,
                                server_url: None,
                            });
                            let _ = wizard.state.save(&wizard.base_dir);
                            app.active_vault_keys.insert(
                                shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(),
                                key,
                            );
                            wizard.status_message = Some(
                                "Default vault successfully encrypted! Please save your recovery code."
                                    .to_string(),
                            );
                        }
                        Err(e) => {
                            wizard.error = Some(format!("Vault creation failed: {e}"));
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Unlocks an existing local default vault using password or recovery code.
    pub(crate) fn wizard_unlock_existing_vault(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = &mut self.setup_wizard_modal else {
            return;
        };

        let secret = wizard
            .existing_unlock_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if secret.is_empty() {
            wizard.error = Some("Please enter your vault password or recovery code.".to_string());
            cx.notify();
            return;
        }

        let is_recovery = wizard.unlock_with_recovery_code;
        wizard.is_busy = true;
        wizard.error = None;
        wizard.status_message = Some("Verifying credentials and unlocking vault…".to_string());
        cx.notify();

        let base_dir = wizard.base_dir.clone();
        let task = cx.background_executor().spawn(async move {
            let default_vid =
                VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID).map_err(|e| e.to_string())?;

            let manifests_dir = base_dir.join("vault_manifests");
            let manifest_path =
                manifests_dir.join(format!("{}.manifest.json", default_vid.as_str()));
            let alt_path = manifests_dir.join("default.manifest.json");
            let active_path = if manifest_path.exists() {
                manifest_path
            } else if alt_path.exists() {
                alt_path
            } else {
                return Err("Default vault manifest not found.".to_string());
            };

            let manifest = shellumi_core::VaultManifest::load_from_path(&active_path)
                .map_err(|e| format!("Failed to read manifest: {e}"))?;

            let key = if is_recovery {
                let rec_env = manifest.recovery_envelope.ok_or_else(|| {
                    "This vault manifest does not contain a recovery envelope.".to_string()
                })?;
                VaultKey::recover(&default_vid, &secret, &rec_env)
                    .map_err(|_| "Invalid recovery code for default vault.".to_string())?
            } else {
                VaultKey::unwrap_with_password(&default_vid, &secret, &manifest.primary_envelope)
                    .map_err(|_| "Incorrect password for default vault.".to_string())?
            };

            // Test opening default.db to verify unlock
            let db_path = base_dir.join("vaults").join("default.db");
            let db_str = db_path
                .to_str()
                .ok_or_else(|| "Invalid UTF-8 database path".to_string())?;
            let store = HostStore::open_with_key(db_str, Some(&key))
                .await
                .map_err(|e| format!("Failed to open encrypted database with key: {e}"))?;

            let dir = store.build_vault_directory().await.unwrap_or_default();
            Ok::<_, String>((key, dir))
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(wizard) = &mut app.setup_wizard_modal {
                    wizard.is_busy = false;
                    match result {
                        Ok((key, dir)) => {
                            app.active_vault_keys
                                .insert(shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(), key);
                            wizard.state.default_vault_created = true;
                            wizard.state.configured_vaults.clear();
                            for entry in dir.entries {
                                wizard.state.configured_vaults.push(ConfiguredVaultSummary {
                                    id: entry.id.clone(),
                                    name: entry.name,
                                    is_default: entry.id == shellumi_storage::LEGACY_LOCAL_VAULT_ID,
                                    is_remote: matches!(
                                        entry.location,
                                        shellumi_core::VaultDirectoryLocation::Remote { .. }
                                    ),
                                    sync_enabled: entry.sync_enabled,
                                    server_url: None,
                                });
                            }
                            let _ = wizard.state.save(&wizard.base_dir);
                            wizard.status_message =
                                Some("Default vault unlocked successfully!".to_string());
                            wizard.state.current_step = SetupStep::GoogleDrive;
                            let _ = wizard.state.save(&wizard.base_dir);
                        }
                        Err(e) => {
                            wizard.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Performs legacy database migration during setup.
    pub(crate) fn wizard_migrate_legacy(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = &mut self.setup_wizard_modal else {
            return;
        };

        let password = wizard
            .legacy_password_input
            .read(cx)
            .value()
            .trim()
            .to_string();

        wizard.is_busy = true;
        wizard.error = None;
        wizard.status_message =
            Some("Migrating legacy database into encrypted vaults…".to_string());
        cx.notify();

        let base_dir = wizard.base_dir.clone();
        let task = cx.background_executor().spawn(async move {
            let cred_store = shellumi_core::default_credential_store();
            let key = if !password.is_empty() {
                let default_vid = VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID)
                    .map_err(|e| e.to_string())?;
                let (k, _, _) = VaultKeyManager::provision_new_vault(
                    cred_store.as_ref(),
                    &default_vid,
                    &password,
                    false,
                    None,
                )
                .map_err(|e| e.to_string())?;
                Some(k)
            } else {
                None
            };

            shellumi_storage::MigrationEngine::migrate_legacy_database_if_needed(
                &base_dir,
                key.as_ref(),
                cred_store.as_ref(),
            )
            .await
            .map_err(|e| format!("Migration failed: {e}"))?;

            Ok::<_, String>(key)
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(wizard) = &mut app.setup_wizard_modal {
                    wizard.is_busy = false;
                    match result {
                        Ok(key) => {
                            if let Some(k) = key {
                                app.active_vault_keys
                                    .insert(shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(), k);
                            }
                            wizard.state.default_vault_created = true;
                            wizard.state.configured_vaults.push(ConfiguredVaultSummary {
                                id: shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(),
                                name: "Default Vault (Migrated)".to_string(),
                                is_default: true,
                                is_remote: false,
                                sync_enabled: false,
                                server_url: None,
                            });
                            wizard.status_message = Some(
                                "Legacy database successfully migrated to encrypted vaults!"
                                    .to_string(),
                            );
                            wizard.state.current_step = SetupStep::GoogleDrive;
                            let _ = wizard.state.save(&wizard.base_dir);
                        }
                        Err(e) => {
                            wizard.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Connects to Google Drive using OAuth 2.0 PKCE without requiring environment variables.
    pub(crate) fn wizard_connect_google_drive(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = &mut self.setup_wizard_modal else {
            return;
        };

        let client_id = wizard
            .google_client_id_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let client_id = if client_id.is_empty() {
            "shellumi-desktop".to_string()
        } else {
            client_id
        };

        wizard.is_connecting_google = true;
        wizard.is_busy = true;
        wizard.error = None;
        wizard.status_message =
            Some("Binding local OAuth callback listener and generating authorization URL…".into());
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let listener = OAuthCallbackListener::bind_ephemeral()
                .map_err(|e| format!("Failed to bind loopback listener: {e}"))?;
            let redirect_uri = listener.redirect_uri();
            let pkce = generate_pkce_challenge()
                .map_err(|e| format!("Failed to generate PKCE challenge: {e}"))?;
            let state = Uuid::new_v4().to_string();

            let auth_url = build_google_drive_auth_url(&client_id, &redirect_uri, &pkce, &state);

            // Attempt to launch default browser via platform command
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&auth_url).spawn();
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", &auth_url])
                .spawn();
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let _ = std::process::Command::new("xdg-open")
                .arg(&auth_url)
                .spawn();

            // Wait for incoming redirect code (timeout: 120s)
            let auth_code = listener
                .wait_for_code(&state, std::time::Duration::from_secs(120))
                .map_err(|e| format!("OAuth authorization callback failed: {e}"))?;

            // Exchange authorization code for token bundle
            let client = GoogleDriveSyncClient::new(SystemHttpTransport::default(), &client_id);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let bundle = client
                .exchange_code(&auth_code, &pkce.code_verifier, &redirect_uri, now)
                .map_err(|e| format!("Token exchange failed: {e}"))?;

            // Save token bundle securely into platform credential store
            let cred_store = shellumi_core::default_credential_store();
            bundle
                .save_to_store(cred_store.as_ref())
                .map_err(|e| format!("Failed to save OAuth credentials to Keychain: {e}"))?;

            Ok::<_, String>((auth_url, bundle))
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(wizard) = &mut app.setup_wizard_modal {
                    wizard.is_connecting_google = false;
                    wizard.is_busy = false;
                    match result {
                        Ok((auth_url, _bundle)) => {
                            wizard.google_auth_url = Some(auth_url);
                            wizard.state.google_drive_connected = true;
                            let _ = wizard.state.save(&wizard.base_dir);
                            wizard.status_message = Some(
                                "Google Drive authenticated successfully! Credentials stored securely."
                                    .to_string(),
                            );
                        }
                        Err(e) => {
                            wizard.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Toggles the Google Drive sync opt-in checkbox for the default vault.
    pub(crate) fn wizard_toggle_drive_opt_in(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.sync_default_opt_in = !wizard.sync_default_opt_in;
            wizard.state.google_drive_sync_enabled = wizard.sync_default_opt_in;
            cx.notify();
        }
    }

    /// Switches tab in the Additional Vaults step.
    pub(crate) fn wizard_set_additional_tab(
        &mut self,
        tab: AdditionalVaultsTab,
        cx: &mut Context<Self>,
    ) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.additional_tab = tab;
            wizard.error = None;
            wizard.status_message = None;
            cx.notify();
        }
    }

    /// Toggles keychain checkbox for secondary vault creation.
    pub(crate) fn wizard_toggle_sec_keychain(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.sec_vault_keychain = !wizard.sec_vault_keychain;
            cx.notify();
        }
    }

    /// Toggles Google Drive sync opt-in for secondary vault creation.
    pub(crate) fn wizard_toggle_sec_drive_opt_in(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.sec_vault_drive_opt_in = !wizard.sec_vault_drive_opt_in;
            cx.notify();
        }
    }

    /// Toggles confirmation of secondary vault recovery code.
    pub(crate) fn wizard_toggle_sec_recovery_confirmed(&mut self, cx: &mut Context<Self>) {
        if let Some(wizard) = &mut self.setup_wizard_modal {
            wizard.sec_recovery_confirmed = !wizard.sec_recovery_confirmed;
            cx.notify();
        }
    }

    /// Creates an independent local secondary vault with its own DEK, password, and recovery code.
    pub(crate) fn wizard_create_secondary_vault(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = &mut self.setup_wizard_modal else {
            return;
        };

        let vault_name = wizard
            .sec_vault_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let password = wizard
            .sec_vault_password_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let confirm = wizard
            .sec_vault_password_confirm_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let remember_in_keychain = wizard.sec_vault_keychain;
        let sync_opt_in = wizard.sec_vault_drive_opt_in;

        if vault_name.is_empty() {
            wizard.error = Some("Secondary vault name cannot be empty.".to_string());
            cx.notify();
            return;
        }
        if password.is_empty() {
            wizard.error = Some("Password cannot be empty.".to_string());
            cx.notify();
            return;
        }
        if password != confirm {
            wizard.error = Some("Passwords do not match.".to_string());
            cx.notify();
            return;
        }
        if let Err(e) = shellumi_core::validate_vault_password(&password) {
            wizard.error = Some(format!("Password requirements not met: {e}"));
            cx.notify();
            return;
        }

        wizard.is_busy = true;
        wizard.error = None;
        wizard.status_message =
            Some("Provisioning secondary vault and encrypting database…".into());
        cx.notify();

        let base_dir = wizard.base_dir.clone();
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);

        let task = cx.background_executor().spawn(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let vault_id_str = format!("vlt-{}-{}", now.as_secs(), now.subsec_nanos());
            let vault_id = VaultId::new(&vault_id_str).map_err(|e| e.to_string())?;

            let cred_store = shellumi_core::default_credential_store();
            let recovery_code = VaultKey::generate_recovery_code().map_err(|e| e.to_string())?;
            let (key, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
                cred_store.as_ref(),
                &vault_id,
                &password,
                remember_in_keychain,
                Some(&recovery_code),
            )
            .map_err(|e| e.to_string())?;

            // Write manifest
            let manifests_dir = base_dir.join("vault_manifests");
            let manifest_path = manifests_dir.join(format!("{}.manifest.json", vault_id.as_str()));
            let manifest = shellumi_core::VaultManifest::new(
                vault_id.as_str().to_string(),
                vault_name.clone(),
                primary_env.clone(),
                recovery_env.clone(),
                remember_in_keychain,
            );
            manifest
                .save_to_path(&manifest_path)
                .map_err(|e| format!("Failed to write manifest: {e}"))?;

            // Initialize separate physical database
            let vaults_dir = base_dir.join("vaults");
            let db_path = vaults_dir.join(format!("{}.db", vault_id.as_str()));
            let db_str = db_path
                .to_str()
                .ok_or_else(|| "Invalid UTF-8 database path".to_string())?;

            let mut sec_store = match HostStore::open_with_key(db_str, Some(&key)).await {
                Ok(s) => s,
                Err(err) => {
                    let _ = std::fs::remove_file(&manifest_path);
                    let _ = std::fs::remove_file(&db_path);
                    return Err(format!("Failed to open new encrypted database: {err}"));
                }
            };
            sec_store.set_vault_id(vault_id.clone());

            // Register in default registry
            let reg_path = vaults_dir.join("default.db");
            let mut reg_store =
                match HostStore::open_with_key(reg_path.to_str().unwrap(), default_key.as_ref())
                    .await
                {
                    Ok(s) => s,
                    Err(err) => {
                        let _ = std::fs::remove_file(&manifest_path);
                        let _ = std::fs::remove_file(&db_path);
                        return Err(format!("Failed to open registry database: {err}"));
                    }
                };

            let context = VaultContext::new(vault_id.clone(), VaultLocation::Local);
            reg_store
                .register_vault_context(&vault_name, &context)
                .await
                .map_err(|e| format!("Failed to register vault: {e}"))?;

            if sync_opt_in {
                let _ = reg_store
                    .set_vault_sync_enabled(vault_id.as_str(), true)
                    .await;
            }

            Ok::<_, String>((vault_id_str, vault_name, recovery_code, key, sync_opt_in))
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(wizard) = &mut app.setup_wizard_modal {
                    wizard.is_busy = false;
                    match result {
                        Ok((vid, vname, rec_code, key, sync_in)) => {
                            wizard.sec_recovery_code = Some(rec_code);
                            app.active_vault_keys.insert(vid.clone(), key);
                            wizard.state.configured_vaults.push(ConfiguredVaultSummary {
                                id: vid,
                                name: vname,
                                is_default: false,
                                is_remote: false,
                                sync_enabled: sync_in,
                                server_url: None,
                            });
                            let _ = wizard.state.save(&wizard.base_dir);
                            wizard.status_message = Some(
                                "Secondary vault created! Please save its recovery code."
                                    .to_string(),
                            );
                        }
                        Err(e) => {
                            wizard.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Tests Shellumi Server reachability during setup.
    pub(crate) fn wizard_test_server(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = &mut self.setup_wizard_modal else {
            return;
        };

        let server_url = wizard.server_url_input.read(cx).value().trim().to_string();
        if server_url.is_empty() {
            wizard.error = Some("Server URL cannot be empty.".into());
            cx.notify();
            return;
        }

        wizard.is_testing_server = true;
        wizard.error = None;
        wizard.status_message = Some("Checking server health and protocol capabilities…".into());
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let client = ShellumiServerClient::new(&server_url);
            let health = client.health_live().map_err(|e| e.to_string())?;
            let caps = client.capabilities().map_err(|e| e.to_string())?;
            Ok::<_, String>((health.status, caps.protocol_version))
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(wizard) = &mut app.setup_wizard_modal {
                    wizard.is_testing_server = false;
                    match result {
                        Ok((status, proto)) => {
                            wizard.status_message = Some(format!(
                                "Server online: status='{status}', protocol version={proto}"
                            ));
                        }
                        Err(e) => {
                            wizard.error = Some(format!("Server connection test failed: {e}"));
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Connects to a remote Shellumi Server and enrolls the device.
    pub(crate) fn wizard_connect_server(&mut self, cx: &mut Context<Self>) {
        let Some(wizard) = &mut self.setup_wizard_modal else {
            return;
        };

        let server_url = wizard.server_url_input.read(cx).value().trim().to_string();
        let username = wizard.server_user_input.read(cx).value().trim().to_string();
        let password = wizard.server_pass_input.read(cx).value().trim().to_string();
        let device_name = wizard
            .server_device_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();

        if server_url.is_empty() || username.is_empty() || password.is_empty() {
            wizard.error = Some("Server URL, username, and password are required.".into());
            cx.notify();
            return;
        }

        wizard.is_busy = true;
        wizard.error = None;
        wizard.status_message = Some("Enrolling device and connecting server vault…".into());
        cx.notify();

        let base_dir = wizard.base_dir.clone();
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
            let signing_keypair = keypair_from_pkcs8(&signing_pkcs8).map_err(|e| e.to_string())?;

            // Challenge and sign
            let challenge_resp = client
                .device_challenge(&DeviceChallengeRequest::default())
                .map_err(|e| format!("Device challenge failed: {e}"))?;
            let challenge_sig = sign_device_challenge(&signing_keypair, &challenge_resp.challenge);

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
            let existing_vaults = client.list_vaults().map(|r| r.vaults).unwrap_or_default();
            let remote_vault = if let Some(v) = existing_vaults.first() {
                v.clone()
            } else {
                let create_req = CreateVaultRequest {
                    name: Some(format!("{username}'s Vault")),
                    encrypted_name: Some(format!("{username}'s Vault")),
                    vault_type: "personal".to_string(),
                };
                client
                    .create_vault(&create_req)
                    .map_err(|e| format!("Failed to create vault on server: {e}"))?
            };

            // 5. Save connection record in default registry
            let vaults_dir = base_dir.join("vaults");
            let reg_path = vaults_dir.join("default.db");
            let cred_store = shellumi_core::default_credential_store();
            let mut reg_store =
                HostStore::open_with_key(reg_path.to_str().unwrap(), default_key.as_ref())
                    .await
                    .map_err(|e| format!("Failed to open registry: {e}"))?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let server_inst = client
                .capabilities()
                .ok()
                .map(|c| c.server_instance_id.to_string());
            let conn_id = format!("srv-{}", Uuid::new_v4());
            let conn_rec = VaultServerConnectionRecord {
                id: conn_id.clone(),
                base_url: server_url.clone(),
                auth_token: Some(login_resp.access_token),
                refresh_token: Some(login_resp.refresh_token),
                device_id: Some(device_resp.device_id.to_string()),
                device_signing_key: Some(BASE64_STANDARD.encode(&signing_pkcs8)),
                server_instance_id: server_inst,
                device_agreement_key: Some(BASE64_STANDARD.encode(agreement_secret.as_ref())),
                created_at: now,
                updated_at: now,
            };

            reg_store
                .save_vault_server_connection_secured(&conn_rec, cred_store.as_ref())
                .await
                .map_err(|e| format!("Saving connection record failed: {e}"))?;

            let local_vault_id =
                VaultId::new(format!("v-{}", Uuid::new_v4())).map_err(|e| e.to_string())?;
            let vault_display_name = remote_vault
                .encrypted_name
                .clone()
                .unwrap_or_else(|| format!("{username}'s Vault"));
            let context = VaultContext::new(
                local_vault_id.clone(),
                VaultLocation::Remote {
                    server: ServerConnectionId::new(&conn_id).map_err(|e| e.to_string())?,
                    server_vault_id: VaultId::new(remote_vault.vault_id.to_string())
                        .map_err(|e| e.to_string())?,
                },
            );
            reg_store
                .register_vault_context(&vault_display_name, &context)
                .await
                .map_err(|e| format!("Registering remote vault context failed: {e}"))?;

            Ok::<_, String>((
                remote_vault.vault_id.to_string(),
                vault_display_name,
                server_url,
            ))
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(wizard) = &mut app.setup_wizard_modal {
                    wizard.is_busy = false;
                    match result {
                        Ok((vid, vname, url)) => {
                            wizard.state.configured_vaults.push(ConfiguredVaultSummary {
                                id: vid,
                                name: vname,
                                is_default: false,
                                is_remote: true,
                                sync_enabled: false,
                                server_url: Some(url),
                            });
                            let _ = wizard.state.save(&wizard.base_dir);
                            wizard.status_message =
                                Some("Shellumi Server vault connected successfully!".to_string());
                        }
                        Err(e) => {
                            wizard.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Finalizes setup after verification, marks state completed, and opens Shellumi.
    pub(crate) fn wizard_finish(&mut self, cx: &mut Context<Self>) {
        let key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let Some(wizard) = &mut self.setup_wizard_modal else {
            return;
        };

        let report = verify_setup_integrity(&wizard.base_dir, key.as_ref());
        if !report.is_success() {
            wizard.error = Some(format!(
                "Pre-finish integrity check failed: {}",
                report.errors.join("; ")
            ));
            wizard.verification_report = Some(report);
            cx.notify();
            return;
        }

        // Apply Google Drive sync opt-in for default vault if selected
        if wizard.sync_default_opt_in {
            let base_dir = wizard.base_dir.clone();
            let def_key = key.clone();
            cx.background_executor()
                .spawn(async move {
                    let vaults_dir = base_dir.join("vaults");
                    let db_path = vaults_dir.join("default.db");
                    if let Ok(mut store) =
                        HostStore::open_with_key(db_path.to_str().unwrap(), def_key.as_ref()).await
                    {
                        let _ = store
                            .set_vault_sync_enabled(shellumi_storage::LEGACY_LOCAL_VAULT_ID, true)
                            .await;
                    }
                })
                .detach();
        }

        // Mark setup completed permanently
        let base_dir = wizard.base_dir.clone();
        if let Err(e) = wizard.state.mark_completed(&base_dir) {
            wizard.error = Some(format!("Failed to persist setup completion: {e}"));
            cx.notify();
            return;
        }

        // Dismiss wizard modal and boot Shellumi with active vault
        self.active_vault_id = wizard.state.active_vault_id.clone();
        self.setup_wizard_modal = None;

        self.load_vault_list(cx);
        self.start_storage(false, cx);
        cx.notify();
    }
}

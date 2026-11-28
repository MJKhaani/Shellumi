//! Production onboarding controller & background workflows for `SetupWizardView`.
//!
//! All expensive operations (Argon2id KDF, Turso DB open/migration/discard, OAuth loopback,
//! Google Drive API, Shellumi Server enrollment) run off the GPUI foreground thread,
//! gated by `OnboardingController` single-flight operation IDs and session generations.

use crate::data_path;
use crate::migration_worker::{run_supervised_migration_worker, WorkerRequest};
use crate::ui::setup_wizard_window::SetupWizardView;
use gpui::{App, Context, SharedString};
use shellumi_core::onboarding::{
    resolve_onboarding_route, InstallationInventory, OnboardingController, OnboardingOperationId,
    OnboardingRoute, OperationPhase, RouteResolution, UserFacingError, WizardPage,
};
use shellumi_core::server::{CreateVaultRequest, LoginRequest, ShellumiServerClient};
use shellumi_core::setup::{verify_setup_integrity, ConfiguredVaultSummary, SetupState, SetupStep};
use shellumi_core::sync::{
    build_google_drive_auth_url, generate_pkce_challenge, resolve_google_oauth_config,
    GoogleDriveSyncClient, OAuthCallbackListener, OAuthTokenBundle, SystemHttpTransport,
};
use shellumi_core::{
    ServerConnectionId, VaultContext, VaultId, VaultKey, VaultKeyManager, VaultLocation,
    VaultManifest,
};
use shellumi_storage::{
    DiscardConfirmation, HostStore, LegacyDiscardEngine, MigrationEngine, MigrationProgressEvent,
    RestoreVaultOptions, VaultServerConnectionRecord, LEGACY_LOCAL_VAULT_ID,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// Performs a read-only onboarding inventory & route resolution for `base_dir`.
pub fn inspect_onboarding(
    base_dir: &Path,
    explicit_reentry: bool,
) -> (InstallationInventory, RouteResolution) {
    let migration = MigrationEngine::inspect_state(base_dir);
    let discard = LegacyDiscardEngine::inspect_state(base_dir);
    let inventory = InstallationInventory::inspect_filesystem(base_dir, migration, discard);
    let resolution = resolve_onboarding_route(&inventory, explicit_reentry);
    (inventory, resolution)
}

/// Opens a URL in the system default browser without shell interpolation or hidden console windows.
pub(crate) fn open_system_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = shellumi_core::process::background_command("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not launch default browser: {e}"))
}

impl SetupWizardView {
    pub(crate) fn refresh_inventory(&mut self, cx: &mut Context<Self>) {
        let (inventory, resolution) = inspect_onboarding(&self.base_dir, self.explicit_reentry);
        self.inventory = inventory;
        self.resolution = resolution;
        cx.notify();
    }

    pub(crate) fn prepare_discard_confirmation(&mut self, cx: &mut Context<Self>) {
        self.discard_expected_fingerprints =
            LegacyDiscardEngine::compute_source_fingerprints(&self.base_dir).ok();
        self.controller.reset_discard_confirmation();
        self.discard_backup_path = None;
        cx.notify();
    }

    pub(crate) fn cancel_active_operation(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel_flag) = &self.oauth_cancel_flag {
            cancel_flag.store(true, Ordering::Release);
        }
        if self.controller.cancel_active_operation().is_some() {
            self.google_auth_url = None;
            self.oauth_cancel_flag = None;
            tracing::info!("User cancelled active onboarding operation");
            cx.notify();
        }
    }

    pub fn start_fresh_vault_creation(&mut self, cx: &mut Context<Self>) {
        let pwd = self.default_password_input.read(cx).value().to_string();
        let confirm = self
            .default_password_confirm_input
            .read(cx)
            .value()
            .to_string();
        if pwd.is_empty() {
            self.set_local_validation_error("Vault password cannot be empty.", cx);
            return;
        }
        if pwd != confirm {
            self.set_local_validation_error("Passwords do not match.", cx);
            return;
        }
        if let Err(err) = shellumi_core::validate_vault_password(&pwd) {
            self.set_local_validation_error(&format!("Password requirements not met: {err}"), cx);
            return;
        }

        if self.inventory.has_valid_default_vault() {
            self.set_local_validation_error(
                "An active Default Vault already exists. Unlock it instead of creating a new one.",
                cx,
            );
            return;
        }

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::CreatingVault, false)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        let remember = self.remember_in_keychain;

        let task = cx.background_executor().spawn(async move {
            let vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).map_err(|e| e.to_string())?;
            let manifests_dir = base_dir.join("vault_manifests");
            let vaults_dir = base_dir.join("vaults");
            data_path::prepare_parent(&manifests_dir.join("placeholder"))?;
            data_path::prepare_parent(&vaults_dir.join("placeholder"))?;

            let manifest_path = manifests_dir.join(format!("{}.manifest.json", vid.as_str()));
            let db_path = vaults_dir.join("default.db");
            if db_path.exists() || manifest_path.exists() {
                return Err(
                    "Default vault database or manifest already exists; creation aborted."
                        .to_string(),
                );
            }

            // Transaction-owned staging directory with single UUID
            let tx_id = Uuid::new_v4().to_string();
            let tx_dir = vaults_dir.join(".creation_staging").join(&tx_id);
            std::fs::create_dir_all(&tx_dir)
                .map_err(|e| format!("Failed to create staging directory: {e}"))?;

            let cred_store = shellumi_core::default_credential_store();
            let recovery_code = VaultKey::generate_recovery_code().map_err(|e| e.to_string())?;
            let (key, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
                cred_store.as_ref(),
                &vid,
                &pwd,
                remember,
                Some(&recovery_code),
            )
            .map_err(|e| {
                let _ = std::fs::remove_dir_all(&tx_dir);
                e.to_string()
            })?;

            let staged_manifest_path = tx_dir.join("default.manifest.json");
            let manifest = VaultManifest::new(
                vid.as_str().to_string(),
                "Default Vault".to_string(),
                primary_env.clone(),
                recovery_env,
                remember,
            );
            manifest
                .save_new_to_path(&staged_manifest_path)
                .map_err(|e| {
                    let _ = std::fs::remove_dir_all(&tx_dir);
                    format!("Failed to write staged manifest: {e}")
                })?;

            let staged_db_path = tx_dir.join("default.db");
            let staged_str = staged_db_path
                .to_str()
                .ok_or_else(|| "Invalid UTF-8 database path".to_string())?;

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime init failed: {e}"))?;

            let init_res: Result<(), String> = runtime.block_on(async {
                let mut store = HostStore::open_with_key(staged_str, Some(&key))
                    .await
                    .map_err(|e| format!("Failed to initialize encrypted database: {e}"))?;
                store.set_vault_id(vid.clone());
                let context = VaultContext::new(vid.clone(), VaultLocation::Local);
                store
                    .register_vault_context("Default Vault", &context)
                    .await
                    .map_err(|e| format!("Failed to register vault context: {e}"))?;
                if let Ok(env_json) = serde_json::to_string(&primary_env) {
                    store
                        .save_vault_recovery_envelope(vid.as_str(), &env_json)
                        .await
                        .map_err(|e| format!("Failed to save recovery envelope: {e}"))?;
                }
                // Checkpoint WAL and close handle before publishing self-contained DB file
                let _ = store.checkpoint_wal().await;
                drop(store);

                // Reopen with both password-derived and recovery-derived keys to verify
                let loaded_m = VaultManifest::load_from_path(&staged_manifest_path)
                    .map_err(|e| format!("Staged manifest reload failed: {e}"))?;
                let k_pwd = loaded_m
                    .unlock(&pwd)
                    .map_err(|_| "Staged manifest password unwrap failed".to_string())?;
                let rec_env = loaded_m
                    .recovery_envelope
                    .as_ref()
                    .ok_or_else(|| "Staged manifest missing recovery envelope".to_string())?;
                let k_rec = VaultKey::recover(&vid, &recovery_code, rec_env)
                    .map_err(|_| "Staged manifest recovery unwrap failed".to_string())?;
                if k_pwd.expose_bytes() != k_rec.expose_bytes() {
                    return Err("Password and recovery keys disagree".to_string());
                }

                let reopened = HostStore::open_existing_with_key(staged_str, Some(&k_pwd))
                    .await
                    .map_err(|e| format!("Encrypted verification reopen failed: {e}"))?;
                let _ = reopened
                    .build_vault_directory()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            });

            if let Err(e) = init_res {
                let _ = std::fs::remove_dir_all(&tx_dir);
                return Err(e);
            }

            if let Err(e) = crate::actions::setup_wizard::publish_database_create_only(
                &staged_manifest_path,
                &manifest_path,
            ) {
                let _ = std::fs::remove_dir_all(&tx_dir);
                return Err(e);
            }
            if let Err(e) = crate::actions::setup_wizard::publish_database_create_only(
                &staged_db_path,
                &db_path,
            ) {
                // Only remove manifest if we created it (db publish failed)
                let _ = std::fs::remove_file(&manifest_path);
                let _ = std::fs::remove_dir_all(&tx_dir);
                return Err(e);
            }
            let _ = std::fs::remove_dir_all(&tx_dir);

            Ok::<_, String>((recovery_code, key))
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok((recovery_code, key)) => {
                        let remember_val = this.remember_in_keychain;
                        let checkpoint_res = this.persist_checkpoint(|s| {
                            s.default_vault_created = true;
                            s.remember_in_keychain = remember_val;
                            s.current_step = SetupStep::DefaultVault;
                            if !s
                                .configured_vaults
                                .iter()
                                .any(|v| v.id == LEGACY_LOCAL_VAULT_ID)
                            {
                                s.configured_vaults.push(ConfiguredVaultSummary {
                                    id: LEGACY_LOCAL_VAULT_ID.to_string(),
                                    name: "Default Vault".to_string(),
                                    is_default: true,
                                    is_remote: false,
                                    sync_enabled: false,
                                    server_url: None,
                                });
                            }
                        });
                        if let Err(cp_err) = checkpoint_res {
                            this.controller.finish_operation_err(
                                gen,
                                op_id,
                                UserFacingError::new(
                                    "Checkpoint write failed",
                                    cp_err,
                                    "E_CHECKPOINT",
                                ),
                                true,
                            );
                            cx.notify();
                            return;
                        }
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            "Default vault created and verified.",
                        ) {
                            this.active_vault_keys
                                .insert(LEGACY_LOCAL_VAULT_ID.to_string(), key);
                            this.default_recovery_code = Some(recovery_code);
                            this.controller.default_vault_unlocked = true;
                            this.controller.recovery_confirmed = false;
                            this.refresh_inventory(cx);
                            this.controller.current_page = WizardPage::RecoveryCode;
                        }
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Vault creation failed", e, "E_VAULT_CREATE"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_legacy_migration(&mut self, cx: &mut Context<Self>) {
        let new_pwd = self.default_password_input.read(cx).value().to_string();
        let new_confirm = self
            .default_password_confirm_input
            .read(cx)
            .value()
            .to_string();
        let legacy_pwd = self.legacy_password_input.read(cx).value().to_string();

        if new_pwd.is_empty() {
            self.set_local_validation_error(
                "Please choose a strong password for your migrated Default Vault.",
                cx,
            );
            return;
        }
        if new_pwd != new_confirm {
            self.set_local_validation_error("New vault passwords do not match.", cx);
            return;
        }
        if let Err(err) = shellumi_core::validate_vault_password(&new_pwd) {
            self.set_local_validation_error(
                &format!("New password requirements not met: {err}"),
                cx,
            );
            return;
        }

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::BackingUp, false)
        else {
            return;
        };
        self.controller.current_page = WizardPage::MigrationProgress;
        self.migration_progress_label = "Starting isolated migration worker…".to_string();
        cx.notify();

        let base_dir = self.base_dir.clone();
        let remember = self.remember_in_keychain;

        // Generate recovery code once per migration attempt (and verify against staged manifest if resuming)
        let recovery_code = match VaultKey::generate_recovery_code() {
            Ok(c) => c,
            Err(e) => {
                self.controller.finish_operation_err(
                    gen,
                    op_id,
                    UserFacingError::new("Key generation error", e.to_string(), "E_KDF"),
                    true,
                );
                cx.notify();
                return;
            }
        };

        let (prog_tx, prog_rx) = async_channel::bounded::<MigrationProgressEvent>(32);

        let rec_code_worker = recovery_code.clone();
        let new_pwd_worker = new_pwd.clone();
        let base_dir_worker = base_dir.clone();
        let task = cx.background_executor().spawn(async move {
            let req = WorkerRequest {
                default_password: new_pwd_worker,
                default_recovery_code: rec_code_worker,
                source_password: if legacy_pwd.is_empty() {
                    None
                } else {
                    Some(legacy_pwd)
                },
                remember_in_keychain: remember,
            };
            run_supervised_migration_worker(&base_dir_worker, &req, &mut |ev| {
                let _ = prog_tx.try_send(ev);
            })
        });

        // Bounded UI progress pump
        cx.spawn(async move |view, cx| {
            while let Ok(ev) = prog_rx.recv().await {
                let _ = view.update(cx, |this, cx| {
                    let (phase, label) = match &ev {
                        MigrationProgressEvent::Inspection => (
                            OperationPhase::Preparing,
                            "Inspecting source database and staging…".to_string(),
                        ),
                        MigrationProgressEvent::Backup => (
                            OperationPhase::BackingUp,
                            "Creating verified source backup…".to_string(),
                        ),
                        MigrationProgressEvent::SourceOpen => (
                            OperationPhase::ReadingAndPlanning,
                            "Opening legacy database copy…".to_string(),
                        ),
                        MigrationProgressEvent::SchemaRead => (
                            OperationPhase::ReadingAndPlanning,
                            "Reading legacy schema…".to_string(),
                        ),
                        MigrationProgressEvent::Plan { vault_count } => (
                            OperationPhase::ReadingAndPlanning,
                            format!("Planned migration for {vault_count} vault(s)…"),
                        ),
                        MigrationProgressEvent::Copy {
                            vault_id,
                            done,
                            total,
                            ..
                        } => (
                            OperationPhase::CopyingAndEncrypting,
                            format!("Copying & encrypting vault '{vault_id}' ({done}/{total})…"),
                        ),
                        MigrationProgressEvent::Encryption { vault_id } => (
                            OperationPhase::CopyingAndEncrypting,
                            format!("Sealing encrypted vault '{vault_id}'…"),
                        ),
                        MigrationProgressEvent::Verification { vault_id } => (
                            OperationPhase::Verifying,
                            format!("Verifying encrypted records for '{vault_id}'…"),
                        ),
                        MigrationProgressEvent::Activation => (
                            OperationPhase::Activating,
                            "Activating verified vaults (create-only)…".to_string(),
                        ),
                        MigrationProgressEvent::Completion => (
                            OperationPhase::Finalizing,
                            "Migration complete; verifying final state…".to_string(),
                        ),
                    };
                    if this
                        .controller
                        .update_operation_progress(gen, op_id, phase, false, None)
                    {
                        this.migration_progress_label = label;
                        cx.notify();
                    }
                });
            }
        })
        .detach();

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok((report, recovery_matches)) => {
                        if !recovery_matches {
                            this.refresh_inventory(cx);
                            this.controller.finish_operation_err(
                                gen,
                                op_id,
                                UserFacingError::new(
                                    "Recovery code verification failed",
                                    "Migrated vault manifest could not be unlocked with the generated recovery code; reconcile recovery credentials before continuing.",
                                    "E_MIGRATION_RECOVERY",
                                ),
                                true,
                            );
                            cx.notify();
                            return;
                        }
                        // Unlock verified manifest to hold active DEK in memory
                        let mpath = this
                            .base_dir
                            .join("vault_manifests/legacy-local.manifest.json");
                        let key_res = VaultManifest::load_from_path(&mpath)
                            .map_err(|e| e.to_string())
                            .and_then(|m| m.unlock(&new_pwd).map_err(|_| "Password unwrap failed".to_string()));
                        let key = match key_res {
                            Ok(k) => k,
                            Err(e) => {
                                this.controller.finish_operation_err(
                                    gen,
                                    op_id,
                                    UserFacingError::new("Post-migration verification failed", e, "E_MIGRATION_VERIFY"),
                                    true,
                                );
                                cx.notify();
                                return;
                            }
                        };

                        let remember_val = this.remember_in_keychain;
                        let checkpoint_res = this.persist_checkpoint(|s| {
                            s.default_vault_created = true;
                            s.remember_in_keychain = remember_val;
                            s.current_step = SetupStep::DefaultVault;
                            if !s
                                .configured_vaults
                                .iter()
                                .any(|v| v.id == LEGACY_LOCAL_VAULT_ID)
                            {
                                s.configured_vaults.push(ConfiguredVaultSummary {
                                    id: LEGACY_LOCAL_VAULT_ID.to_string(),
                                    name: "Default Vault (Migrated)".to_string(),
                                    is_default: true,
                                    is_remote: false,
                                    sync_enabled: false,
                                    server_url: None,
                                });
                            }
                        });
                        if let Err(cp_err) = checkpoint_res {
                            this.controller.finish_operation_err(
                                gen,
                                op_id,
                                UserFacingError::new("Checkpoint write failed", cp_err, "E_CHECKPOINT"),
                                true,
                            );
                            cx.notify();
                            return;
                        }

                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!(
                                "Migrated {} vault(s) ({} default records) and verified encryption.",
                                report.vaults_migrated, report.default_records_migrated
                            ),
                        ) {
                            this.active_vault_keys
                                .insert(LEGACY_LOCAL_VAULT_ID.to_string(), key);
                            // Only present the recovery code if it actually unwraps the committed envelope
                            this.default_recovery_code = if recovery_matches {
                                Some(recovery_code)
                            } else {
                                None
                            };
                            this.controller.default_vault_unlocked = true;
                            this.controller.recovery_confirmed = false;
                            this.refresh_inventory(cx);
                            this.controller.current_page = if recovery_matches {
                                WizardPage::RecoveryCode
                            } else {
                                WizardPage::GoogleDrive
                            };
                        }
                    }
                    Err(e) => {
                        this.refresh_inventory(cx);
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Migration stopped safely", e, "E_MIGRATION"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_unlock_existing_vault(&mut self, cx: &mut Context<Self>) {
        let secret = self.existing_unlock_input.read(cx).value().to_string();
        if secret.is_empty() {
            self.set_local_validation_error(
                if self.unlock_with_recovery_code {
                    "Please enter your 8-word recovery code."
                } else {
                    "Please enter your vault password."
                },
                cx,
            );
            return;
        }

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::Verifying, false)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        let is_recovery = self.unlock_with_recovery_code;

        let task = cx.background_executor().spawn(async move {
            let vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).map_err(|e| e.to_string())?;
            let candidates = [
                base_dir.join("vault_manifests/legacy-local.manifest.json"),
                base_dir.join("vault_manifests/default.manifest.json"),
                base_dir.join("manifests/legacy-local.manifest.json"),
                base_dir.join("manifests/default.manifest.json"),
            ];
            let mpath = candidates
                .into_iter()
                .find(|p| p.exists())
                .ok_or_else(|| "Default vault manifest not found.".to_string())?;

            let manifest = VaultManifest::load_from_path(&mpath)
                .map_err(|e| format!("Failed to read manifest: {e}"))?;

            let key = if is_recovery {
                let rec = manifest
                    .recovery_envelope
                    .ok_or_else(|| "This vault manifest has no recovery envelope.".to_string())?;
                VaultKey::recover(&vid, &secret, &rec)
                    .map_err(|_| "Invalid recovery code for default vault.".to_string())?
            } else {
                VaultKey::unwrap_with_password(&vid, &secret, &manifest.primary_envelope)
                    .map_err(|_| "Incorrect password for default vault.".to_string())?
            };

            let db_path = base_dir.join("vaults/default.db");
            let db_str = db_path
                .to_str()
                .ok_or_else(|| "Invalid UTF-8 database path".to_string())?;

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime init failed: {e}"))?;

            let dir = runtime.block_on(async {
                let store = HostStore::open_existing_with_key(db_str, Some(&key))
                    .await
                    .map_err(|e| format!("Failed to open encrypted database: {e}"))?;
                store
                    .build_vault_directory()
                    .await
                    .map_err(|e| format!("Failed to read vault directory: {e}"))
            })?;

            Ok::<_, String>((key, dir))
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok((key, dir)) => {
                        let already_confirmed = matches!(
                            SetupState::load(&this.base_dir),
                            Ok(Some(s)) if s.recovery_code_confirmed
                                || s.confirmed_recovery_vaults.iter().any(|v| v == LEGACY_LOCAL_VAULT_ID)
                        );
                        let cp_res = this.persist_checkpoint(|s| {
                            s.default_vault_created = true;
                            s.configured_vaults = dir
                                .entries
                                .iter()
                                .map(|e| ConfiguredVaultSummary {
                                    id: e.id.clone(),
                                    name: e.name.clone(),
                                    is_default: e.id == LEGACY_LOCAL_VAULT_ID,
                                    is_remote: matches!(
                                        e.location,
                                        shellumi_core::VaultDirectoryLocation::Remote { .. }
                                    ),
                                    sync_enabled: e.sync_enabled,
                                    server_url: None,
                                })
                                .collect();
                            s.current_step = SetupStep::GoogleDrive;
                        });
                        if let Err(cp_err) = cp_res {
                            this.controller.finish_operation_err(
                                gen,
                                op_id,
                                UserFacingError::new("Checkpoint write failed", cp_err, "E_CHECKPOINT"),
                                true,
                            );
                            cx.notify();
                            return;
                        }
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            "Default vault unlocked and verified.",
                        ) {
                            this.active_vault_keys
                                .insert(LEGACY_LOCAL_VAULT_ID.to_string(), key);
                            this.controller.default_vault_unlocked = true;
                            this.controller.recovery_confirmed = already_confirmed;
                            this.controller.current_page = WizardPage::GoogleDrive;
                        }
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Unlock failed", e, "E_UNLOCK"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_legacy_backup_picker(&mut self, cx: &mut Context<Self>) {
        if self.controller.operation.is_running() {
            return;
        }
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from("Select Backup Folder")),
        });
        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::BackingUp, true)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        cx.spawn(async move |view, cx| {
            let picked = receiver.await;
            let result = match picked {
                Ok(Ok(Some(paths))) if !paths.is_empty() => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let dest_dir = paths[0].join(format!("Shellumi-Legacy-Backup-{ts}"));
                    LegacyDiscardEngine::create_backup(&base_dir, &dest_dir)
                        .map_err(|e| e.to_string())
                }
                Ok(Ok(_)) => Err("Backup folder selection was cancelled.".to_string()),
                Ok(Err(e)) => Err(format!("Folder picker error: {e}")),
                Err(_) => Err("Folder picker closed.".to_string()),
            };

            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(report) => {
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!(
                                "Verified backup saved to {} ({} files).",
                                report.backup_dir.display(),
                                report.files_copied.len()
                            ),
                        ) {
                            this.discard_backup_path = Some(report.backup_dir);
                            this.controller.discard_backup_completed = true;
                            this.discard_expected_fingerprints =
                                LegacyDiscardEngine::compute_source_fingerprints(&this.base_dir)
                                    .ok();
                        }
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Backup not completed", e, "E_BACKUP"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_legacy_discard(&mut self, cx: &mut Context<Self>) {
        if !self.controller.can_confirm_legacy_discard() {
            self.set_local_validation_error(
                "Please acknowledge permanent removal and type DELETE to continue.",
                cx,
            );
            return;
        }

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::Quarantining, false)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        let expected_fps = self
            .discard_expected_fingerprints
            .clone()
            .or_else(|| LegacyDiscardEngine::compute_source_fingerprints(&base_dir).ok());
        let conf = DiscardConfirmation {
            acknowledged: self.controller.discard_acknowledged,
            token: self.controller.discard_token_input.clone(),
            expected_source_fingerprints: expected_fps,
        };

        let task = cx.background_executor().spawn(async move {
            let cred_store = shellumi_core::default_credential_store();
            LegacyDiscardEngine::execute_discard(&base_dir, &conf, cred_store.as_ref())
                .map_err(|e| e.to_string())
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(report) => {
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!(
                                "Removed {} legacy database file(s). Ready to create a new vault.",
                                report.removed_files.len()
                            ),
                        ) {
                            this.controller.reset_discard_confirmation();
                            this.discard_expected_fingerprints = None;
                            this.controller.route = OnboardingRoute::Fresh;
                            this.refresh_inventory(cx);
                            this.controller.current_page = WizardPage::VaultPassword;
                        }
                    }
                    Err(e) => {
                        if e.contains("changed since confirmation") {
                            this.prepare_discard_confirmation(cx);
                        }
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Database removal failed", e, "E_DISCARD"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_resume_or_rollback_discard(&mut self, cx: &mut Context<Self>) {
        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::Purging, false)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        let task = cx.background_executor().spawn(async move {
            let cred_store = shellumi_core::default_credential_store();
            LegacyDiscardEngine::resume_or_rollback(&base_dir, cred_store.as_ref())
                .map_err(|e| e.to_string())
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(outcome) => {
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!("Resolved interrupted removal: {outcome:?}"),
                        ) {
                            this.refresh_inventory(cx);
                            let next = OnboardingController::from_resolution(&this.resolution);
                            this.controller.route = next.route;
                            this.controller.current_page = next.current_page;
                        }
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Discard recovery failed", e, "E_DISCARD_RESUME"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_rollback_interrupted_migration(&mut self, cx: &mut Context<Self>) {
        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::RollingBack, false)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        let task = cx.background_executor().spawn(async move {
            MigrationEngine::rollback_if_uncommitted(&base_dir).map_err(|e| e.to_string())
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            "Rolled back uncommitted migration staging; original database preserved.",
                        ) {
                            this.refresh_inventory(cx);
                            let next = OnboardingController::from_resolution(&this.resolution);
                            this.controller.route = next.route;
                            this.controller.current_page = next.current_page;
                        }
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Rollback failed", e, "E_MIGRATION_ROLLBACK"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_google_sign_in(&mut self, cx: &mut Context<Self>) {
        let adv_id = self.google_client_id_input.read(cx).value().to_string();
        let adv_secret = self.google_client_secret_input.read(cx).value().to_string();
        let cred_store = shellumi_core::default_credential_store();

        let resolved = resolve_google_oauth_config(
            cred_store.as_ref(),
            if adv_id.trim().is_empty() {
                None
            } else {
                Some((adv_id.as_str(), Some(adv_secret.as_str())))
            },
        );

        let config = match resolved {
            Ok(Some(cfg)) => cfg,
            Ok(None) => {
                self.set_local_validation_error(
                    "Google Drive OAuth is not configured in this build. Expand Advanced OAuth Configuration to enter a Client ID, or skip Google Drive for local-first setup.",
                    cx,
                );
                return;
            }
            Err(e) => {
                self.set_local_validation_error(&e.to_string(), cx);
                return;
            }
        };

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::WaitingForGoogleSignIn, true)
        else {
            return;
        };

        let listener = match OAuthCallbackListener::bind_ephemeral() {
            Ok(l) => l,
            Err(e) => {
                self.controller.finish_operation_err(
                    gen,
                    op_id,
                    UserFacingError::new(
                        "Unable to start local OAuth callback",
                        e.to_string(),
                        "E_OAUTH_BIND",
                    ),
                    true,
                );
                cx.notify();
                return;
            }
        };
        let redirect_uri = listener.redirect_uri();
        let pkce = match generate_pkce_challenge() {
            Ok(p) => p,
            Err(e) => {
                self.controller.finish_operation_err(
                    gen,
                    op_id,
                    UserFacingError::new("PKCE error", e.to_string(), "E_OAUTH_PKCE"),
                    true,
                );
                cx.notify();
                return;
            }
        };
        let state = Uuid::new_v4().to_string();
        let auth_url = build_google_drive_auth_url(&config.client_id, &redirect_uri, &pkce, &state);

        // Immediately expose URL for manual "Copy sign-in link" fallback (never logged).
        // Keep listener live even if browser launch fails so the copied URL still completes OAuth!
        self.google_auth_url = Some(auth_url.clone());
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.oauth_cancel_flag = Some(cancel_flag.clone());
        self.browser_launch_warning = open_system_browser(&auth_url).err();
        cx.notify();

        let cancel_for_worker = cancel_flag.clone();
        let task = cx.background_executor().spawn(async move {
            let code = listener
                .wait_for_code_cancellable(
                    &state,
                    std::time::Duration::from_secs(120),
                    &cancel_for_worker,
                )
                .map_err(|e| format!("Google sign-in callback failed: {e}"))?;

            if cancel_for_worker.load(Ordering::Acquire) {
                return Err("Google sign-in was cancelled.".to_string());
            }

            let client =
                GoogleDriveSyncClient::new(SystemHttpTransport::default(), &config.client_id)
                    .with_client_secret(config.client_secret.clone());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut bundle = client
                .exchange_code(&code, &pkce.code_verifier, &redirect_uri, now)
                .map_err(|e| format!("Token exchange failed: {e}"))?;
            bundle.client_id = Some(config.client_id.clone());

            if cancel_for_worker.load(Ordering::Acquire) {
                return Err("Google sign-in was cancelled before saving credentials.".to_string());
            }

            let store = shellumi_core::default_credential_store();
            bundle
                .save_to_store(store.as_ref())
                .map_err(|e| format!("Failed to save OAuth tokens to OS Keychain: {e}"))?;

            let discovery_result = client
                .discover_cloud_vaults(&bundle.access_token)
                .map_err(|e| format!("Signed in, but cloud vault discovery failed: {e}"));

            Ok::<_, String>(discovery_result)
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                // Guard all UI mutations by active operation identity
                if this.controller.session_generation != gen
                    || this.controller.operation.active_id() != Some(op_id)
                {
                    return;
                }
                this.google_auth_url = None;
                this.oauth_cancel_flag = None;
                this.browser_launch_warning = None;
                match result {
                    Ok(Ok(discovered)) => {
                        this.google_discovery_error = None;
                        let cp_res = this.persist_checkpoint(|s| {
                            s.google_drive_connected = true;
                            s.google_drive_skipped = false;
                        });
                        if let Err(cp_err) = cp_res {
                            this.controller.finish_operation_err(
                                gen,
                                op_id,
                                UserFacingError::new(
                                    "Checkpoint write failed",
                                    cp_err,
                                    "E_CHECKPOINT",
                                ),
                                true,
                            );
                            cx.notify();
                            return;
                        }
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!(
                                "Google Drive connected. Found {} encrypted backup(s).",
                                discovered.len()
                            ),
                        ) {
                            this.cloud_vaults = discovered;
                            if this.controller.mode
                                == shellumi_core::setup::SetupWizardMode::RestoreSetup
                            {
                                this.controller.current_page = WizardPage::CloudRestore;
                            }
                        }
                    }
                    Ok(Err(disc_err)) => {
                        // Signed in, but discovery failed — allow explicit retry without re-auth
                        let _ = this.persist_checkpoint(|s| {
                            s.google_drive_connected = true;
                            s.google_drive_skipped = false;
                        });
                        this.google_discovery_error = Some(disc_err.clone());
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new(
                                "Connected, but backup scan failed",
                                disc_err,
                                "E_DRIVE_DISCOVERY",
                            ),
                            true,
                        );
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Google Drive sign-in failed", e, "E_OAUTH"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn fetch_cloud_vaults(&mut self, cx: &mut Context<Self>) {
        let adv_id = self.google_client_id_input.read(cx).value().to_string();
        let adv_secret = self.google_client_secret_input.read(cx).value().to_string();
        let cred_store = shellumi_core::default_credential_store();
        let config = match resolve_google_oauth_config(
            cred_store.as_ref(),
            if adv_id.trim().is_empty() {
                None
            } else {
                Some((adv_id.as_str(), Some(adv_secret.as_str())))
            },
        ) {
            Ok(Some(c)) => c,
            Ok(None) => {
                self.set_local_validation_error("Google OAuth Client ID is not configured.", cx);
                return;
            }
            Err(e) => {
                self.set_local_validation_error(&e.to_string(), cx);
                return;
            }
        };

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::DiscoveringCloudVaults, false)
        else {
            return;
        };
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let store = shellumi_core::default_credential_store();
            let client =
                GoogleDriveSyncClient::new(SystemHttpTransport::default(), &config.client_id)
                    .with_client_secret(config.client_secret);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let token = OAuthTokenBundle::resolve_valid_token(store.as_ref(), &client, now)
                .map_err(|e| format!("Google Drive authentication required: {e}"))?;
            client
                .discover_cloud_vaults(&token)
                .map_err(|e| format!("Failed to list cloud backups: {e}"))
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(vaults) => {
                        this.google_discovery_error = None;
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!("Found {} encrypted cloud backup(s).", vaults.len()),
                        ) {
                            this.cloud_vaults = vaults;
                        }
                    }
                    Err(e) => {
                        this.google_discovery_error = Some(e.clone());
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Cloud scan failed", e, "E_DRIVE_DISCOVERY"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn restore_selected_cloud_vault(
        &mut self,
        vault_id_str: String,
        cx: &mut Context<Self>,
    ) {
        let password = self.cloud_vault_password_input.read(cx).value().to_string();
        if password.is_empty() {
            self.set_local_validation_error(
                &format!("Enter the password for cloud vault '{vault_id_str}'."),
                cx,
            );
            return;
        }
        let is_default = vault_id_str == LEGACY_LOCAL_VAULT_ID;
        let db_path = if is_default {
            self.base_dir.join("vaults/default.db")
        } else {
            self.base_dir.join(format!("vaults/{vault_id_str}.db"))
        };
        let manifest_path = self
            .base_dir
            .join(format!("vault_manifests/{vault_id_str}.manifest.json"));
        if db_path.exists() || manifest_path.exists() {
            self.set_local_validation_error(
                &format!("Vault '{vault_id_str}' already exists locally. Overwrite rejected."),
                cx,
            );
            return;
        }

        let adv_id = self.google_client_id_input.read(cx).value().to_string();
        let adv_secret = self.google_client_secret_input.read(cx).value().to_string();
        let cred_store = shellumi_core::default_credential_store();
        let config = match resolve_google_oauth_config(
            cred_store.as_ref(),
            if adv_id.trim().is_empty() {
                None
            } else {
                Some((adv_id.as_str(), Some(adv_secret.as_str())))
            },
        ) {
            Ok(Some(c)) => c,
            Ok(None) => {
                self.set_local_validation_error("Google OAuth Client ID is not configured.", cx);
                return;
            }
            Err(e) => {
                self.set_local_validation_error(&e.to_string(), cx);
                return;
            }
        };

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::RestoringCloudVault, false)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        let remember = self.remember_in_keychain;

        let task = cx.background_executor().spawn(async move {
            let vid = VaultId::new(&vault_id_str).map_err(|e| e.to_string())?;
            let store = shellumi_core::default_credential_store();
            let client =
                GoogleDriveSyncClient::new(SystemHttpTransport::default(), &config.client_id)
                    .with_client_secret(config.client_secret);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let access_token = OAuthTokenBundle::resolve_valid_token(store.as_ref(), &client, now)
                .map_err(|e| format!("Google Drive authentication error: {e}"))?;

            let snapshot = client
                .download_current_vault(&access_token, vid.as_str(), &password)
                .map_err(|e| format!("Failed to download cloud snapshot: {e}"))?;

            let manifests_dir = base_dir.join("vault_manifests");
            let vaults_dir = base_dir.join("vaults");
            data_path::prepare_parent(&manifests_dir.join("placeholder"))?;
            data_path::prepare_parent(&vaults_dir.join("placeholder"))?;

            let options = RestoreVaultOptions {
                vault_id: &vid,
                vault_name: if is_default {
                    "Default Vault"
                } else {
                    &vault_id_str
                },
                password: &password,
                target_db_path: &db_path,
                manifests_dir: &manifests_dir,
                cred_store: if remember { Some(store.as_ref()) } else { None },
                remember_in_keychain: remember,
            };

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime init failed: {e}"))?;
            runtime.block_on(async {
                HostStore::restore_vault_from_sync(&snapshot, &options)
                    .await
                    .map_err(|e| format!("Restore failed: {e}"))?;
                Ok::<_, String>(())
            })?;

            let envelope = snapshot
                .key_envelope
                .as_ref()
                .ok_or_else(|| "Cloud snapshot missing key envelope".to_string())?;
            let key = VaultKey::unwrap_with_password(&vid, &password, envelope)
                .map_err(|e| format!("DEK unwrap failed: {e}"))?;

            let rec_code = VaultKey::generate_recovery_code().map_err(|e| e.to_string())?;
            let mut manifest = VaultManifest::load_from_path(&manifest_path)
                .map_err(|e| format!("Load restored manifest: {e}"))?;
            manifest
                .set_recovery_key(&key, &rec_code)
                .map_err(|e| format!("Set recovery key: {e}"))?;
            manifest
                .save_to_path(&manifest_path)
                .map_err(|e| format!("Save restored manifest: {e}"))?;

            Ok::<_, String>((vault_id_str, is_default, key, rec_code))
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok((vid, is_default, key, rec_code)) => {
                        let cp_res = this.persist_checkpoint(|s| {
                            if is_default {
                                s.default_vault_created = true;
                            }
                            if !s.configured_vaults.iter().any(|v| v.id == vid) {
                                s.configured_vaults.push(ConfiguredVaultSummary {
                                    id: vid.clone(),
                                    name: if is_default {
                                        "Default Vault (Restored)".to_string()
                                    } else {
                                        vid.clone()
                                    },
                                    is_default,
                                    is_remote: false,
                                    sync_enabled: false,
                                    server_url: None,
                                });
                            }
                        });
                        if let Err(cp_err) = cp_res {
                            this.controller.finish_operation_err(
                                gen,
                                op_id,
                                UserFacingError::new(
                                    "Checkpoint write failed",
                                    cp_err,
                                    "E_CHECKPOINT",
                                ),
                                true,
                            );
                            cx.notify();
                            return;
                        }
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!("Vault '{vid}' restored and verified."),
                        ) {
                            this.active_vault_keys.insert(vid, key);
                            if is_default {
                                this.default_recovery_code = Some(rec_code);
                                this.controller.default_vault_unlocked = true;
                                this.controller.recovery_confirmed = false;
                                this.refresh_inventory(cx);
                                this.controller.current_page = WizardPage::RecoveryCode;
                            }
                        }
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Cloud restore failed", e, "E_CLOUD_RESTORE"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn disconnect_google_drive(&mut self, cx: &mut Context<Self>) {
        let store = shellumi_core::default_credential_store();
        if let Err(e) = OAuthTokenBundle::delete_from_store(store.as_ref()) {
            self.set_local_validation_error(&format!("Failed to clear OAuth credentials: {e}"), cx);
            return;
        }
        self.cloud_vaults.clear();
        self.sync_default_opt_in = false;
        self.google_discovery_error = None;
        if let Err(e) = self.persist_checkpoint(|s| {
            s.google_drive_connected = false;
            s.google_drive_sync_enabled = false;
        }) {
            self.set_local_validation_error(&e, cx);
            return;
        }
        cx.notify();
    }

    pub(crate) fn start_create_secondary_vault(&mut self, cx: &mut Context<Self>) {
        let name = self
            .sec_vault_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let pwd = self.sec_vault_password_input.read(cx).value().to_string();
        let confirm = self
            .sec_vault_password_confirm_input
            .read(cx)
            .value()
            .to_string();

        if name.is_empty() {
            self.set_local_validation_error("Secondary vault name cannot be empty.", cx);
            return;
        }
        if pwd != confirm {
            self.set_local_validation_error("Secondary vault passwords do not match.", cx);
            return;
        }
        if let Err(e) = shellumi_core::validate_vault_password(&pwd) {
            self.set_local_validation_error(&format!("Password requirements not met: {e}"), cx);
            return;
        }

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::CreatingVault, false)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        let default_key = self.active_vault_keys.get(LEGACY_LOCAL_VAULT_ID).cloned();
        let remember = self.sec_vault_keychain;

        let task = cx.background_executor().spawn(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let vid_str = format!("vlt-{}-{}", now.as_secs(), now.subsec_nanos());
            let vid = VaultId::new(&vid_str).map_err(|e| e.to_string())?;
            let store = shellumi_core::default_credential_store();
            let rec_code = VaultKey::generate_recovery_code().map_err(|e| e.to_string())?;
            let (key, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
                store.as_ref(),
                &vid,
                &pwd,
                remember,
                Some(&rec_code),
            )
            .map_err(|e| e.to_string())?;

            let manifest_path = base_dir.join(format!("vault_manifests/{vid_str}.manifest.json"));
            let db_path = base_dir.join(format!("vaults/{vid_str}.db"));
            let manifest = VaultManifest::new(
                vid_str.clone(),
                name.clone(),
                primary_env,
                recovery_env,
                remember,
            );
            manifest
                .save_new_to_path(&manifest_path)
                .map_err(|e| e.to_string())?;

            let staged = base_dir.join(format!("vaults/.staged_{}.db", Uuid::new_v4()));
            let staged_str = staged
                .to_str()
                .ok_or_else(|| "Invalid staged path".to_string())?;

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime init failed: {e}"))?;
            runtime.block_on(async {
                let mut sec_store = HostStore::open_with_key(staged_str, Some(&key))
                    .await
                    .map_err(|e| e.to_string())?;
                sec_store.set_vault_id(vid.clone());
                let _ = sec_store.checkpoint_wal().await;
                drop(sec_store);

                crate::actions::setup_wizard::publish_database_create_only(&staged, &db_path)?;

                let reg_path = base_dir.join("vaults/default.db");
                let mut reg = HostStore::open_existing_with_key(
                    reg_path.to_str().unwrap_or_default(),
                    default_key.as_ref(),
                )
                .await
                .map_err(|e| e.to_string())?;
                let ctx = VaultContext::new(vid, VaultLocation::Local);
                reg.register_vault_context(&name, &ctx)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>(())
            })?;

            Ok::<_, String>((vid_str, name, key, rec_code))
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok((vid, vname, key, _rec)) => {
                        let cp_res = this.persist_checkpoint(|s| {
                            if !s.configured_vaults.iter().any(|v| v.id == vid) {
                                s.configured_vaults.push(ConfiguredVaultSummary {
                                    id: vid.clone(),
                                    name: vname.clone(),
                                    is_default: false,
                                    is_remote: false,
                                    sync_enabled: false,
                                    server_url: None,
                                });
                            }
                        });
                        if let Err(cp_err) = cp_res {
                            this.controller.finish_operation_err(
                                gen,
                                op_id,
                                UserFacingError::new(
                                    "Checkpoint write failed",
                                    cp_err,
                                    "E_CHECKPOINT",
                                ),
                                true,
                            );
                            cx.notify();
                            return;
                        }
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!("Secondary vault '{vname}' created and verified."),
                        ) {
                            this.active_vault_keys.insert(vid, key);
                            this.refresh_inventory(cx);
                        }
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new(
                                "Secondary vault creation failed",
                                e,
                                "E_SEC_VAULT",
                            ),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_connect_server(&mut self, cx: &mut Context<Self>) {
        let server_url = self.server_url_input.read(cx).value().trim().to_string();
        let username = self.server_user_input.read(cx).value().trim().to_string();
        let password = self.server_pass_input.read(cx).value().to_string();
        let device_name = self
            .server_device_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();

        if server_url.is_empty() || username.is_empty() || password.is_empty() {
            self.set_local_validation_error("Server URL, username, and password are required.", cx);
            return;
        }

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::ConnectingServer, false)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        let default_key = self.active_vault_keys.get(LEGACY_LOCAL_VAULT_ID).cloned();

        let task = cx.background_executor().spawn(async move {
            let mut client = ShellumiServerClient::new(&server_url);
            let login_resp = client
                .login(&LoginRequest {
                    identifier: username.clone(),
                    password,
                    device_id: None,
                })
                .map_err(|e| format!("Server login failed: {e}"))?;
            client.set_token(login_resp.access_token.clone());

            let vaults = client
                .list_vaults()
                .map_err(|e| format!("Unable to list server vaults: {e}"))?
                .vaults;
            let remote = if let Some(v) = vaults.first() {
                v.clone()
            } else {
                client
                    .create_vault(&CreateVaultRequest {
                        name: Some(format!("{username}'s Vault")),
                        encrypted_name: Some(format!("{username}'s Vault")),
                        vault_type: "personal".to_string(),
                    })
                    .map_err(|e| format!("Failed to create server vault: {e}"))?
            };

            let caps = client
                .capabilities()
                .map_err(|e| format!("Unable to verify server capabilities: {e}"))?;

            let reg_path = base_dir.join("vaults/default.db");
            let cred_store = shellumi_core::default_credential_store();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime init failed: {e}"))?;

            let (local_vid_str, display_name) = runtime.block_on(async {
                let mut reg_store = HostStore::open_existing_with_key(
                    reg_path.to_str().unwrap_or_default(),
                    default_key.as_ref(),
                )
                .await
                .map_err(|e| format!("Failed to open registry: {e}"))?;

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let conn_id = format!("srv-{}", Uuid::new_v4());
                let conn_rec = VaultServerConnectionRecord {
                    id: conn_id.clone(),
                    base_url: server_url.clone(),
                    auth_token: Some(login_resp.access_token),
                    refresh_token: Some(login_resp.refresh_token),
                    device_id: None,
                    device_signing_key: None,
                    server_instance_id: Some(caps.server_instance_id.to_string()),
                    device_agreement_key: None,
                    created_at: now,
                    updated_at: now,
                };
                reg_store
                    .save_vault_server_connection_secured(&conn_rec, cred_store.as_ref())
                    .await
                    .map_err(|e| format!("Saving connection record failed: {e}"))?;

                let local_vid =
                    VaultId::new(format!("v-{}", Uuid::new_v4())).map_err(|e| e.to_string())?;
                let display_name = remote
                    .encrypted_name
                    .clone()
                    .unwrap_or_else(|| format!("{username}'s Vault"));
                let context = VaultContext::new(
                    local_vid.clone(),
                    VaultLocation::Remote {
                        server: ServerConnectionId::new(&conn_id).map_err(|e| e.to_string())?,
                        server_vault_id: VaultId::new(remote.vault_id.to_string())
                            .map_err(|e| e.to_string())?,
                    },
                );
                reg_store
                    .register_vault_context(&display_name, &context)
                    .await
                    .map_err(|e| format!("Registering remote vault failed: {e}"))?;

                let binding = reg_store
                    .resolve_verified_remote_vault_binding(local_vid.as_str(), Some(&conn_id))
                    .await
                    .map_err(|e| e.to_string())?;
                reg_store.set_vault_id(local_vid.clone());
                reg_store
                    .enroll_initial_connection_device(
                        &binding,
                        &mut client,
                        &display_name,
                        &device_name,
                        cred_store.as_ref(),
                    )
                    .await
                    .map_err(|e| e.to_string())?;

                Ok::<_, String>((local_vid.as_str().to_string(), display_name))
            })?;

            Ok::<_, String>((local_vid_str, display_name, server_url))
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok((vid, vname, url)) => {
                        let cp_res = this.persist_checkpoint(|s| {
                            if !s.configured_vaults.iter().any(|v| v.id == vid) {
                                s.configured_vaults.push(ConfiguredVaultSummary {
                                    id: vid.clone(),
                                    name: vname.clone(),
                                    is_default: false,
                                    is_remote: true,
                                    sync_enabled: false,
                                    server_url: Some(url),
                                });
                            }
                        });
                        if let Err(cp_err) = cp_res {
                            this.controller.finish_operation_err(
                                gen,
                                op_id,
                                UserFacingError::new(
                                    "Checkpoint write failed",
                                    cp_err,
                                    "E_CHECKPOINT",
                                ),
                                true,
                            );
                            cx.notify();
                            return;
                        }
                        this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!("Connected Shellumi Server vault '{vname}'."),
                        );
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Server enrollment failed", e, "E_SERVER"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn finish_and_launch(&mut self, cx: &mut Context<Self>) {
        if self.controller.operation.is_running() {
            return;
        }

        let Some((gen, op_id)) = self
            .controller
            .try_start_operation(OperationPhase::Finalizing, false)
        else {
            return;
        };
        cx.notify();

        let base_dir = self.base_dir.clone();
        let in_mem_key = self.active_vault_keys.get(LEGACY_LOCAL_VAULT_ID).cloned();
        let sync_enabled = self.sync_default_opt_in && self.checkpoint_google_connected();

        // Run all verification (KDF/Keychain/integrity/DB readback) off the GPUI foreground thread
        let task = cx.background_executor().spawn(async move {
            let default_key = in_mem_key.or_else(|| {
                let vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).ok()?;
                let store = shellumi_core::default_credential_store();
                VaultKeyManager::retrieve_key(store.as_ref(), &vid)
                    .ok()
                    .flatten()
            });

            let report = verify_setup_integrity(&base_dir, default_key.as_ref());
            if !report.is_success() {
                return Err((Some(report.clone()), report.errors.join("; ")));
            }

            let db_path = base_dir.join("vaults/default.db");
            let db_str = db_path
                .to_str()
                .ok_or_else(|| (Some(report.clone()), "Invalid default.db path".to_string()))?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| (Some(report.clone()), format!("Runtime init failed: {e}")))?;

            runtime
                .block_on(async {
                    let mut store = HostStore::open_existing_with_key(db_str, default_key.as_ref())
                        .await
                        .map_err(|e| format!("Failed to verify default.db: {e}"))?;
                    store
                        .set_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID, sync_enabled)
                        .await
                        .map_err(|e| format!("Failed to persist sync opt-in: {e}"))?;
                    let readback = store
                        .is_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID)
                        .await
                        .map_err(|e| format!("Failed to verify sync opt-in: {e}"))?;
                    if readback != sync_enabled {
                        return Err("Sync opt-in readback mismatch.".to_string());
                    }
                    Ok::<_, String>(())
                })
                .map_err(|e| (Some(report.clone()), e))?;

            Ok((report, sync_enabled))
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok((report, sync_val)) => {
                        this.verification_report = Some(report);
                        let confirmed_flag = this.controller.recovery_confirmed;
                        let persist_res = this.persist_checkpoint(|s| {
                            s.google_drive_sync_enabled = sync_val;
                            s.completed = true;
                            s.current_step = SetupStep::Completed;
                            if confirmed_flag
                                && !s
                                    .confirmed_recovery_vaults
                                    .iter()
                                    .any(|v| v == LEGACY_LOCAL_VAULT_ID)
                            {
                                s.confirmed_recovery_vaults
                                    .push(LEGACY_LOCAL_VAULT_ID.to_string());
                            }
                            s.completed_at = Some(
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            );
                        });
                        if let Err(e) = persist_res {
                            this.controller.finish_operation_err(
                                gen,
                                op_id,
                                UserFacingError::new(
                                    "Failed to save completion state",
                                    e,
                                    "E_CHECKPOINT",
                                ),
                                true,
                            );
                            cx.notify();
                            return;
                        }
                        this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            "Setup verified. Launching Shellumi…",
                        );
                        let keys = this.active_vault_keys.clone();
                        let on_complete = this.on_complete.take();
                        if let Some(cb) = on_complete {
                            cb(keys, cx);
                        }
                    }
                    Err((maybe_report, err_msg)) => {
                        if let Some(r) = maybe_report {
                            this.verification_report = Some(r);
                        }
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Final verification failed", err_msg, "E_FINISH"),
                            true,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn set_local_validation_error(&mut self, msg: &str, cx: &mut Context<Self>) {
        let id = OnboardingOperationId(self.controller.next_operation_seq);
        self.controller.next_operation_seq += 1;
        self.controller.operation = shellumi_core::onboarding::OperationState::Failed {
            id,
            error: UserFacingError::new("Check input", msg.to_string(), "E_VALIDATION"),
            retryable: true,
        };
        cx.notify();
    }

    pub(crate) fn persist_checkpoint<F>(&mut self, mutate: F) -> Result<(), String>
    where
        F: FnOnce(&mut SetupState),
    {
        let mut state = match SetupState::load(&self.base_dir) {
            Ok(Some(s)) => s,
            Ok(None) => SetupState::new(),
            Err(e) => return Err(format!("Cannot update setup checkpoint: {e}")),
        };
        mutate(&mut state);
        state
            .save(&self.base_dir)
            .map_err(|e| format!("Failed to persist setup checkpoint: {e}"))
    }

    pub(crate) fn checkpoint_google_connected(&self) -> bool {
        matches!(
            SetupState::load(&self.base_dir),
            Ok(Some(s)) if s.google_drive_connected
        )
    }

    pub(crate) fn build_sanitized_diagnostics(&self) -> String {
        format!(
            "Shellumi Onboarding Diagnostics\n\
             Version: {} ({})\n\
             OS/Arch: {}/{}\n\
             Route: {:?}\n\
             Page: {:?}\n\
             Operation: {:?}\n\
             Root Accessible: {}\n\
             Legacy Present: {}\n\
             Vault Count: {}\n\
             Migration: {:?}\n\
             Discard: {:?}",
            env!("CARGO_PKG_VERSION"),
            option_env!("SHELLUMI_GIT_SHA").unwrap_or("unknown"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            self.controller.route,
            self.controller.current_page,
            self.controller.operation,
            self.inventory.root_accessible,
            self.inventory.legacy.is_present(),
            self.inventory.vaults.len(),
            self.inventory.migration,
            self.inventory.discard,
        )
    }
}

/// Computes responsive initial bounds for the dedicated onboarding window (preferred 920×640, min 760×540).
pub(crate) fn wizard_window_bounds(cx: &App) -> gpui::Bounds<gpui::Pixels> {
    use gpui::{px, size, Bounds};
    let (avail_w, avail_h) = cx
        .primary_display()
        .map(|d| {
            let b = d.bounds();
            (f32::from(b.size.width), f32::from(b.size.height))
        })
        .unwrap_or((1440.0, 900.0));
    let w = 920.0_f32.min((avail_w - 40.0).max(680.0));
    let h = 640.0_f32.min((avail_h - 60.0).max(500.0));
    Bounds::centered(None, size(px(w), px(h)), cx)
}

//! Production onboarding controller & background workflows for `SetupWizardView`.
//!
//! All expensive operations (Argon2id KDF, Turso DB open/migration/discard, OAuth loopback,
//! Google Drive API, Shellumi Server enrollment) run on `cx.background_executor()`,
//! gated by `OnboardingController` single-flight operation IDs and session generations.

use crate::data_path;
use crate::ui::setup_wizard_window::SetupWizardView;
use gpui::{App, Context, SharedString};
use shellumi_core::onboarding::{
    resolve_onboarding_route, InstallationInventory, OnboardingController, OnboardingOperationId,
    OnboardingRoute, OperationPhase, RouteResolution, UserFacingError, WizardPage,
};
use shellumi_core::setup::{verify_setup_integrity, ConfiguredVaultSummary, SetupState, SetupStep};
use shellumi_core::sync::{
    build_google_drive_auth_url, generate_pkce_challenge, resolve_google_oauth_config,
    GoogleDriveSyncClient, OAuthCallbackListener, OAuthTokenBundle, SystemHttpTransport,
};
use shellumi_core::{
    VaultContext, VaultId, VaultKey, VaultKeyManager, VaultLocation, VaultManifest,
};
use shellumi_storage::{
    DiscardConfirmation, HostStore, LegacyDiscardEngine, MigrationCredentials, MigrationEngine,
    LEGACY_LOCAL_VAULT_ID,
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

/// Opens a URL in the system default browser without shell interpolation.
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

    pub(crate) fn cancel_active_operation(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel_flag) = &self.oauth_cancel_flag {
            cancel_flag.store(true, Ordering::Release);
        }
        if self.controller.cancel_active_operation().is_some() {
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

        // Guard against overwriting an existing vault
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

            let cred_store = shellumi_core::default_credential_store();
            let recovery_code = VaultKey::generate_recovery_code().map_err(|e| e.to_string())?;
            let (key, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
                cred_store.as_ref(),
                &vid,
                &pwd,
                remember,
                Some(&recovery_code),
            )
            .map_err(|e| e.to_string())?;

            let staged_manifest_path =
                manifests_dir.join(format!(".staged_{}.manifest.json", Uuid::new_v4()));
            let manifest = VaultManifest::new(
                vid.as_str().to_string(),
                "Default Vault".to_string(),
                primary_env.clone(),
                recovery_env,
                remember,
            );
            manifest
                .save_new_to_path(&staged_manifest_path)
                .map_err(|e| format!("Failed to write staged manifest: {e}"))?;

            let staged_db_path = vaults_dir.join(format!(".staged_{}.db", Uuid::new_v4()));
            let staged_str = staged_db_path
                .to_str()
                .ok_or_else(|| "Invalid UTF-8 database path".to_string())?;

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime init failed: {e}"))?;

            runtime.block_on(async {
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
                    let _ = store
                        .save_vault_recovery_envelope(vid.as_str(), &env_json)
                        .await;
                }
                drop(store);

                // Reopen staged DB with password-derived key to verify encryption before publication.
                let reopened = HostStore::open_existing_with_key(staged_str, Some(&key))
                    .await
                    .map_err(|e| format!("Encrypted verification reopen failed: {e}"))?;
                let _ = reopened
                    .build_vault_directory()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>(())
            })?;

            if let Err(e) = crate::actions::setup_wizard::publish_database_create_only(
                &staged_manifest_path,
                &manifest_path,
            ) {
                let _ = std::fs::remove_file(&staged_manifest_path);
                let _ = std::fs::remove_file(&staged_db_path);
                return Err(e);
            }
            if let Err(e) = crate::actions::setup_wizard::publish_database_create_only(
                &staged_db_path,
                &db_path,
            ) {
                let _ = std::fs::remove_file(&manifest_path);
                let _ = std::fs::remove_file(&staged_db_path);
                return Err(e);
            }

            Ok::<_, String>((recovery_code, key))
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok((recovery_code, key)) => {
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
                            let remember = this.remember_in_keychain;
                            let _ = this.persist_checkpoint(|s| {
                                s.default_vault_created = true;
                                s.remember_in_keychain = remember;
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
        cx.notify();

        let base_dir = self.base_dir.clone();
        let remember = self.remember_in_keychain;

        let task = cx.background_executor().spawn(async move {
            let cred_store = shellumi_core::default_credential_store();
            let recovery_code = VaultKey::generate_recovery_code().map_err(|e| e.to_string())?;

            // Separate source credential check if source is encrypted
            let source_key = if !legacy_pwd.is_empty() {
                let vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).map_err(|e| e.to_string())?;
                let manifests_dir = base_dir.join("vault_manifests");
                let mpath = manifests_dir.join("legacy-local.manifest.json");
                if mpath.exists() {
                    let manifest =
                        VaultManifest::load_from_path(&mpath).map_err(|e| e.to_string())?;
                    Some(
                        manifest
                            .unlock(&legacy_pwd)
                            .map_err(|_| "Incorrect legacy database password.".to_string())?,
                    )
                } else {
                    VaultKeyManager::retrieve_key(cred_store.as_ref(), &vid)
                        .map_err(|e| e.to_string())?
                }
            } else {
                None
            };

            let creds = MigrationCredentials {
                source_key,
                default_password: Some(new_pwd.clone()),
                default_recovery_code: Some(recovery_code.clone()),
                secondary_passwords: Default::default(),
                secondary_recovery_codes: Default::default(),
                require_portable_manifests: true,
                remember_in_keychain: remember,
            };

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime init failed: {e}"))?;

            let (report, unlocked_key) = runtime.block_on(async {
                let report = MigrationEngine::migrate_legacy_database_with_credentials(
                    &base_dir,
                    None,
                    Some(&creds),
                    cred_store.as_ref(),
                )
                .await
                .map_err(|e| format!("Migration failed: {e}"))?;

                // Independently verify activated Default Vault manifest + encrypted DB reopen.
                let mpath = base_dir
                    .join("vault_manifests")
                    .join("legacy-local.manifest.json");
                let manifest = VaultManifest::load_from_path(&mpath)
                    .map_err(|e| format!("Activated manifest verification failed: {e}"))?;
                let vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).map_err(|e| e.to_string())?;
                let key =
                    VaultKey::unwrap_with_password(&vid, &new_pwd, &manifest.primary_envelope)
                        .map_err(|_| "Activated manifest password unwrap failed.".to_string())?;

                let db_path = base_dir.join("vaults/default.db");
                let db_str = db_path
                    .to_str()
                    .ok_or_else(|| "Invalid default.db path".to_string())?;
                let store = HostStore::open_existing_with_key(db_str, Some(&key))
                    .await
                    .map_err(|e| format!("Activated database reopen verification failed: {e}"))?;
                let _ = store
                    .build_vault_directory()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>((report, key))
            })?;

            Ok::<_, String>((report, recovery_code, unlocked_key))
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok((report, recovery_code, key)) => {
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
                            this.default_recovery_code = Some(recovery_code);
                            this.controller.default_vault_unlocked = true;
                            this.controller.recovery_confirmed = false;
                            let remember = this.remember_in_keychain;
                            let _ = this.persist_checkpoint(|s| {
                                s.default_vault_created = true;
                                s.remember_in_keychain = remember;
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
                            this.refresh_inventory(cx);
                            this.controller.current_page = WizardPage::RecoveryCode;
                        }
                    }
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new(
                                "Migration stopped safely",
                                e,
                                "E_MIGRATION",
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
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            "Default vault unlocked and verified.",
                        ) {
                            this.active_vault_keys
                                .insert(LEGACY_LOCAL_VAULT_ID.to_string(), key);
                            this.controller.default_vault_unlocked = true;
                            this.controller.recovery_confirmed = true;
                            let _ = this.persist_checkpoint(|s| {
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
        let conf = DiscardConfirmation {
            acknowledged: self.controller.discard_acknowledged,
            token: self.controller.discard_token_input.clone(),
            expected_source_fingerprints: None,
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
                            this.controller.route = OnboardingRoute::Fresh;
                            this.refresh_inventory(cx);
                            this.controller.current_page = WizardPage::VaultPassword;
                        }
                    }
                    Err(e) => {
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

        // Immediately expose URL for manual "Copy sign-in link" fallback (never logged!).
        self.google_auth_url = Some(auth_url.clone());
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.oauth_cancel_flag = Some(cancel_flag.clone());

        if let Err(browser_err) = open_system_browser(&auth_url) {
            cancel_flag.store(true, Ordering::Release);
            self.controller.finish_operation_err(
                gen,
                op_id,
                UserFacingError::new(
                    "Could not open browser automatically",
                    format!("{browser_err}. Use 'Copy sign-in link' below to open it manually, or skip Google Drive."),
                    "E_BROWSER_LAUNCH",
                ),
                true,
            );
            cx.notify();
            return;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let code = listener
                .wait_for_code_cancellable(
                    &state,
                    std::time::Duration::from_secs(120),
                    &cancel_flag,
                )
                .map_err(|e| format!("Google sign-in callback failed: {e}"))?;

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

            let store = shellumi_core::default_credential_store();
            bundle
                .save_to_store(store.as_ref())
                .map_err(|e| format!("Failed to save OAuth tokens to OS Keychain: {e}"))?;

            let discovered = client
                .discover_cloud_vaults(&bundle.access_token)
                .map_err(|e| format!("Signed in, but cloud vault discovery failed: {e}"))?;

            Ok::<_, String>(discovered)
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                this.google_auth_url = None;
                this.oauth_cancel_flag = None;
                match result {
                    Ok(discovered) => {
                        if this.controller.finish_operation_ok(
                            gen,
                            op_id,
                            format!(
                                "Google Drive connected. Found {} encrypted backup(s).",
                                discovered.len()
                            ),
                        ) {
                            this.cloud_vaults = discovered;
                            let _ = this.persist_checkpoint(|s| {
                                s.google_drive_connected = true;
                                s.google_drive_skipped = false;
                            });
                            if this.controller.mode
                                == shellumi_core::setup::SetupWizardMode::RestoreSetup
                                && !this.cloud_vaults.is_empty()
                            {
                                this.controller.current_page = WizardPage::CloudRestore;
                            }
                        }
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

    pub(crate) fn disconnect_google_drive(&mut self, cx: &mut Context<Self>) {
        let store = shellumi_core::default_credential_store();
        let _ = OAuthTokenBundle::delete_from_store(store.as_ref());
        self.cloud_vaults.clear();
        self.sync_default_opt_in = false;
        let _ = self.persist_checkpoint(|s| {
            s.google_drive_connected = false;
            s.google_drive_sync_enabled = false;
        });
        cx.notify();
    }

    pub(crate) fn finish_and_launch(&mut self, cx: &mut Context<Self>) {
        if self.controller.operation.is_running() {
            return;
        }
        let default_key = self
            .active_vault_keys
            .get(LEGACY_LOCAL_VAULT_ID)
            .cloned()
            .or_else(|| {
                let vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).ok()?;
                let store = shellumi_core::default_credential_store();
                VaultKeyManager::retrieve_key(store.as_ref(), &vid)
                    .ok()
                    .flatten()
            });

        let report = verify_setup_integrity(&self.base_dir, default_key.as_ref());
        self.verification_report = Some(report.clone());
        if !report.is_success() {
            self.set_local_validation_error(&report.errors.join("; "), cx);
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
        let sync_enabled = self.sync_default_opt_in && self.checkpoint_google_connected();

        let task = cx.background_executor().spawn(async move {
            let db_path = base_dir.join("vaults/default.db");
            let db_str = db_path
                .to_str()
                .ok_or_else(|| "Invalid default.db path".to_string())?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Runtime init failed: {e}"))?;

            runtime.block_on(async {
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
                Ok::<_, String>(sync_enabled)
            })
        });

        cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(sync_val) => {
                        let persist_res = this.persist_checkpoint(|s| {
                            s.google_drive_sync_enabled = sync_val;
                            s.completed = true;
                            s.current_step = SetupStep::Completed;
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
                    Err(e) => {
                        this.controller.finish_operation_err(
                            gen,
                            op_id,
                            UserFacingError::new("Final verification failed", e, "E_FINISH"),
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
             Version: {}\n\
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

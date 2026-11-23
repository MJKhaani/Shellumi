//! Sync actions.
use super::*;

impl Shellumi {
    pub(crate) fn select_conflict_item(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.conflict_modal {
            if idx < modal.items.len() {
                modal.selected_index = idx;
                cx.notify();
            }
        }
    }

    pub(crate) fn set_conflict_choice(
        &mut self,
        choice: ConflictResolutionChoice,
        cx: &mut Context<Self>,
    ) {
        if let Some(modal) = &mut self.conflict_modal {
            let idx = modal.selected_index;
            if idx < modal.resolutions.len() {
                modal.resolutions[idx] = choice;
                cx.notify();
            }
        }
    }

    pub(crate) fn dismiss_conflict_modal(&mut self, cx: &mut Context<Self>) {
        self.conflict_modal = None;
        cx.notify();
    }

    pub(crate) fn apply_conflict_resolutions(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.conflict_modal.take() else {
            return;
        };
        if modal.vault_id != self.active_vault_id {
            self.sync_status = AppSyncStatus::Error(
                "Select the vault whose conflicts were reviewed before applying choices.".into(),
            );
            cx.notify();
            return;
        }
        let Some(context) = modal
            .items
            .first()
            .and_then(|item| item.revision_context.clone())
        else {
            self.sync_status =
                AppSyncStatus::Error("Refresh cloud conflicts before applying choices.".into());
            cx.notify();
            return;
        };
        let Some(registry_path) = self.registry_db_path() else {
            return;
        };
        let vault_id = self.active_vault_id.clone();
        let Some(path) = self.active_vault_db_path() else {
            self.sync_status = AppSyncStatus::Error("Database not open.".into());
            cx.notify();
            return;
        };

        let cred_store = shellumi_core::default_credential_store();
        let token = cred_store
            .get_secret("org.shellumi.sync", "gdrive_oauth_token")
            .ok()
            .flatten()
            .map(|s| s.expose_secret().to_string())
            .or_else(|| std::env::var("SHELLUMI_GDRIVE_TOKEN").ok())
            .or_else(|| std::env::var("GOOGLE_DRIVE_TOKEN").ok())
            .unwrap_or_default();

        if token.trim().is_empty() {
            self.sync_status = AppSyncStatus::Error(
                "Google Drive OAuth token not found. Please authenticate with Google Drive.".into(),
            );
            cx.notify();
            return;
        }

        let passphrase = match std::env::var("SHELLUMI_VAULT_PASSPHRASE") {
            Ok(value) if value.chars().count() >= 8 => shellumi_core::SecretString::new(value),
            _ => {
                if let Some(key) = self.active_vault_key() {
                    shellumi_core::SecretString::new(hex::encode(key.expose_bytes()))
                } else {
                    self.sync_status = AppSyncStatus::Error(
                        "Vault must be unlocked with a valid encryption key before syncing.".into(),
                    );
                    cx.notify();
                    return;
                }
            }
        };
        let device_id =
            std::env::var("SHELLUMI_DEVICE_ID").unwrap_or_else(|_| "desktop-app".to_string());
        let active_key = self.active_vault_key();
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);

        let resolutions = modal.resolutions;
        self.sync_status = AppSyncStatus::Syncing;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, active_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                let scoped = shellumi_core::VaultId::new(&vault_id).map_err(|e| e.to_string())?;
                store.set_vault_id(scoped.clone());
                let key = default_key
                    .as_ref()
                    .ok_or("Unlock the default registry before resolving Drive conflicts.")?;
                let registry = HostStore::open_with_key(&registry_path, Some(key))
                    .await
                    .map_err(|e| e.to_string())?;
                if !registry
                    .is_vault_sync_enabled(&vault_id)
                    .await
                    .map_err(|e| e.to_string())?
                {
                    return Err(
                        "Google Drive consent was disabled before conflict resolution.".into(),
                    );
                }
                let directory = registry
                    .build_vault_directory()
                    .await
                    .map_err(|e| e.to_string())?;
                registry
                    .save_vault_directory_from_model(
                        &shellumi_core::VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID)
                            .map_err(|e| e.to_string())?,
                        key,
                        &directory,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                let local_vault = store
                    .export_vault(passphrase.expose_secret(), &device_id)
                    .await
                    .map_err(|e| e.to_string())?;
                let client =
                    GoogleDriveSyncClient::new(SystemHttpTransport::default(), "shellumi-desktop");
                let cycle_result = shellumi_core::execute_cloud_vault_sync_bound(
                    &client,
                    &token,
                    &local_vault,
                    passphrase.expose_secret(),
                    &shellumi_core::CloudConflictResolution {
                        context,
                        choices: resolutions,
                    },
                )
                .map_err(|e| e.to_string())?;

                match cycle_result {
                    SyncCycleResult::AlreadySynchronized => Ok(()),
                    SyncCycleResult::Success {
                        merged_records,
                        report,
                    } => {
                        let mut write_store = HostStore::open_with_key(&path, active_key.as_ref())
                            .await
                            .map_err(|e| e.to_string())?;
                        write_store.set_vault_id(scoped.clone());
                        let merged_vault = report.synchronized_vault(&local_vault, merged_records);
                        write_store
                            .import_vault(passphrase.expose_secret(), &merged_vault)
                            .await
                            .map_err(|e| e.to_string())?;

                        Ok(())
                    }
                    SyncCycleResult::Conflicts(_) => {
                        Err("Unexpected unresolved conflicts after resolution choices.".into())
                    }
                    SyncCycleResult::Error(err) => Err(err),
                }
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(()) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        app.sync_status = AppSyncStatus::UpToDate {
                            last_sync_time: now,
                        };
                        app.start_storage(false, cx);
                    }
                    Err(e) => {
                        app.sync_status = AppSyncStatus::Error(format!("Resolution error: {e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn set_active_vault_drive_opt_in(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.desktop.busy || self.is_active_vault_remote() {
            return;
        }
        let Some(path) = self.registry_db_path() else {
            self.desktop.message = Some("Vault registry is unavailable.".into());
            cx.notify();
            return;
        };
        let Some(key) = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID) else {
            self.desktop.message =
                Some("Unlock the default vault to change Google Drive consent.".into());
            cx.notify();
            return;
        };
        let vault_id = self.active_vault_id.clone();
        let name = self.active_vault.clone();
        self.desktop.busy = true;
        self.desktop.message = None;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread().build().map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut registry = HostStore::open_with_key(&path, Some(&key)).await.map_err(|e| e.to_string())?;
                if !registry.set_vault_sync_enabled(&vault_id, enabled).await.map_err(|e| e.to_string())? {
                    return Err("Active vault is missing from the default registry.".to_string());
                }
                tracing::info!(vault_id = %vault_id, enabled, "Saved explicit Google Drive synchronization consent");
                Ok(())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                app.desktop.busy = false;
                app.desktop.message = Some(match result {
                    Ok(()) => format!(
                        "Google Drive synchronization {} for {name}.",
                        if enabled { "enabled" } else { "disabled" }
                    ),
                    Err(error) => {
                        tracing::error!(error = %error, "Could not save Google Drive consent");
                        error
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn trigger_sync(&mut self, cx: &mut Context<Self>) {
        if self.is_active_vault_remote() {
            self.sync_active_vault(cx);
            return;
        }
        let Some(path) = self.active_vault_db_path() else {
            self.sync_status = AppSyncStatus::Error("Database not open.".into());
            cx.notify();
            return;
        };

        let cred_store = shellumi_core::default_credential_store();
        let token = cred_store
            .get_secret("org.shellumi.sync", "gdrive_oauth_token")
            .ok()
            .flatten()
            .map(|s| s.expose_secret().to_string())
            .or_else(|| std::env::var("SHELLUMI_GDRIVE_TOKEN").ok())
            .or_else(|| std::env::var("GOOGLE_DRIVE_TOKEN").ok())
            .unwrap_or_default();

        if token.trim().is_empty() {
            self.sync_status = AppSyncStatus::Error(
                "Google Drive OAuth token not found. Please authenticate with Google Drive in Settings or configure your credentials."
                    .into(),
            );
            cx.notify();
            return;
        }

        let passphrase = match std::env::var("SHELLUMI_VAULT_PASSPHRASE") {
            Ok(value) if value.chars().count() >= 8 => shellumi_core::SecretString::new(value),
            _ => {
                if let Some(key) = self.active_vault_key() {
                    shellumi_core::SecretString::new(hex::encode(key.expose_bytes()))
                } else {
                    self.sync_status = AppSyncStatus::Error(
                        "Vault must be unlocked with a valid encryption key before syncing.".into(),
                    );
                    cx.notify();
                    return;
                }
            }
        };
        let device_id =
            std::env::var("SHELLUMI_DEVICE_ID").unwrap_or_else(|_| "desktop-app".to_string());
        let active_key = self.active_vault_key();
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let active_vault_id = self.active_vault_id.clone();
        let conflict_vault_id = active_vault_id.clone();
        let Some(registry_path) = self.registry_db_path() else {
            self.sync_status = AppSyncStatus::Error("Vault registry not available.".into());
            cx.notify();
            return;
        };

        self.sync_status = AppSyncStatus::Syncing;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, active_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                let scoped_id = shellumi_core::VaultId::new(&active_vault_id).map_err(|e| e.to_string())?;
                store.set_vault_id(scoped_id.clone());
                let registry = HostStore::open_with_key(&registry_path, default_key.as_ref())
                    .await
                    .map_err(|e| format!("Failed to open vault registry: {e}"))?;
                if !registry
                    .is_vault_sync_enabled(&active_vault_id)
                    .await
                    .map_err(|e| format!("Failed to read vault synchronization setting: {e}"))?
                {
                    tracing::info!(vault_id = %active_vault_id, "Google Drive synchronization is disabled for vault");
                    return Err("Google Drive synchronization is disabled for this vault. Enable it in Settings → Synchronization before syncing.".to_string());
                }
                let directory = registry.build_vault_directory().await.map_err(|e| e.to_string())?;
                let default_vid = shellumi_core::VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID).map_err(|e| e.to_string())?;
                let key = default_key.as_ref().ok_or("Unlock the default vault registry before synchronizing.")?;
                let directory_store = HostStore::open_with_key(&registry_path, Some(key)).await.map_err(|e| e.to_string())?;
                directory_store.save_vault_directory_from_model(&default_vid, key, &directory).await.map_err(|e| e.to_string())?;
                let local_vault = store
                    .export_vault(passphrase.expose_secret(), &device_id)
                    .await
                    .map_err(|e| e.to_string())?;
                let client =
                    GoogleDriveSyncClient::new(SystemHttpTransport::default(), "shellumi-desktop");
                let cycle_result = execute_cloud_vault_sync(
                    &client,
                    &token,
                    &local_vault,
                    passphrase.expose_secret(),
                    None,
                )
                .map_err(|e| e.to_string())?;

                match cycle_result {
                    SyncCycleResult::AlreadySynchronized => Ok((None, 0)),
                    SyncCycleResult::Success {
                        merged_records,
                        report,
                    } => {
                        let mut write_store = HostStore::open_with_key(&path, active_key.as_ref())
                            .await
                            .map_err(|e| e.to_string())?;
                        write_store.set_vault_id(scoped_id.clone());
                        let merged_vault = report.synchronized_vault(&local_vault, merged_records);
                        let summary = write_store
                            .import_vault(passphrase.expose_secret(), &merged_vault)
                            .await
                            .map_err(|e| e.to_string())?;



                        let count = summary.hosts_imported
                            + summary.groups_imported
                            + summary.tunnels_imported
                            + summary.snippets_imported;
                        Ok((None, count))
                    }
                    SyncCycleResult::Conflicts(conflict_views) => Ok((Some(conflict_views), 0)),
                    SyncCycleResult::Error(err) => Err(err),
                }
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok((Some(conflict_items), _)) => {
                        let total_conflicts = conflict_items.len();
                        let default_resolutions =
                            vec![ConflictResolutionChoice::KeepLocal; total_conflicts];
                        app.conflict_modal = Some(ConflictModalState {
                            items: conflict_items,
                            selected_index: 0,
                            resolutions: default_resolutions,
                            file_id: "google-drive-sync".into(),
                            vault_id: conflict_vault_id.clone(),
                        });
                        app.sync_status = AppSyncStatus::Conflict {
                            message: format!("{total_conflicts} conflicting changes detected"),
                        };
                    }
                    Ok((None, _count)) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        app.sync_status = AppSyncStatus::UpToDate {
                            last_sync_time: now,
                        };
                        app.start_storage(false, cx);
                    }
                    Err(e) => {
                        app.sync_status = AppSyncStatus::Error(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

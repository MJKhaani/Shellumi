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
        let Some(path) = self.database.clone() else {
            self.sync_status = AppSyncStatus::Error("Database not open.".into());
            cx.notify();
            return;
        };

        let token = std::env::var("SHELLUMI_GDRIVE_TOKEN")
            .or_else(|_| std::env::var("GOOGLE_DRIVE_TOKEN"))
            .unwrap_or_default();

        if token.trim().is_empty() {
            self.sync_status = AppSyncStatus::Error("Google Drive OAuth token not found.".into());
            cx.notify();
            return;
        }

        let passphrase = match std::env::var("SHELLUMI_VAULT_PASSPHRASE") {
            Ok(value) if value.chars().count() >= 12 => shellumi_core::SecretString::new(value),
            _ => {
                self.sync_status = AppSyncStatus::Error(
                    "Set a vault passphrase of at least 12 characters before syncing.".into(),
                );
                cx.notify();
                return;
            }
        };
        let device_id =
            std::env::var("SHELLUMI_DEVICE_ID").unwrap_or_else(|_| "desktop-app".to_string());

        let resolutions = modal.resolutions;
        self.sync_status = AppSyncStatus::Syncing;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                // Refresh encrypted vault directory if default vault key is available
                if let Ok(dir) = store.build_vault_directory().await {
                    if let Ok(default_vid) =
                        shellumi_core::VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID)
                    {
                        if let Ok(Some(key)) = shellumi_core::VaultKeyManager::retrieve_key(
                            &*shellumi_core::default_credential_store(),
                            &default_vid,
                        ) {
                            let mut wstore =
                                HostStore::open(&path).await.map_err(|e| e.to_string())?;
                            let _ = wstore
                                .save_vault_directory_from_model(&default_vid, &key, &dir)
                                .await;
                        }
                    }
                }
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
                    Some(&resolutions),
                )
                .map_err(|e| e.to_string())?;

                match cycle_result {
                    SyncCycleResult::AlreadySynchronized => Ok(()),
                    SyncCycleResult::Success {
                        merged_records,
                        report,
                    } => {
                        let mut write_store =
                            HostStore::open(&path).await.map_err(|e| e.to_string())?;
                        let mut merged_vault = SyncVault::new("cloud-vault", 0);
                        merged_vault.records = merged_records;
                        merged_vault.encrypted_directory = report.encrypted_directory.clone();
                        write_store
                            .import_vault(passphrase.expose_secret(), &merged_vault)
                            .await
                            .map_err(|e| e.to_string())?;

                        if let Some(enc_dir) = &report.encrypted_directory {
                            if let Ok(default_vid) =
                                shellumi_core::VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID)
                            {
                                let default_key = shellumi_core::VaultKeyManager::retrieve_key(
                                    &*shellumi_core::default_credential_store(),
                                    &default_vid,
                                )
                                .ok()
                                .flatten();
                                let _ = write_store
                                    .import_encrypted_vault_directory(
                                        &default_vid,
                                        default_key.as_ref(),
                                        enc_dir,
                                    )
                                    .await;
                            }
                        }
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

    pub(crate) fn trigger_sync(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            self.sync_status = AppSyncStatus::Error("Database not open.".into());
            cx.notify();
            return;
        };

        let token = std::env::var("SHELLUMI_GDRIVE_TOKEN")
            .or_else(|_| std::env::var("GOOGLE_DRIVE_TOKEN"))
            .unwrap_or_default();

        if token.trim().is_empty() {
            self.sync_status = AppSyncStatus::Error(
                "Google Drive OAuth token not found. Set GOOGLE_DRIVE_TOKEN or SHELLUMI_GDRIVE_TOKEN environment variable.".into(),
            );
            cx.notify();
            return;
        }

        let passphrase = match std::env::var("SHELLUMI_VAULT_PASSPHRASE") {
            Ok(value) if value.chars().count() >= 12 => shellumi_core::SecretString::new(value),
            _ => {
                self.sync_status = AppSyncStatus::Error(
                    "Set a vault passphrase of at least 12 characters before syncing.".into(),
                );
                cx.notify();
                return;
            }
        };
        let device_id =
            std::env::var("SHELLUMI_DEVICE_ID").unwrap_or_else(|_| "desktop-app".to_string());

        self.sync_status = AppSyncStatus::Syncing;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                // Refresh encrypted vault directory if default vault key is available
                if let Ok(dir) = store.build_vault_directory().await {
                    if let Ok(default_vid) =
                        shellumi_core::VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID)
                    {
                        if let Ok(Some(key)) = shellumi_core::VaultKeyManager::retrieve_key(
                            &*shellumi_core::default_credential_store(),
                            &default_vid,
                        ) {
                            let mut wstore =
                                HostStore::open(&path).await.map_err(|e| e.to_string())?;
                            let _ = wstore
                                .save_vault_directory_from_model(&default_vid, &key, &dir)
                                .await;
                        }
                    }
                }
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
                        let mut write_store =
                            HostStore::open(&path).await.map_err(|e| e.to_string())?;
                        let mut merged_vault = SyncVault::new("cloud-vault", 0);
                        merged_vault.records = merged_records;
                        merged_vault.encrypted_directory = report.encrypted_directory.clone();
                        let summary = write_store
                            .import_vault(passphrase.expose_secret(), &merged_vault)
                            .await
                            .map_err(|e| e.to_string())?;

                        if let Some(enc_dir) = &report.encrypted_directory {
                            if let Ok(default_vid) =
                                shellumi_core::VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID)
                            {
                                let default_key = shellumi_core::VaultKeyManager::retrieve_key(
                                    &*shellumi_core::default_credential_store(),
                                    &default_vid,
                                )
                                .ok()
                                .flatten();
                                let _ = write_store
                                    .import_encrypted_vault_directory(
                                        &default_vid,
                                        default_key.as_ref(),
                                        enc_dir,
                                    )
                                    .await;
                            }
                        }

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

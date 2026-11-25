//! Vault lock, unlock, and recovery passphrase actions.
use super::*;
use shellumi_core::{default_credential_store, VaultId, VaultKey, VaultKeyManager, VaultManifest};

impl Shellumi {
    /// Opens the interactive Unlock Vault modal for a locked vault.
    pub(crate) fn open_unlock_vault_modal(
        &mut self,
        vault_id: String,
        vault_name: String,
        cx: &mut Context<Self>,
    ) {
        let input =
            cx.new(|cx| TextInput::new(cx, "", "Enter vault password or recovery code…").masked());
        self.unlock_vault_modal = Some(UnlockVaultModalState {
            vault_id,
            vault_name,
            passphrase_input: input,
            remember_in_keychain: false,
            error: None,
            is_submitting: false,
        });
        cx.notify();
    }

    /// Dismisses the Unlock Vault modal.
    pub(crate) fn dismiss_unlock_vault_modal(&mut self, cx: &mut Context<Self>) {
        self.unlock_vault_modal = None;
        cx.notify();
    }

    /// Toggles the "remember in keychain" checkbox for unlocking.
    pub(crate) fn toggle_unlock_remember_keychain(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.unlock_vault_modal {
            modal.remember_in_keychain = !modal.remember_in_keychain;
            cx.notify();
        }
    }

    /// Submits the entered recovery passphrase or password to unlock the vault.
    pub(crate) fn submit_unlock_vault(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = &mut self.unlock_vault_modal else {
            return;
        };
        let passphrase = modal.passphrase_input.read(cx).value().trim().to_string();
        if passphrase.is_empty() {
            modal.error = Some("Password or recovery code cannot be empty.".to_string());
            cx.notify();
            return;
        }

        let vault_id_str = modal.vault_id.clone();
        let vault_name = modal.vault_name.clone();
        let remember_in_keychain = modal.remember_in_keychain;
        modal.error = None;
        modal.is_submitting = true;
        cx.notify();

        let Some(path) = self.database.clone() else {
            if let Some(m) = &mut self.unlock_vault_modal {
                m.error = Some("Database not configured.".to_string());
                m.is_submitting = false;
            }
            cx.notify();
            return;
        };

        let vault_id_task = vault_id_str.clone();
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let pending_recovery = self
            .pending_rotation_recoveries
            .iter()
            .find(|r| r.vault_id == vault_id_str)
            .cloned();
        if let Some(rec) = &pending_recovery {
            if rec.is_corrupted {
                if let Some(m) = &mut self.unlock_vault_modal {
                    m.error = Some(format!(
                        "Recovery required: {}. Preserve the rotation journal at {}.",
                        rec.reason,
                        rec.journal_path.display()
                    ));
                    m.is_submitting = false;
                }
                cx.notify();
                return;
            }
        }
        let has_pending_recovery = pending_recovery.is_some();

        let task = cx.background_executor().spawn(async move {
            let vid =
                VaultId::new(&vault_id_task).map_err(|_| "Invalid vault identifier".to_string())?;

            let manifest_path =
                VaultManifest::manifest_path_for_vault(std::path::Path::new(&path), &vid);

            let recovered_key = if has_pending_recovery {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                let cred_store = default_credential_store();
                let outcome =
                    runtime.block_on(crate::vault_rotation::reconcile_rotation_with_password(
                        std::path::Path::new(&path),
                        &vault_id_task,
                        &passphrase,
                        default_key.as_ref(),
                        cred_store.as_ref(),
                    ))?;
                outcome.key
            } else if manifest_path.exists() {
                let mut manifest = VaultManifest::load_from_path(&manifest_path)
                    .map_err(|e| format!("Failed to read vault manifest: {e}"))?;
                let key = manifest.unlock(&passphrase).map_err(|_| {
                    "Incorrect vault password, invalid recovery code, or corrupted envelope."
                        .to_string()
                })?;

                let cred_store = default_credential_store();
                if remember_in_keychain {
                    VaultKeyManager::provision_key(&*cred_store, &vid, &key).map_err(|e| {
                        format!("Failed to store vault key in OS credential store: {e}")
                    })?;
                    manifest.remember_in_keychain = true;
                } else {
                    let _ = VaultKeyManager::forget_key(&*cred_store, &vid);
                    manifest.remember_in_keychain = false;
                }
                let _ = manifest.save_to_path(&manifest_path);
                key
            } else {
                // Backwards-compatible fallback for legacy unmanifested recovery envelopes
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                let reg_db_path = data_path::resolve_vault_db_file(
                    std::path::Path::new(&path),
                    shellumi_storage::LEGACY_LOCAL_VAULT_ID,
                );
                let reg_key = default_key.clone();
                runtime.block_on(async {
                    let store =
                        HostStore::open_with_key(reg_db_path.to_str().unwrap(), reg_key.as_ref())
                            .await
                            .map_err(|e| e.to_string())?;
                    let envelope_str = store
                        .get_vault_recovery_envelope(&vault_id_task)
                        .await
                        .map_err(|e| e.to_string())?
                        .ok_or_else(|| {
                            "No vault manifest or recovery envelope found for this vault."
                                .to_string()
                        })?;

                    let envelope: shellumi_core::sync::EncryptedEnvelope =
                        serde_json::from_str(&envelope_str)
                            .map_err(|e| format!("Corrupt recovery envelope JSON format: {e}"))?;

                    let key = VaultKey::unwrap_with_password(&vid, &passphrase, &envelope)
                        .map_err(|_| {
                            "Incorrect recovery passphrase or corrupted recovery envelope."
                                .to_string()
                        })?;

                    if remember_in_keychain {
                        let cred_store = default_credential_store();
                        let _ = VaultKeyManager::provision_key(&*cred_store, &vid, &key);
                    }

                    Ok::<_, String>(key)
                })?
            };

            // Open the vault's physical database with the recovered DEK to verify decryption
            let target_vault_path =
                data_path::resolve_vault_db_file(std::path::Path::new(&path), &vault_id_task);
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let _verified_store = HostStore::open_with_key(
                    target_vault_path.to_str().unwrap(),
                    Some(&recovered_key),
                )
                .await
                .map_err(|e| format!("Failed to open encrypted vault database: {e}"))?;

                // Mark unlocked in default vault registry
                let reg_db_path = data_path::resolve_vault_db_file(
                    std::path::Path::new(&path),
                    shellumi_storage::LEGACY_LOCAL_VAULT_ID,
                );
                let reg_key = if vault_id_task == shellumi_storage::LEGACY_LOCAL_VAULT_ID {
                    Some(&recovered_key)
                } else {
                    default_key.as_ref()
                };
                if let Ok(mut reg_store) =
                    HostStore::open_with_key(reg_db_path.to_str().unwrap(), reg_key).await
                {
                    let _ = reg_store.set_vault_locked(&vault_id_task, false).await;
                }
                Ok::<_, String>(())
            })?;

            Ok::<_, String>(recovered_key)
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok(recovered_key) => {
                        app.pending_rotation_recoveries
                            .retain(|r| r.vault_id != vault_id_str);
                        app.active_vault_keys
                            .insert(vault_id_str.clone(), recovered_key);
                        app.unlock_vault_modal = None;
                        app.active_vault_id = vault_id_str;
                        app.active_vault = vault_name;
                        app.load_vault_list(cx);
                        app.start_storage(false, cx);
                    }
                    Err(err) => {
                        if let Some(m) = &mut app.unlock_vault_modal {
                            m.error = Some(err);
                            m.is_submitting = false;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Locks a vault, marking it locked in storage, evicting keys/data, and prompting for unlock if active.
    pub(crate) fn lock_vault(&mut self, vault_id: String, cx: &mut Context<Self>) {
        let Some(path) = self.registry_db_path() else {
            return;
        };
        let vid_clone = vault_id.clone();
        let is_active = self.active_vault_id == vault_id;
        let vname = self.active_vault.clone();

        // 1. Evict from active in-memory keys
        let _ = self.active_vault_keys.remove(&vault_id);

        // 2. Evict from OS Keychain when explicitly locked
        if let Ok(vid) = VaultId::new(&vault_id) {
            let cred_store = default_credential_store();
            let _ = VaultKeyManager::forget_key(&*cred_store, &vid);
            let manifest_path =
                VaultManifest::manifest_path_for_vault(std::path::Path::new(&path), &vid);
            if let Ok(mut manifest) = VaultManifest::load_from_path(&manifest_path) {
                manifest.remember_in_keychain = false;
                let _ = manifest.save_to_path(&manifest_path);
            }
        }

        // 3. If active, revoke access: clear in-memory sensitive data immediately
        if is_active {
            self.hosts = HostState::Loaded(Vec::new());
            self.snippets.clear();
            self.workspaces.clear();
            self.identities.clear();
        }

        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, default_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .set_vault_locked(&vid_clone, true)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let _ = task.await;
            let _ = app.update(cx, |app, cx| {
                app.load_vault_list(cx);
                if is_active {
                    app.open_unlock_vault_modal(vault_id, vname, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Locks the currently active vault.
    pub(crate) fn lock_active_vault(&mut self, cx: &mut Context<Self>) {
        let active_id = self.active_vault_id.clone();
        self.lock_vault(active_id, cx);
    }
}

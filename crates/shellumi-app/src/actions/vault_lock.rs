//! Vault lock, unlock, and recovery passphrase actions.
use super::*;
use shellumi_core::{default_credential_store, VaultId, VaultKey, VaultKeyManager};

impl Shellumi {
    /// Opens the interactive Unlock Vault modal for a locked vault.
    pub(crate) fn open_unlock_vault_modal(
        &mut self,
        vault_id: String,
        vault_name: String,
        cx: &mut Context<Self>,
    ) {
        let input = cx
            .new(|cx| TextInput::new(cx, "", "Enter recovery passphrase (min 12 chars)…").masked());
        self.unlock_vault_modal = Some(UnlockVaultModalState {
            vault_id,
            vault_name,
            passphrase_input: input,
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

    /// Submits the entered recovery passphrase to unlock the vault.
    pub(crate) fn submit_unlock_vault(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = &mut self.unlock_vault_modal else {
            return;
        };
        let passphrase = modal.passphrase_input.read(cx).value().trim().to_string();
        if passphrase.len() < 12 {
            modal.error = Some("Passphrase must be at least 12 characters long.".to_string());
            cx.notify();
            return;
        }

        let vault_id_str = modal.vault_id.clone();
        let vault_name = modal.vault_name.clone();
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
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let envelope_str = store
                    .get_vault_recovery_envelope(&vault_id_task)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| {
                        "No recovery envelope found for this vault in storage.".to_string()
                    })?;

                let envelope: shellumi_core::sync::EncryptedEnvelope =
                    serde_json::from_str(&envelope_str)
                        .map_err(|e| format!("Corrupt recovery envelope JSON format: {e}"))?;

                let vid = VaultId::new(&vault_id_task)
                    .map_err(|_| "Invalid vault identifier".to_string())?;

                let recovered_key =
                    VaultKey::recover(&vid, &passphrase, &envelope).map_err(|_| {
                        "Incorrect recovery passphrase or corrupted recovery envelope.".to_string()
                    })?;

                // Provision the recovered key into the OS credential store
                let cred_store = default_credential_store();
                VaultKeyManager::provision_key(&*cred_store, &vid, &recovered_key).map_err(
                    |e| format!("Failed to provision vault key in OS credential store: {e}"),
                )?;

                // Unlock the vault in database
                store
                    .set_vault_locked(&vault_id_task, false)
                    .await
                    .map_err(|e| e.to_string())?;

                Ok::<_, String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok(()) => {
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

    /// Locks a vault, marking it locked in storage and prompting for unlock if active.
    pub(crate) fn lock_vault(&mut self, vault_id: String, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let vid_clone = vault_id.clone();
        let is_active = self.active_vault_id == vault_id;
        let vname = self.active_vault.clone();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
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

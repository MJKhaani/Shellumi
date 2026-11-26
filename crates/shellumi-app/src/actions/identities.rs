//! Identities actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_identity_modal(&mut self, id: Option<i64>, cx: &mut Context<Self>) {
        let existing = id.and_then(|id_val| self.identities.iter().find(|i| i.id == id_val));
        let name = existing.map(|i| i.name.as_str()).unwrap_or("");
        let user = existing.map(|i| i.username.as_str()).unwrap_or("");
        let key = existing.and_then(|i| i.key_path.as_deref()).unwrap_or("");
        let tags = existing.map(|i| i.tags.join(", ")).unwrap_or_default();

        let name_input =
            cx.new(|cx| TextInput::new(cx, name, "Identity name (e.g. aldaleel ssh key login)"));
        let user_input = cx.new(|cx| TextInput::new(cx, user, "Username (e.g. root)"));
        let key_input = cx.new(|cx| TextInput::new(cx, key, "Key path (e.g. ~/.ssh/id_ed25519)"));
        let password_input =
            cx.new(|cx| TextInput::new(cx, "", "Password (leave blank to keep existing)").masked());
        let pass_input = cx
            .new(|cx| TextInput::new(cx, "", "Passphrase (leave blank to keep existing)").masked());
        let tags_input = cx.new(|cx| TextInput::new(cx, &tags, "Tags (comma-separated)"));

        cx.observe(&name_input, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&user_input, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&key_input, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&password_input, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&pass_input, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&tags_input, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();

        self.identity_modal = Some(IdentityModalState {
            editing_id: id,
            name: name_input,
            username: user_input,
            key_path: key_input,
            password: password_input,
            passphrase: pass_input,
            tags: tags_input,
            error: None,
        });
        cx.notify();
    }

    pub(crate) fn save_identity_modal(&mut self, cx: &mut Context<Self>) {
        let Some(gen) = self.inspector_coordinator.start_save() else {
            return;
        };
        let Some(modal) = &self.identity_modal else {
            return;
        };
        let active_vid = self.active_vault_id.clone();
        let name = modal.name.read(cx).value().trim().to_string();
        let username = modal.username.read(cx).value().trim().to_string();
        let key_path = modal.key_path.read(cx).value().trim().to_string();
        let tags: Vec<String> = modal
            .tags
            .read(cx)
            .value()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let editing_id = modal.editing_id;
        let password = modal.password.read(cx).value().to_string();
        let passphrase = modal.passphrase.read(cx).value().to_string();
        let existing = editing_id
            .and_then(|id| self.identities.iter().find(|identity| identity.id == id))
            .cloned();

        if name.is_empty() || username.is_empty() {
            if let Some(m) = &mut self.identity_modal {
                m.error = Some("Identity name and username cannot be empty".into());
                self.inspector_coordinator.fail_save(
                    &active_vid,
                    gen,
                    "Identity name and username cannot be empty".into(),
                );
                cx.notify();
            }
            return;
        }

        let auth_method = if !key_path.is_empty() {
            shellumi_core::AuthMethod::PublicKey {
                key_name: name.clone(),
            }
        } else if !password.is_empty() {
            shellumi_core::AuthMethod::Password
        } else {
            existing
                .as_ref()
                .map(|identity| identity.auth_method.clone())
                .unwrap_or(shellumi_core::AuthMethod::Interactive)
        };
        let mut new_id = shellumi_core::NewIdentity {
            name: name.clone(),
            username: username.clone(),
            auth_method,
            password_secret_ref: existing
                .as_ref()
                .and_then(|identity| identity.password_secret_ref.clone()),
            key_path: if key_path.is_empty() {
                None
            } else {
                Some(key_path)
            },
            passphrase_secret_ref: existing
                .as_ref()
                .and_then(|identity| identity.passphrase_secret_ref.clone()),
            certificate_path: existing
                .as_ref()
                .and_then(|identity| identity.certificate_path.clone()),
            tags,
            comment: existing
                .as_ref()
                .and_then(|identity| identity.comment.clone()),
        };

        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
        let vault_key = self.active_vault_key();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                let vid = shellumi_core::VaultId::new(&active_vault_id_task)
                    .map_err(|e| e.to_string())?;
                store.set_vault_id(vid.clone());
                let credential_store = shellumi_core::default_credential_store();
                let vault_store =
                    shellumi_core::VaultCredentialStore::new(credential_store.as_ref(), &vid);
                let mut created_refs = Vec::new();
                if !password.is_empty() {
                    let reference = vault_store
                        .store_unique("identity-password", &password)
                        .map_err(|e| e.to_string())?;
                    created_refs.push(reference.clone());
                    new_id.password_secret_ref = Some(reference);
                }
                if !passphrase.is_empty() {
                    match vault_store.store_unique("identity-passphrase", &passphrase) {
                        Ok(reference) => {
                            created_refs.push(reference.clone());
                            new_id.passphrase_secret_ref = Some(reference);
                        }
                        Err(error) => {
                            for reference in &created_refs {
                                let _ = vault_store.delete(reference);
                            }
                            return Err(error.to_string());
                        }
                    }
                }
                let result = if let Some(id) = editing_id {
                    store
                        .update_identity(id, &new_id)
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|updated| {
                            if updated {
                                Ok(())
                            } else {
                                Err("Identity no longer exists.".into())
                            }
                        })
                } else {
                    store
                        .create_identity(&new_id)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                };
                if result.is_err() {
                    for reference in &created_refs {
                        let _ = shellumi_core::delete_secret_by_ref(
                            credential_store.as_ref(),
                            reference,
                        );
                    }
                }
                result
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(err) = res {
                    if let Some(m) = &mut app.identity_modal {
                        m.error = Some(err.clone());
                        cx.notify();
                    }
                    app.inspector_coordinator
                        .fail_save(&active_vault_id, gen, err);
                } else {
                    let target_id = editing_id
                        .or_else(|| app.identities.iter().find(|i| i.name == name).map(|i| i.id));
                    let target =
                        crate::ui::inspector::coordinator::InspectorTarget::Identity(target_id);
                    let ok = app
                        .inspector_coordinator
                        .complete_save(&active_vault_id, gen, target);
                    if !ok {
                        tracing::warn!(
                            originating_vault = %active_vault_id,
                            current_vault = %app.active_vault_id,
                            gen = gen,
                            "Discarded stale save completion for identity"
                        );
                        return;
                    }

                    app.identity_modal = None;
                    app.load_identities(cx);
                    app.desktop.message = Some(format!("Identity '{}' saved successfully", name));

                    if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                        app.apply_navigation_request(req, None, cx);
                    } else {
                        app.open_identity_inspector_mode(
                            target_id,
                            crate::ui::inspector::InspectorMode::View,
                            cx,
                        );
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn delete_identity(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
        let vault_key = self.active_vault_key();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, vault_key.as_ref())
                    .await
                    .map_err(|error| error.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id_task) {
                    store.set_vault_id(vid);
                }
                let credential_store = shellumi_core::default_credential_store();
                let deleted = store
                    .delete_identity_with_secrets(id, &*credential_store)
                    .await
                    .map_err(|e| e.to_string())?;
                if !deleted {
                    return Err(format!("Identity with ID {} not found", id));
                }
                Ok::<(), String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(()) => {
                        if app.active_vault_id == active_vault_id {
                            app.load_identities(cx);
                            app.desktop.message = Some("Identity deleted successfully".into());
                        }
                    }
                    Err(err) => {
                        app.desktop.message = Some(format!("Failed to delete identity: {err}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

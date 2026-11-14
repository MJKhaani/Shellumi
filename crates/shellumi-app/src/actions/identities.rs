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
        let Some(modal) = &self.identity_modal else {
            return;
        };
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

        let Some(db_path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&db_path).await.map_err(|e| e.to_string())?;
                let credential_store = shellumi_core::default_credential_store();
                let nonce = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos();
                let account = format!("desktop-identity-{}-{nonce}", std::process::id());
                let mut created_refs = Vec::new();
                if !password.is_empty() {
                    let reference = shellumi_core::store_secret_to_ref(
                        credential_store.as_ref(),
                        shellumi_core::DEFAULT_KEYCHAIN_SERVICE,
                        &format!("{account}-password"),
                        &password,
                    )
                    .map_err(|e| e.to_string())?;
                    created_refs.push(reference.clone());
                    new_id.password_secret_ref = Some(reference);
                }
                if !passphrase.is_empty() {
                    match shellumi_core::store_secret_to_ref(
                        credential_store.as_ref(),
                        shellumi_core::DEFAULT_KEYCHAIN_SERVICE,
                        &format!("{account}-passphrase"),
                        &passphrase,
                    ) {
                        Ok(reference) => {
                            created_refs.push(reference.clone());
                            new_id.passphrase_secret_ref = Some(reference);
                        }
                        Err(error) => {
                            for reference in &created_refs {
                                let _ = shellumi_core::delete_secret_by_ref(
                                    credential_store.as_ref(),
                                    reference,
                                );
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
                        m.error = Some(err);
                        cx.notify();
                    }
                } else {
                    app.identity_modal = None;
                    app.load_identities(cx);
                    app.desktop.message = Some(format!("Identity '{}' saved successfully", name));
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

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
                let op_id = shellumi_core::VaultCredentialStore::generate_credential_id()
                    .map_err(|e| e.to_string())?;
                let _op_guard = HostStore::acquire_credential_op_guard(&op_id);
                let now_ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                let mut old_refs: Vec<String> = Vec::new();
                let mut planned_new_refs: Vec<String> = Vec::new();
                let mut staged_secrets: Vec<(String, String)> = Vec::new();

                if !password.is_empty() {
                    if let Some(prev) = new_id.password_secret_ref.take() {
                        old_refs.push(prev);
                    }
                    let reference = vault_store
                        .plan_unique_ref("identity-password")
                        .map_err(|e| e.to_string())?;
                    planned_new_refs.push(reference.clone());
                    staged_secrets.push((reference.clone(), password.clone()));
                    new_id.password_secret_ref = Some(reference);
                }
                if !passphrase.is_empty() {
                    if let Some(prev) = new_id.passphrase_secret_ref.take() {
                        old_refs.push(prev);
                    }
                    let reference = vault_store
                        .plan_unique_ref("identity-passphrase")
                        .map_err(|e| e.to_string())?;
                    planned_new_refs.push(reference.clone());
                    staged_secrets.push((reference.clone(), passphrase.clone()));
                    new_id.passphrase_secret_ref = Some(reference);
                }

                let has_journal = !planned_new_refs.is_empty() || !old_refs.is_empty();
                if has_journal {
                    store
                        .record_credential_operation(&shellumi_storage::CredentialOperationRecord {
                            operation_id: op_id.clone(),
                            vault_id: active_vault_id_task.clone(),
                            target_host_id: None,
                            target_identity_id: editing_id,
                            operation_type: if editing_id.is_some() {
                                "edit_identity".to_string()
                            } else {
                                "create_identity".to_string()
                            },
                            old_refs: old_refs.clone(),
                            planned_new_refs: planned_new_refs.clone(),
                            phase: shellumi_storage::CredentialOpPhase::Planned,
                            cleanup_status: shellumi_storage::CredentialCleanupStatus::Pending,
                            last_error: None,
                            updated_at: now_ts,
                        })
                        .await
                        .map_err(|e| e.to_string())?;
                }

                let mut staged_refs: Vec<String> = Vec::new();
                for (planned_ref, secret) in &staged_secrets {
                    if let Err(error) = vault_store.store_planned(planned_ref, secret) {
                        let mut rollback_ok = true;
                        let mut rollback_err = error.to_string();
                        for r in &staged_refs {
                            if let Err(del_err) = vault_store.delete(r) {
                                rollback_ok = false;
                                rollback_err =
                                    format!("{rollback_err}; rollback delete failed: {del_err}");
                            }
                        }
                        if has_journal {
                            let _ = store
                                .update_credential_operation_phase(
                                    &op_id,
                                    shellumi_storage::CredentialOpPhase::RolledBack,
                                    if rollback_ok {
                                        shellumi_storage::CredentialCleanupStatus::Completed
                                    } else {
                                        shellumi_storage::CredentialCleanupStatus::RetryPending
                                    },
                                    Some(&rollback_err),
                                )
                                .await;
                        }
                        return Err(error.to_string());
                    }
                    staged_refs.push(planned_ref.clone());
                }
                if has_journal && !staged_refs.is_empty() {
                    store
                        .update_credential_operation_phase(
                            &op_id,
                            shellumi_storage::CredentialOpPhase::Staged,
                            shellumi_storage::CredentialCleanupStatus::Pending,
                            None,
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                }

                let result = store
                    .save_identity_with_journal_tx(
                        editing_id,
                        &new_id,
                        if has_journal {
                            Some(op_id.as_str())
                        } else {
                            None
                        },
                    )
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string());

                if let Err(ref err_msg) = result {
                    let mut rollback_ok = true;
                    let mut rollback_err = err_msg.clone();
                    for r in &staged_refs {
                        if let Err(del_err) = vault_store.delete(r) {
                            rollback_ok = false;
                            rollback_err =
                                format!("{rollback_err}; rollback delete failed: {del_err}");
                        }
                    }
                    if has_journal {
                        let _ = store
                            .update_credential_operation_phase(
                                &op_id,
                                shellumi_storage::CredentialOpPhase::RolledBack,
                                if rollback_ok {
                                    shellumi_storage::CredentialCleanupStatus::Completed
                                } else {
                                    shellumi_storage::CredentialCleanupStatus::RetryPending
                                },
                                Some(&rollback_err),
                            )
                            .await;
                    }
                } else if has_journal {
                    let mut cleanup_ok = true;
                    let mut cleanup_err = None;
                    for old_r in &old_refs {
                        if !store.is_credential_ref_in_use(old_r).await.unwrap_or(true)
                            && vault_store.validate_reference(old_r).is_ok()
                        {
                            if let Err(e) = vault_store.delete(old_r) {
                                cleanup_ok = false;
                                cleanup_err = Some(e.to_string());
                            }
                        }
                    }
                    let _ = store
                        .update_credential_operation_phase(
                            &op_id,
                            if cleanup_ok {
                                shellumi_storage::CredentialOpPhase::Completed
                            } else {
                                shellumi_storage::CredentialOpPhase::Committed
                            },
                            if cleanup_ok {
                                shellumi_storage::CredentialCleanupStatus::Completed
                            } else {
                                shellumi_storage::CredentialCleanupStatus::RetryPending
                            },
                            cleanup_err.as_deref(),
                        )
                        .await;
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
                let vid = shellumi_core::VaultId::new(&active_vault_id_task)
                    .map_err(|e| e.to_string())?;
                store.set_vault_id(vid.clone());
                let existing = store.get_identity(id).await.map_err(|e| e.to_string())?;
                let mut old_refs = Vec::new();
                if let Some(ident) = existing {
                    if let Some(r) = ident.password_secret_ref {
                        old_refs.push(r);
                    }
                    if let Some(r) = ident.passphrase_secret_ref {
                        old_refs.push(r);
                    }
                }
                let op_id = shellumi_core::VaultCredentialStore::generate_credential_id()
                    .map_err(|e| e.to_string())?;
                let _op_guard = HostStore::acquire_credential_op_guard(&op_id);
                let has_journal = !old_refs.is_empty();
                if has_journal {
                    let now_ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    store
                        .record_credential_operation(&shellumi_storage::CredentialOperationRecord {
                            operation_id: op_id.clone(),
                            vault_id: active_vault_id_task.clone(),
                            target_host_id: None,
                            target_identity_id: Some(id),
                            operation_type: "delete_identity".to_string(),
                            old_refs: old_refs.clone(),
                            planned_new_refs: Vec::new(),
                            phase: shellumi_storage::CredentialOpPhase::Planned,
                            cleanup_status: shellumi_storage::CredentialCleanupStatus::Pending,
                            last_error: None,
                            updated_at: now_ts,
                        })
                        .await
                        .map_err(|e| e.to_string())?;
                }
                let deleted = store
                    .delete_identity_with_journal_tx(
                        id,
                        if has_journal {
                            Some(op_id.as_str())
                        } else {
                            None
                        },
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                if !deleted {
                    return Err(format!("Identity with ID {} not found", id));
                }
                if has_journal {
                    let credential_store = shellumi_core::default_credential_store();
                    let vault_store =
                        shellumi_core::VaultCredentialStore::new(credential_store.as_ref(), &vid);
                    let mut cleanup_ok = true;
                    let mut cleanup_err = None;
                    for r in &old_refs {
                        if !store.is_credential_ref_in_use(r).await.unwrap_or(true)
                            && vault_store.validate_reference(r).is_ok()
                        {
                            if let Err(e) = vault_store.delete(r) {
                                cleanup_ok = false;
                                cleanup_err = Some(e.to_string());
                            }
                        }
                    }
                    let _ = store
                        .update_credential_operation_phase(
                            &op_id,
                            if cleanup_ok {
                                shellumi_storage::CredentialOpPhase::Completed
                            } else {
                                shellumi_storage::CredentialOpPhase::Committed
                            },
                            if cleanup_ok {
                                shellumi_storage::CredentialCleanupStatus::Completed
                            } else {
                                shellumi_storage::CredentialCleanupStatus::RetryPending
                            },
                            cleanup_err.as_deref(),
                        )
                        .await;
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

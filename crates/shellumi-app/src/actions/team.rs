//! Team actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_team_modal(&mut self, cx: &mut Context<Self>) {
        let new_vault_name =
            cx.new(|cx| TextInput::new(cx, "", "Team vault name (e.g. Platform Team)"));
        let new_vault_desc = cx.new(|cx| TextInput::new(cx, "", "Description (optional)"));
        let invite_user_id =
            cx.new(|cx| TextInput::new(cx, "", "Collaborator user ID (e.g. dev@corp)"));
        let invite_name =
            cx.new(|cx| TextInput::new(cx, "", "Display name (e.g. Alice Developer)"));

        self.team_modal = Some(TeamModalState {
            active_tab: TeamModalTab::Vaults,
            vaults: Vec::new(),
            selected_vault_id: None,
            members: Vec::new(),
            audit_records: Vec::new(),
            new_vault_name,
            new_vault_desc,
            invite_user_id,
            invite_name,
            invite_public_key: cx
                .new(|cx| TextInput::new(cx, "", "Collaborator OpenSSH public key")),
            invite_role: TeamRole::Editor,
            error: None,
            status_message: None,
            is_loading: true,
        });
        cx.notify();
        self.load_team_vaults(cx);
    }

    pub(crate) fn dismiss_team_modal(&mut self, cx: &mut Context<Self>) {
        self.team_modal = None;
        cx.notify();
    }

    fn load_team_vaults(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            if let Some(m) = &mut self.team_modal {
                m.is_loading = false;
                m.error = Some(
                    "No database path configured. Team vaults require a local database.".into(),
                );
            }
            cx.notify();
            return;
        };

        if let Some(m) = &mut self.team_modal {
            m.is_loading = true;
            m.error = None;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let vaults = store
                    .list_shared_vaults()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(vaults)
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    modal.is_loading = false;
                    match result {
                        Ok(vaults) => {
                            let first_id = vaults.first().map(|v| v.id.clone());
                            modal.vaults = vaults;
                            if modal.selected_vault_id.is_none() {
                                modal.selected_vault_id = first_id;
                            }
                            if let Some(vid) = modal.selected_vault_id.clone() {
                                app.refresh_team_modal_vault(&vid, cx);
                            } else {
                                cx.notify();
                            }
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn refresh_team_modal_vault(&mut self, vault_id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let vid = vault_id.to_string();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let members = store
                    .list_team_members(&vid)
                    .await
                    .map_err(|e| e.to_string())?;
                let audit = store
                    .list_audit_log(&vid, Some(50))
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((members, audit))
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok((members, audit)) => {
                            modal.members = members;
                            modal.audit_records = audit;
                        }
                        Err(e) => {
                            modal.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn create_team_vault(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let Some(modal) = &mut self.team_modal else {
            return;
        };

        let name = modal.new_vault_name.read(cx).value().trim().to_string();
        let desc = modal.new_vault_desc.read(cx).value().trim().to_string();
        let desc_opt = if desc.is_empty() { None } else { Some(desc) };

        if name.is_empty() {
            modal.error = Some("Vault name cannot be empty".into());
            cx.notify();
            return;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let vault_id = format!("vlt-{now_secs}-{now_nanos}");
        let now_iso = format!("{now_secs}");

        let vault = match SharedVault::new(&vault_id, &name, desc_opt, "local_owner", &now_iso) {
            Ok(v) => v,
            Err(e) => {
                modal.error = Some(e.to_string());
                cx.notify();
                return;
            }
        };

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vault_id,
            AuditAction::VaultCreated,
            "local_owner",
            None,
            Some(format!("Created vault '{name}'")),
            &now_iso,
        );

        modal.is_loading = true;
        modal.error = None;
        modal.status_message = Some(format!("Creating vault '{name}'…"));
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .create_shared_vault(&vault)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    modal.is_loading = false;
                    match result {
                        Ok(()) => {
                            modal.status_message = Some("Vault created successfully.".into());
                            modal
                                .new_vault_name
                                .update(cx, |input, cx| input.set_value("", cx));
                            modal
                                .new_vault_desc
                                .update(cx, |input, cx| input.set_value("", cx));
                            modal.selected_vault_id = Some(vault_id.clone());
                            app.load_team_vaults(cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn rotate_team_vault_key(&mut self, vault_id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let vid = vault_id.to_string();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let now_iso = format!("{now_secs}");

        let current_epoch = self
            .team_modal
            .as_ref()
            .and_then(|m| m.vaults.iter().find(|v| v.id == vid))
            .map(|v| v.current_epoch)
            .unwrap_or(1);
        let new_epoch = current_epoch + 1;

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vid,
            AuditAction::KeyRotated,
            "local_owner",
            None,
            Some(format!("Rotated key to epoch {new_epoch}")),
            &now_iso,
        );

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .update_shared_vault_epoch(&vid, new_epoch, &now_iso)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok(()) => {
                            modal.status_message =
                                Some(format!("Vault key rotated to epoch {new_epoch}."));
                            app.load_team_vaults(cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn delete_team_vault(&mut self, vault_id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let vid = vault_id.to_string();
        let vid_task = vid.clone();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .delete_shared_vault(&vid_task)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok(()) => {
                            modal.status_message = Some("Vault deleted.".into());
                            if modal.selected_vault_id.as_deref() == Some(&vid) {
                                modal.selected_vault_id = None;
                                modal.members.clear();
                                modal.audit_records.clear();
                            }
                            app.load_team_vaults(cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn invite_team_member(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let Some(modal) = &mut self.team_modal else {
            return;
        };
        let Some(vid) = modal.selected_vault_id.clone() else {
            modal.error = Some("Please select a shared vault first".into());
            cx.notify();
            return;
        };

        let user_id = modal.invite_user_id.read(cx).value().trim().to_string();
        let display_name = modal.invite_name.read(cx).value().trim().to_string();
        let role = modal.invite_role;
        let public_key = modal.invite_public_key.read(cx).value().trim().to_owned();
        if shellumi_core::fingerprint_public_key(&public_key).is_err() {
            modal.error = Some("Provide a valid OpenSSH public key for this member.".into());
            cx.notify();
            return;
        }

        if user_id.is_empty() || display_name.is_empty() {
            modal.error = Some("User ID and display name are required".into());
            cx.notify();
            return;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let member_id = format!("mem-{now_secs}-{now_nanos}");
        let now_iso = format!("{now_secs}");

        let member = TeamMember::new(
            &member_id,
            &vid,
            &user_id,
            &display_name,
            &public_key,
            role,
            &now_iso,
            "local_owner",
        );

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vid,
            AuditAction::MemberInvited,
            "local_owner",
            Some(user_id.clone()),
            Some(format!("Invited '{display_name}' as {role}")),
            &now_iso,
        );

        modal.is_loading = true;
        modal.error = None;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .add_team_member(&member)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    modal.is_loading = false;
                    match result {
                        Ok(()) => {
                            modal.status_message =
                                Some(format!("Invited member '{display_name}' as {role}."));
                            modal
                                .invite_user_id
                                .update(cx, |input, cx| input.set_value("", cx));
                            modal
                                .invite_name
                                .update(cx, |input, cx| input.set_value("", cx));
                            app.refresh_team_modal_vault(&vid, cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn update_member_role(
        &mut self,
        member_id: &str,
        new_role: TeamRole,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let mid = member_id.to_string();
        let Some(modal) = &self.team_modal else {
            return;
        };
        let Some(vid) = modal.selected_vault_id.clone() else {
            return;
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let now_iso = format!("{now_secs}");

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vid,
            AuditAction::MemberRoleUpdated,
            "local_owner",
            Some(mid.clone()),
            Some(format!("Updated role to {new_role}")),
            &now_iso,
        );

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .update_team_member_role(&mid, new_role)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok(()) => {
                            modal.status_message =
                                Some(format!("Updated member role to {new_role}."));
                            app.refresh_team_modal_vault(&vid, cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn revoke_team_member(&mut self, member_id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let mid = member_id.to_string();
        let Some(modal) = &self.team_modal else {
            return;
        };
        let Some(vid) = modal.selected_vault_id.clone() else {
            return;
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let now_iso = format!("{now_secs}");

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vid,
            AuditAction::MemberRevoked,
            "local_owner",
            Some(mid.clone()),
            Some("Access revoked".into()),
            &now_iso,
        );

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .revoke_team_member(&mid, &now_iso)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok(()) => {
                            modal.status_message =
                                Some("Member revoked. Recommend rotating vault key next.".into());
                            app.refresh_team_modal_vault(&vid, cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn toggle_multiplayer_on_active_tab(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };

        if tab.multiplayer.is_some() {
            tab.multiplayer = None;
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let session_id = format!("collab-{}", tab.id);
            tab.multiplayer = Some(MultiplayerSession::new(
                session_id,
                None,
                "local_user",
                format!("{now}"),
            ));
        }
        cx.notify();
    }

    pub(crate) fn grant_multiplayer_control(&mut self, target_user: &str, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };
        if let Some(session) = &mut tab.multiplayer {
            let requester = session.owner_id.clone();
            let _ = session.grant_keyboard_control(&requester, target_user);
            cx.notify();
        }
    }

    pub(crate) fn revoke_multiplayer_control(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };
        if let Some(session) = &mut tab.multiplayer {
            let requester = session.owner_id.clone();
            let _ = session.revoke_keyboard_control(&requester);
            cx.notify();
        }
    }

    pub(crate) fn add_multiplayer_demo_participant(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };
        if let Some(session) = &mut tab.multiplayer {
            let count = session.participants.len();
            let user_id = format!("teammate_{count}");
            let name = format!("Collaborator {count}");
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let _ = session.add_participant(&user_id, &name, format!("{now}"));
            cx.notify();
        }
    }

    // =========================================================================
    // Infrastructure Integrations: Cloud Providers & Discovered Inventory
    // =========================================================================
}

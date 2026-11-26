//! Team actions.
use super::*;
use shellumi_core::server::crypto::{
    decrypt_collab_frame, encrypt_collab_frame, generate_collab_session_key,
};
use shellumi_core::server::types::{
    CollabFrameType, CollabPollQuery, CollabPollResponse, CreateCollabSessionRequest,
    GrantControlRequest, SendCollabFrameRequest,
};
use shellumi_core::server::ShellumiServerClient;

impl Shellumi {
    #[allow(dead_code)]
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
        let Some(path) = self.active_vault_db_path() else {
            if let Some(m) = &mut self.team_modal {
                m.is_loading = false;
                m.error = Some(
                    "No database path configured. Team vaults require a local database.".into(),
                );
            }
            cx.notify();
            return;
        };
        let active_key = self.active_vault_key();

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
                let store = HostStore::open_with_key(&path, active_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
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
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let active_key = self.active_vault_key();
        let vid = vault_id.to_string();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open_with_key(&path, active_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
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
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let active_key = self.active_vault_key();
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
                let mut store = HostStore::open_with_key(&path, active_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
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

    pub(crate) fn report_team_rotation_unavailable(&mut self, cx: &mut Context<Self>) {
        let message = "Key rotation and revocation require verified device enrollment and epoch-key synchronization.".to_string();
        tracing::warn!("Blocked unverified team key change");
        if let Some(modal) = &mut self.team_modal {
            modal.error = Some(message.clone());
            modal.status_message = None;
        }
        self.desktop.message = Some(message);
        cx.notify();
    }

    pub(crate) fn rotate_team_vault_key(&mut self, _vault_id: &str, cx: &mut Context<Self>) {
        self.report_team_rotation_unavailable(cx);
    }

    pub(crate) fn delete_team_vault(&mut self, vault_id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let active_key = self.active_vault_key();
        let vid = vault_id.to_string();
        let vid_task = vid.clone();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, active_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
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
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let active_key = self.active_vault_key();
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
                let mut store = HostStore::open_with_key(&path, active_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
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
                            modal.status_message = Some(format!(
                                "Saved local collaborator '{display_name}' ({role}). Use Team Inspector on a remote-bound vault to send a server invitation."
                            ));
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
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let active_key = self.active_vault_key();
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
                let mut store = HostStore::open_with_key(&path, active_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
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

    pub(crate) fn revoke_team_member(&mut self, _member_id: &str, cx: &mut Context<Self>) {
        self.report_team_rotation_unavailable(cx);
    }

    pub(crate) fn toggle_multiplayer_on_active_tab(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };

        if tab.multiplayer.is_some() {
            if let Some(collab) = &tab.collab_state {
                let session_id = collab.session_id;
                let server_url = collab.server_url.clone();
                let token = collab.token.clone();
                cx.background_executor()
                    .spawn(async move {
                        let mut client = ShellumiServerClient::new(&server_url);
                        client.set_token(token);
                        let _ = client.terminate_collab_session(session_id);
                    })
                    .detach();
            }
            tab.multiplayer = None;
            tab.collab_state = None;
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
            tab.collab_state = None;
        }
        cx.notify();
    }

    #[allow(dead_code)]
    pub(crate) fn start_server_collab_session(
        &mut self,
        server_url: &str,
        token: &str,
        title: &str,
        mode: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };
        let tab_id = tab.id;

        let key = match generate_collab_session_key() {
            Ok(k) => k,
            Err(e) => {
                eprintln!("Failed to generate collab session key: {e}");
                return;
            }
        };

        let server_url_owned = server_url.to_string();
        let token_owned = token.to_string();
        let title_owned = title.to_string();
        let mode_owned = mode.to_string();

        let task = cx.background_executor().spawn(async move {
            let mut client = ShellumiServerClient::new(&server_url_owned);
            client.set_token(token_owned);
            client.create_collab_session(&CreateCollabSessionRequest {
                title: title_owned,
                mode: mode_owned,
                ttl_secs: Some(7200),
            })
        });

        let server_url_cloned = server_url.to_string();
        let token_cloned = token.to_string();

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(tab) = app.tabs.iter_mut().find(|t| t.id == tab_id) {
                    match result {
                        Ok(resp) => {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            tab.multiplayer = Some(
                                MultiplayerSession::new(
                                    format!("srv-{}", resp.session_id),
                                    None,
                                    "local_user",
                                    format!("{now}"),
                                )
                                .with_server_coordination(
                                    resp.session_id,
                                    resp.lease_token,
                                    resp.mode,
                                    true,
                                ),
                            );
                            tab.collab_state = Some(TabCollabState {
                                session_id: resp.session_id,
                                key,
                                outbound_seq: 0,
                                last_polled_seq: 0,
                                server_url: server_url_cloned,
                                token: token_cloned,
                            });
                            cx.notify();
                        }
                        Err(e) => {
                            eprintln!("Failed to create server collab session: {e}");
                        }
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn grant_multiplayer_control(&mut self, target_user: &str, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };
        let tab_id = tab.id;

        if let Some(collab) = &tab.collab_state {
            let session_id = collab.session_id;
            let server_url = collab.server_url.clone();
            let token = collab.token.clone();
            let target_uuid = uuid::Uuid::parse_str(target_user).unwrap_or_else(|_| {
                uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, target_user.as_bytes())
            });
            let target_user_owned = target_user.to_string();

            let task = cx.background_executor().spawn(async move {
                let mut client = ShellumiServerClient::new(&server_url);
                client.set_token(token);
                client.grant_collab_control(
                    session_id,
                    &GrantControlRequest {
                        target_user_id: target_uuid,
                        target_device_id: target_uuid,
                        ttl_secs: Some(300),
                    },
                )
            });

            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Some(tab) = app.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let Ok(resp) = result {
                            if let Some(session) = &mut tab.multiplayer {
                                session
                                    .advance_lease_token(resp.lease_token, Some(target_user_owned));
                                cx.notify();
                            }
                        }
                    }
                });
            })
            .detach();
            return;
        }

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
        let tab_id = tab.id;

        if let Some(collab) = &tab.collab_state {
            let session_id = collab.session_id;
            let server_url = collab.server_url.clone();
            let token = collab.token.clone();

            let task = cx.background_executor().spawn(async move {
                let mut client = ShellumiServerClient::new(&server_url);
                client.set_token(token);
                client.revoke_collab_control(session_id)
            });

            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Some(tab) = app.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let Ok(resp) = result {
                            if let Some(session) = &mut tab.multiplayer {
                                let owner = session.owner_id.clone();
                                session.advance_lease_token(resp.lease_token, Some(owner));
                                cx.notify();
                            }
                        }
                    }
                });
            })
            .detach();
            return;
        }

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

    pub(crate) fn broadcast_collab_output(
        &mut self,
        tab_id: u64,
        bytes: &[u8],
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(mp) = &tab.multiplayer else {
            return;
        };
        if !mp.is_active || !mp.is_host {
            return;
        }
        let Some(collab) = &mut tab.collab_state else {
            return;
        };
        collab.outbound_seq = collab.outbound_seq.wrapping_add(1);
        let seq = collab.outbound_seq;
        let session_id = collab.session_id;
        let lease_token = mp.lease_token;
        let server_url = collab.server_url.clone();
        let token = collab.token.clone();

        match encrypt_collab_frame(&collab.key, session_id, seq, bytes) {
            Ok(envelope) => {
                cx.background_executor()
                    .spawn(async move {
                        let mut client = ShellumiServerClient::new(&server_url);
                        client.set_token(token);
                        let _ = client.send_collab_frame(
                            session_id,
                            &SendCollabFrameRequest {
                                frame_type: CollabFrameType::Output,
                                lease_token: Some(lease_token),
                                encrypted_envelope: envelope,
                            },
                        );
                    })
                    .detach();
            }
            Err(e) => {
                eprintln!("Failed to encrypt collab output frame: {e}");
            }
        }
    }

    pub(crate) fn forward_collab_input(
        &mut self,
        tab_id: u64,
        bytes: &[u8],
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(mp) = &tab.multiplayer else {
            return;
        };
        if !mp.is_active {
            return;
        }
        if !mp.can_write("local_user", Some(mp.lease_token)) {
            return;
        }
        let Some(collab) = &mut tab.collab_state else {
            return;
        };
        collab.outbound_seq = collab.outbound_seq.wrapping_add(1);
        let seq = collab.outbound_seq;
        let session_id = collab.session_id;
        let lease_token = mp.lease_token;
        let server_url = collab.server_url.clone();
        let token = collab.token.clone();

        match encrypt_collab_frame(&collab.key, session_id, seq, bytes) {
            Ok(envelope) => {
                cx.background_executor()
                    .spawn(async move {
                        let mut client = ShellumiServerClient::new(&server_url);
                        client.set_token(token);
                        let _ = client.send_collab_frame(
                            session_id,
                            &SendCollabFrameRequest {
                                frame_type: CollabFrameType::Input,
                                lease_token: Some(lease_token),
                                encrypted_envelope: envelope,
                            },
                        );
                    })
                    .detach();
            }
            Err(e) => {
                eprintln!("Failed to encrypt collab input frame: {e}");
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn poll_server_collab_relay(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(mp) = &tab.multiplayer else {
            return;
        };
        if !mp.is_active {
            return;
        }
        let Some(collab) = &tab.collab_state else {
            return;
        };

        let session_id = collab.session_id;
        let last_seq = collab.last_polled_seq;
        let server_url = collab.server_url.clone();
        let token = collab.token.clone();
        let key = collab.key.clone();
        let is_host = mp.is_host;

        let task = cx.background_executor().spawn(async move {
            let mut client = ShellumiServerClient::new(&server_url);
            client.set_token(token);
            client.poll_collab_frames(
                session_id,
                &CollabPollQuery {
                    since_sequence: Some(last_seq),
                    limit: Some(50),
                },
            )
        });

        cx.spawn(async move |app, cx| {
            if let Ok(resp) = task.await {
                let _ = app.update(cx, |app, cx| {
                    app.handle_polled_collab_frames(tab_id, resp, &key, is_host, cx);
                });
            }
        })
        .detach();
    }

    #[allow(dead_code)]
    pub(crate) fn handle_polled_collab_frames(
        &mut self,
        tab_id: u64,
        resp: CollabPollResponse,
        key: &zeroize::Zeroizing<[u8; 32]>,
        is_host: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(mp) = &mut tab.multiplayer else {
            return;
        };
        if !mp.is_active {
            return;
        }
        let Some(collab) = &mut tab.collab_state else {
            return;
        };

        for frame in resp.frames {
            if frame.sequence <= collab.last_polled_seq {
                continue;
            }
            collab.last_polled_seq = frame.sequence;

            match frame.frame_type {
                CollabFrameType::Output => {
                    if !is_host {
                        if let Ok(decrypted) = decrypt_collab_frame(
                            key,
                            collab.session_id,
                            frame.sequence,
                            &frame.encrypted_envelope,
                        ) {
                            let active_pane =
                                tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
                            if let Some(pane) = tab.panes.get_mut(active_pane) {
                                pane.terminal.process_bytes(&decrypted);
                                pane.scroll.scroll_to_bottom();
                            }
                        }
                    }
                }
                CollabFrameType::Input => {
                    if is_host {
                        if let Ok(decrypted) = decrypt_collab_frame(
                            key,
                            collab.session_id,
                            frame.sequence,
                            &frame.encrypted_envelope,
                        ) {
                            let active_pane =
                                tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
                            if let Some(pane) = tab.panes.get_mut(active_pane) {
                                if let Some(sess) = &pane.session {
                                    let _ = sess.input(decrypted);
                                }
                            }
                        }
                    }
                }
                CollabFrameType::ControlGranted => {
                    if let Some(token) = frame.lease_token {
                        mp.advance_lease_token(token, Some(frame.sender_user_id.to_string()));
                    }
                }
                CollabFrameType::ControlRevoked => {
                    if let Some(token) = frame.lease_token {
                        mp.advance_lease_token(token, Some(mp.owner_id.clone()));
                    }
                }
                CollabFrameType::Terminated => {
                    mp.is_active = false;
                }
                CollabFrameType::Signal => {}
            }
        }
        cx.notify();
    }

    // =========================================================================
    // Infrastructure Integrations: Cloud Providers & Discovered Inventory
    // =========================================================================
}

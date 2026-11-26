#![allow(dead_code)]
//! Team member and invitation inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout::{self, InspectorPresentation};
use crate::{Shellumi, TeamFormState, TextInput};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};
use shellumi_core::{TeamMember, TeamRole};
use shellumi_storage::HostStore;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
struct InvitationRequest {
    epoch: Arc<AtomicU64>,
    generation: u64,
}

impl InvitationRequest {
    fn capture(epoch: &Arc<AtomicU64>) -> Self {
        Self {
            epoch: Arc::clone(epoch),
            generation: epoch.load(Ordering::Acquire),
        }
    }

    fn is_current(&self) -> bool {
        self.epoch.load(Ordering::Acquire) == self.generation
    }
    fn can_deliver(
        &self,
        inspector: &super::coordinator::InspectorCoordinator,
        vault: &str,
        generation: u64,
    ) -> bool {
        self.is_current()
            && inspector
                .active()
                .is_some_and(|state| state.vault_id == vault && state.generation == generation)
    }

    async fn session_is_current(
        &self,
        registry: &HostStore,
        binding: &shellumi_storage::server_sync::VerifiedRemoteVaultBinding,
        session: &shellumi_core::SecretString,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<bool, shellumi_storage::StorageError> {
        if !self.is_current() || registry.is_vault_locked(binding.local_vault_id()).await? {
            return Ok(false);
        }
        let current = registry
            .resolve_verified_remote_vault_binding(binding.local_vault_id(), None)
            .await?;
        let token = registry
            .get_vault_server_connection(current.server_connection_id())
            .await?
            .and_then(|connection| connection.resolve_auth_token(credentials));
        Ok(current == *binding && token.as_ref() == Some(session) && self.is_current())
    }
}

impl Shellumi {
    /// Render the unified Team Member Inspector for viewing or inviting collaborators.
    fn invite_is_in_scope(&self, invite: &crate::CreatedInvite) -> bool {
        !self
            .available_vaults
            .iter()
            .any(|vault| vault.id == self.active_vault_id && vault.is_locked)
            && invite.is_visible(
                self.invite_scope.as_ref(),
                &self.active_vault_id,
                self.inspector_coordinator.active(),
                invite_timestamp(),
            )
    }

    fn copy_current_invitation(&mut self, invitation_id: &str, cx: &mut Context<Self>) {
        self.clear_out_of_scope_invite();
        let Some(invite) = self
            .last_created_invite
            .as_ref()
            .filter(|invite| invite.invitation_id == invitation_id && invite.token.is_some())
        else {
            return;
        };
        let (Some(registry_path), Some(vault_path)) =
            (self.registry_db_path(), self.active_vault_db_path())
        else {
            self.invalidate_invitation_scope();
            cx.notify();
            return;
        };
        let registry_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let vault_key = self.active_vault_key();
        let scope = invite.scope.clone();
        let session = invite.issuing_session.clone();
        let request = InvitationRequest::capture(&self.invitation_generation);
        let background_request = request.clone();
        let invitation_id = invitation_id.to_owned();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread().build().ok()?;
            runtime.block_on(async {
                if !background_request.is_current() {
                    return None;
                }
                let registry = HostStore::open_with_key(&registry_path, registry_key.as_ref())
                    .await
                    .ok()?;
                let binding = scope.binding;
                let credentials = shellumi_core::default_credential_store();
                if !background_request
                    .session_is_current(&registry, &binding, &session, &*credentials)
                    .await
                    .ok()?
                {
                    return None;
                }
                let connection = registry
                    .get_vault_server_connection(binding.server_connection_id())
                    .await
                    .ok()??;
                let mut store = HostStore::open_with_key(&vault_path, vault_key.as_ref())
                    .await
                    .ok()?;
                store.set_vault_id(shellumi_core::VaultId::new(binding.local_vault_id()).ok()?);
                let mut client =
                    shellumi_core::server::ShellumiServerClient::new(&connection.base_url);
                client.set_token(session.expose_secret().to_owned());
                if binding.authenticate_vault_member(&store, &client).ok()? != scope.account {
                    return None;
                }
                background_request
                    .session_is_current(&registry, &binding, &session, &*credentials)
                    .await
                    .ok()?
                    .then_some(())
            })
        });
        cx.spawn(async move |app, cx| {
            let verified = task.await.is_some();
            let _ = app.update(cx, |app, cx| {
                if !request.is_current() {
                    return;
                }
                if !verified {
                    tracing::warn!(
                        "Discarded invitation after live authentication verification failed"
                    );
                    app.invalidate_invitation_scope();
                    cx.notify();
                    return;
                }
                if let Some(token) = app.last_created_invite.as_ref().and_then(|invite| {
                    invite.copy_token(
                        &invitation_id,
                        app.invite_scope.as_ref(),
                        &app.active_vault_id,
                        app.inspector_coordinator.active(),
                        invite_timestamp(),
                    )
                }) {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(token.to_owned()));
                    app.desktop.message =
                        Some("Server invitation token copied to clipboard.".into());
                    tracing::debug!("Copied invitation after checking the authenticated vault");
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn invalidate_invitation_scope(&mut self) {
        self.invitation_generation.fetch_add(1, Ordering::AcqRel);
        self.invite_scope = None;
        self.pending_invitation_inspector = None;
        self.last_created_invite = None;
        self.team_invite_token_input = None;
    }

    pub(crate) fn clear_out_of_scope_invite(&mut self) {
        let inspector = self
            .inspector_coordinator
            .active()
            .map(|state| state.generation);
        let request_left_inspector = self.pending_invitation_inspector.is_some()
            && self.pending_invitation_inspector != inspector;
        let stored_invite_out_of_scope = self
            .last_created_invite
            .as_ref()
            .is_some_and(|invite| !self.invite_is_in_scope(invite));
        if request_left_inspector || stored_invite_out_of_scope {
            self.invalidate_invitation_scope();
        }
    }

    pub(crate) fn render_team_inspector(
        &mut self,
        member_id: Option<String>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_team_view_inspector(member_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_team_edit_inspector(member_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render team member inspector in View mode.
    fn render_team_view_inspector(
        &mut self,
        member_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let member = member_id
            .as_deref()
            .and_then(|id| {
                self.team_members
                    .iter()
                    .find(|m| m.id == id || m.user_id == id)
            })
            .or_else(|| self.team_members.first())
            .cloned();

        let Some(member) = member else {
            let close_btn = InspectorShell::close_button(
                "close-team-inspector-btn",
                false,
                |app, _, _, cx| {
                    app.inspector_close(cx);
                },
                cx,
            );
            let header = InspectorShell::header(
                "Team Member".to_string(),
                Some("Vault Collaborator"),
                None,
                None,
                Some(close_btn),
            );
            return InspectorShell::new(
                header,
                div()
                    .p_4()
                    .text_color(text_secondary())
                    .child("No team collaborator selected or enrolled yet.")
                    .children(self.team_enrollment_error.clone().map(error_notice))
                    .children(self.render_enrollment_recovery(cx)),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let mem_id = member.id.clone();
        let m_name = member.display_name.clone();
        let user_id = member.user_id.clone();
        let m_role = member.role.badge_label().to_string();
        let is_revoked = member.revoked;
        let enrolled = member.enrolled_at.clone();
        let pubkey_fp = if !member.public_key.is_empty() {
            shellumi_core::fingerprint_public_key(&member.public_key)
                .unwrap_or_else(|_| "ssh-key-configured".into())
        } else {
            "None configured".to_string()
        };

        let badge = if is_revoked {
            div()
                .px_2()
                .py_0p5()
                .rounded_md()
                .bg(rgb(0x3a1e1e))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(danger())
                .child("Revoked")
        } else {
            div()
                .px_2()
                .py_0p5()
                .rounded_md()
                .bg(rgb(0x183322))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(0x3dd68c))
                .child(m_role.clone())
        };

        let edit_mem_id = mem_id.clone();
        let edit_btn = div()
            .id("inspector-edit-team-btn")
            .px_2p5()
            .py_1()
            .rounded_md()
            .bg(surface())
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(text_primary())
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.open_team_inspector_mode(Some(edit_mem_id.clone()), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let close_btn = InspectorShell::close_button(
            "close-team-inspector-btn",
            false,
            |app, _, _, cx| {
                app.inspector_close(cx);
            },
            cx,
        );
        let header = InspectorShell::header(
            m_name.clone(),
            Some("Vault Collaborator"),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            Some(close_btn),
        );

        let mut body = div().flex().flex_col().gap_4();

        if let Some(ref err_msg) = self.team_enrollment_error {
            body = body.child(error_notice(err_msg.clone()));
        }

        body = body.children(self.render_enrollment_recovery(cx));

        // Action buttons
        let edit_id_action = mem_id.clone();
        let delete_id_action = mem_id.clone();
        let delete_name = m_name.clone();

        let mut actions_row = div().flex().gap_2().child(
            AppButton::new(
                "team-view-edit-btn",
                "Change Role",
                ButtonVariant::Secondary,
            )
            .on_click(cx.listener(move |app, _, _, cx| {
                app.open_team_inspector_mode(Some(edit_id_action.clone()), InspectorMode::Edit, cx);
            })),
        );

        if !is_revoked && !member.is_enrolled() {
            let enroll_mem_id = mem_id.clone();
            let busy = self.team_enrollment_busy;
            actions_row = actions_row.child(
                AppButton::new(
                    "team-view-enroll-device-btn",
                    if busy {
                        "Enrolling..."
                    } else {
                        "Enroll Current Device"
                    },
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.enroll_member_device(enroll_mem_id.clone(), "This Device".to_string(), cx);
                })),
            );
        }

        if !is_revoked {
            actions_row = actions_row.child(
                AppButton::new(
                    "team-view-revoke-btn",
                    "Revoke Access",
                    ButtonVariant::Danger,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.delete_confirm = Some(crate::DeleteTarget::TeamMember(
                        delete_id_action.clone(),
                        delete_name.clone(),
                    ));
                    cx.notify();
                })),
            );
        }
        body = body.child(actions_row);

        // Details
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Display Name", m_name))
            .child(self.render_detail_row("User Identifier", user_id))
            .child(self.render_detail_row("Assigned Role", m_role))
            .child(self.render_detail_row("Access Status", member.enrollment_label().to_owned()))
            .child(
                self.render_detail_row(
                    "X25519 Agreement Key",
                    member
                        .agreement_public_key
                        .as_deref()
                        .map(|k| {
                            let prefix: String = k.chars().take(16).collect();
                            format!("{prefix}...")
                        })
                        .unwrap_or_else(|| "Not enrolled".to_string()),
                ),
            )
            .child(self.render_detail_row("Invited At", enrolled))
            .child(self.render_detail_row("SSH Key Fingerprint", pubkey_fp));
        body = body.child(simple_section("Collaborator Details", details_items));

        // Recipient-side Invitation Acceptance (when not yet enrolled)
        if !is_revoked && !member.is_enrolled() {
            let token_input = self
                .team_invite_token_input
                .get_or_insert_with(|| {
                    cx.new(|cx| TextInput::new(cx, "", "Paste server invitation token"))
                })
                .clone();
            let accept_mem_id = mem_id.clone();
            let accept_name = member.display_name.clone();
            let accept_ssh = member.public_key.clone();
            let busy = self.team_enrollment_busy;
            body = body.child(simple_section(
                "Accept Server Invitation",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(simple_field("Invitation Token", token_input.clone()))
                    .child(
                        AppButton::new(
                            "team-accept-invite-btn",
                            if busy {
                                "Accepting..."
                            } else {
                                "Accept Invitation & Enroll Device"
                            },
                            ButtonVariant::Primary,
                        )
                        .on_click(cx.listener(move |app, _, _, cx| {
                            let tok = token_input.read(cx).value().to_string();
                            app.accept_team_invitation(
                                accept_mem_id.clone(),
                                accept_name.clone(),
                                accept_ssh.clone(),
                                tok,
                                "This Device".to_string(),
                                cx,
                            );
                        })),
                    ),
            ));
        }

        // Enrolled Devices for this collaborator
        let member_devices: Vec<_> = self
            .enrolled_devices
            .iter()
            .filter(|d| d.user_id == member.user_id)
            .cloned()
            .collect();
        let mut dev_list = div().flex().flex_col().gap_2();
        if member_devices.is_empty() {
            dev_list = dev_list.child(
                div()
                    .text_xs()
                    .text_color(text_secondary())
                    .child("No enrolled devices registered for this collaborator in this vault."),
            );
        } else {
            for dev in member_devices {
                let status_str = if dev.revoked {
                    "Revoked"
                } else if dev.verified {
                    "Verified"
                } else {
                    "Pending"
                };
                let dev_id_replace = dev.device_id.clone();
                let dev_id_revoke = dev.device_id.clone();
                let dev_name = dev.device_name.clone();
                let dev_name_revoke = dev.device_name.clone();
                let mem_for_replace = mem_id.clone();
                let mem_for_revoke = mem_id.clone();
                let mut row = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .p_2()
                    .rounded_md()
                    .bg(surface())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(text_primary())
                                    .child(format!("{} ({status_str})", dev.device_name)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text_secondary())
                                    .child(format!("ID: {}", dev.device_id)),
                            ),
                    );
                if !dev.revoked {
                    row = row.child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                AppButton::new(
                                    gpui::SharedString::from(format!(
                                        "replace-dev-{}",
                                        dev.device_id
                                    )),
                                    "Replace Keys",
                                    ButtonVariant::Secondary,
                                )
                                .on_click(cx.listener(
                                    move |app, _, _, cx| {
                                        app.pending_device_replace = Some((
                                            mem_for_replace.clone(),
                                            dev_id_replace.clone(),
                                            dev_name.clone(),
                                        ));
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                AppButton::new(
                                    gpui::SharedString::from(format!(
                                        "revoke-dev-{}",
                                        dev.device_id
                                    )),
                                    "Revoke",
                                    ButtonVariant::Danger,
                                )
                                .on_click(cx.listener(
                                    move |app, _, _, cx| {
                                        app.pending_device_revoke = Some((
                                            mem_for_revoke.clone(),
                                            dev_id_revoke.clone(),
                                            dev_name_revoke.clone(),
                                        ));
                                        cx.notify();
                                    },
                                )),
                            ),
                    );
                }
                dev_list = dev_list.child(row);
            }
        }
        body = body.child(simple_section("Enrolled Devices", dev_list));

        if let Some((confirm_mem_id, confirm_dev_id, confirm_dev_name)) =
            self.pending_device_revoke.clone()
        {
            body = body.child(simple_section(
                "Confirm Device Revocation",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(danger())
                            .child(format!(
                                "Revoke device '{confirm_dev_name}' ({confirm_dev_id})? This globally revokes the device on Shellumi Server and removes its access across vaults."
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                AppButton::new(
                                    "confirm-device-revoke-btn",
                                    "Confirm Revoke",
                                    ButtonVariant::Danger,
                                )
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.revoke_member_device(
                                        confirm_mem_id.clone(),
                                        confirm_dev_id.clone(),
                                        cx,
                                    );
                                })),
                            )
                            .child(
                                AppButton::new(
                                    "cancel-device-revoke-btn",
                                    "Cancel",
                                    ButtonVariant::Secondary,
                                )
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.pending_device_revoke = None;
                                    cx.notify();
                                })),
                            ),
                    ),
            ));
        }

        if let Some((confirm_mem_id, confirm_dev_id, confirm_dev_name)) =
            self.pending_device_replace.clone()
        {
            body = body.child(simple_section(
                "Confirm Key Replacement",
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_secondary())
                            .child(format!(
                                "Replace keys for '{confirm_dev_name}' ({confirm_dev_id})? A new device identity will be enrolled and verified before the old device is revoked globally across vaults."
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                AppButton::new(
                                    "confirm-device-replace-btn",
                                    "Confirm Replace Keys",
                                    ButtonVariant::Primary,
                                )
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.replace_member_device_keys(
                                        confirm_mem_id.clone(),
                                        confirm_dev_id.clone(),
                                        confirm_dev_name.clone(),
                                        cx,
                                    );
                                })),
                            )
                            .child(
                                AppButton::new(
                                    "cancel-device-replace-btn",
                                    "Cancel",
                                    ButtonVariant::Secondary,
                                )
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.pending_device_replace = None;
                                    cx.notify();
                                })),
                            ),
                    ),
            ));
        }

        if let Some(invite) = self
            .last_created_invite
            .as_ref()
            .filter(|invite| invite.member_id == member.id && self.invite_is_in_scope(invite))
        {
            let invite_id = &invite.invitation_id;
            let expires_at = invite.expires_at;
            let mut inv_box = div().flex().flex_col().gap_2().child(
                div()
                    .text_xs()
                    .text_color(text_primary())
                    .child(format!("Invite ID: {invite_id} (expires at {expires_at})")),
            );
            let mut btn_row = div().flex().gap_2();
            if invite.token.is_some() {
                let invitation_id = invite.invitation_id.clone();
                btn_row = btn_row.child(
                    AppButton::new(
                        "copy-invite-token-btn",
                        "Copy Invite Token",
                        ButtonVariant::Primary,
                    )
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.copy_current_invitation(&invitation_id, cx);
                    })),
                );
            }
            btn_row = btn_row.child(
                AppButton::new(
                    "dismiss-invite-meta-btn",
                    "Dismiss",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.last_created_invite = None;
                    cx.notify();
                })),
            );
            inv_box = inv_box.child(btn_row);
            body = body.child(simple_section("Server Invitation Created", inv_box));
        }

        InspectorShell::new(header, body, None::<Div>, self.active_inspector_width())
    }

    fn render_enrollment_recovery(&self, cx: &mut Context<Self>) -> Option<Div> {
        if self.incomplete_enrollments.is_empty() && !self.has_pending_device_replacements {
            return None;
        }
        let busy = self.team_enrollment_busy;
        Some(simple_section(
            "Device Recovery",
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(self.incomplete_enrollments.iter().filter_map(|operation| {
                    operation.error_message.as_ref().map(|message| {
                        div()
                            .text_xs()
                            .text_color(text_secondary())
                            .child(message.clone())
                    })
                }))
                .when(self.has_pending_device_replacements, |panel| {
                    panel.child(
                        div()
                            .text_xs()
                            .text_color(text_secondary())
                            .child("A device replacement still needs reconciliation."),
                    )
                })
                .child(
                    AppButton::new(
                        "team-resume-enrollment-btn",
                        if busy {
                            "Resuming..."
                        } else {
                            "Resume Enrollment"
                        },
                        ButtonVariant::Primary,
                    )
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.resume_incomplete_enrollments(cx);
                    })),
                ),
        ))
    }

    /// Render team member inspector in Edit or Create mode.
    fn render_team_edit_inspector(
        &mut self,
        _member_id: Option<String>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = if is_create {
            "Invite Team Member"
        } else {
            "Edit Member Role"
        };

        let close_btn = InspectorShell::close_button(
            "close-team-edit-inspector-btn",
            true,
            |app, _, _, cx| {
                app.inspector_back(cx);
            },
            cx,
        );
        let header = InspectorShell::header(
            title,
            Some("Grant encrypted vault access and define permissions"),
            None,
            None,
            Some(close_btn),
        );

        let Some(form) = &self.team_form else {
            return InspectorShell::new(
                header,
                div().p_4().child("Form not initialized"),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let mut body = div().flex().flex_col().gap_4().p_1();

        if let Some(err) = &form.error {
            body = body.child(error_notice(err));
        }

        // Email / Name
        let email_input = form.email.clone();
        body = body.child(simple_field(
            "Email or Display Name *",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(email_input),
        ));

        // Role selector
        let current_role = form.role.clone();
        let mut role_selector = div().flex().gap_1p5().py_1();
        for (idx, (role_id, label)) in [
            ("editor", "Editor"),
            ("admin", "Administrator"),
            ("viewer", "Viewer"),
        ]
        .iter()
        .enumerate()
        {
            let is_sel = current_role == *role_id;
            let r_val = role_id.to_string();
            role_selector = role_selector.child(
                div()
                    .id(("team-role", idx))
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .font_weight(if is_sel {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if is_sel {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        surface()
                    })
                    .text_color(if is_sel {
                        accent_blue()
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if let Some(ref mut f) = app.team_form {
                            f.role = r_val.clone();
                            app.inspector_mark_dirty(cx);
                        }
                    }))
                    .child(*label),
            );
        }
        body = body.child(simple_field("Vault Role", role_selector));

        // Public Key (for end-to-end encrypted vault share)
        let pubkey_input = form.public_key.clone();
        body = body.child(simple_field(
            "Collaborator Public Key (Optional)",
            div()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_md()
                .bg(surface())
                .p_2()
                .child(pubkey_input),
        ));

        let is_sending = form.sending;
        let footer = div()
            .p_4()
            .border_t_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(surface())
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(
                AppButton::new("team-inspector-cancel-btn", "Cancel", ButtonVariant::Ghost)
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.inspector_back(cx);
                    })),
            )
            .child(
                AppButton::new(
                    "team-inspector-save-btn",
                    if is_sending {
                        "Saving…"
                    } else if is_create {
                        "Send Invitation"
                    } else {
                        "Update Role"
                    },
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(f) = &app.team_form {
                        if !f.sending {
                            app.save_team_inspector(cx);
                        }
                    }
                })),
            );

        InspectorShell::new(header, body, Some(footer), self.active_inspector_width())
    }

    /// Open team member inspector in a specific mode.
    pub(crate) fn open_team_inspector_mode(
        &mut self,
        member_id: Option<String>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::TeamMember(member_id.clone());
        if self
            .inspector_coordinator
            .open(vault_id, target, mode, None)
        {
            if mode == InspectorMode::Edit || mode == InspectorMode::Create {
                let existing = member_id.as_deref().and_then(|id| {
                    self.team_members
                        .iter()
                        .find(|m| m.id == id || m.user_id == id)
                });

                let (initial_email, initial_role, initial_pk) = match existing {
                    Some(m) => {
                        let r_str = match m.role {
                            TeamRole::Administrator => "admin",
                            TeamRole::Viewer => "viewer",
                            _ => "editor",
                        };
                        (m.user_id.clone(), r_str.to_string(), m.public_key.clone())
                    }
                    None => (String::new(), "editor".to_string(), String::new()),
                };

                let email = cx.new(|cx| TextInput::new(cx, &initial_email, "user@example.com"));
                let public_key =
                    cx.new(|cx| TextInput::new(cx, &initial_pk, "ssh-ed25519 ... (optional)"));

                cx.observe(&email, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();
                cx.observe(&public_key, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();

                self.team_form = Some(TeamFormState {
                    member_id: existing.map(|m| m.id.clone()).or(member_id),
                    email,
                    role: initial_role,
                    public_key,
                    error: None,
                    sending: false,
                });
            } else {
                self.team_form = None;
            }
        }
        cx.notify();
    }

    /// Save / invite team member.
    pub(crate) fn save_team_inspector(&mut self, cx: &mut Context<Self>) {
        let Some(gen) = self.inspector_coordinator.start_save() else {
            return;
        };

        let Some(form) = &mut self.team_form else {
            return;
        };

        let email = form.email.read(cx).value().trim().to_string();
        let role_str = form.role.clone();
        let pubkey = form.public_key.read(cx).value().trim().to_string();
        let existing_member_id = form.member_id.clone();

        if email.is_empty() {
            form.error = Some("Email or display name is required.".into());
            let active_vid = self.active_vault_id.clone();
            self.inspector_coordinator.fail_save(
                &active_vid,
                gen,
                "Email or display name is required.".into(),
            );
            cx.notify();
            return;
        }

        form.sending = true;
        form.error = None;
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
        let vault_key = self.active_vault_key();
        let reg_path = self.registry_db_path();
        let default_key = self.vault_key_for(shellumi_storage::LEGACY_LOCAL_VAULT_ID);
        let email_for_msg = email.clone();
        self.invalidate_invitation_scope();
        self.pending_invitation_inspector = Some(gen);
        let request = InvitationRequest::capture(&self.invitation_generation);
        let background_request = request.clone();
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id_task) {
                    store.set_vault_id(vid);
                }
                let role = match role_str.as_str() {
                    "admin" => TeamRole::Administrator,
                    "viewer" => TeamRole::Viewer,
                    _ => TeamRole::Editor,
                };
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string();

                if let Some(ref mid) = existing_member_id {
                    store
                        .update_team_member_role(mid, role)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok::<(String, bool, Option<crate::CreatedInvite>), String>((
                        mid.clone(),
                        false,
                        None,
                    ))
                } else {
                    // For remote vaults, require verified binding and server invitation creation;
                    // never silently fall back to a local-only record when a remote vault fails.
                    let mut created_on_server = false;
                    let target_id = format!("tm-{}", uuid::Uuid::new_v4().as_simple());
                    let mut invite_meta: Option<crate::CreatedInvite> = None;
                    if let Some(ref rp) = reg_path {
                        let reg_store = HostStore::open_with_key(rp, default_key.as_ref())
                            .await
                            .map_err(|e| e.to_string())?;
                        let ctx = reg_store
                            .get_vault_context(&active_vault_id_task)
                            .await
                            .map_err(|e| e.to_string())?;
                        if let Some(ref c) = ctx {
                            if matches!(c.location(), shellumi_core::VaultLocation::Remote { .. }) {
                                let binding = reg_store
                                    .resolve_verified_remote_vault_binding(
                                        &active_vault_id_task,
                                        None,
                                    )
                                    .await
                                    .map_err(|e| {
                                        format!("Remote vault binding verification failed: {e}")
                                    })?;
                                let conn = reg_store
                                    .get_vault_server_connection(binding.server_connection_id())
                                    .await
                                    .map_err(|e| e.to_string())?
                                    .ok_or_else(|| {
                                        format!(
                                            "Server connection '{}' not found",
                                            binding.server_connection_id()
                                        )
                                    })?;
                                let cred_store = shellumi_core::default_credential_store();
                                let token_sec =
                                    conn.resolve_auth_token(&*cred_store).ok_or_else(|| {
                                        "Server connection missing auth token; cannot create server invitation".to_string()
                                    })?;
                                let mut client = shellumi_core::server::ShellumiServerClient::new(
                                    &conn.base_url,
                                );
                                client.set_token(token_sec.expose_secret().to_string());
                                let account = binding.authenticate_vault_member(&store, &client)
                                    .map_err(|_| "Server invitation scope could not be verified".to_string())?;
                                if !background_request.session_is_current(&reg_store, &binding, &token_sec, &*cred_store)
                                    .await.map_err(|_| "Server invitation session could not be verified".to_string())?
                                {
                                    tracing::debug!("Invitation request invalidated before dispatch");
                                    return Err("Invitation request was canceled".into());
                                }
                                let invite_resp = client
                                    .create_invite(
                                        binding.remote_vault_id(),
                                        &shellumi_core::server::types::CreateInviteRequest {
                                            role: role.as_str().to_string(),
                                            recipient_email: Some(email.clone()),
                                            ttl_secs: Some(7 * 24 * 3600),
                                        },
                                    )
                                    .map_err(|_| "Server invitation creation failed".to_string())?;
                                if invite_resp.vault_id != binding.remote_vault_id() || invite_resp.invite_id.is_nil() {
                                    tracing::warn!("Discarded inconsistent server invitation identity");
                                    return Err("Server returned an invitation for an unexpected vault".into());
                                }
                                if binding.authenticate_vault_member(&store, &client).ok() != Some(account)
                                    || !background_request.session_is_current(&reg_store, &binding, &token_sec, &*cred_store)
                                        .await.map_err(|_| "Server invitation session could not be verified".to_string())?
                                {
                                    tracing::debug!("Discarded invitation for invalidated server session");
                                    return Err("Server invitation session changed".into());
                                }
                                created_on_server = true;
                                invite_meta = Some(crate::CreatedInvite {
                                    scope: crate::InviteScope { binding, account },
                                    issuing_session: token_sec,
                                    member_id: target_id.clone(),
                                    invitation_id: invite_resp.invite_id.to_string(),
                                    token: invite_resp.invite_token.map(shellumi_core::SecretString::new),
                                    expires_at: invite_resp.expires_at,
                                    inspector_generation: gen,
                                });
                            }
                        }
                    }

                    let member = TeamMember {
                        id: target_id.clone(),
                        vault_id: active_vault_id_task,
                        user_id: email.clone(),
                        display_name: email,
                        public_key: pubkey,
                        agreement_public_key: None,
                        enrollment_status: if created_on_server {
                            shellumi_core::EnrollmentStatus::InvitationPending
                        } else {
                            shellumi_core::EnrollmentStatus::LegacyRequiresMigration
                        },
                        role,
                        enrolled_at: now,
                        invited_by: "current_user".into(),
                        revoked: false,
                        revoked_at: None,
                    };
                    store
                        .add_team_member(&member)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok::<(String, bool, Option<crate::CreatedInvite>), String>((
                        target_id,
                        created_on_server,
                        invite_meta,
                    ))
                }
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if !request.is_current()
                    || app.active_vault_id != active_vault_id
                    || !request.can_deliver(&app.inspector_coordinator, &active_vault_id, gen)
                {
                    tracing::debug!("Discarded stale invitation completion");
                    return;
                }
                app.pending_invitation_inspector = None;
                if let Some(ref mut f) = app.team_form {
                    f.sending = false;
                }
                match res {
                    Ok((target_id, created_on_server, invite_meta)) => {
                        let target = InspectorTarget::TeamMember(Some(target_id.clone()));
                        let ok =
                            app.inspector_coordinator
                                .complete_save(&active_vault_id, gen, target);
                        if !ok {
                            tracing::warn!(
                                originating_vault = %active_vault_id,
                                current_vault = %app.active_vault_id,
                                gen = gen,
                                "Discarded stale save completion for team member"
                            );
                            return;
                        }

                        app.team_form = None;
                        app.load_team_members(cx);
                        app.desktop.message = Some(if created_on_server {
                            format!("Server invitation created for '{}'", email_for_msg)
                        } else {
                            format!(
                                "Local collaborator record saved for '{}' (not bound to a remote server)",
                                email_for_msg
                            )
                        });

                        if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                            app.apply_navigation_request(req, None, cx);
                        } else {
                            app.open_team_inspector_mode(
                                Some(target_id.clone()),
                                crate::ui::inspector::InspectorMode::View,
                                cx,
                            );
                            if let Some(mut invite) = invite_meta {
                                let expires_at = invite.expires_at;
                                invite.inspector_generation = app.inspector_coordinator.active()
                                    .map_or(gen, |state| state.generation);
                                app.invite_scope = Some(invite.scope.clone());
                                if app.invite_is_in_scope(&invite) {
                                    app.last_created_invite = Some(invite);
                                    let lifetime = expires_at.saturating_sub(invite_timestamp()).max(0) as u64;
                                    let timer = cx.background_executor().timer(std::time::Duration::from_secs(lifetime));
                                    cx.spawn(async move |app, cx| {
                                        timer.await;
                                        let _ = app.update(cx, |app, cx| {
                                            app.clear_out_of_scope_invite();
                                            cx.notify();
                                        });
                                    }).detach();
                                }
                            }
                        }
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.team_form {
                            f.error = Some(err.clone());
                        }
                        app.inspector_coordinator
                            .fail_save(&active_vault_id, gen, err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn delete_team_member_inspector(&mut self, _id: &str, cx: &mut Context<Self>) {
        self.report_team_rotation_unavailable(cx);
    }
}

fn invite_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

impl crate::CreatedInvite {
    fn copy_token(
        &self,
        invitation_id: &str,
        active_scope: Option<&crate::InviteScope>,
        active_vault: &str,
        inspector: Option<&super::coordinator::InspectorState>,
        now: i64,
    ) -> Option<&str> {
        if self.invitation_id != invitation_id
            || !self.is_visible(active_scope, active_vault, inspector, now)
        {
            return None;
        }
        self.token
            .as_ref()
            .map(shellumi_core::SecretString::expose_secret)
    }

    fn is_visible(
        &self,
        active_scope: Option<&crate::InviteScope>,
        active_vault: &str,
        inspector: Option<&super::coordinator::InspectorState>,
        now: i64,
    ) -> bool {
        active_scope == Some(&self.scope)
            && self.scope.binding.local_vault_id() == active_vault
            && self.expires_at > now
            && inspector.is_some_and(|state| {
                state.vault_id == active_vault
                    && state.generation == self.inspector_generation
                    && state.mode == InspectorMode::View
                    && matches!(&state.target, InspectorTarget::TeamMember(Some(id)) if id == &self.member_id)
            })
    }
}

#[cfg(test)]
mod invite_state_tests {
    use super::super::coordinator::InspectorCoordinator;
    use super::*;

    #[test]
    fn delayed_invitation_results_are_invalidated_across_context_changes() {
        let epoch = Arc::new(AtomicU64::new(0));
        let original = InvitationRequest::capture(&epoch);
        let worker = original.clone();
        let (ready, wait_for_switch) = std::sync::mpsc::channel();
        let delayed = std::thread::spawn(move || {
            wait_for_switch.recv().unwrap();
            worker.is_current()
        });

        // Switch away and back, then begin a new invitation in the original vault.
        epoch.fetch_add(1, Ordering::AcqRel);
        epoch.fetch_add(1, Ordering::AcqRel);
        let new_request = InvitationRequest::capture(&epoch);
        ready.send(()).unwrap();
        assert!(!delayed.join().unwrap());
        assert!(!original.is_current());
        assert!(new_request.is_current());

        // An account or server change invalidates outstanding responses in the same vault.
        epoch.fetch_add(1, Ordering::AcqRel);
        assert!(!new_request.is_current());
        let after_account_change = InvitationRequest::capture(&epoch);
        // Locking the active vault invalidates requests even if the vault ID stays the same.
        epoch.fetch_add(1, Ordering::AcqRel);
        assert!(!after_account_change.is_current());
    }

    #[tokio::test]
    async fn invitation_delivery_checks_live_registry_credentials_and_inspector_transitions() {
        use shellumi_core::{CredentialStore, EphemeralCredentialStore, SecretString};
        let mut registry = HostStore::open(":memory:").await.unwrap();
        let connection_id = "invitation-test";
        let credential_store = EphemeralCredentialStore::new();
        let service = shellumi_storage::VaultServerConnectionRecord::CREDENTIAL_SERVICE;
        let account = "invite-auth";
        credential_store
            .set_secret(service, account, "original")
            .unwrap();
        let mut connection = shellumi_storage::VaultServerConnectionRecord {
            id: connection_id.into(),
            base_url: "https://example.test".into(),
            server_instance_id: Some(uuid::Uuid::new_v4().to_string()),
            auth_token: Some(format!("keychain://{service}/{account}")),
            ..Default::default()
        };
        registry
            .save_vault_server_connection(&connection)
            .await
            .unwrap();
        for vault in ["one", "two"] {
            registry
                .register_vault_context(
                    vault,
                    &shellumi_core::VaultContext::new(
                        shellumi_core::VaultId::new(vault).unwrap(),
                        shellumi_core::VaultLocation::Remote {
                            server: shellumi_core::ServerConnectionId::new(connection_id).unwrap(),
                            server_vault_id: shellumi_core::VaultId::new(
                                uuid::Uuid::new_v4().to_string(),
                            )
                            .unwrap(),
                        },
                    ),
                )
                .await
                .unwrap();
        }
        let binding = registry
            .resolve_verified_remote_vault_binding("one", None)
            .await
            .unwrap();
        let token = SecretString::new("original".into());
        let epoch = Arc::new(AtomicU64::new(0));
        let original = InvitationRequest::capture(&epoch);
        let mut inspector = InspectorCoordinator::new();
        let target = InspectorTarget::TeamMember(None);
        assert!(inspector.open("one", target.clone(), InspectorMode::Create, None));
        let generation = inspector.active().unwrap().generation;
        assert!(original.can_deliver(&inspector, "one", generation));
        assert!(original
            .session_is_current(&registry, &binding, &token, &credential_store)
            .await
            .unwrap());

        // A concurrent request replaces the earlier request within the same Inspector.
        epoch.fetch_add(1, Ordering::AcqRel);
        let newer = InvitationRequest::capture(&epoch);
        assert!(!original.can_deliver(&inspector, "one", generation));
        assert!(newer.can_deliver(&inspector, "one", generation));

        // Actual coordinator transitions invalidate results even after selecting the originating vault again.
        inspector.invalidate_scope();
        assert!(inspector.open("two", target.clone(), InspectorMode::Create, None));
        assert!(!newer.can_deliver(&inspector, "one", generation));
        inspector.invalidate_scope();
        assert!(inspector.open("one", target, InspectorMode::Create, None));
        assert!(!newer.can_deliver(&inspector, "one", generation));
        let active = inspector.active().unwrap().generation;
        assert!(inspector.request_close());
        assert!(!newer.can_deliver(&inspector, "one", active));

        credential_store
            .set_secret(service, account, "different")
            .unwrap();
        assert!(!newer
            .session_is_current(&registry, &binding, &token, &credential_store)
            .await
            .unwrap());
        credential_store
            .set_secret(service, account, "original")
            .unwrap();
        registry.set_vault_locked("one", true).await.unwrap();
        assert!(!newer
            .session_is_current(&registry, &binding, &token, &credential_store)
            .await
            .unwrap());
        registry.set_vault_locked("one", false).await.unwrap();
        connection.base_url = "https://changed.example.test".into();
        registry
            .save_vault_server_connection(&connection)
            .await
            .unwrap();
        assert!(!newer
            .session_is_current(&registry, &binding, &token, &credential_store)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn invite_visibility_follows_vault_target_generation_and_expiration() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = HostStore::open(directory.path().join("registry.db").to_str().unwrap())
            .await
            .unwrap();
        let server = "connection-a";
        store
            .save_vault_server_connection(&shellumi_storage::VaultServerConnectionRecord {
                id: server.into(),
                base_url: "https://example.test".into(),
                server_instance_id: Some(uuid::Uuid::new_v4().to_string()),
                ..Default::default()
            })
            .await
            .unwrap();
        store
            .register_vault_context(
                "Vault A",
                &shellumi_core::VaultContext::new(
                    shellumi_core::VaultId::new("vault-a").unwrap(),
                    shellumi_core::VaultLocation::Remote {
                        server: shellumi_core::ServerConnectionId::new(server).unwrap(),
                        server_vault_id: shellumi_core::VaultId::new(
                            uuid::Uuid::new_v4().to_string(),
                        )
                        .unwrap(),
                    },
                ),
            )
            .await
            .unwrap();
        let binding = store
            .resolve_verified_remote_vault_binding("vault-a", None)
            .await
            .unwrap();
        let mut inspector = InspectorCoordinator::new();
        let target = InspectorTarget::TeamMember(Some("alice".into()));
        assert!(inspector.open("vault-a", target.clone(), InspectorMode::View, None));
        let invite = crate::CreatedInvite {
            scope: crate::InviteScope {
                binding,
                account: uuid::Uuid::new_v4(),
            },
            issuing_session: shellumi_core::SecretString::new("test-session".into()),
            member_id: "alice".into(),
            invitation_id: "invite-a".into(),
            token: Some(shellumi_core::SecretString::new("test-token".into())),
            expires_at: 200,
            inspector_generation: inspector.active().unwrap().generation,
        };
        assert_eq!(
            invite.copy_token(
                "invite-a",
                Some(&invite.scope),
                "vault-a",
                inspector.active(),
                199
            ),
            Some("test-token")
        );
        assert_eq!(
            invite.copy_token(
                "stale-invite",
                Some(&invite.scope),
                "vault-a",
                inspector.active(),
                199
            ),
            None
        );
        assert_eq!(
            invite.copy_token(
                "invite-a",
                Some(&invite.scope),
                "vault-b",
                inspector.active(),
                199
            ),
            None
        );
        let mut changed_account = invite.scope.clone();
        changed_account.account = uuid::Uuid::new_v4();
        assert!(invite
            .copy_token(
                "invite-a",
                Some(&changed_account),
                "vault-a",
                inspector.active(),
                199
            )
            .is_none());
        assert!(invite
            .copy_token("invite-a", None, "vault-a", inspector.active(), 199)
            .is_none());
        assert!(invite.is_visible(Some(&invite.scope), "vault-a", inspector.active(), 199));
        assert!(!invite.is_visible(Some(&invite.scope), "vault-b", inspector.active(), 199));
        assert!(!invite.is_visible(Some(&invite.scope), "vault-a", inspector.active(), 200));
        assert!(inspector.open(
            "vault-a",
            InspectorTarget::TeamMember(Some("bob".into())),
            InspectorMode::View,
            None
        ));
        assert!(!invite.is_visible(Some(&invite.scope), "vault-a", inspector.active(), 199));
        let pending = inspector.start_save().unwrap();
        inspector.invalidate_scope();
        assert!(!inspector.complete_save("vault-a", pending, target.clone()));
        assert!(!invite.is_visible(Some(&invite.scope), "vault-a", inspector.active(), 199));
        assert!(inspector.open("vault-a", target, InspectorMode::View, None));
        assert!(!invite.is_visible(Some(&invite.scope), "vault-a", inspector.active(), 199));
    }
}

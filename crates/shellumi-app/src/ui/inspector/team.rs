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

impl Shellumi {
    /// Render the unified Team Member Inspector for viewing or inviting collaborators.
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
                    .child("No team collaborator selected or enrolled yet."),
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
            .child(self.render_detail_row(
                "Access Status",
                if is_revoked {
                    "Revoked".to_string()
                } else {
                    "Active".to_string()
                },
            ))
            .child(self.render_detail_row("Enrolled At", enrolled))
            .child(self.render_detail_row("Public Key Fingerprint", pubkey_fp));
        body = body.child(simple_section("Collaborator Details", details_items));

        InspectorShell::new(header, body, None::<Div>, self.active_inspector_width())
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
        let vault_key = self.active_vault_key();
        let email_for_msg = email.clone();
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id) {
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
                    Ok::<String, String>(mid.clone())
                } else {
                    let target_id = format!("tm-{}", uuid::Uuid::new_v4().as_simple());
                    let member = TeamMember {
                        id: target_id.clone(),
                        vault_id: active_vault_id,
                        user_id: email.clone(),
                        display_name: email,
                        public_key: pubkey,
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
                    Ok::<String, String>(target_id)
                }
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(ref mut f) = app.team_form {
                    f.sending = false;
                }
                match res {
                    Ok(target_id) => {
                        app.team_form = None;
                        let target = InspectorTarget::TeamMember(Some(target_id.clone()));
                        let active_vid = app.active_vault_id.clone();
                        let ok = app
                            .inspector_coordinator
                            .complete_save(&active_vid, gen, target);
                        app.load_team_members(cx);
                        app.open_team_inspector_mode(
                            Some(target_id),
                            crate::ui::inspector::InspectorMode::View,
                            cx,
                        );
                        app.desktop.message =
                            Some(format!("Team permissions updated for '{}'", email_for_msg));
                        if ok {
                            if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                                app.apply_navigation_request(req, None, cx);
                            }
                        }
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.team_form {
                            f.error = Some(err.clone());
                        }
                        let active_vid = app.active_vault_id.clone();
                        app.inspector_coordinator.fail_save(&active_vid, gen, err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Revoke/delete team member.
    pub(crate) fn delete_team_member_inspector(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let key = self.active_vault_key();
        let id_str = id.to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();
        let id_str_task = id_str.clone();
        let now_task = now.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .revoke_team_member(&id_str_task, &now_task)
                    .await
                    .map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(_) => {
                        if let Some(m) = app.team_members.iter_mut().find(|m| m.id == id_str) {
                            m.revoked = true;
                            m.revoked_at = Some(now);
                        }
                        app.inspector_close(cx);
                        app.load_team_members(cx);
                        app.desktop.message =
                            Some("Team member access revoked successfully".into());
                    }
                    Err(e) => {
                        if let Some(ref mut f) = app.team_form {
                            f.error = Some(e);
                        } else {
                            app.desktop.message = Some(format!("Failed to revoke access: {e}"));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

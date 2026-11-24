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
        let m_name = member_id.clone().unwrap_or_else(|| "Collaborator".into());
        let m_role = "Member".to_string();

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x183322))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x3dd68c))
            .child(m_role.clone());

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
            .on_click(cx.listener({
                let name_clone = m_name.clone();
                move |app, _, _, cx| {
                    app.open_team_inspector_mode(Some(name_clone.clone()), InspectorMode::Edit, cx);
                }
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
        let delete_name = m_name.clone();
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "team-view-edit-btn",
                    "Change Role",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener({
                    let name_clone = m_name.clone();
                    move |app, _, _, cx| {
                        app.open_team_inspector_mode(
                            Some(name_clone.clone()),
                            InspectorMode::Edit,
                            cx,
                        );
                    }
                })),
            )
            .child(
                AppButton::new(
                    "team-view-revoke-btn",
                    "Revoke Access",
                    ButtonVariant::Danger,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.delete_confirm = Some(crate::DeleteTarget::TeamMember(
                        delete_name.clone(),
                        delete_name.clone(),
                    ));
                    cx.notify();
                })),
            );
        body = body.child(actions_row);

        // Details
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Display Name / Email", m_name))
            .child(self.render_detail_row("Assigned Role", m_role));
        body = body.child(simple_section("Collaborator Details", details_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
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
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
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
                        "Sending…"
                    } else {
                        "Send Invitation"
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

        InspectorShell::new(
            header,
            body,
            Some(footer),
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
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
                let email_val = member_id.as_deref().unwrap_or("");
                let email = cx.new(|cx| TextInput::new(cx, email_val, "user@example.com"));
                let public_key = cx.new(|cx| TextInput::new(cx, "", "ssh-ed25519 ... (optional)"));

                cx.observe(&email, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();
                cx.observe(&public_key, |app, _, cx| app.inspector_mark_dirty(cx))
                    .detach();

                self.team_form = Some(TeamFormState {
                    member_id,
                    email,
                    role: "editor".into(),
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
        let Some(form) = &mut self.team_form else {
            return;
        };

        let email = form.email.read(cx).value().trim().to_string();
        let role_str = form.role.clone();
        let pubkey = form.public_key.read(cx).value().trim().to_string();

        if email.is_empty() {
            form.error = Some("Email or display name is required.".into());
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
                let member = TeamMember {
                    id: format!("tm-{}", uuid::Uuid::new_v4().as_simple()),
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
                    .map_err(|e| e.to_string())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(ref mut f) = app.team_form {
                    f.sending = false;
                }
                match res {
                    Ok(_) => {
                        app.team_form = None;
                        app.inspector_coordinator.set_dirty(false);
                        app.open_team_inspector_mode(
                            Some(email_for_msg.clone()),
                            crate::ui::inspector::InspectorMode::View,
                            cx,
                        );
                        app.desktop.message =
                            Some(format!("Invitation sent to '{}'", email_for_msg));
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.team_form {
                            f.error = Some(err);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Revoke/delete team member.
    pub(crate) fn delete_team_member_inspector(&mut self, _id: &str, cx: &mut Context<Self>) {
        self.inspector_close(cx);
        self.desktop.message = Some("Team member access revoked".into());
        cx.notify();
    }
}

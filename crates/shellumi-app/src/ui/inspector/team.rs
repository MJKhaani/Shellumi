#![allow(dead_code)]
//! Team member and invitation inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout;
use crate::Shellumi;
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};

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
        let m_name = member_id.unwrap_or_else(|| "Collaborator".into());
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

        let header = InspectorShell::header(
            m_name.clone(),
            Some("Vault Collaborator"),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            "close-team-inspector-btn",
            false,
        );

        let mut body = div().flex().flex_col().gap_4();

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

        let header = InspectorShell::header(
            title,
            Some("Grant encrypted vault access and define permissions"),
            None,
            None,
            "close-team-edit-inspector-btn",
            false,
        );

        let body = div().flex().flex_col().gap_3().p_2().child(
            div()
                .text_xs()
                .text_color(text_secondary())
                .child("Team invitation fields (collaborator email/public key, role selection)."),
        );

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
                        app.inspector_close(cx);
                    })),
            )
            .child(
                AppButton::new(
                    "team-inspector-save-btn",
                    "Send Invitation",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.inspector_close(cx);
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
        let target = InspectorTarget::TeamMember(member_id);
        let _ = self
            .inspector_coordinator
            .open(vault_id, target, mode, None);
        cx.notify();
    }
}

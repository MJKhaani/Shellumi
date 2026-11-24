#![allow(dead_code)]
//! Workspace inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout;
use crate::{SavedWorkspace, Shellumi};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};

impl Shellumi {
    /// Render the unified Workspace Inspector for viewing or editing multi-session workspaces.
    pub(crate) fn render_workspace_inspector(
        &mut self,
        workspace_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_workspace_view_inspector(workspace_id, cx),
            InspectorMode::Edit | InspectorMode::Create => self.render_workspace_edit_inspector(
                workspace_id,
                mode == InspectorMode::Create,
                cx,
            ),
        }
    }

    /// Render workspace inspector in View mode.
    fn render_workspace_view_inspector(
        &mut self,
        workspace_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let ws = workspace_id.and_then(|id| self.workspaces.iter().find(|w| w.id == id));

        let Some(ws) = ws else {
            return InspectorShell::new(
                div().p_4().child("Workspace not found"),
                div()
                    .p_4()
                    .child("The selected workspace layout does not exist or was deleted."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let id_val = ws.id;
        let ws_name = ws.name.clone();
        let session_count = ws.sessions.len();
        let layout_str = ws.layout.clone();
        let desc_str = ws.description.clone().unwrap_or_else(|| "—".into());

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x2d1f3d))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0xbb86fc))
            .child("Workspace");

        let edit_btn = div()
            .id("inspector-edit-workspace-btn")
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
                app.open_workspace_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let header = InspectorShell::header(
            ws_name.clone(),
            Some(format!(
                "{} sessions (Layout: {})",
                session_count, layout_str
            )),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            "close-workspace-inspector-btn",
            false,
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "workspace-view-launch-btn",
                    "Launch Workspace",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(move |_app, _, _, _cx| {
                    // Launch workspace
                })),
            )
            .child(
                AppButton::new(
                    "workspace-view-edit-btn",
                    "Edit Layout",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.open_workspace_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
                })),
            );
        body = body.child(actions_row);

        // Details
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Workspace Name", ws_name))
            .child(self.render_detail_row("Layout Type", layout_str))
            .child(self.render_detail_row("Description", desc_str));
        body = body.child(simple_section("Workspace Properties", details_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render workspace inspector in Edit or Create mode.
    fn render_workspace_edit_inspector(
        &mut self,
        _workspace_id: Option<i64>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = if is_create {
            "New Workspace"
        } else {
            "Edit Workspace"
        };

        let header = InspectorShell::header(
            title,
            Some("Configure multi-session grid and pane presets"),
            None,
            None,
            "close-workspace-edit-inspector-btn",
            false,
        );

        let body = div().flex().flex_col().gap_3().p_2().child(
            div()
                .text_xs()
                .text_color(text_secondary())
                .child("Workspace layout and session preset editor."),
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
                AppButton::new(
                    "workspace-inspector-cancel-btn",
                    "Cancel",
                    ButtonVariant::Ghost,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.inspector_close(cx);
                })),
            )
            .child(
                AppButton::new(
                    "workspace-inspector-save-btn",
                    "Save",
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

    /// Open workspace inspector in a specific mode.
    pub(crate) fn open_workspace_inspector_mode(
        &mut self,
        workspace_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Workspace(workspace_id);
        let _ = self
            .inspector_coordinator
            .open(vault_id, target, mode, None);
        cx.notify();
    }
}

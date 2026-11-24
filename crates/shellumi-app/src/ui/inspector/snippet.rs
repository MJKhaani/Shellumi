#![allow(dead_code)]
//! Snippet inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout;
use crate::{SavedSnippet, Shellumi};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};

impl Shellumi {
    /// Render the unified Snippet Inspector for viewing, running, or editing a snippet.
    pub(crate) fn render_snippet_inspector(
        &mut self,
        snippet_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_snippet_view_inspector(snippet_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_snippet_edit_inspector(snippet_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render snippet inspector in View mode.
    fn render_snippet_view_inspector(
        &mut self,
        snippet_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let snippet = snippet_id.and_then(|id| self.snippets.iter().find(|s| s.id == id));

        let Some(snippet) = snippet else {
            return InspectorShell::new(
                div().p_4().child("Snippet not found"),
                div()
                    .p_4()
                    .child("The selected snippet does not exist or was deleted."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let id_val = snippet.id;
        let s_name = snippet.name.clone();
        let s_cmd = snippet.command.clone();
        let s_desc = snippet.description.clone().unwrap_or_else(|| "—".into());
        let tags_str = if snippet.tags.is_empty() {
            "—".into()
        } else {
            snippet.tags.join(", ")
        };

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x183322))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x3dd68c))
            .child("Snippet");

        let edit_btn = div()
            .id("inspector-edit-snippet-btn")
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
                app.open_snippet_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let header = InspectorShell::header(
            s_name.clone(),
            Some(format!("{} variables", snippet.variables.len())),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            "close-snippet-inspector-btn",
            false,
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "snippet-view-run-btn",
                    "Run Snippet",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.insert_snippet(id_val, true, cx);
                })),
            )
            .child(
                AppButton::new("snippet-view-paste-btn", "Paste", ButtonVariant::Secondary)
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.insert_snippet(id_val, false, cx);
                    })),
            );
        body = body.child(actions_row);

        // Command script card
        let cmd_card = div()
            .p_3()
            .rounded_md()
            .bg(rgb(0x12141c))
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .text_xs()
            .font_family(crate::MONOSPACE_FONT)
            .text_color(rgb(0x389bf2))
            .child(s_cmd);
        body = body.child(simple_section("Command / Script", cmd_card));

        // Metadata section
        let mut meta_items = div().flex().flex_col().gap_1();
        meta_items = meta_items
            .child(self.render_detail_row("Description", s_desc))
            .child(self.render_detail_row("Tags", tags_str));
        body = body.child(simple_section("Properties", meta_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render snippet inspector in Edit or Create mode.
    fn render_snippet_edit_inspector(
        &mut self,
        _snippet_id: Option<i64>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = if is_create {
            "New Snippet"
        } else {
            "Edit Snippet"
        };

        let header = InspectorShell::header(
            title,
            Some("Configure reusable terminal script and parameters"),
            None,
            None,
            "close-snippet-edit-inspector-btn",
            false,
        );

        let body = div().flex().flex_col().gap_3().p_2().child(
            div()
                .text_xs()
                .text_color(text_secondary())
                .child("Snippet editor fields (title, script, tags)."),
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
                    "snippet-inspector-cancel-btn",
                    "Cancel",
                    ButtonVariant::Ghost,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.inspector_close(cx);
                })),
            )
            .child(
                AppButton::new("snippet-inspector-save-btn", "Save", ButtonVariant::Primary)
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

    /// Open snippet inspector in a specific mode.
    pub(crate) fn open_snippet_inspector_mode(
        &mut self,
        snippet_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Snippet(snippet_id);
        let _ = self
            .inspector_coordinator
            .open(vault_id, target, mode, None);
        cx.notify();
    }
}

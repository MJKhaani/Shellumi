#![allow(dead_code)]
//! Infrastructure provider inspector rendering and actions for View, Edit, and Create modes.

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
    /// Render the unified Cloud Provider Inspector for viewing or editing infrastructure sync.
    pub(crate) fn render_provider_inspector(
        &mut self,
        provider_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_provider_view_inspector(provider_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_provider_edit_inspector(provider_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render provider inspector in View mode.
    fn render_provider_view_inspector(
        &mut self,
        _provider_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let p_name = "Cloud Integration".to_string();
        let p_kind = "AWS / Azure / DO".to_string();

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x1e3a5f))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x389bf2))
            .child(p_kind.clone());

        let edit_btn = div()
            .id("inspector-edit-provider-btn")
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
            .on_click(cx.listener(|app, _, _, cx| {
                app.open_provider_inspector_mode(None, InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let header = InspectorShell::header(
            p_name.clone(),
            Some("Infrastructure Provider"),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            "close-provider-inspector-btn",
            false,
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "provider-view-sync-btn",
                    "Sync Inventory",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(|_app, _, _, _cx| {
                    // Trigger inventory sync
                })),
            )
            .child(
                AppButton::new(
                    "provider-view-edit-btn",
                    "Edit Credentials",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.open_provider_inspector_mode(None, InspectorMode::Edit, cx);
                })),
            );
        body = body.child(actions_row);

        // Details
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Integration Name", p_name))
            .child(self.render_detail_row("Provider Type", p_kind));
        body = body.child(simple_section("Provider Settings", details_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render provider inspector in Edit or Create mode.
    fn render_provider_edit_inspector(
        &mut self,
        _provider_id: Option<i64>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = if is_create {
            "New Cloud Provider"
        } else {
            "Edit Cloud Provider"
        };

        let header = InspectorShell::header(
            title,
            Some("Configure cloud credentials for automated host discovery"),
            None,
            None,
            "close-provider-edit-inspector-btn",
            false,
        );

        let body = div().flex().flex_col().gap_3().p_2().child(
            div()
                .text_xs()
                .text_color(text_secondary())
                .child("Provider configuration fields (AWS API key/secret, Azure tenant, or DigitalOcean token)."),
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
                    "provider-inspector-cancel-btn",
                    "Cancel",
                    ButtonVariant::Ghost,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.inspector_close(cx);
                })),
            )
            .child(
                AppButton::new(
                    "provider-inspector-save-btn",
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

    /// Open provider inspector in a specific mode.
    pub(crate) fn open_provider_inspector_mode(
        &mut self,
        provider_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Provider(provider_id);
        let _ = self
            .inspector_coordinator
            .open(vault_id, target, mode, None);
        cx.notify();
    }
}

#![allow(dead_code)]
//! Identity inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout;
use crate::{SavedIdentity, Shellumi};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};

impl Shellumi {
    /// Render the unified Identity Inspector for viewing or editing an SSH identity.
    pub(crate) fn render_identity_inspector(
        &mut self,
        identity_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_identity_view_inspector(identity_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_identity_edit_inspector(identity_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render identity inspector in View mode.
    fn render_identity_view_inspector(
        &mut self,
        identity_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let identity = identity_id.and_then(|id| self.identities.iter().find(|i| i.id == id));

        let Some(identity) = identity else {
            return InspectorShell::new(
                div().p_4().child("Identity not found"),
                div()
                    .p_4()
                    .child("The selected identity does not exist or was deleted."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let id_val = identity.id;
        let id_name = identity.name.clone();
        let username_str = identity.username.clone();
        let key_path_str = identity
            .key_path
            .clone()
            .unwrap_or_else(|| "— (None/Agent)".into());
        let tags_str = if identity.tags.is_empty() {
            "—".into()
        } else {
            identity.tags.join(", ")
        };

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x1e3a5f))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x389bf2))
            .child("Key");

        let edit_btn = div()
            .id("inspector-edit-identity-btn")
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
                app.open_identity_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let header = InspectorShell::header(
            id_name.clone(),
            Some(format!("User: {}", username_str)),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            "close-identity-inspector-btn",
            false,
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "identity-view-edit-btn",
                    "Edit Key",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.open_identity_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
                })),
            )
            .child(
                AppButton::new(
                    "identity-view-delete-btn",
                    "Delete Identity",
                    ButtonVariant::Danger,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.delete_identity(id_val, cx);
                    app.inspector_close(cx);
                })),
            );
        body = body.child(actions_row);

        // Details section
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Identity Name", id_name))
            .child(self.render_detail_row("Default Username", username_str))
            .child(self.render_detail_row("Private Key Path", key_path_str))
            .child(self.render_detail_row("Tags", tags_str));
        body = body.child(simple_section("Key Details", details_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render identity inspector in Edit or Create mode.
    fn render_identity_edit_inspector(
        &mut self,
        _identity_id: Option<i64>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(modal) = &self.identity_modal else {
            return InspectorShell::new(
                div().p_4().child("Identity form not initialized"),
                div().p_4().child("Draft state is missing."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let title = if is_create {
            "New Identity"
        } else {
            "Edit Identity"
        };

        let header = InspectorShell::header(
            title,
            Some("Configure reusable SSH key and credentials"),
            None,
            None,
            "close-identity-edit-inspector-btn",
            false,
        );

        let mut form_body = div().flex().flex_col().gap_3();

        form_body = form_body
            .child(simple_field("Identity Name", modal.name.clone()))
            .child(simple_field("Username (e.g. root)", modal.username.clone()))
            .child(simple_field("Private Key Path", modal.key_path.clone()))
            .child(simple_field("Password (optional)", modal.password.clone()))
            .child(simple_field(
                "Passphrase (optional)",
                modal.passphrase.clone(),
            ))
            .child(simple_field("Tags (comma separated)", modal.tags.clone()));

        let mut footer = div()
            .p_4()
            .border_t_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(surface())
            .flex()
            .flex_col()
            .gap_2();

        if let Some(ref err) = modal.error {
            footer = footer.child(error_notice(err.clone()));
        }

        let buttons_row = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(
                AppButton::new(
                    "identity-inspector-cancel-btn",
                    "Cancel",
                    ButtonVariant::Ghost,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.inspector_close(cx);
                })),
            )
            .child(
                AppButton::new(
                    "identity-inspector-save-btn",
                    "Save",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.save_identity_modal(cx);
                    app.inspector_close(cx);
                })),
            );

        footer = footer.child(buttons_row);

        InspectorShell::new(
            header,
            form_body,
            Some(footer),
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Open identity inspector in a specific mode.
    pub(crate) fn open_identity_inspector_mode(
        &mut self,
        identity_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Identity(identity_id);
        if self
            .inspector_coordinator
            .open(vault_id, target, mode, None)
        {
            self.selected_host_id = None;
            self.open_identity_modal(identity_id, cx);
        }
        cx.notify();
    }
}

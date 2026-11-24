#![allow(dead_code)]
//! Vault inspector rendering and actions for View, Edit, and Create modes.

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
    /// Render the unified Vault Inspector for viewing or editing vaults.
    pub(crate) fn render_vault_inspector(
        &mut self,
        vault_id: Option<String>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_vault_view_inspector(vault_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_vault_edit_inspector(vault_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render vault inspector in View mode.
    fn render_vault_view_inspector(
        &mut self,
        vault_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let id_str = vault_id.unwrap_or_else(|| self.active_vault_id.clone());
        let vault = self
            .available_vaults
            .iter()
            .find(|v| v.id == id_str)
            .cloned();

        let Some(vault) = vault else {
            return InspectorShell::new(
                div().p_4().child("Vault not found"),
                div()
                    .p_4()
                    .child("The requested vault does not exist or has been removed."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let v_id = vault.id.clone();
        let v_name = vault.name.clone();
        let mode_str = if vault.is_remote {
            "Cloud / Remote"
        } else {
            "Local Encrypted"
        };
        let lock_str = if vault.is_locked {
            "Locked"
        } else {
            "Unlocked / Active"
        };

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x332800))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0xf5a623))
            .child(mode_str);

        let edit_btn = div()
            .id("inspector-edit-vault-btn")
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
                let id_clone = v_id.clone();
                move |app, _, _, cx| {
                    app.open_vault_inspector_mode(Some(id_clone.clone()), InspectorMode::Edit, cx);
                }
            }))
            .child("Edit");

        let header = InspectorShell::header(
            v_name.clone(),
            Some(format!("Vault ID: {}", v_id)),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            "close-vault-inspector-btn",
            false,
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new("vault-view-sync-btn", "Sync Now", ButtonVariant::Primary).on_click(
                    cx.listener(|app, _, _, cx| {
                        app.sync_active_vault(cx);
                    }),
                ),
            )
            .child(
                AppButton::new(
                    "vault-view-lock-btn",
                    "Lock Vault",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.lock_active_vault(cx);
                })),
            );
        body = body.child(actions_row);

        // Details
        let mut details_items = div().flex().flex_col().gap_1();
        details_items = details_items
            .child(self.render_detail_row("Vault Name", v_name))
            .child(self.render_detail_row("Storage Mode", mode_str.into()))
            .child(self.render_detail_row("Status", lock_str.into()));
        if let Some(ref url) = vault.server_url {
            details_items = details_items.child(self.render_detail_row("Server URL", url.clone()));
        }
        body = body.child(simple_section("Vault Security & Sync", details_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render vault inspector in Edit or Create mode.
    fn render_vault_edit_inspector(
        &mut self,
        _vault_id: Option<String>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = if is_create { "New Vault" } else { "Edit Vault" };

        let header = InspectorShell::header(
            title,
            Some("Configure encrypted database and sync credentials"),
            None,
            None,
            "close-vault-edit-inspector-btn",
            false,
        );

        let body = div().flex().flex_col().gap_3().p_2().child(
            div()
                .text_xs()
                .text_color(text_secondary())
                .child("Vault configuration options (name, remote server URL, passphrase)."),
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
                AppButton::new("vault-inspector-cancel-btn", "Cancel", ButtonVariant::Ghost)
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.inspector_close(cx);
                    })),
            )
            .child(
                AppButton::new("vault-inspector-save-btn", "Save", ButtonVariant::Primary)
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

    /// Open vault inspector in a specific mode.
    pub(crate) fn open_vault_inspector_mode(
        &mut self,
        vault_id: Option<String>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_context = vault_id
            .clone()
            .unwrap_or_else(|| self.active_vault_id.clone());
        let target = InspectorTarget::Vault(vault_id);
        let _ = self
            .inspector_coordinator
            .open(vault_context, target, mode, None);
        cx.notify();
    }
}

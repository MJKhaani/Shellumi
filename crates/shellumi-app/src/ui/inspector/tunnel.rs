#![allow(dead_code)]
//! Tunnel inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout;
use crate::{SavedTunnel, Shellumi};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};

impl Shellumi {
    /// Render the unified Tunnel Inspector for viewing or editing an SSH port forwarding tunnel.
    pub(crate) fn render_tunnel_inspector(
        &mut self,
        tunnel_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_tunnel_view_inspector(tunnel_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_tunnel_edit_inspector(tunnel_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render tunnel inspector in View mode.
    fn render_tunnel_view_inspector(
        &mut self,
        tunnel_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let tunnels = match &self.tunnel_modal {
            Some(modal) => modal.tunnels.as_slice(),
            None => &[],
        };

        let tunnel = tunnel_id.and_then(|id| tunnels.iter().find(|t| t.id == id));

        let Some(tunnel) = tunnel else {
            return InspectorShell::new(
                div().p_4().child("Tunnel not found"),
                div()
                    .p_4()
                    .child("The selected port forward does not exist or was deleted."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let id_val = tunnel.id;
        let t_name = tunnel.name.clone();
        let direction_str = match &tunnel.rule.forwarding_type {
            shellumi_core::ForwardingType::Local => "Local Forward",
            shellumi_core::ForwardingType::Remote => "Remote Forward",
            shellumi_core::ForwardingType::Dynamic => "Dynamic SOCKS5",
        };

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x1e3a5f))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x389bf2))
            .child(direction_str);

        let edit_btn = div()
            .id("inspector-edit-tunnel-btn")
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
                app.open_tunnel_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let header = InspectorShell::header(
            t_name.clone(),
            Some(direction_str),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            "close-tunnel-inspector-btn",
            false,
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "tunnel-view-start-btn",
                    "Start Tunnel",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(move |_app, _, _, _cx| {
                    // Trigger start tunnel
                })),
            )
            .child(
                AppButton::new(
                    "tunnel-view-edit-btn",
                    "Edit Settings",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.open_tunnel_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
                })),
            );
        body = body.child(actions_row);

        // Tunnel Configuration
        let mut config_items = div().flex().flex_col().gap_1();
        config_items = config_items
            .child(self.render_detail_row("Tunnel Name", t_name))
            .child(self.render_detail_row("Forwarding Mode", direction_str.into()))
            .child(
                self.render_detail_row(
                    "Auto Start",
                    if tunnel.auto_start {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                    .into(),
                ),
            );
        body = body.child(simple_section("Configuration", config_items));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render tunnel inspector in Edit or Create mode.
    fn render_tunnel_edit_inspector(
        &mut self,
        _tunnel_id: Option<i64>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = if is_create {
            "New Port Forward"
        } else {
            "Edit Port Forward"
        };

        let header = InspectorShell::header(
            title,
            Some("Configure SSH local, remote, or dynamic tunnel"),
            None,
            None,
            "close-tunnel-edit-inspector-btn",
            false,
        );

        let body = div().flex().flex_col().gap_3().p_2().child(
            div()
                .text_xs()
                .text_color(text_secondary())
                .child("Port forwarding editor fields (name, bind port, destination host/port)."),
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
                    "tunnel-inspector-cancel-btn",
                    "Cancel",
                    ButtonVariant::Ghost,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.inspector_close(cx);
                })),
            )
            .child(
                AppButton::new("tunnel-inspector-save-btn", "Save", ButtonVariant::Primary)
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

    /// Open tunnel inspector in a specific mode.
    pub(crate) fn open_tunnel_inspector_mode(
        &mut self,
        tunnel_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Tunnel(tunnel_id);
        let _ = self
            .inspector_coordinator
            .open(vault_id, target, mode, None);
        cx.notify();
    }
}

#![allow(dead_code)]
//! Remote Desktop Session Inspector rendering live technical telemetry and active device status.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::simple_section;
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout::{self, InspectorPresentation};
use crate::{RemoteDesktopProtocol, RemoteDesktopStatus, Shellumi};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Styled, Window,
};

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

impl Shellumi {
    /// Render the unified Remote Desktop Session Inspector displaying live technical telemetry.
    pub(crate) fn render_session_inspector(
        &mut self,
        tab_idx_opt: Option<u64>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let active_tab_idx = tab_idx_opt
            .map(|id| id as usize)
            .or(self.active_tab_idx)
            .unwrap_or(0);

        let rd_state = self
            .tabs
            .get(active_tab_idx)
            .and_then(|t| t.remote_desktop_state.as_ref());

        let Some(state) = rd_state else {
            return InspectorShell::new(
                div().p_4().child("Session Telemetry"),
                div()
                    .p_4()
                    .child("No active remote desktop session selected."),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let is_vnc = state.protocol == RemoteDesktopProtocol::Vnc;
        let is_connected = state.status == RemoteDesktopStatus::Connected;

        let proto_badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(if is_vnc { rgb(0x3a1b5c) } else { rgb(0x103d3d) })
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if is_vnc { rgb(0xbb86fc) } else { rgb(0x3de0d8) })
            .child(if is_vnc { "VNC" } else { "RDP" });

        let close_btn = InspectorShell::close_button(
            "close-session-inspector-btn",
            false,
            |app, _, _, cx| {
                app.inspector_close(cx);
            },
            cx,
        );
        let header = InspectorShell::header(
            state.target.clone(),
            Some(format!(
                "Remote Desktop Session · {}",
                if is_connected { "Active" } else { "Offline" }
            )),
            Some(proto_badge.into_any_element()),
            None,
            Some(close_btn),
        );

        let mut body = div().flex().flex_col().gap_4();

        // 1. Live Performance Metrics
        let fps_val = state
            .fps
            .map(|f| format!("{f} FPS"))
            .unwrap_or_else(|| "— FPS".into());
        let latency_val = state
            .latency_ms
            .map(|l| format!("{l} ms"))
            .unwrap_or_else(|| "— ms".into());
        let res_val = format!(
            "{} × {} ({} bpp)",
            state.width, state.height, state.color_depth
        );

        let mut perf_items = div().flex().flex_col().gap_1();
        perf_items = perf_items
            .child(self.render_detail_row("Frame Rate", fps_val))
            .child(self.render_detail_row("Round-Trip Latency", latency_val))
            .child(self.render_detail_row("Negotiated Resolution", res_val))
            .child(self.render_detail_row(
                "Display Resize",
                if state.display_control_ready {
                    "Dynamic (Display Control Active)".into()
                } else {
                    "Static (Fixed Display Size)".into()
                },
            ));
        body = body.child(simple_section("Performance Telemetry", perf_items));

        // 2. Audio & Media Redirection
        let (audio_status, audio_detail) = if !state.enable_audio {
            ("Disabled", "Audio channel disabled".into())
        } else if state.audio_muted {
            ("Muted", format!("Volume: {}%", state.audio_volume))
        } else if state.audio_active {
            (
                "Active",
                format!(
                    "Volume: {}% · {}",
                    state.audio_volume,
                    format_bytes(state.audio_bytes_received)
                ),
            )
        } else {
            ("Idle", format!("Volume: {}%", state.audio_volume))
        };

        let (mic_status, mic_detail) = if is_vnc {
            ("Not Supported", "Microphone not available in VNC".into())
        } else if !state.enable_microphone {
            ("Disabled", "Microphone channel disabled".into())
        } else if state.mic_muted {
            ("Muted", format!("Volume: {}%", state.mic_volume))
        } else if state.mic_active {
            (
                "Active",
                format!(
                    "Volume: {}% · {}",
                    state.mic_volume,
                    format_bytes(state.mic_bytes_sent)
                ),
            )
        } else {
            ("Idle", format!("Volume: {}%", state.mic_volume))
        };

        let mut media_items = div().flex().flex_col().gap_1();
        media_items = media_items
            .child(
                self.render_detail_row("Audio Output", format!("{audio_status} ({audio_detail})")),
            )
            .child(
                self.render_detail_row("Microphone Input", format!("{mic_status} ({mic_detail})")),
            )
            .child(self.render_detail_row("Clipboard Redirection", "Bidirectional Sync".into()));

        if !is_vnc {
            let drive_status = if state.drive_mounted {
                format!(
                    "Mounted: {} (Read: {}, Written: {})",
                    state.drive_name.as_deref().unwrap_or("Shared"),
                    format_bytes(state.drive_bytes_read),
                    format_bytes(state.drive_bytes_written)
                )
            } else {
                "Unmounted (Select folder in toolbar)".into()
            };
            media_items =
                media_items.child(self.render_detail_row("Shared Drive Redirection", drive_status));
        }

        body = body.child(simple_section("Peripheral Redirection", media_items));

        // Footer Actions
        let footer_buttons = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(
                AppButton::new(
                    "session-inspector-reconnect-btn",
                    "Reconnect",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.remote_desktop_reconnect(active_tab_idx, cx);
                })),
            )
            .child(
                AppButton::new(
                    "session-inspector-disconnect-btn",
                    "Disconnect",
                    ButtonVariant::Danger,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.remote_desktop_disconnect(active_tab_idx, cx);
                    app.inspector_close(cx);
                })),
            );

        InspectorShell::new(
            header,
            body,
            Some(
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .bg(surface())
                    .child(footer_buttons),
            ),
            self.active_inspector_width(),
        )
    }

    /// Open or toggle the Session Inspector for an active Remote Desktop session.
    pub(crate) fn toggle_session_inspector(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        let target = InspectorTarget::Session(Some(tab_idx as u64));
        if self.inspector_coordinator.is_open()
            && self.inspector_coordinator.active().map(|a| &a.target) == Some(&target)
        {
            self.inspector_close(cx);
        } else {
            let vault_id = self.active_vault_id.clone();
            let _ = self
                .inspector_coordinator
                .open(vault_id, target, InspectorMode::View, None);
            cx.notify();
        }
    }
}

//! Remote desktop session rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_remote_desktop_session(
        &self,
        tab_idx: usize,
        state: &RemoteDesktopState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let is_vnc = state.protocol == RemoteDesktopProtocol::Vnc;
        let is_connected = state.status == RemoteDesktopStatus::Connected;

        // 1. Top control toolbar
        let proto_pill = div()
            .flex()
            .items_center()
            .gap_1p5()
            .px_2p5()
            .py_1()
            .rounded_md()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .bg(if is_vnc { rgb(0x3a1b5c) } else { rgb(0x103d3d) })
            .text_color(if is_vnc { rgb(0xbb86fc) } else { rgb(0x3de0d8) })
            .child(if is_vnc { "🖥️ VNC" } else { "🪟 RDP" });

        let (status_bg, status_color, status_text) = match state.status {
            RemoteDesktopStatus::Connected => {
                let lat_str = state
                    .latency_ms
                    .map(|l| format!("{l}ms"))
                    .unwrap_or_else(|| "— ms".into());
                let fps_str = state
                    .fps
                    .map(|f| format!("{f} FPS"))
                    .unwrap_or_else(|| "— FPS".into());
                (
                    rgb(0x0e3b2e),
                    rgb(0x3dd68c),
                    format!(
                        "● Connected · {lat_str} · {fps_str} · {}x{} · {}bpp",
                        state.width, state.height, state.color_depth
                    ),
                )
            }
            RemoteDesktopStatus::Connecting => (
                rgb(0x1a2e3b),
                rgb(MacTheme::ACCENT_BLUE),
                "◌ Connecting…".to_string(),
            ),
            RemoteDesktopStatus::NotConnected => (
                rgb(0x282338),
                rgb(0xbb86fc),
                "○ Not connected (native engine in development)".to_string(),
            ),
            RemoteDesktopStatus::Disconnected => {
                (rgb(0x3e1717), rgb(0xeb5757), "○ Disconnected".to_string())
            }
            RemoteDesktopStatus::Error => (
                rgb(0x3e1717),
                rgb(MacTheme::DANGER),
                format!("⚠ Error: {}", state.error.as_deref().unwrap_or("Failed")),
            ),
        };

        let status_pill = div()
            .flex()
            .items_center()
            .gap_1p5()
            .px_2p5()
            .py_1()
            .rounded_md()
            .text_xs()
            .bg(status_bg)
            .text_color(status_color)
            .child(status_text);

        let cad_btn = div()
            .id(("rd-cad", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(MacTheme::SURFACE_ACTIVE))
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_send_key(
                    tab_idx,
                    "Ctrl+Alt+Del",
                    rfb_constants::KEYSYM_DELETE,
                    cx,
                );
            }))
            .child("Ctrl+Alt+Del");

        let win_btn = div()
            .id(("rd-win", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(MacTheme::SURFACE_ACTIVE))
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_send_key(
                    tab_idx,
                    "Win/Super",
                    rfb_constants::KEYSYM_SUPER_L,
                    cx,
                );
            }))
            .child("⊞ Win");

        let alt_tab_btn = div()
            .id(("rd-alttab", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(MacTheme::SURFACE_ACTIVE))
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_send_key(tab_idx, "Alt+Tab", rfb_constants::KEYSYM_TAB, cx);
            }))
            .child("Alt+Tab");

        let clip_btn = div()
            .id(("rd-clip", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(MacTheme::SURFACE_ACTIVE))
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_sync_clipboard(tab_idx, cx);
            }))
            .child("📋 Sync Clipboard");

        let scale_fit = div()
            .id(("rd-scale-fit", tab_idx))
            .px_2()
            .py_1()
            .rounded_l_sm()
            .text_xs()
            .cursor_pointer()
            .bg(if state.scale_mode == RemoteDesktopScaleMode::Fit {
                rgb(MacTheme::ACCENT_BLUE)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .text_color(if state.scale_mode == RemoteDesktopScaleMode::Fit {
                rgb(0xffffff)
            } else {
                rgb(MacTheme::TEXT_SECONDARY)
            })
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_set_scale_mode(tab_idx, RemoteDesktopScaleMode::Fit, cx);
            }))
            .child("Fit");

        let scale_orig = div()
            .id(("rd-scale-orig", tab_idx))
            .px_2()
            .py_1()
            .text_xs()
            .cursor_pointer()
            .bg(if state.scale_mode == RemoteDesktopScaleMode::Original {
                rgb(MacTheme::ACCENT_BLUE)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .text_color(if state.scale_mode == RemoteDesktopScaleMode::Original {
                rgb(0xffffff)
            } else {
                rgb(MacTheme::TEXT_SECONDARY)
            })
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_set_scale_mode(tab_idx, RemoteDesktopScaleMode::Original, cx);
            }))
            .child("100%");

        let scale_fill = div()
            .id(("rd-scale-fill", tab_idx))
            .px_2()
            .py_1()
            .rounded_r_sm()
            .text_xs()
            .cursor_pointer()
            .bg(if state.scale_mode == RemoteDesktopScaleMode::Fill {
                rgb(MacTheme::ACCENT_BLUE)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .text_color(if state.scale_mode == RemoteDesktopScaleMode::Fill {
                rgb(0xffffff)
            } else {
                rgb(MacTheme::TEXT_SECONDARY)
            })
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_set_scale_mode(tab_idx, RemoteDesktopScaleMode::Fill, cx);
            }))
            .child("Fill");

        let view_only_btn = div()
            .id(("rd-viewonly", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .text_xs()
            .cursor_pointer()
            .bg(if state.view_only {
                rgb(0x4a3200)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .text_color(if state.view_only {
                rgb(0xf5a623)
            } else {
                rgb(MacTheme::TEXT_SECONDARY)
            })
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_toggle_view_only(tab_idx, cx);
            }))
            .child(if state.view_only {
                "👁️ View Only"
            } else {
                "🖱️ Interactive"
            });

        let disc_btn = if is_connected || state.status == RemoteDesktopStatus::Connecting {
            div()
                .id(("rd-disconnect", tab_idx))
                .px_2p5()
                .py_1()
                .rounded_sm()
                .text_xs()
                .bg(rgb(0x3e1717))
                .text_color(rgb(MacTheme::DANGER))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(MacTheme::DANGER)).text_color(rgb(0xffffff)))
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.remote_desktop_disconnect(tab_idx, cx);
                }))
                .child("⏹ Disconnect")
        } else {
            div()
                .id(("rd-reconnect", tab_idx))
                .px_2p5()
                .py_1()
                .rounded_sm()
                .text_xs()
                .bg(rgb(MacTheme::ACCENT_BLUE))
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.remote_desktop_reconnect(tab_idx, cx);
                }))
                .child(if state.status == RemoteDesktopStatus::NotConnected {
                    "▶ Connect"
                } else {
                    "🔄 Reconnect"
                })
        };

        let action_note = state.last_action_note.as_ref().map(|note| {
            div()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(rgb(MacTheme::SURFACE_ACTIVE))
                .text_xs()
                .text_color(rgb(MacTheme::ACCENT_BLUE))
                .child(note.clone())
        });

        let toolbar = div()
            .h(px(44.0))
            .px_4()
            .flex()
            .items_center()
            .justify_between()
            .bg(rgb(MacTheme::SURFACE))
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(proto_pill)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(state.target.clone()),
                    )
                    .child(status_pill)
                    .children(action_note),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(cad_btn)
                            .child(win_btn)
                            .child(alt_tab_btn)
                            .child(clip_btn),
                    )
                    .child(div().w(px(1.0)).h(px(16.0)).bg(rgb(MacTheme::SEPARATOR)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(scale_fit)
                            .child(scale_orig)
                            .child(scale_fill),
                    )
                    .child(view_only_btn)
                    .child(disc_btn),
            );

        fn spec_row(label: &'static str, val: &str) -> gpui::Div {
            div()
                .flex()
                .items_center()
                .justify_between()
                .py_1()
                .px_2p5()
                .rounded_sm()
                .bg(rgb(MacTheme::SURFACE_ACTIVE))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child(label),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child(val.to_string()),
                )
        }

        let viewport = match state.status {
            RemoteDesktopStatus::Connected => self.render_remote_desktop_canvas(tab_idx, state, cx),
            RemoteDesktopStatus::NotConnected | RemoteDesktopStatus::Connecting => {
                let is_connecting = state.status == RemoteDesktopStatus::Connecting;
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x0c0f14))
                    .p_6()
                    .child(
                        div()
                            .w(px(560.0))
                            .p_6()
                            .rounded_xl()
                            .bg(rgb(MacTheme::SURFACE))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .child(div().text_2xl().child(if is_vnc { "🖥️" } else { "🪟" }))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_base()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                            .child(format!(
                                                                "{} ({})",
                                                                state.host_name,
                                                                if is_vnc { "VNC / RFB" } else { "RDP Session" }
                                                            )),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                                            .child(state.target.clone()),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2p5()
                                            .py_1()
                                            .rounded_md()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .bg(if is_connecting { rgb(0x1a2e3b) } else { rgb(0x282338) })
                                            .text_color(if is_connecting { rgb(MacTheme::ACCENT_BLUE) } else { rgb(0xbb86fc) })
                                            .child(if is_connecting { "◌ Connecting…" } else { "○ Not Connected" }),
                                    ),
                            )
                            .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child("Session Technical Specification"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
                                            .child(spec_row("Protocol", if is_vnc { "RFB 3.8 (VNC)" } else { "RDP 10.x (IronRDP)" }))
                                            .child(spec_row("Target Endpoint", &state.target))
                                            .child(spec_row("Display Geometry", &format!("{}×{} pixels", state.width, state.height)))
                                            .child(spec_row("Color Depth", &format!("{} bpp", state.color_depth)))
                                            .child(spec_row("Scaling Mode", match state.scale_mode {
                                                RemoteDesktopScaleMode::Fit => "Fit Window",
                                                RemoteDesktopScaleMode::Original => "1:1 Original",
                                                RemoteDesktopScaleMode::Fill => "Fill / Stretch",
                                            }))
                                            .child(spec_row("Interaction Mode", if state.view_only { "View-Only (Observe)" } else { "Interactive" }))
                                            .child(spec_row("Telemetry Latency", &state.latency_ms.map(|l| format!("{l} ms")).unwrap_or_else(|| "Unmeasured (awaiting live frames)".into())))
                                            .child(spec_row("Telemetry FPS", &state.fps.map(|f| format!("{f} FPS")).unwrap_or_else(|| "Unmeasured (awaiting live frames)".into()))),
                                    ),
                            )
                            .child(
                                div()
                                    .p_3()
                                    .rounded_md()
                                    .bg(rgb(0x1e1b2e))
                                    .border_1()
                                    .border_color(rgb(0x352e4d))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1p5()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xbb86fc))
                                            .child("⚡ Native Engine Architecture"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child(
                                                "Per AGENTS.md Rule 7, synthetic frames, mock desktops, and fake metrics have been removed. \
                                                 Phase 1–6 native protocol engine integration is in active execution (crates/shellumi-remote-desktop, \
                                                 shellumi-vnc, and shellumi-rdp)."
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_end()
                                    .gap_2()
                                    .mt_2()
                                    .child(
                                        div()
                                            .id(("rd-connect-center", tab_idx))
                                            .px_4()
                                            .py_2()
                                            .rounded_md()
                                            .bg(rgb(MacTheme::ACCENT_BLUE))
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.remote_desktop_reconnect(tab_idx, cx);
                                            }))
                                            .child(if is_connecting { "◌ Connecting…" } else { "▶ Connect Session" }),
                                    ),
                            ),
                    )
            }
            RemoteDesktopStatus::Disconnected | RemoteDesktopStatus::Error => {
                let is_err = state.status == RemoteDesktopStatus::Error;
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x0c0f14))
                    .child(
                        div()
                            .w(px(460.0))
                            .p_6()
                            .rounded_xl()
                            .bg(rgb(MacTheme::SURFACE))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_3()
                            .child(div().text_2xl().child(if is_err { "⚠" } else { "○" }))
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_err {
                                        rgb(MacTheme::DANGER)
                                    } else {
                                        rgb(MacTheme::TEXT_PRIMARY)
                                    })
                                    .child(if is_err {
                                        "Connection Error"
                                    } else {
                                        "Session Disconnected"
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child(if let Some(err) = &state.error {
                                        format!(
                                            "Failed to connect to {} ({}): {err}",
                                            state.host_name, state.target
                                        )
                                    } else {
                                        format!(
                                            "Session to {} ({}) has ended.",
                                            state.host_name, state.target
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .id(("rd-reconnect-center", tab_idx))
                                    .mt_2()
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.remote_desktop_reconnect(tab_idx, cx);
                                    }))
                                    .child("🔄 Reconnect Session"),
                            ),
                    )
            }
        };

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::CONTENT_BG))
            .child(toolbar)
            .child(viewport)
    }
}

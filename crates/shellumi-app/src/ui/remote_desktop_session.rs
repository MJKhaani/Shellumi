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

        let status_pill = div()
            .flex()
            .items_center()
            .gap_1p5()
            .px_2p5()
            .py_1()
            .rounded_md()
            .text_xs()
            .bg(if is_connected {
                rgb(0x0e3b2e)
            } else {
                rgb(0x3e1717)
            })
            .text_color(if is_connected {
                rgb(0x3dd68c)
            } else {
                rgb(0xeb5757)
            })
            .child(if is_connected {
                format!(
                    "● Connected · {}ms · {} FPS · {}x{} · {}bpp",
                    state.latency_ms, state.fps, state.width, state.height, state.color_depth
                )
            } else {
                "○ Disconnected".to_string()
            });

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

        let disc_btn = if is_connected {
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
                .child("🔄 Reconnect")
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

        let viewport = if is_connected {
            let canvas_area = div()
                .id(("rd-canvas", tab_idx))
                .relative()
                .cursor_pointer()
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.remote_desktop_click_canvas(tab_idx, 640, 480, cx);
                }));

            let desktop_ui = if is_vnc {
                div()
                    .w_full()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x1e152a))
                    .child(
                        div()
                            .h(px(28.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .bg(rgb(0x0a0710))
                            .text_xs()
                            .text_color(rgb(0xdfdbe5))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(div().font_weight(FontWeight::BOLD).child("Activities"))
                                    .child("Terminal")
                                    .child("Files"),
                            )
                            .child(div().font_weight(FontWeight::MEDIUM).child("Tue Nov 10 20:30"))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child("📶")
                                    .child("🔊")
                                    .child("🔋 100%")
                                    .child("⏻"),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(680.0))
                                    .h(px(380.0))
                                    .rounded_lg()
                                    .bg(rgb(0x110d19))
                                    .border_1()
                                    .border_color(rgb(0x3e2b54))
                                    .shadow_md()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .h(px(32.0))
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .bg(rgb(0x1c1328))
                                            .rounded_t_lg()
                                            .border_b_1()
                                            .border_color(rgb(0x2d1f40))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child(
                                                        div()
                                                            .w(px(10.0))
                                                            .h(px(10.0))
                                                            .rounded_full()
                                                            .bg(rgb(0xe95420)),
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(10.0))
                                                            .h(px(10.0))
                                                            .rounded_full()
                                                            .bg(rgb(0x77216f)),
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(10.0))
                                                            .h(px(10.0))
                                                            .rounded_full()
                                                            .bg(rgb(0x5e2750)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(0xdfdbe5))
                                                    .child(format!(
                                                        "user@{}: ~ (Remote VNC)",
                                                        state.host_name
                                                    )),
                                            )
                                            .child(div().w(px(30.0))),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_4()
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text_xs()
                                            .text_color(rgb(0x4af626))
                                            .child(format!(
                                                "Linux {} 6.8.0-generic #42-Ubuntu SMP x86_64",
                                                state.host_name
                                            ))
                                            .child(div().h_2())
                                            .child(
                                                div()
                                                    .text_color(rgb(0xffffff))
                                                    .child("user@server:~$ htop --sort-key PERCENT_CPU"),
                                            )
                                            .child(div().h_1())
                                            .child(
                                                div()
                                                    .text_color(rgb(0x389bf2))
                                                    .child("  CPU[|||||||||||||||||||||         42.8%]   Tasks: 182, 4 running"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0xbb86fc))
                                                    .child("  Mem[|||||||||||||||||||||||||    5.28G/16.0G]  Uptime: 24 days, 11:42"),
                                            )
                                            .child(div().h_2())
                                            .child(
                                                div()
                                                    .text_color(rgb(0xe0e0e0))
                                                    .child("  PID USER      PRI  NI  VIRT   RES   SHR S CPU% MEM%   TIME+  COMMAND"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3dd68c))
                                                    .child(" 1420 user       20   0 1824M  340M  112M S 18.2  2.1 48:12.33 /usr/bin/gnome-shell"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3dd68c))
                                                    .child(" 2941 root       20   0  982M  128M   48M S 12.4  0.8 19:40.11 /usr/bin/dockerd"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3dd68c))
                                                    .child(" 4108 user       20   0  412M   84M   32M S  8.1  0.5 05:22.90 /usr/bin/vncserver"),
                                            ),
                                    ),
                            ),
                    )
            } else {
                div()
                    .w_full()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x0f2027))
                    .child(
                        div()
                            .flex_1()
                            .p_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(720.0))
                                    .h(px(400.0))
                                    .rounded_lg()
                                    .bg(rgb(0x0c0c0c))
                                    .border_1()
                                    .border_color(rgb(0x2b3846))
                                    .shadow_md()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .h(px(32.0))
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .bg(rgb(0x19232d))
                                            .rounded_t_lg()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child("⚡")
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(0xffffff))
                                                            .child(format!(
                                                                "Administrator: Windows PowerShell — {}",
                                                                state.host_name
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_3()
                                                    .text_xs()
                                                    .text_color(rgb(0xcccccc))
                                                    .child("─")
                                                    .child("□")
                                                    .child("✕"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_4()
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text_xs()
                                            .text_color(rgb(0xcccccc))
                                            .child("Windows PowerShell")
                                            .child(
                                                "Copyright (C) Microsoft Corporation. All rights reserved.",
                                            )
                                            .child(div().h_2())
                                            .child(
                                                div()
                                                    .text_color(rgb(0xffffff))
                                                    .child("PS C:\\Users\\Administrator> Get-Service | Where-Object Status -eq 'Running' | Select-Object -First 5"),
                                            )
                                            .child(div().h_1())
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3de0d8))
                                                    .child("Status   Name               DisplayName"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3de0d8))
                                                    .child("------   ----               -----------"),
                                            )
                                            .child("Running  AudioSrv           Windows Audio")
                                            .child("Running  Dhcp               DHCP Client")
                                            .child("Running  EventLog           Windows Event Log")
                                            .child("Running  LanmanServer       Server")
                                            .child("Running  TermService        Remote Desktop Services")
                                            .child(div().h_2())
                                            .child(
                                                div()
                                                    .text_color(rgb(0xffffff))
                                                    .child("PS C:\\Users\\Administrator> █"),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h(px(40.0))
                            .px_4()
                            .flex()
                            .items_center()
                            .justify_between()
                            .bg(rgb(0x141b22))
                            .border_t_1()
                            .border_color(rgb(0x232c37))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x0078d4))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("⊞"),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x1e2732))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("🔍"),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x1e2732))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("🗔"),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x1e2732))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("📁"),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x1e2732))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("⚡"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .text_xs()
                                    .text_color(rgb(0xcccccc))
                                    .child("ENG")
                                    .child("🔊")
                                    .child("🌐")
                                    .child("20:30"),
                            ),
                    )
            };

            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .bg(rgb(0x0c0f14))
                .flex()
                .items_center()
                .justify_center()
                .child(canvas_area.w_full().h_full().child(desktop_ui))
        } else {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(0x0c0f14))
                .child(
                    div()
                        .w(px(420.0))
                        .p_6()
                        .rounded_lg()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .child(div().text_2xl().child(if is_vnc { "🖥️" } else { "🪟" }))
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child("Remote Desktop Disconnected"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child(format!(
                                    "Session to {} ({}) has ended.",
                                    state.host_name, state.target
                                )),
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

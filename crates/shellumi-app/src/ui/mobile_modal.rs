//! Mobile modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_mobile_modal(
        &self,
        modal: &MobileSimulatorModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("mobile-modal-overlay")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("mobile-modal-box")
            .w(px(760.0))
            .max_h(px(620.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(if let Some(p) = resolve_icon_path("mobile", false, 48) {
                        div()
                            .w(px(22.0))
                            .h(px(22.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().child("📱").into_any_element()
                    })
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Mobile Touch & Virtual Key Simulator (Phase 11)"),
                    ),
            )
            .child(
                div()
                    .id("dismiss-mobile-modal-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_mobile_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Viewport selector & Lifecycle status banner
        let (status_bg, status_fg, status_icon) = match modal.lifecycle_manager.state {
            MobileSessionState::Connected => (rgb(0x133820), rgb(MacTheme::SUCCESS), "🟢"),
            MobileSessionState::BackgroundGracePeriod { .. } => {
                (rgb(0x3a2c10), rgb(MacTheme::WARNING), "🟡")
            }
            MobileSessionState::Suspended { .. } => (rgb(0x3a1a28), rgb(0xf472b6), "🟠"),
            MobileSessionState::Reconnecting { .. } => {
                (rgb(0x183048), rgb(MacTheme::ACCENT_BLUE), "🔄")
            }
            MobileSessionState::Connecting => (rgb(0x323236), rgb(MacTheme::TEXT_SECONDARY), "⏳"),
            MobileSessionState::Disconnected { .. } => (rgb(0x4a1818), rgb(MacTheme::DANGER), "🔴"),
        };

        let is_iphone = modal.preset == MobileViewportPreset::IPhone15Pro;
        let is_pixel = modal.preset == MobileViewportPreset::AndroidPixel8;
        let is_ipad = modal.preset == MobileViewportPreset::IPadPro11;

        let top_row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("preset-iphone")
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if is_iphone {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .bg(if is_iphone {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_color(if is_iphone {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .hover(move |s| {
                                if is_iphone {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.switch_mobile_preset(MobileViewportPreset::IPhone15Pro, cx);
                            }))
                            .child("iPhone 15 Pro"),
                    )
                    .child(
                        div()
                            .id("preset-pixel")
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if is_pixel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .bg(if is_pixel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_color(if is_pixel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .hover(move |s| {
                                if is_pixel {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.switch_mobile_preset(MobileViewportPreset::AndroidPixel8, cx);
                            }))
                            .child("Pixel 8"),
                    )
                    .child(
                        div()
                            .id("preset-ipad")
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if is_ipad {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .bg(if is_ipad {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_color(if is_ipad {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .hover(move |s| {
                                if is_ipad {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.switch_mobile_preset(MobileViewportPreset::IPadPro11, cx);
                            }))
                            .child("iPad Pro 11\""),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(status_fg)
                    .bg(status_bg)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(status_fg)
                    .child(status_icon)
                    .child(modal.lifecycle_manager.state.status_label()),
            );
        modal_box = modal_box.child(top_row);

        // Lifecycle trigger controls
        let lifecycle_toolbar = div()
            .flex()
            .items_center()
            .gap_2()
            .p_2()
            .rounded_md()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("OS Lifecycle:"),
            )
            .child(
                div()
                    .id("life-bg-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(
                            MobileLifecycleEvent::AppBackgrounded { grace_seconds: 30 },
                            cx,
                        );
                    }))
                    .child("📱 Background (30s)"),
            )
            .child(
                div()
                    .id("life-tick-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(
                            MobileLifecycleEvent::BackgroundTimerTick {
                                elapsed_seconds: 10,
                            },
                            cx,
                        );
                    }))
                    .child("⏱ Tick 10s"),
            )
            .child(
                div()
                    .id("life-expire-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(MobileLifecycleEvent::BackgroundExpired, cx);
                    }))
                    .child("⏳ Expire"),
            )
            .child(
                div()
                    .id("life-netdrop-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(MobileLifecycleEvent::NetworkLost, cx);
                    }))
                    .child("📶 Drop Net"),
            )
            .child(
                div()
                    .id("life-reconnect-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(MobileLifecycleEvent::NetworkRestored, cx);
                    }))
                    .child("🔄 Reconnect"),
            )
            .child(
                div()
                    .id("life-foreground-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .bg(rgb(0x133820))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x1c4a2c)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(MobileLifecycleEvent::AppForegrounded, cx);
                    }))
                    .child("☀️ Foreground"),
            );
        modal_box = modal_box.child(lifecycle_toolbar);

        // Simulated Mobile Device Viewport
        let (cols, rows) = modal.preset.terminal_cols_rows();
        let mut screen_area = div()
            .id("simulated-mobile-screen")
            .p_4()
            .rounded_lg()
            .bg(rgb(MacTheme::CONTENT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .flex()
            .flex_col()
            .gap_1()
            .min_h(px(160.0))
            .max_h(px(180.0))
            .overflow_y_scroll();

        let notch = div().flex().justify_center().pb_1().child(
            div()
                .w(px(100.0))
                .h(px(12.0))
                .rounded_full()
                .bg(rgb(0x323236))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(8.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(rgb(MacTheme::SEPARATOR)),
                ),
        );
        screen_area = screen_area.child(notch);

        for line in &modal.simulated_output {
            screen_area = screen_area.child(
                div()
                    .text_xs()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(line.clone()),
            );
        }
        modal_box = modal_box.child(screen_area);

        // Virtual Keyboard Accessory Bar
        let mut accessory_strip = div()
            .id("virtual-accessory-strip")
            .p_2()
            .rounded_md()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .flex()
            .items_center()
            .gap_1p5()
            .overflow_x_scroll();

        // Sticky Modifier Buttons
        let ctrl_active = modal.accessory_bar.has_modifier(VirtualKeyModifier::Ctrl);
        let alt_active = modal.accessory_bar.has_modifier(VirtualKeyModifier::Alt);

        accessory_strip = accessory_strip
            .child(
                div()
                    .id("mod-ctrl-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if ctrl_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .bg(if ctrl_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if ctrl_active {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(move |s| {
                        if ctrl_active {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.toggle_mobile_modifier(VirtualKeyModifier::Ctrl, cx);
                    }))
                    .child("Ctrl"),
            )
            .child(
                div()
                    .id("mod-alt-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if alt_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .bg(if alt_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if alt_active {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(move |s| {
                        if alt_active {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.toggle_mobile_modifier(VirtualKeyModifier::Alt, cx);
                    }))
                    .child("Alt"),
            );

        // Virtual Keys
        for (k_idx, key) in modal.accessory_bar.keys.iter().enumerate() {
            let key_clone = key.clone();
            let label = key.label();
            accessory_strip = accessory_strip.child(
                div()
                    .id(("vkey-btn", k_idx))
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .cursor_pointer()
                    .bg(rgb(0x323236))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.press_mobile_virtual_key(key_clone.clone(), cx);
                    }))
                    .child(label),
            );
        }
        modal_box = modal_box.child(accessory_strip);

        // Input & Hex Telemetry Log
        let mut telemetry = div()
            .id("mobile-telemetry-log")
            .p_2p5()
            .rounded_md()
            .bg(rgb(MacTheme::CONTENT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(90.0))
            .overflow_y_scroll();

        if modal.input_history.is_empty() {
            telemetry = telemetry.child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Tap virtual keys above to stream ANSI escape codes..."),
            );
        } else {
            for entry in modal.input_history.iter().rev().take(6) {
                telemetry = telemetry.child(
                    div()
                        .text_xs()
                        .font_family(".AppleSystemUIFontMonospaced")
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                        .child(entry.clone()),
                );
            }
        }
        modal_box = modal_box.child(telemetry);

        // Footer info
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .pt_2()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child(format!(
                        "Viewport: {} ({} × {} cols/rows)",
                        modal.preset.label(),
                        cols,
                        rows
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Portable Core FFI Bridge: Ready for Swift/Kotlin integration"),
            );
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }
}

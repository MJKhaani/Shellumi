//! Host editor overlay rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_host_editor(
        &self,
        mut result: gpui::Stateful<gpui::Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        if let Some(form) = &self.form {
            let active_cat = form.active_category;
            let hostname_label = if form.protocol == ConnectionProtocol::Serial {
                "Serial device port"
            } else if form.protocol == ConnectionProtocol::Vnc {
                "VNC host or IP"
            } else if form.protocol == ConnectionProtocol::Rdp {
                "RDP host or IP"
            } else {
                "Hostname or IP"
            };
            let port_label = if form.protocol == ConnectionProtocol::Serial {
                "Baud rate"
            } else if form.protocol == ConnectionProtocol::Vnc {
                "Port (5900)"
            } else if form.protocol == ConnectionProtocol::Rdp {
                "Port (3389)"
            } else {
                "Port"
            };

            let editor_categories = [
                (HostEditorCategory::General, "General", "⚙"),
                (HostEditorCategory::Connection, "Connection", "🌐"),
                (HostEditorCategory::Authentication, "Authentication", "🔑"),
                (HostEditorCategory::Proxy, "Proxy", "⇄"),
                (HostEditorCategory::Advanced, "Advanced", "⚡"),
            ];

            let mut nav = div().w(px(160.0)).flex().flex_col().gap_1().p_2();

            for (cat, label, icon) in editor_categories {
                let is_active = cat == active_cat;
                nav = nav.child(
                    div()
                        .id(gpui::SharedString::from(format!(
                            "cat-{}",
                            label.to_lowercase()
                        )))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .text_sm()
                        .cursor_pointer()
                        .font_weight(if is_active {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .bg(if is_active {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgba(0x00000000)
                        })
                        .text_color(if is_active {
                            rgb(0xffffff)
                        } else {
                            rgb(MacTheme::TEXT_SECONDARY)
                        })
                        .hover(|style| {
                            if !is_active {
                                style
                                    .bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            } else {
                                style
                            }
                        })
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.set_host_editor_category(cat, cx);
                        }))
                        .child(div().text_sm().child(icon))
                        .child(label),
                );
            }

            let category_content = match active_cat {
                HostEditorCategory::General => {
                    let mut gen_div = div().flex().flex_col().gap_3().child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Protocol"),
                            )
                            .child({
                                let mut proto_row = div().flex().flex_row().gap_2();
                                let protocols = [
                                    (ConnectionProtocol::Ssh, "SSH"),
                                    (ConnectionProtocol::Mosh, "Mosh"),
                                    (ConnectionProtocol::Telnet, "Telnet"),
                                    (ConnectionProtocol::Serial, "Serial"),
                                    (ConnectionProtocol::Vnc, "VNC"),
                                    (ConnectionProtocol::Rdp, "RDP"),
                                ];
                                for (idx, (p, label)) in protocols.into_iter().enumerate() {
                                    let is_sel = form.protocol == p;
                                    let btn_bg = if is_sel {
                                        rgb(MacTheme::ACCENT_BLUE)
                                    } else {
                                        rgb(MacTheme::INPUT_BG)
                                    };
                                    let btn_fg = if is_sel {
                                        rgb(0xffffff)
                                    } else {
                                        rgb(MacTheme::TEXT_SECONDARY)
                                    };
                                    proto_row = proto_row.child(
                                        div()
                                            .id(("proto-select", idx))
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .bg(btn_bg)
                                            .text_color(btn_fg)
                                            .text_xs()
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.set_form_protocol(p, cx);
                                            }))
                                            .child(label),
                                    );
                                }
                                proto_row
                            }),
                    );

                    if form.protocol == ConnectionProtocol::Telnet {
                        gen_div = gen_div.child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x4a1818))
                                .text_color(rgb(MacTheme::DANGER))
                                .text_xs()
                                .child("⚠️ UNENCRYPTED PROTOCOL: Telnet transmits all credentials, keystrokes, and session data in cleartext over the network."),
                        );
                    } else if form.protocol == ConnectionProtocol::Serial {
                        gen_div = gen_div.child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x133820))
                                .text_color(rgb(MacTheme::SUCCESS))
                                .text_xs()
                                .child("🔌 Direct serial line connection. Specify serial device path and baud rate in Connection."),
                        );
                    } else if form.protocol == ConnectionProtocol::Vnc {
                        gen_div = gen_div.child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x2a1445))
                                .text_color(rgb(0xbb86fc))
                                .text_xs()
                                .child("🖥️ VNC Graphical Desktop: Remote Framebuffer protocol (default port 5900). Supports view-only and custom compression quality."),
                        );
                    } else if form.protocol == ConnectionProtocol::Rdp {
                        gen_div = gen_div.child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x0e2f2f))
                                .text_color(rgb(0x3de0d8))
                                .text_xs()
                                .child("🪟 RDP Graphical Desktop: Remote Desktop Protocol (default port 3389). Supports Windows NLA and multi-resolution remote desktop."),
                        );
                    }

                    gen_div = gen_div
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(host_form_field("Display name", form.name.clone()))
                                .child(host_form_field("Group (optional)", form.group.clone())),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(host_form_field(hostname_label, form.hostname.clone()))
                                .child(host_form_field(port_label, form.port.clone())),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(host_form_field("Username", form.username.clone()))
                                .child(host_form_field("Tags (comma-separated)", form.tags.clone())),
                        )
                        .child(host_form_field("Jump host (optional)", form.jump_host.clone()))
                        .child(self.identity_picker(form.identity_id, false, cx))
                        .child(card(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_sm()
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child("Local Vault Security"),
                                )
                                .child(caption(
                                    "Host keys, passphrases, and credentials are encrypted and stored locally in Turso SQLite.",
                                )),
                        ));
                    gen_div
                }
                HostEditorCategory::Connection => div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(host_form_field(hostname_label, form.hostname.clone()))
                            .child(host_form_field(port_label, form.port.clone())),
                    )
                    .child(host_form_field("Username", form.username.clone())),
                HostEditorCategory::Authentication => {
                    let mut auth_div = div().flex().flex_col().gap_3();
                    if form.protocol == ConnectionProtocol::Ssh {
                        auth_div =
                            auth_div.child(self.identity_picker(form.identity_id, false, cx));
                    } else {
                        auth_div = auth_div.child(
                            div()
                                .p_3()
                                .rounded_md()
                                .bg(rgb(MacTheme::INPUT_BG))
                                .text_sm()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Authentication credentials for this protocol are managed during connection."),
                        );
                    }
                    auth_div
                }
                HostEditorCategory::Proxy => div().flex().flex_col().gap_3().child(
                    host_form_field("Jump host (e.g. jump.corp:2222)", form.jump_host.clone()),
                ),
                HostEditorCategory::Advanced => {
                    div().flex().flex_col().gap_3().child(host_form_field(
                        "Startup snippet ID (optional)",
                        form.startup_snippet.clone(),
                    ))
                }
            };

            let header_row = div()
                .flex()
                .items_center()
                .justify_between()
                .child(heading(
                    if form.id.is_some() {
                        "Edit Host"
                    } else {
                        "New Host"
                    },
                    "Host connection configuration · Local storage",
                ))
                .child(
                    div()
                        .id("close-host-dialog")
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .cursor_pointer()
                        .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.form = None;
                            app.form_error = None;
                            cx.notify();
                        }))
                        .child("✕"),
                );

            let mut body_content = div()
                .id("host-form-body-scroll")
                .flex_1()
                .min_w_0()
                .max_h(px(
                    (window.viewport_size().height.to_f64() as f32 - 230.0).max(160.0)
                ))
                .overflow_y_scroll()
                .child(category_content);

            if let Some(error) = &self.form_error {
                body_content = body_content.child(
                    div()
                        .mt_2()
                        .text_color(rgb(MacTheme::DANGER))
                        .text_sm()
                        .child(error.clone()),
                );
            }

            let body_row = div()
                .flex()
                .items_start()
                .gap_4()
                .child(nav)
                .child(
                    div()
                        .w(px(1.0))
                        .h(px(200.0))
                        .flex()
                        .flex_col()
                        .child(div().h(px(24.0)).bg(gpui::linear_gradient(
                            180.0,
                            gpui::linear_color_stop(rgba(0x46464a00), 0.0),
                            gpui::linear_color_stop(rgb(MacTheme::BORDER), 1.0),
                        )))
                        .child(div().flex_1().bg(rgb(MacTheme::BORDER)))
                        .child(div().h(px(24.0)).bg(gpui::linear_gradient(
                            180.0,
                            gpui::linear_color_stop(rgb(MacTheme::BORDER), 0.0),
                            gpui::linear_color_stop(rgba(0x46464a00), 1.0),
                        ))),
                )
                .child(body_content);

            let footer_row = if self.saving {
                div()
                    .flex()
                    .justify_end()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Saving…")
            } else {
                div()
                    .flex()
                    .justify_end()
                    .gap_3()
                    .child(
                        div()
                            .id("cancel-host")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .text_sm()
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.form = None;
                                app.form_error = None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("save-host")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| app.save_host(cx)))
                            .child("Save"),
                    )
            };

            let editor = div()
                .id("host-editor")
                .w(px(720.0))
                .max_w_full()
                .rounded_xl()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .flex()
                .flex_col()
                .gap_4()
                .p_6()
                .bg(rgb(MacTheme::SURFACE))
                .child(header_row)
                .child(body_row)
                .child(footer_row);

            result = result.child(
                div()
                    .id("host-editor-overlay")
                    .occlude()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(rgba(0x000000bb))
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(editor),
            );
        }
        result
    }
}

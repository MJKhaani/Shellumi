//! Tunnel modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_tunnel_modal(
        &self,
        modal: &TunnelModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("tunnel-modal-overlay")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut modal_box = div()
            .id("tunnel-modal-container")
            .w(px(840.0))
            .max_h(px(720.0))
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
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
                    .child(
                        if let Some(p) = resolve_icon_path("port_forwarding", false, 48) {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().child("⇄").into_any_element()
                        },
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Port forwarding"),
                    ),
            )
            .child(
                div()
                    .id("dismiss-tunnel-modal-btn")
                    .when(self.page == Some("Port Forwarding"), |element| {
                        element.hidden()
                    })
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_tunnel_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Status messages
        if let Some(ref err) = modal.error {
            modal_box = modal_box.child(
                div()
                    .id("tunnel-modal-error")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x4a1818))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("⚠ {err}")),
            );
        }
        if let Some(ref msg) = modal.status_message {
            modal_box = modal_box.child(
                div()
                    .id("tunnel-modal-status")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x133820))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(format!("✓ {msg}")),
            );
        }

        // Two-column layout
        let mut body = div().flex().gap_4().flex_1().overflow_hidden();

        // Left column: list of configured tunnels
        let mut list_col = div()
            .flex()
            .flex_col()
            .w(px(380.0))
            .gap_2()
            .border_r_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pr_4();

        list_col = list_col.child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(format!("CONFIGURED TUNNELS ({})", modal.tunnels.len())),
        );

        if modal.is_loading {
            list_col = list_col.child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Loading tunnels…"),
            );
        } else if modal.tunnels.is_empty() {
            list_col = list_col.child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("No port forwards configured yet. Create one on the right."),
            );
        } else {
            let mut scroll_list = div()
                .id("tunnel-scroll-list")
                .flex()
                .flex_col()
                .gap_2()
                .overflow_y_scroll();
            for (idx, tunnel) in modal.tunnels.iter().enumerate() {
                let is_selected = idx == modal.selected_index;
                let tid = tunnel.id;
                let is_active = self.tabs.iter().any(|tab| {
                    tab.name == format!("Tunnel: {}", tunnel.name)
                        && tab.panes.iter().any(|pane| pane.session.is_some())
                });
                let status_pill = if is_active {
                    ("Session open", "open")
                } else {
                    ("Stopped", "stopped")
                };

                let (flag_str, rule_summary) = match tunnel.rule.forwarding_type {
                    ForwardingType::Local => (
                        "Local (-L)",
                        format!(
                            "{}:{} → {}:{}",
                            tunnel.rule.bind_address.as_deref().unwrap_or("127.0.0.1"),
                            tunnel.rule.bind_port,
                            tunnel.rule.target_host.as_deref().unwrap_or("localhost"),
                            tunnel.rule.target_port.unwrap_or(80)
                        ),
                    ),
                    ForwardingType::Remote => (
                        "Remote (-R)",
                        format!(
                            "{}:{} → {}:{}",
                            tunnel.rule.bind_address.as_deref().unwrap_or("127.0.0.1"),
                            tunnel.rule.bind_port,
                            tunnel.rule.target_host.as_deref().unwrap_or("localhost"),
                            tunnel.rule.target_port.unwrap_or(80)
                        ),
                    ),
                    ForwardingType::Dynamic => (
                        "Dynamic (-D SOCKS)",
                        format!(
                            "{}:{}",
                            tunnel.rule.bind_address.as_deref().unwrap_or("127.0.0.1"),
                            tunnel.rule.bind_port
                        ),
                    ),
                };

                let item_bg = if is_selected {
                    rgb(MacTheme::SURFACE_HOVER)
                } else {
                    rgb(MacTheme::INPUT_BG)
                };

                let mut card = div()
                    .id(("tunnel-item", tid as usize))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .rounded_md()
                    .bg(item_bg)
                    .border_1()
                    .border_color(if is_selected {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.select_tunnel_modal_item(idx, cx);
                    }));

                let glyph_box = div()
                    .w(px(30.0))
                    .h(px(30.0))
                    .rounded_md()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                    .child("⇄");

                let status_dot = div()
                    .w(px(7.0))
                    .h(px(7.0))
                    .rounded_full()
                    .bg(rgb(if is_active {
                        MacTheme::SUCCESS
                    } else {
                        MacTheme::TEXT_TERTIARY
                    }));

                // Top row: glyph + dot + name + status pill
                let top_row = div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(glyph_box)
                    .child(status_dot)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child(tunnel.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child(format!("{flag_str}: {rule_summary}")),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(if is_active {
                                rgb(0x133820)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .text_xs()
                            .text_color(if is_active {
                                rgb(MacTheme::SUCCESS)
                            } else {
                                rgb(MacTheme::TEXT_TERTIARY)
                            })
                            .child(status_pill.0),
                    );
                card = card.child(top_row);

                // Actions row: Start/Stop button + Delete button
                let actions_row = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_1()
                    .child(
                        div()
                            .id(("toggle-tunnel-btn", tid as usize))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(if is_active {
                                rgb(MacTheme::WARNING)
                            } else {
                                rgb(MacTheme::SUCCESS)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_active {
                                rgb(0x000000)
                            } else {
                                rgb(0xffffff)
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, window, cx| {
                                app.toggle_tunnel_state(tid, window, cx);
                            }))
                            .child(if is_active { "⏹ Stop" } else { "▶ Start" }),
                    )
                    .child(
                        div()
                            .id(("del-tunnel-btn", tid as usize))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::DANGER))
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.delete_tunnel_modal(tid, cx);
                            }))
                            .child("🗑 Delete"),
                    );
                card = card.child(
                    div()
                        .id(("edit-tunnel", tid as u64))
                        .px_2()
                        .py_1()
                        .text_sm()
                        .cursor_pointer()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .on_click(cx.listener(move |app, _, _, cx| app.edit_saved_tunnel(tid, cx)))
                        .child("Edit rule"),
                );
                card = card.child(actions_row);

                scroll_list = scroll_list.child(card);
            }
            list_col = list_col.child(scroll_list);
        }
        body = body.child(list_col);

        // Right column: Selected Tunnel Details & Add New Tunnel Form
        let mut right_col = div()
            .id("tunnel-right-col")
            .flex()
            .flex_col()
            .flex_1()
            .gap_4()
            .overflow_y_scroll();

        // Selected tunnel details panel (if one is selected)
        if let Some(selected_tunnel) = modal.tunnels.get(modal.selected_index) {
            let host = match &self.hosts {
                HostState::Loaded(hosts) => hosts
                    .iter()
                    .find(|host| Some(host.id) == selected_tunnel.host_id),
                _ => None,
            };
            let preview_str = if let Some(host) = host {
                let target = format!(
                    "{}@{}",
                    host.destination.username(),
                    host.destination.host()
                );
                let mut args = build_ssh_tunnel_command(
                    &selected_tunnel.rule,
                    &target,
                    host.jump_host.as_deref(),
                );
                let insert = args.len().saturating_sub(1);
                args.splice(
                    insert..insert,
                    ["-p".to_string(), host.destination.port().to_string()],
                );
                format!("ssh {}", args.join(" "))
            } else {
                "Select a saved host to start this rule.".into()
            };

            let details_box = div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_md()
                .bg(rgb(MacTheme::INPUT_BG))
                .border_1()
                .border_color(rgb(MacTheme::SEPARATOR))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                        .child(format!("Saved rule: {}", selected_tunnel.name)),
                )
                .child(div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child("Open the tunnel terminal to inspect authentication and forwarding errors. Traffic counters are not measured."))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child("SSH Command Preview:"),
                        )
                        .child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(MacTheme::CONTENT_BG))
                                .text_xs()
                                .text_color(rgb(MacTheme::SUCCESS))
                                .child(preview_str),
                        ),
                );
            right_col = right_col.child(details_box);
        }

        // Add New Tunnel Form
        let mut form_box = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_md()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("➕ Configure New Tunnel"),
            );

        let mut host_picker = div().flex().flex_wrap().gap_2();
        if let HostState::Loaded(hosts) = &self.hosts {
            for host in hosts.iter().filter(|host| host.is_ssh()) {
                let id = host.id;
                host_picker = host_picker.child(
                    div()
                        .id(("tunnel-host", id as u64))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_sm()
                        .bg(if modal.selected_host_id == Some(id) {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SURFACE)
                        })
                        .text_color(if modal.selected_host_id == Some(id) {
                            rgb(0xffffff)
                        } else {
                            rgb(MacTheme::TEXT_SECONDARY)
                        })
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            if let Some(modal) = &mut app.tunnel_modal {
                                modal.selected_host_id = Some(id);
                            }
                            cx.notify();
                        }))
                        .child(host.name.clone()),
                );
            }
        }
        form_box = form_box
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("SSH host"),
            )
            .child(host_picker);
        // Forwarding Type Buttons
        let ftype_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("ftype-local-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(if modal.forwarding_type == ForwardingType::Local {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_xs()
                    .text_color(if modal.forwarding_type == ForwardingType::Local {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.set_tunnel_forwarding_type(ForwardingType::Local, cx)
                    }))
                    .child("Local (-L)"),
            )
            .child(
                div()
                    .id("ftype-remote-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(if modal.forwarding_type == ForwardingType::Remote {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_xs()
                    .text_color(if modal.forwarding_type == ForwardingType::Remote {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.set_tunnel_forwarding_type(ForwardingType::Remote, cx)
                    }))
                    .child("Remote (-R)"),
            )
            .child(
                div()
                    .id("ftype-dynamic-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(if modal.forwarding_type == ForwardingType::Dynamic {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_xs()
                    .text_color(if modal.forwarding_type == ForwardingType::Dynamic {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.set_tunnel_forwarding_type(ForwardingType::Dynamic, cx)
                    }))
                    .child("Dynamic SOCKS5 (-D)"),
            );
        form_box = form_box.child(ftype_row);

        // Inputs
        form_box = form_box.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Tunnel Name:"),
                )
                .child(modal.name_input.clone()),
        );

        let mut ports_row = div().flex().items_center().gap_3().child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Bind Port:"),
                )
                .child(modal.bind_port_input.clone()),
        );

        if modal.forwarding_type != ForwardingType::Dynamic {
            ports_row = ports_row
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Target Host:"),
                        )
                        .child(modal.target_host_input.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Target Port:"),
                        )
                        .child(modal.target_port_input.clone()),
                );
        }
        form_box = form_box.child(ports_row);

        // Create Button
        form_box = form_box.child(
            div()
                .id("save-tunnel-btn")
                .px_4()
                .py_2()
                .rounded_md()
                .bg(rgb(MacTheme::ACCENT_BLUE))
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .on_click(cx.listener(|app, _, _, cx| {
                    app.save_new_tunnel_modal(cx);
                }))
                .child(if modal.editing_id.is_some() {
                    "Save changes"
                } else {
                    "Add tunnel"
                }),
        );

        right_col = right_col.child(form_box);
        body = body.child(right_col);
        modal_box = modal_box.child(body);

        if self.page == Some("Port Forwarding") {
            div()
                .w_full()
                .child(modal_box.w_full().max_h(px(1800.0)))
                .into_any_element()
        } else {
            overlay.child(modal_box).into_any_element()
        }
    }
}

//! Desktop public inventory page and its actions.
use super::*;

impl Shellumi {
    pub(super) fn refresh_public_inventory(&mut self, cx: &mut Context<Self>) {
        let keys = self.page == Some("Keychain");
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from);
        let Some(home) = home else {
            self.desktop.message = Some("Home directory is unavailable.".into());
            cx.notify();
            return;
        };
        self.desktop.public_records.clear();
        self.desktop.public_selected = 0;
        if keys {
            self.load_identities(cx);
        }
        let task = cx
            .background_executor()
            .spawn(async move { shellumi_storage::desktop::public_ssh_records(&home, keys) });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok(records) => app.desktop.public_records = records,
                    Err(error) => app.desktop.message = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn public_inventory_page(
        &self,
        page: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let keys = page == "Keychain";
        if keys {
            let action_bar = div()
                .id("keychain-action-bar")
                .flex()
                .items_center()
                .justify_between()
                .px_6()
                .py_3()
                .border_b_1()
                .border_color(rgb(MacTheme::BORDER))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            button_icon("generate-ssh-key", "private_key", "+ New key ⌵")
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                                .text_color(rgb(0xffffff))
                                .on_click(
                                    cx.listener(|app, _, window, cx| {
                                        app.start_session(
                                            "Generate SSH identity".into(),
                                            SessionKind::Local,
                                            TerminalCommand::new("ssh-keygen").args(["-t", "ed25519"]),
                                            window,
                                            cx,
                                        )
                                    }),
                                ),
                        )
                        .child(
                            button_icon("new-identity-btn", "identity_card", "+ New Identity").on_click(
                                cx.listener(|app, _, _, cx| {
                                    app.open_identity_modal(None, cx);
                                }),
                            ),
                        )
                        .child(
                            button_icon("cert-btn", "certificate", "Certificate").on_click(cx.listener(|app, _, _, cx| {
                                app.desktop.message = Some("Certificate management: add certificates to identities or OpenSSH config.".into());
                                cx.notify();
                            })),
                        )
                        .child(
                            button_icon("fido2-btn", "fido2", "FIDO2").on_click(cx.listener(|app, _, _, cx| {
                                app.desktop.message = Some("FIDO2 hardware keys supported via OpenSSH sk-ecdsa/sk-ed25519 keys.".into());
                                cx.notify();
                            })),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("keychain-search-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.open_command_palette(window, cx)
                                }))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("search", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("🔍").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .id("keychain-grid-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("grid_view", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("⊞").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .id("keychain-refresh-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.refresh_public_inventory(cx)
                                }))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("calendar_sort", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("📅").into_any_element()
                                    },
                                ),
                        ),
                );

            let mut keys_grid = div().flex().flex_wrap().gap_3();
            for (idx, record) in self.desktop.public_records.iter().enumerate() {
                let is_sel = idx == self.desktop.public_selected;
                keys_grid = keys_grid.child(
                    div()
                        .id(("key-card", idx))
                        .w(px(250.0))
                        .p_3p5()
                        .rounded_xl()
                        .bg(rgb(if is_sel {
                            MacTheme::SURFACE_ACTIVE
                        } else {
                            MacTheme::SURFACE
                        }))
                        .border_1()
                        .border_color(rgb(if is_sel {
                            MacTheme::ACCENT_BLUE
                        } else {
                            MacTheme::BORDER
                        }))
                        .flex()
                        .items_center()
                        .gap_3()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.desktop.public_selected = idx;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(36.0))
                                .rounded_lg()
                                .bg(rgb(MacTheme::BADGE_NAVY))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_base()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("public_key", false, 48)
                                    {
                                        div()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("🔑").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .truncate()
                                        .child(record.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(format!("Type {}", record.key_type)),
                                ),
                        ),
                );
            }
            if self.desktop.public_records.is_empty() {
                keys_grid = keys_grid.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("No SSH keys found in ~/.ssh."),
                );
            }

            let mut identities_grid = div().flex().flex_wrap().gap_3();
            for (idx, id) in self.identities.iter().enumerate() {
                let edit_id = id.id;
                identities_grid = identities_grid.child(
                    div()
                        .id(("identity-card", idx))
                        .w(px(260.0))
                        .p_3p5()
                        .rounded_xl()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .cursor_pointer()
                        .hover(|s| {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                        })
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.open_identity_modal(Some(edit_id), cx);
                        }))
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(36.0))
                                .rounded_lg()
                                .bg(rgb(MacTheme::BADGE_NAVY))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_base()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("identity_card", false, 48)
                                    {
                                        div()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("🪪").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .truncate()
                                        .child(id.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(format!("User: {}", id.username)),
                                ),
                        )
                        .child(
                            div()
                                .id(("edit-identity-badge", idx))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::INPUT_BG))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    if let Some(p) = crate::resolve_icon_path("edit", false, 32) {
                                        div()
                                            .w(px(14.0))
                                            .h(px(14.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("✏️").into_any_element()
                                    },
                                )
                                .child("Edit"),
                        ),
                );
            }
            if self.identities.is_empty() {
                identities_grid = identities_grid.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("No saved reusable identities yet."),
                );
            }

            let daemon = shellumi_core::probe_keychain_daemon();
            let daemon_card = div()
                .p_3()
                .rounded_lg()
                .bg(rgb(MacTheme::SURFACE))
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(format!("🔐 OS Keychain Daemon: {}", daemon.backend_name)),
                        )
                        .child(
                            div()
                                .p_1()
                                .rounded_md()
                                .text_xs()
                                .bg(if daemon.is_available {
                                    rgba(0x30d15826)
                                } else {
                                    rgba(0xffd60a26)
                                })
                                .text_color(if daemon.is_available {
                                    rgb(MacTheme::SUCCESS)
                                } else {
                                    rgb(MacTheme::WARNING)
                                })
                                .child(if daemon.is_available {
                                    "ACTIVE"
                                } else {
                                    "FALLBACK"
                                }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child(format!("Daemon: {}", daemon.daemon_identifier)),
                );

            let content_scroll = div()
                .id("keychain-content-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_6()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Keys"),
                )
                .child(keys_grid)
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Identities"),
                )
                .child(identities_grid)
                .child(div().h(px(8.0)))
                .child(daemon_card);

            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .bg(rgb(MacTheme::CONTENT_BG))
                .child(action_bar)
                .child(content_scroll)
        } else {
            // Known Hosts view
            let action_bar = div()
                .id("known-hosts-action-bar")
                .flex()
                .items_center()
                .justify_between()
                .px_6()
                .py_3()
                .border_b_1()
                .border_color(rgb(MacTheme::BORDER))
                .child(
                    button_icon("import-known-hosts", "import", "Import").on_click(cx.listener(
                        |app, _, _, cx| {
                            app.refresh_public_inventory(cx);
                        },
                    )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("known-hosts-search-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.open_command_palette(window, cx)
                                }))
                                .child(
                                    if let Some(p) = crate::resolve_icon_path("search", false, 48) {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("🔍").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .id("known-hosts-grid-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("grid_view", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("⊞").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .id("known-hosts-refresh-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .on_click(
                                    cx.listener(|app, _, _, cx| app.refresh_public_inventory(cx)),
                                )
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("calendar_sort", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("📅").into_any_element()
                                    },
                                ),
                        ),
                );

            let mut hosts_grid = div().flex().flex_wrap().gap_3();
            for (idx, record) in self.desktop.public_records.iter().enumerate() {
                hosts_grid = hosts_grid.child(
                    div()
                        .id(("known-host-card", idx))
                        .min_w(px(250.0))
                        .flex_1()
                        .p_3p5()
                        .rounded_xl()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .flex()
                        .items_center()
                        .gap_3()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(36.0))
                                .rounded_lg()
                                .bg(rgb(MacTheme::BADGE_NAVY))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_base()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("fingerprint", false, 48)
                                    {
                                        div()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("🛡").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div().flex().flex_col().gap_0p5().min_w_0().child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .truncate()
                                    .child(record.name.clone()),
                            ),
                        ),
                );
            }
            if self.desktop.public_records.is_empty() {
                hosts_grid = hosts_grid.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("No known-host records found in ~/.ssh/known_hosts."),
                );
            }

            let content_scroll = div()
                .id("known-hosts-content-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_6()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Known Hosts"),
                )
                .child(hosts_grid);

            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .bg(rgb(MacTheme::CONTENT_BG))
                .child(action_bar)
                .child(content_scroll)
        }
    }
}

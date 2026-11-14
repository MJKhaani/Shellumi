//! SFTP host selection rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_sftp_connect_to_host(
        &self,
        is_left: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .p_8()
            .gap_4()
            .child(
                div()
                    .w(px(56.0))
                    .h(px(56.0))
                    .rounded_xl()
                    .bg(rgb(0x232634))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if let Some(p) = resolve_icon_path("sftp", false, 48) {
                        div()
                            .w(px(30.0))
                            .h(px(30.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().text_2xl().child("📁").into_any_element()
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Connect to host"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child(
                            "Start by connecting to a saved host to manage your files with SFTP.",
                        ),
                    ),
            )
            .child(
                div()
                    .id(if is_left {
                        "sftp-l-select-host-btn"
                    } else {
                        "sftp-r-select-host-btn"
                    })
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x2a2e3d))
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x353a4c)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        let pane = if is_left {
                            &mut app.sftp_left_pane
                        } else {
                            &mut app.sftp_right_pane
                        };
                        pane.is_selecting_host = true;
                        cx.notify();
                    }))
                    .child("Select host"),
            )
    }

    pub(crate) fn render_sftp_select_host(
        &self,
        is_left: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        // Header
        let header = div()
            .h(px(46.0))
            .px_4()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id(if is_left {
                                "sftp-l-back-btn"
                            } else {
                                "sftp-r-back-btn"
                            })
                            .cursor_pointer()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                let pane = if is_left {
                                    &mut app.sftp_left_pane
                                } else {
                                    &mut app.sftp_right_pane
                                };
                                pane.is_selecting_host = false;
                                cx.notify();
                            }))
                            .child("←"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Select Host"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(format!("{} ⌵", self.active_vault)),
                            ),
                    ),
            )
            .child(
                div()
                    .id(if is_left {
                        "sftp-left-local-btn"
                    } else {
                        "sftp-right-local-btn"
                    })
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .bg(rgb(MacTheme::ACCENT_BLUE))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_select_host(is_left, SftpPaneHost::Local, cx);
                    }))
                    .child(
                        if let Some(p) = resolve_icon_path("cloud_sync", false, 32) {
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().child("🖧").into_any_element()
                        },
                    )
                    .child("Local"),
            );

        // Search Bar with tag and calendar sort buttons
        let search_bar = div()
            .px_4()
            .py_2()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1p5()
                    .rounded_lg()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .child(if let Some(p) = resolve_icon_path("search", false, 32) {
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("🔍")
                            .into_any_element()
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("Search"),
                    ),
            )
            .child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(MacTheme::SURFACE))
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .cursor_pointer()
                    .child(if let Some(p) = resolve_icon_path("tags", false, 32) {
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().text_xs().child("🏷").into_any_element()
                    }),
            )
            .child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(MacTheme::SURFACE))
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .cursor_pointer()
                    .child(
                        if let Some(p) = resolve_icon_path("calendar_sort", false, 32) {
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().text_xs().child("📅").into_any_element()
                        },
                    ),
            );

        // Groups & Hosts Lists
        let mut list_container = div()
            .id(if is_left {
                "sftp-left-select-scroll"
            } else {
                "sftp-right-select-scroll"
            })
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_4()
            .py_2()
            .flex()
            .flex_col()
            .gap_3();

        // Groups Header & Items
        let groups = &self.desktop.groups;
        if !groups.is_empty() {
            list_container = list_container.child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Groups"),
            );

            let mut groups_grid = div().flex().flex_wrap().gap_2();
            for (g_idx, group) in groups.iter().enumerate() {
                let g_name = group.name.clone();
                let host_count = match &self.hosts {
                    HostState::Loaded(hosts) => hosts
                        .iter()
                        .filter(|h| h.group_name.as_deref() == Some(&g_name))
                        .count(),
                    _ => 0,
                };
                groups_grid = groups_grid.child(
                    div()
                        .id((
                            if is_left {
                                "sftp-left-group"
                            } else {
                                "sftp-right-group"
                            },
                            g_idx,
                        ))
                        .w(px(220.0))
                        .flex()
                        .items_center()
                        .gap_3()
                        .p_2p5()
                        .rounded_lg()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .child(
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(0x1a365d))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(if let Some(p) = resolve_icon_path("groups", false, 32) {
                                    div()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().text_sm().child("📁").into_any_element()
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(g_name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                        .child(format!("{host_count} Hosts")),
                                ),
                        ),
                );
            }
            list_container = list_container.child(groups_grid);
        }

        // Hosts Header & Items
        list_container = list_container.child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                .child("Hosts"),
        );

        let hosts = match &self.hosts {
            HostState::Loaded(h) => h.clone(),
            _ => Vec::new(),
        };

        if hosts.is_empty() {
            list_container = list_container.child(
                div()
                    .p_4()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child(
                        "No saved hosts found. Use '+ New host' in Hosts view to add one, or select 'Local' above.",
                    ),
            );
        } else {
            let mut hosts_grid = div().flex().flex_wrap().gap_2();
            for (h_idx, host) in hosts.into_iter().enumerate() {
                let h_clone = host.clone();
                let is_ssh = host.protocol.is_empty() || host.protocol == "ssh";
                let os_icon = resolve_host_os_icon(&host);
                hosts_grid = hosts_grid.child(
                    div()
                        .id((
                            if is_left {
                                "sftp-left-host"
                            } else {
                                "sftp-right-host"
                            },
                            h_idx,
                        ))
                        .w(px(220.0))
                        .flex()
                        .items_center()
                        .gap_3()
                        .p_2p5()
                        .rounded_lg()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.sftp_select_host(
                                is_left,
                                SftpPaneHost::Remote(Box::new(h_clone.clone())),
                                cx,
                            );
                        }))
                        .child(
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(if is_ssh { rgb(0x432818) } else { rgb(0x1e293b) })
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(if let Some(p) = resolve_icon_path(os_icon, false, 32) {
                                    div()
                                        .w(px(22.0))
                                        .h(px(22.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div()
                                        .text_sm()
                                        .text_color(if is_ssh {
                                            rgb(0xfb923c)
                                        } else {
                                            rgb(0x94a3b8)
                                        })
                                        .child("🖥")
                                        .into_any_element()
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(host.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                        .child(format!(
                                            "{}, {}",
                                            host.protocol,
                                            host.destination.username()
                                        )),
                                ),
                        ),
                );
            }
            list_container = list_container.child(hosts_grid);
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .child(search_bar)
            .child(list_container)
    }
}

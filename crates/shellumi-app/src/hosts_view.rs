//! Hosts inventory page and editor rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_hosts_view(
        &mut self,
        root: gpui::Stateful<gpui::Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let hosts = match &self.hosts {
            HostState::Loaded(hosts) => hosts.as_slice(),
            _ => &[],
        };
        let search_val = self.search_input.read(cx).value().trim().to_lowercase();
        let quick_val = self
            .quick_connect_input
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let query = if !quick_val.is_empty() {
            quick_val
        } else {
            search_val
        };

        let filtered: Vec<&SavedHost> = hosts
            .iter()
            .filter(|host| {
                group_matches(host.group_name.as_deref(), self.selected_group.as_deref())
                    && (query.is_empty()
                        || host.name.to_lowercase().contains(&query)
                        || host.destination.host().to_lowercase().contains(&query)
                        || host.destination.username().to_lowercase().contains(&query)
                        || host
                            .group_name
                            .as_ref()
                            .is_some_and(|group| group.to_lowercase().contains(&query))
                        || host
                            .tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&query)))
            })
            .collect();
        let selected = filtered
            .iter()
            .copied()
            .find(|host| Some(host.id) == self.selected_host_id);

        // Termius Sub-Action Bar
        let view_mode_menu = self.render_hosts_view_mode_menu(cx);

        let tags_filter_menu = self.render_hosts_tags_filter_menu(hosts, cx);

        let sort_menu = self.render_hosts_sort_menu(cx);

        let profile_menu = self.render_hosts_profile_menu(cx);

        let action_bar = div()
            .id("hosts-action-bar")
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
                        div()
                            .id("add-host")
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_1p5()
                            .rounded(px(MacTheme::CONTROL_RADIUS))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(MacTheme::SURFACE_ACTIVE))
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE)).text_color(rgb(0xffffff)))
                            .on_click(cx.listener(|app, _, _, cx| app.edit_host(None, cx)))
                            .child(if let Some(p) = resolve_icon_path("add_host", false, 48) {
                                div()
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .child(gpui::img(p).size_full())
                                    .into_any_element()
                            } else {
                                div().into_any_element()
                            })
                            .child("New host")
                            .child("⌵"),
                    )
                    .child(self.host_library_actions(cx)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("hosts-view-mode")
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_md()
                            .bg(if self.view_mode_menu_open {
                                rgb(MacTheme::SURFACE_HOVER)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.view_mode_menu_open = !app.view_mode_menu_open;
                                app.tag_filter_popover_open = false;
                                app.sort_menu_open = false;
                                app.profile_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                if let Some(p) = resolve_icon_path(
                                    if self.view_mode == HostViewMode::Grid {
                                        "grid_view"
                                    } else {
                                        "list_view"
                                    },
                                    self.view_mode_menu_open,
                                    48,
                                ) {
                                    div()
                                        .w(px(20.0))
                                        .h(px(20.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div()
                                        .child(if self.view_mode == HostViewMode::Grid {
                                            "▦"
                                        } else {
                                            "≡"
                                        })
                                        .into_any_element()
                                },
                            ),
                    )
                    .child(
                        div()
                            .id("hosts-tags-filter")
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_md()
                            .bg(if self.active_tag_filter.is_some() {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else if self.tag_filter_popover_open {
                                rgb(MacTheme::SURFACE_HOVER)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(if self.active_tag_filter.is_some() {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.tag_filter_popover_open = !app.tag_filter_popover_open;
                                app.view_mode_menu_open = false;
                                app.sort_menu_open = false;
                                app.profile_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                if let Some(p) = resolve_icon_path(
                                    "tag",
                                    self.active_tag_filter.is_some()
                                        || self.tag_filter_popover_open,
                                    48,
                                ) {
                                    div()
                                        .w(px(20.0))
                                        .h(px(20.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().child("🏷").into_any_element()
                                },
                            ),
                    )
                    .child(
                        div()
                            .id("hosts-sort")
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_md()
                            .bg(if self.sort_menu_open {
                                rgb(MacTheme::SURFACE_HOVER)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_menu_open = !app.sort_menu_open;
                                app.view_mode_menu_open = false;
                                app.tag_filter_popover_open = false;
                                app.profile_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                if let Some(p) =
                                    resolve_icon_path("calendar_sort", self.sort_menu_open, 48)
                                {
                                    div()
                                        .w(px(20.0))
                                        .h(px(20.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().child("📅").into_any_element()
                                },
                            ),
                    )
                    .child(
                        div()
                            .id("hosts-team-invite")
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_md()
                            .bg(if self.profile_menu_open {
                                rgb(MacTheme::SURFACE_HOVER)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.profile_menu_open = !app.profile_menu_open;
                                app.view_mode_menu_open = false;
                                app.tag_filter_popover_open = false;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                if let Some(p) =
                                    resolve_icon_path("profile", self.profile_menu_open, 48)
                                {
                                    div()
                                        .w(px(22.0))
                                        .h(px(22.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().child("👤+").into_any_element()
                                },
                            ),
                    ),
            );

        // Breadcrumbs if a group is currently filtered
        let breadcrumbs = if let Some(selected_grp) = &self.selected_group {
            let grp_clone = selected_grp.clone();
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .child(
                        div()
                            .id("breadcrumb-all-hosts")
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.selected_group = None;
                                cx.notify();
                            }))
                            .child("← All hosts"),
                    )
                    .child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child("/"))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(grp_clone),
                    ),
            )
        } else {
            None
        };

        // Top team invite banner
        let invite_banner = div()
            .id("team-invite-banner")
            .p_3()
            .rounded_lg()
            .bg(rgb(0x18202d))
            .border_1()
            .border_color(rgb(0x24344d))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .child(
                        if let Some(p) = resolve_icon_path("invite_member", false, 48) {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().text_base().child("👥").into_any_element()
                        },
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Invite members."),
                            )
                            .child(
                                div()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Manage and use data together with your team."),
                            ),
                    ),
            )
            .child(
                div()
                    .id("invite-members-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x252a36))
                    .border_1()
                    .border_color(rgb(0x384154))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x323a4b)).border_color(rgb(MacTheme::ACCENT_BLUE)))
                    .on_click(cx.listener(|app, _, _, cx| app.open_invite_modal(cx)))
                    .child("Invite members"),
            );

        let sidebar_width = if !self.sidebar_visible {
            0.0
        } else if self.sidebar_collapsed {
            56.0
        } else {
            MacTheme::SIDEBAR_WIDTH
        };
        let inspector_width = if self.group_inspector.is_some() {
            320.0
        } else {
            0.0
        } + if selected.is_some() { 290.0 } else { 0.0 };
        let grid_width =
            (window.viewport_size().width.to_f64() as f32 - sidebar_width - inspector_width - 48.0)
                .max(1.0);
        let card_width = responsive_card_width(grid_width, 280.0, 12.0);

        // Groups cards section (only shown when no group filter is selected and no search query)
        let mut groups: Vec<&str> = hosts
            .iter()
            .filter_map(|host| host.group_name.as_deref())
            .collect();
        groups.extend(self.desktop.groups.iter().map(|group| group.name.as_str()));
        groups.sort_unstable();
        groups.dedup();

        let groups_section = if self.selected_group.is_none()
            && !groups.is_empty()
            && query.is_empty()
        {
            let mut grid = div().flex().flex_wrap().gap_3();
            for (idx, group) in groups.iter().enumerate() {
                let grp_name = group.to_string();
                let grp_menu_name = group.to_string();
                let count = hosts
                    .iter()
                    .filter(|h| h.group_name.as_deref() == Some(*group))
                    .count();
                let card = div()
                    .id(("group-card", idx))
                    .w(px(card_width))
                    .h(px(58.0))
                    .rounded_lg()
                    .bg(rgb(MacTheme::SURFACE))
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .p_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                            .border_color(rgb(MacTheme::ACCENT_BLUE))
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.selected_group = Some(grp_name.clone());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .min_w_0()
                            .child(
                                div()
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .rounded_md()
                                    .bg(rgb(MacTheme::BADGE_NAVY))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                    .font_weight(FontWeight::BOLD)
                                    .text_sm()
                                    .child(
                                        if let Some(p) = resolve_icon_path("groups", false, 48) {
                                            div()
                                                .w(px(32.0))
                                                .h(px(32.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("⊞").into_any_element()
                                        },
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .truncate()
                                            .child(group.to_string()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child(format!("{count} hosts")),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id(("group-menu-btn", idx))
                            .px_1p5()
                            .py_0p5()
                            .rounded_md()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .hover(|s| {
                                s.text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .bg(rgb(MacTheme::INPUT_BG))
                            })
                            .on_click(cx.listener(
                                move |app, event: &gpui::ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    let position = event.position();
                                    let size = window.viewport_size();
                                    let x = (position.x.to_f64() as f32)
                                        .clamp(0.0, (size.width.to_f64() as f32 - 210.0).max(0.0));
                                    let y = (position.y.to_f64() as f32)
                                        .clamp(0.0, (size.height.to_f64() as f32 - 260.0).max(0.0));
                                    app.group_context_menu = Some((grp_menu_name.clone(), x, y));
                                    cx.notify();
                                },
                            ))
                            .child(if let Some(p) = resolve_icon_path("more", false, 48) {
                                div()
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .child(gpui::img(p).size_full())
                                    .into_any_element()
                            } else {
                                div().child("···").into_any_element()
                            }),
                    );
                grid = grid.child(card);
            }
            Some(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Groups"),
                    )
                    .child(grid),
            )
        } else {
            None
        };

        // Filtering and sorting
        let mut filtered: Vec<&SavedHost> = hosts
            .iter()
            .filter(|h| {
                if let Some(grp) = &self.selected_group {
                    if h.group_name.as_deref() != Some(grp.as_str()) {
                        return false;
                    }
                }
                if let Some(tag) = &self.active_tag_filter {
                    if !h.tags.contains(tag) {
                        return false;
                    }
                }
                if query.is_empty() {
                    return true;
                }
                let query = query.to_lowercase();
                h.name.to_lowercase().contains(&query)
                    || h.destination.host().to_lowercase().contains(&query)
                    || h.destination.username().to_lowercase().contains(&query)
                    || h.tags.iter().any(|t| t.to_lowercase().contains(&query))
                    || h.group_name
                        .as_ref()
                        .map(|g| g.to_lowercase().contains(&query))
                        .unwrap_or(false)
            })
            .collect();

        match self.sort_mode {
            HostSortMode::AlphaAsc => filtered.sort_by_key(|a| a.name.to_lowercase()),
            HostSortMode::AlphaDesc => {
                filtered.sort_by_key(|a| std::cmp::Reverse(a.name.to_lowercase()))
            }
            HostSortMode::Newest => filtered.sort_by_key(|a| std::cmp::Reverse(a.id)),
            HostSortMode::Oldest => filtered.sort_by_key(|a| a.id),
        }

        // Hosts cards section
        let mut hosts_grid = div().flex().flex_wrap().gap_3();
        for host in &filtered {
            let id = host.id;
            let is_sel = selected.is_some_and(|s| s.id == id);
            let select_host_id = id;

            let badge_bg = match host.protocol() {
                ConnectionProtocol::Ssh => rgb(0x0f3661),
                ConnectionProtocol::Serial => rgb(0x0e3b2e),
                ConnectionProtocol::Mosh => rgb(0x3d2800),
                ConnectionProtocol::Telnet => rgb(0x3e1717),
                ConnectionProtocol::Local => rgb(0x1a2e26),
                ConnectionProtocol::Vnc => rgb(0x3a1b5c),
                ConnectionProtocol::Rdp => rgb(0x103d3d),
            };
            let badge_fg = match host.protocol() {
                ConnectionProtocol::Ssh => rgb(0x389bf2),
                ConnectionProtocol::Serial => rgb(0x3dd68c),
                ConnectionProtocol::Mosh => rgb(0xf5a623),
                ConnectionProtocol::Telnet => rgb(0xeb5757),
                ConnectionProtocol::Local => rgb(0x3dd68c),
                ConnectionProtocol::Vnc => rgb(0xbb86fc),
                ConnectionProtocol::Rdp => rgb(0x3de0d8),
            };
            let badge_icon = match host.protocol() {
                ConnectionProtocol::Ssh => "🖧",
                ConnectionProtocol::Serial => "🔌",
                ConnectionProtocol::Mosh => "⚡",
                ConnectionProtocol::Telnet => "⚠️",
                ConnectionProtocol::Local => ">_",
                ConnectionProtocol::Vnc => "🖥️",
                ConnectionProtocol::Rdp => "🪟",
            };

            let card = div()
                .id(("host-card", id as u64))
                .w(px(card_width))
                .h(px(64.0))
                .rounded_lg()
                .bg(if is_sel {
                    rgb(MacTheme::SURFACE_ACTIVE)
                } else {
                    rgb(MacTheme::SURFACE)
                })
                .border_1()
                .border_color(if is_sel {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::BORDER)
                })
                .p_3()
                .flex()
                .items_center()
                .gap_3()
                .cursor_pointer()
                .hover(|s| {
                    if !is_sel {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                            .border_color(rgb(MacTheme::ACCENT_BLUE))
                    } else {
                        s
                    }
                })
                .on_click(cx.listener(move |app, _, _, cx| {
                    if app.selected_host_id == Some(select_host_id) {
                        app.selected_host_id = None;
                    } else {
                        app.selected_host_id = Some(select_host_id);
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .w(px(38.0))
                        .h(px(38.0))
                        .rounded_md()
                        .bg(badge_bg)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(badge_fg)
                        .text_base()
                        .child(
                            if let Some(p) =
                                resolve_icon_path(resolve_host_os_icon(host), false, 48)
                            {
                                div()
                                    .w(px(38.0))
                                    .h(px(38.0))
                                    .child(gpui::img(p).size_full())
                                    .into_any_element()
                            } else {
                                div().child(badge_icon).into_any_element()
                            },
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .truncate()
                                .child(host.name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .truncate()
                                .child(format!(
                                    "{}, {}",
                                    host.protocol().display_name().to_lowercase(),
                                    if host.destination.username().is_empty() {
                                        "default"
                                    } else {
                                        host.destination.username()
                                    }
                                )),
                        ),
                );
            hosts_grid = hosts_grid.child(card);
        }

        if filtered.is_empty() {
            let message = match &self.hosts {
                HostState::Loading => "Loading hosts…".to_string(),
                HostState::Error(error) => format!("Could not load hosts: {error}"),
                HostState::NoDatabase => "Host storage is unavailable.".to_string(),
                HostState::Loaded(hosts) if hosts.is_empty() => {
                    "No saved hosts yet. Click '+ New host' above to add your first connection."
                        .to_string()
                }
                _ => "No hosts match your search.".to_string(),
            };
            hosts_grid = hosts_grid.child(
                div()
                    .p_8()
                    .w_full()
                    .text_center()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(message),
            );
        }

        let hosts_display = match self.view_mode {
            HostViewMode::Grid => div().child(hosts_grid),
            HostViewMode::List => {
                let mut list_col = div()
                    .id("hosts-list-table")
                    .w_full()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .rounded_xl()
                    .overflow_hidden()
                    .bg(rgb(MacTheme::SURFACE));

                list_col = list_col.child(
                    div()
                        .flex()
                        .items_center()
                        .px_4()
                        .py_2()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .bg(rgb(MacTheme::INPUT_BG))
                        .border_b_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .child(div().w(px(220.0)).child("NAME"))
                        .child(div().w(px(140.0)).child("GROUP"))
                        .child(div().w(px(90.0)).child("PROTOCOL"))
                        .child(div().flex_1().child("DESTINATION / USER"))
                        .child(div().w(px(160.0)).child("TAGS")),
                );

                for (l_idx, host) in filtered.iter().enumerate() {
                    let id = host.id;
                    let is_sel = selected.is_some_and(|s| s.id == id);
                    let badge_fg = match host.protocol() {
                        ConnectionProtocol::Ssh => rgb(0x389bf2),
                        ConnectionProtocol::Serial => rgb(0x3dd68c),
                        ConnectionProtocol::Mosh => rgb(0xf5a623),
                        ConnectionProtocol::Telnet => rgb(0xeb5757),
                        ConnectionProtocol::Local => rgb(0x3dd68c),
                        ConnectionProtocol::Vnc => rgb(0xbb86fc),
                        ConnectionProtocol::Rdp => rgb(0x3de0d8),
                    };
                    let badge_icon = match host.protocol() {
                        ConnectionProtocol::Ssh => "🖧",
                        ConnectionProtocol::Serial => "🔌",
                        ConnectionProtocol::Mosh => "⚡",
                        ConnectionProtocol::Telnet => "⚠️",
                        ConnectionProtocol::Local => ">_",
                        ConnectionProtocol::Vnc => "🖥️",
                        ConnectionProtocol::Rdp => "🪟",
                    };

                    list_col = list_col.child(
                        div()
                            .id(("host-list-row", l_idx))
                            .flex()
                            .items_center()
                            .px_4()
                            .py_2p5()
                            .border_b_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .bg(if is_sel {
                                rgb(MacTheme::SURFACE_ACTIVE)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                if app.selected_host_id == Some(id) {
                                    app.selected_host_id = None;
                                } else {
                                    app.selected_host_id = Some(id);
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(220.0))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) =
                                            resolve_icon_path(resolve_host_os_icon(host), false, 48)
                                        {
                                            div()
                                                .w(px(22.0))
                                                .h(px(22.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div()
                                                .text_sm()
                                                .text_color(badge_fg)
                                                .child(badge_icon)
                                                .into_any_element()
                                        },
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .truncate()
                                            .child(host.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(140.0))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .truncate()
                                    .child(host.group_name.clone().unwrap_or_else(|| "—".into())),
                            )
                            .child(
                                div()
                                    .w(px(90.0))
                                    .text_xs()
                                    .text_color(badge_fg)
                                    .child(host.protocol().display_name()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .truncate()
                                    .child(format!(
                                        "{}@{}",
                                        if host.destination.username().is_empty() {
                                            "default"
                                        } else {
                                            host.destination.username()
                                        },
                                        host.destination.host()
                                    )),
                            )
                            .child(div().w(px(160.0)).flex().flex_wrap().gap_1().children(
                                host.tags.iter().map(|t| {
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(MacTheme::INPUT_BG))
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(t.clone())
                                }),
                            )),
                    );
                }

                if filtered.is_empty() {
                    list_col = list_col.child(
                        div()
                            .p_8()
                            .w_full()
                            .text_center()
                            .text_sm()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("No hosts match your filter criteria."),
                    );
                }

                div().child(list_col)
            }
        };

        // Slide-over Inspector Drawer (right side)
        let inspector_panel = if let Some(host) = selected {
            let edit_host = host.clone();
            let connect_host = host.clone();
            let sftp_host = host.clone();
            let id = host.id;
            Some(
                div()
                    .id("host-inspector-panel")
                    .w(px(290.0))
                    .flex_shrink_0()
                    .h_full()
                    .border_l_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .bg(rgb(MacTheme::SURFACE))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .truncate()
                                    .child(host.name.clone()),
                            )
                            .child(
                                div()
                                    .id("close-inspector")
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.selected_host_id = None;
                                        cx.notify();
                                    }))
                                    .child("✕"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("💬")
                            .child("Personal")
                            .child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child("⌵")),
                    )
                    .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                    .child(host_detail(
                        "Protocol",
                        host.protocol().display_name().to_string(),
                    ))
                    .child(host_detail(
                        "Address",
                        format_endpoint(
                            host.destination.username(),
                            host.destination.host(),
                            host.destination.port().get(),
                        ),
                    ))
                    .child(host_detail(
                        "Group",
                        host.group_name.clone().unwrap_or_else(|| "—".into()),
                    ))
                    .child(host_detail(
                        "Tags",
                        if host.tags.is_empty() {
                            "—".into()
                        } else {
                            host.tags.join(", ")
                        },
                    ))
                    .child(div().flex_1())
                    .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                    .child(
                        div()
                            .id("connect-selected")
                            .w_full()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x389bf2)))
                            .on_click(cx.listener(move |app, _, window, cx| {
                                app.connect(&connect_host, window, cx)
                            }))
                            .child(if host.protocol().is_graphical() {
                                "Connect Desktop"
                            } else {
                                "Connect"
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .id("edit-selected")
                                    .flex_1()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::INPUT_BG))
                                    .border_1()
                                    .border_color(rgb(MacTheme::BORDER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .text_xs()
                                    .text_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.edit_host(Some(&edit_host), cx)
                                    }))
                                    .child("Edit"),
                            )
                            .children(if host.protocol().supports_sftp() {
                                Some(
                                    div()
                                        .id("sftp-selected")
                                        .flex_1()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(rgb(MacTheme::INPUT_BG))
                                        .border_1()
                                        .border_color(rgb(MacTheme::BORDER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .text_xs()
                                        .text_center()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                        .on_click(cx.listener(move |app, _, window, cx| {
                                            app.connect_sftp(&sftp_host, window, cx)
                                        }))
                                        .child("SFTP"),
                                )
                            } else {
                                None
                            }),
                    )
                    .child(
                        div()
                            .id("delete-selected")
                            .py_1()
                            .text_xs()
                            .text_center()
                            .text_color(rgb(MacTheme::DANGER))
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(cx.listener(move |app, _, _, cx| app.delete_host(id, cx)))
                            .child("Delete host"),
                    ),
            )
        } else {
            None
        };

        let group_inspector_panel = if self.group_inspector.is_some() {
            Some(self.render_group_details_inspector(cx))
        } else {
            None
        };

        let content_scroll = div()
            .id("hosts-content-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(invite_banner)
            .children(breadcrumbs)
            .children(groups_section)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Hosts"),
                    )
                    .child(hosts_display),
            );

        let main_hosts_area = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .child(content_scroll)
            .children(group_inspector_panel)
            .children(inspector_panel);

        let mut body = div()
            .id("shellumi")
            .relative()
            .track_focus(&self.focus)
            .on_action(cx.listener(|app, _: &RefreshHosts, _, cx| app.load_hosts(cx)))
            .on_action(cx.listener(|app, _: &ToggleCommandPalette, window, cx| {
                app.toggle_command_palette(window, cx)
            }))
            .on_key_down(cx.listener(Self::handle_palette_key_down))
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::CONTENT_BG))
            .child(action_bar)
            .child(main_hosts_area);

        if self.view_mode_menu_open
            || self.tag_filter_popover_open
            || self.sort_menu_open
            || self.profile_menu_open
        {
            let backdrop = div()
                .id("hosts-popover-backdrop")
                .absolute()
                .inset_0()
                .on_click(cx.listener(|app, _, _, cx| {
                    app.view_mode_menu_open = false;
                    app.tag_filter_popover_open = false;
                    app.sort_menu_open = false;
                    app.profile_menu_open = false;
                    cx.notify();
                }));
            body = body.child(backdrop);
            if let Some(menu) = view_mode_menu {
                body = body.child(menu);
            }
            if let Some(menu) = tags_filter_menu {
                body = body.child(menu);
            }
            if let Some(menu) = sort_menu {
                body = body.child(menu);
            }
            if let Some(menu) = profile_menu {
                body = body.child(menu);
            }
        }

        let sidebar = self.workspace_sidebar("Hosts", cx);
        let mut row = div().flex().flex_1().min_h_0();
        if self.sidebar_visible {
            row = row.child(sidebar);
        }
        let mut result = root.child(row.child(body));
        result = self.render_host_editor(result, window, cx);
        if let Some(modal) = &self.conflict_modal {
            result = result.child(self.render_conflict_modal(modal, cx));
        }
        if let Some(modal) = &self.snippet_modal {
            result = result.child(self.render_snippet_modal(modal, cx));
        }
        if let Some(modal) = &self.batch_modal {
            result = result.child(self.render_batch_modal(modal, cx));
        }
        if let Some(palette) = &self.command_palette {
            result = result.child(self.render_command_palette(palette, cx));
        }
        if let Some(modal) = &self.team_modal {
            result = result.child(self.render_team_modal(modal, cx));
        }
        if let Some(modal) = &self.infra_modal {
            result = result.child(self.render_infra_modal(modal, cx));
        }
        if let Some(modal) = &self.diagnostics_modal {
            result = result.child(self.render_diagnostics_modal(modal, cx));
        }
        if let Some(modal) = &self.mobile_modal {
            result = result.child(self.render_mobile_modal(modal, cx));
        }
        if let Some(modal) = &self.shell_integration_modal {
            result = result.child(self.render_shell_integration_modal(modal, cx));
        }
        if let Some(modal) = &self.session_replay_modal {
            result = result.child(self.render_session_replay_modal(modal, cx));
        }
        if let Some(modal) = &self.tunnel_modal {
            result = result.child(self.render_tunnel_modal(modal, cx));
        }
        if let Some(modal) = &self.create_vault_modal {
            result = result.child(self.render_create_vault_modal(modal, cx));
        }
        if let Some(dropdown) = self.render_vault_dropdown_overlay(cx) {
            result = result.child(dropdown);
        }
        if let Some(modal) = &self.invite_modal {
            result = result.child(self.render_invite_team_modal(modal, cx));
        }
        if let Some(modal) = &self.identity_modal {
            result = result.child(self.render_identity_modal(modal, cx));
        }
        if let Some(ctx_menu) = self.render_group_context_menu(cx) {
            result = result.child(ctx_menu);
        }
        if self.desktop.editor.is_some() || self.desktop.confirm.is_some() {
            result = self.desktop_overlays(result, cx);
        }
        if let Some(message) = &self.desktop.message {
            result = result.child(
                div()
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .p_3()
                    .bg(rgb(0x141e2c))
                    .text_sm()
                    .child(message.clone()),
            );
        }
        result
    }
}

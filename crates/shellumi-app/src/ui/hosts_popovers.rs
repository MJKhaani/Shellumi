//! Hosts inventory popovers.
use super::*;

impl Shellumi {
    pub(crate) fn render_hosts_view_mode_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        if self.view_mode_menu_open {
            Some(
                div()
                    .id("view-mode-popover")
                    .absolute()
                    .top(px(48.0))
                    .right(px(120.0))
                    .w(px(120.0))
                    .rounded_lg()
                    .bg(rgb(0x181a24))
                    .border_1()
                    .border_color(rgb(0x2d3245))
                    .p_1()
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .id("view-opt-grid")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.view_mode == HostViewMode::Grid {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.view_mode = HostViewMode::Grid;
                                app.view_mode_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) = resolve_icon_path(
                                            "grid_view",
                                            self.view_mode == HostViewMode::Grid,
                                            32,
                                        ) {
                                            div()
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("▦").into_any_element()
                                        },
                                    )
                                    .child("Grid"),
                            )
                            .child(if self.view_mode == HostViewMode::Grid {
                                "✓"
                            } else {
                                ""
                            }),
                    )
                    .child(
                        div()
                            .id("view-opt-list")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.view_mode == HostViewMode::List {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.view_mode = HostViewMode::List;
                                app.view_mode_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) = resolve_icon_path(
                                            "list_view",
                                            self.view_mode == HostViewMode::List,
                                            32,
                                        ) {
                                            div()
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("≡").into_any_element()
                                        },
                                    )
                                    .child("List"),
                            )
                            .child(if self.view_mode == HostViewMode::List {
                                "✓"
                            } else {
                                ""
                            }),
                    ),
            )
        } else {
            None
        }
    }
    pub(crate) fn render_hosts_tags_filter_menu(
        &self,
        hosts: &[SavedHost],
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        if self.tag_filter_popover_open {
            let mut all_tags: Vec<String> = hosts.iter().flat_map(|h| h.tags.clone()).collect();
            all_tags.sort();
            all_tags.dedup();

            let tag_content = if all_tags.is_empty() {
                div()
                    .py_4()
                    .px_3()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_center()
                    .child(
                        if let Some(p) = resolve_icon_path("tags", false, 48) {
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().text_xl().child("🏷").into_any_element()
                        },
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Add tags"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Tags help you filter your hosts. You can add a tag when editing a host."),
                    )
            } else {
                let mut tags_list = div().flex().flex_wrap().gap_1p5();
                for (t_idx, tag) in all_tags.into_iter().enumerate() {
                    let tag_name = tag.clone();
                    let is_active = self.active_tag_filter.as_deref() == Some(&tag);
                    tags_list = tags_list.child(
                        div()
                            .id(("tag-filter-opt", t_idx))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .bg(if is_active {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x252a36)
                            })
                            .text_color(if is_active {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                if app.active_tag_filter.as_deref() == Some(&tag_name) {
                                    app.active_tag_filter = None;
                                } else {
                                    app.active_tag_filter = Some(tag_name.clone());
                                }
                                app.tag_filter_popover_open = false;
                                cx.notify();
                            }))
                            .child(tag),
                    );
                }

                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(if let Some(p) = resolve_icon_path("tags", false, 32) {
                                        div()
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("🏷").into_any_element()
                                    })
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child("Filter by tags"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("clear-tag-filter-btn")
                                    .text_xs()
                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                    .cursor_pointer()
                                    .hover(|s| s.underline())
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.active_tag_filter = None;
                                        app.tag_filter_popover_open = false;
                                        cx.notify();
                                    }))
                                    .child("Clear"),
                            ),
                    )
                    .child(tags_list)
            };

            Some(
                div()
                    .id("tags-filter-popover")
                    .absolute()
                    .top(px(48.0))
                    .right(px(64.0))
                    .w(px(220.0))
                    .rounded_lg()
                    .bg(rgb(0x181a24))
                    .border_1()
                    .border_color(rgb(0x2d3245))
                    .p_3()
                    .shadow_2xl()
                    .child(tag_content),
            )
        } else {
            None
        }
    }
    pub(crate) fn render_hosts_sort_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        if self.sort_menu_open {
            Some(
                div()
                    .id("sort-popover")
                    .absolute()
                    .top(px(48.0))
                    .right(px(36.0))
                    .w(px(170.0))
                    .rounded_lg()
                    .bg(rgb(0x181a24))
                    .border_1()
                    .border_color(rgb(0x2d3245))
                    .p_1()
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .id("sort-opt-alpha-asc")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.sort_mode == HostSortMode::AlphaAsc {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_mode = HostSortMode::AlphaAsc;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(div().flex().items_center().gap_2().child("Az").child("A-z"))
                            .child(if self.sort_mode == HostSortMode::AlphaAsc {
                                "✓"
                            } else {
                                ""
                            }),
                    )
                    .child(
                        div()
                            .id("sort-opt-alpha-desc")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.sort_mode == HostSortMode::AlphaDesc {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_mode = HostSortMode::AlphaDesc;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(div().flex().items_center().gap_2().child("Za").child("Z-a"))
                            .child(if self.sort_mode == HostSortMode::AlphaDesc {
                                "✓"
                            } else {
                                ""
                            }),
                    )
                    .child(
                        div()
                            .id("sort-opt-newest")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.sort_mode == HostSortMode::Newest {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_mode = HostSortMode::Newest;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) = resolve_icon_path(
                                            "calendar_sort",
                                            self.sort_mode == HostSortMode::Newest,
                                            32,
                                        ) {
                                            div()
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("📅").into_any_element()
                                        },
                                    )
                                    .child("Newest to oldest"),
                            )
                            .child(if self.sort_mode == HostSortMode::Newest {
                                "✓"
                            } else {
                                ""
                            }),
                    )
                    .child(
                        div()
                            .id("sort-opt-oldest")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.sort_mode == HostSortMode::Oldest {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_mode = HostSortMode::Oldest;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) = resolve_icon_path(
                                            "calendar_sort",
                                            self.sort_mode == HostSortMode::Oldest,
                                            32,
                                        ) {
                                            div()
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("📅").into_any_element()
                                        },
                                    )
                                    .child("Oldest to newest"),
                            )
                            .child(if self.sort_mode == HostSortMode::Oldest {
                                "✓"
                            } else {
                                ""
                            }),
                    ),
            )
        } else {
            None
        }
    }
    pub(crate) fn render_hosts_profile_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        if self.profile_menu_open {
            Some(
                div()
                    .id("profile-popover")
                    .absolute()
                    .top(px(48.0))
                    .right(px(24.0))
                    .w(px(220.0))
                    .rounded_xl()
                    .bg(rgb(0x181a24))
                    .border_1()
                    .border_color(rgb(0x2d3245))
                    .p_1p5()
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .id("profile-opt-invite-members")
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .gap_2p5()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.profile_menu_open = false;
                                app.open_invite_modal(cx);
                            }))
                            .child(
                                if let Some(p) = resolve_icon_path("invite_member", false, 48) {
                                    div()
                                        .w(px(20.0))
                                        .h(px(20.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().child("👤+").into_any_element()
                                },
                            )
                            .child("Invite team members"),
                    )
                    .child(
                        div()
                            .id("profile-opt-user-info")
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .flex()
                            .items_center()
                            .gap_2p5()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .child(if let Some(p) = resolve_icon_path("profile", false, 48) {
                                div()
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .rounded_full()
                                    .overflow_hidden()
                                    .child(gpui::img(p).size_full())
                                    .into_any_element()
                            } else {
                                div().child("👤").into_any_element()
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .truncate()
                                    .child("javaj2se.5@gmail.com"),
                            ),
                    ),
            )
        } else {
            None
        }
    }
}

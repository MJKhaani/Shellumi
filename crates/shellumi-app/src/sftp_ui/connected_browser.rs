//! SFTP connected browser rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_sftp_connected_browser(
        &self,
        is_left: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let pane = if is_left {
            &self.sftp_left_pane
        } else {
            &self.sftp_right_pane
        };

        let (badge_bg, badge_fg, host_title) = match &pane.host {
            Some(SftpPaneHost::Local) => (rgb(0x1a365d), rgb(0x60a5fa), "127.0.0.1".to_string()),
            Some(SftpPaneHost::Remote(h)) => (rgb(0x432818), rgb(0xfb923c), h.name.clone()),
            None => (rgb(0x222630), rgb(0x94a3b8), "Disconnected".to_string()),
        };

        // Header
        let actions_open = pane.actions_open;
        let header = div()
            .h(px(46.0))
            .px_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .id(if is_left {
                        "sftp-left-host-pill"
                    } else {
                        "sftp-right-host-pill"
                    })
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(badge_bg)
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(badge_fg)
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        let p = if is_left {
                            &mut app.sftp_left_pane
                        } else {
                            &mut app.sftp_right_pane
                        };
                        p.is_selecting_host = true;
                        cx.notify();
                    }))
                    .child(if let Some(p) = resolve_icon_path("server", false, 32) {
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().child("🖧").into_any_element()
                    })
                    .child(host_title),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id(if is_left {
                                "sftp-left-filter-btn"
                            } else {
                                "sftp-right-filter-btn"
                            })
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .text_color(if pane.filter_active {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                let p = if is_left {
                                    &mut app.sftp_left_pane
                                } else {
                                    &mut app.sftp_right_pane
                                };
                                p.filter_active = !p.filter_active;
                                cx.notify();
                            }))
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                if let Some(p) = resolve_icon_path("search", pane.filter_active, 32)
                                {
                                    div()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().child("🔍").into_any_element()
                                },
                            )
                            .child("Filter"),
                    )
                    .child(
                        div()
                            .id(if is_left {
                                "sftp-left-actions-btn"
                            } else {
                                "sftp-right-actions-btn"
                            })
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .bg(if actions_open {
                                rgb(MacTheme::SURFACE_ACTIVE)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_toggle_actions(is_left, cx);
                            }))
                            .child(if actions_open {
                                "Actions ⌃"
                            } else {
                                "Actions ⌵"
                            }),
                    ),
            );

        // Path Breadcrumbs (macOS style with interactable segments)
        let current_path = pane.current_path.clone();
        let crumbs: Vec<&str> = current_path.split('/').filter(|s| !s.is_empty()).collect();
        let can_back = !pane.history_back.is_empty();
        let can_fwd = !pane.history_forward.is_empty();

        let mut breadcrumb_row = div()
            .h(px(34.0))
            .px_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .flex()
            .items_center()
            .gap_1p5()
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_SECONDARY))
            .child(
                div()
                    .id(if is_left {
                        "sftp-l-crumb-back"
                    } else {
                        "sftp-r-crumb-back"
                    })
                    .cursor_pointer()
                    .text_color(if can_back {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_history_back(is_left, cx);
                    }))
                    .child("<"),
            )
            .child(
                div()
                    .id(if is_left {
                        "sftp-l-crumb-fwd"
                    } else {
                        "sftp-r-crumb-fwd"
                    })
                    .cursor_pointer()
                    .text_color(if can_fwd {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_history_forward(is_left, cx);
                    }))
                    .child(">"),
            );

        let mut acc_path = String::new();
        for (c_idx, crumb) in crumbs.iter().enumerate() {
            acc_path.push('/');
            acc_path.push_str(crumb);
            let target_path = acc_path.clone();
            breadcrumb_row = breadcrumb_row.child(
                div()
                    .id((
                        if is_left {
                            "sftp-left-crumb"
                        } else {
                            "sftp-right-crumb"
                        },
                        c_idx,
                    ))
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .text_color(if c_idx == crumbs.len() - 1 {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::ACCENT_BLUE)
                    })
                    .hover(|s| s.underline())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_dir(is_left, target_path.clone(), cx);
                    }))
                    .child(if let Some(p) = resolve_folder_icon_path(crumb) {
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().text_xs().child("📁").into_any_element()
                    })
                    .child(crumb.to_string()),
            );
            if c_idx < crumbs.len() - 1 {
                breadcrumb_row =
                    breadcrumb_row.child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child(">"));
            }
        }

        // Table Header
        let table_header = div()
            .h(px(28.0))
            .px_3()
            .bg(rgb(MacTheme::SURFACE))
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .flex()
            .items_center()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(MacTheme::TEXT_TERTIARY))
            .child(div().flex_1().child("Name"))
            .child(div().w(px(140.0)).child("Date Modified"))
            .child(div().w(px(90.0)).child("Size"))
            .child(div().w(px(70.0)).child("Kind"));

        // Table Rows with Drop handling and right-click backdrop
        let mut table_body = div()
            .id(if is_left {
                "sftp-left-table-scroll"
            } else {
                "sftp-right-table-scroll"
            })
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .on_drop(cx.listener(move |app, payload: &SftpDragPayload, _, cx| {
                if payload.from_left != is_left {
                    app.sftp_transfer_files(payload.from_left, payload.files.clone(), cx);
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |app, _, _, cx| {
                    let p = if is_left {
                        &mut app.sftp_left_pane
                    } else {
                        &mut app.sftp_right_pane
                    };
                    p.selected_files.clear();
                    p.selected_file = None;
                    app.sftp_context_menu = None;
                    cx.notify();
                }),
            )
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(move |app, event: &gpui::MouseDownEvent, _, cx| {
                    if app.sftp_context_menu.is_none() {
                        let cursor_x = event.position.x.to_f64() as f32;
                        let cursor_y = event.position.y.to_f64() as f32;
                        app.sftp_open_context_menu(
                            is_left,
                            String::new(),
                            String::new(),
                            true,
                            0,
                            cursor_x,
                            cursor_y,
                            cx,
                        );
                        cx.notify();
                    }
                }),
            );

        // Loading state
        if pane.loading {
            table_body = table_body.child(
                div()
                    .h(px(36.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("⏳ Loading directory contents…"),
            );
        }

        // Error banner with retry
        if let Some(err) = &pane.error {
            table_body = table_body.child(
                div()
                    .px_3()
                    .py_2()
                    .bg(rgb(0x3b1114))
                    .border_b_1()
                    .border_color(rgb(0x7f1d1d))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0xfca5a5))
                            .child(format!("SFTP error: {err}")),
                    )
                    .child(
                        div()
                            .id(if is_left {
                                "sftp-l-retry-load"
                            } else {
                                "sftp-r-retry-load"
                            })
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(MacTheme::SURFACE_ACTIVE))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.refresh_sftp_pane(is_left, cx);
                            }))
                            .child("Retry"),
                    ),
            );
        }

        // ".." Row
        let default_folder_icon = resolve_file_icon_path(&FileEntry {
            name: "folder".into(),
            path: String::new(),
            size: 0,
            kind: FileKind::Directory,
            modified: None,
            permissions: None,
        });
        table_body = table_body.child(
            div()
                .id(if is_left {
                    "sftp-left-parent-row"
                } else {
                    "sftp-right-parent-row"
                })
                .h(px(34.0))
                .px_3()
                .flex()
                .items_center()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.sftp_navigate_parent(is_left, cx);
                }))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(if let Some(ref p) = default_folder_icon {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .child(gpui::img(p.clone()).size_full())
                                .into_any_element()
                        } else {
                            div().text_xs().child("📁").into_any_element()
                        })
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child(".."),
                        ),
                )
                .child(div().w(px(140.0)))
                .child(
                    div()
                        .w(px(90.0))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("--"),
                )
                .child(
                    div()
                        .w(px(70.0))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("folder"),
                ),
        );

        let selected_file = pane.selected_file.clone();
        let ctx_entry = self
            .sftp_context_menu
            .as_ref()
            .filter(|c| c.is_left == is_left)
            .map(|c| c.entry_name.clone());

        for (entry_idx, entry) in pane.entries.iter().enumerate() {
            let is_dir = entry.kind == FileKind::Directory;
            let entry_name = entry.name.clone();
            let entry_path = entry.path.clone();
            let ext_str = std::path::Path::new(&entry_name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());
            let kind_label = if is_dir {
                "folder".to_string()
            } else {
                ext_str.unwrap_or_else(|| "file".to_string())
            };
            let formatted_size = if is_dir {
                "--".to_string()
            } else {
                entry.formatted_size()
            };
            let formatted_permissions = entry.formatted_permissions();
            let formatted_date = format_file_timestamp(entry.modified);

            let is_selected = pane.selected_files.contains(&entry.name)
                || selected_file.as_deref() == Some(&entry.name);
            let is_context_active = ctx_entry.as_deref() == Some(&entry.name);

            let drag_files: Vec<String> =
                if pane.selected_files.contains(&entry.name) && pane.selected_files.len() > 1 {
                    pane.selected_files.iter().cloned().collect()
                } else {
                    vec![entry.name.clone()]
                };
            let drag_label = if drag_files.len() == 1 {
                entry.name.clone()
            } else {
                format!("{} files", drag_files.len())
            };

            let file_icon_path = resolve_file_icon_path(entry);
            let icon_el = if let Some(ref icon_p) = file_icon_path {
                div()
                    .w(px(22.0))
                    .h(px(22.0))
                    .child(gpui::img(icon_p.clone()).size_full())
                    .into_any_element()
            } else {
                div()
                    .text_xs()
                    .text_color(if is_dir {
                        rgb(0x60a5fa)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .child(if is_dir { "📁" } else { "📄" })
                    .into_any_element()
            };

            let mut row = div()
                .id((
                    if is_left {
                        "sftp-left-entry"
                    } else {
                        "sftp-right-entry"
                    },
                    entry_idx,
                ))
                .h(px(40.0))
                .px_3()
                .flex()
                .items_center()
                .cursor_pointer()
                .bg(if is_context_active {
                    rgb(MacTheme::ACCENT_BLUE)
                } else if is_selected {
                    rgb(MacTheme::SURFACE_ACTIVE)
                } else {
                    rgba(0x00000000)
                })
                .hover(|s| {
                    if !is_selected && !is_context_active {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                    } else {
                        s
                    }
                })
                .on_drag(
                    SftpDragPayload {
                        from_left: is_left,
                        files: drag_files,
                    },
                    move |_, _, _, cx| {
                        cx.new(|_| DragPreviewView {
                            label: drag_label.clone(),
                        })
                    },
                )
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener({
                        let entry_name = entry_name.clone();
                        let entry_path = entry_path.clone();
                        move |app, event: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            if event.click_count == 2 {
                                if is_dir {
                                    app.sftp_navigate_dir(is_left, entry_path.clone(), cx);
                                }
                            } else if event.modifiers.shift
                                || event.modifiers.platform
                                || event.modifiers.control
                            {
                                if p.selected_files.contains(&entry_name) {
                                    p.selected_files.remove(&entry_name);
                                    if p.selected_file.as_deref() == Some(&entry_name) {
                                        p.selected_file = p.selected_files.iter().next().cloned();
                                    }
                                } else {
                                    p.selected_files.insert(entry_name.clone());
                                    p.selected_file = Some(entry_name.clone());
                                }
                                app.sftp_context_menu = None;
                                cx.notify();
                            } else {
                                p.selected_files.clear();
                                p.selected_files.insert(entry_name.clone());
                                p.selected_file = Some(entry_name.clone());
                                app.sftp_context_menu = None;
                                cx.notify();
                            }
                        }
                    }),
                )
                .on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener({
                        let entry_name = entry_name.clone();
                        let entry_path = entry_path.clone();
                        move |app, event: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            if !p.selected_files.contains(&entry_name) {
                                p.selected_files.clear();
                                p.selected_files.insert(entry_name.clone());
                                p.selected_file = Some(entry_name.clone());
                            }
                            let cursor_x = event.position.x.to_f64() as f32;
                            let cursor_y = event.position.y.to_f64() as f32;
                            app.sftp_open_context_menu(
                                is_left,
                                entry_name.clone(),
                                entry_path.clone(),
                                is_dir,
                                entry_idx,
                                cursor_x,
                                cursor_y,
                                cx,
                            );
                            cx.notify();
                        }
                    }),
                );

            if is_dir {
                let entry_path_for_drop = entry_path.clone();
                row = row.on_drop(cx.listener({
                    let entry_path_for_drop = entry_path_for_drop.clone();
                    move |app, payload: &SftpDragPayload, _, cx| {
                        if payload.from_left != is_left {
                            let p = if !payload.from_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            let previous_path =
                                std::mem::replace(&mut p.current_path, entry_path_for_drop.clone());
                            app.sftp_transfer_files(payload.from_left, payload.files.clone(), cx);
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            p.current_path = previous_path;
                            cx.stop_propagation();
                        }
                    }
                }));
            }

            row = row
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_2p5()
                        .child(icon_el)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(if is_dir {
                                            FontWeight::MEDIUM
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(if is_context_active {
                                            rgb(0xffffff)
                                        } else {
                                            rgb(MacTheme::TEXT_PRIMARY)
                                        })
                                        .truncate()
                                        .child(entry.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if is_context_active {
                                            rgba(0xffffffaa)
                                        } else {
                                            rgb(MacTheme::TEXT_TERTIARY)
                                        })
                                        .child(formatted_permissions),
                                ),
                        ),
                )
                .child(
                    div()
                        .w(px(140.0))
                        .text_xs()
                        .text_color(if is_context_active {
                            rgba(0xffffffcc)
                        } else {
                            rgb(MacTheme::TEXT_TERTIARY)
                        })
                        .child(formatted_date),
                )
                .child(
                    div()
                        .w(px(90.0))
                        .text_xs()
                        .text_color(if is_context_active {
                            rgba(0xffffffcc)
                        } else {
                            rgb(MacTheme::TEXT_TERTIARY)
                        })
                        .child(formatted_size),
                )
                .child(
                    div()
                        .w(px(70.0))
                        .text_xs()
                        .text_color(if is_context_active {
                            rgba(0xffffffcc)
                        } else {
                            rgb(MacTheme::TEXT_TERTIARY)
                        })
                        .child(kind_label),
                );

            table_body = table_body.child(row);
        }

        table_body = table_body.child(div().flex_1().min_h(px(60.0)).cursor_default());

        let sel_count = pane.selected_files.len();
        let total_count = pane.entries.len();
        let status_bar = div()
            .id(if is_left {
                "sftp-left-status-bar"
            } else {
                "sftp-right-status-bar"
            })
            .h(px(26.0))
            .px_3()
            .bg(rgb(MacTheme::SURFACE))
            .border_t_1()
            .border_color(rgb(MacTheme::BORDER))
            .flex()
            .items_center()
            .justify_between()
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_TERTIARY))
            .child(div().flex().items_center().gap_2().child(if sel_count > 0 {
                format!("{sel_count} of {total_count} items selected")
            } else {
                format!("{total_count} items")
            }))
            .children(if sel_count > 0 {
                Some(
                    div()
                        .id(if is_left {
                            "sftp-left-deselect-btn"
                        } else {
                            "sftp-right-deselect-btn"
                        })
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(MacTheme::INPUT_BG))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .cursor_pointer()
                        .hover(|s| {
                            s.text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                        })
                        .on_click(cx.listener(move |app, _, _, cx| {
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            p.selected_files.clear();
                            p.selected_file = None;
                            app.sftp_context_menu = None;
                            cx.notify();
                        }))
                        .child("Deselect"),
                )
            } else {
                None
            });

        let mut content = div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .on_drop(cx.listener(move |app, payload: &SftpDragPayload, _, cx| {
                if payload.from_left != is_left {
                    app.sftp_transfer_files(payload.from_left, payload.files.clone(), cx);
                    cx.stop_propagation();
                }
            }))
            .child(header)
            .child(breadcrumb_row)
            .child(table_header)
            .child(table_body)
            .child(status_bar);

        // Actions Dropdown Menu Overlay
        if actions_open {
            let backdrop = div()
                .id(if is_left {
                    "sftp-left-actions-backdrop"
                } else {
                    "sftp-right-actions-backdrop"
                })
                .absolute()
                .inset_0()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |app, _, _, cx| {
                        app.sftp_toggle_actions(is_left, cx);
                    }),
                );
            content = content.child(backdrop);

            let actions_menu = div()
                .id(if is_left {
                    "sftp-left-actions-menu"
                } else {
                    "sftp-right-actions-menu"
                })
                .absolute()
                .top(px(46.0))
                .right(px(12.0))
                .w(px(200.0))
                .rounded_lg()
                .bg(rgb(MacTheme::SURFACE))
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .p_1()
                .shadow_2xl()
                .flex()
                .flex_col()
                .gap_0p5()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-new-folder"
                        } else {
                            "sftp-r-act-new-folder"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            p.actions_open = false;
                            app.sftp_open_new_folder_modal(is_left, cx);
                        }))
                        .child("New Folder"),
                )
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-copy"
                        } else {
                            "sftp-r-act-copy"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.sftp_copy_to_target(is_left, cx);
                        }))
                        .child("Copy to target directory"),
                )
                .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-refresh"
                        } else {
                            "sftp-r-act-refresh"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.refresh_sftp_pane(is_left, cx);
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            p.actions_open = false;
                        }))
                        .child("Refresh"),
                )
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-hidden"
                        } else {
                            "sftp-r-act-hidden"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.sftp_toggle_pane_hidden(is_left, cx);
                        }))
                        .child(if pane.show_hidden {
                            "Hide Hidden Files"
                        } else {
                            "Show Hidden Files"
                        }),
                )
                .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-close"
                        } else {
                            "sftp-r-act-close"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::DANGER))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.sftp_close_host(is_left, cx);
                        }))
                        .child("Close"),
                );

            content = content.child(actions_menu);
        }

        content
    }
}

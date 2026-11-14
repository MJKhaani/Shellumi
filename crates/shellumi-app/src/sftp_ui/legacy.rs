//! SFTP legacy UI.
use super::*;

impl Shellumi {
    pub(crate) fn render_sftp_dual_pane(
        &self,
        tab_idx: usize,
        state: &SftpBrowserState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut container = div().flex().flex_col().gap_3().flex_1().min_h_0();

        if let Some(err) = &state.error {
            container = container.child(
                div()
                    .p_2()
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .rounded_md()
                    .text_sm()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("Error: {err}")),
            );
        } else if let Some(msg) = &state.status_message {
            container = container.child(
                div()
                    .p_2()
                    .bg(rgb(0x133820))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .rounded_md()
                    .text_sm()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(msg.clone()),
            );
        }

        // Dual pane row
        let mut panes_row = div().flex().flex_row().gap_4().flex_1().min_h_0();

        // 1. Local Pane
        let local_crumbs = PathUtils::breadcrumbs(&state.local_path);
        let mut local_crumbs_div = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_SECONDARY))
            .flex_wrap();
        local_crumbs_div = local_crumbs_div.child("💻 Local: ");
        for (i, (name, path)) in local_crumbs.into_iter().enumerate() {
            let p_clone = path.clone();
            local_crumbs_div = local_crumbs_div.child(
                div()
                    .id(("local-crumb", i))
                    .cursor_pointer()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                    .hover(|s| s.underline())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_local(tab_idx, p_clone.clone(), cx);
                    }))
                    .child(format!("{name}/")),
            );
        }

        let mut local_list_div = div()
            .id("sftp-local-list")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .overflow_y_scroll();

        if state.local_entries.is_empty() {
            local_list_div = local_list_div.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Empty directory"),
            );
        } else {
            for (i, entry) in state.local_entries.iter().enumerate() {
                let is_selected = state.selected_local.as_deref() == Some(&entry.name);
                let name_clone = entry.name.clone();
                let is_dir = entry.kind.is_dir();
                let icon = match entry.kind {
                    FileKind::Directory => "📁 ",
                    FileKind::Symlink { .. } => "🔗 ",
                    _ => "📄 ",
                };

                let row_base = div()
                    .id(("local-row", i))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .p_1()
                    .rounded_sm()
                    .text_xs()
                    .cursor_pointer()
                    .bg(if is_selected {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_color(if is_selected {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .hover(|s| {
                        s.bg(if is_selected {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SURFACE_HOVER)
                        })
                    });

                let row = if is_dir {
                    let d_name = entry.name.clone();
                    row_base.on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_open_local_dir(tab_idx, d_name.clone(), cx);
                    }))
                } else {
                    let f_name = entry.name.clone();
                    row_base.on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_select_local(tab_idx, f_name.clone(), cx);
                    }))
                };

                let row_el = row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .flex_1()
                            .child(icon)
                            .child(name_clone),
                    )
                    .child(
                        div()
                            .w(px(70.0))
                            .text_right()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(entry.formatted_size()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_center()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child(entry.formatted_permissions()),
                    );

                local_list_div = local_list_div.child(row_el);
            }
        }

        let local_pane = div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .p_3()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(local_crumbs_div)
                    .child(
                        div()
                            .id("sftp-local-up")
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
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_navigate_local_up(tab_idx, cx);
                            }))
                            .child("⬆ Up"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .font_weight(FontWeight::BOLD)
                    .child(div().flex_1().child("Name"))
                    .child(div().w(px(70.0)).text_right().child("Size"))
                    .child(div().w(px(80.0)).text_center().child("Mode")),
            )
            .child(local_list_div);

        // 2. Middle Actions Column
        let has_local_sel = state.selected_local.is_some();
        let has_remote_sel = state.selected_remote.is_some();

        let actions_column = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .px_2()
            .child(
                div()
                    .id("sftp-upload-btn")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(if has_local_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if has_local_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_color(if has_local_sel {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .text_sm()
                    .cursor_pointer()
                    .hover(move |s| {
                        if has_local_sel {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_upload_selected(tab_idx, cx);
                    }))
                    .child("Upload →"),
            )
            .child(
                div()
                    .id("sftp-download-btn")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(if has_remote_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if has_remote_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_color(if has_remote_sel {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .text_sm()
                    .cursor_pointer()
                    .hover(move |s| {
                        if has_remote_sel {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_download_selected(tab_idx, cx);
                    }))
                    .child("← Download"),
            );

        // 3. Remote Pane
        let remote_crumbs = PathUtils::breadcrumbs(&state.remote_path);
        let mut remote_crumbs_div = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_SECONDARY))
            .flex_wrap();
        remote_crumbs_div = remote_crumbs_div.child(format!("🌐 Remote ({}): ", state.host_name));
        for (i, (name, path)) in remote_crumbs.into_iter().enumerate() {
            let p_clone = path.clone();
            remote_crumbs_div = remote_crumbs_div.child(
                div()
                    .id(("remote-crumb", i))
                    .cursor_pointer()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                    .hover(|s| s.underline())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_remote(tab_idx, p_clone.clone(), cx);
                    }))
                    .child(format!("{name}/")),
            );
        }

        let mut remote_list_div = div()
            .id("sftp-remote-list")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .overflow_y_scroll();

        if state.loading {
            remote_list_div = remote_list_div.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                    .child("Loading remote directory…"),
            );
        } else if state.remote_entries.is_empty() {
            remote_list_div = remote_list_div.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Empty directory"),
            );
        } else {
            for (i, entry) in state.remote_entries.iter().enumerate() {
                let is_selected = state.selected_remote.as_deref() == Some(&entry.name);
                let name_clone = entry.name.clone();
                let is_dir = entry.kind.is_dir();
                let icon = match entry.kind {
                    FileKind::Directory => "📁 ",
                    FileKind::Symlink { .. } => "🔗 ",
                    _ => "📄 ",
                };

                let row_base = div()
                    .id(("remote-row", i))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .p_1()
                    .rounded_sm()
                    .text_xs()
                    .cursor_pointer()
                    .bg(if is_selected {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_color(if is_selected {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .hover(|s| {
                        s.bg(if is_selected {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SURFACE_HOVER)
                        })
                    });

                let row = if is_dir {
                    let d_name = entry.name.clone();
                    row_base.on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_open_remote_dir(tab_idx, d_name.clone(), cx);
                    }))
                } else {
                    let f_name = entry.name.clone();
                    row_base.on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_select_remote(tab_idx, f_name.clone(), cx);
                    }))
                };

                let row_el = row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .flex_1()
                            .child(icon)
                            .child(name_clone),
                    )
                    .child(
                        div()
                            .w(px(70.0))
                            .text_right()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(entry.formatted_size()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_center()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child(entry.formatted_permissions()),
                    );

                remote_list_div = remote_list_div.child(row_el);
            }
        }

        let remote_pane = div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .p_3()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(remote_crumbs_div)
                    .child(
                        div()
                            .id("sftp-remote-up")
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
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_navigate_remote_up(tab_idx, cx);
                            }))
                            .child("⬆ Up"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .font_weight(FontWeight::BOLD)
                    .child(div().flex_1().child("Name"))
                    .child(div().w(px(70.0)).text_right().child("Size"))
                    .child(div().w(px(80.0)).text_center().child("Mode")),
            )
            .child(remote_list_div);

        panes_row = panes_row
            .child(local_pane)
            .child(actions_column)
            .child(remote_pane);

        container = container.child(panes_row);

        // 4. Bottom Transfer Queue Drawer
        let q_summary = state.queue.summary();
        let mut queue_drawer = div()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .p_3()
            .gap_2();

        let queue_header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(format!(
                        "Transfers & Queue: {} completed / {} total ({:.1}%)",
                        q_summary.completed_tasks,
                        q_summary.total_tasks,
                        q_summary.overall_percentage
                    )),
            )
            .child(
                div()
                    .id("sftp-clear-completed-btn")
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
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_clear_completed(tab_idx, cx);
                    }))
                    .child("Clear Completed"),
            );

        // Progress bar
        let pct = q_summary.overall_percentage.clamp(0.0, 100.0);
        let progress_bar = div()
            .w_full()
            .h(px(6.0))
            .bg(rgb(MacTheme::INPUT_BG))
            .rounded_full()
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .bg(rgb(MacTheme::ACCENT_BLUE))
                    .w(gpui::relative(pct / 100.0)),
            );

        queue_drawer = queue_drawer.child(queue_header).child(progress_bar);

        if !state.queue.tasks().is_empty() {
            let mut tasks_list = div()
                .id("sftp-tasks-list")
                .flex()
                .flex_col()
                .gap_1()
                .max_h(px(70.0))
                .overflow_y_scroll();

            for task in state.queue.tasks() {
                let status_badge = match &task.status {
                    TransferStatus::Completed => div()
                        .text_color(rgb(MacTheme::SUCCESS))
                        .child("✓ Completed"),
                    TransferStatus::InProgress => div()
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                        .child("⟳ In Progress"),
                    TransferStatus::Failed(err) => div()
                        .text_color(rgb(MacTheme::DANGER))
                        .child(format!("✗ Failed: {err}")),
                    TransferStatus::Queued => div()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("⏳ Queued"),
                    TransferStatus::Cancelled => div()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("Cancelled"),
                };

                let mut task_row = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(div().flex_1().min_w_0().truncate().child(format!(
                        "#{} {} · {} → {}",
                        task.id, task.direction, task.local_path, task.remote_path
                    )))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(format!(
                                "{}/{} ({:.1}%)",
                                task.bytes_transferred,
                                task.total_bytes,
                                task.progress_percentage()
                            ))
                            .child(status_badge),
                    );

                if matches!(
                    task.status,
                    TransferStatus::Queued | TransferStatus::InProgress
                ) {
                    let task_id = task.id;
                    let tab_id = self.tabs[tab_idx].id;
                    let requested = self
                        .sftp_jobs
                        .get(&(tab_id, task_id))
                        .is_some_and(|job| job.control.is_cancelled());
                    if requested {
                        task_row = task_row.child("Cancelling…");
                    } else {
                        task_row = task_row.child(
                            div()
                                .id(("cancel-transfer", task_id))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .bg(rgb(0x323236))
                                .text_color(rgb(MacTheme::DANGER))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .child("Cancel")
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.sftp_cancel_transfer(tab_id, task_id, cx)
                                })),
                        );
                    }
                }
                tasks_list = tasks_list.child(task_row);
            }

            queue_drawer = queue_drawer.child(tasks_list);
        }

        container = container.child(queue_drawer);
        container
    }
}

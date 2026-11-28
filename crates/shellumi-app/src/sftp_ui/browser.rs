//! SFTP browser UI, integrated editor switcher, and transfer drawer.
use super::*;

impl Shellumi {
    pub(crate) fn render_sftp_redesigned_dual_pane(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut root = div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::CONTENT_BG));

        // Always render SFTP workspace bar so Transfer Drawer & Editor tabs remain reachable.
        {
            let active_ed = self.active_sftp_editor_idx;
            let jobs_count = self.sftp_controller.authorized_snapshots().len();
            let bar = div()
                .h(px(32.0))
                .px_3()
                .border_b_1()
                .border_color(rgb(MacTheme::BORDER))
                .bg(rgb(MacTheme::SURFACE))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .child(
                            div()
                                .id("sftp-tab-browser")
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .bg(if active_ed.is_none() {
                                    rgb(MacTheme::SURFACE_ACTIVE)
                                } else {
                                    rgb(MacTheme::SURFACE)
                                })
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.active_sftp_editor_idx = None;
                                    cx.notify();
                                }))
                                .child("Files"),
                        )
                        .children(self.sftp_editors.iter().enumerate().map(|(idx, ed)| {
                            let is_active = active_ed == Some(idx);
                            div()
                                .id(("sftp-editor-tab", idx))
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .bg(if is_active {
                                    rgb(MacTheme::SURFACE_ACTIVE)
                                } else {
                                    rgb(MacTheme::SURFACE)
                                })
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.active_sftp_editor_idx = Some(idx);
                                    cx.notify();
                                }))
                                .child(ed.title(cx))
                        })),
                )
                .child(
                    div()
                        .id("sftp-toggle-transfer-drawer")
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(MacTheme::SURFACE_HOVER))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.sftp_transfer_drawer_open = !app.sftp_transfer_drawer_open;
                            cx.notify();
                        }))
                        .child(format!("Transfers ({jobs_count})")),
                );
            root = root.child(bar);
        }

        // Main content: either active integrated editor or dual-pane browser (with narrow switcher)
        let is_narrow = self.measured_workspace_width.is_some_and(|w| w < 760.0);
        let main_area = if let Some(ed) = self
            .active_sftp_editor_idx
            .and_then(|idx| self.sftp_editors.get(idx))
        {
            div()
                .flex_1()
                .min_h_0()
                .child(self.render_sftp_editor_panel(ed, cx))
        } else if is_narrow {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(28.0))
                        .px_3()
                        .border_b_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("sftp-narrow-left")
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .bg(if self.sftp_narrow_active_left {
                                    rgb(MacTheme::SURFACE_ACTIVE)
                                } else {
                                    rgb(MacTheme::SURFACE)
                                })
                                .text_xs()
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_narrow_active_left = true;
                                    cx.notify();
                                }))
                                .child("Source Pane"),
                        )
                        .child(
                            div()
                                .id("sftp-narrow-right")
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .bg(if !self.sftp_narrow_active_left {
                                    rgb(MacTheme::SURFACE_ACTIVE)
                                } else {
                                    rgb(MacTheme::SURFACE)
                                })
                                .text_xs()
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_narrow_active_left = false;
                                    cx.notify();
                                }))
                                .child("Target Pane"),
                        ),
                )
                .child(self.render_sftp_pane(self.sftp_narrow_active_left, cx))
        } else {
            let left_weight = self.sftp_split_ratio.clamp(0.25, 0.75);
            let right_weight = 1.0 - left_weight;
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(
                    div()
                        .flex_grow()
                        .flex_basis(gpui::relative(left_weight))
                        .min_w_0()
                        .h_full()
                        .child(self.render_sftp_pane(true, cx)),
                )
                .child(
                    div()
                        .flex_grow()
                        .flex_basis(gpui::relative(right_weight))
                        .min_w_0()
                        .h_full()
                        .child(self.render_sftp_pane(false, cx)),
                )
        };

        root = root.child(main_area);

        // Collapsible Transfer Drawer (vault-scoped via SftpController)
        if self.sftp_transfer_drawer_open {
            let snapshots = self.sftp_controller.authorized_snapshots();
            root = root.child(
                div()
                    .h(px(150.0))
                    .border_t_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .bg(rgb(MacTheme::SURFACE))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(28.0))
                            .px_3()
                            .border_b_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(format!("Transfer Queue ({})", snapshots.len()))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id("sftp-clear-drawer-history")
                                            .cursor_pointer()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                let scope = app.current_vault_scope();
                                                app.sftp_controller
                                                    .transfers
                                                    .clear_terminal_history(&scope);
                                                cx.notify();
                                            }))
                                            .child("Clear Done"),
                                    )
                                    .child(
                                        div()
                                            .id("sftp-close-drawer")
                                            .cursor_pointer()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                app.sftp_transfer_drawer_open = false;
                                                cx.notify();
                                            }))
                                            .child("Hide"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("sftp-transfer-drawer-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_2()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(snapshots.into_iter().enumerate().map(|(i, snap)| {
                                let job_id = snap.intent.job_id;
                                let status_text = format!(
                                    "{} · {} -> {} · {:?} ({} B)",
                                    snap.intent.route,
                                    snap.intent.source_path.display(),
                                    snap.intent.destination_path.display(),
                                    snap.state,
                                    snap.bytes_transferred
                                );
                                div()
                                    .id(("sftp-drawer-row", i))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(MacTheme::CONTENT_BG))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child(status_text)
                                    .when(snap.state.is_active(), |row| {
                                        row.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .id(("sftp-drawer-pause", i))
                                                        .cursor_pointer()
                                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                        .on_click(cx.listener(
                                                            move |app, _, _, cx| {
                                                                app.sftp_controller
                                                                    .transfers
                                                                    .pause_job(job_id);
                                                                cx.notify();
                                                            },
                                                        ))
                                                        .child("Pause"),
                                                )
                                                .child(
                                                    div()
                                                        .id(("sftp-drawer-cancel", i))
                                                        .cursor_pointer()
                                                        .text_color(rgb(MacTheme::DANGER))
                                                        .on_click(cx.listener(
                                                            move |app, _, _, cx| {
                                                                app.sftp_controller
                                                                    .transfers
                                                                    .cancel_job(job_id);
                                                                cx.notify();
                                                            },
                                                        ))
                                                        .child("Cancel"),
                                                ),
                                        )
                                    })
                                    .when(
                                        snap.state == shellumi_core::TransferJobState::Paused,
                                        |row| {
                                            row.child(
                                                div()
                                                    .id(("sftp-drawer-resume", i))
                                                    .cursor_pointer()
                                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                                    .on_click(cx.listener(move |app, _, _, cx| {
                                                        let scope = app.current_vault_scope();
                                                        if app
                                                            .sftp_controller
                                                            .transfers
                                                            .resume_job(job_id, &scope)
                                                            .is_ok()
                                                        {
                                                            app.sftp_pump_transfers(cx);
                                                        }
                                                        cx.notify();
                                                    }))
                                                    .child("Resume"),
                                            )
                                        },
                                    )
                                    .when(
                                        snap.state
                                            == shellumi_core::TransferJobState::AwaitingConflict,
                                        |row| {
                                            row.child(
                                                div()
                                                    .id(("sftp-drawer-resolve", i))
                                                    .cursor_pointer()
                                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                                    .on_click(cx.listener(move |app, _, _, cx| {
                                                        app.sftp_surface_next_conflict(cx);
                                                    }))
                                                    .child("Resolve"),
                                            )
                                        },
                                    )
                                    .when(
                                        matches!(
                                            snap.state,
                                            shellumi_core::TransferJobState::Failed
                                                | shellumi_core::TransferJobState::Cancelled
                                        ),
                                        |row| {
                                            row.child(
                                                div()
                                                    .id(("sftp-drawer-retry", i))
                                                    .cursor_pointer()
                                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                                    .on_click(cx.listener(move |app, _, _, cx| {
                                                        let scope = app.current_vault_scope();
                                                        if app
                                                            .sftp_controller
                                                            .transfers
                                                            .retry_job(job_id, &scope)
                                                            .is_ok()
                                                        {
                                                            app.sftp_pump_transfers(cx);
                                                        }
                                                        cx.notify();
                                                    }))
                                                    .child("Retry"),
                                            )
                                        },
                                    )
                            })),
                    ),
            );
        }

        if let Some(ctx_menu) = &self.sftp_context_menu {
            root = root.child(self.render_sftp_context_menu_overlay(ctx_menu, cx));
        }

        if let Some(modal) = &self.sftp_modal {
            root = root.child(self.render_sftp_modal_overlay(modal, cx));
        }

        root
    }

    fn render_sftp_pane(&self, is_left: bool, cx: &mut Context<Self>) -> gpui::Div {
        let pane = if is_left {
            &self.sftp_left_pane
        } else {
            &self.sftp_right_pane
        };
        let mut pane_box = div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden();

        if is_left {
            pane_box = pane_box.border_r_1().border_color(rgb(MacTheme::BORDER));
        }

        if pane.is_selecting_host {
            pane_box.child(self.render_sftp_select_host(is_left, cx))
        } else if pane.host.is_none() {
            pane_box.child(self.render_sftp_connect_to_host(is_left, cx))
        } else {
            pane_box.child(self.render_sftp_connected_browser(is_left, cx))
        }
    }
}

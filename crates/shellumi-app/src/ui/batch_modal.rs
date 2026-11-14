//! Batch modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_batch_modal(
        &self,
        modal: &BatchModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("batch-modal-overlay")
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
            .id("batch-modal-container")
            .w(px(720.0))
            .max_h(px(640.0))
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
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child(format!("⚡ Multi-Host Execution: {}", modal.snippet_name)),
                ),
            )
            .child(
                div()
                    .id("dismiss-batch-modal-btn")
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
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_batch_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Template card
        let template_card = div()
            .p_2()
            .rounded_md()
            .bg(rgb(MacTheme::CONTENT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .font_family(MONOSPACE_FONT)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(format!("Template: {}", modal.command_template)),
            );
        modal_box = modal_box.child(template_card);

        // Variables fields if any
        if !modal.variables.is_empty() {
            let mut vars_list = div()
                .id("batch-vars-list")
                .flex()
                .flex_col()
                .gap_2()
                .overflow_y_scroll()
                .max_h(px(120.0));
            for var in &modal.variables {
                let var_header = div().flex().items_center().gap_2().child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child(var.name.clone()),
                );
                let field = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(var_header)
                    .child(var.input.clone());
                vars_list = vars_list.child(field);
            }
            modal_box = modal_box.child(vars_list);
        }

        // Policy & Dry-run row
        let is_parallel = modal.policy == BatchExecutionPolicyChoice::Parallel;
        let is_dry_run = modal.dry_run;
        let controls_row = div()
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
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Policy:"),
                    )
                    .child(
                        div()
                            .id("policy-parallel-btn")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if is_parallel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if is_parallel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if is_parallel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if is_parallel {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.set_batch_policy(BatchExecutionPolicyChoice::Parallel, cx);
                            }))
                            .child("Parallel (5)"),
                    )
                    .child(
                        div()
                            .id("policy-sequential-btn")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if !is_parallel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if !is_parallel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if !is_parallel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if !is_parallel {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.set_batch_policy(BatchExecutionPolicyChoice::Sequential, cx);
                            }))
                            .child("Sequential"),
                    ),
            )
            .child(
                div()
                    .id("toggle-dry-run-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if is_dry_run {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if is_dry_run {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_dry_run {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if is_dry_run {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| app.toggle_batch_dry_run(cx)))
                    .child(if is_dry_run {
                        "✔ Dry Run (Preview Only)"
                    } else {
                        "Dry Run Mode: Off"
                    }),
            );
        modal_box = modal_box.child(controls_row);

        // Host filter & Selection header
        let selected_count = modal.hosts.iter().filter(|h| h.selected).count();
        let total_count = modal.hosts.len();
        let query = modal.search_input.read(cx).value().trim().to_lowercase();

        let host_selection_bar = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(format!(
                        "Target Hosts ({selected_count}/{total_count} selected):"
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("batch-select-all-btn")
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(0x323236))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(
                                cx.listener(|app, _, _, cx| app.batch_select_all_hosts(true, cx)),
                            )
                            .child("Select All"),
                    )
                    .child(
                        div()
                            .id("batch-deselect-all-btn")
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(0x323236))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(
                                cx.listener(|app, _, _, cx| app.batch_select_all_hosts(false, cx)),
                            )
                            .child("Deselect All"),
                    ),
            );
        modal_box = modal_box.child(host_selection_bar);

        // Search input
        modal_box = modal_box.child(modal.search_input.clone());

        // Target hosts scrollable list
        let mut hosts_list = div()
            .id("batch-hosts-list")
            .flex()
            .flex_col()
            .gap_2()
            .overflow_y_scroll()
            .max_h(px(220.0));

        let filtered_hosts: Vec<&BatchHostItem> = modal
            .hosts
            .iter()
            .filter(|h| {
                if query.is_empty() {
                    true
                } else {
                    h.name.to_lowercase().contains(&query)
                        || h.destination.host().to_lowercase().contains(&query)
                        || h.destination.username().to_lowercase().contains(&query)
                        || h.tags.iter().any(|t| t.to_lowercase().contains(&query))
                }
            })
            .collect();

        if filtered_hosts.is_empty() {
            hosts_list = hosts_list.child(
                div()
                    .p_3()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("No hosts match the filter criteria."),
            );
        } else {
            for host in filtered_hosts {
                let hid = host.id;
                let is_selected = host.selected;
                let (r, g, b) = host.status.badge_color();
                let is_expanded = modal.expanded_host_output == Some(hid);

                let mut row = div()
                    .id(("batch-host-row", hid as usize))
                    .flex()
                    .flex_col()
                    .p_2()
                    .rounded_md()
                    .bg(if is_selected {
                        rgb(MacTheme::INPUT_BG)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .border_1()
                    .border_color(if is_selected {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    });

                let mut top_line = div().flex().items_center().justify_between().gap_2();

                let left_side = div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id(("batch-host-check", hid as usize))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .border_1()
                            .border_color(if is_selected {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if is_selected {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_batch_host_selection(hid, cx);
                            }))
                            .child(if is_selected { "☑" } else { "☐" }),
                    )
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
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(format!(
                                "({}@{}:{})",
                                host.destination.username(),
                                host.destination.host(),
                                host.destination.port()
                            )),
                    );

                let mut right_side = div().flex().items_center().gap_2();
                if let Some(dur) = host.duration_ms {
                    right_side = right_side.child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(format!("{dur}ms")),
                    );
                }
                right_side = right_side.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child(host.status.as_str()),
                );

                if !host.output.is_empty() {
                    right_side = right_side.child(
                        div()
                            .id(("toggle-output", hid as usize))
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(0x323236))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_batch_host_output(hid, cx);
                            }))
                            .child(if is_expanded {
                                "Hide Output"
                            } else {
                                "View Output"
                            }),
                    );
                }

                top_line = top_line.child(left_side).child(right_side);
                row = row.child(top_line);

                if is_expanded && !host.output.is_empty() {
                    let out_card = div()
                        .mt_2()
                        .p_2()
                        .rounded_sm()
                        .bg(rgb(MacTheme::CONTENT_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .child(
                            div()
                                .text_xs()
                                .font_family(MONOSPACE_FONT)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(host.output.clone()),
                        );
                    row = row.child(out_card);
                }

                hosts_list = hosts_list.child(row);
            }
        }
        modal_box = modal_box.child(hosts_list);

        // Summary bar if completed
        if let Some(summary) = &modal.summary {
            let is_failed = summary.failed > 0;
            let summary_card = div()
                .p_2()
                .rounded_md()
                .bg(if is_failed {
                    rgb(0x4a1818)
                } else {
                    rgb(0x133820)
                })
                .border_1()
                .border_color(if is_failed {
                    rgb(MacTheme::DANGER)
                } else {
                    rgb(MacTheme::SUCCESS)
                })
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child(format!(
                            "Execution Summary: {}/{} succeeded, {} failed ({}ms total)",
                            summary.succeeded,
                            summary.total,
                            summary.failed,
                            summary.total_duration_ms
                        )),
                );
            modal_box = modal_box.child(summary_card);
        }

        // Error message if any
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("⚠ {err}")),
            );
        }

        // Action buttons
        let is_running = modal.is_running;
        let action_btn_text = if is_running {
            "Executing on hosts…".to_string()
        } else if modal.dry_run {
            format!("⚡ Preview on {selected_count} Hosts")
        } else {
            format!("⚡ Execute on {selected_count} Hosts")
        };

        let actions = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_3()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pt_3()
            .child(
                div()
                    .id("cancel-batch-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_batch_modal(cx)))
                    .child("Close"),
            )
            .child(
                div()
                    .id("run-batch-btn")
                    .px_4()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if is_running || selected_count == 0 {
                        rgb(MacTheme::SEPARATOR)
                    } else {
                        rgb(MacTheme::ACCENT_BLUE)
                    })
                    .bg(if is_running || selected_count == 0 {
                        rgb(0x38383a)
                    } else {
                        rgb(MacTheme::ACCENT_BLUE)
                    })
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_running || selected_count == 0 {
                        rgb(MacTheme::TEXT_TERTIARY)
                    } else {
                        rgb(0xffffff)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if !is_running && selected_count > 0 {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| app.execute_batch(cx)))
                    .child(action_btn_text),
            );
        modal_box = modal_box.child(actions);

        overlay.child(modal_box)
    }
}

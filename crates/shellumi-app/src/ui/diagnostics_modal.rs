//! Diagnostics modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_diagnostics_modal(
        &self,
        modal: &DiagnosticsModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("diagnostics-modal-overlay")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("diagnostics-modal-box")
            .w(px(740.0))
            .max_h(px(580.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .shadow_lg()
            .flex()
            .flex_col()
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
                        if let Some(p) = resolve_icon_path("diagnostics", false, 48) {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().child("🩺").into_any_element()
                        },
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("System Diagnostics & Production Health Check"),
                    ),
            )
            .child(
                div()
                    .id("dismiss-diagnostics-modal-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_diagnostics_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Sub-Navigation Tabs
        let is_health = modal.active_tab == DiagnosticsModalTab::HealthChecks;
        let is_bench = modal.active_tab == DiagnosticsModalTab::Benchmarks;
        let tabs = div()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_2()
            .child(
                div()
                    .id("diag-tab-health")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(if is_health {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .bg(if is_health {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_health {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(move |s| {
                        if is_health {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(ref mut m) = app.diagnostics_modal {
                            m.active_tab = DiagnosticsModalTab::HealthChecks;
                            if m.report.is_none() && !m.is_running {
                                app.run_diagnostics_checks(cx);
                            }
                        }
                        cx.notify();
                    }))
                    .child("🩺 System Health Checks"),
            )
            .child(
                div()
                    .id("diag-tab-benchmarks")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(if is_bench {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .bg(if is_bench {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_bench {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(move |s| {
                        if is_bench {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(ref mut m) = app.diagnostics_modal {
                            m.active_tab = DiagnosticsModalTab::Benchmarks;
                            if m.benchmark_report.is_none() && !m.is_running_benchmarks {
                                app.run_benchmarks(cx);
                            }
                        }
                        cx.notify();
                    }))
                    .child("⚡ Performance Benchmarks"),
            );
        modal_box = modal_box.child(tabs);

        match modal.active_tab {
            DiagnosticsModalTab::HealthChecks => {
                if modal.is_running {
                    let loading = div()
                        .p_8()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .child(
                            div()
                                .text_lg()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child("⏳ Running system diagnostic checks..."),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Testing OpenSSH binary, CSPRNG, AEAD ciphers, network loopback, and storage integrity..."),
                        );
                    modal_box = modal_box.child(loading);
                } else if let Some(ref report) = modal.report {
                    let (passed, warnings, failures) = report.counts();

                    // Status Banner
                    let (banner_bg, banner_fg, banner_text) = if failures > 0 {
                        (
                            rgb(0x4a1818),
                            rgb(MacTheme::DANGER),
                            format!("❌ Action Required: {failures} critical check(s) failed."),
                        )
                    } else if warnings > 0 {
                        (
                            rgb(0x3a2c10),
                            rgb(MacTheme::WARNING),
                            format!("⚠️ Degraded: All critical tests passed, but {warnings} warning(s) detected."),
                        )
                    } else {
                        (
                            rgb(0x133820),
                            rgb(MacTheme::SUCCESS),
                            "✅ All Systems Ready: All production environment prerequisites passed."
                                .to_string(),
                        )
                    };

                    let status_strip = div()
                        .px_4()
                        .py_2p5()
                        .rounded_md()
                        .bg(banner_bg)
                        .border_1()
                        .border_color(banner_fg)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(banner_fg)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(banner_text)
                        .child(format!(
                            "Passed: {passed} | Warnings: {warnings} | Failed: {failures}"
                        ));
                    modal_box = modal_box.child(status_strip);

                    // Checks list
                    let mut checks_list = div()
                        .id("diagnostics-checks-list")
                        .flex()
                        .flex_col()
                        .gap_2()
                        .overflow_y_scroll()
                        .max_h(px(320.0));

                    for (c_idx, check) in report.checks.iter().enumerate() {
                        let (badge_bg, badge_fg) = match check.status {
                            DiagnosticStatus::Passed => (rgb(0x133820), rgb(MacTheme::SUCCESS)),
                            DiagnosticStatus::Warning => (rgb(0x3a2c10), rgb(MacTheme::WARNING)),
                            DiagnosticStatus::Failed => (rgb(0x4a1818), rgb(MacTheme::DANGER)),
                        };

                        let mut row = div()
                            .id(("diag-check-row", c_idx))
                            .p_3()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .flex()
                            .flex_col()
                            .gap_1p5();

                        let row_header = div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_sm().child(check.category.icon()))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child(check.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child(format!("{}ms", check.duration_ms)),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(badge_bg)
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(badge_fg)
                                            .child(check.status.badge()),
                                    ),
                            );
                        row = row.child(row_header);

                        let row_msg = div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(check.message.clone());
                        row = row.child(row_msg);

                        if let Some(ref rem) = check.remediation {
                            let row_rem = div()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::CONTENT_BG))
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .text_xs()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child(format!("↳ Remediation: {rem}"));
                            row = row.child(row_rem);
                        }

                        checks_list = checks_list.child(row);
                    }
                    modal_box = modal_box.child(checks_list);

                    // Footer controls
                    let footer = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pt_2()
                        .border_t_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child(format!(
                                    "Platform: {} [{}]",
                                    report.os_platform, report.os_arch
                                )),
                        )
                        .child(
                            div()
                                .id("rerun-diagnostics-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.run_diagnostics_checks(cx);
                                }))
                                .child("🔄 Re-run Checks"),
                        );
                    modal_box = modal_box.child(footer);
                }
            }
            DiagnosticsModalTab::Benchmarks => {
                if modal.is_running_benchmarks {
                    let loading = div()
                        .p_8()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .child(
                            div()
                                .text_lg()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child("⏳ Running deterministic micro-benchmarks..."),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Evaluating AES-256-GCM AEAD, PBKDF2 stretching, autocomplete, template engine, and sync diffing..."),
                        );
                    modal_box = modal_box.child(loading);
                } else if let Some(ref b_report) = modal.benchmark_report {
                    let (b_bg, b_fg, b_title) = if b_report.failed_count > 0 {
                        (
                            rgb(0x4a1818),
                            rgb(MacTheme::DANGER),
                            format!(
                                "❌ SLA Breach: {} benchmark(s) failed strict SLAs.",
                                b_report.failed_count
                            ),
                        )
                    } else if b_report.warning_count > 0 {
                        (
                            rgb(0x3a2c10),
                            rgb(MacTheme::WARNING),
                            format!(
                                "⚠️ SLA Warning: {} benchmark(s) close to SLA threshold.",
                                b_report.warning_count
                            ),
                        )
                    } else {
                        (
                            rgb(0x133820),
                            rgb(MacTheme::SUCCESS),
                            "✅ All Performance Benchmarks Passed: Subsystem throughput and latency exceed strict SLA targets."
                                .to_string(),
                        )
                    };

                    let status_strip = div()
                        .px_4()
                        .py_2p5()
                        .rounded_md()
                        .bg(b_bg)
                        .border_1()
                        .border_color(b_fg)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(b_fg)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(b_title)
                        .child(format!(
                            "Total: {:.1}ms | Passed: {} | Warnings: {} | Failed: {}",
                            b_report.total_duration_ms,
                            b_report.passed_count,
                            b_report.warning_count,
                            b_report.failed_count
                        ));
                    modal_box = modal_box.child(status_strip);

                    let mut bench_list = div()
                        .id("diagnostics-bench-list")
                        .flex()
                        .flex_col()
                        .gap_2()
                        .overflow_y_scroll()
                        .max_h(px(320.0));

                    for (b_idx, metric) in b_report.metrics.iter().enumerate() {
                        let (badge_bg, badge_fg) = match metric.status {
                            BenchmarkStatus::Passed => (rgb(0x133820), rgb(MacTheme::SUCCESS)),
                            BenchmarkStatus::Warning => (rgb(0x3a2c10), rgb(MacTheme::WARNING)),
                            BenchmarkStatus::Failed => (rgb(0x4a1818), rgb(MacTheme::DANGER)),
                        };

                        let latency_str = if metric.latency_us_per_op >= 1000.0 {
                            format!("{:.2} ms/op", metric.latency_us_per_op / 1000.0)
                        } else {
                            format!("{:.1} µs/op", metric.latency_us_per_op)
                        };

                        let throughput_str = if let Some(mbps) = metric.throughput_mb_per_sec {
                            format!("{:.1} MB/s", mbps)
                        } else if metric.ops_per_sec >= 1000.0 {
                            format!("{:.0} k ops/s", metric.ops_per_sec / 1000.0)
                        } else {
                            format!("{:.0} ops/s", metric.ops_per_sec)
                        };

                        let mut row = div()
                            .id(("diag-bench-row", b_idx))
                            .p_3()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .flex()
                            .flex_col()
                            .gap_1p5();

                        let row_header = div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_sm().child(metric.category.icon()))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child(metric.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child(format!("({} iters)", metric.iterations)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                                            .child(throughput_str),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child(latency_str),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(badge_bg)
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(badge_fg)
                                            .child(metric.status.badge()),
                                    ),
                            );
                        row = row.child(row_header);

                        let row_desc = div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child(metric.description.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::WARNING))
                                    .child(format!("SLA: {}", metric.target_sla)),
                            );
                        row = row.child(row_desc);

                        if !metric.note.is_empty() {
                            let row_note = div()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::CONTENT_BG))
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child(format!("↳ {}", metric.note));
                            row = row.child(row_note);
                        }

                        bench_list = bench_list.child(row);
                    }
                    modal_box = modal_box.child(bench_list);

                    let bench_footer = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pt_2()
                        .border_t_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child("Local Deterministic Micro-Benchmarks (All Offline)"),
                        )
                        .child(
                            div()
                                .id("rerun-benchmarks-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.run_benchmarks(cx);
                                }))
                                .child("⚡ Re-run Benchmarks"),
                        );
                    modal_box = modal_box.child(bench_footer);
                }
            }
        }

        overlay.child(modal_box)
    }
}

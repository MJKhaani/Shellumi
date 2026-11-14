//! Diagnostics actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_diagnostics_modal(&mut self, cx: &mut Context<Self>) {
        self.diagnostics_modal = Some(DiagnosticsModalState {
            active_tab: DiagnosticsModalTab::HealthChecks,
            report: None,
            benchmark_report: None,
            is_running: true,
            is_running_benchmarks: false,
            error: None,
        });
        cx.notify();
        self.run_diagnostics_checks(cx);
    }

    pub(crate) fn open_benchmarks_modal(&mut self, cx: &mut Context<Self>) {
        self.diagnostics_modal = Some(DiagnosticsModalState {
            active_tab: DiagnosticsModalTab::Benchmarks,
            report: None,
            benchmark_report: None,
            is_running: false,
            is_running_benchmarks: true,
            error: None,
        });
        cx.notify();
        self.run_benchmarks(cx);
    }

    pub(crate) fn dismiss_diagnostics_modal(&mut self, cx: &mut Context<Self>) {
        self.diagnostics_modal = None;
        cx.notify();
    }

    pub(crate) fn run_benchmarks(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.diagnostics_modal {
            m.is_running_benchmarks = true;
            m.error = None;
        }
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { BenchmarkRunner::run_suite(None) });

        cx.spawn(async move |app, cx| {
            let report = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(m) = &mut app.diagnostics_modal {
                    m.is_running_benchmarks = false;
                    m.benchmark_report = Some(report);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn run_diagnostics_checks(&mut self, cx: &mut Context<Self>) {
        let db_opt = self.database.clone();

        if let Some(ref mut m) = self.diagnostics_modal {
            m.is_running = true;
            m.error = None;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let mut runner = DiagnosticsRunner::new();
            if let Some(ref path) = db_opt {
                runner = runner.with_database_path(path);
            }
            runner.run_all()
        });

        cx.spawn(async move |app, cx| {
            let report = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(m) = &mut app.diagnostics_modal {
                    m.is_running = false;
                    m.report = Some(report);
                }
                cx.notify();
            });
        })
        .detach();
    }

    // =========================================================================
    // Mobile Touch & Virtual Key Simulator Engine
    // =========================================================================
}

//! Desktop overview page.
use super::*;

impl Shellumi {
    pub(super) fn overview_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let hosts = match &self.hosts {
            HostState::Loaded(hosts) => hosts.as_slice(),
            _ => &[],
        };
        let active = self
            .tabs
            .iter()
            .flat_map(|tab| &tab.panes)
            .filter(|p| p.session.is_some())
            .count();
        let metric_defs = [
            (
                "Saved hosts",
                hosts.len(),
                "⌘",
                MacTheme::BADGE_NAVY,
                MacTheme::ACCENT_BLUE,
            ),
            ("Open sessions", active, "●", 0x1a3a2a, MacTheme::SUCCESS),
            (
                "Saved workspaces",
                self.workspaces.len(),
                "▣",
                MacTheme::BADGE_DEEP_BLUE,
                MacTheme::ACCENT_BLUE,
            ),
            (
                "Command snippets",
                self.snippets.len(),
                "⚡",
                0x3d3010,
                MacTheme::WARNING,
            ),
        ];
        let mut metrics = div().flex().flex_wrap().gap_3();
        for (label, value, glyph, bg, fg) in metric_defs {
            metrics = metrics.child(
                panel().flex_1().min_w(px(140.0)).child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(36.0))
                                .rounded_md()
                                .bg(rgb(bg))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(fg))
                                .font_weight(FontWeight::BOLD)
                                .child(glyph),
                        )
                        .child(
                            div().flex().flex_col().child(muted(label)).child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child(value.to_string()),
                            ),
                        ),
                ),
            );
        }
        let mut host_list = panel()
            .flex_1()
            .flex()
            .flex_col()
            .gap_3()
            .child(heading("Saved connections"));
        for host in hosts.iter().take(6) {
            let host = host.clone();
            host_list = host_list.child(
                button(
                    ("overview-host", host.id as u64),
                    format!("⌘  {}", host.name),
                )
                .on_click(cx.listener(move |app, _, window, cx| app.connect(&host, window, cx))),
            );
        }
        if hosts.is_empty() {
            host_list = host_list.child(muted("Add your first host to get started."));
        }
        let actions = panel()
            .flex_1()
            .flex()
            .flex_col()
            .gap_3()
            .child(heading("Quick actions"))
            .child(
                primary("overview-new-host", "New host").on_click(cx.listener(|app, _, _, cx| {
                    app.page = None;
                    app.edit_host(None, cx);
                })),
            )
            .child(
                button("overview-local", "Open local terminal").on_click(
                    cx.listener(|app, _, window, cx| app.open_local_terminal(window, cx)),
                ),
            )
            .child(
                button("overview-new-snippet", "New snippet").on_click(cx.listener(
                    |app, _, _, cx| {
                        app.page = Some("Snippets");
                        app.edit_library_item(EditorKind::Snippet, None, cx);
                    },
                )),
            )
            .child(button("overview-workspaces", "Manage workspaces").on_click(
                cx.listener(|app, _, window, cx| app.navigate_page("Workspaces", window, cx)),
            ));
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Your infrastructure, in focus."),
            )
            .child(muted(
                "Manage connections, files and workflows from your local workspace.",
            ))
            .child(metrics)
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_start()
                    .child(host_list)
                    .child(actions),
            )
    }
}

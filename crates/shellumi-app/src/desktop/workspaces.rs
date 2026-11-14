//! Desktop workspaces page.
use super::*;

impl Shellumi {
    pub(super) fn workspaces_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut grid = div().flex().flex_wrap().gap_3();
        for (index, workspace) in self.workspaces.iter().enumerate() {
            let id = workspace.id;
            let name = workspace.name.clone();
            let duplicate = format!("{} copy", workspace.name);
            let glyph = match index % 4 {
                0 => "◉",
                1 => "⌘",
                2 => "▣",
                _ => "▧",
            };
            let mut preview = div()
                .flex()
                .flex_wrap()
                .gap_1()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .bg(rgb(MacTheme::CONTENT_BG));
            for session in &workspace.sessions {
                preview = preview.child(
                    div()
                        .w(px(140.0))
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .bg(rgb(MacTheme::SURFACE))
                        .text_xs()
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                        .child(session.name.clone()),
                );
            }
            grid = grid.child(
                panel()
                    .w(px(320.0))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(35.0))
                                    .h(px(35.0))
                                    .rounded_md()
                                    .bg(rgb(MacTheme::BADGE_DEEP_BLUE))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                    .font_weight(FontWeight::BOLD)
                                    .child(glyph),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(heading(workspace.name.clone()))
                                    .child(muted(workspace.description.clone().unwrap_or_else(
                                        || {
                                            format!(
                                                "{} sessions · {}",
                                                workspace.sessions.len(),
                                                workspace.layout
                                            )
                                        },
                                    ))),
                            ),
                    )
                    .child(preview)
                    .child(muted(format!(
                        "{} sessions · {}",
                        workspace.sessions.len(),
                        workspace.layout
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(primary(("open-workspace", id as u64), "Open").on_click(
                                cx.listener(move |app, _, window, cx| {
                                    app.load_workspace(id, window, cx)
                                }),
                            ))
                            .child(button(("edit-workspace", id as u64), "Edit").on_click(
                                cx.listener(move |app, _, _, cx| {
                                    app.edit_library_item(EditorKind::Workspace, Some(id), cx)
                                }),
                            ))
                            .child(
                                button(("duplicate-workspace", id as u64), "Duplicate").on_click(
                                    cx.listener(move |app, _, _, cx| {
                                        app.desktop_command(
                                            Command::DuplicateWorkspace {
                                                id,
                                                name: duplicate.clone(),
                                            },
                                            cx,
                                        )
                                    }),
                                ),
                            )
                            .child(button(("delete-workspace", id as u64), "Delete").on_click(
                                cx.listener(move |app, _, _, cx| {
                                    app.desktop.confirm = Some((
                                        format!("Delete workspace '{name}'?"),
                                        Command::DeleteWorkspace(id),
                                    ));
                                    cx.notify();
                                }),
                            )),
                    ),
            );
        }
        if self.workspaces.is_empty() {
            grid=grid.child(empty("Build your workspace","Save named layouts containing local shells and SSH hosts. Opening a workspace starts new sessions."));
        }
        let action_bar = div()
            .id("workspaces-action-bar")
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(heading("Saved workspaces"))
            .child(primary("new-workspace", "+ New workspace").on_click(
                cx.listener(|app, _, _, cx| app.edit_library_item(EditorKind::Workspace, None, cx)),
            ));

        let content_scroll = div()
            .id("workspaces-content-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .child(grid);

        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::CONTENT_BG))
            .child(action_bar)
            .child(content_scroll)
    }
}

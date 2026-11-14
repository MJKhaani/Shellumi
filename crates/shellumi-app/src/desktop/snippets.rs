//! Desktop snippets page.
use super::*;

impl Shellumi {
    pub(super) fn snippets_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let action_bar = div()
            .id("snippets-action-bar")
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(heading("Command snippets"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button_icon("import-snippets", "import", "Import package").on_click(
                            cx.listener(|app, _, _, cx| {
                                app.edit_library_item(EditorKind::Import, None, cx)
                            }),
                        ),
                    )
                    .child(
                        button_icon("new-snippet", "edit", "+ New snippet")
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .border_color(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.edit_library_item(EditorKind::Snippet, None, cx)
                            })),
                    ),
            );
        let mut packages = panel()
            .w(px(170.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        if let Some(p) = crate::resolve_icon_path("tags", false, 32) {
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        },
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("PACKAGES"),
                    ),
            )
            .child(
                button("all-snippets", "All snippets")
                    .bg(if self.desktop.snippet_tag.is_none() {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_color(if self.desktop.snippet_tag.is_none() {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.desktop.snippet_tag = None;
                        cx.notify();
                    })),
            );
        let mut tags: Vec<_> = self.snippets.iter().flat_map(|s| s.tags.iter()).collect();
        tags.sort();
        tags.dedup();
        for (index, tag) in tags.iter().enumerate() {
            let tag = (*tag).clone();
            let selected_tag = tag.clone();
            let is_sel = self.desktop.snippet_tag.as_deref() == Some(&tag);
            packages = packages.child(
                button(("snippet-tag", index), tag)
                    .bg(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.desktop.snippet_tag = Some(selected_tag.clone());
                        cx.notify();
                    })),
            );
        }
        let query = self.desktop.search.read(cx).value().trim().to_lowercase();
        let snippets: Vec<_> = self
            .snippets
            .iter()
            .filter(|s| {
                self.desktop
                    .snippet_tag
                    .as_ref()
                    .is_none_or(|tag| s.tags.contains(tag))
                    && (query.is_empty()
                        || s.name.to_lowercase().contains(&query)
                        || s.command.to_lowercase().contains(&query))
            })
            .collect();
        let selected = snippets
            .iter()
            .copied()
            .find(|s| Some(s.id) == self.desktop.selected_snippet)
            .or_else(|| snippets.first().copied());
        let mut list = panel()
            .w(px(220.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.desktop.search.clone());
        for snippet in snippets {
            let id = snippet.id;
            let is_sel = selected.is_some_and(|s| s.id == id);
            list = list.child(
                button(("snippet", id as u64), snippet.name.clone())
                    .bg(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.desktop.selected_snippet = Some(id);
                        cx.notify();
                    })),
            );
        }
        let mut editor = panel().flex_1().min_w_0().flex().flex_col().gap_3();
        if let Some(snippet) = selected {
            let id = snippet.id;
            let name = snippet.name.clone();
            let command = snippet.command.clone();
            let export = serde_json::to_string_pretty(&vec![shellumi_core::Snippet {
                name: snippet.name.clone(),
                command: snippet.command.clone(),
                description: snippet.description.clone(),
                tags: snippet.tags.clone(),
                variables: snippet.variables.clone(),
            }])
            .unwrap_or_default();
            editor =
                editor
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(heading(snippet.name.clone()))
                            .child(button_icon("edit-snippet", "edit", "Edit").on_click(
                                cx.listener(move |app, _, _, cx| {
                                    app.edit_library_item(EditorKind::Snippet, Some(id), cx)
                                }),
                            )),
                    )
                    .child(muted(snippet.description.clone().unwrap_or_default()))
                    .child(muted(snippet.tags.join(" · ")))
                    .child(
                        div()
                            .min_h(px(260.0))
                            .p_4()
                            .rounded_lg()
                            .bg(rgb(MacTheme::CODE_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .font_family(MONOSPACE_FONT)
                            .text_sm()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .children(snippet.command.lines().enumerate().map(|(i, line)| {
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(28.0))
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child(format!("{}", i + 1)),
                                    )
                                    .child(
                                        div()
                                            .text_color(if i == 0 && line.starts_with('#') {
                                                rgb(MacTheme::SUCCESS)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
                                            })
                                            .child(line.to_owned()),
                                    )
                            })),
                    )
                    .child(muted(format!(
                        "{} template parameters · execution always requires your action",
                        snippet.variables.len()
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(button("copy-snippet", "Copy").on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        command.clone(),
                                    ))
                                },
                            )))
                            .child(button("export-snippet", "Copy JSON package").on_click(
                                cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        export.clone(),
                                    ))
                                }),
                            ))
                            .child(button("paste-snippet", "Paste into terminal").on_click(
                                cx.listener(move |app, _, window, cx| {
                                    if app.active_tab_idx.is_none() {
                                        app.desktop.message =
                                            Some("Open a terminal session first.".into());
                                        cx.notify();
                                    } else {
                                        app.page = None;
                                        app.insert_snippet(id, false, cx);
                                        window.focus(&app.focus);
                                    }
                                }),
                            ))
                            .child(primary("run-snippet", "Run…").on_click(cx.listener(
                                move |app, _, _, cx| {
                                    if app.active_tab_idx.is_none() {
                                        app.desktop.message = Some(
                                            "Open a terminal session before running a snippet."
                                                .into(),
                                        );
                                        cx.notify();
                                    } else {
                                        app.open_snippet_modal_for_run(id, cx);
                                    }
                                },
                            )))
                            .child(
                                button("delete-snippet", "Delete")
                                    .border_color(rgb(MacTheme::DANGER))
                                    .text_color(rgb(MacTheme::DANGER))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.desktop.confirm = Some((
                                            format!("Delete snippet '{name}'?"),
                                            Command::DeleteSnippet(id),
                                        ));
                                        cx.notify();
                                    })),
                            ),
                    );
        } else {
            editor = editor.child(heading("Your command library")).child(muted(
                "Create a snippet to save commands, scripts and parameterized templates.",
            ));
        }
        let content_scroll = div()
            .id("snippets-content-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_3()
                    .child(packages)
                    .child(list)
                    .child(editor),
            );

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
    pub(super) fn open_snippet_modal_for_run(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(snippet) = self.snippets.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        self.page = None;
        self.open_snippet_modal(&snippet, true, cx);
    }
}

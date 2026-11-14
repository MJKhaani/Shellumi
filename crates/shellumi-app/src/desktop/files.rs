//! Desktop files page and its actions.
use super::*;

impl Shellumi {
    pub(super) fn refresh_local_files(&mut self, cx: &mut Context<Self>) {
        let path = self.desktop.file_path.read(cx).value().to_owned();
        let hidden = self.desktop.preferences.show_hidden_files;
        let task = cx.background_executor().spawn(async move {
            shellumi_core::list_local_directory(&path, hidden).map_err(|e| e.to_string())
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok(entries) => {
                        app.desktop.local_entries = entries;
                        app.desktop.message = None;
                    }
                    Err(error) => app.desktop.message = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    #[allow(dead_code)]
    pub(super) fn files_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut local = panel()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(heading("Local files"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(self.desktop.file_path.clone()))
                    .child(
                        button("local-path-open", "Open")
                            .on_click(cx.listener(|app, _, _, cx| app.refresh_local_files(cx))),
                    )
                    .child(button("local-path-parent", "↑").on_click(cx.listener(
                        |app, _, _, cx| {
                            let path =
                                std::path::PathBuf::from(app.desktop.file_path.read(cx).value());
                            if let Some(parent) = path.parent() {
                                let parent = parent.to_string_lossy().into_owned();
                                app.desktop
                                    .file_path
                                    .update(cx, |input, cx| input.set_value(&parent, cx));
                                app.refresh_local_files(cx);
                            }
                        },
                    ))),
            )
            .child(
                button(
                    "local-files-hidden",
                    if self.desktop.preferences.show_hidden_files {
                        "Hide hidden files"
                    } else {
                        "Show hidden files"
                    },
                )
                .on_click(cx.listener(|app, _, _, cx| app.change_preference("hidden", 0, cx))),
            );
        for (index, entry) in self.desktop.local_entries.iter().enumerate() {
            let path = entry.path.clone();
            let directory = entry.kind == FileKind::Directory;
            local = local.child(
                button(
                    ("local-file", index),
                    format!(
                        "{} {}     {}",
                        if directory { "📁" } else { "◧" },
                        entry.name,
                        if directory {
                            "—".to_string()
                        } else {
                            format!("{} B", entry.size)
                        }
                    ),
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    if directory {
                        app.desktop
                            .file_path
                            .update(cx, |input, cx| input.set_value(&path, cx));
                        app.refresh_local_files(cx);
                    } else {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(path.clone()));
                        app.desktop.message = Some(
                            "Local file path copied. Use put in the SFTP session to upload.".into(),
                        );
                        cx.notify();
                    }
                })),
            );
        }
        let remote = panel()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(heading("Remote connections"))
            .child(muted(
                "Select a host to open its live SFTP terminal and graphical dual-pane browser.",
            ))
            .child(self.sessions_page(true, cx));
        div()
            .flex()
            .items_start()
            .gap_4()
            .child(local)
            .child(remote)
    }
}

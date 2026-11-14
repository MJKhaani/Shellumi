//! SFTP overlays rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_sftp_context_menu_overlay(
        &self,
        ctx_menu: &SftpContextMenuState,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let is_left = ctx_menu.is_left;
        let entry_name = ctx_menu.entry_name.clone();
        let entry_path = ctx_menu.entry_path.clone();
        let is_dir = ctx_menu.is_dir;

        let left_offset = if ctx_menu.cursor_x > 0.0 {
            px((ctx_menu.cursor_x - 10.0).clamp(16.0, 950.0))
        } else if is_left {
            px(120.0)
        } else {
            px(600.0)
        };
        let top_offset = if ctx_menu.cursor_y > 0.0 {
            px((ctx_menu.cursor_y - 10.0).clamp(60.0, 550.0))
        } else {
            px(120.0 + (ctx_menu.target_idx as f32 * 36.0).min(380.0))
        };

        let has_item = !entry_name.is_empty();

        div()
            .id("sftp-context-menu-backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000022))
            .on_click(cx.listener(|app, _, _, cx| {
                app.sftp_close_context_menu(cx);
            }))
            .child(
                div()
                    .id("sftp-context-menu-popup")
                    .absolute()
                    .top(top_offset)
                    .left(left_offset)
                    .w(px(220.0))
                    .rounded_lg()
                    .bg(rgb(MacTheme::SURFACE))
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .p_1p5()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .when(has_item, |this| {
                        this.child(
                            div()
                                .id("sftp-ctx-copy")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let entry_name = entry_name.clone();
                                    move |app, _, _, cx| {
                                        let files = {
                                            let p = if is_left {
                                                &app.sftp_left_pane
                                            } else {
                                                &app.sftp_right_pane
                                            };
                                            if p.selected_files.contains(&entry_name)
                                                && p.selected_files.len() > 1
                                            {
                                                p.selected_files.iter().cloned().collect()
                                            } else {
                                                vec![entry_name.clone()]
                                            }
                                        };
                                        app.sftp_transfer_files(is_left, files, cx);
                                    }
                                }))
                                .child("Copy to target directory"),
                        )
                        .child(
                            div()
                                .id("sftp-ctx-rename")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let entry_name = entry_name.clone();
                                    let entry_path = entry_path.clone();
                                    move |app, _, _, cx| {
                                        app.sftp_open_rename_modal(
                                            is_left,
                                            entry_name.clone(),
                                            entry_path.clone(),
                                            cx,
                                        );
                                    }
                                }))
                                .child("Rename"),
                        )
                        .child(
                            div()
                                .id("sftp-ctx-delete")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::DANGER))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let entry_name = entry_name.clone();
                                    let entry_path = entry_path.clone();
                                    move |app, _, _, cx| {
                                        app.sftp_open_delete_modal(
                                            is_left,
                                            entry_name.clone(),
                                            entry_path.clone(),
                                            is_dir,
                                            cx,
                                        );
                                    }
                                }))
                                .child("Delete"),
                        )
                    })
                    .child(
                        div()
                            .id("sftp-ctx-refresh")
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_close_context_menu(cx);
                                app.refresh_sftp_pane(is_left, cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(
                        div()
                            .id("sftp-ctx-new-folder")
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_open_new_folder_modal(is_left, cx);
                            }))
                            .child("New Folder"),
                    )
                    .when(has_item, |this| {
                        this.child(
                            div()
                                .id("sftp-ctx-permissions")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let entry_name = entry_name.clone();
                                    let entry_path = entry_path.clone();
                                    move |app, _, _, cx| {
                                        app.sftp_open_permissions_modal(
                                            is_left,
                                            entry_name.clone(),
                                            entry_path.clone(),
                                            cx,
                                        );
                                    }
                                }))
                                .child("Edit Permissions"),
                        )
                    })
                    .when(!has_item, |this| {
                        let show_hidden = {
                            let p = if is_left {
                                &self.sftp_left_pane
                            } else {
                                &self.sftp_right_pane
                            };
                            p.show_hidden
                        };
                        this.child(
                            div()
                                .id("sftp-ctx-toggle-hidden")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.sftp_close_context_menu(cx);
                                    app.sftp_toggle_pane_hidden(is_left, cx);
                                }))
                                .child(if show_hidden {
                                    "Hide Hidden Files"
                                } else {
                                    "Show Hidden Files"
                                }),
                        )
                    }),
            )
    }

    pub(crate) fn render_sftp_modal_overlay(
        &self,
        modal: &SftpModalState,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (title, content, btn_label, is_danger) = match &modal.action {
            SftpModalAction::Rename {
                entry_name, input, ..
            } => (
                format!("Rename '{entry_name}'"),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Enter a new name:"),
                    )
                    .child(
                        div()
                            .w_full()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .px_3()
                            .py_1p5()
                            .child(input.clone()),
                    ),
                "Rename",
                false,
            ),
            SftpModalAction::Delete {
                entry_name, is_dir, ..
            } => (
                format!("Delete '{}'?", entry_name),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(format!(
                                "Are you sure you want to permanently delete this {}?",
                                if *is_dir { "folder" } else { "file" }
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("This action cannot be undone."),
                    ),
                "Delete",
                true,
            ),
            SftpModalAction::NewFolder { input } => (
                "New Folder".to_string(),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Folder name:"),
                    )
                    .child(
                        div()
                            .w_full()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .px_3()
                            .py_1p5()
                            .child(input.clone()),
                    ),
                "Create Folder",
                false,
            ),
            SftpModalAction::EditPermissions {
                entry_name, input, ..
            } => (
                format!("Edit Permissions for '{entry_name}'"),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Enter octal mode (e.g. 0755, 0644):"),
                    )
                    .child(
                        div()
                            .w_full()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .px_3()
                            .py_1p5()
                            .child(input.clone()),
                    ),
                "Apply Permissions",
                false,
            ),
        };

        let mut card = div()
            .id("sftp-modal-card")
            .w(px(380.0))
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .p_5()
            .shadow_xl()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(title),
            )
            .child(content);

        if let Some(err) = &modal.error {
            card = card.child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(err.clone()),
            );
        }

        div()
            .id("sftp-modal-backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000aa))
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .child(
                card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .child(
                            div()
                                .id("sftp-modal-cancel-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::BORDER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_close_modal(cx);
                                }))
                                .child("Cancel"),
                        )
                        .child(
                            div()
                                .id("sftp-modal-submit-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(if is_danger {
                                    rgb(MacTheme::DANGER)
                                } else {
                                    rgb(MacTheme::ACCENT_BLUE)
                                })
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .hover(|s| {
                                    if is_danger {
                                        s.bg(rgb(0xdc2626))
                                    } else {
                                        s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                    }
                                })
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_submit_modal(cx);
                                }))
                                .child(btn_label),
                        ),
                ),
            )
    }
}

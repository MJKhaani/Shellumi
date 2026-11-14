//! Desktop backup page and its actions.
use super::*;

impl Shellumi {
    pub(super) fn backup_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let sync_badge = match &self.sync_status {
            AppSyncStatus::LocalOnly => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgb(0x38383a))
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child("Local only"),
            AppSyncStatus::UpToDate { .. } => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgba(0x30d15826))
                .text_xs()
                .text_color(rgb(MacTheme::SUCCESS))
                .child("Up to date"),
            AppSyncStatus::Syncing => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgba(0x0a84ff26))
                .text_xs()
                .text_color(rgb(MacTheme::ACCENT_BLUE))
                .child("Syncing…"),
            AppSyncStatus::Conflict { message } => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgba(0xffd60a26))
                .text_xs()
                .text_color(rgb(MacTheme::WARNING))
                .child(format!("Conflict: {message}")),
            AppSyncStatus::Error(e) => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgba(0xff453a26))
                .text_xs()
                .text_color(rgb(MacTheme::DANGER))
                .child(format!("Error: {e}")),
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                panel()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(heading("Google Drive Cloud Synchronization"))
                            .child(sync_badge),
                    )
                    .child(muted(
                        "End-to-end encrypted synchronization over HTTPS directly to your private Google Drive appDataFolder via system HTTP transport. All records are encrypted with AES-256-GCM before transport; Google cannot inspect your data.",
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                primary("trigger-gdrive-sync", "Sync with Google Drive now")
                                    .on_click(cx.listener(|app, _, _, cx| app.trigger_sync(cx))),
                            ),
                    ),
            )
            .child(
                panel()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(heading("Encrypted Backups"))
                    .child(muted(
                        "Export your local vault, or restore an existing encrypted backup. Only ciphertext is written to the backup file.",
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                primary("export-vault-backup", "Export encrypted backup…")
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.edit_library_item(EditorKind::ExportBackup, None, cx)
                                    })),
                            )
                            .child(
                                button("restore-vault-backup", "Restore encrypted backup…")
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.edit_library_item(EditorKind::RestoreBackup, None, cx)
                                    })),
                            ),
                    ),
            )
    }
}

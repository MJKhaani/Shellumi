//! Interactive Unlock Vault modal dialog.
use super::*;

impl Shellumi {
    pub(crate) fn render_unlock_vault_modal(
        &self,
        modal: &UnlockVaultModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("unlock-vault-modal-overlay")
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
            .id("unlock-vault-modal-container")
            .w(px(520.0))
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
                div()
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .child(div().text_xl().child("🔐"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child(format!("Unlock Vault — {}", modal.vault_name)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child("Enter the 12+ character recovery passphrase to unlock this vault."),
                            ),
                    ),
            )
            .child(
                div()
                    .id("btn-close-unlock-modal")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.dismiss_unlock_vault_modal(cx);
                    }))
                    .child("✕"),
            );

        // Body with Passphrase Input
        let body = div().flex().flex_col().gap_3().child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Recovery Passphrase"),
                )
                .child(
                    div()
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .rounded_md()
                        .p_2()
                        .bg(rgb(MacTheme::SURFACE_ACTIVE))
                        .child(modal.passphrase_input.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child(
                        "The passphrase wraps the AES-256-GCM vault key using PBKDF2-HMAC-SHA256.",
                    ),
                ),
        );

        modal_box = modal_box.child(header).child(body);

        // Error message if present
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_2p5()
                    .rounded_md()
                    .bg(rgba(0xef444422))
                    .border_1()
                    .border_color(rgba(0xef444466))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child("⚠️")
                    .child(err.clone()),
            );
        }

        // Actions Footer
        let is_submitting = modal.is_submitting;
        let footer = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .pt_2()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .id("btn-cancel-unlock")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .bg(rgb(MacTheme::SURFACE_ACTIVE))
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.dismiss_unlock_vault_modal(cx);
                    }))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("btn-submit-unlock")
                    .px_4()
                    .py_1p5()
                    .rounded_md()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .bg(if is_submitting {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        rgb(MacTheme::ACCENT_BLUE)
                    })
                    .text_color(rgb(0xffffff))
                    .hover(|s| {
                        if !is_submitting {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s
                        }
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if !is_submitting {
                            app.submit_unlock_vault(cx);
                        }
                    }))
                    .child(if is_submitting {
                        "Unlocking…"
                    } else {
                        "Unlock Vault"
                    }),
            );

        modal_box = modal_box.child(footer);
        overlay.child(modal_box)
    }
}

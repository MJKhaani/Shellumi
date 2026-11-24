//! Interactive Unlock Vault modal dialog using standardized safety alert.
use super::*;
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::safety_alert::{alert_backdrop, security_challenge_dialog};

impl Shellumi {
    pub(crate) fn render_unlock_vault_modal(
        &self,
        modal: &UnlockVaultModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let remember = modal.remember_in_keychain;
        let is_submitting = modal.is_submitting;

        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Vault Password or Recovery Code"),
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
                                "The password unwraps the AES-256-GCM vault DEK using Argon2id (64 MiB, 3 iterations).",
                            ),
                    ),
            )
            .child(
                div()
                    .id("toggle-unlock-remember-keychain")
                    .flex()
                    .items_center()
                    .gap_2()
                    .pt_1()
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.toggle_unlock_remember_keychain(cx);
                    }))
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(if remember {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SURFACE_ACTIVE)
                            })
                            .text_color(rgb(0xffffff))
                            .text_xs()
                            .child(if remember { "✓" } else { "" }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Remember on this device (stores DEK in OS Keychain)"),
                    ),
            );

        if let Some(err) = &modal.error {
            body = body.child(
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

        let cancel_btn = AppButton::new("btn-cancel-unlock", "Cancel", ButtonVariant::Ghost)
            .on_click(cx.listener(|app, _, _, cx| {
                app.dismiss_unlock_vault_modal(cx);
            }));

        let submit_btn = if is_submitting {
            AppButton::new("btn-submit-unlock", "Unlocking…", ButtonVariant::Secondary)
        } else {
            AppButton::new("btn-submit-unlock", "Unlock Vault", ButtonVariant::Primary).on_click(
                cx.listener(|app, _, _, cx| {
                    app.submit_unlock_vault(cx);
                }),
            )
        };

        let actions = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(cancel_btn)
            .child(submit_btn);

        let card = security_challenge_dialog(
            format!("Unlock Vault — {}", modal.vault_name),
            "Enter the password or 32-character recovery code to unlock this vault.",
            body,
            actions,
        )
        .id("unlock-vault-modal-container");

        alert_backdrop(card)
            .id("unlock-vault-modal-overlay")
            .occlude()
    }
}

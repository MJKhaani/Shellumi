//! Remote Server and Local Vault creation modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_create_vault_modal(
        &self,
        modal: &CreateVaultModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("create-vault-modal-overlay")
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
            .id("create-vault-modal-container")
            .w(px(640.0))
            .max_h(px(720.0))
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
            .gap_4();

        // -------------------------------------------------------------------
        // Header
        // -------------------------------------------------------------------
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
                    .child(if let Some(p) = resolve_icon_path("vaults", false, 48) {
                        div()
                            .w(px(22.0))
                            .h(px(22.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().child("🔒").into_any_element()
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Create or Connect Vault"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(
                                    "Local isolated storage or end-to-end encrypted remote sync",
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .id("dismiss-create-vault-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.dismiss_create_vault_modal(cx);
                    }))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // -------------------------------------------------------------------
        // Status & Error Messages
        // -------------------------------------------------------------------
        if let Some(ref err) = modal.error {
            modal_box = modal_box.child(
                div()
                    .id("create-vault-modal-error")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("⚠ {err}")),
            );
        }
        if let Some(ref msg) = modal.status_message {
            modal_box = modal_box.child(
                div()
                    .id("create-vault-modal-status")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x163820))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(format!("✓ {msg}")),
            );
        }

        // -------------------------------------------------------------------
        // Segmented Tab Selector
        // -------------------------------------------------------------------
        let is_remote = modal.active_tab == CreateVaultModalTab::RemoteServer;
        let tabs = div()
            .flex()
            .gap_1()
            .p_1()
            .rounded_lg()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .id("tab-remote-server-vault")
                    .flex_1()
                    .py_1p5()
                    .rounded_md()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1p5()
                    .bg(if is_remote {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        rgba(0x00000000)
                    })
                    .text_color(if is_remote {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.set_create_vault_tab(CreateVaultModalTab::RemoteServer, cx);
                    }))
                    .child("🌐 Remote Server Vault"),
            )
            .child(
                div()
                    .id("tab-local-vault")
                    .flex_1()
                    .py_1p5()
                    .rounded_md()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_1p5()
                    .bg(if !is_remote {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        rgba(0x00000000)
                    })
                    .text_color(if !is_remote {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.set_create_vault_tab(CreateVaultModalTab::Local, cx);
                    }))
                    .child("💻 Local Vault"),
            );
        modal_box = modal_box.child(tabs);

        // -------------------------------------------------------------------
        // Tab Content
        // -------------------------------------------------------------------
        match modal.active_tab {
            CreateVaultModalTab::RemoteServer => {
                let mut form = div().flex().flex_col().gap_3();

                // Explanatory Banner
                form = form.child(
                    div()
                        .p_2p5()
                        .rounded_md()
                        .bg(rgb(0x182438))
                        .border_1()
                        .border_color(rgb(MacTheme::ACCENT_BLUE_MUTED))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Connect to a self-hosted or managed Shellumi Server. All items are encrypted on this device using zero-knowledge encryption before transmission."),
                );

                // Server URL Input
                form = form.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("SERVER URL"),
                        )
                        .child(modal.server_url_input.clone()),
                );

                // Credentials: Username & Password in a 2-column row
                let creds_row = div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("USERNAME OR EMAIL"),
                            )
                            .child(modal.username_input.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("PASSWORD"),
                            )
                            .child(modal.password_input.clone()),
                    );
                form = form.child(creds_row);

                // Device Name & Vault Name in a 2-column row
                let names_row = div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("FRIENDLY DEVICE NAME"),
                            )
                            .child(modal.device_name_input.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("VAULT NAME"),
                            )
                            .child(modal.vault_name_input.clone()),
                    );
                form = form.child(names_row);

                // Zero-Knowledge Passphrase
                form = form.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child("ZERO-KNOWLEDGE PASSPHRASE"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                        .child("Min 12 chars • Never sent to server"),
                                ),
                        )
                        .child(modal.vault_passphrase_input.clone()),
                );

                // Footer Actions
                let is_testing = modal.is_testing;
                let is_submitting = modal.is_submitting;

                let footer = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_2()
                    .border_t_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .child(
                        div()
                            .id("test-server-connection-btn")
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::ACCENT_BLUE))
                            .bg(rgb(0x182438))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x223652)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.test_remote_server_connection(cx);
                            }))
                            .child(if is_testing {
                                "Testing… ⏳"
                            } else {
                                "⚡ Test Server Connection"
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .id("cancel-create-vault-btn")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::SEPARATOR))
                                    .bg(rgb(0x323236))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.dismiss_create_vault_modal(cx);
                                    }))
                                    .child("Cancel"),
                            )
                            .child(
                                div()
                                    .id("submit-remote-vault-btn")
                                    .px_4()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.submit_create_vault(cx);
                                    }))
                                    .child(if is_submitting {
                                        "Connecting & Enrolling… ⏳"
                                    } else {
                                        "Connect & Create Vault"
                                    }),
                            ),
                    );

                form = form.child(footer);
                modal_box = modal_box.child(form);
            }
            CreateVaultModalTab::Local => {
                let mut form = div().flex().flex_col().gap_3();

                // Explanatory Banner
                form = form.child(
                    div()
                        .p_2p5()
                        .rounded_md()
                        .bg(rgb(0x28282c))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Create an isolated local vault stored strictly on this machine. Data remains in the local Turso database and is never transmitted over the network."),
                );

                // Vault Name Input
                form = form.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("VAULT NAME"),
                        )
                        .child(modal.vault_name_input.clone()),
                );

                // Description Input
                form = form.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("DESCRIPTION (OPTIONAL)"),
                        )
                        .child(modal.local_vault_desc_input.clone()),
                );

                // Footer Actions
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
                            .id("cancel-create-vault-btn-local")
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(0x323236))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.dismiss_create_vault_modal(cx);
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("submit-local-vault-btn")
                            .px_4()
                            .py_1p5()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.submit_create_vault(cx);
                            }))
                            .child(if is_submitting {
                                "Creating… ⏳"
                            } else {
                                "+ Create Local Vault"
                            }),
                    );

                form = form.child(footer);
                modal_box = modal_box.child(form);
            }
        }

        overlay.child(modal_box).into_any_element()
    }
}

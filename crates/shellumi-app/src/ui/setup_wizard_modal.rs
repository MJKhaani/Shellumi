//! Native GPUI First-Run Setup Wizard modal dialog rendering.
use super::*;
use gpui::Div;
use shellumi_core::setup::{InstallationStatus, SetupStep, SetupWizardMode};

impl Shellumi {
    pub(crate) fn render_setup_wizard_modal(
        &self,
        wizard: &SetupWizardModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("setup-wizard-modal-overlay")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgb(MacTheme::WINDOW_BG))
            .flex()
            .flex_col()
            .p_0();

        let mut modal_box = div()
            .id("setup-wizard-container")
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .p_6()
            .gap_4();

        // -------------------------------------------------------------------
        // 1. Wizard Header & Stepper
        // -------------------------------------------------------------------
        let current_step = wizard.state.current_step;
        let steps = [
            (SetupStep::Welcome, "1. Welcome"),
            (SetupStep::DefaultVault, "2. Default Vault"),
            (SetupStep::GoogleDrive, "3. Google Drive"),
            (SetupStep::AdditionalVaults, "4. Additional Vaults"),
            (SetupStep::ReviewAndFinish, "5. Review & Finish"),
        ];

        let mut stepper = div()
            .id("wizard-stepper-bar")
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .gap_1();

        for (step, label) in steps {
            let is_current = current_step == step;
            let is_completed = current_step > step;

            let pill_el = if is_current {
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::ACCENT_BLUE))
                    .text_color(rgb(0xffffff))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .child(label)
            } else if is_completed {
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::SUCCESS))
                    .text_color(rgb(0x101317))
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("✓ {}", &label[3..]))
            } else {
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::SEPARATOR_SUBTLE))
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .text_xs()
                    .child(label)
            };

            stepper = stepper.child(pill_el);
        }

        modal_box = modal_box.child(stepper);

        // -------------------------------------------------------------------
        // 2. Step Body Content
        // -------------------------------------------------------------------
        let body = match current_step {
            SetupStep::Welcome => self.render_wizard_welcome(wizard, cx),
            SetupStep::DefaultVault => self.render_wizard_default_vault(wizard, cx),
            SetupStep::GoogleDrive => self.render_wizard_google_drive(wizard, cx),
            SetupStep::AdditionalVaults => self.render_wizard_additional_vaults(wizard, cx),
            SetupStep::ReviewAndFinish | SetupStep::Completed => {
                self.render_wizard_review_and_finish(wizard, cx)
            }
        };

        let mut scroll_content = div()
            .id("wizard-scroll-content")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .child(body);

        // -------------------------------------------------------------------
        // 3. Status & Error Messages
        // -------------------------------------------------------------------
        if let Some(err) = &wizard.error {
            scroll_content = scroll_content.child(
                div()
                    .id("wizard-error-banner")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgba(0xff453a22))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .child(format!("⚠ {err}")),
            );
        }

        if let Some(msg) = &wizard.status_message {
            scroll_content = scroll_content.child(
                div()
                    .id("wizard-status-banner")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgba(0x30d15822))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .text_color(rgb(MacTheme::SUCCESS))
                    .text_xs()
                    .child(format!("✓ {msg}")),
            );
        }

        modal_box = modal_box.child(scroll_content);

        // -------------------------------------------------------------------
        // 4. Navigation Footer
        // -------------------------------------------------------------------
        let mut footer = div()
            .id("wizard-navigation-footer")
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .pt_3()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR));

        // Left button: Back
        if current_step > SetupStep::Welcome && current_step < SetupStep::Completed {
            footer = footer.child(
                div()
                    .id("wizard-back-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::SEPARATOR_SUBTLE))
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .text_xs()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.wizard_prev_step(cx);
                    }))
                    .child("← Back"),
            );
        } else {
            footer = footer.child(div());
        }

        // Right actions
        let mut right_actions = div().flex().items_center().gap_2();

        match current_step {
            SetupStep::GoogleDrive => {
                right_actions = right_actions.child(
                    div()
                        .id("wizard-skip-drive-btn")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(MacTheme::SEPARATOR_SUBTLE))
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .text_xs()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_advance_to_step(SetupStep::AdditionalVaults, cx);
                        }))
                        .child("Skip Google Drive"),
                );
                right_actions = right_actions.child(
                    div()
                        .id("wizard-continue-drive-btn")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(MacTheme::ACCENT_BLUE))
                        .text_color(rgb(0xffffff))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_advance_to_step(SetupStep::AdditionalVaults, cx);
                        }))
                        .child("Continue →"),
                );
            }
            SetupStep::AdditionalVaults => {
                right_actions = right_actions.child(
                    div()
                        .id("wizard-skip-additional-btn")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(MacTheme::SEPARATOR_SUBTLE))
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .text_xs()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_advance_to_step(SetupStep::ReviewAndFinish, cx);
                        }))
                        .child("Skip Additional Vaults"),
                );
                right_actions = right_actions.child(
                    div()
                        .id("wizard-continue-additional-btn")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(MacTheme::ACCENT_BLUE))
                        .text_color(rgb(0xffffff))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_advance_to_step(SetupStep::ReviewAndFinish, cx);
                        }))
                        .child("Review Setup →"),
                );
            }
            SetupStep::DefaultVault => {
                let can_continue = wizard.state.default_vault_created
                    && (wizard.recovery_code_confirmed
                        || wizard.state.mode == SetupWizardMode::RestoreSetup);

                if can_continue {
                    right_actions = right_actions.child(
                        div()
                            .id("wizard-continue-default-vault-btn")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.wizard_advance_to_step(SetupStep::GoogleDrive, cx);
                            }))
                            .child("Continue →"),
                    );
                }
            }
            SetupStep::ReviewAndFinish | SetupStep::Completed => {
                let finish_label = if wizard.is_busy {
                    "Finalizing Setup…"
                } else {
                    "Finish & Launch Shellumi"
                };
                right_actions = right_actions.child(
                    div()
                        .id("wizard-finish-btn")
                        .px_4()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(MacTheme::SUCCESS))
                        .text_color(rgb(0x101317))
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x38e562)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_finish(cx);
                        }))
                        .child(finish_label),
                );
            }
            _ => {}
        }

        footer = footer.child(right_actions);
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }

    // -----------------------------------------------------------------------
    // Step 1: Welcome
    // -----------------------------------------------------------------------
    fn render_wizard_welcome(&self, wizard: &SetupWizardModalState, cx: &mut Context<Self>) -> Div {
        let mut view = div().flex().flex_col().gap_4();

        // Welcome Header
        view = view.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("Welcome to Shellumi"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Local-first, end-to-end encrypted remote infrastructure manager"),
                ),
        );

        // System Inspection Card
        let (status_badge, status_desc, is_warn) = match &wizard.installation_status {
            InstallationStatus::Fresh => (
                "✓ Fresh Installation",
                "No existing vaults found on this device. Ready for new setup.",
                false,
            ),
            InstallationStatus::IncompleteSetup { .. } => (
                "⚠ Incomplete Setup Detected",
                "A previous setup session was interrupted. You can resume safely where you left off.",
                true,
            ),
            InstallationStatus::LegacyDatabase { .. } => (
                "ℹ Legacy Database Detected",
                "Found an unmigrated shellumi.db. Safe migration will retain all hosts and credentials.",
                false,
            ),
            InstallationStatus::ExistingVaults { .. } => (
                "ℹ Existing Encrypted Vaults Found",
                "Encrypted vault databases exist on this machine. You can unlock them or restore a profile.",
                false,
            ),
        };

        view = view.child(
            div()
                .p_3()
                .rounded_lg()
                .bg(rgb(MacTheme::INPUT_BG))
                .border_1()
                .border_color(if is_warn {
                    rgb(MacTheme::WARNING)
                } else {
                    rgb(MacTheme::SEPARATOR)
                })
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_warn {
                            rgb(MacTheme::WARNING)
                        } else {
                            rgb(MacTheme::ACCENT_BLUE)
                        })
                        .child(status_badge),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child(status_desc),
                ),
        );

        // Safety Guarantee Banner
        view = view.child(
            div()
                .p_2()
                .rounded_md()
                .bg(rgba(0x0a84ff18))
                .border_1()
                .border_color(rgb(MacTheme::ACCENT_BLUE_MUTED))
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                .child(
                    "🛡 Safety Guarantee: Shellumi never overwrites or silently reinitializes existing user databases or manifests."
                ),
        );

        // Two Primary Choices
        let has_existing = matches!(
            wizard.installation_status,
            InstallationStatus::ExistingVaults { .. }
                | InstallationStatus::LegacyDatabase { .. }
                | InstallationStatus::IncompleteSetup { .. }
        );

        let new_setup_card = div()
            .id("wizard-choice-new-setup")
            .p_4()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE_HOVER))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_ACTIVE)))
            .on_click(cx.listener(|app, _, _, cx| {
                app.wizard_select_mode(SetupWizardMode::NewSetup, cx);
            }))
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("⚡ Create New Setup"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(
                        "Initialize a fresh installation with a newly generated encrypted Default Vault and random 256-bit DEK."
                    ),
            );

        let restore_setup_card = div()
            .id("wizard-choice-restore-setup")
            .p_4()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE_HOVER))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_ACTIVE)))
            .on_click(cx.listener(|app, _, _, cx| {
                app.wizard_select_mode(SetupWizardMode::RestoreSetup, cx);
            }))
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("🔄 Restore Existing Setup"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(if has_existing {
                        "Unlock existing local vaults, restore from Google Drive cloud snapshots, or migrate legacy data."
                    } else {
                        "Restore vaults from Google Drive cloud snapshots or existing backups."
                    }),
            );

        view.child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(new_setup_card)
                .child(restore_setup_card),
        )
    }

    // -----------------------------------------------------------------------
    // Step 2: Default Vault
    // -----------------------------------------------------------------------
    fn render_wizard_default_vault(
        &self,
        wizard: &SetupWizardModalState,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut view = div().flex().flex_col().gap_3();

        view = view.child(
            div()
                .text_base()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                .child("Step 2: Default Vault Encryption"),
        );
        view = view.child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(
                    "Every Shellumi installation requires a primary encrypted Default Vault with its own random 256-bit Data Encryption Key (DEK) and Argon2id key derivation."
                ),
        );

        // Interrupted Setup Recovery Code Confirmation State
        if wizard.interrupted_recovery_code {
            view = view.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::WARNING))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::WARNING))
                            .child("⚠ Interrupted Setup: Emergency Recovery Code Confirmation Required"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(
                                "Your vault was initialized in a previous session, but setup was interrupted before you backed up your recovery code. To prevent permanent lockouts, please enter your vault password below to unlock your vault and establish a confirmed recovery code before proceeding."
                            ),
                    )
                    .child(wizard.existing_unlock_input.clone())
                    .child(
                        div()
                            .id("wizard-unlock-confirm-recovery-btn")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.wizard_unlock_to_confirm_recovery_code(cx);
                            }))
                            .child("Unlock Vault & View Recovery Code"),
                    ),
            );
            return view;
        }

        if let Some(recovery_code) = &wizard.default_recovery_code {
            // Recovery Code Generated Display
            view = view.child(
                    div()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(MacTheme::INPUT_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::WARNING))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::WARNING))
                                .child("🔑 Emergency Recovery Code Generated"),
                        )
                        .child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(MacTheme::CODE_BG))
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(recovery_code.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child(
                                    "Save this recovery code in a secure location. If you forget your vault password, this code is the ONLY way to recover your vault."
                                ),
                        ),
                );

            // Confirmation checkbox
            let confirmed = wizard.recovery_code_confirmed;
            view = view.child(
                div()
                    .id("wizard-recovery-confirm-checkbox")
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.wizard_toggle_recovery_confirmed(cx);
                    }))
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded_sm()
                            .border_1()
                            .border_color(if confirmed {
                                rgb(MacTheme::SUCCESS)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if confirmed {
                                rgb(MacTheme::SUCCESS)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(0x101317))
                            .child(if confirmed { "✓" } else { "" }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("I have safely backed up my emergency recovery code"),
                    ),
            );

            if confirmed {
                view = view.child(
                    div()
                        .id("wizard-confirm-recovery-btn")
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(MacTheme::SUCCESS))
                        .text_color(rgb(0x101317))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x38e562)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_confirm_recovery_code(cx);
                        }))
                        .child(if wizard.state.google_drive_connected {
                            "Confirm & Continue →"
                        } else {
                            "Confirm & Continue to Google Drive →"
                        }),
                );
            }
            return view;
        }

        if wizard.state.mode == SetupWizardMode::NewSetup {
            // Password Inputs for New Vault
            view = view.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Choose a Strong Vault Password:"),
                    )
                    .child(wizard.default_password_input.clone()),
            );

            view = view.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Confirm Vault Password:"),
                    )
                    .child(wizard.default_password_confirm_input.clone()),
            );

            // Optional Keychain Checkbox (disabled by default)
            let keychain_checked = wizard.remember_in_keychain;
            view = view.child(
                    div()
                        .id("wizard-keychain-checkbox")
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_toggle_keychain(cx);
                        }))
                        .child(
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .rounded_sm()
                                .border_1()
                                .border_color(if keychain_checked {
                                    rgb(MacTheme::ACCENT_BLUE)
                                } else {
                                    rgb(MacTheme::SEPARATOR)
                                })
                                .bg(if keychain_checked {
                                    rgb(MacTheme::ACCENT_BLUE)
                                } else {
                                    rgb(MacTheme::INPUT_BG)
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(rgb(0xffffff))
                                .child(if keychain_checked { "✓" } else { "" }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(
                                    "Remember DEK in OS Keychain (disabled by default for maximum security)"
                                ),
                        ),
                );

            // Action button
            view = view.child(
                div()
                    .id("wizard-generate-default-vault-btn")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(MacTheme::ACCENT_BLUE))
                    .text_color(rgb(0xffffff))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.wizard_create_default_vault(cx);
                    }))
                    .child("Generate Encrypted Vault & Recovery Code"),
            );
        } else {
            // Restore Setup Mode: Choice between Google Drive Cloud Restore vs Local Vault Unlock
            let is_gdrive = wizard.restore_with_google_drive;

            view = view.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("wizard-restore-source-gdrive")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(if is_gdrive {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR_SUBTLE)
                            })
                            .text_color(if is_gdrive {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .text_xs()
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.wizard_set_restore_source(true, cx);
                            }))
                            .child("☁ Google Drive Cloud Restore"),
                    )
                    .child(
                        div()
                            .id("wizard-restore-source-local")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(if !is_gdrive {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR_SUBTLE)
                            })
                            .text_color(if !is_gdrive {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .text_xs()
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.wizard_set_restore_source(false, cx);
                            }))
                            .child("💻 Local Vault Unlock / Migration"),
                    ),
            );

            if is_gdrive {
                // Cloud restoration branch
                if !wizard.state.google_drive_connected {
                    view = view.child(
                        div()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child(
                                        "Connect to your Google Drive to discover encrypted cloud vault snapshots without needing keys or keychain items from your other device.",
                                    ),
                            )
                            .child(
                                div()
                                    .id("wizard-connect-google-drive-restore-btn")
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_color(rgb(0xffffff))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.wizard_connect_google_drive(cx);
                                    }))
                                    .child(if wizard.is_connecting_google {
                                        "Connecting in browser…"
                                    } else {
                                        "Connect Google Drive to Discover Cloud Vaults"
                                    }),
                            ),
                    );
                } else {
                    let mut cloud_card = div()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(MacTheme::INPUT_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::SUCCESS))
                        .flex()
                        .flex_col()
                        .gap_2();

                    cloud_card = cloud_card.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::SUCCESS))
                                    .child(format!(
                                        "✓ Google Drive Connected ({} cloud snapshot(s) discovered)",
                                        wizard.cloud_vaults.len()
                                    )),
                            )
                            .child(
                                div()
                                    .id("wizard-refresh-cloud-vaults-btn")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::SEPARATOR_SUBTLE))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .text_xs()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.wizard_fetch_cloud_vaults(cx);
                                    }))
                                    .child("Refresh"),
                            ),
                    );

                    for cv in &wizard.cloud_vaults {
                        let is_def = cv.is_default || cv.id == "default" || cv.id == "legacy-local";
                        cloud_card = cloud_card.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .p_2()
                                .rounded_md()
                                .bg(if is_def {
                                    rgba(0x0a84ff18)
                                } else {
                                    rgb(MacTheme::SURFACE_HOVER)
                                })
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(format!("{} (Vault ID: {})", cv.name, cv.id)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if is_def {
                                            rgb(MacTheme::ACCENT_BLUE)
                                        } else {
                                            rgb(MacTheme::TEXT_TERTIARY)
                                        })
                                        .child(if is_def {
                                            "Default Vault Target"
                                        } else {
                                            "Secondary Vault"
                                        }),
                                ),
                        );
                    }

                    cloud_card = cloud_card.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Enter Default Vault Password:"),
                            )
                            .child(wizard.cloud_vault_password_input.clone()),
                    );

                    // Optional Keychain Checkbox
                    let keychain_checked = wizard.remember_in_keychain;
                    cloud_card = cloud_card.child(
                        div()
                            .id("wizard-cloud-restore-keychain-checkbox")
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.wizard_toggle_keychain(cx);
                            }))
                            .child(
                                div()
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(if keychain_checked {
                                        rgb(MacTheme::ACCENT_BLUE)
                                    } else {
                                        rgb(MacTheme::SEPARATOR)
                                    })
                                    .bg(if keychain_checked {
                                        rgb(MacTheme::ACCENT_BLUE)
                                    } else {
                                        rgb(MacTheme::INPUT_BG)
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .child(if keychain_checked { "✓" } else { "" }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Remember DEK in OS Keychain (disabled by default)"),
                            ),
                    );

                    cloud_card = cloud_card.child(
                        div()
                            .id("wizard-restore-cloud-default-btn")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.wizard_restore_cloud_default_vault(cx);
                            }))
                            .child("Restore Default Vault from Google Drive"),
                    );

                    view = view.child(cloud_card);
                }
            } else {
                // Local unlock / migration branch
                match &wizard.installation_status {
                    InstallationStatus::LegacyDatabase { path } => {
                        view = view.child(
                            div()
                                .p_3()
                                .rounded_lg()
                                .bg(rgb(MacTheme::INPUT_BG))
                                .border_1()
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                                        .child("Legacy Database Migration"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(format!(
                                            "Shellumi detected an unmigrated database at {path}. Migration will convert your data into isolated physical encrypted vaults."
                                        )),
                                )
                                .child(wizard.legacy_password_input.clone())
                                .child(
                                    div()
                                        .id("wizard-migrate-legacy-btn")
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(rgb(MacTheme::ACCENT_BLUE))
                                        .text_color(rgb(0xffffff))
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                        .on_click(cx.listener(|app, _, _, cx| {
                                            app.wizard_migrate_legacy(cx);
                                        }))
                                        .child("Migrate Legacy Database"),
                                ),
                        );
                    }
                    _ => {
                        // Unlock Existing Local Default Vault
                        let is_recovery = wizard.unlock_with_recovery_code;
                        view = view.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .id("wizard-unlock-password-tab")
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(if !is_recovery {
                                            rgb(MacTheme::ACCENT_BLUE)
                                        } else {
                                            rgb(MacTheme::SEPARATOR_SUBTLE)
                                        })
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(|app, _, _, cx| {
                                            app.wizard_toggle_unlock_mode(cx);
                                        }))
                                        .child("Password"),
                                )
                                .child(
                                    div()
                                        .id("wizard-unlock-recovery-tab")
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(if is_recovery {
                                            rgb(MacTheme::ACCENT_BLUE)
                                        } else {
                                            rgb(MacTheme::SEPARATOR_SUBTLE)
                                        })
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(|app, _, _, cx| {
                                            app.wizard_toggle_unlock_mode(cx);
                                        }))
                                        .child("Recovery Code"),
                                ),
                        );

                        view = view.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(if is_recovery {
                                            "Enter your 32-character recovery code:"
                                        } else {
                                            "Enter your default vault password:"
                                        }),
                                )
                                .child(wizard.existing_unlock_input.clone()),
                        );

                        view = view.child(
                            div()
                                .id("wizard-unlock-existing-btn")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_color(rgb(0xffffff))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.wizard_unlock_existing_vault(cx);
                                }))
                                .child("Unlock Default Vault"),
                        );
                    }
                }
            }
        }

        view
    }

    // -----------------------------------------------------------------------
    // Step 3: Google Drive
    // -----------------------------------------------------------------------
    fn render_wizard_google_drive(
        &self,
        wizard: &SetupWizardModalState,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut view = div().flex().flex_col().gap_3();

        view = view.child(
            div()
                .text_base()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                .child("Step 3: Google Drive Synchronization (Optional)"),
        );
        view = view.child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(
                    "Synchronize encrypted vault snapshots directly to your personal Google Drive application data folder without third-party servers. No environment variables required."
                ),
        );

        if wizard.state.google_drive_connected {
            view = view.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(rgba(0x30d15822))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::SUCCESS))
                            .child("✓ Connected to Google Drive"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(
                            "OAuth 2.0 PKCE token bundle is securely stored in your OS Keychain.",
                        ),
                    ),
            );

            // Consent Checkbox
            let opt_in = wizard.sync_default_opt_in;
            view = view.child(
                div()
                    .id("wizard-sync-default-consent-checkbox")
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.wizard_toggle_drive_opt_in(cx);
                    }))
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded_sm()
                            .border_1()
                            .border_color(if opt_in {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if opt_in {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .child(if opt_in { "✓" } else { "" }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(
                                "Enable Google Drive synchronization for Default Vault (Explicit Consent)"
                            ),
                    ),
            );

            if wizard.is_fetching_snapshots {
                view = view.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Discovering cloud vault snapshots…"),
                );
            } else if !wizard.cloud_snapshots.is_empty() {
                let mut snaps = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Discovered Cloud Vault Snapshots:"),
                    );
                for snap in &wizard.cloud_snapshots {
                    snaps = snaps.child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(format!(
                                "• Vault ID: {} (schema v{})",
                                snap.vault_id, snap.schema_version
                            )),
                    );
                }
                view = view.child(snaps);
            }
        } else {
            // Connect Button Card
            view = view.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Authenticate with Google Drive via your default browser:"),
                    )
                    .child(
                        div()
                            .id("wizard-connect-google-drive-btn")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.wizard_connect_google_drive(cx);
                            }))
                            .child(if wizard.is_connecting_google {
                                "Connecting… Please authorize in browser"
                            } else {
                                "Connect with Google Drive"
                            }),
                    ),
            );
        }

        view
    }

    // -----------------------------------------------------------------------
    // Step 4: Additional Vaults
    // -----------------------------------------------------------------------
    fn render_wizard_additional_vaults(
        &self,
        wizard: &SetupWizardModalState,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut view = div().flex().flex_col().gap_3();

        view = view.child(
            div()
                .text_base()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                .child("Step 4: Additional Vaults & Integrations (Optional)"),
        );
        view = view.child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(
                    "You can configure additional isolated local vaults or connect a corporate Shellumi Server. You can also skip and add vaults later."
                ),
        );

        // Tab Selector
        let active_tab = wizard.additional_tab;
        view = view.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .id("wizard-tab-create-local")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(if active_tab == AdditionalVaultsTab::CreateLocal {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SEPARATOR_SUBTLE)
                        })
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .text_xs()
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_set_additional_tab(AdditionalVaultsTab::CreateLocal, cx);
                        }))
                        .child("Create Local Vault"),
                )
                .child(
                    div()
                        .id("wizard-tab-restore-local")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(if active_tab == AdditionalVaultsTab::RestoreLocal {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SEPARATOR_SUBTLE)
                        })
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .text_xs()
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_set_additional_tab(AdditionalVaultsTab::RestoreLocal, cx);
                        }))
                        .child("Restore Local Vault"),
                )
                .child(
                    div()
                        .id("wizard-tab-connect-server")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(if active_tab == AdditionalVaultsTab::ConnectServer {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SEPARATOR_SUBTLE)
                        })
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .text_xs()
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_set_additional_tab(AdditionalVaultsTab::ConnectServer, cx);
                        }))
                        .child("Connect Shellumi Server"),
                ),
        );

        match active_tab {
            AdditionalVaultsTab::CreateLocal => {
                let mut form = div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR));

                form = form.child(wizard.sec_vault_name_input.clone());
                form = form.child(wizard.sec_vault_password_input.clone());
                form = form.child(wizard.sec_vault_password_confirm_input.clone());

                // Checkboxes
                let sec_keychain = wizard.sec_vault_keychain;
                form = form.child(
                    div()
                        .id("sec-vault-keychain-toggle")
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_toggle_sec_keychain(cx);
                        }))
                        .child(
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .bg(if sec_keychain {
                                    rgb(MacTheme::ACCENT_BLUE)
                                } else {
                                    rgb(MacTheme::INPUT_BG)
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .child(if sec_keychain { "✓" } else { "" }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child("Remember in Keychain"),
                        ),
                );

                let sec_drive = wizard.sec_vault_drive_opt_in;
                form = form.child(
                    div()
                        .id("sec-vault-drive-toggle")
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_toggle_sec_drive_opt_in(cx);
                        }))
                        .child(
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .bg(if sec_drive {
                                    rgb(MacTheme::ACCENT_BLUE)
                                } else {
                                    rgb(MacTheme::INPUT_BG)
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .child(if sec_drive { "✓" } else { "" }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child("Enable Google Drive Sync for this secondary vault"),
                        ),
                );

                if let Some(rec_code) = &wizard.sec_recovery_code {
                    let sec_rec_confirmed = wizard.sec_recovery_confirmed;
                    form = form
                        .child(
                            div()
                                .p_3()
                                .rounded_md()
                                .bg(rgb(0x1a2e1a))
                                .border_1()
                                .border_color(rgb(0x38e562))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0x38e562))
                                        .child("Secondary Vault Recovery Code"),
                                )
                                .child(
                                    div()
                                        .font_family("Courier New")
                                        .text_sm()
                                        .text_color(rgb(0xffffff))
                                        .child(rec_code.clone()),
                                ),
                        )
                        .child(
                            div()
                                .id("sec-vault-rec-confirm-checkbox")
                                .flex()
                                .items_center()
                                .gap_2()
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.wizard_toggle_sec_recovery_confirmed(cx);
                                }))
                                .child(
                                    div()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(MacTheme::SEPARATOR))
                                        .bg(if sec_rec_confirmed {
                                            rgb(MacTheme::ACCENT_BLUE)
                                        } else {
                                            rgb(MacTheme::INPUT_BG)
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .child(if sec_rec_confirmed { "✓" } else { "" }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(
                                        "I have safely recorded this secondary vault recovery code",
                                    ),
                                ),
                        );
                }

                form = form.child(
                    div()
                        .id("wizard-create-sec-vault-btn")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(MacTheme::ACCENT_BLUE))
                        .text_color(rgb(0xffffff))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_create_secondary_vault(cx);
                        }))
                        .child("Create Secondary Vault"),
                );

                view = view.child(form);
            }
            AdditionalVaultsTab::RestoreLocal => {
                let mut form = div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR));

                form = form.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Restore an independently encrypted vault from Google Drive snapshots or existing local files using its individual password."),
                );

                if wizard.state.google_drive_connected && !wizard.cloud_vaults.is_empty() {
                    let mut cloud_sec_list = div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::ACCENT_BLUE))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                                        .child("Discovered Cloud Vaults (Google Drive):"),
                                )
                                .child(
                                    div()
                                        .id("wizard-refresh-sec-cloud-vaults-btn")
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(rgb(MacTheme::SEPARATOR_SUBTLE))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .text_xs()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                        .on_click(cx.listener(|app, _, _, cx| {
                                            app.wizard_fetch_cloud_vaults(cx);
                                        }))
                                        .child("Refresh"),
                                ),
                        );

                    for cv in &wizard.cloud_vaults {
                        let is_configured =
                            wizard.state.configured_vaults.iter().any(|v| v.id == cv.id)
                                || cv.id == "default"
                                || cv.id == "legacy-local";

                        let vault_id = cv.id.clone();
                        let vault_name = cv.name.clone();

                        let mut item = div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE_HOVER))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child(format!("{vault_name} ({vault_id})")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child(format!("{} records", cv.record_count)),
                                    ),
                            );

                        if is_configured {
                            item = item.child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::SUCCESS))
                                    .child("✓ Configured"),
                            );
                        } else {
                            let vid_for_click = vault_id.clone();
                            item = item.child(
                                div()
                                    .id(gpui::SharedString::from(format!(
                                        "restore-cloud-sec-{vid_for_click}"
                                    )))
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_color(rgb(0xffffff))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.wizard_restore_cloud_secondary_vault(
                                            vid_for_click.clone(),
                                            cx,
                                        );
                                    }))
                                    .child("Restore Vault"),
                            );
                        }

                        cloud_sec_list = cloud_sec_list.child(item);
                    }

                    cloud_sec_list = cloud_sec_list.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .mt_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Password for selected cloud vault:"),
                            )
                            .child(wizard.cloud_vault_password_input.clone()),
                    );

                    form = form.child(cloud_sec_list);
                }

                if let Some(rec_code) = &wizard.sec_recovery_code {
                    let sec_rec_confirmed = wizard.sec_recovery_confirmed;
                    form = form
                        .child(
                            div()
                                .p_3()
                                .rounded_md()
                                .bg(rgb(0x1a2e1a))
                                .border_1()
                                .border_color(rgb(0x38e562))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0x38e562))
                                        .child("Restored Vault Recovery Code"),
                                )
                                .child(
                                    div()
                                        .font_family("Courier New")
                                        .text_sm()
                                        .text_color(rgb(0xffffff))
                                        .child(rec_code.clone()),
                                ),
                        )
                        .child(
                            div()
                                .id("sec-vault-rec-confirm-checkbox-restore")
                                .flex()
                                .items_center()
                                .gap_2()
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.wizard_toggle_sec_recovery_confirmed(cx);
                                }))
                                .child(
                                    div()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(MacTheme::SEPARATOR))
                                        .bg(if sec_rec_confirmed {
                                            rgb(MacTheme::ACCENT_BLUE)
                                        } else {
                                            rgb(MacTheme::INPUT_BG)
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .child(if sec_rec_confirmed { "✓" } else { "" }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(
                                        "I have safely recorded this secondary vault recovery code",
                                    ),
                                ),
                        );
                }

                form = form.child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("Or Restore Local Vault from file:"),
                );

                form = form.child(host_form_field(
                    "Local Vault Name to Restore",
                    wizard.sec_vault_name_input.clone(),
                ));

                form = form.child(host_form_field(
                    "Vault Password or Recovery Code",
                    wizard.sec_vault_password_input.clone(),
                ));

                form = form.child(
                    div()
                        .id("wizard-restore-sec-vault-btn")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(MacTheme::ACCENT_BLUE))
                        .text_color(rgb(0xffffff))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_create_secondary_vault(cx);
                        }))
                        .child("Restore Local Vault"),
                );

                view = view.child(form);
            }
            AdditionalVaultsTab::ConnectServer => {
                let mut form = div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR));

                form = form.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(wizard.server_url_input.clone())
                        .child(
                            div()
                                .id("wizard-test-server-btn")
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SEPARATOR_SUBTLE))
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .text_xs()
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.wizard_test_server(cx);
                                }))
                                .child("Test Health"),
                        ),
                );

                form = form.child(wizard.server_user_input.clone());
                form = form.child(wizard.server_pass_input.clone());
                form = form.child(wizard.server_device_name_input.clone());

                form = form.child(
                    div()
                        .id("wizard-enroll-server-btn")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(MacTheme::ACCENT_BLUE))
                        .text_color(rgb(0xffffff))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.wizard_connect_server(cx);
                        }))
                        .child("Enroll Device & Connect Server Vault"),
                );

                view = view.child(form);
            }
        }

        // Configured Vaults summary in Step 4
        if !wizard.state.configured_vaults.is_empty() {
            let mut vault_list = div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .bg(rgb(MacTheme::SURFACE_HOVER));
            for v in &wizard.state.configured_vaults {
                vault_list = vault_list.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_xs()
                        .child(
                            div()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(format!("• {} ({})", v.name, v.id)),
                        )
                        .child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child(
                            if v.is_remote {
                                "Server"
                            } else if v.sync_enabled {
                                "Local (Drive Sync)"
                            } else {
                                "Local Only"
                            },
                        )),
                );
            }
            view = view.child(vault_list);
        }

        view
    }

    // -----------------------------------------------------------------------
    // Step 5: Review & Finish
    // -----------------------------------------------------------------------
    fn render_wizard_review_and_finish(
        &self,
        wizard: &SetupWizardModalState,
        _cx: &mut Context<Self>,
    ) -> Div {
        let mut view = div().flex().flex_col().gap_3();

        view = view.child(
            div()
                .text_base()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                .child("Step 5: Review & Finish"),
        );
        view = view.child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(
                    "All essential setup elements are verified below. Click Finish to save and launch Shellumi."
                ),
        );

        // Verification Card
        let report = &wizard.verification_report;
        let is_ok = report.as_ref().map(|r| r.is_success()).unwrap_or(false);

        view = view.child(
            div()
                .p_3()
                .rounded_lg()
                .bg(rgb(MacTheme::INPUT_BG))
                .border_1()
                .border_color(if is_ok {
                    rgb(MacTheme::SUCCESS)
                } else {
                    rgb(MacTheme::WARNING)
                })
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_ok {
                            rgb(MacTheme::SUCCESS)
                        } else {
                            rgb(MacTheme::WARNING)
                        })
                        .child(if is_ok {
                            "✓ Automated Integrity Verification Passed"
                        } else {
                            "⚠ Pre-Finish Verification Incomplete"
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("• Database AES-256-GCM encryption verified"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("• Portable wrapped-key manifests verified"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("• Default registry integrity & unlock verified"),
                ),
        );

        // Summary of Configured Setup
        let mut summary = div()
            .p_3()
            .rounded_lg()
            .bg(rgb(MacTheme::SURFACE_HOVER))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .flex()
            .flex_col()
            .gap_1();

        summary = summary.child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                .child(format!(
                    "Configured Vaults ({}):",
                    wizard.state.configured_vaults.len()
                )),
        );

        for v in &wizard.state.configured_vaults {
            summary = summary.child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(format!(
                        "  - {} [{}]: {}",
                        v.name,
                        v.id,
                        if v.is_remote {
                            "Remote Server"
                        } else if v.sync_enabled {
                            "Local (Google Drive Opt-in)"
                        } else {
                            "Local Only"
                        }
                    )),
            );
        }

        summary = summary.child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                .child(format!(
                    "Google Drive Sync: {}",
                    if wizard.state.google_drive_connected {
                        "Connected"
                    } else {
                        "Skipped"
                    }
                )),
        );

        view.child(summary)
    }
}

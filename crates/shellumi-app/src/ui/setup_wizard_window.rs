//! Dedicated native GPUI onboarding window (`SetupWizardView`).
//!
//! Replaces the full-window 1440x900 modal overlay with a focused, responsive
//! 920x640 (min 760x540) onboarding window using platform-safe chrome insets,
//! shared design-system controls, and route-specific pages driven by `OnboardingController`.

use crate::actions::setup_wizard_controller::{inspect_onboarding, wizard_window_bounds};
use crate::text_input::TextInput;
use crate::theme::MacTheme;
use crate::ui::components::{
    error_notice, inline_notice, toggle_switch, AppButton, ButtonSize, ButtonVariant,
    PasswordField, TextField,
};
use crate::ui::window_chrome::{shellumi_titlebar, ChromeMetrics};
use gpui::{
    div, img, prelude::*, px, rgb, rgba, size, AnyElement, App, ClipboardItem, Context, Div,
    Entity, FontWeight, Render, SharedString, Stateful, Styled, Window, WindowBounds,
    WindowOptions,
};
use shellumi_core::onboarding::{
    InstallationInventory, LegacySourceStatus, OnboardingController, OnboardingRoute,
    OperationState, RouteResolution, WizardPage,
};
use shellumi_core::setup::{SetupVerificationReport, SetupWizardMode};
use shellumi_core::sync::resolve_google_oauth_config;
use shellumi_core::{CloudVaultSummary, VaultKey};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

type CompletionCallback = Box<dyn FnOnce(HashMap<String, VaultKey>, &mut App) + Send>;
type ClickHandler = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>;

pub struct SetupWizardView {
    pub base_dir: PathBuf,
    pub explicit_reentry: bool,
    pub inventory: InstallationInventory,
    pub resolution: RouteResolution,
    pub controller: OnboardingController,
    pub(crate) active_vault_keys: HashMap<String, VaultKey>,
    pub(crate) on_complete: Option<CompletionCallback>,

    // Vault creation / migration inputs
    pub(crate) default_password_input: Entity<TextInput>,
    pub(crate) default_password_confirm_input: Entity<TextInput>,
    pub(crate) legacy_password_input: Entity<TextInput>,
    pub(crate) remember_in_keychain: bool,
    pub(crate) default_recovery_code: Option<String>,

    // Existing vault unlock
    pub(crate) existing_unlock_input: Entity<TextInput>,
    pub(crate) unlock_with_recovery_code: bool,

    // Legacy decision & discard confirmation
    pub(crate) legacy_choice_migrate: bool,
    pub(crate) discard_token_entity: Entity<TextInput>,
    pub(crate) discard_backup_path: Option<PathBuf>,

    // Google Drive OAuth & restore
    pub(crate) google_client_id_input: Entity<TextInput>,
    pub(crate) google_client_secret_input: Entity<TextInput>,
    pub(crate) show_advanced_oauth: bool,
    pub(crate) google_auth_url: Option<String>,
    pub(crate) oauth_cancel_flag: Option<Arc<AtomicBool>>,
    pub(crate) cloud_vaults: Vec<CloudVaultSummary>,
    pub(crate) sync_default_opt_in: bool,

    // Verification report
    pub(crate) verification_report: Option<SetupVerificationReport>,
}

impl SetupWizardView {
    pub fn new(
        base_dir: PathBuf,
        explicit_reentry: bool,
        on_complete: Option<CompletionCallback>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (inventory, resolution) = inspect_onboarding(&base_dir, explicit_reentry);
        let controller = OnboardingController::from_resolution(&resolution);

        let default_password_input = cx.new(|cx| {
            TextInput::new(cx, "", "Minimum 8 chars (upper, lower, digit, symbol)").masked()
        });
        let default_password_confirm_input =
            cx.new(|cx| TextInput::new(cx, "", "Re-enter password").masked());
        let legacy_password_input = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                "Existing legacy password (leave blank if unencrypted)",
            )
            .masked()
        });
        let existing_unlock_input =
            cx.new(|cx| TextInput::new(cx, "", "Enter vault password").masked());
        let discard_token_entity = cx.new(|cx| TextInput::new(cx, "", "Type DELETE to confirm"));

        let google_client_id_input =
            cx.new(|cx| TextInput::new(cx, "", "Google OAuth Client ID (optional override)"));
        let google_client_secret_input = cx
            .new(|cx| TextInput::new(cx, "", "Google OAuth Client Secret (if required)").masked());

        // Observe discard token text input so the destructive button updates reactively.
        cx.observe(&discard_token_entity, |this, entity, cx| {
            this.controller.discard_token_input = entity.read(cx).value().to_string();
            cx.notify();
        })
        .detach();

        Self {
            base_dir,
            explicit_reentry,
            inventory,
            resolution,
            controller,
            active_vault_keys: HashMap::new(),
            on_complete,
            default_password_input,
            default_password_confirm_input,
            legacy_password_input,
            remember_in_keychain: false,
            default_recovery_code: None,
            existing_unlock_input,
            unlock_with_recovery_code: false,
            legacy_choice_migrate: true,
            discard_token_entity,
            discard_backup_path: None,
            google_client_id_input,
            google_client_secret_input,
            show_advanced_oauth: false,
            google_auth_url: None,
            oauth_cancel_flag: None,
            cloud_vaults: Vec::new(),
            sync_default_opt_in: false,
            verification_report: None,
        }
    }

    /// Opens the dedicated SetupWizardView window with responsive bounds and safe chrome.
    pub(crate) fn open_window(
        base_dir: PathBuf,
        explicit_reentry: bool,
        on_complete: Option<CompletionCallback>,
        cx: &mut App,
    ) -> Option<gpui::WindowHandle<Self>> {
        let bounds = wizard_window_bounds(cx);
        #[cfg(target_os = "macos")]
        let window_decorations = None;
        #[cfg(not(target_os = "macos"))]
        let window_decorations = Some(gpui::WindowDecorations::Server);

        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(760.0), px(540.0))),
            titlebar: Some(shellumi_titlebar("Shellumi Setup".into())),
            window_decorations,
            app_id: Some("dev.shellumi.app.setup".to_string()),
            ..Default::default()
        };

        cx.open_window(opts, |_window, cx| {
            cx.new(|cx| Self::new(base_dir, explicit_reentry, on_complete, cx))
        })
        .ok()
    }

    fn route_steps(&self) -> Vec<(WizardPage, &'static str)> {
        match &self.controller.route {
            OnboardingRoute::LegacyMigration => vec![
                (WizardPage::LegacyDecision, "Existing Data"),
                (WizardPage::VaultPassword, "Secure Vault"),
                (WizardPage::RecoveryCode, "Recovery Code"),
                (WizardPage::GoogleDrive, "Google Drive"),
                (WizardPage::ReviewAndFinish, "Finish"),
            ],
            OnboardingRoute::InterruptedMigration => vec![
                (WizardPage::InterruptedMigration, "Resume Migration"),
                (WizardPage::RecoveryCode, "Recovery Code"),
                (WizardPage::GoogleDrive, "Google Drive"),
                (WizardPage::ReviewAndFinish, "Finish"),
            ],
            OnboardingRoute::InterruptedDiscard => vec![
                (WizardPage::InterruptedDiscard, "Resume Removal"),
                (WizardPage::VaultPassword, "Secure Vault"),
                (WizardPage::RecoveryCode, "Recovery Code"),
                (WizardPage::ReviewAndFinish, "Finish"),
            ],
            OnboardingRoute::InconsistentInstallation { .. } => {
                vec![(WizardPage::InconsistentInstallation, "Diagnostics")]
            }
            OnboardingRoute::ExistingVault
            | OnboardingRoute::CoexistingLegacyAndModern
            | OnboardingRoute::ExplicitReentry => vec![
                (WizardPage::LocalUnlock, "Unlock Vault"),
                (WizardPage::GoogleDrive, "Google Drive"),
                (WizardPage::ReviewAndFinish, "Finish"),
            ],
            _ => vec![
                (WizardPage::FreshWelcome, "Welcome"),
                (WizardPage::VaultPassword, "Secure Vault"),
                (WizardPage::RecoveryCode, "Recovery Code"),
                (WizardPage::GoogleDrive, "Google Drive"),
                (WizardPage::ReviewAndFinish, "Finish"),
            ],
        }
    }

    fn render_sidebar(&self, steps: &[(WizardPage, &'static str)]) -> Stateful<Div> {
        let current_idx = steps
            .iter()
            .position(|(p, _)| *p == self.controller.current_page)
            .unwrap_or(0);

        let mut list = div().flex().flex_col().gap_2();
        for (idx, (_, label)) in steps.iter().enumerate() {
            let is_current = idx == current_idx;
            let is_done = idx < current_idx;
            let marker = if is_done {
                "✓"
            } else if is_current {
                "●"
            } else {
                "○"
            };
            let color = if is_current {
                rgb(MacTheme::TEXT_PRIMARY)
            } else if is_done {
                rgb(MacTheme::SUCCESS)
            } else {
                rgb(MacTheme::TEXT_TERTIARY)
            };

            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .when(is_current, |d| d.bg(rgba(0x0a84ff1a)))
                    .child(div().text_xs().text_color(color).child(marker))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(if is_current {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(color)
                            .child(*label),
                    ),
            );
        }

        div()
            .id("wizard-sidebar")
            .w(px(220.0))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(MacTheme::SIDEBAR_BG))
            .border_r_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .p_5()
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                img("icon_pack/icons/48/regular/shellumi_logo.png")
                                    .w(px(28.0))
                                    .h(px(28.0)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Shellumi Setup"),
                            ),
                    )
                    .child(list),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Local-first.\nPrivate by design."),
            )
    }

    fn render_operation_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        match &self.controller.operation {
            OperationState::Idle => None,
            OperationState::Running {
                phase, can_cancel, ..
            } => Some(
                div()
                    .id("wizard-op-running")
                    .p_3()
                    .rounded_md()
                    .bg(rgba(0x0a84ff14))
                    .border_1()
                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(phase.to_string()),
                    )
                    .when(*can_cancel, |d| {
                        d.child(
                            AppButton::sized(
                                "wizard-op-cancel",
                                "Cancel",
                                ButtonVariant::Secondary,
                                ButtonSize::Sm,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cancel_active_operation(cx);
                            })),
                        )
                    })
                    .into_any_element(),
            ),
            OperationState::Succeeded { summary, .. } => {
                Some(inline_notice(None, summary.clone(), false).into_any_element())
            }
            OperationState::Failed { error, .. } => {
                let diag = self.build_sanitized_diagnostics();
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(error_notice(format!("{}: {}", error.title, error.message)))
                        .child(
                            div().flex().items_center().gap_2().child(
                                AppButton::sized(
                                    "wizard-copy-diag",
                                    "Copy Diagnostics",
                                    ButtonVariant::Ghost,
                                    ButtonSize::Sm,
                                )
                                .on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            diag.clone(),
                                        ));
                                    },
                                )),
                            ),
                        )
                        .into_any_element(),
                )
            }
            OperationState::Cancelled { .. } => Some(
                inline_notice(
                    None,
                    "Operation cancelled. No changes were committed.",
                    false,
                )
                .into_any_element(),
            ),
        }
    }

    fn render_page_body(&self, cx: &mut Context<Self>) -> Div {
        match self.controller.current_page {
            WizardPage::FreshWelcome => self.render_fresh_welcome(cx),
            WizardPage::LegacyDecision => self.render_legacy_decision(cx),
            WizardPage::DiscardConfirm => self.render_discard_confirm(cx),
            WizardPage::InterruptedMigration => self.render_interrupted_migration(cx),
            WizardPage::InterruptedDiscard => self.render_interrupted_discard(cx),
            WizardPage::InconsistentInstallation => self.render_inconsistent(cx),
            WizardPage::VaultPassword => self.render_vault_password(cx),
            WizardPage::RecoveryCode => self.render_recovery_code(cx),
            WizardPage::MigrationProgress => self.render_migration_progress(),
            WizardPage::LocalUnlock => self.render_local_unlock(cx),
            WizardPage::GoogleDrive => self.render_google_drive(cx),
            WizardPage::CloudRestore => self.render_cloud_restore(cx),
            WizardPage::ReviewAndFinish => self.render_review_and_finish(cx),
            _ => self.render_fresh_welcome(cx),
        }
    }

    fn render_fresh_welcome(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Welcome to Shellumi"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Your SSH hosts, credentials, and infrastructure stay encrypted locally on your device. No account is required."),
            )
            .child(self.choice_card(
                "welcome-create-new",
                "Create a new vault",
                "Set up a fresh encrypted Default Vault with its own password and recovery code.",
                self.controller.mode == SetupWizardMode::NewSetup,
                cx.listener(|this, _, _, cx| {
                    this.controller.mode = SetupWizardMode::NewSetup;
                    cx.notify();
                }),
            ))
            .child(self.choice_card(
                "welcome-restore-existing",
                "Restore existing setup",
                "Sign in to Google Drive or unlock an existing vault.",
                self.controller.mode == SetupWizardMode::RestoreSetup,
                cx.listener(|this, _, _, cx| {
                    this.controller.mode = SetupWizardMode::RestoreSetup;
                    cx.notify();
                }),
            ))
    }

    fn render_legacy_decision(&self, cx: &mut Context<Self>) -> Div {
        let (path_str, size_str, sidecar_count) = match &self.inventory.legacy {
            LegacySourceStatus::Present {
                path,
                size_bytes,
                sidecars,
                ..
            } => (
                path.display().to_string(),
                format!("{} KB", (*size_bytes).max(1024) / 1024),
                sidecars.len(),
            ),
            _ => ("shellumi.db".to_string(), "Unknown".to_string(), 0),
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        img("icon_pack/icons/48/regular/vault_encryption.png")
                            .w(px(36.0))
                            .h(px(36.0)),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Welcome back"),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("We found an existing local database from an earlier version of Shellumi. Choose how to proceed:"),
            )
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::WINDOW_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(format!(
                        "Database: {path_str} ({size_str}, {sidecar_count} sidecar file(s))"
                    )),
            )
            .child(self.choice_card(
                "legacy-choice-migrate",
                "Migrate existing data (Recommended)",
                "Preserve the original database and migrate hosts, credentials, and settings into encrypted vaults.",
                self.legacy_choice_migrate,
                cx.listener(|this, _, _, cx| {
                    this.legacy_choice_migrate = true;
                    cx.notify();
                }),
            ))
            .child(self.choice_card(
                "legacy-choice-discard",
                "Start fresh — remove existing database",
                "Permanently remove the old local database and its sidecars, then create a new empty encrypted vault.",
                !self.legacy_choice_migrate,
                cx.listener(|this, _, _, cx| {
                    this.legacy_choice_migrate = false;
                    cx.notify();
                }),
            ))
    }

    fn render_discard_confirm(&self, cx: &mut Context<Self>) -> Div {
        let (path_str, size_str, sidecar_desc) = match &self.inventory.legacy {
            LegacySourceStatus::Present {
                path,
                size_bytes,
                sidecars,
                ..
            } => {
                let sc = if sidecars.is_empty() {
                    "none".to_string()
                } else {
                    sidecars
                        .iter()
                        .map(|s| format!("shellumi.db{}", s.suffix))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                (
                    path.display().to_string(),
                    format!("{} bytes", size_bytes),
                    sc,
                )
            }
            _ => (
                "shellumi.db".to_string(),
                "0 bytes".to_string(),
                "none".to_string(),
            ),
        };

        let can_remove = self.controller.can_confirm_legacy_discard();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::DANGER))
                    .child("Remove existing database and start fresh?"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Shellumi found data from an earlier installation. Continuing will permanently remove the existing local database and its related local database files. Your saved hosts, credentials, groups, snippets and any other data stored only in this database will no longer be available. This action cannot be undone by Shellumi after deletion completes."),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("We recommend creating a backup before continuing. Backups you create, other independent vaults, and Google Drive backups are not deleted by this operation."),
            )
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::WINDOW_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(format!("Target: {path_str} ({size_str}) | Sidecars: {sidecar_desc}")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        AppButton::sized(
                            "discard-backup-btn",
                            "Back up existing data…",
                            ButtonVariant::Secondary,
                            ButtonSize::Md,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.start_legacy_backup_picker(cx);
                        })),
                    )
                    .when(!self.controller.discard_backup_completed, |d| {
                        d.child(
                            toggle_switch(
                                "discard-skip-backup",
                                self.controller.discard_proceed_without_backup,
                                "Proceed without backup",
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.controller.discard_proceed_without_backup =
                                    !this.controller.discard_proceed_without_backup;
                                cx.notify();
                            })),
                        )
                    }),
            )
            .when_some(self.discard_backup_path.as_ref(), |d, bp| {
                d.child(inline_notice(
                    None,
                    format!("Verified backup created at {}", bp.display()),
                    false,
                ))
            })
            .child(
                toggle_switch(
                    "discard-ack-checkbox",
                    self.controller.discard_acknowledged,
                    "I understand that my existing local database will be permanently removed.",
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.controller.discard_acknowledged = !this.controller.discard_acknowledged;
                    cx.notify();
                })),
            )
            .child(TextField::new(
                "Type DELETE to confirm permanent removal",
                self.discard_token_entity.clone(),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .pt_2()
                    .child(
                        AppButton::new("discard-cancel-btn", "Cancel", ButtonVariant::Secondary)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.controller.navigate_to(WizardPage::LegacyDecision);
                                cx.notify();
                            })),
                    )
                    .child(if can_remove {
                        AppButton::new(
                            "discard-confirm-btn",
                            "Remove Database & Start Fresh",
                            ButtonVariant::Danger,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.start_legacy_discard(cx);
                        }))
                        .into_any_element()
                    } else {
                        AppButton::disabled("Remove Database & Start Fresh", ButtonSize::Md)
                            .into_any_element()
                    }),
            )
    }

    fn render_interrupted_migration(&self, cx: &mut Context<Self>) -> Div {
        let details = self.resolution.diagnostics.join(" • ");
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Interrupted Migration Detected"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("A previous migration did not finish. Your original database and verified backup remain intact."),
            )
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::WINDOW_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(details),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        AppButton::new(
                            "mig-resume-btn",
                            "Resume Migration",
                            ButtonVariant::Primary,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.controller.navigate_to(WizardPage::VaultPassword);
                            cx.notify();
                        })),
                    )
                    .child(
                        AppButton::new(
                            "mig-rollback-btn",
                            "Roll Back Safely",
                            ButtonVariant::Secondary,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.start_rollback_interrupted_migration(cx);
                        })),
                    ),
            )
    }

    fn render_interrupted_discard(&self, cx: &mut Context<Self>) -> Div {
        let details = self.resolution.diagnostics.join(" • ");
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Interrupted Database Removal"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("A previous legacy database removal was interrupted. Resolve it before creating a new vault."),
            )
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::WINDOW_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(details),
            )
            .child(
                AppButton::new(
                    "discard-resolve-btn",
                    "Resolve Removal Transaction",
                    ButtonVariant::Primary,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.start_resume_or_rollback_discard(cx);
                })),
            )
    }

    fn render_inconsistent(&self, cx: &mut Context<Self>) -> Div {
        let (reason, details) = match &self.controller.route {
            OnboardingRoute::InconsistentInstallation { reason, details } => {
                (reason.clone(), details.join("\n"))
            }
            _ => ("Inconsistent installation state".to_string(), String::new()),
        };
        let diag = self.build_sanitized_diagnostics();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::DANGER))
                    .child("Installation Needs Attention"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(reason),
            )
            .when(!details.is_empty(), |d| {
                d.child(
                    div()
                        .p_3()
                        .rounded_md()
                        .bg(rgb(MacTheme::WINDOW_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child(details),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        AppButton::new(
                            "inconsistent-copy-diag",
                            "Copy Diagnostics",
                            ButtonVariant::Secondary,
                        )
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(diag.clone()));
                        })),
                    )
                    .child(
                        AppButton::new(
                            "inconsistent-refresh",
                            "Re-scan Directory",
                            ButtonVariant::Primary,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.refresh_inventory(cx);
                            let next = OnboardingController::from_resolution(&this.resolution);
                            this.controller.route = next.route;
                            this.controller.current_page = next.current_page;
                            cx.notify();
                        })),
                    ),
            )
    }

    fn render_vault_password(&self, cx: &mut Context<Self>) -> Div {
        let is_migration = matches!(
            self.controller.route,
            OnboardingRoute::LegacyMigration | OnboardingRoute::InterruptedMigration
        );
        let legacy_encrypted = matches!(
            self.inventory.legacy,
            LegacySourceStatus::Present {
                is_encrypted_or_non_sqlite: true,
                ..
            }
        );

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(if is_migration {
                        "Protect your migrated vault"
                    } else {
                        "Create your vault password"
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Each vault is encrypted with its own independent key. Choose a strong password (at least 8 characters with uppercase, lowercase, number, and symbol)."),
            )
            .when(is_migration && legacy_encrypted, |d| {
                d.child(PasswordField::new(
                    "Existing legacy database password",
                    self.legacy_password_input.clone(),
                ))
            })
            .child(
                PasswordField::new("Vault password", self.default_password_input.clone())
                    .required(true),
            )
            .child(
                PasswordField::new(
                    "Confirm vault password",
                    self.default_password_confirm_input.clone(),
                )
                .required(true),
            )
            .child(
                toggle_switch(
                    "vault-remember-keychain",
                    self.remember_in_keychain,
                    "Remember unlock key in OS Keychain",
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.remember_in_keychain = !this.remember_in_keychain;
                    cx.notify();
                })),
            )
    }

    fn render_recovery_code(&self, cx: &mut Context<Self>) -> Div {
        let code = self
            .default_recovery_code
            .clone()
            .unwrap_or_else(|| "Recovery code unavailable".to_string());
        let code_for_copy = code.clone();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Save your recovery code"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("If you ever forget your vault password, this 8-word recovery code is the only way to regain access to your local vault."),
            )
            .child(
                div()
                    .p_4()
                    .rounded_md()
                    .bg(rgb(MacTheme::WINDOW_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(code),
                    )
                    .child(
                        div().flex().gap_2().child(
                            AppButton::sized(
                                "recovery-copy-btn",
                                "Copy Recovery Code",
                                ButtonVariant::Secondary,
                                ButtonSize::Sm,
                            )
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    code_for_copy.clone(),
                                ));
                            })),
                        ),
                    ),
            )
            .child(
                toggle_switch(
                    "recovery-confirm-toggle",
                    self.controller.recovery_confirmed,
                    "I have saved my recovery code in a safe place.",
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.controller.recovery_confirmed = !this.controller.recovery_confirmed;
                    let confirmed = this.controller.recovery_confirmed;
                    let _ = this.persist_checkpoint(|s| {
                        s.recovery_code_confirmed = confirmed;
                    });
                    cx.notify();
                })),
            )
    }

    fn render_migration_progress(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Migrating your data"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Shellumi is backing up your legacy database, encrypting your vaults, and verifying integrity. Your original database will remain intact."),
            )
    }

    fn render_local_unlock(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Unlock your Default Vault"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(
                    "Choose your unlock method to verify access to your existing encrypted vault.",
                ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        AppButton::sized(
                            "unlock-mode-password",
                            "Password",
                            if !self.unlock_with_recovery_code {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            },
                            ButtonSize::Sm,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.unlock_with_recovery_code = false;
                            cx.notify();
                        })),
                    )
                    .child(
                        AppButton::sized(
                            "unlock-mode-recovery",
                            "Recovery Code",
                            if self.unlock_with_recovery_code {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            },
                            ButtonSize::Sm,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.unlock_with_recovery_code = true;
                            cx.notify();
                        })),
                    ),
            )
            .child(PasswordField::new(
                if self.unlock_with_recovery_code {
                    "8-word recovery code"
                } else {
                    "Vault password"
                },
                self.existing_unlock_input.clone(),
            ))
    }

    fn render_google_drive(&self, cx: &mut Context<Self>) -> Div {
        let cred_store = shellumi_core::default_credential_store();
        let configured = resolve_google_oauth_config(cred_store.as_ref(), None)
            .ok()
            .flatten()
            .is_some();
        let connected = self.checkpoint_google_connected();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        img("icon_pack/icons/48/regular/google_drive.png")
                            .w(px(36.0))
                            .h(px(36.0)),
                    )
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Google Drive Backup & Sync"),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Optional end-to-end encrypted backup in your Google Drive AppData folder. Your vault remains fully functional offline without Google Drive."),
            )
            .when(!configured && !connected, |d| {
                d.child(inline_notice(
                    None,
                    "Google Drive OAuth is not preconfigured in this self-build. You can skip Google Drive or enter a Client ID under Advanced OAuth Configuration.",
                    true,
                ))
            })
            .when_some(self.google_auth_url.as_ref(), |d, url| {
                let url_copy = url.clone();
                let url_open = url.clone();
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            AppButton::sized(
                                "oauth-open-again",
                                "Open browser again",
                                ButtonVariant::Secondary,
                                ButtonSize::Sm,
                            )
                            .on_click(cx.listener(move |_, _, _, _| {
                                let _ = crate::actions::setup_wizard_controller::open_system_browser(&url_open);
                            })),
                        )
                        .child(
                            AppButton::sized(
                                "oauth-copy-link",
                                "Copy sign-in link",
                                ButtonVariant::Ghost,
                                ButtonSize::Sm,
                            )
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(url_copy.clone()));
                            })),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(!connected, |d| {
                        d.child(if self.controller.operation.is_running() {
                            AppButton::disabled("Sign in with Google", ButtonSize::Md)
                                .into_any_element()
                        } else {
                            AppButton::new(
                                "google-signin-btn",
                                "Sign in with Google",
                                ButtonVariant::Primary,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start_google_sign_in(cx);
                            }))
                            .into_any_element()
                        })
                    })
                    .when(connected, |d| {
                        d.child(inline_notice(
                            None,
                            format!(
                                "Google Drive connected ({} backup(s) discovered).",
                                self.cloud_vaults.len()
                            ),
                            false,
                        ))
                        .child(
                            AppButton::sized(
                                "google-disconnect-btn",
                                "Disconnect",
                                ButtonVariant::Secondary,
                                ButtonSize::Sm,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.disconnect_google_drive(cx);
                            })),
                        )
                    }),
            )
            .when(connected, |d| {
                d.child(
                    toggle_switch(
                        "google-sync-opt-in",
                        self.sync_default_opt_in,
                        "Enable encrypted Google Drive sync for Default Vault",
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sync_default_opt_in = !this.sync_default_opt_in;
                        cx.notify();
                    })),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        AppButton::sized(
                            "oauth-adv-toggle",
                            if self.show_advanced_oauth {
                                "Hide Advanced OAuth Configuration"
                            } else {
                                "Advanced OAuth Configuration"
                            },
                            ButtonVariant::Ghost,
                            ButtonSize::Sm,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_advanced_oauth = !this.show_advanced_oauth;
                            cx.notify();
                        })),
                    )
                    .when(self.show_advanced_oauth, |d| {
                        d.child(TextField::new(
                            "Google OAuth Client ID",
                            self.google_client_id_input.clone(),
                        ))
                        .child(PasswordField::new(
                            "Google OAuth Client Secret (optional)",
                            self.google_client_secret_input.clone(),
                        ))
                    }),
            )
    }

    fn render_cloud_restore(&self, _cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Restore from Google Drive"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(format!(
                        "Discovered {} encrypted vault snapshot(s) in your Google Drive AppData folder.",
                        self.cloud_vaults.len()
                    )),
            )
    }

    fn render_review_and_finish(&self, _cx: &mut Context<Self>) -> Div {
        let default_ok = self.inventory.has_valid_default_vault();
        let sync_label = if self.sync_default_opt_in && self.checkpoint_google_connected() {
            "Enabled (Default Vault)"
        } else {
            "Disabled (Local-only)"
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Ready to launch Shellumi"),
            )
            .child(
                div()
                    .p_4()
                    .rounded_md()
                    .bg(rgb(MacTheme::WINDOW_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(format!(
                        "Default Vault: {}",
                        if default_ok {
                            "Encrypted & Verified"
                        } else {
                            "Pending verification"
                        }
                    ))
                    .child(format!(
                        "Recovery Code: {}",
                        if self.controller.recovery_confirmed {
                            "Confirmed"
                        } else {
                            "Not confirmed"
                        }
                    ))
                    .child(format!("Google Drive Sync: {sync_label}")),
            )
    }

    fn choice_card(
        &self,
        id: &'static str,
        title: &'static str,
        desc: &'static str,
        selected: bool,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> gpui::Stateful<Div> {
        div()
            .id(id)
            .p_4()
            .rounded_lg()
            .bg(if selected {
                rgba(0x0a84ff18)
            } else {
                rgb(MacTheme::WINDOW_BG)
            })
            .border_1()
            .border_color(if selected {
                rgb(MacTheme::ACCENT_BLUE)
            } else {
                rgb(MacTheme::SEPARATOR)
            })
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(on_click)
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(title),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(desc),
            )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let is_busy = self.controller.operation.is_running();
        let can_back = self.controller.can_go_back();

        let btn_or_disabled = |id: &'static str,
                               label: &'static str,
                               variant: ButtonVariant,
                               enabled: bool,
                               on_click: ClickHandler|
         -> AnyElement {
            if enabled {
                AppButton::new(id, label, variant)
                    .on_click(on_click)
                    .into_any_element()
            } else {
                AppButton::disabled(label, ButtonSize::Md).into_any_element()
            }
        };

        let mut right = div().flex().items_center().gap_2();
        match self.controller.current_page {
            WizardPage::FreshWelcome => {
                right = right.child(btn_or_disabled(
                    "wizard-next-btn",
                    "Continue",
                    ButtonVariant::Primary,
                    !is_busy,
                    Box::new(cx.listener(|this, _, _, cx| {
                        if this.controller.mode == SetupWizardMode::RestoreSetup {
                            this.controller.navigate_to(WizardPage::GoogleDrive);
                        } else {
                            this.controller.navigate_to(WizardPage::VaultPassword);
                        }
                        cx.notify();
                    })),
                ));
            }
            WizardPage::LegacyDecision => {
                right = right.child(btn_or_disabled(
                    "wizard-next-btn",
                    "Continue",
                    ButtonVariant::Primary,
                    !is_busy,
                    Box::new(cx.listener(|this, _, _, cx| {
                        if this.legacy_choice_migrate {
                            this.controller.navigate_to(WizardPage::VaultPassword);
                        } else {
                            this.controller.navigate_to(WizardPage::DiscardConfirm);
                        }
                        cx.notify();
                    })),
                ));
            }
            WizardPage::VaultPassword => {
                let is_migration = matches!(
                    self.controller.route,
                    OnboardingRoute::LegacyMigration | OnboardingRoute::InterruptedMigration
                );
                right = right.child(btn_or_disabled(
                    "wizard-create-or-migrate-btn",
                    if is_migration {
                        "Migrate Existing Data"
                    } else {
                        "Create Encrypted Vault"
                    },
                    ButtonVariant::Primary,
                    !is_busy,
                    Box::new(cx.listener(move |this, _, _, cx| {
                        if is_migration {
                            this.start_legacy_migration(cx);
                        } else {
                            this.start_fresh_vault_creation(cx);
                        }
                    })),
                ));
            }
            WizardPage::RecoveryCode => {
                right = right.child(btn_or_disabled(
                    "wizard-recovery-next-btn",
                    "Continue",
                    ButtonVariant::Primary,
                    !is_busy && self.controller.recovery_confirmed,
                    Box::new(cx.listener(|this, _, _, cx| {
                        this.controller.navigate_to(WizardPage::GoogleDrive);
                        cx.notify();
                    })),
                ));
            }
            WizardPage::LocalUnlock => {
                right = right.child(btn_or_disabled(
                    "wizard-unlock-btn",
                    "Unlock Vault",
                    ButtonVariant::Primary,
                    !is_busy,
                    Box::new(cx.listener(|this, _, _, cx| {
                        this.start_unlock_existing_vault(cx);
                    })),
                ));
            }
            WizardPage::GoogleDrive => {
                right = right
                    .child(btn_or_disabled(
                        "wizard-skip-drive",
                        "Skip",
                        ButtonVariant::Secondary,
                        !is_busy,
                        Box::new(cx.listener(|this, _, _, cx| {
                            this.controller.google_drive_skipped = true;
                            let _ = this.persist_checkpoint(|s| {
                                s.google_drive_skipped = true;
                                s.google_drive_sync_enabled = false;
                            });
                            this.controller.navigate_to(WizardPage::ReviewAndFinish);
                            cx.notify();
                        })),
                    ))
                    .child(btn_or_disabled(
                        "wizard-drive-next",
                        "Continue",
                        ButtonVariant::Primary,
                        !is_busy,
                        Box::new(cx.listener(|this, _, _, cx| {
                            this.controller.navigate_to(WizardPage::ReviewAndFinish);
                            cx.notify();
                        })),
                    ));
            }
            WizardPage::ReviewAndFinish => {
                right = right.child(btn_or_disabled(
                    "wizard-launch-btn",
                    "Launch Shellumi",
                    ButtonVariant::Primary,
                    !is_busy,
                    Box::new(cx.listener(|this, _, _, cx| {
                        this.finish_and_launch(cx);
                    })),
                ));
            }
            _ => {}
        }

        div()
            .id("wizard-footer")
            .flex_shrink_0()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .bg(rgb(MacTheme::SURFACE))
            .flex()
            .items_center()
            .justify_between()
            .child(btn_or_disabled(
                "wizard-back-btn",
                "Back",
                ButtonVariant::Secondary,
                can_back && !is_busy,
                Box::new(cx.listener(|this, _, _, cx| {
                    this.controller.go_back();
                    cx.notify();
                })),
            ))
            .child(right)
    }
}

impl Render for SetupWizardView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chrome = ChromeMetrics::from_window(window);
        let viewport_w = f32::from(window.bounds().size.width);
        let show_sidebar = viewport_w >= 820.0;
        let steps = self.route_steps();

        let top_chrome = div()
            .id("wizard-titlebar")
            .h(px(38.0))
            .flex_shrink_0()
            .pl(px(chrome.left_reserved_width))
            .pr_4()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .bg(rgb(MacTheme::SURFACE))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(SharedString::from(format!(
                        "Shellumi Setup — {}",
                        self.controller.current_page.title()
                    ))),
            )
            .when(!show_sidebar, |d| {
                let idx = steps
                    .iter()
                    .position(|(p, _)| *p == self.controller.current_page)
                    .unwrap_or(0);
                d.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child(format!("Step {} of {}", idx + 1, steps.len().max(1))),
                )
            });

        let mut content_col = div()
            .w_full()
            .max_w(px(520.0))
            .mx_auto()
            .py_6()
            .px_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.render_page_body(cx));

        if let Some(banner) = self.render_operation_banner(cx) {
            content_col = content_col.child(banner);
        }

        let main_pane = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .child(
                div()
                    .id("wizard-scroll-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(content_col),
            )
            .child(self.render_footer(cx));

        div()
            .id("setup-wizard-window-root")
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .child(top_chrome)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .when(show_sidebar, |d| d.child(self.render_sidebar(&steps)))
                    .child(main_pane),
            )
    }
}

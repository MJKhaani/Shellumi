//! Dedicated native GPUI onboarding window (`SetupWizardView`).
//!
//! Focused, responsive 920x640 (min 760x540) onboarding window using platform-safe
//! chrome insets, typed `IconName` assets, definite column widths, shared design-system
//! controls, and route-specific pages driven by `OnboardingController`.

use crate::actions::setup_wizard_controller::{inspect_onboarding, wizard_window_bounds};
use crate::text_input::TextInput;
use crate::theme::MacTheme;
use crate::ui::components::{
    error_notice, inline_notice, toggle_switch, AppButton, ButtonSize, ButtonVariant,
    PasswordField, TextField,
};
use crate::ui::icon::{Icon, IconName};
use crate::ui::window_chrome::{shellumi_titlebar, ChromeMetrics};
use gpui::{
    div, prelude::*, px, rgb, rgba, size, AnyElement, App, ClipboardItem, Context, Div, Entity,
    FontWeight, Render, SharedString, Stateful, Styled, Window, WindowBounds, WindowOptions,
};
use shellumi_core::onboarding::{
    InstallationInventory, LegacySourceStatus, OnboardingController, OnboardingRoute,
    OperationState, RouteResolution, WizardPage,
};
use shellumi_core::setup::{SetupVerificationReport, SetupWizardMode};
use shellumi_core::sync::resolve_google_oauth_config;
use shellumi_core::{CloudVaultSummary, VaultKey};
use std::collections::{BTreeMap, HashMap};
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
    pub(crate) migration_progress_label: String,

    // Existing vault unlock
    pub(crate) existing_unlock_input: Entity<TextInput>,
    pub(crate) unlock_with_recovery_code: bool,

    // Legacy decision & discard confirmation
    pub(crate) legacy_choice_migrate: bool,
    pub(crate) discard_token_entity: Entity<TextInput>,
    pub(crate) discard_backup_path: Option<PathBuf>,
    pub(crate) discard_expected_fingerprints: Option<BTreeMap<String, String>>,

    // Google Drive OAuth & restore
    pub(crate) google_client_id_input: Entity<TextInput>,
    pub(crate) google_client_secret_input: Entity<TextInput>,
    pub(crate) show_advanced_oauth: bool,
    pub(crate) google_auth_url: Option<String>,
    pub(crate) browser_launch_warning: Option<String>,
    pub(crate) google_discovery_error: Option<String>,
    pub(crate) oauth_cancel_flag: Option<Arc<AtomicBool>>,
    pub(crate) cloud_vaults: Vec<CloudVaultSummary>,
    pub(crate) selected_cloud_vault_id: Option<String>,
    pub(crate) cloud_vault_password_input: Entity<TextInput>,
    pub(crate) sync_default_opt_in: bool,

    // Additional vaults & server enrollment
    pub(crate) additional_tab_server: bool,
    pub(crate) sec_vault_name_input: Entity<TextInput>,
    pub(crate) sec_vault_password_input: Entity<TextInput>,
    pub(crate) sec_vault_password_confirm_input: Entity<TextInput>,
    pub(crate) sec_vault_keychain: bool,
    pub(crate) server_url_input: Entity<TextInput>,
    pub(crate) server_user_input: Entity<TextInput>,
    pub(crate) server_pass_input: Entity<TextInput>,
    pub(crate) server_device_name_input: Entity<TextInput>,

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

        let default_password_input =
            cx.new(|cx| TextInput::new(cx, "", "Enter a strong password").masked());
        let default_password_confirm_input =
            cx.new(|cx| TextInput::new(cx, "", "Re-enter password").masked());
        let legacy_password_input =
            cx.new(|cx| TextInput::new(cx, "", "Existing legacy password").masked());
        let existing_unlock_input =
            cx.new(|cx| TextInput::new(cx, "", "Enter vault password").masked());
        let discard_token_entity = cx.new(|cx| TextInput::new(cx, "", "Type DELETE to confirm"));

        let google_client_id_input = cx.new(|cx| TextInput::new(cx, "", "Google OAuth Client ID"));
        let google_client_secret_input =
            cx.new(|cx| TextInput::new(cx, "", "Google OAuth Client Secret (optional)").masked());
        let cloud_vault_password_input =
            cx.new(|cx| TextInput::new(cx, "", "Password for selected cloud vault").masked());

        let sec_vault_name_input = cx.new(|cx| TextInput::new(cx, "", "Vault name (e.g. Work)"));
        let sec_vault_password_input =
            cx.new(|cx| TextInput::new(cx, "", "Secondary vault password").masked());
        let sec_vault_password_confirm_input =
            cx.new(|cx| TextInput::new(cx, "", "Confirm secondary password").masked());
        let server_url_input = cx.new(|cx| {
            TextInput::new(
                cx,
                "https://ssh.example.com:8443",
                "https://ssh.example.com:8443",
            )
        });
        let server_user_input = cx.new(|cx| TextInput::new(cx, "", "Username or email"));
        let server_pass_input = cx.new(|cx| TextInput::new(cx, "", "Account password").masked());
        let default_device = std::env::var("USER")
            .map(|u| format!("{u}-desktop"))
            .unwrap_or_else(|_| "Desktop-Client".to_string());
        let server_device_name_input =
            cx.new(|cx| TextInput::new(cx, &default_device, "Friendly device name"));

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
            migration_progress_label: "Preparing migration…".to_string(),
            existing_unlock_input,
            unlock_with_recovery_code: false,
            legacy_choice_migrate: true,
            discard_token_entity,
            discard_backup_path: None,
            discard_expected_fingerprints: None,
            google_client_id_input,
            google_client_secret_input,
            show_advanced_oauth: false,
            google_auth_url: None,
            browser_launch_warning: None,
            google_discovery_error: None,
            oauth_cancel_flag: None,
            cloud_vaults: Vec::new(),
            selected_cloud_vault_id: None,
            cloud_vault_password_input,
            sync_default_opt_in: false,
            additional_tab_server: false,
            sec_vault_name_input,
            sec_vault_password_input,
            sec_vault_password_confirm_input,
            sec_vault_keychain: false,
            server_url_input,
            server_user_input,
            server_pass_input,
            server_device_name_input,
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
                (WizardPage::AdditionalVaults, "More Vaults"),
                (WizardPage::ReviewAndFinish, "Finish"),
            ],
            OnboardingRoute::InterruptedMigration => vec![
                (WizardPage::InterruptedMigration, "Resume Migration"),
                (WizardPage::RecoveryCode, "Recovery Code"),
                (WizardPage::GoogleDrive, "Google Drive"),
                (WizardPage::AdditionalVaults, "More Vaults"),
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
                (WizardPage::AdditionalVaults, "More Vaults"),
                (WizardPage::ReviewAndFinish, "Finish"),
            ],
            _ => {
                if self.controller.mode == SetupWizardMode::RestoreSetup {
                    vec![
                        (WizardPage::FreshWelcome, "Welcome"),
                        (WizardPage::GoogleDrive, "Google Drive"),
                        (WizardPage::CloudRestore, "Restore Backup"),
                        (WizardPage::AdditionalVaults, "More Vaults"),
                        (WizardPage::ReviewAndFinish, "Finish"),
                    ]
                } else {
                    vec![
                        (WizardPage::FreshWelcome, "Welcome"),
                        (WizardPage::VaultPassword, "Secure Vault"),
                        (WizardPage::RecoveryCode, "Recovery Code"),
                        (WizardPage::GoogleDrive, "Google Drive"),
                        (WizardPage::AdditionalVaults, "More Vaults"),
                        (WizardPage::ReviewAndFinish, "Finish"),
                    ]
                }
            }
        }
    }

    fn render_sidebar(&self, steps: &[(WizardPage, &'static str)]) -> Stateful<Div> {
        let current_idx = steps
            .iter()
            .position(|(p, _)| *p == self.controller.current_page)
            .unwrap_or(0);

        let mut list = div().w_full().flex().flex_col().gap_2();
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
                    .w_full()
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
                            .flex_1()
                            .min_w_0()
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
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Icon::new(IconName::ShellumiLogo).size(28.0))
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
                    .w_full()
                    .min_w_0()
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
                            .flex_1()
                            .min_w_0()
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
            OperationState::Succeeded { summary, .. } => Some(
                div()
                    .w_full()
                    .min_w_0()
                    .child(inline_notice(None, summary.clone(), false))
                    .into_any_element(),
            ),
            OperationState::Failed { error, .. } => {
                let diag = self.build_sanitized_diagnostics();
                Some(
                    div()
                        .w_full()
                        .min_w_0()
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
                div()
                    .w_full()
                    .min_w_0()
                    .child(inline_notice(
                        None,
                        "Operation cancelled. No changes were committed.",
                        false,
                    ))
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
            WizardPage::MigrationReview => self.render_legacy_decision(cx),
            WizardPage::MigrationProgress => self.render_migration_progress(),
            WizardPage::LocalUnlock => self.render_local_unlock(cx),
            WizardPage::GoogleDrive => self.render_google_drive(cx),
            WizardPage::CloudRestore => self.render_cloud_restore(cx),
            WizardPage::AdditionalVaults => self.render_additional_vaults(cx),
            WizardPage::ReviewAndFinish => self.render_review_and_finish(cx),
        }
    }

    fn render_fresh_welcome(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .w_full()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Welcome to Shellumi"),
            )
            .child(
                div()
                    .w_full()
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

    fn legacy_summary_text(&self) -> (String, String, usize, u64) {
        match &self.inventory.legacy {
            LegacySourceStatus::Present {
                path,
                size_bytes,
                sidecars,
                ..
            } => {
                let sidecar_bytes: u64 = sidecars.iter().map(|s| s.size_bytes).sum();
                let total_bytes = size_bytes.saturating_add(sidecar_bytes);
                let total_kb = (total_bytes / 1024).max(1);
                (
                    path.display().to_string(),
                    format!("{total_kb} KB total ({} sidecar file(s))", sidecars.len()),
                    sidecars.len(),
                    total_bytes,
                )
            }
            _ => ("shellumi.db".to_string(), "Unknown size".to_string(), 0, 0),
        }
    }

    fn render_legacy_decision(&self, cx: &mut Context<Self>) -> Div {
        let (path_str, size_desc, _, _) = self.legacy_summary_text();
        let path_for_copy = path_str.clone();

        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_shrink_0()
                            .child(Icon::new(IconName::VaultEncryption).size(36.0)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Welcome back"),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("We found an existing local database from an earlier version of Shellumi. Choose how to proceed:"),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::WINDOW_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(format!("shellumi.db — {size_desc}")),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(path_str),
                            )
                            .child(
                                AppButton::sized(
                                    "legacy-copy-path",
                                    "Copy Path",
                                    ButtonVariant::Ghost,
                                    ButtonSize::Sm,
                                )
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        path_for_copy.clone(),
                                    ));
                                })),
                            ),
                    ),
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
        let (path_str, size_desc, _, total_bytes) = self.legacy_summary_text();
        let can_remove = self.controller.can_confirm_legacy_discard();

        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .w_full()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::DANGER))
                    .child("Remove existing database and start fresh?"),
            )
            .child(
                div()
                    .w_full()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Shellumi found data from an earlier installation. Continuing will permanently remove the existing local database and its related local database files. Your saved hosts, credentials, groups, snippets and any other data stored only in this database will no longer be available. This action cannot be undone by Shellumi after deletion completes."),
            )
            .child(
                div()
                    .w_full()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("We recommend creating a backup before continuing. Backups you create, other independent vaults, and Google Drive backups are not deleted by this operation. Local file removal does not guarantee forensic erasure from SSDs or OS snapshots."),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::WINDOW_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(format!(
                        "Target: {path_str}\nTotal size: {total_bytes} bytes ({size_desc})"
                    )),
            )
            .child(
                div()
                    .w_full()
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
                            div().flex_1().min_w_0().child(
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
                            ),
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
                div().w_full().min_w_0().child(
                    toggle_switch(
                        "discard-ack-checkbox",
                        self.controller.discard_acknowledged,
                        "I understand that my existing local database will be permanently removed.",
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.controller.discard_acknowledged =
                            !this.controller.discard_acknowledged;
                        cx.notify();
                    })),
                ),
            )
            .child(TextField::new(
                "Type DELETE to confirm permanent removal",
                self.discard_token_entity.clone(),
            ))
            .child(
                div()
                    .w_full()
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
            .w_full()
            .min_w_0()
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
                    .w_full()
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
            .w_full()
            .min_w_0()
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
                    .w_full()
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
            .w_full()
            .min_w_0()
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
                        .w_full()
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
            .w_full()
            .min_w_0()
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
                    .child("Each vault is encrypted with its own independent key."),
            )
            .when(is_migration && legacy_encrypted, |d| {
                d.child(PasswordField::new(
                    "Existing legacy database password",
                    self.legacy_password_input.clone(),
                ))
            })
            .child(
                PasswordField::new("Vault password", self.default_password_input.clone())
                    .required(true)
                    .helper(Some(
                        "Minimum 8 characters with uppercase, lowercase, number, and symbol.",
                    )),
            )
            .child(
                PasswordField::new(
                    "Confirm vault password",
                    self.default_password_confirm_input.clone(),
                )
                .required(true),
            )
            .child(
                div().w_full().min_w_0().child(
                    toggle_switch(
                        "vault-remember-keychain",
                        self.remember_in_keychain,
                        "Remember unlock key in OS Keychain",
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.remember_in_keychain = !this.remember_in_keychain;
                        cx.notify();
                    })),
                ),
            )
    }

    fn render_recovery_code(&self, cx: &mut Context<Self>) -> Div {
        let maybe_code = self.default_recovery_code.clone();

        div()
            .w_full()
            .min_w_0()
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
                    .w_full()
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
                            .child(
                                maybe_code
                                    .clone()
                                    .unwrap_or_else(|| "Recovery code is already sealed on this vault.".to_string()),
                            ),
                    )
                    .when_some(maybe_code, |d, code_for_copy| {
                        d.child(
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
                        )
                    }),
            )
            .child(
                div().w_full().min_w_0().child(
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
                ),
            )
    }

    fn render_migration_progress(&self) -> Div {
        div()
            .w_full()
            .min_w_0()
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
                    .child("Shellumi is backing up your legacy database, encrypting your vaults in an isolated worker, and verifying integrity. Your original database remains intact."),
            )
            .child(inline_notice(
                None,
                self.migration_progress_label.clone(),
                false,
            ))
    }

    fn render_local_unlock(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w_full()
            .min_w_0()
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
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_shrink_0()
                            .child(Icon::new(IconName::GoogleDrive).size(36.0)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
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
            .when_some(self.browser_launch_warning.as_ref(), |d, warn| {
                d.child(inline_notice(
                    None,
                    format!("{warn}. Copy the sign-in link below to complete sign-in in your browser."),
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
                        .when(self.google_discovery_error.is_some(), |d2| {
                            d2.child(
                                AppButton::sized(
                                    "google-retry-scan",
                                    "Retry Scan",
                                    ButtonVariant::Secondary,
                                    ButtonSize::Sm,
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.fetch_cloud_vaults(cx);
                                })),
                            )
                        })
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
                    div().w_full().min_w_0().child(
                        toggle_switch(
                            "google-sync-opt-in",
                            self.sync_default_opt_in,
                            "Enable encrypted Google Drive sync for Default Vault",
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sync_default_opt_in = !this.sync_default_opt_in;
                            cx.notify();
                        })),
                    ),
                )
            })
            .child(
                div()
                    .w_full()
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

    fn render_cloud_restore(&self, cx: &mut Context<Self>) -> Div {
        let selected_id = self.selected_cloud_vault_id.clone();
        let mut list = div().w_full().flex().flex_col().gap_2();
        for v in &self.cloud_vaults {
            let vid = v.id.clone();
            let is_sel = selected_id.as_deref() == Some(vid.as_str());
            let title = format!("{} ({})", v.name, v.id);
            let desc = format!(
                "{} • {} records",
                if v.is_default {
                    "Default Vault"
                } else {
                    "Secondary Vault"
                },
                v.record_count
            );
            list = list.child(
                div()
                    .id(SharedString::from(format!("cloud-vault-{}", v.id)))
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .bg(if is_sel {
                        rgba(0x0a84ff18)
                    } else {
                        rgb(MacTheme::WINDOW_BG)
                    })
                    .border_1()
                    .border_color(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_cloud_vault_id = Some(vid.clone());
                        cx.notify();
                    }))
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
                    ),
            );
        }

        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Restore from Google Drive"),
                    )
                    .child(
                        AppButton::sized(
                            "cloud-refresh-btn",
                            "Refresh",
                            ButtonVariant::Secondary,
                            ButtonSize::Sm,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.fetch_cloud_vaults(cx);
                        })),
                    ),
            )
            .when(self.cloud_vaults.is_empty(), |d| {
                d.child(inline_notice(
                    None,
                    "No encrypted vault backups were found in Google Drive AppData.",
                    false,
                ))
            })
            .child(list)
            .when_some(selected_id, |d, vid| {
                d.child(PasswordField::new(
                    "Vault password for restoration",
                    self.cloud_vault_password_input.clone(),
                ))
                .child(
                    AppButton::new(
                        "cloud-restore-selected-btn",
                        "Restore Selected Vault",
                        ButtonVariant::Primary,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.restore_selected_cloud_vault(vid.clone(), cx);
                    })),
                )
            })
    }

    fn render_additional_vaults(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("Additional Vaults (Optional)"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Create an independent local vault or connect to a Shellumi Server."),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        AppButton::sized(
                            "add-tab-local",
                            "Create Local Vault",
                            if !self.additional_tab_server {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            },
                            ButtonSize::Sm,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.additional_tab_server = false;
                            cx.notify();
                        })),
                    )
                    .child(
                        AppButton::sized(
                            "add-tab-server",
                            "Connect Shellumi Server",
                            if self.additional_tab_server {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Secondary
                            },
                            ButtonSize::Sm,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.additional_tab_server = true;
                            cx.notify();
                        })),
                    ),
            )
            .when(!self.additional_tab_server, |d| {
                d.child(TextField::new(
                    "Vault name",
                    self.sec_vault_name_input.clone(),
                ))
                .child(PasswordField::new(
                    "Secondary vault password",
                    self.sec_vault_password_input.clone(),
                ))
                .child(PasswordField::new(
                    "Confirm secondary password",
                    self.sec_vault_password_confirm_input.clone(),
                ))
                .child(
                    AppButton::new(
                        "sec-vault-create-btn",
                        "Create Secondary Vault",
                        ButtonVariant::Secondary,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.start_create_secondary_vault(cx);
                    })),
                )
            })
            .when(self.additional_tab_server, |d| {
                d.child(TextField::new("Server URL", self.server_url_input.clone()))
                    .child(TextField::new(
                        "Username or email",
                        self.server_user_input.clone(),
                    ))
                    .child(PasswordField::new(
                        "Password",
                        self.server_pass_input.clone(),
                    ))
                    .child(TextField::new(
                        "Device name",
                        self.server_device_name_input.clone(),
                    ))
                    .child(
                        AppButton::new(
                            "server-connect-btn",
                            "Connect & Enroll Device",
                            ButtonVariant::Secondary,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.start_connect_server(cx);
                        })),
                    )
            })
    }

    fn render_review_and_finish(&self, _cx: &mut Context<Self>) -> Div {
        let default_ok = self.inventory.has_valid_default_vault();
        let sync_label = if self.sync_default_opt_in && self.checkpoint_google_connected() {
            "Enabled (Default Vault)"
        } else {
            "Disabled (Local-only)"
        };

        div()
            .w_full()
            .min_w_0()
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
                    .w_full()
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
                    .child(format!("Google Drive Sync: {sync_label}"))
                    .child(format!("Total Vaults: {}", self.inventory.vaults.len())),
            )
    }

    fn choice_card(
        &self,
        id: &'static str,
        title: &'static str,
        desc: &'static str,
        selected: bool,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .w_full()
            .min_w_0()
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
                    .w_full()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(title),
            )
            .child(
                div()
                    .w_full()
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
            WizardPage::LegacyDecision | WizardPage::MigrationReview => {
                right = right.child(btn_or_disabled(
                    "wizard-next-btn",
                    "Continue",
                    ButtonVariant::Primary,
                    !is_busy,
                    Box::new(cx.listener(|this, _, _, cx| {
                        if this.legacy_choice_migrate {
                            this.controller.navigate_to(WizardPage::VaultPassword);
                        } else {
                            this.prepare_discard_confirmation(cx);
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
                let is_restore = self.controller.mode == SetupWizardMode::RestoreSetup
                    && !self.inventory.has_valid_default_vault();
                right = right
                    .child(btn_or_disabled(
                        "wizard-skip-drive",
                        "Skip",
                        ButtonVariant::Secondary,
                        !is_busy && !is_restore,
                        Box::new(cx.listener(|this, _, _, cx| {
                            this.controller.google_drive_skipped = true;
                            if let Err(e) = this.persist_checkpoint(|s| {
                                s.google_drive_skipped = true;
                                s.google_drive_sync_enabled = false;
                            }) {
                                this.set_local_validation_error(&e, cx);
                                return;
                            }
                            this.controller.navigate_to(WizardPage::AdditionalVaults);
                            cx.notify();
                        })),
                    ))
                    .child(btn_or_disabled(
                        "wizard-drive-next",
                        "Continue",
                        ButtonVariant::Primary,
                        !is_busy,
                        Box::new(cx.listener(|this, _, _, cx| {
                            if this.controller.mode == SetupWizardMode::RestoreSetup
                                && !this.inventory.has_valid_default_vault()
                            {
                                this.controller.navigate_to(WizardPage::CloudRestore);
                            } else {
                                this.controller.navigate_to(WizardPage::AdditionalVaults);
                            }
                            cx.notify();
                        })),
                    ));
            }
            WizardPage::CloudRestore => {
                right = right.child(btn_or_disabled(
                    "wizard-cloud-next",
                    "Continue",
                    ButtonVariant::Primary,
                    !is_busy && self.inventory.has_valid_default_vault(),
                    Box::new(cx.listener(|this, _, _, cx| {
                        this.controller.navigate_to(WizardPage::AdditionalVaults);
                        cx.notify();
                    })),
                ));
            }
            WizardPage::AdditionalVaults => {
                right = right.child(btn_or_disabled(
                    "wizard-add-next",
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
                    !is_busy && self.inventory.has_valid_default_vault(),
                    Box::new(cx.listener(|this, _, _, cx| {
                        this.finish_and_launch(cx);
                    })),
                ));
            }
            WizardPage::DiscardConfirm
            | WizardPage::InterruptedMigration
            | WizardPage::InterruptedDiscard
            | WizardPage::InconsistentInstallation
            | WizardPage::MigrationProgress => {}
        }

        div()
            .id("wizard-footer")
            .w_full()
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
        let main_w = if show_sidebar {
            (viewport_w - 220.0).max(320.0)
        } else {
            viewport_w.max(320.0)
        };
        let col_w = (main_w - 48.0).clamp(280.0, 520.0);
        let steps = self.route_steps();

        let top_chrome = div()
            .id("wizard-titlebar")
            .w_full()
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
            .w(px(col_w))
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
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .child(
                div()
                    .id("wizard-scroll-body")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .flex()
                    .justify_center()
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
                    .min_w_0()
                    .w_full()
                    .flex()
                    .flex_row()
                    .when(show_sidebar, |d| d.child(self.render_sidebar(&steps)))
                    .child(main_pane),
            )
    }
}

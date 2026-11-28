use gpui::AppContext;
use shellumi_app::{
    inspect_onboarding, LegacyDiscardEngine, OnboardingRoute, OperationPhase, OperationState,
    SetupState, SetupStep, SetupWizardMode, SetupWizardView, WizardPage,
};
use shellumi_core::CredentialStore;
use std::fs;
use tempfile::tempdir;

fn main() {
    gpui::Application::headless().run(|cx| {
        // 1. Fresh root -> FreshWelcome -> VaultPassword -> validation
        let fresh_dir = tempdir().unwrap();
        let (inv, res) = inspect_onboarding(fresh_dir.path(), false);
        assert_eq!(res.route, OnboardingRoute::Fresh);
        assert!(!inv.legacy.is_present());

        let wh_fresh = cx
            .open_window(gpui::WindowOptions::default(), |_, cx| {
                cx.new(|cx| SetupWizardView::new(fresh_dir.path().to_path_buf(), false, None, cx))
            })
            .unwrap();
        wh_fresh
            .update(cx, |view, _, cx| {
                assert_eq!(view.controller.current_page, WizardPage::FreshWelcome);
                assert!(view.controller.navigate_to(WizardPage::VaultPassword));
                assert_eq!(view.controller.current_page, WizardPage::VaultPassword);

                // Empty password rejected via single OperationState
                view.start_fresh_vault_creation(cx);
                assert!(matches!(
                    view.controller.operation,
                    OperationState::Failed { .. }
                ));
                assert!(view.controller.operation.success_summary().is_none());
            })
            .unwrap();

        // 2. Legacy DB + stale RestoreSetup checkpoint -> LegacyDecision -> DiscardConfirm gates
        let legacy_dir = tempdir().unwrap();
        let mut stale = SetupState::new();
        stale.mode = SetupWizardMode::RestoreSetup;
        stale.current_step = SetupStep::DefaultVault;
        stale.save(legacy_dir.path()).unwrap();
        fs::write(
            legacy_dir.path().join("shellumi.db"),
            b"SQLite format 3\0legacy-db",
        )
        .unwrap();
        fs::write(legacy_dir.path().join("shellumi.db-wal"), b"wal-bytes").unwrap();

        let (_, res_legacy) = inspect_onboarding(legacy_dir.path(), false);
        assert_eq!(res_legacy.route, OnboardingRoute::LegacyMigration);

        let wh_legacy = cx
            .open_window(gpui::WindowOptions::default(), |_, cx| {
                cx.new(|cx| SetupWizardView::new(legacy_dir.path().to_path_buf(), false, None, cx))
            })
            .unwrap();
        wh_legacy
            .update(cx, |view, _, cx| {
                assert_eq!(view.controller.current_page, WizardPage::LegacyDecision);
                assert!(view.controller.navigate_to(WizardPage::DiscardConfirm));
                assert!(!view.controller.can_confirm_legacy_discard());

                view.controller.discard_acknowledged = true;
                view.controller.discard_proceed_without_backup = true;
                view.controller.discard_token_input = "DELETE".to_string();
                assert!(view.controller.can_confirm_legacy_discard());

                // Going back resets confirmation gates and leaves legacy DB untouched
                assert!(view.controller.go_back());
                assert_eq!(view.controller.current_page, WizardPage::LegacyDecision);
                assert!(!view.controller.discard_acknowledged);
                assert!(view.controller.discard_token_input.is_empty());
                assert!(legacy_dir.path().join("shellumi.db").exists());
                let _ = cx;
            })
            .unwrap();

        // 3. Interrupted migration with stale checkpoint -> InterruptedMigration (never LocalUnlock!)
        let mig_dir = tempdir().unwrap();
        stale.save(mig_dir.path()).unwrap();
        fs::write(
            mig_dir.path().join("shellumi.db"),
            b"SQLite format 3\0legacy",
        )
        .unwrap();
        let staging = mig_dir.path().join("vaults/.migration_staging");
        fs::create_dir_all(staging.join("backup")).unwrap();
        fs::write(staging.join("backup/source.db"), b"SQLite format 3\0legacy").unwrap();
        let journal = serde_json::json!({
            "version": 2,
            "transaction": uuid::Uuid::new_v4().to_string(),
            "phase": "BackedUp",
            "source_files": { "": "abc" },
            "source_digest": null,
            "vaults": [],
            "activation": [],
            "report": {
                "legacy_db_found": true,
                "backup_path": null,
                "vaults_migrated": 0,
                "default_records_migrated": 0,
                "secondary_records_migrated": 0,
                "server_credentials_migrated": 0
            }
        });
        fs::write(
            staging.join("migration_journal.json"),
            serde_json::to_vec_pretty(&journal).unwrap(),
        )
        .unwrap();

        let wh_mig = cx
            .open_window(gpui::WindowOptions::default(), |_, cx| {
                cx.new(|cx| SetupWizardView::new(mig_dir.path().to_path_buf(), false, None, cx))
            })
            .unwrap();
        wh_mig
            .update(cx, |view, _, _| {
                assert_eq!(view.controller.route, OnboardingRoute::InterruptedMigration);
                assert_eq!(
                    view.controller.current_page,
                    WizardPage::InterruptedMigration
                );
                // Single-flight operation gate
                let (gen, op) = view
                    .controller
                    .try_start_operation(OperationPhase::Verifying, true)
                    .unwrap();
                assert!(view
                    .controller
                    .try_start_operation(OperationPhase::BackingUp, false)
                    .is_none());
                assert!(view.controller.finish_operation_ok(gen, op, "ok"));
                let _ = LegacyDiscardEngine::inspect_state(mig_dir.path());
                let _ = shellumi_core::EphemeralCredentialStore::new()
                    .get_secret("svc", "acc")
                    .ok();
            })
            .unwrap();

        cx.quit();
    });
    println!("gpui_onboarding_harness: PASS");
}

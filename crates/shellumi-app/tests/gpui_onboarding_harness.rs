use gpui::{px, size, AppContext, AssetSource, Bounds, WindowBounds, WindowOptions};
use shellumi_app::{
    inspect_onboarding, LegacyDiscardEngine, OnboardingRoute, OperationPhase, OperationState,
    SetupState, SetupStep, SetupWizardMode, SetupWizardView, ShellumiAssetSource, WizardPage,
};
use shellumi_core::CredentialStore;
use shellumi_storage::DiscardConfirmation;
use std::fs;
use tempfile::tempdir;

fn main() {
    // 1. Verify ShellumiAssetSource loads canonical PNGs (disk or embedded fallback)
    let asset_source = ShellumiAssetSource::new();
    for icon_key in [
        "icons/48/regular/shellumi_logo.png",
        "icons/48/regular/vault_encryption.png",
        "icons/48/regular/google_drive.png",
        "icons/48/regular/recovery_code.png",
    ] {
        let bytes = asset_source
            .load(icon_key)
            .expect("AssetSource::load must not error")
            .unwrap_or_else(|| panic!("Missing icon asset: {icon_key}"));
        assert!(
            bytes.len() > 8 && &bytes[..8] == b"\x89PNG\r\n\x1a\n",
            "Icon {icon_key} must be a valid PNG"
        );
    }
    // Path traversal must be rejected
    assert!(asset_source
        .load("../resources/logo.png")
        .unwrap()
        .is_none());

    gpui::Application::headless()
        .with_assets(ShellumiAssetSource::new())
        .run(|cx| {
            // 2. Fresh root across minimum (760x540) and default (920x640) viewport sizes
            let fresh_dir = tempdir().unwrap();
            let (inv, res) = inspect_onboarding(fresh_dir.path(), false);
            assert_eq!(res.route, OnboardingRoute::Fresh);
            assert!(!inv.legacy.is_present());

            for (w, h) in [(760.0_f32, 540.0_f32), (820.0, 600.0), (920.0, 640.0)] {
                let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
                let opts = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(760.0), px(540.0))),
                    ..Default::default()
                };
                let wh = match cx.open_window(opts, |_, cx| {
                    cx.new(|cx| {
                        SetupWizardView::new(fresh_dir.path().to_path_buf(), false, None, cx)
                    })
                }) {
                    Ok(wh) => wh,
                    Err(e)
                        if cfg!(target_os = "linux")
                            && e.to_string().contains("neither DISPLAY nor WAYLAND_DISPLAY") =>
                    {
                        println!(
                            "gpui_onboarding_harness: window creation skipped on headless Linux without DISPLAY/WAYLAND_DISPLAY"
                        );
                        cx.quit();
                        return;
                    }
                    Err(e) => panic!("open_window failed: {e}"),
                };
                wh.update(cx, |view, window, cx| {
                    let wb = window.bounds();
                    assert!(f32::from(wb.size.width) >= 760.0);
                    assert!(f32::from(wb.size.height) >= 540.0);
                    assert_eq!(view.controller.current_page, WizardPage::FreshWelcome);
                    assert!(view.controller.navigate_to(WizardPage::VaultPassword));
                    assert!(view.controller.navigate_to(WizardPage::GoogleDrive));
                    assert!(view.controller.navigate_to(WizardPage::CloudRestore));
                    assert!(view.controller.navigate_to(WizardPage::AdditionalVaults));
                    assert!(view.controller.navigate_to(WizardPage::ReviewAndFinish));
                    let _ = cx;
                })
                .unwrap();
            }

            // 3. Legacy DB + WAL: fingerprint-bound discard gate & stale-fingerprint rejection
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

            let fps = LegacyDiscardEngine::compute_source_fingerprints(legacy_dir.path()).unwrap();
            assert!(fps.contains_key(""));
            assert!(fps.contains_key("-wal"));

            // Mutate WAL after confirmation -> must reject stale fingerprints
            fs::write(
                legacy_dir.path().join("shellumi.db-wal"),
                b"wal-bytes-modified",
            )
            .unwrap();
            let store = shellumi_core::EphemeralCredentialStore::new();
            let err = LegacyDiscardEngine::execute_discard(
                legacy_dir.path(),
                &DiscardConfirmation {
                    acknowledged: true,
                    token: "DELETE".into(),
                    expected_source_fingerprints: Some(fps),
                },
                &store,
            )
            .unwrap_err();
            assert!(err.to_string().contains("changed since confirmation"));
            assert!(legacy_dir.path().join("shellumi.db").exists());

            // 4. Interrupted migration with stale checkpoint -> InterruptedMigration (never LocalUnlock!)
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
                .update(cx, |view, _, cx| {
                    assert_eq!(view.controller.route, OnboardingRoute::InterruptedMigration);
                    assert_eq!(
                        view.controller.current_page,
                        WizardPage::InterruptedMigration
                    );
                    view.start_fresh_vault_creation(cx);
                    assert!(matches!(
                        view.controller.operation,
                        OperationState::Failed { .. }
                    ));
                    let (gen, op) = view
                        .controller
                        .try_start_operation(OperationPhase::Verifying, true)
                        .unwrap();
                    assert!(view
                        .controller
                        .try_start_operation(OperationPhase::BackingUp, false)
                        .is_none());
                    assert!(view.controller.finish_operation_ok(gen, op, "ok"));

                    // 5. Failed MigrationProgress allows Back -> VaultPassword and exposes failure state
                    view.controller.route = OnboardingRoute::LegacyMigration;
                    view.controller.current_page = WizardPage::MigrationProgress;
                    let (fgen, fop) = view
                        .controller
                        .try_start_operation(OperationPhase::Verifying, false)
                        .unwrap();
                    assert!(!view.controller.can_go_back());
                    assert!(view.controller.finish_operation_err(
                        fgen,
                        fop,
                        shellumi_core::onboarding::UserFacingError::new(
                            "Migration stopped safely",
                            "Verification failed for vault legacy-local, table snippets: row_count_mismatch",
                            "E_MIGRATION",
                        ),
                        true,
                    ));
                    assert!(view.controller.can_go_back());
                    assert!(view.controller.go_back());
                    assert_eq!(view.controller.current_page, WizardPage::VaultPassword);
                    let _ = store.get_secret("svc", "acc").ok();
                })
                .unwrap();

            cx.quit();
        });
    println!("gpui_onboarding_harness: PASS");
}

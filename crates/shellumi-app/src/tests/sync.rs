#[test]
fn verifies_zero_knowledge_sync_payload_and_pkce_challenge() {
    use shellumi_core::{
        build_google_drive_auth_url, determine_sync_action, generate_pkce_challenge,
        EncryptedEnvelope, SyncAction, SyncEntityType, SyncRecord,
    };

    // 1. Verify PKCE Challenge
    let pkce = generate_pkce_challenge().unwrap();
    assert_eq!(pkce.code_verifier.len(), 43);
    assert_eq!(pkce.code_challenge.len(), 43);

    let auth_url = build_google_drive_auth_url(
        "client-id.apps.googleusercontent.com",
        "http://127.0.0.1:8080/oauth/callback",
        &pkce,
        "random_state_123",
    );
    assert!(auth_url.contains("client_id=client-id.apps.googleusercontent.com"));
    assert!(auth_url.contains("code_challenge="));
    assert!(auth_url.contains("drive.appdata"));

    // 2. Verify Sync Conflict Logic
    let env = EncryptedEnvelope {
        salt: "salt".into(),
        nonce: "nonce".into(),
        ciphertext: "cipher".into(),
        associated_data: "host:1".into(),
        kdf_iterations: 1000,
    };
    let local_rec = SyncRecord::new(
        "host-1",
        SyncEntityType::Host,
        2,
        "device-1",
        1000,
        env.clone(),
    );
    let remote_older = SyncRecord::new(
        "host-1",
        SyncEntityType::Host,
        1,
        "device-2",
        900,
        env.clone(),
    );
    let remote_newer = SyncRecord::new(
        "host-1",
        SyncEntityType::Host,
        3,
        "device-2",
        1100,
        env.clone(),
    );
    let remote_conflict = SyncRecord::new("host-1", SyncEntityType::Host, 2, "device-2", 1050, env);

    assert_eq!(
        determine_sync_action(Some(&local_rec), &remote_older),
        SyncAction::UploadLocal
    );
    assert_eq!(
        determine_sync_action(Some(&local_rec), &remote_newer),
        SyncAction::ApplyRemote
    );
    assert_eq!(
        determine_sync_action(Some(&local_rec), &remote_conflict),
        SyncAction::Conflict
    );
}

#[test]
fn formats_sync_status_labels_and_badge_colors() {
    use crate::AppSyncStatus;

    let local = AppSyncStatus::LocalOnly;
    assert_eq!(local.label(), "☁ Sync: Local only");
    assert_eq!(local.badge_color(), (0x37, 0x41, 0x51));

    let up_to_date = AppSyncStatus::UpToDate {
        last_sync_time: 12345,
    };
    assert_eq!(up_to_date.label(), "☁ Sync: Up to date");
    assert_eq!(up_to_date.badge_color(), (0x06, 0x5f, 0x46));

    let syncing = AppSyncStatus::Syncing;
    assert_eq!(syncing.label(), "☁ Syncing…");
    assert_eq!(syncing.badge_color(), (0x1e, 0x40, 0xaf));

    let conflict = AppSyncStatus::Conflict {
        message: "2 hosts changed concurrently".into(),
    };
    assert_eq!(
        conflict.label(),
        "⚠ Sync conflict: 2 hosts changed concurrently"
    );
    assert_eq!(conflict.badge_color(), (0x92, 0x40, 0x0e));

    let err = AppSyncStatus::Error("network unreachable".into());
    assert_eq!(err.label(), "⚠ Sync error: network unreachable");
    assert_eq!(err.badge_color(), (0x99, 0x1b, 0x1b));
}

#[test]
fn verifies_conflict_modal_state_and_resolution_flow() {
    use crate::ConflictModalState;
    use shellumi_core::{ConflictItemView, ConflictResolutionChoice, SyncEntityType};

    let items = vec![
        ConflictItemView {
            object_id: "host-1".into(),
            entity_type: SyncEntityType::Host,
            local_version: 2,
            remote_version: 2,
            local_updated_at: 1000,
            remote_updated_at: 1050,
            local_summary: "Web Prod (alice@web1:22)".into(),
            remote_summary: "Web Prod (bob@web1:2222)".into(),
        },
        ConflictItemView {
            object_id: "snippet-1".into(),
            entity_type: SyncEntityType::Snippet,
            local_version: 1,
            remote_version: 1,
            local_updated_at: 2000,
            remote_updated_at: 2050,
            local_summary: "Restart (systemctl restart)".into(),
            remote_summary: "Restart (service restart)".into(),
        },
    ];

    let mut modal = ConflictModalState {
        items,
        selected_index: 0,
        resolutions: vec![
            ConflictResolutionChoice::KeepLocal,
            ConflictResolutionChoice::KeepLocal,
        ],
        file_id: "vault_file_123".into(),
    };

    assert_eq!(modal.selected_index, 0);
    assert_eq!(modal.resolutions[0], ConflictResolutionChoice::KeepLocal);
    assert_eq!(modal.resolutions[1], ConflictResolutionChoice::KeepLocal);

    // Select second item and change resolution to AcceptRemote
    modal.selected_index = 1;
    modal.resolutions[1] = ConflictResolutionChoice::AcceptRemote;

    assert_eq!(modal.selected_index, 1);
    assert_eq!(modal.resolutions[0], ConflictResolutionChoice::KeepLocal);
    assert_eq!(modal.resolutions[1], ConflictResolutionChoice::AcceptRemote);
    assert_eq!(modal.resolutions[0].to_string(), "Keep Local");
    assert_eq!(modal.resolutions[1].to_string(), "Accept Remote");
}

#[test]
fn verifies_sync_scheduler_and_local_mutation_integration() {
    use shellumi_core::{SyncScheduler, SyncTrigger};

    let mut scheduler = SyncScheduler::new(5, 300);
    assert_eq!(scheduler.check_trigger(100), Some(SyncTrigger::Startup));

    // After startup sync success
    scheduler.record_success(100);

    // After local mutation, should not trigger immediately due to 5s debounce window
    scheduler.notify_local_mutation(100);
    assert_eq!(scheduler.check_trigger(102), None);
    assert_eq!(
        scheduler.check_trigger(105),
        Some(SyncTrigger::LocalMutation)
    );

    // On failure, scheduler applies backoff delay
    scheduler.record_failure(105);
    assert_eq!(scheduler.consecutive_failures, 1);
    assert_eq!(scheduler.check_trigger(106), None);
    assert_eq!(
        scheduler.check_trigger(120),
        Some(SyncTrigger::LocalMutation)
    );

    // On success, failures reset
    scheduler.record_success(120);
    assert_eq!(scheduler.consecutive_failures, 0);
    assert_eq!(scheduler.last_sync_time, Some(120));
}

#[test]
fn verifies_sync_cycle_execution_and_conflict_resolution_in_app() {
    use crate::*;

    let dummy_env = EncryptedEnvelope {
        salt: "s".into(),
        nonce: "n".into(),
        ciphertext: "c".into(),
        associated_data: "host".into(),
        kdf_iterations: 1000,
    };

    let rec_local = SyncRecord::new(
        "host:1",
        SyncEntityType::Host,
        1,
        "devA",
        1000,
        dummy_env.clone(),
    );
    let rec_remote = SyncRecord::new(
        "host:1",
        SyncEntityType::Host,
        1,
        "devA",
        1000,
        dummy_env.clone(),
    );

    // 1. Identical records -> AlreadySynchronized
    let res = execute_sync_cycle(
        std::slice::from_ref(&rec_local),
        std::slice::from_ref(&rec_remote),
        "default-pass",
        None,
    );
    assert!(matches!(res, SyncCycleResult::AlreadySynchronized));

    // 2. Conflicting records -> Conflicts
    let rec_remote_conflict = SyncRecord::new(
        "host:1",
        SyncEntityType::Host,
        1,
        "devB",
        1050,
        dummy_env.clone(),
    );

    let res_conflict = execute_sync_cycle(
        std::slice::from_ref(&rec_local),
        std::slice::from_ref(&rec_remote_conflict),
        "default-pass",
        None,
    );
    match res_conflict {
        SyncCycleResult::Conflicts(conflicts) => {
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].object_id, "host:1");
        }
        _ => panic!("Expected conflict"),
    }

    // 3. User resolution -> Success
    let resolutions = vec![ConflictResolutionChoice::AcceptRemote];

    let res_resolved = execute_sync_cycle(
        std::slice::from_ref(&rec_local),
        std::slice::from_ref(&rec_remote_conflict),
        "default-pass",
        Some(&resolutions),
    );
    match res_resolved {
        SyncCycleResult::Success {
            report,
            merged_records,
        } => {
            assert_eq!(report.conflicts_resolved, 1);
            assert_eq!(merged_records.len(), 1);
        }
        _ => panic!("Expected successful sync after resolution"),
    }
}

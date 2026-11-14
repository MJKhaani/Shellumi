mod conflicts;
mod crypto;
mod drive;
mod http;
mod oauth;
mod scheduler;

#[test]
fn private_http_config_escapes_data_and_rejects_header_injection() {
    let request =
        super::HttpRequest::post("https://example.org", b"{\"value\":\"hello\"}".to_vec())
            .with_header("Authorization", "Bearer private-token");
    let config = super::curl_private_config(&request).unwrap();
    assert!(config.contains("header = \"Authorization: Bearer private-token\""));
    assert!(config.contains("data-raw = "));
    assert!(super::curl_private_config(
        &super::HttpRequest::get("https://example.org")
            .with_header("Authorization", "token\nurl=https://evil.example")
    )
    .is_err());
}

use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TestHost {
    name: String,
    hostname: String,
    port: u16,
}

#[test]
fn determines_sync_actions_correctly() {
    let salt = generate_salt().unwrap();
    let nonce = generate_nonce().unwrap();
    let dummy_env = EncryptedEnvelope {
        salt: BASE64_STANDARD.encode(salt),
        nonce: BASE64_STANDARD.encode(nonce),
        ciphertext: "dummy".into(),
        associated_data: "host:1".into(),
        kdf_iterations: 1000,
    };

    let local_v1 = SyncRecord::new("1", SyncEntityType::Host, 1, "devA", 100, dummy_env.clone());
    let remote_v2 = SyncRecord::new("1", SyncEntityType::Host, 2, "devB", 200, dummy_env.clone());
    let remote_v1_same =
        SyncRecord::new("1", SyncEntityType::Host, 1, "devA", 100, dummy_env.clone());
    let remote_v1_conflict =
        SyncRecord::new("1", SyncEntityType::Host, 1, "devB", 150, dummy_env.clone());

    // Local missing
    assert_eq!(
        determine_sync_action(None, &remote_v2),
        SyncAction::ApplyRemote
    );

    // Remote newer
    assert_eq!(
        determine_sync_action(Some(&local_v1), &remote_v2),
        SyncAction::ApplyRemote
    );

    // Local newer
    assert_eq!(
        determine_sync_action(Some(&remote_v2), &local_v1),
        SyncAction::UploadLocal
    );

    // In sync
    assert_eq!(
        determine_sync_action(Some(&local_v1), &remote_v1_same),
        SyncAction::InSync
    );

    // Concurrent edit on different devices
    assert_eq!(
        determine_sync_action(Some(&local_v1), &remote_v1_conflict),
        SyncAction::Conflict
    );
}

#[test]
fn creates_and_serializes_vault_snapshot() {
    let mut vault = SyncVault::new("test-vault-123", 1700000000);
    let tombstone = SyncRecord::tombstone(
        "deleted-host-1",
        SyncEntityType::Host,
        3,
        "devA",
        1700000100,
    );
    vault.records.push(tombstone);

    let json = vault.to_json().unwrap();
    assert!(json.contains("test-vault-123"));
    assert!(json.contains("deleted-host-1"));

    let restored = SyncVault::from_json(&json).unwrap();
    assert_eq!(restored, vault);
}

#[test]
fn verifies_plan_sync_cycle() {
    let salt = generate_salt().unwrap();
    let nonce = generate_nonce().unwrap();
    let dummy_env = EncryptedEnvelope {
        salt: BASE64_STANDARD.encode(salt),
        nonce: BASE64_STANDARD.encode(nonce),
        ciphertext: "c".into(),
        associated_data: "host:1".into(),
        kdf_iterations: 1000,
    };

    let local_record = SyncRecord::new(
        "host1",
        SyncEntityType::Host,
        1,
        "devA",
        100,
        dummy_env.clone(),
    );
    let mut local_vault = SyncVault::new("snap1", 100);
    local_vault.records.push(local_record.clone());

    // 1. Remote file missing -> UploadInitial
    assert_eq!(
        plan_sync_cycle(&local_vault.records, None, None, None),
        SyncPlan::UploadInitial
    );

    let remote_meta = DriveFileMetadata {
        id: "remote_file_1".into(),
        name: "shellumi_vault.json".into(),
        mime_type: "application/json".into(),
        modified_time: None,
        size: None,
    };

    // 2. Remote file exists, but local is empty and remote vault not downloaded -> DownloadInitial
    assert_eq!(
        plan_sync_cycle(&[], Some(&remote_meta), None, None),
        SyncPlan::DownloadInitial {
            file_id: "remote_file_1".into(),
        }
    );

    // 3. Both local and remote exist with identical records -> AlreadySynchronized
    let mut remote_vault_identical = SyncVault::new("snap2", 100);
    remote_vault_identical.records.push(local_record.clone());
    assert_eq!(
        plan_sync_cycle(
            &local_vault.records,
            Some(&remote_meta),
            Some(&remote_vault_identical),
            None
        ),
        SyncPlan::AlreadySynchronized
    );

    // 4. Local has newer version -> UploadLocalUpdates
    let local_newer_rec = SyncRecord::new(
        "host1",
        SyncEntityType::Host,
        2,
        "devA",
        200,
        dummy_env.clone(),
    );
    assert_eq!(
        plan_sync_cycle(
            std::slice::from_ref(&local_newer_rec),
            Some(&remote_meta),
            Some(&remote_vault_identical),
            None
        ),
        SyncPlan::UploadLocalUpdates {
            file_id: "remote_file_1".into(),
        }
    );

    // 5. Remote has newer version -> ApplyRemoteUpdates
    let remote_newer_rec = SyncRecord::new(
        "host1",
        SyncEntityType::Host,
        2,
        "devB",
        200,
        dummy_env.clone(),
    );
    let mut remote_vault_newer = SyncVault::new("snap3", 200);
    remote_vault_newer.records.push(remote_newer_rec);
    assert_eq!(
        plan_sync_cycle(
            &local_vault.records,
            Some(&remote_meta),
            Some(&remote_vault_newer),
            None
        ),
        SyncPlan::ApplyRemoteUpdates {
            file_id: "remote_file_1".into(),
        }
    );

    // 6. Conflicting versions across different devices -> ConflictDetected
    let remote_conflict_rec =
        SyncRecord::new("host1", SyncEntityType::Host, 1, "devB", 150, dummy_env);
    let mut remote_vault_conflict = SyncVault::new("snap4", 150);
    remote_vault_conflict.records.push(remote_conflict_rec);
    assert_eq!(
        plan_sync_cycle(
            &local_vault.records,
            Some(&remote_meta),
            Some(&remote_vault_conflict),
            None
        ),
        SyncPlan::ConflictDetected {
            file_id: "remote_file_1".into(),
            conflicting_object_ids: vec!["host1".into()],
        }
    );
}

#[test]
fn verifies_execute_sync_cycle_and_resolution() {
    let passphrase = "test-secret-passphrase";

    #[derive(Serialize)]
    struct SimpleHost {
        name: String,
        port: u16,
    }

    let loc_env = encrypt_entity_with_iterations(
        passphrase,
        "host",
        "srv-1",
        &SimpleHost {
            name: "Server 1".into(),
            port: 22,
        },
        1000,
    )
    .unwrap();

    let rem_env = encrypt_entity_with_iterations(
        passphrase,
        "host",
        "srv-1",
        &SimpleHost {
            name: "Server 1 Remote".into(),
            port: 2222,
        },
        1000,
    )
    .unwrap();

    let local_rec = SyncRecord::new("srv-1", SyncEntityType::Host, 1, "dev1", 100, loc_env);
    let remote_rec = SyncRecord::new("srv-1", SyncEntityType::Host, 1, "dev2", 105, rem_env);

    // 1. Conflict detected without resolutions
    let res = execute_sync_cycle(
        std::slice::from_ref(&local_rec),
        std::slice::from_ref(&remote_rec),
        passphrase,
        None,
    );
    match res {
        SyncCycleResult::Conflicts(views) => {
            assert_eq!(views.len(), 1);
            assert_eq!(views[0].object_id, "srv-1");
        }
        _ => panic!("expected conflicts"),
    }

    // 2. Conflict resolved with KeepLocal
    let res_keep_local = execute_sync_cycle(
        std::slice::from_ref(&local_rec),
        std::slice::from_ref(&remote_rec),
        passphrase,
        Some(&[ConflictResolutionChoice::KeepLocal]),
    );
    match res_keep_local {
        SyncCycleResult::Success {
            report,
            merged_records,
        } => {
            assert_eq!(report.conflicts_resolved, 1);
            assert_eq!(merged_records.len(), 1);
            assert_eq!(merged_records[0].device_id, "dev1"); // local retained
        }
        _ => panic!("expected success"),
    }

    // 3. Conflict resolved with AcceptRemote
    let res_accept_remote = execute_sync_cycle(
        &[local_rec],
        &[remote_rec],
        passphrase,
        Some(&[ConflictResolutionChoice::AcceptRemote]),
    );
    match res_accept_remote {
        SyncCycleResult::Success {
            report,
            merged_records,
        } => {
            assert_eq!(report.conflicts_resolved, 1);
            assert_eq!(merged_records.len(), 1);
            assert_eq!(merged_records[0].device_id, "dev2"); // remote accepted
        }
        _ => panic!("expected success"),
    }
}

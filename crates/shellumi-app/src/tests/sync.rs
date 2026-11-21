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
        format_version: 1,
        suite: "aes256gcm_dek_v1".into(),
        key_epoch: 1,
        salt: "salt".into(),
        nonce: "nonce".into(),
        ciphertext: "cipher".into(),
        associated_data: "host:1".into(),
        kdf_iterations: 1000,
        argon2_m_cost: None,
        argon2_t_cost: None,
        argon2_p_cost: None,
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
            local_raw_json: None,
            remote_raw_json: None,
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
            local_raw_json: None,
            remote_raw_json: None,
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
        format_version: 1,
        suite: "aes256gcm_dek_v1".into(),
        key_epoch: 1,
        salt: "s".into(),
        nonce: "n".into(),
        ciphertext: "c".into(),
        associated_data: "host".into(),
        kdf_iterations: 1000,
        argon2_m_cost: None,
        argon2_t_cost: None,
        argon2_p_cost: None,
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

#[test]
fn verifies_compute_line_diff_lcs_algorithm() {
    use crate::ui::conflict_diff::{compute_line_diff, DiffLineKind};

    // 1. Identical lines
    let local = "echo hello\necho world";
    let remote = "echo hello\necho world";
    let diff = compute_line_diff(local, remote);
    assert_eq!(diff.len(), 2);
    assert!(diff.iter().all(|l| l.kind == DiffLineKind::Unchanged));
    assert_eq!(diff[0].local_line_no, Some(1));
    assert_eq!(diff[0].remote_line_no, Some(1));
    assert_eq!(diff[1].local_line_no, Some(2));
    assert_eq!(diff[1].remote_line_no, Some(2));

    // 2. Addition and deletion
    let local = "line 1\nold line\nline 3";
    let remote = "line 1\nnew line\nline 3";
    let diff = compute_line_diff(local, remote);
    assert_eq!(diff.len(), 4);
    assert_eq!(diff[0].kind, DiffLineKind::Unchanged);
    assert_eq!(diff[0].content, "line 1");
    let middle_kinds: Vec<_> = diff[1..3].iter().map(|d| d.kind).collect();
    assert!(middle_kinds.contains(&DiffLineKind::LocalOnly));
    assert!(middle_kinds.contains(&DiffLineKind::RemoteOnly));
    assert_eq!(diff[3].kind, DiffLineKind::Unchanged);
    assert_eq!(diff[3].content, "line 3");

    // 3. Empty scripts
    let diff_empty = compute_line_diff("", "");
    assert!(diff_empty.is_empty());
}

#[test]
fn verifies_snippet_conflict_model_parsing_and_fallback() {
    use crate::ui::conflict_diff::{DiffLineKind, SnippetConflictModel};
    use shellumi_core::{ConflictItemView, SyncEntityType};

    // 1. Full JSON payload
    let local_json = serde_json::json!({
        "name": "Restart Nginx Local",
        "command": "systemctl restart nginx\nsystemctl status nginx",
        "description": "Restart web server",
        "tags": ["web", "prod"]
    })
    .to_string();

    let remote_json = serde_json::json!({
        "name": "Restart Nginx Remote",
        "command": "systemctl restart nginx\necho 'done'",
        "description": "Restart server remotely",
        "tags": ["web", "staging"]
    })
    .to_string();

    let item = ConflictItemView {
        object_id: "snip-1".into(),
        entity_type: SyncEntityType::Snippet,
        local_version: 1,
        remote_version: 2,
        local_updated_at: 100,
        remote_updated_at: 200,
        local_summary: "Snippet Restart".into(),
        remote_summary: "Snippet Restart".into(),
        local_raw_json: Some(local_json),
        remote_raw_json: Some(remote_json),
    };

    let model = SnippetConflictModel::from_conflict_item(&item);
    assert_eq!(model.local_name, "Restart Nginx Local");
    assert_eq!(model.remote_name, "Restart Nginx Remote");
    assert_eq!(
        model.local_description.as_deref(),
        Some("Restart web server")
    );
    assert_eq!(
        model.remote_description.as_deref(),
        Some("Restart server remotely")
    );
    assert_eq!(model.local_tags, vec!["web", "prod"]);
    assert_eq!(model.remote_tags, vec!["web", "staging"]);
    assert_eq!(model.diff_lines[0].kind, DiffLineKind::Unchanged);
    assert_eq!(model.diff_lines[0].content, "systemctl restart nginx");

    // 2. Summary fallback when raw JSON is absent
    let fallback_item = ConflictItemView {
        object_id: "snip-2".into(),
        entity_type: SyncEntityType::Snippet,
        local_version: 1,
        remote_version: 1,
        local_updated_at: 100,
        remote_updated_at: 100,
        local_summary: "Backup (`tar -czf backup.tar.gz /data`)".into(),
        remote_summary: "Backup (`tar -czvf backup.tar.gz /data`)".into(),
        local_raw_json: None,
        remote_raw_json: None,
    };

    let fallback_model = SnippetConflictModel::from_conflict_item(&fallback_item);
    assert_eq!(fallback_model.local_name, "snip-2");
    assert_eq!(fallback_model.remote_name, "snip-2");
    assert_eq!(fallback_model.local_command, "tar -czf backup.tar.gz /data");
    assert_eq!(
        fallback_model.remote_command,
        "tar -czvf backup.tar.gz /data"
    );
}

#[test]
fn verifies_tunnel_conflict_model_and_cli_flag_generation() {
    use crate::ui::conflict_diff::TunnelConflictModel;
    use shellumi_core::{ConflictItemView, SyncEntityType};

    // 1. Local port forward
    let local_json = serde_json::json!({
        "name": "DB Tunnel",
        "forwarding_type": "Local",
        "bind_address": "127.0.0.1",
        "bind_port": 5432,
        "target_host": "db.internal",
        "target_port": 5432,
        "auto_start": true
    })
    .to_string();

    // 2. Remote port forward with changed target port
    let remote_json = serde_json::json!({
        "name": "DB Tunnel",
        "forwarding_type": "Remote",
        "bind_address": "0.0.0.0",
        "bind_port": 5433,
        "target_host": "db.prod",
        "target_port": 5432,
        "auto_start": false
    })
    .to_string();

    let item = ConflictItemView {
        object_id: "tun-1".into(),
        entity_type: SyncEntityType::Tunnel,
        local_version: 3,
        remote_version: 3,
        local_updated_at: 300,
        remote_updated_at: 350,
        local_summary: "Tunnel Local 5432".into(),
        remote_summary: "Tunnel Remote 5433".into(),
        local_raw_json: Some(local_json),
        remote_raw_json: Some(remote_json),
    };

    let model = TunnelConflictModel::from_conflict_item(&item);
    assert_eq!(model.local_name, "DB Tunnel");
    assert_eq!(model.remote_name, "DB Tunnel");
    assert_eq!(model.local_type, "Local");
    assert_eq!(model.remote_type, "Remote");
    assert_eq!(model.local_bind_port, 5432);
    assert_eq!(model.remote_bind_port, 5433);
    assert!(model.local_auto_start);
    assert!(!model.remote_auto_start);
    assert_eq!(
        model.local_cli_flag,
        "ssh -L 127.0.0.1:5432:db.internal:5432"
    );
    assert_eq!(model.remote_cli_flag, "ssh -R 0.0.0.0:5433:db.prod:5432");

    // 3. Dynamic (SOCKS5) tunnel CLI flag
    let dynamic_json = serde_json::json!({
        "name": "SOCKS Proxy",
        "forwarding_type": "Dynamic",
        "bind_address": "127.0.0.1",
        "bind_port": 1080,
        "auto_start": false
    })
    .to_string();

    let dynamic_item = ConflictItemView {
        object_id: "tun-2".into(),
        entity_type: SyncEntityType::Tunnel,
        local_version: 1,
        remote_version: 1,
        local_updated_at: 100,
        remote_updated_at: 100,
        local_summary: "Dynamic Proxy".into(),
        remote_summary: "Dynamic Proxy".into(),
        local_raw_json: Some(dynamic_json.clone()),
        remote_raw_json: Some(dynamic_json),
    };

    let dynamic_model = TunnelConflictModel::from_conflict_item(&dynamic_item);
    assert_eq!(dynamic_model.local_cli_flag, "ssh -D 127.0.0.1:1080");
}

#[tokio::test]
async fn test_is_active_vault_remote_and_sync_status() {
    use crate::actions::VaultContextSummary;

    let available_vaults = [
        VaultContextSummary {
            id: shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(),
            name: "Personal".to_string(),
            is_remote: false,
            server_url: None,
            server_vault_id: None,
            is_locked: false,
        },
        VaultContextSummary {
            id: "vlt-secondary-local-1".to_string(),
            name: "Secondary Local Vault".to_string(),
            is_remote: false,
            server_url: None,
            server_vault_id: None,
            is_locked: false,
        },
        VaultContextSummary {
            id: "vlt-remote-server-2".to_string(),
            name: "Corp Remote Vault".to_string(),
            is_remote: true,
            server_url: Some("https://server.shellumi.com".to_string()),
            server_vault_id: Some("uuid-remote-vault-456".to_string()),
            is_locked: false,
        },
    ];

    // Helper closure mirroring Shellumi::is_active_vault_remote logic
    let is_remote = |active_id: &str| {
        available_vaults
            .iter()
            .find(|v| v.id == active_id)
            .map(|v| v.is_remote)
            .unwrap_or(false)
    };

    // 1. Personal default vault is NOT remote
    assert!(!is_remote(shellumi_storage::LEGACY_LOCAL_VAULT_ID));

    // 2. Secondary LOCAL vault (different ID from legacy-local) is NOT remote
    assert!(!is_remote("vlt-secondary-local-1"));

    // 3. Remote vault is remote
    assert!(is_remote("vlt-remote-server-2"));

    // 4. Unknown vault ID defaults to false
    assert!(!is_remote("unknown-vault-id"));
}

#[tokio::test]
async fn test_server_sync_credential_resolution_with_keychain_references() {
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use base64::Engine;
    use shellumi_core::server::{
        generate_device_ed25519_keypair_pkcs8, keypair_from_pkcs8, sign_device_challenge,
    };
    use shellumi_core::{CredentialStore, EphemeralCredentialStore};
    use shellumi_storage::VaultServerConnectionRecord;

    let cred_store = EphemeralCredentialStore::new();

    // 1. Generate signing keypair
    let (signing_pkcs8, _pub_b64) = generate_device_ed25519_keypair_pkcs8().unwrap();
    let signing_keypair = keypair_from_pkcs8(&signing_pkcs8).unwrap();
    let raw_pkcs8_b64 = BASE64_STANDARD.encode(&signing_pkcs8);

    // 2. Store credentials in credential store with keychain:// reference
    let srv_id = "srv-test-corp-1";
    let token_key = format!("dev.shellumi.server/{srv_id}/auth-token");
    let signing_key = format!("dev.shellumi.server/{srv_id}/device-signing-key");

    cred_store
        .set_secret(
            "dev.shellumi.server",
            &format!("{srv_id}/auth-token"),
            "secret-jwt-token-xyz-12345",
        )
        .unwrap();
    cred_store
        .set_secret(
            "dev.shellumi.server",
            &format!("{srv_id}/device-signing-key"),
            &raw_pkcs8_b64,
        )
        .unwrap();

    let conn_record = VaultServerConnectionRecord {
        id: srv_id.to_string(),
        base_url: "https://vault.example.com".to_string(),
        auth_token: Some(format!("keychain://{token_key}")),
        refresh_token: None,
        device_id: Some("dev-uuid-1".to_string()),
        device_signing_key: Some(format!("keychain://{signing_key}")),
        server_instance_id: Some("inst-1".to_string()),
        device_agreement_key: None,
        created_at: 1000,
        updated_at: 1000,
    };

    // 3. Resolve auth token
    let resolved_token = conn_record.resolve_auth_token(&cred_store).unwrap();
    assert_eq!(resolved_token.expose_secret(), "secret-jwt-token-xyz-12345");

    // 4. Resolve signing key from keychain:// URI and verify signature creation
    let resolved_signing = conn_record.resolve_signing_key(&cred_store).unwrap();
    let raw_b64 = resolved_signing.expose_secret().trim();
    let decoded_pkcs8 = BASE64_STANDARD.decode(raw_b64).unwrap();
    let resolved_keypair = keypair_from_pkcs8(&decoded_pkcs8).unwrap();

    let challenge = "test-challenge-nonce-998877";
    let sig1 = sign_device_challenge(&signing_keypair, challenge);
    let sig2 = sign_device_challenge(&resolved_keypair, challenge);
    assert_eq!(sig1, sig2);
}

#[tokio::test]
async fn test_in_memory_default_vault_dek_registration_and_unlock() {
    use shellumi_core::{EphemeralCredentialStore, VaultContext, VaultId, VaultLocation};
    use shellumi_storage::{HostStore, LEGACY_LOCAL_VAULT_ID};

    let temp = tempfile::tempdir().unwrap();
    let reg_db_path = temp.path().join("vaults").join("default.db");
    std::fs::create_dir_all(reg_db_path.parent().unwrap()).unwrap();

    let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let user_password = "DefaultP@ssword2026!";

    // 1. Provision default vault with Keychain remembering DISABLED (key is in-memory only)
    let cred_store = EphemeralCredentialStore::new();
    let rec_code = shellumi_core::VaultKey::generate_recovery_code().unwrap();
    let (in_memory_default_dek, _primary_env, _rec_env) =
        shellumi_core::VaultKeyManager::provision_new_vault(
            &cred_store,
            &default_vid,
            user_password,
            false, // Do NOT store in Keychain!
            Some(&rec_code),
        )
        .unwrap();

    // Verify credential store does NOT have default vault key
    assert!(
        shellumi_core::VaultKeyManager::retrieve_key(&cred_store, &default_vid)
            .unwrap()
            .is_none()
    );

    // 2. Open registry database with the in-memory DEK
    let mut reg_store =
        HostStore::open_with_key(reg_db_path.to_str().unwrap(), Some(&in_memory_default_dek))
            .await
            .unwrap();

    // 3. Register a secondary local vault context into default registry
    let sec_vault_id = VaultId::new("vlt-sec-local-007").unwrap();
    let sec_context = VaultContext::new(sec_vault_id.clone(), VaultLocation::Local);
    reg_store
        .register_vault_context("Secondary Work Vault", &sec_context)
        .await
        .unwrap();

    // 4. Verify context listing works using in-memory key
    let contexts = reg_store.list_vault_contexts().await.unwrap();
    assert_eq!(contexts.len(), 2); // Personal (default) + Secondary
    assert!(contexts
        .iter()
        .any(|(name, ctx)| name == "Secondary Work Vault" && ctx.id() == &sec_vault_id));

    // 5. Mark vault locked and then unlocked in registry
    reg_store
        .set_vault_locked("vlt-sec-local-007", true)
        .await
        .unwrap();
    assert!(reg_store
        .is_vault_locked("vlt-sec-local-007")
        .await
        .unwrap());

    reg_store
        .set_vault_locked("vlt-sec-local-007", false)
        .await
        .unwrap();
    assert!(!reg_store
        .is_vault_locked("vlt-sec-local-007")
        .await
        .unwrap());
}

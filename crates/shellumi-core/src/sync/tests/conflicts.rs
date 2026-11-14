use super::*;

#[test]
fn computes_record_diffs_correctly() {
    let salt = generate_salt().unwrap();
    let nonce = generate_nonce().unwrap();
    let dummy_env = EncryptedEnvelope {
        salt: BASE64_STANDARD.encode(salt),
        nonce: BASE64_STANDARD.encode(nonce),
        ciphertext: "c".into(),
        associated_data: "host:1".into(),
        kdf_iterations: 1000,
    };

    let local1 = SyncRecord::new(
        "host1",
        SyncEntityType::Host,
        2,
        "devA",
        200,
        dummy_env.clone(),
    );
    let local2 = SyncRecord::new(
        "host2",
        SyncEntityType::Host,
        1,
        "devA",
        100,
        dummy_env.clone(),
    );
    let local3 = SyncRecord::new(
        "host3",
        SyncEntityType::Host,
        1,
        "devA",
        100,
        dummy_env.clone(),
    );

    let remote1 = SyncRecord::new(
        "host1",
        SyncEntityType::Host,
        1,
        "devB",
        100,
        dummy_env.clone(),
    );
    let remote2 = SyncRecord::new(
        "host2",
        SyncEntityType::Host,
        3,
        "devB",
        300,
        dummy_env.clone(),
    );
    let remote3 = SyncRecord::new(
        "host3",
        SyncEntityType::Host,
        1,
        "devB",
        150,
        dummy_env.clone(),
    );
    let remote4 = SyncRecord::new("host4", SyncEntityType::Host, 1, "devB", 100, dummy_env);

    let local_records = vec![local1, local2, local3];
    let remote_records = vec![remote1, remote2, remote3, remote4];

    let diffs = diff_sync_records(&local_records, &remote_records);
    assert_eq!(diffs.len(), 4);

    // host1: local version 2 > remote 1 => LocalNewer
    let d1 = diffs.iter().find(|d| d.object_id == "host1").unwrap();
    assert_eq!(d1.status, DiffStatus::LocalNewer);

    // host2: remote version 3 > local 1 => RemoteNewer
    let d2 = diffs.iter().find(|d| d.object_id == "host2").unwrap();
    assert_eq!(d2.status, DiffStatus::RemoteNewer);

    // host3: same version 1, different devices => Conflict
    let d3 = diffs.iter().find(|d| d.object_id == "host3").unwrap();
    assert_eq!(d3.status, DiffStatus::Conflict);

    // host4: only in remote => RemoteOnly
    let d4 = diffs.iter().find(|d| d.object_id == "host4").unwrap();
    assert_eq!(d4.status, DiffStatus::RemoteOnly);
}

#[test]
fn verifies_diff_with_passphrase_detects_identical_payloads() {
    #[derive(Serialize, Deserialize, PartialEq, Eq)]
    struct SampleData {
        field: String,
    }

    let passphrase = "test-sync-passphrase-99";
    let data = SampleData {
        field: "same_value".into(),
    };

    // Two envelopes sealed separately with different salts and nonces
    let env_a = encrypt_entity_with_iterations(passphrase, "sample", "obj1", &data, 1000).unwrap();
    let env_b = encrypt_entity_with_iterations(passphrase, "sample", "obj1", &data, 1000).unwrap();

    let rec_a = SyncRecord::new("obj1", SyncEntityType::Host, 1, "dev1", 100, env_a);
    let rec_b = SyncRecord::new("obj1", SyncEntityType::Host, 1, "dev2", 200, env_b);

    // Without passphrase: looks like conflict due to different devices
    let diffs_no_pass =
        diff_sync_records(std::slice::from_ref(&rec_a), std::slice::from_ref(&rec_b));
    assert_eq!(diffs_no_pass[0].status, DiffStatus::Conflict);

    // With passphrase: decrypts and discovers payloads are identical
    let diffs_with_pass = diff_sync_records_with_passphrase(
        std::slice::from_ref(&rec_a),
        std::slice::from_ref(&rec_b),
        Some(passphrase),
    );
    assert_eq!(diffs_with_pass[0].status, DiffStatus::Identical);
}

#[test]
fn verifies_resolve_record_conflict_choices() {
    let dummy_env = EncryptedEnvelope {
        salt: "salt".into(),
        nonce: "nonce".into(),
        ciphertext: "cipher".into(),
        associated_data: "host:server1".into(),
        kdf_iterations: 1000,
    };

    let local_record = SyncRecord::new(
        "server1",
        SyncEntityType::Host,
        2,
        "device-local",
        100,
        dummy_env.clone(),
    );

    let remote_record = SyncRecord::new(
        "server1",
        SyncEntityType::Host,
        2,
        "device-remote",
        105,
        dummy_env.clone(),
    );

    // Keep local: version should bump to 3, device remains local, timestamp updated
    let resolved_local = resolve_record_conflict(
        &local_record,
        &remote_record,
        ConflictResolutionChoice::KeepLocal,
        200,
    );
    assert_eq!(resolved_local.version, 3);
    assert_eq!(resolved_local.device_id, "device-local");
    assert_eq!(resolved_local.updated_at, 200);

    // Accept remote: version and device should match remote
    let resolved_remote = resolve_record_conflict(
        &local_record,
        &remote_record,
        ConflictResolutionChoice::AcceptRemote,
        200,
    );
    assert_eq!(resolved_remote.version, 2);
    assert_eq!(resolved_remote.device_id, "device-remote");
    assert_eq!(resolved_remote.updated_at, 105);
}

#[test]
fn verifies_format_record_summary_for_all_entity_types() {
    let passphrase = "test-secret-passphrase";

    // 1. Host
    #[derive(Serialize)]
    struct TestHost {
        name: String,
        hostname: String,
        username: String,
        port: u16,
    }
    let host_payload = TestHost {
        name: "Production Bastion".into(),
        hostname: "bastion.example.com".into(),
        username: "admin".into(),
        port: 2222,
    };
    let host_env =
        encrypt_entity_with_iterations(passphrase, "host", "host-1", &host_payload, 1000).unwrap();
    let host_rec = SyncRecord::new("host-1", SyncEntityType::Host, 1, "dev1", 100, host_env);
    let host_summary = format_record_summary(Some(passphrase), &host_rec);
    assert_eq!(
        host_summary,
        "Production Bastion (admin@bastion.example.com:2222)"
    );

    // 2. Group
    #[derive(Serialize)]
    struct TestGroup {
        name: String,
    }
    let group_payload = TestGroup {
        name: "Database Tier".into(),
    };
    let group_env =
        encrypt_entity_with_iterations(passphrase, "group", "group-1", &group_payload, 1000)
            .unwrap();
    let group_rec = SyncRecord::new("group-1", SyncEntityType::Group, 1, "dev1", 100, group_env);
    let group_summary = format_record_summary(Some(passphrase), &group_rec);
    assert_eq!(group_summary, "Database Tier (Group)");

    // 3. Tunnel
    #[derive(Serialize)]
    struct TestTunnel {
        name: String,
        forwarding_type: String,
        bind_port: u16,
    }
    let tunnel_payload = TestTunnel {
        name: "Postgres Forward".into(),
        forwarding_type: "local".into(),
        bind_port: 5432,
    };
    let tunnel_env =
        encrypt_entity_with_iterations(passphrase, "tunnel", "tunnel-1", &tunnel_payload, 1000)
            .unwrap();
    let tunnel_rec = SyncRecord::new(
        "tunnel-1",
        SyncEntityType::Tunnel,
        1,
        "dev1",
        100,
        tunnel_env,
    );
    let tunnel_summary = format_record_summary(Some(passphrase), &tunnel_rec);
    assert_eq!(tunnel_summary, "Postgres Forward (local:5432)");

    // 4. Snippet
    #[derive(Serialize)]
    struct TestSnippet {
        name: String,
        command: String,
    }
    let snippet_payload = TestSnippet {
        name: "Flush Redis".into(),
        command: "redis-cli flushall".into(),
    };
    let snippet_env =
        encrypt_entity_with_iterations(passphrase, "snippet", "snippet-1", &snippet_payload, 1000)
            .unwrap();
    let snippet_rec = SyncRecord::new(
        "snippet-1",
        SyncEntityType::Snippet,
        1,
        "dev1",
        100,
        snippet_env,
    );
    let snippet_summary = format_record_summary(Some(passphrase), &snippet_rec);
    assert_eq!(snippet_summary, "Flush Redis (`redis-cli flushall`)");

    // 5. CloudProvider
    #[derive(Serialize)]
    struct TestCloudProvider {
        name: String,
        kind: String,
    }
    let cloud_payload = TestCloudProvider {
        name: "AWS Production".into(),
        kind: "aws".into(),
    };
    let cloud_env = encrypt_entity_with_iterations(
        passphrase,
        "cloud_provider",
        "cloud-1",
        &cloud_payload,
        1000,
    )
    .unwrap();
    let cloud_rec = SyncRecord::new(
        "cloud-1",
        SyncEntityType::CloudProvider,
        1,
        "dev1",
        100,
        cloud_env,
    );
    let cloud_summary = format_record_summary(Some(passphrase), &cloud_rec);
    assert_eq!(cloud_summary, "AWS Production (Cloud Provider: aws)");

    // 6. Tombstone
    let mut tombstone = snippet_rec.clone();
    tombstone.deleted = true;
    let tombstone_summary = format_record_summary(Some(passphrase), &tombstone);
    assert!(tombstone_summary.contains("Deleted / Tombstone"));
}

#[test]
fn verifies_build_conflict_item_views() {
    let passphrase = "test-secret-passphrase";

    #[derive(Serialize)]
    struct HostPayload {
        name: String,
        hostname: String,
        username: String,
        port: u16,
    }

    let loc_payload = HostPayload {
        name: "Server A Local".into(),
        hostname: "srv-a.internal".into(),
        username: "root".into(),
        port: 22,
    };
    let rem_payload = HostPayload {
        name: "Server A Remote".into(),
        hostname: "srv-a.internal".into(),
        username: "ubuntu".into(),
        port: 22,
    };

    let loc_env =
        encrypt_entity_with_iterations(passphrase, "host", "srv-a", &loc_payload, 1000).unwrap();
    let rem_env =
        encrypt_entity_with_iterations(passphrase, "host", "srv-a", &rem_payload, 1000).unwrap();

    let local_record = SyncRecord::new("srv-a", SyncEntityType::Host, 1, "dev-local", 100, loc_env);
    let remote_record =
        SyncRecord::new("srv-a", SyncEntityType::Host, 1, "dev-remote", 105, rem_env);

    let views = build_conflict_item_views(&[local_record], &[remote_record], Some(passphrase));

    assert_eq!(views.len(), 1);
    let item = &views[0];
    assert_eq!(item.object_id, "srv-a");
    assert_eq!(item.entity_type, SyncEntityType::Host);
    assert_eq!(item.local_version, 1);
    assert_eq!(item.remote_version, 1);
    assert_eq!(
        item.local_summary,
        "Server A Local (root@srv-a.internal:22)"
    );
    assert_eq!(
        item.remote_summary,
        "Server A Remote (ubuntu@srv-a.internal:22)"
    );
}

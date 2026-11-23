use super::*;
#[path = "../../../tests/support/drive.rs"]
mod immutable_drive;

#[test]
fn verifies_google_drive_request_builders_and_parsers() {
    let token = "ya29.test_token";

    // List files
    let list_req = build_list_appdata_files_request(token);
    assert_eq!(list_req.method, "GET");
    assert!(list_req.url.contains("spaces=appDataFolder"));
    assert_eq!(
        list_req.headers,
        vec![("Authorization".into(), "Bearer ya29.test_token".into())]
    );

    let list_resp = HttpResponse::new(
        200,
        r#"{"files":[{"id":"file_123","name":"shellumi_vault.json","mimeType":"application/json","modifiedTime":"2026-11-04T12:00:00Z","size":"4096"}]}"#,
    );
    let files = parse_list_files_response(&list_resp).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, "file_123");
    assert_eq!(files[0].name, "shellumi_vault.json");
    assert_eq!(files[0].mime_type, "application/json");

    // Download file
    let dl_req = build_download_file_request(token, "file_123");
    assert_eq!(dl_req.method, "GET");
    assert_eq!(
        dl_req.url,
        "https://www.googleapis.com/drive/v3/files/file_123?alt=media"
    );
    let dl_resp = HttpResponse::new(200, b"raw vault content bytes".to_vec());
    let dl_bytes = parse_download_file_response(&dl_resp).unwrap();
    assert_eq!(dl_bytes, b"raw vault content bytes");

    // Multipart upload
    let upload_req = build_multipart_upload_request(
        token,
        "shellumi_vault.json",
        b"{\"records\":[]}",
        "BOUNDARY_XYZ",
    );
    assert_eq!(upload_req.method, "POST");
    assert_eq!(
        upload_req.url,
        "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart"
    );
    let up_body_str = std::str::from_utf8(&upload_req.body).unwrap();
    assert!(up_body_str.contains("--BOUNDARY_XYZ"));
    assert!(up_body_str.contains("appDataFolder"));
    assert!(up_body_str.contains("{\"records\":[]}"));

    let up_resp = HttpResponse::new(
        200,
        r#"{"id":"new_file_789","name":"shellumi_vault.json","mimeType":"application/json"}"#,
    );
    let uploaded = parse_upload_file_response(&up_resp).unwrap();
    assert_eq!(uploaded.id, "new_file_789");

    // Update file
    let update_req = build_update_file_request(token, "file_123", b"updated content");
    assert_eq!(update_req.method, "PATCH");
    assert_eq!(
        update_req.url,
        "https://www.googleapis.com/upload/drive/v3/files/file_123?uploadType=media"
    );

    // Delete file
    let del_req = build_delete_file_request(token, "file_123");
    assert_eq!(del_req.method, "DELETE");
    assert_eq!(
        del_req.url,
        "https://www.googleapis.com/drive/v3/files/file_123"
    );
}

#[test]
fn verifies_google_drive_sync_client_workflow_with_mock_transport() {
    use std::sync::Mutex;

    struct MockTransport {
        requests: Mutex<Vec<HttpRequest>>,
    }

    impl HttpTransport for MockTransport {
        fn send(&self, req: HttpRequest) -> Result<HttpResponse, SyncError> {
            let mut reqs = self.requests.lock().unwrap();
            reqs.push(req.clone());

            if req.url.contains("oauth2.googleapis.com/token") {
                Ok(HttpResponse::new(
                    200,
                    r#"{"access_token":"ya29.mock_token","refresh_token":"1//mock_refresh","expires_in":3600}"#,
                ))
            } else if req.method == "GET" && req.url.contains("spaces=appDataFolder") {
                Ok(HttpResponse::new(
                    200,
                    r#"{"files":[{"id":"mock_file_id","name":"shellumi_vault.json","mimeType":"application/json"}]}"#,
                ))
            } else if req.method == "GET" && req.url.contains("alt=media") {
                let vault = SyncVault::new("mock_snap", 100);
                Ok(HttpResponse::new(
                    200,
                    vault.to_json().unwrap().into_bytes(),
                ))
            } else if req.method == "POST" && req.url.contains("uploadType=multipart") {
                Ok(HttpResponse::new(
                    200,
                    r#"{"id":"new_mock_file_id","name":"shellumi_vault.json","mimeType":"application/json"}"#,
                ))
            } else if req.method == "PATCH" || req.method == "DELETE" {
                Ok(HttpResponse::new(200, b"".to_vec()))
            } else {
                Err(SyncError::Http(404, "not found".into()))
            }
        }
    }

    let transport = MockTransport {
        requests: Mutex::new(Vec::new()),
    };
    let client = GoogleDriveSyncClient::new(transport, "client_id_test");

    // 1. Exchange code
    let bundle = client
        .exchange_code(
            "code_123",
            "verifier_456",
            "http://127.0.0.1:0/callback",
            1000,
        )
        .unwrap();
    assert_eq!(bundle.access_token, "ya29.mock_token");

    // 2. List files
    let files = client.list_vault_files(&bundle.access_token).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, "mock_file_id");

    // 3. Download vault
    let downloaded_vault = client
        .download_vault(&bundle.access_token, &files[0].id)
        .unwrap();
    assert_eq!(downloaded_vault.vault_id, "mock_snap");

    // 4. Upload vault
    let vault_to_upload = SyncVault::new("snap_up", 200);
    let uploaded = client
        .upload_vault(&bundle.access_token, &vault_to_upload, "boundary_test")
        .unwrap();
    assert_eq!(uploaded.id, "new_mock_file_id");

    // 5. Update vault
    client
        .update_vault(&bundle.access_token, "mock_file_id", &vault_to_upload)
        .unwrap();

    // 6. Delete file
    client
        .delete_file(&bundle.access_token, "mock_file_id")
        .unwrap();

    assert_eq!(client.transport().requests.lock().unwrap().len(), 6);
}

#[test]
fn verifies_execute_cloud_vault_sync_orchestration() {
    let client = GoogleDriveSyncClient::new(immutable_drive::Drive::default(), "test_client_id");

    let mut local_vault = SyncVault::new("local_vault_1", 1000);
    let host = TestHost {
        name: "Cloud Bastion".into(),
        hostname: "bastion.cloud.org".into(),
        port: 22,
    };
    let passphrase = "my-secret-cloud-password";
    let env =
        encrypt_entity_with_iterations(passphrase, "host", "host_cloud_1", &host, 2_000).unwrap();
    local_vault.records.push(SyncRecord {
        entity_type: SyncEntityType::Host,
        object_id: "host_cloud_1".into(),
        version: 1,
        updated_at: 1000,
        device_id: "dev_local".into(),
        deleted: false,
        envelope: Some(env),
    });

    // 1. First sync -> UploadInitial
    let res1 =
        execute_cloud_vault_sync(&client, "fake_token", &local_vault, passphrase, None).unwrap();
    match res1 {
        SyncCycleResult::Success {
            report,
            merged_records,
        } => {
            assert_eq!(report.remote_updated, 1);
            assert_eq!(merged_records.len(), 1);
            local_vault = report.synchronized_vault(&local_vault, merged_records);
        }
        _ => panic!("expected success for initial upload"),
    }

    // 2. Second sync with identical records -> AlreadySynchronized
    let res2 =
        execute_cloud_vault_sync(&client, "fake_token", &local_vault, passphrase, None).unwrap();
    assert_eq!(res2, SyncCycleResult::AlreadySynchronized);

    // Invalid or foreign wrapped keys cannot be published.
    let before = client.transport.0.lock().unwrap().files.clone();
    local_vault.key_envelope =
        Some(encrypt_entity_with_iterations(passphrase, "key", "vault_key", &host, 2000).unwrap());
    assert!(
        execute_cloud_vault_sync(&client, "fake_token", &local_vault, passphrase, None).is_err()
    );
    assert_eq!(client.transport.0.lock().unwrap().files, before);
    local_vault.key_envelope = None;

    // 3. Local update -> upload updates
    let updated_host = TestHost {
        name: "Cloud Bastion Renamed".into(),
        hostname: "bastion.cloud.org".into(),
        port: 22,
    };
    let updated_env =
        encrypt_entity_with_iterations(passphrase, "host", "host_cloud_1", &updated_host, 2_000)
            .unwrap();
    local_vault.records[0].version = 2;
    local_vault.records[0].updated_at = 1200;
    local_vault.records[0].envelope = Some(updated_env);

    let res3 =
        execute_cloud_vault_sync(&client, "fake_token", &local_vault, passphrase, None).unwrap();
    match res3 {
        SyncCycleResult::Success {
            report,
            merged_records,
        } => {
            assert_eq!(report.remote_updated, 1);
            assert_eq!(merged_records[0].version, 2);
        }
        _ => panic!("expected success for local update"),
    }
}

#[test]
fn verifies_multi_vault_google_drive_separation_and_non_overlapping_files() {
    use crate::{VaultId, VaultKey};
    let client = GoogleDriveSyncClient::new(immutable_drive::Drive::default(), "test_client_id");

    // 1. Vault Alpha (default vault)
    let vid_alpha = "legacy-local";
    let key_alpha = VaultKey::generate().unwrap();
    let pwd_alpha = "AlphaPass#2026!";
    let env_alpha = key_alpha
        .wrap_with_password(&VaultId::new(vid_alpha).unwrap(), pwd_alpha)
        .unwrap();

    let mut vault_alpha = SyncVault::new(vid_alpha, 1000);
    vault_alpha.key_envelope = Some(env_alpha.clone());
    let host_a = TestHost {
        name: "Alpha-Server".into(),
        hostname: "alpha.local".into(),
        port: 22,
    };
    let env_ha =
        encrypt_entity_with_iterations(pwd_alpha, "host", "host_a", &host_a, 2000).unwrap();
    vault_alpha.records.push(SyncRecord {
        entity_type: SyncEntityType::Host,
        object_id: "host_a".into(),
        version: 1,
        updated_at: 1000,
        device_id: "dev_a".into(),
        deleted: false,
        envelope: Some(env_ha),
    });

    let res_a = execute_cloud_vault_sync(&client, "token", &vault_alpha, pwd_alpha, None).unwrap();
    assert!(matches!(res_a, SyncCycleResult::Success { .. }));

    // 2. Vault Beta (secondary local vault)
    let vid_beta = "vlt-finance-99";
    let key_beta = VaultKey::generate().unwrap();
    let pwd_beta = "BetaPass#2026!";
    let env_beta = key_beta
        .wrap_with_password(&VaultId::new(vid_beta).unwrap(), pwd_beta)
        .unwrap();

    let mut vault_beta = SyncVault::new(vid_beta, 2000);
    vault_beta.key_envelope = Some(env_beta.clone());
    let host_b = TestHost {
        name: "Beta-Database".into(),
        hostname: "beta.local".into(),
        port: 5432,
    };
    let env_hb = encrypt_entity_with_iterations(pwd_beta, "host", "host_b", &host_b, 2000).unwrap();
    vault_beta.records.push(SyncRecord {
        entity_type: SyncEntityType::Host,
        object_id: "host_b".into(),
        version: 1,
        updated_at: 2000,
        device_id: "dev_b".into(),
        deleted: false,
        envelope: Some(env_hb),
    });

    let res_b = execute_cloud_vault_sync(&client, "token", &vault_beta, pwd_beta, None).unwrap();
    assert!(matches!(res_b, SyncCycleResult::Success { .. }));

    // 3. Verify separation on Google Drive: two distinct files exist
    let files = client.list_vault_files("token").unwrap();
    assert_eq!(files.len(), 2);
    let alpha_file = files
        .iter()
        .find(|f| f.name == "shellumi_vault_legacy-local.json");
    let beta_file = files
        .iter()
        .find(|f| f.name == "shellumi_vault_vlt-finance-99.json");
    assert!(
        alpha_file.is_some(),
        "Default vault snapshot must exist independently"
    );
    assert!(
        beta_file.is_some(),
        "Secondary vault snapshot must exist independently"
    );

    // 4. Download and verify neither overwrote the other and wrapped DEKs are distinct
    let dl_alpha = client
        .download_vault("token", &alpha_file.unwrap().id)
        .unwrap();
    let dl_beta = client
        .download_vault("token", &beta_file.unwrap().id)
        .unwrap();

    assert_eq!(dl_alpha.vault_id, "legacy-local");
    assert_eq!(dl_alpha.records.len(), 1);
    assert_eq!(dl_alpha.records[0].object_id, "host_a");
    assert_eq!(
        dl_alpha.key_envelope.unwrap().ciphertext,
        env_alpha.ciphertext
    );

    assert_eq!(dl_beta.vault_id, "vlt-finance-99");
    assert_eq!(dl_beta.records.len(), 1);
    assert_eq!(dl_beta.records[0].object_id, "host_b");
    assert_eq!(
        dl_beta.key_envelope.unwrap().ciphertext,
        env_beta.ciphertext
    );

    // 5. Test list_drive_vault_ids
    let available_ids = list_drive_vault_ids(&files);
    assert!(available_ids.contains(&"legacy-local".to_string()));
    assert!(available_ids.contains(&"vlt-finance-99".to_string()));
}

#[test]
fn verifies_interrupted_drive_operations_do_not_corrupt_existing_snapshot() {
    let client = GoogleDriveSyncClient::new(immutable_drive::Drive::default(), "test_client");

    use crate::{VaultId, VaultKey};
    let vid = "vlt-safe";
    let pwd = "SafePassword#123";
    let key = VaultKey::generate().unwrap();
    let env = key
        .wrap_with_password(&VaultId::new(vid).unwrap(), pwd)
        .unwrap();

    let mut vault = SyncVault::new(vid, 1000);
    vault.key_envelope = Some(env.clone());

    // Initial upload
    let res1 = execute_cloud_vault_sync(&client, "token", &vault, pwd, None).unwrap();
    assert!(matches!(&res1, SyncCycleResult::Success { .. }));
    if let SyncCycleResult::Success {
        report,
        merged_records,
    } = res1
    {
        vault = report.synchronized_vault(&vault, merged_records);
    }

    // Prepare update
    let host = TestHost {
        name: "Safe Node".into(),
        hostname: "node.safe".into(),
        port: 22,
    };
    let henvelope = encrypt_entity_with_iterations(pwd, "host", "h1", &host, 2000).unwrap();
    vault.records.push(SyncRecord {
        entity_type: SyncEntityType::Host,
        object_id: "h1".into(),
        version: 1,
        updated_at: 1500,
        device_id: "dev".into(),
        deleted: false,
        envelope: Some(henvelope),
    });

    // Simulate network interruption on upload
    client.transport().0.lock().unwrap().fail_upload = true;
    let failed_res = execute_cloud_vault_sync(&client, "token", &vault, pwd, None);
    assert!(
        failed_res.is_err(),
        "Interrupted operation must report error"
    );

    // Existing snapshot on Drive remains intact (unmodified initial upload)
    let current_dl = client.download_vault("token", "file-1").unwrap();
    assert_eq!(current_dl.records.len(), 0);
    assert_eq!(current_dl.key_envelope.unwrap().ciphertext, env.ciphertext);

    // Retry succeeds cleanly
    client.transport().0.lock().unwrap().fail_upload = false;
    let retry_res = execute_cloud_vault_sync(&client, "token", &vault, pwd, None).unwrap();
    assert!(matches!(retry_res, SyncCycleResult::Success { .. }));

    let updated_dl = client.download_current_vault("token", vid, pwd).unwrap();
    assert_eq!(updated_dl.records.len(), 1);
    assert_eq!(updated_dl.records[0].object_id, "h1");
}

#[test]
fn verifies_password_change_updates_and_syncs_wrapped_dek_envelope() {
    use crate::{VaultId, VaultKey, VaultManifest};
    let vid = "vlt-rot";
    let vid_obj = VaultId::new(vid).unwrap();
    let dek = VaultKey::generate().unwrap();

    let old_pwd = "OldPassword#123";
    let new_pwd = "NewPassword#456";

    let mut manifest = VaultManifest::new(
        vid.to_string(),
        "Rotating Vault".to_string(),
        dek.wrap_with_password(&vid_obj, old_pwd).unwrap(),
        None,
        false,
    );
    assert_eq!(
        manifest.unlock(old_pwd).unwrap().expose_bytes(),
        dek.expose_bytes()
    );

    // Rotate password
    manifest.change_password(&dek, new_pwd).unwrap();
    assert!(
        manifest.unlock(old_pwd).is_err(),
        "Old password must be rejected"
    );
    assert_eq!(
        manifest.unlock(new_pwd).unwrap().expose_bytes(),
        dek.expose_bytes()
    );

    // Sync vault receives updated primary envelope
    let mut sync_vault = SyncVault::new(vid, 1000);
    sync_vault.key_envelope = Some(manifest.primary_envelope.clone());

    // Verify recovery on second device using new password and envelope only
    let recovered = VaultKey::unwrap_with_password(
        &vid_obj,
        new_pwd,
        sync_vault.key_envelope.as_ref().unwrap(),
    )
    .unwrap();
    assert_eq!(recovered.expose_bytes(), dek.expose_bytes());
}

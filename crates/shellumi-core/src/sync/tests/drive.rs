use super::*;

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
    use std::sync::Mutex;

    struct TestMockTransport {
        uploaded: Mutex<Option<SyncVault>>,
        updated: Mutex<Option<SyncVault>>,
    }

    impl HttpTransport for TestMockTransport {
        fn send(&self, req: HttpRequest) -> Result<HttpResponse, SyncError> {
            if req.method == "GET" && req.url.contains("spaces=appDataFolder") {
                let has_file = self.uploaded.lock().unwrap().is_some();
                let files_json = if has_file {
                    r#"{"files":[{"id":"drive_file_1","name":"shellumi_vault.json","mimeType":"application/json"}]}"#
                } else {
                    r#"{"files":[]}"#
                };
                Ok(HttpResponse::new(200, files_json))
            } else if req.method == "POST" && req.url.contains("uploadType=multipart") {
                let body_str = String::from_utf8_lossy(&req.body);
                let marker = "Content-Type: application/json\r\n\r\n";
                if let Some(idx) = body_str.find(marker) {
                    let json_part = &body_str[idx + marker.len()..];
                    if let Some(end) = json_part.find("\r\n--") {
                        let vault = SyncVault::from_json(&json_part[..end]).unwrap();
                        *self.uploaded.lock().unwrap() = Some(vault);
                    }
                }
                Ok(HttpResponse::new(
                    200,
                    r#"{"id":"drive_file_1","name":"shellumi_vault.json","mimeType":"application/json"}"#,
                ))
            } else if req.method == "GET" && req.url.contains("drive_file_1?alt=media") {
                let vault = self.uploaded.lock().unwrap().clone().unwrap();
                Ok(HttpResponse::new(
                    200,
                    vault.to_json().unwrap().into_bytes(),
                ))
            } else if req.method == "PATCH" && req.url.contains("drive_file_1") {
                let vault = SyncVault::from_json(req.body_str().unwrap()).unwrap();
                *self.uploaded.lock().unwrap() = Some(vault.clone());
                *self.updated.lock().unwrap() = Some(vault);
                Ok(HttpResponse::new(200, b"".to_vec()))
            } else {
                Err(SyncError::Http(404, "not found".into()))
            }
        }
    }

    let transport = TestMockTransport {
        uploaded: Mutex::new(None),
        updated: Mutex::new(None),
    };
    let client = GoogleDriveSyncClient::new(transport, "test_client_id");

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
        }
        _ => panic!("expected success for initial upload"),
    }

    // 2. Second sync with identical records -> AlreadySynchronized
    let res2 =
        execute_cloud_vault_sync(&client, "fake_token", &local_vault, passphrase, None).unwrap();
    assert_eq!(res2, SyncCycleResult::AlreadySynchronized);

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

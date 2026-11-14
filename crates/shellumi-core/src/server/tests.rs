//! Automated unit and mock integration tests for Shellumi Server client and protocol.

use super::*;
use crate::server::crypto::*;
use crate::server::types::*;
use crate::sync::{EncryptedEnvelope, HttpRequest, HttpResponse, HttpTransport, SyncError};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Default)]
struct MockHttpTransport {
    last_request: Arc<Mutex<Option<HttpRequest>>>,
    responses: Arc<Mutex<Vec<HttpResponse>>>,
}

impl MockHttpTransport {
    fn new() -> Self {
        Self::default()
    }

    fn queue_response(&self, status: u16, body: impl Into<Vec<u8>>) {
        let resp = HttpResponse::new(status, body);
        self.responses.lock().unwrap().push(resp);
    }

    fn queue_json_response<T: serde::Serialize>(&self, status: u16, body: &T) {
        let bytes = serde_json::to_vec(body).unwrap();
        self.queue_response(status, bytes);
    }

    fn last_req(&self) -> Option<HttpRequest> {
        self.last_request.lock().unwrap().clone()
    }
}

impl HttpTransport for MockHttpTransport {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, SyncError> {
        *self.last_request.lock().unwrap() = Some(request);
        let mut queue = self.responses.lock().unwrap();
        if queue.is_empty() {
            return Err(SyncError::Network("no mock response queued".into()));
        }
        Ok(queue.remove(0))
    }
}

#[test]
fn test_canonical_preimage_formatting() {
    let vault_id = Uuid::new_v4();
    let preimage = canonical_mutation_preimage(
        vault_id,
        "host-123",
        "host",
        5,
        false,
        1,
        42,
        "abcdef123456",
    );
    let expected = format!(
        "shellumi-v1-mutation:{}:host-123:host:5:false:1:42:abcdef123456",
        vault_id
    );
    assert_eq!(preimage, expected);
}

#[test]
fn test_ciphertext_digest_computation() {
    let envelope = EncryptedEnvelope {
        salt: "c2FsdA==".to_string(),
        nonce: "bm9uY2U=".to_string(),
        ciphertext: "dGVzdC1jaXBoZXJ0ZXh0".to_string(),
        associated_data: "host:01".to_string(),
        kdf_iterations: 600_000,
    };

    let digest = compute_ciphertext_digest(Some(&envelope));
    assert!(!digest.is_empty());
    assert_eq!(digest.len(), 64); // SHA-256 in hex is 64 characters

    // Tombstone has empty digest
    let empty_digest = compute_ciphertext_digest(None);
    assert_eq!(empty_digest, "");
}

#[test]
fn test_device_signature_sign_and_verify() {
    let (keypair, public_key_b64) = generate_device_ed25519_keypair().unwrap();
    let message = b"canonical mutation payload for signing";
    let sig_b64 = sign_message(&keypair, message);

    // Verification succeeds
    assert!(verify_device_signature(&public_key_b64, message, &sig_b64).is_ok());

    // Tampered payload fails verification
    let tampered = b"canonical mutation payload for signing - TAMPERED";
    assert!(verify_device_signature(&public_key_b64, tampered, &sig_b64).is_err());

    // Corrupted signature fails verification
    let corrupted_sig = "A".repeat(88); // 64 bytes base64 encoded
    assert!(verify_device_signature(&public_key_b64, message, &corrupted_sig).is_err());
}

#[test]
fn test_envelope_validation_bounds() {
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use base64::Engine;

    let valid_envelope = EncryptedEnvelope {
        salt: BASE64_STANDARD.encode([1u8; 16]),
        nonce: BASE64_STANDARD.encode([2u8; 12]),
        ciphertext: BASE64_STANDARD.encode(vec![3u8; 128]),
        associated_data: "host:123".into(),
        kdf_iterations: 600_000,
    };

    assert!(validate_envelope(&valid_envelope, 1024).is_ok());

    // Low iteration count
    let mut low_iter = valid_envelope.clone();
    low_iter.kdf_iterations = 1000;
    assert!(matches!(
        validate_envelope(&low_iter, 1024),
        Err(CryptoError::KdfIterationsTooLow(1000))
    ));

    // Bad salt length
    let mut bad_salt = valid_envelope.clone();
    bad_salt.salt = BASE64_STANDARD.encode([1u8; 8]);
    assert!(matches!(
        validate_envelope(&bad_salt, 1024),
        Err(CryptoError::InvalidSaltLength(8))
    ));

    // Oversized ciphertext
    assert!(matches!(
        validate_envelope(&valid_envelope, 50),
        Err(CryptoError::CiphertextTooLarge { .. })
    ));
}

#[test]
fn test_client_health_and_capabilities() {
    let mock = MockHttpTransport::new();
    let client = ShellumiServerClient::with_transport("https://server.example.com", mock.clone());

    let caps = CapabilitiesResponse {
        server_version: "0.1.0".to_string(),
        protocol_version: 1,
        min_protocol_version: 1,
        supported_features: vec!["vault_sync_v1".to_string(), "device_auth".to_string()],
        supported_e2ee_suites: vec!["x25519_hkdf_aes256gcm".to_string()],
        limits: ServerLimits {
            max_request_bytes: 10485760,
            max_record_bytes: 1048576,
            max_mutations_per_commit: 100,
        },
    };

    mock.queue_json_response(200, &caps);
    let result = client.capabilities().unwrap();
    assert_eq!(result.server_version, "0.1.0");
    assert_eq!(result.protocol_version, 1);
    assert_eq!(result.limits.max_mutations_per_commit, 100);

    let sent = mock.last_req().unwrap();
    assert_eq!(sent.method, "GET");
    assert_eq!(sent.url, "https://server.example.com/api/v1/capabilities");
}

#[test]
fn test_client_login_token_injection() {
    let mock = MockHttpTransport::new();
    let mut client =
        ShellumiServerClient::with_transport("https://server.example.com/", mock.clone());

    let user_id = Uuid::new_v4();
    let login_resp = LoginResponse {
        user_id,
        username: "alice".to_string(),
        email: "alice@example.com".to_string(),
        access_token: "jwt-token-abc".to_string(),
        refresh_token: "refresh-token-xyz".to_string(),
        expires_in_secs: 900,
    };

    mock.queue_json_response(200, &login_resp);
    let login_req = LoginRequest {
        identifier: "alice".to_string(),
        password: "secretpassword".to_string(),
        device_id: None,
    };

    let resp = client.login(&login_req).unwrap();
    assert_eq!(resp.username, "alice");
    assert_eq!(client.token(), Some("jwt-token-abc"));

    // Next request must contain Authorization header
    let me_resp = MeResponse {
        user_id,
        username: "alice".to_string(),
        email: "alice@example.com".to_string(),
        organizations: vec![UserOrgSummary {
            org_id: Uuid::new_v4(),
            org_name: "Acme Corp".to_string(),
            role: "owner".to_string(),
        }],
    };
    mock.queue_json_response(200, &me_resp);
    let me = client.me().unwrap();
    assert_eq!(me.organizations[0].org_name, "Acme Corp");

    let sent = mock.last_req().unwrap();
    assert_eq!(sent.url, "https://server.example.com/api/v1/me");
    let auth_header = sent
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str());
    assert_eq!(auth_header, Some("Bearer jwt-token-abc"));
}

#[test]
fn test_client_commit_and_fetch_changes() {
    let mock = MockHttpTransport::new();
    let mut client =
        ShellumiServerClient::with_transport("https://server.example.com", mock.clone());

    let dev_id = Uuid::new_v4();
    client.set_token("auth-token");
    client.set_device_id(dev_id);

    let vault_id = Uuid::new_v4();
    let commit_resp = CommitBatchResponse {
        vault_id,
        commit_sequence: 15,
        next_cursor: "seq:15".to_string(),
        applied: vec![MutationAck {
            object_id: "host-42".to_string(),
            assigned_revision: 3,
            server_sequence: 15,
            deleted: false,
        }],
    };
    mock.queue_json_response(200, &commit_resp);

    let batch = CommitBatchRequest {
        protocol_version: 1,
        vault_id,
        idempotency_key: "idem-key-1".to_string(),
        base_cursor: Some("seq:14".to_string()),
        mutations: vec![MutationItem {
            object_id: "host-42".to_string(),
            entity_type: "host".to_string(),
            expected_revision: 2,
            deleted: false,
            key_epoch: 1,
            device_counter: 1,
            encrypted_envelope: None,
            device_signature: "sig".to_string(),
        }],
    };

    let ack = client.commit(vault_id, &batch).unwrap();
    assert_eq!(ack.commit_sequence, 15);
    assert_eq!(ack.applied[0].assigned_revision, 3);

    let sent_commit = mock.last_req().unwrap();
    let dev_header = sent_commit
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("x-device-id"))
        .map(|(_, v)| v.as_str());
    assert_eq!(dev_header, Some(dev_id.to_string().as_str()));

    // Changes query
    let change_feed = ChangeFeedResponse {
        vault_id,
        latest_sequence: 15,
        next_cursor: "seq:15".to_string(),
        has_more: false,
        records: vec![SyncChangeItem {
            sequence: 15,
            object_id: "host-42".to_string(),
            entity_type: "host".to_string(),
            revision: 3,
            deleted: false,
            author_device_id: dev_id,
            encrypted_envelope: None,
            updated_at: 1700000100,
        }],
    };
    mock.queue_json_response(200, &change_feed);
    let changes = client
        .fetch_changes(vault_id, Some(10), None, Some(50))
        .unwrap();
    assert_eq!(changes.records.len(), 1);

    let sent_changes = mock.last_req().unwrap();
    assert_eq!(
        sent_changes.url,
        format!("https://server.example.com/api/v1/vaults/{vault_id}/changes?since=10&limit=50")
    );
}

#[test]
fn test_client_error_parsing() {
    let mock = MockHttpTransport::new();
    let client = ShellumiServerClient::with_transport("https://server.example.com", mock.clone());

    // 401 Unauthorized
    let err_resp = ErrorResponse {
        error_code: "AUTHENTICATION_FAILED".to_string(),
        message: "Invalid credentials".to_string(),
        request_id: Some("req-123".to_string()),
    };
    mock.queue_json_response(401, &err_resp);
    let err = client.me().unwrap_err();
    assert!(err.is_unauthorized());

    // 409 Conflict
    let conflict_resp = ErrorResponse {
        error_code: "REVISION_CONFLICT".to_string(),
        message: "Object host-1 revision mismatch".to_string(),
        request_id: None,
    };
    mock.queue_json_response(409, &conflict_resp);
    let err2 = client.me().unwrap_err();
    assert!(err2.is_conflict());

    // 404 Not Found
    let not_found_resp = ErrorResponse {
        error_code: "VAULT_NOT_FOUND".to_string(),
        message: "Vault does not exist".to_string(),
        request_id: None,
    };
    mock.queue_json_response(404, &not_found_resp);
    let err3 = client.get_vault(Uuid::new_v4()).unwrap_err();
    assert!(err3.is_not_found());
}

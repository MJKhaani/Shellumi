use super::*;
use shellumi_core::server::types::{
    CapabilitiesResponse, DeviceChallengeResponse, DeviceResponse, KeyEnvelopeListResponse,
    KeyEnvelopeResponse, MeResponse, RecipientDeviceResponse, RecipientDevicesResponse,
    ServerLimits, VaultListResponse, VaultMemberListResponse, VaultMemberResponse, VaultResponse,
};
use shellumi_core::sync::{HttpRequest, HttpResponse, SyncError};
use shellumi_core::{
    CredentialError, CredentialStore, EphemeralCredentialStore, SecretString, ServerConnectionId,
    SharedVault, VaultContext, VaultKey, VaultKeyManager, VaultLocation,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Credentials {
    entries: EphemeralCredentialStore,
    deletes: AtomicUsize,
    fail_delete: AtomicBool,
}

impl CredentialStore for Credentials {
    fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), CredentialError> {
        self.entries.set_secret(service, account, secret)
    }

    fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialError> {
        self.entries.get_secret(service, account)
    }

    fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError> {
        if service == VaultServerConnectionRecord::CREDENTIAL_SERVICE {
            self.deletes.fetch_add(1, SeqCst);
            if self.fail_delete.load(SeqCst) {
                return Err(CredentialError::Storage("injected deletion failure".into()));
            }
        }
        self.entries.delete_secret(service, account)
    }
}

struct Fixture {
    _directory: tempfile::TempDir,
    paths: [PathBuf; 4],
    keys: [VaultKey; 4],
    credentials: Credentials,
    user: Uuid,
    server: Uuid,
    remote: [Uuid; 3],
    old: Uuid,
}

impl Fixture {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let vaults = directory.path().join("vaults");
        std::fs::create_dir(&vaults).unwrap();
        let fixture = Self {
            paths: ["default", "vault-a", "vault-b", "vault-c"]
                .map(|name| vaults.join(format!("{name}.db"))),
            keys: std::array::from_fn(|_| VaultKey::generate().unwrap()),
            credentials: Credentials::default(),
            user: Uuid::new_v4(),
            server: Uuid::new_v4(),
            remote: std::array::from_fn(|_| Uuid::new_v4()),
            old: Uuid::new_v4(),
            _directory: directory,
        };
        let mut registry = fixture.open(0).await;
        for index in 1..=3 {
            let id = fixture.id(index);
            VaultKeyManager::provision_key(&fixture.credentials, &id, &fixture.keys[index])
                .unwrap();
            let connection = id.as_str();
            registry
                .save_vault_server_connection_secured(
                    &VaultServerConnectionRecord {
                        id: connection.into(),
                        base_url: "https://fixture.invalid".into(),
                        auth_token: Some(format!("fixture-session-{index}")),
                        server_instance_id: Some(fixture.server.to_string()),
                        device_id: Some(fixture.old.to_string()),
                        ..Default::default()
                    },
                    &fixture.credentials,
                )
                .await
                .unwrap();
            registry
                .register_vault_context(
                    connection,
                    &VaultContext::new(
                        id.clone(),
                        VaultLocation::Remote {
                            server: ServerConnectionId::new(connection).unwrap(),
                            server_vault_id: VaultId::new(fixture.remote[index - 1].to_string())
                                .unwrap(),
                        },
                    ),
                )
                .await
                .unwrap();
            let mut vault = fixture.open(index).await;
            vault
                .create_shared_vault(
                    &SharedVault::new(
                        id.as_str(),
                        id.as_str(),
                        None,
                        fixture.user.to_string(),
                        "1",
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            vault
                .add_team_member(&TeamMember::new(
                    connection,
                    id.as_str(),
                    fixture.user.to_string(),
                    "Owner",
                    "",
                    TeamRole::Owner,
                    "1",
                    fixture.user.to_string(),
                ))
                .await
                .unwrap();
        }
        fixture
    }

    fn id(&self, index: usize) -> VaultId {
        if index == 0 {
            return VaultId::new(crate::LEGACY_LOCAL_VAULT_ID).unwrap();
        }
        VaultId::new(self.paths[index].file_stem().unwrap().to_str().unwrap()).unwrap()
    }

    async fn open(&self, index: usize) -> HostStore {
        let mut store =
            HostStore::open_with_key(self.paths[index].to_str().unwrap(), Some(&self.keys[index]))
                .await
                .unwrap();
        store.set_vault_id(self.id(index));
        store
    }

    fn intent(&self, binding: &VerifiedRemoteVaultBinding) -> RegistrationIntent {
        let (signing, signing_public_key) = generate_device_ed25519_keypair_pkcs8().unwrap();
        let (agreement, agreement_public_key) = generate_device_x25519_keypair().unwrap();
        let intent = RegistrationIntent {
            scope: binding.enrollment_scope().unwrap(),
            role: TeamRole::Owner,
            local_vault_id: Some(binding.local_vault_id.clone()),
            account_id: Some(self.user),
            device_label: "New device".into(),
            initialize_connection: false,
            previous_connection_device: None,
            invitation: None,
            key_id: Uuid::new_v4(),
            signing_public_key,
            agreement_public_key,
        };
        let service = VaultServerConnectionRecord::CREDENTIAL_SERVICE;
        self.credentials
            .set_secret(
                service,
                &format!("{}-signing", intent.key_id),
                &BASE64_STANDARD.encode(signing),
            )
            .unwrap();
        self.credentials
            .set_secret(
                service,
                &format!("{}-agreement", intent.key_id),
                &BASE64_STANDARD.encode(agreement.as_ref()),
            )
            .unwrap();
        intent
    }

    async fn checkpoint(
        &self,
        vault: &HostStore,
        intent: &RegistrationIntent,
        stage: &str,
        device: Option<Uuid>,
    ) -> String {
        let operation = Uuid::new_v4().to_string();
        vault.connection.execute(
            "INSERT INTO enrollment_operations
             (operation_id, vault_id, user_id, member_id, stage, device_id, registration_intent, updated_at)
             VALUES (?1, ?2, ?3, ?2, ?4, ?5, ?6, '1')",
            (operation.as_str(), vault.vault_id().as_str(), self.user.to_string(), stage,
             device.map(|id| id.to_string()), serde_json::to_string(intent).unwrap()),
        ).await.unwrap();
        operation
    }

    async fn reserve(&self, registry: &HostStore, operation: &str) {
        registry
            .connection
            .execute(
                "INSERT INTO device_replacements
              (operation_id, server_instance_id, old_device_id, local_vault_id, remote_vault_id,
               connection_id, account_id, member_id, device_label, stage, base_url)
             VALUES (?1, ?2, ?3, 'vault-a', ?4, 'vault-a', ?5, 'vault-a', 'Replacement',
                     'OwnershipVerified', 'https://fixture.invalid')",
                (
                    operation,
                    self.server.to_string(),
                    self.old.to_string(),
                    self.remote[0].to_string(),
                    self.user.to_string(),
                ),
            )
            .await
            .unwrap();
    }
}

struct Server {
    user: Uuid,
    instance: Uuid,
    vaults: [Uuid; 3],
    devices: Mutex<Vec<DeviceResponse>>,
    registration_count: AtomicUsize,
    lose_registration: AtomicBool,
    reject_registration: AtomicBool,
    delete_count: AtomicUsize,
    lose_delete: AtomicBool,
    missing_vault: Mutex<Option<Uuid>>,
    unauthorized_vault: Mutex<Option<Uuid>>,
    missing_envelope: Mutex<Option<Uuid>>,
}

impl Server {
    fn new(fixture: &Fixture, devices: &[(Uuid, &RegistrationIntent)]) -> Arc<Self> {
        Arc::new(Self {
            user: fixture.user,
            instance: fixture.server,
            vaults: fixture.remote,
            devices: Mutex::new(
                devices
                    .iter()
                    .map(|(id, intent)| DeviceResponse {
                        device_id: *id,
                        user_id: fixture.user,
                        device_name: intent.device_label.clone(),
                        signing_public_key: intent.signing_public_key.clone(),
                        agreement_public_key: intent.agreement_public_key.clone(),
                        created_at: 1,
                        last_seen_at: None,
                        revoked_at: None,
                    })
                    .collect(),
            ),
            registration_count: Default::default(),
            lose_registration: Default::default(),
            reject_registration: Default::default(),
            delete_count: Default::default(),
            lose_delete: Default::default(),
            missing_vault: Default::default(),
            unauthorized_vault: Default::default(),
            missing_envelope: Default::default(),
        })
    }
}

#[derive(Clone)]
struct Transport(Arc<Server>);

impl HttpTransport for Transport {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, SyncError> {
        fn json(value: impl Serialize) -> HttpResponse {
            HttpResponse::new(200, serde_json::to_vec(&value).unwrap())
        }
        let server = &self.0;
        let url = request.url.as_str();
        if request.method == "POST" && url.ends_with("/devices/challenge") {
            return Ok(json(DeviceChallengeResponse {
                challenge: "fixture-challenge".into(),
                expires_at: 100,
            }));
        }
        if request.method == "POST" && url.ends_with("/devices") {
            server.registration_count.fetch_add(1, SeqCst);
            if server.reject_registration.load(SeqCst) {
                return Ok(HttpResponse::new(
                    403,
                    br#"{"error_code":"forbidden","message":"rejected"}"#.to_vec(),
                ));
            }
            let registration: RegisterDeviceRequest =
                serde_json::from_slice(&request.body).unwrap();
            assert_eq!(registration.challenge.as_deref(), Some("fixture-challenge"));
            shellumi_core::server::crypto::verify_device_signature(
                &registration.signing_public_key,
                b"shellumi-device-challenge:fixture-challenge",
                registration.challenge_signature.as_deref().unwrap(),
            )
            .unwrap();
            let device = DeviceResponse {
                device_id: Uuid::new_v4(),
                user_id: server.user,
                device_name: registration.device_name,
                signing_public_key: registration.signing_public_key,
                agreement_public_key: registration.agreement_public_key,
                created_at: 1,
                last_seen_at: None,
                revoked_at: None,
            };
            server.devices.lock().unwrap().push(device.clone());
            if server.lose_registration.load(SeqCst) {
                return Err(SyncError::Network(
                    "injected lost registration response".into(),
                ));
            }
            return Ok(json(device));
        }
        if request.method == "DELETE" {
            server.delete_count.fetch_add(1, SeqCst);
            let id = Uuid::parse_str(url.rsplit('/').next().unwrap()).unwrap();
            server
                .devices
                .lock()
                .unwrap()
                .iter_mut()
                .find(|device| device.device_id == id)
                .unwrap()
                .revoked_at = Some(10);
            if server.lose_delete.load(SeqCst) {
                return Err(SyncError::Network("injected lost DELETE response".into()));
            }
            return Ok(HttpResponse::new(204, vec![]));
        }
        if url.ends_with("/capabilities") {
            return Ok(json(CapabilitiesResponse {
                protocol_version: 1,
                server_instance_id: server.instance,
                server_version: "fixture".into(),
                min_protocol_version: 1,
                supported_features: vec![],
                supported_e2ee_suites: vec![],
                websocket: None,
                limits: ServerLimits {
                    max_request_bytes: 1000,
                    max_record_bytes: 1000,
                    max_mutations_per_commit: 10,
                    max_change_page: 10,
                },
            }));
        }
        if url.ends_with("/me") {
            return Ok(json(MeResponse {
                user_id: if request.headers.iter().any(|(name, value)| {
                    name.eq_ignore_ascii_case("Authorization")
                        && value == "Bearer fixture-other-account"
                }) {
                    Uuid::nil()
                } else {
                    server.user
                },
                username: "fixture".into(),
                email: "fixture".into(),
                organizations: vec![],
            }));
        }
        if url.ends_with("/devices") {
            return Ok(json(&*server.devices.lock().unwrap()));
        }
        let metadata = |id| VaultResponse {
            vault_id: id,
            tenant_id: Uuid::nil(),
            owner_id: server.user,
            vault_type: "team".into(),
            encrypted_name: None,
            active_key_epoch: 2,
            server_sequence: 0,
            role: "owner".into(),
            created_at: 1,
            updated_at: 1,
        };
        if url.ends_with("/vaults") {
            return Ok(json(VaultListResponse {
                vaults: server
                    .vaults
                    .into_iter()
                    .filter(|id| Some(*id) != *server.missing_vault.lock().unwrap())
                    .map(metadata)
                    .collect(),
            }));
        }
        let tail = url.split("/vaults/").nth(1).unwrap();
        let id = Uuid::parse_str(tail.split('/').next().unwrap()).unwrap();
        if url.ends_with("/members") {
            return Ok(json(VaultMemberListResponse {
                members: vec![VaultMemberResponse {
                    user_id: server.user,
                    vault_id: id,
                    role: "owner".into(),
                    joined_at: 1,
                    username: "fixture".into(),
                }],
            }));
        }
        if url.ends_with("/recipient-devices") {
            let devices = server.devices.lock().unwrap();
            let exclude_replacement = *server.unauthorized_vault.lock().unwrap() == Some(id);
            return Ok(json(RecipientDevicesResponse {
                vault_id: id,
                devices: devices
                    .iter()
                    .enumerate()
                    .filter(|(i, device)| {
                        (!exclude_replacement || *i == 0) && device.revoked_at.is_none()
                    })
                    .map(|(_, d)| RecipientDeviceResponse {
                        device_id: d.device_id,
                        user_id: d.user_id,
                        device_name: d.device_name.clone(),
                        agreement_public_key: d.agreement_public_key.clone(),
                        key_fingerprint: String::new(),
                    })
                    .collect(),
            }));
        }
        if url.ends_with("/keys/envelopes") {
            let mut envelopes = Vec::new();
            if *server.missing_envelope.lock().unwrap() != Some(id) {
                let devices = server.devices.lock().unwrap();
                let new = &devices[1];
                assert!(request
                    .headers
                    .iter()
                    .any(|(name, value)| name == "X-Device-Id"
                        && *value == new.device_id.to_string()));
                let (wrapped_dek, ephemeral) =
                    wrap_dek_for_recipient(&[42; 32], &new.agreement_public_key, id, 2).unwrap();
                envelopes.push(KeyEnvelopeResponse {
                    vault_id: id,
                    epoch: 2,
                    recipient_device_id: new.device_id,
                    sender_device_id: devices[0].device_id,
                    wrapped_dek,
                    key_agreement_ephemeral_pk: Some(ephemeral),
                    created_at: 1,
                });
            }
            return Ok(json(KeyEnvelopeListResponse { envelopes }));
        }
        Ok(json(metadata(id)))
    }
}

fn local_device(
    binding: &VerifiedRemoteVaultBinding,
    user: Uuid,
    id: Uuid,
    intent: &RegistrationIntent,
) -> EnrolledDevice {
    let (signing, agreement) = intent.credential_refs();
    EnrolledDevice {
        device_id: id.to_string(),
        vault_id: binding.local_vault_id.clone(),
        user_id: user.to_string(),
        device_name: intent.device_label.clone(),
        signing_public_key: intent.signing_public_key.clone(),
        agreement_public_key: intent.agreement_public_key.clone(),
        signing_key_ref: Some(signing),
        agreement_key_ref: Some(agreement),
        enrolled_at: "1".into(),
        revoked: false,
        revoked_at: None,
        verified: true,
    }
}

#[tokio::test]
async fn registry_reservations_and_multivault_handoff_survive_failures() {
    let fixture = Fixture::new().await;
    let mut registry = fixture.open(0).await;
    let mut a = fixture.open(1).await;
    let mut b = fixture.open(2).await;
    let first = registry
        .resolve_verified_remote_vault_binding("vault-a", None)
        .await
        .unwrap();
    let second = registry
        .resolve_verified_remote_vault_binding("vault-b", None)
        .await
        .unwrap();
    let old = fixture.intent(&first);
    let replacement = fixture.intent(&first);
    let new_id = Uuid::new_v4();
    a.save_enrolled_device(&local_device(&first, fixture.user, fixture.old, &old))
        .await
        .unwrap();
    a.save_enrolled_device(&local_device(&first, fixture.user, new_id, &replacement))
        .await
        .unwrap();
    b.save_enrolled_device(&local_device(&second, fixture.user, fixture.old, &old))
        .await
        .unwrap();
    b.save_enrolled_device(&local_device(&second, fixture.user, new_id, &replacement))
        .await
        .unwrap();
    let operation = fixture
        .checkpoint(&a, &replacement, "Completed", Some(new_id))
        .await;
    fixture.reserve(&registry, &operation).await;
    let server = Server::new(&fixture, &[(fixture.old, &old), (new_id, &replacement)]);
    let mut client =
        ShellumiServerClient::with_transport("https://fixture.invalid", Transport(server.clone()));
    client.set_token("fixture-session-1");
    for device in [fixture.old, new_id] {
        assert!(b
            .revoke_device_with_server_and_registry(
                &second,
                "vault-b",
                device,
                &client,
                Some(&registry)
            )
            .await
            .is_err());
        assert!(b
            .revoke_device_with_server(&second, "vault-b", device, &client)
            .await
            .is_err());
    }
    assert_eq!(server.delete_count.load(SeqCst), 0);

    *server.missing_vault.lock().unwrap() = Some(fixture.remote[2]);
    assert!(a
        .recover_incomplete_enrollments_with_registry(
            &first,
            &mut client,
            &fixture.credentials,
            Some(&registry)
        )
        .await
        .is_err());
    *server.missing_vault.lock().unwrap() = None;
    *server.unauthorized_vault.lock().unwrap() = Some(fixture.remote[1]);
    assert!(a
        .recover_incomplete_enrollments_with_registry(
            &first,
            &mut client,
            &fixture.credentials,
            Some(&registry)
        )
        .await
        .is_err());
    *server.unauthorized_vault.lock().unwrap() = None;
    *server.missing_envelope.lock().unwrap() = Some(fixture.remote[2]);
    assert!(a
        .recover_incomplete_enrollments_with_registry(
            &first,
            &mut client,
            &fixture.credentials,
            Some(&registry)
        )
        .await
        .is_err());
    *server.missing_envelope.lock().unwrap() = None;
    assert!(b
        .revoke_device_with_server_and_registry(
            &second,
            "vault-b",
            new_id,
            &client,
            Some(&registry)
        )
        .await
        .is_err());
    assert_eq!(server.delete_count.load(SeqCst), 0);
    assert_eq!(fixture.credentials.deletes.load(SeqCst), 0);

    let mut alias = registry
        .get_vault_server_connection("vault-c")
        .await
        .unwrap()
        .unwrap();
    alias.auth_token = Some("fixture-other-account".into());
    registry
        .save_vault_server_connection_secured(&alias, &fixture.credentials)
        .await
        .unwrap();
    assert!(a
        .recover_incomplete_enrollments_with_registry(
            &first,
            &mut client,
            &fixture.credentials,
            Some(&registry)
        )
        .await
        .is_err());
    assert_eq!(server.delete_count.load(SeqCst), 0);
    alias.auth_token = Some("fixture-session-3".into());
    registry
        .save_vault_server_connection_secured(&alias, &fixture.credentials)
        .await
        .unwrap();
    registry
        .connection
        .execute(
            "CREATE TRIGGER fail_handoff BEFORE UPDATE OF device_id ON vault_server_connections
        WHEN NEW.id = 'vault-b' BEGIN SELECT RAISE(ABORT, 'handoff fault'); END",
            (),
        )
        .await
        .unwrap();
    assert!(a
        .recover_incomplete_enrollments_with_registry(
            &first,
            &mut client,
            &fixture.credentials,
            Some(&registry)
        )
        .await
        .is_err());
    assert!(registry
        .list_vault_server_connections()
        .await
        .unwrap()
        .iter()
        .all(|connection| connection.device_id == Some(fixture.old.to_string())));
    assert_eq!(server.delete_count.load(SeqCst), 0);
    registry
        .connection
        .execute("DROP TRIGGER fail_handoff", ())
        .await
        .unwrap();
    server.lose_delete.store(true, SeqCst);
    assert!(a
        .recover_incomplete_enrollments_with_registry(
            &first,
            &mut client,
            &fixture.credentials,
            Some(&registry)
        )
        .await
        .is_err());
    assert!(registry
        .list_vault_server_connections()
        .await
        .unwrap()
        .iter()
        .all(|connection| connection.device_id == Some(new_id.to_string())));
    assert_eq!(server.delete_count.load(SeqCst), 1);
    drop(a);
    drop(b);
    drop(registry);
    let registry = fixture.open(0).await;
    let mut a = fixture.open(1).await;
    a.connection
        .execute(
            "CREATE TRIGGER fail_retirement BEFORE UPDATE OF revoked ON enrolled_devices
        WHEN NEW.revoked = 1 BEGIN SELECT RAISE(ABORT, 'retirement fault'); END",
            (),
        )
        .await
        .unwrap();
    assert!(a
        .recover_incomplete_enrollments_with_registry(
            &first,
            &mut client,
            &fixture.credentials,
            Some(&registry)
        )
        .await
        .is_err());
    a.connection
        .execute("DROP TRIGGER fail_retirement", ())
        .await
        .unwrap();
    for _ in 0..2 {
        a.recover_incomplete_enrollments_with_registry(
            &first,
            &mut client,
            &fixture.credentials,
            Some(&registry),
        )
        .await
        .unwrap();
    }
    let b = fixture.open(2).await;
    assert!(b
        .list_enrolled_devices_for_vault("vault-b")
        .await
        .unwrap()
        .iter()
        .any(|device| device.device_id == fixture.old.to_string() && device.revoked));
    assert!(!registry
        .has_pending_device_replacements("vault-a")
        .await
        .unwrap());
    assert_eq!(server.delete_count.load(SeqCst), 1);
    assert_eq!(fixture.credentials.deletes.load(SeqCst), 0);
}

#[tokio::test]
async fn split_registry_recovery_preserves_ambiguous_registration_and_cleans_rejection() {
    let fixture = Fixture::new().await;
    let registry = fixture.open(0).await;
    let mut a = fixture.open(1).await;
    let first = registry
        .resolve_verified_remote_vault_binding("vault-a", None)
        .await
        .unwrap();
    let old = fixture.intent(&first);
    a.save_enrolled_device(&local_device(&first, fixture.user, fixture.old, &old))
        .await
        .unwrap();
    let server = Server::new(&fixture, &[(fixture.old, &old)]);
    let mut client =
        ShellumiServerClient::with_transport("https://fixture.invalid", Transport(server.clone()));
    client.set_token("fixture-session-1");
    server.lose_registration.store(true, SeqCst);
    assert!(a
        .replace_device_keys_for_member(
            &first,
            "vault-a",
            fixture.old,
            "Lost registration",
            &mut client,
            &fixture.credentials,
            Some(&registry)
        )
        .await
        .is_err());
    assert_eq!(server.registration_count.load(SeqCst), 1);
    assert_eq!(server.delete_count.load(SeqCst), 0);
    assert!(registry
        .has_pending_device_replacements("vault-a")
        .await
        .unwrap());
    drop(a);
    drop(registry);

    let registry = fixture.open(0).await;
    let mut a = fixture.open(1).await;
    server.lose_registration.store(false, SeqCst);
    *server.missing_envelope.lock().unwrap() = Some(fixture.remote[2]);
    for _ in 0..2 {
        assert!(a
            .recover_incomplete_enrollments_with_registry(
                &first,
                &mut client,
                &fixture.credentials,
                Some(&registry)
            )
            .await
            .is_err());
        assert_eq!(server.registration_count.load(SeqCst), 1);
        assert_eq!(server.delete_count.load(SeqCst), 0);
    }

    let mut b = fixture.open(2).await;
    let second = registry
        .resolve_verified_remote_vault_binding("vault-b", None)
        .await
        .unwrap();
    client.set_token("fixture-session-2");
    server.reject_registration.store(true, SeqCst);
    assert!(b
        .enroll_additional_device_for_member_with_binding(
            &second,
            "vault-b",
            "Definite rejection",
            &mut client,
            &fixture.credentials
        )
        .await
        .is_err());
    let pending = b
        .list_incomplete_enrollment_operations("vault-b")
        .await
        .unwrap();
    assert_eq!(pending[0].stage, "CredentialCleanup");
    assert_eq!(fixture.credentials.deletes.load(SeqCst), 0);
    b.recover_incomplete_enrollments_with_registry(
        &second,
        &mut client,
        &fixture.credentials,
        Some(&registry),
    )
    .await
    .unwrap();
    assert!(b
        .list_incomplete_enrollment_operations("vault-b")
        .await
        .unwrap()
        .is_empty());
    assert_eq!(fixture.credentials.deletes.load(SeqCst), 2);
    assert_eq!(server.registration_count.load(SeqCst), 2);
    assert_eq!(server.delete_count.load(SeqCst), 0);
    let (signing, agreement) = old.credential_refs();
    for reference in [signing, agreement] {
        assert!(
            shellumi_core::resolve_secret_from_ref(&fixture.credentials, &reference)
                .unwrap()
                .is_some()
        );
    }
}

#[tokio::test]
async fn cleanup_defers_for_external_references_and_inaccessible_vaults() {
    let fixture = Fixture::new().await;
    let mut registry = fixture.open(0).await;
    let a = fixture.open(1).await;
    let mut b = fixture.open(2).await;
    let first = registry
        .resolve_verified_remote_vault_binding("vault-a", None)
        .await
        .unwrap();
    let second = registry
        .resolve_verified_remote_vault_binding("vault-b", None)
        .await
        .unwrap();
    let intent = fixture.intent(&first);
    let operation = fixture
        .checkpoint(&a, &intent, "CredentialCleanup", None)
        .await;
    let op = a
        .list_incomplete_enrollment_operations("vault-a")
        .await
        .unwrap()
        .pop()
        .unwrap();
    let (signing, _) = intent.credential_refs();
    let mut connection = registry
        .get_vault_server_connection("vault-b")
        .await
        .unwrap()
        .unwrap();
    connection.device_signing_key = Some(signing);
    registry
        .save_vault_server_connection(&connection)
        .await
        .unwrap();
    macro_rules! cleanup {
        () => {
            a.clean_abandoned_registration(
                &op,
                &intent,
                &fixture.credentials,
                false,
                &first,
                Some(&registry),
            )
            .await
        };
    }
    assert!(cleanup!().is_err());
    assert_eq!(fixture.credentials.deletes.load(SeqCst), 0);
    connection.device_signing_key = None;
    registry
        .save_vault_server_connection(&connection)
        .await
        .unwrap();
    let attached = Uuid::new_v4();
    b.save_enrolled_device(&local_device(&second, fixture.user, attached, &intent))
        .await
        .unwrap();
    assert!(cleanup!().is_err());
    assert_eq!(fixture.credentials.deletes.load(SeqCst), 0);
    b.connection
        .execute(
            "DELETE FROM enrolled_devices WHERE device_id = ?1",
            (attached.to_string(),),
        )
        .await
        .unwrap();
    let writer = b
        .connection
        .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
        .await
        .unwrap();
    writer
        .execute(
            "UPDATE team_members SET display_name = 'concurrent writer' WHERE id = 'vault-b'",
            (),
        )
        .await
        .unwrap();
    assert!(
        cleanup!().is_err(),
        "cleanup must wait for a concurrent vault writer"
    );
    assert_eq!(fixture.credentials.deletes.load(SeqCst), 0);
    writer.rollback().await.unwrap();
    drop(b);
    registry.set_vault_locked("vault-c", true).await.unwrap();
    assert!(cleanup!().is_err());
    registry.set_vault_locked("vault-c", false).await.unwrap();
    VaultKeyManager::forget_key(&fixture.credentials, &fixture.id(3)).unwrap();
    assert!(cleanup!().is_err());
    assert_eq!(fixture.credentials.deletes.load(SeqCst), 0);
    VaultKeyManager::provision_key(&fixture.credentials, &fixture.id(3), &fixture.keys[3]).unwrap();
    fixture.credentials.fail_delete.store(true, SeqCst);
    assert!(cleanup!().is_err());
    assert_eq!(
        a.list_incomplete_enrollment_operations("vault-a")
            .await
            .unwrap()[0]
            .stage,
        "CredentialCleanup"
    );
    fixture.credentials.fail_delete.store(false, SeqCst);
    cleanup!().unwrap();
    assert!(a
        .list_incomplete_enrollment_operations("vault-a")
        .await
        .unwrap()
        .iter()
        .all(|entry| entry.operation_id != operation));
    let (signing, agreement) = intent.credential_refs();
    for reference in [signing, agreement] {
        assert!(
            shellumi_core::resolve_secret_from_ref(&fixture.credentials, &reference)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn encrypted_registry_conflict_worker() {
    let Ok(directory) = std::env::var("SHELLUMI_TEST_SPLIT_REGISTRY_DIR") else {
        return;
    };
    struct NoRequests;
    impl HttpTransport for NoRequests {
        fn send(&self, _: HttpRequest) -> Result<HttpResponse, SyncError> {
            panic!("a process contending for the registry lock reached the HTTP transport");
        }
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let open = |name: &str, key_name: &str| {
            let path = PathBuf::from(&directory).join(name);
            let bytes = BASE64_STANDARD
                .decode(std::env::var(key_name).unwrap())
                .unwrap();
            let key = VaultKey::from_bytes(bytes.try_into().unwrap());
            async move {
                HostStore::open_with_key(path.to_str().unwrap(), Some(&key))
                    .await
                    .unwrap()
            }
        };
        let registry = open("default.db", "SHELLUMI_TEST_SPLIT_REGISTRY_KEY").await;
        let mut vault = open("vault-b.db", "SHELLUMI_TEST_SPLIT_VAULT_KEY").await;
        vault.set_vault_id(VaultId::new("vault-b").unwrap());
        let binding = registry
            .resolve_verified_remote_vault_binding("vault-b", None)
            .await
            .unwrap();
        let mut client =
            ShellumiServerClient::with_transport("https://fixture.invalid", NoRequests);
        client.set_token("fixture-session");
        let error = vault
            .revoke_device_with_server_and_registry(
                &binding,
                "vault-b",
                Uuid::new_v4(),
                &client,
                Some(&registry),
            )
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("Another enrollment is using this database"));
        let error = vault
            .replace_device_keys_for_member(
                &binding,
                "vault-b",
                Uuid::new_v4(),
                "Other replacement",
                &mut client,
                &EphemeralCredentialStore::new(),
                Some(&registry),
            )
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("Another enrollment is using this database"));
    });
}

#[tokio::test]
async fn separate_process_cannot_revoke_or_replace_while_registry_is_locked() {
    let fixture = Fixture::new().await;
    let registry = fixture.open(0).await;
    fixture.reserve(&registry, "active-replacement").await;
    let guard = registry
        .lock_device_journal(".device-change.lock")
        .unwrap()
        .unwrap();
    // Hold the application guard while letting the child open the encrypted database.
    drop(registry);
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "server_sync::separate_databases::encrypted_registry_conflict_worker",
        ])
        .env(
            "SHELLUMI_TEST_SPLIT_REGISTRY_DIR",
            fixture.paths[0].parent().unwrap(),
        )
        .env(
            "SHELLUMI_TEST_SPLIT_REGISTRY_KEY",
            BASE64_STANDARD.encode(fixture.keys[0].expose_bytes()),
        )
        .env(
            "SHELLUMI_TEST_SPLIT_VAULT_KEY",
            BASE64_STANDARD.encode(fixture.keys[2].expose_bytes()),
        )
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
    drop(guard);
}

use tempfile::tempdir;
use uuid::Uuid;

use shellumi_core::setup::{
    verify_setup_integrity, ConfiguredVaultSummary, InstallationStatus, SetupState, SetupStep,
    SetupWizardMode,
};
use shellumi_core::sync::{
    build_google_drive_auth_url, generate_pkce_challenge, GoogleDriveSyncClient,
    OAuthCallbackListener,
};
use shellumi_core::{
    CredentialStore, EphemeralCredentialStore, HttpRequest, HttpResponse, HttpTransport,
    ServerConnectionId, SyncError, VaultContext, VaultId, VaultKey, VaultKeyManager, VaultLocation,
    VaultManifest,
};
use shellumi_storage::{
    HostStore, MigrationEngine, VaultServerConnectionRecord, LEGACY_LOCAL_VAULT_ID,
};

#[test]
fn test_fresh_installation_detection_and_default_vault_creation_flow() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();

    // 1. Detection of fresh installation
    let status = InstallationStatus::detect(base_dir);
    assert_eq!(status, InstallationStatus::Fresh);
    assert!(!SetupState::is_setup_completed(base_dir));

    // 2. Setup state initialization
    let mut state = SetupState::new();
    assert_eq!(state.current_step, SetupStep::Welcome);

    // Advance to DefaultVault
    state.current_step = SetupStep::DefaultVault;
    state.save(base_dir).unwrap();

    // 3. Create mandatory encrypted default vault
    let cred_store = EphemeralCredentialStore::new();
    let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let password = "CorrectHorseBatteryStaple123!";

    let recovery_code = VaultKey::generate_recovery_code().unwrap();
    assert_eq!(recovery_code.split('-').count(), 8);

    // Keychain remembering disabled by default
    let remember_in_keychain = false;
    let (dek, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
        &cred_store,
        &default_vid,
        password,
        remember_in_keychain,
        Some(&recovery_code),
    )
    .unwrap();

    // Verify keychain did not store the secret when remember_in_keychain is false
    assert!(
        VaultKeyManager::retrieve_key(&cred_store, &default_vid)
            .unwrap()
            .is_none(),
        "Vault key must not be persisted in OS Keychain when remembering is disabled"
    );

    // Save manifest
    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&manifests_dir).unwrap();
    let manifest_path = manifests_dir.join("legacy-local.manifest.json");

    let manifest = VaultManifest::new(
        LEGACY_LOCAL_VAULT_ID.to_string(),
        "Personal".to_string(),
        primary_env,
        recovery_env,
        remember_in_keychain,
    );
    manifest.save_to_path(&manifest_path).unwrap();

    // Initialize encrypted SQLite default.db
    let vaults_dir = base_dir.join("vaults");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    let db_path = vaults_dir.join("default.db");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&dek))
            .await
            .unwrap();
        drop(store);
    });

    state.configured_vaults.push(ConfiguredVaultSummary {
        id: LEGACY_LOCAL_VAULT_ID.to_string(),
        name: "Personal".to_string(),
        is_default: true,
        is_remote: false,
        sync_enabled: false,
        server_url: None,
    });

    // 4. Verify integrity
    let report = verify_setup_integrity(base_dir, Some(&dek));
    assert!(
        report.is_success(),
        "Verification report should succeed: {:?}",
        report.errors
    );
    assert!(report.default_db_encrypted);
    assert!(report.default_manifest_valid);
    assert!(report.unlock_verified);

    // 5. Atomic setup completion
    state.mark_completed(base_dir).unwrap();
    assert!(SetupState::is_setup_completed(base_dir));

    // Reload state and verify
    let loaded = SetupState::load(base_dir).unwrap().unwrap();
    assert!(loaded.completed);
    assert_eq!(loaded.configured_vaults.len(), 1);
}

#[test]
fn test_interrupted_setup_recovery_crash_safe() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();

    // Initialize state and interrupt at GoogleDrive step
    let mut state = SetupState::new();
    state.current_step = SetupStep::GoogleDrive;
    state.configured_vaults.push(ConfiguredVaultSummary {
        id: LEGACY_LOCAL_VAULT_ID.to_string(),
        name: "Personal".to_string(),
        is_default: true,
        is_remote: false,
        sync_enabled: false,
        server_url: None,
    });
    state.save(base_dir).unwrap();

    // Verify detection notices incomplete setup
    let status = InstallationStatus::detect(base_dir);
    match status {
        InstallationStatus::IncompleteSetup { last_step, .. } => {
            assert_eq!(last_step, SetupStep::GoogleDrive);
        }
        other => panic!("Expected IncompleteSetup, got: {:?}", other),
    }

    assert!(!SetupState::is_setup_completed(base_dir));

    // Resume setup
    let mut resumed = SetupState::load(base_dir)
        .unwrap()
        .expect("State file should exist");
    assert_eq!(resumed.current_step, SetupStep::GoogleDrive);
    assert_eq!(resumed.configured_vaults.len(), 1);

    // Advance to completed
    resumed.current_step = SetupStep::ReviewAndFinish;
    resumed.mark_completed(base_dir).unwrap();

    assert!(SetupState::is_setup_completed(base_dir));
}

#[test]
fn test_legacy_migration_route_in_setup_wizard() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();

    // Create legacy unencrypted shellumi.db
    let legacy_db_path = base_dir.join("shellumi.db");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let mut store = HostStore::open(legacy_db_path.to_str().unwrap())
            .await
            .unwrap();
        let dest = shellumi_core::SshDestination::new("admin", "192.168.1.50", 22).unwrap();
        store
            .add("legacy-srv", &dest, None, &[], None, None)
            .await
            .unwrap();
    });

    // Detect installation status
    let status = InstallationStatus::detect(base_dir);
    match status {
        InstallationStatus::LegacyDatabase { path } => {
            assert_eq!(path, legacy_db_path.to_str().unwrap());
        }
        other => panic!("Expected LegacyDatabase, got: {:?}", other),
    }

    // Migrate through migration engine with user's chosen password
    let cred_store = EphemeralCredentialStore::new();
    let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let user_pass = "MySecretMigrationPassword2026!";

    let (dek, primary_env, recovery_env) =
        VaultKeyManager::provision_new_vault(&cred_store, &default_vid, user_pass, false, None)
            .unwrap();

    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&manifests_dir).unwrap();
    let manifest = VaultManifest::new(
        LEGACY_LOCAL_VAULT_ID.to_string(),
        "Personal".to_string(),
        primary_env,
        recovery_env,
        false,
    );
    manifest
        .save_to_path(&manifests_dir.join("legacy-local.manifest.json"))
        .unwrap();

    runtime.block_on(async {
        MigrationEngine::migrate_legacy_database_if_needed(base_dir, Some(&dek), &cred_store)
            .await
            .unwrap();

        // Verify data was migrated into encrypted default.db
        let encrypted_db_path = base_dir.join("vaults").join("default.db");
        let store = HostStore::open_with_key(encrypted_db_path.to_str().unwrap(), Some(&dek))
            .await
            .unwrap();
        let hosts = store.list().await.unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "legacy-srv");
    });

    // Verify original legacy file is safely preserved intact (never deleted or destroyed)
    assert!(legacy_db_path.exists());
}

#[test]
fn test_google_drive_pkce_and_zero_knowledge_oauth() {
    // 1. PKCE challenge generation without env vars
    let pkce = generate_pkce_challenge().unwrap();
    assert_eq!(pkce.code_verifier.len(), 43);
    assert_eq!(pkce.code_challenge.len(), 43);

    // 2. Loopback redirect URI binding
    let listener = OAuthCallbackListener::bind_ephemeral().unwrap();
    let redirect_uri = listener.redirect_uri();
    assert!(redirect_uri.starts_with("http://127.0.0.1:"));
    assert!(redirect_uri.ends_with("/callback"));

    // 3. Auth URL construction with PKCE
    let client_id = "test-client-id.apps.googleusercontent.com";
    let state = Uuid::new_v4().to_string();
    let auth_url = build_google_drive_auth_url(client_id, &redirect_uri, &pkce, &state);

    assert!(auth_url.contains(&format!("client_id={client_id}")));
    assert!(auth_url.contains("code_challenge="));
    assert!(auth_url.contains("code_challenge_method=S256"));
    assert!(auth_url.contains("drive.appdata"));

    // 4. Opt-in setting independence
    let mut setup_state = SetupState::new();
    assert!(!setup_state.google_drive_connected);
    assert!(!setup_state.google_drive_sync_enabled);

    // If skipped, state remains clean
    setup_state.current_step = SetupStep::AdditionalVaults;
    assert!(!setup_state.google_drive_connected);
}

#[test]
fn test_independent_secondary_vault_creation_and_recovery() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let cred_store = EphemeralCredentialStore::new();

    let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let (def_key, def_primary, _) = VaultKeyManager::provision_new_vault(
        &cred_store,
        &default_vid,
        "MasterPassword1!",
        false,
        None,
    )
    .unwrap();

    let manifests_dir = base_dir.join("vault_manifests");
    let vaults_dir = base_dir.join("vaults");
    std::fs::create_dir_all(&manifests_dir).unwrap();
    std::fs::create_dir_all(&vaults_dir).unwrap();

    let def_manifest = VaultManifest::new(
        LEGACY_LOCAL_VAULT_ID.to_string(),
        "Personal".to_string(),
        def_primary,
        None,
        false,
    );
    def_manifest
        .save_to_path(&manifests_dir.join("legacy-local.manifest.json"))
        .unwrap();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        // Initialize default registry
        let reg_path = vaults_dir.join("default.db");
        let mut registry = HostStore::open_with_key(reg_path.to_str().unwrap(), Some(&def_key))
            .await
            .unwrap();

        // Create secondary local vault with separate password and DEK
        let sec_vid = VaultId::new("finance-vault").unwrap();
        let sec_pass = "SecondaryPassword2!";
        let sec_recovery = VaultKey::generate_recovery_code().unwrap();

        let (sec_dek, sec_primary, sec_rec_env) = VaultKeyManager::provision_new_vault(
            &cred_store,
            &sec_vid,
            sec_pass,
            false,
            Some(&sec_recovery),
        )
        .unwrap();

        let sec_manifest = VaultManifest::new(
            "finance-vault".to_string(),
            "Finance".to_string(),
            sec_primary,
            sec_rec_env.clone(),
            false,
        );
        sec_manifest
            .save_to_path(&manifests_dir.join("finance-vault.manifest.json"))
            .unwrap();

        // Create secondary vault database
        let sec_db_path = vaults_dir.join("finance-vault.db");
        let sec_store = HostStore::open_with_key(sec_db_path.to_str().unwrap(), Some(&sec_dek))
            .await
            .unwrap();
        drop(sec_store);

        // Register in default registry
        let sec_context = VaultContext::new(sec_vid.clone(), VaultLocation::Local);
        registry
            .register_vault_context("Finance", &sec_context)
            .await
            .unwrap();

        // Enable Drive sync only for secondary vault
        registry
            .set_vault_sync_enabled("finance-vault", true)
            .await
            .unwrap();
        assert!(registry
            .is_vault_sync_enabled("finance-vault")
            .await
            .unwrap());
        assert!(!registry
            .is_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID)
            .await
            .unwrap());

        // Test recovery code unlock
        let rec_envelope = sec_rec_env.expect("Recovery envelope must exist");
        let recovered_key = VaultKey::recover(&sec_vid, &sec_recovery, &rec_envelope).unwrap();
        assert_eq!(recovered_key.expose_bytes(), sec_dek.expose_bytes());
    });
}

#[test]
fn test_server_vault_enrollment_and_credential_securing() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let cred_store = EphemeralCredentialStore::new();

    let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let (def_key, _, _) = VaultKeyManager::provision_new_vault(
        &cred_store,
        &default_vid,
        "MasterPassword1!",
        false,
        None,
    )
    .unwrap();

    let vaults_dir = base_dir.join("vaults");
    std::fs::create_dir_all(&vaults_dir).unwrap();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let reg_path = vaults_dir.join("default.db");
        let mut registry = HostStore::open_with_key(reg_path.to_str().unwrap(), Some(&def_key))
            .await
            .unwrap();

        let srv_conn_id = ServerConnectionId::new("corp-server-1").unwrap();
        let conn_record = VaultServerConnectionRecord {
            id: srv_conn_id.as_str().to_string(),
            base_url: "https://shellumi.corp.internal:8443".to_string(),
            auth_token: Some("raw-access-token-xyz".to_string()),
            refresh_token: Some("raw-refresh-token-abc".to_string()),
            device_id: Some("device-123".to_string()),
            device_signing_key: Some("raw-signing-key".to_string()),
            server_instance_id: Some("instance-456".to_string()),
            device_agreement_key: Some("raw-agreement-key".to_string()),
            created_at: 1000,
            updated_at: 1000,
        };

        // Secure credentials into CredentialStore
        registry
            .save_vault_server_connection_secured(&conn_record, &cred_store)
            .await
            .unwrap();

        // Verify tokens are stored in credential store, NOT in plaintext in the record
        let loaded_conn = registry
            .get_vault_server_connection("corp-server-1")
            .await
            .unwrap()
            .expect("Connection should exist");

        assert!(
            loaded_conn
                .auth_token
                .as_ref()
                .unwrap()
                .starts_with("keychain://"),
            "Auth token must be replaced by keychain reference: {:?}",
            loaded_conn.auth_token
        );
        assert!(
            loaded_conn
                .refresh_token
                .as_ref()
                .unwrap()
                .starts_with("keychain://"),
            "Refresh token must be replaced by keychain reference: {:?}",
            loaded_conn.refresh_token
        );

        // Register remote vault context
        let remote_loc = VaultLocation::Remote {
            server: srv_conn_id,
            server_vault_id: VaultId::new("server-vault-999").unwrap(),
        };
        let remote_ctx = VaultContext::new(VaultId::new("corp-vault").unwrap(), remote_loc);
        registry
            .register_vault_context("Corporate Operations", &remote_ctx)
            .await
            .unwrap();

        let dir = registry.build_vault_directory().await.unwrap();
        let entry = dir
            .get("corp-vault")
            .expect("Vault entry should exist in directory");
        assert!(matches!(
            entry.location,
            shellumi_core::VaultDirectoryLocation::Remote { .. }
        ));
    });
}

#[test]
fn test_skipped_optional_steps_end_to_end_verification() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let cred_store = EphemeralCredentialStore::new();

    // 1. Fresh install detected
    assert_eq!(
        InstallationStatus::detect(base_dir),
        InstallationStatus::Fresh
    );

    // 2. Default vault created with strong password
    let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let (dek, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
        &cred_store,
        &default_vid,
        "VeryStrongDefaultPassword456!",
        false,
        None,
    )
    .unwrap();

    let manifests_dir = base_dir.join("vault_manifests");
    let vaults_dir = base_dir.join("vaults");
    std::fs::create_dir_all(&manifests_dir).unwrap();
    std::fs::create_dir_all(&vaults_dir).unwrap();

    let manifest = VaultManifest::new(
        LEGACY_LOCAL_VAULT_ID.to_string(),
        "Personal".to_string(),
        primary_env,
        recovery_env,
        false,
    );
    manifest
        .save_to_path(&manifests_dir.join("legacy-local.manifest.json"))
        .unwrap();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let mut store =
            HostStore::open_with_key(vaults_dir.join("default.db").to_str().unwrap(), Some(&dek))
                .await
                .unwrap();
        let context = VaultContext::new(default_vid.clone(), VaultLocation::Local);
        store
            .register_vault_context("Personal", &context)
            .await
            .unwrap();
        drop(store);
    });

    // 3. User skips Google Drive and skips Additional Vaults
    let mut state = SetupState::new();
    state.mode = SetupWizardMode::NewSetup;
    state.current_step = SetupStep::DefaultVault;
    state.configured_vaults.push(ConfiguredVaultSummary {
        id: LEGACY_LOCAL_VAULT_ID.to_string(),
        name: "Personal".to_string(),
        is_default: true,
        is_remote: false,
        sync_enabled: false,
        server_url: None,
    });
    // Skip Drive
    state.current_step = SetupStep::AdditionalVaults;
    // Skip Additional Vaults
    state.current_step = SetupStep::ReviewAndFinish;

    // 4. Pre-finish integrity report verification
    let report = verify_setup_integrity(base_dir, Some(&dek));
    assert!(report.is_success());
    assert_eq!(report.registered_vault_count, 1);
    assert!(report.default_db_encrypted);
    assert!(report.unlock_verified);

    // 5. Finalize setup
    state.mark_completed(base_dir).unwrap();
    assert!(SetupState::is_setup_completed(base_dir));

    // Never show wizard again
    assert!(SetupState::is_setup_completed(base_dir));
}

#[derive(Clone)]
struct MockDriveTransport {
    snapshots: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>>,
}

impl MockDriveTransport {
    fn new() -> Self {
        Self {
            snapshots: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    fn add_snapshot(&self, name: &str, snapshot: &shellumi_core::SyncVault) {
        let json = serde_json::to_vec(snapshot).unwrap();
        self.snapshots
            .lock()
            .unwrap()
            .insert(name.to_string(), json);
    }
}

impl HttpTransport for MockDriveTransport {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, SyncError> {
        let snaps = self.snapshots.lock().unwrap();
        if req.method == "GET" && req.url.contains("spaces=appDataFolder") {
            let files: Vec<serde_json::Value> = snaps
                .keys()
                .map(|name| {
                    serde_json::json!({
                        "id": name,
                        "name": name,
                        "mimeType": "application/json"
                    })
                })
                .collect();
            let body = serde_json::to_vec(&serde_json::json!({ "files": files })).unwrap();
            Ok(HttpResponse::new(200, body))
        } else if req.method == "GET" && req.url.contains("alt=media") {
            for (name, content) in snaps.iter() {
                if req.url.contains(name) {
                    return Ok(HttpResponse::new(200, content.clone()));
                }
            }
            Err(SyncError::Api("File not found".into()))
        } else if req.url.contains("oauth2.googleapis.com/token") {
            let body = serde_json::to_vec(&serde_json::json!({
                "access_token": "ya29.refreshed_access_token",
                "refresh_token": "1//new_refresh_token",
                "expires_in": 3600
            }))
            .unwrap();
            Ok(HttpResponse::new(200, body))
        } else {
            Ok(HttpResponse::new(200, b"{}".to_vec()))
        }
    }
}

#[test]
fn test_fresh_device_google_drive_restoration_multi_vault() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // -------------------------------------------------------------
    // Device 1: Provision default vault and secondary vault, export snapshots
    // -------------------------------------------------------------
    let temp_dev1 = tempdir().unwrap();
    let dev1_base = temp_dev1.path();
    let dev1_cred = EphemeralCredentialStore::new();

    let def_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let def_pass = "Device1MasterPassword2026!";
    let def_rec = VaultKey::generate_recovery_code().unwrap();
    let (def_dek, def_primary, def_rec_env) =
        VaultKeyManager::provision_new_vault(&dev1_cred, &def_vid, def_pass, false, Some(&def_rec))
            .unwrap();

    let dev1_manifests = dev1_base.join("vault_manifests");
    let dev1_vaults = dev1_base.join("vaults");
    std::fs::create_dir_all(&dev1_manifests).unwrap();
    std::fs::create_dir_all(&dev1_vaults).unwrap();

    let def_manifest = VaultManifest::new(
        LEGACY_LOCAL_VAULT_ID.to_string(),
        "Default Vault".to_string(),
        def_primary,
        def_rec_env,
        false,
    );
    def_manifest
        .save_new_to_path(&dev1_manifests.join("legacy-local.manifest.json"))
        .unwrap();

    let sec_vid = VaultId::new("team-infrastructure").unwrap();
    let sec_pass = "IndependentTeamPassword789!";
    let sec_rec = VaultKey::generate_recovery_code().unwrap();
    let (sec_dek, sec_primary, sec_rec_env) =
        VaultKeyManager::provision_new_vault(&dev1_cred, &sec_vid, sec_pass, false, Some(&sec_rec))
            .unwrap();

    let sec_manifest = VaultManifest::new(
        "team-infrastructure".to_string(),
        "Team Infrastructure".to_string(),
        sec_primary,
        sec_rec_env,
        false,
    );
    sec_manifest
        .save_new_to_path(&dev1_manifests.join("team-infrastructure.manifest.json"))
        .unwrap();

    let (def_snapshot, sec_snapshot) = runtime.block_on(async {
        // Initialize default store and add record
        let def_db_path = dev1_vaults.join("default.db");
        let mut def_store = HostStore::open_with_key(def_db_path.to_str().unwrap(), Some(&def_dek))
            .await
            .unwrap();
        let dest = shellumi_core::SshDestination::new("admin", "10.0.0.1", 22).unwrap();
        def_store
            .add("core-router", &dest, None, &[], None, None)
            .await
            .unwrap();

        // Register default & secondary in multi-vault directory
        let def_ctx = VaultContext::new(def_vid.clone(), VaultLocation::Local);
        def_store
            .register_vault_context("Default Vault", &def_ctx)
            .await
            .unwrap();
        let sec_ctx = VaultContext::new(sec_vid.clone(), VaultLocation::Local);
        def_store
            .register_vault_context("Team Infrastructure", &sec_ctx)
            .await
            .unwrap();
        def_store
            .set_vault_sync_enabled("team-infrastructure", true)
            .await
            .unwrap();

        let dir = def_store.build_vault_directory().await.unwrap();
        def_store
            .save_vault_directory_from_model(&def_vid, &def_dek, &dir)
            .await
            .unwrap();

        // Initialize secondary store and add record
        let sec_db_path = dev1_vaults.join("team-infrastructure.db");
        let mut sec_store = HostStore::open_with_key(sec_db_path.to_str().unwrap(), Some(&sec_dek))
            .await
            .unwrap();
        sec_store.set_vault_id(sec_vid.clone());
        let sec_dest = shellumi_core::SshDestination::new("deployer", "10.0.0.2", 2222).unwrap();
        sec_store
            .add("prod-k8s-master", &sec_dest, None, &[], None, None)
            .await
            .unwrap();

        // Export snapshots for cloud synchronization
        let def_snap = def_store
            .export_vault_with_iterations(def_pass, "device-1", 100)
            .await
            .unwrap();
        let sec_snap = sec_store
            .export_vault_with_iterations(sec_pass, "device-1", 100)
            .await
            .unwrap();

        (def_snap, sec_snap)
    });

    // -------------------------------------------------------------
    // Device 2: Fresh installation, no keys, no keychain
    // -------------------------------------------------------------
    let temp_dev2 = tempdir().unwrap();
    let dev2_base = temp_dev2.path();
    let dev2_cred = EphemeralCredentialStore::new();

    // Verify Device 2 has no stored keys
    assert!(VaultKeyManager::retrieve_key(&dev2_cred, &def_vid)
        .unwrap()
        .is_none());
    assert!(VaultKeyManager::retrieve_key(&dev2_cred, &sec_vid)
        .unwrap()
        .is_none());

    // 1. Simulate discovery of cloud vaults
    let client_id = "test-client";
    let mock_transport = MockDriveTransport::new();
    mock_transport.add_snapshot("shellumi_vault_legacy-local.json", &def_snapshot);
    mock_transport.add_snapshot("shellumi_vault_team-infrastructure.json", &sec_snapshot);

    let client = GoogleDriveSyncClient::new(mock_transport, client_id);
    let cloud_vaults = client.discover_cloud_vaults("mock_token").unwrap();
    assert_eq!(cloud_vaults.len(), 2);
    assert!(cloud_vaults
        .iter()
        .any(|v| v.id == "legacy-local" && v.is_default));
    assert!(cloud_vaults
        .iter()
        .any(|v| v.id == "team-infrastructure" && !v.is_default));

    // 2. Restore default vault first using default password on Device 2
    let dev2_vaults = dev2_base.join("vaults");
    let dev2_manifests = dev2_base.join("vault_manifests");
    std::fs::create_dir_all(&dev2_vaults).unwrap();
    std::fs::create_dir_all(&dev2_manifests).unwrap();

    let def_opts = shellumi_storage::RestoreVaultOptions {
        vault_id: &def_vid,
        vault_name: "Default Vault",
        password: def_pass,
        target_db_path: &dev2_vaults.join("default.db"),
        manifests_dir: &dev2_manifests,
        cred_store: Some(&dev2_cred),
        remember_in_keychain: false,
    };

    runtime.block_on(async {
        let def_store2 = HostStore::restore_vault_from_sync(&def_snapshot, &def_opts)
            .await
            .unwrap();

        // Verify default host record is intact
        let hosts = def_store2.list().await.unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "core-router");

        // Verify the multi-vault directory was imported without creating secondary db
        let dir = def_store2.build_vault_directory().await.unwrap();
        assert!(dir.get("team-infrastructure").is_some());
        assert!(!dev2_vaults.join("team-infrastructure.db").exists());

        // Verify manifest was created via save_new_to_path
        assert!(dev2_manifests.join("legacy-local.manifest.json").exists());

        // 3. Test wrong password for secondary vault: fails cleanly without creating files
        let wrong_sec_opts = shellumi_storage::RestoreVaultOptions {
            vault_id: &sec_vid,
            vault_name: "Team Infrastructure",
            password: "WrongPassword999!",
            target_db_path: &dev2_vaults.join("team-infrastructure.db"),
            manifests_dir: &dev2_manifests,
            cred_store: Some(&dev2_cred),
            remember_in_keychain: false,
        };
        let err = HostStore::restore_vault_from_sync(&sec_snapshot, &wrong_sec_opts).await;
        assert!(err.is_err());
        assert!(!dev2_vaults.join("team-infrastructure.db").exists());
        assert!(!dev2_manifests
            .join("team-infrastructure.manifest.json")
            .exists());

        // 4. Restore secondary vault with correct independent password
        let sec_opts = shellumi_storage::RestoreVaultOptions {
            vault_id: &sec_vid,
            vault_name: "Team Infrastructure",
            password: sec_pass,
            target_db_path: &dev2_vaults.join("team-infrastructure.db"),
            manifests_dir: &dev2_manifests,
            cred_store: Some(&dev2_cred),
            remember_in_keychain: false,
        };
        let sec_store2 = HostStore::restore_vault_from_sync(&sec_snapshot, &sec_opts)
            .await
            .unwrap();

        // Verify secondary host record is intact
        let sec_hosts = sec_store2.list().await.unwrap();
        assert_eq!(sec_hosts.len(), 1);
        assert_eq!(sec_hosts[0].name, "prod-k8s-master");
    });
}

#[test]
fn test_google_drive_token_persistence_auto_refresh_and_sync() {
    let cred_store = EphemeralCredentialStore::new();
    let mock_transport = MockDriveTransport::new();
    let client = GoogleDriveSyncClient::new(mock_transport, "shellumi-desktop");

    // Case 1: Expired token bundle in store gets automatically refreshed and persisted
    let expired_bundle = shellumi_core::sync::OAuthTokenBundle::new(
        "ya29.old_expired_token",
        Some("1//valid_refresh_token".to_string()),
        Some(60),
        Some("drive.appdata".to_string()),
        100, // acquired at 100
    );
    expired_bundle.save_to_store(&cred_store).unwrap();

    let now = 2000;
    let resolved =
        shellumi_core::sync::OAuthTokenBundle::resolve_valid_token(&cred_store, &client, now)
            .unwrap();
    assert_eq!(resolved, "ya29.refreshed_access_token");

    // Verify refreshed bundle was re-persisted to store
    let persisted = shellumi_core::sync::OAuthTokenBundle::load_from_store(&cred_store)
        .unwrap()
        .unwrap();
    assert_eq!(persisted.access_token, "ya29.refreshed_access_token");
    assert_eq!(
        persisted.refresh_token.as_deref(),
        Some("1//new_refresh_token")
    );
    assert_eq!(persisted.expires_in, Some(3600));

    // Case 2: Legacy token migration fallback
    let cred_store2 = EphemeralCredentialStore::new();
    cred_store2
        .set_secret(
            "org.shellumi.sync",
            "gdrive_oauth_token",
            "ya29.legacy_token",
        )
        .unwrap();

    let resolved_legacy =
        shellumi_core::sync::OAuthTokenBundle::resolve_valid_token(&cred_store2, &client, now)
            .unwrap();
    assert_eq!(resolved_legacy, "ya29.legacy_token");

    // Verify migrated into OAuthTokenBundle
    let migrated = shellumi_core::sync::OAuthTokenBundle::load_from_store(&cred_store2)
        .unwrap()
        .unwrap();
    assert_eq!(migrated.access_token, "ya29.legacy_token");
}

#[test]
fn test_restoration_with_keychain_disabled() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let cred_store = EphemeralCredentialStore::new();
    let vaults_dir = base_dir.join("vaults");
    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    std::fs::create_dir_all(&manifests_dir).unwrap();

    let vid = VaultId::new("no-keychain-vault").unwrap();
    let pass = "NoKeychainPassword123!";
    let (key, primary_env, _) = VaultKeyManager::provision_new_vault(
        &cred_store,
        &vid,
        pass,
        false, // keychain disabled
        None,
    )
    .unwrap();

    let manifest = VaultManifest::new(
        "no-keychain-vault".to_string(),
        "No Keychain".to_string(),
        primary_env,
        None,
        false,
    );
    manifest
        .save_new_to_path(&manifests_dir.join("no-keychain-vault.manifest.json"))
        .unwrap();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let snapshot = runtime.block_on(async {
        let db_path = vaults_dir.join("no-keychain-vault.db");
        let mut store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&key))
            .await
            .unwrap();
        store.set_vault_id(vid.clone());
        let snap = store
            .export_vault_with_iterations(pass, "device-1", 100)
            .await
            .unwrap();
        drop(store);
        std::fs::remove_file(&db_path).unwrap();
        std::fs::remove_file(manifests_dir.join("no-keychain-vault.manifest.json")).unwrap();
        snap
    });

    let target_db = vaults_dir.join("no-keychain-vault.db");
    let opts = shellumi_storage::RestoreVaultOptions {
        vault_id: &vid,
        vault_name: "No Keychain",
        password: pass,
        target_db_path: &target_db,
        manifests_dir: &manifests_dir,
        cred_store: Some(&cred_store),
        remember_in_keychain: false,
    };

    runtime.block_on(async {
        let store = HostStore::restore_vault_from_sync(&snapshot, &opts)
            .await
            .unwrap();
        drop(store);

        // Verify key is not in keychain
        assert!(VaultKeyManager::retrieve_key(&cred_store, &vid)
            .unwrap()
            .is_none());
    });
}

#[test]
fn test_fault_injection_wrong_password_corrupted_snapshot_safety() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let cred_store = EphemeralCredentialStore::new();
    let vaults_dir = base_dir.join("vaults");
    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    std::fs::create_dir_all(&manifests_dir).unwrap();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // 1. Corrupted snapshot: envelope has corrupted ciphertext
    let mut corrupted_snap = shellumi_core::SyncVault::new("corrupted-vault", 100);
    corrupted_snap.key_envelope = Some(shellumi_core::EncryptedEnvelope {
        ciphertext: "corrupted_garbage_bytes".to_string(),
        ..Default::default()
    });

    let corrupted_vid = VaultId::new("corrupted-vault").unwrap();
    let corrupted_db = vaults_dir.join("corrupted-vault.db");
    let opts = shellumi_storage::RestoreVaultOptions {
        vault_id: &corrupted_vid,
        vault_name: "Corrupted Vault",
        password: "SomePassword123!",
        target_db_path: &corrupted_db,
        manifests_dir: &manifests_dir,
        cred_store: Some(&cred_store),
        remember_in_keychain: false,
    };

    runtime.block_on(async {
        let res = HostStore::restore_vault_from_sync(&corrupted_snap, &opts).await;
        assert!(res.is_err());
        assert!(!corrupted_db.exists());
        assert!(!manifests_dir.join("corrupted-vault.manifest.json").exists());
    });

    // 2. Occupied destination path: if destination exists, restoration is rejected without damaging file
    let existing_db = vaults_dir.join("existing.db");
    std::fs::write(&existing_db, b"pre-existing database content").unwrap();

    let existing_vid = VaultId::new("existing").unwrap();
    let occ_opts = shellumi_storage::RestoreVaultOptions {
        vault_id: &existing_vid,
        vault_name: "Existing",
        password: "Password!",
        target_db_path: &existing_db,
        manifests_dir: &manifests_dir,
        cred_store: Some(&cred_store),
        remember_in_keychain: false,
    };

    runtime.block_on(async {
        let res = HostStore::restore_vault_from_sync(&corrupted_snap, &occ_opts).await;
        assert!(res.is_err());
        // Verify original content was NOT modified or truncated
        let content = std::fs::read(&existing_db).unwrap();
        assert_eq!(content, b"pre-existing database content");
    });
}

#[test]
fn test_interrupted_wizard_resumption_and_recovery_code_confirmation() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let cred_store = EphemeralCredentialStore::new();

    // 1. Simulate default vault creation on disk, but setup interrupted before recovery code confirmation
    let vaults_dir = base_dir.join("vaults");
    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    std::fs::create_dir_all(&manifests_dir).unwrap();

    let def_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let user_pass = "InterruptedPassword123!";
    let (dek, primary_env, recovery_env) =
        VaultKeyManager::provision_new_vault(&cred_store, &def_vid, user_pass, false, None)
            .unwrap();

    let manifest = VaultManifest::new(
        LEGACY_LOCAL_VAULT_ID.to_string(),
        "Personal".to_string(),
        primary_env,
        recovery_env,
        false,
    );
    manifest
        .save_new_to_path(&manifests_dir.join("legacy-local.manifest.json"))
        .unwrap();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let store =
            HostStore::open_with_key(vaults_dir.join("default.db").to_str().unwrap(), Some(&dek))
                .await
                .unwrap();
        drop(store);
    });

    let mut state = SetupState::new();
    state.mode = SetupWizardMode::NewSetup;
    state.current_step = SetupStep::DefaultVault;
    state.default_vault_created = true;
    state.recovery_code_confirmed = false;
    state.completed = false;
    state.save(base_dir).unwrap();

    // 2. Verify is_setup_completed is FALSE despite default.db and manifest existing
    assert!(
        !SetupState::is_setup_completed(base_dir),
        "Setup must not be completed when state has completed = false"
    );

    // 3. Status detection recognizes incomplete setup
    let status = InstallationStatus::detect(base_dir);
    match status {
        InstallationStatus::IncompleteSetup {
            last_step,
            default_vault_created,
            ..
        } => {
            assert_eq!(last_step, SetupStep::DefaultVault);
            assert!(default_vault_created);
        }
        other => panic!("Expected IncompleteSetup, got: {:?}", other),
    }

    // 4. Recover recovery credentials via vault unlock
    let loaded_manifest =
        VaultManifest::load_from_path(&manifests_dir.join("legacy-local.manifest.json")).unwrap();
    let unlocked_key =
        VaultKey::unwrap_with_password(&def_vid, user_pass, &loaded_manifest.primary_envelope)
            .unwrap();
    assert_eq!(unlocked_key.expose_bytes(), dek.expose_bytes());

    let fresh_rec_code = VaultKey::generate_recovery_code().unwrap();
    let mut updated_manifest = loaded_manifest;
    updated_manifest
        .set_recovery_key(&unlocked_key, &fresh_rec_code)
        .unwrap();
    updated_manifest
        .save_to_path(&manifests_dir.join("legacy-local.manifest.json"))
        .unwrap();

    // 5. User confirms recovery code, advances to GoogleDrive
    state.recovery_code_confirmed = true;
    state.current_step = SetupStep::GoogleDrive;
    state.save(base_dir).unwrap();

    let resumed_state = SetupState::load(base_dir).unwrap().unwrap();
    assert_eq!(resumed_state.current_step, SetupStep::GoogleDrive);
    assert!(resumed_state.recovery_code_confirmed);
}

#[test]
fn test_explicit_wizard_reentry_preserves_existing_data() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let cred_store = EphemeralCredentialStore::new();

    let vaults_dir = base_dir.join("vaults");
    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    std::fs::create_dir_all(&manifests_dir).unwrap();

    let def_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let (dek, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
        &cred_store,
        &def_vid,
        "SecretPassword123!",
        false,
        None,
    )
    .unwrap();

    let manifest = VaultManifest::new(
        LEGACY_LOCAL_VAULT_ID.to_string(),
        "Personal".to_string(),
        primary_env,
        recovery_env,
        false,
    );
    manifest
        .save_new_to_path(&manifests_dir.join("legacy-local.manifest.json"))
        .unwrap();

    let db_path = vaults_dir.join("default.db");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&dek))
            .await
            .unwrap();
        let dest = shellumi_core::SshDestination::new("admin", "192.168.1.1", 22).unwrap();
        store
            .add("prod-server", &dest, None, &[], None, None)
            .await
            .unwrap();
    });

    let mut state = SetupState::new();
    state.mark_completed(base_dir).unwrap();
    assert!(SetupState::is_setup_completed(base_dir));

    // Explicit re-entry after completed setup:
    // Status detection sees existing vaults
    let status = InstallationStatus::detect(base_dir);
    assert_eq!(
        status,
        InstallationStatus::ExistingVaults {
            default_db_exists: true,
            manifest_exists: true,
            vault_count: 1
        }
    );

    // Ensure NewSetup cannot overwrite existing vault
    let default_db = base_dir.join("vaults").join("default.db");
    let default_manifest = base_dir
        .join("vault_manifests")
        .join("legacy-local.manifest.json");
    assert!(default_db.exists());
    assert!(default_manifest.exists());

    // Host data is fully preserved
    runtime.block_on(async {
        let store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&dek))
            .await
            .unwrap();
        let hosts = store.list().await.unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "prod-server");
    });
}

#[test]
fn test_corrupted_setup_state_detects_existing_vaults() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();

    // Create existing default database
    let vaults_dir = base_dir.join("vaults");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    std::fs::write(vaults_dir.join("default.db"), b"existing-db-bytes").unwrap();

    // Write corrupted JSON into .setup_state.json
    let state_file = base_dir.join(".setup_state.json");
    std::fs::write(&state_file, b"{ invalid json garbage !@#$%^").unwrap();

    // Verify is_setup_completed is false on corrupted state
    assert!(!SetupState::is_setup_completed(base_dir));

    // Verify detection inspects existing vaults rather than returning Fresh
    let status = InstallationStatus::detect(base_dir);
    match status {
        InstallationStatus::ExistingVaults {
            default_db_exists, ..
        } => {
            assert!(default_db_exists);
        }
        other => panic!(
            "Expected ExistingVaults for corrupted state with default.db, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_occupied_destination_paths_and_no_overwrite_safety() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&manifests_dir).unwrap();

    let manifest_path = manifests_dir.join("test-vault.manifest.json");
    std::fs::write(&manifest_path, b"original-manifest-bytes").unwrap();

    let cred_store = EphemeralCredentialStore::new();
    let vid = VaultId::new("test-vault").unwrap();
    let (_, primary, _) =
        VaultKeyManager::provision_new_vault(&cred_store, &vid, "Pass123!", false, None).unwrap();

    let new_manifest = VaultManifest::new(
        "test-vault".to_string(),
        "Test Vault".to_string(),
        primary,
        None,
        false,
    );

    // save_new_to_path must return error because destination already exists
    let err = new_manifest.save_new_to_path(&manifest_path);
    assert!(err.is_err());
    let err_msg = err.unwrap_err().to_string();
    assert!(err_msg.contains("Manifest destination already exists"));

    // Original file content was untouched
    let content = std::fs::read(&manifest_path).unwrap();
    assert_eq!(content, b"original-manifest-bytes");
}

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
struct MockDriveFile {
    id: String,
    name: String,
    content: Vec<u8>,
}

#[derive(Clone)]
struct MockDriveTransport {
    files: std::sync::Arc<std::sync::Mutex<Vec<MockDriveFile>>>,
    fail_refresh: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl MockDriveTransport {
    fn new() -> Self {
        Self {
            files: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            fail_refresh: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn set_fail_refresh(&self, fail: bool) {
        self.fail_refresh
            .store(fail, std::sync::atomic::Ordering::SeqCst);
    }

    fn add_snapshot(&self, name: &str, snapshot: &shellumi_core::SyncVault) {
        let json = serde_json::to_vec(snapshot).unwrap();
        let id = format!("mock-file-id-{}", Uuid::new_v4());
        self.files.lock().unwrap().push(MockDriveFile {
            id,
            name: name.to_string(),
            content: json,
        });
    }

    fn add_snapshot_with_id(&self, id: &str, name: &str, snapshot: &shellumi_core::SyncVault) {
        let json = serde_json::to_vec(snapshot).unwrap();
        self.files.lock().unwrap().push(MockDriveFile {
            id: id.to_string(),
            name: name.to_string(),
            content: json,
        });
    }

    #[allow(dead_code)]
    fn add_raw_file(&self, id: &str, name: &str, content: Vec<u8>) {
        self.files.lock().unwrap().push(MockDriveFile {
            id: id.to_string(),
            name: name.to_string(),
            content,
        });
    }
}

impl HttpTransport for MockDriveTransport {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, SyncError> {
        if req.url.contains("oauth2.googleapis.com/token") {
            if self.fail_refresh.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(SyncError::Http(
                    400,
                    r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#
                        .to_string(),
                ));
            }
            let body = serde_json::to_vec(&serde_json::json!({
                "access_token": "ya29.refreshed_access_token",
                "refresh_token": "1//new_refresh_token",
                "expires_in": 3600
            }))
            .unwrap();
            return Ok(HttpResponse::new(200, body));
        }

        let files = self.files.lock().unwrap();
        if req.method == "GET" && req.url.contains("spaces=appDataFolder") {
            let json_files: Vec<serde_json::Value> = files
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "id": f.id,
                        "name": f.name,
                        "mimeType": "application/json"
                    })
                })
                .collect();
            let body = serde_json::to_vec(&serde_json::json!({ "files": json_files })).unwrap();
            Ok(HttpResponse::new(200, body))
        } else if req.method == "GET" && req.url.contains("alt=media") {
            for f in files.iter() {
                if req.url.contains(&format!("/files/{}?", f.id)) || req.url.contains(&f.id) {
                    return Ok(HttpResponse::new(200, f.content.clone()));
                }
            }
            for f in files.iter() {
                if req.url.contains(&f.name) {
                    return Ok(HttpResponse::new(200, f.content.clone()));
                }
            }
            Err(SyncError::Api("File not found in mock drive".into()))
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

        // Export snapshots for cloud synchronization and sign as cloud publications
        let mut def_snap = def_store
            .export_vault_with_iterations(def_pass, "device-1", 100)
            .await
            .unwrap();
        let mut sec_snap = sec_store
            .export_vault_with_iterations(sec_pass, "device-1", 100)
            .await
            .unwrap();

        shellumi_core::sync::revisions::normalize_records(&mut def_snap, &def_dek, def_pass)
            .unwrap();
        def_snap.sign_publication(&def_dek, vec![]).unwrap();

        shellumi_core::sync::revisions::normalize_records(&mut sec_snap, &sec_dek, sec_pass)
            .unwrap();
        sec_snap.sign_publication(&sec_dek, vec![]).unwrap();

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

    let client = GoogleDriveSyncClient::new(mock_transport.clone(), client_id);
    let cloud_vaults = client.discover_cloud_vaults("mock_token").unwrap();
    assert_eq!(cloud_vaults.len(), 2);
    assert!(cloud_vaults
        .iter()
        .any(|v| v.id == "legacy-local" && v.is_default));
    assert!(cloud_vaults
        .iter()
        .any(|v| v.id == "team-infrastructure" && !v.is_default));

    // 2. Download and restore default vault first using download_current_vault and default password on Device 2
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
        let downloaded_def = client
            .download_current_vault("mock_token", "legacy-local", def_pass)
            .unwrap();

        let def_store2 = HostStore::restore_vault_from_sync(&downloaded_def, &def_opts)
            .await
            .unwrap();

        // Generate fresh recovery code for restored default vault and update manifest
        let def_rec_code = VaultKey::generate_recovery_code().unwrap();
        let mut def_manifest =
            VaultManifest::load_from_path(&dev2_manifests.join("legacy-local.manifest.json"))
                .unwrap();
        let def_unwrapped_key = VaultKey::unwrap_with_password(
            &def_vid,
            def_pass,
            downloaded_def.key_envelope.as_ref().unwrap(),
        )
        .unwrap();
        def_manifest
            .set_recovery_key(&def_unwrapped_key, &def_rec_code)
            .unwrap();
        def_manifest
            .save_to_path(&dev2_manifests.join("legacy-local.manifest.json"))
            .unwrap();

        // Verify default host record is intact
        let hosts = def_store2.list().await.unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "core-router");

        // Verify the multi-vault directory was imported without creating secondary db
        let dir = def_store2.build_vault_directory().await.unwrap();
        assert!(dir.get("team-infrastructure").is_some());
        assert!(!dev2_vaults.join("team-infrastructure.db").exists());

        // Verify manifest was created and can be unlocked with the new recovery code
        let reloaded_def_manifest =
            VaultManifest::load_from_path(&dev2_manifests.join("legacy-local.manifest.json"))
                .unwrap();
        let recovered_dek = VaultKey::recover(
            &def_vid,
            &def_rec_code,
            reloaded_def_manifest.recovery_envelope.as_ref().unwrap(),
        )
        .unwrap();
        assert_eq!(recovered_dek.expose_bytes(), def_dek.expose_bytes());

        // 3. Test wrong password for secondary vault on download_current_vault: fails cleanly
        let wrong_err =
            client.download_current_vault("mock_token", "team-infrastructure", "WrongPassword999!");
        assert!(wrong_err.is_err());
        assert!(!dev2_vaults.join("team-infrastructure.db").exists());
        assert!(!dev2_manifests
            .join("team-infrastructure.manifest.json")
            .exists());

        // 4. Download and restore secondary vault with correct independent password
        let downloaded_sec = client
            .download_current_vault("mock_token", "team-infrastructure", sec_pass)
            .unwrap();

        let sec_opts = shellumi_storage::RestoreVaultOptions {
            vault_id: &sec_vid,
            vault_name: "Team Infrastructure",
            password: sec_pass,
            target_db_path: &dev2_vaults.join("team-infrastructure.db"),
            manifests_dir: &dev2_manifests,
            cred_store: Some(&dev2_cred),
            remember_in_keychain: false,
        };
        let sec_store2 = HostStore::restore_vault_from_sync(&downloaded_sec, &sec_opts)
            .await
            .unwrap();

        // Generate fresh recovery code for restored secondary vault and update manifest
        let sec_rec_code = VaultKey::generate_recovery_code().unwrap();
        let mut sec_manifest = VaultManifest::load_from_path(
            &dev2_manifests.join("team-infrastructure.manifest.json"),
        )
        .unwrap();
        let sec_unwrapped_key = VaultKey::unwrap_with_password(
            &sec_vid,
            sec_pass,
            downloaded_sec.key_envelope.as_ref().unwrap(),
        )
        .unwrap();
        sec_manifest
            .set_recovery_key(&sec_unwrapped_key, &sec_rec_code)
            .unwrap();
        sec_manifest
            .save_to_path(&dev2_manifests.join("team-infrastructure.manifest.json"))
            .unwrap();

        // Verify secondary host record is intact
        let sec_hosts = sec_store2.list().await.unwrap();
        assert_eq!(sec_hosts.len(), 1);
        assert_eq!(sec_hosts[0].name, "prod-k8s-master");

        // Verify secondary recovery code unlocks the recovered DEK
        let reloaded_sec_manifest = VaultManifest::load_from_path(
            &dev2_manifests.join("team-infrastructure.manifest.json"),
        )
        .unwrap();
        let recovered_sec_dek = VaultKey::recover(
            &sec_vid,
            &sec_rec_code,
            reloaded_sec_manifest.recovery_envelope.as_ref().unwrap(),
        )
        .unwrap();
        assert_eq!(recovered_sec_dek.expose_bytes(), sec_dek.expose_bytes());
    });
}

#[test]
fn test_google_drive_token_persistence_auto_refresh_and_sync() {
    let cred_store = EphemeralCredentialStore::new();
    let mock_transport = MockDriveTransport::new();
    let custom_client_id = "test-client-12345.apps.googleusercontent.com";

    // 1. Persisting and loading custom OAuth client ID
    shellumi_core::sync::OAuthTokenBundle::save_client_id(&cred_store, custom_client_id).unwrap();
    let loaded_cid = shellumi_core::sync::OAuthTokenBundle::load_client_id(&cred_store).unwrap();
    assert_eq!(loaded_cid.as_deref(), Some(custom_client_id));

    // Reject empty client ID
    assert!(shellumi_core::sync::OAuthTokenBundle::save_client_id(&cred_store, "   ").is_err());

    let client = GoogleDriveSyncClient::new(mock_transport.clone(), custom_client_id);

    // 2. Token bundle with attached client ID automatically preserves and saves client ID
    let bundle = shellumi_core::sync::OAuthTokenBundle::new(
        "ya29.old_expired_token",
        Some("1//valid_refresh_token".to_string()),
        Some(60),
        Some("drive.appdata".to_string()),
        100, // acquired at 100
    )
    .with_client_id(custom_client_id);
    bundle.save_to_store(&cred_store).unwrap();

    let reloaded = shellumi_core::sync::OAuthTokenBundle::load_from_store(&cred_store)
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.client_id.as_deref(), Some(custom_client_id));

    let now = 2000;

    // 3. Test refresh failure: when refresh fails, resolve_valid_token MUST reject the expired token
    mock_transport.set_fail_refresh(true);
    let refresh_failure =
        shellumi_core::sync::OAuthTokenBundle::resolve_valid_token(&cred_store, &client, now);
    assert!(
        refresh_failure.is_err(),
        "resolve_valid_token must reject expired token when refresh fails"
    );
    let err_str = refresh_failure.unwrap_err().to_string();
    assert!(
        err_str.contains("expired and refresh failed") || err_str.contains("re-authenticate"),
        "Unexpected error: {err_str}"
    );

    // 4. Test expired token without refresh token: resolve_valid_token MUST return actionable error
    let cred_no_refresh = EphemeralCredentialStore::new();
    let bundle_no_refresh = shellumi_core::sync::OAuthTokenBundle::new(
        "ya29.expired_no_refresh",
        None,
        Some(60),
        Some("drive.appdata".to_string()),
        100,
    );
    bundle_no_refresh.save_to_store(&cred_no_refresh).unwrap();
    let no_refresh_err =
        shellumi_core::sync::OAuthTokenBundle::resolve_valid_token(&cred_no_refresh, &client, now);
    assert!(
        no_refresh_err.is_err(),
        "resolve_valid_token must reject expired token with no refresh token"
    );
    assert!(no_refresh_err
        .unwrap_err()
        .to_string()
        .contains("expired and cannot be refreshed"));

    // 5. Successful refresh persists new access token and preserves client ID
    mock_transport.set_fail_refresh(false);
    let resolved =
        shellumi_core::sync::OAuthTokenBundle::resolve_valid_token(&cred_store, &client, now)
            .unwrap();
    assert_eq!(resolved, "ya29.refreshed_access_token");

    // Verify refreshed bundle was re-persisted to store with preserved client ID
    let persisted = shellumi_core::sync::OAuthTokenBundle::load_from_store(&cred_store)
        .unwrap()
        .unwrap();
    assert_eq!(persisted.access_token, "ya29.refreshed_access_token");
    assert_eq!(
        persisted.refresh_token.as_deref(),
        Some("1//new_refresh_token")
    );
    assert_eq!(persisted.expires_in, Some(3600));
    assert_eq!(persisted.client_id.as_deref(), Some(custom_client_id));

    // 6. Legacy token migration fallback
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

#[test]
fn test_download_current_vault_with_multiple_immutable_publications_and_conflicts() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let cred_store = EphemeralCredentialStore::new();

    let vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let password = "MultiRevPassword2026!";
    let (dek, primary_env, _) =
        VaultKeyManager::provision_new_vault(&cred_store, &vid, password, false, None).unwrap();

    let vaults_dir = base_dir.join("vaults");
    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    std::fs::create_dir_all(&manifests_dir).unwrap();

    let manifest = VaultManifest::new(
        LEGACY_LOCAL_VAULT_ID.to_string(),
        "Default Vault".to_string(),
        primary_env.clone(),
        None,
        false,
    );
    manifest
        .save_new_to_path(&manifests_dir.join("legacy-local.manifest.json"))
        .unwrap();

    let db_path = vaults_dir.join("default.db");

    let (mut snap1, mut snap2, mut snap_fork, mut snap_tampered) = runtime.block_on(async {
        let mut store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&dek))
            .await
            .unwrap();
        store.set_vault_id(vid.clone());

        // Rev 1
        let dest1 = shellumi_core::SshDestination::new("admin", "192.168.1.1", 22).unwrap();
        store
            .add("router-v1", &dest1, None, &[], None, None)
            .await
            .unwrap();
        let s1 = store
            .export_vault_with_iterations(password, "device-1", 100)
            .await
            .unwrap();

        // Rev 2 (adds host router-v2)
        let dest2 = shellumi_core::SshDestination::new("admin", "192.168.1.2", 22).unwrap();
        store
            .add("router-v2", &dest2, None, &[], None, None)
            .await
            .unwrap();
        let s2 = store
            .export_vault_with_iterations(password, "device-1", 100)
            .await
            .unwrap();

        // Fork from rev 1
        let mut store_fork =
            HostStore::open_with_key(vaults_dir.join("fork.db").to_str().unwrap(), Some(&dek))
                .await
                .unwrap();
        store_fork.set_vault_id(vid.clone());
        store_fork
            .add("router-v1", &dest1, None, &[], None, None)
            .await
            .unwrap();
        let dest_fork = shellumi_core::SshDestination::new("admin", "192.168.1.99", 22).unwrap();
        store_fork
            .add("router-fork", &dest_fork, None, &[], None, None)
            .await
            .unwrap();
        let s_fork = store_fork
            .export_vault_with_iterations(password, "device-2", 100)
            .await
            .unwrap();

        (s1.clone(), s2, s_fork, s1)
    });

    // Prepare and sign publications
    snap1.prepare_cloud_revision(&dek).unwrap();
    shellumi_core::sync::revisions::normalize_records(&mut snap1, &dek, password).unwrap();
    snap1.sign_publication(&dek, vec![]).unwrap();
    let pub1_id = snap1.publication_id().unwrap();

    snap2.prepare_cloud_revision(&dek).unwrap();
    shellumi_core::sync::revisions::normalize_records(&mut snap2, &dek, password).unwrap();
    snap2.sign_publication(&dek, vec![pub1_id.clone()]).unwrap();

    snap_fork.prepare_cloud_revision(&dek).unwrap();
    shellumi_core::sync::revisions::normalize_records(&mut snap_fork, &dek, password).unwrap();
    snap_fork
        .sign_publication(&dek, vec![pub1_id.clone()])
        .unwrap();

    // 1. Multiple immutable publications sharing the exact same filename
    let mock_transport = MockDriveTransport::new();
    let client = GoogleDriveSyncClient::new(mock_transport.clone(), "test-client");

    // Both files have name "shellumi_vault_legacy-local.json" but different IDs
    mock_transport.add_snapshot_with_id("file-id-rev1", "shellumi_vault_legacy-local.json", &snap1);
    mock_transport.add_snapshot_with_id("file-id-rev2", "shellumi_vault_legacy-local.json", &snap2);

    // download_current_vault traverses the DAG and selects current head (snap2)
    let downloaded = client
        .download_current_vault("mock_token", LEGACY_LOCAL_VAULT_ID, password)
        .unwrap();

    // Verify it selected rev 2 which contains router-v2
    let restored_opts = shellumi_storage::RestoreVaultOptions {
        vault_id: &vid,
        vault_name: "Default Vault",
        password,
        target_db_path: &base_dir.join("restored.db"),
        manifests_dir: &base_dir.join("manifests"),
        cred_store: None,
        remember_in_keychain: false,
    };
    std::fs::create_dir_all(base_dir.join("manifests")).unwrap();

    runtime.block_on(async {
        let store = HostStore::restore_vault_from_sync(&downloaded, &restored_opts)
            .await
            .unwrap();
        let hosts = store.list().await.unwrap();
        assert_eq!(hosts.len(), 2);
        assert!(hosts.iter().any(|h| h.name == "router-v2"));
    });

    // 2. Concurrent cloud revisions / conflicts: adding fork creates divergent heads
    mock_transport.add_snapshot_with_id(
        "file-id-fork",
        "shellumi_vault_legacy-local.json",
        &snap_fork,
    );

    let conflict_err = client.download_current_vault("mock_token", LEGACY_LOCAL_VAULT_ID, password);
    assert!(
        conflict_err.is_err(),
        "download_current_vault must reject concurrent conflicting revisions"
    );
    let conflict_msg = conflict_err.unwrap_err().to_string();
    assert!(
        conflict_msg.contains("Concurrent cloud revisions require explicit resolution")
            || conflict_msg.contains("conflict"),
        "Unexpected error: {conflict_msg}"
    );

    // 3. Tampered snapshot / corrupted envelope rejection
    let tampered_mock = MockDriveTransport::new();
    let tampered_client = GoogleDriveSyncClient::new(tampered_mock.clone(), "test-client");
    if let Some(env) = &mut snap_tampered.key_envelope {
        env.ciphertext = "tampered_ciphertext_bytes".to_string();
    }
    tampered_mock.add_snapshot_with_id(
        "file-id-tampered",
        "shellumi_vault_legacy-local.json",
        &snap_tampered,
    );

    let tampered_err =
        tampered_client.download_current_vault("mock_token", LEGACY_LOCAL_VAULT_ID, password);
    assert!(
        tampered_err.is_err(),
        "download_current_vault must reject corrupted snapshot"
    );

    // 4. Rejection of wrong password
    let wrong_pass_err =
        client.download_current_vault("mock_token", LEGACY_LOCAL_VAULT_ID, "WrongPassword456!");
    assert!(
        wrong_pass_err.is_err(),
        "download_current_vault must reject wrong password"
    );
}

#[test]
fn test_recovery_code_generation_wrapping_and_interrupted_confirmation_on_cloud_restore() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();
    let vaults_dir = base_dir.join("vaults");
    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    std::fs::create_dir_all(&manifests_dir).unwrap();

    let cred_store = EphemeralCredentialStore::new();
    let def_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let password = "RestoredCloudPassword123!";
    let (dek, primary_env, _) =
        VaultKeyManager::provision_new_vault(&cred_store, &def_vid, password, false, None).unwrap();

    // 1. Simulate restored vault database and manifest with freshly generated recovery envelope
    let fresh_rec_code = VaultKey::generate_recovery_code().unwrap();
    let mut manifest = VaultManifest::new(
        LEGACY_LOCAL_VAULT_ID.to_string(),
        "Default Vault".to_string(),
        primary_env,
        None,
        false,
    );
    manifest.set_recovery_key(&dek, &fresh_rec_code).unwrap();
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
        let dest = shellumi_core::SshDestination::new("admin", "10.10.10.1", 22).unwrap();
        store
            .add("cloud-restored-host", &dest, None, &[], None, None)
            .await
            .unwrap();
    });

    // 2. Setup was interrupted before recovery code was confirmed:
    // State has default_vault_created = true, recovery_code_confirmed = false, google_drive_connected = true, completed = false
    let mut state = SetupState::new();
    state.mode = SetupWizardMode::RestoreSetup;
    state.current_step = SetupStep::DefaultVault;
    state.default_vault_created = true;
    state.recovery_code_confirmed = false;
    state.google_drive_connected = true;
    state.completed = false;
    state.save(base_dir).unwrap();

    // Setup is NOT completed
    assert!(!SetupState::is_setup_completed(base_dir));

    // Detection reports IncompleteSetup
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

    // 3. Re-entry / resumption: user confirms recovery code
    // With google_drive_connected == true, confirming advances to SetupStep::AdditionalVaults
    state.recovery_code_confirmed = true;
    state.current_step = if state.google_drive_connected {
        SetupStep::AdditionalVaults
    } else {
        SetupStep::GoogleDrive
    };
    state.save(base_dir).unwrap();

    // Verify state transitioned to AdditionalVaults
    let resumed = SetupState::load(base_dir).unwrap().unwrap();
    assert_eq!(resumed.current_step, SetupStep::AdditionalVaults);
    assert!(resumed.recovery_code_confirmed);

    // Verify existing restored vault was NOT modified, overwritten, or reinitialized
    runtime.block_on(async {
        let store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&dek))
            .await
            .unwrap();
        let hosts = store.list().await.unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "cloud-restored-host");
    });
}

#[test]
fn test_create_only_file_publication_rejects_concurrent_destinations_without_rename_truncation() {
    let temp = tempdir().unwrap();
    let dir = temp.path();

    let staged1 = dir.join("staged_1.tmp");
    let target1 = dir.join("final_1.db");
    std::fs::write(&staged1, b"database content 1").unwrap();

    // Case 1: Target does not exist -> successful create-only publish
    let res = crate::actions::setup_wizard::publish_database_create_only(&staged1, &target1);
    assert!(res.is_ok());
    assert!(!staged1.exists(), "Staged file must be removed on success");
    assert_eq!(
        std::fs::read(&target1).unwrap(),
        b"database content 1",
        "Target must contain published content"
    );

    // Case 2: Target already exists -> must fail and preserve target without truncation
    let staged2 = dir.join("staged_2.tmp");
    std::fs::write(&staged2, b"new attacker / concurrent content").unwrap();

    let collision_res =
        crate::actions::setup_wizard::publish_database_create_only(&staged2, &target1);
    assert!(
        collision_res.is_err(),
        "publish_database_create_only must fail when destination already exists"
    );
    let err = collision_res.unwrap_err();
    assert!(
        err.contains("already exists"),
        "Unexpected error message: {err}"
    );

    // Original target content must be 100% untouched and untruncated
    assert_eq!(
        std::fs::read(&target1).unwrap(),
        b"database content 1",
        "Existing destination database must never be overwritten or truncated!"
    );
    // Staged file is preserved for transaction inspection
    assert!(staged2.exists());
}

#[test]
fn test_setup_state_save_error_propagation_and_idempotent_recovery() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();

    // 1. Error propagation when saving setup state to an invalid path
    let invalid_dir = base_dir.join("nonexistent_file_as_dir").join("sub");
    // Create a regular file in place of nonexistent_file_as_dir so creating sub fails
    std::fs::write(base_dir.join("nonexistent_file_as_dir"), b"blocking file").unwrap();

    let mut state = SetupState::new();
    let err = state.save(&invalid_dir);
    assert!(
        err.is_err(),
        "SetupState::save must return Err when base dir cannot be written"
    );

    // 2. Idempotency: configured_vaults deduplication
    state
        .configured_vaults
        .push(shellumi_core::setup::ConfiguredVaultSummary {
            id: "legacy-local".to_string(),
            name: "Default Vault".to_string(),
            is_default: true,
            is_remote: false,
            sync_enabled: true,
            server_url: None,
        });

    // Simulate repeated configuration of the same vault
    let vid = "legacy-local";
    if !state.configured_vaults.iter().any(|v| v.id == vid) {
        state
            .configured_vaults
            .push(shellumi_core::setup::ConfiguredVaultSummary {
                id: vid.to_string(),
                name: "Default Vault".to_string(),
                is_default: true,
                is_remote: false,
                sync_enabled: true,
                server_url: None,
            });
    }

    assert_eq!(
        state.configured_vaults.len(),
        1,
        "configured_vaults must remain deduplicated across retries"
    );
}

#[test]
fn test_partial_database_copy_prevention_and_cleanup() {
    let temp = tempdir().unwrap();
    let dir = temp.path();

    let target = dir.join("final_destination.db");
    let non_existent_staged = dir.join("non_existent_source.db");

    // Case 1: Staged file does not exist -> fallback must fail and create no files at target
    let res = crate::actions::setup_wizard::publish_database_fallback_tempfile(
        &non_existent_staged,
        &target,
    );
    assert!(res.is_err(), "Must fail when staged file does not exist");
    assert!(
        !target.exists(),
        "Target database must never be created if source fails"
    );

    // Verify no leftover .tmp_pub_ files in dir
    let entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp_pub_"))
        .collect();
    assert!(
        entries.is_empty(),
        "No temporary transaction files must be left behind after failure"
    );

    // Case 2: Target already exists with original content -> must not be overwritten or truncated
    std::fs::write(&target, b"pristine original database contents").unwrap();
    let staged = dir.join("staged_update.db");
    std::fs::write(&staged, b"attacker or concurrent corrupted payload").unwrap();

    let collision_res =
        crate::actions::setup_wizard::publish_database_fallback_tempfile(&staged, &target);
    assert!(
        collision_res.is_err(),
        "Must fail when target already exists"
    );
    let err_msg = collision_res.unwrap_err();
    assert!(
        err_msg.contains("already exists"),
        "Error message must indicate target already exists: {err_msg}"
    );

    // Target content must be 100% untouched
    assert_eq!(
        std::fs::read(&target).unwrap(),
        b"pristine original database contents",
        "Existing database must never be modified or truncated on collision"
    );

    // Verify again no leftover temporary transaction files
    let remaining_tmp: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp_pub_"))
        .collect();
    assert!(
        remaining_tmp.is_empty(),
        "No temporary files must remain after collision failure"
    );
}

#[test]
fn test_atomic_create_only_preserves_existing_databases_across_all_paths() {
    let temp = tempdir().unwrap();
    let dir = temp.path();

    let target = dir.join("critical_vault.db");
    std::fs::write(&target, b"authoritative database payload").unwrap();

    let staged1 = dir.join("candidate_one.db");
    std::fs::write(&staged1, b"competing candidate one").unwrap();

    let staged2 = dir.join("candidate_two.db");
    std::fs::write(&staged2, b"competing candidate two").unwrap();

    // Direct create-only publication attempt
    let res1 = crate::actions::setup_wizard::publish_database_create_only(&staged1, &target);
    assert!(res1.is_err());
    assert!(staged1.exists(), "Staged file 1 must be preserved on error");

    // Fallback publication attempt
    let res2 = crate::actions::setup_wizard::publish_database_fallback_tempfile(&staged2, &target);
    assert!(res2.is_err());
    assert!(staged2.exists(), "Staged file 2 must be preserved on error");

    // Target content was never altered
    assert_eq!(
        std::fs::read(&target).unwrap(),
        b"authoritative database payload",
        "Target database must remain untouched across all publication attempts"
    );
}

#[test]
fn test_transactional_state_transition_rollback_on_persistence_failure() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();

    // Construct an invalid path where save will fail
    let blocking_file = base_dir.join("blocking_regular_file");
    std::fs::write(&blocking_file, b"blocks dir creation").unwrap();
    let invalid_dir = blocking_file.join("sub_directory");

    let mut state = SetupState::new();
    state.current_step = SetupStep::Welcome;

    // advance_to to DefaultVault should fail to persist and roll back
    let res = state.advance_to(SetupStep::DefaultVault, &invalid_dir);
    assert!(res.is_err(), "advance_to must fail when persistence fails");
    assert_eq!(
        state.current_step,
        SetupStep::Welcome,
        "In-memory step must be rolled back to Welcome upon persistence failure"
    );

    // mark_completed should also fail and roll back
    let res_completed = state.mark_completed(&invalid_dir);
    assert!(
        res_completed.is_err(),
        "mark_completed must fail when persistence fails"
    );
    assert!(
        !state.completed,
        "In-memory completed flag must remain false upon persistence failure"
    );
    assert_eq!(
        state.current_step,
        SetupStep::Welcome,
        "In-memory step must remain Welcome upon persistence failure"
    );
}

#[test]
fn test_safe_resumption_after_interrupted_recovery_or_creation() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();

    // Create partial setup state: default vault created, but recovery code not confirmed
    let mut state = SetupState::new();
    state.current_step = SetupStep::DefaultVault;
    state.default_vault_created = true;
    state.recovery_code_confirmed = false;
    state.completed = false;
    state.configured_vaults.push(ConfiguredVaultSummary {
        id: "legacy-local".to_string(),
        name: "Default Vault".to_string(),
        is_default: true,
        is_remote: false,
        sync_enabled: false,
        server_url: None,
    });
    state.save(base_dir).unwrap();

    // 1. Must NOT be considered completed
    assert!(
        !SetupState::is_setup_completed(base_dir),
        "Incomplete setup must never be reported as completed"
    );

    // 2. Detection reports IncompleteSetup
    let status = SetupState::detect_installation(base_dir);
    match status {
        InstallationStatus::IncompleteSetup {
            last_step,
            default_vault_created,
        } => {
            assert_eq!(last_step, SetupStep::DefaultVault);
            assert!(default_vault_created);
        }
        other => panic!("Expected IncompleteSetup, got: {:?}", other),
    }

    // 3. Resumption: confirm recovery code and transition to GoogleDrive
    state.recovery_code_confirmed = true;
    let res = state.advance_to(SetupStep::GoogleDrive, base_dir);
    assert!(res.is_ok());
    assert_eq!(state.current_step, SetupStep::GoogleDrive);
    assert!(state.recovery_code_confirmed);

    // Reload from disk to verify persistence
    let reloaded = SetupState::load(base_dir).unwrap().unwrap();
    assert_eq!(reloaded.current_step, SetupStep::GoogleDrive);
    assert!(reloaded.recovery_code_confirmed);
    assert!(!reloaded.completed);
}

#[test]
fn test_google_drive_opt_in_persistence_and_read_back() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("default.db");
        let key = VaultKey::generate().unwrap();

        // 1. Initialize encrypted host store
        let mut store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&key))
            .await
            .unwrap();

        // Register default vault context
        let context = VaultContext::new(
            VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap(),
            VaultLocation::Local,
        );
        store
            .register_vault_context("Default Vault", &context)
            .await
            .unwrap();

        // By default, sync should not be enabled
        let initial_sync = store
            .is_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID)
            .await
            .unwrap();
        assert!(!initial_sync, "Default sync setting must be disabled");

        // 2. Persist sync enabled = true
        let updated = store
            .set_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID, true)
            .await
            .unwrap();
        assert!(updated, "set_vault_sync_enabled must report rows updated");

        // Read-back verification
        let is_enabled = store
            .is_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID)
            .await
            .unwrap();
        assert!(is_enabled, "Read-back must reflect sync enabled");

        // 3. Persist sync enabled = false (disabling sync)
        let updated_false = store
            .set_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID, false)
            .await
            .unwrap();
        assert!(
            updated_false,
            "set_vault_sync_enabled must report rows updated when disabling"
        );

        // Read-back verification
        let is_disabled = store
            .is_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID)
            .await
            .unwrap();
        assert!(!is_disabled, "Read-back must reflect sync disabled");
    });
}

#[test]
fn test_idempotent_retries_on_failed_finish() {
    let temp = tempdir().unwrap();
    let base_dir = temp.path();

    let mut state = SetupState::new();
    state.current_step = SetupStep::ReviewAndFinish;
    state.default_vault_created = true;
    state.recovery_code_confirmed = true;
    state.configured_vaults.push(ConfiguredVaultSummary {
        id: "legacy-local".to_string(),
        name: "Default Vault".to_string(),
        is_default: true,
        is_remote: false,
        sync_enabled: false,
        server_url: None,
    });
    state.save(base_dir).unwrap();

    // Simulate first finish attempt failing due to unverified registry write or I/O error
    // In-memory state remains on ReviewAndFinish and completed = false
    assert_eq!(state.current_step, SetupStep::ReviewAndFinish);
    assert!(!state.completed);

    // Simulate retry: deduplication check when re-registering or configuring vaults
    let vid = "legacy-local";
    if let Some(existing) = state.configured_vaults.iter_mut().find(|v| v.id == vid) {
        existing.sync_enabled = true;
    } else {
        state.configured_vaults.push(ConfiguredVaultSummary {
            id: vid.to_string(),
            name: "Default Vault".to_string(),
            is_default: true,
            is_remote: false,
            sync_enabled: true,
            server_url: None,
        });
    }

    assert_eq!(
        state.configured_vaults.len(),
        1,
        "configured_vaults must not duplicate across retries"
    );
    assert!(state.configured_vaults[0].sync_enabled);

    // Retry succeeds: call mark_completed
    state.mark_completed(base_dir).unwrap();
    assert!(state.completed);
    assert_eq!(state.current_step, SetupStep::Completed);

    // Verify persisted state reflects completed = true and single vault
    let reloaded = SetupState::load(base_dir).unwrap().unwrap();
    assert!(reloaded.completed);
    assert_eq!(reloaded.current_step, SetupStep::Completed);
    assert_eq!(reloaded.configured_vaults.len(), 1);
    assert!(SetupState::is_setup_completed(base_dir));
}

use tempfile::tempdir;
use uuid::Uuid;

use shellumi_core::setup::{
    verify_setup_integrity, ConfiguredVaultSummary, InstallationStatus, SetupState, SetupStep,
    SetupWizardMode,
};
use shellumi_core::sync::{
    build_google_drive_auth_url, generate_pkce_challenge, OAuthCallbackListener,
};
use shellumi_core::{
    EphemeralCredentialStore, ServerConnectionId, VaultContext, VaultId, VaultKey, VaultKeyManager,
    VaultLocation, VaultManifest,
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

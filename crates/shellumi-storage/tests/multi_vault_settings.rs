use shellumi_core::{SshDestination, VaultContext, VaultId, VaultKey, VaultLocation};
use shellumi_storage::{HostStore, LEGACY_LOCAL_VAULT_ID};

#[tokio::test]
async fn independent_drive_opt_in_survives_restart_and_secondary_export_keeps_identity() {
    let temp = tempfile::tempdir().unwrap();
    let vaults = temp.path().join("vaults");
    std::fs::create_dir_all(&vaults).unwrap();
    let registry_path = vaults.join("default.db");
    let default_key = VaultKey::generate().unwrap();
    let mut registry =
        HostStore::open_with_key(registry_path.to_str().unwrap(), Some(&default_key))
            .await
            .unwrap();
    for id in ["opted-in", "private"] {
        let context = VaultContext::new(VaultId::new(id).unwrap(), VaultLocation::Local);
        registry.register_vault_context(id, &context).await.unwrap();
        assert!(!registry.is_vault_sync_enabled(id).await.unwrap());
    }
    assert!(!registry
        .is_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID)
        .await
        .unwrap());
    registry
        .set_vault_sync_enabled("opted-in", true)
        .await
        .unwrap();
    registry
        .set_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID, true)
        .await
        .unwrap();
    registry
        .set_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID, false)
        .await
        .unwrap();
    let directory = registry.build_vault_directory().await.unwrap();
    assert!(directory.get("opted-in").unwrap().sync_enabled);
    assert!(!directory.get("private").unwrap().sync_enabled);
    assert!(!directory.get(LEGACY_LOCAL_VAULT_ID).unwrap().sync_enabled);
    drop(registry);
    let registry = HostStore::open_with_key(registry_path.to_str().unwrap(), Some(&default_key))
        .await
        .unwrap();
    assert!(registry.is_vault_sync_enabled("opted-in").await.unwrap());
    assert!(!registry.is_vault_sync_enabled("private").await.unwrap());
    assert!(!registry
        .is_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID)
        .await
        .unwrap());
    let secondary_key = VaultKey::generate().unwrap();
    let secondary_path = vaults.join("opted-in.db");
    let mut secondary =
        HostStore::open_with_key(secondary_path.to_str().unwrap(), Some(&secondary_key))
            .await
            .unwrap();
    secondary.set_vault_id(VaultId::new("opted-in").unwrap());
    secondary
        .add(
            "secondary-host",
            &SshDestination::new("example.org", "user", 22).unwrap(),
            None,
            &[],
            None,
            None,
        )
        .await
        .unwrap();
    // A secondary database has no authority over the registry's opt-in setting.
    assert!(!secondary.is_vault_sync_enabled("opted-in").await.unwrap());
    let exported = secondary
        .export_vault_with_iterations("StrongPassword123!", "second-device", 1000)
        .await
        .unwrap();
    assert_eq!(exported.vault_id, "opted-in");
    assert_eq!(exported.records.len(), 1);
    assert!(registry
        .is_vault_sync_enabled(&exported.vault_id)
        .await
        .unwrap());
}

#[tokio::test]
async fn directory_restore_updates_default_consent_and_preserves_shared_server_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("registry.db");
    let key = VaultKey::generate().unwrap();
    let default_id = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let mut registry = HostStore::open_with_key(path.to_str().unwrap(), Some(&key))
        .await
        .unwrap();
    registry
        .set_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID, true)
        .await
        .unwrap();
    let connection = shellumi_storage::VaultServerConnectionRecord {
        id: "server".into(),
        base_url: "https://example.org".into(),
        auth_token: Some("keychain://org.shellumi.server/existing".into()),
        server_instance_id: Some("instance".into()),
        ..Default::default()
    };
    registry
        .save_vault_server_connection(&connection)
        .await
        .unwrap();
    let mut directory = shellumi_core::VaultDirectory::new();
    for (id, enabled, location) in [
        (
            LEGACY_LOCAL_VAULT_ID,
            false,
            shellumi_core::VaultDirectoryLocation::Local,
        ),
        (
            "remote-one",
            true,
            shellumi_core::VaultDirectoryLocation::Remote {
                server_connection_id: "server".into(),
                server_vault_id: "remote-id-one".into(),
            },
        ),
        (
            "remote-two",
            false,
            shellumi_core::VaultDirectoryLocation::Remote {
                server_connection_id: "server".into(),
                server_vault_id: "remote-id-two".into(),
            },
        ),
    ] {
        directory.upsert(shellumi_core::VaultDirectoryEntry {
            id: id.into(),
            name: id.into(),
            location,
            has_recovery_envelope: false,
            sync_enabled: enabled,
            created_at: 1,
            updated_at: 2,
        });
    }
    let envelope = serde_json::to_string(&directory.encrypt(&default_id, &key).unwrap()).unwrap();
    assert_eq!(
        registry
            .import_encrypted_vault_directory(&default_id, Some(&key), &envelope)
            .await
            .unwrap(),
        2
    );
    assert!(!registry
        .is_vault_sync_enabled(LEGACY_LOCAL_VAULT_ID)
        .await
        .unwrap());
    assert!(registry.is_vault_sync_enabled("remote-one").await.unwrap());
    assert!(!registry.is_vault_sync_enabled("remote-two").await.unwrap());
    assert_eq!(
        registry
            .get_vault_server_connection("server")
            .await
            .unwrap()
            .unwrap(),
        connection
    );
}

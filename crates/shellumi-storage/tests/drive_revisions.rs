#[path = "../../shellumi-core/tests/support/drive.rs"]
mod drive;
use drive::Drive;
use shellumi_core::{
    execute_cloud_vault_sync, execute_cloud_vault_sync_bound, CloudConflictResolution,
    ConflictResolutionChoice, GoogleDriveSyncClient, SyncCycleResult, SyncVault, VaultId, VaultKey,
    VaultManifest,
};
use shellumi_storage::{HostStore, RestoreVaultOptions, LEGACY_LOCAL_VAULT_ID};

const OLD: &str = "OriginalPassword123!";
const NEW: &str = "UpdatedPassword456!";

struct Device {
    temp: tempfile::TempDir,
    key: VaultKey,
    id: VaultId,
    store: HostStore,
}
impl Device {
    async fn new(id: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let key = VaultKey::generate().unwrap();
        let id = VaultId::new(id).unwrap();
        let manifest = VaultManifest::new(
            id.as_str().into(),
            "Vault".into(),
            key.wrap_with_argon2_params(&id, OLD, 64, 1, 1).unwrap(),
            None,
            false,
        );
        let manifests = temp.path().join("vault_manifests");
        manifest
            .save_to_path(&manifests.join(format!("{}.manifest.json", id.as_str())))
            .unwrap();
        let vaults = temp.path().join("vaults");
        std::fs::create_dir_all(&vaults).unwrap();
        let path = vaults.join(if id.as_str() == LEGACY_LOCAL_VAULT_ID {
            "default.db".into()
        } else {
            format!("{}.db", id.as_str())
        });
        let mut store = HostStore::open_with_key(path.to_str().unwrap(), Some(&key))
            .await
            .unwrap();
        store.set_vault_id(id.clone());
        store
            .add(
                "host",
                &shellumi_core::SshDestination::new("example.org", "user", 22).unwrap(),
                None,
                &[],
                None,
                None,
            )
            .await
            .unwrap();
        if id.as_str() == LEGACY_LOCAL_VAULT_ID {
            store
                .set_vault_sync_enabled(id.as_str(), true)
                .await
                .unwrap();
        }
        Self {
            temp,
            key,
            id,
            store,
        }
    }
    fn manifest_path(&self) -> std::path::PathBuf {
        self.temp.path().join(format!(
            "vault_manifests/{}.manifest.json",
            self.id.as_str()
        ))
    }
    fn rotate(&self, pwd: &str) {
        let mut manifest = VaultManifest::load_from_path(&self.manifest_path()).unwrap();
        manifest
            .change_password_with_params(&self.key, pwd, 64, 1, 1)
            .unwrap();
        manifest.save_to_path(&self.manifest_path()).unwrap();
    }
    async fn export(&mut self) -> SyncVault {
        self.store
            .export_vault_with_iterations(&hex::encode(self.key.expose_bytes()), "device", 1000)
            .await
            .unwrap()
    }
    async fn apply(&mut self, result: &SyncCycleResult) {
        if let SyncCycleResult::Success {
            report,
            merged_records,
        } = result
        {
            let snapshot = report
                .snapshot
                .as_deref()
                .expect("cloud success must carry exact authenticated snapshot");
            assert_eq!(&snapshot.records, merged_records);
            self.store
                .import_vault(&hex::encode(self.key.expose_bytes()), snapshot)
                .await
                .unwrap();
        }
    }
    async fn sync(&mut self, drive: &Drive) -> Result<SyncCycleResult, shellumi_core::SyncError> {
        let local = self.export().await;
        let result = execute_cloud_vault_sync(
            &GoogleDriveSyncClient::new(drive.clone(), "test"),
            "token",
            &local,
            &hex::encode(self.key.expose_bytes()),
            None,
        )?;
        self.apply(&result).await;
        Ok(result)
    }
    async fn restore(snapshot: &SyncVault, pwd: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let id = VaultId::new(&snapshot.vault_id).unwrap();
        let key = VaultKey::unwrap_with_password(&id, pwd, snapshot.key_envelope.as_ref().unwrap())
            .unwrap();
        let path = temp
            .path()
            .join("vaults")
            .join(if id.as_str() == LEGACY_LOCAL_VAULT_ID {
                "default.db".into()
            } else {
                format!("{}.db", id.as_str())
            });
        let manifests = temp.path().join("vault_manifests");
        let options = RestoreVaultOptions {
            vault_id: &id,
            vault_name: "Restored",
            password: pwd,
            target_db_path: &path,
            manifests_dir: &manifests,
            cred_store: None,
            remember_in_keychain: false,
        };
        let store = HostStore::restore_vault_from_sync(snapshot, &options)
            .await
            .unwrap();
        Self {
            temp,
            key,
            id,
            store,
        }
    }
}
fn latest(drive: &Drive, id: &str, pwd: &str) -> SyncVault {
    GoogleDriveSyncClient::new(drive.clone(), "test")
        .download_current_vault("token", id, pwd)
        .unwrap()
}

#[tokio::test]
async fn password_only_rotation_restores_on_fresh_device_without_original_secrets() {
    let drive = Drive::default();
    let mut first = Device::new(LEGACY_LOCAL_VAULT_ID).await;
    first.sync(&drive).await.unwrap();
    assert_eq!(
        first.sync(&drive).await.unwrap(),
        SyncCycleResult::AlreadySynchronized
    );
    let records = first.export().await.records;
    first.rotate(NEW);
    let result = first.sync(&drive).await.unwrap();
    assert!(matches!(result, SyncCycleResult::Success { .. }));
    let current = latest(&drive, LEGACY_LOCAL_VAULT_ID, NEW);
    assert_eq!(current.records.len(), records.len());
    assert_eq!(current.records[0].version, records[0].version);
    assert!(
        VaultKey::unwrap_with_password(&first.id, OLD, current.key_envelope.as_ref().unwrap())
            .is_err()
    );
    let rejected = tempfile::tempdir().unwrap();
    let rejected_db = rejected.path().join("vaults/default.db");
    let rejected_manifests = rejected.path().join("vault_manifests");
    let old_options = RestoreVaultOptions {
        vault_id: &first.id,
        vault_name: "Rejected",
        password: OLD,
        target_db_path: &rejected_db,
        manifests_dir: &rejected_manifests,
        cred_store: None,
        remember_in_keychain: false,
    };
    assert!(HostStore::restore_vault_from_sync(&current, &old_options)
        .await
        .is_err());
    assert!(!rejected_db.exists());
    assert!(!rejected_manifests.exists());
    // The fresh device receives only the cloud snapshot and password.
    let second = Device::restore(&current, NEW).await;
    assert_eq!(second.store.list().await.unwrap()[0].name, "host");
    assert_eq!(
        first.sync(&drive).await.unwrap(),
        SyncCycleResult::AlreadySynchronized
    );
    let state = drive.0.lock().unwrap();
    assert_eq!(
        state.files.len(),
        2,
        "password-only change must publish exactly one new immutable snapshot"
    );
    assert!(!state
        .requests
        .iter()
        .any(|r| r.method == "PATCH" || r.method == "PUT" || r.method == "DELETE"));
}

#[tokio::test]
async fn stale_device_record_update_cannot_restore_an_old_password_envelope() {
    let drive = Drive::default();
    let mut first = Device::new(LEGACY_LOCAL_VAULT_ID).await;
    first.sync(&drive).await.unwrap();
    let mut stale = Device::restore(&latest(&drive, LEGACY_LOCAL_VAULT_ID, OLD), OLD).await;
    first.rotate(NEW);
    first.sync(&drive).await.unwrap();
    stale
        .store
        .add(
            "stale-device-host",
            &shellumi_core::SshDestination::new("stale.example.org", "user", 22).unwrap(),
            None,
            &[],
            None,
            None,
        )
        .await
        .unwrap();
    stale.sync(&drive).await.unwrap();
    let current = latest(&drive, LEGACY_LOCAL_VAULT_ID, NEW);
    assert_eq!(current.records.len(), 2);
    assert!(
        VaultKey::unwrap_with_password(&first.id, OLD, current.key_envelope.as_ref().unwrap())
            .is_err()
    );
    let restored = Device::restore(&current, NEW).await;
    assert_eq!(restored.store.list().await.unwrap().len(), 2);
}

#[tokio::test]
async fn concurrent_password_changes_require_bound_explicit_resolution() {
    let drive = Drive::default();
    let mut first = Device::new(LEGACY_LOCAL_VAULT_ID).await;
    first.sync(&drive).await.unwrap();
    let mut second = Device::restore(&latest(&drive, LEGACY_LOCAL_VAULT_ID, OLD), OLD).await;
    first.rotate(NEW);
    second.rotate("OtherPassword789!");
    first.sync(&drive).await.unwrap();
    let conflict = second.sync(&drive).await.unwrap();
    let SyncCycleResult::Conflicts(views) = conflict else {
        panic!("concurrent changes silently selected an envelope")
    };
    assert!(views.iter().any(|v| v.object_id == "vault:key"));
    let resolution = CloudConflictResolution {
        context: views[0].revision_context.clone().unwrap(),
        choices: vec![ConflictResolutionChoice::KeepLocal; views.len()],
    };
    first.rotate("ThirdPassword123!");
    first.sync(&drive).await.unwrap();
    let local = second.export().await;
    let client = GoogleDriveSyncClient::new(drive.clone(), "test");
    assert!(
        execute_cloud_vault_sync_bound(
            &client,
            "token",
            &local,
            &hex::encode(second.key.expose_bytes()),
            &resolution
        )
        .is_err(),
        "stale conflict view was applied"
    );
    let SyncCycleResult::Conflicts(views) = second.sync(&drive).await.unwrap() else {
        panic!("expected refreshed conflict")
    };
    let resolution = CloudConflictResolution {
        context: views[0].revision_context.clone().unwrap(),
        choices: vec![ConflictResolutionChoice::KeepLocal; views.len()],
    };
    let local = second.export().await;
    let result = execute_cloud_vault_sync_bound(
        &client,
        "token",
        &local,
        &hex::encode(second.key.expose_bytes()),
        &resolution,
    )
    .unwrap();
    second.apply(&result).await;
    let restored = Device::restore(
        &latest(&drive, LEGACY_LOCAL_VAULT_ID, "OtherPassword789!"),
        "OtherPassword789!",
    )
    .await;
    assert_eq!(restored.store.list().await.unwrap().len(), 1);
    first.sync(&drive).await.unwrap();
    assert!(VaultManifest::load_from_path(&first.manifest_path())
        .unwrap()
        .unlock("OtherPassword789!")
        .is_ok());
}

#[tokio::test]
async fn metadata_only_changes_and_concurrent_registry_updates_are_not_silent() {
    let drive = Drive::default();
    let mut first = Device::new(LEGACY_LOCAL_VAULT_ID).await;
    first.sync(&drive).await.unwrap();
    let mut second = Device::restore(&latest(&drive, LEGACY_LOCAL_VAULT_ID, OLD), OLD).await;
    first
        .store
        .register_vault_context(
            "Renamed",
            &shellumi_core::VaultContext::new(
                first.id.clone(),
                shellumi_core::VaultLocation::Local,
            ),
        )
        .await
        .unwrap();
    first.sync(&drive).await.unwrap();
    second
        .store
        .register_vault_context(
            "Concurrent rename",
            &shellumi_core::VaultContext::new(
                second.id.clone(),
                shellumi_core::VaultLocation::Local,
            ),
        )
        .await
        .unwrap();
    let SyncCycleResult::Conflicts(views) = second.sync(&drive).await.unwrap() else {
        panic!("concurrent metadata was silently overwritten")
    };
    assert!(views.iter().any(|v| v.object_id == "vault:metadata"));
    let local = second.export().await;
    let resolution = CloudConflictResolution {
        context: views[0].revision_context.clone().unwrap(),
        choices: vec![ConflictResolutionChoice::AcceptRemote; views.len()],
    };
    let result = execute_cloud_vault_sync_bound(
        &GoogleDriveSyncClient::new(drive.clone(), "test"),
        "token",
        &local,
        &hex::encode(second.key.expose_bytes()),
        &resolution,
    )
    .unwrap();
    second.apply(&result).await;
    assert_eq!(
        second.store.list_vault_contexts().await.unwrap()[0].0,
        "Renamed"
    );
    assert_eq!(
        second.sync(&drive).await.unwrap(),
        SyncCycleResult::AlreadySynchronized
    );
}

#[tokio::test]
async fn conditional_creation_lost_ack_retries_and_tamper_checks_preserve_access() {
    let drive = Drive::default();
    let mut first = Device::new(LEGACY_LOCAL_VAULT_ID).await;
    drive.0.lock().unwrap().lose_ack = true;
    first.sync(&drive).await.unwrap();
    drive.0.lock().unwrap().page_size = 1;
    first.rotate(NEW);
    drive.0.lock().unwrap().fail_upload = true;
    assert!(first.sync(&drive).await.is_err());
    drive.0.lock().unwrap().fail_upload = false;
    first.sync(&drive).await.unwrap();
    let current = latest(&drive, LEGACY_LOCAL_VAULT_ID, NEW);
    let client = GoogleDriveSyncClient::new(drive.clone(), "test");
    let (id, original) = drive
        .0
        .lock()
        .unwrap()
        .files
        .iter()
        .find(|(_, (_, v))| v.publication_id().unwrap() == current.publication_id().unwrap())
        .map(|(id, (_, v))| (id.clone(), v.clone()))
        .unwrap();
    drive.0.lock().unwrap().reuse_id = Some(id.clone());
    client.publish_immutable("token", &current).unwrap(); // 409 + exact read-back is an idempotent create.
    let mut altered = current.clone();
    altered.updated_at += 1;
    drive.0.lock().unwrap().reuse_id = Some(id.clone());
    assert!(client.publish_immutable("token", &altered).is_err());
    assert_eq!(drive.0.lock().unwrap().files[&id].1, original);
    let mut tampered = current.clone();
    tampered.cloud_state.as_mut().unwrap().key.head = "forged".into();
    assert!(first
        .store
        .import_vault(&hex::encode(first.key.expose_bytes()), &tampered)
        .await
        .is_err());
    assert!(
        Device::restore(&current, NEW)
            .await
            .store
            .list()
            .await
            .unwrap()
            .len()
            == 1
    );
}

#[tokio::test]
async fn independent_vault_histories_and_unknown_versions_fail_closed() {
    let drive = Drive::default();
    let mut first = Device::new("first").await;
    let mut second = Device::new("second").await;
    first.sync(&drive).await.unwrap();
    second.sync(&drive).await.unwrap();
    let before = latest(&drive, "second", OLD);
    first.rotate(NEW);
    first.sync(&drive).await.unwrap();
    assert_eq!(latest(&drive, "second", OLD), before);
    assert_eq!(
        second.sync(&drive).await.unwrap(),
        SyncCycleResult::AlreadySynchronized
    );
    let mut foreign = latest(&drive, "first", NEW);
    foreign.vault_id = "second".into();
    assert!(second
        .store
        .import_vault(&hex::encode(second.key.expose_bytes()), &foreign)
        .await
        .is_err());
    let mut future = before;
    future.cloud_state.as_mut().unwrap().version = 999;
    assert!(second
        .store
        .import_vault(&hex::encode(second.key.expose_bytes()), &future)
        .await
        .is_err());
}

#[tokio::test]
async fn legacy_snapshot_upgrades_without_replacing_original_and_survives_restart() {
    let drive = Drive::default();
    let mut first = Device::new(LEGACY_LOCAL_VAULT_ID).await;
    let mut legacy = first.export().await;
    legacy.cloud_state = None;
    drive.0.lock().unwrap().files.insert(
        "legacy".into(),
        (
            shellumi_core::sync::drive_vault_filename(LEGACY_LOCAL_VAULT_ID),
            legacy.clone(),
        ),
    );
    let readable = latest(&drive, LEGACY_LOCAL_VAULT_ID, OLD);
    let mut second = Device::restore(&readable, OLD).await;
    second.sync(&drive).await.unwrap();
    assert_eq!(drive.0.lock().unwrap().files["legacy"].1, legacy);
    second.rotate(NEW);
    second.sync(&drive).await.unwrap();
    let path = second.store.db_path().unwrap().to_owned();
    let mut reopened = HostStore::open_with_key(&path, Some(&second.key))
        .await
        .unwrap();
    reopened.set_vault_id(second.id.clone());
    second.store = reopened;
    assert_eq!(
        second.sync(&drive).await.unwrap(),
        SyncCycleResult::AlreadySynchronized
    );
    assert_eq!(
        Device::restore(&latest(&drive, LEGACY_LOCAL_VAULT_ID, NEW), NEW)
            .await
            .store
            .list()
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn racing_password_publications_are_retained_and_require_resolution() {
    let drive = Drive::default();
    let mut first = Device::new(LEGACY_LOCAL_VAULT_ID).await;
    first.sync(&drive).await.unwrap();
    let original = latest(&drive, LEGACY_LOCAL_VAULT_ID, OLD);
    let mut second = Device::restore(&original, OLD).await;
    first.rotate(NEW);
    second.rotate("RacingPassword123!");
    let mut racing = second.export().await;
    shellumi_core::sync::revisions::normalize_records(
        &mut racing,
        &second.key,
        &hex::encode(second.key.expose_bytes()),
    )
    .unwrap();
    racing
        .sign_publication(&second.key, vec![original.publication_id().unwrap()])
        .unwrap();
    drive.0.lock().unwrap().race = Some(racing);
    assert!(matches!(
        first.sync(&drive).await.unwrap(),
        SyncCycleResult::Conflicts(_)
    ));
    assert_eq!(drive.0.lock().unwrap().files.len(), 3);
    let client = GoogleDriveSyncClient::new(drive.clone(), "test");
    assert!(client
        .download_current_vault("token", LEGACY_LOCAL_VAULT_ID, NEW)
        .is_err());
    let SyncCycleResult::Conflicts(views) = first.sync(&drive).await.unwrap() else {
        panic!("racing fork disappeared")
    };
    let local = first.export().await;
    let resolution = CloudConflictResolution {
        context: views[0].revision_context.clone().unwrap(),
        choices: vec![ConflictResolutionChoice::KeepLocal; views.len()],
    };
    let result = execute_cloud_vault_sync_bound(
        &client,
        "token",
        &local,
        &hex::encode(first.key.expose_bytes()),
        &resolution,
    )
    .unwrap();
    first.apply(&result).await;
    assert_eq!(drive.0.lock().unwrap().files.len(), 4);
    assert_eq!(
        Device::restore(&latest(&drive, LEGACY_LOCAL_VAULT_ID, NEW), NEW)
            .await
            .store
            .list()
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn missing_accepted_history_or_incomplete_listing_never_publishes() {
    let drive = Drive::default();
    let mut first = Device::new(LEGACY_LOCAL_VAULT_ID).await;
    first.sync(&drive).await.unwrap();
    let retained = drive.0.lock().unwrap().files.clone();
    drive.0.lock().unwrap().incomplete = true;
    assert!(first.sync(&drive).await.is_err());
    assert_eq!(drive.0.lock().unwrap().files, retained);
    drive.0.lock().unwrap().incomplete = false;
    drive.0.lock().unwrap().files.clear();
    assert!(first.sync(&drive).await.is_err());
    assert!(drive.0.lock().unwrap().files.is_empty());
    assert!(VaultManifest::load_from_path(&first.manifest_path())
        .unwrap()
        .unlock(OLD)
        .is_ok());
}

#[tokio::test]
async fn metadata_and_password_updates_do_not_duplicate_unchanged_snippets_or_tunnels() {
    let drive = Drive::default();
    let mut first = Device::new(LEGACY_LOCAL_VAULT_ID).await;
    first
        .store
        .add_snippet(&shellumi_core::Snippet::new("Command", "uptime", None, &[]).unwrap())
        .await
        .unwrap();
    first
        .store
        .add_tunnel(
            "Tunnel",
            None,
            &shellumi_core::PortForwardRule::dynamic(None, 1080).unwrap(),
            false,
        )
        .await
        .unwrap();
    first.sync(&drive).await.unwrap();
    let snippet_id = first.store.list_snippets().await.unwrap()[0].id;
    let tunnel_id = first.store.list_tunnels().await.unwrap()[0].id;
    first.rotate(NEW);
    first.sync(&drive).await.unwrap();
    first
        .store
        .register_vault_context(
            "Updated metadata",
            &shellumi_core::VaultContext::new(
                first.id.clone(),
                shellumi_core::VaultLocation::Local,
            ),
        )
        .await
        .unwrap();
    first.sync(&drive).await.unwrap();
    let snippets = first.store.list_snippets().await.unwrap();
    let tunnels = first.store.list_tunnels().await.unwrap();
    assert_eq!(snippets.len(), 1);
    assert_eq!(tunnels.len(), 1);
    assert_eq!(snippets[0].id, snippet_id);
    assert_eq!(tunnels[0].id, tunnel_id);
    assert_eq!(
        first.sync(&drive).await.unwrap(),
        SyncCycleResult::AlreadySynchronized
    );
    let second = Device::restore(&latest(&drive, LEGACY_LOCAL_VAULT_ID, NEW), NEW).await;
    assert_eq!(second.store.list_snippets().await.unwrap().len(), 1);
    assert_eq!(second.store.list_tunnels().await.unwrap().len(), 1);
}

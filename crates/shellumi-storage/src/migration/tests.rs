use super::*;
use shellumi_core::{EphemeralCredentialStore, VaultContext, VaultLocation};

struct Fixture {
    _temp: tempfile::TempDir,
    base: PathBuf,
    secrets: EphemeralCredentialStore,
    default_key: VaultKey,
    secondary_key: VaultKey,
    source_bytes: Vec<u8>,
    manifests: Vec<(PathBuf, Vec<u8>)>,
}

impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let secrets = EphemeralCredentialStore::new();
        let default_key = VaultKey::generate().unwrap();
        let secondary_key = VaultKey::generate().unwrap();
        let mut manifests = Vec::new();
        for (id, key) in [
            (LEGACY_LOCAL_VAULT_ID, &default_key),
            ("secondary", &secondary_key),
        ] {
            let vid = VaultId::new(id).unwrap();
            VaultKeyManager::provision_key(&secrets, &vid, key).unwrap();
            let env = key
                .wrap_with_argon2_params(&vid, "StrongPassword123!", 64, 1, 1)
                .unwrap();
            let manifest = VaultManifest::new(id.into(), id.into(), env, None, true);
            let path = temp
                .path()
                .join(format!("vault_manifests/{id}.manifest.json"));
            manifest.save_to_path(&path).unwrap();
            manifests.push((path.clone(), fs::read(path).unwrap()));
        }
        secrets
            .set_secret(
                "org.shellumi.server",
                "server-auth-token",
                "preexisting-token",
            )
            .unwrap();
        let path = temp.path().join("shellumi.db");
        let mut source = HostStore::open(path.to_str().unwrap()).await.unwrap();
        source
            .register_vault_context(
                "secondary",
                &VaultContext::new(VaultId::new("secondary").unwrap(), VaultLocation::Local),
            )
            .await
            .unwrap();
        source.connection.execute("UPDATE vault_contexts SET sync_enabled = 1, active_key_epoch = 7, recovery_envelope = 'old-recovery', dek_envelope = 'old-dek', sync_provider = 'gdrive' WHERE id = 'secondary'", ()).await.unwrap();
        for (id, offset) in [(LEGACY_LOCAL_VAULT_ID, 0i64), ("secondary", 100)] {
            source.connection.execute("INSERT INTO snippets(id,name,command,vault_id,created_at,updated_at) VALUES (?1,'startup','uptime',?2,'2001','2002')", (42 + offset, id)).await.unwrap();
            source.connection.execute("INSERT INTO identities(id,name,username,auth_method,vault_id,created_at,updated_at) VALUES (?1,?2,'user','agent',?3,'2001','2002')", (51 + offset, format!("identity-{id}"), id)).await.unwrap();
            source
                .connection
                .execute(
                    "INSERT INTO groups(name,identity_id,vault_id) VALUES ('group',?1,?2)",
                    (51 + offset, id),
                )
                .await
                .unwrap();
            source.connection.execute("INSERT INTO hosts(id,name,hostname,username,port,group_name,startup_snippet_id,identity_id,vault_id,protocol,protocol_config) VALUES (?1,'host','example.org','user',22,'group',?2,?3,?4,'vnc','{}')", (63 + offset,42 + offset,51 + offset,id)).await.unwrap();
            source.connection.execute("INSERT INTO tunnels(id,host_id,name,forwarding_type,bind_port,vault_id) VALUES (?1,?2,'tunnel','local',8080,?3)", (74 + offset,63 + offset,id)).await.unwrap();
            source.connection.execute("INSERT INTO workspaces(id,name,sessions_json,vault_id,created_at,updated_at) VALUES (?1,'workspace',?2,?3,'2001','2002')", (85 + offset, serde_json::to_string(&vec![shellumi_core::WorkspaceSession::new("session", shellumi_core::WorkspaceSessionKind::Ssh, 0).with_host(63 + offset)]).unwrap(),id)).await.unwrap();
            source.connection.execute("INSERT INTO transfer_history(id,host_id,direction,local_path,remote_path,bytes_transferred,total_bytes,status) VALUES (?1,?2,'upload','local','remote',1,1,'completed')", (96 + offset,63 + offset)).await.unwrap();
            source.connection.execute("INSERT INTO session_recordings(id,session_id,host_id,title,duration_secs,event_count,byte_size,cast_data,recorded_at,vault_id) VALUES (?1,'session',?2,'recording',1.0,1,1,'cast','2001',?3)", (format!("recording-{id}"),63 + offset,id)).await.unwrap();
            source.connection.execute("INSERT INTO vault_mutations(id,vault_id,object_id,entity_type,expected_revision,deleted,payload_json,created_at,idempotency_key,key_epoch,device_counter) VALUES (?1,?2,'object','host',12,0,'{}',123,'idempotency',7,9)", (107 + offset,id)).await.unwrap();
            source.connection.execute("INSERT INTO vault_sync_state(vault_id,server_sequence,last_cursor,last_synced_at,status,active_key_epoch,conflicted_object_id,conflict_details) VALUES (?1,123,'cursor',100,'idle',7,'conflict','details')", (id,)).await.unwrap();
        }
        source.connection.execute("INSERT INTO vault_server_connections(id,base_url,auth_token,refresh_token,device_id,device_signing_key,device_agreement_key,server_instance_id,created_at,updated_at) VALUES ('server','https://example.org','raw-token','env://OLD_REFRESH','device','raw-signing','raw-agreement','instance',12,13)", ()).await.unwrap();
        source.connection.execute("INSERT INTO cloud_providers(id,name,provider_kind,created_at,updated_at) VALUES ('provider','cloud','aws',1,2)", ()).await.unwrap();
        for (id, host) in [("discovered-default", 63), ("discovered-secondary", 163)] {
            source.connection.execute("INSERT INTO discovered_hosts(id,provider_id,external_id,name,hostname,imported_host_id,last_seen_at) VALUES (?1,'provider','external','name','example.org',?2,123)", (id,host)).await.unwrap();
        }
        source.connection.execute("INSERT INTO shared_vaults(id,name,owner_id,created_at,updated_at) VALUES ('shared','shared','owner','1','2')", ()).await.unwrap();
        source.connection.execute("INSERT INTO team_members(id,vault_id,user_id,display_name,public_key,role,enrolled_at,invited_by) VALUES ('member','shared','user','user','key','admin','1','owner')", ()).await.unwrap();
        source.connection.execute("INSERT INTO sharing_audit_log(id,vault_id,action,actor_id,timestamp_utc) VALUES ('audit','shared','action','owner','1')", ()).await.unwrap();
        source.connection.execute("INSERT INTO shared_session_logs(id,vault_id,host_name,author_id,started_at) VALUES ('shared-session','shared','host','owner','1')", ()).await.unwrap();
        source.connection.execute("INSERT INTO sync_snapshots(snapshot_id,vault_json,record_count) VALUES ('snapshot','{}',1)", ()).await.unwrap();
        source.connection.execute("INSERT INTO vault_directories(id,encrypted_data,updated_at) VALUES ('primary_directory','encrypted-directory','2001')", ()).await.unwrap();
        checkpoint(&source).await.unwrap();
        drop(source);
        let source_bytes = fs::read(path).unwrap();
        let base = temp.path().canonicalize().unwrap();
        Self {
            _temp: temp,
            base,
            secrets,
            default_key,
            secondary_key,
            source_bytes,
            manifests,
        }
    }

    fn base(&self) -> &Path {
        &self.base
    }

    async fn migrate(&self, fault: &mut Fault<'_>) -> Result<MigrationReport, StorageError> {
        MigrationEngine::run(
            self.base(),
            Some(&self.default_key),
            None,
            &self.secrets,
            fault,
        )
        .await
    }

    fn originals_intact(&self) {
        assert_eq!(
            fs::read(self.base().join("shellumi.db")).unwrap(),
            self.source_bytes
        );
        for (path, bytes) in &self.manifests {
            assert_eq!(&fs::read(path).unwrap(), bytes);
        }
        assert_eq!(
            self.secrets
                .get_secret("org.shellumi.server", "server-auth-token")
                .unwrap()
                .unwrap()
                .expose_secret(),
            "preexisting-token"
        );
        for (id, key) in [
            (LEGACY_LOCAL_VAULT_ID, &self.default_key),
            ("secondary", &self.secondary_key),
        ] {
            assert_eq!(
                VaultKeyManager::retrieve_key(&self.secrets, &VaultId::new(id).unwrap())
                    .unwrap()
                    .unwrap()
                    .expose_bytes(),
                key.expose_bytes()
            );
        }
    }

    async fn complete_and_verify(&self) {
        self.migrate(&mut |_| Ok(())).await.unwrap();
        self.originals_intact();
        let journal = load(&self.base().join(ROOT)).unwrap();
        assert_eq!(journal.phase, Phase::Complete);
        assert!(!MigrationEngine::is_migration_needed(self.base()));
        let source = HostStore::open(
            self.base()
                .join(ROOT)
                .join("source/source.db")
                .to_str()
                .unwrap(),
        )
        .await
        .unwrap();
        for (index, vault) in journal.vaults.iter().enumerate() {
            let key = if vault.id == LEGACY_LOCAL_VAULT_ID {
                &self.default_key
            } else {
                &self.secondary_key
            };
            let mut target = HostStore::open_with_key(
                destination(self.base(), vault, false).to_str().unwrap(),
                Some(key),
            )
            .await
            .unwrap();
            target.set_vault_id(VaultId::new(&vault.id).unwrap());
            let hosts = target.list().await.unwrap();
            assert_eq!(
                hosts[0].startup_snippet_id,
                Some(if vault.id == LEGACY_LOCAL_VAULT_ID {
                    42
                } else {
                    142
                })
            );
            assert_eq!(
                hosts[0].identity_id,
                Some(if vault.id == LEGACY_LOCAL_VAULT_ID {
                    51
                } else {
                    151
                })
            );
            assert_eq!(
                target.list_workspaces().await.unwrap()[0].sessions[0].host_id,
                Some(hosts[0].id)
            );
            assert_eq!(target.list_identities().await.unwrap().len(), 1);
            verify_tables(&source, &target, &journal, index, &self.secrets)
                .await
                .unwrap();
            assert_eq!(
                read_rows(&target, "hosts", &vault.id, true)
                    .await
                    .unwrap()
                    .values
                    .len(),
                1
            );
        }
    }
}

#[tokio::test]
async fn preserves_every_table_relationship_and_server_metadata() {
    let fixture = Fixture::new().await;
    fixture.complete_and_verify().await;
}

#[tokio::test]
async fn resumes_every_critical_checkpoint_without_rebuilding_verified_vaults() {
    let fixture = Fixture::new().await;
    let mut events = Vec::new();
    fixture
        .migrate(&mut |event| {
            events.push(event.to_owned());
            Ok(())
        })
        .await
        .unwrap();
    assert!(events.contains(&"completion_marker".into()));
    for event in events {
        let fixture = Fixture::new().await;
        let error = fixture
            .migrate(&mut |point| {
                if point == event {
                    Err(StorageError::Serialization("injected crash".into()))
                } else {
                    Ok(())
                }
            })
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("injected crash"),
            "{event}: {error}"
        );
        fixture.originals_intact();
        let before = load(&fixture.base().join(ROOT)).unwrap();
        let witnesses: Vec<_> = before
            .vaults
            .iter()
            .enumerate()
            .filter(|(_, v)| v.staged)
            .map(|(i, _)| {
                let path = staged_db(&fixture.base().join(ROOT), i);
                (i, File::open(&path).unwrap(), digest(&path).unwrap())
            })
            .collect();
        fixture.complete_and_verify().await;
        for (index, file, hash) in witnesses {
            let path = staged_db(&fixture.base().join(ROOT), index);
            assert_eq!(
                digest(&path).unwrap(),
                hash,
                "verified vault rebuilt after {event}"
            );
            // Retaining the open witness also detects inode replacement on Unix.
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                assert_eq!(
                    file.metadata().unwrap().ino(),
                    fs::metadata(&path).unwrap().ino(),
                    "{event}"
                );
            }
            #[cfg(not(unix))]
            drop(file);
        }
    }
}

#[tokio::test]
async fn rolls_back_only_owned_activation_when_destination_collides() {
    let fixture = Fixture::new().await;
    fixture
        .migrate(&mut |event| {
            if event == "activated_file:0" {
                Err(StorageError::Serialization("crash".into()))
            } else {
                Ok(())
            }
        })
        .await
        .unwrap_err();
    let existing = fixture.base().join("vaults/secondary.db");
    let mut unrelated =
        HostStore::open_with_key(existing.to_str().unwrap(), Some(&fixture.secondary_key))
            .await
            .unwrap();
    unrelated.set_vault_id(VaultId::new("secondary").unwrap());
    unrelated
        .add(
            "preexisting host",
            &shellumi_core::SshDestination::new("existing.example.org", "user", 22).unwrap(),
            None,
            &[],
            None,
            None,
        )
        .await
        .unwrap();
    checkpoint(&unrelated).await.unwrap();
    drop(unrelated);
    let original_bytes = fs::read(&existing).unwrap();
    assert!(fixture.migrate(&mut |_| Ok(())).await.is_err());
    assert_eq!(fs::read(&existing).unwrap(), original_bytes);
    let mut unrelated =
        HostStore::open_with_key(existing.to_str().unwrap(), Some(&fixture.secondary_key))
            .await
            .unwrap();
    unrelated.set_vault_id(VaultId::new("secondary").unwrap());
    assert_eq!(unrelated.list().await.unwrap()[0].name, "preexisting host");
    assert!(!fixture.base().join("vaults/default.db").exists());
    assert!(!fixture.base().join(MARKER).exists());
    assert_eq!(
        load(&fixture.base().join(ROOT)).unwrap().phase,
        Phase::RolledBack
    );
    fixture.originals_intact();
}

#[tokio::test]
async fn rollback_retains_changed_artifact_for_inspection() {
    let fixture = Fixture::new().await;
    fixture
        .migrate(&mut |event| {
            if event == "activated_file:0" {
                Err(StorageError::Serialization("crash".into()))
            } else {
                Ok(())
            }
        })
        .await
        .unwrap_err();
    fs::write(staged_db(&fixture.base().join(ROOT), 0), b"corruption").unwrap();
    assert!(fixture.migrate(&mut |_| Ok(())).await.is_err());
    assert_eq!(
        fs::read(fixture.base().join("vaults/default.db")).unwrap(),
        b"corruption"
    );
    assert_eq!(
        load(&fixture.base().join(ROOT)).unwrap().phase,
        Phase::RollingBack
    );
    assert!(!fixture.base().join(MARKER).exists());
    fixture.originals_intact();
}

#[tokio::test]
async fn rejects_legacy_journal_and_forged_marker_without_deleting_data() {
    let fixture = Fixture::new().await;
    let root = fixture.base().join(ROOT);
    fs::create_dir_all(&root).unwrap();
    let bytes = b"{\"completed_stages\":[\"data_integrity_verified\"]}";
    fs::write(root.join(JOURNAL), bytes).unwrap();
    fs::write(fixture.base().join(MARKER), b"verified\n").unwrap();
    assert!(MigrationEngine::is_migration_needed(fixture.base()));
    assert!(fixture.migrate(&mut |_| Ok(())).await.is_err());
    assert_eq!(fs::read(root.join(JOURNAL)).unwrap(), bytes);
    fixture.originals_intact();
}

#[tokio::test]
async fn detects_source_change_during_backup() {
    let fixture = Fixture::new().await;
    fixture
        .migrate(&mut |event| {
            if event == "journal_created" {
                Err(StorageError::Serialization("crash".into()))
            } else {
                Ok(())
            }
        })
        .await
        .unwrap_err();
    let path = fixture.base().join("shellumi.db");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(0);
    fs::write(&path, &bytes).unwrap();
    assert!(fixture.migrate(&mut |_| Ok(())).await.is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(!fixture.base().join(MARKER).exists());
}

#[tokio::test]
async fn resumes_new_manifest_activation_and_key_finalization() {
    let points = [
        "key_stored:0",
        "manifest_written:0",
        "key:0",
        "manifest_written:1",
        "staged:1",
        "verified",
        "activation_intent:1",
        "activation_link:1",
        "activated_file:1",
        "activation_link:3",
        "activated",
        "final_key:0",
        "final_key:1",
        "finalized",
        "completion_marker",
    ];
    for point in points {
        let fixture = Fixture::new().await;
        for (path, _) in &fixture.manifests {
            fs::remove_file(path).unwrap();
        }
        for id in [LEGACY_LOCAL_VAULT_ID, "secondary"] {
            VaultKeyManager::delete_key(&fixture.secrets, &VaultId::new(id).unwrap()).unwrap();
        }
        let credentials = MigrationCredentials {
            default_password: Some("StrongPassword123!".into()),
            secondary_passwords: [("secondary".into(), "SecondaryPassword123!".into())].into(),
            ..Default::default()
        };
        MigrationEngine::run(
            fixture.base(),
            Some(&fixture.default_key),
            Some(&credentials),
            &fixture.secrets,
            &mut |event| {
                if event == point {
                    Err(StorageError::Serialization("injected crash".into()))
                } else {
                    Ok(())
                }
            },
        )
        .await
        .unwrap_err();
        MigrationEngine::migrate_legacy_database_with_credentials(
            fixture.base(),
            Some(&fixture.default_key),
            Some(&credentials),
            &fixture.secrets,
        )
        .await
        .unwrap();
        assert_eq!(
            fs::read(fixture.base().join("shellumi.db")).unwrap(),
            fixture.source_bytes
        );
        for id in [LEGACY_LOCAL_VAULT_ID, "secondary"] {
            let manifest = VaultManifest::load_from_path(
                &fixture
                    .base()
                    .join(format!("vault_manifests/{id}.manifest.json")),
            )
            .unwrap();
            let pwd = password(Some(&credentials), id).unwrap();
            let unwrapped = manifest.unlock(pwd).unwrap();
            let remembered =
                VaultKeyManager::retrieve_key(&fixture.secrets, &VaultId::new(id).unwrap())
                    .unwrap()
                    .unwrap();
            assert_eq!(
                unwrapped.expose_bytes(),
                remembered.expose_bytes(),
                "{point}"
            );
            let target = HostStore::open_with_key(
                fixture
                    .base()
                    .join("vaults")
                    .join(db_name(id))
                    .to_str()
                    .unwrap(),
                Some(&unwrapped),
            )
            .await
            .unwrap();
            assert_eq!(
                read_rows(&target, "hosts", id, true)
                    .await
                    .unwrap()
                    .values
                    .len(),
                1
            );
        }
    }
}

#[tokio::test]
async fn resumes_interrupted_rollback_and_preserves_replacement_file() {
    for point in ["rollback_started", "rollback_file:0", "rolled_back"] {
        let fixture = Fixture::new().await;
        fixture
            .migrate(&mut |event| {
                if event == "activated_file:0" {
                    Err(StorageError::Serialization("crash".into()))
                } else {
                    Ok(())
                }
            })
            .await
            .unwrap_err();
        let other = fixture.base().join("vaults/secondary.db");
        fs::write(&other, b"unrelated vault").unwrap();
        fixture
            .migrate(&mut |event| {
                if event == point {
                    Err(StorageError::Serialization("rollback crash".into()))
                } else {
                    Ok(())
                }
            })
            .await
            .unwrap_err();
        fixture.migrate(&mut |_| Ok(())).await.unwrap_err();
        assert!(!fixture.base().join("vaults/default.db").exists());
        assert_eq!(fs::read(&other).unwrap(), b"unrelated vault");
        assert_eq!(
            load(&fixture.base().join(ROOT)).unwrap().phase,
            Phase::RolledBack
        );
        fixture.originals_intact();
    }
}

#[tokio::test]
async fn copies_committed_wal_without_modifying_source() {
    let fixture = Fixture::new().await;
    let path = fixture.base().join("shellumi.db");
    let source = HostStore::open(path.to_str().unwrap()).await.unwrap();
    source.connection.execute("INSERT INTO hosts(id,name,hostname,username,port) VALUES (1001,'wal-host','wal.example.org','user',22)",()).await.unwrap();
    let before = source_fingerprints(fixture.base()).unwrap();
    assert!(before.contains_key("-wal"));
    fixture.migrate(&mut |_| Ok(())).await.unwrap();
    assert_eq!(source_fingerprints(fixture.base()).unwrap(), before);
    let target = HostStore::open_with_key(
        fixture.base().join("vaults/default.db").to_str().unwrap(),
        Some(&fixture.default_key),
    )
    .await
    .unwrap();
    assert!(target.list().await.unwrap().iter().any(|h| h.id == 1001));
}

#[tokio::test]
async fn rejects_changed_source_after_staging_and_rolls_back_activation() {
    let fixture = Fixture::new().await;
    fixture
        .migrate(&mut |event| {
            if event == "activated_file:0" {
                Err(StorageError::Serialization("crash".into()))
            } else {
                Ok(())
            }
        })
        .await
        .unwrap_err();
    let source = HostStore::open(fixture.base().join("shellumi.db").to_str().unwrap())
        .await
        .unwrap();
    source
        .connection
        .execute(
            "UPDATE hosts SET name = 'new source record' WHERE id = 63",
            (),
        )
        .await
        .unwrap();
    checkpoint(&source).await.unwrap();
    drop(source);
    assert!(fixture.migrate(&mut |_| Ok(())).await.is_err());
    assert!(!fixture.base().join("vaults/default.db").exists());
    assert!(!fixture.base().join(MARKER).exists());
}

#[tokio::test]
async fn unsafe_vault_id_and_concurrent_migrator_are_rejected() {
    let fixture = Fixture::new().await;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(fixture.base().join(".migration.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(fixture
        .migrate(&mut |_| Ok(()))
        .await
        .unwrap_err()
        .to_string()
        .contains("Another migration"));
    drop(lock);
    let source = HostStore::open(fixture.base().join("shellumi.db").to_str().unwrap())
        .await
        .unwrap();
    source
        .connection
        .execute(
            "INSERT INTO vault_contexts(id,name) VALUES ('../escape','unsafe')",
            (),
        )
        .await
        .unwrap();
    checkpoint(&source).await.unwrap();
    drop(source);
    assert!(fixture.migrate(&mut |_| Ok(())).await.is_err());
    assert!(!fixture.base().join(MARKER).exists());
}

#[tokio::test]
async fn resumes_encrypted_legacy_source_with_retained_recovery_key() {
    let temp = tempfile::tempdir().unwrap();
    let key = VaultKey::generate().unwrap();
    let path = temp.path().join("shellumi.db");
    let source = HostStore::open_with_key(path.to_str().unwrap(), Some(&key))
        .await
        .unwrap();
    source.connection.execute("INSERT INTO hosts(id,name,hostname,username,port) VALUES (99,'encrypted','example.org','user',22)",()).await.unwrap();
    checkpoint(&source).await.unwrap();
    drop(source);
    let original = fs::read(&path).unwrap();
    let secrets = EphemeralCredentialStore::new();
    MigrationEngine::run(temp.path(), Some(&key), None, &secrets, &mut |event| {
        if event == "planned" {
            Err(StorageError::Serialization("crash".into()))
        } else {
            Ok(())
        }
    })
    .await
    .unwrap_err();
    MigrationEngine::migrate_legacy_database_if_needed(temp.path(), None, &secrets)
        .await
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
    let target = HostStore::open_with_key(
        temp.path().join("vaults/default.db").to_str().unwrap(),
        Some(&key),
    )
    .await
    .unwrap();
    assert_eq!(target.list().await.unwrap()[0].id, 99);
}

#[tokio::test]
async fn rejects_future_schema_without_changing_original() {
    let fixture = Fixture::new().await;
    let path = fixture.base().join("shellumi.db");
    let source = HostStore::open(path.to_str().unwrap()).await.unwrap();
    source
        .connection
        .execute(
            format!("PRAGMA user_version = {}", crate::SCHEMA_VERSION + 1),
            (),
        )
        .await
        .unwrap();
    checkpoint(&source).await.unwrap();
    drop(source);
    let before = fs::read(&path).unwrap();
    assert!(fixture.migrate(&mut |_| Ok(())).await.is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!fixture.base().join(MARKER).exists());
}

#[tokio::test]
async fn resumes_before_first_journal_publish_without_removing_unknown_data() {
    for partial in [false, true] {
        let fixture = Fixture::new().await;
        let root = fixture.base().join(ROOT);
        fs::create_dir_all(&root).unwrap();
        if partial {
            fs::write(root.join(".write-interrupted"), b"incomplete journal").unwrap();
        }
        fixture.complete_and_verify().await;
        if partial {
            assert_eq!(
                fs::read(root.join(".write-interrupted")).unwrap(),
                b"incomplete journal"
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn refuses_symlinked_migration_artifacts() {
    let fixture = Fixture::new().await;
    fixture
        .migrate(&mut |event| {
            if event == "planned" {
                Err(StorageError::Serialization("crash".into()))
            } else {
                Ok(())
            }
        })
        .await
        .unwrap_err();
    let root = fixture.base().join(ROOT);
    let original = root.join("source");
    let moved = root.join("retained-source");
    fs::rename(&original, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &original).unwrap();
    assert!(fixture.migrate(&mut |_| Ok(())).await.is_err());
    assert!(!fixture.base().join(MARKER).exists());
    fixture.originals_intact();
}

#[tokio::test]
async fn preserves_autoincrement_high_watermark_after_deleted_records() {
    let mut fixture = Fixture::new().await;
    let path = fixture.base().join("shellumi.db");
    let source = HostStore::open(path.to_str().unwrap()).await.unwrap();
    source.connection.execute("INSERT INTO hosts(id,name,hostname,username,port) VALUES (9001,'removed','example.org','user',22)",()).await.unwrap();
    source
        .connection
        .execute("DELETE FROM hosts WHERE id = 9001", ())
        .await
        .unwrap();
    checkpoint(&source).await.unwrap();
    drop(source);
    fixture.source_bytes = fs::read(path).unwrap();
    fixture.complete_and_verify().await;
    let path = fixture.base().join("vaults/default.db");
    let mut target = HostStore::open_with_key(path.to_str().unwrap(), Some(&fixture.default_key))
        .await
        .unwrap();
    let id = target
        .add(
            "new",
            &shellumi_core::SshDestination::new("example.org", "user", 22).unwrap(),
            None,
            &[],
            None,
            None,
        )
        .await
        .unwrap();
    assert!(id > 9001, "migration reused a historical record ID: {id}");
}

#[tokio::test]
async fn startup_resumes_receipt_publication_before_declaring_completion() {
    let fixture = Fixture::new().await;
    fixture
        .migrate(&mut |event| {
            if event == "completion_marker" {
                Err(StorageError::Serialization("crash".into()))
            } else {
                Ok(())
            }
        })
        .await
        .unwrap_err();
    assert!(fixture.base().join(MARKER).exists());
    assert!(MigrationEngine::is_migration_needed(fixture.base()));
    fixture.complete_and_verify().await;
}

#[tokio::test]
async fn rollback_preserves_records_written_to_a_partial_activation() {
    let fixture = Fixture::new().await;
    fixture
        .migrate(&mut |event| {
            if event == "activated_file:0" {
                Err(StorageError::Serialization("crash".into()))
            } else {
                Ok(())
            }
        })
        .await
        .unwrap_err();
    let path = fixture.base().join("vaults/default.db");
    let target = HostStore::open_with_key(path.to_str().unwrap(), Some(&fixture.default_key))
        .await
        .unwrap();
    target.connection.execute("INSERT INTO hosts(id,name,hostname,username,port) VALUES (9999,'external record','external.example.org','user',22)",()).await.unwrap();
    checkpoint(&target).await.unwrap();
    drop(target);
    assert!(fixture.migrate(&mut |_| Ok(())).await.is_err());
    assert!(!fixture.base().join(MARKER).exists());
    let target = HostStore::open_with_key(path.to_str().unwrap(), Some(&fixture.default_key))
        .await
        .unwrap();
    assert!(target
        .list()
        .await
        .unwrap()
        .iter()
        .any(|host| host.id == 9999));
    fixture.originals_intact();
}

#[tokio::test]
async fn enrollment_journal_upgrade_preserves_legacy_rows_and_scoped_intents() {
    for old_schema in [27, 28] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.db");
        let path = path.to_str().unwrap();
        let mut store = crate::HostStore::open(path).await.unwrap();
        store
            .save_enrollment_operation_progress(
                "legacy-operation",
                "vault-a",
                "account-a",
                "member-a",
                "InvitationAccepted",
                Some("device-a"),
                Some("pending"),
                "1000",
            )
            .await
            .unwrap();
        if old_schema == 28 {
            store.connection.execute(
                "UPDATE enrollment_operations SET registration_intent = ?1 WHERE operation_id = 'legacy-operation'",
                (r#"{"device":"old"}"#,)
            ).await.unwrap();
        } else {
            store
                .connection
                .execute(
                    "ALTER TABLE enrollment_operations DROP COLUMN registration_intent",
                    (),
                )
                .await
                .unwrap();
        }
        store
            .connection
            .execute(format!("PRAGMA user_version = {old_schema}"), ())
            .await
            .unwrap();
        drop(store);

        let reopened = crate::HostStore::open(path).await.unwrap();
        let records = reopened
            .list_incomplete_enrollment_operations("vault-a")
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].operation_id, "legacy-operation");
        assert_eq!(records[0].device_id.as_deref(), Some("device-a"));
        assert_eq!(records[0].error_message.as_deref(), Some("pending"));
        assert_eq!(
            records[0].registration_intent.as_deref(),
            (old_schema == 28).then_some(r#"{"device":"old"}"#)
        );
        for operation_id in ["prepared-one", "prepared-two"] {
            let count = reopened.connection.execute(
                "INSERT INTO enrollment_operations (operation_id, vault_id, user_id, member_id, stage, updated_at)
                 VALUES (?1, 'vault-a', 'account-a', 'member-a', 'RegistrationPrepared', '1001')
                 ON CONFLICT DO NOTHING",
                (operation_id,),
            ).await.unwrap();
            assert_eq!(count, u64::from(operation_id == "prepared-one"));
        }
    }
}

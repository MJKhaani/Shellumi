//! Journaled, identity-preserving migration of legacy storage.
//!
//! The source and its backup are never changed or removed. Verified staging files
//! are retained as ownership witnesses: create-only hard links publish each file,
//! and the completion receipt is published only after verification and key setup.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use shellumi_core::{CredentialStore, VaultId, VaultKey, VaultKeyManager, VaultManifest};
use turso::Value;
use uuid::Uuid;

use crate::{HostStore, StorageError, LEGACY_LOCAL_VAULT_ID};

const JOURNAL_VERSION: u32 = 2;
const ROOT: &str = "vaults/.migration_staging";
const MARKER: &str = "vaults/.migration_complete";
const JOURNAL: &str = "migration_journal.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationReport {
    pub legacy_db_found: bool,
    pub backup_path: Option<PathBuf>,
    pub vaults_migrated: usize,
    pub default_records_migrated: usize,
    pub secondary_records_migrated: usize,
    pub server_credentials_migrated: usize,
}

#[derive(Clone, Debug, Default)]
pub struct MigrationCredentials {
    pub default_password: Option<String>,
    pub default_recovery_code: Option<String>,
    pub secondary_passwords: std::collections::HashMap<String, String>,
    pub secondary_recovery_codes: std::collections::HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Phase {
    Created,
    BackedUp,
    Planned,
    Verified,
    Activating,
    Activated,
    Finalized,
    Complete,
    RollingBack,
    RolledBack,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct VaultPlan {
    id: String,
    name: String,
    staged: bool,
    db_digest: Option<String>,
    manifest_digest: Option<String>,
    existing_manifest_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum LinkState {
    Pending,
    Intent,
    Linked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Activation {
    vault_index: usize,
    manifest: bool,
    state: LinkState,
}

/// The on-disk journal contains paths derived from validated vault identities,
/// ciphertext hashes, and progress only. It never contains credentials or DEKs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MigrationJournal {
    version: u32,
    transaction: String,
    phase: Phase,
    source_files: BTreeMap<String, String>,
    source_digest: Option<String>,
    vaults: Vec<VaultPlan>,
    activation: Vec<Activation>,
    report: MigrationReport,
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    version: u32,
    transaction: String,
}

pub struct MigrationEngine;

type Fault<'a> = dyn FnMut(&str) -> Result<(), StorageError> + Send + 'a;

fn failure(message: impl Into<String>) -> StorageError {
    let message = message.into();
    tracing::error!(reason = %message, "Legacy migration stopped; source and recovery artifacts preserved");
    StorageError::Serialization(message)
}

fn io<T>(result: std::io::Result<T>, operation: &str) -> Result<T, StorageError> {
    result.map_err(|e| failure(format!("{operation}: {e}")))
}

fn safe_id(id: &str) -> Result<(), StorageError> {
    if id.is_empty()
        || id == "."
        || id == ".."
        || id.eq_ignore_ascii_case("default")
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(failure("Migration requires vault IDs containing only letters, digits, '-' or '_' and distinct database filenames"));
    }
    Ok(())
}

fn no_symlink(path: &Path) -> Result<(), StorageError> {
    for component in path.ancestors() {
        match fs::symlink_metadata(component) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(failure(format!(
                    "Refusing migration through symbolic link {}",
                    component.display()
                )))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(failure(format!(
                    "Cannot inspect {}: {e}",
                    component.display()
                )))
            }
        }
    }
    Ok(())
}

fn directory(path: &Path) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            directory(parent)?;
        }
    }
    no_symlink(path)?;
    if !path.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        io(builder.create(path), "Create private migration directory")?;
        if let Some(parent) = path.parent() {
            sync_dir(parent)?;
        }
    }
    Ok(())
}

fn sync_dir(path: &Path) -> Result<(), StorageError> {
    #[cfg(unix)]
    io(
        File::open(path).and_then(|f| f.sync_all()),
        "Flush migration directory",
    )?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<(), StorageError> {
    no_symlink(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| failure("Migration file has no parent"))?;
    directory(parent)?;
    let temp = parent.join(format!(".write-{}", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = io(options.open(&temp), "Create migration temporary file")?;
    io(
        file.write_all(data).and_then(|()| file.sync_all()),
        "Flush migration file",
    )?;
    drop(file);
    io(fs::rename(&temp, path), "Publish migration file")?;
    sync_dir(parent)
}

fn digest(path: &Path) -> Result<String, StorageError> {
    no_symlink(path)?;
    let mut file = io(File::open(path), "Read migration artifact")?;
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0u8; 65536];
    loop {
        let count = io(file.read(&mut buffer), "Hash migration artifact")?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hex::encode(hash.finish()))
}

fn source_fingerprints(base: &Path) -> Result<BTreeMap<String, String>, StorageError> {
    let mut files = BTreeMap::new();
    for suffix in ["", "-wal", "-shm"] {
        let path = base.join(format!("shellumi.db{suffix}"));
        no_symlink(&path)?;
        if path.exists() {
            files.insert(suffix.to_string(), digest(&path)?);
        }
    }
    if !files.contains_key("") {
        return Err(failure(
            "Legacy database is missing; restore the source before resuming migration",
        ));
    }
    Ok(files)
}

fn db_name(id: &str) -> String {
    if id == LEGACY_LOCAL_VAULT_ID {
        "default.db".into()
    } else {
        format!("{id}.db")
    }
}

fn staged_db(root: &Path, index: usize) -> PathBuf {
    root.join(format!("work/{index}/vault.db"))
}
fn staged_manifest(root: &Path, index: usize) -> PathBuf {
    root.join(format!("manifests/{index}.json"))
}
fn destination(base: &Path, vault: &VaultPlan, manifest: bool) -> PathBuf {
    if manifest {
        base.join("vault_manifests")
            .join(format!("{}.manifest.json", vault.id))
    } else {
        base.join("vaults").join(db_name(&vault.id))
    }
}

fn save(root: &Path, journal: &MigrationJournal) -> Result<(), StorageError> {
    let bytes = serde_json::to_vec_pretty(journal)
        .map_err(|e| failure(format!("Serialize migration journal: {e}")))?;
    atomic_write(&root.join(JOURNAL), &bytes)?;
    tracing::debug!(phase = ?journal.phase, "Migration checkpoint persisted");
    Ok(())
}

fn validate(journal: &MigrationJournal) -> Result<(), StorageError> {
    if journal.version != JOURNAL_VERSION || Uuid::parse_str(&journal.transaction).is_err() {
        return Err(failure("Unsupported migration journal; preserve staging and restore from the original database or backup"));
    }
    if journal
        .source_files
        .keys()
        .any(|suffix| !["", "-wal", "-shm"].contains(&suffix.as_str()))
        || !journal.source_files.contains_key("")
    {
        return Err(failure("Migration journal has invalid backup paths"));
    }
    let mut filenames = BTreeSet::new();
    for vault in &journal.vaults {
        safe_id(&vault.id)?;
        if !filenames.insert(db_name(&vault.id).to_ascii_lowercase()) {
            return Err(failure(
                "Migration journal contains colliding vault filenames",
            ));
        }
    }
    if journal
        .activation
        .iter()
        .any(|entry| entry.vault_index >= journal.vaults.len())
    {
        return Err(failure("Migration journal has an invalid activation entry"));
    }
    if !matches!(journal.phase, Phase::Created | Phase::BackedUp) && journal.vaults.is_empty() {
        return Err(failure("Migration journal has no vault plan"));
    }
    if matches!(
        journal.phase,
        Phase::Verified | Phase::Activating | Phase::Activated | Phase::Finalized | Phase::Complete
    ) {
        if journal
            .vaults
            .iter()
            .any(|v| !v.staged || v.db_digest.is_none())
        {
            return Err(failure(
                "Migration journal claims verification without sealed vaults",
            ));
        }
        let expected: BTreeSet<_> = journal
            .vaults
            .iter()
            .enumerate()
            .flat_map(|(i, v)| {
                let mut files = vec![(i, false)];
                if v.manifest_digest.is_some() {
                    files.push((i, true));
                }
                files
            })
            .collect();
        let actual: BTreeSet<_> = journal
            .activation
            .iter()
            .map(|a| (a.vault_index, a.manifest))
            .collect();
        if expected != actual || actual.len() != journal.activation.len() {
            return Err(failure(
                "Migration journal has an incomplete activation plan",
            ));
        }
    }
    Ok(())
}

fn load(root: &Path) -> Result<MigrationJournal, StorageError> {
    no_symlink(&root.join(JOURNAL))?;
    let data = io(
        fs::read(root.join(JOURNAL)),
        "Read migration journal; preserve staging if recovery is required",
    )?;
    let journal = serde_json::from_slice(&data).map_err(|_| failure("Invalid or legacy migration journal; ownership cannot be established safely; original and staging preserved"))?;
    validate(&journal)?;
    Ok(journal)
}

fn key_service(journal: &MigrationJournal) -> String {
    format!("org.shellumi.migration.{}", journal.transaction)
}

fn store_owned_secret(
    store: &dyn CredentialStore,
    service: &str,
    account: &str,
    value: &str,
) -> Result<(), StorageError> {
    let previous = store
        .get_secret(service, account)
        .map_err(|e| failure(format!("Read migration credential: {e}")))?;
    if let Some(previous) = previous {
        if previous.expose_secret() != value {
            return Err(failure(
                "Migration credential collision; existing secret was preserved",
            ));
        }
    } else {
        store
            .set_secret(service, account, value)
            .map_err(|e| failure(format!("Persist migration credential: {e}")))?;
    }
    let saved = store
        .get_secret(service, account)
        .map_err(|e| failure(format!("Verify migration credential: {e}")))?
        .ok_or_else(|| failure("Credential store did not retain migration credential"))?;
    if saved.expose_secret() != value {
        return Err(failure(
            "Credential store failed migration read-back verification",
        ));
    }
    Ok(())
}

fn password<'a>(credentials: Option<&'a MigrationCredentials>, id: &str) -> Option<&'a str> {
    credentials.and_then(|c| {
        if id == LEGACY_LOCAL_VAULT_ID {
            c.default_password.as_deref()
        } else {
            c.secondary_passwords.get(id).map(String::as_str)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn vault_key(
    base: &Path,
    root: &Path,
    journal: &MigrationJournal,
    index: usize,
    default_dek: Option<&VaultKey>,
    credentials: Option<&MigrationCredentials>,
    store: &dyn CredentialStore,
) -> Result<VaultKey, StorageError> {
    let vault = &journal.vaults[index];
    let vid = VaultId::new(&vault.id).map_err(|e| failure(e.to_string()))?;
    let account = format!("key-{index}");
    let service = key_service(journal);
    if let Some(secret) = store
        .get_secret(&service, &account)
        .map_err(|e| failure(format!("Read staged vault key: {e}")))?
    {
        let data = STANDARD
            .decode(secret.expose_secret())
            .map_err(|_| failure("Staged vault key is corrupt"))?;
        let bytes: [u8; 32] = data
            .try_into()
            .map_err(|_| failure("Staged vault key has invalid length"))?;
        return Ok(VaultKey::from_bytes(bytes));
    }
    let supplied = if vault.id == LEGACY_LOCAL_VAULT_ID {
        if let Some(key) = default_dek {
            Some(key.clone())
        } else if let Some(secret) = store
            .get_secret(&service, "source-key")
            .map_err(|e| failure(format!("Read original encrypted source key: {e}")))?
        {
            let bytes: [u8; 32] = STANDARD
                .decode(secret.expose_secret())
                .map_err(|_| failure("Corrupt original encrypted source key"))?
                .try_into()
                .map_err(|_| failure("Invalid original encrypted source key length"))?;
            Some(VaultKey::from_bytes(bytes))
        } else {
            None
        }
    } else {
        None
    };
    let remembered = VaultKeyManager::retrieve_key(store, &vid)
        .map_err(|e| failure(format!("Read original vault key: {e}")))?;
    if let (Some(a), Some(b)) = (&supplied, &remembered) {
        if a.expose_bytes() != b.expose_bytes() {
            return Err(failure(
                "Supplied and remembered vault keys disagree; original key preserved",
            ));
        }
    }
    let manifest_path = if staged_manifest(root, index).exists() {
        staged_manifest(root, index)
    } else {
        destination(base, vault, true)
    };
    let manifest = if manifest_path.exists() {
        no_symlink(&manifest_path)?;
        Some(
            VaultManifest::load_from_path(&manifest_path)
                .map_err(|e| failure(format!("Read original/staged manifest: {e}")))?,
        )
    } else {
        None
    };
    if manifest.as_ref().is_some_and(|m| m.vault_id != vault.id) {
        return Err(failure(
            "Manifest vault identity does not match migration plan",
        ));
    }
    let recovered =
        if let (Some(manifest), Some(pwd)) = (&manifest, password(credentials, &vault.id)) {
            Some(manifest.unlock(pwd).map_err(|_| {
                failure(format!(
                    "MigrationRequired: password cannot unlock vault '{}'",
                    vault.id
                ))
            })?)
        } else {
            None
        };
    let key = supplied.or(remembered).or_else(|| recovered.clone());
    let key = match key {
        Some(key) => {
            if recovered
                .as_ref()
                .is_some_and(|other| key.expose_bytes() != other.expose_bytes())
            {
                return Err(failure("Manifest and remembered vault key disagree"));
            }
            key
        }
        None if manifest.is_some() => {
            return Err(failure(format!(
                "MigrationRequired: unlock existing vault '{}' to resume",
                vault.id
            )))
        }
        None => {
            let pwd = password(credentials, &vault.id).ok_or_else(|| {
                failure(format!(
                    "MigrationRequired: vault '{}' requires an explicit user password",
                    vault.id
                ))
            })?;
            shellumi_core::validate_vault_password(pwd).map_err(|e| {
                failure(format!(
                    "MigrationRequired: password policy for '{}': {e}",
                    vault.id
                ))
            })?;
            VaultKey::generate().map_err(|e| failure(e.to_string()))?
        }
    };
    store_owned_secret(
        store,
        &service,
        &account,
        &STANDARD.encode(key.expose_bytes()),
    )?;
    Ok(key)
}

fn ensure_manifest(
    root: &Path,
    journal: &MigrationJournal,
    index: usize,
    key: &VaultKey,
    credentials: Option<&MigrationCredentials>,
) -> Result<(), StorageError> {
    let vault = &journal.vaults[index];
    if vault.existing_manifest_digest.is_some() {
        return Ok(());
    }
    if staged_manifest(root, index).exists() {
        if vault.manifest_digest.is_none() {
            let manifest = VaultManifest::load_from_path(&staged_manifest(root, index))
                .map_err(|e| failure(format!("Read unsealed staged manifest: {e}")))?;
            let pwd = password(credentials, &vault.id).ok_or_else(|| failure("MigrationRequired: provide the vault password to verify an interrupted manifest write"))?;
            let recovered = manifest.unlock(pwd).map_err(|_| {
                failure("Interrupted staged manifest cannot be unlocked; original preserved")
            })?;
            if manifest.vault_id != vault.id || recovered.expose_bytes() != key.expose_bytes() {
                return Err(failure(
                    "Interrupted staged manifest key does not match migration key",
                ));
            }
        }
        return Ok(());
    }
    if let Some(pwd) = password(credentials, &vault.id) {
        shellumi_core::validate_vault_password(pwd).map_err(|e| {
            failure(format!(
                "MigrationRequired: password policy for '{}': {e}",
                vault.id
            ))
        })?;
        let vid = VaultId::new(&vault.id).map_err(|e| failure(e.to_string()))?;
        let primary = key
            .wrap_with_password(&vid, pwd)
            .map_err(|e| failure(e.to_string()))?;
        let recovery = credentials
            .and_then(|c| {
                if vault.id == LEGACY_LOCAL_VAULT_ID {
                    c.default_recovery_code.as_deref()
                } else {
                    c.secondary_recovery_codes
                        .get(&vault.id)
                        .map(String::as_str)
                }
            })
            .map(|pwd| key.wrap_with_password(&vid, pwd))
            .transpose()
            .map_err(|e| failure(e.to_string()))?;
        let manifest = VaultManifest::new(
            vault.id.clone(),
            vault.name.clone(),
            primary,
            recovery,
            true,
        );
        atomic_write(
            &staged_manifest(root, index),
            manifest
                .to_json()
                .map_err(|e| failure(e.to_string()))?
                .as_bytes(),
        )?;
    }
    // An explicitly supplied/remembered DEK can predate portable manifests. It
    // remains recoverable through the original and migration credential accounts.
    Ok(())
}

// The order preserves all foreign keys. Tables without a vault scope remain in
// the default vault, except rows tied to a host, which follow that host.
const TABLES: &[&str] = &[
    "vault_server_connections",
    "vault_contexts",
    "snippets",
    "identities",
    "groups",
    "hosts",
    "tunnels",
    "workspaces",
    "vault_mutations",
    "vault_sync_state",
    "transfer_history",
    "sync_snapshots",
    "shared_vaults",
    "team_members",
    "enrolled_devices",
    "vault_device_authorizations",
    "enrollment_operations",
    "device_replacements",
    "shared_vault_key_envelopes",
    "sharing_audit_log",
    "shared_session_logs",
    "cloud_providers",
    "discovered_hosts",
    "session_recordings",
    "vault_directories",
    "sqlite_sequence",
];

fn filter(table: &str, default: bool) -> &'static str {
    match table {
        "snippets" | "identities" | "groups" | "hosts" | "tunnels" | "workspaces" | "vault_mutations" | "session_recordings" | "shared_vault_key_envelopes" => "vault_id = ?1",
        "vault_sync_state" if !default => "vault_id = ?1",
        "transfer_history" if default => "host_id IS NULL OR host_id IN (SELECT id FROM hosts WHERE vault_id = ?1)",
        "transfer_history" => "host_id IN (SELECT id FROM hosts WHERE vault_id = ?1)",
        "discovered_hosts" if default => "imported_host_id IS NULL OR imported_host_id IN (SELECT id FROM hosts WHERE vault_id = ?1)",
        "discovered_hosts" => "imported_host_id IN (SELECT id FROM hosts WHERE vault_id = ?1)",
        "cloud_providers" if !default => "id IN (SELECT provider_id FROM discovered_hosts WHERE imported_host_id IN (SELECT id FROM hosts WHERE vault_id = ?1))",
        "sqlite_sequence" => "1 = 1 AND ?1 IS NOT NULL",
        _ if default => "1 = 1 AND ?1 IS NOT NULL",
        _ => "0 = 1 AND ?1 IS NOT NULL",
    }
}

struct TableRows {
    columns: Vec<String>,
    values: Vec<Vec<Value>>,
}

async fn read_rows(
    store: &HostStore,
    table: &str,
    id: &str,
    all: bool,
) -> Result<TableRows, StorageError> {
    let predicate = if all {
        "1 = 1 AND ?1 IS NOT NULL"
    } else {
        filter(table, id == LEGACY_LOCAL_VAULT_ID)
    };
    let sql = format!("SELECT * FROM {table} WHERE {predicate} ORDER BY rowid");
    let mut rows = store.connection.query(sql, (id,)).await?;
    let columns = rows.column_names();
    let mut values = Vec::new();
    while let Some(row) = rows.next().await? {
        values.push(
            (0..columns.len())
                .map(|i| row.get_value(i))
                .collect::<Result<_, _>>()?,
        );
    }
    Ok(TableRows { columns, values })
}

fn secure_rows(
    rows: &mut TableRows,
    journal: &MigrationJournal,
    credentials: &dyn CredentialStore,
) -> Result<(), StorageError> {
    let id_column = rows
        .columns
        .iter()
        .position(|c| c == "id")
        .ok_or_else(|| failure("Missing server connection ID"))?;
    for row in &mut rows.values {
        let connection_id = match &row[id_column] {
            Value::Text(id) => id,
            _ => return Err(failure("Invalid server connection ID")),
        };
        let connection_digest = hex::encode(ring::digest::digest(
            &ring::digest::SHA256,
            connection_id.as_bytes(),
        ));
        for field in [
            "auth_token",
            "refresh_token",
            "device_signing_key",
            "device_agreement_key",
        ] {
            let column = rows
                .columns
                .iter()
                .position(|c| c == field)
                .ok_or_else(|| failure("Missing server credential column"))?;
            if let Value::Text(raw) = &row[column] {
                if !raw.is_empty() && !raw.starts_with("keychain://") && !raw.starts_with("env://")
                {
                    let account = format!(
                        "migration-{}-{connection_digest}-{field}",
                        journal.transaction
                    );
                    store_owned_secret(credentials, "org.shellumi.server", &account, raw)?;
                    row[column] = Value::Text(format!("keychain://org.shellumi.server/{account}"));
                }
            }
        }
    }
    Ok(())
}

async fn open_source(
    root: &Path,
    journal: &MigrationJournal,
    supplied: Option<&VaultKey>,
    credentials: &dyn CredentialStore,
) -> Result<HostStore, StorageError> {
    let path = root.join("source/source.db");
    no_symlink(&path)?;
    let path_str = path
        .to_str()
        .ok_or_else(|| failure("Invalid source path"))?;
    if !crate::is_protected_vault_path(path_str) {
        return HostStore::open(path_str).await;
    }
    let service = key_service(journal);
    let saved = credentials
        .get_secret(&service, "source-key")
        .map_err(|e| failure(format!("Read migration source key: {e}")))?;
    let saved = saved
        .map(|secret| {
            let data = STANDARD
                .decode(secret.expose_secret())
                .map_err(|_| failure("Corrupt migration source key"))?;
            let bytes: [u8; 32] = data
                .try_into()
                .map_err(|_| failure("Invalid migration source key length"))?;
            Ok::<_, StorageError>(VaultKey::from_bytes(bytes))
        })
        .transpose()?;
    let key = if let Some(key) = supplied {
        key.clone()
    } else if let Some(key) = saved {
        key
    } else {
        VaultKeyManager::retrieve_key(
            credentials,
            &VaultId::new(LEGACY_LOCAL_VAULT_ID).map_err(|e| failure(e.to_string()))?,
        )
        .map_err(|e| failure(format!("Read encrypted legacy key: {e}")))?
        .ok_or_else(|| {
            failure("MigrationRequired: unlock the encrypted legacy database to resume")
        })?
    };
    store_owned_secret(
        credentials,
        &service,
        "source-key",
        &STANDARD.encode(key.expose_bytes()),
    )?;
    HostStore::open_with_key(path_str, Some(&key)).await
}

async fn checkpoint(store: &HostStore) -> Result<(), StorageError> {
    let mut rows = store
        .connection
        .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
        .await?;
    while let Some(row) = rows.next().await? {
        let busy: i64 = row.get(0)?;
        if busy != 0 {
            return Err(failure(
                "Migration database checkpoint is busy; close other database sessions and retry",
            ));
        }
    }
    Ok(())
}

async fn foreign_keys(store: &HostStore) -> Result<(), StorageError> {
    let mut rows = store
        .connection
        .query("PRAGMA foreign_key_check", ())
        .await?;
    if rows.next().await?.is_some() {
        return Err(failure(
            "Migration found a broken or cross-vault foreign key; source and backup preserved",
        ));
    }
    Ok(())
}

async fn verify_tables(
    source: &HostStore,
    target: &HostStore,
    journal: &MigrationJournal,
    index: usize,
    credentials: &dyn CredentialStore,
) -> Result<(), StorageError> {
    let id = &journal.vaults[index].id;
    for table in TABLES {
        let mut expected = read_rows(source, table, id, false).await?;
        if *table == "vault_server_connections" {
            secure_rows(&mut expected, journal, credentials)?;
        }
        let actual = read_rows(target, table, id, true).await?;
        if expected.columns != actual.columns || expected.values != actual.values {
            return Err(failure(format!("Verification failed: migration changed rows, identifiers, relationships or metadata in {table} for vault {id}")));
        }
    }
    foreign_keys(target).await
}

async fn copy_tables(
    source: &HostStore,
    target: &HostStore,
    journal: &MigrationJournal,
    index: usize,
    credentials: &dyn CredentialStore,
    fault: &mut Fault<'_>,
) -> Result<(), StorageError> {
    let id = &journal.vaults[index].id;
    target
        .connection
        .execute("PRAGMA foreign_keys = OFF", ())
        .await?;
    target.connection.execute("BEGIN IMMEDIATE", ()).await?;
    // Schema initialization seeds a registry row. This database is a fresh,
    // migration-owned attempt; replace every seed with the original rows.
    for table in TABLES.iter().rev() {
        target
            .connection
            .execute(format!("DELETE FROM {table}"), ())
            .await?;
    }
    for table in TABLES {
        let mut data = read_rows(source, table, id, false).await?;
        if *table == "vault_server_connections" {
            secure_rows(&mut data, journal, credentials)?;
        }
        if *table == "sqlite_sequence" {
            target
                .connection
                .execute("DELETE FROM sqlite_sequence", ())
                .await?;
        }
        let names = data
            .columns
            .iter()
            .map(|name| format!("\"{}\"", name.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(",");
        let parameters = (1..=data.columns.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let insert = format!("INSERT INTO {table} ({names}) VALUES ({parameters})");
        for row in data.values {
            target.connection.execute(&insert, row).await?;
        }
        tracing::debug!(vault_id = %id, table, "Migration copied original rows");
        fault(&format!("copy:{index}:{table}"))?;
    }
    target.connection.execute("COMMIT", ()).await?;
    target
        .connection
        .execute("PRAGMA foreign_keys = ON", ())
        .await?;
    verify_tables(source, target, journal, index, credentials).await?;
    checkpoint(target).await
}

fn verify_seals(base: &Path, root: &Path, journal: &MigrationJournal) -> Result<(), StorageError> {
    for (index, vault) in journal.vaults.iter().enumerate() {
        if vault.staged && Some(digest(&staged_db(root, index))?) != vault.db_digest {
            return Err(failure(
                "Verified staged database changed; migration activation refused",
            ));
        }
        if let Some(expected) = &vault.manifest_digest {
            if digest(&staged_manifest(root, index))? != *expected {
                return Err(failure(
                    "Verified staged manifest changed; migration activation refused",
                ));
            }
        }
        if let Some(expected) = &vault.existing_manifest_digest {
            if digest(&destination(base, vault, true))? != *expected {
                return Err(failure(
                    "Original manifest changed during migration; activation refused",
                ));
            }
        }
    }
    Ok(())
}

fn artifact(root: &Path, entry: &Activation) -> PathBuf {
    if entry.manifest {
        staged_manifest(root, entry.vault_index)
    } else {
        staged_db(root, entry.vault_index)
    }
}

fn linked(source: &Path, destination: &Path) -> Result<bool, StorageError> {
    no_symlink(source)?;
    no_symlink(destination)?;
    if !destination.exists() {
        return Ok(false);
    }
    io(
        same_file::is_same_file(source, destination),
        "Establish migration file ownership",
    )
}

fn rollback(
    base: &Path,
    root: &Path,
    journal: &mut MigrationJournal,
    fault: &mut Fault<'_>,
) -> Result<(), StorageError> {
    journal.phase = Phase::RollingBack;
    save(root, journal)?;
    fault("rollback_started")?;
    for (index, entry) in journal.activation.iter().enumerate().rev() {
        if entry.state == LinkState::Pending {
            continue;
        }
        let source = artifact(root, entry);
        let target = destination(base, &journal.vaults[entry.vault_index], entry.manifest);
        if linked(&source, &target)? {
            let vault = &journal.vaults[entry.vault_index];
            let expected = if entry.manifest {
                &vault.manifest_digest
            } else {
                &vault.db_digest
            };
            if Some(digest(&target)?) != *expected {
                return Err(failure(format!(
                    "Activated file {} changed after verification; it was preserved. Inspect it alongside the original and backup before completing rollback",
                    target.display()
                )));
            }
            io(
                fs::remove_file(&target),
                "Roll back migration-owned activation",
            )?;
            sync_dir(
                target
                    .parent()
                    .ok_or_else(|| failure("Activation has no parent"))?,
            )?;
            fault(&format!("rollback_file:{index}"))?;
        } else if target.exists() {
            tracing::warn!(path = %target.display(), "Preserved unrelated destination during migration rollback");
        }
    }
    let receipt = root.join("completion_receipt.json");
    let marker = base.join(MARKER);
    if receipt.exists() && marker.exists() && linked(&receipt, &marker)? {
        io(
            fs::remove_file(&marker),
            "Remove migration-owned incomplete completion receipt",
        )?;
        sync_dir(&base.join("vaults"))?;
        fault("rollback_marker")?;
    }
    journal.phase = Phase::RolledBack;
    save(root, journal)?;
    fault("rolled_back")?;
    tracing::warn!(transaction = %journal.transaction, "Migration-owned activation rolled back; source, backup and encrypted staging retained");
    Ok(())
}

impl MigrationEngine {
    pub fn is_migration_needed(base_dir: &Path) -> bool {
        let canonical = match base_dir.canonicalize() {
            Ok(path) => path,
            Err(_) => return base_dir.join("shellumi.db").exists(),
        };
        let base_dir = canonical.as_path();
        if !base_dir.join("shellumi.db").exists() && !base_dir.join(ROOT).exists() {
            return false;
        }
        let root = base_dir.join(ROOT);
        if !root.exists() {
            return true;
        }
        match load(&root) {
            Ok(journal) if journal.phase == Phase::Complete => {
                !Self::receipt_matches(base_dir, &journal)
            }
            _ => true,
        }
    }

    fn receipt_matches(base: &Path, journal: &MigrationJournal) -> bool {
        if no_symlink(&base.join(MARKER)).is_err() {
            return false;
        }
        let bytes = match fs::read(base.join(MARKER)) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let receipt: Receipt = match serde_json::from_slice(&bytes) {
            Ok(receipt) => receipt,
            Err(_) => return false,
        };
        if receipt.version != JOURNAL_VERSION || receipt.transaction != journal.transaction {
            return false;
        }
        if journal.phase == Phase::Complete {
            return true;
        }
        journal.vaults.iter().all(|vault| {
            let db = destination(base, vault, false);
            let manifest = destination(base, vault, true);
            no_symlink(&db).is_ok()
                && db.is_file()
                && ((vault.manifest_digest.is_none() && vault.existing_manifest_digest.is_none())
                    || (no_symlink(&manifest).is_ok() && manifest.is_file()))
        })
    }

    pub async fn migrate_legacy_database_if_needed(
        base_dir: &Path,
        default_dek: Option<&VaultKey>,
        store: &dyn CredentialStore,
    ) -> Result<MigrationReport, StorageError> {
        Self::migrate_legacy_database_with_credentials(base_dir, default_dek, None, store).await
    }

    pub async fn migrate_legacy_database_with_credentials(
        base_dir: &Path,
        default_dek: Option<&VaultKey>,
        credentials: Option<&MigrationCredentials>,
        store: &dyn CredentialStore,
    ) -> Result<MigrationReport, StorageError> {
        let result = Self::run(base_dir, default_dek, credentials, store, &mut |_| Ok(())).await;
        if let Err(error) = &result {
            tracing::error!(error = %error, "Legacy migration requires attention");
        }
        result
    }

    async fn run(
        base_dir: &Path,
        default_dek: Option<&VaultKey>,
        credentials: Option<&MigrationCredentials>,
        store: &dyn CredentialStore,
        fault: &mut Fault<'_>,
    ) -> Result<MigrationReport, StorageError> {
        let base = io(base_dir.canonicalize(), "Locate migration source directory")?;
        if !base.join("shellumi.db").exists() && !base.join(ROOT).exists() {
            return Ok(MigrationReport::default());
        }
        no_symlink(&base.join(".migration.lock"))?;
        let lock = io(
            OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(base.join(".migration.lock")),
            "Open migration lock",
        )?;
        lock.try_lock().map_err(|e| {
            failure(format!(
                "Another migration is running; retry after it exits: {e}"
            ))
        })?;
        let root = base.join(ROOT);
        no_symlink(&base.join("vaults"))?;
        no_symlink(&base.join("vault_manifests"))?;
        no_symlink(&root)?;
        let initializable = if root.exists() && !root.join(JOURNAL).exists() {
            let entries = io(
                fs::read_dir(&root),
                "Inspect interrupted journal initialization",
            )?;
            let mut safe = true;
            for entry in entries {
                let entry = io(entry, "Inspect initial journal artifact")?;
                if !entry.file_name().to_string_lossy().starts_with(".write-")
                    || !io(entry.file_type(), "Inspect initial journal file")?.is_file()
                {
                    safe = false;
                }
            }
            safe
        } else {
            false
        };
        let mut journal = if root.exists() && !initializable {
            load(&root)?
        } else {
            if base.join(MARKER).exists() {
                return Err(failure("Unverified legacy migration marker exists; original data preserved; inspect existing vaults before recovery"));
            }
            let source_files = source_fingerprints(&base)?;
            directory(&root)?;
            let journal = MigrationJournal {
                version: JOURNAL_VERSION,
                transaction: Uuid::new_v4().to_string(),
                phase: Phase::Created,
                source_files,
                source_digest: None,
                vaults: Vec::new(),
                activation: Vec::new(),
                report: MigrationReport {
                    legacy_db_found: true,
                    backup_path: Some(root.join("backup/source.db")),
                    ..Default::default()
                },
            };
            save(&root, &journal)?;
            fault("journal_created")?;
            journal
        };
        if matches!(journal.phase, Phase::Complete | Phase::Finalized)
            && Self::receipt_matches(&base, &journal)
        {
            if journal.phase != Phase::Complete {
                if let Err(error) = verify_seals(&base, &root, &journal) {
                    rollback(&base, &root, &mut journal, fault)?;
                    return Err(error);
                }
                for entry in &journal.activation {
                    if !linked(
                        &artifact(&root, entry),
                        &destination(&base, &journal.vaults[entry.vault_index], entry.manifest),
                    )? {
                        rollback(&base, &root, &mut journal, fault)?;
                        return Err(failure(
                            "Completion receipt lacked verified activation; migration rolled back",
                        ));
                    }
                }
                sync_dir(&base.join("vaults"))?;
                journal.phase = Phase::Complete;
                save(&root, &journal)?;
            }
            return Ok(MigrationReport {
                legacy_db_found: true,
                ..Default::default()
            });
        }
        if journal.phase == Phase::Complete {
            return Err(failure(
                "Completed migration receipt is missing or changed; existing vaults preserved",
            ));
        }
        if matches!(journal.phase, Phase::RollingBack | Phase::RolledBack) {
            rollback(&base, &root, &mut journal, fault)?;
            return Err(failure("Migration was rolled back; source and backup are intact. Resolve the reported conflict and preserve this recovery directory before starting a new migration"));
        }
        if journal.phase == Phase::Created {
            let current = source_fingerprints(&base)?;
            if current != journal.source_files {
                return Err(failure("Legacy database changed during backup; close other clients and preserve recovery data before retrying"));
            }
            directory(&root.join("backup"))?;
            for (suffix, expected) in &journal.source_files {
                let target = root.join(format!("backup/source.db{suffix}"));
                let data = io(
                    fs::read(base.join(format!("shellumi.db{suffix}"))),
                    "Read legacy backup source",
                )?;
                atomic_write(&target, &data)?;
                if digest(&target)? != *expected {
                    return Err(failure(
                        "Legacy backup changed while being copied; source preserved",
                    ));
                }
                fault(&format!("backup:{suffix}"))?;
            }
            if source_fingerprints(&base)? != journal.source_files {
                return Err(failure(
                    "Legacy source changed during backup; migration stopped",
                ));
            }
            journal.phase = Phase::BackedUp;
            save(&root, &journal)?;
            fault("backed_up")?;
        }
        if journal.phase == Phase::BackedUp {
            // Only the unverified working source may be recreated. The immutable
            // backup and every previously verified vault remain untouched.
            let source_dir = root.join("source");
            no_symlink(&source_dir)?;
            if source_dir.exists() {
                io(
                    fs::remove_dir_all(&source_dir),
                    "Reset unverified migration source",
                )?;
            }
            directory(&source_dir)?;
            for (suffix, expected) in &journal.source_files {
                let backup = root.join(format!("backup/source.db{suffix}"));
                if digest(&backup)? != *expected {
                    return Err(failure(
                        "Migration backup is corrupt; original database preserved",
                    ));
                }
                atomic_write(
                    &source_dir.join(format!("source.db{suffix}")),
                    &io(fs::read(backup), "Read verified backup")?,
                )?;
            }
            let source_path = source_dir.join("source.db");
            let source = open_source(&root, &journal, default_dek, store).await?;
            Self::plan(&base, &source, &mut journal).await?;
            checkpoint(&source).await?;
            drop(source);
            io(
                File::open(&source_path).and_then(|f| f.sync_all()),
                "Flush upgraded migration source",
            )?;
            journal.source_digest = Some(digest(&source_path)?);
            journal.phase = Phase::Planned;
            save(&root, &journal)?;
            fault("planned")?;
        }
        if let Err(error) = verify_seals(&base, &root, &journal) {
            rollback(&base, &root, &mut journal, fault)?;
            return Err(error);
        }
        if journal.phase == Phase::Planned {
            let source_path = root.join("source/source.db");
            if Some(digest(&source_path)?) != journal.source_digest {
                return Err(failure(
                    "Verified migration source changed; backup and original preserved",
                ));
            }
            let source = open_source(&root, &journal, default_dek, store).await?;
            for index in 0..journal.vaults.len() {
                let key = vault_key(
                    &base,
                    &root,
                    &journal,
                    index,
                    default_dek,
                    credentials,
                    store,
                )?;
                fault(&format!("key_stored:{index}"))?;
                ensure_manifest(&root, &journal, index, &key, credentials)?;
                fault(&format!("manifest_written:{index}"))?;
                if staged_manifest(&root, index).exists() {
                    journal.vaults[index].manifest_digest =
                        Some(digest(&staged_manifest(&root, index))?);
                    save(&root, &journal)?;
                }
                fault(&format!("key:{index}"))?;
                if journal.vaults[index].staged {
                    continue;
                }
                let work = root.join(format!("work/{index}"));
                no_symlink(&work)?;
                if work.exists() {
                    io(fs::remove_dir_all(&work), "Reset unverified vault attempt")?;
                }
                directory(&work)?;
                let path = staged_db(&root, index);
                let target = HostStore::open_with_key(
                    path.to_str()
                        .ok_or_else(|| failure("Invalid staged path"))?,
                    Some(&key),
                )
                .await?;
                copy_tables(&source, &target, &journal, index, store, fault).await?;
                drop(target);
                fault(&format!("staging_committed:{index}"))?;
                io(
                    File::open(&path).and_then(|f| f.sync_all()),
                    "Flush encrypted staged database",
                )?;
                sync_dir(&work)?;
                journal.vaults[index].db_digest = Some(digest(&path)?);
                if staged_manifest(&root, index).exists() {
                    journal.vaults[index].manifest_digest =
                        Some(digest(&staged_manifest(&root, index))?);
                }
                journal.vaults[index].staged = true;
                save(&root, &journal)?;
                fault(&format!("staged:{index}"))?;
            }
            for index in 0..journal.vaults.len() {
                let key = vault_key(
                    &base,
                    &root,
                    &journal,
                    index,
                    default_dek,
                    credentials,
                    store,
                )?;
                let target = HostStore::open_with_key(
                    staged_db(&root, index)
                        .to_str()
                        .ok_or_else(|| failure("Invalid staged path"))?,
                    Some(&key),
                )
                .await?;
                verify_tables(&source, &target, &journal, index, store).await?;
                checkpoint(&target).await?;
                drop(target);
                // Opening for verification must not introduce an unsealed WAL.
                journal.vaults[index].db_digest = Some(digest(&staged_db(&root, index))?);
                fault(&format!("verified:{index}"))?;
            }
            drop(source);
            journal.activation = journal
                .vaults
                .iter()
                .enumerate()
                .flat_map(|(index, vault)| {
                    let mut files = vec![Activation {
                        vault_index: index,
                        manifest: false,
                        state: LinkState::Pending,
                    }];
                    if vault.manifest_digest.is_some() {
                        files.push(Activation {
                            vault_index: index,
                            manifest: true,
                            state: LinkState::Pending,
                        });
                    }
                    files
                })
                .collect();
            journal.phase = Phase::Verified;
            save(&root, &journal)?;
            fault("verified")?;
        }
        if matches!(journal.phase, Phase::Verified | Phase::Activating) {
            if source_fingerprints(&base)? != journal.source_files {
                rollback(&base, &root, &mut journal, fault)?;
                return Err(failure("Legacy database changed after backup; activation rolled back; close other clients before migration"));
            }
            journal.phase = Phase::Activating;
            save(&root, &journal)?;
            for index in 0..journal.activation.len() {
                let entry = &journal.activation[index];
                let source = artifact(&root, entry);
                let target = destination(&base, &journal.vaults[entry.vault_index], entry.manifest);
                directory(
                    target
                        .parent()
                        .ok_or_else(|| failure("Invalid activation path"))?,
                )?;
                if target.exists() {
                    if entry.state == LinkState::Pending || !linked(&source, &target)? {
                        rollback(&base, &root, &mut journal, fault)?;
                        return Err(failure(format!("Existing destination {} was preserved; migration activation rolled back", target.display())));
                    }
                } else {
                    journal.activation[index].state = LinkState::Intent;
                    save(&root, &journal)?;
                    fault(&format!("activation_intent:{index}"))?;
                    if let Err(error) = fs::hard_link(&source, &target) {
                        rollback(&base, &root, &mut journal, fault)?;
                        return Err(failure(format!(
                            "Create-only activation failed for {}: {error}",
                            target.display()
                        )));
                    }
                    sync_dir(
                        target
                            .parent()
                            .ok_or_else(|| failure("Invalid activation parent"))?,
                    )?;
                    fault(&format!("activation_link:{index}"))?;
                }
                journal.activation[index].state = LinkState::Linked;
                save(&root, &journal)?;
                fault(&format!("activated_file:{index}"))?;
            }
            verify_seals(&base, &root, &journal)?;
            for entry in &journal.activation {
                if !linked(
                    &artifact(&root, entry),
                    &destination(&base, &journal.vaults[entry.vault_index], entry.manifest),
                )? {
                    rollback(&base, &root, &mut journal, fault)?;
                    return Err(failure(
                        "Migration activation ownership verification failed",
                    ));
                }
            }
            journal.phase = Phase::Activated;
            save(&root, &journal)?;
            fault("activated")?;
        }
        if matches!(journal.phase, Phase::Activated | Phase::Finalized) {
            for entry in &journal.activation {
                let target = destination(&base, &journal.vaults[entry.vault_index], entry.manifest);
                if !linked(&artifact(&root, entry), &target)? {
                    rollback(&base, &root, &mut journal, fault)?;
                    return Err(failure(
                        "Activated migration file was replaced; unrelated files preserved",
                    ));
                }
            }
            if source_fingerprints(&base)? != journal.source_files {
                rollback(&base, &root, &mut journal, fault)?;
                return Err(failure(
                    "Legacy source changed before finalization; activation rolled back",
                ));
            }
        }
        if journal.phase == Phase::Activated {
            verify_seals(&base, &root, &journal)?;
            for index in 0..journal.vaults.len() {
                let key = vault_key(
                    &base,
                    &root,
                    &journal,
                    index,
                    default_dek,
                    credentials,
                    store,
                )?;
                let vid =
                    VaultId::new(&journal.vaults[index].id).map_err(|e| failure(e.to_string()))?;
                let existing = VaultKeyManager::retrieve_key(store, &vid)
                    .map_err(|e| failure(format!("Read final vault key: {e}")))?;
                if let Some(existing) = existing {
                    if existing.expose_bytes() != key.expose_bytes() {
                        return Err(failure("An existing vault key changed during migration; key was not overwritten"));
                    }
                } else {
                    VaultKeyManager::provision_key(store, &vid, &key)
                        .map_err(|e| failure(format!("Remember migrated vault key: {e}")))?;
                    let saved = VaultKeyManager::retrieve_key(store, &vid)
                        .map_err(|e| failure(format!("Verify final vault key: {e}")))?
                        .ok_or_else(|| failure("Final vault key did not persist"))?;
                    if saved.expose_bytes() != key.expose_bytes() {
                        return Err(failure("Final vault key failed verification"));
                    }
                }
                fault(&format!("final_key:{index}"))?;
            }
            journal.phase = Phase::Finalized;
            save(&root, &journal)?;
            fault("finalized")?;
        }
        if journal.phase == Phase::Finalized {
            verify_seals(&base, &root, &journal)?;
            let receipt = root.join("completion_receipt.json");
            let bytes = serde_json::to_vec(&Receipt {
                version: JOURNAL_VERSION,
                transaction: journal.transaction.clone(),
            })
            .map_err(|e| failure(e.to_string()))?;
            if !receipt.exists() {
                atomic_write(&receipt, &bytes)?;
            }
            let marker = base.join(MARKER);
            if !marker.exists() {
                io(
                    fs::hard_link(&receipt, &marker),
                    "Publish verified migration completion receipt",
                )?;
                sync_dir(&base.join("vaults"))?;
            }
            if !Self::receipt_matches(&base, &journal) {
                return Err(failure(
                    "Existing completion marker does not belong to this migration; preserved",
                ));
            }
            fault("completion_marker")?;
            journal.phase = Phase::Complete;
            save(&root, &journal)?;
            tracing::info!(
                vaults = journal.vaults.len(),
                "Legacy migration activated and verified; original and recovery artifacts retained"
            );
        }
        Ok(journal.report)
    }

    async fn plan(
        base: &Path,
        source: &HostStore,
        journal: &mut MigrationJournal,
    ) -> Result<(), StorageError> {
        let mut catalog = source
            .connection
            .query(
                "SELECT name, type FROM sqlite_schema WHERE type IN ('table','view','trigger')",
                (),
            )
            .await?;
        while let Some(row) = catalog.next().await? {
            let name: String = row.get(0)?;
            let kind: String = row.get(1)?;
            if kind != "table" {
                return Err(failure(format!(
                    "Unsupported legacy schema object {name}; source preserved"
                )));
            }
            let internal_sequence = name
                .strip_prefix("__turso_internal_seq___turso_internal_autoincrement_")
                .is_some_and(|table| TABLES.contains(&table));
            if !TABLES.contains(&name.as_str()) && !internal_sequence {
                return Err(failure(format!(
                    "Unsupported legacy table {name}; migration stopped without changing source"
                )));
            }
        }
        drop(catalog);
        let mut ids = BTreeMap::new();
        ids.insert(LEGACY_LOCAL_VAULT_ID.to_string(), "Personal".to_string());
        for (name, context) in source.list_vault_contexts().await? {
            ids.insert(context.id().as_str().to_string(), name);
        }
        for table in [
            "hosts",
            "identities",
            "snippets",
            "groups",
            "tunnels",
            "workspaces",
            "session_recordings",
            "vault_mutations",
            "vault_sync_state",
            "shared_vault_key_envelopes",
        ] {
            let mut rows = source
                .connection
                .query(format!("SELECT DISTINCT vault_id FROM {table}"), ())
                .await?;
            while let Some(row) = rows.next().await? {
                let id: String = row.get(0)?;
                ids.entry(id.clone()).or_insert(id);
            }
        }
        let mut filenames = BTreeSet::new();
        for (id, name) in ids {
            safe_id(&id)?;
            if !filenames.insert(db_name(&id).to_ascii_lowercase()) {
                return Err(failure(
                    "Legacy vault IDs have colliding filenames; source preserved",
                ));
            }
            let mut vault = VaultPlan {
                id,
                name,
                staged: false,
                db_digest: None,
                manifest_digest: None,
                existing_manifest_digest: None,
            };
            let db = destination(base, &vault, false);
            no_symlink(&db)?;
            if db.exists() {
                return Err(failure(format!(
                    "Existing vault database {} will not be overwritten",
                    db.display()
                )));
            }
            let manifest = destination(base, &vault, true);
            no_symlink(&manifest)?;
            if manifest.exists() {
                let existing = VaultManifest::load_from_path(&manifest)
                    .map_err(|e| failure(format!("Existing manifest cannot be read: {e}")))?;
                if existing.vault_id != vault.id {
                    return Err(failure("Existing manifest identity mismatch"));
                }
                vault.existing_manifest_digest = Some(digest(&manifest)?);
            }
            let records = ["hosts", "snippets", "workspaces", "identities"];
            let mut count = 0;
            for table in records {
                count += read_rows(source, table, &vault.id, false)
                    .await?
                    .values
                    .len();
            }
            if vault.id == LEGACY_LOCAL_VAULT_ID {
                journal.report.default_records_migrated = count;
            } else {
                journal.report.secondary_records_migrated += count;
            }
            journal.vaults.push(vault);
        }
        journal.report.vaults_migrated = journal.vaults.len();
        journal.report.server_credentials_migrated =
            source.list_vault_server_connections().await?.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests;

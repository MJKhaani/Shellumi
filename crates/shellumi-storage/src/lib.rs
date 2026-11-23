//! Local Turso storage. Credentials are never stored in this database.
pub mod desktop;
pub mod migration;
pub mod server_sync;
pub use migration::{MigrationCredentials, MigrationEngine, MigrationReport};
pub use server_sync::{
    PendingMutationRecord, ServerSyncEngine, ServerSyncReport, VaultServerConnectionRecord,
    VaultSyncStateRecord,
};
use shellumi_core::{
    resolve_effective_tags, validate_jump_host, AuditAction, CollaborationError, DestinationError,
    ForwardingType, GroupError, HostGroup, LogBookmark, LogComment, PortForwardRule,
    SessionRecordingSummary, SharedSessionLog, SharedVault, SharingAuditRecord, Snippet,
    SnippetError, SnippetVariable, SshDestination, TeamMember, TeamRole, TransferDirection,
    TunnelError, WorkspaceError, WorkspaceSession,
};
use std::fmt;
use std::str::FromStr;
use turso::{transaction::TransactionBehavior, Builder, Connection, Database};

const SCHEMA_VERSION: i64 = 22;

/// Registry identity reserved for the existing, unscoped local data.
pub const LEGACY_LOCAL_VAULT_ID: &str = "legacy-local";

#[derive(Debug)]
pub enum StorageError {
    Database(turso::Error),
    UnsupportedSchema(i64),
    InvalidName,
    InvalidGroupName,
    InvalidTag,
    InvalidDestination(DestinationError),
    InvalidGroup(GroupError),
    InvalidTunnel(TunnelError),
    InvalidJumpHost(TunnelError),
    InvalidPort(i64),
    InvalidTransferPath,
    InvalidSnippet(SnippetError),
    InvalidWorkspace(WorkspaceError),
    InvalidCollaboration(CollaborationError),
    InvalidRecording(String),
    InvalidIdentity(shellumi_core::IdentityError),
    InvalidCredential(shellumi_core::CredentialError),
    InvalidVaultIdentity(shellumi_core::VaultIdentityError),
    UnknownLocalVault,
    InvalidIdentityReference,
    Serialization(String),
    InvalidSync(shellumi_core::SyncError),
    SnapshotNotFound(String),
    MissingSchemaVersion,
    CrossVault(shellumi_core::CrossVaultError),
    ItemNotFound(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "database error: {error}"),
            Self::UnsupportedSchema(version) => {
                write!(f, "unsupported database schema version {version}")
            }
            Self::InvalidName => f.write_str("invalid host name"),
            Self::InvalidGroupName => f.write_str("group name must not contain control characters"),
            Self::InvalidTag => f.write_str("tag must not contain commas or control characters"),
            Self::InvalidDestination(error) => write!(f, "invalid stored destination: {error}"),
            Self::InvalidGroup(error) => write!(f, "invalid stored group: {error}"),
            Self::InvalidTunnel(error) => write!(f, "invalid tunnel configuration: {error}"),
            Self::InvalidJumpHost(error) => write!(f, "invalid jump host: {error}"),
            Self::InvalidPort(port) => write!(f, "invalid stored port: {port}"),
            Self::InvalidTransferPath => f.write_str("transfer paths must not be empty"),
            Self::InvalidSnippet(error) => write!(f, "invalid snippet: {error}"),
            Self::InvalidWorkspace(error) => write!(f, "invalid workspace: {error}"),
            Self::InvalidCollaboration(error) => {
                write!(f, "invalid collaboration operation: {error}")
            }
            Self::InvalidRecording(msg) => write!(f, "invalid session recording: {msg}"),
            Self::InvalidIdentity(error) => write!(f, "invalid identity: {error}"),
            Self::InvalidCredential(error) => write!(f, "credential store error: {error}"),
            Self::InvalidVaultIdentity(error) => write!(f, "invalid vault identity: {error}"),
            Self::UnknownLocalVault => f.write_str("local vault is not registered"),
            Self::InvalidIdentityReference => f.write_str("identity does not belong to this vault"),
            Self::Serialization(error) => write!(f, "serialization error: {error}"),
            Self::InvalidSync(error) => write!(f, "sync error: {error}"),
            Self::SnapshotNotFound(id) => write!(f, "sync snapshot not found: {id}"),
            Self::MissingSchemaVersion => f.write_str("database did not return a schema version"),
            Self::CrossVault(error) => write!(f, "cross-vault transfer error: {error}"),
            Self::ItemNotFound(msg) => write!(f, "item not found: {msg}"),
        }
    }
}

impl From<shellumi_core::CrossVaultError> for StorageError {
    fn from(error: shellumi_core::CrossVaultError) -> Self {
        Self::CrossVault(error)
    }
}

impl From<shellumi_core::IdentityError> for StorageError {
    fn from(error: shellumi_core::IdentityError) -> Self {
        Self::InvalidIdentity(error)
    }
}

impl From<shellumi_core::CredentialError> for StorageError {
    fn from(error: shellumi_core::CredentialError) -> Self {
        Self::InvalidCredential(error)
    }
}

impl From<shellumi_core::SyncError> for StorageError {
    fn from(error: shellumi_core::SyncError) -> Self {
        Self::InvalidSync(error)
    }
}

impl std::error::Error for StorageError {}

impl From<turso::Error> for StorageError {
    fn from(error: turso::Error) -> Self {
        Self::Database(error)
    }
}

impl From<GroupError> for StorageError {
    fn from(error: GroupError) -> Self {
        Self::InvalidGroup(error)
    }
}

impl From<TunnelError> for StorageError {
    fn from(error: TunnelError) -> Self {
        Self::InvalidTunnel(error)
    }
}

impl From<SnippetError> for StorageError {
    fn from(error: SnippetError) -> Self {
        Self::InvalidSnippet(error)
    }
}

impl From<CollaborationError> for StorageError {
    fn from(error: CollaborationError) -> Self {
        Self::InvalidCollaboration(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedHost {
    pub id: i64,
    pub name: String,
    pub destination: SshDestination,
    pub group_name: Option<String>,
    pub tags: Vec<String>,
    pub jump_host: Option<String>,
    pub startup_snippet_id: Option<i64>,
    pub protocol: String,
    pub protocol_config: Option<String>,
    pub identity_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedIdentity {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub auth_method: shellumi_core::AuthMethod,
    pub password_secret_ref: Option<String>,
    pub key_path: Option<String>,
    pub passphrase_secret_ref: Option<String>,
    pub certificate_path: Option<String>,
    pub tags: Vec<String>,
    pub comment: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl SavedIdentity {
    pub fn to_identity(&self) -> shellumi_core::Identity {
        shellumi_core::Identity {
            id: self.id,
            name: self.name.clone(),
            username: self.username.clone(),
            auth_method: self.auth_method.clone(),
            password_secret_ref: self.password_secret_ref.clone(),
            key_path: self.key_path.clone(),
            passphrase_secret_ref: self.passphrase_secret_ref.clone(),
            certificate_path: self.certificate_path.clone(),
            tags: self.tags.clone(),
            comment: self.comment.clone(),
            created_at: Some(self.created_at.clone()),
            updated_at: Some(self.updated_at.clone()),
        }
    }
}

impl SavedHost {
    pub fn protocol(&self) -> shellumi_core::ConnectionProtocol {
        shellumi_core::ConnectionProtocol::parse(&self.protocol)
            .unwrap_or(shellumi_core::ConnectionProtocol::Ssh)
    }

    pub fn is_ssh(&self) -> bool {
        self.protocol() == shellumi_core::ConnectionProtocol::Ssh
    }

    pub fn is_mosh(&self) -> bool {
        self.protocol() == shellumi_core::ConnectionProtocol::Mosh
    }

    pub fn is_telnet(&self) -> bool {
        self.protocol() == shellumi_core::ConnectionProtocol::Telnet
    }

    pub fn is_serial(&self) -> bool {
        self.protocol() == shellumi_core::ConnectionProtocol::Serial
    }

    pub fn is_vnc(&self) -> bool {
        self.protocol() == shellumi_core::ConnectionProtocol::Vnc
    }

    pub fn is_rdp(&self) -> bool {
        self.protocol() == shellumi_core::ConnectionProtocol::Rdp
    }

    pub fn is_graphical(&self) -> bool {
        matches!(
            self.protocol(),
            shellumi_core::ConnectionProtocol::Vnc | shellumi_core::ConnectionProtocol::Rdp
        )
    }

    pub fn vnc_config(&self) -> Option<shellumi_core::VncConfig> {
        if self.is_vnc() {
            self.protocol_config
                .as_deref()
                .and_then(|c| serde_json::from_str(c).ok())
                .or_else(|| {
                    shellumi_core::VncConfig::new(
                        self.destination.host(),
                        self.destination.port().get(),
                    )
                    .ok()
                })
        } else {
            None
        }
    }

    pub fn rdp_config(&self) -> Option<shellumi_core::RdpConfig> {
        if self.is_rdp() {
            self.protocol_config
                .as_deref()
                .and_then(|c| serde_json::from_str(c).ok())
                .or_else(|| {
                    let mut cfg = shellumi_core::RdpConfig::new(
                        self.destination.host(),
                        self.destination.port().get(),
                    )
                    .ok()?;
                    if !self.destination.username().is_empty() {
                        cfg.username = Some(self.destination.username().to_string());
                    }
                    Some(cfg)
                })
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTunnel {
    pub id: i64,
    pub host_id: Option<i64>,
    pub name: String,
    pub rule: PortForwardRule,
    pub auto_start: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTransfer {
    pub id: i64,
    pub host_id: Option<i64>,
    pub direction: TransferDirection,
    pub local_path: String,
    pub remote_path: String,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTransfer<'a> {
    pub host_id: Option<i64>,
    pub direction: TransferDirection,
    pub local_path: &'a str,
    pub remote_path: &'a str,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub status: &'a str,
    pub error_message: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedSnippet {
    pub id: i64,
    pub name: String,
    pub command: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub variables: Vec<SnippetVariable>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SavedWorkspace {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub layout: String,
    pub sessions: Vec<WorkspaceSession>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SyncWorkspacePayload {
    pub name: String,
    pub description: Option<String>,
    pub layout: String,
    pub sessions: Vec<WorkspaceSession>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotHistoryEntry {
    pub id: i64,
    pub snapshot_id: String,
    pub record_count: usize,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SyncHostPayload {
    pub name: String,
    pub hostname: String,
    pub username: String,
    pub port: u16,
    pub group_name: Option<String>,
    pub tags: Vec<String>,
    pub jump_host: Option<String>,
    #[serde(default)]
    pub startup_snippet_id: Option<i64>,
    #[serde(default = "default_host_protocol")]
    pub protocol: String,
    #[serde(default)]
    pub protocol_config: Option<String>,
    #[serde(default)]
    pub identity_id: Option<i64>,
}

fn default_host_protocol() -> String {
    "ssh".to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SyncGroupPayload {
    pub name: String,
    pub default_username: Option<String>,
    pub default_port: Option<u16>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub identity_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SyncIdentityPayload {
    pub name: String,
    pub username: String,
    pub auth_method: shellumi_core::AuthMethod,
    #[serde(default)]
    pub password_secret_ref: Option<String>,
    #[serde(default)]
    pub key_path: Option<String>,
    #[serde(default)]
    pub passphrase_secret_ref: Option<String>,
    #[serde(default)]
    pub certificate_path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SyncTunnelPayload {
    pub name: String,
    pub forwarding_type: String,
    pub bind_address: Option<String>,
    pub bind_port: u16,
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
    pub auto_start: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SyncSnippetPayload {
    pub name: String,
    pub command: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CloudProviderRecord {
    pub id: String,
    pub name: String,
    pub provider_kind: String,
    pub config_json: String,
    pub sync_interval_mins: i64,
    pub last_synced_at: Option<i64>,
    pub auto_sync: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredHostRecord {
    pub id: String,
    pub provider_id: String,
    pub external_id: String,
    pub name: String,
    pub hostname: String,
    pub port: i64,
    pub username: Option<String>,
    pub state: String,
    pub region: Option<String>,
    pub group_path: Option<String>,
    pub tags_json: String,
    pub metadata_json: String,
    pub imported_host_id: Option<i64>,
    pub last_seen_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportVaultSummary {
    pub hosts_imported: usize,
    pub groups_imported: usize,
    pub tunnels_imported: usize,
    pub snippets_imported: usize,
    pub workspaces_imported: usize,
    pub shared_vaults_imported: usize,
    pub team_members_imported: usize,
    pub cloud_providers_imported: usize,
    pub errors: Vec<String>,
}

pub struct HostStore {
    _database: Database,
    connection: Connection,
    host_vault_id: shellumi_core::VaultId,
    db_path: Option<String>,
}

/// Restricted host-only access; this does not isolate identities, groups, or secrets.
pub struct VaultHostStore(HostStore);

impl VaultHostStore {
    pub async fn open(path: &str, vault_id: &shellumi_core::VaultId) -> Result<Self, StorageError> {
        let mut store = HostStore::open(path).await?;
        let mut rows = store
            .connection
            .query(
                "SELECT id FROM vault_contexts WHERE id = ?1 AND server_connection_id IS NULL",
                (vault_id.as_str(),),
            )
            .await?;
        let exists = rows.next().await?.is_some();
        drop(rows);
        if !exists {
            return Err(StorageError::UnknownLocalVault);
        }
        store.host_vault_id = vault_id.clone();
        Ok(Self(store))
    }

    pub async fn add(
        &mut self,
        name: &str,
        destination: &SshDestination,
    ) -> Result<i64, StorageError> {
        self.0.add(name, destination, None, &[], None, None).await
    }

    pub async fn list(&self) -> Result<Vec<SavedHost>, StorageError> {
        self.0.list().await
    }

    pub async fn get(&self, id: i64) -> Result<Option<SavedHost>, StorageError> {
        self.0.get_host(id).await
    }

    pub async fn update(
        &mut self,
        id: i64,
        name: &str,
        destination: &SshDestination,
    ) -> Result<bool, StorageError> {
        self.0
            .update(id, name, destination, None, &[], None, None)
            .await
    }

    pub async fn delete(&mut self, id: i64) -> Result<bool, StorageError> {
        self.0.delete(id).await
    }

    /// Only vault-scoped keychain references are accepted for new identities.
    pub async fn create_identity(
        &mut self,
        identity: &shellumi_core::NewIdentity,
        backend: &dyn shellumi_core::CredentialStore,
    ) -> Result<i64, StorageError> {
        self.validate_identity_refs(identity, backend)?;
        self.0.create_identity(identity).await
    }

    pub async fn update_identity(
        &mut self,
        id: i64,
        identity: &shellumi_core::NewIdentity,
        backend: &dyn shellumi_core::CredentialStore,
    ) -> Result<bool, StorageError> {
        self.validate_identity_refs(identity, backend)?;
        self.0.update_identity(id, identity).await
    }

    fn validate_identity_refs(
        &self,
        identity: &shellumi_core::NewIdentity,
        backend: &dyn shellumi_core::CredentialStore,
    ) -> Result<(), StorageError> {
        let credentials = shellumi_core::VaultCredentialStore::new(backend, &self.0.host_vault_id);
        for reference in [
            &identity.password_secret_ref,
            &identity.passphrase_secret_ref,
        ]
        .into_iter()
        .flatten()
        {
            credentials.validate_reference(reference)?;
        }
        Ok(())
    }

    pub async fn get_identity(&self, id: i64) -> Result<Option<SavedIdentity>, StorageError> {
        self.0.get_identity(id).await
    }

    pub async fn list_identities(&self) -> Result<Vec<SavedIdentity>, StorageError> {
        self.0.list_identities().await
    }

    pub async fn delete_identity(&mut self, id: i64) -> Result<bool, StorageError> {
        self.0.delete_identity(id).await
    }

    pub async fn resolve_identity_password(
        &self,
        id: i64,
        backend: &dyn shellumi_core::CredentialStore,
    ) -> Result<Option<shellumi_core::SecretString>, StorageError> {
        let identity = self
            .get_identity(id)
            .await?
            .ok_or(StorageError::InvalidIdentityReference)?;
        match identity.password_secret_ref {
            Some(reference) => Ok(shellumi_core::VaultCredentialStore::new(
                backend,
                &self.0.host_vault_id,
            )
            .resolve(&reference)?),
            None => Ok(None),
        }
    }
}

fn validated_name(name: &str) -> Result<&str, StorageError> {
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err(StorageError::InvalidName);
    }
    Ok(name)
}

fn validated_group(group: Option<&str>) -> Result<Option<String>, StorageError> {
    match group {
        Some(g) => {
            let trimmed = g.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else if trimmed.chars().any(char::is_control) {
                Err(StorageError::InvalidGroupName)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        None => Ok(None),
    }
}

fn validated_jump_host(jump: Option<&str>) -> Result<Option<String>, StorageError> {
    match jump {
        Some(j) => {
            let trimmed = j.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(
                    validate_jump_host(trimmed).map_err(StorageError::InvalidJumpHost)?,
                ))
            }
        }
        None => Ok(None),
    }
}

fn validated_tags(tags: &[String]) -> Result<String, StorageError> {
    let mut clean_tags = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().any(|c| c.is_control() || c == ',') {
            return Err(StorageError::InvalidTag);
        }
        clean_tags.push(trimmed.to_string());
    }
    Ok(clean_tags.join(","))
}

fn parse_tags(tags_str: &str) -> Vec<String> {
    tags_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Checks whether a given database path is an encrypted, protected vault database.
pub fn is_protected_vault_path(path: &str) -> bool {
    if path == ":memory:" {
        return false;
    }
    let p = std::path::Path::new(path);
    // 1. Located in a vaults/ directory or named default.db
    let is_in_vaults_dir = p
        .parent()
        .is_some_and(|parent| parent.file_name() == Some(std::ffi::OsStr::new("vaults")));
    let is_default_db = p.file_name() == Some(std::ffi::OsStr::new("default.db"));
    if is_in_vaults_dir || is_default_db {
        return true;
    }
    // 2. Has an associated manifest file on disk
    let parent = p.parent().unwrap_or(p);
    let root = if parent.file_name() == Some(std::ffi::OsStr::new("vaults")) {
        parent.parent().unwrap_or(parent)
    } else {
        parent
    };
    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
        let manifest_vid = if stem == "default" {
            LEGACY_LOCAL_VAULT_ID
        } else {
            stem
        };
        let manifest_path = root
            .join("vault_manifests")
            .join(format!("{manifest_vid}.manifest.json"));
        if manifest_path.exists() {
            return true;
        }
    }
    // 3. File exists on disk and is non-empty, but does NOT start with standard SQLite format header
    if let Ok(metadata) = std::fs::metadata(p) {
        if metadata.len() > 0 {
            if let Ok(mut file) = std::fs::File::open(p) {
                use std::io::Read;
                let mut header = [0u8; 16];
                if file.read_exact(&mut header).is_ok() && &header != b"SQLite format 3\0" {
                    return true;
                }
            }
        }
    }
    false
}

/// Configuration options for restoring a synchronized vault on a second device.
pub struct RestoreVaultOptions<'a> {
    pub vault_id: &'a shellumi_core::VaultId,
    pub vault_name: &'a str,
    pub password: &'a str,
    pub target_db_path: &'a std::path::Path,
    pub manifests_dir: &'a std::path::Path,
    pub cred_store: Option<&'a dyn shellumi_core::CredentialStore>,
    pub remember_in_keychain: bool,
}

impl HostStore {
    /// Open a local database path, or `:memory:` for an ephemeral database.
    pub async fn open(path: &str) -> Result<Self, StorageError> {
        Self::open_with_key(path, None).await
    }

    /// Open a database path with optional Data Encryption Key (DEK).
    pub async fn open_with_key(
        path: &str,
        key: Option<&shellumi_core::VaultKey>,
    ) -> Result<Self, StorageError> {
        tracing::debug!(
            path,
            encrypted = key.is_some(),
            "Opening HostStore database"
        );
        if is_protected_vault_path(path) && key.is_none() {
            tracing::warn!(
                path,
                "Rejected unencrypted open for protected vault database"
            );
            return Err(StorageError::ItemNotFound(
                "Vault database is encrypted and requires a valid Data Encryption Key (DEK) to open"
                    .to_string(),
            ));
        }
        let builder = Builder::new_local(path);
        let builder = if let Some(k) = key {
            builder
                .experimental_encryption(true)
                .with_encryption(turso::core::EncryptionOpts {
                    cipher: "aes256gcm".to_string(),
                    hexkey: hex::encode(k.expose_bytes()),
                })
        } else {
            builder
        };
        let database = builder.build().await?;
        let mut connection = database.connect()?;
        connection.execute("PRAGMA foreign_keys = ON", ()).await?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let mut rows = tx.query("PRAGMA user_version", ()).await?;
        let version: i64 = rows
            .next()
            .await?
            .ok_or(StorageError::MissingSchemaVersion)?
            .get(0)?;
        drop(rows);
        if !(0..=SCHEMA_VERSION).contains(&version) {
            tracing::error!(
                version,
                max_version = SCHEMA_VERSION,
                "Unsupported database schema version"
            );
            return Err(StorageError::UnsupportedSchema(version));
        }
        if version < SCHEMA_VERSION {
            tracing::info!(
                from_version = version,
                to_version = SCHEMA_VERSION,
                "Applying database schema migrations"
            );
        }
        if version == 0 {
            tx.execute(
                "CREATE TABLE IF NOT EXISTS identities (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE CHECK(length(trim(name)) > 0),
                    username TEXT NOT NULL CHECK(length(trim(username)) > 0),
                    auth_method TEXT NOT NULL,
                    password_secret_ref TEXT,
                    key_path TEXT,
                    passphrase_secret_ref TEXT,
                    certificate_path TEXT,
                    tags TEXT NOT NULL DEFAULT '[]',
                    comment TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE snippets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    command TEXT NOT NULL CHECK(length(command) > 0),
                    description TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    variables TEXT NOT NULL DEFAULT '[]',
                    vault_id TEXT NOT NULL DEFAULT 'legacy-local',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS snippets_vault_id_idx ON snippets(vault_id)",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE hosts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    hostname TEXT NOT NULL CHECK(length(hostname) > 0),
                    username TEXT NOT NULL CHECK(length(username) > 0),
                    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
                    group_name TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    jump_host TEXT,
                    startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL,
                    protocol TEXT NOT NULL DEFAULT 'ssh',
                    protocol_config TEXT,
                    identity_id INTEGER REFERENCES identities(id) ON DELETE SET NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE groups (
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    default_username TEXT,
                    default_port INTEGER CHECK(default_port IS NULL OR (default_port BETWEEN 1 AND 65535)),
                    tags TEXT NOT NULL DEFAULT '',
                    identity_id INTEGER REFERENCES identities(id) ON DELETE SET NULL,
                    vault_id TEXT NOT NULL DEFAULT 'legacy-local',
                    PRIMARY KEY (name, vault_id)
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS groups_vault_id_idx ON groups(vault_id)",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE tunnels (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    forwarding_type TEXT NOT NULL,
                    bind_address TEXT,
                    bind_port INTEGER NOT NULL CHECK(bind_port BETWEEN 1 AND 65535),
                    target_host TEXT,
                    target_port INTEGER CHECK(target_port IS NULL OR (target_port BETWEEN 1 AND 65535)),
                    auto_start INTEGER NOT NULL DEFAULT 0,
                    vault_id TEXT NOT NULL DEFAULT 'legacy-local',
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE CASCADE
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS tunnels_vault_id_idx ON tunnels(vault_id)",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE transfer_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    direction TEXT NOT NULL CHECK(direction IN ('upload', 'download')),
                    local_path TEXT NOT NULL CHECK(length(local_path) > 0),
                    remote_path TEXT NOT NULL CHECK(length(remote_path) > 0),
                    bytes_transferred INTEGER NOT NULL CHECK(bytes_transferred >= 0),
                    total_bytes INTEGER NOT NULL CHECK(total_bytes >= 0),
                    status TEXT NOT NULL CHECK(length(status) > 0),
                    error_message TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    completed_at TEXT,
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE SET NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE sync_snapshots (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    snapshot_id TEXT NOT NULL CHECK(length(snapshot_id) > 0),
                    vault_json TEXT NOT NULL CHECK(length(vault_json) > 0),
                    record_count INTEGER NOT NULL CHECK(record_count >= 0),
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    vault_id TEXT NOT NULL DEFAULT 'legacy-local',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS workspaces_vault_id_idx ON workspaces(vault_id)",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE shared_vaults (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    current_epoch INTEGER NOT NULL DEFAULT 1 CHECK(current_epoch >= 1),
                    owner_id TEXT NOT NULL CHECK(length(trim(owner_id)) > 0),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE team_members (
                    id TEXT PRIMARY KEY,
                    vault_id TEXT NOT NULL REFERENCES shared_vaults(id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL CHECK(length(trim(user_id)) > 0),
                    display_name TEXT NOT NULL CHECK(length(trim(display_name)) > 0),
                    public_key TEXT NOT NULL,
                    role TEXT NOT NULL,
                    enrolled_at TEXT NOT NULL,
                    invited_by TEXT NOT NULL,
                    revoked INTEGER NOT NULL DEFAULT 0,
                    revoked_at TEXT
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE sharing_audit_log (
                    id TEXT PRIMARY KEY,
                    vault_id TEXT NOT NULL REFERENCES shared_vaults(id) ON DELETE CASCADE,
                    action TEXT NOT NULL,
                    actor_id TEXT NOT NULL,
                    target_id TEXT,
                    details TEXT,
                    timestamp_utc TEXT NOT NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE shared_session_logs (
                    id TEXT PRIMARY KEY,
                    vault_id TEXT NOT NULL REFERENCES shared_vaults(id) ON DELETE CASCADE,
                    host_name TEXT NOT NULL,
                    author_id TEXT NOT NULL,
                    started_at TEXT NOT NULL,
                    ended_at TEXT,
                    retention_days INTEGER NOT NULL DEFAULT 30,
                    comments_json TEXT NOT NULL DEFAULT '[]',
                    bookmarks_json TEXT NOT NULL DEFAULT '[]'
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE cloud_providers (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    provider_kind TEXT NOT NULL,
                    config_json TEXT NOT NULL DEFAULT '{}',
                    sync_interval_mins INTEGER NOT NULL DEFAULT 60,
                    last_synced_at INTEGER,
                    auto_sync INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE discovered_hosts (
                    id TEXT PRIMARY KEY,
                    provider_id TEXT NOT NULL REFERENCES cloud_providers(id) ON DELETE CASCADE,
                    external_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    hostname TEXT NOT NULL,
                    port INTEGER NOT NULL DEFAULT 22 CHECK(port BETWEEN 1 AND 65535),
                    username TEXT,
                    state TEXT NOT NULL DEFAULT 'active',
                    region TEXT,
                    group_path TEXT,
                    tags_json TEXT NOT NULL DEFAULT '{}',
                    metadata_json TEXT NOT NULL DEFAULT '{}',
                    imported_host_id INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
                    last_seen_at INTEGER NOT NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE session_recordings (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    host_id INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
                    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
                    duration_secs REAL NOT NULL CHECK(duration_secs >= 0.0),
                    event_count INTEGER NOT NULL CHECK(event_count >= 0),
                    byte_size INTEGER NOT NULL CHECK(byte_size >= 0),
                    cast_data TEXT NOT NULL,
                    recorded_at TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute("PRAGMA user_version = 14", ()).await?;
        } else if version == 1 {
            // Migrate schema version 1 to 8
            tx.execute("ALTER TABLE hosts ADD COLUMN group_name TEXT", ())
                .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN tags TEXT NOT NULL DEFAULT ''",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN jump_host TEXT", ())
                .await?;
            tx.execute(
                "CREATE TABLE groups (
                    name TEXT PRIMARY KEY NOT NULL CHECK(length(name) > 0),
                    default_username TEXT,
                    default_port INTEGER CHECK(default_port IS NULL OR (default_port BETWEEN 1 AND 65535)),
                    tags TEXT NOT NULL DEFAULT ''
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE tunnels (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    forwarding_type TEXT NOT NULL,
                    bind_address TEXT,
                    bind_port INTEGER NOT NULL CHECK(bind_port BETWEEN 1 AND 65535),
                    target_host TEXT,
                    target_port INTEGER CHECK(target_port IS NULL OR (target_port BETWEEN 1 AND 65535)),
                    auto_start INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE CASCADE
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE transfer_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    direction TEXT NOT NULL CHECK(direction IN ('upload', 'download')),
                    local_path TEXT NOT NULL CHECK(length(local_path) > 0),
                    remote_path TEXT NOT NULL CHECK(length(remote_path) > 0),
                    bytes_transferred INTEGER NOT NULL CHECK(bytes_transferred >= 0),
                    total_bytes INTEGER NOT NULL CHECK(total_bytes >= 0),
                    status TEXT NOT NULL CHECK(length(status) > 0),
                    error_message TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    completed_at TEXT,
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE SET NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE snippets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    command TEXT NOT NULL CHECK(length(command) > 0),
                    description TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    variables TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE sync_snapshots (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    snapshot_id TEXT NOT NULL CHECK(length(snapshot_id) > 0),
                    vault_json TEXT NOT NULL CHECK(length(vault_json) > 0),
                    record_count INTEGER NOT NULL CHECK(record_count >= 0),
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN protocol TEXT NOT NULL DEFAULT 'ssh'",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN protocol_config TEXT", ())
                .await?;
            tx.execute("PRAGMA user_version = 10", ()).await?;
        } else if version == 2 {
            // Migrate schema version 2 to 8
            tx.execute("ALTER TABLE hosts ADD COLUMN jump_host TEXT", ())
                .await?;
            tx.execute(
                "CREATE TABLE groups (
                    name TEXT PRIMARY KEY NOT NULL CHECK(length(name) > 0),
                    default_username TEXT,
                    default_port INTEGER CHECK(default_port IS NULL OR (default_port BETWEEN 1 AND 65535)),
                    tags TEXT NOT NULL DEFAULT ''
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE tunnels (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    forwarding_type TEXT NOT NULL,
                    bind_address TEXT,
                    bind_port INTEGER NOT NULL CHECK(bind_port BETWEEN 1 AND 65535),
                    target_host TEXT,
                    target_port INTEGER CHECK(target_port IS NULL OR (target_port BETWEEN 1 AND 65535)),
                    auto_start INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE CASCADE
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE transfer_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    direction TEXT NOT NULL CHECK(direction IN ('upload', 'download')),
                    local_path TEXT NOT NULL CHECK(length(local_path) > 0),
                    remote_path TEXT NOT NULL CHECK(length(remote_path) > 0),
                    bytes_transferred INTEGER NOT NULL CHECK(bytes_transferred >= 0),
                    total_bytes INTEGER NOT NULL CHECK(total_bytes >= 0),
                    status TEXT NOT NULL CHECK(length(status) > 0),
                    error_message TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    completed_at TEXT,
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE SET NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE snippets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    command TEXT NOT NULL CHECK(length(command) > 0),
                    description TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    variables TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE sync_snapshots (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    snapshot_id TEXT NOT NULL CHECK(length(snapshot_id) > 0),
                    vault_json TEXT NOT NULL CHECK(length(vault_json) > 0),
                    record_count INTEGER NOT NULL CHECK(record_count >= 0),
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN protocol TEXT NOT NULL DEFAULT 'ssh'",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN protocol_config TEXT", ())
                .await?;
            tx.execute("PRAGMA user_version = 10", ()).await?;
        } else if version == 3 {
            // Migrate schema version 3 to 8
            tx.execute("ALTER TABLE hosts ADD COLUMN jump_host TEXT", ())
                .await?;
            tx.execute(
                "CREATE TABLE tunnels (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    forwarding_type TEXT NOT NULL,
                    bind_address TEXT,
                    bind_port INTEGER NOT NULL CHECK(bind_port BETWEEN 1 AND 65535),
                    target_host TEXT,
                    target_port INTEGER CHECK(target_port IS NULL OR (target_port BETWEEN 1 AND 65535)),
                    auto_start INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE CASCADE
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE transfer_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    direction TEXT NOT NULL CHECK(direction IN ('upload', 'download')),
                    local_path TEXT NOT NULL CHECK(length(local_path) > 0),
                    remote_path TEXT NOT NULL CHECK(length(remote_path) > 0),
                    bytes_transferred INTEGER NOT NULL CHECK(bytes_transferred >= 0),
                    total_bytes INTEGER NOT NULL CHECK(total_bytes >= 0),
                    status TEXT NOT NULL CHECK(length(status) > 0),
                    error_message TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    completed_at TEXT,
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE SET NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE snippets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    command TEXT NOT NULL CHECK(length(command) > 0),
                    description TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    variables TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE sync_snapshots (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    snapshot_id TEXT NOT NULL CHECK(length(snapshot_id) > 0),
                    vault_json TEXT NOT NULL CHECK(length(vault_json) > 0),
                    record_count INTEGER NOT NULL CHECK(record_count >= 0),
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN protocol TEXT NOT NULL DEFAULT 'ssh'",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN protocol_config TEXT", ())
                .await?;
            tx.execute("PRAGMA user_version = 10", ()).await?;
        } else if version == 4 {
            // Migrate schema version 4 to 8
            tx.execute(
                "CREATE TABLE transfer_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    direction TEXT NOT NULL CHECK(direction IN ('upload', 'download')),
                    local_path TEXT NOT NULL CHECK(length(local_path) > 0),
                    remote_path TEXT NOT NULL CHECK(length(remote_path) > 0),
                    bytes_transferred INTEGER NOT NULL CHECK(bytes_transferred >= 0),
                    total_bytes INTEGER NOT NULL CHECK(total_bytes >= 0),
                    status TEXT NOT NULL CHECK(length(status) > 0),
                    error_message TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    completed_at TEXT,
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE SET NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE snippets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    command TEXT NOT NULL CHECK(length(command) > 0),
                    description TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    variables TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE sync_snapshots (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    snapshot_id TEXT NOT NULL CHECK(length(snapshot_id) > 0),
                    vault_json TEXT NOT NULL CHECK(length(vault_json) > 0),
                    record_count INTEGER NOT NULL CHECK(record_count >= 0),
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN protocol TEXT NOT NULL DEFAULT 'ssh'",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN protocol_config TEXT", ())
                .await?;
            tx.execute("PRAGMA user_version = 10", ()).await?;
        } else if version == 5 {
            // Migrate schema version 5 to 8
            tx.execute(
                "CREATE TABLE snippets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    command TEXT NOT NULL CHECK(length(command) > 0),
                    description TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    variables TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE sync_snapshots (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    snapshot_id TEXT NOT NULL CHECK(length(snapshot_id) > 0),
                    vault_json TEXT NOT NULL CHECK(length(vault_json) > 0),
                    record_count INTEGER NOT NULL CHECK(record_count >= 0),
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN protocol TEXT NOT NULL DEFAULT 'ssh'",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN protocol_config TEXT", ())
                .await?;
            tx.execute("PRAGMA user_version = 10", ()).await?;
        } else if version == 6 {
            // Migrate schema version 6 to 8
            tx.execute(
                "CREATE TABLE sync_snapshots (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    snapshot_id TEXT NOT NULL CHECK(length(snapshot_id) > 0),
                    vault_json TEXT NOT NULL CHECK(length(vault_json) > 0),
                    record_count INTEGER NOT NULL CHECK(record_count >= 0),
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN protocol TEXT NOT NULL DEFAULT 'ssh'",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN protocol_config TEXT", ())
                .await?;
            tx.execute("PRAGMA user_version = 10", ()).await?;
        } else if version == 7 {
            // Migrate schema version 7 to 9
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN protocol TEXT NOT NULL DEFAULT 'ssh'",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN protocol_config TEXT", ())
                .await?;
            tx.execute("PRAGMA user_version = 10", ()).await?;
        } else if version == 8 {
            // Migrate schema version 8 to 9
            tx.execute(
                "CREATE TABLE workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN protocol TEXT NOT NULL DEFAULT 'ssh'",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN protocol_config TEXT", ())
                .await?;
            tx.execute("PRAGMA user_version = 10", ()).await?;
        } else if version == 9 {
            // Migrate schema version 9 to 10
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN protocol TEXT NOT NULL DEFAULT 'ssh'",
                (),
            )
            .await?;
            tx.execute("ALTER TABLE hosts ADD COLUMN protocol_config TEXT", ())
                .await?;
            tx.execute("PRAGMA user_version = 10", ()).await?;
        }

        if version > 0 && version < 14 {
            if version < 12 {
                // Migrate schema to version 11: Shared Vaults & Team Collaboration
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS shared_vaults (
                        id TEXT PRIMARY KEY,
                        name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                        description TEXT,
                        current_epoch INTEGER NOT NULL DEFAULT 1 CHECK(current_epoch >= 1),
                        owner_id TEXT NOT NULL CHECK(length(trim(owner_id)) > 0),
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    )",
                    (),
                )
                .await?;
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS team_members (
                        id TEXT PRIMARY KEY,
                        vault_id TEXT NOT NULL REFERENCES shared_vaults(id) ON DELETE CASCADE,
                        user_id TEXT NOT NULL CHECK(length(trim(user_id)) > 0),
                        display_name TEXT NOT NULL CHECK(length(trim(display_name)) > 0),
                        public_key TEXT NOT NULL,
                        role TEXT NOT NULL,
                        enrolled_at TEXT NOT NULL,
                        invited_by TEXT NOT NULL,
                        revoked INTEGER NOT NULL DEFAULT 0,
                        revoked_at TEXT
                    )",
                    (),
                )
                .await?;
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS sharing_audit_log (
                        id TEXT PRIMARY KEY,
                        vault_id TEXT NOT NULL REFERENCES shared_vaults(id) ON DELETE CASCADE,
                        action TEXT NOT NULL,
                        actor_id TEXT NOT NULL,
                        target_id TEXT,
                        details TEXT,
                        timestamp_utc TEXT NOT NULL
                    )",
                    (),
                )
                .await?;
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS shared_session_logs (
                        id TEXT PRIMARY KEY,
                        vault_id TEXT NOT NULL REFERENCES shared_vaults(id) ON DELETE CASCADE,
                        host_name TEXT NOT NULL,
                        author_id TEXT NOT NULL,
                        started_at TEXT NOT NULL,
                        ended_at TEXT,
                        retention_days INTEGER NOT NULL DEFAULT 30,
                        comments_json TEXT NOT NULL DEFAULT '[]',
                        bookmarks_json TEXT NOT NULL DEFAULT '[]'
                    )",
                    (),
                )
                .await?;

                // Migrate schema to version 12: Cloud Providers & Infrastructure Discovery
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS cloud_providers (
                        id TEXT PRIMARY KEY,
                        name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                        provider_kind TEXT NOT NULL,
                        config_json TEXT NOT NULL DEFAULT '{}',
                        sync_interval_mins INTEGER NOT NULL DEFAULT 60,
                        last_synced_at INTEGER,
                        auto_sync INTEGER NOT NULL DEFAULT 0,
                        created_at INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL
                    )",
                    (),
                )
                .await?;
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS discovered_hosts (
                        id TEXT PRIMARY KEY,
                        provider_id TEXT NOT NULL REFERENCES cloud_providers(id) ON DELETE CASCADE,
                        external_id TEXT NOT NULL,
                        name TEXT NOT NULL,
                        hostname TEXT NOT NULL,
                        port INTEGER NOT NULL DEFAULT 22 CHECK(port BETWEEN 1 AND 65535),
                        username TEXT,
                        state TEXT NOT NULL DEFAULT 'active',
                        region TEXT,
                        group_path TEXT,
                        tags_json TEXT NOT NULL DEFAULT '{}',
                        metadata_json TEXT NOT NULL DEFAULT '{}',
                        imported_host_id INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
                        last_seen_at INTEGER NOT NULL
                    )",
                    (),
                )
                .await?;
            }

            if version < 13 {
                // Migrate schema to version 13: Terminal Session Recordings
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS session_recordings (
                        id TEXT PRIMARY KEY,
                        session_id TEXT NOT NULL,
                        host_id INTEGER REFERENCES hosts(id) ON DELETE SET NULL,
                        title TEXT NOT NULL CHECK(length(trim(title)) > 0),
                        duration_secs REAL NOT NULL CHECK(duration_secs >= 0.0),
                        event_count INTEGER NOT NULL CHECK(event_count >= 0),
                        byte_size INTEGER NOT NULL CHECK(byte_size >= 0),
                        cast_data TEXT NOT NULL,
                        recorded_at TEXT NOT NULL,
                        created_at TEXT NOT NULL DEFAULT (datetime('now'))
                    )",
                    (),
                )
                .await?;
            }

            if version < 14 {
                // Migrate schema to version 14: Reusable Identities & OpenSSH Certificates
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS identities (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT NOT NULL UNIQUE CHECK(length(trim(name)) > 0),
                        username TEXT NOT NULL CHECK(length(trim(username)) > 0),
                        auth_method TEXT NOT NULL,
                        password_secret_ref TEXT,
                        key_path TEXT,
                        passphrase_secret_ref TEXT,
                        certificate_path TEXT,
                        tags TEXT NOT NULL DEFAULT '[]',
                        comment TEXT,
                        created_at TEXT NOT NULL DEFAULT (datetime('now')),
                        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                    )",
                    (),
                )
                .await?;

                let mut has_host_identity_id = false;
                let mut h_rows = tx.query("PRAGMA table_info(hosts)", ()).await?;
                while let Some(row) = h_rows.next().await? {
                    let col: String = row.get(1)?;
                    if col == "identity_id" {
                        has_host_identity_id = true;
                        break;
                    }
                }
                if !has_host_identity_id {
                    tx.execute(
                        "ALTER TABLE hosts ADD COLUMN identity_id INTEGER REFERENCES identities(id) ON DELETE SET NULL",
                        (),
                    )
                    .await?;
                }

                let mut has_groups_table = false;
                let mut g_table_rows = tx
                    .query(
                        "SELECT name FROM sqlite_master WHERE type='table' AND name='groups'",
                        (),
                    )
                    .await?;
                if g_table_rows.next().await?.is_some() {
                    has_groups_table = true;
                }

                if has_groups_table {
                    let mut has_group_identity_id = false;
                    let mut g_rows = tx.query("PRAGMA table_info(groups)", ()).await?;
                    while let Some(row) = g_rows.next().await? {
                        let col: String = row.get(1)?;
                        if col == "identity_id" {
                            has_group_identity_id = true;
                            break;
                        }
                    }
                    if !has_group_identity_id {
                        tx.execute(
                            "ALTER TABLE groups ADD COLUMN identity_id INTEGER REFERENCES identities(id) ON DELETE SET NULL",
                            (),
                        )
                        .await?;
                    }
                } else {
                    tx.execute(
                        "CREATE TABLE IF NOT EXISTS groups (
                            name TEXT PRIMARY KEY NOT NULL CHECK(length(name) > 0),
                            default_username TEXT,
                            default_port INTEGER CHECK(default_port IS NULL OR (default_port BETWEEN 1 AND 65535)),
                            tags TEXT NOT NULL DEFAULT '',
                            identity_id INTEGER REFERENCES identities(id) ON DELETE SET NULL
                        )",
                        (),
                    )
                    .await?;
                }
            }
            tx.execute("PRAGMA user_version = 14", ()).await?;
        }
        if version < 15 {
            tx.execute(
                "CREATE TABLE vault_server_connections (
                    id TEXT PRIMARY KEY NOT NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE TABLE vault_contexts (
                    id TEXT PRIMARY KEY NOT NULL,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    server_connection_id TEXT REFERENCES vault_server_connections(id),
                    server_vault_id TEXT,
                    CHECK((server_connection_id IS NULL AND server_vault_id IS NULL) OR
                          (server_connection_id IS NOT NULL AND server_vault_id IS NOT NULL)),
                    UNIQUE(server_connection_id, server_vault_id)
                )",
                (),
            )
            .await?;
            tx.execute(
                "INSERT INTO vault_contexts (id, name) VALUES (?1, ?2)",
                (LEGACY_LOCAL_VAULT_ID, "Local"),
            )
            .await?;
            tx.execute("PRAGMA user_version = 15", ()).await?;
        }
        if version < 16 {
            let mut columns = tx.query("PRAGMA table_info(hosts)", ()).await?;
            let mut has_vault_id = false;
            while let Some(row) = columns.next().await? {
                let name: String = row.get(1)?;
                has_vault_id |= name == "vault_id";
            }
            drop(columns);
            if !has_vault_id {
                tx.execute(
                    "ALTER TABLE hosts ADD COLUMN vault_id TEXT NOT NULL DEFAULT 'legacy-local'",
                    (),
                )
                .await?;
            }
            tx.execute(
                "CREATE INDEX IF NOT EXISTS hosts_vault_id_idx ON hosts(vault_id)",
                (),
            )
            .await?;
            tx.execute("PRAGMA user_version = 16", ()).await?;
        }
        if version < 17 {
            let mut columns = tx.query("PRAGMA table_info(identities)", ()).await?;
            let mut has_vault_id = false;
            while let Some(row) = columns.next().await? {
                let name: String = row.get(1)?;
                has_vault_id |= name == "vault_id";
            }
            drop(columns);
            if !has_vault_id {
                tx.execute("ALTER TABLE identities ADD COLUMN vault_id TEXT NOT NULL DEFAULT 'legacy-local'", ()).await?;
            }
            tx.execute(
                "CREATE INDEX IF NOT EXISTS identities_vault_id_idx ON identities(vault_id)",
                (),
            )
            .await?;
            tx.execute("PRAGMA user_version = 17", ()).await?;
        }
        if version < 18 {
            let mut columns = tx
                .query("PRAGMA table_info(vault_server_connections)", ())
                .await?;
            let mut has_base_url = false;
            let mut has_auth_token = false;
            let mut has_refresh_token = false;
            let mut has_device_id = false;
            let mut has_device_signing_key = false;
            let mut has_created_at = false;
            let mut has_updated_at = false;
            while let Some(row) = columns.next().await? {
                let col_name: String = row.get(1)?;
                has_base_url |= col_name == "base_url";
                has_auth_token |= col_name == "auth_token";
                has_refresh_token |= col_name == "refresh_token";
                has_device_id |= col_name == "device_id";
                has_device_signing_key |= col_name == "device_signing_key";
                has_created_at |= col_name == "created_at";
                has_updated_at |= col_name == "updated_at";
            }
            drop(columns);

            if !has_base_url {
                tx.execute(
                    "ALTER TABLE vault_server_connections ADD COLUMN base_url TEXT",
                    (),
                )
                .await?;
            }
            if !has_auth_token {
                tx.execute(
                    "ALTER TABLE vault_server_connections ADD COLUMN auth_token TEXT",
                    (),
                )
                .await?;
            }
            if !has_refresh_token {
                tx.execute(
                    "ALTER TABLE vault_server_connections ADD COLUMN refresh_token TEXT",
                    (),
                )
                .await?;
            }
            if !has_device_id {
                tx.execute(
                    "ALTER TABLE vault_server_connections ADD COLUMN device_id TEXT",
                    (),
                )
                .await?;
            }
            if !has_device_signing_key {
                tx.execute(
                    "ALTER TABLE vault_server_connections ADD COLUMN device_signing_key TEXT",
                    (),
                )
                .await?;
            }
            if !has_created_at {
                tx.execute(
                    "ALTER TABLE vault_server_connections ADD COLUMN created_at INTEGER",
                    (),
                )
                .await?;
            }
            if !has_updated_at {
                tx.execute(
                    "ALTER TABLE vault_server_connections ADD COLUMN updated_at INTEGER",
                    (),
                )
                .await?;
            }

            tx.execute(
                "CREATE TABLE IF NOT EXISTS vault_sync_state (
                    vault_id TEXT PRIMARY KEY NOT NULL,
                    server_sequence INTEGER NOT NULL DEFAULT 0,
                    last_cursor TEXT,
                    last_synced_at INTEGER,
                    status TEXT NOT NULL DEFAULT 'idle',
                    error_message TEXT
                )",
                (),
            )
            .await?;

            tx.execute(
                "CREATE TABLE IF NOT EXISTS vault_mutations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    vault_id TEXT NOT NULL,
                    object_id TEXT NOT NULL,
                    entity_type TEXT NOT NULL,
                    expected_revision INTEGER NOT NULL DEFAULT 0,
                    deleted INTEGER NOT NULL DEFAULT 0,
                    payload_json TEXT,
                    created_at INTEGER NOT NULL
                )",
                (),
            )
            .await?;
            tx.execute(
                "CREATE INDEX IF NOT EXISTS vault_mutations_vault_idx ON vault_mutations(vault_id)",
                (),
            )
            .await?;
            tx.execute("PRAGMA user_version = 18", ()).await?;
        }
        if version < 19 {
            // 1. snippets
            tx.execute(
                "CREATE TABLE IF NOT EXISTS snippets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    command TEXT NOT NULL CHECK(length(command) > 0),
                    description TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    variables TEXT NOT NULL DEFAULT '[]',
                    vault_id TEXT NOT NULL DEFAULT 'legacy-local',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            let mut columns = tx.query("PRAGMA table_info(snippets)", ()).await?;
            let mut has_vault_id = false;
            while let Some(row) = columns.next().await? {
                let name: String = row.get(1)?;
                has_vault_id |= name == "vault_id";
            }
            drop(columns);
            if !has_vault_id {
                tx.execute(
                    "ALTER TABLE snippets ADD COLUMN vault_id TEXT NOT NULL DEFAULT 'legacy-local'",
                    (),
                )
                .await?;
            }
            tx.execute(
                "CREATE INDEX IF NOT EXISTS snippets_vault_id_idx ON snippets(vault_id)",
                (),
            )
            .await?;

            // 2. tunnels
            tx.execute(
                "CREATE TABLE IF NOT EXISTS tunnels (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    host_id INTEGER,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    forwarding_type TEXT NOT NULL,
                    bind_address TEXT,
                    bind_port INTEGER NOT NULL CHECK(bind_port BETWEEN 1 AND 65535),
                    target_host TEXT,
                    target_port INTEGER CHECK(target_port IS NULL OR (target_port BETWEEN 1 AND 65535)),
                    auto_start INTEGER NOT NULL DEFAULT 0,
                    vault_id TEXT NOT NULL DEFAULT 'legacy-local',
                    FOREIGN KEY (host_id) REFERENCES hosts(id) ON DELETE CASCADE
                )",
                (),
            )
            .await?;
            let mut columns = tx.query("PRAGMA table_info(tunnels)", ()).await?;
            let mut has_vault_id = false;
            while let Some(row) = columns.next().await? {
                let name: String = row.get(1)?;
                has_vault_id |= name == "vault_id";
            }
            drop(columns);
            if !has_vault_id {
                tx.execute(
                    "ALTER TABLE tunnels ADD COLUMN vault_id TEXT NOT NULL DEFAULT 'legacy-local'",
                    (),
                )
                .await?;
            }
            tx.execute(
                "CREATE INDEX IF NOT EXISTS tunnels_vault_id_idx ON tunnels(vault_id)",
                (),
            )
            .await?;

            // 3. workspaces
            tx.execute(
                "CREATE TABLE IF NOT EXISTS workspaces (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
                    description TEXT,
                    layout TEXT NOT NULL DEFAULT 'single',
                    sessions_json TEXT NOT NULL DEFAULT '[]',
                    tags TEXT NOT NULL DEFAULT '[]',
                    vault_id TEXT NOT NULL DEFAULT 'legacy-local',
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;
            let mut columns = tx.query("PRAGMA table_info(workspaces)", ()).await?;
            let mut has_vault_id = false;
            while let Some(row) = columns.next().await? {
                let name: String = row.get(1)?;
                has_vault_id |= name == "vault_id";
            }
            drop(columns);
            if !has_vault_id {
                tx.execute(
                    "ALTER TABLE workspaces ADD COLUMN vault_id TEXT NOT NULL DEFAULT 'legacy-local'",
                    (),
                )
                .await?;
            }
            tx.execute(
                "CREATE INDEX IF NOT EXISTS workspaces_vault_id_idx ON workspaces(vault_id)",
                (),
            )
            .await?;

            tx.execute("PRAGMA user_version = 19", ()).await?;
        }
        if version < 20 {
            // 1. vault_contexts additions
            let mut columns = tx.query("PRAGMA table_info(vault_contexts)", ()).await?;
            let mut has_recovery_envelope = false;
            let mut has_is_locked = false;
            while let Some(row) = columns.next().await? {
                let name: String = row.get(1)?;
                has_recovery_envelope |= name == "recovery_envelope";
                has_is_locked |= name == "is_locked";
            }
            drop(columns);
            if !has_recovery_envelope {
                tx.execute(
                    "ALTER TABLE vault_contexts ADD COLUMN recovery_envelope TEXT",
                    (),
                )
                .await?;
            }
            if !has_is_locked {
                tx.execute(
                    "ALTER TABLE vault_contexts ADD COLUMN is_locked INTEGER NOT NULL DEFAULT 0",
                    (),
                )
                .await?;
            }

            // 2. groups scoped by vault_id with composite primary key (name, vault_id)
            let mut columns = tx.query("PRAGMA table_info(groups)", ()).await?;
            let mut has_groups_vault_id = false;
            while let Some(row) = columns.next().await? {
                let name: String = row.get(1)?;
                has_groups_vault_id |= name == "vault_id";
            }
            drop(columns);
            if !has_groups_vault_id {
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS groups_v20 (
                        name TEXT NOT NULL CHECK(length(name) > 0),
                        default_username TEXT,
                        default_port INTEGER CHECK(default_port IS NULL OR (default_port BETWEEN 1 AND 65535)),
                        tags TEXT NOT NULL DEFAULT '',
                        identity_id INTEGER REFERENCES identities(id) ON DELETE SET NULL,
                        vault_id TEXT NOT NULL DEFAULT 'legacy-local',
                        PRIMARY KEY (name, vault_id)
                    )",
                    (),
                )
                .await?;
                tx.execute(
                    "INSERT OR IGNORE INTO groups_v20 (name, default_username, default_port, tags, identity_id, vault_id)
                     SELECT name, default_username, default_port, tags, identity_id, 'legacy-local' FROM groups",
                    (),
                )
                .await?;
                tx.execute("DROP TABLE groups", ()).await?;
                tx.execute("ALTER TABLE groups_v20 RENAME TO groups", ())
                    .await?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS groups_vault_id_idx ON groups(vault_id)",
                    (),
                )
                .await?;
            }

            // 3. session_recordings scoped by vault_id
            let mut columns = tx
                .query("PRAGMA table_info(session_recordings)", ())
                .await?;
            let mut has_rec_vault_id = false;
            while let Some(row) = columns.next().await? {
                let name: String = row.get(1)?;
                has_rec_vault_id |= name == "vault_id";
            }
            drop(columns);
            if !has_rec_vault_id {
                tx.execute(
                    "ALTER TABLE session_recordings ADD COLUMN vault_id TEXT NOT NULL DEFAULT 'legacy-local'",
                    (),
                )
                .await?;
            }
            tx.execute(
                "CREATE INDEX IF NOT EXISTS session_recordings_vault_id_idx ON session_recordings(vault_id)",
                (),
            )
            .await?;

            // 4. vault_directories table
            tx.execute(
                "CREATE TABLE IF NOT EXISTS vault_directories (
                    id TEXT PRIMARY KEY NOT NULL,
                    encrypted_data TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                (),
            )
            .await?;

            tx.execute("PRAGMA user_version = 20", ()).await?;
        }
        if version < 21 {
            // 1. vault_server_connections: server_instance_id and device_agreement_key
            let mut columns = tx
                .query("PRAGMA table_info(vault_server_connections)", ())
                .await?;
            let mut has_server_instance_id = false;
            let mut has_device_agreement_key = false;
            while let Some(row) = columns.next().await? {
                let col_name: String = row.get(1)?;
                has_server_instance_id |= col_name == "server_instance_id";
                has_device_agreement_key |= col_name == "device_agreement_key";
            }
            drop(columns);

            if !has_server_instance_id {
                tx.execute(
                    "ALTER TABLE vault_server_connections ADD COLUMN server_instance_id TEXT",
                    (),
                )
                .await?;
            }
            if !has_device_agreement_key {
                tx.execute(
                    "ALTER TABLE vault_server_connections ADD COLUMN device_agreement_key TEXT",
                    (),
                )
                .await?;
            }

            // 2. vault_contexts: active_key_epoch, dek_envelope, sync_provider
            let mut columns = tx.query("PRAGMA table_info(vault_contexts)", ()).await?;
            let mut has_active_key_epoch = false;
            let mut has_dek_envelope = false;
            let mut has_sync_provider = false;
            while let Some(row) = columns.next().await? {
                let col_name: String = row.get(1)?;
                has_active_key_epoch |= col_name == "active_key_epoch";
                has_dek_envelope |= col_name == "dek_envelope";
                has_sync_provider |= col_name == "sync_provider";
            }
            drop(columns);

            if !has_active_key_epoch {
                tx.execute(
                    "ALTER TABLE vault_contexts ADD COLUMN active_key_epoch INTEGER NOT NULL DEFAULT 1",
                    (),
                )
                .await?;
            }
            if !has_dek_envelope {
                tx.execute(
                    "ALTER TABLE vault_contexts ADD COLUMN dek_envelope TEXT",
                    (),
                )
                .await?;
            }
            if !has_sync_provider {
                tx.execute(
                    "ALTER TABLE vault_contexts ADD COLUMN sync_provider TEXT NOT NULL DEFAULT 'local'",
                    (),
                )
                .await?;
            }

            // 3. vault_mutations: idempotency_key, key_epoch, device_counter
            let mut columns = tx.query("PRAGMA table_info(vault_mutations)", ()).await?;
            let mut has_idempotency_key = false;
            let mut has_key_epoch = false;
            let mut has_device_counter = false;
            while let Some(row) = columns.next().await? {
                let col_name: String = row.get(1)?;
                has_idempotency_key |= col_name == "idempotency_key";
                has_key_epoch |= col_name == "key_epoch";
                has_device_counter |= col_name == "device_counter";
            }
            drop(columns);

            if !has_idempotency_key {
                tx.execute(
                    "ALTER TABLE vault_mutations ADD COLUMN idempotency_key TEXT",
                    (),
                )
                .await?;
            }
            if !has_key_epoch {
                tx.execute(
                    "ALTER TABLE vault_mutations ADD COLUMN key_epoch INTEGER NOT NULL DEFAULT 1",
                    (),
                )
                .await?;
            }
            if !has_device_counter {
                tx.execute(
                    "ALTER TABLE vault_mutations ADD COLUMN device_counter INTEGER NOT NULL DEFAULT 1",
                    (),
                )
                .await?;
            }

            // 4. vault_sync_state: active_key_epoch, conflicted_object_id, conflict_details
            let mut columns = tx.query("PRAGMA table_info(vault_sync_state)", ()).await?;
            let mut has_state_epoch = false;
            let mut has_conflicted_obj = false;
            let mut has_conflict_details = false;
            while let Some(row) = columns.next().await? {
                let col_name: String = row.get(1)?;
                has_state_epoch |= col_name == "active_key_epoch";
                has_conflicted_obj |= col_name == "conflicted_object_id";
                has_conflict_details |= col_name == "conflict_details";
            }
            drop(columns);

            if !has_state_epoch {
                tx.execute(
                    "ALTER TABLE vault_sync_state ADD COLUMN active_key_epoch INTEGER NOT NULL DEFAULT 1",
                    (),
                )
                .await?;
            }
            if !has_conflicted_obj {
                tx.execute(
                    "ALTER TABLE vault_sync_state ADD COLUMN conflicted_object_id TEXT",
                    (),
                )
                .await?;
            }
            if !has_conflict_details {
                tx.execute(
                    "ALTER TABLE vault_sync_state ADD COLUMN conflict_details TEXT",
                    (),
                )
                .await?;
            }

            tx.execute("PRAGMA user_version = 21", ()).await?;
        }
        if version < 22 {
            let mut columns = tx.query("PRAGMA table_info(vault_contexts)", ()).await?;
            let mut has_sync_enabled = false;
            while let Some(row) = columns.next().await? {
                let name: String = row.get(1)?;
                has_sync_enabled |= name == "sync_enabled";
            }
            drop(columns);
            if !has_sync_enabled {
                tx.execute(
                    "ALTER TABLE vault_contexts ADD COLUMN sync_enabled INTEGER NOT NULL DEFAULT 0",
                    (),
                )
                .await?;
            }
            tx.execute("PRAGMA user_version = 22", ()).await?;
        }
        tx.commit().await?;
        tracing::debug!(
            path,
            version = SCHEMA_VERSION,
            "Database opened and schema verified"
        );
        Ok(Self {
            _database: database,
            connection,
            host_vault_id: shellumi_core::VaultId::new(LEGACY_LOCAL_VAULT_ID)
                .map_err(StorageError::InvalidVaultIdentity)?,
            db_path: Some(path.to_string()),
        })
    }

    /// Returns the database path if opened with a filesystem path.
    pub fn db_path(&self) -> Option<&str> {
        self.db_path.as_deref()
    }

    /// Sets the active scoped vault ID for this store instance.
    pub fn set_vault_id(&mut self, vault_id: shellumi_core::VaultId) {
        self.host_vault_id = vault_id;
    }

    /// Returns the currently scoped vault ID.
    pub fn vault_id(&self) -> &shellumi_core::VaultId {
        &self.host_vault_id
    }

    /// Register an identity only; this does not configure or authenticate a server.
    pub async fn register_vault_server(
        &mut self,
        server: &shellumi_core::ServerConnectionId,
    ) -> Result<(), StorageError> {
        self.connection
            .execute(
                "INSERT INTO vault_server_connections (id) VALUES (?1)",
                (server.as_str(),),
            )
            .await?;
        Ok(())
    }

    /// Store a context in the registry. Existing data is not yet vault-scoped.
    pub async fn register_vault_context(
        &mut self,
        name: &str,
        context: &shellumi_core::VaultContext,
    ) -> Result<(), StorageError> {
        let name = validated_name(name)?;
        let (server, remote) = match context.location() {
            shellumi_core::VaultLocation::Local => (None, None),
            shellumi_core::VaultLocation::Remote {
                server,
                server_vault_id,
            } => (Some(server.as_str()), Some(server_vault_id.as_str())),
        };
        self.connection.execute(
            "INSERT INTO vault_contexts (id, name, server_connection_id, server_vault_id) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, server_connection_id = excluded.server_connection_id, server_vault_id = excluded.server_vault_id",
            (context.id().as_str(), name, server, remote),
        ).await?;
        Ok(())
    }

    pub async fn list_vault_contexts(
        &self,
    ) -> Result<Vec<(String, shellumi_core::VaultContext)>, StorageError> {
        use shellumi_core::{ServerConnectionId, VaultContext, VaultId, VaultLocation};
        let mut rows = self.connection.query(
            "SELECT id, name, server_connection_id, server_vault_id FROM vault_contexts ORDER BY name, id", (),
        ).await?;
        let mut contexts = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let server: Option<String> = row.get(2)?;
            let remote: Option<String> = row.get(3)?;
            let location = match (server, remote) {
                (None, None) => VaultLocation::Local,
                (Some(server), Some(remote)) => VaultLocation::Remote {
                    server: ServerConnectionId::new(server)
                        .map_err(StorageError::InvalidVaultIdentity)?,
                    server_vault_id: VaultId::new(remote)
                        .map_err(StorageError::InvalidVaultIdentity)?,
                },
                _ => return Err(StorageError::Serialization("invalid vault location".into())),
            };
            contexts.push((
                name,
                VaultContext::new(
                    VaultId::new(id).map_err(StorageError::InvalidVaultIdentity)?,
                    location,
                ),
            ));
        }
        Ok(contexts)
    }

    pub async fn get_vault_context(
        &self,
        id: &str,
    ) -> Result<Option<shellumi_core::VaultContext>, StorageError> {
        use shellumi_core::{ServerConnectionId, VaultContext, VaultId, VaultLocation};
        let mut rows = self.connection.query(
            "SELECT id, name, server_connection_id, server_vault_id FROM vault_contexts WHERE id = ?1", (id,),
        ).await?;
        if let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let _name: String = row.get(1)?;
            let server: Option<String> = row.get(2)?;
            let remote: Option<String> = row.get(3)?;
            let location = match (server, remote) {
                (None, None) => VaultLocation::Local,
                (Some(server), Some(remote)) => VaultLocation::Remote {
                    server: ServerConnectionId::new(server)
                        .map_err(StorageError::InvalidVaultIdentity)?,
                    server_vault_id: VaultId::new(remote)
                        .map_err(StorageError::InvalidVaultIdentity)?,
                },
                _ => return Err(StorageError::Serialization("invalid vault location".into())),
            };
            Ok(Some(VaultContext::new(
                VaultId::new(id).map_err(StorageError::InvalidVaultIdentity)?,
                location,
            )))
        } else {
            Ok(None)
        }
    }

    /// Saves a recovery envelope for a vault context.
    pub async fn save_vault_recovery_envelope(
        &mut self,
        vault_id: &str,
        recovery_envelope: &str,
    ) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "UPDATE vault_contexts SET recovery_envelope = ?1 WHERE id = ?2",
                (recovery_envelope, vault_id),
            )
            .await?;
        Ok(rows > 0)
    }

    /// Retrieves a recovery envelope for a vault context.
    pub async fn get_vault_recovery_envelope(
        &self,
        vault_id: &str,
    ) -> Result<Option<String>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT recovery_envelope FROM vault_contexts WHERE id = ?1",
                (vault_id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let env: Option<String> = row.get(0)?;
            Ok(env)
        } else {
            Ok(None)
        }
    }

    /// Checks whether Google Drive synchronization is enabled for a given vault context.
    pub async fn is_vault_sync_enabled(&self, vault_id: &str) -> Result<bool, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT sync_enabled FROM vault_contexts WHERE id = ?1",
                (vault_id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let val: i64 = row.get(0)?;
            Ok(val > 0)
        } else {
            Ok(false)
        }
    }

    /// Sets whether Google Drive synchronization is enabled for a given vault context.
    pub async fn set_vault_sync_enabled(
        &mut self,
        vault_id: &str,
        enabled: bool,
    ) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "UPDATE vault_contexts SET sync_enabled = ?1 WHERE id = ?2",
                (if enabled { 1 } else { 0 }, vault_id),
            )
            .await?;
        Ok(rows > 0)
    }

    /// Retrieves the wrapped DEK envelope for a vault context.
    pub async fn get_vault_dek_envelope(
        &self,
        vault_id: &str,
    ) -> Result<Option<String>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT dek_envelope FROM vault_contexts WHERE id = ?1",
                (vault_id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let env: Option<String> = row.get(0)?;
            Ok(env)
        } else {
            Ok(None)
        }
    }

    /// Sets the wrapped DEK envelope and key epoch for a vault context.
    pub async fn set_vault_dek_envelope(
        &mut self,
        vault_id: &str,
        envelope_json: Option<&str>,
        epoch: u32,
    ) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "UPDATE vault_contexts SET dek_envelope = ?1, active_key_epoch = ?2 WHERE id = ?3",
                (envelope_json, epoch as i64, vault_id),
            )
            .await?;
        Ok(rows > 0)
    }

    /// Gets the active key epoch for a vault context.
    pub async fn get_vault_active_key_epoch(&self, vault_id: &str) -> Result<u32, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT active_key_epoch FROM vault_contexts WHERE id = ?1",
                (vault_id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let epoch: i64 = row.get(0)?;
            Ok(epoch.max(1) as u32)
        } else {
            Ok(1)
        }
    }

    /// Sets the active key epoch for a vault context.
    pub async fn set_vault_active_key_epoch(
        &mut self,
        vault_id: &str,
        epoch: u32,
    ) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "UPDATE vault_contexts SET active_key_epoch = ?1 WHERE id = ?2",
                (epoch as i64, vault_id),
            )
            .await?;
        Ok(rows > 0)
    }

    /// Sets the locked status of a vault context.
    pub async fn set_vault_locked(
        &mut self,
        vault_id: &str,
        locked: bool,
    ) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "UPDATE vault_contexts SET is_locked = ?1 WHERE id = ?2",
                (if locked { 1i64 } else { 0i64 }, vault_id),
            )
            .await?;
        Ok(rows > 0)
    }

    /// Returns whether a vault context is currently marked locked.
    pub async fn is_vault_locked(&self, vault_id: &str) -> Result<bool, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT is_locked FROM vault_contexts WHERE id = ?1",
                (vault_id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let locked: i64 = row.get(0)?;
            Ok(locked != 0)
        } else {
            Ok(false)
        }
    }

    /// Saves an encrypted vault directory envelope.
    pub async fn save_encrypted_vault_directory(
        &mut self,
        id: &str,
        encrypted_data: &str,
    ) -> Result<(), StorageError> {
        self.connection
            .execute(
                "INSERT INTO vault_directories (id, encrypted_data, updated_at)
                 VALUES (?1, ?2, datetime('now'))
                 ON CONFLICT(id) DO UPDATE SET
                     encrypted_data = excluded.encrypted_data,
                     updated_at = excluded.updated_at",
                (id, encrypted_data),
            )
            .await?;
        Ok(())
    }

    /// Retrieves an encrypted vault directory envelope.
    pub async fn get_encrypted_vault_directory(
        &self,
        id: &str,
    ) -> Result<Option<String>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT encrypted_data FROM vault_directories WHERE id = ?1",
                (id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let data: String = row.get(0)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    /// Constructs the unencrypted `VaultDirectory` representing all known vaults.
    pub async fn build_vault_directory(
        &self,
    ) -> Result<shellumi_core::VaultDirectory, StorageError> {
        let contexts = self.list_vault_contexts().await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let mut dir = shellumi_core::VaultDirectory::new();
        for (name, ctx) in contexts {
            let location = match ctx.location() {
                shellumi_core::VaultLocation::Local => shellumi_core::VaultDirectoryLocation::Local,
                shellumi_core::VaultLocation::Remote {
                    server,
                    server_vault_id,
                } => shellumi_core::VaultDirectoryLocation::Remote {
                    server_connection_id: server.as_str().to_string(),
                    server_vault_id: server_vault_id.as_str().to_string(),
                },
            };
            let has_recovery = self
                .get_vault_recovery_envelope(ctx.id().as_str())
                .await?
                .is_some();
            let is_sync_enabled = self.is_vault_sync_enabled(ctx.id().as_str()).await?;
            dir.upsert(shellumi_core::VaultDirectoryEntry {
                id: ctx.id().as_str().to_string(),
                name,
                location,
                has_recovery_envelope: has_recovery,
                sync_enabled: is_sync_enabled,
                created_at: now,
                updated_at: now,
            });
        }
        Ok(dir)
    }

    /// Encrypts and persists the vault directory under the default local vault's key.
    pub async fn save_vault_directory_from_model(
        &mut self,
        default_vault_id: &shellumi_core::VaultId,
        default_vault_key: &shellumi_core::VaultKey,
        directory: &shellumi_core::VaultDirectory,
    ) -> Result<String, StorageError> {
        let envelope = directory
            .encrypt(default_vault_id, default_vault_key)
            .map_err(|e| {
                StorageError::Serialization(format!("Failed to encrypt vault directory: {e}"))
            })?;
        let serialized = serde_json::to_string(&envelope).map_err(|e| {
            StorageError::Serialization(format!(
                "Failed to serialize vault directory envelope: {e}"
            ))
        })?;
        self.save_encrypted_vault_directory("primary_directory", &serialized)
            .await?;
        Ok(serialized)
    }

    /// Imports and applies an encrypted vault directory envelope.
    /// If default vault key is provided, decrypts and registers any newly discovered vaults.
    pub async fn import_encrypted_vault_directory(
        &mut self,
        default_vault_id: &shellumi_core::VaultId,
        default_vault_key: Option<&shellumi_core::VaultKey>,
        envelope_json: &str,
    ) -> Result<usize, StorageError> {
        self.save_encrypted_vault_directory("primary_directory", envelope_json)
            .await?;
        let envelope: shellumi_core::sync::EncryptedEnvelope = serde_json::from_str(envelope_json)
            .map_err(|e| {
                StorageError::Serialization(format!("Corrupt vault directory envelope: {e}"))
            })?;

        let Some(key) = default_vault_key else {
            return Ok(0);
        };
        let directory = shellumi_core::VaultDirectory::decrypt(default_vault_id, key, &envelope)
            .map_err(|e| {
                StorageError::Serialization(format!("Failed to decrypt vault directory: {e}"))
            })?;

        let existing = self.list_vault_contexts().await?;
        let mut newly_registered = 0;
        for entry in directory.entries {
            let Ok(vid) = shellumi_core::VaultId::new(&entry.id) else {
                continue;
            };
            if !existing
                .iter()
                .any(|(_, ctx)| ctx.id().as_str() == entry.id)
            {
                let loc = match entry.location {
                    shellumi_core::VaultDirectoryLocation::Local => {
                        shellumi_core::VaultLocation::Local
                    }
                    shellumi_core::VaultDirectoryLocation::Remote {
                        server_connection_id,
                        server_vault_id,
                    } => {
                        let Ok(srv_conn) =
                            shellumi_core::ServerConnectionId::new(&server_connection_id)
                        else {
                            continue;
                        };
                        self.connection.execute(
                            "INSERT INTO vault_server_connections (id) VALUES (?1) ON CONFLICT(id) DO NOTHING",
                            (srv_conn.as_str(),),
                        ).await?;
                        let Ok(srv_vid) = shellumi_core::VaultId::new(&server_vault_id) else {
                            continue;
                        };
                        shellumi_core::VaultLocation::Remote {
                            server: srv_conn,
                            server_vault_id: srv_vid,
                        }
                    }
                };
                let ctx = shellumi_core::VaultContext::new(vid, loc);
                self.register_vault_context(&entry.name, &ctx).await?;
                newly_registered += 1;
            }
            self.set_vault_sync_enabled(&entry.id, entry.sync_enabled)
                .await?;
        }
        Ok(newly_registered)
    }

    /// Copies a host from its source vault to target vault according to the specified policy.
    pub async fn copy_host_across_vaults(
        &mut self,
        host_id: i64,
        policy: &shellumi_core::CrossVaultTransferPolicy,
    ) -> Result<i64, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT name, hostname, username, port, group_name, tags, jump_host, startup_snippet_id, protocol, protocol_config, identity_id
                 FROM hosts WHERE id = ?1 AND vault_id = ?2",
                (host_id, policy.source_vault.as_str()),
            )
            .await?;

        let row = rows.next().await?.ok_or_else(|| {
            StorageError::ItemNotFound(format!(
                "host {} not found in vault {}",
                host_id,
                policy.source_vault.as_str()
            ))
        })?;

        let name: String = row.get(0)?;
        let hostname: String = row.get(1)?;
        let username: String = row.get(2)?;
        let port: i64 = row.get(3)?;
        let group_name: Option<String> = row.get(4)?;
        let tags: String = row.get(5)?;
        let jump_host: Option<String> = row.get(6)?;
        let _startup_snippet_id: Option<i64> = row.get(7)?;
        let protocol: String = row.get(8)?;
        let protocol_config: Option<String> = row.get(9)?;
        let identity_id: Option<i64> = row.get(10)?;
        drop(rows);

        let has_attached_credentials = identity_id.is_some();
        policy.validate(has_attached_credentials)?;

        let target_identity_id = if policy.strip_credential_references || !has_attached_credentials
        {
            None
        } else {
            if let Some(src_id) = identity_id {
                let mut id_rows = self
                    .connection
                    .query(
                        "SELECT name, username, auth_method, password_secret_ref, key_path, passphrase_secret_ref, certificate_path, tags, comment
                         FROM identities WHERE id = ?1 AND vault_id = ?2",
                        (src_id, policy.source_vault.as_str()),
                    )
                    .await?;
                if let Some(id_row) = id_rows.next().await? {
                    let id_name: String = id_row.get(0)?;
                    let id_username: String = id_row.get(1)?;
                    let id_auth: String = id_row.get(2)?;
                    let id_pw: Option<String> = id_row.get(3)?;
                    let id_key: Option<String> = id_row.get(4)?;
                    let id_pass: Option<String> = id_row.get(5)?;
                    let id_cert: Option<String> = id_row.get(6)?;
                    let id_tags: String = id_row.get(7)?;
                    let id_comment: Option<String> = id_row.get(8)?;
                    drop(id_rows);

                    let mut existing_any = self
                        .connection
                        .query(
                            "SELECT id, vault_id FROM identities WHERE name = ?1",
                            (id_name.as_str(),),
                        )
                        .await?;
                    if let Some(row) = existing_any.next().await? {
                        let existing_id: i64 = row.get(0)?;
                        let existing_vault: String = row.get(1)?;
                        drop(existing_any);
                        if existing_vault == policy.target_vault.as_str() {
                            Some(existing_id)
                        } else {
                            // Derive a target-scoped unique name to avoid violating the database unique name constraint
                            let target_name =
                                format!("{} ({})", id_name, policy.target_vault.as_str());
                            let mut target_existing = self
                                .connection
                                .query(
                                    "SELECT id FROM identities WHERE name = ?1",
                                    (target_name.as_str(),),
                                )
                                .await?;
                            if let Some(target_row) = target_existing.next().await? {
                                let tid: i64 = target_row.get(0)?;
                                drop(target_existing);
                                Some(tid)
                            } else {
                                drop(target_existing);
                                self.connection
                                    .execute(
                                        "INSERT INTO identities (name, username, auth_method, password_secret_ref, key_path, passphrase_secret_ref, certificate_path, tags, comment, vault_id)
                                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                                        (
                                            target_name,
                                            id_username,
                                            id_auth,
                                            id_pw,
                                            id_key,
                                            id_pass,
                                            id_cert,
                                            id_tags,
                                            id_comment,
                                            policy.target_vault.as_str(),
                                        ),
                                    )
                                    .await?;
                                Some(self.connection.last_insert_rowid())
                            }
                        }
                    } else {
                        drop(existing_any);
                        self.connection
                            .execute(
                                "INSERT INTO identities (name, username, auth_method, password_secret_ref, key_path, passphrase_secret_ref, certificate_path, tags, comment, vault_id)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                                (
                                    id_name,
                                    id_username,
                                    id_auth,
                                    id_pw,
                                    id_key,
                                    id_pass,
                                    id_cert,
                                    id_tags,
                                    id_comment,
                                    policy.target_vault.as_str(),
                                ),
                            )
                            .await?;
                        Some(self.connection.last_insert_rowid())
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Don't carry cross-vault snippet foreign keys
        let target_snippet_id: Option<i64> = None;

        self.connection
            .execute(
                "INSERT INTO hosts (name, hostname, username, port, group_name, tags, jump_host, startup_snippet_id, protocol, protocol_config, identity_id, vault_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                (
                    name,
                    hostname,
                    username,
                    port,
                    group_name,
                    tags,
                    jump_host,
                    target_snippet_id,
                    protocol,
                    protocol_config,
                    target_identity_id,
                    policy.target_vault.as_str(),
                ),
            )
            .await?;

        Ok(self.connection.last_insert_rowid())
    }

    /// Moves a host from its source vault to target vault according to the specified policy.
    pub async fn move_host_across_vaults(
        &mut self,
        host_id: i64,
        policy: &shellumi_core::CrossVaultTransferPolicy,
    ) -> Result<i64, StorageError> {
        let new_id = self.copy_host_across_vaults(host_id, policy).await?;
        self.connection
            .execute(
                "DELETE FROM hosts WHERE id = ?1 AND vault_id = ?2",
                (host_id, policy.source_vault.as_str()),
            )
            .await?;
        Ok(new_id)
    }

    /// Copies a snippet from its source vault to target vault according to the specified policy.
    pub async fn copy_snippet_across_vaults(
        &mut self,
        snippet_id: i64,
        policy: &shellumi_core::CrossVaultTransferPolicy,
    ) -> Result<i64, StorageError> {
        policy.validate(false)?;
        let mut rows = self
            .connection
            .query(
                "SELECT name, command, description, tags, variables FROM snippets WHERE id = ?1 AND vault_id = ?2",
                (snippet_id, policy.source_vault.as_str()),
            )
            .await?;

        let row = rows.next().await?.ok_or_else(|| {
            StorageError::ItemNotFound(format!(
                "snippet {} not found in vault {}",
                snippet_id,
                policy.source_vault.as_str()
            ))
        })?;

        let name: String = row.get(0)?;
        let command: String = row.get(1)?;
        let description: Option<String> = row.get(2)?;
        let tags: String = row.get(3)?;
        let variables: String = row.get(4)?;
        drop(rows);

        self.connection
            .execute(
                "INSERT INTO snippets (name, command, description, tags, variables, vault_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    name,
                    command,
                    description,
                    tags,
                    variables,
                    policy.target_vault.as_str(),
                ),
            )
            .await?;

        Ok(self.connection.last_insert_rowid())
    }

    /// Moves a snippet from its source vault to target vault according to the specified policy.
    pub async fn move_snippet_across_vaults(
        &mut self,
        snippet_id: i64,
        policy: &shellumi_core::CrossVaultTransferPolicy,
    ) -> Result<i64, StorageError> {
        let new_id = self.copy_snippet_across_vaults(snippet_id, policy).await?;
        self.connection
            .execute(
                "DELETE FROM snippets WHERE id = ?1 AND vault_id = ?2",
                (snippet_id, policy.source_vault.as_str()),
            )
            .await?;
        Ok(new_id)
    }

    /// Copies a workspace from its source vault to target vault according to the specified policy.
    pub async fn copy_workspace_across_vaults(
        &mut self,
        workspace_id: i64,
        policy: &shellumi_core::CrossVaultTransferPolicy,
    ) -> Result<i64, StorageError> {
        policy.validate(false)?;
        let mut rows = self
            .connection
            .query(
                "SELECT name, description, layout, sessions_json, tags FROM workspaces WHERE id = ?1 AND vault_id = ?2",
                (workspace_id, policy.source_vault.as_str()),
            )
            .await?;

        let row = rows.next().await?.ok_or_else(|| {
            StorageError::ItemNotFound(format!(
                "workspace {} not found in vault {}",
                workspace_id,
                policy.source_vault.as_str()
            ))
        })?;

        let name: String = row.get(0)?;
        let description: Option<String> = row.get(1)?;
        let layout: String = row.get(2)?;
        let sessions_json: String = row.get(3)?;
        let tags: String = row.get(4)?;
        drop(rows);

        self.connection
            .execute(
                "INSERT INTO workspaces (name, description, layout, sessions_json, tags, vault_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    name,
                    description,
                    layout,
                    sessions_json,
                    tags,
                    policy.target_vault.as_str(),
                ),
            )
            .await?;

        Ok(self.connection.last_insert_rowid())
    }

    /// Moves a workspace from its source vault to target vault according to the specified policy.
    pub async fn move_workspace_across_vaults(
        &mut self,
        workspace_id: i64,
        policy: &shellumi_core::CrossVaultTransferPolicy,
    ) -> Result<i64, StorageError> {
        let new_id = self
            .copy_workspace_across_vaults(workspace_id, policy)
            .await?;
        self.connection
            .execute(
                "DELETE FROM workspaces WHERE id = ?1 AND vault_id = ?2",
                (workspace_id, policy.source_vault.as_str()),
            )
            .await?;
        Ok(new_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn add_full(
        &mut self,
        name: &str,
        destination: &SshDestination,
        group_name: Option<&str>,
        tags: &[String],
        jump_host: Option<&str>,
        startup_snippet_id: Option<i64>,
        protocol: &str,
        protocol_config: Option<&str>,
        identity_id: Option<i64>,
    ) -> Result<i64, StorageError> {
        if let Some(id) = identity_id {
            if self.get_identity(id).await?.is_none() {
                return Err(StorageError::InvalidIdentityReference);
            }
        }
        let name = validated_name(name)?;
        let group = validated_group(group_name)?;
        let tags_serialized = validated_tags(tags)?;
        let jump = validated_jump_host(jump_host)?;
        let proto = if protocol.trim().is_empty() {
            "ssh"
        } else {
            protocol.trim()
        };

        self.connection
            .execute(
                "INSERT INTO hosts (name, hostname, username, port, group_name, tags, jump_host, startup_snippet_id, protocol, protocol_config, identity_id, vault_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                (
                    name,
                    destination.host(),
                    destination.username(),
                    i64::from(destination.port().get()),
                    group,
                    tags_serialized,
                    jump,
                    startup_snippet_id,
                    proto,
                    protocol_config,
                    identity_id,
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(self.connection.last_insert_rowid())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn add_with_protocol(
        &mut self,
        name: &str,
        destination: &SshDestination,
        group_name: Option<&str>,
        tags: &[String],
        jump_host: Option<&str>,
        startup_snippet_id: Option<i64>,
        protocol: &str,
        protocol_config: Option<&str>,
    ) -> Result<i64, StorageError> {
        self.add_full(
            name,
            destination,
            group_name,
            tags,
            jump_host,
            startup_snippet_id,
            protocol,
            protocol_config,
            None,
        )
        .await
    }

    pub async fn add(
        &mut self,
        name: &str,
        destination: &SshDestination,
        group_name: Option<&str>,
        tags: &[String],
        jump_host: Option<&str>,
        startup_snippet_id: Option<i64>,
    ) -> Result<i64, StorageError> {
        self.add_with_protocol(
            name,
            destination,
            group_name,
            tags,
            jump_host,
            startup_snippet_id,
            "ssh",
            None,
        )
        .await
    }

    /// Update an existing host with full options including protocol and identity.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_full(
        &mut self,
        id: i64,
        name: &str,
        destination: &SshDestination,
        group_name: Option<&str>,
        tags: &[String],
        jump_host: Option<&str>,
        startup_snippet_id: Option<i64>,
        protocol: &str,
        protocol_config: Option<&str>,
        identity_id: Option<i64>,
    ) -> Result<bool, StorageError> {
        if let Some(identity_id) = identity_id {
            if self.get_identity(identity_id).await?.is_none() {
                return Err(StorageError::InvalidIdentityReference);
            }
        }
        let name = validated_name(name)?;
        let group = validated_group(group_name)?;
        let tags_serialized = validated_tags(tags)?;
        let jump = validated_jump_host(jump_host)?;
        let proto = if protocol.trim().is_empty() {
            "ssh"
        } else {
            protocol.trim()
        };

        Ok(self
            .connection
            .execute(
                "UPDATE hosts SET name = ?1, hostname = ?2, username = ?3, port = ?4, group_name = ?5, tags = ?6, jump_host = ?7, startup_snippet_id = ?8, protocol = ?9, protocol_config = ?10, identity_id = ?11 WHERE id = ?12 AND vault_id = ?13",
                (
                    name,
                    destination.host(),
                    destination.username(),
                    i64::from(destination.port().get()),
                    group,
                    tags_serialized,
                    jump,
                    startup_snippet_id,
                    proto,
                    protocol_config,
                    identity_id,
                    id,
                    self.host_vault_id.as_str(),
                ),
            )
            .await?
            != 0)
    }

    /// Update an existing host, returning false if its ID is not present.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_with_protocol(
        &mut self,
        id: i64,
        name: &str,
        destination: &SshDestination,
        group_name: Option<&str>,
        tags: &[String],
        jump_host: Option<&str>,
        startup_snippet_id: Option<i64>,
        protocol: &str,
        protocol_config: Option<&str>,
    ) -> Result<bool, StorageError> {
        self.update_full(
            id,
            name,
            destination,
            group_name,
            tags,
            jump_host,
            startup_snippet_id,
            protocol,
            protocol_config,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &mut self,
        id: i64,
        name: &str,
        destination: &SshDestination,
        group_name: Option<&str>,
        tags: &[String],
        jump_host: Option<&str>,
        startup_snippet_id: Option<i64>,
    ) -> Result<bool, StorageError> {
        self.update_with_protocol(
            id,
            name,
            destination,
            group_name,
            tags,
            jump_host,
            startup_snippet_id,
            "ssh",
            None,
        )
        .await
    }

    pub async fn list(&self) -> Result<Vec<SavedHost>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, hostname, username, port, group_name, tags, jump_host, startup_snippet_id, protocol, protocol_config, identity_id FROM hosts WHERE vault_id = ?1 ORDER BY id",
                (self.host_vault_id.as_str(),),
            )
            .await?;
        let mut hosts = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let host: String = row.get(2)?;
            let username: String = row.get(3)?;
            let port: i64 = row.get(4)?;
            let group_name: Option<String> = row.get(5)?;
            let tags_str: String = row.get(6).unwrap_or_default();
            let jump_host: Option<String> = row.get(7)?;
            let startup_snippet_id: Option<i64> = row.get(8)?;
            let protocol: Option<String> = row.get(9)?;
            let protocol_config: Option<String> = row.get(10)?;
            let identity_id: Option<i64> = row.get(11)?;

            if name.trim().is_empty() || name.chars().any(char::is_control) {
                return Err(StorageError::InvalidName);
            }
            let port = u16::try_from(port).map_err(|_| StorageError::InvalidPort(port))?;
            let destination = SshDestination::new(&host, &username, port)
                .map_err(StorageError::InvalidDestination)?;

            hosts.push(SavedHost {
                id,
                name,
                destination,
                group_name,
                tags: parse_tags(&tags_str),
                jump_host,
                startup_snippet_id,
                protocol: protocol.unwrap_or_else(|| "ssh".to_string()),
                protocol_config,
                identity_id,
            });
        }
        Ok(hosts)
    }

    pub async fn get_host(&self, id: i64) -> Result<Option<SavedHost>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, hostname, username, port, group_name, tags, jump_host, startup_snippet_id, protocol, protocol_config, identity_id FROM hosts WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let host: String = row.get(2)?;
            let username: String = row.get(3)?;
            let port: i64 = row.get(4)?;
            let group_name: Option<String> = row.get(5)?;
            let tags_str: String = row.get(6).unwrap_or_default();
            let jump_host: Option<String> = row.get(7)?;
            let startup_snippet_id: Option<i64> = row.get(8)?;
            let protocol: Option<String> = row.get(9)?;
            let protocol_config: Option<String> = row.get(10)?;
            let identity_id: Option<i64> = row.get(11)?;

            if name.trim().is_empty() || name.chars().any(char::is_control) {
                return Err(StorageError::InvalidName);
            }
            let port = u16::try_from(port).map_err(|_| StorageError::InvalidPort(port))?;
            let destination = SshDestination::new(&host, &username, port)
                .map_err(StorageError::InvalidDestination)?;

            Ok(Some(SavedHost {
                id,
                name,
                destination,
                group_name,
                tags: parse_tags(&tags_str),
                jump_host,
                startup_snippet_id,
                protocol: protocol.unwrap_or_else(|| "ssh".to_string()),
                protocol_config,
                identity_id,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_by_group(&self, group: &str) -> Result<Vec<SavedHost>, StorageError> {
        let all = self.list().await?;
        let target = group.trim().to_lowercase();
        Ok(all
            .into_iter()
            .filter(|h| h.group_name.as_deref().map(|g| g.to_lowercase()) == Some(target.clone()))
            .collect())
    }

    pub async fn list_by_tag(&self, tag: &str) -> Result<Vec<SavedHost>, StorageError> {
        let all = self.list().await?;
        let target = tag.trim().to_lowercase();
        Ok(all
            .into_iter()
            .filter(|h| h.tags.iter().any(|t| t.to_lowercase() == target))
            .collect())
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SavedHost>, StorageError> {
        let all = self.list().await?;
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Ok(all);
        }
        Ok(all
            .into_iter()
            .filter(|h| {
                h.name.to_lowercase().contains(&query)
                    || h.destination.host().to_lowercase().contains(&query)
                    || h.destination.username().to_lowercase().contains(&query)
                    || h.group_name
                        .as_ref()
                        .is_some_and(|g| g.to_lowercase().contains(&query))
                    || h.jump_host
                        .as_ref()
                        .is_some_and(|j| j.to_lowercase().contains(&query))
                    || h.tags.iter().any(|t| t.to_lowercase().contains(&query))
            })
            .collect())
    }

    pub async fn add_group(&mut self, group: &HostGroup) -> Result<(), StorageError> {
        let tags_serialized = validated_tags(&group.tags)?;
        let default_port = group.default_port.map(|p| i64::from(p.get()));
        self.connection
            .execute(
                "INSERT INTO groups (name, default_username, default_port, tags, identity_id, vault_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    group.name.as_str(),
                    group.default_username.as_deref(),
                    default_port,
                    tags_serialized,
                    group.identity_id,
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn update_group(&mut self, group: &HostGroup) -> Result<bool, StorageError> {
        let tags_serialized = validated_tags(&group.tags)?;
        let default_port = group.default_port.map(|p| i64::from(p.get()));
        let rows = self
            .connection
            .execute(
                "UPDATE groups SET default_username = ?1, default_port = ?2, tags = ?3, identity_id = ?4 WHERE name = ?5 AND vault_id = ?6",
                (
                    group.default_username.as_deref(),
                    default_port,
                    tags_serialized,
                    group.identity_id,
                    group.name.as_str(),
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn get_group(&self, name: &str) -> Result<Option<HostGroup>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT name, default_username, default_port, tags, identity_id FROM groups WHERE name = ?1 AND vault_id = ?2",
                (name.trim(), self.host_vault_id.as_str()),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            let default_username: Option<String> = row.get(1)?;
            let default_port: Option<i64> = row.get(2)?;
            let tags_str: String = row.get(3)?;
            let identity_id: Option<i64> = row.get(4)?;
            let tags = parse_tags(&tags_str);
            let port = match default_port {
                Some(p) => {
                    let u = u16::try_from(p).map_err(|_| StorageError::InvalidPort(p))?;
                    Some(u)
                }
                None => None,
            };
            let group = HostGroup::new(&name, default_username.as_deref(), port, &tags)
                .map_err(StorageError::InvalidGroup)?
                .with_identity_id(identity_id);
            Ok(Some(group))
        } else {
            Ok(None)
        }
    }

    pub async fn list_groups(&self) -> Result<Vec<HostGroup>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT name, default_username, default_port, tags, identity_id FROM groups WHERE vault_id = ?1 ORDER BY name",
                (self.host_vault_id.as_str(),),
            )
            .await?;
        let mut groups = Vec::new();
        while let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            let default_username: Option<String> = row.get(1)?;
            let default_port: Option<i64> = row.get(2)?;
            let tags_str: String = row.get(3)?;
            let identity_id: Option<i64> = row.get(4)?;
            let tags = parse_tags(&tags_str);
            let port = match default_port {
                Some(p) => {
                    let u = u16::try_from(p).map_err(|_| StorageError::InvalidPort(p))?;
                    Some(u)
                }
                None => None,
            };
            let group = HostGroup::new(&name, default_username.as_deref(), port, &tags)
                .map_err(StorageError::InvalidGroup)?
                .with_identity_id(identity_id);
            groups.push(group);
        }
        Ok(groups)
    }

    pub async fn delete_group(&mut self, name: &str) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "DELETE FROM groups WHERE name = ?1 AND vault_id = ?2",
                (name.trim(), self.host_vault_id.as_str()),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn create_identity(
        &mut self,
        identity: &shellumi_core::NewIdentity,
    ) -> Result<i64, StorageError> {
        let name = validated_name(&identity.name)?;
        let username = identity.username.trim();
        if username.is_empty()
            || username
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(StorageError::InvalidIdentity(
                shellumi_core::IdentityError::InvalidIdentityField(
                    "username must not be empty or contain whitespace".to_string(),
                ),
            ));
        }
        let auth_method_json = serde_json::to_string(&identity.auth_method)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let tags_json = serde_json::to_string(&identity.tags)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        self.connection
            .execute(
                "INSERT INTO identities (name, username, auth_method, password_secret_ref, key_path, passphrase_secret_ref, certificate_path, tags, comment, vault_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                (
                    name,
                    username,
                    auth_method_json,
                    identity.password_secret_ref.as_deref(),
                    identity.key_path.as_deref(),
                    identity.passphrase_secret_ref.as_deref(),
                    identity.certificate_path.as_deref(),
                    tags_json,
                    identity.comment.as_deref(),
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(self.connection.last_insert_rowid())
    }

    pub async fn get_identity(&self, id: i64) -> Result<Option<SavedIdentity>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, username, auth_method, password_secret_ref, key_path, passphrase_secret_ref, certificate_path, tags, comment, created_at, updated_at FROM identities WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            Self::parse_identity_row(&row).map(Some)
        } else {
            Ok(None)
        }
    }

    pub async fn get_identity_by_name(
        &self,
        name: &str,
    ) -> Result<Option<SavedIdentity>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, username, auth_method, password_secret_ref, key_path, passphrase_secret_ref, certificate_path, tags, comment, created_at, updated_at FROM identities WHERE name = ?1 AND vault_id = ?2",
                (name.trim(), self.host_vault_id.as_str()),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            Self::parse_identity_row(&row).map(Some)
        } else {
            Ok(None)
        }
    }

    pub async fn list_identities(&self) -> Result<Vec<SavedIdentity>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, username, auth_method, password_secret_ref, key_path, passphrase_secret_ref, certificate_path, tags, comment, created_at, updated_at FROM identities WHERE vault_id = ?1 ORDER BY name",
                (self.host_vault_id.as_str(),),
            )
            .await?;
        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            list.push(Self::parse_identity_row(&row)?);
        }
        Ok(list)
    }

    pub async fn update_identity(
        &mut self,
        id: i64,
        identity: &shellumi_core::NewIdentity,
    ) -> Result<bool, StorageError> {
        let name = validated_name(&identity.name)?;
        let username = identity.username.trim();
        if username.is_empty()
            || username
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(StorageError::InvalidIdentity(
                shellumi_core::IdentityError::InvalidIdentityField(
                    "username must not be empty or contain whitespace".to_string(),
                ),
            ));
        }
        let auth_method_json = serde_json::to_string(&identity.auth_method)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let tags_json = serde_json::to_string(&identity.tags)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let rows = self
            .connection
            .execute(
                "UPDATE identities SET name = ?1, username = ?2, auth_method = ?3, password_secret_ref = ?4, key_path = ?5, passphrase_secret_ref = ?6, certificate_path = ?7, tags = ?8, comment = ?9, updated_at = datetime('now') WHERE id = ?10 AND vault_id = ?11",
                (
                    name,
                    username,
                    auth_method_json,
                    identity.password_secret_ref.as_deref(),
                    identity.key_path.as_deref(),
                    identity.passphrase_secret_ref.as_deref(),
                    identity.certificate_path.as_deref(),
                    tags_json,
                    identity.comment.as_deref(),
                    id,
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn delete_identity(&mut self, id: i64) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "DELETE FROM identities WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        Ok(rows != 0)
    }

    /// Creates an identity, persisting any provided plaintext password or passphrase into the
    /// native credential store using canonical `keychain://` references, ensuring no secrets
    /// ever enter the database or disk in plaintext.
    pub async fn create_identity_with_secrets(
        &mut self,
        mut identity: shellumi_core::NewIdentity,
        password: Option<&str>,
        passphrase: Option<&str>,
        store: &dyn shellumi_core::CredentialStore,
    ) -> Result<i64, StorageError> {
        let account_base = format!("identity:{}", identity.name.trim());
        if let Some(pw) = password {
            let acc = format!("{account_base}:password");
            let uri = shellumi_core::store_secret_to_ref(
                store,
                shellumi_core::DEFAULT_KEYCHAIN_SERVICE,
                &acc,
                pw,
            )?;
            identity.password_secret_ref = Some(uri);
        }
        if let Some(pp) = passphrase {
            let acc = format!("{account_base}:passphrase");
            let uri = shellumi_core::store_secret_to_ref(
                store,
                shellumi_core::DEFAULT_KEYCHAIN_SERVICE,
                &acc,
                pp,
            )?;
            identity.passphrase_secret_ref = Some(uri);
        }
        self.create_identity(&identity).await
    }

    /// Resolves the password secret for an identity from the configured credential store.
    pub fn resolve_identity_password(
        &self,
        identity: &SavedIdentity,
        store: &dyn shellumi_core::CredentialStore,
    ) -> Result<Option<shellumi_core::SecretString>, StorageError> {
        match &identity.password_secret_ref {
            Some(ref_uri) => Ok(shellumi_core::resolve_secret_from_ref(store, ref_uri)?),
            None => Ok(None),
        }
    }

    /// Resolves the private key passphrase for an identity from the configured credential store.
    pub fn resolve_identity_passphrase(
        &self,
        identity: &SavedIdentity,
        store: &dyn shellumi_core::CredentialStore,
    ) -> Result<Option<shellumi_core::SecretString>, StorageError> {
        match &identity.passphrase_secret_ref {
            Some(ref_uri) => Ok(shellumi_core::resolve_secret_from_ref(store, ref_uri)?),
            None => Ok(None),
        }
    }

    /// Deletes an identity along with any secrets referenced in native credential store.
    pub async fn delete_identity_with_secrets(
        &mut self,
        id: i64,
        store: &dyn shellumi_core::CredentialStore,
    ) -> Result<bool, StorageError> {
        if let Some(identity) = self.get_identity(id).await? {
            if let Some(ref_uri) = &identity.password_secret_ref {
                let _ = shellumi_core::delete_secret_by_ref(store, ref_uri);
            }
            if let Some(ref_uri) = &identity.passphrase_secret_ref {
                let _ = shellumi_core::delete_secret_by_ref(store, ref_uri);
            }
        }
        self.delete_identity(id).await
    }

    fn parse_identity_row(row: &turso::Row) -> Result<SavedIdentity, StorageError> {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let username: String = row.get(2)?;
        let auth_method_str: String = row.get(3)?;
        let password_secret_ref: Option<String> = row.get(4)?;
        let key_path: Option<String> = row.get(5)?;
        let passphrase_secret_ref: Option<String> = row.get(6)?;
        let certificate_path: Option<String> = row.get(7)?;
        let tags_str: String = row.get(8)?;
        let comment: Option<String> = row.get(9)?;
        let created_at: String = row.get(10)?;
        let updated_at: String = row.get(11)?;

        let auth_method: shellumi_core::AuthMethod = serde_json::from_str(&auth_method_str)
            .or_else(|_| shellumi_core::AuthMethod::parse(&auth_method_str, None))
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let tags: Vec<String> =
            serde_json::from_str(&tags_str).unwrap_or_else(|_| parse_tags(&tags_str));

        Ok(SavedIdentity {
            id,
            name,
            username,
            auth_method,
            password_secret_ref,
            key_path,
            passphrase_secret_ref,
            certificate_path,
            tags,
            comment,
            created_at,
            updated_at,
        })
    }

    pub async fn get_effective_destination(
        &self,
        host: &SavedHost,
    ) -> Result<SshDestination, StorageError> {
        let identity = if let Some(id) = host.identity_id {
            self.get_identity(id).await?.map(|i| i.to_identity())
        } else {
            None
        };

        let group = if let Some(group_name) = &host.group_name {
            self.get_group(group_name).await?
        } else {
            None
        };

        let fallback_identity = if identity.is_none() {
            if let Some(ref g) = group {
                if let Some(gid) = g.identity_id {
                    self.get_identity(gid).await?.map(|i| i.to_identity())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            identity
        };

        shellumi_core::resolve_effective_destination_with_identity(
            host.destination.host(),
            Some(host.destination.username()),
            Some(host.destination.port().get()),
            group.as_ref(),
            fallback_identity.as_ref(),
        )
        .map_err(StorageError::InvalidDestination)
    }

    pub async fn get_effective_tags(&self, host: &SavedHost) -> Result<Vec<String>, StorageError> {
        if let Some(group_name) = &host.group_name {
            if let Some(group) = self.get_group(group_name).await? {
                return Ok(resolve_effective_tags(&host.tags, Some(&group)));
            }
        }
        Ok(host.tags.clone())
    }

    pub async fn delete(&mut self, id: i64) -> Result<bool, StorageError> {
        Ok(self
            .connection
            .execute(
                "DELETE FROM hosts WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?
            != 0)
    }

    pub async fn add_tunnel(
        &mut self,
        name: &str,
        host_id: Option<i64>,
        rule: &PortForwardRule,
        auto_start: bool,
    ) -> Result<i64, StorageError> {
        let name = validated_name(name)?;
        let target_port = rule.target_port.map(i64::from);

        self.connection
            .execute(
                "INSERT INTO tunnels (host_id, name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start, vault_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                (
                    host_id,
                    name,
                    rule.forwarding_type.as_str(),
                    rule.bind_address.as_deref(),
                    i64::from(rule.bind_port),
                    rule.target_host.as_deref(),
                    target_port,
                    if auto_start { 1i64 } else { 0i64 },
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(self.connection.last_insert_rowid())
    }

    pub async fn update_tunnel(
        &mut self,
        id: i64,
        name: &str,
        host_id: Option<i64>,
        rule: &PortForwardRule,
        auto_start: bool,
    ) -> Result<bool, StorageError> {
        let name = validated_name(name)?;
        let target_port = rule.target_port.map(i64::from);

        let rows = self
            .connection
            .execute(
                "UPDATE tunnels SET host_id = ?1, name = ?2, forwarding_type = ?3, bind_address = ?4, bind_port = ?5, target_host = ?6, target_port = ?7, auto_start = ?8 WHERE id = ?9 AND vault_id = ?10",
                (
                    host_id,
                    name,
                    rule.forwarding_type.as_str(),
                    rule.bind_address.as_deref(),
                    i64::from(rule.bind_port),
                    rule.target_host.as_deref(),
                    target_port,
                    if auto_start { 1i64 } else { 0i64 },
                    id,
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn list_tunnels(&self) -> Result<Vec<SavedTunnel>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, host_id, name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start FROM tunnels WHERE vault_id = ?1 ORDER BY id",
                (self.host_vault_id.as_str(),),
            )
            .await?;
        Self::parse_tunnel_rows(&mut rows).await
    }

    pub async fn list_tunnels_for_host(
        &self,
        host_id: i64,
    ) -> Result<Vec<SavedTunnel>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, host_id, name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start FROM tunnels WHERE host_id = ?1 AND vault_id = ?2 ORDER BY id",
                (host_id, self.host_vault_id.as_str()),
            )
            .await?;
        Self::parse_tunnel_rows(&mut rows).await
    }

    pub async fn delete_tunnel(&mut self, id: i64) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "DELETE FROM tunnels WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn get_tunnel(&self, id: i64) -> Result<Option<SavedTunnel>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, host_id, name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start FROM tunnels WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        let mut list = Self::parse_tunnel_rows(&mut rows).await?;
        Ok(list.pop())
    }

    pub async fn get_tunnel_by_name(
        &self,
        name: &str,
    ) -> Result<Option<SavedTunnel>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, host_id, name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start FROM tunnels WHERE name = ?1 AND vault_id = ?2",
                (name, self.host_vault_id.as_str()),
            )
            .await?;
        let mut list = Self::parse_tunnel_rows(&mut rows).await?;
        Ok(list.pop())
    }

    async fn parse_tunnel_rows(rows: &mut turso::Rows) -> Result<Vec<SavedTunnel>, StorageError> {
        let mut tunnels = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let host_id: Option<i64> = row.get(1)?;
            let name: String = row.get(2)?;
            let ftype_str: String = row.get(3)?;
            let bind_address: Option<String> = row.get(4)?;
            let bind_port: i64 = row.get(5)?;
            let target_host: Option<String> = row.get(6)?;
            let target_port: Option<i64> = row.get(7)?;
            let auto_start: i64 = row.get(8)?;

            let ftype = ForwardingType::from_str_loose(&ftype_str)
                .ok_or_else(|| StorageError::InvalidTunnel(TunnelError::InvalidSpec(ftype_str)))?;
            let bind_port =
                u16::try_from(bind_port).map_err(|_| StorageError::InvalidPort(bind_port))?;
            let rule = match ftype {
                ForwardingType::Dynamic => {
                    PortForwardRule::dynamic(bind_address.as_deref(), bind_port)
                        .map_err(StorageError::InvalidTunnel)?
                }
                ForwardingType::Local => {
                    let thost = target_host.as_deref().unwrap_or("localhost");
                    let tport = match target_port {
                        Some(p) => u16::try_from(p).map_err(|_| StorageError::InvalidPort(p))?,
                        None => {
                            return Err(StorageError::InvalidTunnel(TunnelError::MissingTarget))
                        }
                    };
                    PortForwardRule::local(bind_address.as_deref(), bind_port, thost, tport)
                        .map_err(StorageError::InvalidTunnel)?
                }
                ForwardingType::Remote => {
                    let thost = target_host.as_deref().unwrap_or("localhost");
                    let tport = match target_port {
                        Some(p) => u16::try_from(p).map_err(|_| StorageError::InvalidPort(p))?,
                        None => {
                            return Err(StorageError::InvalidTunnel(TunnelError::MissingTarget))
                        }
                    };
                    PortForwardRule::remote(bind_address.as_deref(), bind_port, thost, tport)
                        .map_err(StorageError::InvalidTunnel)?
                }
            };
            tunnels.push(SavedTunnel {
                id,
                host_id,
                name,
                rule,
                auto_start: auto_start != 0,
            });
        }
        Ok(tunnels)
    }

    pub async fn log_transfer(&mut self, transfer: &NewTransfer<'_>) -> Result<i64, StorageError> {
        let local_path = transfer.local_path.trim();
        let remote_path = transfer.remote_path.trim();
        if local_path.is_empty() || remote_path.is_empty() {
            return Err(StorageError::InvalidTransferPath);
        }
        let dir_str = match transfer.direction {
            TransferDirection::Upload => "upload",
            TransferDirection::Download => "download",
        };

        self.connection
            .execute(
                "INSERT INTO transfer_history (host_id, direction, local_path, remote_path, bytes_transferred, total_bytes, status, error_message)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                (
                    transfer.host_id,
                    dir_str,
                    local_path,
                    remote_path,
                    transfer.bytes_transferred as i64,
                    transfer.total_bytes as i64,
                    transfer.status.trim(),
                    transfer.error_message,
                ),
            )
            .await?;
        Ok(self.connection.last_insert_rowid())
    }

    pub async fn update_transfer(
        &mut self,
        id: i64,
        bytes_transferred: u64,
        status: &str,
        error_message: Option<&str>,
        completed: bool,
    ) -> Result<bool, StorageError> {
        let rows = if completed {
            self.connection
                .execute(
                    "UPDATE transfer_history SET bytes_transferred = ?1, status = ?2, error_message = ?3, completed_at = datetime('now') WHERE id = ?4",
                    (
                        bytes_transferred as i64,
                        status.trim(),
                        error_message,
                        id,
                    ),
                )
                .await?
        } else {
            self.connection
                .execute(
                    "UPDATE transfer_history SET bytes_transferred = ?1, status = ?2, error_message = ?3 WHERE id = ?4",
                    (
                        bytes_transferred as i64,
                        status.trim(),
                        error_message,
                        id,
                    ),
                )
                .await?
        };
        Ok(rows != 0)
    }

    pub async fn list_transfers(
        &self,
        host_id: Option<i64>,
        limit: Option<usize>,
    ) -> Result<Vec<SavedTransfer>, StorageError> {
        let limit_val = limit.map(|l| l as i64).unwrap_or(-1);
        let mut rows = match host_id {
            Some(hid) => {
                self.connection
                    .query(
                        "SELECT id, host_id, direction, local_path, remote_path, bytes_transferred, total_bytes, status, error_message, created_at, completed_at
                         FROM transfer_history WHERE host_id = ?1 ORDER BY id DESC LIMIT ?2",
                        (hid, limit_val),
                    )
                    .await?
            }
            None => {
                self.connection
                    .query(
                        "SELECT id, host_id, direction, local_path, remote_path, bytes_transferred, total_bytes, status, error_message, created_at, completed_at
                         FROM transfer_history ORDER BY id DESC LIMIT ?1",
                        (limit_val,),
                    )
                    .await?
            }
        };

        let mut transfers = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let host_id: Option<i64> = row.get(1)?;
            let dir_str: String = row.get(2)?;
            let local_path: String = row.get(3)?;
            let remote_path: String = row.get(4)?;
            let bytes_transferred: i64 = row.get(5)?;
            let total_bytes: i64 = row.get(6)?;
            let status: String = row.get(7)?;
            let error_message: Option<String> = row.get(8)?;
            let created_at: String = row.get(9)?;
            let completed_at: Option<String> = row.get(10)?;

            let direction = match dir_str.as_str() {
                "upload" => TransferDirection::Upload,
                "download" => TransferDirection::Download,
                _ => TransferDirection::Upload,
            };

            transfers.push(SavedTransfer {
                id,
                host_id,
                direction,
                local_path,
                remote_path,
                bytes_transferred: bytes_transferred.max(0) as u64,
                total_bytes: total_bytes.max(0) as u64,
                status,
                error_message,
                created_at,
                completed_at,
            });
        }
        Ok(transfers)
    }

    pub async fn clear_transfer_history(
        &mut self,
        host_id: Option<i64>,
    ) -> Result<usize, StorageError> {
        let rows = match host_id {
            Some(hid) => {
                self.connection
                    .execute("DELETE FROM transfer_history WHERE host_id = ?1", (hid,))
                    .await?
            }
            None => {
                self.connection
                    .execute("DELETE FROM transfer_history", ())
                    .await?
            }
        };
        Ok(rows as usize)
    }

    pub async fn add_snippet(&mut self, snippet: &Snippet) -> Result<i64, StorageError> {
        let name = validated_name(&snippet.name)?;
        let command = snippet.command.trim();
        if command.is_empty() {
            return Err(StorageError::InvalidSnippet(SnippetError::InvalidCommand));
        }
        let tags_str = snippet.tags.join(",");
        let vars_json = serde_json::to_string(&snippet.variables).map_err(|e| {
            StorageError::InvalidSnippet(SnippetError::Serialization(e.to_string()))
        })?;

        self.connection
            .execute(
                "INSERT INTO snippets (name, command, description, tags, variables, vault_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    name,
                    command,
                    snippet.description.as_deref(),
                    tags_str.as_str(),
                    vars_json.as_str(),
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(self.connection.last_insert_rowid())
    }

    pub async fn update_snippet(
        &mut self,
        id: i64,
        snippet: &Snippet,
    ) -> Result<bool, StorageError> {
        let name = validated_name(&snippet.name)?;
        let command = snippet.command.trim();
        if command.is_empty() {
            return Err(StorageError::InvalidSnippet(SnippetError::InvalidCommand));
        }
        let tags_str = snippet.tags.join(",");
        let vars_json = serde_json::to_string(&snippet.variables).map_err(|e| {
            StorageError::InvalidSnippet(SnippetError::Serialization(e.to_string()))
        })?;

        let rows = self
            .connection
            .execute(
                "UPDATE snippets SET name = ?1, command = ?2, description = ?3, tags = ?4, variables = ?5, updated_at = datetime('now') WHERE id = ?6 AND vault_id = ?7",
                (
                    name,
                    command,
                    snippet.description.as_deref(),
                    tags_str.as_str(),
                    vars_json.as_str(),
                    id,
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn delete_snippet(&mut self, id: i64) -> Result<bool, StorageError> {
        self.connection
            .execute(
                "UPDATE hosts SET startup_snippet_id = NULL WHERE startup_snippet_id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        let rows = self
            .connection
            .execute(
                "DELETE FROM snippets WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn get_snippet(&self, id: i64) -> Result<Option<SavedSnippet>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, command, description, tags, variables, created_at, updated_at FROM snippets WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let command: String = row.get(2)?;
            let description: Option<String> = row.get(3)?;
            let tags_str: String = row.get(4)?;
            let vars_json: String = row.get(5)?;
            let created_at: String = row.get(6)?;
            let updated_at: String = row.get(7)?;

            let tags = parse_tags(&tags_str);
            let variables: Vec<SnippetVariable> =
                serde_json::from_str(&vars_json).unwrap_or_default();

            Ok(Some(SavedSnippet {
                id,
                name,
                command,
                description,
                tags,
                variables,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_snippet_by_name(
        &self,
        name: &str,
    ) -> Result<Option<SavedSnippet>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, command, description, tags, variables, created_at, updated_at FROM snippets WHERE name = ?1 AND vault_id = ?2",
                (name, self.host_vault_id.as_str()),
            )
            .await?;
        let mut list = Self::parse_snippet_rows(&mut rows).await?;
        Ok(list.pop())
    }

    pub async fn list_snippets(&self) -> Result<Vec<SavedSnippet>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, command, description, tags, variables, created_at, updated_at FROM snippets WHERE vault_id = ?1 ORDER BY name ASC",
                (self.host_vault_id.as_str(),),
            )
            .await?;
        Self::parse_snippet_rows(&mut rows).await
    }

    pub async fn list_snippets_by_tag(&self, tag: &str) -> Result<Vec<SavedSnippet>, StorageError> {
        let all = self.list_snippets().await?;
        let tag_lower = tag.trim().to_lowercase();
        Ok(all
            .into_iter()
            .filter(|s| s.tags.iter().any(|t| t.to_lowercase() == tag_lower))
            .collect())
    }

    pub async fn search_snippets(&self, query: &str) -> Result<Vec<SavedSnippet>, StorageError> {
        let query_trimmed = query.trim().to_lowercase();
        if query_trimmed.is_empty() {
            return self.list_snippets().await;
        }
        let all = self.list_snippets().await?;
        Ok(all
            .into_iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_trimmed)
                    || s.command.to_lowercase().contains(&query_trimmed)
                    || s.description
                        .as_deref()
                        .is_some_and(|d| d.to_lowercase().contains(&query_trimmed))
                    || s.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_trimmed))
            })
            .collect())
    }

    async fn parse_snippet_rows(rows: &mut turso::Rows) -> Result<Vec<SavedSnippet>, StorageError> {
        let mut snippets = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let command: String = row.get(2)?;
            let description: Option<String> = row.get(3)?;
            let tags_str: String = row.get(4)?;
            let vars_json: String = row.get(5)?;
            let created_at: String = row.get(6)?;
            let updated_at: String = row.get(7)?;

            let tags = parse_tags(&tags_str);
            let variables: Vec<SnippetVariable> =
                serde_json::from_str(&vars_json).unwrap_or_default();

            snippets.push(SavedSnippet {
                id,
                name,
                command,
                description,
                tags,
                variables,
                created_at,
                updated_at,
            });
        }
        Ok(snippets)
    }

    pub async fn add_workspace(
        &mut self,
        name: &str,
        description: Option<&str>,
        layout: &str,
        sessions: &[WorkspaceSession],
        tags: &[String],
    ) -> Result<i64, StorageError> {
        let name_trimmed = name.trim();
        if name_trimmed.is_empty() {
            return Err(StorageError::InvalidWorkspace(WorkspaceError::InvalidName));
        }
        let sessions_json = serde_json::to_string(sessions)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let tags_serialized = validated_tags(tags)?;

        self.connection
            .execute(
                "INSERT INTO workspaces (name, description, layout, sessions_json, tags, vault_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    name_trimmed,
                    description,
                    layout,
                    sessions_json,
                    tags_serialized,
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(self.connection.last_insert_rowid())
    }

    pub async fn update_workspace(
        &mut self,
        id: i64,
        name: &str,
        description: Option<&str>,
        layout: &str,
        sessions: &[WorkspaceSession],
        tags: &[String],
    ) -> Result<bool, StorageError> {
        let name_trimmed = name.trim();
        if name_trimmed.is_empty() {
            return Err(StorageError::InvalidWorkspace(WorkspaceError::InvalidName));
        }
        let sessions_json = serde_json::to_string(sessions)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let tags_serialized = validated_tags(tags)?;

        let rows = self
            .connection
            .execute(
                "UPDATE workspaces SET name = ?1, description = ?2, layout = ?3, sessions_json = ?4, tags = ?5, updated_at = datetime('now') WHERE id = ?6 AND vault_id = ?7",
                (
                    name_trimmed,
                    description,
                    layout,
                    sessions_json,
                    tags_serialized,
                    id,
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;
        Ok(rows > 0)
    }

    pub async fn get_workspace(&self, id: i64) -> Result<Option<SavedWorkspace>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, description, layout, sessions_json, tags, created_at, updated_at FROM workspaces WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let description: Option<String> = row.get(2)?;
            let layout: String = row.get(3)?;
            let sessions_raw: String = row.get(4)?;
            let tags_raw: String = row.get(5)?;
            let created_at: String = row.get(6)?;
            let updated_at: String = row.get(7)?;

            let sessions: Vec<WorkspaceSession> =
                serde_json::from_str(&sessions_raw).unwrap_or_default();
            let tags = parse_tags(&tags_raw);

            Ok(Some(SavedWorkspace {
                id,
                name,
                description,
                layout,
                sessions,
                tags,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_workspace_by_name(
        &self,
        name: &str,
    ) -> Result<Option<SavedWorkspace>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, description, layout, sessions_json, tags, created_at, updated_at FROM workspaces WHERE name = ?1 AND vault_id = ?2",
                (name, self.host_vault_id.as_str()),
            )
            .await?;
        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let description: Option<String> = row.get(2)?;
            let layout: String = row.get(3)?;
            let sessions_raw: String = row.get(4)?;
            let tags_raw: String = row.get(5)?;
            let created_at: String = row.get(6)?;
            let updated_at: String = row.get(7)?;

            let sessions: Vec<WorkspaceSession> =
                serde_json::from_str(&sessions_raw).unwrap_or_default();
            let tags = parse_tags(&tags_raw);

            list.push(SavedWorkspace {
                id,
                name,
                description,
                layout,
                sessions,
                tags,
                created_at,
                updated_at,
            });
        }
        Ok(list.pop())
    }

    pub async fn list_workspaces(&self) -> Result<Vec<SavedWorkspace>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, description, layout, sessions_json, tags, created_at, updated_at FROM workspaces WHERE vault_id = ?1 ORDER BY id ASC",
                (self.host_vault_id.as_str(),),
            )
            .await?;
        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let description: Option<String> = row.get(2)?;
            let layout: String = row.get(3)?;
            let sessions_raw: String = row.get(4)?;
            let tags_raw: String = row.get(5)?;
            let created_at: String = row.get(6)?;
            let updated_at: String = row.get(7)?;

            let sessions: Vec<WorkspaceSession> =
                serde_json::from_str(&sessions_raw).unwrap_or_default();
            let tags = parse_tags(&tags_raw);

            list.push(SavedWorkspace {
                id,
                name,
                description,
                layout,
                sessions,
                tags,
                created_at,
                updated_at,
            });
        }
        Ok(list)
    }

    pub async fn delete_workspace(&mut self, id: i64) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "DELETE FROM workspaces WHERE id = ?1 AND vault_id = ?2",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        Ok(rows > 0)
    }

    pub async fn duplicate_workspace(
        &mut self,
        id: i64,
        new_name: &str,
    ) -> Result<i64, StorageError> {
        let ws = self
            .get_workspace(id)
            .await?
            .ok_or(StorageError::InvalidWorkspace(WorkspaceError::InvalidName))?;
        self.add_workspace(
            new_name,
            ws.description.as_deref(),
            &ws.layout,
            &ws.sessions,
            &ws.tags,
        )
        .await
    }

    // =========================================================================
    // Team Collaboration & Shared Vault Operations
    // =========================================================================

    pub async fn create_shared_vault(&mut self, vault: &SharedVault) -> Result<(), StorageError> {
        self.connection
            .execute(
                "INSERT INTO shared_vaults (id, name, description, current_epoch, owner_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                (
                    vault.id.clone(),
                    vault.name.clone(),
                    vault.description.clone(),
                    vault.current_epoch as i64,
                    vault.owner_id.clone(),
                    vault.created_at.clone(),
                    vault.updated_at.clone(),
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn get_shared_vault(&self, id: &str) -> Result<Option<SharedVault>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, description, current_epoch, owner_id, created_at, updated_at FROM shared_vaults WHERE id = ?1",
                (id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let description: Option<String> = row.get(2)?;
            let epoch: i64 = row.get(3)?;
            let owner_id: String = row.get(4)?;
            let created_at: String = row.get(5)?;
            let updated_at: String = row.get(6)?;

            Ok(Some(SharedVault {
                id,
                name,
                description,
                current_epoch: epoch as u64,
                owner_id,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_shared_vaults(&self) -> Result<Vec<SharedVault>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, description, current_epoch, owner_id, created_at, updated_at FROM shared_vaults ORDER BY name ASC",
                (),
            )
            .await?;
        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let description: Option<String> = row.get(2)?;
            let epoch: i64 = row.get(3)?;
            let owner_id: String = row.get(4)?;
            let created_at: String = row.get(5)?;
            let updated_at: String = row.get(6)?;

            list.push(SharedVault {
                id,
                name,
                description,
                current_epoch: epoch as u64,
                owner_id,
                created_at,
                updated_at,
            });
        }
        Ok(list)
    }

    pub async fn update_shared_vault_epoch(
        &mut self,
        id: &str,
        new_epoch: u64,
        updated_at: &str,
    ) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "UPDATE shared_vaults SET current_epoch = ?1, updated_at = ?2 WHERE id = ?3",
                (new_epoch as i64, updated_at, id),
            )
            .await?;
        Ok(rows > 0)
    }

    pub async fn delete_shared_vault(&mut self, id: &str) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute("DELETE FROM shared_vaults WHERE id = ?1", (id,))
            .await?;
        Ok(rows > 0)
    }

    pub async fn add_team_member(&mut self, member: &TeamMember) -> Result<(), StorageError> {
        self.connection
            .execute(
                "INSERT INTO team_members (id, vault_id, user_id, display_name, public_key, role, enrolled_at, invited_by, revoked, revoked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                (
                    member.id.clone(),
                    member.vault_id.clone(),
                    member.user_id.clone(),
                    member.display_name.clone(),
                    member.public_key.clone(),
                    member.role.to_string(),
                    member.enrolled_at.clone(),
                    member.invited_by.clone(),
                    if member.revoked { 1i64 } else { 0i64 },
                    member.revoked_at.clone(),
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn get_team_member(&self, id: &str) -> Result<Option<TeamMember>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, vault_id, user_id, display_name, public_key, role, enrolled_at, invited_by, revoked, revoked_at FROM team_members WHERE id = ?1",
                (id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let vault_id: String = row.get(1)?;
            let user_id: String = row.get(2)?;
            let display_name: String = row.get(3)?;
            let public_key: String = row.get(4)?;
            let role_str: String = row.get(5)?;
            let enrolled_at: String = row.get(6)?;
            let invited_by: String = row.get(7)?;
            let revoked_int: i64 = row.get(8)?;
            let revoked_at: Option<String> = row.get(9)?;

            let role = TeamRole::from_str(&role_str).unwrap_or(TeamRole::Viewer);

            Ok(Some(TeamMember {
                id,
                vault_id,
                user_id,
                display_name,
                public_key,
                role,
                enrolled_at,
                invited_by,
                revoked: revoked_int != 0,
                revoked_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_team_members(&self, vault_id: &str) -> Result<Vec<TeamMember>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, vault_id, user_id, display_name, public_key, role, enrolled_at, invited_by, revoked, revoked_at FROM team_members WHERE vault_id = ?1 ORDER BY enrolled_at ASC",
                (vault_id,),
            )
            .await?;
        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let vault_id: String = row.get(1)?;
            let user_id: String = row.get(2)?;
            let display_name: String = row.get(3)?;
            let public_key: String = row.get(4)?;
            let role_str: String = row.get(5)?;
            let enrolled_at: String = row.get(6)?;
            let invited_by: String = row.get(7)?;
            let revoked_int: i64 = row.get(8)?;
            let revoked_at: Option<String> = row.get(9)?;

            let role = TeamRole::from_str(&role_str).unwrap_or(TeamRole::Viewer);

            list.push(TeamMember {
                id,
                vault_id,
                user_id,
                display_name,
                public_key,
                role,
                enrolled_at,
                invited_by,
                revoked: revoked_int != 0,
                revoked_at,
            });
        }
        Ok(list)
    }

    pub async fn update_team_member_role(
        &mut self,
        id: &str,
        new_role: TeamRole,
    ) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "UPDATE team_members SET role = ?1 WHERE id = ?2",
                (new_role.to_string(), id),
            )
            .await?;
        Ok(rows > 0)
    }

    pub async fn revoke_team_member(
        &mut self,
        id: &str,
        revoked_at: &str,
    ) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute(
                "UPDATE team_members SET revoked = 1, revoked_at = ?1 WHERE id = ?2",
                (revoked_at, id),
            )
            .await?;
        Ok(rows > 0)
    }

    pub async fn log_audit_event(
        &mut self,
        record: &SharingAuditRecord,
    ) -> Result<(), StorageError> {
        self.connection
            .execute(
                "INSERT INTO sharing_audit_log (id, vault_id, action, actor_id, target_id, details, timestamp_utc)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                (
                    record.id.clone(),
                    record.vault_id.clone(),
                    record.action.to_string(),
                    record.actor_id.clone(),
                    record.target_id.clone(),
                    record.details.clone(),
                    record.timestamp_utc.clone(),
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn list_audit_log(
        &self,
        vault_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SharingAuditRecord>, StorageError> {
        let lim = limit.unwrap_or(100) as i64;
        let mut rows = self
            .connection
            .query(
                "SELECT id, vault_id, action, actor_id, target_id, details, timestamp_utc FROM sharing_audit_log WHERE vault_id = ?1 ORDER BY timestamp_utc DESC LIMIT ?2",
                (vault_id, lim),
            )
            .await?;
        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let vault_id: String = row.get(1)?;
            let action_str: String = row.get(2)?;
            let actor_id: String = row.get(3)?;
            let target_id: Option<String> = row.get(4)?;
            let details: Option<String> = row.get(5)?;
            let timestamp_utc: String = row.get(6)?;

            let action = AuditAction::from_str(&action_str).unwrap_or(AuditAction::RecordShared);

            list.push(SharingAuditRecord {
                id,
                vault_id,
                action,
                actor_id,
                target_id,
                details,
                timestamp_utc,
            });
        }
        Ok(list)
    }

    pub async fn create_shared_session_log(
        &mut self,
        log: &SharedSessionLog,
    ) -> Result<(), StorageError> {
        let comments_raw = serde_json::to_string(&log.comments)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let bookmarks_raw = serde_json::to_string(&log.bookmarks)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        self.connection
            .execute(
                "INSERT INTO shared_session_logs (id, vault_id, host_name, author_id, started_at, ended_at, retention_days, comments_json, bookmarks_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                (
                    log.id.clone(),
                    log.vault_id.clone(),
                    log.host_name.clone(),
                    log.author_id.clone(),
                    log.started_at.clone(),
                    log.ended_at.clone(),
                    log.retention_days as i64,
                    comments_raw,
                    bookmarks_raw,
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn list_shared_session_logs(
        &self,
        vault_id: &str,
    ) -> Result<Vec<SharedSessionLog>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, vault_id, host_name, author_id, started_at, ended_at, retention_days, comments_json, bookmarks_json FROM shared_session_logs WHERE vault_id = ?1 ORDER BY started_at DESC",
                (vault_id,),
            )
            .await?;
        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let vault_id: String = row.get(1)?;
            let host_name: String = row.get(2)?;
            let author_id: String = row.get(3)?;
            let started_at: String = row.get(4)?;
            let ended_at: Option<String> = row.get(5)?;
            let retention: i64 = row.get(6)?;
            let comments_raw: String = row.get(7)?;
            let bookmarks_raw: String = row.get(8)?;

            let comments: Vec<LogComment> = serde_json::from_str(&comments_raw).unwrap_or_default();
            let bookmarks: Vec<LogBookmark> =
                serde_json::from_str(&bookmarks_raw).unwrap_or_default();

            list.push(SharedSessionLog {
                id,
                vault_id,
                host_name,
                author_id,
                started_at,
                ended_at,
                retention_days: retention as u32,
                comments,
                bookmarks,
            });
        }
        Ok(list)
    }

    // =========================================================================
    // Infrastructure Integrations: Cloud Providers & Discovered Hosts
    // =========================================================================

    pub async fn create_cloud_provider(
        &mut self,
        provider: &CloudProviderRecord,
    ) -> Result<(), StorageError> {
        self.connection
            .execute(
                "INSERT INTO cloud_providers (id, name, provider_kind, config_json, sync_interval_mins, last_synced_at, auto_sync, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                (
                    provider.id.clone(),
                    provider.name.clone(),
                    provider.provider_kind.clone(),
                    provider.config_json.clone(),
                    provider.sync_interval_mins,
                    provider.last_synced_at,
                    if provider.auto_sync { 1i64 } else { 0i64 },
                    provider.created_at,
                    provider.updated_at,
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn get_cloud_provider(
        &self,
        id: &str,
    ) -> Result<Option<CloudProviderRecord>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, provider_kind, config_json, sync_interval_mins, last_synced_at, auto_sync, created_at, updated_at FROM cloud_providers WHERE id = ?1",
                (id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let provider_kind: String = row.get(2)?;
            let config_json: String = row.get(3)?;
            let sync_interval_mins: i64 = row.get(4)?;
            let last_synced_at: Option<i64> = row.get(5)?;
            let auto_sync_int: i64 = row.get(6)?;
            let created_at: i64 = row.get(7)?;
            let updated_at: i64 = row.get(8)?;

            Ok(Some(CloudProviderRecord {
                id,
                name,
                provider_kind,
                config_json,
                sync_interval_mins,
                last_synced_at,
                auto_sync: auto_sync_int != 0,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_cloud_providers(&self) -> Result<Vec<CloudProviderRecord>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, provider_kind, config_json, sync_interval_mins, last_synced_at, auto_sync, created_at, updated_at FROM cloud_providers ORDER BY name ASC",
                (),
            )
            .await?;
        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let provider_kind: String = row.get(2)?;
            let config_json: String = row.get(3)?;
            let sync_interval_mins: i64 = row.get(4)?;
            let last_synced_at: Option<i64> = row.get(5)?;
            let auto_sync_int: i64 = row.get(6)?;
            let created_at: i64 = row.get(7)?;
            let updated_at: i64 = row.get(8)?;

            list.push(CloudProviderRecord {
                id,
                name,
                provider_kind,
                config_json,
                sync_interval_mins,
                last_synced_at,
                auto_sync: auto_sync_int != 0,
                created_at,
                updated_at,
            });
        }
        Ok(list)
    }

    pub async fn update_cloud_provider_sync_time(
        &mut self,
        id: &str,
        last_synced_at: i64,
    ) -> Result<bool, StorageError> {
        let affected = self
            .connection
            .execute(
                "UPDATE cloud_providers SET last_synced_at = ?1, updated_at = ?1 WHERE id = ?2",
                (last_synced_at, id),
            )
            .await?;
        Ok(affected > 0)
    }

    pub async fn delete_cloud_provider(&mut self, id: &str) -> Result<bool, StorageError> {
        let affected = self
            .connection
            .execute("DELETE FROM cloud_providers WHERE id = ?1", (id,))
            .await?;
        Ok(affected > 0)
    }

    pub async fn save_discovered_hosts(
        &mut self,
        provider_id: &str,
        hosts: &[DiscoveredHostRecord],
    ) -> Result<usize, StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        // Delete existing discovered hosts for this provider
        tx.execute(
            "DELETE FROM discovered_hosts WHERE provider_id = ?1",
            (provider_id,),
        )
        .await?;

        let mut count = 0;
        for h in hosts {
            tx.execute(
                "INSERT INTO discovered_hosts (id, provider_id, external_id, name, hostname, port, username, state, region, group_path, tags_json, metadata_json, imported_host_id, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                (
                    h.id.clone(),
                    provider_id,
                    h.external_id.clone(),
                    h.name.clone(),
                    h.hostname.clone(),
                    h.port,
                    h.username.clone(),
                    h.state.clone(),
                    h.region.clone(),
                    h.group_path.clone(),
                    h.tags_json.clone(),
                    h.metadata_json.clone(),
                    h.imported_host_id,
                    h.last_seen_at,
                ),
            )
            .await?;
            count += 1;
        }

        tx.commit().await?;
        Ok(count)
    }

    pub async fn list_discovered_hosts(
        &self,
        provider_id: Option<&str>,
    ) -> Result<Vec<DiscoveredHostRecord>, StorageError> {
        let (query, param): (&str, Option<String>) = if let Some(pid) = provider_id {
            (
                "SELECT id, provider_id, external_id, name, hostname, port, username, state, region, group_path, tags_json, metadata_json, imported_host_id, last_seen_at FROM discovered_hosts WHERE provider_id = ?1 ORDER BY name ASC",
                Some(pid.to_string()),
            )
        } else {
            (
                "SELECT id, provider_id, external_id, name, hostname, port, username, state, region, group_path, tags_json, metadata_json, imported_host_id, last_seen_at FROM discovered_hosts ORDER BY name ASC",
                None,
            )
        };

        let mut rows = if let Some(ref p) = param {
            self.connection.query(query, (p.as_str(),)).await?
        } else {
            self.connection.query(query, ()).await?
        };

        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let provider_id: String = row.get(1)?;
            let external_id: String = row.get(2)?;
            let name: String = row.get(3)?;
            let hostname: String = row.get(4)?;
            let port: i64 = row.get(5)?;
            let username: Option<String> = row.get(6)?;
            let state: String = row.get(7)?;
            let region: Option<String> = row.get(8)?;
            let group_path: Option<String> = row.get(9)?;
            let tags_json: String = row.get(10)?;
            let metadata_json: String = row.get(11)?;
            let imported_host_id: Option<i64> = row.get(12)?;
            let last_seen_at: i64 = row.get(13)?;

            list.push(DiscoveredHostRecord {
                id,
                provider_id,
                external_id,
                name,
                hostname,
                port,
                username,
                state,
                region,
                group_path,
                tags_json,
                metadata_json,
                imported_host_id,
                last_seen_at,
            });
        }
        Ok(list)
    }

    pub async fn mark_discovered_host_imported(
        &mut self,
        discovered_id: &str,
        imported_host_id: i64,
    ) -> Result<bool, StorageError> {
        let affected = self
            .connection
            .execute(
                "UPDATE discovered_hosts SET imported_host_id = ?1 WHERE id = ?2",
                (imported_host_id, discovered_id),
            )
            .await?;
        Ok(affected > 0)
    }

    /// Exports all hosts, groups, tunnels, and snippets into an encrypted `SyncVault` using default KDF iterations.
    pub async fn export_vault(
        &self,
        passphrase: &str,
        device_id: &str,
    ) -> Result<shellumi_core::SyncVault, StorageError> {
        self.export_vault_with_iterations(
            passphrase,
            device_id,
            shellumi_core::DEFAULT_KDF_ITERATIONS,
        )
        .await
    }

    /// Exports all hosts, groups, tunnels, and snippets into an encrypted `SyncVault` with custom KDF iterations.
    pub async fn export_vault_with_iterations(
        &self,
        passphrase: &str,
        device_id: &str,
        iterations: u32,
    ) -> Result<shellumi_core::SyncVault, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut vault = shellumi_core::SyncVault::new(self.vault_id().as_str(), now);
        vault.schema_version = 2;

        // Include encrypted directory describing other vaults during default-vault sync
        if self.vault_id().as_str() == LEGACY_LOCAL_VAULT_ID {
            if let Ok(Some(enc_dir)) = self
                .get_encrypted_vault_directory("primary_directory")
                .await
            {
                vault.encrypted_directory = Some(enc_dir);
            }
        }

        // Include key envelope for this vault:
        // Try to load primary envelope from VaultManifest on disk first
        let manifest_opt = if let Some(ref dbp) = self.db_path {
            let p = std::path::Path::new(dbp);
            let parent = p.parent().unwrap_or(p);
            let root = if parent.file_name() == Some(std::ffi::OsStr::new("vaults")) {
                parent.parent().unwrap_or(parent)
            } else {
                parent
            };
            let vid = self.vault_id().as_str();
            let mfile = root
                .join("vault_manifests")
                .join(format!("{vid}.manifest.json"));
            if mfile.exists() {
                shellumi_core::VaultManifest::load_from_path(&mfile).ok()
            } else {
                let direct_mfile = std::path::PathBuf::from(format!("{dbp}.manifest"));
                if direct_mfile.exists() {
                    shellumi_core::VaultManifest::load_from_path(&direct_mfile).ok()
                } else {
                    None
                }
            }
        } else {
            None
        };

        if let Some(manifest) = manifest_opt {
            vault.key_envelope = Some(manifest.primary_envelope);
        } else if let Ok(Some(env_str)) = self
            .get_vault_recovery_envelope(self.vault_id().as_str())
            .await
        {
            if let Ok(env) = serde_json::from_str::<shellumi_core::EncryptedEnvelope>(&env_str) {
                vault.key_envelope = Some(env);
            }
        }

        // Export identities
        let identities = self.list_identities().await?;
        for ident in identities {
            let payload = SyncIdentityPayload {
                name: ident.name.clone(),
                username: ident.username.clone(),
                auth_method: ident.auth_method.clone(),
                password_secret_ref: ident.password_secret_ref.clone(),
                key_path: ident.key_path.clone(),
                passphrase_secret_ref: ident.passphrase_secret_ref.clone(),
                certificate_path: ident.certificate_path.clone(),
                tags: ident.tags.clone(),
                comment: ident.comment.clone(),
            };
            let envelope = shellumi_core::encrypt_entity_with_iterations(
                passphrase,
                "identity",
                &ident.name,
                &payload,
                iterations,
            )?;
            vault.records.push(shellumi_core::SyncRecord::new(
                ident.name,
                shellumi_core::SyncEntityType::Identity,
                1,
                device_id,
                now,
                envelope,
            ));
        }

        // Export groups
        let groups = self.list_groups().await?;
        for group in groups {
            let payload = SyncGroupPayload {
                name: group.name.clone(),
                default_username: group.default_username.clone(),
                default_port: group.default_port.map(|p| p.get()),
                tags: group.tags.clone(),
                identity_id: group.identity_id,
            };
            let envelope = shellumi_core::encrypt_entity_with_iterations(
                passphrase,
                "group",
                &group.name,
                &payload,
                iterations,
            )?;
            vault.records.push(shellumi_core::SyncRecord::new(
                group.name,
                shellumi_core::SyncEntityType::Group,
                1,
                device_id,
                now,
                envelope,
            ));
        }

        // Export hosts
        let hosts = self.list().await?;
        for host in hosts {
            let payload = SyncHostPayload {
                name: host.name.clone(),
                hostname: host.destination.host().to_string(),
                username: host.destination.username().to_string(),
                port: host.destination.port().get(),
                group_name: host.group_name.clone(),
                tags: host.tags.clone(),
                jump_host: host.jump_host.clone(),
                startup_snippet_id: host.startup_snippet_id,
                protocol: host.protocol.clone(),
                protocol_config: host.protocol_config.clone(),
                identity_id: host.identity_id,
            };
            let object_id = format!("{}:{}", host.name, host.destination.host());
            let envelope = shellumi_core::encrypt_entity_with_iterations(
                passphrase, "host", &object_id, &payload, iterations,
            )?;
            vault.records.push(shellumi_core::SyncRecord::new(
                object_id,
                shellumi_core::SyncEntityType::Host,
                1,
                device_id,
                now,
                envelope,
            ));
        }

        // Export tunnels
        let tunnels = self.list_tunnels().await?;
        for tunnel in tunnels {
            let payload = SyncTunnelPayload {
                name: tunnel.name.clone(),
                forwarding_type: tunnel.rule.forwarding_type.as_str().to_string(),
                bind_address: tunnel.rule.bind_address.clone(),
                bind_port: tunnel.rule.bind_port,
                target_host: tunnel.rule.target_host.clone(),
                target_port: tunnel.rule.target_port,
                auto_start: tunnel.auto_start,
            };
            let envelope = shellumi_core::encrypt_entity_with_iterations(
                passphrase,
                "tunnel",
                &tunnel.name,
                &payload,
                iterations,
            )?;
            vault.records.push(shellumi_core::SyncRecord::new(
                tunnel.name,
                shellumi_core::SyncEntityType::Tunnel,
                1,
                device_id,
                now,
                envelope,
            ));
        }

        // Export snippets
        let snippets = self.list_snippets().await?;
        for snippet in snippets {
            let payload = SyncSnippetPayload {
                name: snippet.name.clone(),
                command: snippet.command.clone(),
                description: snippet.description.clone(),
                tags: snippet.tags.clone(),
            };
            let envelope = shellumi_core::encrypt_entity_with_iterations(
                passphrase,
                "snippet",
                &snippet.name,
                &payload,
                iterations,
            )?;
            vault.records.push(shellumi_core::SyncRecord::new(
                snippet.name,
                shellumi_core::SyncEntityType::Snippet,
                1,
                device_id,
                now,
                envelope,
            ));
        }

        // Export workspaces
        let workspaces = self.list_workspaces().await?;
        for ws in workspaces {
            let payload = SyncWorkspacePayload {
                name: ws.name.clone(),
                description: ws.description.clone(),
                layout: ws.layout.clone(),
                sessions: ws.sessions.clone(),
                tags: ws.tags.clone(),
            };
            let envelope = shellumi_core::encrypt_entity_with_iterations(
                passphrase,
                "workspace",
                &ws.name,
                &payload,
                iterations,
            )?;
            vault.records.push(shellumi_core::SyncRecord::new(
                ws.name,
                shellumi_core::SyncEntityType::Workspace,
                1,
                device_id,
                now,
                envelope,
            ));
        }

        // Export shared vaults and team members
        let shared_vaults = self.list_shared_vaults().await?;
        for sv in &shared_vaults {
            let envelope = shellumi_core::encrypt_entity_with_iterations(
                passphrase,
                "shared_vault",
                &sv.id,
                sv,
                iterations,
            )?;
            vault.records.push(shellumi_core::SyncRecord::new(
                sv.id.clone(),
                shellumi_core::SyncEntityType::SharedVault,
                sv.current_epoch,
                device_id,
                now,
                envelope,
            ));

            let members = self.list_team_members(&sv.id).await?;
            for member in members {
                let envelope = shellumi_core::encrypt_entity_with_iterations(
                    passphrase,
                    "team_member",
                    &member.id,
                    &member,
                    iterations,
                )?;
                vault.records.push(shellumi_core::SyncRecord::new(
                    member.id.clone(),
                    shellumi_core::SyncEntityType::TeamMember,
                    1,
                    device_id,
                    now,
                    envelope,
                ));
            }
        }

        // Export cloud providers
        if let Ok(providers) = self.list_cloud_providers().await {
            for prov in &providers {
                let envelope = shellumi_core::encrypt_entity_with_iterations(
                    passphrase,
                    "cloud_provider",
                    &prov.id,
                    prov,
                    iterations,
                )?;
                vault.records.push(shellumi_core::SyncRecord::new(
                    prov.id.clone(),
                    shellumi_core::SyncEntityType::CloudProvider,
                    1,
                    device_id,
                    now,
                    envelope,
                ));
            }
        }

        let _ = self.archive_snapshot(&vault.vault_id, &vault, 3).await;
        Ok(vault)
    }

    /// Imports records from a decrypted `SyncVault`, restoring or updating local records.
    pub async fn import_vault(
        &mut self,
        passphrase: &str,
        vault: &shellumi_core::SyncVault,
    ) -> Result<ImportVaultSummary, StorageError> {
        let mut summary = ImportVaultSummary::default();

        // Restore encrypted vault directory envelope if present in sync payload
        if let Some(enc_dir) = &vault.encrypted_directory {
            let _ = self
                .save_encrypted_vault_directory("primary_directory", enc_dir)
                .await;
        }

        // Restore key envelope if present in sync payload
        if let Some(key_env) = &vault.key_envelope {
            if let Ok(json) = serde_json::to_string(key_env) {
                let vid = self.vault_id().as_str().to_string();
                let _ = self.save_vault_recovery_envelope(&vid, &json).await;
            }
            // Update manifest on disk if primary envelope rotated
            if let Some(ref dbp) = self.db_path {
                let p = std::path::Path::new(dbp);
                let parent = p.parent().unwrap_or(p);
                let root = if parent.file_name() == Some(std::ffi::OsStr::new("vaults")) {
                    parent.parent().unwrap_or(parent)
                } else {
                    parent
                };
                let vid = self.vault_id().as_str();
                let mfile = root
                    .join("vault_manifests")
                    .join(format!("{vid}.manifest.json"));
                if mfile.exists() {
                    if let Ok(mut manifest) = shellumi_core::VaultManifest::load_from_path(&mfile) {
                        if manifest.primary_envelope != *key_env {
                            manifest.primary_envelope = key_env.clone();
                            let _ = manifest.save_to_path(&mfile);
                        }
                    }
                }
            }
        }

        for record in &vault.records {
            if record.deleted {
                continue;
            }
            let Some(envelope) = &record.envelope else {
                continue;
            };

            match record.entity_type {
                shellumi_core::SyncEntityType::Group => {
                    match shellumi_core::decrypt_entity::<SyncGroupPayload>(passphrase, envelope) {
                        Ok(payload) => {
                            let group = HostGroup::new(
                                &payload.name,
                                payload.default_username.as_deref(),
                                payload.default_port,
                                &payload.tags,
                            )
                            .map_err(StorageError::InvalidGroup)?;

                            if self.get_group(&group.name).await?.is_some() {
                                let _ = self.update_group(&group).await;
                            } else {
                                let _ = self.add_group(&group).await;
                            }
                            summary.groups_imported += 1;
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("Group {}: {e}", record.object_id)),
                    }
                }
                shellumi_core::SyncEntityType::Host => {
                    match shellumi_core::decrypt_entity::<SyncHostPayload>(passphrase, envelope) {
                        Ok(payload) => {
                            if let Ok(dest) = SshDestination::new(
                                &payload.hostname,
                                &payload.username,
                                payload.port,
                            ) {
                                let existing = self.search(&payload.name).await?;
                                let matched = existing.iter().find(|h| h.name == payload.name);
                                if let Some(m) = matched {
                                    let _ = self
                                        .update_with_protocol(
                                            m.id,
                                            &payload.name,
                                            &dest,
                                            payload.group_name.as_deref(),
                                            &payload.tags,
                                            payload.jump_host.as_deref(),
                                            payload.startup_snippet_id,
                                            &payload.protocol,
                                            payload.protocol_config.as_deref(),
                                        )
                                        .await;
                                } else {
                                    let _ = self
                                        .add_with_protocol(
                                            &payload.name,
                                            &dest,
                                            payload.group_name.as_deref(),
                                            &payload.tags,
                                            payload.jump_host.as_deref(),
                                            payload.startup_snippet_id,
                                            &payload.protocol,
                                            payload.protocol_config.as_deref(),
                                        )
                                        .await;
                                }
                                summary.hosts_imported += 1;
                            }
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("Host {}: {e}", record.object_id)),
                    }
                }
                shellumi_core::SyncEntityType::Tunnel => {
                    match shellumi_core::decrypt_entity::<SyncTunnelPayload>(passphrase, envelope) {
                        Ok(payload) => {
                            let rule_res =
                                match payload.forwarding_type.as_str() {
                                    "local" => payload.target_host.zip(payload.target_port).map(
                                        |(th, tp)| {
                                            PortForwardRule::local(
                                                payload.bind_address.as_deref(),
                                                payload.bind_port,
                                                &th,
                                                tp,
                                            )
                                        },
                                    ),
                                    "remote" => payload.target_host.zip(payload.target_port).map(
                                        |(th, tp)| {
                                            PortForwardRule::remote(
                                                payload.bind_address.as_deref(),
                                                payload.bind_port,
                                                &th,
                                                tp,
                                            )
                                        },
                                    ),
                                    "dynamic" => Some(PortForwardRule::dynamic(
                                        payload.bind_address.as_deref(),
                                        payload.bind_port,
                                    )),
                                    _ => None,
                                };
                            if let Some(Ok(rule)) = rule_res {
                                let _ = self
                                    .add_tunnel(&payload.name, None, &rule, payload.auto_start)
                                    .await;
                                summary.tunnels_imported += 1;
                            }
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("Tunnel {}: {e}", record.object_id)),
                    }
                }
                shellumi_core::SyncEntityType::Snippet => {
                    match shellumi_core::decrypt_entity::<SyncSnippetPayload>(passphrase, envelope)
                    {
                        Ok(payload) => {
                            if let Ok(snippet) = Snippet::new(
                                &payload.name,
                                &payload.command,
                                payload.description,
                                &payload.tags,
                            ) {
                                let _ = self.add_snippet(&snippet).await;
                                summary.snippets_imported += 1;
                            }
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("Snippet {}: {e}", record.object_id)),
                    }
                }
                shellumi_core::SyncEntityType::Workspace => {
                    match shellumi_core::decrypt_entity::<SyncWorkspacePayload>(
                        passphrase, envelope,
                    ) {
                        Ok(payload) => {
                            let existing = self.list_workspaces().await?;
                            if let Some(target) = existing.iter().find(|w| w.name == payload.name) {
                                let _ = self
                                    .update_workspace(
                                        target.id,
                                        &payload.name,
                                        payload.description.as_deref(),
                                        &payload.layout,
                                        &payload.sessions,
                                        &payload.tags,
                                    )
                                    .await;
                            } else {
                                let _ = self
                                    .add_workspace(
                                        &payload.name,
                                        payload.description.as_deref(),
                                        &payload.layout,
                                        &payload.sessions,
                                        &payload.tags,
                                    )
                                    .await;
                            }
                            summary.workspaces_imported += 1;
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("Workspace {}: {e}", record.object_id)),
                    }
                }
                shellumi_core::SyncEntityType::SharedVault => {
                    match shellumi_core::decrypt_entity::<SharedVault>(passphrase, envelope) {
                        Ok(vault) => {
                            if self.get_shared_vault(&vault.id).await?.is_none() {
                                let _ = self.create_shared_vault(&vault).await;
                            }
                            summary.shared_vaults_imported += 1;
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("SharedVault {}: {e}", record.object_id)),
                    }
                }
                shellumi_core::SyncEntityType::TeamMember => {
                    match shellumi_core::decrypt_entity::<TeamMember>(passphrase, envelope) {
                        Ok(member) => {
                            if self.get_team_member(&member.id).await?.is_none() {
                                let _ = self.add_team_member(&member).await;
                            }
                            summary.team_members_imported += 1;
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("TeamMember {}: {e}", record.object_id)),
                    }
                }
                shellumi_core::SyncEntityType::AuditRecord => {
                    match shellumi_core::decrypt_entity::<SharingAuditRecord>(passphrase, envelope)
                    {
                        Ok(audit) => {
                            let _ = self.log_audit_event(&audit).await;
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("AuditRecord {}: {e}", record.object_id)),
                    }
                }
                shellumi_core::SyncEntityType::CloudProvider => {
                    match shellumi_core::decrypt_entity::<CloudProviderRecord>(passphrase, envelope)
                    {
                        Ok(provider) => {
                            if self.get_cloud_provider(&provider.id).await?.is_none() {
                                let _ = self.create_cloud_provider(&provider).await;
                            }
                            summary.cloud_providers_imported += 1;
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("CloudProvider {}: {e}", record.object_id)),
                    }
                }
                shellumi_core::SyncEntityType::Identity => {
                    match shellumi_core::decrypt_entity::<SyncIdentityPayload>(passphrase, envelope)
                    {
                        Ok(payload) => {
                            if let Ok(mut new_id) = shellumi_core::NewIdentity::new(
                                &payload.name,
                                &payload.username,
                                payload.auth_method,
                            ) {
                                new_id.password_secret_ref = payload.password_secret_ref;
                                new_id.key_path = payload.key_path;
                                new_id.passphrase_secret_ref = payload.passphrase_secret_ref;
                                new_id.certificate_path = payload.certificate_path;
                                new_id.tags = payload.tags;
                                new_id.comment = payload.comment;

                                if let Ok(Some(existing)) =
                                    self.get_identity_by_name(&new_id.name).await
                                {
                                    let _ = self.update_identity(existing.id, &new_id).await;
                                } else {
                                    let _ = self.create_identity(&new_id).await;
                                }
                            }
                        }
                        Err(e) => summary
                            .errors
                            .push(format!("Identity {}: {e}", record.object_id)),
                    }
                }
            }
        }

        let _ = self.archive_snapshot(&vault.vault_id, vault, 3).await;
        Ok(summary)
    }

    /// Restores a synchronized vault on a second device using only the user password and sync payload.
    /// Obtains the wrapped DEK from `sync_vault.key_envelope`, unwraps it with `password` (Argon2id + AES-256-GCM),
    /// initializes a fresh encrypted database at `target_db_path`, writes the portable manifest to `manifests_dir`,
    /// provisions the key into `cred_store` if requested, and imports all synced records.
    pub async fn restore_vault_from_sync(
        sync_vault: &shellumi_core::SyncVault,
        options: &RestoreVaultOptions<'_>,
    ) -> Result<HostStore, StorageError> {
        let envelope = sync_vault.key_envelope.as_ref().ok_or_else(|| {
            StorageError::Serialization(
                "Sync payload does not contain wrapped DEK key_envelope required for second-device restoration".into(),
            )
        })?;

        let recovered_dek = shellumi_core::VaultKey::unwrap_with_password(
            options.vault_id,
            options.password,
            envelope,
        )
        .map_err(|e| StorageError::Serialization(format!("Failed to unwrap DEK: {e}")))?;

        std::fs::create_dir_all(options.manifests_dir).map_err(|e| {
            StorageError::Serialization(format!("Failed to create manifests dir: {e}"))
        })?;
        let manifest_path = options
            .manifests_dir
            .join(format!("{}.manifest.json", options.vault_id.as_str()));
        let manifest = shellumi_core::VaultManifest::new(
            options.vault_id.as_str().to_string(),
            options.vault_name.to_string(),
            envelope.clone(),
            None,
            options.remember_in_keychain,
        );
        manifest.save_to_path(&manifest_path).map_err(|e| {
            StorageError::Serialization(format!("Failed to save restored vault manifest: {e}"))
        })?;

        if options.remember_in_keychain {
            if let Some(cs) = options.cred_store {
                let _ = shellumi_core::VaultKeyManager::provision_key(
                    cs,
                    options.vault_id,
                    &recovered_dek,
                );
            }
        }

        if let Some(parent) = options.target_db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let target_str = options.target_db_path.to_str().ok_or_else(|| {
            StorageError::Serialization("Invalid target database path encoding".into())
        })?;
        let mut store = HostStore::open_with_key(target_str, Some(&recovered_dek)).await?;
        store.set_vault_id(options.vault_id.clone());

        let dek_passphrase = hex::encode(recovered_dek.expose_bytes());
        let import_res = store.import_vault(&dek_passphrase, sync_vault).await;
        if import_res.is_err() {
            store.import_vault(options.password, sync_vault).await?;
        }

        if options.vault_id.as_str() == LEGACY_LOCAL_VAULT_ID {
            if let Some(ref enc_dir) = sync_vault.encrypted_directory {
                let _ = store
                    .import_encrypted_vault_directory(
                        options.vault_id,
                        Some(&recovered_dek),
                        enc_dir,
                    )
                    .await;
            }
        }

        Ok(store)
    }

    /// Archives a snapshot of the SyncVault into the local database for disaster recovery.
    /// Prunes old snapshots to keep at most `max_keep` latest snapshots.
    pub async fn archive_snapshot(
        &self,
        snapshot_id: &str,
        vault: &shellumi_core::SyncVault,
        max_keep: usize,
    ) -> Result<i64, StorageError> {
        let vault_json = serde_json::to_string(vault).map_err(|e| {
            StorageError::InvalidSync(shellumi_core::SyncError::Serialization(e.to_string()))
        })?;
        let record_count = vault.records.len() as i64;

        let mut rows = self
            .connection
            .query(
                "INSERT INTO sync_snapshots (snapshot_id, vault_json, record_count)
                 VALUES (?, ?, ?)
                 RETURNING id",
                (snapshot_id, vault_json.as_str(), record_count),
            )
            .await?;
        let id: i64 = rows
            .next()
            .await?
            .ok_or(StorageError::InvalidName)?
            .get(0)?;
        drop(rows);

        if max_keep > 0 {
            let limit = max_keep as i64;
            self.connection
                .execute(
                    "DELETE FROM sync_snapshots WHERE id NOT IN (
                         SELECT id FROM sync_snapshots ORDER BY id DESC LIMIT ?
                     )",
                    (limit,),
                )
                .await?;
        }

        Ok(id)
    }

    /// Lists snapshot history ordered by creation time descending (newest first).
    pub async fn list_snapshot_history(&self) -> Result<Vec<SnapshotHistoryEntry>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, snapshot_id, record_count, created_at
                 FROM sync_snapshots
                 ORDER BY id DESC",
                (),
            )
            .await?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let snapshot_id: String = row.get(1)?;
            let record_count_i64: i64 = row.get(2)?;
            let created_at: String = row.get(3)?;
            entries.push(SnapshotHistoryEntry {
                id,
                snapshot_id,
                record_count: record_count_i64.max(0) as usize,
                created_at,
            });
        }
        Ok(entries)
    }

    /// Retrieves an archived SyncVault by snapshot_id (most recent match).
    pub async fn get_snapshot_vault(
        &self,
        snapshot_id: &str,
    ) -> Result<Option<shellumi_core::SyncVault>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT vault_json FROM sync_snapshots WHERE snapshot_id = ? ORDER BY id DESC LIMIT 1",
                (snapshot_id,),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let json_str: String = row.get(0)?;
            let vault: shellumi_core::SyncVault = serde_json::from_str(&json_str).map_err(|e| {
                StorageError::InvalidSync(shellumi_core::SyncError::Serialization(e.to_string()))
            })?;
            Ok(Some(vault))
        } else {
            Ok(None)
        }
    }

    /// Reverts the database using an archived snapshot, restoring records with the given passphrase.
    pub async fn rollback_snapshot(
        &mut self,
        snapshot_id: &str,
        passphrase: &str,
    ) -> Result<ImportVaultSummary, StorageError> {
        let vault = self
            .get_snapshot_vault(snapshot_id)
            .await?
            .ok_or_else(|| StorageError::SnapshotNotFound(snapshot_id.to_string()))?;
        self.import_vault(passphrase, &vault).await
    }

    /// Saves or updates a terminal session recording.
    pub async fn save_session_recording(
        &mut self,
        record: &SessionRecordingRecord,
    ) -> Result<(), StorageError> {
        let title_trimmed = record.title.trim();
        if title_trimmed.is_empty() {
            return Err(StorageError::InvalidRecording(
                "title cannot be empty".to_string(),
            ));
        }
        if record.id.trim().is_empty() {
            return Err(StorageError::InvalidRecording(
                "id cannot be empty".to_string(),
            ));
        }

        self.connection
            .execute(
                "INSERT INTO session_recordings (
                    id, session_id, host_id, title, duration_secs, event_count, byte_size, cast_data, recorded_at, created_at, vault_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    session_id = excluded.session_id,
                    host_id = excluded.host_id,
                    title = excluded.title,
                    duration_secs = excluded.duration_secs,
                    event_count = excluded.event_count,
                    byte_size = excluded.byte_size,
                    cast_data = excluded.cast_data,
                    recorded_at = excluded.recorded_at,
                    created_at = excluded.created_at,
                    vault_id = excluded.vault_id",
                (
                    record.id.as_str(),
                    record.session_id.as_str(),
                    record.host_id,
                    title_trimmed,
                    record.duration_secs,
                    record.event_count as i64,
                    record.byte_size as i64,
                    record.cast_data.as_str(),
                    record.recorded_at.as_str(),
                    record.created_at.as_str(),
                    self.host_vault_id.as_str(),
                ),
            )
            .await?;

        Ok(())
    }

    /// Retrieves a session recording by its ID.
    pub async fn get_session_recording(
        &self,
        id: &str,
    ) -> Result<Option<SessionRecordingRecord>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, session_id, host_id, title, duration_secs, event_count, byte_size, cast_data, recorded_at, created_at
                 FROM session_recordings WHERE id = ? AND vault_id = ?",
                (id, self.host_vault_id.as_str()),
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let host_id: Option<i64> = row.get(2)?;
            let title: String = row.get(3)?;
            let duration_secs: f64 = row.get(4)?;
            let event_count: i64 = row.get(5)?;
            let byte_size: i64 = row.get(6)?;
            let cast_data: String = row.get(7)?;
            let recorded_at: String = row.get(8)?;
            let created_at: String = row.get(9)?;

            Ok(Some(SessionRecordingRecord {
                id,
                session_id,
                host_id,
                title,
                duration_secs,
                event_count: event_count.max(0) as usize,
                byte_size: byte_size.max(0) as usize,
                cast_data,
                recorded_at,
                created_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// Lists summaries of recorded sessions, optionally filtered by host.
    pub async fn list_session_recordings(
        &self,
        host_id: Option<i64>,
    ) -> Result<Vec<SessionRecordingSummary>, StorageError> {
        let mut rows = if let Some(hid) = host_id {
            self.connection
                .query(
                    "SELECT id, session_id, host_id, title, duration_secs, event_count, byte_size, recorded_at
                     FROM session_recordings WHERE host_id = ? AND vault_id = ? ORDER BY recorded_at DESC, id DESC",
                    (hid, self.host_vault_id.as_str()),
                )
                .await?
        } else {
            self.connection
                .query(
                    "SELECT id, session_id, host_id, title, duration_secs, event_count, byte_size, recorded_at
                     FROM session_recordings WHERE vault_id = ? ORDER BY recorded_at DESC, id DESC",
                    (self.host_vault_id.as_str(),),
                )
                .await?
        };

        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let host_id: Option<i64> = row.get(2)?;
            let title: String = row.get(3)?;
            let duration_secs: f64 = row.get(4)?;
            let event_count: i64 = row.get(5)?;
            let byte_size: i64 = row.get(6)?;
            let recorded_at: String = row.get(7)?;

            list.push(SessionRecordingSummary {
                id,
                session_id,
                host_id,
                title,
                duration_secs,
                event_count: event_count.max(0) as usize,
                byte_size: byte_size.max(0) as usize,
                recorded_at,
            });
        }

        Ok(list)
    }

    /// Deletes a session recording by its ID. Returns true if a record was removed.
    pub async fn delete_session_recording(&mut self, id: &str) -> Result<bool, StorageError> {
        let changes = self
            .connection
            .execute(
                "DELETE FROM session_recordings WHERE id = ? AND vault_id = ?",
                (id, self.host_vault_id.as_str()),
            )
            .await?;
        Ok(changes > 0)
    }

    /// Deletes all session recordings. Returns the number of removed recordings.
    pub async fn delete_all_session_recordings(&mut self) -> Result<usize, StorageError> {
        let changes = self
            .connection
            .execute(
                "DELETE FROM session_recordings WHERE vault_id = ?",
                (self.host_vault_id.as_str(),),
            )
            .await?;
        Ok(changes as usize)
    }
}

/// Persistent record representing a saved terminal session recording.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionRecordingRecord {
    pub id: String,
    pub session_id: String,
    pub host_id: Option<i64>,
    pub title: String,
    pub duration_secs: f64,
    pub event_count: usize,
    pub byte_size: usize,
    pub cast_data: String,
    pub recorded_at: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn vault_identity_access_rejects_other_vaults_and_unscoped_secrets() {
        use shellumi_core::{
            AuthMethod, EphemeralCredentialStore, NewIdentity, VaultContext, VaultCredentialStore,
            VaultId, VaultLocation,
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("identities.db");
        let path = path.to_str().unwrap();
        let backend = EphemeralCredentialStore::new();
        let mut legacy = HostStore::open(path).await.unwrap();
        let legacy_identity =
            NewIdentity::new("Old identity", "alice", AuthMethod::Password).unwrap();
        let legacy_id = legacy.create_identity(&legacy_identity).await.unwrap();
        let id_a = VaultId::new("a").unwrap();
        let id_b = VaultId::new("b").unwrap();
        for id in [&id_a, &id_b] {
            legacy
                .register_vault_context(
                    id.as_str(),
                    &VaultContext::new(id.clone(), VaultLocation::Local),
                )
                .await
                .unwrap();
        }
        let mut a = VaultHostStore::open(path, &id_a).await.unwrap();
        let mut b = VaultHostStore::open(path, &id_b).await.unwrap();
        let credentials = VaultCredentialStore::new(&backend, &id_a);
        let reference = credentials.store("login", "company-secret").unwrap();
        let mut identity =
            NewIdentity::new("Company A identity", "alice", AuthMethod::Password).unwrap();
        identity.password_secret_ref = Some(reference);
        let identity_id = a.create_identity(&identity, &backend).await.unwrap();
        assert!(b.create_identity(&identity, &backend).await.is_err());
        assert!(b.get_identity(identity_id).await.unwrap().is_none());
        assert!(b.get_identity(legacy_id).await.unwrap().is_none());
        assert!(!b
            .update_identity(
                identity_id,
                &NewIdentity::new("other", "alice", AuthMethod::Password).unwrap(),
                &backend
            )
            .await
            .unwrap());
        assert!(!b.delete_identity(identity_id).await.unwrap());
        assert!(b
            .resolve_identity_password(identity_id, &backend)
            .await
            .is_err());
        assert_eq!(
            a.resolve_identity_password(identity_id, &backend)
                .await
                .unwrap()
                .unwrap()
                .expose_secret(),
            "company-secret"
        );
        assert!(legacy.get_identity(identity_id).await.unwrap().is_none());
        assert_eq!(legacy.list_identities().await.unwrap().len(), 1);
        drop(a);
        let a = VaultHostStore::open(path, &id_a).await.unwrap();
        assert_eq!(a.list_identities().await.unwrap()[0].id, identity_id);
    }

    #[tokio::test]
    async fn migrates_v16_identities_without_losing_secret_references() {
        use shellumi_core::{AuthMethod, NewIdentity};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v16.db");
        let path = path.to_str().unwrap();
        let mut store = HostStore::open(path).await.unwrap();
        let mut identity =
            NewIdentity::new("Legacy identity", "alice", AuthMethod::Password).unwrap();
        identity.password_secret_ref = Some("keychain://legacy/reference".into());
        let id = store.create_identity(&identity).await.unwrap();
        store
            .connection
            .execute("DROP INDEX identities_vault_id_idx", ())
            .await
            .unwrap();
        store
            .connection
            .execute("ALTER TABLE identities DROP COLUMN vault_id", ())
            .await
            .unwrap();
        store
            .connection
            .execute("PRAGMA user_version = 16", ())
            .await
            .unwrap();
        drop(store);
        let store = HostStore::open(path).await.unwrap();
        let saved = store.get_identity(id).await.unwrap().unwrap();
        assert_eq!(saved.password_secret_ref, identity.password_secret_ref);
        let mut rows = store
            .connection
            .query("SELECT vault_id FROM identities WHERE id = ?1", (id,))
            .await
            .unwrap();
        let vault: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(vault, LEGACY_LOCAL_VAULT_ID);
    }

    #[tokio::test]
    async fn vault_hosts_are_isolated_and_existing_hosts_remain_local() {
        use shellumi_core::{VaultContext, VaultId, VaultLocation};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts-isolation.db");
        let path = path.to_str().unwrap();
        let mut legacy = HostStore::open(path).await.unwrap();
        let destination = SshDestination::new("example.org", "alice", 22).unwrap();
        let legacy_id = legacy
            .add("Legacy", &destination, None, &[], None, None)
            .await
            .unwrap();
        let company = VaultId::new("company-a").unwrap();
        legacy
            .register_vault_context(
                "Company",
                &VaultContext::new(company.clone(), VaultLocation::Local),
            )
            .await
            .unwrap();
        let missing = VaultId::new("missing").unwrap();
        assert!(matches!(
            VaultHostStore::open(path, &missing).await,
            Err(StorageError::UnknownLocalVault)
        ));
        let mut scoped = VaultHostStore::open(path, &company).await.unwrap();
        assert!(scoped.list().await.unwrap().is_empty());
        assert!(scoped.get(legacy_id).await.unwrap().is_none());
        assert!(!scoped
            .update(legacy_id, "Changed", &destination)
            .await
            .unwrap());
        assert!(!scoped.delete(legacy_id).await.unwrap());
        let company_id = scoped.add("Company", &destination).await.unwrap();
        assert!(legacy.get_host(company_id).await.unwrap().is_none());
        assert!(!legacy.delete(company_id).await.unwrap());
        assert_eq!(legacy.list().await.unwrap().len(), 1);
        drop(scoped);
        drop(legacy);
        let reopened = VaultHostStore::open(path, &company).await.unwrap();
        assert_eq!(reopened.list().await.unwrap()[0].id, company_id);
        let legacy = HostStore::open(path).await.unwrap();
        assert_eq!(legacy.list().await.unwrap()[0].id, legacy_id);
    }

    #[tokio::test]
    async fn host_store_set_vault_id_switches_scope() {
        use shellumi_core::VaultId;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scope-switch.db");
        let path = path.to_str().unwrap();

        let mut store = HostStore::open(path).await.unwrap();
        assert_eq!(store.vault_id().as_str(), LEGACY_LOCAL_VAULT_ID);

        let dest_a = SshDestination::new("host-a.com", "user", 22).unwrap();
        let id_a = store
            .add("Host A", &dest_a, None, &[], None, None)
            .await
            .unwrap();
        assert_eq!(store.list().await.unwrap().len(), 1);

        let vault_b = VaultId::new("vault-b").unwrap();
        store.set_vault_id(vault_b.clone());
        assert_eq!(store.vault_id().as_str(), "vault-b");
        assert!(store.list().await.unwrap().is_empty());

        let dest_b = SshDestination::new("host-b.com", "admin", 2222).unwrap();
        let id_b = store
            .add("Host B", &dest_b, None, &[], None, None)
            .await
            .unwrap();
        assert_eq!(store.list().await.unwrap().len(), 1);
        assert_eq!(store.list().await.unwrap()[0].id, id_b);

        store.set_vault_id(VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap());
        let legacy_hosts = store.list().await.unwrap();
        assert_eq!(legacy_hosts.len(), 1);
        assert_eq!(legacy_hosts[0].id, id_a);
    }

    #[tokio::test]
    async fn vault_snippets_tunnels_workspaces_are_isolated_across_vaults() {
        use shellumi_core::VaultId;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault-isolation.db");
        let path = path.to_str().unwrap();

        let mut store = HostStore::open(path).await.unwrap();
        assert_eq!(store.vault_id().as_str(), LEGACY_LOCAL_VAULT_ID);

        // Add snippet, tunnel, workspace in default local vault
        let snip_local = Snippet {
            name: "deploy-local".into(),
            command: "echo local".into(),
            description: None,
            tags: vec!["local".into()],
            variables: vec![],
        };
        let snip_id_local = store.add_snippet(&snip_local).await.unwrap();

        let tunnel_rule_local =
            PortForwardRule::local(Some("127.0.0.1"), 8080, "localhost", 80).unwrap();
        let tunnel_id_local = store
            .add_tunnel("tunnel-local", None, &tunnel_rule_local, false)
            .await
            .unwrap();

        let ws_id_local = store
            .add_workspace("workspace-local", None, "single", &[], &[])
            .await
            .unwrap();

        assert_eq!(store.list_snippets().await.unwrap().len(), 1);
        assert_eq!(store.list_tunnels().await.unwrap().len(), 1);
        assert_eq!(store.list_workspaces().await.unwrap().len(), 1);

        // Switch to remote vault
        let remote_vid = VaultId::new("vault-remote").unwrap();
        store.set_vault_id(remote_vid.clone());

        assert!(store.list_snippets().await.unwrap().is_empty());
        assert!(store.list_tunnels().await.unwrap().is_empty());
        assert!(store.list_workspaces().await.unwrap().is_empty());

        // Add entities in remote vault
        let snip_remote = Snippet {
            name: "deploy-remote".into(),
            command: "echo remote".into(),
            description: None,
            tags: vec!["remote".into()],
            variables: vec![],
        };
        let snip_id_remote = store.add_snippet(&snip_remote).await.unwrap();

        let tunnel_rule_remote =
            PortForwardRule::remote(Some("0.0.0.0"), 9090, "remotehost", 443).unwrap();
        let tunnel_id_remote = store
            .add_tunnel("tunnel-remote", None, &tunnel_rule_remote, true)
            .await
            .unwrap();

        let ws_id_remote = store
            .add_workspace("workspace-remote", None, "split-horizontal", &[], &[])
            .await
            .unwrap();

        assert_eq!(store.list_snippets().await.unwrap().len(), 1);
        assert_eq!(store.list_snippets().await.unwrap()[0].id, snip_id_remote);
        assert_eq!(store.list_tunnels().await.unwrap().len(), 1);
        assert_eq!(store.list_tunnels().await.unwrap()[0].id, tunnel_id_remote);
        assert_eq!(store.list_workspaces().await.unwrap().len(), 1);
        assert_eq!(store.list_workspaces().await.unwrap()[0].id, ws_id_remote);

        // Switch back to legacy-local
        store.set_vault_id(VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap());
        let local_snippets = store.list_snippets().await.unwrap();
        assert_eq!(local_snippets.len(), 1);
        assert_eq!(local_snippets[0].id, snip_id_local);
        assert_eq!(local_snippets[0].name, "deploy-local");

        let local_tunnels = store.list_tunnels().await.unwrap();
        assert_eq!(local_tunnels.len(), 1);
        assert_eq!(local_tunnels[0].id, tunnel_id_local);
        assert_eq!(local_tunnels[0].name, "tunnel-local");

        let local_workspaces = store.list_workspaces().await.unwrap();
        assert_eq!(local_workspaces.len(), 1);
        assert_eq!(local_workspaces[0].id, ws_id_local);
        assert_eq!(local_workspaces[0].name, "workspace-local");

        // Delete from local vault and verify remote vault is unchanged
        assert!(store.delete_snippet(snip_id_local).await.unwrap());
        assert!(store.delete_tunnel(tunnel_id_local).await.unwrap());
        assert!(store.delete_workspace(ws_id_local).await.unwrap());

        assert!(store.list_snippets().await.unwrap().is_empty());
        assert!(store.list_tunnels().await.unwrap().is_empty());
        assert!(store.list_workspaces().await.unwrap().is_empty());

        store.set_vault_id(remote_vid);
        assert_eq!(store.list_snippets().await.unwrap().len(), 1);
        assert_eq!(store.list_tunnels().await.unwrap().len(), 1);
        assert_eq!(store.list_workspaces().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn migrates_v15_host_rows_to_default_local_vault() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v15.db");
        let path = path.to_str().unwrap();
        let mut store = HostStore::open(path).await.unwrap();
        let destination = SshDestination::new("legacy.example", "user", 22).unwrap();
        store
            .add("Legacy", &destination, None, &[], None, None)
            .await
            .unwrap();
        store
            .connection
            .execute("DROP INDEX hosts_vault_id_idx", ())
            .await
            .unwrap();
        store
            .connection
            .execute("ALTER TABLE hosts DROP COLUMN vault_id", ())
            .await
            .unwrap();
        store
            .connection
            .execute("PRAGMA user_version = 15", ())
            .await
            .unwrap();
        drop(store);
        let reopened = HostStore::open(path).await.unwrap();
        assert_eq!(reopened.list().await.unwrap()[0].destination, destination);
        let mut rows = reopened
            .connection
            .query("SELECT vault_id FROM hosts", ())
            .await
            .unwrap();
        let vault_id: String = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(vault_id, LEGACY_LOCAL_VAULT_ID);
    }

    #[tokio::test]
    async fn vault_registry_migrates_legacy_data_and_persists_server_scopes() {
        use shellumi_core::{ServerConnectionId, VaultContext, VaultId, VaultLocation};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault-registry.db");
        let path = path.to_str().unwrap();
        let mut store = HostStore::open(path).await.unwrap();
        let destination = SshDestination::new("example.org", "alice", 22).unwrap();
        store
            .add("Existing", &destination, None, &[], None, None)
            .await
            .unwrap();
        store
            .connection
            .execute("DROP TABLE vault_contexts", ())
            .await
            .unwrap();
        store
            .connection
            .execute("DROP TABLE vault_server_connections", ())
            .await
            .unwrap();
        store
            .connection
            .execute("PRAGMA user_version = 14", ())
            .await
            .unwrap();
        drop(store);

        let mut store = HostStore::open(path).await.unwrap();
        assert_eq!(store.list().await.unwrap()[0].destination, destination);
        let initial = store.list_vault_contexts().await.unwrap();
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].1.id().as_str(), LEGACY_LOCAL_VAULT_ID);
        assert_eq!(initial[0].1.location(), &VaultLocation::Local);

        let a = ServerConnectionId::new("server-a").unwrap();
        let b = ServerConnectionId::new("server-b").unwrap();
        store.register_vault_server(&a).await.unwrap();
        store.register_vault_server(&b).await.unwrap();
        let make = |id: &str, server: ServerConnectionId| {
            VaultContext::new(
                VaultId::new(id).unwrap(),
                VaultLocation::Remote {
                    server,
                    server_vault_id: VaultId::new("same-remote-id").unwrap(),
                },
            )
        };
        let first = make("company-a", a.clone());
        let second = make("company-b", b);
        store
            .register_vault_context("Company A", &first)
            .await
            .unwrap();
        store
            .register_vault_context("Company B", &second)
            .await
            .unwrap();
        assert!(store
            .register_vault_context("Duplicate", &make("other", a))
            .await
            .is_err());
        let missing = make("missing", ServerConnectionId::new("unknown").unwrap());
        assert!(store
            .register_vault_context("Missing", &missing)
            .await
            .is_err());
        drop(store);
        let store = HostStore::open(path).await.unwrap();
        let contexts = store.list_vault_contexts().await.unwrap();
        assert_eq!(contexts.len(), 3);
        assert!(contexts.iter().any(|(_, context)| context == &first));
        assert!(contexts.iter().any(|(_, context)| context == &second));
        assert_eq!(store.list().await.unwrap()[0].destination, destination);
    }

    #[tokio::test]
    async fn persists_across_reopen_and_rejects_future_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.db");
        let path = path.to_str().unwrap();
        let mut store = HostStore::open(path).await.unwrap();
        let destination = SshDestination::new("example.org", "alice", 22).unwrap();
        store
            .add("Server", &destination, None, &[], None, None)
            .await
            .unwrap();
        drop(store);
        let store = HostStore::open(path).await.unwrap();
        assert_eq!(store.list().await.unwrap()[0].destination, destination);
        store
            .connection
            .execute("PRAGMA user_version = 999", ())
            .await
            .unwrap();
        drop(store);
        assert!(matches!(
            HostStore::open(path).await,
            Err(StorageError::UnsupportedSchema(999))
        ));
    }

    #[tokio::test]
    async fn migrates_from_schema_versions_1_to_8_to_version_9() {
        let dir = tempfile::tempdir().unwrap();
        let path1 = dir.path().join("v1.db");
        let path1_str = path1.to_str().unwrap();

        // Create a v1 database manually
        let db = Builder::new_local(path1_str).build().await.unwrap();
        let conn = db.connect().unwrap();
        conn.execute(
            "CREATE TABLE hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL
            )",
            (),
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO hosts (name, hostname, username, port) VALUES ('OldHost', 'legacy.net', 'user', 22)",
            (),
        )
        .await
        .unwrap();
        conn.execute("PRAGMA user_version = 1", ()).await.unwrap();
        drop(conn);
        drop(db);

        // Open with HostStore and verify migration from v1 to v8
        let mut store = HostStore::open(path1_str).await.unwrap();
        let hosts = store.list().await.unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "OldHost");
        assert_eq!(hosts[0].group_name, None);
        assert_eq!(hosts[0].tags, Vec::<String>::new());
        assert_eq!(hosts[0].jump_host, None);
        assert_eq!(hosts[0].startup_snippet_id, None);

        // Verify group operations, tunnel operations, transfer logging, and snippets work on migrated store
        let group = HostGroup::new("Infra", Some("ops"), Some(2200), &["cloud".into()]).unwrap();
        store.add_group(&group).await.unwrap();
        assert_eq!(store.list_groups().await.unwrap().len(), 1);

        let rule = PortForwardRule::local(None, 8080, "db", 5432).unwrap();
        store
            .add_tunnel("DBTunnel", Some(hosts[0].id), &rule, true)
            .await
            .unwrap();
        assert_eq!(store.list_tunnels().await.unwrap().len(), 1);

        let tr_id = store
            .log_transfer(&NewTransfer {
                host_id: Some(hosts[0].id),
                direction: TransferDirection::Upload,
                local_path: "/local/app.bin",
                remote_path: "/remote/app.bin",
                bytes_transferred: 0,
                total_bytes: 1024,
                status: "queued",
                error_message: None,
            })
            .await
            .unwrap();
        assert_eq!(store.list_transfers(None, None).await.unwrap().len(), 1);
        assert!(tr_id > 0);

        let snip = Snippet::new("Restart App", "systemctl restart app", None, &[]).unwrap();
        let s_id = store.add_snippet(&snip).await.unwrap();
        assert_eq!(store.list_snippets().await.unwrap().len(), 1);
        assert!(s_id > 0);
        assert!(store.list_snapshot_history().await.unwrap().is_empty());

        // Test migration from v2 to v7
        let path2 = dir.path().join("v2.db");
        let path2_str = path2.to_str().unwrap();
        let db2 = Builder::new_local(path2_str).build().await.unwrap();
        let conn2 = db2.connect().unwrap();
        conn2
            .execute(
                "CREATE TABLE hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL,
                group_name TEXT,
                tags TEXT NOT NULL DEFAULT ''
            )",
                (),
            )
            .await
            .unwrap();
        conn2.execute("PRAGMA user_version = 2", ()).await.unwrap();
        drop(conn2);
        drop(db2);

        let mut store2 = HostStore::open(path2_str).await.unwrap();
        store2.add_group(&group).await.unwrap();
        assert_eq!(store2.list_groups().await.unwrap().len(), 1);
        assert!(store2.list_snapshot_history().await.unwrap().is_empty());

        // Test migration from v3 to v7
        let path3 = dir.path().join("v3.db");
        let path3_str = path3.to_str().unwrap();
        let db3 = Builder::new_local(path3_str).build().await.unwrap();
        let conn3 = db3.connect().unwrap();
        conn3
            .execute(
                "CREATE TABLE hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL,
                group_name TEXT,
                tags TEXT NOT NULL DEFAULT ''
            )",
                (),
            )
            .await
            .unwrap();
        conn3
            .execute(
                "CREATE TABLE groups (
                name TEXT PRIMARY KEY NOT NULL,
                default_username TEXT,
                default_port INTEGER,
                tags TEXT NOT NULL DEFAULT ''
            )",
                (),
            )
            .await
            .unwrap();
        conn3.execute("PRAGMA user_version = 3", ()).await.unwrap();
        drop(conn3);
        drop(db3);

        let mut store3 = HostStore::open(path3_str).await.unwrap();
        store3
            .add_tunnel(
                "SocksProxy",
                None,
                &PortForwardRule::dynamic(None, 1080).unwrap(),
                false,
            )
            .await
            .unwrap();
        assert_eq!(store3.list_tunnels().await.unwrap().len(), 1);
        assert!(store3.list_snapshot_history().await.unwrap().is_empty());

        // Test migration from v4 to v7
        let path4 = dir.path().join("v4.db");
        let path4_str = path4.to_str().unwrap();
        let db4 = Builder::new_local(path4_str).build().await.unwrap();
        let conn4 = db4.connect().unwrap();
        conn4
            .execute(
                "CREATE TABLE hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL,
                group_name TEXT,
                tags TEXT NOT NULL DEFAULT '',
                jump_host TEXT
            )",
                (),
            )
            .await
            .unwrap();
        conn4.execute("PRAGMA user_version = 4", ()).await.unwrap();
        drop(conn4);
        drop(db4);

        let mut store4 = HostStore::open(path4_str).await.unwrap();
        store4
            .log_transfer(&NewTransfer {
                host_id: None,
                direction: TransferDirection::Download,
                local_path: "/local/log.txt",
                remote_path: "/var/log/syslog",
                bytes_transferred: 500,
                total_bytes: 500,
                status: "completed",
                error_message: None,
            })
            .await
            .unwrap();
        assert_eq!(store4.list_transfers(None, None).await.unwrap().len(), 1);
        assert!(store4.list_snapshot_history().await.unwrap().is_empty());

        // Test migration from v5 to v8
        let path5 = dir.path().join("v5.db");
        let path5_str = path5.to_str().unwrap();
        let db5 = Builder::new_local(path5_str).build().await.unwrap();
        let conn5 = db5.connect().unwrap();
        conn5
            .execute(
                "CREATE TABLE hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL,
                group_name TEXT,
                tags TEXT NOT NULL DEFAULT '',
                jump_host TEXT
            )",
                (),
            )
            .await
            .unwrap();
        conn5
            .execute(
                "CREATE TABLE transfer_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                host_id INTEGER,
                direction TEXT NOT NULL,
                local_path TEXT NOT NULL,
                remote_path TEXT NOT NULL,
                bytes_transferred INTEGER NOT NULL,
                total_bytes INTEGER NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                completed_at TEXT
            )",
                (),
            )
            .await
            .unwrap();
        conn5.execute("PRAGMA user_version = 5", ()).await.unwrap();
        drop(conn5);
        drop(db5);

        let mut store5 = HostStore::open(path5_str).await.unwrap();
        let snip5 = Snippet::new("Check Disk", "df -h", None, &[]).unwrap();
        assert!(store5.add_snippet(&snip5).await.unwrap() > 0);
        assert_eq!(store5.list_snippets().await.unwrap().len(), 1);
        assert!(store5.list_snapshot_history().await.unwrap().is_empty());

        // Test migration from v6 to v7
        let path6 = dir.path().join("v6.db");
        let path6_str = path6.to_str().unwrap();
        let db6 = Builder::new_local(path6_str).build().await.unwrap();
        let conn6 = db6.connect().unwrap();
        conn6
            .execute(
                "CREATE TABLE hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL,
                group_name TEXT,
                tags TEXT NOT NULL DEFAULT '',
                jump_host TEXT
            )",
                (),
            )
            .await
            .unwrap();
        conn6
            .execute(
                "CREATE TABLE groups (
                name TEXT PRIMARY KEY NOT NULL,
                default_username TEXT,
                default_port INTEGER,
                tags TEXT NOT NULL DEFAULT ''
            )",
                (),
            )
            .await
            .unwrap();
        conn6
            .execute(
                "CREATE TABLE tunnels (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                host_id INTEGER,
                name TEXT NOT NULL,
                forwarding_type TEXT NOT NULL,
                bind_address TEXT,
                bind_port INTEGER NOT NULL,
                target_host TEXT,
                target_port INTEGER,
                auto_start INTEGER NOT NULL DEFAULT 0
            )",
                (),
            )
            .await
            .unwrap();
        conn6
            .execute(
                "CREATE TABLE transfer_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                host_id INTEGER,
                direction TEXT NOT NULL,
                local_path TEXT NOT NULL,
                remote_path TEXT NOT NULL,
                bytes_transferred INTEGER NOT NULL,
                total_bytes INTEGER NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                completed_at TEXT
            )",
                (),
            )
            .await
            .unwrap();
        conn6
            .execute(
                "CREATE TABLE snippets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                description TEXT,
                tags TEXT NOT NULL DEFAULT '',
                variables TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
                (),
            )
            .await
            .unwrap();
        conn6.execute("PRAGMA user_version = 6", ()).await.unwrap();
        drop(conn6);
        drop(db6);

        let store6 = HostStore::open(path6_str).await.unwrap();
        assert!(store6.list_snapshot_history().await.unwrap().is_empty());

        // Test migration from v7 to v8
        let path7 = dir.path().join("v7.db");
        let path7_str = path7.to_str().unwrap();
        let db7 = Builder::new_local(path7_str).build().await.unwrap();
        let conn7 = db7.connect().unwrap();
        conn7
            .execute(
                "CREATE TABLE hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL,
                group_name TEXT,
                tags TEXT NOT NULL DEFAULT '',
                jump_host TEXT
            )",
                (),
            )
            .await
            .unwrap();
        conn7
            .execute(
                "INSERT INTO hosts (name, hostname, username, port) VALUES ('V7Host', 'v7.example.com', 'deploy', 22)",
                (),
            )
            .await
            .unwrap();
        conn7
            .execute(
                "CREATE TABLE snippets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                description TEXT,
                tags TEXT NOT NULL DEFAULT '',
                variables TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
                (),
            )
            .await
            .unwrap();
        conn7
            .execute(
                "CREATE TABLE sync_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snapshot_id TEXT NOT NULL,
                vault_json TEXT NOT NULL,
                record_count INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
                (),
            )
            .await
            .unwrap();
        conn7.execute("PRAGMA user_version = 7", ()).await.unwrap();
        drop(conn7);
        drop(db7);

        let mut store7 = HostStore::open(path7_str).await.unwrap();
        let hosts7 = store7.list().await.unwrap();
        assert_eq!(hosts7.len(), 1);
        assert_eq!(hosts7[0].name, "V7Host");
        assert_eq!(hosts7[0].startup_snippet_id, None);

        // Add a snippet and assign it as startup snippet to V7Host
        let snip = Snippet::new("Login MOTD", "uname -a", None, &[]).unwrap();
        let snip_id = store7.add_snippet(&snip).await.unwrap();
        let dest = SshDestination::new("v7.example.com", "deploy", 22).unwrap();
        assert!(store7
            .update(
                hosts7[0].id,
                "V7Host",
                &dest,
                None,
                &[],
                None,
                Some(snip_id)
            )
            .await
            .unwrap());
        let fetched = store7.get_host(hosts7[0].id).await.unwrap().unwrap();
        assert_eq!(fetched.startup_snippet_id, Some(snip_id));

        // Test migration from v8 to v9
        let path8 = dir.path().join("v8.db");
        let path8_str = path8.to_str().unwrap();
        let db8 = Builder::new_local(path8_str).build().await.unwrap();
        let conn8 = db8.connect().unwrap();
        conn8
            .execute(
                "CREATE TABLE hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL,
                group_name TEXT,
                tags TEXT NOT NULL DEFAULT '',
                jump_host TEXT,
                startup_snippet_id INTEGER
            )",
                (),
            )
            .await
            .unwrap();
        conn8
            .execute(
                "INSERT INTO hosts (name, hostname, username, port) VALUES ('V8Host', 'v8.example.com', 'admin', 22)",
                (),
            )
            .await
            .unwrap();
        conn8
            .execute(
                "CREATE TABLE snippets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                description TEXT,
                tags TEXT NOT NULL DEFAULT '',
                variables TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
                (),
            )
            .await
            .unwrap();
        conn8
            .execute(
                "CREATE TABLE sync_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snapshot_id TEXT NOT NULL,
                vault_json TEXT NOT NULL,
                record_count INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
                (),
            )
            .await
            .unwrap();
        conn8.execute("PRAGMA user_version = 8", ()).await.unwrap();
        drop(conn8);
        drop(db8);

        let mut store8 = HostStore::open(path8_str).await.unwrap();
        assert_eq!(store8.list().await.unwrap().len(), 1);
        assert_eq!(store8.list_workspaces().await.unwrap().len(), 0);

        // Can add and retrieve workspaces on migrated database
        let sess = shellumi_core::WorkspaceSession::new(
            "Terminal 1",
            shellumi_core::WorkspaceSessionKind::Local,
            0,
        );
        let ws_id = store8
            .add_workspace(
                "Ops Center",
                Some("Monitoring"),
                "vertical",
                &[sess],
                &["ops".into()],
            )
            .await
            .unwrap();
        assert!(ws_id > 0);
        let ws = store8.get_workspace(ws_id).await.unwrap().unwrap();
        assert_eq!(ws.name, "Ops Center");
        assert_eq!(ws.layout, "vertical");
        assert_eq!(ws.sessions.len(), 1);

        // --- Schema version 9 -> 10 migration test ---
        let dir9 = tempfile::tempdir().unwrap();
        let path9 = dir9.path().join("v9.db");
        let path9_str = path9.to_str().unwrap();
        let db9 = Builder::new_local(path9_str).build().await.unwrap();
        let conn9 = db9.connect().unwrap();
        conn9
            .execute(
                "CREATE TABLE hosts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                hostname TEXT NOT NULL,
                username TEXT NOT NULL,
                port INTEGER NOT NULL,
                group_name TEXT,
                tags TEXT NOT NULL DEFAULT '',
                jump_host TEXT,
                startup_snippet_id INTEGER
            )",
                (),
            )
            .await
            .unwrap();
        conn9
            .execute(
                "INSERT INTO hosts (name, hostname, username, port, tags) VALUES ('LegacyV9', 'v9.example.com', 'admin', 22, 'legacy')",
                (),
            )
            .await
            .unwrap();
        conn9
            .execute(
                "CREATE TABLE workspaces (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT,
                layout TEXT NOT NULL DEFAULT 'single',
                sessions_json TEXT NOT NULL DEFAULT '[]',
                tags TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
                (),
            )
            .await
            .unwrap();
        conn9.execute("PRAGMA user_version = 9", ()).await.unwrap();
        drop(conn9);
        drop(db9);

        let mut store9 = HostStore::open(path9_str).await.unwrap();
        let hosts9 = store9.list().await.unwrap();
        assert_eq!(hosts9.len(), 1);
        assert_eq!(hosts9[0].name, "LegacyV9");
        assert_eq!(hosts9[0].protocol, "ssh");
        assert_eq!(hosts9[0].protocol_config, None);
        assert!(hosts9[0].is_ssh());

        // Can add host with non-ssh protocol on migrated database
        let mosh_dest = SshDestination::new("mosh.example.org", "dev", 22).unwrap();
        let mosh_id = store9
            .add_with_protocol(
                "MoshHost",
                &mosh_dest,
                None,
                &["mosh".into()],
                None,
                None,
                "mosh",
                Some(r#"{"predict_mode":"adaptive"}"#),
            )
            .await
            .unwrap();
        let fetched_mosh = store9.get_host(mosh_id).await.unwrap().unwrap();
        assert_eq!(fetched_mosh.protocol, "mosh");
        assert!(fetched_mosh.is_mosh());
        assert_eq!(
            fetched_mosh.protocol_config.as_deref(),
            Some(r#"{"predict_mode":"adaptive"}"#)
        );
    }

    #[tokio::test]
    async fn manages_workspace_lifecycle() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        assert!(store.list_workspaces().await.unwrap().is_empty());

        let s1 = shellumi_core::WorkspaceSession::new(
            "Bastion",
            shellumi_core::WorkspaceSessionKind::Ssh,
            0,
        )
        .with_host(1)
        .with_startup_command("uptime");

        let s2 = shellumi_core::WorkspaceSession::new(
            "Local Shell",
            shellumi_core::WorkspaceSessionKind::Local,
            1,
        )
        .with_working_directory("/tmp")
        .with_broadcast_excluded(true);

        let ws_id = store
            .add_workspace(
                "Incident Response",
                Some("Production triage"),
                "horizontal",
                &[s1, s2],
                &["prod".into(), "incident".into()],
            )
            .await
            .unwrap();
        assert!(ws_id > 0);

        let ws = store.get_workspace(ws_id).await.unwrap().unwrap();
        assert_eq!(ws.name, "Incident Response");
        assert_eq!(ws.description.as_deref(), Some("Production triage"));
        assert_eq!(ws.layout, "horizontal");
        assert_eq!(ws.sessions.len(), 2);
        assert_eq!(ws.sessions[0].name, "Bastion");
        assert_eq!(ws.sessions[0].startup_command.as_deref(), Some("uptime"));
        assert!(ws.sessions[1].excluded_from_broadcast);
        assert_eq!(ws.tags, vec!["prod", "incident"]);

        // Update workspace
        let s_updated = shellumi_core::WorkspaceSession::new(
            "Bastion Primed",
            shellumi_core::WorkspaceSessionKind::Ssh,
            0,
        );
        assert!(store
            .update_workspace(
                ws_id,
                "Incident Response Active",
                Some("Updated desc"),
                "grid",
                &[s_updated],
                &["active".into()],
            )
            .await
            .unwrap());

        let updated_ws = store.get_workspace(ws_id).await.unwrap().unwrap();
        assert_eq!(updated_ws.name, "Incident Response Active");
        assert_eq!(updated_ws.layout, "grid");
        assert_eq!(updated_ws.sessions.len(), 1);
        assert_eq!(updated_ws.tags, vec!["active"]);

        // Duplicate workspace
        let dup_id = store
            .duplicate_workspace(ws_id, "Incident Response (Clone)")
            .await
            .unwrap();
        assert_ne!(dup_id, ws_id);
        let dup = store.get_workspace(dup_id).await.unwrap().unwrap();
        assert_eq!(dup.name, "Incident Response (Clone)");
        assert_eq!(dup.layout, "grid");

        // Delete workspace
        assert!(store.delete_workspace(ws_id).await.unwrap());
        assert_eq!(store.get_workspace(ws_id).await.unwrap(), None);
        assert_eq!(store.list_workspaces().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn manages_sync_snapshot_history_and_rollback() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let destination = SshDestination::new("db.internal", "postgres", 5432).unwrap();
        let h_id = store
            .add(
                "ProductionDB",
                &destination,
                None,
                &["db".into()],
                None,
                None,
            )
            .await
            .unwrap();
        assert!(h_id > 0);

        let group = HostGroup::new("Databases", Some("postgres"), Some(5432), &[]).unwrap();
        store.add_group(&group).await.unwrap();

        let snip = Snippet::new("Vacuum", "VACUUM FULL;", None, &[]).unwrap();
        store.add_snippet(&snip).await.unwrap();

        // Export vault creates and archives snapshot
        let vault = store
            .export_vault_with_iterations("secret123", "device1", 2000)
            .await
            .unwrap();
        let history = store.list_snapshot_history().await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].snapshot_id, vault.vault_id);
        assert_eq!(history[0].record_count, 3);

        // Delete the host
        assert!(store.delete(h_id).await.unwrap());
        assert_eq!(store.list().await.unwrap().len(), 0);

        // Rollback to the snapshot
        let summary = store
            .rollback_snapshot(&vault.vault_id, "secret123")
            .await
            .unwrap();
        assert_eq!(summary.hosts_imported, 1);
        assert_eq!(summary.groups_imported, 1);
        assert_eq!(summary.snippets_imported, 1);

        // Check host is restored
        let restored_hosts = store.list().await.unwrap();
        assert_eq!(restored_hosts.len(), 1);
        assert_eq!(restored_hosts[0].name, "ProductionDB");

        // Verify pruning with max_keep
        for i in 1..=5 {
            let fake_vault = shellumi_core::SyncVault::new(format!("snap-{i}"), 1000 + i);
            store
                .archive_snapshot(&fake_vault.vault_id, &fake_vault, 3)
                .await
                .unwrap();
        }
        let history_pruned = store.list_snapshot_history().await.unwrap();
        assert_eq!(history_pruned.len(), 3);
        assert_eq!(history_pruned[0].snapshot_id, "snap-5");
        assert_eq!(history_pruned[1].snapshot_id, "snap-4");
        assert_eq!(history_pruned[2].snapshot_id, "snap-3");

        // Rollback unknown snapshot fails with SnapshotNotFound
        let err = store.rollback_snapshot("non-existent-id", "pass").await;
        assert!(matches!(err, Err(StorageError::SnapshotNotFound(_))));
    }

    #[tokio::test]
    async fn manages_snippet_lifecycle() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        // Add snippets
        let s1 = Snippet::new(
            "Flush Redis",
            "redis-cli flushall",
            Some("Flushes all redis keys".into()),
            &["cache".into(), "redis".into()],
        )
        .unwrap();

        let s2 = Snippet::new(
            "Tail Nginx",
            "tail -f /var/log/nginx/{{log_type:-access}}.log",
            Some("Follows nginx log".into()),
            &["web".into(), "nginx".into()],
        )
        .unwrap();

        let id1 = store.add_snippet(&s1).await.unwrap();
        let id2 = store.add_snippet(&s2).await.unwrap();

        // List
        let all = store.list_snippets().await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "Flush Redis"); // sorted by name
        assert_eq!(all[1].name, "Tail Nginx");
        assert_eq!(all[1].variables.len(), 1);
        assert_eq!(all[1].variables[0].name, "log_type");
        assert_eq!(all[1].variables[0].default_value.as_deref(), Some("access"));

        // Get single snippet
        let fetched = store.get_snippet(id2).await.unwrap().unwrap();
        assert_eq!(fetched.id, id2);
        assert_eq!(fetched.name, "Tail Nginx");
        assert_eq!(store.get_snippet(9999).await.unwrap(), None);

        // Update snippet
        let updated_s1 = Snippet::new(
            "Flush Redis DB",
            "redis-cli -n {{db:-0}} flushdb",
            Some("Flushes specific redis db".into()),
            &["cache".into()],
        )
        .unwrap();
        assert!(store.update_snippet(id1, &updated_s1).await.unwrap());
        let fetched_updated = store.get_snippet(id1).await.unwrap().unwrap();
        assert_eq!(fetched_updated.name, "Flush Redis DB");
        assert_eq!(fetched_updated.variables.len(), 1);
        assert_eq!(fetched_updated.variables[0].name, "db");

        // Filter by tag
        let cache_snippets = store.list_snippets_by_tag("cache").await.unwrap();
        assert_eq!(cache_snippets.len(), 1);
        assert_eq!(cache_snippets[0].id, id1);

        let web_snippets = store.list_snippets_by_tag("web").await.unwrap();
        assert_eq!(web_snippets.len(), 1);
        assert_eq!(web_snippets[0].id, id2);

        // Search
        assert_eq!(store.search_snippets("flush").await.unwrap().len(), 1);
        assert_eq!(store.search_snippets("nginx").await.unwrap().len(), 1);
        assert_eq!(store.search_snippets("nonexistent").await.unwrap().len(), 0);

        // Startup snippet linkage and foreign key nullification
        let dest_host = SshDestination::new("10.0.0.99", "admin", 22).unwrap();
        let host_with_snippet = store
            .add("LinkedHost", &dest_host, None, &[], None, Some(id2))
            .await
            .unwrap();
        let fetched_host = store.get_host(host_with_snippet).await.unwrap().unwrap();
        assert_eq!(fetched_host.startup_snippet_id, Some(id2));

        // When snippet is deleted, startup_snippet_id is set to None
        assert!(store.delete_snippet(id2).await.unwrap());
        let fetched_after_delete = store.get_host(host_with_snippet).await.unwrap().unwrap();
        assert_eq!(fetched_after_delete.startup_snippet_id, None);

        // Delete snippet
        assert!(store.delete_snippet(id1).await.unwrap());
        assert!(!store.delete_snippet(id1).await.unwrap());
        assert_eq!(store.list_snippets().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn manages_transfer_history_lifecycle() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let dest = SshDestination::new("10.0.0.1", "root", 22).unwrap();
        let host_id = store
            .add("TargetHost", &dest, None, &[], None, None)
            .await
            .unwrap();

        // Rejects empty paths
        assert!(matches!(
            store
                .log_transfer(&NewTransfer {
                    host_id: Some(host_id),
                    direction: TransferDirection::Upload,
                    local_path: "",
                    remote_path: "/remote/file",
                    bytes_transferred: 0,
                    total_bytes: 100,
                    status: "queued",
                    error_message: None,
                })
                .await,
            Err(StorageError::InvalidTransferPath)
        ));
        assert!(matches!(
            store
                .log_transfer(&NewTransfer {
                    host_id: Some(host_id),
                    direction: TransferDirection::Upload,
                    local_path: "/local/file",
                    remote_path: " ",
                    bytes_transferred: 0,
                    total_bytes: 100,
                    status: "queued",
                    error_message: None,
                })
                .await,
            Err(StorageError::InvalidTransferPath)
        ));

        // Add 2 transfers
        let t1 = store
            .log_transfer(&NewTransfer {
                host_id: Some(host_id),
                direction: TransferDirection::Upload,
                local_path: "/home/user/document.pdf",
                remote_path: "/srv/docs/document.pdf",
                bytes_transferred: 0,
                total_bytes: 10240,
                status: "queued",
                error_message: None,
            })
            .await
            .unwrap();

        let t2 = store
            .log_transfer(&NewTransfer {
                host_id: None,
                direction: TransferDirection::Download,
                local_path: "/tmp/report.csv",
                remote_path: "/var/reports/daily.csv",
                bytes_transferred: 2048,
                total_bytes: 2048,
                status: "completed",
                error_message: None,
            })
            .await
            .unwrap();

        // Listing transfers
        let all = store.list_transfers(None, None).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, t2); // ordered by id DESC
        assert_eq!(all[0].direction, TransferDirection::Download);
        assert_eq!(all[0].bytes_transferred, 2048);
        assert_eq!(all[1].id, t1);
        assert_eq!(all[1].direction, TransferDirection::Upload);
        assert_eq!(all[1].total_bytes, 10240);

        // Filter by host
        let host_transfers = store.list_transfers(Some(host_id), None).await.unwrap();
        assert_eq!(host_transfers.len(), 1);
        assert_eq!(host_transfers[0].id, t1);

        // Limit
        let limited = store.list_transfers(None, Some(1)).await.unwrap();
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].id, t2);

        // Update transfer status
        assert!(store
            .update_transfer(t1, 5120, "in_progress", None, false)
            .await
            .unwrap());
        let updated = store.list_transfers(Some(host_id), None).await.unwrap();
        assert_eq!(updated[0].bytes_transferred, 5120);
        assert_eq!(updated[0].status, "in_progress");
        assert_eq!(updated[0].completed_at, None);

        // Mark completed
        assert!(store
            .update_transfer(t1, 10240, "completed", None, true)
            .await
            .unwrap());
        let completed = store.list_transfers(Some(host_id), None).await.unwrap();
        assert_eq!(completed[0].bytes_transferred, 10240);
        assert_eq!(completed[0].status, "completed");
        assert!(completed[0].completed_at.is_some());

        // Clear transfer history for host
        let cleared = store.clear_transfer_history(Some(host_id)).await.unwrap();
        assert_eq!(cleared, 1);
        assert_eq!(
            store
                .list_transfers(Some(host_id), None)
                .await
                .unwrap()
                .len(),
            0
        );
        assert_eq!(store.list_transfers(None, None).await.unwrap().len(), 1);

        // Clear all transfer history
        let cleared_all = store.clear_transfer_history(None).await.unwrap();
        assert_eq!(cleared_all, 1);
        assert_eq!(store.list_transfers(None, None).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn manages_host_groups_and_inheritance() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        // Create groups
        let g1 = HostGroup::new(
            "Production",
            Some("deploy"),
            Some(2222),
            &["prod".into(), "secure".into()],
        )
        .unwrap();
        store.add_group(&g1).await.unwrap();

        let g2 = HostGroup::new("Staging", Some("tester"), None, &["stage".into()]).unwrap();
        store.add_group(&g2).await.unwrap();

        let groups = store.list_groups().await.unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Production");
        assert_eq!(groups[1].name, "Staging");

        // Retrieve specific group
        let fetched = store.get_group("Production").await.unwrap().unwrap();
        assert_eq!(fetched, g1);
        assert_eq!(store.get_group("Missing").await.unwrap(), None);

        // Update group
        let g1_updated = HostGroup::new(
            "Production",
            Some("admin"),
            Some(2200),
            &["prod".into(), "updated".into()],
        )
        .unwrap();
        assert!(store.update_group(&g1_updated).await.unwrap());
        let fetched_updated = store.get_group("Production").await.unwrap().unwrap();
        assert_eq!(fetched_updated.default_username.as_deref(), Some("admin"));
        assert_eq!(fetched_updated.default_port.unwrap().get(), 2200);

        // Host inheritance tests
        let dest = SshDestination::new("10.0.0.5", "node1", 22).unwrap();
        let host_id = store
            .add(
                "Node 1",
                &dest,
                Some("Production"),
                &["web".into(), "prod".into()],
                None,
                None,
            )
            .await
            .unwrap();
        let host = &store.list().await.unwrap()[0];
        assert_eq!(host.id, host_id);

        let eff_dest = store.get_effective_destination(host).await.unwrap();
        assert_eq!(eff_dest.host(), "10.0.0.5");
        // Host overrides username ("node1") but inherits group settings if applicable
        assert_eq!(eff_dest.username(), "node1");

        let eff_tags = store.get_effective_tags(host).await.unwrap();
        // Merged and deduplicated tags: ["prod", "updated", "web"]
        assert_eq!(eff_tags, vec!["prod", "updated", "web"]);

        // Delete group
        assert!(store.delete_group("Production").await.unwrap());
        assert!(!store.delete_group("Production").await.unwrap());
        assert_eq!(store.get_group("Production").await.unwrap(), None);

        // Host still exists with group_name set, but effective destination falls back to host's own
        let host_after = &store.list().await.unwrap()[0];
        let eff_dest_after = store.get_effective_destination(host_after).await.unwrap();
        assert_eq!(eff_dest_after, dest);
    }

    #[tokio::test]
    async fn saves_lists_and_deletes_hosts_with_groups_and_tags() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let destination = SshDestination::new("example.org", "alice", 22).unwrap();
        let tags = vec!["prod".to_string(), "eu-west".to_string()];
        let id = store
            .add(
                " Server ",
                &destination,
                Some(" Production "),
                &tags,
                Some("jump.example.org"),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            store.list().await.unwrap(),
            vec![SavedHost {
                id,
                name: "Server".into(),
                destination: destination.clone(),
                group_name: Some("Production".into()),
                tags: tags.clone(),
                jump_host: Some("jump.example.org".into()),
                startup_snippet_id: None,
                protocol: "ssh".into(),
                protocol_config: None,
                identity_id: None,
            }]
        );

        // Group filtering
        let prod_hosts = store.list_by_group("production").await.unwrap();
        assert_eq!(prod_hosts.len(), 1);
        assert_eq!(store.list_by_group("staging").await.unwrap().len(), 0);

        // Tag filtering
        let eu_hosts = store.list_by_tag("eu-west").await.unwrap();
        assert_eq!(eu_hosts.len(), 1);
        assert_eq!(store.list_by_tag("us-east").await.unwrap().len(), 0);

        // Search
        assert_eq!(store.search("alice").await.unwrap().len(), 1);
        assert_eq!(store.search("prod").await.unwrap().len(), 1);
        assert_eq!(store.search("jump.example").await.unwrap().len(), 1);
        assert_eq!(store.search("notfound").await.unwrap().len(), 0);

        assert!(store.delete(id).await.unwrap());
        assert!(!store.delete(id).await.unwrap());
        assert!(store.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn validates_data_loaded_from_database() {
        let store = HostStore::open(":memory:").await.unwrap();
        store.connection.execute("INSERT INTO hosts(name, hostname, username, port) VALUES ('bad', '-option', 'alice', 22)", ()).await.unwrap();
        assert!(matches!(
            store.list().await,
            Err(StorageError::InvalidDestination(
                DestinationError::InvalidHost
            ))
        ));
    }

    #[tokio::test]
    async fn updates_persist_and_invalid_edits_leave_existing_data_intact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.db");
        let path = path.to_str().unwrap();
        let mut store = HostStore::open(path).await.unwrap();
        let original = SshDestination::new("example.org", "alice", 22).unwrap();
        let id = store
            .add("Server", &original, Some("Staging"), &[], None, None)
            .await
            .unwrap();
        let updated = SshDestination::new("2001:db8::1", "bob", 2222).unwrap();
        assert!(matches!(
            store
                .update(id, "\0", &updated, None, &[], None, None)
                .await,
            Err(StorageError::InvalidName)
        ));
        assert_eq!(store.list().await.unwrap()[0].destination, original);
        assert!(!store
            .update(id + 1, "Other", &updated, None, &[], None, None)
            .await
            .unwrap());
        assert!(store
            .update(
                id,
                " Updated ",
                &updated,
                Some("Prod"),
                &["web".to_string()],
                Some("bastion.corp"),
                None,
            )
            .await
            .unwrap());
        drop(store);
        let store = HostStore::open(path).await.unwrap();
        assert_eq!(
            store.list().await.unwrap(),
            vec![SavedHost {
                id,
                name: "Updated".into(),
                destination: updated,
                group_name: Some("Prod".into()),
                tags: vec!["web".to_string()],
                jump_host: Some("bastion.corp".into()),
                startup_snippet_id: None,
                protocol: "ssh".into(),
                protocol_config: None,
                identity_id: None,
            }]
        );
    }

    #[tokio::test]
    async fn manages_tunnels_lifecycle() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let dest = SshDestination::new("10.0.0.1", "admin", 22).unwrap();
        let host_id = store
            .add("Host1", &dest, None, &[], None, None)
            .await
            .unwrap();

        // Local tunnel
        let rule_local =
            PortForwardRule::local(Some("127.0.0.1"), 8080, "postgres-db", 5432).unwrap();
        let t1 = store
            .add_tunnel("PG Forward", Some(host_id), &rule_local, true)
            .await
            .unwrap();

        // Dynamic tunnel
        let rule_dyn = PortForwardRule::dynamic(None, 1080).unwrap();
        let t2 = store
            .add_tunnel("Socks", None, &rule_dyn, false)
            .await
            .unwrap();

        let all_tunnels = store.list_tunnels().await.unwrap();
        assert_eq!(all_tunnels.len(), 2);
        assert_eq!(all_tunnels[0].id, t1);
        assert_eq!(all_tunnels[0].name, "PG Forward");
        assert_eq!(all_tunnels[0].rule.forwarding_type, ForwardingType::Local);
        assert!(all_tunnels[0].auto_start);

        assert_eq!(all_tunnels[1].id, t2);
        assert_eq!(all_tunnels[1].rule.forwarding_type, ForwardingType::Dynamic);
        assert!(!all_tunnels[1].auto_start);

        // List by host
        let host_tunnels = store.list_tunnels_for_host(host_id).await.unwrap();
        assert_eq!(host_tunnels.len(), 1);
        assert_eq!(host_tunnels[0].id, t1);

        // Update tunnel
        let rule_updated = PortForwardRule::local(None, 9090, "postgres-db", 5432).unwrap();
        assert!(store
            .update_tunnel(t1, "PG Updated", Some(host_id), &rule_updated, false)
            .await
            .unwrap());
        let updated_list = store.list_tunnels_for_host(host_id).await.unwrap();
        assert_eq!(updated_list[0].name, "PG Updated");
        assert_eq!(updated_list[0].rule.bind_port, 9090);
        assert!(!updated_list[0].auto_start);

        // Delete tunnel
        assert!(store.delete_tunnel(t1).await.unwrap());
        assert!(!store.delete_tunnel(t1).await.unwrap());
        assert_eq!(store.list_tunnels_for_host(host_id).await.unwrap().len(), 0);
        assert_eq!(store.list_tunnels().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn manages_encrypted_vault_export_and_import() {
        let mut store_a = HostStore::open(":memory:").await.unwrap();

        // 1. Group
        let group = HostGroup::new(
            "CloudServers",
            Some("ubuntu"),
            Some(22),
            &["production".into()],
        )
        .unwrap();
        store_a.add_group(&group).await.unwrap();

        // 2. Host
        let dest = SshDestination::new("web1.internal", "app", 2222).unwrap();
        store_a
            .add(
                "WebProd",
                &dest,
                Some("CloudServers"),
                &["nginx".into()],
                None,
                None,
            )
            .await
            .unwrap();

        // 3. Tunnel
        let rule = PortForwardRule::local(Some("127.0.0.1"), 8080, "web1.internal", 80).unwrap();
        store_a
            .add_tunnel("Web Tunnel", None, &rule, true)
            .await
            .unwrap();

        // 4. Snippet
        let snippet = Snippet::new(
            "Restart Nginx",
            "sudo systemctl restart nginx",
            Some("restarts web server".into()),
            &["nginx".into()],
        )
        .unwrap();
        store_a.add_snippet(&snippet).await.unwrap();

        // Export vault
        let passphrase = "vault-passphrase-2026";
        let vault = store_a
            .export_vault_with_iterations(passphrase, "macbook-pro", 2_000)
            .await
            .unwrap();
        assert_eq!(vault.records.len(), 4);

        // Verify JSON roundtrip
        let json = vault.to_json().unwrap();
        let parsed_vault = shellumi_core::SyncVault::from_json(&json).unwrap();
        assert_eq!(parsed_vault.records.len(), 4);

        // Import into fresh database
        let mut store_b = HostStore::open(":memory:").await.unwrap();
        let summary = store_b
            .import_vault(passphrase, &parsed_vault)
            .await
            .unwrap();
        assert_eq!(summary.groups_imported, 1);
        assert_eq!(summary.hosts_imported, 1);
        assert_eq!(summary.tunnels_imported, 1);
        assert_eq!(summary.snippets_imported, 1);
        assert!(summary.errors.is_empty());

        // Verify restored entities in store_b
        let groups = store_b.list_groups().await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "CloudServers");

        let hosts = store_b.list().await.unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "WebProd");
        assert_eq!(hosts[0].destination.port().get(), 2222);

        let tunnels = store_b.list_tunnels().await.unwrap();
        assert_eq!(tunnels.len(), 1);
        assert_eq!(tunnels[0].name, "Web Tunnel");
        assert_eq!(tunnels[0].rule.bind_port, 8080);

        let snippets = store_b.list_snippets().await.unwrap();
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].name, "Restart Nginx");

        // Verify failed import with wrong passphrase
        let mut store_c = HostStore::open(":memory:").await.unwrap();
        let wrong_summary = store_c
            .import_vault("wrong-passphrase", &parsed_vault)
            .await
            .unwrap();
        assert_eq!(wrong_summary.hosts_imported, 0);
        assert_eq!(wrong_summary.errors.len(), 4);
    }

    #[tokio::test]
    async fn manages_hosts_with_different_protocols() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        // 1. Add SSH host
        let ssh_dest = SshDestination::new("ssh.example.com", "root", 22).unwrap();
        let ssh_id = store
            .add("SSH Node", &ssh_dest, None, &[], None, None)
            .await
            .unwrap();
        let host1 = store.get_host(ssh_id).await.unwrap().unwrap();
        assert_eq!(host1.protocol, "ssh");
        assert!(host1.is_ssh());
        assert!(!host1.is_telnet());

        // 2. Add Telnet host
        let telnet_dest = SshDestination::new("switch.corp", "admin", 23).unwrap();
        let telnet_id = store
            .add_with_protocol(
                "Cisco Switch",
                &telnet_dest,
                None,
                &["network".into()],
                None,
                None,
                "telnet",
                None,
            )
            .await
            .unwrap();
        let host2 = store.get_host(telnet_id).await.unwrap().unwrap();
        assert_eq!(host2.protocol, "telnet");
        assert!(host2.is_telnet());
        assert_eq!(host2.destination.port().get(), 23);

        // 3. Add Serial host
        let serial_dest = SshDestination::new("ttyUSB0", "console", 1).unwrap();
        let serial_id = store
            .add_with_protocol(
                "Router Serial Console",
                &serial_dest,
                None,
                &["serial".into()],
                None,
                None,
                "serial",
                Some(r#"{"baud_rate":115200,"data_bits":8,"parity":"none","stop_bits":1}"#),
            )
            .await
            .unwrap();
        let host3 = store.get_host(serial_id).await.unwrap().unwrap();
        assert_eq!(host3.protocol, "serial");
        assert!(host3.is_serial());
        assert_eq!(
            host3.protocol_config.as_deref(),
            Some(r#"{"baud_rate":115200,"data_bits":8,"parity":"none","stop_bits":1}"#)
        );

        // 4. Add VNC host
        let vnc_dest = SshDestination::new("desktop.corp", "user", 5900).unwrap();
        let vnc_id = store
            .add_with_protocol(
                "Linux Workstation VNC",
                &vnc_dest,
                None,
                &["desktop".into()],
                None,
                None,
                "vnc",
                Some(r#"{"host":"desktop.corp","port":5900,"color_depth":24,"view_only":false,"shared":true,"quality":8,"enable_clipboard":true}"#),
            )
            .await
            .unwrap();
        let host4 = store.get_host(vnc_id).await.unwrap().unwrap();
        assert_eq!(host4.protocol, "vnc");
        assert!(host4.is_vnc());
        assert!(host4.is_graphical());
        assert_eq!(host4.vnc_config().unwrap().color_depth, 24);

        // 5. Add RDP host
        let rdp_dest = SshDestination::new("win-vm.corp", "Administrator", 3389).unwrap();
        let rdp_id = store
            .add_with_protocol(
                "Windows Server RDP",
                &rdp_dest,
                None,
                &["windows".into()],
                None,
                None,
                "rdp",
                Some(r#"{"host":"win-vm.corp","port":3389,"username":"Administrator","screen_width":1920,"screen_height":1080,"color_depth":32,"enable_audio":false,"enable_clipboard":true,"enable_font_smoothing":true}"#),
            )
            .await
            .unwrap();
        let host5 = store.get_host(rdp_id).await.unwrap().unwrap();
        assert_eq!(host5.protocol, "rdp");
        assert!(host5.is_rdp());
        assert!(host5.is_graphical());
        assert_eq!(host5.rdp_config().unwrap().screen_width, 1920);

        // 6. Update host protocol
        let updated_dest = SshDestination::new("switch.corp", "admin", 22).unwrap();
        assert!(store
            .update_with_protocol(
                telnet_id,
                "Cisco Switch Upgraded",
                &updated_dest,
                None,
                &["network".into(), "hardened".into()],
                None,
                None,
                "ssh",
                None,
            )
            .await
            .unwrap());
        let updated_host = store.get_host(telnet_id).await.unwrap().unwrap();
        assert_eq!(updated_host.name, "Cisco Switch Upgraded");
        assert_eq!(updated_host.protocol, "ssh");
        assert!(updated_host.is_ssh());
    }

    #[tokio::test]
    async fn manages_shared_vaults_and_team_collaboration() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        // 1. Create shared vault
        let vault = SharedVault::new(
            "v-101",
            "Core SRE Vault",
            Some("Infrastructure team secrets".into()),
            "alice@corp.internal",
            "2026-11-05T00:00:00Z",
        )
        .unwrap();

        store.create_shared_vault(&vault).await.unwrap();
        let fetched_vault = store.get_shared_vault("v-101").await.unwrap().unwrap();
        assert_eq!(fetched_vault.name, "Core SRE Vault");
        assert_eq!(fetched_vault.current_epoch, 1);
        assert_eq!(fetched_vault.owner_id, "alice@corp.internal");

        let vaults = store.list_shared_vaults().await.unwrap();
        assert_eq!(vaults.len(), 1);

        // 2. Add team members
        let member1 = TeamMember::new(
            "m-1",
            "v-101",
            "alice@corp.internal",
            "Alice",
            "alice_pubkey_hex",
            TeamRole::Owner,
            "2026-11-05T00:00:00Z",
            "alice@corp.internal",
        );
        let member2 = TeamMember::new(
            "m-2",
            "v-101",
            "bob@corp.internal",
            "Bob",
            "bob_pubkey_hex",
            TeamRole::Editor,
            "2026-11-05T00:05:00Z",
            "alice@corp.internal",
        );

        store.add_team_member(&member1).await.unwrap();
        store.add_team_member(&member2).await.unwrap();

        let members = store.list_team_members("v-101").await.unwrap();
        assert_eq!(members.len(), 2);

        // 3. Update member role and revoke
        assert!(store
            .update_team_member_role("m-2", TeamRole::Administrator)
            .await
            .unwrap());
        let updated_m2 = store.get_team_member("m-2").await.unwrap().unwrap();
        assert_eq!(updated_m2.role, TeamRole::Administrator);

        assert!(store
            .revoke_team_member("m-2", "2026-11-05T01:00:00Z")
            .await
            .unwrap());
        let revoked_m2 = store.get_team_member("m-2").await.unwrap().unwrap();
        assert!(revoked_m2.revoked);
        assert_eq!(
            revoked_m2.revoked_at.as_deref(),
            Some("2026-11-05T01:00:00Z")
        );

        // 4. Log audit events
        let audit = SharingAuditRecord::new(
            "a-1",
            "v-101",
            AuditAction::MemberRevoked,
            "alice@corp.internal",
            Some("bob@corp.internal".into()),
            Some("revoked access and rotating key".into()),
            "2026-11-05T01:00:01Z",
        );
        store.log_audit_event(&audit).await.unwrap();

        let audits = store.list_audit_log("v-101", Some(10)).await.unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, AuditAction::MemberRevoked);

        // 5. Rotate key (increment epoch)
        assert!(store
            .update_shared_vault_epoch("v-101", 2, "2026-11-05T01:00:02Z")
            .await
            .unwrap());
        let rotated_vault = store.get_shared_vault("v-101").await.unwrap().unwrap();
        assert_eq!(rotated_vault.current_epoch, 2);

        // 6. Shared session logs
        let mut session_log = SharedSessionLog::new(
            "log-100",
            "v-101",
            "prod-k8s-master",
            "alice@corp.internal",
            "2026-11-05T01:10:00Z",
            30,
        );
        session_log.add_bookmark(LogBookmark {
            id: "b-1".into(),
            line_number: 120,
            label: "etcd leader election".into(),
            created_at: "2026-11-05T01:12:00Z".into(),
        });
        session_log.add_comment(LogComment {
            id: "c-1".into(),
            author_id: "alice@corp.internal".into(),
            line_number: 120,
            comment: "Confirmed leader election resolved within 50ms".into(),
            created_at: "2026-11-05T01:15:00Z".into(),
        });

        store.create_shared_session_log(&session_log).await.unwrap();
        let logs = store.list_shared_session_logs("v-101").await.unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].host_name, "prod-k8s-master");
        assert_eq!(logs[0].bookmarks.len(), 1);
        assert_eq!(logs[0].comments.len(), 1);

        // 7. Delete vault cascade
        assert!(store.delete_shared_vault("v-101").await.unwrap());
        assert_eq!(store.list_shared_vaults().await.unwrap().len(), 0);
        assert_eq!(store.list_team_members("v-101").await.unwrap().len(), 0);
        assert_eq!(store.list_audit_log("v-101", None).await.unwrap().len(), 0);
        assert_eq!(
            store.list_shared_session_logs("v-101").await.unwrap().len(),
            0
        );
    }

    #[tokio::test]
    async fn manages_cloud_providers_and_discovered_hosts_lifecycle() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        // 1. Create Cloud Provider
        let provider = CloudProviderRecord {
            id: "cp-aws-prod".to_string(),
            name: "AWS US-East Production".to_string(),
            provider_kind: "aws".to_string(),
            config_json: r#"{"region":"us-east-1","use_private_ip":false}"#.to_string(),
            sync_interval_mins: 30,
            last_synced_at: None,
            auto_sync: true,
            created_at: 1700000000,
            updated_at: 1700000000,
        };
        store.create_cloud_provider(&provider).await.unwrap();

        let retrieved = store
            .get_cloud_provider("cp-aws-prod")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.name, "AWS US-East Production");
        assert_eq!(retrieved.provider_kind, "aws");
        assert!(retrieved.auto_sync);

        let all_providers = store.list_cloud_providers().await.unwrap();
        assert_eq!(all_providers.len(), 1);

        // 2. Update sync time
        assert!(store
            .update_cloud_provider_sync_time("cp-aws-prod", 1700001000)
            .await
            .unwrap());
        let updated_prov = store
            .get_cloud_provider("cp-aws-prod")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated_prov.last_synced_at, Some(1700001000));

        // 3. Save Discovered Hosts
        let hosts = vec![
            DiscoveredHostRecord {
                id: "dh-1".to_string(),
                provider_id: "cp-aws-prod".to_string(),
                external_id: "aws:i-0123456789".to_string(),
                name: "api-gw-01".to_string(),
                hostname: "54.210.1.20".to_string(),
                port: 22,
                username: Some("ec2-user".to_string()),
                state: "running".to_string(),
                region: Some("us-east-1".to_string()),
                group_path: Some("AWS/us-east-1/Prod".to_string()),
                tags_json: r#"{"Environment":"Prod"}"#.to_string(),
                metadata_json: r#"{"instance_type":"t3.medium"}"#.to_string(),
                imported_host_id: None,
                last_seen_at: 1700001000,
            },
            DiscoveredHostRecord {
                id: "dh-2".to_string(),
                provider_id: "cp-aws-prod".to_string(),
                external_id: "aws:i-9876543210".to_string(),
                name: "db-replica-01".to_string(),
                hostname: "10.0.5.15".to_string(),
                port: 22,
                username: Some("ec2-user".to_string()),
                state: "stopped".to_string(),
                region: Some("us-east-1".to_string()),
                group_path: Some("AWS/us-east-1/Prod".to_string()),
                tags_json: r#"{"Environment":"Prod"}"#.to_string(),
                metadata_json: r#"{"instance_type":"r6i.large"}"#.to_string(),
                imported_host_id: None,
                last_seen_at: 1700001000,
            },
        ];

        let saved_count = store
            .save_discovered_hosts("cp-aws-prod", &hosts)
            .await
            .unwrap();
        assert_eq!(saved_count, 2);

        let discovered = store
            .list_discovered_hosts(Some("cp-aws-prod"))
            .await
            .unwrap();
        assert_eq!(discovered.len(), 2);
        assert_eq!(discovered[0].name, "api-gw-01");
        assert_eq!(discovered[1].name, "db-replica-01");

        // 4. Mark discovered host imported
        let dest = SshDestination::new("54.210.1.20", "ec2-user", 22).unwrap();
        let host_id = store
            .add("api-gw-01", &dest, None, &[], None, None)
            .await
            .unwrap();
        assert!(store
            .mark_discovered_host_imported("dh-1", host_id)
            .await
            .unwrap());
        let updated_dh = store
            .list_discovered_hosts(Some("cp-aws-prod"))
            .await
            .unwrap();
        let dh1 = updated_dh.iter().find(|h| h.id == "dh-1").unwrap();
        assert_eq!(dh1.imported_host_id, Some(host_id));

        // 5. Delete provider cascade
        assert!(store.delete_cloud_provider("cp-aws-prod").await.unwrap());
        assert_eq!(store.list_cloud_providers().await.unwrap().len(), 0);
        assert_eq!(
            store
                .list_discovered_hosts(Some("cp-aws-prod"))
                .await
                .unwrap()
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn migrates_schema_to_version_13_and_manages_session_recordings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v12.db");
        let path_str = path.to_str().unwrap();

        // Create a database with user_version = 12
        {
            let store = HostStore::open(path_str).await.unwrap();
            store
                .connection
                .execute("DROP TABLE vault_contexts", ())
                .await
                .unwrap();
            store
                .connection
                .execute("DROP TABLE vault_server_connections", ())
                .await
                .unwrap();
            store
                .connection
                .execute("PRAGMA user_version = 12", ())
                .await
                .unwrap();
            store
                .connection
                .execute("DROP TABLE IF EXISTS session_recordings", ())
                .await
                .unwrap();
        }

        // Reopen database; should auto-migrate to the current version
        let mut store = HostStore::open(path_str).await.unwrap();
        let mut rows = store
            .connection
            .query("PRAGMA user_version", ())
            .await
            .unwrap();
        let version: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        // Add a host
        let dest = SshDestination::new("10.0.0.50", "admin", 22).unwrap();
        let host_id = store
            .add("Bastion", &dest, None, &[], None, None)
            .await
            .unwrap();

        // 1. Save session recording with host
        let rec1 = SessionRecordingRecord {
            id: "rec-1".to_string(),
            session_id: "sess-abc".to_string(),
            host_id: Some(host_id),
            title: "Production Deployment".to_string(),
            duration_secs: 45.5,
            event_count: 120,
            byte_size: 4096,
            cast_data:
                "{\"version\":2,\"width\":80,\"height\":24}\n[0.1, \"o\", \"Deploying...\"]\n"
                    .to_string(),
            recorded_at: "2026-11-05T07:00:00Z".to_string(),
            created_at: "2026-11-05T07:01:00Z".to_string(),
        };
        store.save_session_recording(&rec1).await.unwrap();

        // 2. Save session recording without host
        let rec2 = SessionRecordingRecord {
            id: "rec-2".to_string(),
            session_id: "sess-xyz".to_string(),
            host_id: None,
            title: "Local Shell Session".to_string(),
            duration_secs: 15.0,
            event_count: 35,
            byte_size: 1024,
            cast_data: "{\"version\":2,\"width\":80,\"height\":24}\n[0.1, \"o\", \"ls -la\"]\n"
                .to_string(),
            recorded_at: "2026-11-05T07:10:00Z".to_string(),
            created_at: "2026-11-05T07:11:00Z".to_string(),
        };
        store.save_session_recording(&rec2).await.unwrap();

        // 3. List recordings
        let all_recs = store.list_session_recordings(None).await.unwrap();
        assert_eq!(all_recs.len(), 2);
        assert_eq!(all_recs[0].id, "rec-2"); // Ordered by recorded_at DESC
        assert_eq!(all_recs[1].id, "rec-1");

        let host_recs = store.list_session_recordings(Some(host_id)).await.unwrap();
        assert_eq!(host_recs.len(), 1);
        assert_eq!(host_recs[0].id, "rec-1");
        assert_eq!(host_recs[0].title, "Production Deployment");
        assert_eq!(host_recs[0].formatted_duration(), "00:46");
        assert_eq!(host_recs[0].formatted_size(), "4.0 KB");

        // 4. Retrieve single recording
        let fetched = store
            .get_session_recording("rec-1")
            .await
            .unwrap()
            .expect("exists");
        assert_eq!(fetched.id, "rec-1");
        assert_eq!(fetched.duration_secs, 45.5);
        assert_eq!(fetched.cast_data, rec1.cast_data);

        // 5. Update existing recording
        let mut rec1_updated = rec1.clone();
        rec1_updated.title = "Production Deployment (Completed)".to_string();
        rec1_updated.duration_secs = 60.0;
        store.save_session_recording(&rec1_updated).await.unwrap();
        let fetched_updated = store.get_session_recording("rec-1").await.unwrap().unwrap();
        assert_eq!(fetched_updated.title, "Production Deployment (Completed)");
        assert_eq!(fetched_updated.duration_secs, 60.0);

        // 6. Deleting host sets host_id to NULL via foreign key
        store.delete(host_id).await.unwrap();
        let fetched_after_host_del = store.get_session_recording("rec-1").await.unwrap().unwrap();
        assert_eq!(fetched_after_host_del.host_id, None);

        // 7. Delete single recording
        assert!(store.delete_session_recording("rec-1").await.unwrap());
        assert_eq!(store.get_session_recording("rec-1").await.unwrap(), None);

        // 8. Delete all recordings
        assert_eq!(store.delete_all_session_recordings().await.unwrap(), 1);
        assert_eq!(store.list_session_recordings(None).await.unwrap().len(), 0);

        // 9. Validation error on empty title or id
        let invalid = SessionRecordingRecord {
            id: "".to_string(),
            session_id: "s1".to_string(),
            host_id: None,
            title: "Valid".to_string(),
            duration_secs: 1.0,
            event_count: 1,
            byte_size: 1,
            cast_data: "{}".to_string(),
            recorded_at: "now".to_string(),
            created_at: "now".to_string(),
        };
        assert!(store.save_session_recording(&invalid).await.is_err());
    }

    #[tokio::test]
    async fn migrates_schema_to_version_14_and_manages_identities_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v13.db");
        let path_str = path.to_str().unwrap();

        // 1. Create a database with user_version = 13 without identities table
        {
            let store = HostStore::open(path_str).await.unwrap();
            store
                .connection
                .execute("DROP TABLE vault_contexts", ())
                .await
                .unwrap();
            store
                .connection
                .execute("DROP TABLE vault_server_connections", ())
                .await
                .unwrap();
            store
                .connection
                .execute("PRAGMA user_version = 13", ())
                .await
                .unwrap();
            store
                .connection
                .execute("DROP TABLE IF EXISTS identities", ())
                .await
                .unwrap();
        }

        // 2. Reopen database; should auto-migrate to the current version
        let mut store = HostStore::open(path_str).await.unwrap();
        let mut rows = store
            .connection
            .query("PRAGMA user_version", ())
            .await
            .unwrap();
        let version: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        // 3. Create identities
        let mut new_id1 = shellumi_core::NewIdentity::new(
            "Cloud Ops",
            "ubuntu",
            shellumi_core::AuthMethod::PublicKey {
                key_name: "ops_ed25519".into(),
            },
        )
        .unwrap()
        .with_tags(&["cloud".to_string(), "infra".to_string()])
        .unwrap();
        new_id1.key_path = Some("/home/user/.ssh/ops_ed25519".into());
        new_id1.comment = Some("Primary devops identity".into());

        let id1 = store.create_identity(&new_id1).await.unwrap();
        assert!(id1 > 0);

        let mut new_id2 = shellumi_core::NewIdentity::new(
            "Bastion Certificate User",
            "cert-admin",
            shellumi_core::AuthMethod::Certificate {
                cert_path: "/etc/ssh/user-cert.pub".into(),
                key_path: Some("/etc/ssh/user-key".into()),
            },
        )
        .unwrap();
        new_id2.certificate_path = Some("/etc/ssh/user-cert.pub".into());
        let id2 = store.create_identity(&new_id2).await.unwrap();
        assert!(id2 > id1);

        // 4. Retrieve and verify identities
        let fetched1 = store.get_identity(id1).await.unwrap().unwrap();
        assert_eq!(fetched1.name, "Cloud Ops");
        assert_eq!(fetched1.username, "ubuntu");
        assert_eq!(
            fetched1.key_path,
            Some("/home/user/.ssh/ops_ed25519".into())
        );
        assert_eq!(fetched1.tags, vec!["cloud", "infra"]);

        let fetched_by_name = store
            .get_identity_by_name("Bastion Certificate User")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched_by_name.id, id2);
        assert_eq!(fetched_by_name.username, "cert-admin");

        let all_identities = store.list_identities().await.unwrap();
        assert_eq!(all_identities.len(), 2);

        // 5. Update identity
        new_id1.username = "admin_user".into();
        assert!(store.update_identity(id1, &new_id1).await.unwrap());
        let updated1 = store.get_identity(id1).await.unwrap().unwrap();
        assert_eq!(updated1.username, "admin_user");

        // 6. Bind identity to a host and to a group
        let group = HostGroup::new("Cloud Cluster", None, Some(22), &["cloud".into()])
            .unwrap()
            .with_identity_id(Some(id1));
        store.add_group(&group).await.unwrap();

        let host_dest = SshDestination::new("10.0.1.10", "default_placeholder", 22).unwrap();
        let host_id = store
            .add_full(
                "App Server",
                &host_dest,
                Some("Cloud Cluster"),
                &["web".into()],
                None,
                None,
                "ssh",
                None,
                Some(id2),
            )
            .await
            .unwrap();

        let saved_host = store.get_host(host_id).await.unwrap().unwrap();
        assert_eq!(saved_host.identity_id, Some(id2));

        // Effective destination should use host's identity username ("cert-admin")
        let effective_dest = store.get_effective_destination(&saved_host).await.unwrap();
        assert_eq!(effective_dest.username(), "cert-admin");

        // 7. Test group identity inheritance when host identity is None
        let host_id2 = store
            .add_full(
                "DB Server",
                &host_dest,
                Some("Cloud Cluster"),
                &["db".into()],
                None,
                None,
                "ssh",
                None,
                None,
            )
            .await
            .unwrap();
        let saved_host2 = store.get_host(host_id2).await.unwrap().unwrap();
        assert_eq!(saved_host2.identity_id, None);
        // Effective destination should inherit from group's identity username ("admin_user")
        let effective_dest2 = store.get_effective_destination(&saved_host2).await.unwrap();
        assert_eq!(effective_dest2.username(), "admin_user");

        // 8. Delete identity: foreign key should set host and group identity_id to NULL
        assert!(store.delete_identity(id2).await.unwrap());
        let host_after_delete = store.get_host(host_id).await.unwrap().unwrap();
        assert_eq!(host_after_delete.identity_id, None);
    }

    #[tokio::test]
    async fn manages_identity_keychain_secrets_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets_test.db");
        let path_str = path.to_str().unwrap();

        let mut host_store = HostStore::open(path_str).await.unwrap();
        let key_store = shellumi_core::EphemeralCredentialStore::new();

        let new_id = shellumi_core::NewIdentity::new(
            "Production Bastion",
            "deployer",
            shellumi_core::AuthMethod::Password,
        )
        .unwrap();

        let id = host_store
            .create_identity_with_secrets(
                new_id,
                Some("very-secure-password-123"),
                Some("key-passphrase-456"),
                &key_store,
            )
            .await
            .unwrap();

        let saved = host_store.get_identity(id).await.unwrap().unwrap();
        assert!(saved.password_secret_ref.is_some());
        assert!(saved.passphrase_secret_ref.is_some());

        let pw_ref = saved.password_secret_ref.as_ref().unwrap();
        assert!(pw_ref.starts_with("keychain://"));
        let pp_ref = saved.passphrase_secret_ref.as_ref().unwrap();
        assert!(pp_ref.starts_with("keychain://"));

        // Resolve secrets via HostStore
        let resolved_pw = host_store
            .resolve_identity_password(&saved, &key_store)
            .unwrap()
            .unwrap();
        assert_eq!(resolved_pw.expose_secret(), "very-secure-password-123");

        let resolved_pp = host_store
            .resolve_identity_passphrase(&saved, &key_store)
            .unwrap()
            .unwrap();
        assert_eq!(resolved_pp.expose_secret(), "key-passphrase-456");

        // Delete identity and verify secrets are purged from keychain
        assert!(host_store
            .delete_identity_with_secrets(id, &key_store)
            .await
            .unwrap());
        assert!(host_store.get_identity(id).await.unwrap().is_none());

        // Verifying keychain secret deleted
        let (pw_svc, pw_acc) = shellumi_core::parse_keychain_ref(pw_ref).unwrap();
        use shellumi_core::CredentialStore;
        assert_eq!(key_store.get_secret(&pw_svc, &pw_acc).unwrap(), None);

        let (pp_svc, pp_acc) = shellumi_core::parse_keychain_ref(pp_ref).unwrap();
        assert_eq!(key_store.get_secret(&pp_svc, &pp_acc).unwrap(), None);
    }

    #[tokio::test]
    async fn test_schema_20_vault_contexts_recovery_and_lock() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        // Check user_version is at least 20
        let mut rows = store
            .connection
            .query("PRAGMA user_version", ())
            .await
            .unwrap();
        let version: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert!(version >= 20);

        // Verify recovery envelope storage
        assert_eq!(
            store
                .get_vault_recovery_envelope(LEGACY_LOCAL_VAULT_ID)
                .await
                .unwrap(),
            None
        );
        let sample_envelope = "RECOVERY:AESGCM:SALT123:DATA456:TAG789";
        assert!(store
            .save_vault_recovery_envelope(LEGACY_LOCAL_VAULT_ID, sample_envelope)
            .await
            .unwrap());
        assert_eq!(
            store
                .get_vault_recovery_envelope(LEGACY_LOCAL_VAULT_ID)
                .await
                .unwrap(),
            Some(sample_envelope.to_string())
        );

        // Verify vault lock status toggle
        assert!(!store.is_vault_locked(LEGACY_LOCAL_VAULT_ID).await.unwrap());
        assert!(store
            .set_vault_locked(LEGACY_LOCAL_VAULT_ID, true)
            .await
            .unwrap());
        assert!(store.is_vault_locked(LEGACY_LOCAL_VAULT_ID).await.unwrap());
        assert!(store
            .set_vault_locked(LEGACY_LOCAL_VAULT_ID, false)
            .await
            .unwrap());
        assert!(!store.is_vault_locked(LEGACY_LOCAL_VAULT_ID).await.unwrap());

        // Verify encrypted vault directory envelope storage
        assert_eq!(
            store
                .get_encrypted_vault_directory("primary-dir")
                .await
                .unwrap(),
            None
        );
        let sample_dir_data = "ENCRYPTED_VAULT_DIRECTORY_PAYLOAD_HERE";
        store
            .save_encrypted_vault_directory("primary-dir", sample_dir_data)
            .await
            .unwrap();
        assert_eq!(
            store
                .get_encrypted_vault_directory("primary-dir")
                .await
                .unwrap(),
            Some(sample_dir_data.to_string())
        );
    }

    #[tokio::test]
    async fn test_schema_20_groups_scoped_by_vault() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        // 1. Add group in legacy-local
        let group_local = HostGroup::new(
            "Infrastructure",
            Some("local-user"),
            Some(22),
            &["local".into()],
        )
        .unwrap();
        store.add_group(&group_local).await.unwrap();
        assert_eq!(store.list_groups().await.unwrap().len(), 1);

        // 2. Switch to another vault
        let v2 = shellumi_core::VaultId::new("vault-staging").unwrap();
        store.set_vault_id(v2);

        // In vault-staging, groups list should initially be empty
        assert_eq!(store.list_groups().await.unwrap().len(), 0);
        assert!(store.get_group("Infrastructure").await.unwrap().is_none());

        // 3. Create group with identical name in vault-staging but different defaults
        let group_staging = HostGroup::new(
            "Infrastructure",
            Some("staging-user"),
            Some(2222),
            &["staging".into()],
        )
        .unwrap();
        store.add_group(&group_staging).await.unwrap();
        assert_eq!(store.list_groups().await.unwrap().len(), 1);

        let retrieved_staging = store.get_group("Infrastructure").await.unwrap().unwrap();
        assert_eq!(
            retrieved_staging.default_username.as_deref(),
            Some("staging-user")
        );
        assert_eq!(retrieved_staging.default_port.map(|p| p.get()), Some(2222));

        // 4. Switch back to legacy-local and verify original group is unchanged
        let v_local = shellumi_core::VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
        store.set_vault_id(v_local);

        let retrieved_local = store.get_group("Infrastructure").await.unwrap().unwrap();
        assert_eq!(
            retrieved_local.default_username.as_deref(),
            Some("local-user")
        );
        assert_eq!(retrieved_local.default_port.map(|p| p.get()), Some(22));

        // 5. Deleting group in staging does not affect legacy-local
        store.set_vault_id(shellumi_core::VaultId::new("vault-staging").unwrap());
        assert!(store.delete_group("Infrastructure").await.unwrap());
        assert_eq!(store.list_groups().await.unwrap().len(), 0);

        store.set_vault_id(shellumi_core::VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap());
        assert_eq!(store.list_groups().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_schema_20_session_recordings_scoped_by_vault() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        let rec_local = SessionRecordingRecord {
            id: "rec-local-1".to_string(),
            session_id: "sess-1".to_string(),
            host_id: None,
            title: "Local Shell Session".to_string(),
            duration_secs: 12.0,
            event_count: 5,
            byte_size: 120,
            cast_data: "cast-data-1".to_string(),
            recorded_at: "2026-11-15T10:00:00Z".to_string(),
            created_at: "2026-11-15T10:00:00Z".to_string(),
        };
        store.save_session_recording(&rec_local).await.unwrap();
        assert_eq!(store.list_session_recordings(None).await.unwrap().len(), 1);
        assert!(store
            .get_session_recording("rec-local-1")
            .await
            .unwrap()
            .is_some());

        // Switch to vault-prod
        store.set_vault_id(shellumi_core::VaultId::new("vault-prod").unwrap());
        assert_eq!(store.list_session_recordings(None).await.unwrap().len(), 0);
        assert!(store
            .get_session_recording("rec-local-1")
            .await
            .unwrap()
            .is_none());

        let rec_prod = SessionRecordingRecord {
            id: "rec-prod-1".to_string(),
            session_id: "sess-2".to_string(),
            host_id: None,
            title: "Prod Root Session".to_string(),
            duration_secs: 42.0,
            event_count: 20,
            byte_size: 800,
            cast_data: "cast-data-prod".to_string(),
            recorded_at: "2026-11-15T11:00:00Z".to_string(),
            created_at: "2026-11-15T11:00:00Z".to_string(),
        };
        store.save_session_recording(&rec_prod).await.unwrap();
        assert_eq!(store.list_session_recordings(None).await.unwrap().len(), 1);

        // Delete all in vault-prod leaves legacy-local untouched
        assert_eq!(store.delete_all_session_recordings().await.unwrap(), 1);
        assert_eq!(store.list_session_recordings(None).await.unwrap().len(), 0);

        store.set_vault_id(shellumi_core::VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap());
        assert_eq!(store.list_session_recordings(None).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_cross_vault_transfers_policy_and_isolation() {
        use shellumi_core::{CrossVaultAction, CrossVaultError, CrossVaultTransferPolicy, VaultId};

        let mut store = HostStore::open(":memory:").await.unwrap();

        let v_src = VaultId::new("vault-alpha").unwrap();
        let v_dst = VaultId::new("vault-beta").unwrap();

        // 1. Create identity and host in vault-alpha
        store.set_vault_id(v_src.clone());
        let new_id = shellumi_core::NewIdentity::new(
            "Alpha Key",
            "root",
            shellumi_core::AuthMethod::PublicKey {
                key_name: "alpha_ed25519".into(),
            },
        )
        .unwrap();
        let id_row = store.create_identity(&new_id).await.unwrap();

        let dest = SshDestination::new("192.168.1.100", "root", 22).unwrap();
        let host_id = store
            .add_full(
                "Bastion Alpha",
                &dest,
                None,
                &["production".into()],
                None,
                None,
                "ssh",
                None,
                Some(id_row),
            )
            .await
            .unwrap();

        // 2. Reject transfer to same vault
        let policy_same =
            CrossVaultTransferPolicy::new(v_src.clone(), v_src.clone(), CrossVaultAction::Copy)
                .with_user_authorization(true);
        assert_eq!(
            store
                .copy_host_across_vaults(host_id, &policy_same)
                .await
                .unwrap_err()
                .to_string(),
            StorageError::CrossVault(CrossVaultError::SameSourceAndTargetVault).to_string()
        );

        // 3. Reject transfer without user authorization
        let policy_unauth =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Copy);
        assert_eq!(
            store
                .copy_host_across_vaults(host_id, &policy_unauth)
                .await
                .unwrap_err()
                .to_string(),
            StorageError::CrossVault(CrossVaultError::MissingAuthorization).to_string()
        );

        // 4. Reject transfer retaining credentials without confirmation
        let policy_unconfirmed =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Copy)
                .with_user_authorization(true)
                .with_credential_policy(false, false);
        assert!(matches!(
            store
                .copy_host_across_vaults(host_id, &policy_unconfirmed)
                .await
                .unwrap_err(),
            StorageError::CrossVault(CrossVaultError::UnconfirmedCredentialExposure(_))
        ));

        // 5. Successful copy with credentials stripped (safe default)
        let policy_safe =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Copy)
                .with_user_authorization(true)
                .with_credential_policy(true, false);
        let new_host_id = store
            .copy_host_across_vaults(host_id, &policy_safe)
            .await
            .unwrap();
        assert_ne!(host_id, new_host_id);

        // Verify host in destination has identity stripped
        store.set_vault_id(v_dst.clone());
        let dst_host = store.get_host(new_host_id).await.unwrap().unwrap();
        assert_eq!(dst_host.name, "Bastion Alpha");
        assert_eq!(dst_host.identity_id, None);

        // Source host still exists in source vault
        store.set_vault_id(v_src.clone());
        assert!(store.get_host(host_id).await.unwrap().is_some());

        // 6. Successful move with confirmed credentials
        let policy_move =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Move)
                .with_user_authorization(true)
                .with_credential_policy(false, true);
        let moved_host_id = store
            .move_host_across_vaults(host_id, &policy_move)
            .await
            .unwrap();

        // Host deleted from source vault
        store.set_vault_id(v_src.clone());
        assert!(store.get_host(host_id).await.unwrap().is_none());

        // Destination host exists and has cloned identity
        store.set_vault_id(v_dst.clone());
        let moved_dst_host = store.get_host(moved_host_id).await.unwrap().unwrap();
        assert_eq!(moved_dst_host.name, "Bastion Alpha");
        assert!(moved_dst_host.identity_id.is_some());
        let cloned_id = store
            .get_identity(moved_dst_host.identity_id.unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cloned_id.name, "Alpha Key (vault-beta)");

        // 7. Snippets cross-vault copy and move
        store.set_vault_id(v_src.clone());
        let snip = Snippet::new("Docker Cleanup", "docker system prune -af", None, &[]).unwrap();
        let snip_id = store.add_snippet(&snip).await.unwrap();

        let snip_policy =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Copy)
                .with_user_authorization(true);
        let new_snip_id = store
            .copy_snippet_across_vaults(snip_id, &snip_policy)
            .await
            .unwrap();

        store.set_vault_id(v_dst.clone());
        let dst_snip = store.get_snippet(new_snip_id).await.unwrap().unwrap();
        assert_eq!(dst_snip.name, "Docker Cleanup");

        let snip_move_policy =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Move)
                .with_user_authorization(true);
        let moved_snip_id = store
            .move_snippet_across_vaults(snip_id, &snip_move_policy)
            .await
            .unwrap();

        store.set_vault_id(v_src.clone());
        assert!(store.get_snippet(snip_id).await.unwrap().is_none());
        store.set_vault_id(v_dst.clone());
        assert!(store.get_snippet(moved_snip_id).await.unwrap().is_some());

        // 8. Workspace cross-vault copy and move
        store.set_vault_id(v_src.clone());
        let ws_id = store
            .add_workspace("Ops Grid", None, "split-vertical", &[], &["ops".into()])
            .await
            .unwrap();

        let ws_policy =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Copy)
                .with_user_authorization(true);
        let new_ws_id = store
            .copy_workspace_across_vaults(ws_id, &ws_policy)
            .await
            .unwrap();

        store.set_vault_id(v_dst.clone());
        let dst_ws = store.get_workspace(new_ws_id).await.unwrap().unwrap();
        assert_eq!(dst_ws.name, "Ops Grid");

        let ws_move_policy =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Move)
                .with_user_authorization(true);
        let moved_ws_id = store
            .move_workspace_across_vaults(ws_id, &ws_move_policy)
            .await
            .unwrap();

        store.set_vault_id(v_src.clone());
        assert!(store.get_workspace(ws_id).await.unwrap().is_none());
        store.set_vault_id(v_dst.clone());
        assert!(store.get_workspace(moved_ws_id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_vault_directory_sync_and_cross_device_import() {
        use shellumi_core::{ServerConnectionId, VaultContext, VaultId, VaultKey, VaultLocation};

        let mut store_a = HostStore::open(":memory:").await.unwrap();
        let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
        let default_key = VaultKey::generate().unwrap();

        // Register a secondary local vault and a remote server vault in store_a
        let secondary_vid = VaultId::new("work-vault").unwrap();
        let secondary_ctx = VaultContext::new(secondary_vid.clone(), VaultLocation::Local);
        store_a
            .register_vault_context("Work Vault", &secondary_ctx)
            .await
            .unwrap();

        let srv_conn_id = ServerConnectionId::new("srv-corp").unwrap();
        store_a.register_vault_server(&srv_conn_id).await.unwrap();
        let remote_vid = VaultId::new("corp-remote-123").unwrap();
        let remote_vid_obj = VaultId::new("corp-vault").unwrap();
        let remote_ctx = VaultContext::new(
            remote_vid_obj.clone(),
            VaultLocation::Remote {
                server: srv_conn_id,
                server_vault_id: remote_vid,
            },
        );
        store_a
            .register_vault_context("Corp Vault", &remote_ctx)
            .await
            .unwrap();

        // Build vault directory and save it encrypted
        let dir = store_a.build_vault_directory().await.unwrap();
        assert_eq!(dir.entries.len(), 3);
        assert!(dir.get(LEGACY_LOCAL_VAULT_ID).is_some());
        assert!(dir.get("work-vault").is_some());
        assert!(dir.get("corp-vault").is_some());

        let envelope_json = store_a
            .save_vault_directory_from_model(&default_vid, &default_key, &dir)
            .await
            .unwrap();

        // Verify export_vault attaches the encrypted directory
        store_a.set_vault_id(default_vid.clone());
        let exported = store_a
            .export_vault("test-passphrase-1234", "device-a")
            .await
            .unwrap();
        assert_eq!(
            exported.encrypted_directory.as_deref(),
            Some(envelope_json.as_str())
        );

        // In a fresh store_b (representing a second user device), import the encrypted vault directory
        let mut store_b = HostStore::open(":memory:").await.unwrap();
        assert_eq!(store_b.list_vault_contexts().await.unwrap().len(), 1);

        let newly_registered = store_b
            .import_encrypted_vault_directory(&default_vid, Some(&default_key), &envelope_json)
            .await
            .unwrap();
        assert_eq!(newly_registered, 2);

        let b_contexts = store_b.list_vault_contexts().await.unwrap();
        assert_eq!(b_contexts.len(), 3);
        assert!(b_contexts
            .iter()
            .any(|(name, ctx)| name == "Work Vault" && ctx.id().as_str() == "work-vault"));
        assert!(b_contexts
            .iter()
            .any(|(name, ctx)| name == "Corp Vault" && ctx.id().as_str() == "corp-vault"));
    }

    #[tokio::test]
    async fn test_encrypted_database_backend_enforces_dek() {
        use shellumi_core::{SshDestination, VaultKey};

        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("encrypted_vault.db");
        let db_str = db_path.to_str().unwrap();

        let key1 = VaultKey::generate().unwrap();
        let key2 = VaultKey::generate().unwrap();

        // 1. Create and populate database using key1
        {
            let mut store = HostStore::open_with_key(db_str, Some(&key1)).await.unwrap();
            let dest = SshDestination::new("alice", "10.0.0.1", 22).unwrap();
            store
                .add("prod-server", &dest, None, &[], None, None)
                .await
                .unwrap();
            let hosts = store.list().await.unwrap();
            assert_eq!(hosts.len(), 1);
            assert_eq!(hosts[0].name, "prod-server");
        }

        // 2. Opening with key1 succeeds and retrieves data
        {
            let store = HostStore::open_with_key(db_str, Some(&key1)).await.unwrap();
            let hosts = store.list().await.unwrap();
            assert_eq!(hosts.len(), 1);
            assert_eq!(hosts[0].name, "prod-server");
        }

        // 3. Opening with wrong key (key2) MUST fail
        {
            let result = HostStore::open_with_key(db_str, Some(&key2)).await;
            assert!(result.is_err(), "Opening with wrong key must fail");
        }

        // 4. Opening without key MUST fail
        {
            let result = HostStore::open_with_key(db_str, None).await;
            assert!(result.is_err(), "Opening without key must fail");
        }

        // 5. Inspect raw file bytes on disk: verify no plaintext and no standard SQLite format header
        let raw_bytes = std::fs::read(&db_path).expect("must read encrypted db bytes");
        assert!(!raw_bytes.is_empty(), "database file must not be empty");
        assert!(
            !raw_bytes.starts_with(b"SQLite format 3\0"),
            "Encrypted database must not start with standard SQLite format header"
        );
        let search_terms = [
            b"prod-server".as_slice(),
            b"alice".as_slice(),
            b"10.0.0.1".as_slice(),
        ];
        for term in &search_terms {
            assert!(
                !raw_bytes.windows(term.len()).any(|window| window == *term),
                "Raw disk bytes must never contain plaintext: {:?}",
                std::str::from_utf8(term)
            );
        }
        let wal_path = temp_dir.path().join("encrypted_vault.db-wal");
        if wal_path.exists() {
            let wal_bytes = std::fs::read(&wal_path).unwrap_or_default();
            for term in &search_terms {
                assert!(
                    !wal_bytes.windows(term.len()).any(|window| window == *term),
                    "Raw WAL bytes must never contain plaintext: {:?}",
                    std::str::from_utf8(term)
                );
            }
        }
    }

    #[tokio::test]
    async fn test_vault_security_lifecycle_and_envelope_manifest() {
        use shellumi_core::{
            validate_vault_password, SshDestination, VaultId, VaultKey, VaultKeyManager,
            VaultManifest,
        };

        // 1. Password policy verification
        assert!(validate_vault_password("short").is_err());
        assert!(validate_vault_password("nouppercase123!").is_err());
        assert!(validate_vault_password("NOLOWERCASE123!").is_err());
        assert!(validate_vault_password("NoDigitsHere!").is_err());
        assert!(validate_vault_password("NoSymbols1234").is_err());
        let valid_pwd = "Str0ng#VaultP@ss2026";
        assert!(validate_vault_password(valid_pwd).is_ok());

        // 2. Independent random 256-bit DEKs
        let vid = VaultId::new("vault-sec-test").unwrap();
        let key1 = VaultKey::generate().unwrap();
        let key2 = VaultKey::generate().unwrap();
        assert_ne!(key1.expose_bytes(), key2.expose_bytes());
        assert_eq!(key1.expose_bytes().len(), 32);

        // 3. Argon2id KEK derivation and envelope wrapping
        let cred_store = shellumi_core::EphemeralCredentialStore::new();
        let recovery_code = VaultKey::generate_recovery_code().unwrap();
        assert_eq!(recovery_code.chars().count(), 39); // 32 chars + 7 hyphens

        let (dek, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
            &cred_store,
            &vid,
            valid_pwd,
            false,
            Some(&recovery_code),
        )
        .unwrap();

        assert_eq!(primary_env.suite, "argon2id_aes256gcm_v2");
        assert_eq!(primary_env.argon2_m_cost, Some(64 * 1024));
        assert_eq!(primary_env.argon2_t_cost, Some(3));
        assert_eq!(primary_env.argon2_p_cost, Some(1));
        assert!(recovery_env.is_some());

        // 4. Portable manifest persistence
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("secure_vault.db");
        let manifest_path = VaultManifest::manifest_path_for_vault(&db_path, &vid);

        let mut manifest = VaultManifest::new(
            vid.as_str().to_string(),
            "Secure Vault".to_string(),
            primary_env,
            recovery_env,
            false,
        );
        manifest.save_to_path(&manifest_path).unwrap();
        assert!(manifest_path.exists());

        // 5. Open database with generated DEK and store records
        let db_str = db_path.to_str().unwrap();
        {
            let mut store = HostStore::open_with_key(db_str, Some(&dek)).await.unwrap();
            let context =
                shellumi_core::VaultContext::new(vid.clone(), shellumi_core::VaultLocation::Local);
            store
                .register_vault_context("Secure Vault", &context)
                .await
                .unwrap();
            store.set_vault_id(vid.clone());
            let dest = SshDestination::new("admin", "192.168.10.1", 22).unwrap();
            store
                .add("db-cluster", &dest, None, &[], None, None)
                .await
                .unwrap();
        }

        // 6. Unlock via manifest using password
        let loaded_manifest = VaultManifest::load_from_path(&manifest_path).unwrap();
        let unlocked_key = loaded_manifest.unlock(valid_pwd).unwrap();
        assert_eq!(unlocked_key.expose_bytes(), dek.expose_bytes());

        // Verify unlocked DEK can query the database
        {
            let mut store = HostStore::open_with_key(db_str, Some(&unlocked_key))
                .await
                .unwrap();
            store.set_vault_id(vid.clone());
            let hosts = store.list().await.unwrap();
            assert_eq!(hosts.len(), 1);
            assert_eq!(hosts[0].name, "db-cluster");
        }

        // 7. Reject wrong password
        assert!(loaded_manifest.unlock("WrongPassword123!").is_err());

        // 8. Unlock via recovery code
        let recovered_from_code = loaded_manifest.unlock(&recovery_code).unwrap();
        assert_eq!(recovered_from_code.expose_bytes(), dek.expose_bytes());

        // 9. Password change without re-encrypting the database
        let new_pwd = "N3w#StrongVaultPass2026!";
        manifest.change_password(&unlocked_key, new_pwd).unwrap();
        manifest.save_to_path(&manifest_path).unwrap();

        // New password unlocks the DEK
        let reloaded = VaultManifest::load_from_path(&manifest_path).unwrap();
        let new_unlocked = reloaded.unlock(new_pwd).unwrap();
        assert_eq!(new_unlocked.expose_bytes(), dek.expose_bytes());

        // Old password rejected
        assert!(reloaded.unlock(valid_pwd).is_err());

        // Database is still intact with unchanged DEK
        {
            let mut store = HostStore::open_with_key(db_str, Some(&new_unlocked))
                .await
                .unwrap();
            store.set_vault_id(vid.clone());
            let hosts = store.list().await.unwrap();
            assert_eq!(hosts.len(), 1);
            assert_eq!(hosts[0].name, "db-cluster");
        }

        // 10. Tampered envelope rejection
        let mut tampered_manifest = reloaded.clone();
        if let Some(first_byte) = tampered_manifest.primary_envelope.ciphertext.chars().next() {
            let replacement = if first_byte == 'A' { 'B' } else { 'A' };
            tampered_manifest
                .primary_envelope
                .ciphertext
                .replace_range(0..1, &replacement.to_string());
        }
        assert!(
            tampered_manifest.unlock(new_pwd).is_err(),
            "Tampered ciphertext must be rejected"
        );

        // 11. Cross-device sync export and recovery
        // When exported, the non-default vault attaches its password-wrapped DEK envelope
        {
            let mut store = HostStore::open_with_key(db_str, Some(&dek)).await.unwrap();
            store.set_vault_id(vid.clone());
            if let Ok(env_json) = serde_json::to_string(&manifest.primary_envelope) {
                store
                    .save_vault_recovery_envelope(vid.as_str(), &env_json)
                    .await
                    .unwrap();
            }

            let sync_vault = store
                .export_vault_with_iterations("sync-passphrase-12", "device-x", 1000)
                .await
                .unwrap();
            assert!(
                sync_vault.key_envelope.is_some(),
                "Opted-in vault must export key envelope"
            );

            // On another device:
            let remote_env = sync_vault.key_envelope.unwrap();
            let device2_key = VaultKey::unwrap_with_password(&vid, new_pwd, &remote_env).unwrap();
            assert_eq!(device2_key.expose_bytes(), dek.expose_bytes());
        }
    }

    #[tokio::test]
    async fn test_cross_vault_isolation_physical_files() {
        use shellumi_core::{SshDestination, VaultContext, VaultId, VaultKey, VaultLocation};

        let temp_dir = tempfile::tempdir().unwrap();
        let vaults_dir = temp_dir.path().join("vaults");
        std::fs::create_dir_all(&vaults_dir).unwrap();

        let default_db_path = vaults_dir.join("default.db");
        let sec_db_path = vaults_dir.join("sec-vault.db");

        let default_dek = VaultKey::generate().unwrap();
        let sec_dek = VaultKey::generate().unwrap();
        assert_ne!(default_dek.expose_bytes(), sec_dek.expose_bytes());

        let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
        let sec_vid = VaultId::new("sec-vault").unwrap();

        // 1. Initialize default vault
        {
            let mut def_store =
                HostStore::open_with_key(default_db_path.to_str().unwrap(), Some(&default_dek))
                    .await
                    .unwrap();
            def_store.set_vault_id(default_vid.clone());

            // Register secondary vault in default registry
            let sec_ctx = VaultContext::new(sec_vid.clone(), VaultLocation::Local);
            def_store
                .register_vault_context("Secondary Vault", &sec_ctx)
                .await
                .unwrap();

            // Add host to default vault
            let dest = SshDestination::new("root", "192.168.1.1", 22).unwrap();
            def_store
                .add("default-host", &dest, None, &[], None, None)
                .await
                .unwrap();
        }

        // 2. Initialize secondary vault with its own DEK in its own physical file
        {
            let mut sec_store =
                HostStore::open_with_key(sec_db_path.to_str().unwrap(), Some(&sec_dek))
                    .await
                    .unwrap();
            sec_store.set_vault_id(sec_vid.clone());

            let dest = SshDestination::new("developer", "10.10.10.10", 22).unwrap();
            sec_store
                .add("sec-host", &dest, None, &[], None, None)
                .await
                .unwrap();
        }

        // 3. Physical files exist separately on disk
        assert!(default_db_path.exists());
        assert!(sec_db_path.exists());

        // 4. Attempting to open secondary vault with default vault's DEK must FAIL
        assert!(
            HostStore::open_with_key(sec_db_path.to_str().unwrap(), Some(&default_dek))
                .await
                .is_err(),
            "Opening secondary vault with default DEK must fail"
        );

        // 5. Attempting to open default vault with secondary vault's DEK must FAIL
        assert!(
            HostStore::open_with_key(default_db_path.to_str().unwrap(), Some(&sec_dek))
                .await
                .is_err(),
            "Opening default vault with secondary DEK must fail"
        );

        // 6. Secondary vault contains ONLY its own records
        {
            let mut sec_store =
                HostStore::open_with_key(sec_db_path.to_str().unwrap(), Some(&sec_dek))
                    .await
                    .unwrap();
            sec_store.set_vault_id(sec_vid.clone());
            let hosts = sec_store.list().await.unwrap();
            assert_eq!(hosts.len(), 1);
            assert_eq!(hosts[0].name, "sec-host");

            // Secondary database has no knowledge of default-host
            assert!(!hosts.iter().any(|h| h.name == "default-host"));

            // Secondary database does not store the registry contexts of other vaults
            let contexts = sec_store.list_vault_contexts().await.unwrap();
            assert!(!contexts.iter().any(|(name, _)| name == "Secondary Vault"));
        }

        // 7. Default vault contains ONLY default records and the registry
        {
            let def_store =
                HostStore::open_with_key(default_db_path.to_str().unwrap(), Some(&default_dek))
                    .await
                    .unwrap();
            let hosts = def_store.list().await.unwrap();
            assert_eq!(hosts.len(), 1);
            assert_eq!(hosts[0].name, "default-host");
            assert!(!hosts.iter().any(|h| h.name == "sec-host"));

            let contexts = def_store.list_vault_contexts().await.unwrap();
            assert_eq!(contexts.len(), 2);
            assert!(contexts
                .iter()
                .any(|(name, ctx)| name == "Secondary Vault" && ctx.id().as_str() == "sec-vault"));
        }
    }

    #[tokio::test]
    async fn test_migration_engine_from_legacy_to_isolated_physical_vaults() {
        use crate::migration::MigrationEngine;
        use shellumi_core::{SshDestination, VaultContext, VaultId, VaultLocation};

        let temp_dir = tempfile::tempdir().unwrap();
        let base_dir = temp_dir.path();
        let legacy_db_path = base_dir.join("shellumi.db");

        let default_vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
        let corp_vid = VaultId::new("corp-prod").unwrap();

        // 1. Populate legacy database with default and secondary vault records
        {
            let mut store = HostStore::open(legacy_db_path.to_str().unwrap())
                .await
                .unwrap();

            // Default vault data
            store.set_vault_id(default_vid.clone());
            let def_dest = SshDestination::new("alice", "192.168.1.100", 22).unwrap();
            store
                .add("legacy-default-host", &def_dest, None, &[], None, None)
                .await
                .unwrap();

            // Register secondary vault context
            let corp_ctx = VaultContext::new(corp_vid.clone(), VaultLocation::Local);
            store
                .register_vault_context("Corporate Production", &corp_ctx)
                .await
                .unwrap();

            // Secondary vault data
            store.set_vault_id(corp_vid.clone());
            let corp_dest = SshDestination::new("bob", "10.0.0.50", 22).unwrap();
            store
                .add("legacy-corp-host", &corp_dest, None, &[], None, None)
                .await
                .unwrap();

            // Server connection with plaintext credentials
            let conn = crate::server_sync::VaultServerConnectionRecord {
                id: "srv-legacy-1".to_string(),
                base_url: "https://sync.shellumi.dev".to_string(),
                auth_token: Some("secret-raw-access-token-999".to_string()),
                refresh_token: Some("secret-raw-refresh-token-888".to_string()),
                device_id: Some("dev-legacy-1".to_string()),
                device_signing_key: Some("dGVzdC1zaWduaW5nLWtleQ==".to_string()),
                server_instance_id: Some("inst-1".to_string()),
                device_agreement_key: Some("dGVzdC1hZ3JlZW1lbnQta2V5".to_string()),
                created_at: 1000,
                updated_at: 1000,
            };
            store
                .connection
                .execute(
                    "INSERT INTO vault_server_connections (
                        id, base_url, auth_token, refresh_token, device_id, device_signing_key,
                        server_instance_id, device_agreement_key, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    (
                        conn.id.as_str(),
                        conn.base_url.as_str(),
                        conn.auth_token.as_deref().unwrap(),
                        conn.refresh_token.as_deref().unwrap(),
                        conn.device_id.as_deref().unwrap(),
                        conn.device_signing_key.as_deref().unwrap(),
                        conn.server_instance_id.as_deref().unwrap(),
                        conn.device_agreement_key.as_deref().unwrap(),
                        conn.created_at,
                        conn.updated_at,
                    ),
                )
                .await
                .unwrap();
        }

        // 2. Migration is needed
        assert!(MigrationEngine::is_migration_needed(base_dir));

        // 3. Run migration with explicit user credentials
        let cred_store = shellumi_core::EphemeralCredentialStore::new();
        let rec_code = shellumi_core::VaultKey::generate_recovery_code().unwrap();
        let mut sec_passwords = std::collections::HashMap::new();
        sec_passwords.insert("corp-prod".to_string(), "CorpProd#Secure2026!".to_string());
        let creds = MigrationCredentials {
            default_password: Some("M@sterVault2026!".to_string()),
            default_recovery_code: Some(rec_code),
            secondary_passwords: sec_passwords,
            secondary_recovery_codes: std::collections::HashMap::new(),
        };
        let report = MigrationEngine::migrate_legacy_database_with_credentials(
            base_dir,
            None,
            Some(&creds),
            &cred_store,
        )
        .await
        .unwrap();

        assert!(report.legacy_db_found);
        assert!(report.backup_path.is_some());
        assert!(report.backup_path.unwrap().exists());
        assert_eq!(report.vaults_migrated, 2);
        assert_eq!(report.server_credentials_migrated, 1);
        assert_eq!(report.default_records_migrated, 1);
        assert_eq!(report.secondary_records_migrated, 1);

        // 4. Legacy database remains available alongside the completed migration.
        assert!(legacy_db_path.exists());
        assert!(base_dir.join("vaults").join(".migration_complete").exists());

        // 5. Separate physical encrypted databases exist
        let default_db_path = base_dir.join("vaults").join("default.db");
        let corp_db_path = base_dir.join("vaults").join("corp-prod.db");
        assert!(default_db_path.exists());
        assert!(corp_db_path.exists());

        // Manifests exist
        let manifests_dir = base_dir.join("vault_manifests");
        assert!(manifests_dir
            .join(format!("{LEGACY_LOCAL_VAULT_ID}.manifest.json"))
            .exists());
        assert!(manifests_dir.join("corp-prod.manifest.json").exists());

        // 6. Raw bytes on disk are encrypted - no plaintext credentials or hosts
        let def_raw = std::fs::read(&default_db_path).unwrap();
        assert!(!def_raw.starts_with(b"SQLite format 3\0"));
        assert!(!def_raw.windows(19).any(|w| w == b"legacy-default-host"));
        assert!(!def_raw
            .windows(27)
            .any(|w| w == b"secret-raw-access-token-999"));

        let corp_raw = std::fs::read(&corp_db_path).unwrap();
        assert!(!corp_raw.starts_with(b"SQLite format 3\0"));
        assert!(!corp_raw.windows(16).any(|w| w == b"legacy-corp-host"));

        // 7. Verify records and credential references in default database
        let default_key = shellumi_core::VaultKeyManager::retrieve_key(&cred_store, &default_vid)
            .unwrap()
            .unwrap();
        let def_store =
            HostStore::open_with_key(default_db_path.to_str().unwrap(), Some(&default_key))
                .await
                .unwrap();

        let def_hosts = def_store.list().await.unwrap();
        assert_eq!(def_hosts.len(), 1);
        assert_eq!(def_hosts[0].name, "legacy-default-host");

        let conns = def_store.list_vault_server_connections().await.unwrap();
        assert_eq!(conns.len(), 1);
        let migrated_conn = &conns[0];
        assert!(migrated_conn
            .auth_token
            .as_ref()
            .unwrap()
            .starts_with("keychain://org.shellumi.server/"));
        assert!(migrated_conn
            .refresh_token
            .as_ref()
            .unwrap()
            .starts_with("keychain://org.shellumi.server/"));

        // Credential store resolves the actual secret
        let resolved_token = migrated_conn.resolve_auth_token(&cred_store).unwrap();
        assert_eq!(
            resolved_token.expose_secret(),
            "secret-raw-access-token-999"
        );

        // 8. Verify secondary vault records with its own key
        let corp_key = shellumi_core::VaultKeyManager::retrieve_key(&cred_store, &corp_vid)
            .unwrap()
            .unwrap();
        let mut corp_store =
            HostStore::open_with_key(corp_db_path.to_str().unwrap(), Some(&corp_key))
                .await
                .unwrap();
        corp_store.set_vault_id(corp_vid.clone());

        let corp_hosts = corp_store.list().await.unwrap();
        assert_eq!(corp_hosts.len(), 1);
        assert_eq!(corp_hosts[0].name, "legacy-corp-host");

        // 9. Cross-vault key isolation
        assert!(
            HostStore::open_with_key(corp_db_path.to_str().unwrap(), Some(&default_key))
                .await
                .is_err()
        );

        // 10. Migration is now idempotent / no-op
        assert!(!MigrationEngine::is_migration_needed(base_dir));
        let second_run =
            MigrationEngine::migrate_legacy_database_if_needed(base_dir, None, &cred_store)
                .await
                .unwrap();
        assert_eq!(second_run.vaults_migrated, 0);
    }

    #[tokio::test]
    async fn test_interrupted_migration_safety_and_verification() {
        let temp = tempfile::tempdir().unwrap();
        let base_dir = temp.path();
        let legacy_db_path = base_dir.join("shellumi.db");

        // 1. Setup legacy database with records
        {
            let mut store = HostStore::open(legacy_db_path.to_str().unwrap())
                .await
                .unwrap();
            let dest = SshDestination::new("192.168.1.10", "alice", 22).unwrap();
            store
                .add("interrupted-test-host", &dest, None, &[], None, None)
                .await
                .unwrap();
            let snip = Snippet {
                name: "test-snippet".into(),
                command: "uptime".into(),
                description: None,
                tags: vec![],
                variables: vec![],
            };
            store.add_snippet(&snip).await.unwrap();
        }

        // 2. Simulate an interrupted migration: vaults/default.db exists but NO .migration_complete marker
        let vaults_dir = base_dir.join("vaults");
        std::fs::create_dir_all(&vaults_dir).unwrap();
        let partial_db_path = vaults_dir.join("default.db");
        std::fs::write(&partial_db_path, b"corrupted partial sqlite file").unwrap();
        let preexisting_file = vaults_dir.join("unrelated_persisted.txt");
        std::fs::write(&preexisting_file, b"should not be deleted").unwrap();

        // Verify that is_migration_needed is TRUE despite default.db existing (does not treat default.db existence as completion!)
        assert!(MigrationEngine::is_migration_needed(base_dir));

        // 3. Run migration with explicit user credentials
        let cred_store = shellumi_core::EphemeralCredentialStore::new();
        let rec_code = shellumi_core::VaultKey::generate_recovery_code().unwrap();
        let creds = MigrationCredentials {
            default_password: Some("IntErrUptedP@ss123!".to_string()),
            default_recovery_code: Some(rec_code),
            secondary_passwords: std::collections::HashMap::new(),
            secondary_recovery_codes: std::collections::HashMap::new(),
        };

        let result = MigrationEngine::migrate_legacy_database_with_credentials(
            base_dir,
            None,
            Some(&creds),
            &cred_store,
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            std::fs::read(&partial_db_path).unwrap(),
            b"corrupted partial sqlite file"
        );
        assert_eq!(
            std::fs::read(&preexisting_file).unwrap(),
            b"should not be deleted"
        );
        assert!(legacy_db_path.exists());
        assert!(!vaults_dir.join(".migration_complete").exists());
        assert!(vaults_dir.join(".migration_staging").exists());
    }

    #[tokio::test]
    async fn test_second_device_restoration_from_sync_payload() {
        let temp_dev1 = tempfile::tempdir().unwrap();
        let dev1_db_path = temp_dev1.path().join("vaults").join("vault_sync.db");
        std::fs::create_dir_all(dev1_db_path.parent().unwrap()).unwrap();

        let vault_id = shellumi_core::VaultId::new("vlt-finance-001").unwrap();
        let user_password = "Str0ngVaultPassword!#2026";

        // ===================================================================
        // Device 1: Create vault, add records, and export SyncVault with key_envelope
        // ===================================================================
        let dev1_cred_store = shellumi_core::EphemeralCredentialStore::new();
        let rec_code = shellumi_core::VaultKey::generate_recovery_code().unwrap();
        let (dek, primary_env, _rec_env) = shellumi_core::VaultKeyManager::provision_new_vault(
            &dev1_cred_store,
            &vault_id,
            user_password,
            true,
            Some(&rec_code),
        )
        .unwrap();

        let mut dev1_store = HostStore::open_with_key(dev1_db_path.to_str().unwrap(), Some(&dek))
            .await
            .unwrap();
        dev1_store.set_vault_id(vault_id.clone());

        // Add records
        let dest = SshDestination::new("10.0.0.50", "admin", 22).unwrap();
        dev1_store
            .add(
                "fin-prod-server",
                &dest,
                None,
                &["pci-dss".to_string()],
                None,
                None,
            )
            .await
            .unwrap();
        let snip = Snippet {
            name: "check-firewall".into(),
            command: "iptables -L -n -v".into(),
            description: Some("Security audit command".into()),
            tags: vec!["audit".into()],
            variables: vec![],
        };
        dev1_store.add_snippet(&snip).await.unwrap();

        let dek_passphrase = hex::encode(dek.expose_bytes());
        let mut sync_vault = dev1_store
            .export_vault(&dek_passphrase, "device-1-uuid")
            .await
            .unwrap();
        sync_vault.key_envelope = Some(primary_env.clone());

        // Verify sync_vault payload contents
        assert!(sync_vault.key_envelope.is_some());
        assert_eq!(sync_vault.records.len(), 2);

        // ===================================================================
        // Device 2: Completely fresh device (no Keychain, no env vars)
        // Restores vault using ONLY user_password and sync_vault payload
        // ===================================================================
        let temp_dev2 = tempfile::tempdir().unwrap();
        let dev2_db_path = temp_dev2.path().join("vaults").join("vault_sync.db");
        let dev2_manifests_dir = temp_dev2.path().join("vault_manifests");
        let dev2_cred_store = shellumi_core::EphemeralCredentialStore::new();

        // Attempt restoration with wrong password must fail
        let wrong_opts = RestoreVaultOptions {
            vault_id: &vault_id,
            vault_name: "Finance Vault",
            password: "WrongP@ssword123!",
            target_db_path: &dev2_db_path,
            manifests_dir: &dev2_manifests_dir,
            cred_store: Some(&dev2_cred_store),
            remember_in_keychain: true,
        };
        assert!(HostStore::restore_vault_from_sync(&sync_vault, &wrong_opts)
            .await
            .is_err());

        // Restoration with correct user password succeeds
        let correct_opts = RestoreVaultOptions {
            vault_id: &vault_id,
            vault_name: "Finance Vault",
            password: user_password,
            target_db_path: &dev2_db_path,
            manifests_dir: &dev2_manifests_dir,
            cred_store: Some(&dev2_cred_store),
            remember_in_keychain: true,
        };
        let dev2_store = HostStore::restore_vault_from_sync(&sync_vault, &correct_opts)
            .await
            .unwrap();

        // 1. Verify records on Device 2
        let restored_hosts = dev2_store.list().await.unwrap();
        assert_eq!(restored_hosts.len(), 1);
        assert_eq!(restored_hosts[0].name, "fin-prod-server");
        assert_eq!(restored_hosts[0].destination.host(), "10.0.0.50");

        let restored_snippets = dev2_store.list_snippets().await.unwrap();
        assert_eq!(restored_snippets.len(), 1);
        assert_eq!(restored_snippets[0].name, "check-firewall");

        // 2. Verify portable manifest was written on Device 2
        let dev2_manifest_file =
            dev2_manifests_dir.join(format!("{}.manifest.json", vault_id.as_str()));
        assert!(dev2_manifest_file.exists());
        let manifest = shellumi_core::VaultManifest::load_from_path(&dev2_manifest_file).unwrap();
        assert_eq!(manifest.name, "Finance Vault");

        // 3. Verify recovered DEK was provisioned into Device 2's local credential store
        let dev2_key = shellumi_core::VaultKeyManager::retrieve_key(&dev2_cred_store, &vault_id)
            .unwrap()
            .unwrap();
        assert_eq!(dev2_key.expose_bytes(), dek.expose_bytes());

        // 4. Verify raw bytes on Device 2 disk are encrypted (no plaintext host names or IP)
        let raw_disk = std::fs::read(&dev2_db_path).unwrap();
        assert!(!raw_disk.starts_with(b"SQLite format 3\0"));
        assert!(!raw_disk.windows(15).any(|w| w == b"fin-prod-server"));
        assert!(!raw_disk.windows(9).any(|w| w == b"10.0.0.50"));
    }

    #[tokio::test]
    async fn test_corrupted_envelope_and_mismatched_vault_rejected_on_restore() {
        let temp = tempfile::tempdir().unwrap();
        let target_db = temp.path().join("vaults").join("bad_env.db");
        let manifests_dir = temp.path().join("vault_manifests");
        let cred_store = shellumi_core::EphemeralCredentialStore::new();

        let vid = shellumi_core::VaultId::new("vlt-corrupt-001").unwrap();
        let key = shellumi_core::VaultKey::generate().unwrap();
        let env = key
            .wrap_with_password(&vid, "CorrectPassword123!#")
            .unwrap();

        // 1. Corrupt the envelope ciphertext
        let mut corrupted_env = env.clone();
        corrupted_env.ciphertext = "AAAACorruptedCiphertextTag1234567890==".to_string();

        let mut sync_vault = shellumi_core::SyncVault::new(vid.as_str(), 1);
        sync_vault.key_envelope = Some(corrupted_env);

        let opts = RestoreVaultOptions {
            vault_id: &vid,
            vault_name: "Corrupt Vault",
            password: "CorrectPassword123!#",
            target_db_path: &target_db,
            manifests_dir: &manifests_dir,
            cred_store: Some(&cred_store),
            remember_in_keychain: false,
        };

        // Restoring with corrupted ciphertext MUST fail
        let res = HostStore::restore_vault_from_sync(&sync_vault, &opts).await;
        assert!(res.is_err());
        assert!(
            !target_db.exists(),
            "Target database should not exist after failed restore"
        );

        // 2. Mismatched vault ID (AAD binding violation)
        let other_vid = shellumi_core::VaultId::new("vlt-other-999").unwrap();
        let other_opts = RestoreVaultOptions {
            vault_id: &other_vid,
            vault_name: "Other Vault",
            password: "CorrectPassword123!#",
            target_db_path: &target_db,
            manifests_dir: &manifests_dir,
            cred_store: Some(&cred_store),
            remember_in_keychain: false,
        };
        let mut mismatch_sync_vault = shellumi_core::SyncVault::new(other_vid.as_str(), 1);
        mismatch_sync_vault.key_envelope = Some(env); // env was encrypted with vid, not other_vid!

        let mismatch_res =
            HostStore::restore_vault_from_sync(&mismatch_sync_vault, &other_opts).await;
        assert!(mismatch_res.is_err());
        assert!(!target_db.exists());
    }

    #[tokio::test]
    async fn test_second_device_default_and_secondary_restoration_with_registry() {
        let temp_dev1 = tempfile::tempdir().unwrap();
        let dev1_base = temp_dev1.path();
        let dev1_vaults = dev1_base.join("vaults");
        let dev1_manifests = dev1_base.join("vault_manifests");
        std::fs::create_dir_all(&dev1_vaults).unwrap();
        std::fs::create_dir_all(&dev1_manifests).unwrap();

        let dev1_cred_store = shellumi_core::EphemeralCredentialStore::new();
        let default_vid = shellumi_core::VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
        let secondary_vid = shellumi_core::VaultId::new("vlt-sec-opted-in").unwrap();
        let unshared_vid = shellumi_core::VaultId::new("vlt-unshared-local").unwrap();

        let default_pwd = "DefaultMasterPassword#2026!";
        let secondary_pwd = "SecondaryCustomPassword#2026!";

        // Provision default vault
        let (def_dek, def_env, _) = shellumi_core::VaultKeyManager::provision_new_vault(
            &dev1_cred_store,
            &default_vid,
            default_pwd,
            true,
            None,
        )
        .unwrap();
        let def_manifest = shellumi_core::VaultManifest::new(
            default_vid.as_str().to_string(),
            "Default Vault".into(),
            def_env.clone(),
            None,
            true,
        );
        def_manifest
            .save_to_path(&dev1_manifests.join(format!("{}.manifest.json", default_vid.as_str())))
            .unwrap();

        let def_db_path = dev1_vaults.join("default.db");
        let mut def_store = HostStore::open_with_key(def_db_path.to_str().unwrap(), Some(&def_dek))
            .await
            .unwrap();

        // Provision secondary opted-in vault
        let (sec_dek, sec_env, _) = shellumi_core::VaultKeyManager::provision_new_vault(
            &dev1_cred_store,
            &secondary_vid,
            secondary_pwd,
            true,
            None,
        )
        .unwrap();
        let sec_manifest = shellumi_core::VaultManifest::new(
            secondary_vid.as_str().to_string(),
            "Sec Opted In".into(),
            sec_env.clone(),
            None,
            true,
        );
        sec_manifest
            .save_to_path(&dev1_manifests.join(format!("{}.manifest.json", secondary_vid.as_str())))
            .unwrap();

        let sec_db_path = dev1_vaults.join(format!("{}.db", secondary_vid.as_str()));
        let mut sec_store = HostStore::open_with_key(sec_db_path.to_str().unwrap(), Some(&sec_dek))
            .await
            .unwrap();
        sec_store.set_vault_id(secondary_vid.clone());
        let sec_dest = SshDestination::new("192.168.10.5", "ops", 22).unwrap();
        sec_store
            .add("opted-in-host", &sec_dest, None, &[], None, None)
            .await
            .unwrap();

        // Register all vaults in default.db directory
        let mut dir = shellumi_core::VaultDirectory::new();
        dir.upsert(shellumi_core::VaultDirectoryEntry {
            id: default_vid.as_str().to_string(),
            name: "Default Vault".to_string(),
            location: shellumi_core::VaultDirectoryLocation::Local,
            has_recovery_envelope: false,
            sync_enabled: true,
            created_at: 1000,
            updated_at: 1000,
        });
        dir.upsert(shellumi_core::VaultDirectoryEntry {
            id: secondary_vid.as_str().to_string(),
            name: "Sec Opted In".to_string(),
            location: shellumi_core::VaultDirectoryLocation::Local,
            has_recovery_envelope: false,
            sync_enabled: true,
            created_at: 1000,
            updated_at: 1000,
        });
        dir.upsert(shellumi_core::VaultDirectoryEntry {
            id: unshared_vid.as_str().to_string(),
            name: "Unshared Local".to_string(),
            location: shellumi_core::VaultDirectoryLocation::Local,
            has_recovery_envelope: false,
            sync_enabled: false,
            created_at: 1000,
            updated_at: 1000,
        });

        let enc_env = dir.encrypt(&default_vid, &def_dek).unwrap();
        let envelope_json = serde_json::to_string(&enc_env).unwrap();
        def_store
            .import_encrypted_vault_directory(&default_vid, Some(&def_dek), &envelope_json)
            .await
            .unwrap();

        // Export snapshots for cloud
        let def_passphrase = hex::encode(def_dek.expose_bytes());
        let mut def_sync = def_store
            .export_vault(&def_passphrase, "dev-1")
            .await
            .unwrap();
        def_sync.key_envelope = Some(def_env);

        let sec_passphrase = hex::encode(sec_dek.expose_bytes());
        let mut sec_sync = sec_store
            .export_vault(&sec_passphrase, "dev-1")
            .await
            .unwrap();
        sec_sync.key_envelope = Some(sec_env);

        // ===================================================================
        // Device 2: Restore from cloud payloads
        // ===================================================================
        let temp_dev2 = tempfile::tempdir().unwrap();
        let dev2_base = temp_dev2.path();
        let dev2_vaults = dev2_base.join("vaults");
        let dev2_manifests = dev2_base.join("vault_manifests");
        let dev2_cred_store = shellumi_core::EphemeralCredentialStore::new();

        // 1. Restore default vault and registry
        let def_opts = RestoreVaultOptions {
            vault_id: &default_vid,
            vault_name: "Default Vault",
            password: default_pwd,
            target_db_path: &dev2_vaults.join("default.db"),
            manifests_dir: &dev2_manifests,
            cred_store: Some(&dev2_cred_store),
            remember_in_keychain: false,
        };
        let dev2_def_store = HostStore::restore_vault_from_sync(&def_sync, &def_opts)
            .await
            .unwrap();

        // Verify default directory on Device 2 has all 3 vaults
        let restored_dir = dev2_def_store.build_vault_directory().await.unwrap();
        assert_eq!(restored_dir.entries.len(), 3);
        assert!(restored_dir.get(default_vid.as_str()).unwrap().sync_enabled);
        assert!(
            restored_dir
                .get(secondary_vid.as_str())
                .unwrap()
                .sync_enabled
        );
        assert!(
            !restored_dir
                .get(unshared_vid.as_str())
                .unwrap()
                .sync_enabled
        );

        // 2. Restore secondary opted-in vault independently with its own password
        let sec_opts = RestoreVaultOptions {
            vault_id: &secondary_vid,
            vault_name: "Sec Opted In",
            password: secondary_pwd,
            target_db_path: &dev2_vaults.join(format!("{}.db", secondary_vid.as_str())),
            manifests_dir: &dev2_manifests,
            cred_store: Some(&dev2_cred_store),
            remember_in_keychain: false,
        };
        let dev2_sec_store = HostStore::restore_vault_from_sync(&sec_sync, &sec_opts)
            .await
            .unwrap();

        let sec_hosts = dev2_sec_store.list().await.unwrap();
        assert_eq!(sec_hosts.len(), 1);
        assert_eq!(sec_hosts[0].name, "opted-in-host");

        // 3. Verify unsynchronized vault does not have a local database on Device 2
        let unshared_db = dev2_vaults.join(format!("{}.db", unshared_vid.as_str()));
        assert!(
            !unshared_db.exists(),
            "Unshared local vault should not be restored"
        );
    }

    #[tokio::test]
    async fn test_migration_failure_prevents_startup_without_modifying_user_data() {
        let temp = tempfile::tempdir().unwrap();
        let base_dir = temp.path();
        let legacy_db_path = base_dir.join("shellumi.db");

        // 1. Setup legacy database with a secondary vault
        {
            let mut store = HostStore::open(legacy_db_path.to_str().unwrap())
                .await
                .unwrap();
            let dest = SshDestination::new("192.168.1.10", "alice", 22).unwrap();
            store
                .add("production-box", &dest, None, &[], None, None)
                .await
                .unwrap();
            let corp_vid = shellumi_core::VaultId::new("corp-ops").unwrap();
            let corp_ctx = shellumi_core::VaultContext::new(
                corp_vid.clone(),
                shellumi_core::VaultLocation::Local,
            );
            store
                .register_vault_context("Corp Ops", &corp_ctx)
                .await
                .unwrap();
        }

        let original_bytes = std::fs::read(&legacy_db_path).unwrap();

        // 2. Attempt migration without providing the secondary vault's password
        let cred_store = shellumi_core::EphemeralCredentialStore::new();
        let creds = MigrationCredentials {
            default_password: Some("ValidMasterPass#123".to_string()),
            default_recovery_code: None,
            secondary_passwords: std::collections::HashMap::new(), // Missing corp-ops password!
            secondary_recovery_codes: std::collections::HashMap::new(),
        };

        let result = MigrationEngine::migrate_legacy_database_with_credentials(
            base_dir,
            None,
            Some(&creds),
            &cred_store,
        )
        .await;

        // Must fail with MigrationRequired error
        assert!(result.is_err());
        let err_str = result.err().unwrap().to_string();
        assert!(err_str.contains("MigrationRequired"));
        assert!(err_str.contains("corp-ops"));

        // 3. User data must NOT be modified
        assert!(legacy_db_path.exists());
        let current_bytes = std::fs::read(&legacy_db_path).unwrap();
        assert_eq!(current_bytes, original_bytes);

        // No completion marker and no .migrated file
        assert!(!base_dir.join("shellumi.db.migrated").exists());
        assert!(!base_dir.join("vaults").join(".migration_complete").exists());

        // Preserve staging for recovery and diagnosis.
        assert!(base_dir.join("vaults").join(".migration_staging").exists());

        // is_migration_needed is still true, halting startup safely
        assert!(MigrationEngine::is_migration_needed(base_dir));
    }

    #[tokio::test]
    async fn migration_preserves_record_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shellumi.db");
        let store = HostStore::open(path.to_str().unwrap()).await.unwrap();
        store.connection.execute(
            "INSERT INTO hosts(id, name, hostname, username, port) VALUES (42, 'server', 'example.org', 'user', 22)",
            (),
        ).await.unwrap();
        drop(store);
        let credentials = MigrationCredentials {
            default_password: Some("StrongPassword123!".into()),
            ..Default::default()
        };
        let secrets = shellumi_core::EphemeralCredentialStore::new();
        let result = MigrationEngine::migrate_legacy_database_with_credentials(
            dir.path(),
            None,
            Some(&credentials),
            &secrets,
        )
        .await;
        result.unwrap();
        assert!(path.exists());
        assert!(dir.path().join("vaults/.migration_complete").exists());
        let source = HostStore::open(path.to_str().unwrap()).await.unwrap();
        assert_eq!(source.list().await.unwrap()[0].id, 42);
        let vid = shellumi_core::VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
        let key = shellumi_core::VaultKeyManager::retrieve_key(&secrets, &vid)
            .unwrap()
            .unwrap();
        let target = HostStore::open_with_key(
            dir.path().join("vaults/default.db").to_str().unwrap(),
            Some(&key),
        )
        .await
        .unwrap();
        assert_eq!(target.list().await.unwrap()[0].id, 42);
    }
}

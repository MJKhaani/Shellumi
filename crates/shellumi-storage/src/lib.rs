//! Local Turso storage. Credentials are never stored in this database.
use shellumi_core::{
    resolve_effective_destination, resolve_effective_tags, validate_jump_host, DestinationError,
    ForwardingType, GroupError, HostGroup, PortForwardRule, Snippet, SnippetError, SnippetVariable,
    SshDestination, TransferDirection, TunnelError,
};
use std::fmt;
use turso::{transaction::TransactionBehavior, Builder, Connection, Database};

const SCHEMA_VERSION: i64 = 8;

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
    InvalidSync(shellumi_core::SyncError),
    SnapshotNotFound(String),
    MissingSchemaVersion,
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
            Self::InvalidSync(error) => write!(f, "sync error: {error}"),
            Self::SnapshotNotFound(id) => write!(f, "sync snapshot not found: {id}"),
            Self::MissingSchemaVersion => f.write_str("database did not return a schema version"),
        }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedHost {
    pub id: i64,
    pub name: String,
    pub destination: SshDestination,
    pub group_name: Option<String>,
    pub tags: Vec<String>,
    pub jump_host: Option<String>,
    pub startup_snippet_id: Option<i64>,
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
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SyncGroupPayload {
    pub name: String,
    pub default_username: Option<String>,
    pub default_port: Option<u16>,
    pub tags: Vec<String>,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportVaultSummary {
    pub hosts_imported: usize,
    pub groups_imported: usize,
    pub tunnels_imported: usize,
    pub snippets_imported: usize,
    pub errors: Vec<String>,
}

pub struct HostStore {
    _database: Database,
    connection: Connection,
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

impl HostStore {
    /// Open a local database path, or `:memory:` for an ephemeral database.
    pub async fn open(path: &str) -> Result<Self, StorageError> {
        let database = Builder::new_local(path).build().await?;
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
            return Err(StorageError::UnsupportedSchema(version));
        }
        if version == 0 {
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
                "CREATE TABLE hosts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    hostname TEXT NOT NULL CHECK(length(hostname) > 0),
                    username TEXT NOT NULL CHECK(length(username) > 0),
                    port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
                    group_name TEXT,
                    tags TEXT NOT NULL DEFAULT '',
                    jump_host TEXT,
                    startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL
                )",
                (),
            )
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
            tx.execute("PRAGMA user_version = 8", ()).await?;
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
            tx.execute("PRAGMA user_version = 8", ()).await?;
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
            tx.execute("PRAGMA user_version = 8", ()).await?;
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
            tx.execute("PRAGMA user_version = 8", ()).await?;
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
            tx.execute("PRAGMA user_version = 8", ()).await?;
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
            tx.execute("PRAGMA user_version = 8", ()).await?;
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
            tx.execute("PRAGMA user_version = 8", ()).await?;
        } else if version == 7 {
            // Migrate schema version 7 to 8
            tx.execute(
                "ALTER TABLE hosts ADD COLUMN startup_snippet_id INTEGER REFERENCES snippets(id) ON DELETE SET NULL",
                (),
            )
            .await?;
            tx.execute("PRAGMA user_version = 8", ()).await?;
        }
        tx.commit().await?;
        Ok(Self {
            _database: database,
            connection,
        })
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
        let name = validated_name(name)?;
        let group = validated_group(group_name)?;
        let tags_serialized = validated_tags(tags)?;
        let jump = validated_jump_host(jump_host)?;

        self.connection
            .execute(
                "INSERT INTO hosts (name, hostname, username, port, group_name, tags, jump_host, startup_snippet_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                (
                    name,
                    destination.host(),
                    destination.username(),
                    i64::from(destination.port().get()),
                    group,
                    tags_serialized,
                    jump,
                    startup_snippet_id,
                ),
            )
            .await?;
        Ok(self.connection.last_insert_rowid())
    }

    /// Update an existing host, returning false if its ID is not present.
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
        let name = validated_name(name)?;
        let group = validated_group(group_name)?;
        let tags_serialized = validated_tags(tags)?;
        let jump = validated_jump_host(jump_host)?;

        Ok(self
            .connection
            .execute(
                "UPDATE hosts SET name = ?1, hostname = ?2, username = ?3, port = ?4, group_name = ?5, tags = ?6, jump_host = ?7, startup_snippet_id = ?8 WHERE id = ?9",
                (
                    name,
                    destination.host(),
                    destination.username(),
                    i64::from(destination.port().get()),
                    group,
                    tags_serialized,
                    jump,
                    startup_snippet_id,
                    id,
                ),
            )
            .await?
            != 0)
    }

    pub async fn list(&self) -> Result<Vec<SavedHost>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, hostname, username, port, group_name, tags, jump_host, startup_snippet_id FROM hosts ORDER BY id",
                (),
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
            });
        }
        Ok(hosts)
    }

    pub async fn get_host(&self, id: i64) -> Result<Option<SavedHost>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, hostname, username, port, group_name, tags, jump_host, startup_snippet_id FROM hosts WHERE id = ?1",
                (id,),
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
                "INSERT INTO groups (name, default_username, default_port, tags) VALUES (?1, ?2, ?3, ?4)",
                (
                    group.name.as_str(),
                    group.default_username.as_deref(),
                    default_port,
                    tags_serialized,
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
                "UPDATE groups SET default_username = ?1, default_port = ?2, tags = ?3 WHERE name = ?4",
                (
                    group.default_username.as_deref(),
                    default_port,
                    tags_serialized,
                    group.name.as_str(),
                ),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn get_group(&self, name: &str) -> Result<Option<HostGroup>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT name, default_username, default_port, tags FROM groups WHERE name = ?1",
                (name.trim(),),
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            let default_username: Option<String> = row.get(1)?;
            let default_port: Option<i64> = row.get(2)?;
            let tags_str: String = row.get(3)?;
            let tags = parse_tags(&tags_str);
            let port = match default_port {
                Some(p) => {
                    let u = u16::try_from(p).map_err(|_| StorageError::InvalidPort(p))?;
                    Some(u)
                }
                None => None,
            };
            let group = HostGroup::new(&name, default_username.as_deref(), port, &tags)
                .map_err(StorageError::InvalidGroup)?;
            Ok(Some(group))
        } else {
            Ok(None)
        }
    }

    pub async fn list_groups(&self) -> Result<Vec<HostGroup>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT name, default_username, default_port, tags FROM groups ORDER BY name",
                (),
            )
            .await?;
        let mut groups = Vec::new();
        while let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            let default_username: Option<String> = row.get(1)?;
            let default_port: Option<i64> = row.get(2)?;
            let tags_str: String = row.get(3)?;
            let tags = parse_tags(&tags_str);
            let port = match default_port {
                Some(p) => {
                    let u = u16::try_from(p).map_err(|_| StorageError::InvalidPort(p))?;
                    Some(u)
                }
                None => None,
            };
            let group = HostGroup::new(&name, default_username.as_deref(), port, &tags)
                .map_err(StorageError::InvalidGroup)?;
            groups.push(group);
        }
        Ok(groups)
    }

    pub async fn delete_group(&mut self, name: &str) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute("DELETE FROM groups WHERE name = ?1", (name.trim(),))
            .await?;
        Ok(rows != 0)
    }

    pub async fn get_effective_destination(
        &self,
        host: &SavedHost,
    ) -> Result<SshDestination, StorageError> {
        if let Some(group_name) = &host.group_name {
            if let Some(group) = self.get_group(group_name).await? {
                return resolve_effective_destination(
                    host.destination.host(),
                    Some(host.destination.username()),
                    Some(host.destination.port().get()),
                    Some(&group),
                )
                .map_err(StorageError::InvalidDestination);
            }
        }
        Ok(host.destination.clone())
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
            .execute("DELETE FROM hosts WHERE id = ?1", (id,))
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
                "INSERT INTO tunnels (host_id, name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                (
                    host_id,
                    name,
                    rule.forwarding_type.as_str(),
                    rule.bind_address.as_deref(),
                    i64::from(rule.bind_port),
                    rule.target_host.as_deref(),
                    target_port,
                    if auto_start { 1i64 } else { 0i64 },
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
                "UPDATE tunnels SET host_id = ?1, name = ?2, forwarding_type = ?3, bind_address = ?4, bind_port = ?5, target_host = ?6, target_port = ?7, auto_start = ?8 WHERE id = ?9",
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
                ),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn list_tunnels(&self) -> Result<Vec<SavedTunnel>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, host_id, name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start FROM tunnels ORDER BY id",
                (),
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
                "SELECT id, host_id, name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start FROM tunnels WHERE host_id = ?1 ORDER BY id",
                (host_id,),
            )
            .await?;
        Self::parse_tunnel_rows(&mut rows).await
    }

    pub async fn delete_tunnel(&mut self, id: i64) -> Result<bool, StorageError> {
        let rows = self
            .connection
            .execute("DELETE FROM tunnels WHERE id = ?1", (id,))
            .await?;
        Ok(rows != 0)
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
                "INSERT INTO snippets (name, command, description, tags, variables) VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    name,
                    command,
                    snippet.description.as_deref(),
                    tags_str.as_str(),
                    vars_json.as_str(),
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
                "UPDATE snippets SET name = ?1, command = ?2, description = ?3, tags = ?4, variables = ?5, updated_at = datetime('now') WHERE id = ?6",
                (
                    name,
                    command,
                    snippet.description.as_deref(),
                    tags_str.as_str(),
                    vars_json.as_str(),
                    id,
                ),
            )
            .await?;
        Ok(rows != 0)
    }

    pub async fn delete_snippet(&mut self, id: i64) -> Result<bool, StorageError> {
        self.connection
            .execute(
                "UPDATE hosts SET startup_snippet_id = NULL WHERE startup_snippet_id = ?1",
                (id,),
            )
            .await?;
        let rows = self
            .connection
            .execute("DELETE FROM snippets WHERE id = ?1", (id,))
            .await?;
        Ok(rows != 0)
    }

    pub async fn get_snippet(&self, id: i64) -> Result<Option<SavedSnippet>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, command, description, tags, variables, created_at, updated_at FROM snippets WHERE id = ?1",
                (id,),
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

    pub async fn list_snippets(&self) -> Result<Vec<SavedSnippet>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, command, description, tags, variables, created_at, updated_at FROM snippets ORDER BY name ASC",
                (),
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
        let mut vault = shellumi_core::SyncVault::new(format!("vault-{device_id}-{now}"), now);

        // Export groups
        let groups = self.list_groups().await?;
        for group in groups {
            let payload = SyncGroupPayload {
                name: group.name.clone(),
                default_username: group.default_username.clone(),
                default_port: group.default_port.map(|p| p.get()),
                tags: group.tags.clone(),
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
                                        .update(
                                            m.id,
                                            &payload.name,
                                            &dest,
                                            payload.group_name.as_deref(),
                                            &payload.tags,
                                            payload.jump_host.as_deref(),
                                            payload.startup_snippet_id,
                                        )
                                        .await;
                                } else {
                                    let _ = self
                                        .add(
                                            &payload.name,
                                            &dest,
                                            payload.group_name.as_deref(),
                                            &payload.tags,
                                            payload.jump_host.as_deref(),
                                            payload.startup_snippet_id,
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
            }
        }

        let _ = self.archive_snapshot(&vault.vault_id, vault, 3).await;
        Ok(summary)
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn migrates_from_schema_versions_1_to_7_to_version_8() {
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
}

//! Local Turso storage. Credentials are never stored in this database.
use shellumi_core::{DestinationError, SshDestination};
use std::fmt;
use turso::{transaction::TransactionBehavior, Builder, Connection, Database};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug)]
pub enum StorageError {
    Database(turso::Error),
    UnsupportedSchema(i64),
    InvalidName,
    InvalidDestination(DestinationError),
    InvalidPort(i64),
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
            Self::InvalidDestination(error) => write!(f, "invalid stored destination: {error}"),
            Self::InvalidPort(port) => write!(f, "invalid stored port: {port}"),
            Self::MissingSchemaVersion => f.write_str("database did not return a schema version"),
        }
    }
}
impl std::error::Error for StorageError {}
impl From<turso::Error> for StorageError {
    fn from(error: turso::Error) -> Self {
        Self::Database(error)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SavedHost {
    pub id: i64,
    pub name: String,
    pub destination: SshDestination,
}
pub struct HostStore {
    _database: Database,
    connection: Connection,
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
            tx.execute("CREATE TABLE hosts (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL CHECK(length(name) > 0), hostname TEXT NOT NULL CHECK(length(hostname) > 0), username TEXT NOT NULL CHECK(length(username) > 0), port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535))", ()).await?;
            tx.execute("PRAGMA user_version = 1", ()).await?;
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
    ) -> Result<i64, StorageError> {
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(StorageError::InvalidName);
        }
        self.connection
            .execute(
                "INSERT INTO hosts (name, hostname, username, port) VALUES (?1, ?2, ?3, ?4)",
                (
                    name,
                    destination.host(),
                    destination.username(),
                    i64::from(destination.port().get()),
                ),
            )
            .await?;
        Ok(self.connection.last_insert_rowid())
    }
    pub async fn list(&self) -> Result<Vec<SavedHost>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, hostname, username, port FROM hosts ORDER BY id",
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
            });
        }
        Ok(hosts)
    }
    pub async fn delete(&mut self, id: i64) -> Result<bool, StorageError> {
        Ok(self
            .connection
            .execute("DELETE FROM hosts WHERE id = ?1", (id,))
            .await?
            != 0)
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
        store.add("Server", &destination).await.unwrap();
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
    async fn saves_lists_and_deletes_hosts() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let destination = SshDestination::new("example.org", "alice", 22).unwrap();
        let id = store.add(" Server ", &destination).await.unwrap();
        assert_eq!(
            store.list().await.unwrap(),
            vec![SavedHost {
                id,
                name: "Server".into(),
                destination
            }]
        );
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
}

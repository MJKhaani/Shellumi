//! Local storage synchronization engine for Shellumi Server v1.
//!
//! Bridges the local Turso database with the remote Shellumi Server API client.
//! Handles pulling remote changes, decrypting zero-knowledge envelopes, applying
//! local database upserts/tombstones, and packaging/signing pending local mutations.

use crate::{
    HostStore, StorageError, SyncGroupPayload, SyncHostPayload, SyncIdentityPayload,
    SyncSnippetPayload, SyncTunnelPayload,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use shellumi_core::server::{
    canonical_mutation_preimage, compute_ciphertext_digest, keypair_from_pkcs8, sign_message,
    CommitBatchRequest, MutationItem, ShellumiServerClient, CURRENT_PROTOCOL_VERSION,
};
use shellumi_core::sync::{decrypt_entity, encrypt_entity, HttpTransport};
use shellumi_core::VaultId;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Persistent record of a remote Shellumi Server connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultServerConnectionRecord {
    pub id: String,
    pub base_url: String,
    pub auth_token: Option<String>,
    pub refresh_token: Option<String>,
    pub device_id: Option<String>,
    pub device_signing_key: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Synchronization progress and state for a vault.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultSyncStateRecord {
    pub vault_id: String,
    pub server_sequence: u64,
    pub last_cursor: Option<String>,
    pub last_synced_at: Option<i64>,
    pub status: String,
    pub error_message: Option<String>,
}

/// A pending local mutation queued for commit to the remote server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingMutationRecord {
    pub id: i64,
    pub vault_id: String,
    pub object_id: String,
    pub entity_type: String,
    pub expected_revision: u64,
    pub deleted: bool,
    pub payload_json: Option<String>,
    pub created_at: i64,
}

/// Summary report of a completed vault synchronization cycle.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSyncReport {
    pub vault_id: String,
    pub changes_received: usize,
    pub changes_applied: usize,
    pub mutations_committed: usize,
    pub latest_sequence: u64,
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl HostStore {
    /// Upserts a remote server connection record.
    pub async fn save_vault_server_connection(
        &mut self,
        conn: &VaultServerConnectionRecord,
    ) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO vault_server_connections (id, base_url, auth_token, refresh_token, device_id, device_signing_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                base_url = excluded.base_url,
                auth_token = excluded.auth_token,
                refresh_token = excluded.refresh_token,
                device_id = excluded.device_id,
                device_signing_key = excluded.device_signing_key,
                updated_at = excluded.updated_at",
            (
                conn.id.as_str(),
                conn.base_url.as_str(),
                conn.auth_token.as_deref(),
                conn.refresh_token.as_deref(),
                conn.device_id.as_deref(),
                conn.device_signing_key.as_deref(),
                conn.created_at,
                conn.updated_at,
            ),
        ).await?;
        Ok(())
    }

    /// Retrieves a remote server connection by ID.
    pub async fn get_vault_server_connection(
        &self,
        id: &str,
    ) -> Result<Option<VaultServerConnectionRecord>, StorageError> {
        let mut rows = self.connection.query(
            "SELECT id, base_url, auth_token, refresh_token, device_id, device_signing_key, created_at, updated_at
             FROM vault_server_connections WHERE id = ?1",
            (id,),
        ).await?;

        if let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let base_url: Option<String> = row.get(1)?;
            let auth_token: Option<String> = row.get(2)?;
            let refresh_token: Option<String> = row.get(3)?;
            let device_id: Option<String> = row.get(4)?;
            let device_signing_key: Option<String> = row.get(5)?;
            let created_at: Option<i64> = row.get(6)?;
            let updated_at: Option<i64> = row.get(7)?;

            Ok(Some(VaultServerConnectionRecord {
                id,
                base_url: base_url.unwrap_or_default(),
                auth_token,
                refresh_token,
                device_id,
                device_signing_key,
                created_at: created_at.unwrap_or(0),
                updated_at: updated_at.unwrap_or(0),
            }))
        } else {
            Ok(None)
        }
    }

    /// Lists all registered server connections.
    pub async fn list_vault_server_connections(
        &self,
    ) -> Result<Vec<VaultServerConnectionRecord>, StorageError> {
        let mut rows = self.connection.query(
            "SELECT id, base_url, auth_token, refresh_token, device_id, device_signing_key, created_at, updated_at
             FROM vault_server_connections ORDER BY id",
            (),
        ).await?;

        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let base_url: Option<String> = row.get(1)?;
            let auth_token: Option<String> = row.get(2)?;
            let refresh_token: Option<String> = row.get(3)?;
            let device_id: Option<String> = row.get(4)?;
            let device_signing_key: Option<String> = row.get(5)?;
            let created_at: Option<i64> = row.get(6)?;
            let updated_at: Option<i64> = row.get(7)?;

            list.push(VaultServerConnectionRecord {
                id,
                base_url: base_url.unwrap_or_default(),
                auth_token,
                refresh_token,
                device_id,
                device_signing_key,
                created_at: created_at.unwrap_or(0),
                updated_at: updated_at.unwrap_or(0),
            });
        }
        Ok(list)
    }

    /// Retrieves synchronization state for a vault.
    pub async fn get_vault_sync_state(
        &self,
        vault_id: &str,
    ) -> Result<Option<VaultSyncStateRecord>, StorageError> {
        let mut rows = self.connection.query(
            "SELECT vault_id, server_sequence, last_cursor, last_synced_at, status, error_message
             FROM vault_sync_state WHERE vault_id = ?1",
            (vault_id,),
        ).await?;

        if let Some(row) = rows.next().await? {
            let vid: String = row.get(0)?;
            let server_sequence: i64 = row.get(1)?;
            let last_cursor: Option<String> = row.get(2)?;
            let last_synced_at: Option<i64> = row.get(3)?;
            let status: String = row.get(4)?;
            let error_message: Option<String> = row.get(5)?;

            Ok(Some(VaultSyncStateRecord {
                vault_id: vid,
                server_sequence: server_sequence.max(0) as u64,
                last_cursor,
                last_synced_at,
                status,
                error_message,
            }))
        } else {
            Ok(None)
        }
    }

    /// Saves or updates the synchronization state for a vault.
    pub async fn save_vault_sync_state(
        &mut self,
        state: &VaultSyncStateRecord,
    ) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO vault_sync_state (vault_id, server_sequence, last_cursor, last_synced_at, status, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(vault_id) DO UPDATE SET
                server_sequence = excluded.server_sequence,
                last_cursor = excluded.last_cursor,
                last_synced_at = excluded.last_synced_at,
                status = excluded.status,
                error_message = excluded.error_message",
            (
                state.vault_id.as_str(),
                state.server_sequence as i64,
                state.last_cursor.as_deref(),
                state.last_synced_at,
                state.status.as_str(),
                state.error_message.as_deref(),
            ),
        ).await?;
        Ok(())
    }

    /// Enqueues a pending client mutation for eventual commit to the remote server.
    pub async fn record_pending_mutation(
        &mut self,
        vault_id: &str,
        object_id: &str,
        entity_type: &str,
        expected_revision: u64,
        deleted: bool,
        payload_json: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = current_timestamp();
        self.connection.execute(
            "INSERT INTO vault_mutations (vault_id, object_id, entity_type, expected_revision, deleted, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                vault_id,
                object_id,
                entity_type,
                expected_revision as i64,
                if deleted { 1i64 } else { 0i64 },
                payload_json,
                now,
            ),
        ).await?;
        Ok(())
    }

    /// Fetches all pending mutations queued for a vault.
    pub async fn get_pending_mutations(
        &self,
        vault_id: &str,
    ) -> Result<Vec<PendingMutationRecord>, StorageError> {
        let mut rows = self.connection.query(
            "SELECT id, vault_id, object_id, entity_type, expected_revision, deleted, payload_json, created_at
             FROM vault_mutations WHERE vault_id = ?1 ORDER BY id ASC",
            (vault_id,),
        ).await?;

        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0)?;
            let vid: String = row.get(1)?;
            let object_id: String = row.get(2)?;
            let entity_type: String = row.get(3)?;
            let expected_revision: i64 = row.get(4)?;
            let deleted: i64 = row.get(5)?;
            let payload_json: Option<String> = row.get(6)?;
            let created_at: i64 = row.get(7)?;

            list.push(PendingMutationRecord {
                id,
                vault_id: vid,
                object_id,
                entity_type,
                expected_revision: expected_revision.max(0) as u64,
                deleted: deleted != 0,
                payload_json,
                created_at,
            });
        }
        Ok(list)
    }

    /// Removes successfully acknowledged mutations from the pending queue.
    pub async fn clear_pending_mutations(
        &mut self,
        vault_id: &str,
        ids: &[i64],
    ) -> Result<(), StorageError> {
        if ids.is_empty() {
            return Ok(());
        }
        for id in ids {
            self.connection
                .execute(
                    "DELETE FROM vault_mutations WHERE vault_id = ?1 AND id = ?2",
                    (vault_id, *id),
                )
                .await?;
        }
        Ok(())
    }
}

/// Synchronization engine orchestrating full two-way delta sync between local Turso
/// storage and the remote Shellumi Server API client.
pub struct ServerSyncEngine;

impl ServerSyncEngine {
    /// Executes a full synchronization cycle for a specific vault:
    /// 1. Pulls incoming delta changes from the server change feed and applies them locally.
    /// 2. Gathers queued local mutations, seals them in zero-knowledge envelopes, signs with device key, and commits.
    /// 3. Updates persistent sync state and cursors.
    pub async fn sync_vault<T: HttpTransport>(
        store: &mut HostStore,
        vault_id: &VaultId,
        remote_vault_id: Uuid,
        client: &ShellumiServerClient<T>,
        passphrase: &str,
    ) -> Result<ServerSyncReport, StorageError> {
        let vault_str = vault_id.as_str();

        // 1. Fetch current sync sequence
        let current_state =
            store
                .get_vault_sync_state(vault_str)
                .await?
                .unwrap_or(VaultSyncStateRecord {
                    vault_id: vault_str.to_string(),
                    server_sequence: 0,
                    last_cursor: None,
                    last_synced_at: None,
                    status: "syncing".to_string(),
                    error_message: None,
                });

        let mut report = ServerSyncReport {
            vault_id: vault_str.to_string(),
            changes_received: 0,
            changes_applied: 0,
            mutations_committed: 0,
            latest_sequence: current_state.server_sequence,
        };

        // -------------------------------------------------------------------
        // 2. Pull Phase: Fetch and apply delta changes
        // -------------------------------------------------------------------
        let change_feed = client
            .fetch_changes(
                remote_vault_id,
                Some(current_state.server_sequence),
                current_state.last_cursor.as_deref(),
                Some(100),
            )
            .map_err(|e| {
                StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
            })?;

        report.changes_received = change_feed.records.len();

        for item in &change_feed.records {
            if item.deleted {
                // Apply deletion tombstone to local tables
                match item.entity_type.as_str() {
                    "host" => {
                        store
                            .connection
                            .execute(
                                "DELETE FROM hosts WHERE name = ?1 AND vault_id = ?2",
                                (item.object_id.as_str(), vault_str),
                            )
                            .await?;
                    }
                    "identity" => {
                        store
                            .connection
                            .execute(
                                "DELETE FROM identities WHERE name = ?1 AND vault_id = ?2",
                                (item.object_id.as_str(), vault_str),
                            )
                            .await?;
                    }
                    "group" => {
                        store
                            .connection
                            .execute(
                                "DELETE FROM groups WHERE name = ?1",
                                (item.object_id.as_str(),),
                            )
                            .await?;
                    }
                    "snippet" => {
                        store
                            .connection
                            .execute(
                                "DELETE FROM snippets WHERE name = ?1",
                                (item.object_id.as_str(),),
                            )
                            .await?;
                    }
                    "tunnel" => {
                        store
                            .connection
                            .execute(
                                "DELETE FROM tunnels WHERE name = ?1",
                                (item.object_id.as_str(),),
                            )
                            .await?;
                    }
                    _ => {}
                }
            } else if let Some(env) = &item.encrypted_envelope {
                // Decrypt envelope using zero-knowledge passphrase
                match item.entity_type.as_str() {
                    "host" => {
                        if let Ok(payload) = decrypt_entity::<SyncHostPayload>(passphrase, env) {
                            let tags_str = payload.tags.join(",");
                            store
                                .connection
                                .execute(
                                    "DELETE FROM hosts WHERE name = ?1 AND vault_id = ?2",
                                    (payload.name.as_str(), vault_str),
                                )
                                .await?;
                            store.connection.execute(
                                "INSERT INTO hosts (name, hostname, username, port, group_name, tags, jump_host, startup_snippet_id, protocol, protocol_config, identity_id, vault_id)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                                (
                                    payload.name.as_str(),
                                    payload.hostname.as_str(),
                                    payload.username.as_str(),
                                    payload.port as i64,
                                    payload.group_name.as_deref(),
                                    tags_str.as_str(),
                                    payload.jump_host.as_deref(),
                                    payload.startup_snippet_id,
                                    payload.protocol.as_str(),
                                    payload.protocol_config.as_deref(),
                                    payload.identity_id,
                                    vault_str,
                                ),
                            ).await?;
                        }
                    }
                    "identity" => {
                        if let Ok(payload) = decrypt_entity::<SyncIdentityPayload>(passphrase, env)
                        {
                            let tags_json = serde_json::to_string(&payload.tags)
                                .unwrap_or_else(|_| "[]".to_string());
                            let auth_method_str = payload.auth_method.as_str();
                            store
                                .connection
                                .execute(
                                    "DELETE FROM identities WHERE name = ?1 AND vault_id = ?2",
                                    (payload.name.as_str(), vault_str),
                                )
                                .await?;
                            store.connection.execute(
                                "INSERT INTO identities (name, username, auth_method, password_secret_ref, key_path, passphrase_secret_ref, certificate_path, tags, comment, vault_id)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                                (
                                    payload.name.as_str(),
                                    payload.username.as_str(),
                                    auth_method_str,
                                    payload.password_secret_ref.as_deref(),
                                    payload.key_path.as_deref(),
                                    payload.passphrase_secret_ref.as_deref(),
                                    payload.certificate_path.as_deref(),
                                    tags_json.as_str(),
                                    payload.comment.as_deref(),
                                    vault_str,
                                ),
                            ).await?;
                        }
                    }
                    "group" => {
                        if let Ok(payload) = decrypt_entity::<SyncGroupPayload>(passphrase, env) {
                            let tags_str = payload.tags.join(",");
                            store.connection.execute(
                                "INSERT INTO groups (name, default_username, default_port, tags, identity_id)
                                 VALUES (?1, ?2, ?3, ?4, ?5)
                                 ON CONFLICT(name) DO UPDATE SET
                                    default_username = excluded.default_username,
                                    default_port = excluded.default_port,
                                    tags = excluded.tags,
                                    identity_id = excluded.identity_id",
                                (
                                    payload.name.as_str(),
                                    payload.default_username.as_deref(),
                                    payload.default_port.map(|p| p as i64),
                                    tags_str.as_str(),
                                    payload.identity_id,
                                ),
                            ).await?;
                        }
                    }
                    "snippet" => {
                        if let Ok(payload) = decrypt_entity::<SyncSnippetPayload>(passphrase, env) {
                            let tags_str = payload.tags.join(",");
                            store
                                .connection
                                .execute(
                                    "DELETE FROM snippets WHERE name = ?1",
                                    (payload.name.as_str(),),
                                )
                                .await?;
                            store
                                .connection
                                .execute(
                                    "INSERT INTO snippets (name, command, description, tags)
                                 VALUES (?1, ?2, ?3, ?4)",
                                    (
                                        payload.name.as_str(),
                                        payload.command.as_str(),
                                        payload.description.as_deref(),
                                        tags_str.as_str(),
                                    ),
                                )
                                .await?;
                        }
                    }
                    "tunnel" => {
                        if let Ok(payload) = decrypt_entity::<SyncTunnelPayload>(passphrase, env) {
                            store
                                .connection
                                .execute(
                                    "DELETE FROM tunnels WHERE name = ?1",
                                    (payload.name.as_str(),),
                                )
                                .await?;
                            store.connection.execute(
                                "INSERT INTO tunnels (name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                                (
                                    payload.name.as_str(),
                                    payload.forwarding_type.as_str(),
                                    payload.bind_address.as_deref(),
                                    payload.bind_port as i64,
                                    payload.target_host.as_deref(),
                                    payload.target_port.map(|p| p as i64),
                                    if payload.auto_start { 1i64 } else { 0i64 },
                                ),
                            ).await?;
                        }
                    }
                    _ => {}
                }
            }
            report.changes_applied += 1;
            if item.sequence > report.latest_sequence {
                report.latest_sequence = item.sequence;
            }
        }

        if change_feed.latest_sequence > report.latest_sequence {
            report.latest_sequence = change_feed.latest_sequence;
        }

        // -------------------------------------------------------------------
        // 3. Push Phase: Commit pending local mutations
        // -------------------------------------------------------------------
        let pending = store.get_pending_mutations(vault_str).await?;
        if !pending.is_empty() {
            // Find server connection to retrieve device signing key
            let contexts = store.list_vault_contexts().await?;
            let server_conn_id = contexts.into_iter().find_map(|(_, ctx)| {
                if ctx.id() == vault_id {
                    match ctx.location() {
                        shellumi_core::VaultLocation::Remote { server, .. } => Some(server.clone()),
                        _ => None,
                    }
                } else {
                    None
                }
            });

            if let Some(conn_id) = server_conn_id {
                if let Some(conn_record) =
                    store.get_vault_server_connection(conn_id.as_str()).await?
                {
                    if let Some(signing_key_b64) = conn_record.device_signing_key {
                        let pkcs8_bytes =
                            BASE64_STANDARD.decode(&signing_key_b64).map_err(|e| {
                                StorageError::InvalidSync(shellumi_core::SyncError::InvalidBase64(
                                    e.to_string(),
                                ))
                            })?;
                        let keypair = keypair_from_pkcs8(&pkcs8_bytes).map_err(|e| {
                            StorageError::InvalidSync(shellumi_core::SyncError::Network(
                                e.to_string(),
                            ))
                        })?;

                        let mut mutations_to_send = Vec::new();
                        let mut pending_ids = Vec::new();

                        for (counter, p) in pending.iter().enumerate() {
                            let envelope = if p.deleted {
                                None
                            } else if let Some(ref raw_json) = p.payload_json {
                                let val: serde_json::Value = serde_json::from_str(raw_json)
                                    .unwrap_or(serde_json::Value::Null);
                                encrypt_entity(passphrase, &p.entity_type, &p.object_id, &val).ok()
                            } else {
                                None
                            };

                            let digest = compute_ciphertext_digest(envelope.as_ref());
                            let preimage = canonical_mutation_preimage(
                                remote_vault_id,
                                &p.object_id,
                                &p.entity_type,
                                p.expected_revision,
                                p.deleted,
                                1, // key_epoch
                                counter as u64 + 1,
                                &digest,
                            );

                            let signature = sign_message(&keypair, preimage.as_bytes());

                            mutations_to_send.push(MutationItem {
                                object_id: p.object_id.clone(),
                                entity_type: p.entity_type.clone(),
                                expected_revision: p.expected_revision,
                                deleted: p.deleted,
                                key_epoch: 1,
                                device_counter: counter as u64 + 1,
                                encrypted_envelope: envelope,
                                device_signature: signature,
                            });
                            pending_ids.push(p.id);
                        }

                        let commit_req = CommitBatchRequest {
                            protocol_version: CURRENT_PROTOCOL_VERSION,
                            vault_id: remote_vault_id,
                            idempotency_key: Uuid::new_v4().to_string(),
                            base_cursor: Some(format!("seq:{}", report.latest_sequence)),
                            mutations: mutations_to_send,
                        };

                        let commit_ack =
                            client.commit(remote_vault_id, &commit_req).map_err(|e| {
                                StorageError::InvalidSync(shellumi_core::SyncError::Network(
                                    e.to_string(),
                                ))
                            })?;

                        report.mutations_committed = commit_ack.applied.len();
                        if commit_ack.commit_sequence > report.latest_sequence {
                            report.latest_sequence = commit_ack.commit_sequence;
                        }

                        // Remove committed pending rows
                        store
                            .clear_pending_mutations(vault_str, &pending_ids)
                            .await?;
                    }
                }
            }
        }

        // -------------------------------------------------------------------
        // 4. Update sync state
        // -------------------------------------------------------------------
        let updated_state = VaultSyncStateRecord {
            vault_id: vault_str.to_string(),
            server_sequence: report.latest_sequence,
            last_cursor: Some(format!("seq:{}", report.latest_sequence)),
            last_synced_at: Some(current_timestamp()),
            status: "success".to_string(),
            error_message: None,
        };
        store.save_vault_sync_state(&updated_state).await?;

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shellumi_core::server::generate_device_ed25519_keypair_pkcs8;
    use shellumi_core::server::types::{
        ChangeFeedResponse, CommitBatchResponse, MutationAck, SyncChangeItem,
    };
    use shellumi_core::sync::{HttpRequest, HttpResponse, SyncError};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MockHttpTransport {
        last_request: Arc<Mutex<Option<HttpRequest>>>,
        responses: Arc<Mutex<Vec<HttpResponse>>>,
    }

    impl MockHttpTransport {
        fn queue_json_response<T: serde::Serialize>(&self, status: u16, body: &T) {
            let bytes = serde_json::to_vec(body).unwrap();
            self.responses
                .lock()
                .unwrap()
                .push(HttpResponse::new(status, bytes));
        }
    }

    impl HttpTransport for MockHttpTransport {
        fn send(&self, request: HttpRequest) -> Result<HttpResponse, SyncError> {
            *self.last_request.lock().unwrap() = Some(request);
            let mut queue = self.responses.lock().unwrap();
            if queue.is_empty() {
                return Err(SyncError::Network("no mock response queued".into()));
            }
            Ok(queue.remove(0))
        }
    }

    #[tokio::test]
    async fn test_server_connection_crud() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        let conn = VaultServerConnectionRecord {
            id: "corp-server".to_string(),
            base_url: "https://shellumi.corp.internal:8443".to_string(),
            auth_token: Some("token-123".to_string()),
            refresh_token: Some("refresh-456".to_string()),
            device_id: Some("dev-789".to_string()),
            device_signing_key: Some("key-bytes".to_string()),
            created_at: 1000,
            updated_at: 1000,
        };

        store.save_vault_server_connection(&conn).await.unwrap();

        let loaded = store
            .get_vault_server_connection("corp-server")
            .await
            .unwrap()
            .expect("should find connection");
        assert_eq!(loaded.base_url, "https://shellumi.corp.internal:8443");
        assert_eq!(loaded.auth_token.as_deref(), Some("token-123"));

        let list = store.list_vault_server_connections().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "corp-server");
    }

    #[tokio::test]
    async fn test_vault_sync_state_and_mutations() {
        let mut store = HostStore::open(":memory:").await.unwrap();

        let state = VaultSyncStateRecord {
            vault_id: "v-remote-1".to_string(),
            server_sequence: 42,
            last_cursor: Some("seq:42".to_string()),
            last_synced_at: Some(2000),
            status: "success".to_string(),
            error_message: None,
        };
        store.save_vault_sync_state(&state).await.unwrap();

        let fetched = store
            .get_vault_sync_state("v-remote-1")
            .await
            .unwrap()
            .expect("should find state");
        assert_eq!(fetched.server_sequence, 42);
        assert_eq!(fetched.status, "success");

        // Record mutation
        store
            .record_pending_mutation(
                "v-remote-1",
                "host-web-1",
                "host",
                0,
                false,
                Some("{\"name\":\"host-web-1\"}"),
            )
            .await
            .unwrap();

        let pending = store.get_pending_mutations("v-remote-1").await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].object_id, "host-web-1");

        store
            .clear_pending_mutations("v-remote-1", &[pending[0].id])
            .await
            .unwrap();
        let after_clear = store.get_pending_mutations("v-remote-1").await.unwrap();
        assert_eq!(after_clear.len(), 0);
    }

    #[tokio::test]
    async fn test_server_sync_engine_pull_and_push() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let vault_id = VaultId::new("v-corp").unwrap();
        let remote_vault_id = Uuid::new_v4();
        let server_id = shellumi_core::ServerConnectionId::new("corp-server").unwrap();
        let passphrase = "my-secret-vault-password";

        // Register server connection with generated Ed25519 device key
        let (pkcs8, _pub_b64) = generate_device_ed25519_keypair_pkcs8().unwrap();
        let dev_id = Uuid::new_v4();
        let conn_rec = VaultServerConnectionRecord {
            id: server_id.as_str().to_string(),
            base_url: "https://shellumi.corp.internal:8443".to_string(),
            auth_token: Some("jwt-bearer-token".to_string()),
            refresh_token: None,
            device_id: Some(dev_id.to_string()),
            device_signing_key: Some(BASE64_STANDARD.encode(&pkcs8)),
            created_at: 1000,
            updated_at: 1000,
        };
        store.save_vault_server_connection(&conn_rec).await.unwrap();

        // Register vault context
        let context = shellumi_core::VaultContext::new(
            vault_id.clone(),
            shellumi_core::VaultLocation::Remote {
                server: server_id,
                server_vault_id: VaultId::new(remote_vault_id.to_string()).unwrap(),
            },
        );
        store
            .register_vault_context("Corp Vault", &context)
            .await
            .unwrap();

        // 1. Prepare remote incoming host in change feed
        let remote_host = SyncHostPayload {
            name: "remote-db-01".to_string(),
            hostname: "db.corp.internal".to_string(),
            username: "postgres".to_string(),
            port: 5432,
            group_name: None,
            tags: vec!["db".to_string(), "prod".to_string()],
            jump_host: None,
            startup_snippet_id: None,
            protocol: "ssh".to_string(),
            protocol_config: None,
            identity_id: None,
        };
        let host_env = encrypt_entity(passphrase, "host", "remote-db-01", &remote_host).unwrap();

        let change_feed = ChangeFeedResponse {
            vault_id: remote_vault_id,
            latest_sequence: 1,
            next_cursor: "seq:1".to_string(),
            has_more: false,
            records: vec![SyncChangeItem {
                sequence: 1,
                object_id: "remote-db-01".to_string(),
                entity_type: "host".to_string(),
                revision: 1,
                deleted: false,
                author_device_id: dev_id,
                encrypted_envelope: Some(host_env),
                updated_at: 1000,
            }],
        };

        // 2. Queue local pending mutation
        let local_host = SyncHostPayload {
            name: "local-redis-01".to_string(),
            hostname: "redis.corp.internal".to_string(),
            username: "redis".to_string(),
            port: 6379,
            group_name: None,
            tags: vec!["cache".to_string()],
            jump_host: None,
            startup_snippet_id: None,
            protocol: "ssh".to_string(),
            protocol_config: None,
            identity_id: None,
        };
        store
            .record_pending_mutation(
                vault_id.as_str(),
                "local-redis-01",
                "host",
                0,
                false,
                Some(&serde_json::to_string(&local_host).unwrap()),
            )
            .await
            .unwrap();

        // Setup mock transport
        let mock = MockHttpTransport::default();
        // 1st request is fetch_changes -> returns change_feed
        mock.queue_json_response(200, &change_feed);
        // 2nd request is commit -> returns CommitBatchResponse
        let commit_ack = CommitBatchResponse {
            vault_id: remote_vault_id,
            commit_sequence: 2,
            next_cursor: "seq:2".to_string(),
            applied: vec![MutationAck {
                object_id: "local-redis-01".to_string(),
                assigned_revision: 1,
                server_sequence: 2,
                deleted: false,
            }],
        };
        mock.queue_json_response(200, &commit_ack);

        let client =
            ShellumiServerClient::with_transport("https://shellumi.corp.internal:8443", mock);

        // Run sync cycle
        let report = ServerSyncEngine::sync_vault(
            &mut store,
            &vault_id,
            remote_vault_id,
            &client,
            passphrase,
        )
        .await
        .unwrap();

        assert_eq!(report.changes_received, 1);
        assert_eq!(report.changes_applied, 1);
        assert_eq!(report.mutations_committed, 1);
        assert_eq!(report.latest_sequence, 2);

        // Verify remote host was saved locally
        let mut rows = store
            .connection
            .query(
                "SELECT name, hostname, port FROM hosts WHERE name = 'remote-db-01'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("host should exist");
        let name: String = row.get(0).unwrap();
        let hostname: String = row.get(1).unwrap();
        let port: i64 = row.get(2).unwrap();
        assert_eq!(name, "remote-db-01");
        assert_eq!(hostname, "db.corp.internal");
        assert_eq!(port, 5432);

        // Verify pending mutation was cleared
        let remaining_mutations = store
            .get_pending_mutations(vault_id.as_str())
            .await
            .unwrap();
        assert_eq!(remaining_mutations.len(), 0);
    }
}

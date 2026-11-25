//! Local storage synchronization engine for Shellumi Server v1.
//!
//! Bridges the local Turso database with the remote Shellumi Server API client.
//! Handles pulling remote changes, decrypting zero-knowledge envelopes, applying
//! local database upserts/tombstones, and packaging/signing pending local mutations.

use crate::{
    HostStore, StorageError, SyncGroupPayload, SyncHostPayload, SyncIdentityPayload,
    SyncSnippetPayload, SyncTunnelPayload, SyncWorkspacePayload,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use shellumi_core::server::{
    canonical_mutation_preimage, compute_ciphertext_digest, generate_device_ed25519_keypair_pkcs8,
    generate_device_x25519_keypair, keypair_from_pkcs8, sign_device_challenge, sign_message,
    validate_x25519_agreement_public_key, wrap_dek_for_recipient, AcceptInviteRequest,
    CommitBatchRequest, DeviceChallengeRequest, DeviceResponse, KeyEnvelopeItem, MutationItem,
    RegisterDeviceRequest, RotateKeyEpochRequest, ShellumiServerClient, SyncChangeItem,
    CURRENT_PROTOCOL_VERSION,
};
use shellumi_core::sync::{decrypt_entity, encrypt_entity, HttpTransport};
use shellumi_core::{EnrolledDevice, EnrollmentStatus, TeamMember, TeamRole, VaultId};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Persistent record of a remote Shellumi Server connection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultServerConnectionRecord {
    pub id: String,
    pub base_url: String,
    pub auth_token: Option<String>,
    pub refresh_token: Option<String>,
    pub device_id: Option<String>,
    pub device_signing_key: Option<String>,
    pub server_instance_id: Option<String>,
    pub device_agreement_key: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl VaultServerConnectionRecord {
    pub const CREDENTIAL_SERVICE: &'static str = "org.shellumi.server";

    /// Secures secrets in the record by moving raw values to CredentialStore and replacing with `keychain://` references.
    pub fn secure_credentials(
        &mut self,
        store: &dyn shellumi_core::CredentialStore,
    ) -> Result<(), shellumi_core::CredentialError> {
        if let Some(token) = &self.auth_token {
            if !token.starts_with("keychain://") && !token.starts_with("env://") {
                let account = format!("{}-auth-token", self.id);
                let uri = shellumi_core::store_secret_to_ref(
                    store,
                    Self::CREDENTIAL_SERVICE,
                    &account,
                    token,
                )?;
                self.auth_token = Some(uri);
            }
        }
        if let Some(refresh) = &self.refresh_token {
            if !refresh.starts_with("keychain://") && !refresh.starts_with("env://") {
                let account = format!("{}-refresh-token", self.id);
                let uri = shellumi_core::store_secret_to_ref(
                    store,
                    Self::CREDENTIAL_SERVICE,
                    &account,
                    refresh,
                )?;
                self.refresh_token = Some(uri);
            }
        }
        if let Some(signing) = &self.device_signing_key {
            if !signing.starts_with("keychain://") && !signing.starts_with("env://") {
                let account = format!("{}-signing-key", self.id);
                let uri = shellumi_core::store_secret_to_ref(
                    store,
                    Self::CREDENTIAL_SERVICE,
                    &account,
                    signing,
                )?;
                self.device_signing_key = Some(uri);
            }
        }
        if let Some(agreement) = &self.device_agreement_key {
            if !agreement.starts_with("keychain://") && !agreement.starts_with("env://") {
                let account = format!("{}-agreement-key", self.id);
                let uri = shellumi_core::store_secret_to_ref(
                    store,
                    Self::CREDENTIAL_SERVICE,
                    &account,
                    agreement,
                )?;
                self.device_agreement_key = Some(uri);
            }
        }
        Ok(())
    }

    /// Resolves auth token from reference or credential store.
    pub fn resolve_auth_token(
        &self,
        store: &dyn shellumi_core::CredentialStore,
    ) -> Option<shellumi_core::SecretString> {
        let token_ref = self.auth_token.as_deref()?;
        if token_ref.starts_with("keychain://") || token_ref.starts_with("env://") {
            shellumi_core::resolve_secret_from_ref(store, token_ref)
                .ok()
                .flatten()
        } else {
            Some(shellumi_core::SecretString::new(token_ref.to_string()))
        }
    }

    pub fn resolve_refresh_token(
        &self,
        store: &dyn shellumi_core::CredentialStore,
    ) -> Option<shellumi_core::SecretString> {
        let token_ref = self.refresh_token.as_deref()?;
        if token_ref.starts_with("keychain://") || token_ref.starts_with("env://") {
            shellumi_core::resolve_secret_from_ref(store, token_ref)
                .ok()
                .flatten()
        } else {
            Some(shellumi_core::SecretString::new(token_ref.to_string()))
        }
    }

    pub fn resolve_signing_key(
        &self,
        store: &dyn shellumi_core::CredentialStore,
    ) -> Option<shellumi_core::SecretString> {
        let key_ref = self.device_signing_key.as_deref()?;
        if key_ref.starts_with("keychain://") || key_ref.starts_with("env://") {
            shellumi_core::resolve_secret_from_ref(store, key_ref)
                .ok()
                .flatten()
        } else {
            Some(shellumi_core::SecretString::new(key_ref.to_string()))
        }
    }

    pub fn resolve_agreement_key(
        &self,
        store: &dyn shellumi_core::CredentialStore,
    ) -> Option<shellumi_core::SecretString> {
        let key_ref = self.device_agreement_key.as_deref()?;
        if key_ref.starts_with("keychain://") || key_ref.starts_with("env://") {
            shellumi_core::resolve_secret_from_ref(store, key_ref)
                .ok()
                .flatten()
        } else {
            Some(shellumi_core::SecretString::new(key_ref.to_string()))
        }
    }
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
    pub active_key_epoch: u32,
    pub conflicted_object_id: Option<String>,
    pub conflict_details: Option<String>,
}

impl Default for VaultSyncStateRecord {
    fn default() -> Self {
        Self {
            vault_id: String::new(),
            server_sequence: 0,
            last_cursor: None,
            last_synced_at: None,
            status: "idle".to_string(),
            error_message: None,
            active_key_epoch: 1,
            conflicted_object_id: None,
            conflict_details: None,
        }
    }
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
    pub idempotency_key: Option<String>,
    pub key_epoch: u32,
    pub device_counter: u64,
}

/// Summary report of a completed vault synchronization cycle.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSyncReport {
    pub vault_id: String,
    pub changes_received: usize,
    pub changes_applied: usize,
    pub mutations_committed: usize,
    pub latest_sequence: u64,
    pub has_conflicts: bool,
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
        self.connection
            .execute(
                "INSERT INTO vault_server_connections (
                id, base_url, auth_token, refresh_token, device_id, device_signing_key,
                server_instance_id, device_agreement_key, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                base_url = excluded.base_url,
                auth_token = excluded.auth_token,
                refresh_token = excluded.refresh_token,
                device_id = excluded.device_id,
                device_signing_key = excluded.device_signing_key,
                server_instance_id = excluded.server_instance_id,
                device_agreement_key = excluded.device_agreement_key,
                updated_at = excluded.updated_at",
                (
                    conn.id.as_str(),
                    conn.base_url.as_str(),
                    conn.auth_token.as_deref(),
                    conn.refresh_token.as_deref(),
                    conn.device_id.as_deref(),
                    conn.device_signing_key.as_deref(),
                    conn.server_instance_id.as_deref(),
                    conn.device_agreement_key.as_deref(),
                    conn.created_at,
                    conn.updated_at,
                ),
            )
            .await?;
        Ok(())
    }

    /// Upserts a remote server connection record after securing plaintext secrets into the credential store.
    pub async fn save_vault_server_connection_secured(
        &mut self,
        conn: &VaultServerConnectionRecord,
        cred_store: &dyn shellumi_core::CredentialStore,
    ) -> Result<(), StorageError> {
        let mut secured = conn.clone();
        secured
            .secure_credentials(cred_store)
            .map_err(StorageError::InvalidCredential)?;
        self.save_vault_server_connection(&secured).await
    }

    /// Retrieves a remote server connection by ID.
    pub async fn get_vault_server_connection(
        &self,
        id: &str,
    ) -> Result<Option<VaultServerConnectionRecord>, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, base_url, auth_token, refresh_token, device_id, device_signing_key,
                    server_instance_id, device_agreement_key, created_at, updated_at
             FROM vault_server_connections WHERE id = ?1",
                (id,),
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let base_url: Option<String> = row.get(1)?;
            let auth_token: Option<String> = row.get(2)?;
            let refresh_token: Option<String> = row.get(3)?;
            let device_id: Option<String> = row.get(4)?;
            let device_signing_key: Option<String> = row.get(5)?;
            let server_instance_id: Option<String> = row.get(6)?;
            let device_agreement_key: Option<String> = row.get(7)?;
            let created_at: Option<i64> = row.get(8)?;
            let updated_at: Option<i64> = row.get(9)?;

            Ok(Some(VaultServerConnectionRecord {
                id,
                base_url: base_url.unwrap_or_default(),
                auth_token,
                refresh_token,
                device_id,
                device_signing_key,
                server_instance_id,
                device_agreement_key,
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
        let mut rows = self
            .connection
            .query(
                "SELECT id, base_url, auth_token, refresh_token, device_id, device_signing_key,
                    server_instance_id, device_agreement_key, created_at, updated_at
             FROM vault_server_connections ORDER BY id",
                (),
            )
            .await?;

        let mut list = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let base_url: Option<String> = row.get(1)?;
            let auth_token: Option<String> = row.get(2)?;
            let refresh_token: Option<String> = row.get(3)?;
            let device_id: Option<String> = row.get(4)?;
            let device_signing_key: Option<String> = row.get(5)?;
            let server_instance_id: Option<String> = row.get(6)?;
            let device_agreement_key: Option<String> = row.get(7)?;
            let created_at: Option<i64> = row.get(8)?;
            let updated_at: Option<i64> = row.get(9)?;

            list.push(VaultServerConnectionRecord {
                id,
                base_url: base_url.unwrap_or_default(),
                auth_token,
                refresh_token,
                device_id,
                device_signing_key,
                server_instance_id,
                device_agreement_key,
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
            "SELECT vault_id, server_sequence, last_cursor, last_synced_at, status, error_message,
                    active_key_epoch, conflicted_object_id, conflict_details
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
            let active_key_epoch: i64 = row.get(6)?;
            let conflicted_object_id: Option<String> = row.get(7)?;
            let conflict_details: Option<String> = row.get(8)?;

            Ok(Some(VaultSyncStateRecord {
                vault_id: vid,
                server_sequence: server_sequence.max(0) as u64,
                last_cursor,
                last_synced_at,
                status,
                error_message,
                active_key_epoch: active_key_epoch.max(1) as u32,
                conflicted_object_id,
                conflict_details,
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
        self.connection
            .execute(
                "INSERT INTO vault_sync_state (
                vault_id, server_sequence, last_cursor, last_synced_at, status, error_message,
                active_key_epoch, conflicted_object_id, conflict_details
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(vault_id) DO UPDATE SET
                server_sequence = excluded.server_sequence,
                last_cursor = excluded.last_cursor,
                last_synced_at = excluded.last_synced_at,
                status = excluded.status,
                error_message = excluded.error_message,
                active_key_epoch = excluded.active_key_epoch,
                conflicted_object_id = excluded.conflicted_object_id,
                conflict_details = excluded.conflict_details",
                (
                    state.vault_id.as_str(),
                    state.server_sequence as i64,
                    state.last_cursor.as_deref(),
                    state.last_synced_at,
                    state.status.as_str(),
                    state.error_message.as_deref(),
                    state.active_key_epoch as i64,
                    state.conflicted_object_id.as_deref(),
                    state.conflict_details.as_deref(),
                ),
            )
            .await?;
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
        self.record_pending_mutation_full(
            vault_id,
            object_id,
            entity_type,
            expected_revision,
            deleted,
            payload_json,
            None,
            1,
            1,
        )
        .await
    }

    /// Enqueues a pending client mutation with full parameters (idempotency key, epoch, counter).
    #[allow(clippy::too_many_arguments)]
    pub async fn record_pending_mutation_full(
        &mut self,
        vault_id: &str,
        object_id: &str,
        entity_type: &str,
        expected_revision: u64,
        deleted: bool,
        payload_json: Option<&str>,
        idempotency_key: Option<&str>,
        key_epoch: u32,
        device_counter: u64,
    ) -> Result<(), StorageError> {
        let now = current_timestamp();
        self.connection
            .execute(
                "INSERT INTO vault_mutations (
                vault_id, object_id, entity_type, expected_revision, deleted, payload_json,
                created_at, idempotency_key, key_epoch, device_counter
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                (
                    vault_id,
                    object_id,
                    entity_type,
                    expected_revision as i64,
                    if deleted { 1i64 } else { 0i64 },
                    payload_json,
                    now,
                    idempotency_key,
                    key_epoch as i64,
                    device_counter as i64,
                ),
            )
            .await?;
        Ok(())
    }

    /// Fetches all pending mutations queued for a vault.
    pub async fn get_pending_mutations(
        &self,
        vault_id: &str,
    ) -> Result<Vec<PendingMutationRecord>, StorageError> {
        let mut rows = self.connection.query(
            "SELECT id, vault_id, object_id, entity_type, expected_revision, deleted, payload_json,
                    created_at, idempotency_key, key_epoch, device_counter
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
            let idempotency_key: Option<String> = row.get(8)?;
            let key_epoch: i64 = row.get(9)?;
            let device_counter: i64 = row.get(10)?;

            list.push(PendingMutationRecord {
                id,
                vault_id: vid,
                object_id,
                entity_type,
                expected_revision: expected_revision.max(0) as u64,
                deleted: deleted != 0,
                payload_json,
                created_at,
                idempotency_key,
                key_epoch: key_epoch.max(1) as u32,
                device_counter: device_counter.max(1) as u64,
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

    /// Enrolls this desktop device with a remote Shellumi Server using protocol v1 challenge-response.
    pub async fn enroll_device_with_server<T: HttpTransport>(
        &mut self,
        connection_id: &str,
        client: &mut ShellumiServerClient<T>,
        device_label: &str,
    ) -> Result<DeviceResponse, StorageError> {
        let credentials = shellumi_core::default_credential_store();
        self.enroll_device_using_credentials(
            connection_id,
            client,
            device_label,
            credentials.as_ref(),
        )
        .await
    }

    async fn enroll_device_using_credentials<T: HttpTransport>(
        &mut self,
        connection_id: &str,
        client: &mut ShellumiServerClient<T>,
        device_label: &str,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<DeviceResponse, StorageError> {
        let (signing_pkcs8, signing_pk_b64) = generate_device_ed25519_keypair_pkcs8()
            .map_err(|e| StorageError::InvalidSync(shellumi_core::SyncError::Api(e.to_string())))?;

        let (agreement_secret, agreement_pk_b64) = generate_device_x25519_keypair()
            .map_err(|e| StorageError::InvalidSync(shellumi_core::SyncError::Api(e.to_string())))?;

        let signing_keypair = keypair_from_pkcs8(&signing_pkcs8)
            .map_err(|e| StorageError::InvalidSync(shellumi_core::SyncError::Api(e.to_string())))?;

        // 1. Request challenge from server
        let challenge_resp = client
            .device_challenge(&DeviceChallengeRequest::default())
            .map_err(|e| {
                StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
            })?;

        // 2. Sign challenge nonce: "shellumi-device-challenge:{nonce}"
        let challenge_sig = sign_device_challenge(&signing_keypair, &challenge_resp.challenge);

        // 3. Register device with signing & agreement keys
        let reg_req = RegisterDeviceRequest {
            device_name: device_label.to_string(),
            signing_public_key: signing_pk_b64,
            agreement_public_key: agreement_pk_b64,
            challenge: Some(challenge_resp.challenge),
            challenge_signature: Some(challenge_sig),
        };

        let reg_resp = client.register_device(&reg_req).map_err(|e| {
            StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
        })?;

        // 4. Query capabilities to get server_instance_id
        let server_instance_id = client
            .capabilities()
            .ok()
            .map(|caps| caps.server_instance_id.to_string());

        // 5. Save connection record
        let now = current_timestamp();
        let existing = self.get_vault_server_connection(connection_id).await?;
        let conn_record = VaultServerConnectionRecord {
            id: connection_id.to_string(),
            base_url: client.base_url().to_string(),
            auth_token: client.token().map(|t| t.to_string()),
            refresh_token: existing.as_ref().and_then(|c| c.refresh_token.clone()),
            device_id: Some(reg_resp.device_id.to_string()),
            device_signing_key: Some(
                shellumi_core::store_secret_to_ref(
                    credentials,
                    VaultServerConnectionRecord::CREDENTIAL_SERVICE,
                    &format!("{}-signing", reg_resp.device_id),
                    &BASE64_STANDARD.encode(&signing_pkcs8),
                )
                .map_err(StorageError::InvalidCredential)?,
            ),
            server_instance_id,
            device_agreement_key: Some(
                shellumi_core::store_secret_to_ref(
                    credentials,
                    VaultServerConnectionRecord::CREDENTIAL_SERVICE,
                    &format!("{}-agreement", reg_resp.device_id),
                    &BASE64_STANDARD.encode(agreement_secret.as_ref()),
                )
                .map_err(StorageError::InvalidCredential)?,
            ),
            created_at: existing.as_ref().map(|c| c.created_at).unwrap_or(now),
            updated_at: now,
        };

        self.save_vault_server_connection_secured(&conn_record, credentials)
            .await?;
        tracing::info!(device_id = %reg_resp.device_id, "Saved protected credentials for registered device");
        Ok(reg_resp)
    }

    /// Rotates the key epoch for a remote vault:
    /// 1. Generates a new 256-bit DEK.
    /// 2. Fetches active recipient devices for the vault.
    /// 3. Wraps the new DEK for each recipient device.
    /// 4. Submits RotateKeyEpochRequest to the server.
    /// 5. Updates local active key epoch in `vault_sync_state` and `vault_contexts`.
    pub async fn rotate_vault_key_epoch<T: HttpTransport>(
        &mut self,
        vault_id: &VaultId,
        remote_vault_id: Uuid,
        client: &ShellumiServerClient<T>,
    ) -> Result<u32, StorageError> {
        let rng = SystemRandom::new();
        let mut new_dek = [0u8; 32];
        rng.fill(&mut new_dek)
            .map_err(|_| StorageError::Serialization("failed to generate random DEK".into()))?;

        let recipients = client
            .list_recipient_devices(remote_vault_id)
            .map_err(|e| {
                StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
            })?;

        let current_state = self
            .get_vault_sync_state(vault_id.as_str())
            .await?
            .unwrap_or_default();
        let new_epoch = current_state.active_key_epoch + 1;

        let mut envelopes = Vec::new();
        for device in recipients.devices {
            let (wrapped_dek, ephemeral_pk) = wrap_dek_for_recipient(
                &new_dek,
                &device.agreement_public_key,
                remote_vault_id,
                new_epoch,
            )
            .map_err(|e| StorageError::InvalidSync(shellumi_core::SyncError::Api(e.to_string())))?;

            envelopes.push(KeyEnvelopeItem {
                recipient_device_id: device.device_id,
                wrapped_dek,
                key_agreement_ephemeral_pk: Some(ephemeral_pk),
            });
        }

        let req = RotateKeyEpochRequest {
            new_epoch,
            envelopes,
        };

        let resp = client
            .rotate_key_epoch(remote_vault_id, &req)
            .map_err(|e| {
                StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
            })?;

        let mut updated_state = current_state;
        updated_state.vault_id = vault_id.as_str().to_string();
        updated_state.active_key_epoch = resp.active_key_epoch;
        self.save_vault_sync_state(&updated_state).await?;
        self.set_vault_active_key_epoch(vault_id.as_str(), resp.active_key_epoch)
            .await?;

        Ok(resp.active_key_epoch)
    }

    /// Accepts a team invitation as an authenticated recipient, generates local signing and
    /// X25519 agreement keys, stores private keys in `credentials`, registers the device via
    /// server challenge-response, verifies server identity binding, and marks the member/device
    /// as `EnrollmentStatus::DeviceEnrolled`.
    #[allow(clippy::too_many_arguments)]
    pub async fn accept_invitation_and_enroll_device<T: HttpTransport>(
        &mut self,
        connection_id: &str,
        local_vault_id: &str,
        member_id: &str,
        display_name: &str,
        ssh_public_key: &str,
        invite_token: &str,
        device_label: &str,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<(TeamMember, EnrolledDevice), StorageError> {
        if client.token().is_none() {
            return Err(StorageError::InvalidCollaboration(
                shellumi_core::CollaborationError::Unauthorized(
                    "Authenticated session token is required to accept an invitation".into(),
                ),
            ));
        }

        // 1. Verify authenticated user identity from server and validate existing local member BEFORE accepting
        let me = client.me().map_err(|e| {
            StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
        })?;

        let existing = self.get_team_member(member_id).await?;
        if let Some(ref m) = existing {
            if m.vault_id != local_vault_id {
                return Err(StorageError::InvalidCollaboration(
                    shellumi_core::CollaborationError::Unauthorized(format!(
                        "Member '{member_id}' belongs to vault '{}', not '{local_vault_id}'",
                        m.vault_id
                    )),
                ));
            }
            if m.user_id != me.user_id.to_string() {
                return Err(StorageError::InvalidCollaboration(
                    shellumi_core::CollaborationError::Unauthorized(format!(
                        "Member '{member_id}' belongs to user '{}', not authenticated user '{}'",
                        m.user_id, me.user_id
                    )),
                ));
            }
            if m.revoked {
                return Err(StorageError::InvalidCollaboration(
                    shellumi_core::CollaborationError::Unauthorized(format!(
                        "Member '{member_id}' is revoked and cannot enroll"
                    )),
                ));
            }
        }

        // 2. Accept invitation on server
        let accept_resp = client
            .accept_invite(&AcceptInviteRequest {
                invite_token: invite_token.to_string(),
            })
            .map_err(|e| {
                StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
            })?;

        if let Some(ctx) = self.get_vault_context(local_vault_id).await? {
            match ctx.location() {
                shellumi_core::VaultLocation::Remote {
                    server,
                    server_vault_id,
                } => {
                    if server.as_str() != connection_id {
                        return Err(StorageError::InvalidCollaboration(
                            shellumi_core::CollaborationError::Unauthorized(format!(
                                "Vault '{local_vault_id}' is bound to server connection '{}', not '{connection_id}'",
                                server.as_str()
                            )),
                        ));
                    }
                    if server_vault_id.as_str() != accept_resp.vault_id.to_string() {
                        return Err(StorageError::InvalidCollaboration(
                            shellumi_core::CollaborationError::Unauthorized(format!(
                                "Accepted invitation vault '{}' does not match vault context remote vault '{}'",
                                accept_resp.vault_id,
                                server_vault_id.as_str()
                            )),
                        ));
                    }
                }
                shellumi_core::VaultLocation::Local => {
                    return Err(StorageError::InvalidCollaboration(
                        shellumi_core::CollaborationError::Unauthorized(format!(
                            "Cannot accept remote invitation into local-only vault '{local_vault_id}'"
                        )),
                    ));
                }
            }
        }

        if let Some(sv) = self.get_shared_vault(local_vault_id).await? {
            if Uuid::parse_str(&sv.id).is_ok() && sv.id != accept_resp.vault_id.to_string() {
                return Err(StorageError::InvalidCollaboration(
                    shellumi_core::CollaborationError::Unauthorized(format!(
                        "Accepted invitation vault '{}' does not match local shared vault '{}'",
                        accept_resp.vault_id, sv.id
                    )),
                ));
            }
        }

        let role = accept_resp.role.parse::<TeamRole>().map_err(|_| {
            StorageError::InvalidCollaboration(shellumi_core::CollaborationError::Unauthorized(
                format!("Server returned invalid team role '{}'", accept_resp.role),
            ))
        })?;

        let op_id = format!("enroll:{local_vault_id}:{member_id}");
        let op_ts = current_timestamp().to_string();
        self.save_enrollment_operation_progress(
            &op_id,
            local_vault_id,
            &me.user_id.to_string(),
            member_id,
            "InvitationAccepted",
            None,
            None,
            &op_ts,
        )
        .await?;

        // 3. Enroll device locally and register with server challenge-response
        let device = self
            .enroll_device_for_authenticated_user(
                connection_id,
                local_vault_id,
                &me.user_id.to_string(),
                me.user_id,
                device_label,
                client,
                credentials,
            )
            .await?;

        let op_ts2 = current_timestamp().to_string();
        self.save_enrollment_operation_progress(
            &op_id,
            local_vault_id,
            &me.user_id.to_string(),
            member_id,
            "DeviceRegistered",
            Some(&device.device_id),
            None,
            &op_ts2,
        )
        .await?;

        // 4. Persist/update member record with DeviceEnrolled status and verified role
        let now_str = current_timestamp().to_string();
        let existing = self.get_team_member(member_id).await?;
        let mut member = if let Some(mut m) = existing {
            m.agreement_public_key = Some(device.agreement_public_key.clone());
            m.enrollment_status = EnrollmentStatus::DeviceEnrolled;
            m.role = role;
            m
        } else {
            TeamMember::new(
                member_id,
                local_vault_id,
                me.user_id.to_string(),
                display_name,
                ssh_public_key,
                role,
                &now_str,
                "server_invite",
            )
            .with_agreement_key(device.agreement_public_key.clone())
            .with_enrollment_status(EnrollmentStatus::DeviceEnrolled)
        };

        if member.id == member_id && self.get_team_member(member_id).await?.is_none() {
            self.add_team_member(&member).await?;
        } else {
            self.update_team_member_enrollment_with_role(
                &member.id,
                local_vault_id,
                role,
                EnrollmentStatus::DeviceEnrolled,
                Some(&device.agreement_public_key),
            )
            .await?;
            member.enrollment_status = EnrollmentStatus::DeviceEnrolled;
            member.role = role;
        }

        let op_ts3 = current_timestamp().to_string();
        self.save_enrollment_operation_progress(
            &op_id,
            local_vault_id,
            &me.user_id.to_string(),
            member_id,
            "Completed",
            Some(&device.device_id),
            None,
            &op_ts3,
        )
        .await?;

        Ok((member, device))
    }

    /// Enrolls an additional authenticated device for an existing user/member.
    #[allow(clippy::too_many_arguments)]
    pub async fn enroll_additional_device_for_member<T: HttpTransport>(
        &mut self,
        connection_id: &str,
        local_vault_id: &str,
        member_id: &str,
        device_label: &str,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<EnrolledDevice, StorageError> {
        if client.token().is_none() {
            return Err(StorageError::InvalidCollaboration(
                shellumi_core::CollaborationError::Unauthorized(
                    "Authenticated session token is required to enroll a device".into(),
                ),
            ));
        }

        let member = self.get_team_member(member_id).await?.ok_or_else(|| {
            StorageError::ItemNotFound(format!("Team member '{member_id}' not found"))
        })?;
        if member.revoked {
            return Err(StorageError::InvalidCollaboration(
                shellumi_core::CollaborationError::Unauthorized(
                    "Cannot enroll a device for a revoked team member".into(),
                ),
            ));
        }

        let me = client.me().map_err(|e| {
            StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
        })?;
        if me.user_id.to_string() != member.user_id && me.username != member.user_id {
            return Err(StorageError::InvalidCollaboration(
                shellumi_core::CollaborationError::Unauthorized(
                    "Authenticated server user does not match target member identity".into(),
                ),
            ));
        }

        let device = self
            .enroll_device_for_authenticated_user(
                connection_id,
                local_vault_id,
                &member.user_id,
                me.user_id,
                device_label,
                client,
                credentials,
            )
            .await?;

        self.update_team_member_enrollment(
            &member.id,
            EnrollmentStatus::DeviceEnrolled,
            Some(&device.agreement_public_key),
        )
        .await?;

        Ok(device)
    }

    /// Replaces a device's cryptographic keys by enrolling a new device and revoking the old one.
    #[allow(clippy::too_many_arguments)]
    pub async fn replace_device_keys_for_member<T: HttpTransport>(
        &mut self,
        connection_id: &str,
        local_vault_id: &str,
        member_id: &str,
        old_device_id: Uuid,
        new_device_label: &str,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<EnrolledDevice, StorageError> {
        let new_device = self
            .enroll_additional_device_for_member(
                connection_id,
                local_vault_id,
                member_id,
                new_device_label,
                client,
                credentials,
            )
            .await?;

        client.revoke_device(old_device_id).map_err(|e| {
            StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
        })?;
        let now_str = current_timestamp().to_string();
        let _ = self
            .revoke_enrolled_device(&old_device_id.to_string(), &now_str)
            .await?;

        Ok(new_device)
    }

    /// Revokes a specific enrolled device on the server and locally, updating member status if no
    /// active verified devices remain.
    pub async fn revoke_device_with_server<T: HttpTransport>(
        &mut self,
        local_vault_id: &str,
        member_id: &str,
        device_id: Uuid,
        client: &ShellumiServerClient<T>,
    ) -> Result<(), StorageError> {
        client.revoke_device(device_id).map_err(|e| {
            StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
        })?;
        let now_str = current_timestamp().to_string();
        let _ = self
            .revoke_enrolled_device(&device_id.to_string(), &now_str)
            .await?;

        if let Some(member) = self.get_team_member(member_id).await? {
            let remaining = self
                .list_enrolled_devices_for_user(local_vault_id, &member.user_id)
                .await?;
            let has_active = remaining.iter().any(|d| !d.revoked && d.verified);
            if !has_active {
                self.update_team_member_enrollment(
                    member_id,
                    EnrollmentStatus::DeviceRevoked,
                    None,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Synchronizes local `team_members` and `enrolled_devices` with authoritative server state.
    pub async fn sync_team_enrollment_from_server<T: HttpTransport>(
        &mut self,
        local_vault_id: &str,
        remote_vault_id: Uuid,
        client: &ShellumiServerClient<T>,
    ) -> Result<(), StorageError> {
        let server_members = client.list_members(remote_vault_id).map_err(|e| {
            StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
        })?;
        let recipient_devices = client
            .list_recipient_devices(remote_vault_id)
            .map_err(|e| {
                StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
            })?;

        let now_str = current_timestamp().to_string();
        let local_members = self.list_team_members(local_vault_id).await?;

        // Persist authoritative server recipient devices without overwriting local private-key refs
        let existing_devices = self.list_enrolled_devices_for_vault(local_vault_id).await?;
        for rd in &recipient_devices.devices {
            if validate_x25519_agreement_public_key(&rd.agreement_public_key).is_err() {
                tracing::warn!(
                    device_id = %rd.device_id,
                    "Ignoring server recipient device with invalid X25519 agreement key"
                );
                continue;
            }
            let dev_id_str = rd.device_id.to_string();
            if let Some(existing) = existing_devices.iter().find(|d| d.device_id == dev_id_str) {
                if existing.revoked {
                    continue;
                }
                if existing.agreement_public_key != rd.agreement_public_key {
                    return Err(StorageError::InvalidCollaboration(
                        shellumi_core::CollaborationError::Unauthorized(format!(
                            "Server reported unexpected X25519 key replacement for device '{dev_id_str}'"
                        )),
                    ));
                }
                let updated = EnrolledDevice {
                    device_id: dev_id_str,
                    vault_id: local_vault_id.to_string(),
                    user_id: rd.user_id.to_string(),
                    device_name: rd.device_name.clone(),
                    signing_public_key: existing.signing_public_key.clone(),
                    agreement_public_key: rd.agreement_public_key.clone(),
                    signing_key_ref: existing.signing_key_ref.clone(),
                    agreement_key_ref: existing.agreement_key_ref.clone(),
                    enrolled_at: existing.enrolled_at.clone(),
                    revoked: false,
                    revoked_at: None,
                    verified: true,
                };
                self.save_enrolled_device(&updated).await?;
            } else {
                let dev = EnrolledDevice {
                    device_id: dev_id_str,
                    vault_id: local_vault_id.to_string(),
                    user_id: rd.user_id.to_string(),
                    device_name: rd.device_name.clone(),
                    signing_public_key: String::new(),
                    agreement_public_key: rd.agreement_public_key.clone(),
                    signing_key_ref: None,
                    agreement_key_ref: None,
                    enrolled_at: now_str.clone(),
                    revoked: false,
                    revoked_at: None,
                    verified: true,
                };
                self.save_enrolled_device(&dev).await?;
            }
        }

        // Mark local devices absent from server recipient list as revoked
        let active_server_ids: std::collections::HashSet<String> = recipient_devices
            .devices
            .iter()
            .map(|d| d.device_id.to_string())
            .collect();
        for local_dev in self.list_enrolled_devices_for_vault(local_vault_id).await? {
            if !local_dev.revoked && !active_server_ids.contains(&local_dev.device_id) {
                let _ = self
                    .revoke_enrolled_device(&local_dev.device_id, &now_str)
                    .await?;
            }
        }

        // Update local member enrollment status from authoritative server state
        for lm in &local_members {
            if lm.revoked {
                continue;
            }
            let matching_server = server_members
                .members
                .iter()
                .find(|sm| sm.user_id.to_string() == lm.user_id || sm.username == lm.user_id);
            let matching_dev = recipient_devices.devices.iter().find(|rd| {
                (rd.user_id.to_string() == lm.user_id
                    || matching_server.is_some_and(|sm| sm.user_id == rd.user_id))
                    && validate_x25519_agreement_public_key(&rd.agreement_public_key).is_ok()
            });

            if let Some(dev) = matching_dev {
                self.update_team_member_enrollment(
                    &lm.id,
                    EnrollmentStatus::DeviceEnrolled,
                    Some(&dev.agreement_public_key),
                )
                .await?;
            } else if matching_server.is_some() {
                let target_status = if lm.agreement_public_key.is_none()
                    && lm.enrollment_status == EnrollmentStatus::LegacyRequiresMigration
                {
                    EnrollmentStatus::LegacyRequiresMigration
                } else {
                    EnrollmentStatus::DeviceEnrollmentPending
                };
                self.update_team_member_enrollment(&lm.id, target_status, None)
                    .await?;
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn enroll_device_for_authenticated_user<T: HttpTransport>(
        &mut self,
        connection_id: &str,
        local_vault_id: &str,
        member_user_id: &str,
        expected_server_user_id: Uuid,
        device_label: &str,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<EnrolledDevice, StorageError> {
        let reg_resp = self
            .enroll_device_using_credentials(connection_id, client, device_label, credentials)
            .await?;

        if reg_resp.user_id != expected_server_user_id {
            return Err(StorageError::InvalidCollaboration(
                shellumi_core::CollaborationError::Unauthorized(
                    "Server bound registered device to a different user identity".into(),
                ),
            ));
        }

        validate_x25519_agreement_public_key(&reg_resp.agreement_public_key).map_err(|e| {
            StorageError::InvalidCollaboration(shellumi_core::CollaborationError::InvalidAction(
                format!("Registered device returned invalid X25519 key: {e}"),
            ))
        })?;

        // Verify device appears in server device listing for the authenticated user
        let devices = client.list_devices().map_err(|e| {
            StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
        })?;
        let verified_on_server = devices.iter().any(|d| {
            d.device_id == reg_resp.device_id
                && d.user_id == expected_server_user_id
                && d.agreement_public_key == reg_resp.agreement_public_key
                && d.revoked_at.is_none()
        });
        if !verified_on_server {
            return Err(StorageError::InvalidCollaboration(
                shellumi_core::CollaborationError::Unauthorized(
                    "Registered device could not be verified in authenticated server device list"
                        .into(),
                ),
            ));
        }

        let conn = self
            .get_vault_server_connection(connection_id)
            .await?
            .ok_or_else(|| {
                StorageError::ItemNotFound(format!(
                    "Server connection '{connection_id}' missing after enrollment"
                ))
            })?;

        let enrolled = EnrolledDevice {
            device_id: reg_resp.device_id.to_string(),
            vault_id: local_vault_id.to_string(),
            user_id: member_user_id.to_string(),
            device_name: reg_resp.device_name,
            signing_public_key: reg_resp.signing_public_key,
            agreement_public_key: reg_resp.agreement_public_key,
            signing_key_ref: conn.device_signing_key,
            agreement_key_ref: conn.device_agreement_key,
            enrolled_at: reg_resp.created_at.to_string(),
            revoked: false,
            revoked_at: None,
            verified: true,
        };
        self.save_enrolled_device(&enrolled).await?;
        Ok(enrolled)
    }
}

/// Applies an incoming sync change record (tombstone or encrypted envelope) to local tables.
async fn apply_change_item(
    store: &mut HostStore,
    vault_str: &str,
    passphrase: &str,
    item: &SyncChangeItem,
) -> Result<bool, StorageError> {
    if item.deleted {
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
                        "DELETE FROM groups WHERE name = ?1 AND vault_id = ?2",
                        (item.object_id.as_str(), vault_str),
                    )
                    .await?;
            }
            "snippet" => {
                store
                    .connection
                    .execute(
                        "DELETE FROM snippets WHERE name = ?1 AND vault_id = ?2",
                        (item.object_id.as_str(), vault_str),
                    )
                    .await?;
            }
            "tunnel" => {
                store
                    .connection
                    .execute(
                        "DELETE FROM tunnels WHERE name = ?1 AND vault_id = ?2",
                        (item.object_id.as_str(), vault_str),
                    )
                    .await?;
            }
            "workspace" => {
                store
                    .connection
                    .execute(
                        "DELETE FROM workspaces WHERE name = ?1 AND vault_id = ?2",
                        (item.object_id.as_str(), vault_str),
                    )
                    .await?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    } else if let Some(env) = &item.encrypted_envelope {
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
                    store
                        .connection
                        .execute(
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
                        )
                        .await?;
                    return Ok(true);
                }
            }
            "identity" => {
                if let Ok(payload) = decrypt_entity::<SyncIdentityPayload>(passphrase, env) {
                    let tags_json =
                        serde_json::to_string(&payload.tags).unwrap_or_else(|_| "[]".to_string());
                    let auth_method_str = payload.auth_method.as_str();
                    store
                        .connection
                        .execute(
                            "DELETE FROM identities WHERE name = ?1 AND vault_id = ?2",
                            (payload.name.as_str(), vault_str),
                        )
                        .await?;
                    store
                        .connection
                        .execute(
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
                        )
                        .await?;
                    return Ok(true);
                }
            }
            "group" => {
                if let Ok(payload) = decrypt_entity::<SyncGroupPayload>(passphrase, env) {
                    let tags_str = payload.tags.join(",");
                    store
                        .connection
                        .execute(
                            "INSERT INTO groups (name, default_username, default_port, tags, identity_id, vault_id)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                             ON CONFLICT(name, vault_id) DO UPDATE SET
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
                                vault_str,
                            ),
                        )
                        .await?;
                    return Ok(true);
                }
            }
            "snippet" => {
                if let Ok(payload) = decrypt_entity::<SyncSnippetPayload>(passphrase, env) {
                    let tags_str = payload.tags.join(",");
                    store
                        .connection
                        .execute(
                            "DELETE FROM snippets WHERE name = ?1 AND vault_id = ?2",
                            (payload.name.as_str(), vault_str),
                        )
                        .await?;
                    store
                        .connection
                        .execute(
                            "INSERT INTO snippets (name, command, description, tags, vault_id)
                             VALUES (?1, ?2, ?3, ?4, ?5)",
                            (
                                payload.name.as_str(),
                                payload.command.as_str(),
                                payload.description.as_deref(),
                                tags_str.as_str(),
                                vault_str,
                            ),
                        )
                        .await?;
                    return Ok(true);
                }
            }
            "tunnel" => {
                if let Ok(payload) = decrypt_entity::<SyncTunnelPayload>(passphrase, env) {
                    store
                        .connection
                        .execute(
                            "DELETE FROM tunnels WHERE name = ?1 AND vault_id = ?2",
                            (payload.name.as_str(), vault_str),
                        )
                        .await?;
                    store
                        .connection
                        .execute(
                            "INSERT INTO tunnels (name, forwarding_type, bind_address, bind_port, target_host, target_port, auto_start, vault_id)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                            (
                                payload.name.as_str(),
                                payload.forwarding_type.as_str(),
                                payload.bind_address.as_deref(),
                                payload.bind_port as i64,
                                payload.target_host.as_deref(),
                                payload.target_port.map(|p| p as i64),
                                if payload.auto_start { 1i64 } else { 0i64 },
                                vault_str,
                            ),
                        )
                        .await?;
                    return Ok(true);
                }
            }
            "workspace" => {
                if let Ok(payload) = decrypt_entity::<SyncWorkspacePayload>(passphrase, env) {
                    let tags_serialized =
                        serde_json::to_string(&payload.tags).unwrap_or_else(|_| "[]".to_string());
                    let sessions_json = serde_json::to_string(&payload.sessions)
                        .unwrap_or_else(|_| "[]".to_string());
                    store
                        .connection
                        .execute(
                            "DELETE FROM workspaces WHERE name = ?1 AND vault_id = ?2",
                            (payload.name.as_str(), vault_str),
                        )
                        .await?;
                    store
                        .connection
                        .execute(
                            "INSERT INTO workspaces (name, description, layout, sessions_json, tags, vault_id)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                            (
                                payload.name.as_str(),
                                payload.description.as_deref(),
                                payload.layout.as_str(),
                                sessions_json.as_str(),
                                tags_serialized.as_str(),
                                vault_str,
                            ),
                        )
                        .await?;
                    return Ok(true);
                }
            }
            _ => {}
        }
        Ok(false)
    } else {
        Ok(false)
    }
}

/// Synchronization engine orchestrating full two-way delta sync between local Turso
/// storage and the remote Shellumi Server API client.
pub struct ServerSyncEngine;

impl ServerSyncEngine {
    /// Executes a full synchronization cycle for a specific vault:
    /// 1. Verifies server instance identity and capabilities.
    /// 2. Pulls incoming delta changes (with snapshot fallback on expired cursors).
    /// 3. Gathers queued local mutations, seals them in zero-knowledge envelopes, signs with device key, and commits.
    /// 4. Handles revision conflicts and stale key epochs with automatic re-encryption or conflict tracking.
    /// 5. Updates persistent sync state and cursors.
    pub async fn sync_vault<T: HttpTransport>(
        store: &mut HostStore,
        vault_id: &VaultId,
        remote_vault_id: Uuid,
        client: &ShellumiServerClient<T>,
        passphrase: &str,
    ) -> Result<ServerSyncReport, StorageError> {
        let cred_store = shellumi_core::default_credential_store();
        Self::sync_vault_with_connection(
            store,
            vault_id,
            remote_vault_id,
            client,
            passphrase,
            None,
            cred_store.as_ref(),
        )
        .await
    }

    /// Delta synchronizes a vault using a pre-resolved server connection record and credential store.
    pub async fn sync_vault_with_connection<T: HttpTransport>(
        store: &mut HostStore,
        vault_id: &VaultId,
        remote_vault_id: Uuid,
        client: &ShellumiServerClient<T>,
        passphrase: &str,
        conn_record: Option<&VaultServerConnectionRecord>,
        cred_store: &dyn shellumi_core::CredentialStore,
    ) -> Result<ServerSyncReport, StorageError> {
        let vault_str = vault_id.as_str();

        // 1. Fetch current sync sequence & state
        let mut current_state =
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
                    active_key_epoch: 1,
                    conflicted_object_id: None,
                    conflict_details: None,
                });

        // 2. Locate associated server connection record and verify instance ID
        let found_conn_record = if let Some(cr) = conn_record {
            Some(cr.clone())
        } else {
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
            if let Some(ref conn_id) = server_conn_id {
                store.get_vault_server_connection(conn_id.as_str()).await?
            } else {
                None
            }
        };

        if let Some(mut active_conn_record) = found_conn_record.clone() {
            if let Ok(caps) = client.capabilities() {
                let remote_inst = caps.server_instance_id.to_string();
                if let Some(ref stored_inst) = active_conn_record.server_instance_id {
                    if stored_inst != &remote_inst {
                        let err_msg = format!(
                            "Server instance mismatch for {}: expected {stored_inst}, server has {remote_inst}. Remote database may have been reset.",
                            active_conn_record.id
                        );
                        current_state.status = "error".to_string();
                        current_state.error_message = Some(err_msg.clone());
                        store.save_vault_sync_state(&current_state).await?;
                        return Err(StorageError::InvalidSync(shellumi_core::SyncError::Api(
                            err_msg,
                        )));
                    }
                } else {
                    active_conn_record.server_instance_id = Some(remote_inst);
                    active_conn_record.updated_at = current_timestamp();
                    let _ = store
                        .save_vault_server_connection(&active_conn_record)
                        .await;
                }
            }
        }

        let remote = client
            .get_vault(remote_vault_id)
            .map_err(|e| StorageError::InvalidSync(shellumi_core::SyncError::Api(e.to_string())))?;
        if remote.vault_id != remote_vault_id
            || remote.vault_type != "personal"
            || remote.active_key_epoch != 1
            || current_state.active_key_epoch != 1
            || store.get_shared_vault(vault_str).await?.is_some()
        {
            tracing::warn!(vault_id = %remote_vault_id, "Blocked legacy sync for vault requiring an epoch key");
            return Err(StorageError::InvalidSync(shellumi_core::SyncError::Api(
                "This vault requires authenticated epoch-key synchronization. Pending changes have been retained.".into()
            )));
        }

        let mut report = ServerSyncReport {
            vault_id: vault_str.to_string(),
            changes_received: 0,
            changes_applied: 0,
            mutations_committed: 0,
            latest_sequence: current_state.server_sequence,
            has_conflicts: false,
        };

        // -------------------------------------------------------------------
        // 3. Pull Phase: Fetch delta changes (with snapshot recovery on 410 / cursor gap)
        // -------------------------------------------------------------------
        let fetch_result = client.fetch_changes(
            remote_vault_id,
            Some(current_state.server_sequence),
            current_state.last_cursor.as_deref(),
            Some(100),
        );

        match fetch_result {
            Ok(change_feed) => {
                report.changes_received = change_feed.records.len();
                for item in &change_feed.records {
                    if apply_change_item(store, vault_str, passphrase, item).await? {
                        report.changes_applied += 1;
                    }
                    if item.sequence > report.latest_sequence {
                        report.latest_sequence = item.sequence;
                    }
                }
                if change_feed.latest_sequence > report.latest_sequence {
                    report.latest_sequence = change_feed.latest_sequence;
                }
                current_state.last_cursor = Some(change_feed.next_cursor);
            }
            Err(err) => {
                let err_str = err.to_string();
                if err_str.contains("CURSOR_EXPIRED") || err_str.contains("410") {
                    // Trigger snapshot recovery catchup
                    let snapshot = client.fetch_latest_snapshot(remote_vault_id).map_err(|e| {
                        StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
                    })?;

                    report.changes_received = snapshot.records.len();
                    for item in &snapshot.records {
                        if apply_change_item(store, vault_str, passphrase, item).await? {
                            report.changes_applied += 1;
                        }
                    }
                    if snapshot.server_sequence > report.latest_sequence {
                        report.latest_sequence = snapshot.server_sequence;
                    }
                    current_state.last_cursor = Some(format!("seq:{}", snapshot.server_sequence));
                } else {
                    current_state.status = "error".to_string();
                    current_state.error_message = Some(err_str.clone());
                    store.save_vault_sync_state(&current_state).await?;
                    return Err(StorageError::InvalidSync(
                        shellumi_core::SyncError::Network(err_str),
                    ));
                }
            }
        }

        // -------------------------------------------------------------------
        // 4. Push Phase: Commit pending local mutations
        // -------------------------------------------------------------------
        let pending = store.get_pending_mutations(vault_str).await?;
        if !pending.is_empty() {
            if let Some(ref conn_record) = found_conn_record {
                if let Some(signing_key_sec) = conn_record.resolve_signing_key(cred_store) {
                    let raw_key = signing_key_sec.expose_secret().trim();
                    let pkcs8_bytes = BASE64_STANDARD.decode(raw_key).map_err(|e| {
                        StorageError::InvalidSync(shellumi_core::SyncError::InvalidBase64(
                            e.to_string(),
                        ))
                    })?;
                    let keypair = keypair_from_pkcs8(&pkcs8_bytes).map_err(|e| {
                        StorageError::InvalidSync(shellumi_core::SyncError::Network(e.to_string()))
                    })?;

                    let batch_idempotency_key = pending
                        .iter()
                        .find_map(|p| p.idempotency_key.clone())
                        .unwrap_or_else(|| Uuid::new_v4().to_string());

                    let active_epoch = current_state.active_key_epoch.max(1);
                    let mut mutations_to_send = Vec::new();
                    let mut pending_ids = Vec::new();

                    for (counter, p) in pending.iter().enumerate() {
                        let device_counter = counter as u64 + 1;
                        let envelope = if p.deleted {
                            None
                        } else if let Some(ref raw_json) = p.payload_json {
                            let val: serde_json::Value = serde_json::from_str(raw_json)
                                .map_err(|e| StorageError::Serialization(e.to_string()))?;
                            Some(encrypt_entity(
                                passphrase,
                                &p.entity_type,
                                &p.object_id,
                                &val,
                            )?)
                        } else {
                            return Err(StorageError::Serialization(
                                "Pending mutation is missing its record payload".into(),
                            ));
                        };

                        let digest = compute_ciphertext_digest(envelope.as_ref());
                        let preimage = canonical_mutation_preimage(
                            remote_vault_id,
                            &p.object_id,
                            &p.entity_type,
                            p.expected_revision,
                            p.deleted,
                            active_epoch,
                            device_counter,
                            &digest,
                        );

                        let signature = sign_message(&keypair, preimage.as_bytes());

                        mutations_to_send.push(MutationItem {
                            object_id: p.object_id.clone(),
                            entity_type: p.entity_type.clone(),
                            expected_revision: p.expected_revision,
                            deleted: p.deleted,
                            key_epoch: active_epoch,
                            device_counter,
                            encrypted_envelope: envelope,
                            device_signature: signature,
                        });
                        pending_ids.push(p.id);
                    }

                    let commit_req = CommitBatchRequest {
                        protocol_version: CURRENT_PROTOCOL_VERSION,
                        vault_id: remote_vault_id,
                        idempotency_key: batch_idempotency_key,
                        base_cursor: current_state
                            .last_cursor
                            .clone()
                            .or_else(|| Some(format!("seq:{}", report.latest_sequence))),
                        mutations: mutations_to_send,
                    };

                    let commit_result = client.commit(remote_vault_id, &commit_req);

                    match commit_result {
                        Ok(commit_ack) => {
                            report.mutations_committed = commit_ack.applied.len();
                            if commit_ack.commit_sequence > report.latest_sequence {
                                report.latest_sequence = commit_ack.commit_sequence;
                            }
                            current_state.last_cursor = Some(commit_ack.next_cursor);
                            current_state.conflicted_object_id = None;
                            current_state.conflict_details = None;

                            // Remove committed pending rows
                            store
                                .clear_pending_mutations(vault_str, &pending_ids)
                                .await?;
                        }
                        Err(err) => {
                            let err_str = err.to_string();
                            let err_lower = err_str.to_lowercase();
                            if err_str.contains("STALE_KEY_EPOCH") {
                                // Stale key epoch: advance active epoch and report error
                                // Only the server can confirm a committed epoch.
                                current_state.status = "error".to_string();
                                current_state.error_message = Some(err_str.clone());
                                store.save_vault_sync_state(&current_state).await?;
                                return Err(StorageError::InvalidSync(
                                    shellumi_core::SyncError::Network(err_str),
                                ));
                            } else if err_lower.contains("conflict")
                                || err_lower.contains("409")
                                || err_str.contains("REVISION_CONFLICT")
                            {
                                // Conflict: record conflict status, do not discard pending mutations
                                report.has_conflicts = true;
                                current_state.status = "conflict".to_string();
                                current_state.conflicted_object_id =
                                    pending.first().map(|p| p.object_id.clone());
                                current_state.conflict_details = Some(err_str.clone());
                                store.save_vault_sync_state(&current_state).await?;
                                return Ok(report);
                            } else {
                                current_state.status = "error".to_string();
                                current_state.error_message = Some(err_str.clone());
                                store.save_vault_sync_state(&current_state).await?;
                                return Err(StorageError::InvalidSync(
                                    shellumi_core::SyncError::Network(err_str),
                                ));
                            }
                        }
                    }
                }
            }
        }

        // -------------------------------------------------------------------
        // 5. Update sync state
        // -------------------------------------------------------------------
        current_state.server_sequence = report.latest_sequence;
        if current_state.last_cursor.is_none() {
            current_state.last_cursor = Some(format!("seq:{}", report.latest_sequence));
        }
        current_state.last_synced_at = Some(current_timestamp());
        current_state.status = if report.has_conflicts {
            "conflict".to_string()
        } else {
            "success".to_string()
        };
        current_state.error_message = None;

        store.save_vault_sync_state(&current_state).await?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shellumi_core::server::generate_device_ed25519_keypair_pkcs8;
    use shellumi_core::server::types::{
        CapabilitiesResponse, ChangeFeedResponse, CommitBatchResponse, DeviceChallengeResponse,
        MutationAck, ServerLimits, SnapshotResponse, SyncChangeItem,
    };
    use shellumi_core::sync::{HttpRequest, HttpResponse, SyncError};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MockHttpTransport {
        last_request: Arc<Mutex<Option<HttpRequest>>>,
        responses: Arc<Mutex<Vec<HttpResponse>>>,
    }

    impl MockHttpTransport {
        fn queue_personal_vault(&self, vault_id: Uuid) {
            self.queue_json_response(
                200,
                &serde_json::json!({
                    "vault_id": vault_id, "tenant_id": Uuid::new_v4(), "owner_id": Uuid::new_v4(),
                    "vault_type": "personal", "active_key_epoch": 1, "server_sequence": 0,
                    "role": "owner", "created_at": 1, "updated_at": 1
                }),
            );
        }

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
            server_instance_id: Some("srv-inst-abc".to_string()),
            device_agreement_key: Some("agreement-bytes".to_string()),
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
        assert_eq!(loaded.server_instance_id.as_deref(), Some("srv-inst-abc"));
        assert_eq!(
            loaded.device_agreement_key.as_deref(),
            Some("agreement-bytes")
        );

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
            active_key_epoch: 2,
            conflicted_object_id: None,
            conflict_details: None,
        };
        store.save_vault_sync_state(&state).await.unwrap();

        let fetched = store
            .get_vault_sync_state("v-remote-1")
            .await
            .unwrap()
            .expect("should find state");
        assert_eq!(fetched.server_sequence, 42);
        assert_eq!(fetched.status, "success");
        assert_eq!(fetched.active_key_epoch, 2);

        // Record mutation with full parameters
        store
            .record_pending_mutation_full(
                "v-remote-1",
                "host-web-1",
                "host",
                0,
                false,
                Some("{\"name\":\"host-web-1\"}"),
                Some("idem-key-123"),
                2,
                1,
            )
            .await
            .unwrap();

        let pending = store.get_pending_mutations("v-remote-1").await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].object_id, "host-web-1");
        assert_eq!(pending[0].idempotency_key.as_deref(), Some("idem-key-123"));
        assert_eq!(pending[0].key_epoch, 2);

        store
            .clear_pending_mutations("v-remote-1", &[pending[0].id])
            .await
            .unwrap();
        let after_clear = store.get_pending_mutations("v-remote-1").await.unwrap();
        assert_eq!(after_clear.len(), 0);
    }

    #[tokio::test]
    async fn test_device_enrollment_flow() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let mock = MockHttpTransport::default();

        let server_instance = Uuid::new_v4();
        let dev_id = Uuid::new_v4();

        // 1. Queue challenge response
        mock.queue_json_response(
            200,
            &DeviceChallengeResponse {
                challenge: "test-nonce-12345678".to_string(),
                expires_at: 99999999,
            },
        );

        // 2. Queue register device response
        mock.queue_json_response(
            200,
            &DeviceResponse {
                device_id: dev_id,
                user_id: Uuid::new_v4(),
                device_name: "MacBook Pro M4".to_string(),
                signing_public_key: "signing-pk".to_string(),
                agreement_public_key: "agreement-pk".to_string(),
                created_at: 12345,
                last_seen_at: None,
                revoked_at: None,
            },
        );

        // 3. Queue capabilities response
        mock.queue_json_response(
            200,
            &CapabilitiesResponse {
                protocol_version: 1,
                server_instance_id: server_instance,
                server_version: "1.0.0".to_string(),
                min_protocol_version: 1,
                supported_features: vec!["e2ee".to_string()],
                supported_e2ee_suites: vec!["aes256gcm_dek_v1".to_string()],
                websocket: None,
                limits: ServerLimits {
                    max_request_bytes: 1048576,
                    max_record_bytes: 524288,
                    max_mutations_per_commit: 100,
                    max_change_page: 100,
                },
            },
        );

        let mut client =
            ShellumiServerClient::with_transport("https://shellumi.corp.internal:8443", mock);

        let credentials = shellumi_core::EphemeralCredentialStore::new();
        let resp = store
            .enroll_device_using_credentials(
                "corp-server",
                &mut client,
                "MacBook Pro M4",
                &credentials,
            )
            .await
            .unwrap();

        assert_eq!(resp.device_id, dev_id);

        let conn = store
            .get_vault_server_connection("corp-server")
            .await
            .unwrap()
            .expect("connection should be saved");
        assert_eq!(conn.device_id.as_deref(), Some(dev_id.to_string().as_str()));
        assert_eq!(
            conn.server_instance_id.as_deref(),
            Some(server_instance.to_string().as_str())
        );
        assert!(conn.device_signing_key.is_some());
        assert!(conn
            .device_agreement_key
            .as_ref()
            .unwrap()
            .starts_with("keychain://"));
        assert!(conn.resolve_agreement_key(&credentials).is_some());
    }

    #[tokio::test]
    async fn team_sync_retains_queue_without_reading_or_publishing_passphrase_records() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let local = VaultId::new("team-vault").unwrap();
        let remote = Uuid::new_v4();
        store
            .record_pending_mutation(
                local.as_str(),
                "record",
                "host",
                0,
                false,
                Some(r#"{"name":"record"}"#),
            )
            .await
            .unwrap();
        let transport = MockHttpTransport::default();
        transport.queue_json_response(
            200,
            &serde_json::json!({
                "vault_id": remote, "tenant_id": Uuid::new_v4(), "owner_id": Uuid::new_v4(),
                "vault_type": "team", "active_key_epoch": 1, "server_sequence": 0,
                "role": "owner", "created_at": 1, "updated_at": 1
            }),
        );
        let client =
            ShellumiServerClient::with_transport("https://example.invalid", transport.clone());
        let result =
            ServerSyncEngine::sync_vault(&mut store, &local, remote, &client, "legacy-passphrase")
                .await;
        assert!(result.unwrap_err().to_string().contains("epoch-key"));
        let request = transport.last_request.lock().unwrap().clone().unwrap();
        assert_eq!(request.method, "GET");
        assert!(request.url.ends_with(&format!("/vaults/{remote}")));
        assert_eq!(
            store
                .get_pending_mutations(local.as_str())
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(store
            .get_vault_sync_state(local.as_str())
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_snapshot_recovery_on_cursor_expired() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let vault_id = VaultId::new("v-corp").unwrap();
        let remote_vault_id = Uuid::new_v4();
        let passphrase = "test-passphrase";

        // Setup mock transport
        let mock = MockHttpTransport::default();

        // 1. Change feed returns 410 Cursor Expired
        let err_resp = shellumi_core::server::types::ErrorResponse {
            error_code: "CURSOR_EXPIRED".to_string(),
            message: "Cursor has expired".to_string(),
            request_id: None,
        };
        mock.queue_personal_vault(remote_vault_id);
        mock.queue_json_response(410, &err_resp);

        // 2. Snapshot response
        let host = SyncHostPayload {
            name: "recovered-host".to_string(),
            hostname: "recovered.internal".to_string(),
            username: "root".to_string(),
            port: 22,
            group_name: None,
            tags: vec![],
            jump_host: None,
            startup_snippet_id: None,
            protocol: "ssh".to_string(),
            protocol_config: None,
            identity_id: None,
        };
        let host_env = encrypt_entity(passphrase, "host", "recovered-host", &host).unwrap();

        mock.queue_json_response(
            200,
            &SnapshotResponse {
                vault_id: remote_vault_id,
                server_sequence: 10,
                records: vec![SyncChangeItem {
                    sequence: 10,
                    object_id: "recovered-host".to_string(),
                    entity_type: "host".to_string(),
                    revision: 1,
                    deleted: false,
                    author_device_id: Uuid::new_v4(),
                    encrypted_envelope: Some(host_env),
                    updated_at: 1000,
                }],
            },
        );

        let client =
            ShellumiServerClient::with_transport("https://shellumi.corp.internal:8443", mock);

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
        assert_eq!(report.latest_sequence, 10);

        // Verify recovered host is in database
        let mut rows = store
            .connection
            .query(
                "SELECT hostname FROM hosts WHERE name = 'recovered-host'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("host should exist");
        let hostname: String = row.get(0).unwrap();
        assert_eq!(hostname, "recovered.internal");
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
            server_instance_id: None,
            device_agreement_key: None,
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
        // 1st request is capabilities
        let server_instance = Uuid::new_v4();
        mock.queue_json_response(
            200,
            &CapabilitiesResponse {
                protocol_version: 1,
                server_instance_id: server_instance,
                server_version: "1.0.0".to_string(),
                min_protocol_version: 1,
                supported_features: vec!["e2ee".to_string()],
                supported_e2ee_suites: vec!["aes256gcm_dek_v1".to_string()],
                websocket: None,
                limits: ServerLimits {
                    max_request_bytes: 1048576,
                    max_record_bytes: 524288,
                    max_mutations_per_commit: 100,
                    max_change_page: 100,
                },
            },
        );
        // 2nd request is fetch_changes -> returns change_feed
        mock.queue_personal_vault(remote_vault_id);
        mock.queue_json_response(200, &change_feed);
        // 3rd request is commit -> returns CommitBatchResponse
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

    #[tokio::test]
    async fn test_epoch_rotation_flow() {
        use shellumi_core::server::generate_device_x25519_keypair;
        use shellumi_core::server::types::{
            RecipientDeviceResponse, RecipientDevicesResponse, RotateKeyEpochResponse,
        };

        let mut store = HostStore::open(":memory:").await.unwrap();
        let vault_id = VaultId::new("v-corp").unwrap();
        let remote_vault_id = Uuid::new_v4();
        let srv_id = shellumi_core::ServerConnectionId::new("corp-server").unwrap();

        let conn = VaultServerConnectionRecord {
            id: srv_id.as_str().to_string(),
            base_url: "https://shellumi.corp.internal:8443".to_string(),
            auth_token: Some("token".to_string()),
            refresh_token: None,
            device_id: None,
            device_signing_key: None,
            server_instance_id: None,
            device_agreement_key: None,
            created_at: 1000,
            updated_at: 1000,
        };
        store.save_vault_server_connection(&conn).await.unwrap();

        let context = shellumi_core::VaultContext::new(
            vault_id.clone(),
            shellumi_core::VaultLocation::Remote {
                server: srv_id,
                server_vault_id: VaultId::new(remote_vault_id.to_string()).unwrap(),
            },
        );
        store
            .register_vault_context("Corp", &context)
            .await
            .unwrap();
        let mock = MockHttpTransport::default();

        let (_priv_scalar, recipient_pk_b64) = generate_device_x25519_keypair().unwrap();
        let recipient_dev = RecipientDeviceResponse {
            device_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            device_name: "Colleague Laptop".to_string(),
            agreement_public_key: recipient_pk_b64,
            key_fingerprint: "SHA256:fingerprint".to_string(),
        };

        // 1. List recipient devices
        mock.queue_json_response(
            200,
            &RecipientDevicesResponse {
                vault_id: remote_vault_id,
                devices: vec![recipient_dev],
            },
        );

        // 2. Rotate key epoch response
        mock.queue_json_response(
            200,
            &RotateKeyEpochResponse {
                vault_id: remote_vault_id,
                active_key_epoch: 2,
                enrolled_envelopes: 1,
            },
        );

        let client =
            ShellumiServerClient::with_transport("https://shellumi.corp.internal:8443", mock);

        let new_epoch = store
            .rotate_vault_key_epoch(&vault_id, remote_vault_id, &client)
            .await
            .unwrap();

        assert_eq!(new_epoch, 2);

        let sync_state = store
            .get_vault_sync_state(vault_id.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sync_state.active_key_epoch, 2);

        let epoch = store
            .get_vault_active_key_epoch(vault_id.as_str())
            .await
            .unwrap();
        assert_eq!(epoch, 2);
    }

    #[tokio::test]
    async fn test_revision_conflict_handling() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let vault_id = VaultId::new("v-conflict").unwrap();
        let remote_vault_id = Uuid::new_v4();
        let server_id = shellumi_core::ServerConnectionId::new("corp-server").unwrap();
        let passphrase = "vault-passphrase";

        let (pkcs8, _pub_b64) = generate_device_ed25519_keypair_pkcs8().unwrap();
        let conn_rec = VaultServerConnectionRecord {
            id: server_id.as_str().to_string(),
            base_url: "https://shellumi.corp.internal:8443".to_string(),
            auth_token: Some("jwt-bearer-token".to_string()),
            refresh_token: None,
            device_id: Some(Uuid::new_v4().to_string()),
            device_signing_key: Some(BASE64_STANDARD.encode(&pkcs8)),
            server_instance_id: None,
            device_agreement_key: None,
            created_at: 1000,
            updated_at: 1000,
        };
        store.save_vault_server_connection(&conn_rec).await.unwrap();

        let context = shellumi_core::VaultContext::new(
            vault_id.clone(),
            shellumi_core::VaultLocation::Remote {
                server: server_id,
                server_vault_id: VaultId::new(remote_vault_id.to_string()).unwrap(),
            },
        );
        store
            .register_vault_context("Conflict Vault", &context)
            .await
            .unwrap();

        // Pending mutation
        store
            .record_pending_mutation(
                vault_id.as_str(),
                "conflicted-host",
                "host",
                0,
                false,
                Some("{\"name\":\"conflicted-host\"}"),
            )
            .await
            .unwrap();

        let mock = MockHttpTransport::default();
        // 1. Capabilities
        let server_instance = Uuid::new_v4();
        mock.queue_json_response(
            200,
            &CapabilitiesResponse {
                protocol_version: 1,
                server_instance_id: server_instance,
                server_version: "1.0.0".to_string(),
                min_protocol_version: 1,
                supported_features: vec!["e2ee".to_string()],
                supported_e2ee_suites: vec!["aes256gcm_dek_v1".to_string()],
                websocket: None,
                limits: ServerLimits {
                    max_request_bytes: 1048576,
                    max_record_bytes: 524288,
                    max_mutations_per_commit: 100,
                    max_change_page: 100,
                },
            },
        );
        mock.queue_personal_vault(remote_vault_id);
        // 2. Fetch changes (empty)
        mock.queue_json_response(
            200,
            &ChangeFeedResponse {
                vault_id: remote_vault_id,
                latest_sequence: 1,
                next_cursor: "seq:1".to_string(),
                has_more: false,
                records: vec![],
            },
        );
        // 3. Commit returns 409 Conflict
        let err_resp = shellumi_core::server::types::ErrorResponse {
            error_code: "REVISION_CONFLICT".to_string(),
            message: "Object conflicted-host revision conflict".to_string(),
            request_id: None,
        };
        mock.queue_json_response(409, &err_resp);

        let client =
            ShellumiServerClient::with_transport("https://shellumi.corp.internal:8443", mock);

        let report = ServerSyncEngine::sync_vault(
            &mut store,
            &vault_id,
            remote_vault_id,
            &client,
            passphrase,
        )
        .await
        .unwrap();

        assert!(report.has_conflicts);
        assert_eq!(report.mutations_committed, 0);

        // Pending mutation was preserved for resolution
        let remaining = store
            .get_pending_mutations(vault_id.as_str())
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);

        let sync_state = store
            .get_vault_sync_state(vault_id.as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sync_state.status, "conflict");
        assert_eq!(
            sync_state.conflicted_object_id.as_deref(),
            Some("conflicted-host")
        );
    }

    #[tokio::test]
    async fn test_server_instance_id_mismatch_detection() {
        let mut store = HostStore::open(":memory:").await.unwrap();
        let vault_id = VaultId::new("v-mismatch").unwrap();
        let remote_vault_id = Uuid::new_v4();
        let server_id = shellumi_core::ServerConnectionId::new("corp-server").unwrap();

        let inst_a = Uuid::new_v4();
        let inst_b = Uuid::new_v4();

        let conn_rec = VaultServerConnectionRecord {
            id: server_id.as_str().to_string(),
            base_url: "https://shellumi.corp.internal:8443".to_string(),
            auth_token: Some("token".to_string()),
            refresh_token: None,
            device_id: None,
            device_signing_key: None,
            server_instance_id: Some(inst_a.to_string()),
            device_agreement_key: None,
            created_at: 1000,
            updated_at: 1000,
        };
        store.save_vault_server_connection(&conn_rec).await.unwrap();

        let context = shellumi_core::VaultContext::new(
            vault_id.clone(),
            shellumi_core::VaultLocation::Remote {
                server: server_id,
                server_vault_id: VaultId::new(remote_vault_id.to_string()).unwrap(),
            },
        );
        store
            .register_vault_context("Mismatch Vault", &context)
            .await
            .unwrap();

        let mock = MockHttpTransport::default();
        // Server reports instance B
        mock.queue_json_response(
            200,
            &CapabilitiesResponse {
                protocol_version: 1,
                server_instance_id: inst_b,
                server_version: "1.0.0".to_string(),
                min_protocol_version: 1,
                supported_features: vec![],
                supported_e2ee_suites: vec![],
                websocket: None,
                limits: ServerLimits {
                    max_request_bytes: 1000,
                    max_record_bytes: 1000,
                    max_mutations_per_commit: 10,
                    max_change_page: 10,
                },
            },
        );

        let client =
            ShellumiServerClient::with_transport("https://shellumi.corp.internal:8443", mock);

        let res =
            ServerSyncEngine::sync_vault(&mut store, &vault_id, remote_vault_id, &client, "pass")
                .await;

        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Server instance mismatch"));
    }

    #[tokio::test]
    async fn test_authenticated_x25519_team_device_enrollment_lifecycle_and_security() {
        use shellumi_core::server::crypto::{
            unwrap_dek_for_device, validate_x25519_agreement_public_key, verify_device_signature,
            wrap_dek_for_recipient,
        };
        use shellumi_core::server::types::{
            AcceptInviteResponse, DeviceChallengeResponse, MeResponse, RecipientDeviceResponse,
            RecipientDevicesResponse, VaultMemberListResponse, VaultMemberResponse,
        };
        use shellumi_core::{
            resolve_secret_from_ref, CredentialError, CredentialStore, EphemeralCredentialStore,
            SecretString, SharedVault,
        };
        use std::sync::{Arc, Mutex};

        struct StatefulEnrollmentTransport {
            user_id: Uuid,
            username: String,
            vault_id: Uuid,
            server_instance_id: Uuid,
            expected_invite_token: String,
            devices: Mutex<Vec<DeviceResponse>>,
            forge_wrong_user: Mutex<bool>,
            omit_from_list: Mutex<bool>,
        }

        #[derive(Clone)]
        struct SharedTransport(Arc<StatefulEnrollmentTransport>);

        impl HttpTransport for SharedTransport {
            fn send(&self, request: HttpRequest) -> Result<HttpResponse, SyncError> {
                let method = request.method.as_str();
                let url = request.url.as_str();
                let auth_ok = request.headers.iter().any(|(k, v)| {
                    k.eq_ignore_ascii_case("Authorization") && v.starts_with("Bearer ")
                });
                if !auth_ok && !url.ends_with("/capabilities") {
                    return Ok(HttpResponse::new(
                        401,
                        br#"{"error":"unauthorized","message":"missing bearer token"}"#.to_vec(),
                    ));
                }

                if method == "GET" && url.ends_with("/api/v1/me") {
                    let resp = MeResponse {
                        user_id: self.0.user_id,
                        username: self.0.username.clone(),
                        email: self.0.username.clone(),
                        organizations: vec![],
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&resp).unwrap()));
                }

                if method == "POST" && url.ends_with("/api/v1/invites/accept") {
                    let req: AcceptInviteRequest = serde_json::from_slice(&request.body).unwrap();
                    if req.invite_token != self.0.expected_invite_token {
                        return Ok(HttpResponse::new(
                            403,
                            br#"{"error":"forbidden","message":"invalid invite token"}"#.to_vec(),
                        ));
                    }
                    let resp = AcceptInviteResponse {
                        vault_id: self.0.vault_id,
                        role: "editor".to_string(),
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&resp).unwrap()));
                }

                if method == "POST" && url.ends_with("/api/v1/devices/challenge") {
                    let resp = DeviceChallengeResponse {
                        challenge: "nonce-test-challenge-001".to_string(),
                        expires_at: 9_999_999_999,
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&resp).unwrap()));
                }

                if method == "POST" && url.ends_with("/api/v1/devices") {
                    let req: RegisterDeviceRequest = serde_json::from_slice(&request.body).unwrap();
                    // Server verifies Ed25519 challenge signature & contributory X25519 key
                    let challenge = req.challenge.as_deref().unwrap_or("");
                    let sig = req.challenge_signature.as_deref().unwrap_or("");
                    let preimage = format!("shellumi-device-challenge:{challenge}");
                    if verify_device_signature(&req.signing_public_key, preimage.as_bytes(), sig)
                        .is_err()
                    {
                        return Ok(HttpResponse::new(
                            400,
                            br#"{"error":"bad_request","message":"invalid challenge signature"}"#
                                .to_vec(),
                        ));
                    }
                    if validate_x25519_agreement_public_key(&req.agreement_public_key).is_err() {
                        return Ok(HttpResponse::new(
                            400,
                            br#"{"error":"bad_request","message":"invalid agreement key"}"#
                                .to_vec(),
                        ));
                    }
                    let bound_user = if *self.0.forge_wrong_user.lock().unwrap() {
                        Uuid::new_v4()
                    } else {
                        self.0.user_id
                    };
                    let dev = DeviceResponse {
                        device_id: Uuid::new_v4(),
                        user_id: bound_user,
                        device_name: req.device_name,
                        signing_public_key: req.signing_public_key,
                        agreement_public_key: req.agreement_public_key,
                        created_at: 2000,
                        last_seen_at: Some(2000),
                        revoked_at: None,
                    };
                    self.0.devices.lock().unwrap().push(dev.clone());
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&dev).unwrap()));
                }

                if method == "GET" && url.ends_with("/api/v1/devices") {
                    let list = if *self.0.omit_from_list.lock().unwrap() {
                        Vec::new()
                    } else {
                        self.0.devices.lock().unwrap().clone()
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&list).unwrap()));
                }

                if method == "DELETE" && url.contains("/api/v1/devices/") {
                    let id_str = url.rsplit('/').next().unwrap_or("");
                    let target_id = Uuid::parse_str(id_str).unwrap();
                    let mut devs = self.0.devices.lock().unwrap();
                    for d in devs.iter_mut() {
                        if d.device_id == target_id {
                            d.revoked_at = Some(3000);
                        }
                    }
                    return Ok(HttpResponse::new(204, Vec::new()));
                }

                if method == "GET" && url.ends_with("/capabilities") {
                    let caps = CapabilitiesResponse {
                        protocol_version: 1,
                        server_instance_id: self.0.server_instance_id,
                        server_version: "1.0.0".to_string(),
                        min_protocol_version: 1,
                        supported_features: vec![],
                        supported_e2ee_suites: vec![],
                        websocket: None,
                        limits: ServerLimits {
                            max_request_bytes: 1_000_000,
                            max_record_bytes: 1_000_000,
                            max_mutations_per_commit: 100,
                            max_change_page: 100,
                        },
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&caps).unwrap()));
                }

                if method == "GET" && url.ends_with("/members") {
                    let resp = VaultMemberListResponse {
                        members: vec![VaultMemberResponse {
                            vault_id: self.0.vault_id,
                            user_id: self.0.user_id,
                            username: self.0.username.clone(),
                            role: "editor".to_string(),
                            joined_at: 2000,
                        }],
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&resp).unwrap()));
                }

                if method == "GET" && url.ends_with("/recipient-devices") {
                    let devs: Vec<RecipientDeviceResponse> = self
                        .0
                        .devices
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|d| d.revoked_at.is_none())
                        .map(|d| RecipientDeviceResponse {
                            device_id: d.device_id,
                            user_id: d.user_id,
                            device_name: d.device_name.clone(),
                            agreement_public_key: d.agreement_public_key.clone(),
                            key_fingerprint: "fp".to_string(),
                        })
                        .collect();
                    let resp = RecipientDevicesResponse {
                        vault_id: self.0.vault_id,
                        devices: devs,
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&resp).unwrap()));
                }

                Ok(HttpResponse::new(404, b"{}".to_vec()))
            }
        }

        struct FailingCredentialStore {
            inner: EphemeralCredentialStore,
            fail_set: std::sync::atomic::AtomicBool,
        }
        impl CredentialStore for FailingCredentialStore {
            fn set_secret(
                &self,
                service: &str,
                account: &str,
                secret: &str,
            ) -> Result<(), CredentialError> {
                if self.fail_set.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(CredentialError::Storage("Keychain locked".into()));
                }
                self.inner.set_secret(service, account, secret)
            }
            fn get_secret(
                &self,
                service: &str,
                account: &str,
            ) -> Result<Option<SecretString>, CredentialError> {
                self.inner.get_secret(service, account)
            }
            fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError> {
                self.inner.delete_secret(service, account)
            }
        }

        let db_file = std::env::temp_dir().join(format!(
            "shellumi_enroll_test_{}.db",
            Uuid::new_v4().as_simple()
        ));
        let mut store = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        let local_vault_id = "sv-enroll";
        store
            .create_shared_vault(
                &SharedVault::new(local_vault_id, "Platform Team", None, "owner", "1000").unwrap(),
            )
            .await
            .unwrap();

        let user_id = Uuid::new_v4();
        let remote_vault_id = Uuid::new_v4();
        let transport = Arc::new(StatefulEnrollmentTransport {
            user_id,
            username: "alice@corp.internal".into(),
            vault_id: remote_vault_id,
            server_instance_id: Uuid::new_v4(),
            expected_invite_token: "tok-valid-123".into(),
            devices: Mutex::new(Vec::new()),
            forge_wrong_user: Mutex::new(false),
            omit_from_list: Mutex::new(false),
        });
        let cred_store = FailingCredentialStore {
            inner: EphemeralCredentialStore::new(),
            fail_set: std::sync::atomic::AtomicBool::new(false),
        };

        // 1. Invitation created locally starts as InvitationPending (never Enrolled)
        let ssh_pk = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIMockKeyForAlice";
        let invited = TeamMember::new(
            "mem-alice",
            local_vault_id,
            user_id.to_string(),
            "Alice",
            ssh_pk,
            TeamRole::Editor,
            "1000",
            "owner",
        );
        assert_eq!(
            invited.enrollment_status,
            EnrollmentStatus::InvitationPending
        );
        assert!(!invited.is_enrolled());
        assert_eq!(invited.enrollment_label(), "Invitation pending");
        store.add_team_member(&invited).await.unwrap();

        // 2. Reject OpenSSH key & invalid/non-contributory X25519 keys as agreement keys
        assert!(validate_x25519_agreement_public_key(ssh_pk).is_err());
        assert!(validate_x25519_agreement_public_key(&BASE64_STANDARD.encode([0u8; 32])).is_err());
        assert!(store
            .update_team_member_enrollment(
                "mem-alice",
                EnrollmentStatus::DeviceEnrolled,
                Some(ssh_pk),
            )
            .await
            .is_err());

        // 3. Reject unauthorized enrollment (no token)
        let mut unauth_client = ShellumiServerClient::with_transport(
            "https://srv.local",
            SharedTransport(Arc::clone(&transport)),
        );
        assert!(store
            .accept_invitation_and_enroll_device(
                "conn-1",
                local_vault_id,
                "mem-alice",
                "Alice",
                ssh_pk,
                "tok-valid-123",
                "MacBook Pro",
                &mut unauth_client,
                &cred_store,
            )
            .await
            .is_err());

        // 4. Keychain write failure aborts enrollment without marking member enrolled
        let mut auth_client = ShellumiServerClient::with_transport(
            "https://srv.local",
            SharedTransport(Arc::clone(&transport)),
        );
        auth_client.set_token("jwt-alice");
        cred_store
            .fail_set
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(store
            .accept_invitation_and_enroll_device(
                "conn-1",
                local_vault_id,
                "mem-alice",
                "Alice",
                ssh_pk,
                "tok-valid-123",
                "MacBook Pro",
                &mut auth_client,
                &cred_store,
            )
            .await
            .is_err());
        assert!(!store
            .get_team_member("mem-alice")
            .await
            .unwrap()
            .unwrap()
            .is_enrolled());
        cred_store
            .fail_set
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // 5. Reject forged user binding from server
        *transport.forge_wrong_user.lock().unwrap() = true;
        assert!(store
            .accept_invitation_and_enroll_device(
                "conn-1",
                local_vault_id,
                "mem-alice",
                "Alice",
                ssh_pk,
                "tok-valid-123",
                "MacBook Pro",
                &mut auth_client,
                &cred_store,
            )
            .await
            .is_err());
        *transport.forge_wrong_user.lock().unwrap() = false;

        // 6. Complete authenticated invitation acceptance + device 1 enrollment
        let (member, dev1) = store
            .accept_invitation_and_enroll_device(
                "conn-1",
                local_vault_id,
                "mem-alice",
                "Alice",
                ssh_pk,
                "tok-valid-123",
                "MacBook Pro",
                &mut auth_client,
                &cred_store,
            )
            .await
            .expect("Authenticated enrollment must succeed");
        assert!(member.is_enrolled());
        assert_eq!(member.enrollment_label(), "Enrolled");
        assert!(dev1.verified);

        // 7. Verify public/private key correspondence: wrap DEK with registered X25519 pk, unwrap with stored private key
        let content_key = [42u8; 32];
        let (wrapped_b64, ephem_b64) =
            wrap_dek_for_recipient(&content_key, &dev1.agreement_public_key, remote_vault_id, 1)
                .unwrap();
        let priv_secret =
            resolve_secret_from_ref(&cred_store, dev1.agreement_key_ref.as_deref().unwrap())
                .unwrap()
                .unwrap();
        let priv_bytes = BASE64_STANDARD
            .decode(priv_secret.expose_secret().trim())
            .unwrap();
        let mut priv_arr = [0u8; 32];
        priv_arr.copy_from_slice(&priv_bytes);
        let unwrapped =
            unwrap_dek_for_device(&wrapped_b64, &ephem_b64, &priv_arr, remote_vault_id, 1).unwrap();
        assert_eq!(*unwrapped, content_key);

        // 8. Enroll a second device for the same user
        let dev2 = store
            .enroll_additional_device_for_member(
                "conn-1",
                local_vault_id,
                "mem-alice",
                "Workstation Linux",
                &mut auth_client,
                &cred_store,
            )
            .await
            .unwrap();
        assert_ne!(dev1.device_id, dev2.device_id);
        assert_ne!(dev1.agreement_public_key, dev2.agreement_public_key);

        let authorized = store
            .list_authorized_recipient_devices(local_vault_id)
            .await
            .unwrap();
        assert_eq!(authorized.len(), 2);

        // 9. Restart persistence: reopen HostStore from disk and verify enrollment & devices persist
        drop(store);
        let mut reopened = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        let persisted_member = reopened
            .get_team_member("mem-alice")
            .await
            .unwrap()
            .unwrap();
        assert!(persisted_member.is_enrolled());
        assert_eq!(
            reopened
                .list_authorized_recipient_devices(local_vault_id)
                .await
                .unwrap()
                .len(),
            2
        );

        // 10. Revoke device 1 -> device 2 remains active and member stays enrolled
        reopened
            .revoke_device_with_server(
                local_vault_id,
                "mem-alice",
                Uuid::parse_str(&dev1.device_id).unwrap(),
                &auth_client,
            )
            .await
            .unwrap();
        assert_eq!(
            reopened
                .list_authorized_recipient_devices(local_vault_id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(reopened
            .get_team_member("mem-alice")
            .await
            .unwrap()
            .unwrap()
            .is_enrolled());

        // 11. Revoke device 2 -> member transitions to DeviceRevoked & 0 authorized devices
        reopened
            .revoke_device_with_server(
                local_vault_id,
                "mem-alice",
                Uuid::parse_str(&dev2.device_id).unwrap(),
                &auth_client,
            )
            .await
            .unwrap();
        let after_revoke = reopened
            .get_team_member("mem-alice")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            after_revoke.enrollment_status,
            EnrollmentStatus::DeviceRevoked
        );
        assert!(!after_revoke.is_enrolled());
        assert!(reopened
            .list_authorized_recipient_devices(local_vault_id)
            .await
            .unwrap()
            .is_empty());

        // 12. Migrate an existing legacy member preserving SSH key and upgrading to DeviceEnrolled
        let legacy_ssh = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILegacyMemberKey";
        let legacy_member = TeamMember {
            id: "mem-legacy".into(),
            vault_id: local_vault_id.into(),
            user_id: user_id.to_string(),
            display_name: "Legacy Alice".into(),
            public_key: legacy_ssh.into(),
            agreement_public_key: None,
            enrollment_status: EnrollmentStatus::LegacyRequiresMigration,
            role: TeamRole::Owner,
            enrolled_at: "900".into(),
            invited_by: "owner".into(),
            revoked: false,
            revoked_at: None,
        };
        assert_eq!(
            legacy_member.enrollment_label(),
            "Legacy member (migration required)"
        );
        reopened.add_team_member(&legacy_member).await.unwrap();

        let migrated_dev = reopened
            .enroll_additional_device_for_member(
                "conn-1",
                local_vault_id,
                "mem-legacy",
                "Migrated Device",
                &mut auth_client,
                &cred_store,
            )
            .await
            .unwrap();
        let migrated_member = reopened
            .get_team_member("mem-legacy")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(migrated_member.public_key, legacy_ssh);
        assert_eq!(migrated_member.role, TeamRole::Owner);
        assert!(migrated_member.is_enrolled());
        assert_eq!(
            migrated_member.agreement_public_key.as_deref(),
            Some(migrated_dev.agreement_public_key.as_str())
        );

        let _ = std::fs::remove_file(&db_file);
        // --- Additional Issue 2 Security Checks (Invitation Binding & Per-Vault Authorization) ---
        {
            let db_file = std::env::temp_dir()
                .join(format!("shellumi-issue2-sec-{}.db", uuid::Uuid::new_v4()));
            let _ = std::fs::remove_file(&db_file);
            let mut store = HostStore::open_with_key(db_file.to_str().unwrap(), None)
                .await
                .unwrap();
            let cred_store = shellumi_core::EphemeralCredentialStore::new();
            let transport = transport.clone();

            let alice_id = transport.user_id;
            let bob_id = Uuid::new_v4();
            let vault_a = transport.vault_id;
            let vault_b = Uuid::new_v4();

            store
                .create_shared_vault(
                    &shellumi_core::SharedVault::new(
                        vault_a.to_string(),
                        "Vault A",
                        Some("desc".into()),
                        "1000",
                        "https://server.test",
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            store
                .create_shared_vault(
                    &shellumi_core::SharedVault::new(
                        vault_b.to_string(),
                        "Vault B",
                        Some("desc".into()),
                        "1000",
                        "https://server.test",
                    )
                    .unwrap(),
                )
                .await
                .unwrap();

            let ssh_key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPlaceholderTestKeyForUnitTestsOnly";

            let other_vault_mem = TeamMember::new(
                "mem-other-vault",
                vault_b.to_string(),
                alice_id.to_string(),
                "Alice",
                ssh_key,
                TeamRole::Editor,
                "1000",
                "owner",
            );
            store.add_team_member(&other_vault_mem).await.unwrap();

            let mut alice_client = ShellumiServerClient::with_transport(
                "https://server.test",
                SharedTransport(transport.clone()),
            );
            alice_client.set_token("alice-tok");

            // 1. Existing member belonging to another vault must be rejected BEFORE accept_invite
            let err = store
                .accept_invitation_and_enroll_device(
                    "conn-1",
                    &vault_a.to_string(),
                    "mem-other-vault",
                    "Alice",
                    ssh_key,
                    "tok-valid-123",
                    "Alice Mac",
                    &mut alice_client,
                    &cred_store,
                )
                .await
                .unwrap_err();
            assert!(err.to_string().contains("belongs to vault"));

            // 2. Existing member belonging to another user must be rejected
            let bob_mem = TeamMember::new(
                "mem-bob",
                vault_a.to_string(),
                bob_id.to_string(),
                "Bob",
                ssh_key,
                TeamRole::Editor,
                "1000",
                "owner",
            );
            store.add_team_member(&bob_mem).await.unwrap();
            let err = store
                .accept_invitation_and_enroll_device(
                    "conn-1",
                    &vault_a.to_string(),
                    "mem-bob",
                    "Alice",
                    ssh_key,
                    "tok-valid-123",
                    "Alice Mac",
                    &mut alice_client,
                    &cred_store,
                )
                .await
                .unwrap_err();
            assert!(err.to_string().contains("belongs to user"));

            // 3. Valid invitation for vault A succeeds
            let (alice_mem, alice_dev) = store
                .accept_invitation_and_enroll_device(
                    "conn-1",
                    &vault_a.to_string(),
                    "mem-alice-a",
                    "Alice",
                    ssh_key,
                    "tok-valid-123",
                    "Alice Mac",
                    &mut alice_client,
                    &cred_store,
                )
                .await
                .unwrap();
            assert!(alice_mem.is_enrolled());
            assert!(alice_dev.agreement_key_ref.is_some());

            // 4. authorize_device_for_vault: cross-user, multi-vault, per-vault revocation, reactivation rejection, global revocation
            let cross_user_err = store
                .authorize_device_for_vault(
                    &vault_b.to_string(),
                    &alice_dev.device_id,
                    &bob_id.to_string(),
                    "1001",
                )
                .await
                .unwrap_err();
            assert!(cross_user_err.to_string().contains("belongs to user"));

            store
                .authorize_device_for_vault(
                    &vault_b.to_string(),
                    &alice_dev.device_id,
                    &alice_id.to_string(),
                    "1002",
                )
                .await
                .unwrap();
            assert_eq!(
                store
                    .list_enrolled_devices_for_user(&vault_a.to_string(), &alice_id.to_string())
                    .await
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                store
                    .list_enrolled_devices_for_user(&vault_b.to_string(), &alice_id.to_string())
                    .await
                    .unwrap()
                    .len(),
                1
            );

            assert!(store
                .revoke_device_for_vault(&vault_a.to_string(), &alice_dev.device_id, "1003")
                .await
                .unwrap());
            let devs_a = store
                .list_enrolled_devices_for_user(&vault_a.to_string(), &alice_id.to_string())
                .await
                .unwrap();
            let devs_b = store
                .list_enrolled_devices_for_user(&vault_b.to_string(), &alice_id.to_string())
                .await
                .unwrap();
            assert!(devs_a[0].revoked);
            assert!(!devs_b[0].revoked);

            let reactivate_err = store
                .authorize_device_for_vault(
                    &vault_a.to_string(),
                    &alice_dev.device_id,
                    &alice_id.to_string(),
                    "1004",
                )
                .await
                .unwrap_err();
            assert!(reactivate_err.to_string().contains("cannot be reactivated"));

            assert!(store
                .revoke_enrolled_device(&alice_dev.device_id, "1005")
                .await
                .unwrap());
            let devs_b_after = store
                .list_enrolled_devices_for_user(&vault_b.to_string(), &alice_id.to_string())
                .await
                .unwrap();
            assert!(devs_b_after[0].revoked);

            let _ = std::fs::remove_file(&db_file);
        }
    }
}

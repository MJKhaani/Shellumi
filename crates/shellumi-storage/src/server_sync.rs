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
use ring::signature::KeyPair;
use serde::{Deserialize, Serialize};
use shellumi_core::server::{
    canonical_mutation_preimage, compute_ciphertext_digest, generate_device_ed25519_keypair_pkcs8,
    generate_device_x25519_keypair, keypair_from_pkcs8, sign_device_challenge, sign_message,
    validate_x25519_agreement_public_key, wrap_dek_for_recipient, AcceptInviteRequest,
    CommitBatchRequest, DeviceChallengeRequest, KeyEnvelopeItem, MutationItem,
    RegisterDeviceRequest, RotateKeyEpochRequest, ShellumiServerClient, SyncChangeItem,
    CURRENT_PROTOCOL_VERSION,
};
use shellumi_core::sync::{decrypt_entity, encrypt_entity, HttpTransport};
use shellumi_core::{EnrolledDevice, EnrollmentStatus, TeamMember, TeamRole, VaultId};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Authoritative server-to-vault binding resolved from the registry `VaultContext`.
/// Construction is sealed to `HostStore::resolve_verified_remote_vault_binding`.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedRemoteVaultBinding {
    local_vault_id: String,
    server_connection_id: String,
    remote_vault_id: Uuid,
    server_instance_id: Option<String>,
    base_url: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct EnrollmentScope {
    connection_id: String,
    remote_vault_id: Uuid,
    server_instance_id: String,
    #[serde(default)]
    base_url: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct InvitationCheckpoint {
    scope: EnrollmentScope,
    role: Option<TeamRole>,
    display_name: String,
    ssh_public_key: String,
    device_label: String,
}

#[derive(Serialize, Deserialize)]
struct RegistrationIntent {
    scope: EnrollmentScope,
    role: TeamRole,
    #[serde(default)]
    local_vault_id: Option<String>,
    #[serde(default)]
    account_id: Option<Uuid>,
    #[serde(default)]
    device_label: String,
    #[serde(default)]
    initialize_connection: bool,
    #[serde(default)]
    previous_connection_device: Option<String>,
    #[serde(default)]
    invitation: Option<InvitationCheckpoint>,
    key_id: Uuid,
    signing_public_key: String,
    agreement_public_key: String,
}

impl RegistrationIntent {
    fn credential_refs(&self) -> (String, String) {
        let service = VaultServerConnectionRecord::CREDENTIAL_SERVICE;
        (
            format!("keychain://{service}/{}-signing", self.key_id),
            format!("keychain://{service}/{}-agreement", self.key_id),
        )
    }

    fn matches(&self, binding: &VerifiedRemoteVaultBinding, user: Uuid) -> bool {
        binding.matches_scope(&self.scope)
            && self
                .local_vault_id
                .as_ref()
                .is_none_or(|vault| vault == &binding.local_vault_id)
            && self.account_id.is_none_or(|account| account == user)
    }
}

enum EnrollmentSource<'a> {
    Standalone,
    Invitation(&'a str, &'a InvitationCheckpoint),
    Replacement(&'a str),
    Connection(Option<&'a str>),
}

struct ReplacementOperation {
    id: String,
    old_device: Uuid,
    member_id: String,
    label: String,
    stage: String,
}

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

// A single app process can have several windows and storage handles for the same device.
static DEVICE_CHANGES: std::sync::Mutex<Vec<(Uuid, Uuid)>> = std::sync::Mutex::new(Vec::new());

struct DeviceChangeGuard((Uuid, Uuid));

impl DeviceChangeGuard {
    fn acquire(binding: &VerifiedRemoteVaultBinding, device: Uuid) -> Result<Self, StorageError> {
        let server = binding
            .server_instance_id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok())
            .filter(|id| !id.is_nil())
            .ok_or_else(|| enrollment_denied("Unknown server identity for device change"))?;
        let key = (server, device);
        let mut active = DEVICE_CHANGES
            .lock()
            .map_err(|_| enrollment_denied("Device change lock unavailable"))?;
        if active.contains(&key) {
            return Err(enrollment_denied(
                "Another change is already in progress for this device",
            ));
        }
        active.push(key);
        Ok(Self(key))
    }
}

impl Drop for DeviceChangeGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = DEVICE_CHANGES.lock() {
            active.retain(|key| *key != self.0);
        }
    }
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn enrollment_denied(message: &str) -> StorageError {
    tracing::warn!(reason = message, "Remote enrollment validation failed");
    StorageError::InvalidCollaboration(shellumi_core::CollaborationError::Unauthorized(
        message.into(),
    ))
}

fn definitive_request_rejection(error: &shellumi_core::server::ServerError) -> bool {
    use shellumi_core::server::ServerError;
    matches!(
        error,
        ServerError::Unauthorized
            | ServerError::Http(400 | 403 | 422, _)
            | ServerError::Api(400 | 403 | 422, _)
    )
}

fn enrollment_network_error(error: shellumi_core::server::ServerError) -> StorageError {
    use shellumi_core::server::ServerError;
    let reason = match error {
        ServerError::Unauthorized => "The server rejected authentication",
        ServerError::Network(_) => "The server could not be reached",
        ServerError::Deserialization(_) => "The server returned an invalid response",
        _ => "The server request failed",
    };
    tracing::warn!(reason, "Remote enrollment request failed");
    StorageError::InvalidSync(shellumi_core::SyncError::Network(reason.into()))
}

impl VerifiedRemoteVaultBinding {
    pub fn local_vault_id(&self) -> &str {
        &self.local_vault_id
    }

    pub fn server_connection_id(&self) -> &str {
        &self.server_connection_id
    }

    pub fn remote_vault_id(&self) -> Uuid {
        self.remote_vault_id
    }

    fn enrollment_scope(&self) -> Result<EnrollmentScope, StorageError> {
        Ok(EnrollmentScope {
            base_url: Some(self.base_url.clone()),
            connection_id: self.server_connection_id.clone(),
            remote_vault_id: self.remote_vault_id,
            server_instance_id: self
                .server_instance_id
                .clone()
                .ok_or_else(|| enrollment_denied("Missing server instance identity"))?,
        })
    }

    fn matches_scope(&self, scope: &EnrollmentScope) -> bool {
        scope.connection_id == self.server_connection_id
            && scope.remote_vault_id == self.remote_vault_id
            && Some(scope.server_instance_id.as_str()) == self.server_instance_id.as_deref()
            && scope
                .base_url
                .as_ref()
                .is_none_or(|url| url == &self.base_url)
    }

    /// Verifies the authenticated account and its live membership in this bound vault.
    pub fn authenticate_vault_member<T: HttpTransport>(
        &self,
        store: &HostStore,
        client: &ShellumiServerClient<T>,
    ) -> Result<Uuid, StorageError> {
        let user = self.authenticate(store, client)?;
        self.verify_membership(client, user, None)?;
        Ok(user)
    }

    fn authenticate<T: HttpTransport>(
        &self,
        store: &HostStore,
        client: &ShellumiServerClient<T>,
    ) -> Result<Uuid, StorageError> {
        if store.vault_id().as_str() != self.local_vault_id
            || self.remote_vault_id.is_nil()
            || self.base_url.trim_end_matches('/') != client.base_url().trim_end_matches('/')
            || client.token().is_none_or(|token| token.trim().is_empty())
        {
            return Err(enrollment_denied(
                "Invalid vault scope, connection, or authenticated session",
            ));
        }
        let expected_instance = self
            .server_instance_id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok())
            .filter(|id| !id.is_nil())
            .ok_or_else(|| enrollment_denied("Missing or invalid server instance identity"))?;
        if client
            .capabilities()
            .map_err(enrollment_network_error)?
            .server_instance_id
            != expected_instance
        {
            return Err(enrollment_denied(
                "Server instance identity differs from registry",
            ));
        }
        let user = client.me().map_err(enrollment_network_error)?.user_id;
        if user.is_nil() {
            return Err(enrollment_denied("Invalid authenticated user identity"));
        }
        Ok(user)
    }

    fn verify_membership<T: HttpTransport>(
        &self,
        client: &ShellumiServerClient<T>,
        user: Uuid,
        role: Option<TeamRole>,
    ) -> Result<TeamRole, StorageError> {
        let members = client
            .list_members(self.remote_vault_id)
            .map_err(enrollment_network_error)?;
        let mut matches = members
            .members
            .iter()
            .filter(|member| member.user_id == user);
        let assigned = matches
            .next()
            .filter(|member| member.vault_id == self.remote_vault_id)
            .and_then(|member| member.role.parse::<TeamRole>().ok())
            .filter(|assigned| role.is_none_or(|expected| expected == *assigned));
        match (assigned, matches.next()) {
            (Some(assigned), None) => Ok(assigned),
            _ => Err(enrollment_denied(
                "Missing or inconsistent authoritative vault membership",
            )),
        }
    }
}

impl HostStore {
    /// Resolves and validates the authoritative `VaultContext` binding for a remote vault.
    /// Rejects missing, local-only, or mismatched server connection contexts.
    pub async fn resolve_verified_remote_vault_binding(
        &self,
        local_vault_id: &str,
        expected_connection_id: Option<&str>,
    ) -> Result<VerifiedRemoteVaultBinding, StorageError> {
        let ctx = self
            .get_vault_context(local_vault_id)
            .await?
            .ok_or_else(|| {
                StorageError::InvalidCollaboration(
                    shellumi_core::CollaborationError::Unauthorized(format!(
                        "Vault context '{local_vault_id}' not found; refusing unverified remote operation"
                    )),
                )
            })?;
        let (server_conn_id, remote_vault_id) = match ctx.location() {
            shellumi_core::VaultLocation::Remote {
                server,
                server_vault_id,
            } => {
                if let Some(expected) = expected_connection_id {
                    if server.as_str() != expected {
                        return Err(StorageError::InvalidCollaboration(
                            shellumi_core::CollaborationError::Unauthorized(format!(
                                "Vault '{local_vault_id}' is bound to connection '{}', not '{expected}'",
                                server.as_str()
                            )),
                        ));
                    }
                }
                let remote_uuid = Uuid::parse_str(server_vault_id.as_str()).map_err(|_| {
                    StorageError::InvalidCollaboration(
                        shellumi_core::CollaborationError::Unauthorized(format!(
                            "Vault '{local_vault_id}' has non-UUID remote vault ID '{}'",
                            server_vault_id.as_str()
                        )),
                    )
                })?;
                (server.as_str().to_string(), remote_uuid)
            }
            shellumi_core::VaultLocation::Local => {
                return Err(StorageError::InvalidCollaboration(
                    shellumi_core::CollaborationError::Unauthorized(format!(
                        "Vault '{local_vault_id}' is local-only and cannot perform remote enrollment"
                    )),
                ));
            }
        };
        let conn = self
            .get_vault_server_connection(&server_conn_id)
            .await?
            .ok_or_else(|| enrollment_denied("Remote vault has no registered server connection"))?;
        Ok(VerifiedRemoteVaultBinding {
            local_vault_id: local_vault_id.to_string(),
            server_connection_id: server_conn_id,
            remote_vault_id,
            server_instance_id: conn.server_instance_id,
            base_url: conn.base_url,
        })
    }
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

    /// Enrolls the authenticated account's unique active member in this remote vault.
    pub async fn enroll_device_with_server<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        client: &mut ShellumiServerClient<T>,
        device_label: &str,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<EnrolledDevice, StorageError> {
        let user = binding.authenticate(self, client)?;
        let role = binding.verify_membership(client, user, None)?;
        let user_id = user.to_string();
        let mut members = self
            .list_team_members(&binding.local_vault_id)
            .await?
            .into_iter()
            .filter(|member| member.user_id == user_id && !member.revoked);
        let member = members
            .next()
            .filter(|_| members.next().is_none())
            .ok_or_else(|| {
                enrollment_denied(
                    "Use the Team Inspector to accept an invitation for this account and vault",
                )
            })?;
        self.enroll_device_for_authenticated_user(
            binding,
            &member.id,
            user,
            role,
            EnrollmentSource::Standalone,
            device_label,
            client,
            credentials,
        )
        .await
    }

    /// Initializes the device identity of a newly staged connection in the encrypted registry.
    pub async fn enroll_initial_connection_device<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        client: &mut ShellumiServerClient<T>,
        vault_name: &str,
        device_label: &str,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<EnrolledDevice, StorageError> {
        self.enroll_connection_device(binding, client, vault_name, device_label, credentials, None)
            .await
    }

    /// Journals a device enrollment and atomically attaches its keys to the verified connection.
    pub async fn enroll_connection_device<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        client: &mut ShellumiServerClient<T>,
        vault_name: &str,
        device_label: &str,
        credentials: &dyn shellumi_core::CredentialStore,
        previous_device: Option<&str>,
    ) -> Result<EnrolledDevice, StorageError> {
        let _change = self.lock_device_journal(".device-change.lock")?;
        let user = binding.authenticate(self, client)?;
        let role = binding.verify_membership(client, user, None)?;
        self.verify_connection_initialization(binding, client, credentials, previous_device)
            .await?;
        let member_id = format!("connection-owner:{}:{user}", binding.local_vault_id);
        let now = current_timestamp().to_string();
        if self
            .get_shared_vault(&binding.local_vault_id)
            .await?
            .is_none()
        {
            self.create_shared_vault(
                &shellumi_core::SharedVault::new(
                    &binding.local_vault_id,
                    vault_name,
                    None,
                    user.to_string(),
                    &now,
                )
                .map_err(StorageError::InvalidCollaboration)?,
            )
            .await?;
        }
        if self.get_team_member(&member_id).await?.is_none() {
            self.add_team_member(&TeamMember::new(
                &member_id,
                &binding.local_vault_id,
                user.to_string(),
                user.to_string(),
                "",
                role,
                now,
                user.to_string(),
            ))
            .await?;
        }
        self.enroll_device_for_authenticated_user(
            binding,
            &member_id,
            user,
            role,
            EnrollmentSource::Connection(previous_device),
            device_label,
            client,
            credentials,
        )
        .await
    }

    async fn verify_connection_initialization<T: HttpTransport>(
        &self,
        binding: &VerifiedRemoteVaultBinding,
        client: &ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
        previous_device: Option<&str>,
    ) -> Result<String, StorageError> {
        let conn = self
            .get_vault_server_connection(&binding.server_connection_id)
            .await?
            .filter(|conn| {
                conn.device_id.as_deref() == previous_device
                    && (previous_device.is_some()
                        || (conn.device_signing_key.is_none()
                            && conn.device_agreement_key.is_none()))
            })
            .ok_or_else(|| {
                enrollment_denied("Connection already has device credentials or is unavailable")
            })?;
        let token = conn
            .resolve_auth_token(credentials)
            .ok_or_else(|| enrollment_denied("Protected connection session is unavailable"))?;
        if client.token() != Some(token.expose_secret()) {
            return Err(enrollment_denied(
                "Connection enrollment authentication changed",
            ));
        }
        conn.auth_token
            .filter(|reference| {
                reference.starts_with("keychain://") || reference.starts_with("env://")
            })
            .ok_or_else(|| {
                enrollment_denied("Connection session does not have a protected reference")
            })
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
    pub async fn accept_invitation_and_enroll_device_with_binding<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        member_id: &str,
        display_name: &str,
        ssh_public_key: &str,
        invite_token: &str,
        device_label: &str,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<(TeamMember, EnrolledDevice), StorageError> {
        let me = binding.authenticate(self, client)?;
        let local_vault_id = binding.local_vault_id.as_str();
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
            if m.user_id != me.to_string() {
                return Err(StorageError::InvalidCollaboration(
                    shellumi_core::CollaborationError::Unauthorized(format!(
                        "Member '{member_id}' belongs to user '{}', not authenticated user '{}'",
                        m.user_id, me
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

        let op_id = Uuid::new_v4().to_string();
        let op_ts = current_timestamp().to_string();
        let user_id = me.to_string();
        let mut checkpoint = InvitationCheckpoint {
            scope: binding.enrollment_scope()?,
            role: None,
            display_name: display_name.into(),
            ssh_public_key: ssh_public_key.into(),
            device_label: device_label.into(),
        };
        let encoded_checkpoint = serde_json::to_string(&checkpoint)
            .map_err(|_| enrollment_denied("Unable to encode invitation checkpoint"))?;
        let written = self.connection.execute(
            "INSERT INTO enrollment_operations (operation_id, vault_id, user_id, member_id, stage, error_message, updated_at, registration_intent)
             VALUES (?1, ?2, ?3, ?4, 'InvitationAccepting', 'Invitation outcome pending', ?5, ?6)
             ON CONFLICT DO NOTHING",
            (op_id.as_str(), local_vault_id, user_id.as_str(), member_id, op_ts.as_str(), encoded_checkpoint.as_str()),
        ).await?;
        if written != 1 {
            return Err(enrollment_denied(
                "An enrollment operation already exists. Resume Enrollment before retrying.",
            ));
        }

        let accept_resp = match client.accept_invite(&AcceptInviteRequest {
            invite_token: invite_token.to_owned(),
        }) {
            Ok(response) => response,
            Err(error) => {
                if definitive_request_rejection(&error) {
                    self.connection.execute(
                        "UPDATE enrollment_operations SET stage = 'Canceled', error_message = 'Invitation rejected'
                         WHERE operation_id = ?1 AND stage = 'InvitationAccepting' AND registration_intent = ?2",
                        (op_id.as_str(), encoded_checkpoint.as_str()),
                    ).await?;
                    return Err(enrollment_denied("The server rejected the invitation"));
                }
                return Err(enrollment_denied(
                    "Invitation acceptance needs manual reconciliation. Confirm the invitation with the vault administrator before proceeding.",
                ));
            }
        };

        if accept_resp.vault_id != binding.remote_vault_id {
            return Err(enrollment_denied(
                "Accepted invitation belongs to a different remote vault",
            ));
        }
        let role = accept_resp
            .role
            .parse::<TeamRole>()
            .map_err(|_| enrollment_denied("Server returned an invalid invitation role"))?;
        checkpoint.role = Some(role);
        let verified_checkpoint = serde_json::to_string(&checkpoint)
            .map_err(|_| enrollment_denied("Unable to encode verified invitation checkpoint"))?;
        let updated = self
            .connection
            .execute(
                "UPDATE enrollment_operations SET stage = 'InvitationAcceptedBound',
             registration_intent = ?1, error_message = 'Device registration pending'
             WHERE operation_id = ?2 AND vault_id = ?3 AND user_id = ?4 AND member_id = ?5
                AND stage = 'InvitationAccepting' AND registration_intent = ?6",
                (
                    verified_checkpoint.as_str(),
                    op_id.as_str(),
                    local_vault_id,
                    user_id.as_str(),
                    member_id,
                    encoded_checkpoint.as_str(),
                ),
            )
            .await?;
        if updated != 1 {
            return Err(enrollment_denied(
                "Invitation checkpoint changed during validation",
            ));
        }
        binding.verify_membership(client, me, Some(role))?;
        self.ensure_invited_member(binding, member_id, me, role, &checkpoint)
            .await?;
        let device = self
            .enroll_device_for_authenticated_user(
                binding,
                member_id,
                me,
                role,
                EnrollmentSource::Invitation(&op_id, &checkpoint),
                device_label,
                client,
                credentials,
            )
            .await?;
        let member = self
            .get_team_member(member_id)
            .await?
            .ok_or_else(|| enrollment_denied("Authorized member record no longer exists"))?;

        Ok((member, device))
    }

    async fn ensure_invited_member(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        member_id: &str,
        user: Uuid,
        role: TeamRole,
        checkpoint: &InvitationCheckpoint,
    ) -> Result<(), StorageError> {
        if self.get_team_member(member_id).await?.is_none() {
            self.add_team_member(&TeamMember::new(
                member_id,
                &binding.local_vault_id,
                user.to_string(),
                &checkpoint.display_name,
                &checkpoint.ssh_public_key,
                role,
                current_timestamp().to_string(),
                "server_invite",
            ))
            .await?;
        }
        Ok(())
    }

    /// Enrolls an additional authenticated device for an existing user/member.
    #[allow(clippy::too_many_arguments)]
    pub async fn enroll_additional_device_for_member_with_binding<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        member_id: &str,
        device_label: &str,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<EnrolledDevice, StorageError> {
        let me = binding.authenticate(self, client)?;
        let local_vault_id = binding.local_vault_id.as_str();
        let member = self.get_team_member(member_id).await?.ok_or_else(|| {
            StorageError::ItemNotFound(format!("Team member '{member_id}' not found"))
        })?;
        if member.vault_id != local_vault_id {
            return Err(StorageError::InvalidCollaboration(
                shellumi_core::CollaborationError::Unauthorized(format!(
                    "Team member '{member_id}' belongs to vault '{}', not '{local_vault_id}'",
                    member.vault_id
                )),
            ));
        }
        if member.revoked {
            return Err(StorageError::InvalidCollaboration(
                shellumi_core::CollaborationError::Unauthorized(
                    "Cannot enroll a device for a revoked team member".into(),
                ),
            ));
        }

        if me.to_string() != member.user_id {
            return Err(enrollment_denied(
                "Authenticated server user does not match target member identity",
            ));
        }
        let role = binding.verify_membership(client, me, None)?;
        self.enroll_device_for_authenticated_user(
            binding,
            member_id,
            me,
            role,
            EnrollmentSource::Standalone,
            device_label,
            client,
            credentials,
        )
        .await
    }

    /// Replaces a device through a journal shared by all processes opening this database.
    #[allow(clippy::too_many_arguments)]
    pub async fn replace_device_keys_for_member<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        member_id: &str,
        old_device_id: Uuid,
        new_device_label: &str,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
        registry: Option<&HostStore>,
    ) -> Result<EnrolledDevice, StorageError> {
        let _journal = registry
            .unwrap_or(self)
            .lock_device_journal(".device-change.lock")?;
        let _change = DeviceChangeGuard::acquire(binding, old_device_id)?;
        let scope = binding.enrollment_scope()?;
        let user = binding.authenticate(self, client)?;
        let existing = registry
            .unwrap_or(self)
            .pending_replacements(binding, user)
            .await?
            .into_iter()
            .find(|op| op.old_device == old_device_id);
        let op = if let Some(op) = existing.filter(|op| op.member_id == member_id) {
            op
        } else {
            self.verify_device_revocation(binding, member_id, old_device_id, client)
                .await?;
            let op = ReplacementOperation {
                id: Uuid::new_v4().to_string(),
                old_device: old_device_id,
                member_id: member_id.into(),
                label: new_device_label.into(),
                stage: "OwnershipVerified".into(),
            };
            let changed = registry
                .unwrap_or(self)
                .connection
                .execute(
                    "INSERT INTO device_replacements
                  (operation_id, server_instance_id, old_device_id, local_vault_id, remote_vault_id,
                   connection_id, account_id, member_id, device_label, stage, base_url)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'OwnershipVerified', ?10
                 WHERE NOT EXISTS (
                    SELECT 1 FROM device_replacements
                    WHERE server_instance_id = ?2
                      AND (replacement_device_id = ?3 OR (account_id = ?7 AND replacement_device_id IS NULL))
                      AND stage NOT IN ('Completed', 'Canceled')
                 ) ON CONFLICT DO NOTHING",
                    (
                        op.id.as_str(),
                        scope.server_instance_id.as_str(),
                        old_device_id.to_string(),
                        binding.local_vault_id.as_str(),
                        scope.remote_vault_id.to_string(),
                        scope.connection_id.as_str(),
                        user.to_string(),
                        member_id,
                        new_device_label,
                        binding.base_url.as_str(),
                    ),
                )
                .await?;
            if changed != 1 {
                return Err(enrollment_denied(
                    "A replacement already exists for this server device",
                ));
            }
            tracing::debug!(operation_id = %op.id, "Replacement intent and initial ownership verification committed");
            op
        };
        self.advance_replacement(binding, user, op, true, client, credentials, registry)
            .await
    }

    /// Reports unresolved replacements, including ones whose enrollment is already complete.
    pub async fn has_pending_device_replacements(
        &self,
        vault_id: &str,
    ) -> Result<bool, StorageError> {
        let mut rows = self
            .connection
            .query(
                "SELECT 1 FROM device_replacements WHERE local_vault_id = ?1
             AND stage NOT IN ('Completed', 'Canceled') LIMIT 1",
                (vault_id,),
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    async fn pending_replacements(
        &self,
        binding: &VerifiedRemoteVaultBinding,
        user: Uuid,
    ) -> Result<Vec<ReplacementOperation>, StorageError> {
        let scope = binding.enrollment_scope()?;
        let mut rows = self
            .connection
            .query(
                "SELECT operation_id, old_device_id, member_id, device_label, stage
             FROM device_replacements WHERE local_vault_id = ?1 AND remote_vault_id = ?2
               AND connection_id = ?3 AND server_instance_id = ?4 AND account_id = ?5 AND base_url = ?6
               AND stage NOT IN ('Completed', 'Canceled')",
                (
                    binding.local_vault_id.as_str(),
                    scope.remote_vault_id.to_string(),
                    scope.connection_id.as_str(),
                    scope.server_instance_id.as_str(),
                    user.to_string(),
                    binding.base_url.as_str(),
                ),
            )
            .await?;
        let mut pending = Vec::new();
        while let Some(row) = rows.next().await? {
            pending.push(ReplacementOperation {
                id: row.get(0)?,
                old_device: Uuid::parse_str(&row.get::<String>(1)?)
                    .map_err(|_| enrollment_denied("Invalid old device in replacement journal"))?,
                member_id: row.get(2)?,
                label: row.get(3)?,
                stage: row.get(4)?,
            });
        }
        Ok(pending)
    }

    async fn change_replacement_stage(
        &self,
        operation: &mut ReplacementOperation,
        stage: &str,
    ) -> Result<(), StorageError> {
        let changed = self
            .connection
            .execute(
                "UPDATE device_replacements SET stage = ?1 WHERE operation_id = ?2 AND stage = ?3",
                (stage, operation.id.as_str(), operation.stage.as_str()),
            )
            .await?;
        if changed != 1 {
            return Err(enrollment_denied(
                "Replacement checkpoint changed concurrently",
            ));
        }
        tracing::debug!(operation_id = %operation.id, from = %operation.stage, to = stage, "Replacement checkpoint committed");
        operation.stage = stage.into();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn advance_replacement<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        user: Uuid,
        mut operation: ReplacementOperation,
        explicit_retry: bool,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
        registry: Option<&HostStore>,
    ) -> Result<EnrolledDevice, StorageError> {
        let old_id = operation.old_device.to_string();
        let mut rows = self
            .connection
            .query(
                "SELECT stage, device_id, registration_intent FROM enrollment_operations
             WHERE operation_id = ?1 AND vault_id = ?2 AND user_id = ?3 AND member_id = ?4",
                (
                    operation.id.as_str(),
                    binding.local_vault_id.as_str(),
                    user.to_string(),
                    operation.member_id.as_str(),
                ),
            )
            .await?;
        let checkpoint = rows.next().await?;
        drop(rows);
        if checkpoint.is_none() && operation.stage == "OwnershipVerified" {
            self.verify_device_revocation(
                binding,
                &operation.member_id,
                operation.old_device,
                client,
            )
            .await?;
            let role = binding.verify_membership(client, user, None)?;
            self.enroll_device_for_authenticated_user(
                binding,
                &operation.member_id,
                user,
                role,
                EnrollmentSource::Replacement(&operation.id),
                &operation.label,
                client,
                credentials,
            )
            .await?;
            // Re-enter from the durable enrollment checkpoint.
            return Box::pin(self.advance_replacement(
                binding,
                user,
                operation,
                explicit_retry,
                client,
                credentials,
                registry,
            ))
            .await;
        }
        let checkpoint = checkpoint
            .ok_or_else(|| enrollment_denied("Replacement enrollment journal is missing"))?;
        let stage: String = checkpoint.get(0)?;
        if stage == "Canceled" && operation.stage == "OwnershipVerified" {
            registry
                .unwrap_or(self)
                .change_replacement_stage(&mut operation, "Canceled")
                .await?;
            return Err(enrollment_denied(
                "Replacement enrollment was canceled. The old device is still authorized.",
            ));
        }
        if stage != "Completed" {
            return Err(enrollment_denied(
                "Resume Enrollment to reconcile the original replacement device",
            ));
        }
        let new_id = checkpoint
            .get::<Option<String>>(1)?
            .and_then(|id| Uuid::parse_str(&id).ok())
            .filter(|id| *id != operation.old_device && !id.is_nil())
            .ok_or_else(|| enrollment_denied("Invalid replacement device identity"))?;
        let tracked = registry.unwrap_or(self).connection.execute(
            "UPDATE device_replacements SET replacement_device_id = ?1
             WHERE operation_id = ?2 AND (replacement_device_id IS NULL OR replacement_device_id = ?1)",
            (new_id.to_string(), operation.id.as_str()),
        ).await?;
        if tracked != 1 {
            return Err(enrollment_denied(
                "Replacement device conflicts with its durable checkpoint",
            ));
        }
        let intent = checkpoint
            .get::<Option<String>>(2)?
            .and_then(|raw| serde_json::from_str::<RegistrationIntent>(&raw).ok())
            .filter(|intent| intent.matches(binding, user))
            .ok_or_else(|| {
                enrollment_denied("Replacement enrollment belongs to a different scope")
            })?;
        let devices = self
            .list_enrolled_devices_for_user(&binding.local_vault_id, &user.to_string())
            .await?;
        let (signing_ref, agreement_ref) = intent.credential_refs();
        let old_local = devices.iter().find(|device| device.device_id == old_id);
        let replacement = devices
            .iter()
            .find(|device| {
                device.device_id == new_id.to_string()
                    && device.signing_key_ref.as_deref() == Some(signing_ref.as_str())
                    && device.agreement_key_ref.as_deref() == Some(agreement_ref.as_str())
                    && device.signing_public_key == intent.signing_public_key
                    && device.agreement_public_key == intent.agreement_public_key
            })
            .ok_or_else(|| {
                enrollment_denied(
                    "Replacement credentials are not attached to the authorized device",
                )
            })?;

        if matches!(
            operation.stage.as_str(),
            "OwnershipVerified" | "ReplacementVerified"
        ) || (explicit_retry
            && matches!(
                operation.stage.as_str(),
                "RevocationDispatched" | "RevocationRejected"
            ))
        {
            Self::registration_credentials(&intent, credentials)?;
        }
        if operation.stage == "OwnershipVerified" {
            self.verify_device_revocation(binding, &operation.member_id, new_id, client)
                .await?;
            registry
                .unwrap_or(self)
                .change_replacement_stage(&mut operation, "ReplacementVerified")
                .await?;
        }
        if matches!(
            operation.stage.as_str(),
            "RevocationDispatched" | "RevocationRejected"
        ) {
            binding.verify_membership(client, user, None)?;
            let devices = client.list_devices().map_err(enrollment_network_error)?;
            let mut old = devices
                .iter()
                .filter(|device| device.device_id == operation.old_device);
            let revoked = old.next().is_some_and(|device| {
                device.user_id == user
                    && device.revoked_at.is_some()
                    && old_local.is_some_and(|local| {
                        local.signing_public_key == device.signing_public_key
                            && local.agreement_public_key == device.agreement_public_key
                    })
            }) && old.next().is_none();
            if revoked {
                registry
                    .unwrap_or(self)
                    .change_replacement_stage(&mut operation, "RevocationConfirmed")
                    .await?;
            } else if !explicit_retry && operation.stage == "RevocationDispatched" {
                return Err(enrollment_denied("Old-device revocation needs reconciliation. Use Replace Keys for the same device to recheck ownership and retry."));
            }
        }
        if matches!(
            operation.stage.as_str(),
            "ReplacementVerified" | "RevocationDispatched"
        ) {
            self.verify_device_revocation(binding, &operation.member_id, new_id, client)
                .await?;
            self.verify_device_revocation(
                binding,
                &operation.member_id,
                operation.old_device,
                client,
            )
            .await?;
            Self::verify_replacement_vault_access(
                binding,
                new_id,
                &agreement_ref,
                client,
                credentials,
            )?;
            registry
                .unwrap_or(self)
                .handoff_replaced_connection(
                    binding,
                    &old_id,
                    replacement,
                    client,
                    credentials,
                    registry.is_some(),
                )
                .await?;
            registry
                .unwrap_or(self)
                .change_replacement_stage(&mut operation, "RevocationDispatched")
                .await?;
            if let Err(error) = client.revoke_device(operation.old_device) {
                if definitive_request_rejection(&error) {
                    registry
                        .unwrap_or(self)
                        .change_replacement_stage(&mut operation, "RevocationRejected")
                        .await?;
                }
                return Err(enrollment_network_error(error));
            }
            registry
                .unwrap_or(self)
                .change_replacement_stage(&mut operation, "RevocationConfirmed")
                .await?;
        }
        if operation.stage == "RevocationRejected" {
            if !explicit_retry {
                return Err(enrollment_denied("The server rejected old-device revocation. Use Replace Keys for the same device to retry after resolving the rejection."));
            }
            registry
                .unwrap_or(self)
                .change_replacement_stage(&mut operation, "ReplacementVerified")
                .await?;
            return Box::pin(self.advance_replacement(
                binding,
                user,
                operation,
                false,
                client,
                credentials,
                registry,
            ))
            .await;
        }
        if operation.stage != "RevocationConfirmed" {
            return Err(enrollment_denied("Unsupported replacement checkpoint"));
        }
        // The registry may be a different database. Mark it complete only after the
        // vault write commits. If the registry write fails, the vault update is idempotent.
        self.connection
            .execute(
                "UPDATE enrolled_devices SET revoked = 1, revoked_at = ?1
             WHERE device_id = ?2 AND user_id = ?3 AND revoked = 0",
                (current_timestamp().to_string(), old_id, user.to_string()),
            )
            .await?;
        registry
            .unwrap_or(self)
            .change_replacement_stage(&mut operation, "Completed")
            .await?;
        Ok(replacement.clone())
    }

    fn verify_replacement_vault_access<T: HttpTransport>(
        binding: &VerifiedRemoteVaultBinding,
        device_id: Uuid,
        agreement_ref: &str,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<(), StorageError> {
        let previous = client.device_id();
        client.set_device_id(device_id);
        let result = (|| {
            let vault = client
                .get_vault(binding.remote_vault_id)
                .map_err(enrollment_network_error)?;
            if vault.vault_id != binding.remote_vault_id || vault.active_key_epoch == 0 {
                return Err(enrollment_denied(
                    "Replacement vault metadata is inconsistent",
                ));
            }
            if vault.vault_type == "personal" && vault.active_key_epoch == 1 {
                return Ok(());
            }

            let envelopes = client
                .list_key_envelopes(binding.remote_vault_id)
                .map_err(enrollment_network_error)?;
            let mut matches = envelopes.envelopes.iter().filter(|envelope| {
                envelope.vault_id == binding.remote_vault_id
                    && envelope.recipient_device_id == device_id
                    && envelope.epoch == vault.active_key_epoch
            });
            let envelope = matches.next().filter(|_| matches.next().is_none())
                .ok_or_else(|| enrollment_denied("The replacement device needs an unambiguous current vault key envelope before retiring the old device"))?;
            let ephemeral_key =
                envelope
                    .key_agreement_ephemeral_pk
                    .as_deref()
                    .ok_or_else(|| {
                        enrollment_denied("The replacement vault key envelope is invalid")
                    })?;
            let agreement = shellumi_core::resolve_secret_from_ref(credentials, agreement_ref)
                .map_err(StorageError::InvalidCredential)?
                .ok_or_else(|| enrollment_denied("Replacement agreement key is unavailable"))?;
            shellumi_core::server::crypto::verify_device_key_envelope(
                &envelope.wrapped_dek,
                ephemeral_key,
                agreement.expose_secret(),
                binding.remote_vault_id,
                vault.active_key_epoch,
            )
            .map_err(|_| {
                enrollment_denied("The replacement cannot decrypt the current vault key envelope")
            })
        })();
        if let Some(previous) = previous {
            client.set_device_id(previous);
        } else {
            client.clear_device_id();
        }
        result
    }

    async fn handoff_replaced_connection<T: HttpTransport>(
        &self,
        binding: &VerifiedRemoteVaultBinding,
        old_id: &str,
        replacement: &EnrolledDevice,
        client: &ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
        require_registry: bool,
    ) -> Result<(), StorageError> {
        let Some(connection) = self
            .get_vault_server_connection(binding.server_connection_id())
            .await?
        else {
            if require_registry {
                return Err(enrollment_denied(
                    "Server connection was removed during replacement",
                ));
            }
            return Ok(());
        };
        let current_binding = self
            .resolve_verified_remote_vault_binding(&binding.local_vault_id, None)
            .await?;
        if current_binding != *binding
            || connection
                .resolve_auth_token(credentials)
                .as_ref()
                .map(shellumi_core::SecretString::expose_secret)
                != client.token()
        {
            return Err(enrollment_denied(
                "The authenticated connection changed during replacement",
            ));
        }
        if connection.device_id.as_deref() != Some(old_id) {
            return Ok(());
        }
        let changed = self
            .connection
            .execute(
                "UPDATE vault_server_connections
             SET device_id = ?1, device_signing_key = ?2, device_agreement_key = ?3, updated_at = ?4
             WHERE id = ?5 AND device_id = ?6 AND auth_token IS ?7 AND server_instance_id IS ?8
               AND base_url = ?9",
                (
                    replacement.device_id.as_str(),
                    replacement.signing_key_ref.as_deref(),
                    replacement.agreement_key_ref.as_deref(),
                    current_timestamp(),
                    connection.id.as_str(),
                    old_id,
                    connection.auth_token.as_deref(),
                    connection.server_instance_id.as_deref(),
                    connection.base_url.as_str(),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(enrollment_denied(
                "Server connection changed before device handoff",
            ));
        }
        tracing::debug!(connection_id = %connection.id, "Promoted verified replacement for the server connection");
        Ok(())
    }

    async fn verify_device_revocation<T: HttpTransport>(
        &self,
        binding: &VerifiedRemoteVaultBinding,
        member_id: &str,
        old_device_id: Uuid,
        client: &ShellumiServerClient<T>,
    ) -> Result<TeamMember, StorageError> {
        let user = binding.authenticate(self, client)?;
        binding.verify_membership(client, user, None)?;
        let user_id = user.to_string();
        let old_id = old_device_id.to_string();
        let member = self
            .get_team_member(member_id)
            .await?
            .filter(|member| {
                member.vault_id == binding.local_vault_id
                    && member.user_id == user_id
                    && !member.revoked
            })
            .ok_or_else(|| enrollment_denied("Device owner is not an active local member"))?;
        let local_devices = self
            .list_enrolled_devices_for_user(&binding.local_vault_id, &member.user_id)
            .await?;
        let old = local_devices
            .iter()
            .find(|device| device.device_id == old_id && device.verified && !device.revoked)
            .ok_or_else(|| {
                enrollment_denied("Old device has no active local vault authorization")
            })?;
        let devices = client.list_devices().map_err(enrollment_network_error)?;
        let recipients = client
            .list_recipient_devices(binding.remote_vault_id)
            .map_err(enrollment_network_error)?;
        let mut registered = devices
            .iter()
            .filter(|device| device.device_id == old_device_id);
        let mut authorized = recipients
            .devices
            .iter()
            .filter(|device| device.device_id == old_device_id);
        if old_device_id.is_nil()
            || recipients.vault_id != binding.remote_vault_id
            || !registered.next().is_some_and(|device| {
                device.user_id == user
                    && device.revoked_at.is_none()
                    && device.agreement_public_key == old.agreement_public_key
                    && !old.signing_public_key.is_empty()
                    && device.signing_public_key == old.signing_public_key
            })
            || registered.next().is_some()
            || !authorized.next().is_some_and(|device| {
                device.user_id == user && device.agreement_public_key == old.agreement_public_key
            })
            || authorized.next().is_some()
        {
            return Err(enrollment_denied(
                "Old device has no unambiguous active remote vault authorization",
            ));
        }
        Ok(member)
    }

    /// Revokes the authenticated user's device globally, after verifying its vault authorization.
    pub async fn revoke_device_with_server<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        member_id: &str,
        device_id: Uuid,
        client: &ShellumiServerClient<T>,
    ) -> Result<(), StorageError> {
        self.revoke_device_with_server_and_registry(binding, member_id, device_id, client, None)
            .await
    }

    /// Uses the shared registry to serialize global device revocation with replacement.
    pub async fn revoke_device_with_server_and_registry<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        member_id: &str,
        device_id: Uuid,
        client: &ShellumiServerClient<T>,
        registry: Option<&HostStore>,
    ) -> Result<(), StorageError> {
        let _journal = registry
            .unwrap_or(self)
            .lock_device_journal(".device-change.lock")?;
        let _change = DeviceChangeGuard::acquire(binding, device_id)?;
        let member = self
            .verify_device_revocation(binding, member_id, device_id, client)
            .await?;
        let scope = binding.enrollment_scope()?;
        let mut pending = self
            .connection
            .query(
                "SELECT 1 FROM device_replacements
             WHERE server_instance_id = ?1
               AND (old_device_id = ?2 OR replacement_device_id = ?2
                    OR (account_id = ?3 AND replacement_device_id IS NULL))
               AND stage NOT IN ('Completed', 'Canceled')",
                (
                    scope.server_instance_id,
                    device_id.to_string(),
                    member.user_id.as_str(),
                ),
            )
            .await?;
        if pending.next().await?.is_some() {
            return Err(enrollment_denied(
                "A replacement operation owns this device change",
            ));
        }
        drop(pending);
        let local_vault_id = &binding.local_vault_id;
        client
            .revoke_device(device_id)
            .map_err(enrollment_network_error)?;
        let now_str = current_timestamp().to_string();
        let _ = self
            .revoke_enrolled_device(&device_id.to_string(), &now_str)
            .await?;

        let remaining = self
            .list_enrolled_devices_for_user(local_vault_id, &member.user_id)
            .await?;
        let has_active = remaining.iter().any(|d| !d.revoked && d.verified);
        if !has_active {
            self.update_team_member_enrollment(member_id, EnrollmentStatus::DeviceRevoked, None)
                .await?;
        }
        Ok(())
    }

    /// Synchronizes local `team_members` and `enrolled_devices` with authoritative server state.
    pub async fn sync_team_enrollment_from_server<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        client: &ShellumiServerClient<T>,
    ) -> Result<(), StorageError> {
        binding.authenticate(self, client)?;
        let local_vault_id = binding.local_vault_id.as_str();
        let remote_vault_id = binding.remote_vault_id;
        let pending = self
            .list_incomplete_enrollment_operations(local_vault_id)
            .await?;
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
            if pending
                .iter()
                .any(|op| op.user_id == rd.user_id.to_string())
                || !server_members.members.iter().any(|member| {
                    member.vault_id == remote_vault_id
                        && member.user_id == rd.user_id
                        && member.role.parse::<TeamRole>().is_ok()
                })
            {
                continue;
            }
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

        // Revoke per-vault authorization for local devices absent from this vault's server recipient list
        let active_server_ids: std::collections::HashSet<String> = recipient_devices
            .devices
            .iter()
            .map(|d| d.device_id.to_string())
            .collect();
        for local_dev in self.list_enrolled_devices_for_vault(local_vault_id).await? {
            if !local_dev.revoked && !active_server_ids.contains(&local_dev.device_id) {
                let _ = self
                    .revoke_device_for_vault(local_vault_id, &local_dev.device_id, &now_str)
                    .await?;
            }
        }

        // Update local member enrollment status from authoritative server state
        for lm in &local_members {
            if lm.revoked
                || pending
                    .iter()
                    .any(|op| op.member_id == lm.id || op.user_id == lm.user_id)
            {
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

    /// Reconciles interrupted registrations using the exact protected keys committed before POST.
    pub async fn recover_incomplete_enrollments<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<usize, StorageError> {
        self.recover_incomplete_enrollments_with_registry(binding, client, credentials, None)
            .await
    }

    /// Reconciles vault operations and updates the active device in the connection registry.
    pub async fn recover_incomplete_enrollments_with_registry<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
        registry: Option<&HostStore>,
    ) -> Result<usize, StorageError> {
        let _journal = registry
            .unwrap_or(self)
            .lock_device_journal(".device-change.lock")?;
        let user = binding.authenticate(self, client)?;
        let ops = self
            .list_incomplete_enrollment_operations(&binding.local_vault_id)
            .await?;
        let mut recovered = 0;
        let mut first_error = None;
        for op in ops {
            if op.user_id != user.to_string() {
                continue;
            }
            match self
                .recover_enrollment_operation(binding, &op, user, client, credentials)
                .await
            {
                Ok(()) => recovered += 1,
                Err(error) => {
                    tracing::warn!(operation_id = %op.operation_id, "Enrollment operation retained for reconciliation");
                    first_error.get_or_insert(error);
                }
            }
        }
        for operation in registry
            .unwrap_or(self)
            .pending_replacements(binding, user)
            .await?
        {
            let _change = DeviceChangeGuard::acquire(binding, operation.old_device)?;
            if let Err(error) = self
                .advance_replacement(
                    binding,
                    user,
                    operation,
                    false,
                    client,
                    credentials,
                    registry,
                )
                .await
            {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(recovered), Err)
    }

    /// Held across the keychain and journal boundaries. The OS releases this lock on exit.
    fn lock_device_journal(&self, suffix: &str) -> Result<Option<std::fs::File>, StorageError> {
        let Some(path) = self.db_path.as_deref().filter(|path| *path != ":memory:") else {
            return Ok(None);
        };
        let mut path = std::fs::canonicalize(path)
            .map_err(|_| enrollment_denied("Unable to locate the enrollment database"))?
            .into_os_string();
        path.push(suffix);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|_| enrollment_denied("Unable to open the enrollment journal lock"))?;
        file.try_lock()
            .map_err(|_| enrollment_denied("Another enrollment is using this database"))?;
        Ok(Some(file))
    }

    async fn change_enrollment_stage(
        &self,
        op: &crate::EnrollmentOperationRecord,
        from: &str,
        to: &str,
        reason: &str,
    ) -> Result<(), StorageError> {
        let changed = self
            .connection
            .execute(
                "UPDATE enrollment_operations SET stage = ?1, error_message = ?2, updated_at = ?3
             WHERE operation_id = ?4 AND stage = ?5 AND registration_intent = ?6
               AND device_id IS ?7",
                (
                    to,
                    reason,
                    current_timestamp().to_string(),
                    op.operation_id.as_str(),
                    from,
                    op.registration_intent.as_deref(),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(enrollment_denied(
                "Enrollment checkpoint changed concurrently",
            ));
        }
        tracing::debug!(operation_id = %op.operation_id, from, to, "Enrollment checkpoint committed");
        Ok(())
    }

    async fn clean_abandoned_registration(
        &self,
        op: &crate::EnrollmentOperationRecord,
        intent: &RegistrationIntent,
        credentials: &dyn shellumi_core::CredentialStore,
        restore_invitation: bool,
    ) -> Result<(), StorageError> {
        // Only never-dispatched key creation and definitive rejection enter this state.
        // The random key accounts were reserved by this operation and never published.
        let service = VaultServerConnectionRecord::CREDENTIAL_SERVICE;
        let signing_ref = format!("keychain://{service}/{}-signing", intent.key_id);
        let agreement_ref = format!("keychain://{service}/{}-agreement", intent.key_id);
        let mut references = self.connection.query(
            "SELECT 1 FROM enrolled_devices WHERE signing_key_ref IN (?1, ?2) OR agreement_key_ref IN (?1, ?2)
             UNION ALL
             SELECT 1 FROM vault_server_connections WHERE device_signing_key IN (?1, ?2)
               OR device_agreement_key IN (?1, ?2) OR auth_token IN (?1, ?2) OR refresh_token IN (?1, ?2)
             UNION ALL
             SELECT 1 FROM enrollment_operations WHERE operation_id != ?3
               AND stage NOT IN ('Canceled', 'Completed') AND instr(registration_intent, ?4) > 0
             LIMIT 1",
            (signing_ref.as_str(), agreement_ref.as_str(), op.operation_id.as_str(), intent.key_id.to_string()),
        ).await?;
        if references.next().await?.is_some() {
            return Err(enrollment_denied(
                "Credential cleanup refused because the keys are still referenced",
            ));
        }
        drop(references);
        for suffix in ["signing", "agreement"] {
            credentials
                .delete_secret(
                    VaultServerConnectionRecord::CREDENTIAL_SERVICE,
                    &format!("{}-{suffix}", intent.key_id),
                )
                .map_err(|_| enrollment_denied("Protected credential cleanup must be retried"))?;
            tracing::debug!(operation_id = %op.operation_id, credential = suffix, "Abandoned credential reconciled");
        }
        if let Some(checkpoint) = intent.invitation.as_ref().filter(|_| restore_invitation) {
            let encoded = serde_json::to_string(checkpoint).map_err(|_| {
                enrollment_denied("Unable to encode recovered invitation checkpoint")
            })?;
            let changed = self.connection.execute(
                "UPDATE enrollment_operations SET stage = 'InvitationAcceptedBound', registration_intent = ?1,
                 error_message = NULL, updated_at = ?2
                 WHERE operation_id = ?3 AND stage = 'CredentialCleanup' AND registration_intent = ?4",
                (encoded, current_timestamp().to_string(), op.operation_id.as_str(), op.registration_intent.as_deref()),
            ).await?;
            if changed != 1 {
                return Err(enrollment_denied(
                    "Invitation checkpoint changed during credential cleanup",
                ));
            }
            tracing::debug!(operation_id = %op.operation_id, "Restored verified invitation after credential cleanup");
            return Ok(());
        }
        self.change_enrollment_stage(op, "CredentialCleanup", "Canceled", "Enrollment canceled")
            .await
    }

    async fn recover_enrollment_operation<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        op: &crate::EnrollmentOperationRecord,
        user: Uuid,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<(), StorageError> {
        if op.stage == "InvitationAccepting" {
            return Err(enrollment_denied("Invitation acceptance needs manual reconciliation. Confirm the invitation with the vault administrator."));
        }
        if op.stage == "InvitationAcceptedBound" {
            let checkpoint = op
                .registration_intent
                .as_deref()
                .and_then(|raw| serde_json::from_str::<InvitationCheckpoint>(raw).ok())
                .filter(|checkpoint| binding.matches_scope(&checkpoint.scope))
                .ok_or_else(|| {
                    enrollment_denied("Manual intervention required: invalid invitation checkpoint")
                })?;
            let role = checkpoint.role.ok_or_else(|| {
                enrollment_denied("Manual intervention required: invitation role is missing")
            })?;
            binding.verify_membership(client, user, Some(role))?;
            self.ensure_invited_member(binding, &op.member_id, user, role, &checkpoint)
                .await?;
            self.enroll_device_for_authenticated_user(
                binding,
                &op.member_id,
                user,
                role,
                EnrollmentSource::Invitation(&op.operation_id, &checkpoint),
                &checkpoint.device_label,
                client,
                credentials,
            )
            .await?;
            return Ok(());
        }
        let _journal = self.lock_device_journal(".enrollment.lock")?;
        // The list may have been read before another process advanced this operation.
        let current = self
            .list_incomplete_enrollment_operations(&binding.local_vault_id)
            .await?
            .into_iter()
            .find(|current| current.operation_id == op.operation_id)
            .ok_or_else(|| enrollment_denied("Enrollment checkpoint is no longer active"))?;
        let op = &current;
        let intent = op
            .registration_intent
            .as_deref()
            .and_then(|raw| serde_json::from_str::<RegistrationIntent>(raw).ok())
            .filter(|intent| intent.matches(binding, user))
            .ok_or_else(|| {
                enrollment_denied(
                    "Manual intervention required: missing or invalid registration intent",
                )
            })?;
        if !matches!(
            op.stage.as_str(),
            "CredentialsCreating"
                | "CredentialCleanup"
                | "RegistrationPrepared"
                | "RegistrationIntent"
                | "RegistrationReconciliation"
        ) {
            return Err(enrollment_denied(
                "Manual intervention required: invalid registration stage",
            ));
        }
        let mut prepared = op.stage == "RegistrationPrepared";
        let needs_cleanup = if op.stage == "CredentialsCreating" {
            let service = VaultServerConnectionRecord::CREDENTIAL_SERVICE;
            // A read error does not prove that either key is absent.
            let signing = credentials
                .get_secret(service, &format!("{}-signing", intent.key_id))
                .map_err(StorageError::InvalidCredential)?;
            let agreement = credentials
                .get_secret(service, &format!("{}-agreement", intent.key_id))
                .map_err(StorageError::InvalidCredential)?;
            if signing.is_none() || agreement.is_none() {
                self.change_enrollment_stage(
                    op,
                    "CredentialsCreating",
                    "CredentialCleanup",
                    "Interrupted credential creation",
                )
                .await?;
                true
            } else {
                Self::registration_credentials(&intent, credentials)?;
                self.change_enrollment_stage(
                    op,
                    "CredentialsCreating",
                    "RegistrationPrepared",
                    "Device credentials verified",
                )
                .await?;
                prepared = true;
                false
            }
        } else {
            op.stage == "CredentialCleanup"
        };
        if needs_cleanup {
            self.clean_abandoned_registration(
                op,
                &intent,
                credentials,
                op.stage == "CredentialsCreating"
                    || op.error_message.as_deref() == Some("Interrupted credential creation"),
            )
            .await?;
            drop(_journal);
            let remaining = self
                .list_incomplete_enrollment_operations(&binding.local_vault_id)
                .await?;
            if let Some(invitation) = remaining.iter().find(|current| {
                current.operation_id == op.operation_id
                    && current.stage == "InvitationAcceptedBound"
            }) {
                return Box::pin(self.recover_enrollment_operation(
                    binding,
                    invitation,
                    user,
                    client,
                    credentials,
                ))
                .await;
            }
            return Ok(());
        }
        binding.verify_membership(client, user, Some(intent.role))?;
        if prepared {
            self.dispatch_device_registration(binding, op, &intent, client, credentials)
                .await?;
        }
        self.finish_device_registration(binding, op, &intent, client, credentials)
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn enroll_device_for_authenticated_user<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        member_id: &str,
        user: Uuid,
        role: TeamRole,
        source: EnrollmentSource<'_>,
        device_label: &str,
        client: &mut ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<EnrolledDevice, StorageError> {
        let _journal = self.lock_device_journal(".enrollment.lock")?;
        let (signing, signing_public_key) = generate_device_ed25519_keypair_pkcs8()
            .map_err(|_| enrollment_denied("Unable to generate Ed25519 identity"))?;
        let (agreement, agreement_public_key) = generate_device_x25519_keypair()
            .map_err(|_| enrollment_denied("Unable to generate X25519 identity"))?;
        let member = self
            .get_team_member(member_id)
            .await?
            .filter(|member| {
                member.vault_id == binding.local_vault_id
                    && member.user_id == user.to_string()
                    && !member.revoked
            })
            .ok_or_else(|| enrollment_denied("No active local member for registration"))?;
        let operation_id = match source {
            EnrollmentSource::Standalone | EnrollmentSource::Connection(_) => {
                Uuid::new_v4().to_string()
            }
            EnrollmentSource::Invitation(id, _) | EnrollmentSource::Replacement(id) => id.into(),
        };
        // Unique key accounts prevent a concurrent caller from replacing the winning intent's keys.
        let key_id = Uuid::new_v4();
        let intent = RegistrationIntent {
            scope: binding.enrollment_scope()?,
            role,
            local_vault_id: Some(binding.local_vault_id.clone()),
            account_id: Some(user),
            key_id,
            device_label: device_label.into(),
            initialize_connection: matches!(source, EnrollmentSource::Connection(_)),
            previous_connection_device: match source {
                EnrollmentSource::Connection(previous) => previous.map(str::to_owned),
                _ => None,
            },
            invitation: match source {
                EnrollmentSource::Invitation(_, checkpoint) => Some(checkpoint.clone()),
                _ => None,
            },
            signing_public_key,
            agreement_public_key,
        };
        let encoded_intent = serde_json::to_string(&intent)
            .map_err(|_| enrollment_denied("Unable to encode registration intent"))?;
        let now = current_timestamp().to_string();
        let user_id = user.to_string();
        let written = if matches!(source, EnrollmentSource::Invitation(..)) {
            self.connection.execute(
                "UPDATE enrollment_operations SET stage = 'CredentialsCreating', registration_intent = ?1,
                 updated_at = ?2, error_message = 'Device credentials or dispatch pending'
                 WHERE operation_id = ?3 AND vault_id = ?4 AND user_id = ?5 AND member_id = ?6
                   AND stage = 'InvitationAcceptedBound'",
                (encoded_intent.as_str(), now.as_str(), operation_id.as_str(), binding.local_vault_id.as_str(), user_id.as_str(), member.id.as_str()),
            ).await?
        } else {
            self.connection.execute(
                "INSERT INTO enrollment_operations
                  (operation_id, vault_id, user_id, member_id, stage, registration_intent, error_message, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'CredentialsCreating', ?5, 'Device credentials or dispatch pending', ?6)
                 ON CONFLICT DO NOTHING",
                (operation_id.as_str(), binding.local_vault_id.as_str(), user_id.as_str(), member.id.as_str(), encoded_intent.as_str(), now.as_str()),
            ).await?
        };
        if written != 1 {
            return Err(enrollment_denied(
                "An enrollment operation already exists. Resume Enrollment before retrying.",
            ));
        }
        let op = self
            .list_incomplete_enrollment_operations(&binding.local_vault_id)
            .await?
            .into_iter()
            .find(|op| op.operation_id == operation_id)
            .ok_or_else(|| enrollment_denied("Registration intent is no longer pending"))?;
        if op.registration_intent.as_deref() != Some(encoded_intent.as_str()) {
            return Err(enrollment_denied(
                "Registration intent changed before dispatch",
            ));
        }
        let service = VaultServerConnectionRecord::CREDENTIAL_SERVICE;
        shellumi_core::store_secret_to_ref(
            credentials,
            service,
            &format!("{key_id}-signing"),
            &BASE64_STANDARD.encode(&signing),
        )
        .map_err(StorageError::InvalidCredential)?;
        shellumi_core::store_secret_to_ref(
            credentials,
            service,
            &format!("{key_id}-agreement"),
            &BASE64_STANDARD.encode(agreement.as_ref()),
        )
        .map_err(StorageError::InvalidCredential)?;
        Self::registration_credentials(&intent, credentials)?;
        self.change_enrollment_stage(
            &op,
            "CredentialsCreating",
            "RegistrationPrepared",
            "Device credentials verified",
        )
        .await?;
        self.dispatch_device_registration(binding, &op, &intent, client, credentials)
            .await?;
        self.finish_device_registration(binding, &op, &intent, client, credentials)
            .await
    }

    async fn dispatch_device_registration<T: HttpTransport>(
        &self,
        binding: &VerifiedRemoteVaultBinding,
        op: &crate::EnrollmentOperationRecord,
        intent: &RegistrationIntent,
        client: &ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<(), StorageError> {
        let user = binding.authenticate(self, client)?;
        binding.verify_membership(client, user, Some(intent.role))?;
        if op.user_id != user.to_string() || !intent.matches(binding, user) {
            return Err(enrollment_denied("Registration dispatch scope changed"));
        }
        if intent.initialize_connection {
            self.verify_connection_initialization(
                binding,
                client,
                credentials,
                intent.previous_connection_device.as_deref(),
            )
            .await?;
        }
        let (_, _, signing_pair) = Self::registration_credentials(intent, credentials)?;
        let challenge = client
            .device_challenge(&DeviceChallengeRequest {
                account_id: Some(user),
            })
            .map_err(enrollment_network_error)?
            .challenge;
        let changed = self.connection.execute(
            "UPDATE enrollment_operations SET stage = 'RegistrationIntent',
             error_message = 'Registration outcome pending', updated_at = ?1
             WHERE operation_id = ?2 AND stage = 'RegistrationPrepared' AND registration_intent = ?3",
            (current_timestamp().to_string(), op.operation_id.as_str(), op.registration_intent.as_deref()),
        ).await?;
        if changed != 1 {
            return Err(enrollment_denied("Registration dispatch already claimed"));
        }
        let registration = match client.submit_device_registration(&RegisterDeviceRequest {
            device_name: intent.device_label.clone(),
            signing_public_key: intent.signing_public_key.clone(),
            agreement_public_key: intent.agreement_public_key.clone(),
            challenge_signature: Some(sign_device_challenge(&signing_pair, &challenge)),
            challenge: Some(challenge),
        }) {
            Ok(registration) => registration,
            Err(error) => {
                if definitive_request_rejection(&error) {
                    self.change_enrollment_stage(
                        op,
                        "RegistrationIntent",
                        "CredentialCleanup",
                        "Registration definitively rejected",
                    )
                    .await?;
                    self.clean_abandoned_registration(op, intent, credentials, false)
                        .await?;
                    return Err(enrollment_denied(
                        "Server rejected registration. Start a new enrollment.",
                    ));
                }
                tracing::warn!(operation_id = %op.operation_id, "Registration request outcome uncertain");
                return Err(enrollment_denied("Registration outcome uncertain. Use Resume Enrollment to check the original device without submitting a new registration."));
            }
        };
        if registration.device_id.is_nil()
            || registration.user_id != user
            || registration.revoked_at.is_some()
            || registration.signing_public_key != intent.signing_public_key
            || registration.agreement_public_key != intent.agreement_public_key
        {
            return Err(enrollment_denied(
                "Registration returned an inconsistent device identity",
            ));
        }
        let changed = self
            .connection
            .execute(
                "UPDATE enrollment_operations SET device_id = ?1 WHERE operation_id = ?2
             AND stage = 'RegistrationIntent' AND registration_intent = ?3 AND device_id IS NULL",
                (
                    registration.device_id.to_string(),
                    op.operation_id.as_str(),
                    op.registration_intent.as_deref(),
                ),
            )
            .await?;
        if changed != 1 {
            return Err(enrollment_denied(
                "Registration identity checkpoint changed concurrently",
            ));
        }
        tracing::debug!(operation_id = %op.operation_id, "Recorded remote registration identity");
        Ok(())
    }

    fn registration_credentials(
        intent: &RegistrationIntent,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<(String, String, ring::signature::Ed25519KeyPair), StorageError> {
        let (signing_ref, agreement_ref) = intent.credential_refs();
        let signing = shellumi_core::resolve_secret_from_ref(credentials, &signing_ref)
            .map_err(StorageError::InvalidCredential)?
            .ok_or_else(|| enrollment_denied("Missing protected Ed25519 identity"))?;
        let signing_bytes = BASE64_STANDARD
            .decode(signing.expose_secret())
            .map_err(|_| enrollment_denied("Malformed protected Ed25519 identity"))?;
        let signing_pair = keypair_from_pkcs8(&signing_bytes)
            .map_err(|_| enrollment_denied("Invalid protected Ed25519 identity"))?;
        let agreement = shellumi_core::resolve_secret_from_ref(credentials, &agreement_ref)
            .map_err(StorageError::InvalidCredential)?
            .ok_or_else(|| enrollment_denied("Missing protected X25519 identity"))?;
        if BASE64_STANDARD.encode(signing_pair.public_key().as_ref()) != intent.signing_public_key
            || shellumi_core::server::crypto::verify_x25519_keypair_correspondence(
                agreement.expose_secret(),
                &intent.agreement_public_key,
            )
            .is_err()
        {
            return Err(enrollment_denied(
                "Protected private keys do not match the registration intent",
            ));
        }
        Ok((signing_ref, agreement_ref, signing_pair))
    }

    async fn finish_device_registration<T: HttpTransport>(
        &mut self,
        binding: &VerifiedRemoteVaultBinding,
        op: &crate::EnrollmentOperationRecord,
        intent: &RegistrationIntent,
        client: &ShellumiServerClient<T>,
        credentials: &dyn shellumi_core::CredentialStore,
    ) -> Result<EnrolledDevice, StorageError> {
        let user = binding.authenticate(self, client)?;
        if op.user_id != user.to_string()
            || op.vault_id != binding.local_vault_id
            || !intent.matches(binding, user)
        {
            return Err(enrollment_denied(
                "Registration journal scope does not match the authenticated vault",
            ));
        }
        let member = self
            .get_team_member(&op.member_id)
            .await?
            .filter(|member| {
                member.user_id == op.user_id
                    && member.vault_id == binding.local_vault_id
                    && !member.revoked
            })
            .ok_or_else(|| {
                enrollment_denied("Enrollment member no longer has local authorization")
            })?;
        let mut checkpoint = self.connection.query(
            "SELECT stage, device_id FROM enrollment_operations WHERE operation_id = ?1
             AND stage IN ('RegistrationIntent', 'RegistrationReconciliation') AND registration_intent = ?2",
            (op.operation_id.as_str(), op.registration_intent.as_deref()),
        ).await?;
        let row = checkpoint
            .next()
            .await?
            .ok_or_else(|| enrollment_denied("Registration journal is no longer pending"))?;
        let mut current = op.clone();
        current.stage = row.get(0)?;
        current.device_id = row.get(1)?;
        drop(checkpoint);
        let devices = client.list_devices().map_err(enrollment_network_error)?;
        let mut matches = devices.into_iter().filter(|device| {
            device.user_id == user
                && device.signing_public_key == intent.signing_public_key
                && device.agreement_public_key == intent.agreement_public_key
        });
        let matching = matches.next().filter(|_| matches.next().is_none());
        let Some(registered) = matching.filter(|device| {
            !device.device_id.is_nil()
                && current
                    .device_id
                    .as_deref()
                    .is_none_or(|id| Uuid::parse_str(id).ok() == Some(device.device_id))
        }) else {
            if current.stage == "RegistrationIntent" {
                self.change_enrollment_stage(&current, "RegistrationIntent", "RegistrationReconciliation",
                    "Registration outcome uncertain. Resume Enrollment to check the original device again.")
                    .await?;
            }
            return Err(enrollment_denied("Reconciliation required: server listings cannot establish the registration outcome. Resume Enrollment to check again without resubmitting."));
        };
        if registered.revoked_at.is_some() {
            self.change_enrollment_stage(
                &current,
                &current.stage,
                "CredentialCleanup",
                "The server confirmed that the registered device was revoked",
            )
            .await?;
            self.clean_abandoned_registration(&current, intent, credentials, false)
                .await?;
            return Err(enrollment_denied("The server confirmed the original device was revoked. Start a new enrollment if needed."));
        }
        let recipients = client
            .list_recipient_devices(binding.remote_vault_id)
            .map_err(enrollment_network_error)?;
        if recipients.vault_id != binding.remote_vault_id
            || !recipients.devices.iter().any(|device| {
                device.device_id == registered.device_id
                    && device.user_id == user
                    && device.agreement_public_key == intent.agreement_public_key
            })
        {
            if current.stage == "RegistrationIntent" {
                self.change_enrollment_stage(&current, "RegistrationIntent", "RegistrationReconciliation",
                    "The original device exists but vault authorization is missing. Resume Enrollment after the administrator verifies the device.")
                    .await?;
            }
            return Err(enrollment_denied(
                "Manual intervention required: registered device has no remote vault authorization",
            ));
        }
        let (signing_ref, agreement_ref, _) = Self::registration_credentials(intent, credentials)?;
        let device = EnrolledDevice {
            device_id: registered.device_id.to_string(),
            vault_id: binding.local_vault_id.clone(),
            user_id: op.user_id.clone(),
            device_name: registered.device_name,
            signing_public_key: registered.signing_public_key,
            agreement_public_key: registered.agreement_public_key,
            signing_key_ref: Some(signing_ref),
            agreement_key_ref: Some(agreement_ref),
            enrolled_at: registered.created_at.to_string(),
            revoked: false,
            revoked_at: None,
            verified: true,
        };
        let initial_auth_reference = if intent.initialize_connection {
            Some(
                self.verify_connection_initialization(
                    binding,
                    client,
                    credentials,
                    intent.previous_connection_device.as_deref(),
                )
                .await?,
            )
        } else {
            None
        };
        let tx = self.connection.transaction().await?;
        let completed = tx.execute(
            "UPDATE enrollment_operations SET stage = 'Completed', device_id = ?1, error_message = NULL, updated_at = ?2
             WHERE operation_id = ?3 AND stage IN ('RegistrationIntent', 'RegistrationReconciliation') AND registration_intent = ?4
               AND vault_id = ?5 AND member_id = ?6 AND user_id = ?7
               AND (device_id IS NULL OR device_id = ?1)",
            (device.device_id.as_str(), current_timestamp().to_string(), op.operation_id.as_str(),
             op.registration_intent.as_deref(), op.vault_id.as_str(), op.member_id.as_str(), op.user_id.as_str()),
        ).await?;
        if completed != 1 {
            return Err(enrollment_denied(
                "Enrollment journal was changed by another operation",
            ));
        }
        let updated = tx.execute(
            "UPDATE team_members SET role = ?1, enrollment_status = ?2, agreement_public_key = ?3
             WHERE id = ?4 AND vault_id = ?5 AND user_id = ?6 AND revoked = 0",
            (intent.role.as_str(), EnrollmentStatus::DeviceEnrolled.as_str(), device.agreement_public_key.as_str(),
             member.id.as_str(), op.vault_id.as_str(), op.user_id.as_str()),
        ).await?;
        if updated != 1 {
            return Err(enrollment_denied(
                "Enrollment member changed during finalization",
            ));
        }
        let mut existing_credentials = tx
            .query(
                "SELECT 1 FROM enrolled_devices WHERE device_id = ?1 AND
             ((signing_key_ref IS NOT NULL AND signing_key_ref != ?2)
              OR (agreement_key_ref IS NOT NULL AND agreement_key_ref != ?3))",
                (
                    device.device_id.as_str(),
                    device.signing_key_ref.as_deref(),
                    device.agreement_key_ref.as_deref(),
                ),
            )
            .await?;
        if existing_credentials.next().await?.is_some() {
            return Err(enrollment_denied(
                "Enrollment would replace existing device credentials",
            ));
        }
        drop(existing_credentials);
        let mut existing_authorization = tx
            .query(
                "SELECT 1 FROM vault_device_authorizations
             WHERE vault_id = ?1 AND device_id = ?2 AND (revoked != 0 OR user_id != ?3)",
                (
                    device.vault_id.as_str(),
                    device.device_id.as_str(),
                    device.user_id.as_str(),
                ),
            )
            .await?;
        if existing_authorization.next().await?.is_some() {
            return Err(enrollment_denied(
                "Existing vault authorization was revoked or belongs to another user",
            ));
        }
        drop(existing_authorization);
        Self::persist_enrolled_device(&tx, &device).await?;
        if let Some(auth_reference) = initial_auth_reference {
            let attached = tx.execute(
                "UPDATE vault_server_connections
                 SET device_id = ?1, device_signing_key = ?2, device_agreement_key = ?3, updated_at = ?4
                 WHERE id = ?5 AND server_instance_id = ?6 AND base_url = ?7
                 AND auth_token = ?10 AND device_id IS ?11
                 AND (?11 IS NOT NULL OR (device_signing_key IS NULL AND device_agreement_key IS NULL))
                 AND EXISTS (SELECT 1 FROM vault_contexts WHERE id = ?8 AND server_connection_id = ?5 AND server_vault_id = ?9)",
                (device.device_id.as_str(), device.signing_key_ref.as_deref(), device.agreement_key_ref.as_deref(),
                 current_timestamp(), binding.server_connection_id.as_str(), intent.scope.server_instance_id.as_str(),
                 binding.base_url.as_str(), binding.local_vault_id.as_str(), binding.remote_vault_id.to_string(), auth_reference.as_str(), intent.previous_connection_device.as_deref()),
            ).await?;
            if attached != 1 {
                return Err(enrollment_denied(
                    "Connection changed before device credentials were committed",
                ));
            }
        }
        tx.commit().await?;
        tracing::info!(operation_id = %op.operation_id, device_id = %device.device_id, "Completed verified device enrollment");
        Ok(device)
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
        CapabilitiesResponse, ChangeFeedResponse, CommitBatchResponse, MutationAck, ServerLimits,
        SnapshotResponse, SyncChangeItem,
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

    #[test]
    fn device_journal_lock_worker() {
        let Ok(path) = std::env::var("SHELLUMI_TEST_DEVICE_JOURNAL_DB") else {
            return;
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let store = runtime.block_on(HostStore::open(&path)).unwrap();
        let guard = store.lock_device_journal(".device-change.lock");
        if std::env::var_os("SHELLUMI_TEST_DEVICE_JOURNAL_EXPECT_BUSY").is_some() {
            assert!(guard.is_err());
        } else {
            assert!(guard.unwrap().is_some());
        }
    }

    #[tokio::test]
    async fn device_journal_excludes_other_processes_and_releases_on_exit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("shared.db");
        let store = HostStore::open(path.to_str().unwrap()).await.unwrap();
        let exe = std::env::current_exe().unwrap();
        let run_worker = |expect_busy| {
            let mut child = std::process::Command::new(&exe);
            child
                .args(["--exact", "server_sync::tests::device_journal_lock_worker"])
                .env("SHELLUMI_TEST_DEVICE_JOURNAL_DB", &path)
                .env_remove("SHELLUMI_TEST_DEVICE_JOURNAL_EXPECT_BUSY");
            if expect_busy {
                child.env("SHELLUMI_TEST_DEVICE_JOURNAL_EXPECT_BUSY", "1");
            }
            let result = child.output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stdout)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
        };

        let guard = store.lock_device_journal(".device-change.lock").unwrap();
        run_worker(true);
        drop(guard);
        run_worker(false);
        assert!(store.lock_device_journal(".device-change.lock").is_ok());
    }

    #[tokio::test]
    async fn test_authenticated_x25519_team_device_enrollment_lifecycle_and_security() {
        use shellumi_core::server::crypto::{
            unwrap_dek_for_device, validate_x25519_agreement_public_key, verify_device_signature,
            wrap_dek_for_recipient,
        };
        use shellumi_core::server::types::{
            AcceptInviteResponse, DeviceChallengeResponse, DeviceResponse, MeResponse,
            RecipientDeviceResponse, RecipientDevicesResponse, VaultMemberListResponse,
            VaultMemberResponse,
        };
        use shellumi_core::{
            resolve_secret_from_ref, CredentialError, CredentialStore, EphemeralCredentialStore,
            SecretString, SharedVault,
        };
        use std::sync::{Arc, Mutex};

        struct StatefulEnrollmentTransport {
            user_id: Uuid,
            other_authenticated_user: Mutex<Option<Uuid>>,
            username: String,
            vault_id: Uuid,
            server_instance_id: Uuid,
            expected_invite_token: String,
            devices: Mutex<Vec<DeviceResponse>>,
            forge_wrong_user: Mutex<bool>,
            require_epoch_key: std::sync::atomic::AtomicBool,
            provide_epoch_key: std::sync::atomic::AtomicBool,
            omit_from_list: Mutex<bool>,
            member_response: Mutex<Option<Vec<VaultMemberResponse>>>,
            omit_recipient_label: Mutex<Option<String>>,
            fail_challenge: std::sync::atomic::AtomicBool,
            fail_device_listing: std::sync::atomic::AtomicBool,
            revocation_requests: std::sync::atomic::AtomicUsize,
            lose_revocation_response: std::sync::atomic::AtomicBool,
            reject_revocation: std::sync::atomic::AtomicBool,
            lose_registration_response: std::sync::atomic::AtomicBool,
            reject_registration: std::sync::atomic::AtomicBool,
            definitively_reject_registration: std::sync::atomic::AtomicBool,
            registration_requests: std::sync::atomic::AtomicUsize,
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
                        user_id: self
                            .0
                            .other_authenticated_user
                            .lock()
                            .unwrap()
                            .unwrap_or(self.0.user_id),
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
                    if self
                        .0
                        .fail_challenge
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        return Ok(HttpResponse::new(503, b"{}".to_vec()));
                    }
                    let resp = DeviceChallengeResponse {
                        challenge: "nonce-test-challenge-001".to_string(),
                        expires_at: 9_999_999_999,
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&resp).unwrap()));
                }

                if method == "POST" && url.ends_with("/api/v1/devices") {
                    self.0
                        .registration_requests
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if self
                        .0
                        .reject_registration
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        return Ok(HttpResponse::new(503, b"{}".to_vec()));
                    }
                    if self.0.definitively_reject_registration.load(SeqCst) {
                        return Ok(HttpResponse::new(
                            403,
                            br#"{"error_code":"forbidden","message":"registration rejected"}"#
                                .to_vec(),
                        ));
                    }
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
                    if self
                        .0
                        .lose_registration_response
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        return Ok(HttpResponse::new(503, b"{}".to_vec()));
                    }
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&dev).unwrap()));
                }

                if method == "GET" && url.ends_with("/api/v1/devices") {
                    if self
                        .0
                        .fail_device_listing
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        return Ok(HttpResponse::new(503, b"{}".to_vec()));
                    }
                    let list = if *self.0.omit_from_list.lock().unwrap() {
                        Vec::new()
                    } else {
                        self.0.devices.lock().unwrap().clone()
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&list).unwrap()));
                }

                if method == "DELETE" && url.contains("/api/v1/devices/") {
                    self.0
                        .revocation_requests
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if self.0.reject_revocation.load(SeqCst) {
                        return Ok(HttpResponse::new(403, Vec::new()));
                    }
                    let id_str = url.rsplit('/').next().unwrap_or("");
                    let target_id = Uuid::parse_str(id_str).unwrap();
                    let mut devs = self.0.devices.lock().unwrap();
                    for d in devs.iter_mut() {
                        if d.device_id == target_id {
                            d.revoked_at = Some(3000);
                        }
                    }
                    if self.0.lose_revocation_response.load(SeqCst) {
                        return Err(SyncError::Network("revocation response lost".into()));
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

                if method == "GET" && url.ends_with(&format!("/vaults/{}", self.0.vault_id)) {
                    let device_header = request
                        .headers
                        .iter()
                        .find(|(name, _)| name == "X-Device-Id");
                    if !device_header.is_some_and(|(_, value)| {
                        self.0.devices.lock().unwrap().iter().any(|device| {
                            device.device_id.to_string() == *value && device.revoked_at.is_none()
                        })
                    }) {
                        return Ok(HttpResponse::new(403, Vec::new()));
                    }
                    let vault = shellumi_core::server::VaultResponse {
                        vault_id: self.0.vault_id,
                        tenant_id: Uuid::nil(),
                        owner_id: self.0.user_id,
                        vault_type: "personal".into(),
                        encrypted_name: None,
                        active_key_epoch: if self.0.require_epoch_key.load(SeqCst) {
                            2
                        } else {
                            1
                        },
                        server_sequence: 0,
                        role: "owner".into(),
                        created_at: 0,
                        updated_at: 0,
                    };
                    return Ok(HttpResponse::new(200, serde_json::to_vec(&vault).unwrap()));
                }

                if method == "GET" && url.ends_with("/keys/envelopes") {
                    use shellumi_core::server::types::{
                        KeyEnvelopeListResponse, KeyEnvelopeResponse,
                    };
                    let mut envelopes = Vec::new();
                    if self.0.provide_epoch_key.load(SeqCst) {
                        let device_header = request
                            .headers
                            .iter()
                            .find(|(name, _)| name == "X-Device-Id");
                        let devices = self.0.devices.lock().unwrap();
                        let device = devices
                            .iter()
                            .find(|device| {
                                device_header.is_some_and(|(_, value)| {
                                    value == &device.device_id.to_string()
                                })
                            })
                            .unwrap();
                        let (wrapped_dek, ephemeral) = wrap_dek_for_recipient(
                            &[7; 32],
                            &device.agreement_public_key,
                            self.0.vault_id,
                            2,
                        )
                        .unwrap();
                        envelopes.push(KeyEnvelopeResponse {
                            vault_id: self.0.vault_id,
                            epoch: 2,
                            recipient_device_id: device.device_id,
                            sender_device_id: device.device_id,
                            wrapped_dek,
                            key_agreement_ephemeral_pk: Some(ephemeral),
                            created_at: 0,
                        });
                    }
                    return Ok(HttpResponse::new(
                        200,
                        serde_json::to_vec(&KeyEnvelopeListResponse { envelopes }).unwrap(),
                    ));
                }
                if method == "GET" && url.ends_with("/members") {
                    let resp = VaultMemberListResponse {
                        members: self
                            .0
                            .member_response
                            .lock()
                            .unwrap()
                            .clone()
                            .unwrap_or_else(|| {
                                vec![VaultMemberResponse {
                                    vault_id: self.0.vault_id,
                                    user_id: self.0.user_id,
                                    username: self.0.username.clone(),
                                    role: "editor".to_string(),
                                    joined_at: 2000,
                                }]
                            }),
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
                        .filter(|d| {
                            d.revoked_at.is_none()
                                && self.0.omit_recipient_label.lock().unwrap().as_deref()
                                    != Some(d.device_name.as_str())
                        })
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
            fail_agreement_set: std::sync::atomic::AtomicBool,
            fail_delete: std::sync::atomic::AtomicBool,
        }
        impl CredentialStore for FailingCredentialStore {
            fn set_secret(
                &self,
                service: &str,
                account: &str,
                secret: &str,
            ) -> Result<(), CredentialError> {
                if self.fail_set.load(SeqCst)
                    || (account.ends_with("-agreement") && self.fail_agreement_set.load(SeqCst))
                {
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
                if self.fail_delete.load(SeqCst) {
                    return Err(CredentialError::Storage(
                        "Keychain deletion interrupted".into(),
                    ));
                }
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
            other_authenticated_user: Mutex::new(None),
            require_epoch_key: Default::default(),
            provide_epoch_key: Default::default(),
            omit_from_list: Mutex::new(false),
            member_response: Mutex::new(None),
            omit_recipient_label: Mutex::new(None),
            fail_challenge: Default::default(),
            fail_device_listing: Default::default(),
            revocation_requests: Default::default(),
            lose_revocation_response: Default::default(),
            reject_revocation: Default::default(),
            lose_registration_response: Default::default(),
            reject_registration: Default::default(),
            definitively_reject_registration: Default::default(),
            registration_requests: Default::default(),
        });
        store.set_vault_id(VaultId::new(local_vault_id).unwrap());
        let server = shellumi_core::ServerConnectionId::new("conn-1").unwrap();
        assert!(store
            .resolve_verified_remote_vault_binding(local_vault_id, None)
            .await
            .is_err());
        store
            .save_vault_server_connection(&VaultServerConnectionRecord {
                id: "conn-1".into(),
                server_instance_id: Some(transport.server_instance_id.to_string()),
                base_url: "https://srv.local".into(),
                auth_token: Some("jwt-alice".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        store
            .register_vault_context(
                "Platform Team",
                &shellumi_core::VaultContext::new(
                    VaultId::new(local_vault_id).unwrap(),
                    shellumi_core::VaultLocation::Remote {
                        server,
                        server_vault_id: VaultId::new(remote_vault_id.to_string()).unwrap(),
                    },
                ),
            )
            .await
            .unwrap();
        assert!(store
            .resolve_verified_remote_vault_binding(local_vault_id, Some("incorrect"))
            .await
            .is_err());
        let binding = store
            .resolve_verified_remote_vault_binding(local_vault_id, Some("conn-1"))
            .await
            .unwrap();
        let cred_store = FailingCredentialStore {
            inner: EphemeralCredentialStore::new(),
            fail_set: Default::default(),
            fail_agreement_set: Default::default(),
            fail_delete: Default::default(),
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
            .accept_invitation_and_enroll_device_with_binding(
                &binding,
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
        use std::sync::atomic::Ordering::SeqCst;
        let request_count = || transport.registration_requests.load(SeqCst);
        for (id, instance, remote, members) in [
            ("missing-instance", None, remote_vault_id, None),
            (
                "bad-instance",
                Some("malformed".into()),
                remote_vault_id,
                None,
            ),
            (
                "wrong-instance",
                Some(Uuid::new_v4().to_string()),
                remote_vault_id,
                None,
            ),
            (
                "wrong-vault",
                binding.server_instance_id.clone(),
                Uuid::new_v4(),
                None,
            ),
            (
                "empty-members",
                binding.server_instance_id.clone(),
                remote_vault_id,
                Some(vec![]),
            ),
            (
                "bad-role",
                binding.server_instance_id.clone(),
                remote_vault_id,
                Some(vec![VaultMemberResponse {
                    vault_id: remote_vault_id,
                    user_id,
                    username: "alice".into(),
                    role: "viewer".into(),
                    joined_at: 2000,
                }]),
            ),
        ] {
            let mut invalid = binding.clone();
            invalid.server_instance_id = instance;
            invalid.remote_vault_id = remote;
            *transport.member_response.lock().unwrap() = members;
            let before = request_count();
            assert!(
                store
                    .accept_invitation_and_enroll_device_with_binding(
                        &invalid,
                        id,
                        "Alice",
                        ssh_pk,
                        "tok-valid-123",
                        "Test",
                        &mut auth_client,
                        &cred_store,
                    )
                    .await
                    .is_err(),
                "{id}"
            );
            assert_eq!(request_count(), before, "{id}");
            assert!(store.get_team_member(id).await.unwrap().is_none());
        }
        *transport.member_response.lock().unwrap() = None;
        let mut revoked_member = invited.clone();
        revoked_member.id = "revoked-member".into();
        revoked_member.revoked = true;
        store.add_team_member(&revoked_member).await.unwrap();
        let before = request_count();
        assert!(store
            .accept_invitation_and_enroll_device_with_binding(
                &binding,
                "revoked-member",
                "Alice",
                ssh_pk,
                "tok-valid-123",
                "Test",
                &mut auth_client,
                &cred_store,
            )
            .await
            .is_err());
        assert_eq!(request_count(), before);

        cred_store
            .fail_set
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(store
            .accept_invitation_and_enroll_device_with_binding(
                &binding,
                "mem-keychain-failure",
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
            .get_team_member("mem-keychain-failure")
            .await
            .unwrap()
            .unwrap()
            .is_enrolled());
        cred_store
            .fail_set
            .store(false, std::sync::atomic::Ordering::SeqCst);

        // Keep the restart fixture focused on the accepted invitation with a recoverable outcome.
        store
            .connection
            .execute(
                "DELETE FROM enrollment_operations WHERE member_id != 'mem-keychain-failure'",
                (),
            )
            .await
            .unwrap();
        drop(store);
        let mut store = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        store.set_vault_id(VaultId::new(local_vault_id).unwrap());
        assert_eq!(
            store
                .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
                .await
                .unwrap(),
            1
        );
        let after_keychain_recovery = store
            .list_enrolled_devices_for_user(local_vault_id, &user_id.to_string())
            .await
            .unwrap()
            .pop()
            .unwrap();
        store
            .revoke_device_with_server(
                &binding,
                "mem-keychain-failure",
                Uuid::parse_str(&after_keychain_recovery.device_id).unwrap(),
                &auth_client,
            )
            .await
            .unwrap();

        // 5. Reject forged user binding from server
        *transport.forge_wrong_user.lock().unwrap() = true;
        assert!(store
            .accept_invitation_and_enroll_device_with_binding(
                &binding,
                "mem-forged",
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
            .accept_invitation_and_enroll_device_with_binding(
                &binding,
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

        // Isolate the response-loss fixture from earlier intentionally unrecoverable operations.
        store
            .connection
            .execute(
                "DELETE FROM enrollment_operations WHERE stage != 'Completed'",
                (),
            )
            .await
            .unwrap();
        store
            .connection
            .execute(
                "DELETE FROM team_members WHERE id IN ('mem-keychain-failure', 'mem-forged')",
                (),
            )
            .await
            .unwrap();
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
            .enroll_device_with_server(&binding, &mut auth_client, "Workstation Linux", &cred_store)
            .await
            .unwrap();
        assert_ne!(dev1.device_id, dev2.device_id);
        assert_ne!(dev1.agreement_public_key, dev2.agreement_public_key);

        let old_id = Uuid::parse_str(&dev1.device_id).unwrap();
        let before = request_count();
        assert!(store
            .replace_device_keys_for_member(
                &binding,
                "mem-alice",
                Uuid::new_v4(),
                "Invalid old device",
                &mut auth_client,
                &cred_store,
                None,
            )
            .await
            .is_err());
        assert_eq!(request_count(), before);

        // The current connection must be handed off before global retirement of its old identity.
        let mut current_connection = store
            .get_vault_server_connection(binding.server_connection_id())
            .await
            .unwrap()
            .unwrap();
        current_connection.device_id = Some(dev1.device_id.clone());
        current_connection.device_signing_key = dev1.signing_key_ref.clone();
        current_connection.device_agreement_key = dev1.agreement_key_ref.clone();
        store
            .save_vault_server_connection_secured(&current_connection, &cred_store)
            .await
            .unwrap();
        transport.lose_registration_response.store(true, SeqCst);
        assert!(store
            .replace_device_keys_for_member(
                &binding,
                "mem-alice",
                old_id,
                "Response lost",
                &mut auth_client,
                &cred_store,
                None,
            )
            .await
            .is_err());
        transport.lose_registration_response.store(false, SeqCst);
        assert!(transport
            .devices
            .lock()
            .unwrap()
            .iter()
            .any(|device| device.device_id == old_id && device.revoked_at.is_none()));
        // The replacement may already be registered, but its UUID is still unknown
        // to the registry. No device on this account can be safely retired yet.
        let before_revocations = transport.revocation_requests.load(SeqCst);
        let mut other_window = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        other_window.set_vault_id(VaultId::new(local_vault_id).unwrap());
        assert!(other_window
            .revoke_device_with_server(
                &binding,
                "mem-alice",
                Uuid::parse_str(&dev2.device_id).unwrap(),
                &auth_client
            )
            .await
            .is_err());
        assert_eq!(
            transport.revocation_requests.load(SeqCst),
            before_revocations
        );
        drop(other_window);
        let before = request_count();
        drop(store);
        let mut store = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        store.set_vault_id(VaultId::new(local_vault_id).unwrap());
        let pending = store
            .list_incomplete_enrollment_operations(local_vault_id)
            .await
            .unwrap();
        let op = &pending[0];
        let original = op.registration_intent.as_deref().unwrap();
        store.connection.execute(
            "UPDATE enrollment_operations SET registration_intent = 'invalid JSON' WHERE operation_id = ?1",
            (op.operation_id.as_str(),),
        ).await.unwrap();
        assert!(store
            .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
            .await
            .is_err());
        assert_eq!(request_count(), before);
        let mut other_binding = binding.clone();
        other_binding.local_vault_id = "different-vault".into();
        store.set_vault_id(VaultId::new(other_binding.local_vault_id.clone()).unwrap());
        assert_eq!(
            store
                .recover_incomplete_enrollments(&other_binding, &mut auth_client, &cred_store)
                .await
                .unwrap(),
            0
        );
        assert_eq!(request_count(), before);
        store.set_vault_id(VaultId::new(local_vault_id).unwrap());
        store
            .connection
            .execute(
                "UPDATE enrollment_operations SET registration_intent = ?1 WHERE operation_id = ?2",
                (original, op.operation_id.as_str()),
            )
            .await
            .unwrap();
        *transport.other_authenticated_user.lock().unwrap() = Some(Uuid::new_v4());
        assert_eq!(
            store
                .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
                .await
                .unwrap(),
            0
        );
        *transport.other_authenticated_user.lock().unwrap() = None;
        let mut different_server = binding.clone();
        different_server.server_instance_id = Some(Uuid::new_v4().to_string());
        assert!(store
            .recover_incomplete_enrollments(&different_server, &mut auth_client, &cred_store)
            .await
            .is_err());
        assert_eq!(request_count(), before);
        assert_eq!(
            transport.revocation_requests.load(SeqCst),
            before_revocations
        );

        // Fail the authorization write after the journal and member writes have begun.
        store
            .connection
            .execute(
                "ALTER TABLE vault_device_authorizations RENAME TO interrupted_authorizations",
                (),
            )
            .await
            .unwrap();
        assert!(store
            .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
            .await
            .is_err());
        let pending = store
            .list_incomplete_enrollment_operations(local_vault_id)
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].stage, "RegistrationIntent");
        assert_eq!(
            store
                .get_team_member("mem-alice")
                .await
                .unwrap()
                .unwrap()
                .agreement_public_key,
            Some(dev2.agreement_public_key.clone())
        );
        assert_eq!(request_count(), before);
        store
            .connection
            .execute(
                "ALTER TABLE interrupted_authorizations RENAME TO vault_device_authorizations",
                (),
            )
            .await
            .unwrap();
        assert_eq!(
            store
                .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
                .await
                .unwrap(),
            0
        );
        assert_eq!(request_count(), before);
        let recovered = store
            .list_enrolled_devices_for_user(local_vault_id, &user_id.to_string())
            .await
            .unwrap()
            .into_iter()
            .find(|device| device.device_name == "Response lost")
            .unwrap();
        let current_connection = store
            .get_vault_server_connection(binding.server_connection_id())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            current_connection.device_id.as_deref(),
            Some(recovered.device_id.as_str())
        );
        assert_eq!(
            current_connection.device_signing_key,
            recovered.signing_key_ref
        );
        assert_eq!(
            current_connection.device_agreement_key,
            recovered.agreement_key_ref
        );
        assert!(transport
            .devices
            .lock()
            .unwrap()
            .iter()
            .any(|device| device.device_id == old_id && device.revoked_at.is_some()));
        assert!(!store
            .has_pending_device_replacements(local_vault_id)
            .await
            .unwrap());
        let old_id = Uuid::parse_str(&recovered.device_id).unwrap();

        *transport.omit_recipient_label.lock().unwrap() = Some("Unverified replacement".into());
        assert!(store
            .replace_device_keys_for_member(
                &binding,
                "mem-alice",
                old_id,
                "Unverified replacement",
                &mut auth_client,
                &cred_store,
                None,
            )
            .await
            .is_err());
        assert!(transport
            .devices
            .lock()
            .unwrap()
            .iter()
            .any(|device| device.device_id == old_id && device.revoked_at.is_none()));
        assert!(store
            .revoke_device_with_server(&binding, "mem-alice", old_id, &auth_client)
            .await
            .is_err());
        assert!(!store
            .list_enrolled_devices_for_user(local_vault_id, &user_id.to_string())
            .await
            .unwrap()
            .iter()
            .any(|device| device.device_name == "Unverified replacement"));
        *transport.omit_recipient_label.lock().unwrap() = None;
        transport.require_epoch_key.store(true, SeqCst);
        let before_revocation = transport.revocation_requests.load(SeqCst);
        let before_recovery = request_count();
        assert!(store
            .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
            .await
            .is_err());
        assert_eq!(
            transport.revocation_requests.load(SeqCst),
            before_revocation
        );
        transport.provide_epoch_key.store(true, SeqCst);
        assert_eq!(
            store
                .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            transport.revocation_requests.load(SeqCst),
            before_revocation + 1
        );
        assert_eq!(request_count(), before_recovery);
        transport.require_epoch_key.store(false, SeqCst);
        transport.provide_epoch_key.store(false, SeqCst);
        let verified_later = store
            .list_enrolled_devices_for_user(local_vault_id, &user_id.to_string())
            .await
            .unwrap()
            .into_iter()
            .find(|device| device.device_name == "Unverified replacement")
            .unwrap();
        let old_id = Uuid::parse_str(&verified_later.device_id).unwrap();
        let before_registration = request_count();
        transport.reject_revocation.store(true, SeqCst);
        assert!(store
            .replace_device_keys_for_member(
                &binding,
                "mem-alice",
                old_id,
                "Verified replacement",
                &mut auth_client,
                &cred_store,
                None,
            )
            .await
            .is_err());
        assert_eq!(request_count(), before_registration + 1);
        let after_rejection = transport.revocation_requests.load(SeqCst);
        assert!(store
            .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
            .await
            .is_err());
        assert_eq!(transport.revocation_requests.load(SeqCst), after_rejection);
        assert!(transport
            .devices
            .lock()
            .unwrap()
            .iter()
            .any(|device| device.device_id == old_id && device.revoked_at.is_none()));
        transport.reject_revocation.store(false, SeqCst);

        transport.lose_revocation_response.store(true, SeqCst);
        assert!(store
            .replace_device_keys_for_member(
                &binding,
                "mem-alice",
                old_id,
                "Must not create another device",
                &mut auth_client,
                &cred_store,
                None,
            )
            .await
            .is_err());
        assert_eq!(request_count(), before_registration + 1);
        let after_dispatch = transport.revocation_requests.load(SeqCst);
        assert!(store
            .list_enrolled_devices_for_vault(local_vault_id)
            .await
            .unwrap()
            .iter()
            .any(|device| device.device_id == old_id.to_string() && !device.revoked));
        drop(store);
        let mut store = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        store.set_vault_id(VaultId::new(local_vault_id).unwrap());
        store
            .connection
            .execute(
                "CREATE TRIGGER fail_local_retirement BEFORE UPDATE OF revoked ON enrolled_devices
             WHEN NEW.revoked = 1 BEGIN SELECT RAISE(ABORT, 'injected local write failure'); END",
                (),
            )
            .await
            .unwrap();
        assert!(store
            .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
            .await
            .is_err());
        let pending = store.pending_replacements(&binding, user_id).await.unwrap();
        assert_eq!(pending[0].stage, "RevocationConfirmed");
        assert_eq!(transport.revocation_requests.load(SeqCst), after_dispatch);
        store
            .connection
            .execute("DROP TRIGGER fail_local_retirement", ())
            .await
            .unwrap();
        assert_eq!(
            store
                .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
                .await
                .unwrap(),
            0
        );
        assert_eq!(transport.revocation_requests.load(SeqCst), after_dispatch);
        assert!(!store
            .has_pending_device_replacements(local_vault_id)
            .await
            .unwrap());
        assert_eq!(request_count(), before_registration + 1);
        transport.lose_revocation_response.store(false, SeqCst);
        let dev1 = store
            .list_enrolled_devices_for_vault(local_vault_id)
            .await
            .unwrap()
            .into_iter()
            .find(|device| device.device_name == "Verified replacement")
            .unwrap();
        assert!(transport
            .devices
            .lock()
            .unwrap()
            .iter()
            .any(|device| device.device_id == old_id && device.revoked_at.is_some()));
        let before = request_count();
        assert!(store
            .replace_device_keys_for_member(
                &binding,
                "mem-alice",
                old_id,
                "Revoked old device",
                &mut auth_client,
                &cred_store,
                None,
            )
            .await
            .is_err());
        assert_eq!(request_count(), before);

        let authorized = store
            .list_authorized_recipient_devices(local_vault_id)
            .await
            .unwrap();
        assert_eq!(authorized.len(), 2);

        // 9. Restart persistence: reopen HostStore from disk and verify enrollment & devices persist
        drop(store);
        let mut reopened = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        reopened.set_vault_id(VaultId::new(local_vault_id).unwrap());
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
                &binding,
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
                &binding,
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
            .enroll_additional_device_for_member_with_binding(
                &binding,
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
        assert_eq!(migrated_member.role, TeamRole::Editor);
        assert!(migrated_member.is_enrolled());
        assert_eq!(
            migrated_member.agreement_public_key.as_deref(),
            Some(migrated_dev.agreement_public_key.as_str())
        );

        let migrated_id = Uuid::parse_str(&migrated_dev.device_id).unwrap();
        {
            let _in_flight = DeviceChangeGuard::acquire(&binding, migrated_id).unwrap();
            let before = request_count();
            assert!(reopened
                .replace_device_keys_for_member(
                    &binding,
                    "mem-legacy",
                    migrated_id,
                    "Concurrent replacement",
                    &mut auth_client,
                    &cred_store,
                    None,
                )
                .await
                .is_err());
            assert!(reopened
                .revoke_device_with_server(&binding, "mem-legacy", migrated_id, &auth_client)
                .await
                .is_err());
            assert_eq!(request_count(), before);
        }
        let revocations = transport.revocation_requests.load(SeqCst);
        transport.fail_device_listing.store(true, SeqCst);
        assert!(reopened
            .revoke_device_with_server(&binding, "mem-legacy", migrated_id, &auth_client)
            .await
            .is_err());
        transport.fail_device_listing.store(false, SeqCst);
        assert!(reopened
            .revoke_device_with_server(&binding, "mem-legacy", Uuid::new_v4(), &auth_client)
            .await
            .is_err());
        {
            let mut devices = transport.devices.lock().unwrap();
            let old = devices
                .iter_mut()
                .find(|device| device.device_id == migrated_id)
                .unwrap();
            old.user_id = Uuid::new_v4();
        }
        assert!(reopened
            .revoke_device_with_server(&binding, "mem-legacy", migrated_id, &auth_client)
            .await
            .is_err());
        {
            let mut devices = transport.devices.lock().unwrap();
            let old = devices
                .iter_mut()
                .find(|device| device.device_id == migrated_id)
                .unwrap();
            old.user_id = user_id;
            old.signing_public_key = "stale-signing-key".into();
        }
        assert!(reopened
            .revoke_device_with_server(&binding, "mem-legacy", migrated_id, &auth_client)
            .await
            .is_err());
        {
            let mut devices = transport.devices.lock().unwrap();
            let old = devices
                .iter_mut()
                .find(|device| device.device_id == migrated_id)
                .unwrap();
            old.signing_public_key = migrated_dev.signing_public_key.clone();
        }
        assert_eq!(transport.revocation_requests.load(SeqCst), revocations);
        assert!(resolve_secret_from_ref(
            &cred_store,
            migrated_dev.signing_key_ref.as_deref().unwrap()
        )
        .unwrap()
        .is_some());

        let before = request_count();
        let second_handle = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        let journal_lock = second_handle
            .lock_device_journal(".enrollment.lock")
            .unwrap();
        assert!(reopened.lock_device_journal(".enrollment.lock").is_err());
        drop(journal_lock);
        assert!(reopened.lock_device_journal(".enrollment.lock").is_ok());
        transport.fail_challenge.store(true, SeqCst);
        assert!(reopened
            .enroll_additional_device_for_member_with_binding(
                &binding,
                "mem-legacy",
                "Prepared before restart",
                &mut auth_client,
                &cred_store,
            )
            .await
            .is_err());
        assert_eq!(request_count(), before);
        let prepared = reopened
            .list_incomplete_enrollment_operations(local_vault_id)
            .await
            .unwrap();
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].stage, "RegistrationPrepared");
        drop(reopened);
        let mut reopened = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        reopened.set_vault_id(VaultId::new(local_vault_id).unwrap());
        transport.fail_challenge.store(false, SeqCst);
        assert_eq!(
            reopened
                .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            reopened
                .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
                .await
                .unwrap(),
            0
        );
        assert_eq!(request_count(), before + 1);
        transport.reject_registration.store(true, SeqCst);
        assert!(reopened
            .replace_device_keys_for_member(
                &binding,
                "mem-legacy",
                migrated_id,
                "Registration rejected",
                &mut auth_client,
                &cred_store,
                None,
            )
            .await
            .is_err());
        transport.reject_registration.store(false, SeqCst);
        assert!(transport
            .devices
            .lock()
            .unwrap()
            .iter()
            .any(|device| device.device_id == migrated_id && device.revoked_at.is_none()));
        let before = request_count();
        assert!(reopened
            .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
            .await
            .is_err());
        assert_eq!(request_count(), before);

        let pending = reopened
            .list_incomplete_enrollment_operations(local_vault_id)
            .await
            .unwrap();
        assert_eq!(pending[0].stage, "RegistrationReconciliation");

        transport
            .definitively_reject_registration
            .store(true, SeqCst);
        cred_store.fail_delete.store(true, SeqCst);
        assert!(reopened
            .enroll_additional_device_for_member_with_binding(
                &binding,
                "mem-alice",
                "Definitively rejected",
                &mut auth_client,
                &cred_store,
            )
            .await
            .is_err());
        transport
            .definitively_reject_registration
            .store(false, SeqCst);
        cred_store.fail_delete.store(false, SeqCst);
        let rejected = reopened
            .list_incomplete_enrollment_operations(local_vault_id)
            .await
            .unwrap()
            .into_iter()
            .find(|op| op.member_id == "mem-alice")
            .unwrap();
        assert_eq!(rejected.stage, "CredentialCleanup");
        let rejected_intent: RegistrationIntent =
            serde_json::from_str(rejected.registration_intent.as_deref().unwrap()).unwrap();
        let (signing, agreement) = rejected_intent.credential_refs();
        assert!(resolve_secret_from_ref(&cred_store, &signing)
            .unwrap()
            .is_some());
        assert!(resolve_secret_from_ref(&cred_store, &agreement)
            .unwrap()
            .is_some());
        let after_rejection = request_count();
        assert!(reopened
            .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
            .await
            .is_err());
        assert_eq!(request_count(), after_rejection);
        assert!(resolve_secret_from_ref(&cred_store, &signing)
            .unwrap()
            .is_none());
        assert!(resolve_secret_from_ref(&cred_store, &agreement)
            .unwrap()
            .is_none());

        // Failure after the first keychain write leaves a journal, and a failed deletion
        // remains restartable. A separately enrolled device still has both of its keys.
        cred_store.fail_agreement_set.store(true, SeqCst);
        let before = request_count();
        assert!(reopened
            .enroll_additional_device_for_member_with_binding(
                &binding,
                "mem-alice",
                "Interrupted credentials",
                &mut auth_client,
                &cred_store,
            )
            .await
            .is_err());
        cred_store.fail_agreement_set.store(false, SeqCst);
        let pending = reopened
            .list_incomplete_enrollment_operations(local_vault_id)
            .await
            .unwrap();
        let partial = pending
            .iter()
            .find(|op| op.stage == "CredentialsCreating")
            .unwrap();
        let partial_id = partial.operation_id.clone();
        let intent: RegistrationIntent =
            serde_json::from_str(partial.registration_intent.as_deref().unwrap()).unwrap();
        let (signing, agreement) = intent.credential_refs();
        assert!(resolve_secret_from_ref(&cred_store, &signing)
            .unwrap()
            .is_some());
        assert!(resolve_secret_from_ref(&cred_store, &agreement)
            .unwrap()
            .is_none());

        cred_store.fail_delete.store(true, SeqCst);
        assert!(reopened
            .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
            .await
            .is_err());
        assert!(reopened
            .list_incomplete_enrollment_operations(local_vault_id)
            .await
            .unwrap()
            .iter()
            .any(|op| op.operation_id == partial_id && op.stage == "CredentialCleanup"));
        assert!(resolve_secret_from_ref(&cred_store, &signing)
            .unwrap()
            .is_some());
        drop(reopened);
        let mut reopened = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        reopened.set_vault_id(VaultId::new(local_vault_id).unwrap());
        cred_store.fail_delete.store(false, SeqCst);
        // The unrelated 503 is still uncertain. It must not prevent this cleanup.
        assert!(reopened
            .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
            .await
            .is_err());
        assert!(!reopened
            .list_incomplete_enrollment_operations(local_vault_id)
            .await
            .unwrap()
            .iter()
            .any(|op| op.operation_id == partial_id));
        assert!(resolve_secret_from_ref(&cred_store, &signing)
            .unwrap()
            .is_none());
        assert!(resolve_secret_from_ref(
            &cred_store,
            migrated_dev.signing_key_ref.as_deref().unwrap()
        )
        .unwrap()
        .is_some());
        assert_eq!(request_count(), before);

        // Simulate an exit after the durable dispatch checkpoint but before the HTTP POST.
        transport.fail_challenge.store(true, SeqCst);
        assert!(reopened
            .enroll_additional_device_for_member_with_binding(
                &binding,
                "mem-alice",
                "Dispatch never sent",
                &mut auth_client,
                &cred_store,
            )
            .await
            .is_err());
        transport.fail_challenge.store(false, SeqCst);
        let prepared = reopened
            .list_incomplete_enrollment_operations(local_vault_id)
            .await
            .unwrap()
            .into_iter()
            .find(|op| op.stage == "RegistrationPrepared")
            .unwrap();
        reopened
            .change_enrollment_stage(
                &prepared,
                "RegistrationPrepared",
                "RegistrationIntent",
                "Injected termination before POST",
            )
            .await
            .unwrap();
        drop(reopened);
        let mut reopened = HostStore::open(db_file.to_str().unwrap()).await.unwrap();
        reopened.set_vault_id(VaultId::new(local_vault_id).unwrap());
        for _ in 0..2 {
            assert!(reopened
                .recover_incomplete_enrollments(&binding, &mut auth_client, &cred_store)
                .await
                .is_err());
            assert!(reopened
                .list_incomplete_enrollment_operations(local_vault_id)
                .await
                .unwrap()
                .iter()
                .any(|op| op.operation_id == prepared.operation_id
                    && op.stage == "RegistrationReconciliation"));
            assert_eq!(request_count(), before);
        }

        let _ = std::fs::remove_file(&db_file);
        {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("registry.db");
            let path = path.to_str().unwrap();
            let mut registry = HostStore::open(path).await.unwrap();
            let local_id = VaultId::new("bootstrap-vault").unwrap();
            registry
                .save_vault_server_connection_secured(
                    &VaultServerConnectionRecord {
                        id: binding.server_connection_id.clone(),
                        server_instance_id: binding.server_instance_id.clone(),
                        base_url: binding.base_url.clone(),
                        auth_token: Some("jwt-alice".into()),
                        ..Default::default()
                    },
                    &cred_store,
                )
                .await
                .unwrap();
            registry
                .register_vault_context(
                    "Bootstrap vault",
                    &shellumi_core::VaultContext::new(
                        local_id.clone(),
                        shellumi_core::VaultLocation::Remote {
                            server: shellumi_core::ServerConnectionId::new(
                                &binding.server_connection_id,
                            )
                            .unwrap(),
                            server_vault_id: VaultId::new(remote_vault_id.to_string()).unwrap(),
                        },
                    ),
                )
                .await
                .unwrap();
            let initial_binding = registry
                .resolve_verified_remote_vault_binding(local_id.as_str(), None)
                .await
                .unwrap();
            registry.set_vault_id(local_id.clone());
            let before = request_count();
            transport.lose_registration_response.store(true, SeqCst);
            assert!(registry
                .enroll_initial_connection_device(
                    &initial_binding,
                    &mut auth_client,
                    "Bootstrap vault",
                    "Bootstrap device",
                    &cred_store,
                )
                .await
                .is_err());
            assert_eq!(request_count(), before + 1);
            assert!(registry
                .get_vault_server_connection(initial_binding.server_connection_id())
                .await
                .unwrap()
                .unwrap()
                .device_id
                .is_none());
            drop(registry);

            transport.lose_registration_response.store(false, SeqCst);
            let mut registry = HostStore::open(path).await.unwrap();
            registry.set_vault_id(local_id);
            assert_eq!(
                registry
                    .recover_incomplete_enrollments(&initial_binding, &mut auth_client, &cred_store)
                    .await
                    .unwrap(),
                1
            );
            let active_connection = registry
                .get_vault_server_connection(initial_binding.server_connection_id())
                .await
                .unwrap()
                .unwrap();
            assert!(active_connection.device_id.is_some());
            assert!(active_connection.resolve_signing_key(&cred_store).is_some());
            assert!(active_connection
                .resolve_agreement_key(&cred_store)
                .is_some());
            assert_eq!(
                registry
                    .recover_incomplete_enrollments(&initial_binding, &mut auth_client, &cred_store)
                    .await
                    .unwrap(),
                0
            );
            assert_eq!(request_count(), before + 1);

            let previous = active_connection.device_id.as_deref().unwrap();
            transport.lose_registration_response.store(true, SeqCst);
            assert!(registry
                .enroll_connection_device(
                    &initial_binding,
                    &mut auth_client,
                    "Bootstrap vault",
                    "New connection identity",
                    &cred_store,
                    Some(previous)
                )
                .await
                .is_err());
            assert_eq!(
                registry
                    .get_vault_server_connection(initial_binding.server_connection_id())
                    .await
                    .unwrap()
                    .unwrap()
                    .device_id
                    .as_deref(),
                Some(previous)
            );
            drop(registry);
            let mut registry = HostStore::open(path).await.unwrap();
            registry.set_vault_id(VaultId::new("bootstrap-vault").unwrap());
            transport.lose_registration_response.store(false, SeqCst);
            assert_eq!(
                registry
                    .recover_incomplete_enrollments(&initial_binding, &mut auth_client, &cred_store)
                    .await
                    .unwrap(),
                1
            );
            assert_ne!(
                registry
                    .get_vault_server_connection(initial_binding.server_connection_id())
                    .await
                    .unwrap()
                    .unwrap()
                    .device_id
                    .as_deref(),
                Some(previous)
            );
            assert!(active_connection.resolve_signing_key(&cred_store).is_some());
            assert_eq!(request_count(), before + 2);
        }
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
            store.set_vault_id(VaultId::new(vault_a.to_string()).unwrap());
            let binding = VerifiedRemoteVaultBinding {
                local_vault_id: vault_a.to_string(),
                server_connection_id: "conn-1".into(),
                remote_vault_id: vault_a,
                server_instance_id: Some(transport.server_instance_id.to_string()),
                base_url: "https://server.test".into(),
            };

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
            let before = request_count();
            let err = store
                .accept_invitation_and_enroll_device_with_binding(
                    &binding,
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
                .accept_invitation_and_enroll_device_with_binding(
                    &binding,
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

            assert_eq!(request_count(), before);

            // 3. Valid invitation for vault A succeeds
            let (alice_mem, alice_dev) = store
                .accept_invitation_and_enroll_device_with_binding(
                    &binding,
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

            let member_b = TeamMember::new(
                "mem-alice-b",
                vault_b.to_string(),
                alice_id.to_string(),
                "Alice",
                ssh_key,
                TeamRole::Editor,
                "1000",
                "owner",
            );
            store.add_team_member(&member_b).await.unwrap();
            let mut binding_b = binding.clone();
            binding_b.local_vault_id = vault_b.to_string();
            binding_b.remote_vault_id = vault_b;
            store.set_vault_id(VaultId::new(vault_b.to_string()).unwrap());
            let before = request_count();
            assert!(store
                .replace_device_keys_for_member(
                    &binding_b,
                    &member_b.id,
                    Uuid::parse_str(&alice_dev.device_id).unwrap(),
                    "Cross-vault replacement",
                    &mut alice_client,
                    &cred_store,
                    None,
                )
                .await
                .is_err());
            assert_eq!(request_count(), before);
            store.set_vault_id(VaultId::new(vault_a.to_string()).unwrap());

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

            // 5. enroll_additional_device_for_member rejects cross-vault member_id
            let cross_vault_enroll_err = store
                .enroll_additional_device_for_member_with_binding(
                    &binding,
                    "mem-other-vault",
                    "Extra Device",
                    &mut alice_client,
                    &cred_store,
                )
                .await
                .unwrap_err();
            assert!(cross_vault_enroll_err
                .to_string()
                .contains("belongs to vault"));

            // 6. Interrupted enrollment operation recovery via recover_incomplete_enrollments
            store
                .save_enrollment_operation_progress(
                    "op-interrupted-1",
                    &vault_a.to_string(),
                    &alice_id.to_string(),
                    "mem-alice-a",
                    "InvitationAccepted",
                    None,
                    None,
                    "1006",
                )
                .await
                .unwrap();
            assert_eq!(
                store
                    .list_incomplete_enrollment_operations(&vault_a.to_string())
                    .await
                    .unwrap()
                    .len(),
                1
            );
            let ambiguous_err = store
                .recover_incomplete_enrollments(&binding, &mut alice_client, &cred_store)
                .await
                .unwrap_err();
            assert!(ambiguous_err
                .to_string()
                .contains("Manual intervention required"));

            let _ = std::fs::remove_file(&db_file);
        }
    }
}

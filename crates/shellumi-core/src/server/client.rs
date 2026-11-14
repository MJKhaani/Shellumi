//! Shellumi Server v1 HTTP client implementation.

use crate::server::error::ServerError;
use crate::server::types::*;
use crate::sync::{HttpRequest, HttpTransport, SystemHttpTransport};
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

/// Client for communicating with a remote Shellumi Server v1 instance.
#[derive(Clone, Debug)]
pub struct ShellumiServerClient<T: HttpTransport = SystemHttpTransport> {
    base_url: String,
    token: Option<String>,
    device_id: Option<Uuid>,
    transport: T,
}

impl ShellumiServerClient<SystemHttpTransport> {
    /// Creates a new client targeting the specified server base URL with the default system transport.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::with_transport(base_url, SystemHttpTransport::default())
    }
}

impl<T: HttpTransport> ShellumiServerClient<T> {
    /// Creates a new client targeting the specified base URL using a custom or mock `HttpTransport`.
    pub fn with_transport(base_url: impl Into<String>, transport: T) -> Self {
        let base = base_url.into().trim_end_matches('/').to_string();
        Self {
            base_url: base,
            token: None,
            device_id: None,
            transport,
        }
    }

    /// Sets the Bearer authentication token for subsequent requests.
    pub fn set_token(&mut self, token: impl Into<String>) {
        self.token = Some(token.into());
    }

    /// Clears the authentication token.
    pub fn clear_token(&mut self) {
        self.token = None;
    }

    /// Returns the currently configured Bearer authentication token.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Sets the local device UUID used for device-authenticated operations.
    pub fn set_device_id(&mut self, device_id: Uuid) {
        self.device_id = Some(device_id);
    }

    /// Returns the configured local device UUID.
    pub fn device_id(&self) -> Option<Uuid> {
        self.device_id
    }

    /// Returns the target server base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Formats a complete URL from a relative path.
    fn url(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');
        format!("{}/{}", self.base_url, path)
    }

    /// Prepares and sends an HTTP request, attaching auth and device headers, and parsing JSON responses.
    fn send_request<B: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&B>,
    ) -> Result<R, ServerError> {
        let url = self.url(path);
        let body_bytes = match body {
            Some(b) => {
                serde_json::to_vec(b).map_err(|e| ServerError::Serialization(e.to_string()))?
            }
            None => Vec::new(),
        };

        let mut req = match method {
            "GET" => HttpRequest::get(url),
            "POST" => HttpRequest::post(url, body_bytes),
            "PATCH" => HttpRequest::patch(url, body_bytes),
            "DELETE" => HttpRequest::delete(url),
            m => {
                return Err(ServerError::Network(format!(
                    "unsupported HTTP method: {m}"
                )))
            }
        };

        req = req.with_header("Accept", "application/json");

        if body.is_some() {
            req = req.with_header("Content-Type", "application/json");
        }

        if let Some(token) = &self.token {
            req = req.with_header("Authorization", format!("Bearer {token}"));
        }

        if let Some(dev_id) = &self.device_id {
            req = req.with_header("X-Device-Id", dev_id.to_string());
        }

        let resp = self.transport.send(req)?;

        if resp.status_code >= 300 {
            return Err(ServerError::from_http_status_and_body(
                resp.status_code,
                &resp.body,
            ));
        }

        serde_json::from_slice::<R>(&resp.body)
            .map_err(|e| ServerError::Deserialization(e.to_string()))
    }

    /// Sends a request where no JSON body is expected in response (e.g. 200 OK or 204 No Content).
    fn send_empty_response<B: Serialize>(
        &self,
        method: &str,
        path: &str,
        body: Option<&B>,
    ) -> Result<(), ServerError> {
        let url = self.url(path);
        let body_bytes = match body {
            Some(b) => {
                serde_json::to_vec(b).map_err(|e| ServerError::Serialization(e.to_string()))?
            }
            None => Vec::new(),
        };

        let mut req = match method {
            "GET" => HttpRequest::get(url),
            "POST" => HttpRequest::post(url, body_bytes),
            "PATCH" => HttpRequest::patch(url, body_bytes),
            "DELETE" => HttpRequest::delete(url),
            m => {
                return Err(ServerError::Network(format!(
                    "unsupported HTTP method: {m}"
                )))
            }
        };

        req = req.with_header("Accept", "application/json");

        if body.is_some() {
            req = req.with_header("Content-Type", "application/json");
        }

        if let Some(token) = &self.token {
            req = req.with_header("Authorization", format!("Bearer {token}"));
        }

        if let Some(dev_id) = &self.device_id {
            req = req.with_header("X-Device-Id", dev_id.to_string());
        }

        let resp = self.transport.send(req)?;

        if resp.status_code >= 300 {
            return Err(ServerError::from_http_status_and_body(
                resp.status_code,
                &resp.body,
            ));
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // System & Health Endpoints
    // -----------------------------------------------------------------------

    /// Checks the server liveness probe (`GET /health/live`).
    pub fn health_live(&self) -> Result<HealthResponse, ServerError> {
        self.send_request::<(), _>("GET", "health/live", None)
    }

    /// Checks the server readiness probe (`GET /health/ready`).
    pub fn health_ready(&self) -> Result<HealthResponse, ServerError> {
        self.send_request::<(), _>("GET", "health/ready", None)
    }

    /// Fetches server capabilities, wire protocol version, and limits (`GET /api/v1/capabilities`).
    pub fn capabilities(&self) -> Result<CapabilitiesResponse, ServerError> {
        self.send_request::<(), _>("GET", "api/v1/capabilities", None)
    }

    // -----------------------------------------------------------------------
    // Bootstrap & Authentication Endpoints
    // -----------------------------------------------------------------------

    /// Performs first-run initial admin bootstrap (`POST /api/v1/bootstrap`).
    pub fn bootstrap(&mut self, req: &BootstrapRequest) -> Result<BootstrapResponse, ServerError> {
        let resp: BootstrapResponse = self.send_request("POST", "api/v1/bootstrap", Some(req))?;
        self.set_token(resp.access_token.clone());
        Ok(resp)
    }

    /// Authenticates with username/email and password (`POST /api/v1/auth/login`).
    pub fn login(&mut self, req: &LoginRequest) -> Result<LoginResponse, ServerError> {
        let resp: LoginResponse = self.send_request("POST", "api/v1/auth/login", Some(req))?;
        self.set_token(resp.access_token.clone());
        Ok(resp)
    }

    /// Refreshes expired access tokens using a valid refresh token (`POST /api/v1/auth/refresh`).
    pub fn refresh_token(
        &mut self,
        req: &RefreshTokenRequest,
    ) -> Result<RefreshTokenResponse, ServerError> {
        let resp: RefreshTokenResponse =
            self.send_request("POST", "api/v1/auth/refresh", Some(req))?;
        self.set_token(resp.access_token.clone());
        Ok(resp)
    }

    /// Logs out and invalidates the session / refresh token (`POST /api/v1/auth/logout`).
    pub fn logout(&mut self, req: &LogoutRequest) -> Result<(), ServerError> {
        self.send_empty_response("POST", "api/v1/auth/logout", Some(req))?;
        self.clear_token();
        Ok(())
    }

    /// Retrieves current user profile and organization memberships (`GET /api/v1/me`).
    pub fn me(&self) -> Result<MeResponse, ServerError> {
        self.send_request::<(), _>("GET", "api/v1/me", None)
    }

    // -----------------------------------------------------------------------
    // Device Management Endpoints
    // -----------------------------------------------------------------------

    /// Lists all registered devices for the current user (`GET /api/v1/devices`).
    pub fn list_devices(&self) -> Result<Vec<DeviceResponse>, ServerError> {
        self.send_request::<(), _>("GET", "api/v1/devices", None)
    }

    /// Registers a new client device and its cryptographic public keys (`POST /api/v1/devices`).
    pub fn register_device(
        &mut self,
        req: &RegisterDeviceRequest,
    ) -> Result<DeviceResponse, ServerError> {
        let resp: DeviceResponse = self.send_request("POST", "api/v1/devices", Some(req))?;
        self.set_device_id(resp.device_id);
        Ok(resp)
    }

    /// Revokes a registered device (`DELETE /api/v1/devices/{device_id}`).
    pub fn revoke_device(&self, device_id: Uuid) -> Result<(), ServerError> {
        self.send_empty_response::<()>("DELETE", &format!("api/v1/devices/{device_id}"), None)
    }

    // -----------------------------------------------------------------------
    // Vault Management Endpoints
    // -----------------------------------------------------------------------

    /// Lists all vaults accessible to the authenticated user (`GET /api/v1/vaults`).
    pub fn list_vaults(&self) -> Result<VaultListResponse, ServerError> {
        self.send_request::<(), _>("GET", "api/v1/vaults", None)
    }

    /// Creates a new vault (`POST /api/v1/vaults`).
    pub fn create_vault(&self, req: &CreateVaultRequest) -> Result<VaultResponse, ServerError> {
        self.send_request("POST", "api/v1/vaults", Some(req))
    }

    /// Retrieves metadata and sync state for a vault (`GET /api/v1/vaults/{vault_id}`).
    pub fn get_vault(&self, vault_id: Uuid) -> Result<VaultResponse, ServerError> {
        self.send_request::<(), _>("GET", &format!("api/v1/vaults/{vault_id}"), None)
    }

    /// Updates vault metadata (e.g. encrypted name) (`PATCH /api/v1/vaults/{vault_id}`).
    pub fn update_vault(
        &self,
        vault_id: Uuid,
        req: &UpdateVaultRequest,
    ) -> Result<VaultResponse, ServerError> {
        self.send_request("PATCH", &format!("api/v1/vaults/{vault_id}"), Some(req))
    }

    /// Deletes a vault permanently (`DELETE /api/v1/vaults/{vault_id}`).
    pub fn delete_vault(&self, vault_id: Uuid) -> Result<(), ServerError> {
        self.send_empty_response::<()>("DELETE", &format!("api/v1/vaults/{vault_id}"), None)
    }

    // -----------------------------------------------------------------------
    // Synchronization & Commits Endpoints
    // -----------------------------------------------------------------------

    /// Commits a batch of mutations to a vault (`POST /api/v1/vaults/{vault_id}/commits`).
    pub fn commit(
        &self,
        vault_id: Uuid,
        batch: &CommitBatchRequest,
    ) -> Result<CommitBatchResponse, ServerError> {
        self.send_request(
            "POST",
            &format!("api/v1/vaults/{vault_id}/commits"),
            Some(batch),
        )
    }

    /// Fetches delta changes since a sequence number or cursor (`GET /api/v1/vaults/{vault_id}/changes`).
    pub fn fetch_changes(
        &self,
        vault_id: Uuid,
        since: Option<u64>,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<ChangeFeedResponse, ServerError> {
        let mut params = Vec::new();
        if let Some(s) = since {
            params.push(format!("since={s}"));
        }
        if let Some(c) = cursor {
            params.push(format!("cursor={c}"));
        }
        if let Some(l) = limit {
            params.push(format!("limit={l}"));
        }

        let query_string = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };

        let path = format!("api/v1/vaults/{vault_id}/changes{query_string}");
        self.send_request::<(), _>("GET", &path, None)
    }

    /// Fetches a full recovery snapshot for a vault revision (`GET /api/v1/vaults/{vault_id}/snapshots/{revision}`).
    pub fn fetch_snapshot(
        &self,
        vault_id: Uuid,
        revision: u64,
    ) -> Result<SnapshotResponse, ServerError> {
        self.send_request::<(), _>(
            "GET",
            &format!("api/v1/vaults/{vault_id}/snapshots/{revision}"),
            None,
        )
    }

    // -----------------------------------------------------------------------
    // Team Vault Members & Invites Endpoints
    // -----------------------------------------------------------------------

    /// Lists all members and roles in a vault (`GET /api/v1/vaults/{vault_id}/members`).
    pub fn list_members(&self, vault_id: Uuid) -> Result<VaultMemberListResponse, ServerError> {
        self.send_request::<(), _>("GET", &format!("api/v1/vaults/{vault_id}/members"), None)
    }

    /// Adds or updates a vault member's role (`POST /api/v1/vaults/{vault_id}/members`).
    pub fn add_member(
        &self,
        vault_id: Uuid,
        req: &AddVaultMemberRequest,
    ) -> Result<VaultMemberResponse, ServerError> {
        self.send_request(
            "POST",
            &format!("api/v1/vaults/{vault_id}/members"),
            Some(req),
        )
    }

    /// Removes a member from a vault (`DELETE /api/v1/vaults/{vault_id}/members/{user_id}`).
    pub fn remove_member(&self, vault_id: Uuid, user_id: Uuid) -> Result<(), ServerError> {
        self.send_empty_response::<()>(
            "DELETE",
            &format!("api/v1/vaults/{vault_id}/members/{user_id}"),
            None,
        )
    }

    /// Creates a one-time invitation to a vault (`POST /api/v1/vaults/{vault_id}/invites`).
    pub fn create_invite(
        &self,
        vault_id: Uuid,
        req: &CreateInviteRequest,
    ) -> Result<InviteResponse, ServerError> {
        self.send_request(
            "POST",
            &format!("api/v1/vaults/{vault_id}/invites"),
            Some(req),
        )
    }

    /// Lists active invitations for a vault (`GET /api/v1/vaults/{vault_id}/invites`).
    pub fn list_invites(&self, vault_id: Uuid) -> Result<InviteListResponse, ServerError> {
        self.send_request::<(), _>("GET", &format!("api/v1/vaults/{vault_id}/invites"), None)
    }

    /// Accepts an invitation token to join a vault (`POST /api/v1/invites/accept`).
    pub fn accept_invite(
        &self,
        req: &AcceptInviteRequest,
    ) -> Result<AcceptInviteResponse, ServerError> {
        self.send_request("POST", "api/v1/invites/accept", Some(req))
    }

    // -----------------------------------------------------------------------
    // Key Envelopes Endpoints
    // -----------------------------------------------------------------------

    /// Fetches wrapped key envelopes for the current device in a vault (`GET /api/v1/vaults/{vault_id}/keys/envelopes`).
    pub fn list_key_envelopes(
        &self,
        vault_id: Uuid,
    ) -> Result<KeyEnvelopeListResponse, ServerError> {
        self.send_request::<(), _>(
            "GET",
            &format!("api/v1/vaults/{vault_id}/keys/envelopes"),
            None,
        )
    }

    /// Publishes wrapped key envelopes for recipient devices (`POST /api/v1/vaults/{vault_id}/keys/envelopes`).
    pub fn publish_key_envelopes(
        &self,
        vault_id: Uuid,
        req: &PublishKeyEnvelopesRequest,
    ) -> Result<(), ServerError> {
        self.send_empty_response(
            "POST",
            &format!("api/v1/vaults/{vault_id}/keys/envelopes"),
            Some(req),
        )
    }

    /// Rotates the key epoch and publishes newly wrapped envelopes (`POST /api/v1/vaults/{vault_id}/keys/epochs`).
    pub fn rotate_key_epoch(
        &self,
        vault_id: Uuid,
        req: &RotateKeyEpochRequest,
    ) -> Result<RotateKeyEpochResponse, ServerError> {
        self.send_request(
            "POST",
            &format!("api/v1/vaults/{vault_id}/keys/epochs"),
            Some(req),
        )
    }

    // -----------------------------------------------------------------------
    // Live Terminal Collaboration Endpoints
    // -----------------------------------------------------------------------

    /// Creates a new collaboration session (`POST /api/v1/collab/sessions`).
    pub fn create_collab_session(
        &self,
        req: &CreateCollabSessionRequest,
    ) -> Result<CollabSessionResponse, ServerError> {
        self.send_request("POST", "api/v1/collab/sessions", Some(req))
    }

    /// Lists active collaboration sessions (`GET /api/v1/collab/sessions`).
    pub fn list_collab_sessions(&self) -> Result<CollabSessionListResponse, ServerError> {
        self.send_request::<(), _>("GET", "api/v1/collab/sessions", None)
    }

    /// Enrolls or joins a participant in a collaboration session (`POST /api/v1/collab/sessions/{session_id}/participants`).
    pub fn enroll_collab_participant(
        &self,
        session_id: Uuid,
        req: &EnrollParticipantRequest,
    ) -> Result<CollabParticipantResponse, ServerError> {
        self.send_request(
            "POST",
            &format!("api/v1/collab/sessions/{session_id}/participants"),
            Some(req),
        )
    }

    /// Grants terminal write control to a participant (`POST /api/v1/collab/sessions/{session_id}/control/grant`).
    pub fn grant_collab_control(
        &self,
        session_id: Uuid,
        req: &GrantControlRequest,
    ) -> Result<(), ServerError> {
        self.send_empty_response(
            "POST",
            &format!("api/v1/collab/sessions/{session_id}/control/grant"),
            Some(req),
        )
    }

    /// Submits a collaboration frame to the relay (`POST /api/v1/collab/sessions/{session_id}/frames`).
    pub fn send_collab_frame(
        &self,
        session_id: Uuid,
        req: &SendCollabFrameRequest,
    ) -> Result<(), ServerError> {
        self.send_empty_response(
            "POST",
            &format!("api/v1/collab/sessions/{session_id}/frames"),
            Some(req),
        )
    }

    /// Polls collaboration frames since sequence (`GET /api/v1/collab/sessions/{session_id}/poll`).
    pub fn poll_collab_frames(
        &self,
        session_id: Uuid,
        query: &CollabPollQuery,
    ) -> Result<CollabPollResponse, ServerError> {
        let mut params = Vec::new();
        if let Some(s) = query.since_sequence {
            params.push(format!("since_sequence={s}"));
        }
        if let Some(l) = query.limit {
            params.push(format!("limit={l}"));
        }

        let query_string = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };

        let path = format!("api/v1/collab/sessions/{session_id}/poll{query_string}");
        self.send_request::<(), _>("GET", &path, None)
    }

    // -----------------------------------------------------------------------
    // Audit Log Endpoints
    // -----------------------------------------------------------------------

    /// Fetches the immutable hash-chained audit log for a vault (`GET /api/v1/vaults/{vault_id}/audit`).
    pub fn get_audit_log(&self, vault_id: Uuid) -> Result<AuditLogResponse, ServerError> {
        self.send_request::<(), _>("GET", &format!("api/v1/vaults/{vault_id}/audit"), None)
    }
}

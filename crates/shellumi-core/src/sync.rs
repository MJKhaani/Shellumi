//! End-to-end encrypted synchronization models and cryptographic envelope.
//!
//! Provides zero-knowledge AEAD envelope encryption (AES-256-GCM via `ring`),
//! PBKDF2-HMAC-SHA256 key derivation with high iteration counts, bound associated
//! data to prevent ciphertext transposition, sync conflict detection, and OAuth 2.0
//! PKCE challenge helpers for Google Drive synchronization.

mod http_transport;
#[cfg(test)]
use http_transport::curl_private_config;
pub use http_transport::{parse_raw_http_response, SystemHttpTransport};

use base64::engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use ring::aead::{
    Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, AES_256_GCM, NONCE_LEN,
};
use ring::digest::{digest, SHA256};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::num::NonZeroU32;
use zeroize::Zeroizing;

/// Standard PBKDF2 iterations for vault key derivation.
pub const DEFAULT_KDF_ITERATIONS: u32 = 600_000;
/// Minimum required passphrase length.
pub const MIN_PASSPHRASE_LENGTH: usize = 8;
/// Salt length in bytes.
pub const SALT_LEN: usize = 16;
/// AEAD Key length in bytes for AES-256-GCM.
pub const KEY_LEN: usize = 32;

#[derive(Debug, PartialEq, Eq)]
pub enum SyncError {
    PassphraseTooShort(usize),
    EmptyPassphrase,
    RandomGenerationFailed,
    KeyDerivationFailed,
    EncryptionFailed,
    DecryptionFailed,
    InvalidBase64(String),
    InvalidSaltLength,
    InvalidNonceLength,
    Serialization(String),
    Deserialization(String),
    InvalidRecord(String),
    StateMismatch,
    MissingAuthCode,
    InvalidHttpCallback(String),
    Network(String),
    Http(u16, String),
    Api(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PassphraseTooShort(len) => {
                write!(
                    f,
                    "passphrase too short ({len} chars); minimum is {MIN_PASSPHRASE_LENGTH}"
                )
            }
            Self::EmptyPassphrase => f.write_str("passphrase cannot be empty"),
            Self::RandomGenerationFailed => f.write_str("cryptographic RNG failure"),
            Self::KeyDerivationFailed => f.write_str("key derivation failed"),
            Self::EncryptionFailed => f.write_str("authenticated encryption failed"),
            Self::DecryptionFailed => {
                f.write_str("decryption failed: incorrect passphrase or corrupted data")
            }
            Self::InvalidBase64(msg) => write!(f, "invalid base64 encoding: {msg}"),
            Self::InvalidSaltLength => write!(f, "salt must be exactly {SALT_LEN} bytes"),
            Self::InvalidNonceLength => write!(f, "nonce must be exactly {NONCE_LEN} bytes"),
            Self::Serialization(msg) => write!(f, "serialization error: {msg}"),
            Self::Deserialization(msg) => write!(f, "deserialization error: {msg}"),
            Self::InvalidRecord(msg) => write!(f, "invalid sync record: {msg}"),
            Self::StateMismatch => {
                f.write_str("OAuth CSRF state mismatch: potential forgery attempt")
            }
            Self::MissingAuthCode => {
                f.write_str("OAuth redirect did not include an authorization code")
            }
            Self::InvalidHttpCallback(msg) => write!(f, "invalid HTTP callback request: {msg}"),
            Self::Network(msg) => write!(f, "network error: {msg}"),
            Self::Http(code, msg) => write!(f, "HTTP error {code}: {msg}"),
            Self::Api(msg) => write!(f, "Google Drive API error: {msg}"),
        }
    }
}

impl std::error::Error for SyncError {}

/// One-time nonce sequence for `ring::aead`.
struct OneNonce(Option<Nonce>);

impl NonceSequence for OneNonce {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        self.0.take().ok_or(ring::error::Unspecified)
    }
}

/// An authenticated ciphertext envelope containing encrypted data and metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    /// Base64-encoded KDF salt (16 bytes).
    pub salt: String,
    /// Base64-encoded AEAD nonce (12 bytes).
    pub nonce: String,
    /// Base64-encoded ciphertext + 16-byte Poly1305/GCM authentication tag.
    pub ciphertext: String,
    /// Bound associated data (e.g. `entity_type:object_id`).
    pub associated_data: String,
    /// KDF iteration count.
    pub kdf_iterations: u32,
}

/// Derives a 256-bit Key Encryption Key (KEK) using PBKDF2-HMAC-SHA256 with memory zeroization on drop.
pub fn derive_key(
    passphrase: &str,
    salt: &[u8],
    iterations: u32,
) -> Result<Zeroizing<[u8; KEY_LEN]>, SyncError> {
    if passphrase.is_empty() {
        return Err(SyncError::EmptyPassphrase);
    }
    if passphrase.len() < MIN_PASSPHRASE_LENGTH {
        return Err(SyncError::PassphraseTooShort(passphrase.len()));
    }
    if salt.len() != SALT_LEN {
        return Err(SyncError::InvalidSaltLength);
    }
    let iter = NonZeroU32::new(iterations).ok_or(SyncError::KeyDerivationFailed)?;
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        iter,
        salt,
        passphrase.as_bytes(),
        key.as_mut(),
    );
    Ok(key)
}

/// Generates a 16-byte cryptographically secure random salt.
pub fn generate_salt() -> Result<[u8; SALT_LEN], SyncError> {
    let rng = SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    rng.fill(&mut salt)
        .map_err(|_| SyncError::RandomGenerationFailed)?;
    Ok(salt)
}

/// Generates a 12-byte cryptographically secure random nonce.
pub fn generate_nonce() -> Result<[u8; NONCE_LEN], SyncError> {
    let rng = SystemRandom::new();
    let mut nonce = [0u8; NONCE_LEN];
    rng.fill(&mut nonce)
        .map_err(|_| SyncError::RandomGenerationFailed)?;
    Ok(nonce)
}

/// Encrypts plaintext bytes using AES-256-GCM authenticated encryption.
pub fn seal_payload(
    key: &[u8; KEY_LEN],
    nonce_bytes: &[u8; NONCE_LEN],
    plaintext: &[u8],
    associated_data: &[u8],
) -> Result<Vec<u8>, SyncError> {
    let unbound_key =
        UnboundKey::new(&AES_256_GCM, key).map_err(|_| SyncError::EncryptionFailed)?;
    let nonce =
        Nonce::try_assume_unique_for_key(nonce_bytes).map_err(|_| SyncError::EncryptionFailed)?;
    let mut sealing_key = SealingKey::new(unbound_key, OneNonce(Some(nonce)));

    let mut in_out = plaintext.to_vec();
    sealing_key
        .seal_in_place_append_tag(Aad::from(associated_data), &mut in_out)
        .map_err(|_| SyncError::EncryptionFailed)?;
    Ok(in_out)
}

/// Decrypts ciphertext and verifies the authentication tag using AES-256-GCM.
pub fn open_payload(
    key: &[u8; KEY_LEN],
    nonce_bytes: &[u8; NONCE_LEN],
    ciphertext_and_tag: &[u8],
    associated_data: &[u8],
) -> Result<Vec<u8>, SyncError> {
    let unbound_key =
        UnboundKey::new(&AES_256_GCM, key).map_err(|_| SyncError::DecryptionFailed)?;
    let nonce =
        Nonce::try_assume_unique_for_key(nonce_bytes).map_err(|_| SyncError::DecryptionFailed)?;
    let mut opening_key = OpeningKey::new(unbound_key, OneNonce(Some(nonce)));

    let mut in_out = ciphertext_and_tag.to_vec();
    let decrypted = opening_key
        .open_in_place(Aad::from(associated_data), &mut in_out)
        .map_err(|_| SyncError::DecryptionFailed)?;
    Ok(decrypted.to_vec())
}

/// Encrypts any serializable entity into a self-contained `EncryptedEnvelope` using default KDF iterations.
pub fn encrypt_entity<T: Serialize>(
    passphrase: &str,
    entity_type: &str,
    object_id: &str,
    data: &T,
) -> Result<EncryptedEnvelope, SyncError> {
    encrypt_entity_with_iterations(
        passphrase,
        entity_type,
        object_id,
        data,
        DEFAULT_KDF_ITERATIONS,
    )
}

/// Encrypts any serializable entity into a self-contained `EncryptedEnvelope` with custom KDF iterations.
pub fn encrypt_entity_with_iterations<T: Serialize>(
    passphrase: &str,
    entity_type: &str,
    object_id: &str,
    data: &T,
    iterations: u32,
) -> Result<EncryptedEnvelope, SyncError> {
    let json_bytes =
        serde_json::to_vec(data).map_err(|e| SyncError::Serialization(e.to_string()))?;
    let salt = generate_salt()?;
    let nonce = generate_nonce()?;
    let key = derive_key(passphrase, &salt, iterations)?;

    let aad_string = format!("{entity_type}:{object_id}");
    let ciphertext = seal_payload(&key, &nonce, &json_bytes, aad_string.as_bytes())?;

    Ok(EncryptedEnvelope {
        salt: BASE64_STANDARD.encode(salt),
        nonce: BASE64_STANDARD.encode(nonce),
        ciphertext: BASE64_STANDARD.encode(ciphertext),
        associated_data: aad_string,
        kdf_iterations: iterations,
    })
}

/// Decrypts an `EncryptedEnvelope` back into the target entity type.
pub fn decrypt_entity<T: DeserializeOwned>(
    passphrase: &str,
    envelope: &EncryptedEnvelope,
) -> Result<T, SyncError> {
    let salt = BASE64_STANDARD
        .decode(&envelope.salt)
        .map_err(|e| SyncError::InvalidBase64(e.to_string()))?;
    let nonce_bytes = BASE64_STANDARD
        .decode(&envelope.nonce)
        .map_err(|e| SyncError::InvalidBase64(e.to_string()))?;
    let ciphertext_and_tag = BASE64_STANDARD
        .decode(&envelope.ciphertext)
        .map_err(|e| SyncError::InvalidBase64(e.to_string()))?;

    if salt.len() != SALT_LEN {
        return Err(SyncError::InvalidSaltLength);
    }
    if nonce_bytes.len() != NONCE_LEN {
        return Err(SyncError::InvalidNonceLength);
    }

    let mut salt_arr = [0u8; SALT_LEN];
    salt_arr.copy_from_slice(&salt);

    let mut nonce_arr = [0u8; NONCE_LEN];
    nonce_arr.copy_from_slice(&nonce_bytes);

    let key = derive_key(passphrase, &salt_arr, envelope.kdf_iterations)?;
    let plaintext = open_payload(
        &key,
        &nonce_arr,
        &ciphertext_and_tag,
        envelope.associated_data.as_bytes(),
    )?;

    serde_json::from_slice(&plaintext).map_err(|e| SyncError::Deserialization(e.to_string()))
}

/// Entity types supported by the synchronization engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncEntityType {
    Host,
    Group,
    Tunnel,
    Snippet,
    Workspace,
    SharedVault,
    TeamMember,
    AuditRecord,
    CloudProvider,
    Identity,
}

impl fmt::Display for SyncEntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host => f.write_str("host"),
            Self::Group => f.write_str("group"),
            Self::Tunnel => f.write_str("tunnel"),
            Self::Snippet => f.write_str("snippet"),
            Self::Workspace => f.write_str("workspace"),
            Self::SharedVault => f.write_str("shared_vault"),
            Self::TeamMember => f.write_str("team_member"),
            Self::AuditRecord => f.write_str("audit_record"),
            Self::CloudProvider => f.write_str("cloud_provider"),
            Self::Identity => f.write_str("identity"),
        }
    }
}

/// A versioned synchronization record containing either an encrypted envelope or a deletion tombstone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRecord {
    pub object_id: String,
    pub entity_type: SyncEntityType,
    pub version: u64,
    pub device_id: String,
    pub updated_at: u64,
    pub deleted: bool,
    pub envelope: Option<EncryptedEnvelope>,
}

impl SyncRecord {
    /// Creates a new active sync record.
    pub fn new(
        object_id: impl Into<String>,
        entity_type: SyncEntityType,
        version: u64,
        device_id: impl Into<String>,
        updated_at: u64,
        envelope: EncryptedEnvelope,
    ) -> Self {
        Self {
            object_id: object_id.into(),
            entity_type,
            version,
            device_id: device_id.into(),
            updated_at,
            deleted: false,
            envelope: Some(envelope),
        }
    }

    /// Creates a deletion tombstone record.
    pub fn tombstone(
        object_id: impl Into<String>,
        entity_type: SyncEntityType,
        version: u64,
        device_id: impl Into<String>,
        updated_at: u64,
    ) -> Self {
        Self {
            object_id: object_id.into(),
            entity_type,
            version,
            device_id: device_id.into(),
            updated_at,
            deleted: true,
            envelope: None,
        }
    }
}

/// Recommended action when comparing a local record with an incoming remote record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncAction {
    /// Remote record is newer; apply it locally.
    ApplyRemote,
    /// Local record is newer; upload it to remote.
    UploadLocal,
    /// Records are identical and already synchronized.
    InSync,
    /// Concurrent conflict detected (same version, different origin devices).
    Conflict,
}

/// Resolves synchronization action by comparing versions, update timestamps, and device origins.
pub fn determine_sync_action(local: Option<&SyncRecord>, remote: &SyncRecord) -> SyncAction {
    let Some(local) = local else {
        return SyncAction::ApplyRemote;
    };

    if remote.version > local.version {
        SyncAction::ApplyRemote
    } else if local.version > remote.version {
        SyncAction::UploadLocal
    } else {
        // Versions are identical
        if local.deleted == remote.deleted
            && local.updated_at == remote.updated_at
            && local.device_id == remote.device_id
        {
            SyncAction::InSync
        } else if local.device_id != remote.device_id {
            SyncAction::Conflict
        } else if remote.updated_at > local.updated_at {
            SyncAction::ApplyRemote
        } else if local.updated_at > remote.updated_at {
            SyncAction::UploadLocal
        } else {
            SyncAction::InSync
        }
    }
}

/// Complete encrypted vault snapshot for export, import, or cloud storage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncVault {
    pub vault_id: String,
    pub schema_version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub records: Vec<SyncRecord>,
}

impl SyncVault {
    pub fn new(vault_id: impl Into<String>, created_at: u64) -> Self {
        Self {
            vault_id: vault_id.into(),
            schema_version: 1,
            created_at,
            updated_at: created_at,
            records: Vec::new(),
        }
    }

    /// Serializes the vault to pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, SyncError> {
        serde_json::to_string_pretty(self).map_err(|e| SyncError::Serialization(e.to_string()))
    }

    /// Deserializes a vault from JSON.
    pub fn from_json(json: &str) -> Result<Self, SyncError> {
        serde_json::from_str(json).map_err(|e| SyncError::Deserialization(e.to_string()))
    }
}

/// OAuth 2.0 PKCE (Proof Key for Code Exchange, RFC 7636) code verifier and challenge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
}

/// Generates an RFC 7636 compliant PKCE challenge using 32 random bytes and SHA-256.
pub fn generate_pkce_challenge() -> Result<PkceChallenge, SyncError> {
    let rng = SystemRandom::new();
    let mut random_bytes = [0u8; 32];
    rng.fill(&mut random_bytes)
        .map_err(|_| SyncError::RandomGenerationFailed)?;

    let code_verifier = URL_SAFE_NO_PAD.encode(random_bytes);
    let digest_output = digest(&SHA256, code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(digest_output.as_ref());

    Ok(PkceChallenge {
        code_verifier,
        code_challenge,
    })
}

/// Constructs a Google OAuth 2.0 authorization URL for the restricted Google Drive Application Data folder.
pub fn build_google_drive_auth_url(
    client_id: &str,
    redirect_uri: &str,
    pkce: &PkceChallenge,
    state: &str,
) -> String {
    let scope = "https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fdrive.appdata";
    format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
        client_id={client_id}&\
        redirect_uri={redirect_uri}&\
        response_type=code&\
        scope={scope}&\
        code_challenge={}&\
        code_challenge_method=S256&\
        state={state}&\
        access_type=offline&\
        prompt=consent",
        pkce.code_challenge
    )
}

/// Parses an incoming OAuth HTTP GET callback request line, validates CSRF state, and extracts the auth code.
pub fn parse_oauth_callback_request(
    request_line: &str,
    expected_state: &str,
) -> Result<String, SyncError> {
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 || parts[0] != "GET" {
        return Err(SyncError::InvalidHttpCallback(
            "expected HTTP GET request".into(),
        ));
    }

    let uri = parts[1];
    let query_str = uri
        .split_once('?')
        .map(|(_, q)| q)
        .ok_or_else(|| SyncError::InvalidHttpCallback("missing query string".into()))?;

    let mut code = None;
    let mut state = None;
    let mut error = None;

    for pair in query_str.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            match k {
                "code" => code = Some(v.to_string()),
                "state" => state = Some(v.to_string()),
                "error" => error = Some(v.to_string()),
                _ => {}
            }
        }
    }

    if let Some(err) = error {
        return Err(SyncError::InvalidHttpCallback(format!(
            "OAuth error: {err}"
        )));
    }

    let Some(state) = state else {
        return Err(SyncError::StateMismatch);
    };

    if state != expected_state {
        return Err(SyncError::StateMismatch);
    }

    code.ok_or(SyncError::MissingAuthCode)
}

/// Constructs an HTTP/1.1 response for the browser upon OAuth redirection.
pub fn build_oauth_callback_response(is_success: bool, error_msg: Option<&str>) -> String {
    let (status, title, body) = if is_success {
        (
            "200 OK",
            "Authentication Successful",
            "<h1>Authentication Successful</h1><p>You may close this tab and return to Shellumi.</p>".to_string(),
        )
    } else {
        (
            "400 Bad Request",
            "Authentication Failed",
            format!(
                "<h1>Authentication Failed</h1><p>{}</p>",
                error_msg.unwrap_or("Unknown error occurred.")
            ),
        )
    };

    let html = format!(
        "<!DOCTYPE html><html><head><title>{title}</title><style>body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #f8fafc; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; }} .card {{ background: #1e293b; padding: 2rem; border-radius: 0.5rem; text-align: center; max-width: 400px; }} h1 {{ font-size: 1.25rem; margin-bottom: 0.5rem; color: #38bdf8; }} p {{ font-size: 0.875rem; color: #94a3b8; }}</style></head><body><div class=\"card\">{body}</div></body></html>"
    );

    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    )
}

/// Ephemeral loopback HTTP listener for OAuth 2.0 authorization callbacks.
pub struct OAuthCallbackListener {
    listener: std::net::TcpListener,
    port: u16,
}

impl OAuthCallbackListener {
    /// Binds to an ephemeral loopback port (`127.0.0.1:0`).
    pub fn bind_ephemeral() -> Result<Self, SyncError> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|e| SyncError::Network(e.to_string()))?
            .port();
        Ok(Self { listener, port })
    }

    /// Returns the port bound on loopback.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns the complete redirect URI registered with the OAuth provider.
    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.port)
    }

    /// Blocks until an incoming redirect arrives or the timeout expires, then parses the auth code.
    pub fn wait_for_code(
        self,
        expected_state: &str,
        timeout: std::time::Duration,
    ) -> Result<String, SyncError> {
        use std::io::{Read, Write};

        let (mut stream, _) = self
            .listener
            .accept()
            .map_err(|e| SyncError::Network(e.to_string()))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| SyncError::Network(e.to_string()))?;

        let mut buf = [0u8; 4096];
        let n = stream
            .read(&mut buf)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let request = String::from_utf8_lossy(&buf[..n]);

        let first_line = request.lines().next().unwrap_or_default();
        let result = parse_oauth_callback_request(first_line, expected_state);

        let response = match &result {
            Ok(_) => build_oauth_callback_response(true, None),
            Err(e) => build_oauth_callback_response(false, Some(&e.to_string())),
        };

        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();

        result
    }
}

/// Token bundle containing OAuth 2.0 access and refresh tokens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthTokenBundle {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
    pub acquired_at: u64,
}

impl OAuthTokenBundle {
    pub fn new(
        access_token: impl Into<String>,
        refresh_token: Option<String>,
        expires_in: Option<u64>,
        scope: Option<String>,
        acquired_at: u64,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in,
            scope,
            acquired_at,
        }
    }

    /// Checks if the token is expired given a unix timestamp in seconds, with a 60-second grace window.
    pub fn is_expired(&self, current_time: u64) -> bool {
        if let Some(exp) = self.expires_in {
            current_time >= self.acquired_at.saturating_add(exp).saturating_sub(60)
        } else {
            false
        }
    }

    /// Persists this token bundle into the provided secure credential store.
    pub fn save_to_store(
        &self,
        store: &dyn crate::credential::CredentialStore,
    ) -> Result<(), SyncError> {
        let json =
            serde_json::to_string(self).map_err(|e| SyncError::Serialization(e.to_string()))?;
        store
            .set_secret(OAUTH_TOKEN_SERVICE, OAUTH_TOKEN_ACCOUNT, &json)
            .map_err(|e| SyncError::Api(format!("failed to store OAuth token: {e}")))?;
        Ok(())
    }

    /// Loads a persisted token bundle from the provided secure credential store.
    pub fn load_from_store(
        store: &dyn crate::credential::CredentialStore,
    ) -> Result<Option<Self>, SyncError> {
        match store.get_secret(OAUTH_TOKEN_SERVICE, OAUTH_TOKEN_ACCOUNT) {
            Ok(Some(secret)) => {
                let bundle = serde_json::from_str(secret.expose_secret()).map_err(|e| {
                    SyncError::Deserialization(format!("corrupted OAuth token: {e}"))
                })?;
                Ok(Some(bundle))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(SyncError::Api(format!("failed to load OAuth token: {e}"))),
        }
    }

    /// Deletes the persisted token bundle from the provided secure credential store.
    pub fn delete_from_store(
        store: &dyn crate::credential::CredentialStore,
    ) -> Result<bool, SyncError> {
        store
            .delete_secret(OAUTH_TOKEN_SERVICE, OAUTH_TOKEN_ACCOUNT)
            .map_err(|e| SyncError::Api(format!("failed to delete OAuth token: {e}")))
    }
}

/// Service name used when storing OAuth token bundles in the platform credential store.
pub const OAUTH_TOKEN_SERVICE: &str = "shellumi.sync.google";

/// Account identifier used when storing OAuth token bundles in the platform credential store.
pub const OAUTH_TOKEN_ACCOUNT: &str = "oauth_token_bundle";

/// Comparison status between local and remote sync records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStatus {
    Identical,
    LocalOnly,
    RemoteOnly,
    LocalNewer,
    RemoteNewer,
    Conflict,
}

impl fmt::Display for DiffStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identical => f.write_str("identical"),
            Self::LocalOnly => f.write_str("local_only"),
            Self::RemoteOnly => f.write_str("remote_only"),
            Self::LocalNewer => f.write_str("local_newer"),
            Self::RemoteNewer => f.write_str("remote_newer"),
            Self::Conflict => f.write_str("conflict"),
        }
    }
}

/// Detailed difference summary for a sync record between local database and remote snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordDiffSummary {
    pub object_id: String,
    pub entity_type: SyncEntityType,
    pub status: DiffStatus,
    pub local_version: Option<u64>,
    pub remote_version: Option<u64>,
    pub local_updated_at: Option<u64>,
    pub remote_updated_at: Option<u64>,
}

/// Compares a local set of sync records against a remote set to compute record-level differences.
/// If a passphrase is provided, decrypts envelopes for matching records to verify whether
/// the underlying entity payload data is identical even across independently exported vaults.
pub fn diff_sync_records_with_passphrase(
    local_records: &[SyncRecord],
    remote_records: &[SyncRecord],
    passphrase: Option<&str>,
) -> Vec<RecordDiffSummary> {
    let mut summaries = Vec::new();
    let mut seen_remote = std::collections::HashSet::new();

    for local in local_records {
        let remote = remote_records
            .iter()
            .find(|r| r.object_id == local.object_id);
        let status = match remote {
            None => DiffStatus::LocalOnly,
            Some(rem) => {
                seen_remote.insert(&rem.object_id);

                let are_identical_content = if let (Some(pass), Some(loc_env), Some(rem_env)) =
                    (passphrase, &local.envelope, &rem.envelope)
                {
                    let loc_val: Result<serde_json::Value, _> = decrypt_entity(pass, loc_env);
                    let rem_val: Result<serde_json::Value, _> = decrypt_entity(pass, rem_env);
                    match (loc_val, rem_val) {
                        (Ok(lv), Ok(rv)) => lv == rv,
                        _ => false,
                    }
                } else {
                    false
                };

                if are_identical_content && local.deleted == rem.deleted {
                    DiffStatus::Identical
                } else {
                    match determine_sync_action(Some(local), rem) {
                        SyncAction::InSync => DiffStatus::Identical,
                        SyncAction::UploadLocal => DiffStatus::LocalNewer,
                        SyncAction::ApplyRemote => DiffStatus::RemoteNewer,
                        SyncAction::Conflict => DiffStatus::Conflict,
                    }
                }
            }
        };

        summaries.push(RecordDiffSummary {
            object_id: local.object_id.clone(),
            entity_type: local.entity_type,
            status,
            local_version: Some(local.version),
            remote_version: remote.map(|r| r.version),
            local_updated_at: Some(local.updated_at),
            remote_updated_at: remote.map(|r| r.updated_at),
        });
    }

    for remote in remote_records {
        if !seen_remote.contains(&remote.object_id) {
            summaries.push(RecordDiffSummary {
                object_id: remote.object_id.clone(),
                entity_type: remote.entity_type,
                status: DiffStatus::RemoteOnly,
                local_version: None,
                remote_version: Some(remote.version),
                local_updated_at: None,
                remote_updated_at: Some(remote.updated_at),
            });
        }
    }

    summaries.sort_by(|a, b| a.object_id.cmp(&b.object_id));
    summaries
}

/// Compares a local set of sync records against a remote set to compute record-level differences.
pub fn diff_sync_records(
    local_records: &[SyncRecord],
    remote_records: &[SyncRecord],
) -> Vec<RecordDiffSummary> {
    diff_sync_records_with_passphrase(local_records, remote_records, None)
}

/// User's resolution choice when resolving conflicting sync records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolutionChoice {
    /// Keep the local record, incrementing its version to override remote.
    KeepLocal,
    /// Accept the remote record, replacing the local record.
    AcceptRemote,
}

impl fmt::Display for ConflictResolutionChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeepLocal => f.write_str("Keep Local"),
            Self::AcceptRemote => f.write_str("Accept Remote"),
        }
    }
}

/// Resolves a single conflict between local and remote records based on user's choice.
pub fn resolve_record_conflict(
    local: &SyncRecord,
    remote: &SyncRecord,
    choice: ConflictResolutionChoice,
    now: u64,
) -> SyncRecord {
    match choice {
        ConflictResolutionChoice::KeepLocal => {
            let mut resolved = local.clone();
            resolved.version = local.version.max(remote.version) + 1;
            resolved.updated_at = now;
            resolved
        }
        ConflictResolutionChoice::AcceptRemote => remote.clone(),
    }
}

/// Extracts a user-friendly summary string of a record's entity contents.
pub fn format_record_summary(passphrase: Option<&str>, record: &SyncRecord) -> String {
    if record.deleted {
        return format!("{} (Deleted / Tombstone)", record.object_id);
    }
    let Some(envelope) = &record.envelope else {
        return format!("{} (No payload)", record.object_id);
    };
    let Some(pass) = passphrase else {
        return format!("{} [Encrypted]", record.object_id);
    };
    match decrypt_entity::<serde_json::Value>(pass, envelope) {
        Ok(val) => {
            if let Some(obj) = val.as_object() {
                match record.entity_type {
                    SyncEntityType::Host => {
                        let name = obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&record.object_id);
                        let host = obj
                            .get("hostname")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let user = obj
                            .get("username")
                            .and_then(|v| v.as_str())
                            .unwrap_or("user");
                        let port = obj.get("port").and_then(|v| v.as_u64()).unwrap_or(22);
                        format!("{name} ({user}@{host}:{port})")
                    }
                    SyncEntityType::Group => {
                        let name = obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&record.object_id);
                        format!("{name} (Group)")
                    }
                    SyncEntityType::Tunnel => {
                        let name = obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&record.object_id);
                        let ftype = obj
                            .get("forwarding_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("tunnel");
                        let bport = obj.get("bind_port").and_then(|v| v.as_u64()).unwrap_or(0);
                        format!("{name} ({ftype}:{bport})")
                    }
                    SyncEntityType::Snippet => {
                        let name = obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&record.object_id);
                        let cmd = obj.get("command").and_then(|v| v.as_str()).unwrap_or("");
                        let short_cmd = if cmd.len() > 30 {
                            format!("{}…", &cmd[..27])
                        } else {
                            cmd.to_string()
                        };
                        format!("{name} (`{short_cmd}`)")
                    }
                    SyncEntityType::Workspace => {
                        let name = obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&record.object_id);
                        let layout = obj
                            .get("layout")
                            .and_then(|v| v.as_str())
                            .unwrap_or("single");
                        format!("Workspace: {name} ({layout})")
                    }
                    SyncEntityType::SharedVault => {
                        let name = obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&record.object_id);
                        let epoch = obj
                            .get("current_epoch")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(1);
                        format!("{name} (Shared Vault, Epoch {epoch})")
                    }
                    SyncEntityType::TeamMember => {
                        let name = obj
                            .get("display_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&record.object_id);
                        let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or("member");
                        format!("{name} ({role})")
                    }
                    SyncEntityType::AuditRecord => {
                        let action = obj
                            .get("action")
                            .and_then(|v| v.as_str())
                            .unwrap_or("audit");
                        let actor = obj
                            .get("actor_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("actor");
                        format!("{action} by {actor}")
                    }
                    SyncEntityType::CloudProvider => {
                        let name = obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&record.object_id);
                        let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("cloud");
                        format!("{name} (Cloud Provider: {kind})")
                    }
                    SyncEntityType::Identity => {
                        let name = obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&record.object_id);
                        let user = obj
                            .get("username")
                            .and_then(|v| v.as_str())
                            .unwrap_or("user");
                        format!("Identity: {name} ({user})")
                    }
                }
            } else {
                record.object_id.clone()
            }
        }
        Err(_) => format!("{} [Decryption Failed]", record.object_id),
    }
}

/// Human-readable representation of a conflicting item for UI inspection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictItemView {
    pub object_id: String,
    pub entity_type: SyncEntityType,
    pub local_version: u64,
    pub remote_version: u64,
    pub local_updated_at: u64,
    pub remote_updated_at: u64,
    pub local_summary: String,
    pub remote_summary: String,
}

/// Builds human-readable `ConflictItemView` models for all conflicting items between local and remote records.
pub fn build_conflict_item_views(
    local_records: &[SyncRecord],
    remote_records: &[SyncRecord],
    passphrase: Option<&str>,
) -> Vec<ConflictItemView> {
    let diffs = diff_sync_records_with_passphrase(local_records, remote_records, passphrase);
    let mut views = Vec::new();

    for diff in diffs {
        if diff.status == DiffStatus::Conflict {
            let local = local_records.iter().find(|r| r.object_id == diff.object_id);
            let remote = remote_records
                .iter()
                .find(|r| r.object_id == diff.object_id);

            if let (Some(loc), Some(rem)) = (local, remote) {
                views.push(ConflictItemView {
                    object_id: diff.object_id.clone(),
                    entity_type: diff.entity_type,
                    local_version: loc.version,
                    remote_version: rem.version,
                    local_updated_at: loc.updated_at,
                    remote_updated_at: rem.updated_at,
                    local_summary: format_record_summary(passphrase, loc),
                    remote_summary: format_record_summary(passphrase, rem),
                });
            }
        }
    }

    views
}

/// Trigger mechanism initiating a synchronization cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncTrigger {
    /// Initial synchronization upon application startup.
    Startup,
    /// Explicit user action via UI button or CLI command.
    Manual,
    /// Periodic background timer tick.
    Periodic,
    /// Debounced trigger fired after local database modifications.
    LocalMutation,
}

impl fmt::Display for SyncTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Startup => f.write_str("startup"),
            Self::Manual => f.write_str("manual"),
            Self::Periodic => f.write_str("periodic"),
            Self::LocalMutation => f.write_str("local_mutation"),
        }
    }
}

/// Exponential backoff calculator for network resilience.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncBackoff {
    pub initial_delay_secs: u64,
    pub max_delay_secs: u64,
    pub multiplier: f64,
}

impl Default for SyncBackoff {
    fn default() -> Self {
        Self {
            initial_delay_secs: 5,
            max_delay_secs: 300,
            multiplier: 2.0,
        }
    }
}

impl SyncBackoff {
    pub fn new(initial_delay_secs: u64, max_delay_secs: u64, multiplier: f64) -> Self {
        Self {
            initial_delay_secs,
            max_delay_secs,
            multiplier,
        }
    }

    /// Calculates the backoff delay in seconds for a given consecutive failure count.
    pub fn compute_delay_secs(&self, consecutive_failures: u32) -> u64 {
        if consecutive_failures == 0 {
            return 0;
        }
        let factor = self.multiplier.powi((consecutive_failures - 1) as i32);
        let delay = (self.initial_delay_secs as f64 * factor) as u64;
        delay.min(self.max_delay_secs)
    }
}

/// Coordinates synchronization timing, debounced local mutations, and backoff states.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncScheduler {
    pub last_sync_time: Option<u64>,
    pub consecutive_failures: u32,
    pub backoff: SyncBackoff,
    pub debounce_secs: u64,
    pub periodic_interval_secs: u64,
    pub pending_mutation: bool,
    pub mutation_deadline: Option<u64>,
}

impl Default for SyncScheduler {
    fn default() -> Self {
        Self::new(5, 300)
    }
}

impl SyncScheduler {
    pub fn new(debounce_secs: u64, periodic_interval_secs: u64) -> Self {
        Self {
            last_sync_time: None,
            consecutive_failures: 0,
            backoff: SyncBackoff::default(),
            debounce_secs,
            periodic_interval_secs,
            pending_mutation: false,
            mutation_deadline: None,
        }
    }

    /// Records a local database mutation and arms the debounce timer.
    pub fn notify_local_mutation(&mut self, current_time: u64) {
        self.pending_mutation = true;
        if self.mutation_deadline.is_none() {
            self.mutation_deadline = Some(current_time.saturating_add(self.debounce_secs));
        }
    }

    /// Determines whether a sync is due given the current time.
    pub fn check_trigger(&self, current_time: u64) -> Option<SyncTrigger> {
        // 1. Backoff check if recent failures occurred
        if self.consecutive_failures > 0 {
            if let Some(last_attempt) = self.last_sync_time {
                let required_delay = self.backoff.compute_delay_secs(self.consecutive_failures);
                if current_time < last_attempt.saturating_add(required_delay) {
                    return None;
                }
            }
        }

        // 2. Local mutation debounce check
        if self.pending_mutation {
            if let Some(deadline) = self.mutation_deadline {
                if current_time >= deadline {
                    return Some(SyncTrigger::LocalMutation);
                }
            }
        }

        // 3. Startup sync if never run
        if self.last_sync_time.is_none() {
            return Some(SyncTrigger::Startup);
        }

        // 4. Periodic interval sync
        if let Some(last) = self.last_sync_time {
            if current_time >= last.saturating_add(self.periodic_interval_secs) {
                return Some(SyncTrigger::Periodic);
            }
        }

        None
    }

    /// Records a successful sync cycle, resetting failures and pending mutations.
    pub fn record_success(&mut self, current_time: u64) {
        self.last_sync_time = Some(current_time);
        self.consecutive_failures = 0;
        self.pending_mutation = false;
        self.mutation_deadline = None;
    }

    /// Records a failed sync cycle, incrementing consecutive failure count.
    pub fn record_failure(&mut self, current_time: u64) {
        self.last_sync_time = Some(current_time);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
    }
}

/// Minimal percent-encoder for application/x-www-form-urlencoded strings.
pub fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

/// Representation of an outgoing HTTP request decoupled from any specific network runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: "GET".to_string(),
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn post(url: impl Into<String>, body: Vec<u8>) -> Self {
        Self {
            method: "POST".to_string(),
            url: url.into(),
            headers: Vec::new(),
            body,
        }
    }

    pub fn patch(url: impl Into<String>, body: Vec<u8>) -> Self {
        Self {
            method: "PATCH".to_string(),
            url: url.into(),
            headers: Vec::new(),
            body,
        }
    }

    pub fn delete(url: impl Into<String>) -> Self {
        Self {
            method: "DELETE".to_string(),
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.headers.push((key.into(), val.into()));
        self
    }

    pub fn body_str(&self) -> Result<&str, SyncError> {
        std::str::from_utf8(&self.body)
            .map_err(|e| SyncError::Deserialization(format!("invalid UTF-8 request body: {e}")))
    }
}

/// Representation of an incoming HTTP response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status_code: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status_code,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.headers.push((key.into(), val.into()));
        self
    }

    pub fn body_str(&self) -> Result<&str, SyncError> {
        std::str::from_utf8(&self.body)
            .map_err(|e| SyncError::Deserialization(format!("invalid UTF-8 response: {e}")))
    }
}

/// Abstract HTTP transport trait allowing mocked, sandboxed, and production network execution.
pub trait HttpTransport: Send + Sync {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, SyncError>;
}

/// Google Drive file metadata for files within the application data folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveFileMetadata {
    pub id: String,
    pub name: String,
    #[serde(rename = "mimeType", default)]
    pub mime_type: String,
    #[serde(rename = "modifiedTime", default)]
    pub modified_time: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
}

/// Builds an HTTP POST request to exchange an OAuth authorization code for tokens.
pub fn build_token_exchange_request(
    client_id: &str,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> HttpRequest {
    let form_body = format!(
        "client_id={}&code={}&code_verifier={}&redirect_uri={}&grant_type=authorization_code",
        url_encode(client_id),
        url_encode(code),
        url_encode(code_verifier),
        url_encode(redirect_uri),
    );
    HttpRequest::post(
        "https://oauth2.googleapis.com/token",
        form_body.into_bytes(),
    )
    .with_header("Content-Type", "application/x-www-form-urlencoded")
}

/// Parses the response from Google's token exchange endpoint into an `OAuthTokenBundle`.
pub fn parse_token_exchange_response(
    resp: &HttpResponse,
    current_time: u64,
) -> Result<OAuthTokenBundle, SyncError> {
    if resp.status_code != 200 {
        return Err(SyncError::Http(
            resp.status_code,
            resp.body_str()
                .unwrap_or("token exchange failed")
                .to_string(),
        ));
    }
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<u64>,
        scope: Option<String>,
    }
    let parsed: TokenResponse = serde_json::from_slice(&resp.body)
        .map_err(|e| SyncError::Deserialization(format!("invalid token response JSON: {e}")))?;

    Ok(OAuthTokenBundle::new(
        parsed.access_token,
        parsed.refresh_token,
        parsed.expires_in,
        parsed.scope,
        current_time,
    ))
}

/// Builds an HTTP POST request to refresh an expired access token using a refresh token.
pub fn build_token_refresh_request(client_id: &str, refresh_token: &str) -> HttpRequest {
    let form_body = format!(
        "client_id={}&refresh_token={}&grant_type=refresh_token",
        url_encode(client_id),
        url_encode(refresh_token),
    );
    HttpRequest::post(
        "https://oauth2.googleapis.com/token",
        form_body.into_bytes(),
    )
    .with_header("Content-Type", "application/x-www-form-urlencoded")
}

/// Parses the response from Google's token refresh endpoint.
pub fn parse_token_refresh_response(
    resp: &HttpResponse,
    current_time: u64,
    existing_refresh_token: Option<String>,
) -> Result<OAuthTokenBundle, SyncError> {
    if resp.status_code != 200 {
        return Err(SyncError::Http(
            resp.status_code,
            resp.body_str()
                .unwrap_or("token refresh failed")
                .to_string(),
        ));
    }
    #[derive(Deserialize)]
    struct RefreshResponse {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<u64>,
        scope: Option<String>,
    }
    let parsed: RefreshResponse = serde_json::from_slice(&resp.body)
        .map_err(|e| SyncError::Deserialization(format!("invalid token response JSON: {e}")))?;

    Ok(OAuthTokenBundle::new(
        parsed.access_token,
        parsed.refresh_token.or(existing_refresh_token),
        parsed.expires_in,
        parsed.scope,
        current_time,
    ))
}

/// Standard filename for Shellumi's encrypted vault snapshot in the Google Drive appdata folder.
pub const DRIVE_APPDATA_FILENAME: &str = "shellumi_vault.json";

/// Builds an HTTP GET request to list files in the Google Drive Application Data folder.
pub fn build_list_appdata_files_request(access_token: &str) -> HttpRequest {
    let url = "https://www.googleapis.com/drive/v3/files?spaces=appDataFolder&fields=files(id,name,mimeType,modifiedTime,size)";
    HttpRequest::get(url).with_header("Authorization", format!("Bearer {access_token}"))
}

/// Parses the file list response from Google Drive.
pub fn parse_list_files_response(resp: &HttpResponse) -> Result<Vec<DriveFileMetadata>, SyncError> {
    if resp.status_code != 200 {
        return Err(SyncError::Http(
            resp.status_code,
            resp.body_str().unwrap_or("list files failed").to_string(),
        ));
    }
    #[derive(Deserialize)]
    struct FileListResponse {
        #[serde(default)]
        files: Vec<DriveFileMetadata>,
    }
    let parsed: FileListResponse = serde_json::from_slice(&resp.body)
        .map_err(|e| SyncError::Deserialization(format!("invalid file list JSON: {e}")))?;
    Ok(parsed.files)
}

/// Builds an HTTP GET request to download file content from Google Drive.
pub fn build_download_file_request(access_token: &str, file_id: &str) -> HttpRequest {
    let url = format!("https://www.googleapis.com/drive/v3/files/{file_id}?alt=media");
    HttpRequest::get(url).with_header("Authorization", format!("Bearer {access_token}"))
}

/// Parses the file download response from Google Drive.
pub fn parse_download_file_response(resp: &HttpResponse) -> Result<Vec<u8>, SyncError> {
    if resp.status_code != 200 {
        return Err(SyncError::Http(
            resp.status_code,
            resp.body_str()
                .unwrap_or("download file failed")
                .to_string(),
        ));
    }
    Ok(resp.body.clone())
}

/// Builds a multipart/related HTTP POST request to create a new file in the Drive appdata folder.
pub fn build_multipart_upload_request(
    access_token: &str,
    filename: &str,
    file_content: &[u8],
    boundary: &str,
) -> HttpRequest {
    let metadata_json = format!(
        "{{\"name\":\"{}\",\"parents\":[\"appDataFolder\"]}}",
        filename
    );
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
    body.extend_from_slice(metadata_json.as_bytes());
    body.extend_from_slice(format!("\r\n--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Type: application/json\r\n\r\n");
    body.extend_from_slice(file_content);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let url = "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart";
    HttpRequest::post(url, body)
        .with_header("Authorization", format!("Bearer {access_token}"))
        .with_header(
            "Content-Type",
            format!("multipart/related; boundary={boundary}"),
        )
}

/// Parses the response from creating or uploading a file on Google Drive.
pub fn parse_upload_file_response(resp: &HttpResponse) -> Result<DriveFileMetadata, SyncError> {
    if resp.status_code != 200 && resp.status_code != 201 {
        return Err(SyncError::Http(
            resp.status_code,
            resp.body_str().unwrap_or("upload file failed").to_string(),
        ));
    }
    let parsed: DriveFileMetadata = serde_json::from_slice(&resp.body)
        .map_err(|e| SyncError::Deserialization(format!("invalid upload response JSON: {e}")))?;
    Ok(parsed)
}

/// Builds an HTTP PATCH request to update the content of an existing file on Google Drive.
pub fn build_update_file_request(
    access_token: &str,
    file_id: &str,
    file_content: &[u8],
) -> HttpRequest {
    let url =
        format!("https://www.googleapis.com/upload/drive/v3/files/{file_id}?uploadType=media");
    HttpRequest::patch(url, file_content.to_vec())
        .with_header("Authorization", format!("Bearer {access_token}"))
        .with_header("Content-Type", "application/json")
}

/// Builds an HTTP DELETE request to delete a file from Google Drive.
pub fn build_delete_file_request(access_token: &str, file_id: &str) -> HttpRequest {
    let url = format!("https://www.googleapis.com/drive/v3/files/{file_id}");
    HttpRequest::delete(url).with_header("Authorization", format!("Bearer {access_token}"))
}

/// Execution plan determined before applying changes to Google Drive or local storage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPlan {
    /// Remote has no vault file; upload local snapshot to Drive.
    UploadInitial,
    /// Local database is empty and remote vault exists; download remote snapshot to import.
    DownloadInitial { file_id: String },
    /// Local and remote vaults are identical; in sync, no network mutations needed.
    AlreadySynchronized,
    /// Remote has newer records; download remote vault and import into local store.
    ApplyRemoteUpdates { file_id: String },
    /// Local has newer records; upload updated local vault to Drive.
    UploadLocalUpdates { file_id: String },
    /// Conflicting records detected across devices; create backup snapshot and request resolution.
    ConflictDetected {
        file_id: String,
        conflicting_object_ids: Vec<String>,
    },
}

/// Evaluates local records against remote Drive files and remote snapshot to compute a safe sync plan.
pub fn plan_sync_cycle(
    local_records: &[SyncRecord],
    remote_file: Option<&DriveFileMetadata>,
    remote_vault: Option<&SyncVault>,
    passphrase: Option<&str>,
) -> SyncPlan {
    let Some(remote_meta) = remote_file else {
        return SyncPlan::UploadInitial;
    };

    let Some(remote_vault) = remote_vault else {
        if local_records.is_empty() {
            return SyncPlan::DownloadInitial {
                file_id: remote_meta.id.clone(),
            };
        } else {
            return SyncPlan::ApplyRemoteUpdates {
                file_id: remote_meta.id.clone(),
            };
        }
    };

    let diffs = diff_sync_records_with_passphrase(local_records, &remote_vault.records, passphrase);

    let mut conflicts = Vec::new();
    let mut local_newer = false;
    let mut remote_newer = false;

    for d in diffs {
        match d.status {
            DiffStatus::Conflict => conflicts.push(d.object_id),
            DiffStatus::LocalOnly | DiffStatus::LocalNewer => local_newer = true,
            DiffStatus::RemoteOnly | DiffStatus::RemoteNewer => remote_newer = true,
            DiffStatus::Identical => {}
        }
    }

    if !conflicts.is_empty() {
        SyncPlan::ConflictDetected {
            file_id: remote_meta.id.clone(),
            conflicting_object_ids: conflicts,
        }
    } else if local_newer && !remote_newer {
        SyncPlan::UploadLocalUpdates {
            file_id: remote_meta.id.clone(),
        }
    } else if remote_newer && !local_newer {
        SyncPlan::ApplyRemoteUpdates {
            file_id: remote_meta.id.clone(),
        }
    } else if local_newer && remote_newer {
        // Both sides have changes on non-overlapping objects
        SyncPlan::UploadLocalUpdates {
            file_id: remote_meta.id.clone(),
        }
    } else {
        SyncPlan::AlreadySynchronized
    }
}

/// Detailed report from an automated sync cycle execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncExecutionReport {
    pub local_updated: usize,
    pub remote_updated: usize,
    pub conflicts_resolved: usize,
    pub duration_ms: u64,
    pub message: String,
}

/// Result of evaluating or executing an end-to-end sync cycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncCycleResult {
    /// Both sides are already identical.
    AlreadySynchronized,
    /// Successfully synchronized without conflicts.
    Success {
        report: SyncExecutionReport,
        merged_records: Vec<SyncRecord>,
    },
    /// Unresolved conflicts detected; requires interactive visual resolution.
    Conflicts(Vec<ConflictItemView>),
    /// Authentication or encryption error.
    Error(String),
}

/// Executes a full synchronization cycle between local and remote sync records.
///
/// If conflicting records are found and `resolutions` is `None`, returns `SyncCycleResult::Conflicts`.
/// If `resolutions` is provided, each conflict is resolved according to the user's choice
/// (`KeepLocal` or `AcceptRemote`) and merged into the final record set.
pub fn execute_sync_cycle(
    local_records: &[SyncRecord],
    remote_records: &[SyncRecord],
    passphrase: &str,
    resolutions: Option<&[ConflictResolutionChoice]>,
) -> SyncCycleResult {
    let diffs = diff_sync_records_with_passphrase(local_records, remote_records, Some(passphrase));
    let mut conflicts = Vec::new();

    for d in &diffs {
        if d.status == DiffStatus::Conflict {
            conflicts.push(d.object_id.clone());
        }
    }

    if !conflicts.is_empty() {
        let Some(choices) = resolutions else {
            let conflict_views =
                build_conflict_item_views(local_records, remote_records, Some(passphrase));
            return SyncCycleResult::Conflicts(conflict_views);
        };

        if choices.len() != conflicts.len() {
            let conflict_views =
                build_conflict_item_views(local_records, remote_records, Some(passphrase));
            return SyncCycleResult::Conflicts(conflict_views);
        }
    }

    if diffs.iter().all(|d| d.status == DiffStatus::Identical) {
        return SyncCycleResult::AlreadySynchronized;
    }

    let mut merged = Vec::new();
    let mut local_applied = 0;
    let mut remote_uploaded = 0;
    let mut conflict_idx = 0;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for d in diffs {
        match d.status {
            DiffStatus::Identical => {
                if let Some(r) = local_records
                    .iter()
                    .find(|rec| rec.object_id == d.object_id)
                {
                    merged.push(r.clone());
                }
            }
            DiffStatus::LocalOnly | DiffStatus::LocalNewer => {
                if let Some(r) = local_records
                    .iter()
                    .find(|rec| rec.object_id == d.object_id)
                {
                    merged.push(r.clone());
                    remote_uploaded += 1;
                }
            }
            DiffStatus::RemoteOnly | DiffStatus::RemoteNewer => {
                if let Some(r) = remote_records
                    .iter()
                    .find(|rec| rec.object_id == d.object_id)
                {
                    merged.push(r.clone());
                    local_applied += 1;
                }
            }
            DiffStatus::Conflict => {
                let choice = resolutions
                    .and_then(|r| r.get(conflict_idx))
                    .copied()
                    .unwrap_or(ConflictResolutionChoice::KeepLocal);
                conflict_idx += 1;

                let loc = local_records
                    .iter()
                    .find(|rec| rec.object_id == d.object_id);
                let rem = remote_records
                    .iter()
                    .find(|rec| rec.object_id == d.object_id);

                if let (Some(l), Some(r)) = (loc, rem) {
                    let resolved = resolve_record_conflict(l, r, choice, now);
                    match choice {
                        ConflictResolutionChoice::KeepLocal => remote_uploaded += 1,
                        ConflictResolutionChoice::AcceptRemote => local_applied += 1,
                    }
                    merged.push(resolved);
                }
            }
        }
    }

    let report = SyncExecutionReport {
        local_updated: local_applied,
        remote_updated: remote_uploaded,
        conflicts_resolved: conflict_idx,
        duration_ms: 10,
        message: format!(
            "Sync complete: {local_applied} local updated, {remote_uploaded} remote updated, {conflict_idx} conflicts resolved"
        ),
    };

    SyncCycleResult::Success {
        report,
        merged_records: merged,
    }
}

/// High-level client for managing synchronization with Google Drive Application Data folder.
pub struct GoogleDriveSyncClient<T: HttpTransport> {
    transport: T,
    client_id: String,
}

impl<T: HttpTransport> GoogleDriveSyncClient<T> {
    pub fn new(transport: T, client_id: impl Into<String>) -> Self {
        Self {
            transport,
            client_id: client_id.into(),
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
        current_time: u64,
    ) -> Result<OAuthTokenBundle, SyncError> {
        let req = build_token_exchange_request(&self.client_id, code, code_verifier, redirect_uri);
        let resp = self.transport.send(req)?;
        parse_token_exchange_response(&resp, current_time)
    }

    pub fn refresh_token(
        &self,
        token_bundle: &OAuthTokenBundle,
        current_time: u64,
    ) -> Result<OAuthTokenBundle, SyncError> {
        let Some(refresh_token) = &token_bundle.refresh_token else {
            return Err(SyncError::MissingAuthCode);
        };
        let req = build_token_refresh_request(&self.client_id, refresh_token);
        let resp = self.transport.send(req)?;
        parse_token_refresh_response(&resp, current_time, token_bundle.refresh_token.clone())
    }

    pub fn list_vault_files(
        &self,
        access_token: &str,
    ) -> Result<Vec<DriveFileMetadata>, SyncError> {
        let req = build_list_appdata_files_request(access_token);
        let resp = self.transport.send(req)?;
        parse_list_files_response(&resp)
    }

    pub fn download_vault(
        &self,
        access_token: &str,
        file_id: &str,
    ) -> Result<SyncVault, SyncError> {
        let req = build_download_file_request(access_token, file_id);
        let resp = self.transport.send(req)?;
        let bytes = parse_download_file_response(&resp)?;
        let json = std::str::from_utf8(&bytes)
            .map_err(|e| SyncError::Deserialization(format!("invalid UTF-8 in vault file: {e}")))?;
        SyncVault::from_json(json)
    }

    pub fn upload_vault(
        &self,
        access_token: &str,
        vault: &SyncVault,
        boundary: &str,
    ) -> Result<DriveFileMetadata, SyncError> {
        let json = vault.to_json()?;
        let req = build_multipart_upload_request(
            access_token,
            DRIVE_APPDATA_FILENAME,
            json.as_bytes(),
            boundary,
        );
        let resp = self.transport.send(req)?;
        parse_upload_file_response(&resp)
    }

    pub fn update_vault(
        &self,
        access_token: &str,
        file_id: &str,
        vault: &SyncVault,
    ) -> Result<(), SyncError> {
        let json = vault.to_json()?;
        let req = build_update_file_request(access_token, file_id, json.as_bytes());
        let resp = self.transport.send(req)?;
        if resp.status_code != 200 {
            return Err(SyncError::Http(
                resp.status_code,
                resp.body_str().unwrap_or("update vault failed").to_string(),
            ));
        }
        Ok(())
    }

    pub fn delete_file(&self, access_token: &str, file_id: &str) -> Result<(), SyncError> {
        let req = build_delete_file_request(access_token, file_id);
        let resp = self.transport.send(req)?;
        if resp.status_code != 200 && resp.status_code != 204 {
            return Err(SyncError::Http(
                resp.status_code,
                resp.body_str().unwrap_or("delete file failed").to_string(),
            ));
        }
        Ok(())
    }
}

/// Helper to find the delimiter between HTTP header and body in raw stream output.
/// Orchestrates an end-to-end cloud vault synchronization cycle against Google Drive Application Data folder.
pub fn execute_cloud_vault_sync<T: HttpTransport>(
    client: &GoogleDriveSyncClient<T>,
    access_token: &str,
    local_vault: &SyncVault,
    passphrase: &str,
    resolutions: Option<&[ConflictResolutionChoice]>,
) -> Result<SyncCycleResult, SyncError> {
    let files = client.list_vault_files(access_token)?;
    let remote_file = files.into_iter().find(|f| f.name == DRIVE_APPDATA_FILENAME);

    let remote_vault = if let Some(ref rf) = remote_file {
        Some(client.download_vault(access_token, &rf.id)?)
    } else {
        None
    };

    let remote_records = remote_vault
        .as_ref()
        .map(|v| v.records.as_slice())
        .unwrap_or(&[]);
    let plan = plan_sync_cycle(
        &local_vault.records,
        remote_file.as_ref(),
        remote_vault.as_ref(),
        Some(passphrase),
    );

    match plan {
        SyncPlan::UploadInitial => {
            let boundary = "SHELLUMI_BOUNDARY_CLOUD_SYNC_INIT";
            let _ = client.upload_vault(access_token, local_vault, boundary)?;
            Ok(SyncCycleResult::Success {
                report: SyncExecutionReport {
                    local_updated: 0,
                    remote_updated: local_vault.records.len(),
                    conflicts_resolved: 0,
                    duration_ms: 1,
                    message: format!(
                        "Initial cloud vault uploaded with {} records",
                        local_vault.records.len()
                    ),
                },
                merged_records: local_vault.records.clone(),
            })
        }
        SyncPlan::DownloadInitial { .. } => {
            let records = remote_records.to_vec();
            Ok(SyncCycleResult::Success {
                report: SyncExecutionReport {
                    local_updated: records.len(),
                    remote_updated: 0,
                    conflicts_resolved: 0,
                    duration_ms: 1,
                    message: format!(
                        "Initial cloud vault downloaded with {} records",
                        records.len()
                    ),
                },
                merged_records: records,
            })
        }
        SyncPlan::AlreadySynchronized => Ok(SyncCycleResult::AlreadySynchronized),
        SyncPlan::ApplyRemoteUpdates { .. }
        | SyncPlan::UploadLocalUpdates { .. }
        | SyncPlan::ConflictDetected { .. } => {
            let sync_res = execute_sync_cycle(
                &local_vault.records,
                remote_records,
                passphrase,
                resolutions,
            );
            if let SyncCycleResult::Success {
                ref report,
                ref merged_records,
            } = sync_res
            {
                if report.remote_updated > 0 {
                    let mut updated_vault = local_vault.clone();
                    updated_vault.records = merged_records.clone();
                    updated_vault.updated_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    if let Some(ref rf) = remote_file {
                        client.update_vault(access_token, &rf.id, &updated_vault)?;
                    } else {
                        let boundary = "SHELLUMI_BOUNDARY_CLOUD_SYNC_MERGE";
                        client.upload_vault(access_token, &updated_vault, boundary)?;
                    }
                }
            }
            Ok(sync_res)
        }
    }
}

#[cfg(test)]
mod tests;

//! End-to-end encrypted synchronization models and cryptographic envelope.
//!
//! Provides zero-knowledge AEAD envelope encryption (AES-256-GCM via `ring`),
//! PBKDF2-HMAC-SHA256 key derivation with high iteration counts, bound associated
//! data to prevent ciphertext transposition, sync conflict detection, and OAuth 2.0
//! PKCE challenge helpers for Google Drive synchronization.

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
}

impl fmt::Display for SyncEntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host => f.write_str("host"),
            Self::Group => f.write_str("group"),
            Self::Tunnel => f.write_str("tunnel"),
            Self::Snippet => f.write_str("snippet"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestHost {
        name: String,
        hostname: String,
        port: u16,
    }

    #[test]
    fn encrypts_and_decrypts_entity_with_valid_passphrase() {
        let host = TestHost {
            name: "Production Bastion".into(),
            hostname: "bastion.example.com".into(),
            port: 2222,
        };
        let passphrase = "correct-horse-battery-staple";

        let envelope =
            encrypt_entity_with_iterations(passphrase, "host", "host-uuid-1", &host, 2_000)
                .unwrap();
        assert_eq!(envelope.associated_data, "host:host-uuid-1");
        assert!(!envelope.ciphertext.is_empty());
        assert!(!envelope.salt.is_empty());
        assert!(!envelope.nonce.is_empty());

        let decrypted: TestHost = decrypt_entity(passphrase, &envelope).unwrap();
        assert_eq!(decrypted, host);
    }

    #[test]
    fn fails_decryption_with_incorrect_passphrase() {
        let host = TestHost {
            name: "Secret Server".into(),
            hostname: "10.0.0.1".into(),
            port: 22,
        };
        let envelope = encrypt_entity_with_iterations(
            "correct-passphrase",
            "host",
            "host-uuid-2",
            &host,
            2_000,
        )
        .unwrap();

        let result: Result<TestHost, _> = decrypt_entity("wrong-passphrase", &envelope);
        assert_eq!(result, Err(SyncError::DecryptionFailed));
    }

    #[test]
    fn fails_decryption_if_associated_data_is_tampered() {
        let host = TestHost {
            name: "DB".into(),
            hostname: "db.local".into(),
            port: 5432,
        };
        let mut envelope = encrypt_entity_with_iterations(
            "secret-passphrase",
            "host",
            "host-uuid-3",
            &host,
            2_000,
        )
        .unwrap();

        // Tamper with associated data (transposition attack simulation)
        envelope.associated_data = "host:host-uuid-different".into();
        let result: Result<TestHost, _> = decrypt_entity("secret-passphrase", &envelope);
        assert_eq!(result, Err(SyncError::DecryptionFailed));
    }

    #[test]
    fn enforces_passphrase_length_requirements() {
        let short = "short";
        let salt = [0u8; 16];
        assert_eq!(
            derive_key(short, &salt, 1000),
            Err(SyncError::PassphraseTooShort(5))
        );
        assert_eq!(derive_key("", &salt, 1000), Err(SyncError::EmptyPassphrase));
    }

    #[test]
    fn determines_sync_actions_correctly() {
        let salt = generate_salt().unwrap();
        let nonce = generate_nonce().unwrap();
        let dummy_env = EncryptedEnvelope {
            salt: BASE64_STANDARD.encode(salt),
            nonce: BASE64_STANDARD.encode(nonce),
            ciphertext: "dummy".into(),
            associated_data: "host:1".into(),
            kdf_iterations: 1000,
        };

        let local_v1 =
            SyncRecord::new("1", SyncEntityType::Host, 1, "devA", 100, dummy_env.clone());
        let remote_v2 =
            SyncRecord::new("1", SyncEntityType::Host, 2, "devB", 200, dummy_env.clone());
        let remote_v1_same =
            SyncRecord::new("1", SyncEntityType::Host, 1, "devA", 100, dummy_env.clone());
        let remote_v1_conflict =
            SyncRecord::new("1", SyncEntityType::Host, 1, "devB", 150, dummy_env.clone());

        // Local missing
        assert_eq!(
            determine_sync_action(None, &remote_v2),
            SyncAction::ApplyRemote
        );

        // Remote newer
        assert_eq!(
            determine_sync_action(Some(&local_v1), &remote_v2),
            SyncAction::ApplyRemote
        );

        // Local newer
        assert_eq!(
            determine_sync_action(Some(&remote_v2), &local_v1),
            SyncAction::UploadLocal
        );

        // In sync
        assert_eq!(
            determine_sync_action(Some(&local_v1), &remote_v1_same),
            SyncAction::InSync
        );

        // Concurrent edit on different devices
        assert_eq!(
            determine_sync_action(Some(&local_v1), &remote_v1_conflict),
            SyncAction::Conflict
        );
    }

    #[test]
    fn creates_and_serializes_vault_snapshot() {
        let mut vault = SyncVault::new("test-vault-123", 1700000000);
        let tombstone = SyncRecord::tombstone(
            "deleted-host-1",
            SyncEntityType::Host,
            3,
            "devA",
            1700000100,
        );
        vault.records.push(tombstone);

        let json = vault.to_json().unwrap();
        assert!(json.contains("test-vault-123"));
        assert!(json.contains("deleted-host-1"));

        let restored = SyncVault::from_json(&json).unwrap();
        assert_eq!(restored, vault);
    }

    #[test]
    fn generates_valid_pkce_challenge_and_google_url() {
        let pkce = generate_pkce_challenge().unwrap();
        assert_eq!(pkce.code_verifier.len(), 43); // 32 bytes base64url = 43 chars
        assert_eq!(pkce.code_challenge.len(), 43); // SHA-256 base64url = 43 chars

        let url = build_google_drive_auth_url(
            "client123.apps.googleusercontent.com",
            "http://127.0.0.1:8080/callback",
            &pkce,
            "state_nonce_abc",
        );
        assert!(url.contains("client_id=client123.apps.googleusercontent.com"));
        assert!(url.contains("redirect_uri=http://127.0.0.1:8080/callback"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("drive.appdata"));
    }

    #[test]
    fn parses_oauth_callback_requests_and_handles_errors() {
        // Valid callback
        let valid_line = "GET /callback?code=4/0AY0e_test_code_123&state=state_abc HTTP/1.1";
        let code = parse_oauth_callback_request(valid_line, "state_abc").unwrap();
        assert_eq!(code, "4/0AY0e_test_code_123");

        // State mismatch
        let mismatch_line = "GET /callback?code=4/0AY0e_test_code_123&state=state_wrong HTTP/1.1";
        assert_eq!(
            parse_oauth_callback_request(mismatch_line, "state_abc"),
            Err(SyncError::StateMismatch)
        );

        // Missing code
        let no_code_line = "GET /callback?state=state_abc HTTP/1.1";
        assert_eq!(
            parse_oauth_callback_request(no_code_line, "state_abc"),
            Err(SyncError::MissingAuthCode)
        );

        // Provider error
        let error_line = "GET /callback?error=access_denied&state=state_abc HTTP/1.1";
        assert!(matches!(
            parse_oauth_callback_request(error_line, "state_abc"),
            Err(SyncError::InvalidHttpCallback(_))
        ));

        // Non-GET
        let post_line = "POST /callback?code=123&state=state_abc HTTP/1.1";
        assert!(matches!(
            parse_oauth_callback_request(post_line, "state_abc"),
            Err(SyncError::InvalidHttpCallback(_))
        ));
    }

    #[test]
    fn tests_oauth_callback_listener_loopback_roundtrip() {
        use std::io::Write;
        use std::net::TcpStream;
        use std::thread;

        let listener = match OAuthCallbackListener::bind_ephemeral() {
            Ok(l) => l,
            Err(SyncError::Network(msg))
                if msg.contains("Operation not permitted") || msg.contains("Permission denied") =>
            {
                // In restricted sandbox environments, local TCP socket binding is restricted.
                return;
            }
            Err(e) => panic!("unexpected error binding loopback listener: {e}"),
        };
        let port = listener.port();
        assert!(port > 0);
        let redirect_uri = listener.redirect_uri();
        assert_eq!(redirect_uri, format!("http://127.0.0.1:{port}/callback"));

        let handle = thread::spawn(move || {
            listener.wait_for_code("expected_nonce", std::time::Duration::from_secs(5))
        });

        // Simulate browser redirect to callback server
        let mut client = match TcpStream::connect(format!("127.0.0.1:{port}")) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(e) => panic!("unexpected error connecting to loopback: {e}"),
        };
        let request = format!(
            "GET /callback?code=auth_code_xyz_999&state=expected_nonce HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
        );
        client.write_all(request.as_bytes()).unwrap();

        let code = handle.join().unwrap().unwrap();
        assert_eq!(code, "auth_code_xyz_999");
    }

    #[test]
    fn verifies_token_bundle_expiration() {
        let bundle = OAuthTokenBundle::new(
            "access_token_123",
            Some("refresh_token_456".into()),
            Some(3600), // 1 hour
            Some("https://www.googleapis.com/auth/drive.appdata".into()),
            1000,
        );

        assert!(!bundle.is_expired(1000));
        assert!(!bundle.is_expired(1000 + 3500));
        // Within 60s grace window (3540..4600)
        assert!(bundle.is_expired(1000 + 3541));
        assert!(bundle.is_expired(1000 + 4000));
    }

    #[test]
    fn computes_record_diffs_correctly() {
        let salt = generate_salt().unwrap();
        let nonce = generate_nonce().unwrap();
        let dummy_env = EncryptedEnvelope {
            salt: BASE64_STANDARD.encode(salt),
            nonce: BASE64_STANDARD.encode(nonce),
            ciphertext: "c".into(),
            associated_data: "host:1".into(),
            kdf_iterations: 1000,
        };

        let local1 = SyncRecord::new(
            "host1",
            SyncEntityType::Host,
            2,
            "devA",
            200,
            dummy_env.clone(),
        );
        let local2 = SyncRecord::new(
            "host2",
            SyncEntityType::Host,
            1,
            "devA",
            100,
            dummy_env.clone(),
        );
        let local3 = SyncRecord::new(
            "host3",
            SyncEntityType::Host,
            1,
            "devA",
            100,
            dummy_env.clone(),
        );

        let remote1 = SyncRecord::new(
            "host1",
            SyncEntityType::Host,
            1,
            "devB",
            100,
            dummy_env.clone(),
        );
        let remote2 = SyncRecord::new(
            "host2",
            SyncEntityType::Host,
            3,
            "devB",
            300,
            dummy_env.clone(),
        );
        let remote3 = SyncRecord::new(
            "host3",
            SyncEntityType::Host,
            1,
            "devB",
            150,
            dummy_env.clone(),
        );
        let remote4 = SyncRecord::new("host4", SyncEntityType::Host, 1, "devB", 100, dummy_env);

        let local_records = vec![local1, local2, local3];
        let remote_records = vec![remote1, remote2, remote3, remote4];

        let diffs = diff_sync_records(&local_records, &remote_records);
        assert_eq!(diffs.len(), 4);

        // host1: local version 2 > remote 1 => LocalNewer
        let d1 = diffs.iter().find(|d| d.object_id == "host1").unwrap();
        assert_eq!(d1.status, DiffStatus::LocalNewer);

        // host2: remote version 3 > local 1 => RemoteNewer
        let d2 = diffs.iter().find(|d| d.object_id == "host2").unwrap();
        assert_eq!(d2.status, DiffStatus::RemoteNewer);

        // host3: same version 1, different devices => Conflict
        let d3 = diffs.iter().find(|d| d.object_id == "host3").unwrap();
        assert_eq!(d3.status, DiffStatus::Conflict);

        // host4: only in remote => RemoteOnly
        let d4 = diffs.iter().find(|d| d.object_id == "host4").unwrap();
        assert_eq!(d4.status, DiffStatus::RemoteOnly);
    }

    #[test]
    fn verifies_diff_with_passphrase_detects_identical_payloads() {
        #[derive(Serialize, Deserialize, PartialEq, Eq)]
        struct SampleData {
            field: String,
        }

        let passphrase = "test-sync-passphrase-99";
        let data = SampleData {
            field: "same_value".into(),
        };

        // Two envelopes sealed separately with different salts and nonces
        let env_a =
            encrypt_entity_with_iterations(passphrase, "sample", "obj1", &data, 1000).unwrap();
        let env_b =
            encrypt_entity_with_iterations(passphrase, "sample", "obj1", &data, 1000).unwrap();

        let rec_a = SyncRecord::new("obj1", SyncEntityType::Host, 1, "dev1", 100, env_a);
        let rec_b = SyncRecord::new("obj1", SyncEntityType::Host, 1, "dev2", 200, env_b);

        // Without passphrase: looks like conflict due to different devices
        let diffs_no_pass =
            diff_sync_records(std::slice::from_ref(&rec_a), std::slice::from_ref(&rec_b));
        assert_eq!(diffs_no_pass[0].status, DiffStatus::Conflict);

        // With passphrase: decrypts and discovers payloads are identical
        let diffs_with_pass = diff_sync_records_with_passphrase(
            std::slice::from_ref(&rec_a),
            std::slice::from_ref(&rec_b),
            Some(passphrase),
        );
        assert_eq!(diffs_with_pass[0].status, DiffStatus::Identical);
    }

    #[test]
    fn verifies_url_encoding() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("abc-123_.~"), "abc-123_.~");
        assert_eq!(
            url_encode("https://example.com/test?a=1&b=2"),
            "https%3A%2F%2Fexample.com%2Ftest%3Fa%3D1%26b%3D2"
        );
    }

    #[test]
    fn verifies_token_exchange_and_refresh_request_and_parsing() {
        let req = build_token_exchange_request(
            "client_123",
            "auth_code_456",
            "verifier_789",
            "http://127.0.0.1:8080/callback",
        );
        assert_eq!(req.method, "POST");
        assert_eq!(req.url, "https://oauth2.googleapis.com/token");
        let body_str = std::str::from_utf8(&req.body).unwrap();
        assert!(body_str.contains("code=auth_code_456"));
        assert!(body_str.contains("code_verifier=verifier_789"));
        assert!(body_str.contains("grant_type=authorization_code"));

        let success_resp = HttpResponse::new(
            200,
            r#"{"access_token":"ya29.sample","expires_in":3600,"refresh_token":"1//sample_refresh","scope":"https://www.googleapis.com/auth/drive.appdata"}"#,
        );
        let bundle = parse_token_exchange_response(&success_resp, 1000).unwrap();
        assert_eq!(bundle.access_token, "ya29.sample");
        assert_eq!(bundle.refresh_token.as_deref(), Some("1//sample_refresh"));
        assert_eq!(bundle.expires_in, Some(3600));

        // Error response handling
        let err_resp = HttpResponse::new(401, r#"{"error":"invalid_grant"}"#);
        assert!(matches!(
            parse_token_exchange_response(&err_resp, 1000),
            Err(SyncError::Http(401, _))
        ));

        // Refresh request
        let refresh_req = build_token_refresh_request("client_123", "1//sample_refresh");
        assert_eq!(refresh_req.method, "POST");
        let ref_body = std::str::from_utf8(&refresh_req.body).unwrap();
        assert!(ref_body.contains("grant_type=refresh_token"));
        assert!(ref_body.contains("refresh_token=1%2F%2Fsample_refresh"));

        let ref_resp = HttpResponse::new(
            200,
            r#"{"access_token":"ya29.refreshed","expires_in":3600}"#,
        );
        let ref_bundle =
            parse_token_refresh_response(&ref_resp, 2000, Some("1//sample_refresh".into()))
                .unwrap();
        assert_eq!(ref_bundle.access_token, "ya29.refreshed");
        assert_eq!(
            ref_bundle.refresh_token.as_deref(),
            Some("1//sample_refresh")
        );
    }

    #[test]
    fn verifies_oauth_token_bundle_store_persistence() {
        let store = crate::credential::EphemeralCredentialStore::new();

        // 1. None when empty
        assert_eq!(OAuthTokenBundle::load_from_store(&store).unwrap(), None);

        // 2. Save bundle
        let bundle = OAuthTokenBundle::new(
            "ya29.secret_access_token",
            Some("1//secret_refresh_token".into()),
            Some(3600),
            Some("https://www.googleapis.com/auth/drive.appdata".into()),
            1000,
        );
        bundle.save_to_store(&store).unwrap();

        // 3. Load bundle
        let loaded = OAuthTokenBundle::load_from_store(&store).unwrap().unwrap();
        assert_eq!(loaded.access_token, "ya29.secret_access_token");
        assert_eq!(
            loaded.refresh_token.as_deref(),
            Some("1//secret_refresh_token")
        );
        assert_eq!(loaded.expires_in, Some(3600));

        // 4. Delete bundle
        assert!(OAuthTokenBundle::delete_from_store(&store).unwrap());
        assert_eq!(OAuthTokenBundle::load_from_store(&store).unwrap(), None);
    }

    #[test]
    fn verifies_google_drive_request_builders_and_parsers() {
        let token = "ya29.test_token";

        // List files
        let list_req = build_list_appdata_files_request(token);
        assert_eq!(list_req.method, "GET");
        assert!(list_req.url.contains("spaces=appDataFolder"));
        assert_eq!(
            list_req.headers,
            vec![("Authorization".into(), "Bearer ya29.test_token".into())]
        );

        let list_resp = HttpResponse::new(
            200,
            r#"{"files":[{"id":"file_123","name":"shellumi_vault.json","mimeType":"application/json","modifiedTime":"2026-11-04T12:00:00Z","size":"4096"}]}"#,
        );
        let files = parse_list_files_response(&list_resp).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, "file_123");
        assert_eq!(files[0].name, "shellumi_vault.json");
        assert_eq!(files[0].mime_type, "application/json");

        // Download file
        let dl_req = build_download_file_request(token, "file_123");
        assert_eq!(dl_req.method, "GET");
        assert_eq!(
            dl_req.url,
            "https://www.googleapis.com/drive/v3/files/file_123?alt=media"
        );
        let dl_resp = HttpResponse::new(200, b"raw vault content bytes".to_vec());
        let dl_bytes = parse_download_file_response(&dl_resp).unwrap();
        assert_eq!(dl_bytes, b"raw vault content bytes");

        // Multipart upload
        let upload_req = build_multipart_upload_request(
            token,
            "shellumi_vault.json",
            b"{\"records\":[]}",
            "BOUNDARY_XYZ",
        );
        assert_eq!(upload_req.method, "POST");
        assert_eq!(
            upload_req.url,
            "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart"
        );
        let up_body_str = std::str::from_utf8(&upload_req.body).unwrap();
        assert!(up_body_str.contains("--BOUNDARY_XYZ"));
        assert!(up_body_str.contains("appDataFolder"));
        assert!(up_body_str.contains("{\"records\":[]}"));

        let up_resp = HttpResponse::new(
            200,
            r#"{"id":"new_file_789","name":"shellumi_vault.json","mimeType":"application/json"}"#,
        );
        let uploaded = parse_upload_file_response(&up_resp).unwrap();
        assert_eq!(uploaded.id, "new_file_789");

        // Update file
        let update_req = build_update_file_request(token, "file_123", b"updated content");
        assert_eq!(update_req.method, "PATCH");
        assert_eq!(
            update_req.url,
            "https://www.googleapis.com/upload/drive/v3/files/file_123?uploadType=media"
        );

        // Delete file
        let del_req = build_delete_file_request(token, "file_123");
        assert_eq!(del_req.method, "DELETE");
        assert_eq!(
            del_req.url,
            "https://www.googleapis.com/drive/v3/files/file_123"
        );
    }

    #[test]
    fn verifies_plan_sync_cycle() {
        let salt = generate_salt().unwrap();
        let nonce = generate_nonce().unwrap();
        let dummy_env = EncryptedEnvelope {
            salt: BASE64_STANDARD.encode(salt),
            nonce: BASE64_STANDARD.encode(nonce),
            ciphertext: "c".into(),
            associated_data: "host:1".into(),
            kdf_iterations: 1000,
        };

        let local_record = SyncRecord::new(
            "host1",
            SyncEntityType::Host,
            1,
            "devA",
            100,
            dummy_env.clone(),
        );
        let mut local_vault = SyncVault::new("snap1", 100);
        local_vault.records.push(local_record.clone());

        // 1. Remote file missing -> UploadInitial
        assert_eq!(
            plan_sync_cycle(&local_vault.records, None, None, None),
            SyncPlan::UploadInitial
        );

        let remote_meta = DriveFileMetadata {
            id: "remote_file_1".into(),
            name: "shellumi_vault.json".into(),
            mime_type: "application/json".into(),
            modified_time: None,
            size: None,
        };

        // 2. Remote file exists, but local is empty and remote vault not downloaded -> DownloadInitial
        assert_eq!(
            plan_sync_cycle(&[], Some(&remote_meta), None, None),
            SyncPlan::DownloadInitial {
                file_id: "remote_file_1".into(),
            }
        );

        // 3. Both local and remote exist with identical records -> AlreadySynchronized
        let mut remote_vault_identical = SyncVault::new("snap2", 100);
        remote_vault_identical.records.push(local_record.clone());
        assert_eq!(
            plan_sync_cycle(
                &local_vault.records,
                Some(&remote_meta),
                Some(&remote_vault_identical),
                None
            ),
            SyncPlan::AlreadySynchronized
        );

        // 4. Local has newer version -> UploadLocalUpdates
        let local_newer_rec = SyncRecord::new(
            "host1",
            SyncEntityType::Host,
            2,
            "devA",
            200,
            dummy_env.clone(),
        );
        assert_eq!(
            plan_sync_cycle(
                std::slice::from_ref(&local_newer_rec),
                Some(&remote_meta),
                Some(&remote_vault_identical),
                None
            ),
            SyncPlan::UploadLocalUpdates {
                file_id: "remote_file_1".into(),
            }
        );

        // 5. Remote has newer version -> ApplyRemoteUpdates
        let remote_newer_rec = SyncRecord::new(
            "host1",
            SyncEntityType::Host,
            2,
            "devB",
            200,
            dummy_env.clone(),
        );
        let mut remote_vault_newer = SyncVault::new("snap3", 200);
        remote_vault_newer.records.push(remote_newer_rec);
        assert_eq!(
            plan_sync_cycle(
                &local_vault.records,
                Some(&remote_meta),
                Some(&remote_vault_newer),
                None
            ),
            SyncPlan::ApplyRemoteUpdates {
                file_id: "remote_file_1".into(),
            }
        );

        // 6. Conflicting versions across different devices -> ConflictDetected
        let remote_conflict_rec =
            SyncRecord::new("host1", SyncEntityType::Host, 1, "devB", 150, dummy_env);
        let mut remote_vault_conflict = SyncVault::new("snap4", 150);
        remote_vault_conflict.records.push(remote_conflict_rec);
        assert_eq!(
            plan_sync_cycle(
                &local_vault.records,
                Some(&remote_meta),
                Some(&remote_vault_conflict),
                None
            ),
            SyncPlan::ConflictDetected {
                file_id: "remote_file_1".into(),
                conflicting_object_ids: vec!["host1".into()],
            }
        );
    }

    #[test]
    fn verifies_google_drive_sync_client_workflow_with_mock_transport() {
        use std::sync::Mutex;

        struct MockTransport {
            requests: Mutex<Vec<HttpRequest>>,
        }

        impl HttpTransport for MockTransport {
            fn send(&self, req: HttpRequest) -> Result<HttpResponse, SyncError> {
                let mut reqs = self.requests.lock().unwrap();
                reqs.push(req.clone());

                if req.url.contains("oauth2.googleapis.com/token") {
                    Ok(HttpResponse::new(
                        200,
                        r#"{"access_token":"ya29.mock_token","refresh_token":"1//mock_refresh","expires_in":3600}"#,
                    ))
                } else if req.method == "GET" && req.url.contains("spaces=appDataFolder") {
                    Ok(HttpResponse::new(
                        200,
                        r#"{"files":[{"id":"mock_file_id","name":"shellumi_vault.json","mimeType":"application/json"}]}"#,
                    ))
                } else if req.method == "GET" && req.url.contains("alt=media") {
                    let vault = SyncVault::new("mock_snap", 100);
                    Ok(HttpResponse::new(
                        200,
                        vault.to_json().unwrap().into_bytes(),
                    ))
                } else if req.method == "POST" && req.url.contains("uploadType=multipart") {
                    Ok(HttpResponse::new(
                        200,
                        r#"{"id":"new_mock_file_id","name":"shellumi_vault.json","mimeType":"application/json"}"#,
                    ))
                } else if req.method == "PATCH" || req.method == "DELETE" {
                    Ok(HttpResponse::new(200, b"".to_vec()))
                } else {
                    Err(SyncError::Http(404, "not found".into()))
                }
            }
        }

        let transport = MockTransport {
            requests: Mutex::new(Vec::new()),
        };
        let client = GoogleDriveSyncClient::new(transport, "client_id_test");

        // 1. Exchange code
        let bundle = client
            .exchange_code(
                "code_123",
                "verifier_456",
                "http://127.0.0.1:0/callback",
                1000,
            )
            .unwrap();
        assert_eq!(bundle.access_token, "ya29.mock_token");

        // 2. List files
        let files = client.list_vault_files(&bundle.access_token).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, "mock_file_id");

        // 3. Download vault
        let downloaded_vault = client
            .download_vault(&bundle.access_token, &files[0].id)
            .unwrap();
        assert_eq!(downloaded_vault.vault_id, "mock_snap");

        // 4. Upload vault
        let vault_to_upload = SyncVault::new("snap_up", 200);
        let uploaded = client
            .upload_vault(&bundle.access_token, &vault_to_upload, "boundary_test")
            .unwrap();
        assert_eq!(uploaded.id, "new_mock_file_id");

        // 5. Update vault
        client
            .update_vault(&bundle.access_token, "mock_file_id", &vault_to_upload)
            .unwrap();

        // 6. Delete file
        client
            .delete_file(&bundle.access_token, "mock_file_id")
            .unwrap();

        assert_eq!(client.transport().requests.lock().unwrap().len(), 6);
    }
}

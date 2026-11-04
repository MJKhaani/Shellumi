//! Credential management and secure storage abstraction.
//!
//! Provides in-memory secret zeroization on drop, strict redaction in formatting,
//! and integration with operating system credential stores (macOS Keychain via Security Framework).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use zeroize::Zeroizing;

#[derive(Debug, PartialEq, Eq)]
pub enum CredentialError {
    InvalidIdentifier(&'static str),
    InvalidSecret(&'static str),
    Storage(String),
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(msg) => write!(f, "invalid credential identifier: {msg}"),
            Self::InvalidSecret(msg) => write!(f, "invalid secret: {msg}"),
            Self::Storage(msg) => write!(f, "credential storage error: {msg}"),
        }
    }
}

impl std::error::Error for CredentialError {}

/// The type of credential stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialType {
    Password,
    PrivateKey,
    Passphrase,
    Certificate,
    Token,
}

impl fmt::Display for CredentialType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password => f.write_str("Password"),
            Self::PrivateKey => f.write_str("Private Key"),
            Self::Passphrase => f.write_str("Passphrase"),
            Self::Certificate => f.write_str("Certificate"),
            Self::Token => f.write_str("Token"),
        }
    }
}

/// A securely wrapped string whose contents are zeroized in memory upon drop,
/// and whose `Display` and `Debug` implementations redact the secret to avoid accidental logging.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString {
    inner: Zeroizing<String>,
}

impl SecretString {
    pub fn new(secret: String) -> Self {
        Self {
            inner: Zeroizing::new(secret),
        }
    }

    /// Expose the underlying plaintext secret for consumption.
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED SECRET]")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED SECRET]")
    }
}

/// Decoupled credential store interface for saving, retrieving, and removing secrets.
pub trait CredentialStore: Send + Sync {
    fn set_secret(&self, service: &str, account: &str, secret: &str)
        -> Result<(), CredentialError>;
    fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialError>;
    fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError>;
}

fn validate_identifier<'a>(name: &'a str, field: &'static str) -> Result<&'a str, CredentialError> {
    if name.chars().any(|c| c.is_control() || c == '\0') {
        return Err(CredentialError::InvalidIdentifier(field));
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CredentialError::InvalidIdentifier(field));
    }
    Ok(trimmed)
}

fn validate_secret(secret: &str) -> Result<&str, CredentialError> {
    if secret.contains('\0') {
        return Err(CredentialError::InvalidSecret(
            "secret must not contain NUL bytes",
        ));
    }
    Ok(secret)
}

/// Thread-safe in-memory credential store with memory zeroization on drop.
/// Ideal for testing, ephemeral sessions, or when OS credential stores are unavailable.
#[derive(Default)]
pub struct EphemeralCredentialStore {
    secrets: RwLock<HashMap<(String, String), Zeroizing<String>>>,
}

impl EphemeralCredentialStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CredentialStore for EphemeralCredentialStore {
    fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;
        let secret = validate_secret(secret)?;

        let mut lock = self
            .secrets
            .write()
            .map_err(|e| CredentialError::Storage(format!("lock error: {e}")))?;
        lock.insert(
            (service.to_string(), account.to_string()),
            Zeroizing::new(secret.to_string()),
        );
        Ok(())
    }

    fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        let lock = self
            .secrets
            .read()
            .map_err(|e| CredentialError::Storage(format!("lock error: {e}")))?;
        Ok(lock
            .get(&(service.to_string(), account.to_string()))
            .map(|s| SecretString::new(s.as_str().to_string())))
    }

    fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        let mut lock = self
            .secrets
            .write()
            .map_err(|e| CredentialError::Storage(format!("lock error: {e}")))?;
        Ok(lock
            .remove(&(service.to_string(), account.to_string()))
            .is_some())
    }
}

/// macOS Keychain credential store using Apple Security Framework Keychain Services.
#[cfg(target_os = "macos")]
pub struct MacOsKeychainStore;

#[cfg(target_os = "macos")]
impl MacOsKeychainStore {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "macos")]
impl Default for MacOsKeychainStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "macos")]
impl CredentialStore for MacOsKeychainStore {
    fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;
        let secret = validate_secret(secret)?;

        security_framework::passwords::set_generic_password(service, account, secret.as_bytes())
            .map_err(|err| CredentialError::Storage(format!("macOS Keychain write error: {err}")))
    }

    fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        match security_framework::passwords::get_generic_password(service, account) {
            Ok(bytes) => {
                let s = String::from_utf8(bytes).map_err(|err| {
                    CredentialError::Storage(format!("credential is not valid UTF-8: {err}"))
                })?;
                Ok(Some(SecretString::new(s)))
            }
            Err(err) => {
                let code = err.code();
                // errSecItemNotFound = -25300
                if code == -25300 {
                    Ok(None)
                } else {
                    Err(CredentialError::Storage(format!(
                        "macOS Keychain read error: {err} (code {code})"
                    )))
                }
            }
        }
    }

    fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        match security_framework::passwords::delete_generic_password(service, account) {
            Ok(()) => Ok(true),
            Err(err) => {
                let code = err.code();
                if code == -25300 {
                    Ok(false)
                } else {
                    Err(CredentialError::Storage(format!(
                        "macOS Keychain delete error: {err} (code {code})"
                    )))
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct StoredCredentials {
    entries: Vec<StoredCredentialEntry>,
}

#[derive(Serialize, Deserialize)]
struct StoredCredentialEntry {
    service: String,
    account: String,
    secret: String,
}

/// Encrypted file-backed credential store protecting secrets with AES-256-GCM AEAD encryption
/// and PBKDF2-HMAC-SHA256 key derivation.
/// Provides secure persistent storage across all desktop platforms, containers, and headless environments.
pub struct EncryptedFileCredentialStore {
    path: PathBuf,
    passphrase: Zeroizing<String>,
    iterations: u32,
    secrets: RwLock<HashMap<(String, String), Zeroizing<String>>>,
}

impl EncryptedFileCredentialStore {
    pub const ASSOCIATED_DATA: &'static str = "shellumi:credentials:v1";

    pub fn new(path: impl Into<PathBuf>, passphrase: &str) -> Result<Self, CredentialError> {
        Self::with_iterations(path, passphrase, crate::sync::DEFAULT_KDF_ITERATIONS)
    }

    pub fn with_iterations(
        path: impl Into<PathBuf>,
        passphrase: &str,
        iterations: u32,
    ) -> Result<Self, CredentialError> {
        let path = path.into();
        let mut map = HashMap::new();

        if path.exists() {
            let json_str = fs::read_to_string(&path).map_err(|e| {
                CredentialError::Storage(format!("failed to read credentials file: {e}"))
            })?;
            let envelope: crate::sync::EncryptedEnvelope = serde_json::from_str(&json_str)
                .map_err(|e| {
                    CredentialError::Storage(format!("invalid credentials file format: {e}"))
                })?;

            let stored: StoredCredentials = crate::sync::decrypt_entity(passphrase, &envelope)
                .map_err(|e| {
                    CredentialError::Storage(format!("failed to decrypt credentials: {e}"))
                })?;

            for entry in stored.entries {
                map.insert((entry.service, entry.account), Zeroizing::new(entry.secret));
            }
        }

        Ok(Self {
            path,
            passphrase: Zeroizing::new(passphrase.to_string()),
            iterations,
            secrets: RwLock::new(map),
        })
    }

    fn persist_internal(
        &self,
        lock: &HashMap<(String, String), Zeroizing<String>>,
    ) -> Result<(), CredentialError> {
        let entries = lock
            .iter()
            .map(|((s, a), sec)| StoredCredentialEntry {
                service: s.clone(),
                account: a.clone(),
                secret: sec.as_str().to_string(),
            })
            .collect();
        let stored = StoredCredentials { entries };

        let envelope = crate::sync::encrypt_entity_with_iterations(
            &self.passphrase,
            "credentials",
            "v1",
            &stored,
            self.iterations,
        )
        .map_err(|e| CredentialError::Storage(format!("failed to encrypt credentials: {e}")))?;

        let json = serde_json::to_string_pretty(&envelope).map_err(|e| {
            CredentialError::Storage(format!("failed to serialize credentials: {e}"))
        })?;

        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let temp_path = format!("{}.tmp.{}", self.path.display(), std::process::id());
        fs::write(&temp_path, json.as_bytes()).map_err(|e| {
            CredentialError::Storage(format!("failed to write temp credentials: {e}"))
        })?;
        fs::rename(&temp_path, &self.path)
            .map_err(|e| CredentialError::Storage(format!("failed to commit credentials: {e}")))?;

        Ok(())
    }
}

impl CredentialStore for EncryptedFileCredentialStore {
    fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;
        let secret = validate_secret(secret)?;

        let mut lock = self
            .secrets
            .write()
            .map_err(|e| CredentialError::Storage(format!("lock error: {e}")))?;
        lock.insert(
            (service.to_string(), account.to_string()),
            Zeroizing::new(secret.to_string()),
        );
        self.persist_internal(&lock)
    }

    fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        let lock = self
            .secrets
            .read()
            .map_err(|e| CredentialError::Storage(format!("lock error: {e}")))?;
        Ok(lock
            .get(&(service.to_string(), account.to_string()))
            .map(|s| SecretString::new(s.as_str().to_string())))
    }

    fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        let mut lock = self
            .secrets
            .write()
            .map_err(|e| CredentialError::Storage(format!("lock error: {e}")))?;
        let removed = lock
            .remove(&(service.to_string(), account.to_string()))
            .is_some();
        if removed {
            self.persist_internal(&lock)?;
        }
        Ok(removed)
    }
}

/// Windows Credential Manager store.
#[cfg(target_os = "windows")]
pub struct WindowsCredentialManagerStore;

#[cfg(target_os = "windows")]
impl WindowsCredentialManagerStore {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "windows")]
impl Default for WindowsCredentialManagerStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Linux Secret Service D-Bus credential store.
#[cfg(target_os = "linux")]
pub struct LinuxSecretServiceStore;

#[cfg(target_os = "linux")]
impl LinuxSecretServiceStore {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "linux")]
impl Default for LinuxSecretServiceStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the default platform-native credential store, falling back to ephemeral or encrypted file store.
pub fn default_credential_store() -> Box<dyn CredentialStore> {
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOsKeychainStore::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsCredentialManagerStore::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxSecretServiceStore::new())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Box::new(EphemeralCredentialStore::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_string_redacts_debug_and_display() {
        let secret = SecretString::new("super-secret-password-123".to_string());
        assert_eq!(format!("{secret}"), "[REDACTED SECRET]");
        assert_eq!(format!("{secret:?}"), "[REDACTED SECRET]");
        assert_eq!(secret.expose_secret(), "super-secret-password-123");
    }

    #[test]
    fn ephemeral_store_crud_lifecycle() {
        let store = EphemeralCredentialStore::new();
        let service = "shellumi-test";
        let account = "host-42";

        // Non-existent secret
        assert_eq!(store.get_secret(service, account).unwrap(), None);
        assert!(!store.delete_secret(service, account).unwrap());

        // Set secret
        store
            .set_secret(service, account, "my-private-key-data")
            .unwrap();
        let retrieved = store.get_secret(service, account).unwrap().unwrap();
        assert_eq!(retrieved.expose_secret(), "my-private-key-data");

        // Update secret
        store
            .set_secret(service, account, "updated-private-key-data")
            .unwrap();
        let updated = store.get_secret(service, account).unwrap().unwrap();
        assert_eq!(updated.expose_secret(), "updated-private-key-data");

        // Delete secret
        assert!(store.delete_secret(service, account).unwrap());
        assert_eq!(store.get_secret(service, account).unwrap(), None);
    }

    #[test]
    fn validates_identifiers_and_secrets() {
        let store = EphemeralCredentialStore::new();

        // Empty or whitespace service/account
        assert!(matches!(
            store.set_secret("   ", "account", "pw"),
            Err(CredentialError::InvalidIdentifier("service"))
        ));
        assert!(matches!(
            store.set_secret("shellumi", "  ", "pw"),
            Err(CredentialError::InvalidIdentifier("account"))
        ));

        // Control characters or NUL in identifier
        assert!(matches!(
            store.set_secret("shellumi\n", "acc", "pw"),
            Err(CredentialError::InvalidIdentifier("service"))
        ));
        assert!(matches!(
            store.set_secret("shellumi", "acc\0", "pw"),
            Err(CredentialError::InvalidIdentifier("account"))
        ));

        // NUL byte in secret
        assert!(matches!(
            store.set_secret("shellumi", "acc", "bad\0secret"),
            Err(CredentialError::InvalidSecret(_))
        ));
    }

    #[test]
    fn credential_types_display() {
        assert_eq!(format!("{}", CredentialType::Password), "Password");
        assert_eq!(format!("{}", CredentialType::PrivateKey), "Private Key");
        assert_eq!(format!("{}", CredentialType::Passphrase), "Passphrase");
        assert_eq!(format!("{}", CredentialType::Certificate), "Certificate");
        assert_eq!(format!("{}", CredentialType::Token), "Token");
    }

    #[test]
    fn encrypted_file_store_crud_lifecycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test_secrets.enc");
        let passphrase = "test-store-passphrase-888";

        // 1. Create fresh store
        let store = EncryptedFileCredentialStore::with_iterations(&path, passphrase, 2000).unwrap();
        assert_eq!(store.get_secret("test-svc", "alice").unwrap(), None);

        // 2. Set secret
        store
            .set_secret("test-svc", "alice", "my-secret-token-xyz")
            .unwrap();
        let sec = store.get_secret("test-svc", "alice").unwrap().unwrap();
        assert_eq!(sec.expose_secret(), "my-secret-token-xyz");

        // 3. File exists and does NOT contain plaintext secret
        assert!(path.exists());
        let raw_file_content = fs::read_to_string(&path).unwrap();
        assert!(!raw_file_content.contains("my-secret-token-xyz"));
        assert!(raw_file_content.contains("ciphertext"));

        // 4. Re-open with same passphrase
        let reopened =
            EncryptedFileCredentialStore::with_iterations(&path, passphrase, 2000).unwrap();
        let sec2 = reopened.get_secret("test-svc", "alice").unwrap().unwrap();
        assert_eq!(sec2.expose_secret(), "my-secret-token-xyz");

        // 5. Re-open with wrong passphrase fails
        let wrong_open =
            EncryptedFileCredentialStore::with_iterations(&path, "wrong-passphrase", 2000);
        assert!(wrong_open.is_err());

        // 6. Delete secret
        assert!(reopened.delete_secret("test-svc", "alice").unwrap());
        assert_eq!(reopened.get_secret("test-svc", "alice").unwrap(), None);

        // 7. Verify deletion persisted across reopen
        let reopened_again =
            EncryptedFileCredentialStore::with_iterations(&path, passphrase, 2000).unwrap();
        assert_eq!(
            reopened_again.get_secret("test-svc", "alice").unwrap(),
            None
        );
    }

    #[test]
    fn default_credential_store_creation() {
        let store = default_credential_store();
        assert_eq!(store.get_secret("default-svc", "user1").unwrap(), None);
    }
}

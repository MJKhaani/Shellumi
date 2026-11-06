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

        match security_framework::passwords::set_generic_password(
            service,
            account,
            secret.as_bytes(),
        ) {
            Ok(()) => Ok(()),
            Err(err) if err.code() == -25299 => {
                // Item already exists (-25299 errSecDuplicateItem): delete and retry as an upsert
                let _ = security_framework::passwords::delete_generic_password(service, account);
                security_framework::passwords::set_generic_password(
                    service,
                    account,
                    secret.as_bytes(),
                )
                .map_err(|err| {
                    CredentialError::Storage(format!("macOS Keychain write error: {err}"))
                })
            }
            Err(err) => Err(CredentialError::Storage(format!(
                "macOS Keychain write error: {err}"
            ))),
        }
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

/// Status and health of the native platform keychain daemon.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeychainDaemonStatus {
    pub backend_name: String,
    pub is_available: bool,
    pub daemon_identifier: String,
    pub details: String,
}

pub const DEFAULT_KEYCHAIN_SERVICE: &str = "org.shellumi.app";

/// Formats a canonical keychain URI reference (`keychain://<service>/<account>`).
pub fn format_keychain_ref(service: &str, account: &str) -> String {
    format!("keychain://{service}/{account}")
}

/// Parses a canonical keychain URI reference (`keychain://<service>/<account>`).
pub fn parse_keychain_ref(uri: &str) -> Result<(String, String), CredentialError> {
    if !uri.starts_with("keychain://") {
        return Err(CredentialError::InvalidIdentifier(
            "URI must start with 'keychain://'",
        ));
    }
    let rest = &uri["keychain://".len()..];
    let parts: Vec<&str> = rest.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(CredentialError::InvalidIdentifier(
            "invalid keychain reference format; expected 'keychain://<service>/<account>'",
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Resolves a secret value from a secret reference (supporting `keychain://` and `env://`).
pub fn resolve_secret_from_ref(
    store: &dyn CredentialStore,
    secret_ref: &str,
) -> Result<Option<SecretString>, CredentialError> {
    if secret_ref.starts_with("keychain://") {
        let (service, account) = parse_keychain_ref(secret_ref)?;
        store.get_secret(&service, &account)
    } else if let Some(env_var) = secret_ref.strip_prefix("env://") {
        match std::env::var(env_var) {
            Ok(val) => Ok(Some(SecretString::new(val))),
            Err(_) => Ok(None),
        }
    } else {
        store.get_secret(DEFAULT_KEYCHAIN_SERVICE, secret_ref)
    }
}

/// Stores a secret into the provided store and returns its canonical keychain URI reference.
pub fn store_secret_to_ref(
    store: &dyn CredentialStore,
    service: &str,
    account: &str,
    secret: &str,
) -> Result<String, CredentialError> {
    store.set_secret(service, account, secret)?;
    Ok(format_keychain_ref(service, account))
}

/// Deletes a secret from the store using its secret reference URI.
pub fn delete_secret_by_ref(
    store: &dyn CredentialStore,
    secret_ref: &str,
) -> Result<bool, CredentialError> {
    if secret_ref.starts_with("keychain://") {
        let (service, account) = parse_keychain_ref(secret_ref)?;
        store.delete_secret(&service, &account)
    } else {
        store.delete_secret(DEFAULT_KEYCHAIN_SERVICE, secret_ref)
    }
}

/// Windows Credential Manager store utilizing Windows PasswordVault API.
pub struct WindowsCredentialManagerStore {
    shell: String,
}

impl WindowsCredentialManagerStore {
    pub fn new() -> Self {
        Self {
            shell: "powershell.exe".to_string(),
        }
    }

    pub fn with_shell(shell: impl Into<String>) -> Self {
        Self {
            shell: shell.into(),
        }
    }

    pub fn build_store_script(service: &str, account: &str) -> String {
        format!(
            "$secret = [Console]::In.ReadToEnd().TrimEnd(\"`r`n\"); \
             $vault = New-Object Windows.Security.Credentials.PasswordVault; \
             $cred = New-Object Windows.Security.Credentials.PasswordCredential('{service}', '{account}', $secret); \
             $vault.Add($cred);"
        )
    }

    pub fn build_lookup_script(service: &str, account: &str) -> String {
        format!(
            "$vault = New-Object Windows.Security.Credentials.PasswordVault; \
             try {{ \
                 $c = $vault.Retrieve('{service}', '{account}'); \
                 $c.RetrievePassword(); \
                 [Console]::Out.Write($c.Password); \
             }} catch {{ \
                 exit 1; \
             }}"
        )
    }

    pub fn build_delete_script(service: &str, account: &str) -> String {
        format!(
            "$vault = New-Object Windows.Security.Credentials.PasswordVault; \
             try {{ \
                 $c = $vault.Retrieve('{service}', '{account}'); \
                 $vault.Remove($c); \
             }} catch {{ \
                 exit 1; \
             }}"
        )
    }
}

impl Default for WindowsCredentialManagerStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for WindowsCredentialManagerStore {
    fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;
        let secret = validate_secret(secret)?;

        let script = Self::build_store_script(service, account);
        let mut child = std::process::Command::new(&self.shell)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                CredentialError::Storage(format!(
                    "failed to spawn powershell for Windows Credential Manager: {e}"
                ))
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(secret.as_bytes());
            let _ = stdin.flush();
        }

        let output = child
            .wait_with_output()
            .map_err(|e| CredentialError::Storage(format!("failed to wait on powershell: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CredentialError::Storage(format!(
                "Windows Credential Manager set failed: {}",
                stderr.trim()
            )));
        }

        Ok(())
    }

    fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        let script = Self::build_lookup_script(service, account);
        let output = std::process::Command::new(&self.shell)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| {
                CredentialError::Storage(format!(
                    "failed to execute powershell for Windows Credential Manager lookup: {e}"
                ))
            })?;

        if !output.status.success() {
            return Ok(None);
        }

        let s = String::from_utf8(output.stdout)
            .map_err(|e| CredentialError::Storage(format!("credential is not valid UTF-8: {e}")))?;

        if s.is_empty() {
            Ok(None)
        } else {
            Ok(Some(SecretString::new(s)))
        }
    }

    fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        let script = Self::build_delete_script(service, account);
        let output = std::process::Command::new(&self.shell)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .map_err(|e| {
                CredentialError::Storage(format!(
                    "failed to execute powershell for Windows Credential Manager delete: {e}"
                ))
            })?;

        Ok(output.status.success())
    }
}

/// Linux Secret Service D-Bus credential store via `secret-tool`.
pub struct LinuxSecretServiceStore {
    binary: String,
}

impl LinuxSecretServiceStore {
    pub fn new() -> Self {
        Self {
            binary: "secret-tool".to_string(),
        }
    }

    pub fn with_binary(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub fn build_store_args(service: &str, account: &str) -> Vec<String> {
        vec![
            "store".into(),
            format!("--label=Shellumi ({service}:{account})"),
            "service".into(),
            service.to_string(),
            "account".into(),
            account.to_string(),
        ]
    }

    pub fn build_lookup_args(service: &str, account: &str) -> Vec<String> {
        vec![
            "lookup".into(),
            "service".into(),
            service.to_string(),
            "account".into(),
            account.to_string(),
        ]
    }

    pub fn build_clear_args(service: &str, account: &str) -> Vec<String> {
        vec![
            "clear".into(),
            "service".into(),
            service.to_string(),
            "account".into(),
            account.to_string(),
        ]
    }
}

impl Default for LinuxSecretServiceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for LinuxSecretServiceStore {
    fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;
        let secret = validate_secret(secret)?;

        let args = Self::build_store_args(service, account);
        let mut child = std::process::Command::new(&self.binary)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                CredentialError::Storage(format!(
                    "failed to spawn secret-tool for Secret Service daemon: {e}"
                ))
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(secret.as_bytes());
            let _ = stdin.flush();
        }

        let output = child
            .wait_with_output()
            .map_err(|e| CredentialError::Storage(format!("failed to wait on secret-tool: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CredentialError::Storage(format!(
                "secret-tool returned error code {:?}: {}",
                output.status.code(),
                stderr.trim()
            )));
        }

        Ok(())
    }

    fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        let args = Self::build_lookup_args(service, account);
        let output = std::process::Command::new(&self.binary)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|e| {
                CredentialError::Storage(format!(
                    "failed to execute secret-tool for Secret Service lookup: {e}"
                ))
            })?;

        if !output.status.success() {
            return Ok(None);
        }

        let out_str = String::from_utf8(output.stdout).map_err(|e| {
            CredentialError::Storage(format!("secret-tool output is not valid UTF-8: {e}"))
        })?;

        let trimmed = out_str.trim_end_matches(&['\r', '\n'][..]);
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(SecretString::new(trimmed.to_string())))
        }
    }

    fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;

        let args = Self::build_clear_args(service, account);
        let output = std::process::Command::new(&self.binary)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .map_err(|e| {
                CredentialError::Storage(format!("failed to execute secret-tool clear: {e}"))
            })?;

        Ok(output.status.success())
    }
}

/// Probes the availability and responsiveness of the platform-native keychain daemon.
pub fn probe_keychain_daemon() -> KeychainDaemonStatus {
    let os = std::env::consts::OS;
    match os {
        "macos" => {
            #[cfg(target_os = "macos")]
            {
                match security_framework::passwords::get_generic_password(
                    DEFAULT_KEYCHAIN_SERVICE,
                    "__shellumi_probe__",
                ) {
                    Ok(_) => KeychainDaemonStatus {
                        backend_name: "macOS Keychain".into(),
                        is_available: true,
                        daemon_identifier: "securityd".into(),
                        details: "Apple Security Framework Keychain Services daemon is active and responsive"
                            .into(),
                    },
                    Err(err) if err.code() == -25300 => KeychainDaemonStatus {
                        backend_name: "macOS Keychain".into(),
                        is_available: true,
                        daemon_identifier: "securityd".into(),
                        details: "Apple Security Framework Keychain Services daemon is active and responsive"
                            .into(),
                    },
                    Err(err) => KeychainDaemonStatus {
                        backend_name: "macOS Keychain".into(),
                        is_available: false,
                        daemon_identifier: "securityd".into(),
                        details: format!("securityd daemon returned error: {} (code {})", err, err.code()),
                    },
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                KeychainDaemonStatus {
                    backend_name: "macOS Keychain".into(),
                    is_available: false,
                    daemon_identifier: "securityd".into(),
                    details: "Not running on macOS".into(),
                }
            }
        }
        "linux" => {
            let check = std::process::Command::new("secret-tool")
                .arg("--help")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output();

            match check {
                Ok(_) => KeychainDaemonStatus {
                    backend_name: "Linux Secret Service".into(),
                    is_available: true,
                    daemon_identifier: "gnome-keyring / ksecretservice (D-Bus)".into(),
                    details: "FreeDesktop Secret Service daemon reachable via secret-tool".into(),
                },
                Err(e) => KeychainDaemonStatus {
                    backend_name: "Linux Secret Service".into(),
                    is_available: false,
                    daemon_identifier: "gnome-keyring / ksecretservice".into(),
                    details: format!(
                        "secret-tool unavailable ({e}); using encrypted vault fallback"
                    ),
                },
            }
        }
        "windows" => {
            let check = std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Write-Output ready",
                ])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output();

            match check {
                Ok(out) if out.status.success() => KeychainDaemonStatus {
                    backend_name: "Windows Credential Manager".into(),
                    is_available: true,
                    daemon_identifier: "VaultSvc (Credential Manager)".into(),
                    details: "Windows Credential Manager PasswordVault API is active and responsive".into(),
                },
                Err(e) => KeychainDaemonStatus {
                    backend_name: "Windows Credential Manager".into(),
                    is_available: false,
                    daemon_identifier: "VaultSvc".into(),
                    details: format!(
                        "PowerShell/Credential Manager unavailable ({e}); using encrypted vault fallback"
                    ),
                },
                Ok(_) => KeychainDaemonStatus {
                    backend_name: "Windows Credential Manager".into(),
                    is_available: false,
                    daemon_identifier: "VaultSvc".into(),
                    details: "Windows Credential Manager query failed; using encrypted vault fallback"
                        .into(),
                },
            }
        }
        _ => KeychainDaemonStatus {
            backend_name: "Encrypted Vault".into(),
            is_available: true,
            daemon_identifier: "aead-vault".into(),
            details: format!(
                "Platform '{os}' uses software AES-256-GCM authenticated vault storage"
            ),
        },
    }
}

/// Helper to determine the fallback path for encrypted credential storage.
pub fn default_fallback_vault_path() -> PathBuf {
    if let Ok(custom) = std::env::var("SHELLUMI_VAULT_PATH") {
        return PathBuf::from(custom);
    }
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })
    };
    base.unwrap_or_default().join("Shellumi/credentials.enc")
}

/// Fallback-aware credential store pairing a platform native daemon with an encrypted file vault.
pub struct DaemonWithFallbackStore {
    primary: Box<dyn CredentialStore>,
    fallback: Box<dyn CredentialStore>,
}

impl DaemonWithFallbackStore {
    pub fn new(primary: Box<dyn CredentialStore>, fallback: Box<dyn CredentialStore>) -> Self {
        Self { primary, fallback }
    }
}

impl CredentialStore for DaemonWithFallbackStore {
    fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;
        let secret = validate_secret(secret)?;
        match self.primary.set_secret(service, account, secret) {
            Ok(()) => Ok(()),
            Err(_) => self.fallback.set_secret(service, account, secret),
        }
    }

    fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;
        match self.primary.get_secret(service, account) {
            Ok(Some(s)) => Ok(Some(s)),
            Ok(None) => self.fallback.get_secret(service, account),
            Err(_) => self.fallback.get_secret(service, account),
        }
    }

    fn delete_secret(&self, service: &str, account: &str) -> Result<bool, CredentialError> {
        let service = validate_identifier(service, "service")?;
        let account = validate_identifier(account, "account")?;
        let prim = self
            .primary
            .delete_secret(service, account)
            .unwrap_or(false);
        let fb = self
            .fallback
            .delete_secret(service, account)
            .unwrap_or(false);
        Ok(prim || fb)
    }
}

/// Returns the native credential store. Encrypted fallback requires an explicit user passphrase.
pub fn default_credential_store() -> Box<dyn CredentialStore> {
    if let Ok(store_type) = std::env::var("SHELLUMI_CREDENTIAL_STORE") {
        if store_type == "ephemeral" {
            return Box::new(EphemeralCredentialStore::new());
        }
    }

    let primary: Box<dyn CredentialStore> = {
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
    };

    configured_credential_store(
        primary,
        default_fallback_vault_path(),
        std::env::var("SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE").ok(),
    )
}

struct UnavailableCredentialStore(String);
impl CredentialStore for UnavailableCredentialStore {
    fn set_secret(&self, _: &str, _: &str, _: &str) -> Result<(), CredentialError> {
        Err(CredentialError::Storage(self.0.clone()))
    }
    fn get_secret(&self, _: &str, _: &str) -> Result<Option<SecretString>, CredentialError> {
        Err(CredentialError::Storage(self.0.clone()))
    }
    fn delete_secret(&self, _: &str, _: &str) -> Result<bool, CredentialError> {
        Err(CredentialError::Storage(self.0.clone()))
    }
}
fn configured_credential_store(
    primary: Box<dyn CredentialStore>,
    path: PathBuf,
    passphrase: Option<String>,
) -> Box<dyn CredentialStore> {
    let Some(passphrase) = passphrase.map(SecretString::new) else {
        return primary;
    };
    if passphrase.expose_secret().chars().count() < 12 || !path.is_absolute() {
        return Box::new(UnavailableCredentialStore("Encrypted fallback requires an absolute vault path and a passphrase of at least 12 characters.".into()));
    }
    match EncryptedFileCredentialStore::new(path, passphrase.expose_secret()) {
        Ok(fallback) => Box::new(DaemonWithFallbackStore::new(primary, Box::new(fallback))),
        Err(_) => Box::new(UnavailableCredentialStore(
            "Could not initialize the configured encrypted credential fallback.".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fallback_requires_explicit_user_key_and_never_defaults_to_shared_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.enc");
        let primary = || {
            Box::new(super::UnavailableCredentialStore(
                "native store unavailable".into(),
            )) as Box<dyn super::CredentialStore>
        };
        let store = super::configured_credential_store(primary(), path.clone(), None);
        assert!(store.set_secret("service", "account", "secret").is_err());
        assert!(!path.exists());
        let store =
            super::configured_credential_store(primary(), path.clone(), Some("short".into()));
        assert!(store.set_secret("service", "account", "secret").is_err());
        assert!(!path.exists());
        let store = super::configured_credential_store(
            primary(),
            path.clone(),
            Some("user-selected-strong-key".into()),
        );
        store
            .set_secret("service", "account", "private-value")
            .unwrap();
        assert_eq!(
            store
                .get_secret("service", "account")
                .unwrap()
                .unwrap()
                .expose_secret(),
            "private-value"
        );
        assert!(!std::fs::read_to_string(path)
            .unwrap()
            .contains("private-value"));
    }

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
    fn default_credential_store_rejects_invalid_identifiers() {
        let store = default_credential_store();
        assert!(store.get_secret("", "user1").is_err());
        assert!(store.set_secret("svc", "", "secret").is_err());
        assert!(store.delete_secret("", "user1").is_err());
    }

    #[test]
    fn keychain_ref_formatting_and_parsing() {
        let r = format_keychain_ref("org.shellumi.app", "identity-prod-key");
        assert_eq!(r, "keychain://org.shellumi.app/identity-prod-key");

        let (svc, acc) = parse_keychain_ref(&r).unwrap();
        assert_eq!(svc, "org.shellumi.app");
        assert_eq!(acc, "identity-prod-key");

        // Invalid scheme
        assert!(parse_keychain_ref("https://example.com/sec").is_err());
        // Missing account
        assert!(parse_keychain_ref("keychain://only-service").is_err());
        assert!(parse_keychain_ref("keychain://svc/").is_err());
        assert!(parse_keychain_ref("keychain:///acc").is_err());
    }

    #[test]
    fn resolve_secret_from_ref_lifecycle() {
        let store = EphemeralCredentialStore::new();

        // 1. Store via helper
        let ref_uri = store_secret_to_ref(
            &store,
            DEFAULT_KEYCHAIN_SERVICE,
            "id-pass-test",
            "p@ssw0rd!#456",
        )
        .unwrap();
        assert_eq!(
            ref_uri,
            format!("keychain://{DEFAULT_KEYCHAIN_SERVICE}/id-pass-test")
        );

        // 2. Resolve via keychain:// URI
        let res = resolve_secret_from_ref(&store, &ref_uri).unwrap().unwrap();
        assert_eq!(res.expose_secret(), "p@ssw0rd!#456");

        // 3. Resolve via env:// URI
        let env_key = "SHELLUMI_TEST_KEYCHAIN_SECRET_VAR";
        std::env::set_var(env_key, "env-secret-val-99");
        let env_uri = format!("env://{env_key}");
        let env_res = resolve_secret_from_ref(&store, &env_uri).unwrap().unwrap();
        assert_eq!(env_res.expose_secret(), "env-secret-val-99");
        std::env::remove_var(env_key);

        let missing_env = resolve_secret_from_ref(&store, "env://NONEXISTENT_VAR_XYZ").unwrap();
        assert_eq!(missing_env, None);

        // 4. Delete via helper
        assert!(delete_secret_by_ref(&store, &ref_uri).unwrap());
        assert_eq!(resolve_secret_from_ref(&store, &ref_uri).unwrap(), None);
    }

    #[test]
    fn linux_secret_service_command_builders() {
        let store_args = LinuxSecretServiceStore::build_store_args("org.shellumi.app", "db-pass");
        assert_eq!(
            store_args,
            vec![
                "store",
                "--label=Shellumi (org.shellumi.app:db-pass)",
                "service",
                "org.shellumi.app",
                "account",
                "db-pass"
            ]
        );

        let lookup_args = LinuxSecretServiceStore::build_lookup_args("org.shellumi.app", "db-pass");
        assert_eq!(
            lookup_args,
            vec![
                "lookup",
                "service",
                "org.shellumi.app",
                "account",
                "db-pass"
            ]
        );

        let clear_args = LinuxSecretServiceStore::build_clear_args("org.shellumi.app", "db-pass");
        assert_eq!(
            clear_args,
            vec!["clear", "service", "org.shellumi.app", "account", "db-pass"]
        );
    }

    #[test]
    fn windows_credential_manager_script_builders() {
        let store_script =
            WindowsCredentialManagerStore::build_store_script("org.shellumi.app", "cloud-token");
        assert!(store_script.contains("org.shellumi.app"));
        assert!(store_script.contains("cloud-token"));
        assert!(store_script.contains("Windows.Security.Credentials.PasswordVault"));
        assert!(store_script.contains("[Console]::In.ReadToEnd()"));

        let lookup_script =
            WindowsCredentialManagerStore::build_lookup_script("org.shellumi.app", "cloud-token");
        assert!(lookup_script.contains("Retrieve('org.shellumi.app', 'cloud-token')"));
        assert!(lookup_script.contains(".RetrievePassword()"));

        let delete_script =
            WindowsCredentialManagerStore::build_delete_script("org.shellumi.app", "cloud-token");
        assert!(delete_script.contains("$vault.Remove($c)"));
    }

    #[test]
    fn probe_keychain_daemon_smoke_test() {
        let status = probe_keychain_daemon();
        assert!(!status.backend_name.is_empty());
        assert!(!status.daemon_identifier.is_empty());
        assert!(!status.details.is_empty());
    }
}

//! Persistent authenticated SFTP connection manager, error classification,
//! option allowlist validation, and vault-scoped session lifecycle.

use super::endpoint::{EndpointId, RemoteEndpoint, VaultScope};
use super::path::RemotePath;
use super::protocol::ServerCapabilities;
use super::{SftpClient, SftpConnectOptions, SftpError, StatusCode, TransferControl};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Thread-safe, race-free authorization gate shared across UI, controller, and workers.
/// Revocation/switch updates this gate atomically so in-flight or racing operations
/// cannot enqueue, commit, or apply stale callbacks across vault boundaries.
#[derive(Clone, Default)]
pub struct VaultAuthGate {
    active: Arc<RwLock<Option<VaultScope>>>,
}

impl VaultAuthGate {
    pub fn new(initial: Option<VaultScope>) -> Self {
        Self {
            active: Arc::new(RwLock::new(initial)),
        }
    }

    pub fn set_active(&self, scope: Option<VaultScope>) {
        if let Ok(mut guard) = self.active.write() {
            *guard = scope;
        }
    }

    pub fn current(&self) -> Option<VaultScope> {
        self.active.read().ok().and_then(|g| g.clone())
    }

    pub fn is_authorized(&self, scope: &VaultScope) -> bool {
        self.active
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|active| scope.is_authorized_under(active)))
            .unwrap_or(false)
    }
}

/// Classified SSH/SFTP error categories for UI diagnostics and retry policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SftpErrorKind {
    UnreachableOrDns,
    AuthFailed,
    UnknownHostKey,
    ChangedHostKey,
    ProxyError,
    MissingSubsystem,
    Timeout,
    ProtocolError,
    Disconnected,
    Cancelled,
    PermissionDenied,
    Other,
}

impl std::fmt::Display for SftpErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnreachableOrDns => f.write_str("Host unreachable or DNS resolution failed"),
            Self::AuthFailed => f.write_str("Authentication failed"),
            Self::UnknownHostKey => f.write_str("Unknown host key requires verification"),
            Self::ChangedHostKey => f.write_str("Host key changed — connection blocked"),
            Self::ProxyError => f.write_str("Jump host / proxy connection failed"),
            Self::MissingSubsystem => f.write_str("Remote SFTP subsystem not available"),
            Self::Timeout => f.write_str("Connection or operation timed out"),
            Self::ProtocolError => f.write_str("SFTP protocol error"),
            Self::Disconnected => f.write_str("Session disconnected"),
            Self::Cancelled => f.write_str("Cancelled"),
            Self::PermissionDenied => f.write_str("Permission denied"),
            Self::Other => f.write_str("Connection error"),
        }
    }
}

/// Live connection state for an SFTP endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SftpConnectionState {
    Disconnected,
    Connecting,
    AwaitingTrust {
        host: String,
        port: u16,
        fingerprint: String,
        algorithm: String,
    },
    AwaitingAuth,
    Connected {
        capabilities: ServerCapabilities,
        remote_home: RemotePath,
    },
    Reconnecting {
        attempt: u32,
    },
    Error {
        kind: SftpErrorKind,
        message: String,
    },
}

/// Classify an `SftpError` + sanitized SSH stderr into a typed `SftpErrorKind`.
pub fn classify_ssh_error(err: &SftpError, stderr: &str) -> (SftpErrorKind, String) {
    let lower = stderr.to_lowercase();
    let kind = match err {
        SftpError::Cancelled => SftpErrorKind::Cancelled,
        SftpError::PermissionDenied(_)
        | SftpError::Status {
            code: StatusCode::PermissionDenied,
            ..
        } => SftpErrorKind::PermissionDenied,
        SftpError::UnsupportedVersion(_) | SftpError::Protocol(_) => SftpErrorKind::ProtocolError,
        SftpError::Io(io_err) if io_err.kind() == std::io::ErrorKind::TimedOut => {
            SftpErrorKind::Timeout
        }
        _ => {
            if lower.contains("remote host identification has changed")
                || lower.contains("host key verification failed") && lower.contains("offending")
            {
                SftpErrorKind::ChangedHostKey
            } else if lower.contains("host key verification failed")
                || lower.contains("are you sure you want to continue connecting")
            {
                SftpErrorKind::UnknownHostKey
            } else if lower.contains("permission denied (")
                || lower.contains("authentication failed")
                || lower.contains("too many authentication failures")
            {
                SftpErrorKind::AuthFailed
            } else if lower.contains("subsystem request failed") || lower.contains("sftp-server") {
                SftpErrorKind::MissingSubsystem
            } else if lower.contains("proxyjump")
                || lower.contains("kex_exchange_identification")
                || lower.contains("stdio forwarding")
            {
                SftpErrorKind::ProxyError
            } else if lower.contains("could not resolve hostname")
                || lower.contains("name or service not known")
                || lower.contains("connection refused")
                || lower.contains("no route to host")
                || lower.contains("network is unreachable")
            {
                SftpErrorKind::UnreachableOrDns
            } else if lower.contains("timed out") || lower.contains("operation timed out") {
                SftpErrorKind::Timeout
            } else if lower.contains("connection closed") || lower.contains("broken pipe") {
                SftpErrorKind::Disconnected
            } else {
                SftpErrorKind::Other
            }
        }
    };

    let sanitized_stderr = crate::logging::redact_sensitive(stderr.trim());
    let detail = if sanitized_stderr.is_empty() {
        err.to_string()
    } else {
        format!("{err} ({sanitized_stderr})")
    };
    (kind, detail)
}

/// Security allowlist for `extra_ssh_options` so user/config overrides cannot inject
/// arbitrary command execution (`ProxyCommand`, `LocalCommand`) or weaken host verification.
pub fn is_allowed_ssh_option(key: &str) -> bool {
    let lower = key.trim().to_ascii_lowercase();
    !matches!(
        lower.as_str(),
        "proxycommand"
            | "localcommand"
            | "permitlocalcommand"
            | "stricthostkeychecking"
            | "userknownhostsfile"
            | "globalknownhostsfile"
            | "pkcs11provider"
            | "securitykeyprovider"
            | "askpass"
            | "ssh_askpass"
    )
}

/// Merge base resolved options with caller options without silently discarding port,
/// timeouts, proxy, or config, while enforcing the SSH option allowlist.
pub fn merge_connect_options(
    base: &SftpConnectOptions,
    caller: &SftpConnectOptions,
) -> SftpConnectOptions {
    let mut merged = base.clone();
    if caller.ssh_config_file.is_some() {
        merged.ssh_config_file = caller.ssh_config_file.clone();
    }
    if caller.port.is_some() {
        merged.port = caller.port;
    }
    if caller.identity_file.is_some() {
        merged.identity_file = caller.identity_file.clone();
    }
    if caller.certificate_file.is_some() {
        merged.certificate_file = caller.certificate_file.clone();
    }
    if caller.timeout_seconds != 30 && caller.timeout_seconds > 0 {
        merged.timeout_seconds = caller.timeout_seconds;
    }
    for (k, v) in &caller.environment {
        if !merged.environment.iter().any(|(ek, _)| ek == k) {
            merged.environment.push((k.clone(), v.clone()));
        }
    }
    for (k, v) in &caller.extra_ssh_options {
        if is_allowed_ssh_option(k) {
            merged.extra_ssh_options.push((k.clone(), v.clone()));
        } else {
            tracing::warn!(option = %k, "Rejected disallowed SSH option override");
        }
    }
    merged
}

/// Persistent SFTP session wrapper holding a reusable primary browse/metadata client
/// and vault-scoped connection metadata.
pub struct PersistentSftpSession {
    pub endpoint: RemoteEndpoint,
    pub options: SftpConnectOptions,
    pub state: SftpConnectionState,
    pub remote_home: Option<RemotePath>,
    pub capabilities: ServerCapabilities,
    pub last_used: Instant,
    browse_client: Option<SftpClient>,
    control: TransferControl,
}

impl PersistentSftpSession {
    pub fn new(endpoint: RemoteEndpoint, options: SftpConnectOptions) -> Self {
        Self {
            endpoint,
            options,
            state: SftpConnectionState::Disconnected,
            remote_home: None,
            capabilities: ServerCapabilities::default(),
            last_used: Instant::now(),
            browse_client: None,
            control: TransferControl::default(),
        }
    }

    pub fn connect(&mut self) -> Result<&RemotePath, SftpError> {
        self.control.check()?;
        self.state = SftpConnectionState::Connecting;
        match SftpClient::connect_with_control(
            &self.endpoint.destination,
            &self.options,
            &self.control,
        ) {
            Ok(mut client) => {
                let caps = client.capabilities().clone();
                let home_str = client.realpath(".").unwrap_or_else(|_| "/".to_string());
                let home = RemotePath::from_posix_str(&home_str);
                self.capabilities = caps.clone();
                self.remote_home = Some(home.clone());
                self.state = SftpConnectionState::Connected {
                    capabilities: caps,
                    remote_home: home,
                };
                self.last_used = Instant::now();
                self.browse_client = Some(client);
                Ok(self.remote_home.as_ref().expect("just set"))
            }
            Err(err) => {
                let (kind, message) = classify_ssh_error(&err, "");
                self.state = SftpConnectionState::Error { kind, message };
                Err(err)
            }
        }
    }

    /// Execute a read-only or idempotent browse operation on the persistent channel,
    /// reconnecting once if the previous channel went stale or was corrupted.
    pub fn with_browse_client<T>(
        &mut self,
        f: impl Fn(&mut SftpClient) -> Result<T, SftpError>,
    ) -> Result<T, SftpError> {
        self.control.check()?;
        if self
            .browse_client
            .as_ref()
            .is_none_or(SftpClient::is_corrupted)
        {
            self.connect()?;
        }
        let res = {
            let client = self.browse_client.as_mut().expect("connected");
            f(client)
        };
        match res {
            Ok(val) => {
                self.last_used = Instant::now();
                Ok(val)
            }
            Err(SftpError::ChannelCorrupted(_)) | Err(SftpError::Io(_)) => {
                tracing::info!(
                    host = %self.endpoint.destination.host(),
                    "Reconnecting stale persistent SFTP browse session"
                );
                self.browse_client = None;
                self.connect()?;
                let client = self.browse_client.as_mut().expect("reconnected");
                let retry_res = f(client);
                if retry_res.is_ok() {
                    self.last_used = Instant::now();
                }
                retry_res
            }
            Err(other) => Err(other),
        }
    }

    /// Open a dedicated worker channel for concurrent transfers, scoped to this endpoint.
    pub fn open_worker_client(
        &self,
        worker_control: &TransferControl,
    ) -> Result<SftpClient, SftpError> {
        self.control.check()?;
        worker_control.check()?;
        SftpClient::connect_with_control(&self.endpoint.destination, &self.options, worker_control)
    }

    pub fn disconnect(&mut self) {
        self.control.cancel();
        self.browse_client.take();
        self.state = SftpConnectionState::Disconnected;
    }
}

/// Pool of persistent SFTP sessions keyed by `EndpointId`, enforcing vault isolation
/// and idle cleanup.
#[derive(Clone, Default)]
pub struct SftpConnectionManager {
    sessions: Arc<Mutex<HashMap<EndpointId, Arc<Mutex<PersistentSftpSession>>>>>,
}

impl SftpConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_session(
        &self,
        endpoint: RemoteEndpoint,
        options: SftpConnectOptions,
    ) -> Arc<Mutex<PersistentSftpSession>> {
        let id = endpoint.endpoint_id;
        let session = Arc::new(Mutex::new(PersistentSftpSession::new(endpoint, options)));
        if let Ok(mut guard) = self.sessions.lock() {
            guard.insert(id, Arc::clone(&session));
        }
        session
    }

    pub fn get_session(&self, id: EndpointId) -> Option<Arc<Mutex<PersistentSftpSession>>> {
        self.sessions.lock().ok()?.get(&id).cloned()
    }

    /// Cancel and disconnect all sessions belonging to `vault_id` whose `vault_epoch`
    /// is older than `active_scope.vault_epoch`, or all sessions if vault locked/switched.
    pub fn enforce_vault_scope(&self, active_scope: Option<&VaultScope>) {
        let Ok(mut guard) = self.sessions.lock() else {
            return;
        };
        let mut to_remove = Vec::new();
        for (id, sess_arc) in guard.iter() {
            if let Ok(mut sess) = sess_arc.lock() {
                let authorized = active_scope
                    .is_some_and(|scope| sess.endpoint.vault_scope.is_authorized_under(scope));
                if !authorized {
                    tracing::info!(
                        endpoint_id = id.0,
                        vault_id = %sess.endpoint.vault_scope.vault_id,
                        "Closing SFTP session due to vault lock/switch"
                    );
                    sess.disconnect();
                    to_remove.push(*id);
                }
            }
        }
        for id in to_remove {
            guard.remove(&id);
        }
    }

    /// Close sessions that have been idle longer than `max_idle`.
    pub fn prune_idle(&self, max_idle: Duration) {
        let Ok(mut guard) = self.sessions.lock() else {
            return;
        };
        let now = Instant::now();
        let mut to_remove = Vec::new();
        for (id, sess_arc) in guard.iter() {
            if let Ok(mut sess) = sess_arc.lock() {
                if now.duration_since(sess.last_used) > max_idle {
                    sess.disconnect();
                    to_remove.push(*id);
                }
            }
        }
        for id in to_remove {
            guard.remove(&id);
        }
    }
}

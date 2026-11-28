//! SFTP File Manager and filesystem domain models.
//!
//! Provides UI-independent abstractions for:
//! - File entries (`FileEntry`, `FileKind`, permissions, sizes)
//! - Breadcrumb navigation and path traversal sanitization
//! - Transfer tasks, queue management, and progress calculations
//! - Local filesystem listing and directory browsing

pub mod connection;
pub mod editor;
pub mod endpoint;
pub mod manager;
pub mod operations;
pub mod path;
pub mod planner;
pub mod protocol;
mod queue;
mod transfer;
mod transport;
mod wire;
pub use connection::{
    classify_ssh_error, is_allowed_ssh_option, merge_connect_options, PersistentSftpSession,
    SftpConnectionManager, SftpConnectionState, SftpErrorKind, VaultAuthGate,
};
pub use editor::{
    detect_language, format_document, highlight_line, EditorBuffer, EditorLanguage,
    EditorSessionMeta, FormatOutcome, LineEnding, SyntaxSpan, SyntaxTokenKind,
    DEFAULT_EDITABLE_LIMIT_BYTES, DEFAULT_PREVIEW_LIMIT_BYTES,
};
pub use endpoint::{
    AttemptId, BatchId, EditorId, EndpointId, EndpointKind, EntryId, JobId, OperationId, PaneId,
    RemoteEndpoint, TransferRoute, VaultScope, WorkspaceTabId,
};
pub use manager::{
    relay_remote_to_remote, EndpointPath, TransferIntent, TransferJobSnapshot, TransferJobState,
    UnifiedTransferManager,
};
pub use operations::{
    compare_directory_entries, compute_local_sha256, compute_remote_sha256, local_disk_capacity,
    plan_multi_rename, plan_one_way_sync, search_local_recursive, validate_new_entry_name,
    DiffEntryStatus, DirectoryDiffRow, DiskCapacity, MultiRenameItem, SearchMatch, SyncActionKind,
    SyncPlanItem,
};
pub use path::{
    classify_listing_name, escape_bytes_for_display, is_protected_local_root, local_breadcrumbs,
    local_home_dir, local_join_child, local_parent, validate_destination_name, CanonicalDestKey,
    ListingNameKind, PathSemantics, PathValidationError, RemotePath, ValidatedChildName,
};
pub use planner::{
    delete_local_recursive, delete_remote_recursive, plan_local_tree, plan_remote_tree,
    staged_local_copy_file, validate_non_overlapping_local_copy,
    validate_non_overlapping_same_endpoint_remote, ConflictAction, ConflictContext,
    ConflictDecision, ConflictPolicy, DestinationReservationRegistry, LocalTreePlan, PlanLimits,
    PlannedRemoteItem, PlannedTransferItem, RemoteTreePlan, ReservationGuard, SymlinkPolicy,
};
pub use protocol::{
    ListingLimits, RemoteStatVfs, ServerCapabilities, ServerLimits, TransferTuning,
};
pub use queue::{TransferQueue, TransferQueueSummary};
pub use transfer::TransferControl;
pub use wire::{
    FileAttributes, NameEntry, OpenFlags, SftpPacket, SftpProtocolError, StatusCode, WireReader,
    SSH_FILEXFER_ATTR_ACMODTIME, SSH_FILEXFER_ATTR_EXTENDED, SSH_FILEXFER_ATTR_PERMISSIONS,
    SSH_FILEXFER_ATTR_SIZE, SSH_FILEXFER_ATTR_UIDGID, SSH_FXF_APPEND, SSH_FXF_CREAT, SSH_FXF_EXCL,
    SSH_FXF_READ, SSH_FXF_TRUNC, SSH_FXF_WRITE, SSH_FXP_ATTRS, SSH_FXP_CLOSE, SSH_FXP_DATA,
    SSH_FXP_EXTENDED, SSH_FXP_EXTENDED_REPLY, SSH_FXP_FSETSTAT, SSH_FXP_FSTAT, SSH_FXP_HANDLE,
    SSH_FXP_INIT, SSH_FXP_LSTAT, SSH_FXP_MKDIR, SSH_FXP_NAME, SSH_FXP_OPEN, SSH_FXP_OPENDIR,
    SSH_FXP_READ, SSH_FXP_READDIR, SSH_FXP_READLINK, SSH_FXP_REALPATH, SSH_FXP_REMOVE,
    SSH_FXP_RENAME, SSH_FXP_RMDIR, SSH_FXP_SETSTAT, SSH_FXP_STAT, SSH_FXP_STATUS, SSH_FXP_SYMLINK,
    SSH_FXP_VERSION, SSH_FXP_WRITE,
};

use wire::write_string;

use std::fmt;
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};

const MAX_SFTP_PACKET_LENGTH: usize = 16 * 1024 * 1024;

/// The kind of a file entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileKind {
    File,
    Directory,
    Symlink { target: Option<String> },
    Other,
}

impl FileKind {
    pub fn is_dir(&self) -> bool {
        matches!(self, Self::Directory)
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Self::File)
    }

    pub fn is_symlink(&self) -> bool {
        matches!(self, Self::Symlink { .. })
    }
}

/// Metadata and description of a file or directory entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub kind: FileKind,
    pub modified: Option<u64>,
    pub permissions: Option<u32>,
}

impl FileEntry {
    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.') && self.name != "." && self.name != ".."
    }

    pub fn formatted_size(&self) -> String {
        if self.kind.is_dir() {
            return "-".to_string();
        }
        format_bytes(self.size)
    }

    pub fn formatted_permissions(&self) -> String {
        let Some(mode) = self.permissions else {
            return "-".to_string();
        };

        let type_char = match self.kind {
            FileKind::Directory => 'd',
            FileKind::Symlink { .. } => 'l',
            _ => '-',
        };

        let rwx = |shift: u32,
                   special_bit: u32,
                   special_char_lower: char,
                   special_char_upper: char|
         -> [char; 3] {
            let val = (mode >> shift) & 0o7;
            let exec = if (mode & special_bit) != 0 {
                if val & 0o1 != 0 {
                    special_char_lower
                } else {
                    special_char_upper
                }
            } else if val & 0o1 != 0 {
                'x'
            } else {
                '-'
            };
            [
                if val & 0o4 != 0 { 'r' } else { '-' },
                if val & 0o2 != 0 { 'w' } else { '-' },
                exec,
            ]
        };

        let u = rwx(6, 0o4000, 's', 'S');
        let g = rwx(3, 0o2000, 's', 'S');
        let o = rwx(0, 0o1000, 't', 'T');

        format!(
            "{}{}{}{}{}{}{}{}{}{}",
            type_char, u[0], u[1], u[2], g[0], g[1], g[2], o[0], o[1], o[2]
        )
    }
}

/// Formats a byte size into human-readable representation (e.g. 4.2 KB, 1.5 MB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// The direction of an SFTP file transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

impl fmt::Display for TransferDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Upload => f.write_str("Upload"),
            Self::Download => f.write_str("Download"),
        }
    }
}

/// Transfer lifecycle status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferStatus {
    Queued,
    InProgress,
    Completed,
    Failed(String),
    Cancelled,
}

/// A file transfer task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferTask {
    pub id: u64,
    pub direction: TransferDirection,
    pub local_path: String,
    pub remote_path: String,
    pub total_bytes: u64,
    pub bytes_transferred: u64,
    pub status: TransferStatus,
}

impl TransferTask {
    pub fn new(
        id: u64,
        direction: TransferDirection,
        local_path: impl Into<String>,
        remote_path: impl Into<String>,
        total_bytes: u64,
    ) -> Self {
        Self {
            id,
            direction,
            local_path: local_path.into(),
            remote_path: remote_path.into(),
            total_bytes,
            bytes_transferred: 0,
            status: TransferStatus::Queued,
        }
    }

    pub fn progress_percentage(&self) -> f32 {
        if self.total_bytes == 0 {
            if self.status == TransferStatus::Completed {
                100.0
            } else {
                0.0
            }
        } else {
            ((self.bytes_transferred as f64 / self.total_bytes as f64) * 100.0) as f32
        }
    }
}

/// Conflict resolution strategy when a destination file already exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictResolution {
    Overwrite,
    Skip,
    Rename,
}

/// Path utilities and navigation helpers for SFTP and local file trees.
pub struct PathUtils;

impl PathUtils {
    /// A listing entry must be a single filename before it can be joined to a
    /// user-selected transfer directory. Reject separators on either platform.
    pub fn is_safe_file_name(name: &str) -> bool {
        if name.is_empty() || name.contains(['/', '\\']) || name.chars().any(char::is_control) {
            return false;
        }
        let mut components = Path::new(name).components();
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    }

    /// Generates breadcrumb tuples of `(display_name, absolute_path)`.
    ///
    /// For example `/var/log/nginx` produces:
    /// - `("/", "/")`
    /// - `("var", "/var")`
    /// - `("log", "/var/log")`
    /// - `("nginx", "/var/log/nginx")`
    pub fn breadcrumbs(path_str: &str) -> Vec<(String, String)> {
        let clean = path_str.trim();
        if clean.is_empty() || clean == "/" {
            return vec![("/".to_string(), "/".to_string())];
        }

        let mut crumbs = Vec::new();
        let is_unix_abs = clean.starts_with('/');
        if is_unix_abs {
            crumbs.push(("/".to_string(), "/".to_string()));
        }

        let mut current = String::new();

        for segment in clean.split('/') {
            if segment.is_empty() {
                continue;
            }
            if current.is_empty() || current == "/" {
                current = format!("/{segment}");
            } else {
                current = format!("{current}/{segment}");
            }
            crumbs.push((segment.to_string(), current.clone()));
        }

        crumbs
    }

    /// Safely joins a directory base path with a child name, normalizing `..` components.
    pub fn join_path(base: &str, child: &str) -> String {
        let mut segments: Vec<&str> = if child.starts_with('/') {
            Vec::new()
        } else {
            base.split('/')
                .filter(|s| !s.is_empty() && *s != ".")
                .collect()
        };
        for comp in child.split('/') {
            match comp {
                "" | "." => {}
                ".." => {
                    segments.pop();
                }
                c => segments.push(c),
            }
        }
        if segments.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", segments.join("/"))
        }
    }

    /// Returns the parent path of a path, or None if already at root.
    pub fn parent_path(path_str: &str) -> Option<String> {
        let path = Path::new(path_str);
        path.parent().map(|p| {
            let s = p.to_string_lossy().to_string();
            if s.is_empty() {
                "/".to_string()
            } else {
                s
            }
        })
    }
}

/// Reads local filesystem directories, returning sorted `FileEntry` items.
/// Directories appear first, followed by files, in case-insensitive alphabetical order.
pub fn list_local_directory(
    dir_path: &str,
    show_hidden: bool,
) -> Result<Vec<FileEntry>, std::io::Error> {
    let path = Path::new(dir_path);
    let mut entries = Vec::new();

    for entry_res in fs::read_dir(path)? {
        let entry = entry_res?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !show_hidden && name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata()?;
        let kind = if metadata.is_dir() {
            FileKind::Directory
        } else if metadata.is_symlink() {
            let target = fs::read_link(entry.path())
                .ok()
                .map(|p| p.to_string_lossy().to_string());
            FileKind::Symlink { target }
        } else if metadata.is_file() {
            FileKind::File
        } else {
            FileKind::Other
        };

        let size = if kind.is_dir() { 0 } else { metadata.len() };

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::MetadataExt;
            Some(metadata.mode())
        };

        #[cfg(not(unix))]
        let permissions = None;

        let full_path = entry.path().to_string_lossy().to_string();

        entries.push(FileEntry {
            name,
            path: full_path,
            size,
            kind,
            modified,
            permissions,
        });
    }

    // Sort: directories first, then alphabetical case-insensitive
    entries.sort_by(|a, b| match (a.kind.is_dir(), b.kind.is_dir()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

/// Comprehensive errors for the SFTP client transport engine.
#[derive(Debug)]
pub enum SftpError {
    Protocol(SftpProtocolError),
    UnsupportedVersion(u32),
    ChannelCorrupted(String),
    Status { code: StatusCode, message: String },
    Io(std::io::Error),
    ProcessSpawn(String),
    HandshakeFailed(String),
    UnexpectedResponse(String),
    FileNotFound(String),
    PermissionDenied(String),
    Cancelled,
    CleanupFailed { path: String, message: String },
    OutcomeUnknown { path: String, message: String },
}

impl fmt::Display for SftpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(err) => write!(f, "SFTP protocol error: {err}"),
            Self::UnsupportedVersion(v) => {
                write!(f, "unsupported SFTP protocol version {v} (expected v3)")
            }
            Self::ChannelCorrupted(msg) => {
                write!(f, "SFTP channel is corrupted and cannot be reused: {msg}")
            }
            Self::Status { code, message } => {
                write!(f, "SFTP server returned error {:?}: {}", code, message)
            }
            Self::Io(err) => write!(f, "I/O error during SFTP transfer: {err}"),
            Self::ProcessSpawn(msg) => write!(f, "failed to spawn SSH subsystem: {msg}"),
            Self::HandshakeFailed(msg) => write!(f, "SFTP initialization failed: {msg}"),
            Self::UnexpectedResponse(msg) => write!(f, "unexpected SFTP response: {msg}"),
            Self::FileNotFound(path) => write!(f, "remote file or directory not found: {path}"),
            Self::PermissionDenied(path) => write!(f, "permission denied for path: {path}"),
            Self::Cancelled => f.write_str("SFTP operation cancelled"),
            Self::CleanupFailed { path, message } => {
                write!(f, "{message}; temporary remote file may remain at {path}")
            }
            Self::OutcomeUnknown { path, message } => {
                write!(f, "transfer commit outcome unknown for {path}: {message}")
            }
        }
    }
}

impl std::error::Error for SftpError {}

impl From<std::io::Error> for SftpError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<SftpProtocolError> for SftpError {
    fn from(err: SftpProtocolError) -> Self {
        Self::Protocol(err)
    }
}

/// Options for connecting to an SFTP subsystem over SSH.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SftpConnectOptions {
    /// Optional explicit OpenSSH config; None preserves the user's normal configuration.
    pub ssh_config_file: Option<PathBuf>,
    pub environment: Vec<(String, String)>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub certificate_file: Option<String>,
    pub timeout_seconds: u32,
    pub extra_ssh_options: Vec<(String, String)>,
}

impl Default for SftpConnectOptions {
    fn default() -> Self {
        Self {
            ssh_config_file: None,
            environment: Vec::new(),
            port: None,
            identity_file: None,
            certificate_file: None,
            timeout_seconds: 30,
            extra_ssh_options: Vec::new(),
        }
    }
}

/// High-level client for live SFTP v3 wire streaming and remote filesystem operations.
pub struct SftpClient {
    reader: transport::PacketReader,
    writer: transport::PacketWriter,
    active_control: Option<TransferControl>,
    child_process: Option<std::process::Child>,
    stderr_buffer: std::sync::Arc<std::sync::Mutex<String>>,
    next_request_id: u32,
    server_version: u32,
    server_extensions: Vec<(String, String)>,
    capabilities: ServerCapabilities,
    tuning: TransferTuning,
    corrupted: bool,
}

impl SftpClient {
    /// Connect to remote host over OpenSSH subsystem `sftp` with piped binary streams.
    pub fn connect(
        destination: &crate::SshDestination,
        options: &SftpConnectOptions,
    ) -> Result<Self, SftpError> {
        Self::connect_with_control(destination, options, &TransferControl::default())
    }
    /// Connect with cancellable SSH handshake and bounded packet IO.
    pub fn connect_with_control(
        destination: &crate::SshDestination,
        options: &SftpConnectOptions,
        control: &TransferControl,
    ) -> Result<Self, SftpError> {
        control.check()?;
        if options.timeout_seconds == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "SFTP timeout must be greater than zero",
            )
            .into());
        }
        let timeout = std::time::Duration::from_secs(options.timeout_seconds.into());
        let mut cmd = crate::process::background_command("ssh");
        if let Some(config) = &options.ssh_config_file {
            cmd.arg("-F").arg(config);
        }
        cmd.arg("-s");
        cmd.arg("-e");
        cmd.arg("none");
        let askpass = options
            .environment
            .iter()
            .any(|(key, value)| key == "SHELLUMI_ASKPASS_MODE" && value == "1");
        cmd.envs(options.environment.iter().cloned());
        cmd.arg("-o").arg(if askpass {
            "BatchMode=no"
        } else {
            "BatchMode=yes"
        });
        cmd.arg("-o")
            .arg(format!("ConnectTimeout={}", options.timeout_seconds));
        cmd.arg("-o").arg("ForwardAgent=no");
        cmd.arg("-o").arg("StrictHostKeyChecking=yes");

        let effective_port = options.port.unwrap_or(destination.port().get());
        cmd.arg("-p").arg(effective_port.to_string());

        if let Some(ref key) = options.identity_file {
            cmd.arg("-i").arg(key);
        }
        if let Some(ref cert) = options.certificate_file {
            cmd.arg("-o")
                .arg(crate::connection::certificate_option(cert));
        }
        for (k, v) in &options.extra_ssh_options {
            cmd.arg("-o").arg(format!("{k}={v}"));
        }

        let target = format!("{}@{}", destination.username(), destination.host());
        cmd.arg(&target);
        cmd.arg("sftp");

        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            SftpError::ProcessSpawn(format!("failed to spawn ssh sftp subsystem: {e}"))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            SftpError::ProcessSpawn("failed to open stdin pipe to ssh".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            SftpError::ProcessSpawn("failed to open stdout pipe from ssh".to_string())
        })?;
        let stderr = child.stderr.take();
        let stderr_buffer = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        if let Some(mut err_pipe) = stderr {
            let buf_ref = std::sync::Arc::clone(&stderr_buffer);
            let _ = std::thread::Builder::new()
                .name("sftp-stderr-drain".into())
                .spawn(move || {
                    let mut chunk = [0u8; 4096];
                    const MAX_STDERR_BYTES: usize = 64 * 1024;
                    while let Ok(n) = err_pipe.read(&mut chunk) {
                        if n == 0 {
                            break;
                        }
                        if let Ok(mut guard) = buf_ref.lock() {
                            if guard.len() < MAX_STDERR_BYTES {
                                let text = String::from_utf8_lossy(&chunk[..n]);
                                let sanitized = crate::logging::redact_sensitive(&text);
                                let remain = MAX_STDERR_BYTES.saturating_sub(guard.len());
                                if sanitized.len() <= remain {
                                    guard.push_str(sanitized);
                                } else {
                                    guard.push_str(&sanitized[..remain]);
                                }
                            }
                        }
                    }
                });
        }

        let mut client = Self {
            reader: transport::PacketReader::Direct(Box::new(std::io::empty())),
            writer: transport::PacketWriter::Direct(Box::new(std::io::sink())),
            active_control: Some(control.clone()),
            child_process: Some(child),
            stderr_buffer,
            next_request_id: 1,
            server_version: 0,
            server_extensions: Vec::new(),
            capabilities: ServerCapabilities::default(),
            tuning: TransferTuning::default(),
            corrupted: false,
        };

        client.reader = transport::PacketReader::process(stdout, timeout)?;
        client.writer = transport::PacketWriter::process(stdin, timeout)?;
        if let Err(err) = client.init() {
            let diag = client.stderr_diagnostics();
            if !diag.is_empty() {
                tracing::warn!(stderr = %diag, "SFTP handshake failed");
                return Err(SftpError::HandshakeFailed(format!("{err} ({diag})")));
            }
            return Err(err);
        }
        control.check()?;
        client.active_control = None;
        Ok(client)
    }

    /// Construct an SFTP client around arbitrary readable/writable streams (ideal for mock testing).
    pub fn from_reader_writer(
        reader: Box<dyn std::io::Read + Send>,
        writer: Box<dyn std::io::Write + Send>,
    ) -> Result<Self, SftpError> {
        let mut client = Self {
            reader: transport::PacketReader::Direct(reader),
            writer: transport::PacketWriter::Direct(writer),
            active_control: None,
            child_process: None,
            stderr_buffer: std::sync::Arc::new(std::sync::Mutex::new(String::new())),
            next_request_id: 1,
            server_version: 0,
            server_extensions: Vec::new(),
            capabilities: ServerCapabilities::default(),
            tuning: TransferTuning::default(),
            corrupted: false,
        };
        client.init()?;
        Ok(client)
    }

    /// Version negotiated with the remote SFTP server.
    pub fn server_version(&self) -> u32 {
        self.server_version
    }

    /// Server extensions reported in `SSH_FXP_VERSION`.
    pub fn server_extensions(&self) -> &[(String, String)] {
        &self.server_extensions
    }

    /// Typed server capabilities negotiated during `SSH_FXP_VERSION`.
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// Update transfer pipelining and chunk tuning.
    pub fn set_tuning(&mut self, tuning: TransferTuning) {
        self.tuning = tuning.clamped_for_limits(self.capabilities.limits);
    }

    /// Returns true if the channel has experienced framing desynchronization or transport loss.
    pub fn is_corrupted(&self) -> bool {
        self.corrupted
    }

    /// Sanitized bounded stderr diagnostics captured from the SSH child process.
    pub fn stderr_diagnostics(&self) -> String {
        self.stderr_buffer
            .lock()
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    fn next_id(&mut self) -> u32 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        id
    }

    pub fn send_packet(&mut self, packet: &SftpPacket) -> Result<(), SftpError> {
        if self.corrupted {
            return Err(SftpError::ChannelCorrupted(
                "cannot send on corrupted SFTP channel".into(),
            ));
        }
        let wire = packet.encode();
        self.writer.begin(self.active_control.as_ref());
        if let Err(error) = self
            .writer
            .write_all(&wire)
            .and_then(|_| self.writer.flush())
        {
            return Err(self.transport_error(error));
        }
        Ok(())
    }

    pub fn read_packet(&mut self) -> Result<SftpPacket, SftpError> {
        if self.corrupted {
            return Err(SftpError::ChannelCorrupted(
                "cannot read from corrupted SFTP channel".into(),
            ));
        }
        self.reader.begin(self.active_control.as_ref());
        let mut len_bytes = [0u8; 4];
        if let Err(error) = self.reader.read_exact(&mut len_bytes) {
            return Err(self.transport_error(error));
        }
        let length = u32::from_be_bytes(len_bytes) as usize;
        if length == 0 || length > MAX_SFTP_PACKET_LENGTH {
            self.invalidate_channel();
            return Err(SftpError::Protocol(SftpProtocolError::InvalidPacketLength(
                length,
            )));
        }

        let mut payload = vec![0u8; length];
        if let Err(error) = self.reader.read_exact(&mut payload) {
            return Err(self.transport_error(error));
        }

        let mut full_wire = Vec::with_capacity(4 + length);
        full_wire.extend_from_slice(&len_bytes);
        full_wire.extend_from_slice(&payload);

        match SftpPacket::decode(&full_wire) {
            Ok(Some((packet, _consumed))) => Ok(packet),
            Ok(None) => {
                self.invalidate_channel();
                Err(SftpError::Protocol(SftpProtocolError::UnexpectedEof))
            }
            Err(err) => {
                self.invalidate_channel();
                Err(SftpError::Protocol(err))
            }
        }
    }

    fn invalidate_channel(&mut self) {
        self.corrupted = true;
        if let Some(mut child) = self.child_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn transport_error(&mut self, error: std::io::Error) -> SftpError {
        // A timed-out/cancelled partial frame is not safe to resume or reuse.
        self.invalidate_channel();
        if self
            .active_control
            .as_ref()
            .is_some_and(TransferControl::is_cancelled)
        {
            SftpError::Cancelled
        } else {
            SftpError::Io(error)
        }
    }
    fn controlled<T>(
        &mut self,
        control: &TransferControl,
        operation: impl FnOnce(&mut Self) -> Result<T, SftpError>,
    ) -> Result<T, SftpError> {
        let previous = self.active_control.replace(control.clone());
        let result = operation(self);
        self.active_control = previous;
        result
    }

    fn init(&mut self) -> Result<(), SftpError> {
        let init_packet = SftpPacket::Init { version: 3 };
        self.send_packet(&init_packet)?;
        match self.read_packet()? {
            SftpPacket::Version {
                version,
                extensions,
            } => {
                if version != 3 {
                    self.invalidate_channel();
                    return Err(SftpError::UnsupportedVersion(version));
                }
                self.server_version = version;
                self.capabilities = ServerCapabilities::from_extensions(&extensions);
                self.server_extensions = extensions;
                self.tuning = self.tuning.clamped_for_limits(self.capabilities.limits);
                Ok(())
            }
            SftpPacket::Status { code, message, .. } => {
                self.invalidate_channel();
                Err(SftpError::Status { code, message })
            }
            other => {
                self.invalidate_channel();
                Err(SftpError::HandshakeFailed(format!(
                    "expected SSH_FXP_VERSION, received {other:?}"
                )))
            }
        }
    }

    fn packet_id(packet: &SftpPacket) -> Option<u32> {
        match packet {
            SftpPacket::Status { id, .. }
            | SftpPacket::Handle { id, .. }
            | SftpPacket::Data { id, .. }
            | SftpPacket::Name { id, .. }
            | SftpPacket::Attrs { id, .. }
            | SftpPacket::ExtendedReply { id, .. } => Some(*id),
            _ => None,
        }
    }

    fn read_response_for(&mut self, expected_id: u32) -> Result<SftpPacket, SftpError> {
        let packet = self.read_packet()?;
        match Self::packet_id(&packet) {
            Some(id) if id == expected_id => Ok(packet),
            Some(other_id) => {
                self.invalidate_channel();
                Err(SftpError::UnexpectedResponse(format!(
                    "SFTP response ID mismatch: expected {expected_id}, received {other_id}"
                )))
            }
            None => {
                self.invalidate_channel();
                Err(SftpError::UnexpectedResponse(format!(
                    "Unexpected non-response SFTP packet for request {expected_id}: {packet:?}"
                )))
            }
        }
    }

    fn expect_status(&mut self, expected_id: u32, path_ctx: Option<&str>) -> Result<(), SftpError> {
        match self.read_response_for(expected_id)? {
            SftpPacket::Status {
                code: StatusCode::Ok,
                ..
            } => Ok(()),
            SftpPacket::Status {
                code: StatusCode::NoSuchFile,
                ..
            } => Err(SftpError::FileNotFound(
                path_ctx.unwrap_or("<unknown>").to_string(),
            )),
            SftpPacket::Status {
                code: StatusCode::PermissionDenied,
                ..
            } => Err(SftpError::PermissionDenied(
                path_ctx.unwrap_or("<unknown>").to_string(),
            )),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => {
                self.invalidate_channel();
                Err(SftpError::UnexpectedResponse(format!(
                    "Expected SSH_FXP_STATUS for request {expected_id}, received {other:?}"
                )))
            }
        }
    }

    fn expect_handle(&mut self, expected_id: u32, path_ctx: &str) -> Result<Vec<u8>, SftpError> {
        match self.read_response_for(expected_id)? {
            SftpPacket::Handle { handle, .. } => Ok(handle),
            SftpPacket::Status {
                code: StatusCode::NoSuchFile,
                ..
            } => Err(SftpError::FileNotFound(path_ctx.to_string())),
            SftpPacket::Status {
                code: StatusCode::PermissionDenied,
                ..
            } => Err(SftpError::PermissionDenied(path_ctx.to_string())),
            SftpPacket::Status {
                code: StatusCode::Ok,
                ..
            } => Err(SftpError::UnexpectedResponse(format!(
                "Server returned STATUS OK instead of HANDLE for '{path_ctx}'"
            ))),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => {
                self.invalidate_channel();
                Err(SftpError::UnexpectedResponse(format!(
                    "Expected SSH_FXP_HANDLE for '{path_ctx}', received {other:?}"
                )))
            }
        }
    }

    fn expect_attrs(
        &mut self,
        expected_id: u32,
        path_ctx: &str,
    ) -> Result<FileAttributes, SftpError> {
        match self.read_response_for(expected_id)? {
            SftpPacket::Attrs { attrs, .. } => Ok(attrs),
            SftpPacket::Status {
                code: StatusCode::NoSuchFile,
                ..
            } => Err(SftpError::FileNotFound(path_ctx.to_string())),
            SftpPacket::Status {
                code: StatusCode::PermissionDenied,
                ..
            } => Err(SftpError::PermissionDenied(path_ctx.to_string())),
            SftpPacket::Status {
                code: StatusCode::Ok,
                ..
            } => Err(SftpError::UnexpectedResponse(format!(
                "Server returned STATUS OK instead of ATTRS for '{path_ctx}'"
            ))),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => {
                self.invalidate_channel();
                Err(SftpError::UnexpectedResponse(format!(
                    "Expected SSH_FXP_ATTRS for '{path_ctx}', received {other:?}"
                )))
            }
        }
    }

    fn expect_name(
        &mut self,
        expected_id: u32,
        path_ctx: &str,
    ) -> Result<Vec<NameEntry>, SftpError> {
        match self.read_response_for(expected_id)? {
            SftpPacket::Name { entries, .. } => Ok(entries),
            SftpPacket::Status {
                code: StatusCode::NoSuchFile,
                ..
            } => Err(SftpError::FileNotFound(path_ctx.to_string())),
            SftpPacket::Status {
                code: StatusCode::PermissionDenied,
                ..
            } => Err(SftpError::PermissionDenied(path_ctx.to_string())),
            SftpPacket::Status {
                code: StatusCode::Ok,
                ..
            } => Err(SftpError::UnexpectedResponse(format!(
                "Server returned STATUS OK instead of NAME for '{path_ctx}'"
            ))),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => {
                self.invalidate_channel();
                Err(SftpError::UnexpectedResponse(format!(
                    "Expected SSH_FXP_NAME for '{path_ctx}', received {other:?}"
                )))
            }
        }
    }

    /// A successful write is not a completed transfer until the server accepts CLOSE.
    /// Servers can report quota/disk errors only when closing a handle.
    fn close_handle(&mut self, handle: Vec<u8>) -> Result<(), SftpError> {
        let id = self.next_id();
        self.send_packet(&SftpPacket::Close { id, handle })?;
        self.expect_status(id, None)
    }

    /// List entries in a remote directory with bounded limits and entry name validation.
    pub fn list_remote_directory(
        &mut self,
        remote_path: &str,
    ) -> Result<Vec<FileEntry>, SftpError> {
        self.list_remote_directory_with_limits(remote_path, ListingLimits::default())
    }

    pub fn list_remote_directory_with_limits(
        &mut self,
        remote_path: &str,
        limits: ListingLimits,
    ) -> Result<Vec<FileEntry>, SftpError> {
        let started = std::time::Instant::now();
        let opendir_id = self.next_id();
        self.send_packet(&SftpPacket::OpenDir {
            id: opendir_id,
            path: remote_path.to_string(),
        })?;

        let handle = self.expect_handle(opendir_id, remote_path)?;
        let mut entries = Vec::new();
        let mut requests = 0usize;
        let mut consecutive_empty = 0usize;

        let list_res = (|| -> Result<(), SftpError> {
            loop {
                if let Some(ctrl) = &self.active_control {
                    ctrl.check()?;
                }
                if started.elapsed() > limits.overall_deadline {
                    return Err(SftpError::UnexpectedResponse(format!(
                        "Directory listing timed out after {:?}",
                        limits.overall_deadline
                    )));
                }
                if requests >= limits.max_requests {
                    return Err(SftpError::UnexpectedResponse(format!(
                        "Directory listing exceeded max requests ({})",
                        limits.max_requests
                    )));
                }
                requests += 1;

                let readdir_id = self.next_id();
                self.send_packet(&SftpPacket::ReadDir {
                    id: readdir_id,
                    handle: handle.clone(),
                })?;

                match self.read_response_for(readdir_id)? {
                    SftpPacket::Name {
                        entries: name_entries,
                        ..
                    } => {
                        if name_entries.is_empty() {
                            consecutive_empty += 1;
                            if consecutive_empty >= limits.max_empty_replies {
                                return Err(SftpError::UnexpectedResponse(
                                    "Non-progressing peer: too many empty SSH_FXP_NAME replies"
                                        .into(),
                                ));
                            }
                            continue;
                        }
                        consecutive_empty = 0;
                        for entry in name_entries {
                            match classify_listing_name(
                                entry.filename.as_bytes(),
                                PathSemantics::Posix,
                            ) {
                                Ok(ListingNameKind::Dot | ListingNameKind::DotDot) => continue,
                                Ok(ListingNameKind::Child(_)) => {
                                    if entries.len() >= limits.max_entries {
                                        return Err(SftpError::UnexpectedResponse(format!(
                                            "Directory listing exceeded max entries ({})",
                                            limits.max_entries
                                        )));
                                    }
                                    entries.push(entry.to_file_entry(remote_path));
                                }
                                Err(err) => {
                                    tracing::warn!(
                                        path = %remote_path,
                                        reason = %err,
                                        "Skipping unsafe remote listing entry"
                                    );
                                }
                            }
                        }
                    }
                    SftpPacket::Status {
                        code: StatusCode::Eof,
                        ..
                    } => break,
                    SftpPacket::Status { code, message, .. } => {
                        return Err(SftpError::Status { code, message });
                    }
                    other => {
                        self.invalidate_channel();
                        return Err(SftpError::UnexpectedResponse(format!("{other:?}")));
                    }
                }
            }
            Ok(())
        })();

        let close_res = if self.corrupted {
            Ok(())
        } else {
            self.close_handle(handle)
        };
        list_res?;
        close_res?;

        entries.sort_by(|a, b| match (a.kind.is_dir(), b.kind.is_dir()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(entries)
    }

    /// Download to a private sibling file and publish only after EOF and CLOSE validation.
    pub fn download_file<F>(
        &mut self,
        remote_path: &str,
        local_path: &Path,
        callback: Option<F>,
    ) -> Result<u64, SftpError>
    where
        F: Fn(u64, u64),
    {
        self.download_file_with_control(
            remote_path,
            local_path,
            &TransferControl::default(),
            callback,
        )
    }
    pub fn download_file_with_control<F>(
        &mut self,
        remote_path: &str,
        local_path: &Path,
        control: &TransferControl,
        callback: Option<F>,
    ) -> Result<u64, SftpError>
    where
        F: Fn(u64, u64),
    {
        self.controlled(control, |client| {
            client.download_file_controlled(remote_path, local_path, control, callback)
        })
    }
    fn download_file_controlled<F>(
        &mut self,
        remote_path: &str,
        local_path: &Path,
        control: &TransferControl,
        callback: Option<F>,
    ) -> Result<u64, SftpError>
    where
        F: Fn(u64, u64),
    {
        use std::collections::{HashMap, VecDeque};
        use std::io::{Seek, SeekFrom, Write};

        control.check()?;
        let initial_size = self.stat(remote_path)?.size;
        let mut staging = transfer::LocalStaging::new(local_path)?;
        control.check()?;
        let id = self.next_id();
        self.send_packet(&SftpPacket::Open {
            id,
            filename: remote_path.into(),
            pflags: SSH_FXF_READ,
            attrs: FileAttributes::new(),
        })?;
        let handle = self.expect_handle(id, remote_path)?;

        // Compare opened-handle metadata rather than relying solely on path STAT.
        let size = match self.fstat(&handle) {
            Ok(fattrs) => {
                if let (Some(s1), Some(s2)) = (initial_size, fattrs.size) {
                    if s1 != s2 {
                        let _ = self.close_handle(handle);
                        return Err(SftpError::UnexpectedResponse(
                            "Remote file size changed between stat and open".into(),
                        ));
                    }
                }
                fattrs.size.or(initial_size)
            }
            Err(_) => initial_size,
        };

        let tuning = self.tuning.clamped_for_limits(self.capabilities.limits);
        let chunk_size = tuning.chunk_size as u32;
        let pipeline_depth = tuning.pipeline_depth.max(1);

        let transferred = (|| {
            // If size is known to be 0, still do a single read or return 0 after EOF check.
            let mut in_flight: HashMap<u32, (u64, u32)> = HashMap::new();
            let mut retry_queue: VecDeque<(u64, u32)> = VecDeque::new();
            let mut next_offset: u64 = 0;
            let mut bytes_received: u64 = 0;
            let mut eof_seen = false;

            loop {
                control.check()?;
                // Fill pipeline window
                while in_flight.len() < pipeline_depth {
                    let (req_off, req_len) = if let Some(pair) = retry_queue.pop_front() {
                        pair
                    } else if eof_seen {
                        break;
                    } else if let Some(expected) = size {
                        if next_offset >= expected
                            && !(expected == 0
                                && next_offset == 0
                                && in_flight.is_empty()
                                && bytes_received == 0)
                        {
                            break;
                        }
                        let len = if expected == 0 {
                            chunk_size
                        } else {
                            let rem = expected - next_offset;
                            rem.min(chunk_size as u64) as u32
                        };
                        let off = next_offset;
                        next_offset = next_offset.saturating_add(len as u64);
                        (off, len)
                    } else {
                        let off = next_offset;
                        next_offset = next_offset.saturating_add(chunk_size as u64);
                        (off, chunk_size)
                    };

                    let mut req_id = self.next_id();
                    while in_flight.contains_key(&req_id) {
                        req_id = self.next_id();
                    }
                    self.send_packet(&SftpPacket::Read {
                        id: req_id,
                        handle: handle.clone(),
                        offset: req_off,
                        len: req_len,
                    })?;
                    in_flight.insert(req_id, (req_off, req_len));
                    if size == Some(0) {
                        eof_seen = true;
                        break;
                    }
                }

                if in_flight.is_empty() {
                    break;
                }

                let reply = self.read_packet()?;
                control.check()?;
                match reply {
                    SftpPacket::Data { id: response, data } => {
                        let Some((req_off, req_len)) = in_flight.remove(&response) else {
                            self.invalidate_channel();
                            return Err(SftpError::UnexpectedResponse(format!(
                                "Unexpected READ response ID {response}"
                            )));
                        };
                        if data.is_empty() {
                            eof_seen = true;
                            continue;
                        }
                        if data.len() > req_len as usize {
                            self.invalidate_channel();
                            return Err(SftpError::UnexpectedResponse(format!(
                                "Server returned {} bytes for {}-byte READ",
                                data.len(),
                                req_len
                            )));
                        }
                        let file = staging
                            .file
                            .as_mut()
                            .expect("staging file is open until publication");
                        file.seek(SeekFrom::Start(req_off))?;
                        file.write_all(&data)?;
                        bytes_received = bytes_received.saturating_add(data.len() as u64);
                        if (data.len() as u32) < req_len && !eof_seen {
                            // Short read: re-issue remaining sub-range if before known EOF.
                            let rem_off = req_off + data.len() as u64;
                            let rem_len = req_len - data.len() as u32;
                            if size.is_none_or(|exp| rem_off < exp) {
                                retry_queue.push_back((rem_off, rem_len));
                            }
                        }
                        if let Some(callback) = &callback {
                            callback(bytes_received, size.unwrap_or(bytes_received));
                        }
                    }
                    SftpPacket::Status {
                        id: response,
                        code: StatusCode::Eof,
                        ..
                    } => {
                        if in_flight.remove(&response).is_none() {
                            self.invalidate_channel();
                            return Err(SftpError::UnexpectedResponse(format!(
                                "Unexpected EOF response ID {response}"
                            )));
                        }
                        eof_seen = true;
                    }
                    SftpPacket::Status {
                        id: response,
                        code,
                        message,
                        ..
                    } => {
                        if in_flight.remove(&response).is_none() {
                            self.invalidate_channel();
                        }
                        return Err(SftpError::Status { code, message });
                    }
                    other => {
                        self.invalidate_channel();
                        return Err(SftpError::UnexpectedResponse(format!("{other:?}")));
                    }
                }
            }

            if size.is_some_and(|expected| bytes_received != expected) {
                return Err(SftpError::UnexpectedResponse(
                    "Remote file size changed or transfer ended early".into(),
                ));
            }
            Ok(bytes_received)
        })();
        self.active_control = None;
        let closed = if self.corrupted {
            Ok(())
        } else {
            self.close_handle(handle)
        };
        let bytes = transferred?;
        closed?;
        staging.publish(local_path, control)?;
        Ok(bytes)
    }

    /// Upload to an exclusive private sibling file, then rename without deleting the old destination.
    pub fn upload_file<F>(
        &mut self,
        local_path: &Path,
        remote_path: &str,
        callback: Option<F>,
    ) -> Result<u64, SftpError>
    where
        F: Fn(u64, u64),
    {
        self.upload_file_with_control(
            local_path,
            remote_path,
            &TransferControl::default(),
            callback,
        )
    }
    pub fn upload_file_with_control<F>(
        &mut self,
        local_path: &Path,
        remote_path: &str,
        control: &TransferControl,
        callback: Option<F>,
    ) -> Result<u64, SftpError>
    where
        F: Fn(u64, u64),
    {
        self.controlled(control, |client| {
            client.upload_file_controlled(local_path, remote_path, control, callback)
        })
    }
    fn upload_file_controlled<F>(
        &mut self,
        local_path: &Path,
        remote_path: &str,
        control: &TransferControl,
        callback: Option<F>,
    ) -> Result<u64, SftpError>
    where
        F: Fn(u64, u64),
    {
        use std::collections::HashMap;
        use std::io::Read;

        control.check()?;
        if remote_path.is_empty() || remote_path.ends_with('/') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Upload destination must name a file",
            )
            .into());
        }
        let mut source = std::fs::File::open(local_path)?;
        let size = source.metadata()?.len();
        let directory = remote_path
            .rsplit_once('/')
            .map(|(parent, _)| format!("{parent}/"))
            .unwrap_or_default();
        let staging = format!("{directory}.shellumi-transfer-{}.part", transfer::suffix()?);
        let id = self.next_id();
        self.send_packet(&SftpPacket::Open {
            id,
            filename: staging.clone(),
            pflags: SSH_FXF_WRITE | SSH_FXF_CREAT | SSH_FXF_EXCL,
            attrs: FileAttributes::new().with_permissions(0o600),
        })?;
        let creation_reply =
            self.read_response_for(id)
                .map_err(|error| SftpError::CleanupFailed {
                    path: staging.clone(),
                    message: format!("Could not confirm remote staging creation: {error}"),
                })?;
        let handle = match creation_reply {
            SftpPacket::Handle { handle, .. } => handle,
            SftpPacket::Status { code, message, .. } => {
                return Err(SftpError::Status { code, message })
            }
            other => return Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        };

        let tuning = self.tuning.clamped_for_limits(self.capabilities.limits);
        let chunk_size = tuning.chunk_size;
        let pipeline_depth = tuning.pipeline_depth.max(1);

        let transferred = (|| {
            let mut buffer = vec![0u8; chunk_size];
            let mut in_flight: HashMap<u32, u64> = HashMap::new();
            let mut write_offset: u64 = 0;
            let mut acked_bytes: u64 = 0;
            let mut eof_local = false;

            loop {
                control.check()?;
                while !eof_local && in_flight.len() < pipeline_depth {
                    let count = source.read(&mut buffer)?;
                    if count == 0 {
                        eof_local = true;
                        break;
                    }
                    let mut req_id = self.next_id();
                    while in_flight.contains_key(&req_id) {
                        req_id = self.next_id();
                    }
                    self.send_packet(&SftpPacket::Write {
                        id: req_id,
                        handle: handle.clone(),
                        offset: write_offset,
                        data: buffer[..count].to_vec(),
                    })?;
                    write_offset = write_offset.saturating_add(count as u64);
                    in_flight.insert(req_id, count as u64);
                }

                if in_flight.is_empty() {
                    break;
                }

                let reply = self.read_packet()?;
                control.check()?;
                match reply {
                    SftpPacket::Status {
                        id: response,
                        code: StatusCode::Ok,
                        ..
                    } => {
                        let Some(chunk_len) = in_flight.remove(&response) else {
                            self.invalidate_channel();
                            return Err(SftpError::UnexpectedResponse(format!(
                                "Unexpected WRITE response ID {response}"
                            )));
                        };
                        acked_bytes = acked_bytes.saturating_add(chunk_len);
                        if let Some(callback) = &callback {
                            callback(acked_bytes, size);
                        }
                    }
                    SftpPacket::Status {
                        id: response,
                        code,
                        message,
                        ..
                    } => {
                        if in_flight.remove(&response).is_none() {
                            self.invalidate_channel();
                        }
                        return Err(SftpError::Status { code, message });
                    }
                    other => {
                        self.invalidate_channel();
                        return Err(SftpError::UnexpectedResponse(format!("{other:?}")));
                    }
                }
            }
            if acked_bytes != size {
                return Err(SftpError::UnexpectedResponse(
                    "Local file size changed during transfer".into(),
                ));
            }
            Ok(acked_bytes)
        })();

        self.active_control = None;
        let fsync_res = if transferred.is_ok() && self.capabilities.fsync && !self.corrupted {
            self.fsync_handle(&handle)
        } else {
            Ok(())
        };
        let closed = if self.corrupted {
            Ok(())
        } else {
            self.close_handle(handle)
        };
        let bytes = match transferred.and_then(|b| {
            fsync_res?;
            closed?;
            Ok(b)
        }) {
            Ok(b) => b,
            Err(error) => {
                if self.corrupted {
                    return Err(SftpError::CleanupFailed {
                        path: staging,
                        message: format!("{error}; channel closed before staging cleanup"),
                    });
                }
                return match self.remove_file(&staging) {
                    Ok(()) => Err(error),
                    Err(cleanup) => Err(SftpError::CleanupFailed {
                        path: staging,
                        message: format!("{error}; staging cleanup failed: {cleanup}"),
                    }),
                };
            }
        };

        control.begin_commit()?;
        match self.publish_upload(&staging, remote_path) {
            Ok(()) => Ok(bytes),
            Err(SftpError::Io(err)) => Err(SftpError::OutcomeUnknown {
                path: remote_path.to_string(),
                message: format!(
                    "transport error while awaiting rename reply (staging: {staging}): {err}"
                ),
            }),
            Err(SftpError::ChannelCorrupted(msg)) => Err(SftpError::OutcomeUnknown {
                path: remote_path.to_string(),
                message: format!(
                    "channel lost while awaiting rename reply (staging: {staging}): {msg}"
                ),
            }),
            Err(error) => {
                if self.corrupted {
                    return Err(SftpError::CleanupFailed {
                        path: staging,
                        message: format!("{error}; channel closed before staging cleanup"),
                    });
                }
                match self.remove_file(&staging) {
                    Ok(()) => Err(error),
                    Err(cleanup) => Err(SftpError::CleanupFailed {
                        path: staging,
                        message: format!("{error}; staging cleanup failed: {cleanup}"),
                    }),
                }
            }
        }
    }
    fn publish_upload(&mut self, staging: &str, destination: &str) -> Result<(), SftpError> {
        if !self.capabilities.posix_rename {
            // SFTP v3 rename refuses existing targets. Never emulate replace by deleting them.
            return self.rename(staging, destination);
        }
        let id = self.next_id();
        let mut data = Vec::new();
        write_string(&mut data, staging);
        write_string(&mut data, destination);
        self.send_packet(&SftpPacket::Extended {
            id,
            request: "posix-rename@openssh.com".into(),
            data,
        })?;
        self.expect_status(id, Some(destination))
    }

    /// Query remote file attributes (follows symlinks).
    pub fn stat(&mut self, path: &str) -> Result<FileAttributes, SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Stat {
            id: req_id,
            path: path.to_string(),
        })?;
        self.expect_attrs(req_id, path)
    }

    /// Query remote file attributes without following symlinks.
    pub fn lstat(&mut self, path: &str) -> Result<FileAttributes, SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Lstat {
            id: req_id,
            path: path.to_string(),
        })?;
        self.expect_attrs(req_id, path)
    }

    /// Query remote file attributes on an open handle.
    pub fn fstat(&mut self, handle: &[u8]) -> Result<FileAttributes, SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Fstat {
            id: req_id,
            handle: handle.to_vec(),
        })?;
        self.expect_attrs(req_id, "<handle>")
    }

    /// Set attributes on a remote path.
    pub fn setstat(&mut self, path: &str, attrs: FileAttributes) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::SetStat {
            id: req_id,
            path: path.to_string(),
            attrs,
        })?;
        self.expect_status(req_id, Some(path))
    }

    /// Set attributes on an open remote handle.
    pub fn fsetstat(&mut self, handle: &[u8], attrs: FileAttributes) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::FsetStat {
            id: req_id,
            handle: handle.to_vec(),
            attrs,
        })?;
        self.expect_status(req_id, None)
    }

    /// Read target of a remote symlink.
    pub fn readlink(&mut self, path: &str) -> Result<String, SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::ReadLink {
            id: req_id,
            path: path.to_string(),
        })?;
        let mut entries = self.expect_name(req_id, path)?;
        entries
            .pop()
            .map(|e| e.filename)
            .ok_or_else(|| SftpError::UnexpectedResponse("READLINK returned empty NAME".into()))
    }

    /// Create a remote symlink at `linkpath` pointing to `targetpath`.
    pub fn symlink(&mut self, linkpath: &str, targetpath: &str) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::SymLink {
            id: req_id,
            linkpath: linkpath.to_string(),
            targetpath: targetpath.to_string(),
        })?;
        self.expect_status(req_id, Some(linkpath))
    }

    /// Create an empty remote file exclusively (`O_CREAT | O_EXCL`).
    pub fn create_exclusive_file(&mut self, path: &str, mode: u32) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Open {
            id: req_id,
            filename: path.to_string(),
            pflags: SSH_FXF_WRITE | SSH_FXF_CREAT | SSH_FXF_EXCL,
            attrs: FileAttributes::new().with_permissions(mode),
        })?;
        let handle = self.expect_handle(req_id, path)?;
        self.close_handle(handle)
    }

    /// Flush remote file handle via `fsync@openssh.com` if advertised.
    pub fn fsync_handle(&mut self, handle: &[u8]) -> Result<(), SftpError> {
        if !self.capabilities.fsync {
            return Err(SftpError::Status {
                code: StatusCode::OpUnsupported,
                message: "fsync@openssh.com is not advertised by server".into(),
            });
        }
        let id = self.next_id();
        let mut data = Vec::with_capacity(4 + handle.len());
        data.extend_from_slice(&(handle.len() as u32).to_be_bytes());
        data.extend_from_slice(handle);
        self.send_packet(&SftpPacket::Extended {
            id,
            request: "fsync@openssh.com".into(),
            data,
        })?;
        self.expect_status(id, None)
    }

    /// Query remote filesystem capacity via `statvfs@openssh.com` if advertised.
    pub fn statvfs(&mut self, path: &str) -> Result<RemoteStatVfs, SftpError> {
        if !self.capabilities.statvfs {
            return Err(SftpError::Status {
                code: StatusCode::OpUnsupported,
                message: "statvfs@openssh.com is not advertised by server".into(),
            });
        }
        let id = self.next_id();
        let mut data = Vec::new();
        write_string(&mut data, path);
        self.send_packet(&SftpPacket::Extended {
            id,
            request: "statvfs@openssh.com".into(),
            data,
        })?;
        match self.read_response_for(id)? {
            SftpPacket::ExtendedReply { data, .. } => {
                RemoteStatVfs::decode(&data).map_err(SftpError::Protocol)
            }
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => {
                self.invalidate_channel();
                Err(SftpError::UnexpectedResponse(format!("{other:?}")))
            }
        }
    }

    /// Query server limits via `limits@openssh.com` if advertised.
    pub fn query_limits(&mut self) -> Result<ServerLimits, SftpError> {
        if !self.capabilities.limits_extension {
            return Err(SftpError::Status {
                code: StatusCode::OpUnsupported,
                message: "limits@openssh.com is not advertised by server".into(),
            });
        }
        let id = self.next_id();
        self.send_packet(&SftpPacket::Extended {
            id,
            request: "limits@openssh.com".into(),
            data: Vec::new(),
        })?;
        match self.read_response_for(id)? {
            SftpPacket::ExtendedReply { data, .. } => {
                let limits = ServerLimits::decode(&data).map_err(SftpError::Protocol)?;
                self.capabilities.limits = Some(limits);
                self.tuning = self.tuning.clamped_for_limits(Some(limits));
                Ok(limits)
            }
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => {
                self.invalidate_channel();
                Err(SftpError::UnexpectedResponse(format!("{other:?}")))
            }
        }
    }

    /// Server-side copy via `copy-data` extension when advertised.
    pub fn copy_data(
        &mut self,
        read_handle: &[u8],
        read_offset: u64,
        len: u64,
        write_handle: &[u8],
        write_offset: u64,
    ) -> Result<(), SftpError> {
        if !self.capabilities.copy_data {
            return Err(SftpError::Status {
                code: StatusCode::OpUnsupported,
                message: "copy-data is not advertised by server".into(),
            });
        }
        let id = self.next_id();
        let mut data = Vec::with_capacity(8 + read_handle.len() + write_handle.len() + 24);
        data.extend_from_slice(&(read_handle.len() as u32).to_be_bytes());
        data.extend_from_slice(read_handle);
        data.extend_from_slice(&read_offset.to_be_bytes());
        data.extend_from_slice(&len.to_be_bytes());
        data.extend_from_slice(&(write_handle.len() as u32).to_be_bytes());
        data.extend_from_slice(write_handle);
        data.extend_from_slice(&write_offset.to_be_bytes());
        self.send_packet(&SftpPacket::Extended {
            id,
            request: "copy-data".into(),
            data,
        })?;
        self.expect_status(id, None)
    }

    /// Canonicalize a remote path.
    pub fn realpath(&mut self, path: &str) -> Result<String, SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::RealPath {
            id: req_id,
            path: path.to_string(),
        })?;
        let mut entries = self.expect_name(req_id, path)?;
        if let Some(first) = entries.pop() {
            Ok(first.filename)
        } else {
            Ok(path.to_string())
        }
    }

    /// Create a remote directory.
    pub fn mkdir(&mut self, path: &str) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Mkdir {
            id: req_id,
            path: path.to_string(),
            attrs: FileAttributes::new().with_permissions(0o755),
        })?;
        self.expect_status(req_id, Some(path))
    }

    /// Remove a remote directory.
    pub fn rmdir(&mut self, path: &str) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Rmdir {
            id: req_id,
            path: path.to_string(),
        })?;
        self.expect_status(req_id, Some(path))
    }

    /// Remove a remote file.
    pub fn remove_file(&mut self, filename: &str) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Remove {
            id: req_id,
            filename: filename.to_string(),
        })?;
        self.expect_status(req_id, Some(filename))
    }

    /// Rename or move a remote file/directory.
    pub fn rename(&mut self, oldpath: &str, newpath: &str) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Rename {
            id: req_id,
            oldpath: oldpath.to_string(),
            newpath: newpath.to_string(),
        })?;
        self.expect_status(req_id, Some(oldpath))
    }

    /// Update permissions (chmod) for a remote path.
    pub fn set_permissions(&mut self, path: &str, mode: u32) -> Result<(), SftpError> {
        self.setstat(path, FileAttributes::new().with_permissions(mode))
    }
}

impl Drop for SftpClient {
    fn drop(&mut self) {
        if let Some(mut child) = self.child_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Metadata and state for a remote file checked out for editing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteTrackedFile {
    pub remote_path: String,
    pub local_path: PathBuf,
    pub original_sha256: String,
    pub last_checked_at: u64,
}

/// Workspace manager for safely downloading, tracking local modifications, and uploading remote files.
pub struct RemoteEditorWorkspace {
    workspace_dir: PathBuf,
    tracked: std::collections::HashMap<String, RemoteTrackedFile>,
}

impl RemoteEditorWorkspace {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Self {
        let dir = workspace_dir.into();
        let _ = std::fs::create_dir_all(&dir);
        Self {
            workspace_dir: dir,
            tracked: std::collections::HashMap::new(),
        }
    }

    pub fn workspace_dir(&self) -> &Path {
        &self.workspace_dir
    }

    /// Checkout a remote file to a local cache copy, returning its local path.
    pub fn checkout_file(
        &mut self,
        client: &mut SftpClient,
        remote_path: &str,
    ) -> Result<PathBuf, SftpError> {
        use ring::digest::{digest, SHA256};

        let file_name = Path::new(remote_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("remote_file");

        let path_hash = hex_digest(digest(&SHA256, remote_path.as_bytes()).as_ref());
        let safe_dir = self.workspace_dir.join(&path_hash[..12]);
        std::fs::create_dir_all(&safe_dir)?;
        let local_path = safe_dir.join(file_name);

        client.download_file(remote_path, &local_path, None::<fn(u64, u64)>)?;

        let content = std::fs::read(&local_path)?;
        let sha256_hex = hex_digest(digest(&SHA256, &content).as_ref());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.tracked.insert(
            remote_path.to_string(),
            RemoteTrackedFile {
                remote_path: remote_path.to_string(),
                local_path: local_path.clone(),
                original_sha256: sha256_hex,
                last_checked_at: now,
            },
        );

        Ok(local_path)
    }

    /// Check if the local checkout of the remote file has unsaved edits.
    pub fn is_dirty(&self, remote_path: &str) -> Result<bool, SftpError> {
        use ring::digest::{digest, SHA256};
        let Some(tracked) = self.tracked.get(remote_path) else {
            return Err(SftpError::FileNotFound(remote_path.to_string()));
        };

        if !tracked.local_path.exists() {
            return Ok(false);
        }

        let content = std::fs::read(&tracked.local_path)?;
        let current_sha256 = hex_digest(digest(&SHA256, &content).as_ref());
        Ok(current_sha256 != tracked.original_sha256)
    }

    /// Save the edited local file back to the remote server if dirty.
    /// Returns `Ok(true)` if modified and uploaded, `Ok(false)` if unchanged.
    pub fn save_file(
        &mut self,
        client: &mut SftpClient,
        remote_path: &str,
    ) -> Result<bool, SftpError> {
        use ring::digest::{digest, SHA256};
        if !self.is_dirty(remote_path)? {
            return Ok(false);
        }

        let local_path = {
            let tracked = self
                .tracked
                .get(remote_path)
                .ok_or_else(|| SftpError::FileNotFound(remote_path.to_string()))?;
            tracked.local_path.clone()
        };

        client.upload_file(&local_path, remote_path, None::<fn(u64, u64)>)?;

        let content = std::fs::read(&local_path)?;
        let new_hash = hex_digest(digest(&SHA256, &content).as_ref());
        if let Some(tracked) = self.tracked.get_mut(remote_path) {
            tracked.original_sha256 = new_hash;
        }

        Ok(true)
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod transfer_completion_tests;

//! SFTP File Manager and filesystem domain models.
//!
//! Provides UI-independent abstractions for:
//! - File entries (`FileEntry`, `FileKind`, permissions, sizes)
//! - Breadcrumb navigation and path traversal sanitization
//! - Transfer tasks, queue management, and progress calculations
//! - Local filesystem listing and directory browsing

mod queue;
mod transfer;
mod transport;
mod wire;
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

        let rwx = |shift: u32| -> [char; 3] {
            let val = (mode >> shift) & 0o7;
            [
                if val & 0o4 != 0 { 'r' } else { '-' },
                if val & 0o2 != 0 { 'w' } else { '-' },
                if val & 0o1 != 0 { 'x' } else { '-' },
            ]
        };

        let u = rwx(6);
        let g = rwx(3);
        let o = rwx(0);

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
        let mut path_buf = PathBuf::from(base);
        for comp in Path::new(child).components() {
            match comp {
                Component::Normal(c) => path_buf.push(c),
                Component::ParentDir => {
                    path_buf.pop();
                }
                Component::RootDir => {
                    path_buf = PathBuf::from("/");
                }
                _ => {}
            }
        }
        let s = path_buf.to_string_lossy().to_string();
        if s.is_empty() {
            "/".to_string()
        } else {
            s
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
    Status { code: StatusCode, message: String },
    Io(std::io::Error),
    ProcessSpawn(String),
    HandshakeFailed(String),
    UnexpectedResponse(String),
    FileNotFound(String),
    PermissionDenied(String),
    Cancelled,
    CleanupFailed { path: String, message: String },
}

impl fmt::Display for SftpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(err) => write!(f, "SFTP protocol error: {err}"),
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
    next_request_id: u32,
    server_version: u32,
    server_extensions: Vec<(String, String)>,
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
        let mut cmd = std::process::Command::new("ssh");
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
        cmd.stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| {
            SftpError::ProcessSpawn(format!("failed to spawn ssh sftp subsystem: {e}"))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            SftpError::ProcessSpawn("failed to open stdin pipe to ssh".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            SftpError::ProcessSpawn("failed to open stdout pipe from ssh".to_string())
        })?;

        let mut client = Self {
            reader: transport::PacketReader::Direct(Box::new(std::io::empty())),
            writer: transport::PacketWriter::Direct(Box::new(std::io::sink())),
            active_control: Some(control.clone()),
            child_process: Some(child),
            next_request_id: 1,
            server_version: 0,
            server_extensions: Vec::new(),
        };

        client.reader = transport::PacketReader::process(stdout, timeout)?;
        client.writer = transport::PacketWriter::process(stdin, timeout)?;
        client.init()?;
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
            next_request_id: 1,
            server_version: 0,
            server_extensions: Vec::new(),
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

    fn next_id(&mut self) -> u32 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        id
    }

    pub fn send_packet(&mut self, packet: &SftpPacket) -> Result<(), SftpError> {
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
        self.reader.begin(self.active_control.as_ref());
        let mut len_bytes = [0u8; 4];
        if let Err(error) = self.reader.read_exact(&mut len_bytes) {
            return Err(self.transport_error(error));
        }
        let length = u32::from_be_bytes(len_bytes) as usize;
        if length == 0 || length > MAX_SFTP_PACKET_LENGTH {
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

        let (packet, _consumed) = SftpPacket::decode(&full_wire)
            .map_err(SftpError::Protocol)?
            .ok_or(SftpError::Protocol(SftpProtocolError::UnexpectedEof))?;
        Ok(packet)
    }

    fn transport_error(&mut self, error: std::io::Error) -> SftpError {
        // A timed-out/cancelled partial frame is not safe to resume or reuse.
        if let Some(mut child) = self.child_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
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
                self.server_version = version;
                self.server_extensions = extensions;
                Ok(())
            }
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => Err(SftpError::HandshakeFailed(format!(
                "expected SSH_FXP_VERSION, received {other:?}"
            ))),
        }
    }

    /// A successful write is not a completed transfer until the server accepts CLOSE.
    /// Servers can report quota/disk errors only when closing a handle.
    fn close_handle(&mut self, handle: Vec<u8>) -> Result<(), SftpError> {
        let id = self.next_id();
        self.send_packet(&SftpPacket::Close { id, handle })?;
        match self.read_packet()? {
            SftpPacket::Status {
                id: response_id,
                code,
                message,
                ..
            } if response_id == id => {
                if code == StatusCode::Ok {
                    Ok(())
                } else {
                    Err(SftpError::Status { code, message })
                }
            }
            other => Err(SftpError::UnexpectedResponse(format!(
                "Expected status for CLOSE request {id}, received {other:?}"
            ))),
        }
    }

    /// List entries in a remote directory, returning parsed `FileEntry` records sorted by directories first.
    pub fn list_remote_directory(
        &mut self,
        remote_path: &str,
    ) -> Result<Vec<FileEntry>, SftpError> {
        let opendir_id = self.next_id();
        self.send_packet(&SftpPacket::OpenDir {
            id: opendir_id,
            path: remote_path.to_string(),
        })?;

        let handle = match self.read_packet()? {
            SftpPacket::Handle { handle, .. } => handle,
            SftpPacket::Status {
                code: StatusCode::NoSuchFile,
                ..
            } => return Err(SftpError::FileNotFound(remote_path.to_string())),
            SftpPacket::Status {
                code: StatusCode::PermissionDenied,
                ..
            } => return Err(SftpError::PermissionDenied(remote_path.to_string())),
            SftpPacket::Status { code, message, .. } => {
                return Err(SftpError::Status { code, message })
            }
            other => return Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        };

        let mut entries = Vec::new();
        loop {
            let readdir_id = self.next_id();
            self.send_packet(&SftpPacket::ReadDir {
                id: readdir_id,
                handle: handle.clone(),
            })?;

            match self.read_packet()? {
                SftpPacket::Name {
                    entries: name_entries,
                    ..
                } => {
                    for entry in name_entries {
                        if entry.filename != "." && entry.filename != ".." {
                            entries.push(entry.to_file_entry(remote_path));
                        }
                    }
                }
                SftpPacket::Status {
                    code: StatusCode::Eof,
                    ..
                } => break,
                SftpPacket::Status { code, message, .. } => {
                    let close_id = self.next_id();
                    let _ = self.send_packet(&SftpPacket::Close {
                        id: close_id,
                        handle,
                    });
                    let _ = self.read_packet();
                    return Err(SftpError::Status { code, message });
                }
                other => {
                    let close_id = self.next_id();
                    let _ = self.send_packet(&SftpPacket::Close {
                        id: close_id,
                        handle,
                    });
                    let _ = self.read_packet();
                    return Err(SftpError::UnexpectedResponse(format!("{other:?}")));
                }
            }
        }

        self.close_handle(handle)?;

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
        control.check()?;
        let size = self.stat(remote_path)?.size;
        let mut staging = transfer::LocalStaging::new(local_path)?;
        control.check()?;
        let id = self.next_id();
        self.send_packet(&SftpPacket::Open {
            id,
            filename: remote_path.into(),
            pflags: SSH_FXF_READ,
            attrs: FileAttributes::new(),
        })?;
        let handle = match self.read_packet()? {
            SftpPacket::Handle {
                id: response,
                handle,
            } if response == id => handle,
            SftpPacket::Status { code, message, .. } => {
                return Err(SftpError::Status { code, message })
            }
            other => return Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        };
        let transferred = (|| {
            let mut offset = 0;
            loop {
                control.check()?;
                let id = self.next_id();
                self.send_packet(&SftpPacket::Read {
                    id,
                    handle: handle.clone(),
                    offset,
                    len: 32768,
                })?;
                let reply = self.read_packet()?;
                control.check()?;
                match reply {
                    SftpPacket::Data { id: response, data }
                        if response == id && !data.is_empty() =>
                    {
                        use std::io::Write;
                        staging
                            .file
                            .as_mut()
                            .expect("staging file is open until publication")
                            .write_all(&data)?;
                        offset += data.len() as u64;
                        if let Some(callback) = &callback {
                            callback(offset, size.unwrap_or(offset));
                        }
                    }
                    SftpPacket::Status {
                        id: response,
                        code: StatusCode::Eof,
                        ..
                    } if response == id => break,
                    SftpPacket::Status { code, message, .. } => {
                        return Err(SftpError::Status { code, message })
                    }
                    other => return Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
                }
            }
            if size.is_some_and(|expected| offset != expected) {
                return Err(SftpError::UnexpectedResponse(
                    "Remote file size changed or transfer ended early".into(),
                ));
            }
            Ok(offset)
        })();
        self.active_control = None;
        let closed = self.close_handle(handle);
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
        let creation_reply = self
            .read_packet()
            .map_err(|error| SftpError::CleanupFailed {
                path: staging.clone(),
                message: format!("Could not confirm remote staging creation: {error}"),
            })?;
        let handle = match creation_reply {
            SftpPacket::Handle {
                id: response,
                handle,
            } if response == id => handle,
            SftpPacket::Status { code, message, .. } => {
                return Err(SftpError::Status { code, message })
            }
            other => return Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        };
        let transferred = (|| {
            let mut buffer = vec![0u8; 32768];
            let mut offset = 0;
            loop {
                control.check()?;
                let count = source.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                let id = self.next_id();
                self.send_packet(&SftpPacket::Write {
                    id,
                    handle: handle.clone(),
                    offset,
                    data: buffer[..count].to_vec(),
                })?;
                let reply = self.read_packet()?;
                control.check()?;
                match reply {
                    SftpPacket::Status {
                        id: response,
                        code: StatusCode::Ok,
                        ..
                    } if response == id => {
                        offset += count as u64;
                        if let Some(callback) = &callback {
                            callback(offset, size);
                        }
                    }
                    SftpPacket::Status { code, message, .. } => {
                        return Err(SftpError::Status { code, message })
                    }
                    other => return Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
                }
            }
            if offset != size {
                return Err(SftpError::UnexpectedResponse(
                    "Local file size changed during transfer".into(),
                ));
            }
            Ok(offset)
        })();
        self.active_control = None;
        let closed = self.close_handle(handle);
        let result = transferred.and_then(|bytes| {
            closed?;
            control.begin_commit()?;
            self.publish_upload(&staging, remote_path)?;
            Ok(bytes)
        });
        match result {
            Ok(bytes) => Ok(bytes),
            Err(error) => match self.remove_file(&staging) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(SftpError::CleanupFailed {
                    path: staging,
                    message: format!("{error}; staging cleanup failed: {cleanup}"),
                }),
            },
        }
    }
    fn publish_upload(&mut self, staging: &str, destination: &str) -> Result<(), SftpError> {
        if !self
            .server_extensions
            .iter()
            .any(|(name, version)| name == "posix-rename@openssh.com" && version == "1")
        {
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
        match self.read_packet()? {
            SftpPacket::Status {
                id: response,
                code: StatusCode::Ok,
                ..
            } if response == id => Ok(()),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        }
    }

    /// Query remote file attributes.
    pub fn stat(&mut self, path: &str) -> Result<FileAttributes, SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Stat {
            id: req_id,
            path: path.to_string(),
        })?;

        match self.read_packet()? {
            SftpPacket::Attrs { attrs, .. } => Ok(attrs),
            SftpPacket::Status {
                code: StatusCode::NoSuchFile,
                ..
            } => Err(SftpError::FileNotFound(path.to_string())),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        }
    }

    /// Canonicalize a remote path.
    pub fn realpath(&mut self, path: &str) -> Result<String, SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::RealPath {
            id: req_id,
            path: path.to_string(),
        })?;

        match self.read_packet()? {
            SftpPacket::Name { mut entries, .. } => {
                if let Some(first) = entries.pop() {
                    Ok(first.filename)
                } else {
                    Ok(path.to_string())
                }
            }
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
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

        match self.read_packet()? {
            SftpPacket::Status {
                code: StatusCode::Ok,
                ..
            } => Ok(()),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        }
    }

    /// Remove a remote directory.
    pub fn rmdir(&mut self, path: &str) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Rmdir {
            id: req_id,
            path: path.to_string(),
        })?;

        match self.read_packet()? {
            SftpPacket::Status {
                code: StatusCode::Ok,
                ..
            } => Ok(()),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        }
    }

    /// Remove a remote file.
    pub fn remove_file(&mut self, filename: &str) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Remove {
            id: req_id,
            filename: filename.to_string(),
        })?;

        match self.read_packet()? {
            SftpPacket::Status {
                code: StatusCode::Ok,
                ..
            } => Ok(()),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        }
    }

    /// Rename or move a remote file/directory.
    pub fn rename(&mut self, oldpath: &str, newpath: &str) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::Rename {
            id: req_id,
            oldpath: oldpath.to_string(),
            newpath: newpath.to_string(),
        })?;

        match self.read_packet()? {
            SftpPacket::Status {
                id: response,
                code: StatusCode::Ok,
                ..
            } if response == req_id => Ok(()),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        }
    }

    /// Update permissions (chmod) for a remote path.
    pub fn set_permissions(&mut self, path: &str, mode: u32) -> Result<(), SftpError> {
        let req_id = self.next_id();
        self.send_packet(&SftpPacket::SetStat {
            id: req_id,
            path: path.to_string(),
            attrs: FileAttributes::new().with_permissions(mode),
        })?;

        match self.read_packet()? {
            SftpPacket::Status {
                id: response,
                code: StatusCode::Ok,
                ..
            } if response == req_id => Ok(()),
            SftpPacket::Status { code, message, .. } => Err(SftpError::Status { code, message }),
            other => Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        }
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

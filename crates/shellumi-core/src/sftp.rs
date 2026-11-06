//! SFTP File Manager and filesystem domain models.
//!
//! Provides UI-independent abstractions for:
//! - File entries (`FileEntry`, `FileKind`, permissions, sizes)
//! - Breadcrumb navigation and path traversal sanitization
//! - Transfer tasks, queue management, and progress calculations
//! - Local filesystem listing and directory browsing

use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

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

// ---------------------------------------------------------------------------
// SFTP v3 Wire Protocol Definitions & Packet Codecs (draft-ietf-secsh-filexfer-02)
// ---------------------------------------------------------------------------

pub const SSH_FXP_INIT: u8 = 1;
pub const SSH_FXP_VERSION: u8 = 2;
pub const SSH_FXP_OPEN: u8 = 3;
pub const SSH_FXP_CLOSE: u8 = 4;
pub const SSH_FXP_READ: u8 = 5;
pub const SSH_FXP_WRITE: u8 = 6;
pub const SSH_FXP_LSTAT: u8 = 7;
pub const SSH_FXP_FSTAT: u8 = 8;
pub const SSH_FXP_SETSTAT: u8 = 9;
pub const SSH_FXP_FSETSTAT: u8 = 10;
pub const SSH_FXP_OPENDIR: u8 = 11;
pub const SSH_FXP_READDIR: u8 = 12;
pub const SSH_FXP_REMOVE: u8 = 13;
pub const SSH_FXP_MKDIR: u8 = 14;
pub const SSH_FXP_RMDIR: u8 = 15;
pub const SSH_FXP_REALPATH: u8 = 16;
pub const SSH_FXP_STAT: u8 = 17;
pub const SSH_FXP_RENAME: u8 = 18;
pub const SSH_FXP_READLINK: u8 = 19;
pub const SSH_FXP_SYMLINK: u8 = 20;

pub const SSH_FXP_STATUS: u8 = 101;
pub const SSH_FXP_HANDLE: u8 = 102;
pub const SSH_FXP_DATA: u8 = 103;
pub const SSH_FXP_NAME: u8 = 104;
pub const SSH_FXP_ATTRS: u8 = 105;

pub const SSH_FXP_EXTENDED: u8 = 200;
pub const SSH_FXP_EXTENDED_REPLY: u8 = 201;

pub const SSH_FILEXFER_ATTR_SIZE: u32 = 0x0000_0001;
pub const SSH_FILEXFER_ATTR_UIDGID: u32 = 0x0000_0002;
pub const SSH_FILEXFER_ATTR_PERMISSIONS: u32 = 0x0000_0004;
pub const SSH_FILEXFER_ATTR_ACMODTIME: u32 = 0x0000_0008;
pub const SSH_FILEXFER_ATTR_EXTENDED: u32 = 0x8000_0000;

/// Open flags (pflags) according to SFTP v3 (draft-ietf-secsh-filexfer-02).
pub const SSH_FXF_READ: u32 = 0x0000_0001;
pub const SSH_FXF_WRITE: u32 = 0x0000_0002;
pub const SSH_FXF_APPEND: u32 = 0x0000_0004;
pub const SSH_FXF_CREAT: u32 = 0x0000_0008;
pub const SSH_FXF_TRUNC: u32 = 0x0000_0010;
pub const SSH_FXF_EXCL: u32 = 0x0000_0020;

/// Standard SFTP status codes according to RFC draft-ietf-secsh-filexfer-02.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusCode {
    Ok,
    Eof,
    NoSuchFile,
    PermissionDenied,
    Failure,
    BadMessage,
    NoConnection,
    ConnectionLost,
    OpUnsupported,
    Unknown(u32),
}

impl StatusCode {
    pub fn from_u32(val: u32) -> Self {
        match val {
            0 => Self::Ok,
            1 => Self::Eof,
            2 => Self::NoSuchFile,
            3 => Self::PermissionDenied,
            4 => Self::Failure,
            5 => Self::BadMessage,
            6 => Self::NoConnection,
            7 => Self::ConnectionLost,
            8 => Self::OpUnsupported,
            other => Self::Unknown(other),
        }
    }

    pub fn to_u32(&self) -> u32 {
        match self {
            Self::Ok => 0,
            Self::Eof => 1,
            Self::NoSuchFile => 2,
            Self::PermissionDenied => 3,
            Self::Failure => 4,
            Self::BadMessage => 5,
            Self::NoConnection => 6,
            Self::ConnectionLost => 7,
            Self::OpUnsupported => 8,
            Self::Unknown(other) => *other,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn is_eof(&self) -> bool {
        matches!(self, Self::Eof)
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Ok => "Success",
            Self::Eof => "End of file",
            Self::NoSuchFile => "No such file or directory",
            Self::PermissionDenied => "Permission denied",
            Self::Failure => "General failure",
            Self::BadMessage => "Bad message",
            Self::NoConnection => "No connection",
            Self::ConnectionLost => "Connection lost",
            Self::OpUnsupported => "Operation unsupported",
            Self::Unknown(_) => "Unknown status code",
        }
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.to_u32(), self.description())
    }
}

/// Bitflags for opening files over SFTP (`SSH_FXF_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OpenFlags(pub u32);

impl OpenFlags {
    pub const READ: u32 = 0x0000_0001;
    pub const WRITE: u32 = 0x0000_0002;
    pub const APPEND: u32 = 0x0000_0004;
    pub const CREAT: u32 = 0x0000_0008;
    pub const TRUNC: u32 = 0x0000_0010;
    pub const EXCL: u32 = 0x0000_0020;

    pub fn read_only() -> Self {
        Self(Self::READ)
    }

    pub fn write_only() -> Self {
        Self(Self::WRITE)
    }

    pub fn read_write() -> Self {
        Self(Self::READ | Self::WRITE)
    }

    pub fn create_or_truncate() -> Self {
        Self(Self::WRITE | Self::CREAT | Self::TRUNC)
    }

    pub fn contains(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn with(&self, flag: u32) -> Self {
        Self(self.0 | flag)
    }
}

/// SFTP File Attributes wire representation.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FileAttributes {
    pub size: Option<u64>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub permissions: Option<u32>,
    pub atime: Option<u32>,
    pub mtime: Option<u32>,
    pub extended: Vec<(String, String)>,
}

impl FileAttributes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_permissions(mut self, permissions: u32) -> Self {
        self.permissions = Some(permissions);
        self
    }

    pub fn with_mtime(mut self, mtime: u32) -> Self {
        self.mtime = Some(mtime);
        self
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        let mut flags: u32 = 0;
        if self.size.is_some() {
            flags |= SSH_FILEXFER_ATTR_SIZE;
        }
        if self.uid.is_some() && self.gid.is_some() {
            flags |= SSH_FILEXFER_ATTR_UIDGID;
        }
        if self.permissions.is_some() {
            flags |= SSH_FILEXFER_ATTR_PERMISSIONS;
        }
        if self.atime.is_some() && self.mtime.is_some() {
            flags |= SSH_FILEXFER_ATTR_ACMODTIME;
        }
        if !self.extended.is_empty() {
            flags |= SSH_FILEXFER_ATTR_EXTENDED;
        }

        write_u32(buf, flags);
        if let Some(s) = self.size {
            write_u64(buf, s);
        }
        if let (Some(u), Some(g)) = (self.uid, self.gid) {
            write_u32(buf, u);
            write_u32(buf, g);
        }
        if let Some(p) = self.permissions {
            write_u32(buf, p);
        }
        if let (Some(a), Some(m)) = (self.atime, self.mtime) {
            write_u32(buf, a);
            write_u32(buf, m);
        }
        if !self.extended.is_empty() {
            write_u32(buf, self.extended.len() as u32);
            for (k, v) in &self.extended {
                write_string(buf, k);
                write_string(buf, v);
            }
        }
    }

    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, SftpProtocolError> {
        let flags = reader.read_u32()?;
        let size = if (flags & SSH_FILEXFER_ATTR_SIZE) != 0 {
            Some(reader.read_u64()?)
        } else {
            None
        };
        let (uid, gid) = if (flags & SSH_FILEXFER_ATTR_UIDGID) != 0 {
            let u = reader.read_u32()?;
            let g = reader.read_u32()?;
            (Some(u), Some(g))
        } else {
            (None, None)
        };
        let permissions = if (flags & SSH_FILEXFER_ATTR_PERMISSIONS) != 0 {
            Some(reader.read_u32()?)
        } else {
            None
        };
        let (atime, mtime) = if (flags & SSH_FILEXFER_ATTR_ACMODTIME) != 0 {
            let a = reader.read_u32()?;
            let m = reader.read_u32()?;
            (Some(a), Some(m))
        } else {
            (None, None)
        };
        let extended = if (flags & SSH_FILEXFER_ATTR_EXTENDED) != 0 {
            let count = reader.read_u32()?;
            let mut exts = Vec::with_capacity(count.min(128) as usize);
            for _ in 0..count {
                let k = reader.read_string()?;
                let v = reader.read_string()?;
                exts.push((k, v));
            }
            exts
        } else {
            Vec::new()
        };

        Ok(Self {
            size,
            uid,
            gid,
            permissions,
            atime,
            mtime,
            extended,
        })
    }
}

/// An entry in a directory listing response (`SSH_FXP_NAME`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameEntry {
    pub filename: String,
    pub longname: String,
    pub attrs: FileAttributes,
}

impl NameEntry {
    pub fn to_file_entry(&self, parent_dir: &str) -> FileEntry {
        let full_path = PathUtils::join_path(parent_dir, &self.filename);
        let kind = if let Some(mode) = self.attrs.permissions {
            let file_type = mode & 0o170000;
            if file_type == 0o040000 {
                FileKind::Directory
            } else if file_type == 0o120000 {
                FileKind::Symlink { target: None }
            } else if file_type == 0o100000 {
                FileKind::File
            } else {
                FileKind::Other
            }
        } else if self.longname.starts_with('d') {
            FileKind::Directory
        } else if self.longname.starts_with('l') {
            FileKind::Symlink { target: None }
        } else {
            FileKind::File
        };

        FileEntry {
            name: self.filename.clone(),
            path: full_path,
            size: self.attrs.size.unwrap_or(0),
            kind,
            modified: self.attrs.mtime.map(|m| m as u64),
            permissions: self.attrs.permissions,
        }
    }
}

/// SFTP protocol packet representations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SftpPacket {
    Init {
        version: u32,
    },
    Version {
        version: u32,
        extensions: Vec<(String, String)>,
    },
    Open {
        id: u32,
        filename: String,
        pflags: u32,
        attrs: FileAttributes,
    },
    Close {
        id: u32,
        handle: Vec<u8>,
    },
    Read {
        id: u32,
        handle: Vec<u8>,
        offset: u64,
        len: u32,
    },
    Write {
        id: u32,
        handle: Vec<u8>,
        offset: u64,
        data: Vec<u8>,
    },
    Lstat {
        id: u32,
        path: String,
    },
    Fstat {
        id: u32,
        handle: Vec<u8>,
    },
    SetStat {
        id: u32,
        path: String,
        attrs: FileAttributes,
    },
    FsetStat {
        id: u32,
        handle: Vec<u8>,
        attrs: FileAttributes,
    },
    OpenDir {
        id: u32,
        path: String,
    },
    ReadDir {
        id: u32,
        handle: Vec<u8>,
    },
    Remove {
        id: u32,
        filename: String,
    },
    Mkdir {
        id: u32,
        path: String,
        attrs: FileAttributes,
    },
    Rmdir {
        id: u32,
        path: String,
    },
    RealPath {
        id: u32,
        path: String,
    },
    Stat {
        id: u32,
        path: String,
    },
    Rename {
        id: u32,
        oldpath: String,
        newpath: String,
    },
    ReadLink {
        id: u32,
        path: String,
    },
    SymLink {
        id: u32,
        linkpath: String,
        targetpath: String,
    },
    Status {
        id: u32,
        code: StatusCode,
        message: String,
        tag: String,
    },
    Handle {
        id: u32,
        handle: Vec<u8>,
    },
    Data {
        id: u32,
        data: Vec<u8>,
    },
    Name {
        id: u32,
        entries: Vec<NameEntry>,
    },
    Attrs {
        id: u32,
        attrs: FileAttributes,
    },
    Extended {
        id: u32,
        request: String,
        data: Vec<u8>,
    },
    ExtendedReply {
        id: u32,
        data: Vec<u8>,
    },
}

impl SftpPacket {
    /// Returns the wire packet type code.
    pub fn packet_type(&self) -> u8 {
        match self {
            Self::Init { .. } => SSH_FXP_INIT,
            Self::Version { .. } => SSH_FXP_VERSION,
            Self::Open { .. } => SSH_FXP_OPEN,
            Self::Close { .. } => SSH_FXP_CLOSE,
            Self::Read { .. } => SSH_FXP_READ,
            Self::Write { .. } => SSH_FXP_WRITE,
            Self::Lstat { .. } => SSH_FXP_LSTAT,
            Self::Fstat { .. } => SSH_FXP_FSTAT,
            Self::SetStat { .. } => SSH_FXP_SETSTAT,
            Self::FsetStat { .. } => SSH_FXP_FSETSTAT,
            Self::OpenDir { .. } => SSH_FXP_OPENDIR,
            Self::ReadDir { .. } => SSH_FXP_READDIR,
            Self::Remove { .. } => SSH_FXP_REMOVE,
            Self::Mkdir { .. } => SSH_FXP_MKDIR,
            Self::Rmdir { .. } => SSH_FXP_RMDIR,
            Self::RealPath { .. } => SSH_FXP_REALPATH,
            Self::Stat { .. } => SSH_FXP_STAT,
            Self::Rename { .. } => SSH_FXP_RENAME,
            Self::ReadLink { .. } => SSH_FXP_READLINK,
            Self::SymLink { .. } => SSH_FXP_SYMLINK,
            Self::Status { .. } => SSH_FXP_STATUS,
            Self::Handle { .. } => SSH_FXP_HANDLE,
            Self::Data { .. } => SSH_FXP_DATA,
            Self::Name { .. } => SSH_FXP_NAME,
            Self::Attrs { .. } => SSH_FXP_ATTRS,
            Self::Extended { .. } => SSH_FXP_EXTENDED,
            Self::ExtendedReply { .. } => SSH_FXP_EXTENDED_REPLY,
        }
    }

    /// Returns the request/response ID if present.
    pub fn request_id(&self) -> Option<u32> {
        match self {
            Self::Init { .. } | Self::Version { .. } => None,
            Self::Open { id, .. }
            | Self::Close { id, .. }
            | Self::Read { id, .. }
            | Self::Write { id, .. }
            | Self::Lstat { id, .. }
            | Self::Fstat { id, .. }
            | Self::SetStat { id, .. }
            | Self::FsetStat { id, .. }
            | Self::OpenDir { id, .. }
            | Self::ReadDir { id, .. }
            | Self::Remove { id, .. }
            | Self::Mkdir { id, .. }
            | Self::Rmdir { id, .. }
            | Self::RealPath { id, .. }
            | Self::Stat { id, .. }
            | Self::Rename { id, .. }
            | Self::ReadLink { id, .. }
            | Self::SymLink { id, .. }
            | Self::Status { id, .. }
            | Self::Handle { id, .. }
            | Self::Data { id, .. }
            | Self::Name { id, .. }
            | Self::Attrs { id, .. }
            | Self::Extended { id, .. }
            | Self::ExtendedReply { id, .. } => Some(*id),
        }
    }

    /// Serializes this packet into RFC-compliant binary wire format:
    /// `[uint32 packet_length][uint8 type][payload...]`
    pub fn encode(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        match self {
            Self::Init { version } => {
                write_u32(&mut payload, *version);
            }
            Self::Version {
                version,
                extensions,
            } => {
                write_u32(&mut payload, *version);
                for (name, data) in extensions {
                    write_string(&mut payload, name);
                    write_string(&mut payload, data);
                }
            }
            Self::Open {
                id,
                filename,
                pflags,
                attrs,
            } => {
                write_u32(&mut payload, *id);
                write_string(&mut payload, filename);
                write_u32(&mut payload, *pflags);
                attrs.encode(&mut payload);
            }
            Self::Close { id, handle } => {
                write_u32(&mut payload, *id);
                write_bytes(&mut payload, handle);
            }
            Self::Read {
                id,
                handle,
                offset,
                len,
            } => {
                write_u32(&mut payload, *id);
                write_bytes(&mut payload, handle);
                write_u64(&mut payload, *offset);
                write_u32(&mut payload, *len);
            }
            Self::Write {
                id,
                handle,
                offset,
                data,
            } => {
                write_u32(&mut payload, *id);
                write_bytes(&mut payload, handle);
                write_u64(&mut payload, *offset);
                write_bytes(&mut payload, data);
            }
            Self::Lstat { id, path }
            | Self::OpenDir { id, path }
            | Self::Remove { id, filename: path }
            | Self::Rmdir { id, path }
            | Self::RealPath { id, path }
            | Self::Stat { id, path }
            | Self::ReadLink { id, path } => {
                write_u32(&mut payload, *id);
                write_string(&mut payload, path);
            }
            Self::Fstat { id, handle } | Self::ReadDir { id, handle } => {
                write_u32(&mut payload, *id);
                write_bytes(&mut payload, handle);
            }
            Self::SetStat { id, path, attrs } | Self::Mkdir { id, path, attrs } => {
                write_u32(&mut payload, *id);
                write_string(&mut payload, path);
                attrs.encode(&mut payload);
            }
            Self::FsetStat { id, handle, attrs } => {
                write_u32(&mut payload, *id);
                write_bytes(&mut payload, handle);
                attrs.encode(&mut payload);
            }
            Self::Rename {
                id,
                oldpath,
                newpath,
            } => {
                write_u32(&mut payload, *id);
                write_string(&mut payload, oldpath);
                write_string(&mut payload, newpath);
            }
            Self::SymLink {
                id,
                linkpath,
                targetpath,
            } => {
                write_u32(&mut payload, *id);
                write_string(&mut payload, linkpath);
                write_string(&mut payload, targetpath);
            }
            Self::Status {
                id,
                code,
                message,
                tag,
            } => {
                write_u32(&mut payload, *id);
                write_u32(&mut payload, code.to_u32());
                write_string(&mut payload, message);
                write_string(&mut payload, tag);
            }
            Self::Handle { id, handle } => {
                write_u32(&mut payload, *id);
                write_bytes(&mut payload, handle);
            }
            Self::Data { id, data } => {
                write_u32(&mut payload, *id);
                write_bytes(&mut payload, data);
            }
            Self::Name { id, entries } => {
                write_u32(&mut payload, *id);
                write_u32(&mut payload, entries.len() as u32);
                for entry in entries {
                    write_string(&mut payload, &entry.filename);
                    write_string(&mut payload, &entry.longname);
                    entry.attrs.encode(&mut payload);
                }
            }
            Self::Attrs { id, attrs } => {
                write_u32(&mut payload, *id);
                attrs.encode(&mut payload);
            }
            Self::Extended { id, request, data } => {
                write_u32(&mut payload, *id);
                write_string(&mut payload, request);
                payload.extend_from_slice(data);
            }
            Self::ExtendedReply { id, data } => {
                write_u32(&mut payload, *id);
                payload.extend_from_slice(data);
            }
        }

        let packet_length = 1 + payload.len();
        let mut packet = Vec::with_capacity(4 + packet_length);
        write_u32(&mut packet, packet_length as u32);
        packet.push(self.packet_type());
        packet.extend_from_slice(&payload);
        packet
    }

    /// Attempts to decode a single packet from a stream buffer.
    ///
    /// Returns:
    /// - `Ok(Some((packet, bytes_consumed)))` if a full packet was successfully decoded.
    /// - `Ok(None)` if the buffer does not yet contain a complete packet (needs more data).
    /// - `Err(SftpProtocolError)` if packet framing or data is malformed.
    pub fn decode(buf: &[u8]) -> Result<Option<(Self, usize)>, SftpProtocolError> {
        if buf.len() < 4 {
            return Ok(None);
        }

        let length = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        if length == 0 || length > 16 * 1024 * 1024 {
            return Err(SftpProtocolError::InvalidPacketLength(length));
        }

        let total_packet_bytes = 4 + length;
        if buf.len() < total_packet_bytes {
            return Ok(None);
        }

        let packet_type = buf[4];
        let payload = &buf[5..total_packet_bytes];
        let mut reader = WireReader::new(payload);

        let packet = match packet_type {
            SSH_FXP_INIT => {
                let version = reader.read_u32()?;
                Self::Init { version }
            }
            SSH_FXP_VERSION => {
                let version = reader.read_u32()?;
                let mut extensions = Vec::new();
                while reader.remaining() > 0 {
                    let name = reader.read_string()?;
                    let data = reader.read_string()?;
                    extensions.push((name, data));
                }
                Self::Version {
                    version,
                    extensions,
                }
            }
            SSH_FXP_OPEN => {
                let id = reader.read_u32()?;
                let filename = reader.read_string()?;
                let pflags = reader.read_u32()?;
                let attrs = FileAttributes::decode(&mut reader)?;
                Self::Open {
                    id,
                    filename,
                    pflags,
                    attrs,
                }
            }
            SSH_FXP_CLOSE => {
                let id = reader.read_u32()?;
                let handle = reader.read_bytes()?.to_vec();
                Self::Close { id, handle }
            }
            SSH_FXP_READ => {
                let id = reader.read_u32()?;
                let handle = reader.read_bytes()?.to_vec();
                let offset = reader.read_u64()?;
                let len = reader.read_u32()?;
                Self::Read {
                    id,
                    handle,
                    offset,
                    len,
                }
            }
            SSH_FXP_WRITE => {
                let id = reader.read_u32()?;
                let handle = reader.read_bytes()?.to_vec();
                let offset = reader.read_u64()?;
                let data = reader.read_bytes()?.to_vec();
                Self::Write {
                    id,
                    handle,
                    offset,
                    data,
                }
            }
            SSH_FXP_LSTAT => {
                let id = reader.read_u32()?;
                let path = reader.read_string()?;
                Self::Lstat { id, path }
            }
            SSH_FXP_FSTAT => {
                let id = reader.read_u32()?;
                let handle = reader.read_bytes()?.to_vec();
                Self::Fstat { id, handle }
            }
            SSH_FXP_SETSTAT => {
                let id = reader.read_u32()?;
                let path = reader.read_string()?;
                let attrs = FileAttributes::decode(&mut reader)?;
                Self::SetStat { id, path, attrs }
            }
            SSH_FXP_FSETSTAT => {
                let id = reader.read_u32()?;
                let handle = reader.read_bytes()?.to_vec();
                let attrs = FileAttributes::decode(&mut reader)?;
                Self::FsetStat { id, handle, attrs }
            }
            SSH_FXP_OPENDIR => {
                let id = reader.read_u32()?;
                let path = reader.read_string()?;
                Self::OpenDir { id, path }
            }
            SSH_FXP_READDIR => {
                let id = reader.read_u32()?;
                let handle = reader.read_bytes()?.to_vec();
                Self::ReadDir { id, handle }
            }
            SSH_FXP_REMOVE => {
                let id = reader.read_u32()?;
                let filename = reader.read_string()?;
                Self::Remove { id, filename }
            }
            SSH_FXP_MKDIR => {
                let id = reader.read_u32()?;
                let path = reader.read_string()?;
                let attrs = FileAttributes::decode(&mut reader)?;
                Self::Mkdir { id, path, attrs }
            }
            SSH_FXP_RMDIR => {
                let id = reader.read_u32()?;
                let path = reader.read_string()?;
                Self::Rmdir { id, path }
            }
            SSH_FXP_REALPATH => {
                let id = reader.read_u32()?;
                let path = reader.read_string()?;
                Self::RealPath { id, path }
            }
            SSH_FXP_STAT => {
                let id = reader.read_u32()?;
                let path = reader.read_string()?;
                Self::Stat { id, path }
            }
            SSH_FXP_RENAME => {
                let id = reader.read_u32()?;
                let oldpath = reader.read_string()?;
                let newpath = reader.read_string()?;
                Self::Rename {
                    id,
                    oldpath,
                    newpath,
                }
            }
            SSH_FXP_READLINK => {
                let id = reader.read_u32()?;
                let path = reader.read_string()?;
                Self::ReadLink { id, path }
            }
            SSH_FXP_SYMLINK => {
                let id = reader.read_u32()?;
                let linkpath = reader.read_string()?;
                let targetpath = reader.read_string()?;
                Self::SymLink {
                    id,
                    linkpath,
                    targetpath,
                }
            }
            SSH_FXP_STATUS => {
                let id = reader.read_u32()?;
                let code_u32 = reader.read_u32()?;
                let message = reader.read_string()?;
                let tag = reader.read_string()?;
                Self::Status {
                    id,
                    code: StatusCode::from_u32(code_u32),
                    message,
                    tag,
                }
            }
            SSH_FXP_HANDLE => {
                let id = reader.read_u32()?;
                let handle = reader.read_bytes()?.to_vec();
                Self::Handle { id, handle }
            }
            SSH_FXP_DATA => {
                let id = reader.read_u32()?;
                let data = reader.read_bytes()?.to_vec();
                Self::Data { id, data }
            }
            SSH_FXP_NAME => {
                let id = reader.read_u32()?;
                let count = reader.read_u32()?;
                let mut entries = Vec::with_capacity(count.min(4096) as usize);
                for _ in 0..count {
                    let filename = reader.read_string()?;
                    let longname = reader.read_string()?;
                    let attrs = FileAttributes::decode(&mut reader)?;
                    entries.push(NameEntry {
                        filename,
                        longname,
                        attrs,
                    });
                }
                Self::Name { id, entries }
            }
            SSH_FXP_ATTRS => {
                let id = reader.read_u32()?;
                let attrs = FileAttributes::decode(&mut reader)?;
                Self::Attrs { id, attrs }
            }
            SSH_FXP_EXTENDED => {
                let id = reader.read_u32()?;
                let request = reader.read_string()?;
                let data = reader.remaining_slice().to_vec();
                Self::Extended { id, request, data }
            }
            SSH_FXP_EXTENDED_REPLY => {
                let id = reader.read_u32()?;
                let data = reader.remaining_slice().to_vec();
                Self::ExtendedReply { id, data }
            }
            other => return Err(SftpProtocolError::UnknownPacketType(other)),
        };

        Ok(Some((packet, total_packet_bytes)))
    }
}

/// Errors encountered while encoding or decoding SFTP protocol packets.
#[derive(Debug, PartialEq, Eq)]
pub enum SftpProtocolError {
    UnexpectedEof,
    InvalidPacketLength(usize),
    UnknownPacketType(u8),
    InvalidUtf8,
    MalformedPayload(String),
}

impl fmt::Display for SftpProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected end of SFTP data buffer"),
            Self::InvalidPacketLength(len) => {
                write!(f, "invalid SFTP packet length header: {len} bytes")
            }
            Self::UnknownPacketType(t) => write!(f, "unknown SFTP packet type code: {t}"),
            Self::InvalidUtf8 => f.write_str("SFTP packet contains invalid UTF-8 string data"),
            Self::MalformedPayload(msg) => write!(f, "malformed SFTP packet payload: {msg}"),
        }
    }
}

impl std::error::Error for SftpProtocolError {}

/// Helper to decode big-endian wire structures safely with bounds checks.
pub struct WireReader<'a> {
    buf: &'a [u8],
    offset: usize,
}

impl<'a> WireReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, offset: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.offset)
    }

    pub fn remaining_slice(&self) -> &'a [u8] {
        if self.offset >= self.buf.len() {
            &[]
        } else {
            &self.buf[self.offset..]
        }
    }

    pub fn read_u8(&mut self) -> Result<u8, SftpProtocolError> {
        if self.offset >= self.buf.len() {
            return Err(SftpProtocolError::UnexpectedEof);
        }
        let b = self.buf[self.offset];
        self.offset += 1;
        Ok(b)
    }

    pub fn read_u32(&mut self) -> Result<u32, SftpProtocolError> {
        if self.remaining() < 4 {
            return Err(SftpProtocolError::UnexpectedEof);
        }
        let bytes = [
            self.buf[self.offset],
            self.buf[self.offset + 1],
            self.buf[self.offset + 2],
            self.buf[self.offset + 3],
        ];
        self.offset += 4;
        Ok(u32::from_be_bytes(bytes))
    }

    pub fn read_u64(&mut self) -> Result<u64, SftpProtocolError> {
        if self.remaining() < 8 {
            return Err(SftpProtocolError::UnexpectedEof);
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.buf[self.offset..self.offset + 8]);
        self.offset += 8;
        Ok(u64::from_be_bytes(bytes))
    }

    pub fn read_bytes(&mut self) -> Result<&'a [u8], SftpProtocolError> {
        let len = self.read_u32()? as usize;
        if self.remaining() < len {
            return Err(SftpProtocolError::UnexpectedEof);
        }
        let slice = &self.buf[self.offset..self.offset + len];
        self.offset += len;
        Ok(slice)
    }

    pub fn read_string(&mut self) -> Result<String, SftpProtocolError> {
        let bytes = self.read_bytes()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| SftpProtocolError::InvalidUtf8)
    }
}

fn write_u32(buf: &mut Vec<u8>, val: u32) {
    buf.extend_from_slice(&val.to_be_bytes());
}

fn write_u64(buf: &mut Vec<u8>, val: u64) {
    buf.extend_from_slice(&val.to_be_bytes());
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, s.len() as u32);
    buf.extend_from_slice(s.as_bytes());
}

fn write_bytes(buf: &mut Vec<u8>, b: &[u8]) {
    write_u32(buf, b.len() as u32);
    buf.extend_from_slice(b);
}

// ---------------------------------------------------------------------------
// Background Transfer Queue & Progress Aggregation
// ---------------------------------------------------------------------------

/// Manages multiple concurrent SFTP file transfers with concurrency limits.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferQueue {
    tasks: Vec<TransferTask>,
    concurrency_limit: usize,
    next_task_id: u64,
}

impl TransferQueue {
    pub fn new(concurrency_limit: usize) -> Self {
        Self {
            tasks: Vec::new(),
            concurrency_limit: concurrency_limit.max(1),
            next_task_id: 1,
        }
    }

    pub fn tasks(&self) -> &[TransferTask] {
        &self.tasks
    }

    pub fn tasks_mut(&mut self) -> &mut [TransferTask] {
        &mut self.tasks
    }

    pub fn enqueue(
        &mut self,
        direction: TransferDirection,
        local_path: impl Into<String>,
        remote_path: impl Into<String>,
        total_bytes: u64,
    ) -> u64 {
        let id = self.next_task_id;
        self.next_task_id += 1;
        self.tasks.push(TransferTask::new(
            id,
            direction,
            local_path,
            remote_path,
            total_bytes,
        ));
        id
    }

    /// Dispatches queued tasks up to the concurrency limit, transitioning them
    /// to `InProgress`. Returns the IDs of the newly started tasks.
    pub fn dispatch_next(&mut self) -> Vec<u64> {
        let active_count = self
            .tasks
            .iter()
            .filter(|t| matches!(t.status, TransferStatus::InProgress))
            .count();

        let available_slots = self.concurrency_limit.saturating_sub(active_count);
        let mut started = Vec::new();

        if available_slots > 0 {
            for task in &mut self.tasks {
                if started.len() >= available_slots {
                    break;
                }
                if matches!(task.status, TransferStatus::Queued) {
                    task.status = TransferStatus::InProgress;
                    started.push(task.id);
                }
            }
        }

        started
    }

    pub fn update_progress(&mut self, id: u64, bytes: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.bytes_transferred = bytes.min(task.total_bytes);
            if task.bytes_transferred == task.total_bytes && task.total_bytes > 0 {
                task.status = TransferStatus::Completed;
            }
        }
    }

    pub fn mark_completed(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.bytes_transferred = task.total_bytes;
            task.status = TransferStatus::Completed;
        }
    }

    pub fn mark_failed(&mut self, id: u64, reason: String) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.status = TransferStatus::Failed(reason);
        }
    }

    pub fn cancel(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.status = TransferStatus::Cancelled;
        }
    }

    pub fn retry(&mut self, id: u64) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            if matches!(
                task.status,
                TransferStatus::Failed(_) | TransferStatus::Cancelled
            ) {
                task.status = TransferStatus::Queued;
                task.bytes_transferred = 0;
                return true;
            }
        }
        false
    }

    pub fn clear_completed(&mut self) {
        self.tasks
            .retain(|t| !matches!(t.status, TransferStatus::Completed));
    }

    pub fn summary(&self) -> TransferQueueSummary {
        let mut pending = 0;
        let mut in_progress = 0;
        let mut completed = 0;
        let mut failed = 0;
        let mut cancelled = 0;
        let mut total_bytes = 0;
        let mut transferred_bytes = 0;

        for task in &self.tasks {
            total_bytes += task.total_bytes;
            transferred_bytes += task.bytes_transferred;
            match task.status {
                TransferStatus::Queued => pending += 1,
                TransferStatus::InProgress => in_progress += 1,
                TransferStatus::Completed => completed += 1,
                TransferStatus::Failed(_) => failed += 1,
                TransferStatus::Cancelled => cancelled += 1,
            }
        }

        let overall_percentage = if total_bytes == 0 {
            if completed > 0 && pending == 0 && in_progress == 0 {
                100.0
            } else {
                0.0
            }
        } else {
            ((transferred_bytes as f64 / total_bytes as f64) * 100.0) as f32
        };

        TransferQueueSummary {
            total_tasks: self.tasks.len(),
            pending_tasks: pending,
            in_progress_tasks: in_progress,
            completed_tasks: completed,
            failed_tasks: failed,
            cancelled_tasks: cancelled,
            total_bytes,
            transferred_bytes,
            overall_percentage,
        }
    }
}

impl Default for TransferQueue {
    fn default() -> Self {
        Self::new(2)
    }
}

/// Aggregated transfer status across all tasks in a transfer queue.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferQueueSummary {
    pub total_tasks: usize,
    pub pending_tasks: usize,
    pub in_progress_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub cancelled_tasks: usize,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub overall_percentage: f32,
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
    reader: Box<dyn std::io::Read + Send>,
    writer: Box<dyn std::io::Write + Send>,
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
            reader: Box::new(std::io::BufReader::new(stdout)),
            writer: Box::new(std::io::BufWriter::new(stdin)),
            child_process: Some(child),
            next_request_id: 1,
            server_version: 0,
            server_extensions: Vec::new(),
        };

        client.init()?;
        Ok(client)
    }

    /// Construct an SFTP client around arbitrary readable/writable streams (ideal for mock testing).
    pub fn from_reader_writer(
        reader: Box<dyn std::io::Read + Send>,
        writer: Box<dyn std::io::Write + Send>,
    ) -> Result<Self, SftpError> {
        let mut client = Self {
            reader,
            writer,
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
        self.writer.write_all(&wire)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn read_packet(&mut self) -> Result<SftpPacket, SftpError> {
        let mut len_bytes = [0u8; 4];
        self.reader.read_exact(&mut len_bytes)?;
        let length = u32::from_be_bytes(len_bytes) as usize;
        if length > 256 * 1024 * 1024 {
            return Err(SftpError::Protocol(SftpProtocolError::InvalidPacketLength(
                length,
            )));
        }

        let mut payload = vec![0u8; length];
        self.reader.read_exact(&mut payload)?;

        let mut full_wire = Vec::with_capacity(4 + length);
        full_wire.extend_from_slice(&len_bytes);
        full_wire.extend_from_slice(&payload);

        let (packet, _consumed) = SftpPacket::decode(&full_wire)
            .map_err(SftpError::Protocol)?
            .ok_or(SftpError::Protocol(SftpProtocolError::UnexpectedEof))?;
        Ok(packet)
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

    /// Download a remote file to a local destination, invoking an optional progress callback.
    pub fn download_file<F>(
        &mut self,
        remote_path: &str,
        local_path: &Path,
        progress_callback: Option<F>,
    ) -> Result<u64, SftpError>
    where
        F: Fn(u64, u64),
    {
        let total_size = self
            .stat(remote_path)
            .map(|a| a.size.unwrap_or(0))
            .unwrap_or(0);

        let open_id = self.next_id();
        self.send_packet(&SftpPacket::Open {
            id: open_id,
            filename: remote_path.to_string(),
            pflags: SSH_FXF_READ,
            attrs: FileAttributes::new(),
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

        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut local_file = std::fs::File::create(local_path)?;
        let chunk_size = 32768u32;
        let mut offset = 0u64;

        loop {
            let read_id = self.next_id();
            self.send_packet(&SftpPacket::Read {
                id: read_id,
                handle: handle.clone(),
                offset,
                len: chunk_size,
            })?;

            match self.read_packet()? {
                SftpPacket::Data { data, .. } => {
                    if data.is_empty() {
                        break;
                    }
                    use std::io::Write as _;
                    local_file.write_all(&data)?;
                    offset += data.len() as u64;
                    if let Some(ref cb) = progress_callback {
                        cb(offset, total_size.max(offset));
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

        Ok(offset)
    }

    /// Upload a local file to a remote destination path.
    pub fn upload_file<F>(
        &mut self,
        local_path: &Path,
        remote_path: &str,
        progress_callback: Option<F>,
    ) -> Result<u64, SftpError>
    where
        F: Fn(u64, u64),
    {
        use std::io::Read as _;
        let mut local_file = std::fs::File::open(local_path)?;
        let total_size = local_file.metadata().map(|m| m.len()).unwrap_or(0);

        let open_id = self.next_id();
        self.send_packet(&SftpPacket::Open {
            id: open_id,
            filename: remote_path.to_string(),
            pflags: SSH_FXF_WRITE | SSH_FXF_CREAT | SSH_FXF_TRUNC,
            attrs: FileAttributes::new(),
        })?;

        let handle = match self.read_packet()? {
            SftpPacket::Handle { handle, .. } => handle,
            SftpPacket::Status {
                code: StatusCode::PermissionDenied,
                ..
            } => return Err(SftpError::PermissionDenied(remote_path.to_string())),
            SftpPacket::Status { code, message, .. } => {
                return Err(SftpError::Status { code, message })
            }
            other => return Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
        };

        let chunk_size = 32768;
        let mut buf = vec![0u8; chunk_size];
        let mut offset = 0u64;

        loop {
            let bytes_read = local_file.read(&mut buf)?;
            if bytes_read == 0 {
                break;
            }

            let write_id = self.next_id();
            self.send_packet(&SftpPacket::Write {
                id: write_id,
                handle: handle.clone(),
                offset,
                data: buf[..bytes_read].to_vec(),
            })?;

            match self.read_packet()? {
                SftpPacket::Status {
                    code: StatusCode::Ok,
                    ..
                } => {
                    offset += bytes_read as u64;
                    if let Some(ref cb) = progress_callback {
                        cb(offset, total_size);
                    }
                }
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

        Ok(offset)
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
                code: StatusCode::Ok,
                ..
            } => Ok(()),
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
mod tests {
    use super::*;

    #[test]
    fn formats_byte_sizes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
        assert_eq!(format_bytes(1099511627776), "1.0 TB");
    }

    #[test]
    fn formats_permissions_and_identifies_hidden() {
        let dir_entry = FileEntry {
            name: "photos".into(),
            path: "/home/user/photos".into(),
            size: 4096,
            kind: FileKind::Directory,
            modified: Some(1700000000),
            permissions: Some(0o755),
        };
        assert_eq!(dir_entry.formatted_permissions(), "drwxr-xr-x");
        assert_eq!(dir_entry.formatted_size(), "-");
        assert!(!dir_entry.is_hidden());

        let hidden_file = FileEntry {
            name: ".bashrc".into(),
            path: "/home/user/.bashrc".into(),
            size: 250,
            kind: FileKind::File,
            modified: None,
            permissions: Some(0o644),
        };
        assert_eq!(hidden_file.formatted_permissions(), "-rw-r--r--");
        assert_eq!(hidden_file.formatted_size(), "250 B");
        assert!(hidden_file.is_hidden());
    }

    #[test]
    fn breadcrumbs_generation() {
        let crumbs = PathUtils::breadcrumbs("/var/log/nginx");
        assert_eq!(
            crumbs,
            vec![
                ("/".to_string(), "/".to_string()),
                ("var".to_string(), "/var".to_string()),
                ("log".to_string(), "/var/log".to_string()),
                ("nginx".to_string(), "/var/log/nginx".to_string()),
            ]
        );

        let root_crumbs = PathUtils::breadcrumbs("/");
        assert_eq!(root_crumbs, vec![("/".to_string(), "/".to_string())]);
    }

    #[test]
    fn path_join_and_normalization() {
        assert_eq!(PathUtils::join_path("/var/log", "nginx"), "/var/log/nginx");
        assert_eq!(PathUtils::join_path("/var/log", ".."), "/var");
        assert_eq!(PathUtils::join_path("/", ".."), "/");
    }

    #[test]
    fn transfer_task_progress() {
        let mut task = TransferTask::new(
            1,
            TransferDirection::Upload,
            "/local/file",
            "/remote/file",
            1000,
        );
        assert_eq!(task.progress_percentage(), 0.0);

        task.bytes_transferred = 500;
        assert_eq!(task.progress_percentage(), 50.0);

        task.bytes_transferred = 1000;
        task.status = TransferStatus::Completed;
        assert_eq!(task.progress_percentage(), 100.0);
    }

    #[test]
    fn lists_local_temporary_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        // Create folders and files
        fs::create_dir(dir_path.join("b_folder")).unwrap();
        fs::create_dir(dir_path.join("a_folder")).unwrap();
        fs::write(dir_path.join("c_file.txt"), "hello").unwrap();
        fs::write(dir_path.join(".hidden"), "secret").unwrap();

        // Without hidden files
        let list = list_local_directory(dir_path.to_str().unwrap(), false).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].name, "a_folder");
        assert!(list[0].kind.is_dir());
        assert_eq!(list[1].name, "b_folder");
        assert!(list[1].kind.is_dir());
        assert_eq!(list[2].name, "c_file.txt");
        assert!(list[2].kind.is_file());

        // With hidden files
        let list_all = list_local_directory(dir_path.to_str().unwrap(), true).unwrap();
        assert_eq!(list_all.len(), 4);
        assert!(list_all.iter().any(|f| f.name == ".hidden"));
    }

    #[test]
    fn verifies_sftp_packet_init_and_version_encode_decode() {
        let init_packet = SftpPacket::Init { version: 3 };
        let encoded = init_packet.encode();
        assert_eq!(encoded, vec![0, 0, 0, 5, 1, 0, 0, 0, 3]);

        let decoded = SftpPacket::decode(&encoded).unwrap();
        assert_eq!(decoded, Some((init_packet, 9)));

        let version_packet = SftpPacket::Version {
            version: 3,
            extensions: vec![
                ("posix-rename@openssh.com".to_string(), "1".to_string()),
                ("statvfs@openssh.com".to_string(), "2".to_string()),
            ],
        };
        let encoded_ver = version_packet.encode();
        let decoded_ver = SftpPacket::decode(&encoded_ver).unwrap();
        assert_eq!(decoded_ver, Some((version_packet, encoded_ver.len())));
    }

    #[test]
    fn verifies_sftp_packet_open_close_read_write_roundtrip() {
        let open_packet = SftpPacket::Open {
            id: 42,
            filename: "/var/log/syslog".to_string(),
            pflags: OpenFlags::create_or_truncate().0,
            attrs: FileAttributes::new()
                .with_size(1024)
                .with_permissions(0o644),
        };
        let encoded_open = open_packet.encode();
        let decoded_open = SftpPacket::decode(&encoded_open).unwrap().unwrap();
        assert_eq!(decoded_open.0, open_packet);
        assert_eq!(decoded_open.0.request_id(), Some(42));
        assert_eq!(decoded_open.1, encoded_open.len());

        let close_packet = SftpPacket::Close {
            id: 43,
            handle: vec![0xaa, 0xbb, 0xcc, 0xdd],
        };
        let encoded_close = close_packet.encode();
        let decoded_close = SftpPacket::decode(&encoded_close).unwrap().unwrap();
        assert_eq!(decoded_close.0, close_packet);

        let read_packet = SftpPacket::Read {
            id: 44,
            handle: vec![1, 2, 3],
            offset: 4096,
            len: 16384,
        };
        let encoded_read = read_packet.encode();
        let decoded_read = SftpPacket::decode(&encoded_read).unwrap().unwrap();
        assert_eq!(decoded_read.0, read_packet);

        let write_packet = SftpPacket::Write {
            id: 45,
            handle: vec![1, 2, 3],
            offset: 8192,
            data: b"hello sftp streaming protocol".to_vec(),
        };
        let encoded_write = write_packet.encode();
        let decoded_write = SftpPacket::decode(&encoded_write).unwrap().unwrap();
        assert_eq!(decoded_write.0, write_packet);
    }

    #[test]
    fn verifies_sftp_packet_name_attrs_status_roundtrip() {
        let status_packet = SftpPacket::Status {
            id: 100,
            code: StatusCode::NoSuchFile,
            message: "File not found".to_string(),
            tag: "en-US".to_string(),
        };
        assert!(!status_packet.packet_type() != SSH_FXP_STATUS);
        assert_eq!(StatusCode::from_u32(2), StatusCode::NoSuchFile);
        assert_eq!(StatusCode::NoSuchFile.to_u32(), 2);
        assert_eq!(
            StatusCode::NoSuchFile.description(),
            "No such file or directory"
        );
        assert!(!StatusCode::NoSuchFile.is_success());
        assert!(!StatusCode::NoSuchFile.is_eof());
        assert!(StatusCode::Ok.is_success());
        assert!(StatusCode::Eof.is_eof());

        let encoded_status = status_packet.encode();
        let decoded_status = SftpPacket::decode(&encoded_status).unwrap().unwrap();
        assert_eq!(decoded_status.0, status_packet);

        let attrs_packet = SftpPacket::Attrs {
            id: 101,
            attrs: FileAttributes {
                size: Some(8192),
                uid: Some(1000),
                gid: Some(1000),
                permissions: Some(0o755),
                atime: Some(1700000000),
                mtime: Some(1700000500),
                extended: vec![("custom-meta".to_string(), "active".to_string())],
            },
        };
        let encoded_attrs = attrs_packet.encode();
        let decoded_attrs = SftpPacket::decode(&encoded_attrs).unwrap().unwrap();
        assert_eq!(decoded_attrs.0, attrs_packet);

        let name_packet = SftpPacket::Name {
            id: 102,
            entries: vec![
                NameEntry {
                    filename: "docs".to_string(),
                    longname: "drwxr-xr-x 2 user user 4096 Nov 04 12:00 docs".to_string(),
                    attrs: FileAttributes::new().with_permissions(0o040755),
                },
                NameEntry {
                    filename: "notes.txt".to_string(),
                    longname: "-rw-r--r-- 1 user user 512 Nov 04 12:00 notes.txt".to_string(),
                    attrs: FileAttributes::new()
                        .with_size(512)
                        .with_permissions(0o100644),
                },
            ],
        };
        let encoded_name = name_packet.encode();
        let decoded_name = SftpPacket::decode(&encoded_name).unwrap().unwrap();
        assert_eq!(decoded_name.0, name_packet);

        if let SftpPacket::Name { entries, .. } = decoded_name.0 {
            let f0 = entries[0].to_file_entry("/home/user");
            assert_eq!(f0.name, "docs");
            assert_eq!(f0.path, "/home/user/docs");
            assert!(f0.kind.is_dir());

            let f1 = entries[1].to_file_entry("/home/user");
            assert_eq!(f1.name, "notes.txt");
            assert_eq!(f1.path, "/home/user/notes.txt");
            assert!(f1.kind.is_file());
            assert_eq!(f1.size, 512);
        } else {
            panic!("expected SftpPacket::Name");
        }
    }

    #[test]
    fn verifies_sftp_stream_framing_partial_packets_and_errors() {
        let packet = SftpPacket::Lstat {
            id: 55,
            path: "/etc/hosts".to_string(),
        };
        let encoded = packet.encode();

        // 1 byte: partial header
        assert_eq!(SftpPacket::decode(&encoded[0..1]).unwrap(), None);

        // 4 bytes: header present but payload missing
        assert_eq!(SftpPacket::decode(&encoded[0..4]).unwrap(), None);

        // Almost all payload, 1 byte missing
        assert_eq!(
            SftpPacket::decode(&encoded[0..encoded.len() - 1]).unwrap(),
            None
        );

        // Full packet
        let decoded = SftpPacket::decode(&encoded).unwrap();
        assert_eq!(decoded, Some((packet, encoded.len())));

        // Buffer with extra trailing bytes (subsequent packet in stream)
        let mut stream_buf = encoded.clone();
        stream_buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x05]); // start of next packet
        let decoded_extra = SftpPacket::decode(&stream_buf).unwrap();
        assert_eq!(
            decoded_extra,
            Some((
                SftpPacket::Lstat {
                    id: 55,
                    path: "/etc/hosts".to_string(),
                },
                encoded.len()
            ))
        );

        // Error: 0 length header
        assert_eq!(
            SftpPacket::decode(&[0, 0, 0, 0, 1]),
            Err(SftpProtocolError::InvalidPacketLength(0))
        );

        // Error: Absurd length > 16MB header
        assert_eq!(
            SftpPacket::decode(&[0x02, 0x00, 0x00, 0x00]),
            Err(SftpProtocolError::InvalidPacketLength(0x02000000))
        );

        // Error: Unknown packet type
        assert_eq!(
            SftpPacket::decode(&[0, 0, 0, 1, 255]),
            Err(SftpProtocolError::UnknownPacketType(255))
        );

        // Error: Invalid UTF-8 in string
        let mut bad_utf8_buf = vec![0, 0, 0, 10, SSH_FXP_LSTAT, 0, 0, 0, 1, 0, 0, 0, 1, 0xff];
        // length header: 10 bytes (1 type + 4 id + 4 str_len + 1 invalid byte)
        bad_utf8_buf[0..4].copy_from_slice(&10u32.to_be_bytes());
        assert_eq!(
            SftpPacket::decode(&bad_utf8_buf),
            Err(SftpProtocolError::InvalidUtf8)
        );
    }

    #[test]
    fn verifies_sftp_all_management_packets() {
        let packets = vec![
            SftpPacket::Fstat {
                id: 1,
                handle: vec![1, 2],
            },
            SftpPacket::SetStat {
                id: 2,
                path: "/tmp/foo".to_string(),
                attrs: FileAttributes::new().with_permissions(0o777),
            },
            SftpPacket::FsetStat {
                id: 3,
                handle: vec![3, 4],
                attrs: FileAttributes::new().with_size(100),
            },
            SftpPacket::OpenDir {
                id: 4,
                path: "/var".to_string(),
            },
            SftpPacket::ReadDir {
                id: 5,
                handle: vec![5, 6],
            },
            SftpPacket::Remove {
                id: 6,
                filename: "/tmp/del".to_string(),
            },
            SftpPacket::Mkdir {
                id: 7,
                path: "/tmp/newdir".to_string(),
                attrs: FileAttributes::new(),
            },
            SftpPacket::Rmdir {
                id: 8,
                path: "/tmp/olddir".to_string(),
            },
            SftpPacket::RealPath {
                id: 9,
                path: ".".to_string(),
            },
            SftpPacket::Stat {
                id: 10,
                path: "/var/tmp".to_string(),
            },
            SftpPacket::Rename {
                id: 11,
                oldpath: "/a".to_string(),
                newpath: "/b".to_string(),
            },
            SftpPacket::ReadLink {
                id: 12,
                path: "/link".to_string(),
            },
            SftpPacket::SymLink {
                id: 13,
                linkpath: "/link".to_string(),
                targetpath: "/target".to_string(),
            },
            SftpPacket::Handle {
                id: 14,
                handle: vec![7, 8, 9],
            },
            SftpPacket::Data {
                id: 15,
                data: vec![10, 11, 12],
            },
            SftpPacket::Extended {
                id: 16,
                request: "hardlink@openssh.com".to_string(),
                data: vec![1, 2, 3],
            },
            SftpPacket::ExtendedReply {
                id: 17,
                data: vec![4, 5, 6],
            },
        ];

        for pkt in packets {
            let encoded = pkt.encode();
            let (decoded, consumed) = SftpPacket::decode(&encoded).unwrap().unwrap();
            assert_eq!(decoded, pkt);
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn verifies_transfer_queue_lifecycle_and_summary() {
        let mut queue = TransferQueue::new(2);
        assert_eq!(queue.concurrency_limit, 2);

        let t1 = queue.enqueue(TransferDirection::Upload, "/local/a", "/remote/a", 1000);
        let t2 = queue.enqueue(TransferDirection::Download, "/local/b", "/remote/b", 2000);
        let t3 = queue.enqueue(TransferDirection::Upload, "/local/c", "/remote/c", 3000);

        let summary = queue.summary();
        assert_eq!(summary.total_tasks, 3);
        assert_eq!(summary.pending_tasks, 3);
        assert_eq!(summary.in_progress_tasks, 0);
        assert_eq!(summary.total_bytes, 6000);
        assert_eq!(summary.transferred_bytes, 0);
        assert_eq!(summary.overall_percentage, 0.0);

        // Dispatch with concurrency limit 2
        let started = queue.dispatch_next();
        assert_eq!(started, vec![t1, t2]);
        assert_eq!(queue.summary().in_progress_tasks, 2);
        assert_eq!(queue.summary().pending_tasks, 1);

        // Progress on t1
        queue.update_progress(t1, 500);
        assert_eq!(queue.summary().transferred_bytes, 500);
        assert!((queue.summary().overall_percentage - (500.0 / 6000.0 * 100.0)).abs() < 0.1);

        // Complete t1
        queue.mark_completed(t1);
        assert_eq!(queue.summary().completed_tasks, 1);
        assert_eq!(queue.summary().in_progress_tasks, 1);

        // Next dispatch starts t3
        let started2 = queue.dispatch_next();
        assert_eq!(started2, vec![t3]);
        assert_eq!(queue.summary().in_progress_tasks, 2);

        // Fail t2
        queue.mark_failed(t2, "Connection reset by peer".to_string());
        assert_eq!(queue.summary().failed_tasks, 1);

        // Cancel t3
        queue.cancel(t3);
        assert_eq!(queue.summary().cancelled_tasks, 1);

        // Retry t2
        assert!(queue.retry(t2));
        assert_eq!(queue.summary().pending_tasks, 1);
        assert_eq!(queue.summary().failed_tasks, 0);

        // Clear completed
        queue.clear_completed();
        assert_eq!(queue.summary().total_tasks, 2);
    }

    #[test]
    fn verifies_sftp_client_handshake_and_directory_listing() {
        use std::io::{Read as _, Write as _};
        let (client_side, mut server_side) = std::os::unix::net::UnixStream::pair().unwrap();

        let server_thread = std::thread::spawn(move || {
            // 1. Read Init { version: 3 }
            let mut len_buf = [0u8; 4];
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();

            // Reply Version { version: 3, extensions: vec![] }
            let version_pkt = SftpPacket::Version {
                version: 3,
                extensions: vec![("posix-rename@openssh.com".to_string(), "1".to_string())],
            };
            server_side.write_all(&version_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 2. Read OpenDir
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let mut full = len_buf.to_vec();
            full.extend_from_slice(&payload);
            let (pkt, _) = SftpPacket::decode(&full).unwrap().unwrap();
            let id = match pkt {
                SftpPacket::OpenDir { id, path } => {
                    assert_eq!(path, "/var/log");
                    id
                }
                other => panic!("unexpected: {other:?}"),
            };

            let handle_pkt = SftpPacket::Handle {
                id,
                handle: vec![1, 2, 3],
            };
            server_side.write_all(&handle_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 3. Read ReadDir
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let mut full = len_buf.to_vec();
            full.extend_from_slice(&payload);
            let (pkt, _) = SftpPacket::decode(&full).unwrap().unwrap();
            let id = match pkt {
                SftpPacket::ReadDir { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };

            let name_pkt = SftpPacket::Name {
                id,
                entries: vec![
                    NameEntry {
                        filename: "syslog".to_string(),
                        longname: "-rw-r--r-- 1 root root 1024 Jan 1 00:00 syslog".to_string(),
                        attrs: FileAttributes::new().with_size(1024),
                    },
                    NameEntry {
                        filename: "nginx".to_string(),
                        longname: "drwxr-xr-x 2 root root 4096 Jan 1 00:00 nginx".to_string(),
                        attrs: FileAttributes::new().with_size(4096),
                    },
                ],
            };
            server_side.write_all(&name_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 4. Read ReadDir (EOF)
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let mut full = len_buf.to_vec();
            full.extend_from_slice(&payload);
            let (pkt, _) = SftpPacket::decode(&full).unwrap().unwrap();
            let id = match pkt {
                SftpPacket::ReadDir { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };

            let eof_pkt = SftpPacket::Status {
                id,
                code: StatusCode::Eof,
                message: "EOF".to_string(),
                tag: "".to_string(),
            };
            server_side.write_all(&eof_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 5. Read Close
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let mut full = len_buf.to_vec();
            full.extend_from_slice(&payload);
            let (pkt, _) = SftpPacket::decode(&full).unwrap().unwrap();
            let id = match pkt {
                SftpPacket::Close { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };

            let ok_pkt = SftpPacket::Status {
                id,
                code: StatusCode::Ok,
                message: "OK".to_string(),
                tag: "".to_string(),
            };
            server_side.write_all(&ok_pkt.encode()).unwrap();
            server_side.flush().unwrap();
        });

        let reader = Box::new(client_side.try_clone().unwrap());
        let writer = Box::new(client_side);
        let mut client = SftpClient::from_reader_writer(reader, writer).unwrap();

        assert_eq!(client.server_version(), 3);
        assert_eq!(
            client.server_extensions(),
            &[("posix-rename@openssh.com".to_string(), "1".to_string())]
        );

        let entries = client.list_remote_directory("/var/log").unwrap();
        assert_eq!(entries.len(), 2);
        // Directories sorted first
        assert_eq!(entries[0].name, "nginx");
        assert!(entries[0].kind.is_dir());
        assert_eq!(entries[1].name, "syslog");
        assert_eq!(entries[1].size, 1024);

        server_thread.join().unwrap();
    }

    #[test]
    fn verifies_remote_editor_workspace_dirty_tracking_and_save() {
        use std::io::{Read as _, Write as _};
        let (client_side, mut server_side) = std::os::unix::net::UnixStream::pair().unwrap();

        let server_thread = std::thread::spawn(move || {
            let mut len_buf = [0u8; 4];
            // 1. Handshake
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let version_pkt = SftpPacket::Version {
                version: 3,
                extensions: vec![],
            };
            server_side.write_all(&version_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 2. Stat
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
                .unwrap()
                .unwrap();
            let id = match pkt {
                SftpPacket::Stat { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };
            let attrs_pkt = SftpPacket::Attrs {
                id,
                attrs: FileAttributes::new().with_size(12),
            };
            server_side.write_all(&attrs_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 3. Open for Read
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
                .unwrap()
                .unwrap();
            let id = match pkt {
                SftpPacket::Open { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };
            let handle_pkt = SftpPacket::Handle {
                id,
                handle: vec![42],
            };
            server_side.write_all(&handle_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 4. Read Data
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
                .unwrap()
                .unwrap();
            let id = match pkt {
                SftpPacket::Read { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };
            let data_pkt = SftpPacket::Data {
                id,
                data: b"hello world\n".to_vec(),
            };
            server_side.write_all(&data_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 5. Read EOF
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
                .unwrap()
                .unwrap();
            let id = match pkt {
                SftpPacket::Read { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };
            let eof_pkt = SftpPacket::Status {
                id,
                code: StatusCode::Eof,
                message: "".into(),
                tag: "".into(),
            };
            server_side.write_all(&eof_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 6. Close read handle
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
                .unwrap()
                .unwrap();
            let id = match pkt {
                SftpPacket::Close { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };
            let ok_pkt = SftpPacket::Status {
                id,
                code: StatusCode::Ok,
                message: "".into(),
                tag: "".into(),
            };
            server_side.write_all(&ok_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 7. Open for Write (on save)
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
                .unwrap()
                .unwrap();
            let id = match pkt {
                SftpPacket::Open { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };
            let write_handle_pkt = SftpPacket::Handle {
                id,
                handle: vec![43],
            };
            server_side.write_all(&write_handle_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 8. Write Data
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
                .unwrap()
                .unwrap();
            let id = match pkt {
                SftpPacket::Write { id, data, .. } => {
                    assert_eq!(&data, b"hello edited\n");
                    id
                }
                other => panic!("unexpected: {other:?}"),
            };
            let write_ok_pkt = SftpPacket::Status {
                id,
                code: StatusCode::Ok,
                message: "".into(),
                tag: "".into(),
            };
            server_side.write_all(&write_ok_pkt.encode()).unwrap();
            server_side.flush().unwrap();

            // 9. Close write handle
            server_side.read_exact(&mut len_buf).unwrap();
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            server_side.read_exact(&mut payload).unwrap();
            let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
                .unwrap()
                .unwrap();
            let id = match pkt {
                SftpPacket::Close { id, .. } => id,
                other => panic!("unexpected: {other:?}"),
            };
            let ok_pkt = SftpPacket::Status {
                id,
                code: StatusCode::Ok,
                message: "".into(),
                tag: "".into(),
            };
            server_side.write_all(&ok_pkt.encode()).unwrap();
            server_side.flush().unwrap();
        });

        let reader = Box::new(client_side.try_clone().unwrap());
        let writer = Box::new(client_side);
        let mut client = SftpClient::from_reader_writer(reader, writer).unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let mut workspace = RemoteEditorWorkspace::new(temp_dir.path());

        // Checkout file
        let local_path = workspace
            .checkout_file(&mut client, "/etc/config.txt")
            .unwrap();
        assert!(local_path.exists());
        assert_eq!(
            std::fs::read_to_string(&local_path).unwrap(),
            "hello world\n"
        );
        assert!(!workspace.is_dirty("/etc/config.txt").unwrap());

        // Modify local file
        std::fs::write(&local_path, "hello edited\n").unwrap();
        assert!(workspace.is_dirty("/etc/config.txt").unwrap());

        // Save file back to remote
        assert!(workspace.save_file(&mut client, "/etc/config.txt").unwrap());
        assert!(!workspace.is_dirty("/etc/config.txt").unwrap());

        server_thread.join().unwrap();
    }
}

#[cfg(test)]
mod transfer_completion_tests {
    use super::*;
    use std::io::Cursor;

    fn client(mut replies: Vec<SftpPacket>) -> SftpClient {
        replies.insert(
            0,
            SftpPacket::Version {
                version: 3,
                extensions: vec![],
            },
        );
        let bytes: Vec<u8> = replies
            .into_iter()
            .flat_map(|packet| packet.encode())
            .collect();
        SftpClient::from_reader_writer(Box::new(Cursor::new(bytes)), Box::new(Vec::<u8>::new()))
            .unwrap()
    }
    fn status(id: u32, code: StatusCode) -> SftpPacket {
        SftpPacket::Status {
            id,
            code,
            message: "fixture close status".into(),
            tag: String::new(),
        }
    }
    fn handle(id: u32) -> SftpPacket {
        SftpPacket::Handle {
            id,
            handle: b"fixture-handle".to_vec(),
        }
    }
    #[test]
    fn upload_refuses_close_failure_wrong_request_and_disconnect() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        std::fs::write(&source, b"payload").unwrap();
        for close in [
            Some(status(3, StatusCode::Failure)),
            Some(status(99, StatusCode::Ok)),
            None,
        ] {
            let mut replies = vec![handle(1), status(2, StatusCode::Ok)];
            replies.extend(close);
            assert!(
                client(replies)
                    .upload_file(&source, "/remote", None::<fn(u64, u64)>)
                    .is_err(),
                "Upload must not report completion without the matching successful CLOSE"
            );
        }
    }
    #[test]
    fn download_refuses_close_failure_wrong_request_and_disconnect() {
        let dir = tempfile::tempdir().unwrap();
        for close in [
            Some(status(5, StatusCode::Failure)),
            Some(status(99, StatusCode::Ok)),
            None,
        ] {
            let mut replies = vec![
                SftpPacket::Attrs {
                    id: 1,
                    attrs: FileAttributes::new(),
                },
                handle(2),
                SftpPacket::Data {
                    id: 3,
                    data: b"payload".to_vec(),
                },
                status(4, StatusCode::Eof),
            ];
            replies.extend(close);
            assert!(
                client(replies)
                    .download_file("/remote", &dir.path().join("output"), None::<fn(u64, u64)>)
                    .is_err(),
                "Download must not report completion without the matching successful CLOSE"
            );
        }
    }
    #[test]
    fn directory_listing_refuses_close_failure() {
        let result = client(vec![
            handle(1),
            status(2, StatusCode::Eof),
            status(3, StatusCode::Failure),
        ])
        .list_remote_directory("/remote");
        assert!(matches!(
            result,
            Err(SftpError::Status {
                code: StatusCode::Failure,
                ..
            })
        ));
    }
}

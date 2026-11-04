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
}

//! SFTP v3 packet types, codecs, and protocol constants.
use super::{FileEntry, FileKind, PathUtils, MAX_SFTP_PACKET_LENGTH};
use std::fmt;

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
        if length == 0 || length > MAX_SFTP_PACKET_LENGTH {
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

pub(super) fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, s.len() as u32);
    buf.extend_from_slice(s.as_bytes());
}

fn write_bytes(buf: &mut Vec<u8>, b: &[u8]) {
    write_u32(buf, b.len() as u32);
    buf.extend_from_slice(b);
}

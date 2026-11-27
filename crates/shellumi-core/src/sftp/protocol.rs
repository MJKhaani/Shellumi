//! SFTP v3 server capability negotiation, limits, statvfs, and transfer tuning.

use super::wire::WireReader;
use super::SftpProtocolError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Server-advertised limits from `limits@openssh.com`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerLimits {
    pub max_packet_len: u64,
    pub max_read_len: u64,
    pub max_write_len: u64,
    pub max_open_handles: u64,
}

impl ServerLimits {
    pub fn decode(data: &[u8]) -> Result<Self, SftpProtocolError> {
        let mut r = WireReader::new(data);
        let max_packet_len = r.read_u64()?;
        let max_read_len = r.read_u64()?;
        let max_write_len = r.read_u64()?;
        let max_open_handles = r.read_u64()?;
        Ok(Self {
            max_packet_len,
            max_read_len,
            max_write_len,
            max_open_handles,
        })
    }
}

/// Filesystem capacity statistics from `statvfs@openssh.com` / `fstatvfs@openssh.com`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteStatVfs {
    pub bsize: u64,
    pub frsize: u64,
    pub blocks: u64,
    pub bfree: u64,
    pub bavail: u64,
    pub files: u64,
    pub ffree: u64,
    pub favail: u64,
    pub fsid: u64,
    pub flag: u64,
    pub namemax: u64,
}

impl RemoteStatVfs {
    pub fn decode(data: &[u8]) -> Result<Self, SftpProtocolError> {
        let mut r = WireReader::new(data);
        Ok(Self {
            bsize: r.read_u64()?,
            frsize: r.read_u64()?,
            blocks: r.read_u64()?,
            bfree: r.read_u64()?,
            bavail: r.read_u64()?,
            files: r.read_u64()?,
            ffree: r.read_u64()?,
            favail: r.read_u64()?,
            fsid: r.read_u64()?,
            flag: r.read_u64()?,
            namemax: r.read_u64()?,
        })
    }

    pub fn total_bytes(&self) -> u64 {
        self.blocks.saturating_mul(self.frsize)
    }

    pub fn available_bytes(&self) -> u64 {
        self.bavail.saturating_mul(self.frsize)
    }
}

/// Typed SFTP server capabilities parsed from `SSH_FXP_VERSION` extensions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub posix_rename: bool,
    pub fsync: bool,
    pub statvfs: bool,
    pub fstatvfs: bool,
    pub hardlink: bool,
    pub copy_data: bool,
    pub limits_extension: bool,
    pub limits: Option<ServerLimits>,
}

impl ServerCapabilities {
    pub fn from_extensions(extensions: &[(String, String)]) -> Self {
        let mut caps = Self::default();
        for (name, version) in extensions {
            match (name.as_str(), version.as_str()) {
                ("posix-rename@openssh.com", "1") => caps.posix_rename = true,
                ("fsync@openssh.com", "1") => caps.fsync = true,
                ("statvfs@openssh.com", "2") => caps.statvfs = true,
                ("fstatvfs@openssh.com", "2") => caps.fstatvfs = true,
                ("hardlink@openssh.com", "1") => caps.hardlink = true,
                ("copy-data", "1") => caps.copy_data = true,
                ("limits@openssh.com", "1") => caps.limits_extension = true,
                _ => {}
            }
        }
        caps
    }
}

/// Bounds for remote directory listing to prevent unbounded memory/CPU on hostile peers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListingLimits {
    pub max_entries: usize,
    pub max_requests: usize,
    pub max_empty_replies: usize,
    pub overall_deadline: Duration,
}

impl Default for ListingLimits {
    fn default() -> Self {
        Self {
            max_entries: 250_000,
            max_requests: 50_000,
            max_empty_replies: 8,
            overall_deadline: Duration::from_secs(60),
        }
    }
}

/// Pipelined streaming parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransferTuning {
    pub chunk_size: usize,
    pub pipeline_depth: usize,
}

impl Default for TransferTuning {
    fn default() -> Self {
        Self {
            chunk_size: 32 * 1024,
            pipeline_depth: 16,
        }
    }
}

impl TransferTuning {
    pub fn clamped_for_limits(&self, limits: Option<ServerLimits>) -> Self {
        let mut chunk = self.chunk_size.clamp(4096, 256 * 1024);
        if let Some(lim) = limits {
            if lim.max_read_len > 0 {
                chunk = chunk.min(lim.max_read_len as usize);
            }
            if lim.max_write_len > 0 {
                chunk = chunk.min(lim.max_write_len as usize);
            }
        }
        Self {
            chunk_size: chunk.max(1024),
            pipeline_depth: self.pipeline_depth.clamp(1, 64),
        }
    }
}

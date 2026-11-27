//! Native local and remote SFTP path types, entry validation, and OS-aware breadcrumbs.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Component, Path, PathBuf};

/// Path semantics of an endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PathSemantics {
    Posix,
    Windows,
}

impl PathSemantics {
    pub fn local_native() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Posix
        }
    }
}

/// Errors returned when validating untrusted filenames or paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathValidationError {
    Empty,
    ContainsNul,
    ContainsSeparator,
    ContainsControlCharacter,
    TraversalComponent,
    AbsoluteOrDrivePrefix,
    WindowsReservedName(String),
    WindowsInvalidCharacter(char),
    WindowsTrailingDotOrSpace,
    WindowsAlternateDataStream,
    InvalidUtf8ForDestination,
    ProtectedRoot,
}

impl fmt::Display for PathValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("filename cannot be empty"),
            Self::ContainsNul => f.write_str("filename contains NUL byte"),
            Self::ContainsSeparator => f.write_str("filename contains path separator"),
            Self::ContainsControlCharacter => f.write_str("filename contains control character"),
            Self::TraversalComponent => f.write_str("filename cannot be '.' or '..'"),
            Self::AbsoluteOrDrivePrefix => {
                f.write_str("filename contains absolute or drive prefix")
            }
            Self::WindowsReservedName(name) => {
                write!(f, "'{name}' is a reserved Windows device name")
            }
            Self::WindowsInvalidCharacter(ch) => {
                write!(f, "filename contains invalid Windows character '{ch}'")
            }
            Self::WindowsTrailingDotOrSpace => {
                f.write_str("Windows filenames cannot end with a dot or space")
            }
            Self::WindowsAlternateDataStream => {
                f.write_str("Windows alternate data streams (':') are not permitted")
            }
            Self::InvalidUtf8ForDestination => {
                f.write_str("filename contains non-UTF-8 bytes unsupported by destination")
            }
            Self::ProtectedRoot => f.write_str("operation on filesystem root is protected"),
        }
    }
}

impl std::error::Error for PathValidationError {}

/// Classification of a raw directory listing name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListingNameKind {
    Dot,
    DotDot,
    Child(ValidatedChildName),
}

/// A validated single-component child filename with raw byte fidelity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ValidatedChildName {
    raw: Vec<u8>,
    display: String,
}

impl ValidatedChildName {
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    pub fn as_utf8(&self) -> Option<&str> {
        std::str::from_utf8(&self.raw).ok()
    }
}

/// Escape non-UTF-8 or control bytes for safe UI display without altering raw identity.
pub fn escape_bytes_for_display(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let mut out = String::with_capacity(bytes.len());
            for chunk in bytes.utf8_chunks() {
                out.push_str(chunk.valid());
                for b in chunk.invalid() {
                    use std::fmt::Write;
                    let _ = write!(out, "\\x{b:02x}");
                }
            }
            out
        }
    }
}

/// Validate a raw filename from a directory listing (`SSH_FXP_NAME` or `read_dir`).
///
/// Classifies `.` and `..` separately so callers can skip them, and rejects any
/// NUL byte, path separator under `semantics`, or control characters.
pub fn classify_listing_name(
    raw: &[u8],
    semantics: PathSemantics,
) -> Result<ListingNameKind, PathValidationError> {
    if raw.is_empty() {
        return Err(PathValidationError::Empty);
    }
    if raw == b"." {
        return Ok(ListingNameKind::Dot);
    }
    if raw == b".." {
        return Ok(ListingNameKind::DotDot);
    }
    if raw.contains(&0) {
        return Err(PathValidationError::ContainsNul);
    }
    if raw.contains(&b'/') {
        return Err(PathValidationError::ContainsSeparator);
    }
    if semantics == PathSemantics::Windows && raw.contains(&b'\\') {
        return Err(PathValidationError::ContainsSeparator);
    }
    if let Ok(s) = std::str::from_utf8(raw) {
        if s.chars().any(char::is_control) {
            return Err(PathValidationError::ContainsControlCharacter);
        }
        if semantics == PathSemantics::Windows {
            validate_windows_component_str(s)?;
        }
    } else if raw.iter().any(|&b| b < 0x20 || b == 0x7f) {
        return Err(PathValidationError::ContainsControlCharacter);
    }

    Ok(ListingNameKind::Child(ValidatedChildName {
        raw: raw.to_vec(),
        display: escape_bytes_for_display(raw),
    }))
}

/// Validate a filename for creation/transfer at a specific destination OS.
///
/// Even when listing a POSIX remote server where `aux` or `foo\bar` or `con.txt` is legal,
/// transferring that entry to a Windows destination must reject or explicitly remap it.
pub fn validate_destination_name(
    raw: &[u8],
    dest_semantics: PathSemantics,
) -> Result<ValidatedChildName, PathValidationError> {
    match classify_listing_name(raw, dest_semantics)? {
        ListingNameKind::Dot | ListingNameKind::DotDot => {
            Err(PathValidationError::TraversalComponent)
        }
        ListingNameKind::Child(child) => {
            if dest_semantics == PathSemantics::Windows {
                let s = std::str::from_utf8(raw)
                    .map_err(|_| PathValidationError::InvalidUtf8ForDestination)?;
                validate_windows_component_str(s)?;
            }
            Ok(child)
        }
    }
}

fn validate_windows_component_str(name: &str) -> Result<(), PathValidationError> {
    if name.contains(':') {
        return Err(PathValidationError::WindowsAlternateDataStream);
    }
    for ch in name.chars() {
        if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            return Err(PathValidationError::WindowsInvalidCharacter(ch));
        }
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(PathValidationError::WindowsTrailingDotOrSpace);
    }
    // Check Windows reserved device names (case-insensitive, with or without extension).
    let stem = name.split('.').next().unwrap_or(name);
    let upper = stem.to_ascii_uppercase();
    let is_reserved = matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if is_reserved {
        return Err(PathValidationError::WindowsReservedName(name.to_string()));
    }
    Ok(())
}

/// Platform-aware local home directory discovery.
/// Does not rely solely on `$HOME` (handles Windows `USERPROFILE` / `HOMEDRIVE`+`HOMEPATH`).
pub fn local_home_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            if !profile.is_empty() {
                return PathBuf::from(profile);
            }
        }
        if let (Some(drive), Some(path)) =
            (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
        {
            let mut p = PathBuf::from(drive);
            p.push(path);
            if !p.as_os_str().is_empty() {
                return p;
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    std::env::current_dir().unwrap_or_else(|_| {
        if cfg!(windows) {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        }
    })
}

/// Safely join a single validated child name onto a local directory `Path`.
pub fn local_join_child(base: &Path, child_name: &str) -> Result<PathBuf, PathValidationError> {
    let _ = validate_destination_name(child_name.as_bytes(), PathSemantics::local_native())?;
    Ok(base.join(child_name))
}

/// Native parent directory for a local path (handles Unix `/`, Windows `C:\`, and UNC `\\server\share`).
pub fn local_parent(path: &Path) -> Option<PathBuf> {
    path.parent().and_then(|p| {
        if p.as_os_str().is_empty() {
            None
        } else {
            Some(p.to_path_buf())
        }
    })
}

/// Native OS breadcrumbs for a local path.
pub fn local_breadcrumbs(path: &Path) -> Vec<(String, PathBuf)> {
    let mut crumbs = Vec::new();
    let mut acc = PathBuf::new();

    for comp in path.components() {
        match comp {
            Component::Prefix(prefix) => {
                acc.push(prefix.as_os_str());
                let label = prefix.as_os_str().to_string_lossy().to_string();
                crumbs.push((label, acc.clone()));
            }
            Component::RootDir => {
                acc.push(comp.as_os_str());
                if crumbs.is_empty() {
                    crumbs.push(("/".to_string(), acc.clone()));
                } else if let Some(last) = crumbs.last_mut() {
                    last.1 = acc.clone();
                }
            }
            Component::Normal(seg) => {
                acc.push(seg);
                crumbs.push((seg.to_string_lossy().to_string(), acc.clone()));
            }
            Component::CurDir | Component::ParentDir => {}
        }
    }

    if crumbs.is_empty() {
        crumbs.push(("/".to_string(), PathBuf::from("/")));
    }
    crumbs
}

/// Returns true if `path` is a protected filesystem root (`/`, `C:\`, `\\server\share`).
pub fn is_protected_local_root(path: &Path) -> bool {
    let mut comps = path.components();
    match comps.next() {
        None => true,
        Some(Component::RootDir) => comps.next().is_none(),
        Some(Component::Prefix(_)) => match comps.next() {
            None => true,
            Some(Component::RootDir) => comps.next().is_none(),
            _ => false,
        },
        _ => false,
    }
}

/// Dedicated remote SFTP path representation preserving raw server bytes when non-UTF-8.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemotePath {
    raw: Vec<u8>,
    display: String,
    semantics: PathSemantics,
}

impl RemotePath {
    pub fn from_bytes(raw: Vec<u8>, semantics: PathSemantics) -> Self {
        let display = escape_bytes_for_display(&raw);
        Self {
            raw,
            display,
            semantics,
        }
    }

    pub fn from_posix_str(path: &str) -> Self {
        let normalized = if path.is_empty() { "/" } else { path };
        Self {
            raw: normalized.as_bytes().to_vec(),
            display: normalized.to_string(),
            semantics: PathSemantics::Posix,
        }
    }

    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }

    pub fn display(&self) -> &str {
        &self.display
    }

    pub fn semantics(&self) -> PathSemantics {
        self.semantics
    }

    pub fn is_root(&self) -> bool {
        match self.semantics {
            PathSemantics::Posix => self.raw == b"/",
            PathSemantics::Windows => {
                let s = self.display.trim_end_matches(['/', '\\']);
                s.is_empty() || (s.len() == 2 && s.as_bytes()[1] == b':')
            }
        }
    }

    /// Join a validated single child name onto this remote directory path.
    pub fn join_child(&self, child: &ValidatedChildName) -> Result<Self, PathValidationError> {
        // Re-validate against our own semantics.
        let _ = validate_destination_name(child.raw_bytes(), self.semantics)?;
        let sep = match self.semantics {
            PathSemantics::Posix => b'/',
            PathSemantics::Windows => b'/', // OpenSSH SFTP on Windows accepts forward slashes
        };
        let mut raw = self.raw.clone();
        if !raw.ends_with(b"/") && !raw.ends_with(b"\\") {
            raw.push(sep);
        }
        raw.extend_from_slice(child.raw_bytes());
        Ok(Self::from_bytes(raw, self.semantics))
    }

    /// Join a string child name, validating it first.
    pub fn join_child_str(&self, name: &str) -> Result<Self, PathValidationError> {
        let child = validate_destination_name(name.as_bytes(), self.semantics)?;
        self.join_child(&child)
    }

    /// Compute parent remote path without using `std::path::Path` (which is local-OS dependent).
    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }
        let mut bytes = self.raw.clone();
        while bytes.len() > 1 && (bytes.ends_with(b"/") || bytes.ends_with(b"\\")) {
            bytes.pop();
        }
        if let Some(pos) = bytes.iter().rposition(|&b| b == b'/' || b == b'\\') {
            if pos == 0 {
                return Some(Self::from_bytes(vec![bytes[0]], self.semantics));
            }
            // Handle Windows drive root like C:/foo -> C:/
            if self.semantics == PathSemantics::Windows && pos == 2 && bytes[1] == b':' {
                bytes.truncate(3);
                return Some(Self::from_bytes(bytes, self.semantics));
            }
            bytes.truncate(pos);
            Some(Self::from_bytes(bytes, self.semantics))
        } else {
            None
        }
    }

    /// Generate breadcrumbs for the remote path.
    pub fn breadcrumbs(&self) -> Vec<(String, RemotePath)> {
        let s = self.display.trim();
        if s.is_empty() || s == "/" {
            return vec![("/".to_string(), Self::from_posix_str("/"))];
        }
        let mut crumbs = Vec::new();
        let mut acc = String::new();
        if s.starts_with('/') {
            crumbs.push(("/".to_string(), Self::from_posix_str("/")));
            acc.push('/');
        }
        for seg in s.split(['/', '\\']).filter(|seg| !seg.is_empty()) {
            if !acc.is_empty() && !acc.ends_with('/') {
                acc.push('/');
            }
            acc.push_str(seg);
            crumbs.push((seg.to_string(), Self::from_posix_str(&acc)));
        }
        if crumbs.is_empty() {
            crumbs.push(("/".to_string(), Self::from_posix_str("/")));
        }
        crumbs
    }

    pub fn file_name_display(&self) -> &str {
        self.display
            .trim_end_matches(['/', '\\'])
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&self.display)
    }
}

impl fmt::Display for RemotePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display)
    }
}

/// Case-sensitivity aware canonical key for destination reservation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CanonicalDestKey {
    pub endpoint_key: String,
    pub normalized_path: String,
}

impl CanonicalDestKey {
    pub fn for_local(path: &Path) -> Self {
        // Canonicalize parent if possible to resolve symlink aliases on local disk.
        let resolved = if let Some(parent) = path.parent() {
            if let (Ok(canon_parent), Some(name)) =
                (std::fs::canonicalize(parent), path.file_name())
            {
                canon_parent.join(name)
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        };
        let raw = resolved.to_string_lossy().to_string();
        // macOS (APFS/HFS+ default) and Windows are case-insensitive by default.
        let normalized_path = if cfg!(any(target_os = "macos", target_os = "windows")) {
            raw.to_lowercase()
        } else {
            raw
        };
        Self {
            endpoint_key: "local".to_string(),
            normalized_path,
        }
    }

    pub fn for_remote(vault_id: &str, host_id: i64, remote_path: &RemotePath) -> Self {
        let normalized_path = if remote_path.semantics() == PathSemantics::Windows {
            remote_path.display().to_lowercase()
        } else {
            remote_path.display().to_string()
        };
        Self {
            endpoint_key: format!("remote:{vault_id}:{host_id}"),
            normalized_path,
        }
    }
}

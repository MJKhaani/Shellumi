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
}

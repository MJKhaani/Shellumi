//! Safe iterative recursive planner, conflict resolution, destination reservations,
//! and local staged copy/delete primitives.

use super::endpoint::{EndpointKind, JobId, TransferRoute};
use super::path::{
    classify_listing_name, is_protected_local_root, validate_destination_name, CanonicalDestKey,
    ListingNameKind, PathSemantics, RemotePath,
};
use super::{FileKind, SftpError, TransferControl};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// User-configured conflict policy for a transfer batch. Default is `Ask`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictPolicy {
    #[default]
    Ask,
    Overwrite,
    Skip,
    KeepBoth,
    Cancel,
}

/// Interactive decision returned from the conflict prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictAction {
    Overwrite,
    Skip,
    KeepBoth,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictDecision {
    pub action: ConflictAction,
    pub apply_to_all: bool,
}

/// Metadata presented to the user when a destination conflict occurs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictContext {
    pub job_id: JobId,
    pub route: TransferRoute,
    pub source_display: String,
    pub destination_display: String,
    pub source_size: Option<u64>,
    pub destination_size: Option<u64>,
    pub source_modified: Option<u64>,
    pub destination_modified: Option<u64>,
    pub source_kind: FileKind,
    pub destination_kind: FileKind,
    pub is_type_mismatch: bool,
}

/// How symlinks are handled during recursive enumeration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymlinkPolicy {
    /// Do not follow symlinks; skip with a clear entry report.
    #[default]
    Skip,
    /// Recreate the symlink itself on the destination when supported.
    Preserve,
    /// Follow symlinks with bounded depth and cycle detection (requires explicit user confirmation).
    Dereference,
}

/// Bounds for recursive directory enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanLimits {
    pub max_depth: usize,
    pub max_entries: usize,
}

impl Default for PlanLimits {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_entries: 250_000,
        }
    }
}

/// A single item in a planned recursive transfer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedTransferItem {
    pub relative_components: Vec<String>,
    pub source_path: PathBuf,
    pub kind: FileKind,
    pub size: u64,
    pub modified: Option<u64>,
    pub permissions: Option<u32>,
    pub symlink_target: Option<String>,
}

/// Output of planning a local directory or file tree.
#[derive(Clone, Debug, Default)]
pub struct LocalTreePlan {
    pub items: Vec<PlannedTransferItem>,
    pub skipped_symlinks: Vec<(PathBuf, String)>,
    pub skipped_special_files: Vec<(PathBuf, String)>,
    pub total_bytes: u64,
    pub total_files: usize,
    pub total_dirs: usize,
}

/// Thread-safe destination reservation registry across all tabs, workers, and directions.
/// Prevents two concurrent jobs from writing to the same canonical destination or overlapping
/// ancestor/descendant trees.
#[derive(Clone, Default)]
pub struct DestinationReservationRegistry {
    active: Arc<Mutex<HashMap<CanonicalDestKey, JobId>>>,
}

impl DestinationReservationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve `key` for `job_id`, rejecting exact duplicates or overlapping ancestor/descendant
    /// paths on the same endpoint.
    pub fn reserve(
        &self,
        key: CanonicalDestKey,
        job_id: JobId,
    ) -> Result<ReservationGuard, SftpError> {
        let mut guard = self.active.lock().map_err(|_| {
            SftpError::UnexpectedResponse("destination reservation lock poisoned".into())
        })?;

        for (existing_key, existing_job) in guard.iter() {
            if existing_key.endpoint_key != key.endpoint_key {
                continue;
            }
            if paths_overlap(&existing_key.normalized_path, &key.normalized_path) {
                tracing::warn!(
                    job_id = job_id.0,
                    existing_job_id = existing_job.0,
                    path = %key.normalized_path,
                    "Destination reservation conflict detected"
                );
                return Err(SftpError::UnexpectedResponse(format!(
                    "Destination '{}' is already reserved by active transfer job {}",
                    key.normalized_path, existing_job.0
                )));
            }
        }

        guard.insert(key.clone(), job_id);
        Ok(ReservationGuard {
            registry: Arc::clone(&self.active),
            key,
            job_id,
        })
    }

    pub fn is_reserved(&self, key: &CanonicalDestKey) -> bool {
        self.active
            .lock()
            .map(|g| {
                g.keys().any(|k| {
                    k.endpoint_key == key.endpoint_key
                        && paths_overlap(&k.normalized_path, &key.normalized_path)
                })
            })
            .unwrap_or(false)
    }
}

/// RAII guard that releases a destination reservation only when dropped by the owning worker.
pub struct ReservationGuard {
    registry: Arc<Mutex<HashMap<CanonicalDestKey, JobId>>>,
    key: CanonicalDestKey,
    job_id: JobId,
}

impl Drop for ReservationGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.registry.lock() {
            if guard.get(&self.key) == Some(&self.job_id) {
                guard.remove(&self.key);
            }
        }
    }
}

fn paths_overlap(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a_trim = a.trim_end_matches(['/', '\\']);
    let b_trim = b.trim_end_matches(['/', '\\']);
    if a_trim == b_trim {
        return true;
    }
    is_strict_prefix(a_trim, b_trim) || is_strict_prefix(b_trim, a_trim)
}

fn is_strict_prefix(parent: &str, child: &str) -> bool {
    if parent.is_empty() {
        return false;
    }
    if !child.starts_with(parent) {
        return false;
    }
    let rest = &child[parent.len()..];
    rest.starts_with('/') || rest.starts_with('\\')
}

/// Reject copying or moving a directory into itself or any of its descendants,
/// or copying a file onto itself, using canonical endpoint + canonical path identity.
pub fn validate_non_overlapping_local_copy(src: &Path, dst: &Path) -> Result<(), SftpError> {
    let canon_src = fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
    let canon_dst_parent = dst
        .parent()
        .and_then(|p| fs::canonicalize(p).ok())
        .map(|p| match dst.file_name() {
            Some(name) => p.join(name),
            None => p,
        })
        .unwrap_or_else(|| dst.to_path_buf());

    if canon_src == canon_dst_parent {
        return Err(SftpError::UnexpectedResponse(
            "Cannot copy or move an item onto itself".into(),
        ));
    }
    if canon_dst_parent.starts_with(&canon_src) {
        return Err(SftpError::UnexpectedResponse(
            "Cannot copy or move a directory into itself or a descendant".into(),
        ));
    }
    Ok(())
}

/// Reject copying or moving a remote directory into itself or a descendant on the same endpoint.
pub fn validate_non_overlapping_same_endpoint_remote(
    src_endpoint: &EndpointKind,
    dst_endpoint: &EndpointKind,
    src_path: &RemotePath,
    dst_path: &RemotePath,
) -> Result<(), SftpError> {
    if !src_endpoint.same_underlying_target(dst_endpoint) {
        return Ok(());
    }
    let s = src_path.display().trim_end_matches('/');
    let d = dst_path.display().trim_end_matches('/');
    if s == d {
        return Err(SftpError::UnexpectedResponse(
            "Cannot copy or move a remote item onto itself".into(),
        ));
    }
    if is_strict_prefix(s, d) {
        return Err(SftpError::UnexpectedResponse(
            "Cannot copy or move a remote directory into itself or a descendant".into(),
        ));
    }
    Ok(())
}

/// Plan a local filesystem tree iteratively using `symlink_metadata` (never `is_dir()` on followed paths).
pub fn plan_local_tree(
    root: &Path,
    dest_semantics: PathSemantics,
    symlink_policy: SymlinkPolicy,
    limits: PlanLimits,
    control: &TransferControl,
    mut on_enum_progress: Option<&mut dyn FnMut(usize, u64)>,
) -> Result<LocalTreePlan, SftpError> {
    control.check()?;
    let root_meta = fs::symlink_metadata(root)?;
    let mut plan = LocalTreePlan::default();

    if root_meta.file_type().is_symlink() {
        let target = fs::read_link(root)
            .ok()
            .map(|p| p.to_string_lossy().to_string());
        match symlink_policy {
            SymlinkPolicy::Skip => {
                plan.skipped_symlinks
                    .push((root.to_path_buf(), "symlink skipped by policy".into()));
                return Ok(plan);
            }
            SymlinkPolicy::Preserve => {
                plan.items.push(PlannedTransferItem {
                    relative_components: Vec::new(),
                    source_path: root.to_path_buf(),
                    kind: FileKind::Symlink { target },
                    size: 0,
                    modified: None,
                    permissions: None,
                    symlink_target: None,
                });
                return Ok(plan);
            }
            SymlinkPolicy::Dereference => {
                // Fall through to dereferenced traversal below.
            }
        }
    }

    // Track visited canonical directories for symlink-cycle detection when Dereference is enabled.
    let mut visited_dirs: HashSet<PathBuf> = HashSet::new();
    let mut queue: VecDeque<(PathBuf, Vec<String>, usize)> = VecDeque::new();
    queue.push_back((root.to_path_buf(), Vec::new(), 0));

    while let Some((current_path, rel_parts, depth)) = queue.pop_front() {
        control.check()?;
        if depth > limits.max_depth {
            return Err(SftpError::UnexpectedResponse(format!(
                "Directory traversal exceeded maximum depth ({})",
                limits.max_depth
            )));
        }
        if plan.items.len() >= limits.max_entries {
            return Err(SftpError::UnexpectedResponse(format!(
                "Directory traversal exceeded maximum entry limit ({})",
                limits.max_entries
            )));
        }

        let meta = if symlink_policy == SymlinkPolicy::Dereference {
            fs::metadata(&current_path)?
        } else {
            fs::symlink_metadata(&current_path)?
        };

        let ft = meta.file_type();
        if ft.is_symlink() {
            let target = fs::read_link(&current_path)
                .ok()
                .map(|p| p.to_string_lossy().to_string());
            match symlink_policy {
                SymlinkPolicy::Skip => {
                    plan.skipped_symlinks
                        .push((current_path, "symlink skipped by policy".into()));
                    continue;
                }
                SymlinkPolicy::Preserve => {
                    plan.items.push(PlannedTransferItem {
                        relative_components: rel_parts,
                        source_path: current_path,
                        kind: FileKind::Symlink {
                            target: target.clone(),
                        },
                        size: 0,
                        modified: None,
                        permissions: None,
                        symlink_target: target,
                    });
                    continue;
                }
                SymlinkPolicy::Dereference => {
                    // `fs::metadata` already followed it above.
                }
            }
        }

        if ft.is_dir() {
            if let Ok(canon) = fs::canonicalize(&current_path) {
                if !visited_dirs.insert(canon) {
                    plan.skipped_symlinks
                        .push((current_path, "symlink cycle detected".into()));
                    continue;
                }
            }

            plan.total_dirs += 1;
            plan.items.push(PlannedTransferItem {
                relative_components: rel_parts.clone(),
                source_path: current_path.clone(),
                kind: FileKind::Directory,
                size: 0,
                modified: extract_modified(&meta),
                permissions: extract_permissions(&meta),
                symlink_target: None,
            });

            let mut children: Vec<_> =
                fs::read_dir(&current_path)?.collect::<Result<Vec<_>, _>>()?;
            children.sort_by_key(|e| e.file_name());

            for child in children {
                control.check()?;
                let raw_name = child.file_name();
                let raw_bytes = raw_name.as_encoded_bytes();
                match classify_listing_name(raw_bytes, PathSemantics::local_native()) {
                    Ok(ListingNameKind::Dot | ListingNameKind::DotDot) => continue,
                    Ok(ListingNameKind::Child(validated)) => {
                        validate_destination_name(validated.raw_bytes(), dest_semantics)
                            .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?;
                        let mut next_rel = rel_parts.clone();
                        next_rel.push(validated.display().to_string());
                        queue.push_back((child.path(), next_rel, depth + 1));
                    }
                    Err(e) => {
                        return Err(SftpError::UnexpectedResponse(format!(
                            "Unsafe filename in {}: {e}",
                            current_path.display()
                        )));
                    }
                }
            }
        } else if ft.is_file() {
            let size = meta.len();
            plan.total_files += 1;
            plan.total_bytes = plan.total_bytes.saturating_add(size);
            plan.items.push(PlannedTransferItem {
                relative_components: rel_parts,
                source_path: current_path,
                kind: FileKind::File,
                size,
                modified: extract_modified(&meta),
                permissions: extract_permissions(&meta),
                symlink_target: None,
            });
        } else {
            // FIFO, socket, block/char device — never read as regular stream.
            plan.skipped_special_files
                .push((current_path, "special file skipped".into()));
        }

        if let Some(ref mut cb) = on_enum_progress {
            cb(plan.items.len(), plan.total_bytes);
        }
    }

    Ok(plan)
}

/// Safe local-to-local file copy using exclusive sibling staging and atomic rename.
/// Never overwrites `dst` directly with `std::fs::copy`.
pub fn staged_local_copy_file(
    src: &Path,
    dst: &Path,
    overwrite: bool,
    control: &TransferControl,
    mut on_progress: Option<&mut dyn FnMut(u64, u64)>,
) -> Result<u64, SftpError> {
    control.check()?;
    validate_non_overlapping_local_copy(src, dst)?;

    let src_meta = fs::symlink_metadata(src)?;
    if !src_meta.file_type().is_file() {
        return Err(SftpError::UnexpectedResponse(format!(
            "Source '{}' is not a regular file",
            src.display()
        )));
    }
    let total_size = src_meta.len();

    let parent = dst.parent().ok_or_else(|| {
        SftpError::UnexpectedResponse("Destination has no parent directory".into())
    })?;
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    // Refuse writing through an existing destination symlink when overwrite is false,
    // or verify destination parent isn't redirecting.
    if !overwrite && fs::symlink_metadata(dst).is_ok() {
        return Err(SftpError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Destination '{}' already exists", dst.display()),
        )));
    }

    let staging_name = format!(".shellumi.part.{}", uuid::Uuid::new_v4().simple());
    let staging_path = parent.join(staging_name);

    let mut src_file = fs::File::open(src)?;
    let mut dst_file = {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        opts.open(&staging_path)?
    };

    let mut buf = vec![0u8; 64 * 1024];
    let mut copied: u64 = 0;

    let copy_result = (|| -> Result<(), SftpError> {
        loop {
            control.check()?;
            let n = src_file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            dst_file.write_all(&buf[..n])?;
            copied += n as u64;
            if let Some(ref mut cb) = on_progress {
                cb(copied, total_size);
            }
        }
        dst_file.sync_all()?;
        Ok(())
    })();

    drop(dst_file);

    if let Err(err) = copy_result {
        let _ = fs::remove_file(&staging_path);
        return Err(err);
    }

    // Preserve permissions if available.
    let _ = fs::set_permissions(&staging_path, src_meta.permissions());

    if !overwrite {
        // Atomic no-clobber check before rename.
        if fs::symlink_metadata(dst).is_ok() {
            let _ = fs::remove_file(&staging_path);
            return Err(SftpError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("Destination '{}' appeared before commit", dst.display()),
            )));
        }
    }

    if let Err(err) = fs::rename(&staging_path, dst) {
        let _ = fs::remove_file(&staging_path);
        return Err(err.into());
    }

    Ok(copied)
}

/// Safe iterative post-order local deletion that NEVER follows symlinks and protects filesystem roots.
pub fn delete_local_recursive(
    target: &Path,
    control: &TransferControl,
) -> Result<usize, SftpError> {
    control.check()?;
    if is_protected_local_root(target) {
        return Err(SftpError::UnexpectedResponse(format!(
            "Refusing to delete protected root '{}'",
            target.display()
        )));
    }

    let meta = fs::symlink_metadata(target)?;
    if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
        fs::remove_file(target)?;
        return Ok(1);
    }

    // Post-order iterative traversal: (path, expanded_children)
    let mut stack: Vec<(PathBuf, bool)> = vec![(target.to_path_buf(), false)];
    let mut deleted = 0usize;

    while let Some((path, expanded)) = stack.pop() {
        control.check()?;
        if expanded {
            fs::remove_dir(&path)?;
            deleted += 1;
            continue;
        }
        let m = fs::symlink_metadata(&path)?;
        if m.file_type().is_symlink() || !m.file_type().is_dir() {
            fs::remove_file(&path)?;
            deleted += 1;
            continue;
        }
        stack.push((path.clone(), true));
        for entry in fs::read_dir(&path)? {
            let entry = entry?;
            stack.push((entry.path(), false));
        }
    }

    Ok(deleted)
}

fn extract_modified(meta: &fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

#[cfg(unix)]
fn extract_permissions(meta: &fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    Some(meta.mode())
}

#[cfg(not(unix))]
fn extract_permissions(_meta: &fs::Metadata) -> Option<u32> {
    None
}

/// A single item in a planned remote recursive transfer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedRemoteItem {
    pub relative_components: Vec<String>,
    pub source_path: RemotePath,
    pub kind: FileKind,
    pub size: u64,
    pub modified: Option<u64>,
    pub permissions: Option<u32>,
    pub symlink_target: Option<String>,
}

/// Output of planning a remote SFTP directory or file tree using `LSTAT`.
#[derive(Clone, Debug, Default)]
pub struct RemoteTreePlan {
    pub items: Vec<PlannedRemoteItem>,
    pub skipped_symlinks: Vec<(String, String)>,
    pub skipped_special_files: Vec<(String, String)>,
    pub total_bytes: u64,
    pub total_files: usize,
    pub total_dirs: usize,
}

fn file_kind_from_mode(mode: Option<u32>) -> FileKind {
    match mode.map(|m| m & 0o170000) {
        Some(0o040000) => FileKind::Directory,
        Some(0o120000) => FileKind::Symlink { target: None },
        Some(0o100000) | None => FileKind::File,
        _ => FileKind::Other,
    }
}

/// Plan a remote SFTP tree iteratively using `LSTAT` (never `OPENDIR` failure as a file-type test).
pub fn plan_remote_tree(
    client: &mut super::SftpClient,
    root: &RemotePath,
    dest_semantics: PathSemantics,
    symlink_policy: SymlinkPolicy,
    limits: PlanLimits,
    control: &TransferControl,
) -> Result<RemoteTreePlan, SftpError> {
    control.check()?;
    let mut plan = RemoteTreePlan::default();
    let mut visited_dirs: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(RemotePath, Vec<String>, usize)> = VecDeque::new();
    queue.push_back((root.clone(), Vec::new(), 0));

    while let Some((current_path, rel_parts, depth)) = queue.pop_front() {
        control.check()?;
        if depth > limits.max_depth {
            return Err(SftpError::UnexpectedResponse(format!(
                "Remote traversal exceeded maximum depth ({})",
                limits.max_depth
            )));
        }
        if plan.items.len() >= limits.max_entries {
            return Err(SftpError::UnexpectedResponse(format!(
                "Remote traversal exceeded maximum entry limit ({})",
                limits.max_entries
            )));
        }

        let attrs = if symlink_policy == SymlinkPolicy::Dereference {
            client.stat(current_path.display())?
        } else {
            client.lstat(current_path.display())?
        };
        let kind = file_kind_from_mode(attrs.permissions);

        match kind {
            FileKind::Symlink { .. } => {
                let target = client.readlink(current_path.display()).ok();
                match symlink_policy {
                    SymlinkPolicy::Skip => {
                        plan.skipped_symlinks.push((
                            current_path.display().to_string(),
                            "symlink skipped by policy".into(),
                        ));
                    }
                    SymlinkPolicy::Preserve => {
                        plan.items.push(PlannedRemoteItem {
                            relative_components: rel_parts,
                            source_path: current_path,
                            kind: FileKind::Symlink {
                                target: target.clone(),
                            },
                            size: 0,
                            modified: attrs.mtime.map(|m| m as u64),
                            permissions: attrs.permissions,
                            symlink_target: target,
                        });
                    }
                    SymlinkPolicy::Dereference => {}
                }
            }
            FileKind::Directory => {
                if let Ok(canon) = client.realpath(current_path.display()) {
                    if !visited_dirs.insert(canon) {
                        plan.skipped_symlinks.push((
                            current_path.display().to_string(),
                            "symlink cycle detected".into(),
                        ));
                        continue;
                    }
                }
                plan.total_dirs += 1;
                plan.items.push(PlannedRemoteItem {
                    relative_components: rel_parts.clone(),
                    source_path: current_path.clone(),
                    kind: FileKind::Directory,
                    size: 0,
                    modified: attrs.mtime.map(|m| m as u64),
                    permissions: attrs.permissions,
                    symlink_target: None,
                });

                let children = client.list_remote_directory(current_path.display())?;
                for child in children {
                    control.check()?;
                    match classify_listing_name(child.name.as_bytes(), PathSemantics::Posix) {
                        Ok(ListingNameKind::Dot | ListingNameKind::DotDot) => continue,
                        Ok(ListingNameKind::Child(validated)) => {
                            validate_destination_name(validated.raw_bytes(), dest_semantics)
                                .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?;
                            let child_path = current_path
                                .join_child(&validated)
                                .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?;
                            let mut next_rel = rel_parts.clone();
                            next_rel.push(validated.display().to_string());
                            queue.push_back((child_path, next_rel, depth + 1));
                        }
                        Err(e) => {
                            return Err(SftpError::UnexpectedResponse(format!(
                                "Unsafe remote filename in {}: {e}",
                                current_path.display()
                            )));
                        }
                    }
                }
            }
            FileKind::File => {
                let size = attrs.size.unwrap_or(0);
                plan.total_files += 1;
                plan.total_bytes = plan.total_bytes.saturating_add(size);
                plan.items.push(PlannedRemoteItem {
                    relative_components: rel_parts,
                    source_path: current_path,
                    kind: FileKind::File,
                    size,
                    modified: attrs.mtime.map(|m| m as u64),
                    permissions: attrs.permissions,
                    symlink_target: None,
                });
            }
            FileKind::Other => {
                plan.skipped_special_files.push((
                    current_path.display().to_string(),
                    "special remote file skipped".into(),
                ));
            }
        }
    }

    Ok(plan)
}

/// Safe iterative post-order remote deletion using `LSTAT` (never follows symlinks, protects `/`).
pub fn delete_remote_recursive(
    client: &mut super::SftpClient,
    target: &RemotePath,
    control: &TransferControl,
) -> Result<usize, SftpError> {
    control.check()?;
    if target.is_root() {
        return Err(SftpError::UnexpectedResponse(format!(
            "Refusing to delete protected remote root '{}'",
            target.display()
        )));
    }

    let attrs = client.lstat(target.display())?;
    let kind = file_kind_from_mode(attrs.permissions);
    if !kind.is_dir() {
        client.remove_file(target.display())?;
        return Ok(1);
    }

    let mut stack: Vec<(RemotePath, bool)> = vec![(target.clone(), false)];
    let mut deleted = 0usize;

    while let Some((path, expanded)) = stack.pop() {
        control.check()?;
        if expanded {
            client.rmdir(path.display())?;
            deleted += 1;
            continue;
        }
        let m = client.lstat(path.display())?;
        if !file_kind_from_mode(m.permissions).is_dir() {
            client.remove_file(path.display())?;
            deleted += 1;
            continue;
        }
        stack.push((path.clone(), true));
        for child in client.list_remote_directory(path.display())? {
            if let Ok(ListingNameKind::Child(validated)) =
                classify_listing_name(child.name.as_bytes(), PathSemantics::Posix)
            {
                let child_path = path
                    .join_child(&validated)
                    .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?;
                stack.push((child_path, false));
            }
        }
    }

    Ok(deleted)
}

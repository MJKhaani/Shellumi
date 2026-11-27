//! High-level SFTP/local file operations: SHA-256 checksums, directory comparison,
//! deterministic dry-run/executed synchronization, cancellable recursive search,
//! multi-rename planning, and filesystem capacity reporting.

use super::path::{classify_listing_name, ListingNameKind, PathSemantics};
use super::planner::{plan_local_tree, PlanLimits, SymlinkPolicy};
use super::{FileEntry, FileKind, SftpClient, SftpError, TransferControl};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// Stream SHA-256 of a local file without loading the entire file into memory.
pub fn compute_local_sha256(path: &Path, control: &TransferControl) -> Result<String, SftpError> {
    control.check()?;
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        control.check()?;
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Stream SHA-256 of a remote file by reading chunks (without storing a full in-memory copy).
pub fn compute_remote_sha256(
    client: &mut SftpClient,
    remote_path: &str,
    control: &TransferControl,
) -> Result<String, SftpError> {
    use super::wire::{FileAttributes, SftpPacket, StatusCode, SSH_FXF_READ};

    control.check()?;
    let open_id = client.next_id();
    client.send_packet(&SftpPacket::Open {
        id: open_id,
        filename: remote_path.to_string(),
        pflags: SSH_FXF_READ,
        attrs: FileAttributes::new(),
    })?;
    let handle = client.expect_handle(open_id, remote_path)?;
    let mut hasher = Sha256::new();
    let mut offset = 0u64;

    let res = (|| -> Result<(), SftpError> {
        loop {
            control.check()?;
            let rid = client.next_id();
            client.send_packet(&SftpPacket::Read {
                id: rid,
                handle: handle.clone(),
                offset,
                len: 32 * 1024,
            })?;
            match client.read_response_for(rid)? {
                SftpPacket::Data { data, .. } if !data.is_empty() => {
                    hasher.update(&data);
                    offset = offset.saturating_add(data.len() as u64);
                }
                SftpPacket::Status {
                    code: StatusCode::Eof,
                    ..
                } => break,
                SftpPacket::Status { code, message, .. } => {
                    return Err(SftpError::Status { code, message });
                }
                other => return Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
            }
        }
        Ok(())
    })();

    let _ = client.close_handle(handle);
    res?;
    Ok(hex::encode(hasher.finalize()))
}

/// Deterministic multi-rename mapping entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiRenameItem {
    pub old_name: String,
    pub new_name: String,
}

/// Validate a batch of renames for duplicates, collisions with existing entries,
/// and unsafe names before executing any mutation.
pub fn plan_multi_rename(
    items: Vec<MultiRenameItem>,
    existing_names: &HashSet<String>,
    semantics: PathSemantics,
) -> Result<Vec<MultiRenameItem>, SftpError> {
    let mut seen_targets = HashSet::new();
    let old_set: HashSet<String> = items.iter().map(|i| i.old_name.clone()).collect();

    for item in &items {
        super::path::validate_destination_name(item.new_name.as_bytes(), semantics).map_err(
            |e| SftpError::UnexpectedResponse(format!("Invalid new name '{}': {e}", item.new_name)),
        )?;
        let key = if semantics == PathSemantics::Windows {
            item.new_name.to_lowercase()
        } else {
            item.new_name.clone()
        };
        if !seen_targets.insert(key) {
            return Err(SftpError::UnexpectedResponse(format!(
                "Duplicate target name '{}' in multi-rename batch",
                item.new_name
            )));
        }
        if item.new_name != item.old_name
            && existing_names.contains(&item.new_name)
            && !old_set.contains(&item.new_name)
        {
            return Err(SftpError::UnexpectedResponse(format!(
                "Target '{}' already exists in directory",
                item.new_name
            )));
        }
    }

    Ok(items)
}

/// Difference classification for directory comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffEntryStatus {
    OnlyLeft,
    OnlyRight,
    Different,
    Identical,
    TypeMismatch,
}

/// Result row from comparing two directory listings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryDiffRow {
    pub relative_name: String,
    pub status: DiffEntryStatus,
    pub left_size: Option<u64>,
    pub right_size: Option<u64>,
    pub left_modified: Option<u64>,
    pub right_modified: Option<u64>,
}

/// Compare two flat or relative entry maps deterministically.
pub fn compare_directory_entries(left: &[FileEntry], right: &[FileEntry]) -> Vec<DirectoryDiffRow> {
    let mut map: BTreeMap<String, (Option<&FileEntry>, Option<&FileEntry>)> = BTreeMap::new();
    for e in left {
        map.entry(e.name.clone()).or_default().0 = Some(e);
    }
    for e in right {
        map.entry(e.name.clone()).or_default().1 = Some(e);
    }

    map.into_iter()
        .map(|(name, (l, r))| match (l, r) {
            (Some(le), None) => DirectoryDiffRow {
                relative_name: name,
                status: DiffEntryStatus::OnlyLeft,
                left_size: Some(le.size),
                right_size: None,
                left_modified: le.modified,
                right_modified: None,
            },
            (None, Some(re)) => DirectoryDiffRow {
                relative_name: name,
                status: DiffEntryStatus::OnlyRight,
                left_size: None,
                right_size: Some(re.size),
                left_modified: None,
                right_modified: re.modified,
            },
            (Some(le), Some(re)) => {
                let status = if std::mem::discriminant(&le.kind) != std::mem::discriminant(&re.kind)
                {
                    DiffEntryStatus::TypeMismatch
                } else if le.kind.is_dir() {
                    DiffEntryStatus::Identical
                } else if le.size != re.size || le.modified != re.modified {
                    DiffEntryStatus::Different
                } else {
                    DiffEntryStatus::Identical
                };
                DirectoryDiffRow {
                    relative_name: name,
                    status,
                    left_size: Some(le.size),
                    right_size: Some(re.size),
                    left_modified: le.modified,
                    right_modified: re.modified,
                }
            }
            (None, None) => unreachable!(),
        })
        .collect()
}

/// Action planned by a deterministic directory synchronization dry-run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncActionKind {
    CopyToRight,
    UpdateRight,
    DeleteRight,
    ConflictRequiresDecision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPlanItem {
    pub relative_name: String,
    pub action: SyncActionKind,
}

/// Build a deterministic one-way (`left -> right`) synchronization plan.
/// Deletions are only emitted when `allow_deletions == true` (requiring explicit user opt-in).
pub fn plan_one_way_sync(diff: &[DirectoryDiffRow], allow_deletions: bool) -> Vec<SyncPlanItem> {
    let mut plan = Vec::new();
    for row in diff {
        match row.status {
            DiffEntryStatus::OnlyLeft => plan.push(SyncPlanItem {
                relative_name: row.relative_name.clone(),
                action: SyncActionKind::CopyToRight,
            }),
            DiffEntryStatus::Different => plan.push(SyncPlanItem {
                relative_name: row.relative_name.clone(),
                action: SyncActionKind::UpdateRight,
            }),
            DiffEntryStatus::TypeMismatch => plan.push(SyncPlanItem {
                relative_name: row.relative_name.clone(),
                action: SyncActionKind::ConflictRequiresDecision,
            }),
            DiffEntryStatus::OnlyRight if allow_deletions => plan.push(SyncPlanItem {
                relative_name: row.relative_name.clone(),
                action: SyncActionKind::DeleteRight,
            }),
            DiffEntryStatus::OnlyRight | DiffEntryStatus::Identical => {}
        }
    }
    plan
}

/// A search match from bounded recursive local search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub kind: FileKind,
    pub size: u64,
    pub line_preview: Option<(usize, String)>,
}

/// Cancellable bounded recursive local search by filename substring and optional text query.
pub fn search_local_recursive(
    root: &Path,
    name_query: &str,
    content_query: Option<&str>,
    max_results: usize,
    control: &TransferControl,
) -> Result<Vec<SearchMatch>, SftpError> {
    let plan = plan_local_tree(
        root,
        PathSemantics::local_native(),
        SymlinkPolicy::Skip,
        PlanLimits::default(),
        control,
        None,
    )?;

    let needle = name_query.to_lowercase();
    let mut results = Vec::new();
    const MAX_TEXT_SEARCH_BYTES: u64 = 1024 * 1024;

    for item in plan.items {
        control.check()?;
        if results.len() >= max_results {
            break;
        }
        let name = item
            .source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
            continue;
        }

        let mut line_preview = None;
        if let Some(cq) = content_query {
            if item.kind != FileKind::File || item.size > MAX_TEXT_SEARCH_BYTES {
                continue;
            }
            let Ok(bytes) = fs::read(&item.source_path) else {
                continue;
            };
            if bytes.contains(&0) {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            let cq_lower = cq.to_lowercase();
            let found = text
                .lines()
                .enumerate()
                .find(|(_, line)| line.to_lowercase().contains(&cq_lower));
            match found {
                Some((idx, line)) => {
                    line_preview = Some((idx + 1, line.chars().take(160).collect()));
                }
                None => continue,
            }
        }

        results.push(SearchMatch {
            path: item.source_path,
            kind: item.kind,
            size: item.size,
            line_preview,
        });
    }

    Ok(results)
}

/// Query local filesystem free/total capacity using native OS APIs (`statvfs` on Unix).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskCapacity {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[cfg(unix)]
pub fn local_disk_capacity(path: &Path) -> Option<DiskCapacity> {
    let output = crate::process::background_command("df")
        .arg("-k")
        .arg("-P")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let data_line = stdout.lines().nth(1)?;
    let cols: Vec<&str> = data_line.split_whitespace().collect();
    if cols.len() < 4 {
        return None;
    }
    let total_kb: u64 = cols[1].parse().ok()?;
    let avail_kb: u64 = cols[3].parse().ok()?;
    Some(DiskCapacity {
        total_bytes: total_kb.saturating_mul(1024),
        available_bytes: avail_kb.saturating_mul(1024),
    })
}

#[cfg(not(unix))]
pub fn local_disk_capacity(_path: &Path) -> Option<DiskCapacity> {
    None
}

/// Helper to validate a single child name for local/remote creation.
pub fn validate_new_entry_name(name: &str, semantics: PathSemantics) -> Result<(), SftpError> {
    match classify_listing_name(name.as_bytes(), semantics) {
        Ok(ListingNameKind::Child(_)) => Ok(()),
        Ok(ListingNameKind::Dot | ListingNameKind::DotDot) => Err(SftpError::UnexpectedResponse(
            "Entry name cannot be '.' or '..'".into(),
        )),
        Err(e) => Err(SftpError::UnexpectedResponse(e.to_string())),
    }
}

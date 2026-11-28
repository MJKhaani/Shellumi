//! Job-level cancellation, per-item commit guards, and policy-aware local staging.
use super::connection::VaultAuthGate;
use super::endpoint::VaultScope;
use super::{ConflictDetails, ConflictReason, FileKind, SftpError};
use ring::rand::SecureRandom;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

/// Policy governing final publication of a staged file to its destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationPolicy {
    /// Fail atomically if the destination already exists; never replace.
    NoReplace,
    /// Replace an existing destination only when explicitly confirmed.
    /// When `expected_len` or `expected_mtime` is `Some`, destination metadata is
    /// revalidated immediately before commit.
    ReplaceConfirmed {
        expected_len: Option<u64>,
        expected_mtime: Option<u64>,
    },
}

/// Monotonic job-level cancellation and optional vault-authorization binding.
///
/// Unlike [`TransferControl`], cancelling a `JobCancellation` always succeeds
/// even while an individual item is in the middle of its commit window: the
/// in-flight item finishes truthfully, and no subsequent item may begin or
/// commit.
#[derive(Clone, Debug, Default)]
pub struct JobCancellation {
    inner: Arc<JobCancellationInner>,
}

#[derive(Debug, Default)]
struct JobCancellationInner {
    cancelled: AtomicBool,
    vault_gate: Option<(VaultAuthGate, VaultScope)>,
}

impl JobCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_vault_gate(gate: VaultAuthGate, scope: VaultScope) -> Self {
        Self {
            inner: Arc::new(JobCancellationInner {
                cancelled: AtomicBool::new(false),
                vault_gate: Some((gate, scope)),
            }),
        }
    }

    /// Request cancellation for all future items in this job. Always returns `true`.
    pub fn cancel(&self) -> bool {
        self.inner.cancelled.store(true, Ordering::Release);
        true
    }

    pub fn is_cancelled(&self) -> bool {
        if self.inner.cancelled.load(Ordering::Acquire) {
            return true;
        }
        if let Some((ref gate, ref scope)) = self.inner.vault_gate {
            return !gate.is_authorized(scope);
        }
        false
    }

    pub fn check(&self) -> Result<(), SftpError> {
        if self.is_cancelled() {
            Err(SftpError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Create an independent per-item [`TransferControl`] linked to this job's
    /// cancellation and vault-authorization gate.
    pub fn new_item_control(&self) -> Result<TransferControl, SftpError> {
        self.check()?;
        Ok(TransferControl(Arc::new(TransferControlInner {
            state: AtomicU8::new(0),
            parent: Some(self.clone()),
        })))
    }
}

/// Per-item one-shot cancellation and commit guard. Once publication begins
/// (`begin_commit`), cancellation on this item is refused so the UI cannot
/// label a committed item cancelled.
#[derive(Clone, Debug, Default)]
pub struct TransferControl(Arc<TransferControlInner>);

#[derive(Debug, Default)]
struct TransferControlInner {
    state: AtomicU8,
    parent: Option<JobCancellation>,
}

impl TransferControl {
    pub fn cancel(&self) -> bool {
        match self
            .0
            .state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(1) => true,
            Err(_) => false,
        }
    }
    pub fn is_cancelled(&self) -> bool {
        if self.0.state.load(Ordering::Acquire) == 1 {
            return true;
        }
        self.0
            .parent
            .as_ref()
            .is_some_and(JobCancellation::is_cancelled)
    }
    pub fn check(&self) -> Result<(), SftpError> {
        if self.is_cancelled() {
            Err(SftpError::Cancelled)
        } else {
            Ok(())
        }
    }
    pub(super) fn begin_commit(&self) -> Result<(), SftpError> {
        if self
            .0
            .parent
            .as_ref()
            .is_some_and(JobCancellation::is_cancelled)
        {
            return Err(SftpError::Cancelled);
        }
        self.0
            .state
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| SftpError::Cancelled)
    }
}
pub(super) fn suffix() -> Result<String, SftpError> {
    let mut bytes = [0u8; 16];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| std::io::Error::other("Could not generate a secure transfer filename"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
pub(super) struct LocalStaging {
    pub file: Option<File>,
    path: PathBuf,
    published: bool,
}
impl LocalStaging {
    pub fn new(destination: &Path) -> Result<Self, SftpError> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        for _ in 0..8 {
            let path = parent.join(format!(".shellumi-transfer-{}.part", suffix()?));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        file: Some(file),
                        path,
                        published: false,
                    })
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(std::io::Error::other("Could not reserve a unique transfer file").into())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn publish(
        &mut self,
        destination: &Path,
        control: &TransferControl,
    ) -> Result<(), SftpError> {
        self.publish_with_policy(
            destination,
            control,
            &PublicationPolicy::ReplaceConfirmed {
                expected_len: None,
                expected_mtime: None,
            },
        )
    }
    pub fn publish_with_policy(
        &mut self,
        destination: &Path,
        control: &TransferControl,
        policy: &PublicationPolicy,
    ) -> Result<(), SftpError> {
        if let Some(file) = self.file.take() {
            file.sync_all()?;
        }
        match policy {
            PublicationPolicy::NoReplace => {
                control.begin_commit()?;
                // `hard_link` is atomic and fails with AlreadyExists on both Unix and Windows
                // without following symlinks at `destination` or replacing existing files.
                std::fs::hard_link(&self.path, destination)?;
                let _ = std::fs::remove_file(&self.path);
                self.published = true;
                Ok(())
            }
            PublicationPolicy::ReplaceConfirmed {
                expected_len,
                expected_mtime,
            } => {
                if expected_len.is_some() || expected_mtime.is_some() {
                    match std::fs::symlink_metadata(destination) {
                        Ok(meta) => {
                            if meta.file_type().is_symlink() || !meta.file_type().is_file() {
                                return Err(SftpError::ConflictRequired(Box::new(
                                    ConflictDetails {
                                        source_path: self.path.display().to_string(),
                                        destination_path: destination.display().to_string(),
                                        relative_path: destination
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                            .unwrap_or_default(),
                                        item_kind: if meta.file_type().is_symlink() {
                                            FileKind::Symlink { target: None }
                                        } else {
                                            FileKind::Directory
                                        },
                                        source_size: std::fs::metadata(&self.path)
                                            .ok()
                                            .map(|m| m.len()),
                                        destination_size: Some(meta.len()),
                                        destination_mtime: meta
                                            .modified()
                                            .ok()
                                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                                            .map(|d| d.as_secs()),
                                        reason: ConflictReason::TypeMismatch,
                                    },
                                )));
                            }
                            let actual_mtime = meta
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                                .map(|d| d.as_secs());
                            if expected_len.is_some_and(|exp| exp != meta.len())
                                || expected_mtime.is_some_and(|exp| Some(exp) != actual_mtime)
                            {
                                return Err(SftpError::ConflictRequired(Box::new(
                                    ConflictDetails {
                                        source_path: self.path.display().to_string(),
                                        destination_path: destination.display().to_string(),
                                        relative_path: destination
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                            .unwrap_or_default(),
                                        item_kind: FileKind::File,
                                        source_size: std::fs::metadata(&self.path)
                                            .ok()
                                            .map(|m| m.len()),
                                        destination_size: Some(meta.len()),
                                        destination_mtime: actual_mtime,
                                        reason: ConflictReason::TargetChanged,
                                    },
                                )));
                            }
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                            return Err(SftpError::ConflictRequired(Box::new(ConflictDetails {
                                source_path: self.path.display().to_string(),
                                destination_path: destination.display().to_string(),
                                relative_path: destination
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                                item_kind: FileKind::File,
                                source_size: std::fs::metadata(&self.path).ok().map(|m| m.len()),
                                destination_size: None,
                                destination_mtime: None,
                                reason: ConflictReason::TargetDisappeared,
                            })));
                        }
                        Err(err) => return Err(err.into()),
                    }
                }
                control.begin_commit()?;
                std::fs::rename(&self.path, destination)?;
                self.published = true;
                Ok(())
            }
        }
    }
}
impl Drop for LocalStaging {
    fn drop(&mut self) {
        self.file.take();
        if !self.published {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn staging_failure_and_cancellation_preserve_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("existing");
        std::fs::write(&destination, b"original").unwrap();
        {
            let mut stage = LocalStaging::new(&destination).unwrap();
            stage.file.as_mut().unwrap().write_all(b"partial").unwrap();
            let token = TransferControl::default();
            assert!(token.cancel());
            assert!(matches!(
                stage.publish(&destination, &token),
                Err(SftpError::Cancelled)
            ));
            assert_eq!(std::fs::read(&destination).unwrap(), b"original");
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn commit_and_cancellation_are_mutually_exclusive() {
        for _ in 0..128 {
            let token = TransferControl::default();
            let other = token.clone();
            let cancel = std::thread::spawn(move || other.cancel());
            let committed = token.begin_commit().is_ok();
            assert_ne!(committed, cancel.join().unwrap());
        }
    }
    #[cfg(unix)]
    #[test]
    fn publishing_replaces_a_symlink_without_writing_through_it() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("other");
        let destination = dir.path().join("destination");
        std::fs::write(&other, b"untouched").unwrap();
        std::os::unix::fs::symlink(&other, &destination).unwrap();
        let mut stage = LocalStaging::new(&destination).unwrap();
        stage
            .file
            .as_mut()
            .unwrap()
            .write_all(b"completed")
            .unwrap();
        let token = TransferControl::default();
        stage.publish(&destination, &token).unwrap();
        assert!(
            !token.cancel(),
            "Publication cannot be relabelled cancelled"
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"completed");
        assert_eq!(std::fs::read(&other).unwrap(), b"untouched");
        assert!(!std::fs::symlink_metadata(&destination)
            .unwrap()
            .is_symlink());
    }
}

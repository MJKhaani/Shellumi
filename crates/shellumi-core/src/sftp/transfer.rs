//! One-shot transfer cancellation and same-directory local staging.
use super::SftpError;
use ring::rand::SecureRandom;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Cancellation also interrupts live SSH pipe waits. Once publication begins,
/// cancellation is refused so the UI cannot label a committed transfer cancelled.
#[derive(Clone, Debug, Default)]
pub struct TransferControl(Arc<AtomicU8>);
impl TransferControl {
    pub fn cancel(&self) -> bool {
        match self
            .0
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(1) => true,
            Err(_) => false,
        }
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire) == 1
    }
    pub fn check(&self) -> Result<(), SftpError> {
        if self.is_cancelled() {
            Err(SftpError::Cancelled)
        } else {
            Ok(())
        }
    }
    pub(super) fn begin_commit(&self) -> Result<(), SftpError> {
        self.0
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
    pub fn publish(
        &mut self,
        destination: &Path,
        control: &TransferControl,
    ) -> Result<(), SftpError> {
        if let Some(file) = self.file.take() {
            file.sync_all()?;
        }
        control.begin_commit()?;
        std::fs::rename(&self.path, destination)?;
        self.published = true;
        Ok(())
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

//! Authentic RDP clipboard redirection (CLIPRDR) and file transfer backend.
//!
//! Provides bidirectional text clipboard synchronization and streaming file transfers
//! between host machine and remote RDP desktop sessions using Microsoft MS-RDPECLIP specification.

use ironrdp_client::rdp::RdpInputEvent;
use ironrdp_cliprdr::backend::{ClipboardMessage, CliprdrBackend};
use ironrdp_cliprdr::pdu::{
    ClipboardFileAttributes, ClipboardFormat, ClipboardFormatId, ClipboardFormatName,
    ClipboardGeneralCapabilityFlags, FileContentsFlags, FileContentsRequest, FileContentsResponse,
    FileDescriptor, FormatDataRequest, FormatDataResponse, LockDataId, OwnedFormatDataResponse,
};
use ironrdp_core::AsAny;
use shellumi_remote_desktop::{SessionEvent, TransferDirection};
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Default chunk size for RDP file transfers (64 KiB).
pub const DEFAULT_FILE_CHUNK_SIZE: u32 = 64 * 1024;
/// Hard upper bound for a single FileContentsRequest/Response chunk (1 MiB).
pub const MAX_FILE_CHUNK_SIZE: u32 = 1024 * 1024;
/// Maximum total bytes accepted in a single remote file offer (4 GiB).
pub const MAX_TOTAL_TRANSFER_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Maximum text clipboard payload size (16 MiB).
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 16 * 1024 * 1024;

static STAGING_DIR_SEQ: AtomicU64 = AtomicU64::new(1);
static TRANSFER_ID_SEQ: AtomicU64 = AtomicU64::new(1);

/// Create a private per-session/transfer staging directory with 0700 permissions on Unix.
/// Uses randomized suffix to prevent predictable path collisions; never falls back to a shared directory.
pub fn create_private_staging_dir() -> std::io::Result<PathBuf> {
    let seq = STAGING_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("shellumi-rdp-{pid}-{seq}-{nanos:x}"));
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&dir)?;
    Ok(dir)
}

/// Validate and sanitize an untrusted remote filename.
///
/// Rejects path separators (`/` and `\`), traversal (`..`, `.`), drive/ADS colons (`:`),
/// NUL/control characters, and Windows reserved device names.
pub fn sanitize_remote_filename(raw: &str) -> Result<String, String> {
    if raw.is_empty() {
        return Err("Empty filename".into());
    }
    if raw.chars().any(|c| c == '\0' || (c as u32) < 0x20) {
        return Err("Filename contains control characters".into());
    }
    if raw.contains('/') || raw.contains('\\') {
        return Err("Filename contains path separator".into());
    }
    if raw.contains(':') {
        return Err("Filename contains colon / drive / ADS syntax".into());
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err("Invalid filename".into());
    }
    let stem = trimmed
        .split('.')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_uppercase();
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        return Err(format!("Reserved device name: {trimmed}"));
    }
    Ok(trimmed.to_string())
}

/// Expected response kind for an in-flight `FileContentsRequest`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedResponseKind {
    Size,
    Range { expected_offset: u64, max_len: u32 },
}

/// Staged file descriptor and local path for host-to-remote file transfers.
#[derive(Debug, Clone)]
pub struct StagedFile {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub descriptor: FileDescriptor,
}

/// Remote file descriptor and download progress for remote-to-host file transfers.
#[derive(Debug)]
pub struct RemoteFile {
    pub index: usize,
    pub name: String,
    pub size: u64,
    pub downloaded_bytes: u64,
    pub local_path: PathBuf,
    pub handle: Option<std::fs::File>,
}

/// Internal state for Shellumi CLIPRDR backend.
#[derive(Debug)]
pub struct CliprdrInner {
    pub local_text: Option<String>,
    pub staged_files: Vec<StagedFile>,
    pub remote_files: Vec<RemoteFile>,
    pub active_transfer_id: Option<u64>,
    pub active_download_stream_id: Option<u32>,
    pub expected_response: Option<ExpectedResponseKind>,
    pub active_download_index: usize,
    pub active_lock_id: Option<u32>,
    pub stream_id_counter: u32,
    pub negotiated_capabilities: ClipboardGeneralCapabilityFlags,
    pub is_ready: bool,
    pub allow_send: bool,
    pub allow_receive: bool,
}

impl CliprdrInner {
    fn new() -> Self {
        Self {
            local_text: None,
            staged_files: Vec::new(),
            remote_files: Vec::new(),
            active_transfer_id: None,
            active_download_stream_id: None,
            expected_response: None,
            active_download_index: 0,
            active_lock_id: None,
            stream_id_counter: 1,
            negotiated_capabilities: ClipboardGeneralCapabilityFlags::empty(),
            is_ready: false,
            allow_send: true,
            allow_receive: true,
        }
    }

    fn next_stream_id(&mut self) -> u32 {
        let id = self.stream_id_counter;
        self.stream_id_counter = self.stream_id_counter.wrapping_add(1).max(1);
        id
    }
}

/// Thread-safe cross-platform CLIPRDR backend for Shellumi.
#[derive(Clone, Debug)]
pub struct ShellumiCliprdrBackend {
    pub inner: Arc<Mutex<CliprdrInner>>,
    pub proxy: Arc<Mutex<Option<mpsc::UnboundedSender<RdpInputEvent>>>>,
    pub worker_tx: async_channel::Sender<SessionEvent>,
    pub temp_dir: String,
}

impl AsAny for ShellumiCliprdrBackend {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
        self
    }
}

impl ShellumiCliprdrBackend {
    /// Create a new clipboard backend instance.
    pub fn new(
        proxy: Arc<Mutex<Option<mpsc::UnboundedSender<RdpInputEvent>>>>,
        worker_tx: async_channel::Sender<SessionEvent>,
        temp_dir: impl Into<String>,
    ) -> Self {
        let temp_dir_str = temp_dir.into();
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        let _ = builder.create(&temp_dir_str);

        Self {
            inner: Arc::new(Mutex::new(CliprdrInner::new())),
            proxy,
            worker_tx,
            temp_dir: temp_dir_str,
        }
    }

    /// Update clipboard direction permissions at runtime (e.g. view-only or user policy).
    pub fn set_policy(&self, allow_send: bool, allow_receive: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.allow_send = allow_send;
        inner.allow_receive = allow_receive;
        if !allow_send {
            inner.local_text = None;
            inner.staged_files.clear();
        }
    }

    /// Dispatch a clipboard message back to IronRDP runner.
    pub fn send_message(&self, message: ClipboardMessage) {
        if let Some(sender) = self.proxy.lock().unwrap().as_ref() {
            let _ = sender.send(RdpInputEvent::Clipboard(message));
        }
    }

    /// Stage text to copy from host to remote machine.
    pub fn set_local_text(&self, text: String) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.local_text = Some(text);
        }
        self.send_message(ClipboardMessage::SendInitiateCopy(vec![
            ClipboardFormat::new(ClipboardFormatId::CF_UNICODETEXT),
        ]));
    }

    /// Retrieve currently staged local text.
    pub fn get_local_text(&self) -> Option<String> {
        self.inner.lock().unwrap().local_text.clone()
    }

    /// Stage files on the host to transfer to the remote Windows machine.
    pub fn stage_files(&self, paths: &[PathBuf]) -> Result<Vec<FileDescriptor>, String> {
        let mut descriptors = Vec::new();
        let mut staged = Vec::new();

        for path in paths {
            if !path.is_file() {
                continue;
            }
            let metadata = std::fs::metadata(path)
                .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
            let file_size = metadata.len();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();

            let desc = FileDescriptor::new(file_name.clone())
                .with_file_size(file_size)
                .with_attributes(ClipboardFileAttributes::NORMAL);

            descriptors.push(desc.clone());
            staged.push(StagedFile {
                path: path.clone(),
                name: file_name,
                size: file_size,
                descriptor: desc,
            });
        }

        if descriptors.is_empty() {
            return Err("No valid files to stage".into());
        }

        {
            let mut inner = self.inner.lock().unwrap();
            inner.staged_files = staged;
        }

        self.send_message(ClipboardMessage::SendInitiateFileCopy(descriptors.clone()));
        Ok(descriptors)
    }

    /// Advance downloading of remote files or emit completion event when all files are downloaded.
    fn advance_download_or_finish(&self) {
        let mut inner = self.inner.lock().unwrap();
        let idx = inner.active_download_index;
        if let Some(f) = inner.remote_files.get_mut(idx) {
            if let Some(mut handle) = f.handle.take() {
                let _ = handle.flush();
            }
        }
        inner.active_download_index += 1;

        if inner.active_download_index < inner.remote_files.len() {
            let next_idx = inner.active_download_index;
            let stream_id = inner.next_stream_id();
            inner.active_download_stream_id = Some(stream_id);
            inner.expected_response = Some(ExpectedResponseKind::Size);
            let req = FileContentsRequest {
                stream_id,
                index: next_idx as i32,
                flags: FileContentsFlags::SIZE,
                position: 0,
                requested_size: 8,
                data_id: inner.active_lock_id,
            };
            drop(inner);
            self.send_message(ClipboardMessage::SendFileContentsRequest(req));
        } else {
            let file_names: Vec<String> =
                inner.remote_files.iter().map(|f| f.name.clone()).collect();
            let transfer_id = inner.active_transfer_id.take().unwrap_or(0);
            let target_dir = self.temp_dir.clone();
            inner.active_download_stream_id = None;
            inner.expected_response = None;
            drop(inner);

            let _ = self.worker_tx.try_send(SessionEvent::TransferCompleted {
                transfer_id,
                direction: TransferDirection::Download,
                files: file_names.clone(),
                target_dir: Some(target_dir.clone()),
            });
            let _ = self.worker_tx.try_send(SessionEvent::ServerFilesReceived {
                files: file_names,
                target_dir,
            });
        }
    }

    fn fail_active_download(&self, reason: String) {
        let mut inner = self.inner.lock().unwrap();
        let transfer_id = inner.active_transfer_id.take().unwrap_or(0);
        inner.active_download_stream_id = None;
        inner.expected_response = None;
        for f in &mut inner.remote_files {
            f.handle.take();
            if f.downloaded_bytes < f.size || f.local_path.exists() {
                let _ = std::fs::remove_file(&f.local_path);
            }
        }
        inner.remote_files.clear();
        drop(inner);

        let _ = self.worker_tx.try_send(SessionEvent::TransferFailed {
            transfer_id,
            direction: TransferDirection::Download,
            reason,
        });
    }
}

impl CliprdrBackend for ShellumiCliprdrBackend {
    fn temporary_directory(&self) -> &str {
        &self.temp_dir
    }

    fn client_capabilities(&self) -> ClipboardGeneralCapabilityFlags {
        ClipboardGeneralCapabilityFlags::USE_LONG_FORMAT_NAMES
            | ClipboardGeneralCapabilityFlags::STREAM_FILECLIP_ENABLED
            | ClipboardGeneralCapabilityFlags::FILECLIP_NO_FILE_PATHS
            | ClipboardGeneralCapabilityFlags::CAN_LOCK_CLIPDATA
            | ClipboardGeneralCapabilityFlags::HUGE_FILE_SUPPORT_ENABLED
    }

    fn on_ready(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        inner.is_ready = true;
        tracing::debug!("CLIPRDR channel is ready for clipboard operations");
    }

    fn on_request_format_list(&mut self) {
        let (has_text, descriptors) = {
            let inner = self.inner.lock().unwrap();
            let has_text = inner.local_text.is_some();
            let descriptors: Vec<FileDescriptor> = inner
                .staged_files
                .iter()
                .map(|f| f.descriptor.clone())
                .collect();
            (has_text, descriptors)
        };

        if !descriptors.is_empty() {
            self.send_message(ClipboardMessage::SendInitiateFileCopy(descriptors));
        } else if has_text {
            self.send_message(ClipboardMessage::SendInitiateCopy(vec![
                ClipboardFormat::new(ClipboardFormatId::CF_UNICODETEXT),
            ]));
        }
    }

    fn on_format_list_response(&mut self, ok: bool) {
        tracing::debug!(ok, "CLIPRDR format list response received");
    }

    fn on_process_negotiated_capabilities(
        &mut self,
        capabilities: ClipboardGeneralCapabilityFlags,
    ) {
        let mut inner = self.inner.lock().unwrap();
        inner.negotiated_capabilities = capabilities;
        tracing::debug!(
            ?capabilities,
            "CLIPRDR capabilities negotiated with remote RDP server"
        );
    }

    fn on_remote_copy(&mut self, available_formats: &[ClipboardFormat]) {
        for format in available_formats {
            if let Some(name) = &format.name {
                if name.value() == ClipboardFormatName::FILE_LIST.value() {
                    tracing::debug!(format_id = ?format.id, "Remote copied files (FileGroupDescriptorW); requesting descriptor list");
                    self.send_message(ClipboardMessage::SendInitiatePaste(format.id));
                    return;
                }
            }
        }

        for format in available_formats {
            if format.id == ClipboardFormatId::CF_UNICODETEXT {
                tracing::debug!("Remote copied text; requesting CF_UNICODETEXT payload");
                self.send_message(ClipboardMessage::SendInitiatePaste(
                    ClipboardFormatId::CF_UNICODETEXT,
                ));
                return;
            }
        }
    }

    fn on_format_data_request(&mut self, request: FormatDataRequest) {
        if request.format == ClipboardFormatId::CF_UNICODETEXT {
            let inner = self.inner.lock().unwrap();
            if let Some(text) = inner.local_text.as_ref() {
                self.send_message(ClipboardMessage::SendFormatData(
                    OwnedFormatDataResponse::new_unicode_string(text),
                ));
                return;
            }
        } else if request.format == ClipboardFormatId::CF_TEXT {
            let inner = self.inner.lock().unwrap();
            if let Some(text) = inner.local_text.as_ref() {
                self.send_message(ClipboardMessage::SendFormatData(
                    OwnedFormatDataResponse::new_string(text),
                ));
                return;
            }
        }

        self.send_message(ClipboardMessage::SendFormatData(
            OwnedFormatDataResponse::new_error(),
        ));
    }

    fn on_format_data_response(&mut self, response: FormatDataResponse<'_>) {
        if response.is_error() {
            tracing::warn!("Remote format data response indicated an error");
            return;
        }
        if !self.inner.lock().unwrap().allow_receive {
            return;
        }

        if let Ok(text) = response.to_unicode_string() {
            let cleaned = text.trim_matches('\0').to_string();
            if cleaned.len() <= MAX_CLIPBOARD_TEXT_BYTES {
                tracing::debug!(
                    len = cleaned.len(),
                    "Received unicode text clipboard from remote"
                );
                let _ = self
                    .worker_tx
                    .try_send(SessionEvent::ServerClipboard(cleaned));
            } else {
                tracing::warn!(
                    len = cleaned.len(),
                    "Rejected oversized remote clipboard text"
                );
            }
        }
    }

    fn on_file_contents_request(&mut self, request: FileContentsRequest) {
        let (file_path, file_size, allow_send) = {
            let inner = self.inner.lock().unwrap();
            if !inner.allow_send {
                drop(inner);
                self.send_message(ClipboardMessage::SendFileContentsResponse(
                    FileContentsResponse::new_error(request.stream_id),
                ));
                return;
            }
            if request.index < 0 || (request.index as usize) >= inner.staged_files.len() {
                tracing::error!(
                    index = request.index,
                    "Invalid file index in FileContentsRequest"
                );
                drop(inner);
                self.send_message(ClipboardMessage::SendFileContentsResponse(
                    FileContentsResponse::new_error(request.stream_id),
                ));
                return;
            }
            let staged = &inner.staged_files[request.index as usize];
            (staged.path.clone(), staged.size, inner.allow_send)
        };
        let _ = allow_send;

        if request.flags.contains(FileContentsFlags::SIZE) {
            self.send_message(ClipboardMessage::SendFileContentsResponse(
                FileContentsResponse::new_size_response(request.stream_id, file_size),
            ));
            return;
        }

        if request.flags.contains(FileContentsFlags::RANGE) {
            if request.position > file_size {
                self.send_message(ClipboardMessage::SendFileContentsResponse(
                    FileContentsResponse::new_error(request.stream_id),
                ));
                return;
            }
            let capped_len = request.requested_size.min(MAX_FILE_CHUNK_SIZE) as usize;
            let mut file = match std::fs::File::open(&file_path) {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("Failed to open staged file {}: {e}", file_path.display());
                    self.send_message(ClipboardMessage::SendFileContentsResponse(
                        FileContentsResponse::new_error(request.stream_id),
                    ));
                    return;
                }
            };

            if let Err(e) = file.seek(SeekFrom::Start(request.position)) {
                tracing::error!("Failed to seek in staged file: {e}");
                self.send_message(ClipboardMessage::SendFileContentsResponse(
                    FileContentsResponse::new_error(request.stream_id),
                ));
                return;
            }

            let mut buffer = vec![0u8; capped_len];
            match file.read(&mut buffer) {
                Ok(read_bytes) => {
                    buffer.truncate(read_bytes);
                    self.send_message(ClipboardMessage::SendFileContentsResponse(
                        FileContentsResponse::new_data_response(request.stream_id, buffer),
                    ));
                }
                Err(e) => {
                    tracing::error!("Failed to read staged file: {e}");
                    self.send_message(ClipboardMessage::SendFileContentsResponse(
                        FileContentsResponse::new_error(request.stream_id),
                    ));
                }
            }
            return;
        }

        self.send_message(ClipboardMessage::SendFileContentsResponse(
            FileContentsResponse::new_error(request.stream_id),
        ));
    }

    fn on_file_contents_response(&mut self, response: FileContentsResponse<'_>) {
        let stream_id = response.stream_id();
        let mut inner = self.inner.lock().unwrap();

        if inner.active_download_stream_id != Some(stream_id) {
            return;
        }

        if response.is_error() {
            tracing::warn!(stream_id, "FileContentsResponse error received");
            drop(inner);
            self.fail_active_download("Remote host returned file contents error".into());
            return;
        }

        let current_idx = inner.active_download_index;
        if current_idx >= inner.remote_files.len() {
            return;
        }

        let expected = match inner.expected_response {
            Some(e) => e,
            None => {
                drop(inner);
                self.fail_active_download("Unsolicited FileContentsResponse".into());
                return;
            }
        };

        let data = response.data();
        let transfer_id = inner.active_transfer_id.unwrap_or(0);

        match expected {
            ExpectedResponseKind::Size => {
                let Ok(size) = response.data_as_size() else {
                    drop(inner);
                    self.fail_active_download("Malformed SIZE response".into());
                    return;
                };
                if size > MAX_TOTAL_TRANSFER_BYTES {
                    drop(inner);
                    self.fail_active_download("Remote file exceeds transfer size limit".into());
                    return;
                }
                inner.remote_files[current_idx].size = size;
                let local_path = inner.remote_files[current_idx].local_path.clone();

                let mut opts = std::fs::OpenOptions::new();
                opts.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    opts.mode(0o600);
                }
                let mut file = match opts.open(&local_path) {
                    Ok(f) => f,
                    Err(e) => {
                        drop(inner);
                        self.fail_active_download(format!("Failed to create staging file: {e}"));
                        return;
                    }
                };

                if size == 0 {
                    if let Err(e) = file.flush() {
                        drop(inner);
                        self.fail_active_download(format!("Failed to flush zero-byte file: {e}"));
                        return;
                    }
                    drop(file);
                    drop(inner);
                    self.advance_download_or_finish();
                    return;
                }

                inner.remote_files[current_idx].handle = Some(file);
                let chunk_size = std::cmp::min(size, DEFAULT_FILE_CHUNK_SIZE as u64) as u32;
                inner.expected_response = Some(ExpectedResponseKind::Range {
                    expected_offset: 0,
                    max_len: chunk_size,
                });
                let req = FileContentsRequest {
                    stream_id,
                    index: current_idx as i32,
                    flags: FileContentsFlags::RANGE,
                    position: 0,
                    requested_size: chunk_size,
                    data_id: inner.active_lock_id,
                };
                drop(inner);
                self.send_message(ClipboardMessage::SendFileContentsRequest(req));
            }
            ExpectedResponseKind::Range {
                expected_offset,
                max_len,
            } => {
                if data.is_empty() {
                    drop(inner);
                    self.fail_active_download("Unexpected empty RANGE response before EOF".into());
                    return;
                }
                if data.len() > max_len as usize {
                    drop(inner);
                    self.fail_active_download("Oversized RANGE response".into());
                    return;
                }
                let total_size = inner.remote_files[current_idx].size;
                let Some(next_downloaded) = expected_offset.checked_add(data.len() as u64) else {
                    drop(inner);
                    self.fail_active_download("Offset overflow in file transfer".into());
                    return;
                };
                if next_downloaded > total_size {
                    drop(inner);
                    self.fail_active_download("Remote sent more bytes than advertised size".into());
                    return;
                }

                let Some(handle) = inner.remote_files[current_idx].handle.as_mut() else {
                    drop(inner);
                    self.fail_active_download("Missing open staging file handle".into());
                    return;
                };
                if let Err(e) = handle.write_all(data) {
                    drop(inner);
                    self.fail_active_download(format!("Failed to write staging file: {e}"));
                    return;
                }

                inner.remote_files[current_idx].downloaded_bytes = next_downloaded;
                let file_name = inner.remote_files[current_idx].name.clone();
                let _ = self.worker_tx.try_send(SessionEvent::TransferProgress {
                    transfer_id,
                    file_index: current_idx,
                    file_name,
                    bytes_transferred: next_downloaded,
                    file_total_bytes: total_size,
                });

                if next_downloaded >= total_size {
                    drop(inner);
                    self.advance_download_or_finish();
                } else {
                    let remaining = total_size - next_downloaded;
                    let next_chunk =
                        std::cmp::min(remaining, DEFAULT_FILE_CHUNK_SIZE as u64) as u32;
                    inner.expected_response = Some(ExpectedResponseKind::Range {
                        expected_offset: next_downloaded,
                        max_len: next_chunk,
                    });
                    let req = FileContentsRequest {
                        stream_id,
                        index: current_idx as i32,
                        flags: FileContentsFlags::RANGE,
                        position: next_downloaded,
                        requested_size: next_chunk,
                        data_id: inner.active_lock_id,
                    };
                    drop(inner);
                    self.send_message(ClipboardMessage::SendFileContentsRequest(req));
                }
            }
        }
    }

    fn on_lock(&mut self, data_id: LockDataId) {
        let mut inner = self.inner.lock().unwrap();
        inner.active_lock_id = Some(data_id.0);
    }

    fn on_unlock(&mut self, _data_id: LockDataId) {
        let mut inner = self.inner.lock().unwrap();
        inner.active_lock_id = None;
    }

    fn on_remote_file_list(&mut self, files: &[FileDescriptor], clip_data_id: Option<u32>) {
        if files.is_empty() {
            return;
        }
        if !self.inner.lock().unwrap().allow_receive {
            return;
        }

        let target_dir = PathBuf::from(&self.temp_dir);
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        if builder.create(&target_dir).is_err() {
            return;
        }

        let mut seen_names: HashSet<String> = HashSet::new();
        let mut remote_files = Vec::new();
        let mut total_advertised: u64 = 0;

        for (idx, fd) in files.iter().enumerate() {
            let base_name = match sanitize_remote_filename(&fd.name) {
                Ok(n) => n,
                Err(reason) => {
                    tracing::warn!(reason = %reason, "Rejected unsafe remote filename");
                    let transfer_id = TRANSFER_ID_SEQ.fetch_add(1, Ordering::Relaxed);
                    let _ = self.worker_tx.try_send(SessionEvent::TransferFailed {
                        transfer_id,
                        direction: TransferDirection::Download,
                        reason,
                    });
                    return;
                }
            };

            let mut candidate = base_name.clone();
            let mut suffix = 1usize;
            while !seen_names.insert(candidate.to_ascii_lowercase()) {
                candidate = format!("{base_name}_{suffix}");
                suffix += 1;
            }

            let advertised_size = fd.file_size.unwrap_or(0);
            total_advertised = total_advertised.saturating_add(advertised_size);
            if total_advertised > MAX_TOTAL_TRANSFER_BYTES {
                let transfer_id = TRANSFER_ID_SEQ.fetch_add(1, Ordering::Relaxed);
                let _ = self.worker_tx.try_send(SessionEvent::TransferFailed {
                    transfer_id,
                    direction: TransferDirection::Download,
                    reason: "Total advertised transfer exceeds quota".into(),
                });
                return;
            }

            let local_path = target_dir.join(&candidate);
            remote_files.push(RemoteFile {
                index: idx,
                name: candidate,
                size: advertised_size,
                downloaded_bytes: 0,
                local_path,
                handle: None,
            });
        }

        let transfer_id = TRANSFER_ID_SEQ.fetch_add(1, Ordering::Relaxed);
        let _ = self.worker_tx.try_send(SessionEvent::TransferStarted {
            transfer_id,
            direction: TransferDirection::Download,
            file_count: remote_files.len(),
            total_bytes: total_advertised,
        });

        let mut inner = self.inner.lock().unwrap();
        inner.remote_files = remote_files;
        inner.active_transfer_id = Some(transfer_id);
        inner.active_lock_id = clip_data_id;
        inner.active_download_index = 0;

        let first_file_index = inner.remote_files.first().map(|f| f.index);
        if let Some(index) = first_file_index {
            let stream_id = inner.next_stream_id();
            inner.active_download_stream_id = Some(stream_id);
            inner.expected_response = Some(ExpectedResponseKind::Size);
            let req = FileContentsRequest {
                stream_id,
                index: index as i32,
                flags: FileContentsFlags::SIZE,
                position: 0,
                requested_size: 8,
                data_id: clip_data_id,
            };
            drop(inner);
            self.send_message(ClipboardMessage::SendFileContentsRequest(req));
        }
    }
}

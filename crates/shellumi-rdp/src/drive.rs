//! Local drive and directory redirection (`MS-RDPEFS` / `RDPDR`) integration.
//!
//! Provides client-to-server filesystem virtual channel redirection allowing local directories
//! on macOS / Linux clients to be seamlessly mounted and shared with the remote Windows host
//! directly inside Windows File Explorer (under `\\tsclient\<drive_name>` or `<drive_name> on Shellumi`).

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ironrdp_core::impl_as_any;
use ironrdp_pdu::PduResult;
use ironrdp_rdpdr::pdu::efs::{
    CreateDisposition, CreateOptions, DesiredAccess, DeviceControlRequest, DeviceCreateResponse,
    DeviceIoResponse, DeviceWriteResponse, Information, NtStatus, PrinterIoRequest,
    ServerDeviceAnnounceResponse, ServerDriveIoRequest,
};
use ironrdp_rdpdr::pdu::esc::{ScardCall, ScardIoCtlCode};
use ironrdp_rdpdr::pdu::RdpdrPdu;
use ironrdp_rdpdr::{Rdpdr, RdpdrBackend};
use ironrdp_svc::SvcMessage;
use shellumi_remote_desktop::SessionEvent;
use tracing::{debug, info, warn};

/// Default device identifier for the primary redirected drive.
pub const DEFAULT_DRIVE_DEVICE_ID: u32 = 1;

/// Metadata describing a redirected local drive or directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedirectedDrive {
    /// Unique 32-bit device identifier announced in RDPDR channel.
    pub device_id: u32,
    /// User-visible drive name (e.g. "Shared", "Downloads").
    pub name: String,
    /// Absolute local filesystem path mounted to the drive root.
    pub local_path: PathBuf,
}

/// Thread-safe capability, lifecycle, and telemetry state for RDP drive redirection (`RDPDR`).
#[derive(Clone, Debug)]
pub struct DriveControlState {
    device_id: u32,
    drive_name: Arc<Mutex<String>>,
    local_path: Arc<Mutex<PathBuf>>,
    mounted: Arc<AtomicBool>,
    read_only: Arc<AtomicBool>,
    revoked: Arc<AtomicBool>,
    bytes_read: Arc<AtomicU64>,
    bytes_written: Arc<AtomicU64>,
    operations_count: Arc<AtomicU64>,
}

impl DriveControlState {
    /// Create a new `DriveControlState` for a designated local path (read-only by default).
    pub fn new(
        device_id: u32,
        drive_name: impl Into<String>,
        local_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            device_id,
            drive_name: Arc::new(Mutex::new(drive_name.into())),
            local_path: Arc::new(Mutex::new(local_path.into())),
            mounted: Arc::new(AtomicBool::new(false)),
            read_only: Arc::new(AtomicBool::new(true)),
            revoked: Arc::new(AtomicBool::new(false)),
            bytes_read: Arc::new(AtomicU64::new(0)),
            bytes_written: Arc::new(AtomicU64::new(0)),
            operations_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Returns true if this share has been explicitly revoked.
    pub fn is_revoked(&self) -> bool {
        self.revoked.load(Ordering::Relaxed)
    }

    /// Mark this share as revoked (blocks all further I/O until reconnect).
    pub fn set_revoked(&self, revoked: bool) {
        self.revoked.store(revoked, Ordering::Relaxed);
        if revoked {
            self.mounted.store(false, Ordering::Relaxed);
        }
    }

    /// Returns true if the redirected drive enforces read-only access.
    pub fn is_read_only(&self) -> bool {
        self.read_only.load(Ordering::Relaxed)
    }

    /// Set read-only enforcement on the redirected drive.
    pub fn set_read_only(&self, read_only: bool) {
        self.read_only.store(read_only, Ordering::Relaxed);
    }

    /// 32-bit device identifier.
    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    /// User-visible drive name.
    pub fn drive_name(&self) -> String {
        self.drive_name
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "Shared".into())
    }

    /// Local filesystem path mounted to the drive.
    pub fn local_path(&self) -> PathBuf {
        self.local_path
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| PathBuf::from("."))
    }

    /// Returns true if the remote Windows server has acknowledged and mounted the drive.
    pub fn is_mounted(&self) -> bool {
        self.mounted.load(Ordering::Relaxed)
    }

    /// Set server mount confirmation status.
    pub fn set_mounted(&self, mounted: bool) {
        self.mounted.store(mounted, Ordering::Relaxed);
    }

    /// Total bytes read from the local drive by the remote server.
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read.load(Ordering::Relaxed)
    }

    /// Total bytes written to the local drive by the remote server.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written.load(Ordering::Relaxed)
    }

    /// Total filesystem I/O operations executed.
    pub fn operations_count(&self) -> u64 {
        self.operations_count.load(Ordering::Relaxed)
    }

    /// Increment read bytes counter.
    pub fn record_read(&self, bytes: u64) {
        self.bytes_read.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Increment written bytes counter.
    pub fn record_write(&self, bytes: u64) {
        self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Increment filesystem I/O operations counter.
    pub fn record_operation(&self) {
        self.operations_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Update the shared directory path and drive name.
    pub fn update_folder(&self, drive_name: impl Into<String>, local_path: impl Into<PathBuf>) {
        if let Ok(mut name_guard) = self.drive_name.lock() {
            *name_guard = drive_name.into();
        }
        if let Ok(mut path_guard) = self.local_path.lock() {
            *path_guard = local_path.into();
        }
    }
}

/// Custom telemetry and event forwarding wrapper around native RDPDR backend.
pub struct ShellumiRdpdrBackend {
    inner: Box<dyn RdpdrBackend>,
    state: DriveControlState,
    active_path: PathBuf,
    event_tx: Option<async_channel::Sender<SessionEvent>>,
}

impl_as_any!(ShellumiRdpdrBackend);

impl fmt::Debug for ShellumiRdpdrBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShellumiRdpdrBackend")
            .field("state", &self.state)
            .field("active_path", &self.active_path)
            .finish()
    }
}

impl ShellumiRdpdrBackend {
    /// Create a new `ShellumiRdpdrBackend` wrapping an underlying OS-specific backend.
    pub fn new(
        inner: Box<dyn RdpdrBackend>,
        state: DriveControlState,
        event_tx: Option<async_channel::Sender<SessionEvent>>,
    ) -> Self {
        let active_path = state.local_path();
        Self {
            inner,
            state,
            active_path,
            event_tx,
        }
    }

    fn emit_status(&self) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.try_send(SessionEvent::DriveStatus {
                drive_name: self.state.drive_name(),
                local_path: self.state.local_path().to_string_lossy().to_string(),
                mounted: self.state.is_mounted(),
                bytes_read: self.state.bytes_read(),
                bytes_written: self.state.bytes_written(),
                operations_count: self.state.operations_count(),
            });
        }
    }
}

impl RdpdrBackend for ShellumiRdpdrBackend {
    fn handle_server_device_announce_response(
        &mut self,
        pdu: ServerDeviceAnnounceResponse,
    ) -> PduResult<()> {
        let is_target = pdu.device_id == self.state.device_id();
        if is_target {
            let mounted = pdu.result_code == NtStatus::SUCCESS;
            info!(
                device_id = pdu.device_id,
                result = ?pdu.result_code,
                mounted,
                drive_name = %self.state.drive_name(),
                "Received server device announce response for redirected drive"
            );
            self.state.set_mounted(mounted);
            self.emit_status();
        }

        self.inner.handle_server_device_announce_response(pdu)
    }

    fn handle_scard_call(
        &mut self,
        req: DeviceControlRequest<ScardIoCtlCode>,
        call: ScardCall,
    ) -> PduResult<()> {
        self.inner.handle_scard_call(req, call)
    }

    fn handle_drive_io_request(&mut self, req: ServerDriveIoRequest) -> PduResult<Vec<SvcMessage>> {
        if self.state.is_revoked() || self.state.local_path() != self.active_path {
            warn!("Blocked RDPDR I/O on revoked or root-changed share (reconnect required)");
            return deny_io_request(req, NtStatus::ACCESS_DENIED);
        }

        self.state.record_operation();

        // Enforce strict path containment on Create and QueryDirectory requests
        if let ServerDriveIoRequest::ServerDriveQueryDirectoryRequest(ref q_req) = req {
            let clean = q_req.path.trim_end_matches('*');
            if !clean.is_empty() && validate_drive_relative_path(&self.active_path, clean).is_err()
            {
                warn!(path = %q_req.path, "Blocked RDPDR QueryDirectory path escape attempt");
                return deny_io_request(req, NtStatus::ACCESS_DENIED);
            }
        }

        // Enforce strict path containment on Create requests
        if let ServerDriveIoRequest::ServerCreateDriveRequest(ref create_req) = req {
            if let Err(status) = validate_drive_relative_path(&self.active_path, &create_req.path) {
                warn!(path = %create_req.path, "Blocked RDPDR path escape attempt");
                return Ok(vec![SvcMessage::from(RdpdrPdu::DeviceCreateResponse(
                    DeviceCreateResponse {
                        device_io_reply: DeviceIoResponse::new(
                            create_req.device_io_request.clone(),
                            status,
                        ),
                        file_id: 0,
                        information: Information::empty(),
                    },
                ))]);
            }

            if self.state.is_read_only() && is_mutating_create(create_req) {
                debug!(path = %create_req.path, "Denied mutating Create on read-only RDPDR share");
                return Ok(vec![SvcMessage::from(RdpdrPdu::DeviceCreateResponse(
                    DeviceCreateResponse {
                        device_io_reply: DeviceIoResponse::new(
                            create_req.device_io_request.clone(),
                            NtStatus::ACCESS_DENIED,
                        ),
                        file_id: 0,
                        information: Information::empty(),
                    },
                ))]);
            }
        }

        // Enforce read-only policy on Write and SetInformation
        if self.state.is_read_only() {
            match &req {
                ServerDriveIoRequest::DeviceWriteRequest(w) => {
                    return Ok(vec![SvcMessage::from(RdpdrPdu::DeviceWriteResponse(
                        DeviceWriteResponse {
                            device_io_reply: DeviceIoResponse::new(
                                w.device_io_request.clone(),
                                NtStatus::ACCESS_DENIED,
                            ),
                            length: 0,
                        },
                    ))]);
                }
                ServerDriveIoRequest::ServerDriveSetInformationRequest(s) => {
                    if let Ok(resp) =
                        ironrdp_rdpdr::pdu::efs::ClientDriveSetInformationResponse::new(
                            s,
                            NtStatus::ACCESS_DENIED,
                        )
                    {
                        return Ok(vec![SvcMessage::from(
                            RdpdrPdu::ClientDriveSetInformationResponse(resp),
                        )]);
                    }
                    return Ok(Vec::new());
                }
                _ => {}
            }
        }

        let mut read_requested_len = 0u64;
        match &req {
            ServerDriveIoRequest::DeviceWriteRequest(w) => {
                let write_len = w.write_data.len() as u64;
                self.state.record_write(write_len);
                debug!(write_len, "RDPDR write request");
            }
            ServerDriveIoRequest::DeviceReadRequest(r) => {
                read_requested_len = r.length as u64;
                debug!(read_requested_len, "RDPDR read request");
            }
            _ => {}
        }

        let res = self.inner.handle_drive_io_request(req)?;

        if read_requested_len > 0 {
            // Count read bytes from the actual encoded payload if present
            let mut bytes_read = 0u64;
            for msg in &res {
                if let Ok(unframed) = msg.encode_unframed_pdu() {
                    // Unframed DeviceReadResponse contains DeviceIoResponse (16 bytes) + length (4 bytes) + read_data
                    if unframed.len() > 20 {
                        bytes_read = bytes_read.saturating_add((unframed.len() - 20) as u64);
                    }
                }
            }
            if bytes_read > 0 {
                self.state.record_read(bytes_read);
            } else if !res.is_empty() {
                // Fallback to requested length if unframed size not directly parsed
                self.state.record_read(read_requested_len);
            }
        }

        self.emit_status();
        Ok(res)
    }

    fn handle_user_logged_on(&mut self, rdpdr: &mut Rdpdr) -> PduResult<Vec<SvcMessage>> {
        self.inner.handle_user_logged_on(rdpdr)
    }

    fn handle_printer_io_request(&mut self, req: PrinterIoRequest) -> PduResult<Vec<SvcMessage>> {
        self.inner.handle_printer_io_request(req)
    }
}

/// Returns a standard default local directory for drive redirection.
pub fn default_shared_folder() -> PathBuf {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    if let Some(home) = std::env::var_os("HOME") {
        let downloads = PathBuf::from(home).join("Downloads");
        if downloads.exists() {
            return downloads;
        }
    }

    std::env::temp_dir().join("shellumi-shared")
}

/// Derive a human-friendly drive name from a filesystem path.
pub fn derive_drive_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Shared")
        .to_string()
}

/// Build an authentic [`Rdpdr`] static virtual channel processor with local directory redirection.
pub fn create_rdpdr_channel(
    state: DriveControlState,
    event_tx: Option<async_channel::Sender<SessionEvent>>,
) -> Rdpdr {
    let local_path = state.local_path();
    let drive_name = state.drive_name();
    let dev_id = state.device_id();

    // Ensure the shared directory exists locally so backend can access it
    if !local_path.exists() {
        let _ = std::fs::create_dir_all(&local_path);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let native_backend: Box<dyn RdpdrBackend> =
        Box::new(ironrdp_rdpdr_native::backend::NixRdpdrBackend::new(
            local_path.to_string_lossy().to_string(),
        ));

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let native_backend: Box<dyn RdpdrBackend> =
        Box::new(ShellumiFsRdpdrBackend::new(local_path.clone()));

    let shellumi_backend = ShellumiRdpdrBackend::new(native_backend, state, event_tx);

    info!(
        device_id = dev_id,
        drive_name = %drive_name,
        local_path = %local_path.display(),
        "Configured RDPDR drive redirection channel"
    );

    Rdpdr::new(Box::new(shellumi_backend), "Shellumi".to_string())
        .with_drives(Some(vec![(dev_id, drive_name)]))
}

/// Validate that a server-supplied RDPDR path stays strictly inside `root`.
/// Rejects `..`, drive/ADS `:`, NUL bytes, and symlink escapes.
pub fn validate_drive_relative_path(root: &Path, rel: &str) -> Result<PathBuf, NtStatus> {
    if rel.contains('\0') || rel.contains(':') {
        return Err(NtStatus::ACCESS_DENIED);
    }
    let normalized = rel.replace('\\', "/");
    let mut candidate = root.to_path_buf();
    for comp in normalized.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." {
            return Err(NtStatus::ACCESS_DENIED);
        }
        candidate.push(comp);
        if let Ok(meta) = std::fs::symlink_metadata(&candidate) {
            if meta.file_type().is_symlink() {
                return Err(NtStatus::ACCESS_DENIED);
            }
        }
    }
    if let (Ok(canon_root), Ok(canon_target)) = (root.canonicalize(), candidate.canonicalize()) {
        if !canon_target.starts_with(&canon_root) {
            return Err(NtStatus::ACCESS_DENIED);
        }
    }
    Ok(candidate)
}

fn is_mutating_create(req: &ironrdp_rdpdr::pdu::efs::DeviceCreateRequest) -> bool {
    let disp = req.create_disposition;
    if disp == CreateDisposition::FILE_CREATE
        || disp == CreateDisposition::FILE_SUPERSEDE
        || disp == CreateDisposition::FILE_OVERWRITE
        || disp == CreateDisposition::FILE_OVERWRITE_IF
    {
        return true;
    }
    if req
        .create_options
        .contains(CreateOptions::FILE_DELETE_ON_CLOSE)
    {
        return true;
    }
    let write_mask = DesiredAccess::FILE_WRITE_DATA_OR_FILE_ADD_FILE
        | DesiredAccess::FILE_APPEND_DATA_OR_FILE_ADD_SUBDIRECTORY
        | DesiredAccess::FILE_WRITE_EA
        | DesiredAccess::FILE_DELETE_CHILD
        | DesiredAccess::FILE_WRITE_ATTRIBUTES
        | DesiredAccess::DELETE
        | DesiredAccess::WRITE_DAC
        | DesiredAccess::WRITE_OWNER
        | DesiredAccess::GENERIC_ALL;
    // Also check GENERIC_WRITE (0x4000_0000)
    req.desired_access.intersects(write_mask) || (req.desired_access.bits() & 0x4000_0000) != 0
}

/// Cross-platform `std::fs`-backed RDPDR filesystem backend (used on Windows instead of `NoopRdpdrBackend`).
#[derive(Debug)]
pub struct ShellumiFsRdpdrBackend {
    root: PathBuf,
    next_file_id: u32,
    files: std::collections::HashMap<u32, (PathBuf, Option<std::fs::File>)>,
    dir_iters: std::collections::HashMap<u32, Vec<PathBuf>>,
}

impl ShellumiFsRdpdrBackend {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            next_file_id: 1,
            files: std::collections::HashMap::new(),
            dir_iters: std::collections::HashMap::new(),
        }
    }
}

fn deny_io_request(req: ServerDriveIoRequest, status: NtStatus) -> PduResult<Vec<SvcMessage>> {
    use ironrdp_rdpdr::pdu::efs::*;
    match req {
        ServerDriveIoRequest::ServerCreateDriveRequest(c) => Ok(vec![SvcMessage::from(
            RdpdrPdu::DeviceCreateResponse(DeviceCreateResponse {
                device_io_reply: DeviceIoResponse::new(c.device_io_request, status),
                file_id: 0,
                information: Information::empty(),
            }),
        )]),
        ServerDriveIoRequest::DeviceReadRequest(r) => Ok(vec![SvcMessage::from(
            RdpdrPdu::DeviceReadResponse(DeviceReadResponse {
                device_io_reply: DeviceIoResponse::new(r.device_io_request, status),
                read_data: Vec::new(),
            }),
        )]),
        ServerDriveIoRequest::DeviceWriteRequest(w) => Ok(vec![SvcMessage::from(
            RdpdrPdu::DeviceWriteResponse(DeviceWriteResponse {
                device_io_reply: DeviceIoResponse::new(w.device_io_request, status),
                length: 0,
            }),
        )]),
        ServerDriveIoRequest::ServerDriveQueryDirectoryRequest(q) => Ok(vec![SvcMessage::from(
            RdpdrPdu::ClientDriveQueryDirectoryResponse(ClientDriveQueryDirectoryResponse {
                device_io_reply: DeviceIoResponse::new(q.device_io_request, status),
                buffer: None,
            }),
        )]),
        ServerDriveIoRequest::ServerDriveQueryInformationRequest(q) => Ok(vec![SvcMessage::from(
            RdpdrPdu::ClientDriveQueryInformationResponse(ClientDriveQueryInformationResponse {
                device_io_response: DeviceIoResponse::new(q.device_io_request, status),
                buffer: None,
            }),
        )]),
        ServerDriveIoRequest::ServerDriveSetInformationRequest(s) => {
            if let Ok(resp) = ClientDriveSetInformationResponse::new(&s, status) {
                Ok(vec![SvcMessage::from(
                    RdpdrPdu::ClientDriveSetInformationResponse(resp),
                )])
            } else {
                Ok(Vec::new())
            }
        }
        ServerDriveIoRequest::ServerDriveQueryVolumeInformationRequest(q) => {
            Ok(vec![SvcMessage::from(
                RdpdrPdu::ClientDriveQueryVolumeInformationResponse(
                    ClientDriveQueryVolumeInformationResponse {
                        device_io_reply: DeviceIoResponse::new(q.device_io_request, status),
                        buffer: None,
                    },
                ),
            )])
        }
        ServerDriveIoRequest::DeviceCloseRequest(c) => Ok(vec![SvcMessage::from(
            RdpdrPdu::DeviceCloseResponse(DeviceCloseResponse {
                device_io_response: DeviceIoResponse::new(c.device_io_request, NtStatus::SUCCESS),
            }),
        )]),
        _ => Ok(Vec::new()),
    }
}

impl_as_any!(ShellumiFsRdpdrBackend);

impl RdpdrBackend for ShellumiFsRdpdrBackend {
    fn handle_server_device_announce_response(
        &mut self,
        _pdu: ServerDeviceAnnounceResponse,
    ) -> PduResult<()> {
        Ok(())
    }

    fn handle_scard_call(
        &mut self,
        _req: DeviceControlRequest<ScardIoCtlCode>,
        _call: ScardCall,
    ) -> PduResult<()> {
        Ok(())
    }

    fn handle_drive_io_request(&mut self, req: ServerDriveIoRequest) -> PduResult<Vec<SvcMessage>> {
        use ironrdp_rdpdr::pdu::efs::*;
        use std::io::{Read, Seek, SeekFrom, Write};

        match req {
            ServerDriveIoRequest::ServerCreateDriveRequest(c) => {
                let target = match validate_drive_relative_path(&self.root, &c.path) {
                    Ok(p) => p,
                    Err(status) => {
                        return Ok(vec![SvcMessage::from(RdpdrPdu::DeviceCreateResponse(
                            DeviceCreateResponse {
                                device_io_reply: DeviceIoResponse::new(c.device_io_request, status),
                                file_id: 0,
                                information: Information::empty(),
                            },
                        ))]);
                    }
                };
                if target.is_dir() {
                    let fid = self.next_file_id;
                    self.next_file_id = self.next_file_id.wrapping_add(1).max(1);
                    self.files.insert(fid, (target, None));
                    return Ok(vec![SvcMessage::from(RdpdrPdu::DeviceCreateResponse(
                        DeviceCreateResponse {
                            device_io_reply: DeviceIoResponse::new(
                                c.device_io_request,
                                NtStatus::SUCCESS,
                            ),
                            file_id: fid,
                            information: Information::FILE_OPENED,
                        },
                    ))]);
                }
                let mut opts = std::fs::OpenOptions::new();
                opts.read(true);
                if is_mutating_create(&c) {
                    opts.write(true);
                    if c.create_disposition == CreateDisposition::FILE_CREATE {
                        opts.create_new(true);
                    } else if c.create_disposition == CreateDisposition::FILE_OPEN_IF
                        || c.create_disposition == CreateDisposition::FILE_OVERWRITE_IF
                    {
                        opts.create(true);
                    }
                }
                match opts.open(&target) {
                    Ok(f) => {
                        let fid = self.next_file_id;
                        self.next_file_id = self.next_file_id.wrapping_add(1).max(1);
                        self.files.insert(fid, (target, Some(f)));
                        Ok(vec![SvcMessage::from(RdpdrPdu::DeviceCreateResponse(
                            DeviceCreateResponse {
                                device_io_reply: DeviceIoResponse::new(
                                    c.device_io_request,
                                    NtStatus::SUCCESS,
                                ),
                                file_id: fid,
                                information: Information::FILE_OPENED,
                            },
                        ))])
                    }
                    Err(_) => Ok(vec![SvcMessage::from(RdpdrPdu::DeviceCreateResponse(
                        DeviceCreateResponse {
                            device_io_reply: DeviceIoResponse::new(
                                c.device_io_request,
                                NtStatus::NO_SUCH_FILE,
                            ),
                            file_id: 0,
                            information: Information::empty(),
                        },
                    ))]),
                }
            }
            ServerDriveIoRequest::DeviceCloseRequest(c) => {
                self.files.remove(&c.device_io_request.file_id);
                self.dir_iters.remove(&c.device_io_request.file_id);
                Ok(vec![SvcMessage::from(RdpdrPdu::DeviceCloseResponse(
                    DeviceCloseResponse {
                        device_io_response: DeviceIoResponse::new(
                            c.device_io_request,
                            NtStatus::SUCCESS,
                        ),
                    },
                ))])
            }
            ServerDriveIoRequest::DeviceReadRequest(r) => {
                let fid = r.device_io_request.file_id;
                let Some((_, Some(file))) = self.files.get_mut(&fid) else {
                    return Ok(vec![SvcMessage::from(RdpdrPdu::DeviceReadResponse(
                        DeviceReadResponse {
                            device_io_reply: DeviceIoResponse::new(
                                r.device_io_request,
                                NtStatus::UNSUCCESSFUL,
                            ),
                            read_data: Vec::new(),
                        },
                    ))]);
                };
                if file.seek(SeekFrom::Start(r.offset)).is_err() {
                    return Ok(vec![SvcMessage::from(RdpdrPdu::DeviceReadResponse(
                        DeviceReadResponse {
                            device_io_reply: DeviceIoResponse::new(
                                r.device_io_request,
                                NtStatus::UNSUCCESSFUL,
                            ),
                            read_data: Vec::new(),
                        },
                    ))]);
                }
                let mut buf = vec![0u8; (r.length as usize).min(1024 * 1024)];
                match file.read(&mut buf) {
                    Ok(n) => {
                        buf.truncate(n);
                        Ok(vec![SvcMessage::from(RdpdrPdu::DeviceReadResponse(
                            DeviceReadResponse {
                                device_io_reply: DeviceIoResponse::new(
                                    r.device_io_request,
                                    NtStatus::SUCCESS,
                                ),
                                read_data: buf,
                            },
                        ))])
                    }
                    Err(_) => Ok(vec![SvcMessage::from(RdpdrPdu::DeviceReadResponse(
                        DeviceReadResponse {
                            device_io_reply: DeviceIoResponse::new(
                                r.device_io_request,
                                NtStatus::UNSUCCESSFUL,
                            ),
                            read_data: Vec::new(),
                        },
                    ))]),
                }
            }
            ServerDriveIoRequest::DeviceWriteRequest(w) => {
                let fid = w.device_io_request.file_id;
                let Some((_, Some(file))) = self.files.get_mut(&fid) else {
                    return Ok(vec![SvcMessage::from(RdpdrPdu::DeviceWriteResponse(
                        DeviceWriteResponse {
                            device_io_reply: DeviceIoResponse::new(
                                w.device_io_request,
                                NtStatus::UNSUCCESSFUL,
                            ),
                            length: 0,
                        },
                    ))]);
                };
                if file.seek(SeekFrom::Start(w.offset)).is_err() {
                    return Ok(vec![SvcMessage::from(RdpdrPdu::DeviceWriteResponse(
                        DeviceWriteResponse {
                            device_io_reply: DeviceIoResponse::new(
                                w.device_io_request,
                                NtStatus::UNSUCCESSFUL,
                            ),
                            length: 0,
                        },
                    ))]);
                }
                match file.write(&w.write_data) {
                    Ok(n) => Ok(vec![SvcMessage::from(RdpdrPdu::DeviceWriteResponse(
                        DeviceWriteResponse {
                            device_io_reply: DeviceIoResponse::new(
                                w.device_io_request,
                                NtStatus::SUCCESS,
                            ),
                            length: n as u32,
                        },
                    ))]),
                    Err(_) => Ok(vec![SvcMessage::from(RdpdrPdu::DeviceWriteResponse(
                        DeviceWriteResponse {
                            device_io_reply: DeviceIoResponse::new(
                                w.device_io_request,
                                NtStatus::ACCESS_DENIED,
                            ),
                            length: 0,
                        },
                    ))]),
                }
            }
            ServerDriveIoRequest::DeviceControlRequest(c) => Ok(vec![SvcMessage::from(
                RdpdrPdu::DeviceControlResponse(DeviceControlResponse {
                    device_io_reply: DeviceIoResponse::new(c.header, NtStatus::SUCCESS),
                    output_buffer: None,
                }),
            )]),
            ServerDriveIoRequest::ServerDriveQueryInformationRequest(q) => {
                let fid = q.device_io_request.file_id;
                let Some((path, _)) = self.files.get(&fid) else {
                    return Ok(vec![SvcMessage::from(
                        RdpdrPdu::ClientDriveQueryInformationResponse(
                            ClientDriveQueryInformationResponse {
                                device_io_response: DeviceIoResponse::new(
                                    q.device_io_request,
                                    NtStatus::NO_SUCH_FILE,
                                ),
                                buffer: None,
                            },
                        ),
                    )]);
                };
                let Ok(meta) = std::fs::metadata(path) else {
                    return Ok(vec![SvcMessage::from(
                        RdpdrPdu::ClientDriveQueryInformationResponse(
                            ClientDriveQueryInformationResponse {
                                device_io_response: DeviceIoResponse::new(
                                    q.device_io_request,
                                    NtStatus::UNSUCCESSFUL,
                                ),
                                buffer: None,
                            },
                        ),
                    )]);
                };
                let attrs = if meta.is_dir() {
                    FileAttributes::FILE_ATTRIBUTE_DIRECTORY
                } else {
                    FileAttributes::FILE_ATTRIBUTE_NORMAL
                };
                let size_i64 = i64::try_from(meta.len()).unwrap_or(i64::MAX);
                let buffer =
                    if q.file_info_class_lvl == FileInformationClassLevel::FILE_BASIC_INFORMATION {
                        Some(FileInformationClass::Basic(FileBasicInformation {
                            creation_time: 0,
                            last_access_time: 0,
                            last_write_time: 0,
                            change_time: 0,
                            file_attributes: attrs,
                        }))
                    } else if q.file_info_class_lvl
                        == FileInformationClassLevel::FILE_STANDARD_INFORMATION
                    {
                        Some(FileInformationClass::Standard(FileStandardInformation {
                            allocation_size: size_i64,
                            end_of_file: size_i64,
                            number_of_links: 1,
                            delete_pending: Boolean::False,
                            directory: if meta.is_dir() {
                                Boolean::True
                            } else {
                                Boolean::False
                            },
                        }))
                    } else if q.file_info_class_lvl
                        == FileInformationClassLevel::FILE_ATTRIBUTE_TAG_INFORMATION
                    {
                        Some(FileInformationClass::AttributeTag(
                            FileAttributeTagInformation {
                                file_attributes: attrs,
                                reparse_tag: 0,
                            },
                        ))
                    } else {
                        None
                    };
                let status = if buffer.is_some() {
                    NtStatus::SUCCESS
                } else {
                    NtStatus::NOT_SUPPORTED
                };
                Ok(vec![SvcMessage::from(
                    RdpdrPdu::ClientDriveQueryInformationResponse(
                        ClientDriveQueryInformationResponse {
                            device_io_response: DeviceIoResponse::new(q.device_io_request, status),
                            buffer,
                        },
                    ),
                )])
            }
            ServerDriveIoRequest::ServerDriveSetInformationRequest(s) => {
                let fid = s.device_io_request.file_id;
                let status = match &s.set_buffer {
                    FileInformationClass::EndOfFile(eof) => {
                        if let Some((_, Some(file))) = self.files.get_mut(&fid) {
                            if eof.end_of_file >= 0 && file.set_len(eof.end_of_file as u64).is_ok()
                            {
                                NtStatus::SUCCESS
                            } else {
                                NtStatus::UNSUCCESSFUL
                            }
                        } else {
                            NtStatus::NO_SUCH_FILE
                        }
                    }
                    FileInformationClass::Rename(ren) => {
                        if let Some(old_path) = self.files.get(&fid).map(|(p, _)| p.clone()) {
                            if let Ok(new_path) =
                                validate_drive_relative_path(&self.root, &ren.file_name)
                            {
                                if std::fs::rename(&old_path, &new_path).is_ok() {
                                    if let Some((p, _)) = self.files.get_mut(&fid) {
                                        *p = new_path;
                                    }
                                    NtStatus::SUCCESS
                                } else {
                                    NtStatus::ACCESS_DENIED
                                }
                            } else {
                                NtStatus::ACCESS_DENIED
                            }
                        } else {
                            NtStatus::NO_SUCH_FILE
                        }
                    }
                    FileInformationClass::Disposition(disp) if disp.delete_pending != 0 => {
                        if let Some(path) = self.files.get(&fid).map(|(p, _)| p.clone()) {
                            let res = if path.is_dir() {
                                std::fs::remove_dir(&path)
                            } else {
                                std::fs::remove_file(&path)
                            };
                            if res.is_ok() {
                                NtStatus::SUCCESS
                            } else {
                                NtStatus::ACCESS_DENIED
                            }
                        } else {
                            NtStatus::NO_SUCH_FILE
                        }
                    }
                    _ => NtStatus::NOT_SUPPORTED,
                };
                match ClientDriveSetInformationResponse::new(&s, status) {
                    Ok(resp) => Ok(vec![SvcMessage::from(
                        RdpdrPdu::ClientDriveSetInformationResponse(resp),
                    )]),
                    Err(_) => Ok(Vec::new()),
                }
            }
            ServerDriveIoRequest::ServerDriveQueryDirectoryRequest(q) => {
                let fid = q.device_io_request.file_id;
                if q.initial_query > 0 {
                    let clean_rel = q.path.trim_end_matches('*');
                    let dir_path = if clean_rel.is_empty() {
                        self.root.clone()
                    } else if let Ok(p) = validate_drive_relative_path(&self.root, clean_rel) {
                        p
                    } else {
                        return Ok(vec![SvcMessage::from(
                            RdpdrPdu::ClientDriveQueryDirectoryResponse(
                                ClientDriveQueryDirectoryResponse {
                                    device_io_reply: DeviceIoResponse::new(
                                        q.device_io_request,
                                        NtStatus::ACCESS_DENIED,
                                    ),
                                    buffer: None,
                                },
                            ),
                        )]);
                    };
                    let mut entries = Vec::new();
                    if let Ok(rd) = std::fs::read_dir(&dir_path) {
                        for entry in rd.flatten() {
                            entries.push(entry.path());
                        }
                    }
                    self.dir_iters.insert(fid, entries);
                }

                let next_entry = self.dir_iters.get_mut(&fid).and_then(|v| {
                    if v.is_empty() {
                        None
                    } else {
                        Some(v.remove(0))
                    }
                });

                if let Some(entry_path) = next_entry {
                    let meta = std::fs::metadata(&entry_path).ok();
                    let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
                    let size = meta
                        .as_ref()
                        .map_or(0, |m| i64::try_from(m.len()).unwrap_or(i64::MAX));
                    let attrs = if is_dir {
                        FileAttributes::FILE_ATTRIBUTE_DIRECTORY
                    } else {
                        FileAttributes::FILE_ATTRIBUTE_NORMAL
                    };
                    let name = entry_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    let info = FileBothDirectoryInformation::new(0, 0, 0, 0, size, attrs, name);
                    Ok(vec![SvcMessage::from(
                        RdpdrPdu::ClientDriveQueryDirectoryResponse(
                            ClientDriveQueryDirectoryResponse {
                                device_io_reply: DeviceIoResponse::new(
                                    q.device_io_request,
                                    NtStatus::SUCCESS,
                                ),
                                buffer: Some(FileInformationClass::BothDirectory(info)),
                            },
                        ),
                    )])
                } else {
                    Ok(vec![SvcMessage::from(
                        RdpdrPdu::ClientDriveQueryDirectoryResponse(
                            ClientDriveQueryDirectoryResponse {
                                device_io_reply: DeviceIoResponse::new(
                                    q.device_io_request,
                                    NtStatus::NO_MORE_FILES,
                                ),
                                buffer: None,
                            },
                        ),
                    )])
                }
            }
            ServerDriveIoRequest::ServerDriveQueryVolumeInformationRequest(q) => {
                Ok(vec![SvcMessage::from(
                    RdpdrPdu::ClientDriveQueryVolumeInformationResponse(
                        ClientDriveQueryVolumeInformationResponse {
                            device_io_reply: DeviceIoResponse::new(
                                q.device_io_request,
                                NtStatus::NOT_SUPPORTED,
                            ),
                            buffer: None,
                        },
                    ),
                )])
            }
            ServerDriveIoRequest::ServerDriveNotifyChangeDirectoryRequest(_)
            | ServerDriveIoRequest::ServerDriveLockControlRequest(_) => Ok(Vec::new()),
        }
    }

    fn handle_user_logged_on(&mut self, _rdpdr: &mut Rdpdr) -> PduResult<Vec<SvcMessage>> {
        Ok(Vec::new())
    }

    fn handle_printer_io_request(&mut self, _req: PrinterIoRequest) -> PduResult<Vec<SvcMessage>> {
        Ok(Vec::new())
    }
}

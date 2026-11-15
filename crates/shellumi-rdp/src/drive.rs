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
    DeviceControlRequest, NtStatus, PrinterIoRequest, ServerDeviceAnnounceResponse,
    ServerDriveIoRequest,
};
use ironrdp_rdpdr::pdu::esc::{ScardCall, ScardIoCtlCode};
use ironrdp_rdpdr::{Rdpdr, RdpdrBackend};
use ironrdp_svc::SvcMessage;
use shellumi_remote_desktop::SessionEvent;
use tracing::{debug, info};

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
    bytes_read: Arc<AtomicU64>,
    bytes_written: Arc<AtomicU64>,
    operations_count: Arc<AtomicU64>,
}

impl DriveControlState {
    /// Create a new `DriveControlState` for a designated local path.
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
            bytes_read: Arc::new(AtomicU64::new(0)),
            bytes_written: Arc::new(AtomicU64::new(0)),
            operations_count: Arc::new(AtomicU64::new(0)),
        }
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
        // Check if user updated the local shared path dynamically
        let current_path = self.state.local_path();
        if current_path != self.active_path {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            {
                if !current_path.exists() {
                    let _ = std::fs::create_dir_all(&current_path);
                }
                self.inner = Box::new(ironrdp_rdpdr_native::backend::NixRdpdrBackend::new(
                    current_path.to_string_lossy().to_string(),
                ));
                self.active_path = current_path.clone();
                info!(
                    new_path = %current_path.display(),
                    "Swapped active local directory for RDPDR redirected drive"
                );
            }
        }

        self.state.record_operation();

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
    let native_backend: Box<dyn RdpdrBackend> = Box::new(ironrdp_rdpdr::NoopRdpdrBackend);

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

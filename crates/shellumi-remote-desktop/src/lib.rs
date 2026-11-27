//! Core domain abstractions, framebuffer memory model, input events, and telemetry
//! for Shellumi Remote Desktop (VNC / RFB & RDP).

pub mod controller;
pub mod events;
pub mod framebuffer;
pub mod input;
pub mod metrics;
pub mod security;
pub mod session;

pub use controller::{
    InputCapabilities, InputPipelineCounters, InputRejectReason, RemoteInputController,
};
pub use events::{
    create_session_channels, ClipboardPolicy, ClipboardSyncMode, SessionCommand, SessionEvent,
    SessionUiHandle, SessionWorkerHandle, TransferDirection,
};
pub use framebuffer::{
    DirtyRegion, Framebuffer, FramebufferError, FramebufferSnapshot, Rect,
    MAX_FRAMEBUFFER_DIMENSION,
};
pub use input::{
    pointer_mask, translate_coordinates, CanvasTransform, ClipboardEvent, KeyEvent, KeyModifiers,
    KeyboardMappingMode, MouseButton, PanState, PointerEvent, RemoteInput, RemoteKey,
    RemoteShortcut, ScaleMode, WheelEvent,
};
pub use metrics::{SessionMetrics, SessionMetricsSnapshot};
pub use security::{format_fingerprint, CertificateValidationPolicy, SecurityWarning};
pub use session::{ReconnectPolicy, RemoteProtocol, SessionConfig, SessionState};

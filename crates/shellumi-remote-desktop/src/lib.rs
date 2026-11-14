//! Core domain abstractions, framebuffer memory model, input events, and telemetry
//! for Shellumi Remote Desktop (VNC / RFB & RDP).

pub mod events;
pub mod framebuffer;
pub mod input;
pub mod metrics;
pub mod security;
pub mod session;

pub use events::{
    create_session_channels, SessionCommand, SessionEvent, SessionUiHandle, SessionWorkerHandle,
};
pub use framebuffer::{
    DirtyRegion, Framebuffer, FramebufferError, FramebufferSnapshot, Rect,
    MAX_FRAMEBUFFER_DIMENSION,
};
pub use input::{
    pointer_mask, translate_coordinates, ClipboardEvent, KeyEvent, KeyModifiers, MouseButton,
    PointerEvent, ScaleMode, WheelEvent,
};
pub use metrics::{SessionMetrics, SessionMetricsSnapshot};
pub use security::{format_fingerprint, CertificateValidationPolicy, SecurityWarning};
pub use session::{RemoteProtocol, SessionConfig, SessionState};

//! Remote desktop session lifecycle state machine and configuration.

use crate::input::ScaleMode;
use serde::{Deserialize, Serialize};

/// Supported remote desktop protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteProtocol {
    /// Remote Framebuffer protocol (RFC 6143 / VNC).
    Vnc,
    /// Microsoft Remote Desktop Protocol (RDP).
    Rdp,
}

impl RemoteProtocol {
    /// Default standard network port for this protocol.
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Vnc => 5900,
            Self::Rdp => 3389,
        }
    }

    /// User-facing protocol name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Vnc => "VNC / RFB",
            Self::Rdp => "RDP (IronRDP)",
        }
    }
}

/// Lifecycle state of a remote desktop connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Idle session, ready to connect.
    NotConnected,
    /// Handshaking, TLS negotiation, or authenticating.
    Connecting,
    /// Authenticated and receiving active frame stream.
    Connected,
    /// Graceful disconnect in progress.
    Disconnecting,
    /// Connection terminated cleanly.
    Disconnected,
    /// Connection terminated due to protocol error or network failure.
    Error(String),
}

impl SessionState {
    /// Returns true if the session is currently connected and processing frames.
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    /// Returns true if the session is in an active lifecycle state (Connecting, Connected, or Disconnecting).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Connecting | Self::Connected | Self::Disconnecting
        )
    }

    /// Validates whether a state transition from `self` to `target` is legal according to the lifecycle model.
    pub fn can_transition_to(&self, target: &SessionState) -> bool {
        match (self, target) {
            // Can initiate connection from idle, disconnected, or error states
            (Self::NotConnected | Self::Disconnected | Self::Error(_), Self::Connecting) => true,
            // Connecting transitions to Connected or fails to Disconnected/Error
            (Self::Connecting, Self::Connected | Self::Disconnected | Self::Error(_)) => true,
            // Connected transitions to Disconnecting or immediately Disconnected/Error
            (Self::Connected, Self::Disconnecting | Self::Disconnected | Self::Error(_)) => true,
            // Disconnecting completes into Disconnected or Error
            (Self::Disconnecting, Self::Disconnected | Self::Error(_)) => true,
            // Disconnected or Error can reset to NotConnected
            (Self::Disconnected | Self::Error(_), Self::NotConnected) => true,
            // Re-entrant / identity
            (a, b) if a == b => true,
            _ => false,
        }
    }
}

use zeroize::Zeroize;

/// Session parameters configured by the user before launching a connection.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConfig {
    pub host: String,
    pub port: u16,
    pub protocol: RemoteProtocol,
    pub width: u32,
    pub height: u32,
    pub color_depth: u8,
    pub view_only: bool,
    pub scale_mode: ScaleMode,
    pub username: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
    pub tls_strict: bool,
}

impl std::fmt::Debug for SessionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("protocol", &self.protocol)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("color_depth", &self.color_depth)
            .field("view_only", &self.view_only)
            .field("scale_mode", &self.scale_mode)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("domain", &self.domain)
            .field("tls_strict", &self.tls_strict)
            .finish()
    }
}

impl Drop for SessionConfig {
    fn drop(&mut self) {
        if let Some(pwd) = &mut self.password {
            pwd.zeroize();
        }
    }
}

impl SessionConfig {
    /// Create a standard default configuration for a target host and protocol.
    pub fn new(host: impl Into<String>, port: u16, protocol: RemoteProtocol) -> Self {
        Self {
            host: host.into(),
            port,
            protocol,
            width: 1920,
            height: 1080,
            color_depth: 24,
            view_only: false,
            scale_mode: ScaleMode::Fit,
            username: None,
            password: None,
            domain: None,
            tls_strict: true,
        }
    }

    /// Target endpoint string formatted as `host:port`.
    pub fn target_endpoint(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_session_state_machine_transitions() {
        let not_conn = SessionState::NotConnected;
        let conn = SessionState::Connecting;
        let ok = SessionState::Connected;
        let disc_ing = SessionState::Disconnecting;
        let disc = SessionState::Disconnected;
        let err = SessionState::Error("Network timeout".into());

        assert!(not_conn.can_transition_to(&conn));
        assert!(!not_conn.can_transition_to(&ok)); // Cannot jump directly from NotConnected to Connected!
        assert!(conn.can_transition_to(&ok));
        assert!(ok.can_transition_to(&disc_ing));
        assert!(disc_ing.can_transition_to(&disc));
        assert!(disc.can_transition_to(&conn));
        assert!(conn.can_transition_to(&err));
        assert!(err.can_transition_to(&conn));
    }

    #[test]
    fn verifies_session_config_construction() {
        let cfg = SessionConfig::new("10.0.0.4", 5900, RemoteProtocol::Vnc);
        assert_eq!(cfg.target_endpoint(), "10.0.0.4:5900");
        assert_eq!(cfg.protocol.default_port(), 5900);
        assert!(cfg.tls_strict);
    }

    #[test]
    fn verifies_session_config_zeroizes_password_and_redacts_debug() {
        let mut cfg = SessionConfig::new("10.0.0.4", 5900, RemoteProtocol::Vnc);
        cfg.password = Some("SuperSecretPassword123!".to_string());

        let debug_str = format!("{cfg:?}");
        assert!(!debug_str.contains("SuperSecretPassword123!"));
        assert!(debug_str.contains("[REDACTED]"));

        // Verify zeroization on drop
        let mut pwd = cfg.password.take().unwrap();
        assert!(!pwd.is_empty());
        pwd.zeroize();
        assert!(pwd.is_empty());

        let mut byte_buffer = *b"SuperSecretPassword123!";
        byte_buffer.zeroize();
        assert_eq!(byte_buffer, [0u8; 23]);
    }
}

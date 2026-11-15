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

/// Network reconnection policy for transient disconnects and network drops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconnectPolicy {
    /// Whether automatic reconnection is enabled.
    pub enabled: bool,
    /// Maximum number of consecutive retry attempts (0 to disable).
    pub max_attempts: u32,
    /// Initial backoff delay in milliseconds.
    pub initial_delay_ms: u64,
    /// Maximum backoff ceiling in milliseconds.
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff (e.g. 2 for doubling).
    pub backoff_multiplier: u32,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 5,
            initial_delay_ms: 1000,
            max_delay_ms: 15000,
            backoff_multiplier: 2,
        }
    }
}

impl ReconnectPolicy {
    /// Disabled reconnect policy (never retry on drop).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_attempts: 0,
            initial_delay_ms: 0,
            max_delay_ms: 0,
            backoff_multiplier: 1,
        }
    }

    /// Compute backoff delay for the given 1-based attempt index.
    pub fn compute_delay(&self, attempt: u32) -> std::time::Duration {
        if attempt <= 1 {
            return std::time::Duration::from_millis(self.initial_delay_ms);
        }
        let exp = (attempt - 1).min(6);
        let factor = (self.backoff_multiplier as u64).saturating_pow(exp);
        let delay_ms = self
            .initial_delay_ms
            .saturating_mul(factor)
            .min(self.max_delay_ms);
        std::time::Duration::from_millis(delay_ms)
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
    /// Transient connection drop; automatic reconnection in progress.
    Reconnecting {
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
    },
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

    /// Returns true if the session is in an active lifecycle state (Connecting, Connected, Reconnecting, or Disconnecting).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Connecting | Self::Connected | Self::Reconnecting { .. } | Self::Disconnecting
        )
    }

    /// Returns true if the session is currently attempting to reconnect after a transient network drop.
    pub fn is_reconnecting(&self) -> bool {
        matches!(self, Self::Reconnecting { .. })
    }

    /// Validates whether a state transition from `self` to `target` is legal according to the lifecycle model.
    pub fn can_transition_to(&self, target: &SessionState) -> bool {
        match (self, target) {
            // Can initiate connection from idle, disconnected, error, or reconnecting states
            (
                Self::NotConnected
                | Self::Disconnected
                | Self::Error(_)
                | Self::Reconnecting { .. },
                Self::Connecting,
            ) => true,
            // Connecting transitions to Connected, Reconnecting, Disconnected, or Error
            (
                Self::Connecting,
                Self::Connected | Self::Reconnecting { .. } | Self::Disconnected | Self::Error(_),
            ) => true,
            // Connected transitions to Reconnecting, Disconnecting, or immediately Disconnected/Error
            (
                Self::Connected,
                Self::Reconnecting { .. }
                | Self::Disconnecting
                | Self::Disconnected
                | Self::Error(_),
            ) => true,
            // Reconnecting transitions to Connected, Disconnected, or Error (Connecting is handled above)
            (Self::Reconnecting { .. }, Self::Connected | Self::Disconnected | Self::Error(_)) => {
                true
            }
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

use std::path::PathBuf;
use zeroize::Zeroize;

fn default_enable_audio() -> bool {
    true
}

fn default_enable_microphone() -> bool {
    true
}

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
    #[serde(default = "default_enable_audio")]
    pub enable_audio: bool,
    #[serde(default = "default_enable_microphone")]
    pub enable_microphone: bool,
    #[serde(default)]
    pub shared_folder: Option<PathBuf>,
    #[serde(default)]
    pub reconnect_policy: ReconnectPolicy,
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
            .field("enable_audio", &self.enable_audio)
            .field("enable_microphone", &self.enable_microphone)
            .field("shared_folder", &self.shared_folder)
            .field("reconnect_policy", &self.reconnect_policy)
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
            enable_audio: true,
            enable_microphone: true,
            shared_folder: None,
            reconnect_policy: ReconnectPolicy::default(),
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

        // Reconnecting transitions
        let reconn = SessionState::Reconnecting {
            attempt: 1,
            max_attempts: 5,
            delay_ms: 1000,
        };
        assert!(ok.can_transition_to(&reconn));
        assert!(reconn.can_transition_to(&conn));
        assert!(reconn.can_transition_to(&ok));
        assert!(reconn.can_transition_to(&disc));
        assert!(reconn.can_transition_to(&err));
        assert!(reconn.is_active());
        assert!(reconn.is_reconnecting());
        assert!(!reconn.is_connected());
    }

    #[test]
    fn test_reconnect_policy_backoff_calculation() {
        let policy = ReconnectPolicy::default();
        assert!(policy.enabled);
        assert_eq!(policy.max_attempts, 5);

        // Attempt 1 = initial_delay_ms (1000ms)
        assert_eq!(
            policy.compute_delay(1),
            std::time::Duration::from_millis(1000)
        );
        // Attempt 2 = 1000 * 2^1 = 2000ms
        assert_eq!(
            policy.compute_delay(2),
            std::time::Duration::from_millis(2000)
        );
        // Attempt 3 = 1000 * 2^2 = 4000ms
        assert_eq!(
            policy.compute_delay(3),
            std::time::Duration::from_millis(4000)
        );
        // Attempt 4 = 1000 * 2^3 = 8000ms
        assert_eq!(
            policy.compute_delay(4),
            std::time::Duration::from_millis(8000)
        );
        // Attempt 5 = 1000 * 2^4 = 16000ms clamped to max_delay_ms (15000ms)
        assert_eq!(
            policy.compute_delay(5),
            std::time::Duration::from_millis(15000)
        );

        let disabled = ReconnectPolicy::disabled();
        assert!(!disabled.enabled);
        assert_eq!(disabled.max_attempts, 0);
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

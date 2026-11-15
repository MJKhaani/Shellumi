//! Shellumi RDP Display Control (`MS-RDPEDISP`) extension integration.
//!
//! Provides dynamic client-to-server desktop resolution resizing and monitor layout updates
//! in real time without requiring session reconnection or restarting applications.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use ironrdp_displaycontrol::client::DisplayControlClient;
use ironrdp_displaycontrol::pdu::MonitorLayoutEntry;
use shellumi_remote_desktop::SessionEvent;
use tracing::{debug, info};

/// Thread-safe capability and telemetry state for dynamic display resizing (`MS-RDPEDISP`).
#[derive(Clone, Debug)]
pub struct DisplayControlState {
    ready: Arc<AtomicBool>,
    max_monitors: Arc<AtomicU32>,
    max_area: Arc<AtomicU64>,
}

impl Default for DisplayControlState {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayControlState {
    /// Create a new uninitialized `DisplayControlState`.
    pub fn new() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            max_monitors: Arc::new(AtomicU32::new(1)),
            max_area: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Check if the remote Windows host has negotiated `MS-RDPEDISP` capabilities.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// Set readiness state.
    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Relaxed);
    }

    /// Maximum number of monitors supported by the remote host.
    pub fn max_monitors(&self) -> u32 {
        self.max_monitors.load(Ordering::Relaxed)
    }

    /// Maximum combined monitor area in square pixels supported by the remote host.
    pub fn max_area(&self) -> u64 {
        self.max_area.load(Ordering::Relaxed)
    }

    /// Update capability values from remote server PDU.
    pub fn update_caps(&self, max_monitors: u32, max_area: u64) {
        self.max_monitors.store(max_monitors, Ordering::Relaxed);
        self.max_area.store(max_area, Ordering::Relaxed);
        self.ready.store(true, Ordering::Relaxed);
    }
}

/// Constrain and adjust display dimensions according to MS-RDPEDISP § 2.2.2.2.1:
/// - Width MUST be between 200 and 8192 pixels.
/// - Width MUST NOT be an odd value (must be even).
/// - Height MUST be between 200 and 8192 pixels.
pub fn adjust_display_size(width: u32, height: u32) -> (u32, u32) {
    MonitorLayoutEntry::adjust_display_size(width, height)
}

/// Create a `DisplayControlClient` dynamic channel processor tied to Shellumi state and UI events.
pub fn create_display_control_client(
    state: DisplayControlState,
    event_tx: async_channel::Sender<SessionEvent>,
) -> DisplayControlClient {
    DisplayControlClient::new(move |caps| {
        let max_area = caps.max_monitor_area();
        info!(
            max_area,
            "Received MS-RDPEDISP display control capabilities from server"
        );
        state.update_caps(16, max_area);

        let _ = event_tx.try_send(SessionEvent::DisplayControlCaps {
            max_monitors: 16,
            max_area,
        });

        debug!("MS-RDPEDISP dynamic display channel is now ready for live resizing");
        Ok(Vec::new())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjust_display_size_constraints() {
        // Standard 1080p - even width, within bounds
        assert_eq!(adjust_display_size(1920, 1080), (1920, 1080));

        // Odd width must be decremented to even
        assert_eq!(adjust_display_size(1921, 1080), (1920, 1080));
        assert_eq!(adjust_display_size(1281, 720), (1280, 720));

        // Clamping to minimum (200x200)
        assert_eq!(adjust_display_size(100, 100), (200, 200));
        assert_eq!(adjust_display_size(0, 0), (200, 200));

        // Clamping to maximum (8192x8192)
        assert_eq!(adjust_display_size(10000, 10000), (8192, 8192));
        assert_eq!(adjust_display_size(8193, 8192), (8192, 8192));
    }

    #[test]
    fn test_display_control_state_lifecycle() {
        let state = DisplayControlState::new();
        assert!(!state.is_ready());
        assert_eq!(state.max_monitors(), 1);
        assert_eq!(state.max_area(), 0);

        state.update_caps(16, 67108864);
        assert!(state.is_ready());
        assert_eq!(state.max_monitors(), 16);
        assert_eq!(state.max_area(), 67108864);

        state.set_ready(false);
        assert!(!state.is_ready());
    }

    #[test]
    fn test_create_display_control_client_and_channel_name() {
        use ironrdp_dvc::DvcProcessor;

        let state = DisplayControlState::new();
        let (tx, _rx) = async_channel::bounded(16);
        let client = create_display_control_client(state, tx);

        assert_eq!(client.channel_name(), ironrdp_displaycontrol::CHANNEL_NAME);
        assert!(!client.ready());
    }
}

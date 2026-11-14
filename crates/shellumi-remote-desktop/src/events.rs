//! Bidirectional channel communication between UI and protocol worker tasks.

use crate::framebuffer::Rect;
use crate::input::{KeyEvent, PointerEvent, WheelEvent};
use crate::metrics::SessionMetricsSnapshot;
use crate::security::SecurityWarning;
use crate::session::SessionState;
use async_channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};
use serde::{Deserialize, Serialize};

/// Commands sent from the UI / main thread to the protocol background worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionCommand {
    /// Initiate connection to the remote endpoint.
    Connect,
    /// Gracefully terminate connection.
    Disconnect,
    /// Mouse pointer motion or button event.
    Pointer(PointerEvent),
    /// Scroll wheel event.
    Wheel(WheelEvent),
    /// Keyboard key event.
    Key(KeyEvent),
    /// Send local clipboard text to the remote host.
    SendClipboard(String),
    /// Request desktop resize (if protocol extension supported).
    RequestResize { width: u32, height: u32 },
    /// Request a full framebuffer refresh.
    Refresh,
}

/// Events emitted by the protocol background worker to the UI / main thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionEvent {
    /// Session connection state changed.
    StateChanged(SessionState),
    /// Remote screen resolution updated.
    RemoteResize { width: u32, height: u32 },
    /// Framebuffer rectangle updates applied.
    Damage { rects: Vec<Rect>, sequence: u64 },
    /// New complete display frame is ready in the shared framebuffer.
    FrameReady { sequence: u64 },
    /// Clipboard text received from the remote host.
    ServerClipboard(String),
    /// Remote terminal bell event.
    Bell,
    /// Connection or protocol error occurred.
    Error(String),
    /// Security alert or certificate warning.
    SecurityWarning(SecurityWarning),
    /// Measured performance and telemetry metrics snapshot.
    Metrics(SessionMetricsSnapshot),
}

/// UI-side session channel handle.
#[derive(Debug, Clone)]
pub struct SessionUiHandle {
    command_tx: Sender<SessionCommand>,
    event_rx: Receiver<SessionEvent>,
}

impl SessionUiHandle {
    /// Send a command to the background protocol task.
    pub async fn send_command(&self, cmd: SessionCommand) -> Result<(), String> {
        self.command_tx
            .send(cmd)
            .await
            .map_err(|e| format!("Failed to dispatch session command: {e}"))
    }

    /// Try to send a command without awaiting (non-blocking).
    pub fn try_send_command(&self, cmd: SessionCommand) -> Result<(), String> {
        match self.command_tx.try_send(cmd) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err("Session command queue is full".into()),
            Err(TrySendError::Closed(_)) => Err("Session worker has terminated".into()),
        }
    }

    /// Try to receive the next pending event without blocking.
    pub fn try_recv_event(&self) -> Result<Option<SessionEvent>, String> {
        match self.event_rx.try_recv() {
            Ok(ev) => Ok(Some(ev)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Closed) => Err("Session worker channel closed".into()),
        }
    }

    /// Await the next event from the background protocol task.
    pub async fn recv_event(&self) -> Result<SessionEvent, String> {
        self.event_rx
            .recv()
            .await
            .map_err(|e| format!("Failed to receive session event: {e}"))
    }
}

/// Protocol worker session channel handle.
#[derive(Debug, Clone)]
pub struct SessionWorkerHandle {
    command_rx: Receiver<SessionCommand>,
    event_tx: Sender<SessionEvent>,
}

impl SessionWorkerHandle {
    /// Send an event from the protocol task to the UI.
    pub async fn send_event(&self, event: SessionEvent) -> Result<(), String> {
        self.event_tx
            .send(event)
            .await
            .map_err(|e| format!("Failed to dispatch session event: {e}"))
    }

    /// Try to send an event from the protocol task to the UI without awaiting.
    pub fn try_send_event(&self, event: SessionEvent) -> Result<(), String> {
        match self.event_tx.try_send(event) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err("Session event queue is full".into()),
            Err(TrySendError::Closed(_)) => Err("UI receiver has disconnected".into()),
        }
    }

    /// Await the next command from the UI.
    pub async fn recv_command(&self) -> Result<SessionCommand, String> {
        self.command_rx
            .recv()
            .await
            .map_err(|e| format!("Failed to receive session command: {e}"))
    }

    /// Try to receive the next pending command without blocking.
    pub fn try_recv_command(&self) -> Result<Option<SessionCommand>, String> {
        match self.command_rx.try_recv() {
            Ok(cmd) => Ok(Some(cmd)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Closed) => Err("UI command channel closed".into()),
        }
    }
}

/// Create a paired UI handle and Worker handle with bounded queue capacity.
pub fn create_session_channels(
    command_cap: usize,
    event_cap: usize,
) -> (SessionUiHandle, SessionWorkerHandle) {
    let (command_tx, command_rx) = bounded(command_cap.max(16));
    let (event_tx, event_rx) = bounded(event_cap.max(64));

    let ui_handle = SessionUiHandle {
        command_tx,
        event_rx,
    };
    let worker_handle = SessionWorkerHandle {
        command_rx,
        event_tx,
    };

    (ui_handle, worker_handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_channel_plumbing_command_and_event_flow() {
        let (ui, worker) = create_session_channels(32, 32);

        // UI sends command to worker
        ui.try_send_command(SessionCommand::Connect).unwrap();
        let cmd = worker.try_recv_command().unwrap();
        assert_eq!(cmd, Some(SessionCommand::Connect));

        // Worker sends event to UI
        worker
            .try_send_event(SessionEvent::StateChanged(SessionState::Connected))
            .unwrap();
        let ev = ui.try_recv_event().unwrap();
        assert_eq!(
            ev,
            Some(SessionEvent::StateChanged(SessionState::Connected))
        );
    }
}

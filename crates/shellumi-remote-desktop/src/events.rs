//! Bidirectional channel communication between UI and protocol worker tasks.

use crate::framebuffer::{FramebufferSnapshot, Rect};
use crate::input::{
    InputEnvelope, KeyEvent, PointerEvent, RemoteInput, RemoteShortcut, SessionIdentity, WheelEvent,
};
use crate::metrics::SessionMetricsSnapshot;
use crate::security::SecurityWarning;
use crate::session::SessionState;
use async_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Direction of a remote desktop file transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    Upload,
    Download,
}

/// Session clipboard synchronization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ClipboardSyncMode {
    Disabled,
    #[default]
    Manual,
    Automatic,
}

/// Per-session clipboard policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardPolicy {
    pub mode: ClipboardSyncMode,
    pub allow_send: bool,
    pub allow_receive: bool,
}

impl Default for ClipboardPolicy {
    fn default() -> Self {
        Self {
            mode: ClipboardSyncMode::Manual,
            allow_send: true,
            allow_receive: true,
        }
    }
}

/// Latched recovery barrier executed by the worker before any new-epoch input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryBarrier {
    pub identity: SessionIdentity,
    pub input_epoch: u64,
    pub barrier_id: u64,
    pub reason: String,
}

/// Commands sent from the UI / main thread to the protocol background worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionCommand {
    /// Initiate connection to the remote endpoint.
    Connect,
    /// Gracefully terminate connection (delivered out-of-band even under queue pressure).
    Disconnect,
    /// Latched recovery barrier (delivered via independent latch after purging stale-epoch input).
    RecoveryBarrier(RecoveryBarrier),
    /// Normalized semantic remote input event (preferred over legacy Pointer/Wheel/Key).
    Input(RemoteInput),
    /// Epoch-tagged normalized input envelope.
    InputEnvelope(InputEnvelope),
    /// Runtime view-only policy transition with monotonic transition ID.
    SetViewOnly { enabled: bool, transition_id: u64 },
    /// Typed atomic shortcut sequence.
    Shortcut(RemoteShortcut),
    /// Legacy mouse pointer motion or button event.
    Pointer(PointerEvent),
    /// Legacy scroll wheel event.
    Wheel(WheelEvent),
    /// Legacy keyboard key event.
    Key(KeyEvent),
    /// Send local clipboard text to the remote host.
    SendClipboard(String),
    /// Send local files to the remote host via clipboard file transfer.
    SendFiles(Vec<std::path::PathBuf>),
    /// Request desktop resize (if protocol extension supported).
    RequestResize { width: u32, height: u32 },
    /// Request a full framebuffer refresh.
    Refresh,
    /// Send Secure Attention Sequence (Ctrl+Alt+Del).
    CtrlAltDel,
    /// Mute or unmute remote desktop audio output.
    SetAudioMute(bool),
    /// Set remote desktop audio volume (0 - 100 percentage).
    SetAudioVolume(u16),
    /// Configure or change shared drive folder path.
    ConfigureDrive(Option<std::path::PathBuf>),
    /// Configure shared drive path and explicit read-only policy.
    ConfigureDrivePolicy {
        path: Option<std::path::PathBuf>,
        read_only: bool,
    },
    /// Update runtime clipboard policy.
    SetClipboardPolicy(ClipboardPolicy),
    /// Cancel an active file transfer by ID.
    CancelTransfer(u64),
    /// Mute or unmute remote desktop microphone capture.
    SetMicrophoneMute(bool),
    /// Set remote desktop microphone capture volume/gain (0 - 100 percentage).
    SetMicrophoneVolume(u16),
}

/// Events emitted by the protocol background worker to the UI / main thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionEvent {
    /// Session connection state changed.
    StateChanged(SessionState),
    /// Worker acknowledged a runtime view-only transition.
    ViewOnlyChanged { enabled: bool, transition_id: u64 },
    /// Worker started a new transport epoch.
    TransportEpochChanged {
        connection_generation: u64,
        transport_epoch: u64,
    },
    /// Worker performed input release recovery (e.g. after ReleaseAll or ViewOnly).
    InputRecovery {
        transport_epoch: u64,
        released_count: usize,
        reason: String,
    },
    /// Worker completed a latched recovery barrier and drained submitted held state.
    RecoveryCompleted {
        identity: SessionIdentity,
        input_epoch: u64,
        barrier_id: u64,
        released_count: usize,
        reason: String,
    },
    /// Worker failed to complete a latched recovery barrier (transport must terminate).
    RecoveryFailed {
        identity: SessionIdentity,
        input_epoch: u64,
        barrier_id: u64,
        reason: String,
    },
    /// Worker failed to submit an input event to the transport.
    InputError {
        transport_epoch: u64,
        seq: u64,
        reason: String,
    },
    /// Remote screen resolution updated.
    RemoteResize { width: u32, height: u32 },
    /// Framebuffer rectangle updates applied.
    Damage { rects: Vec<Rect>, sequence: u64 },
    /// New complete display frame is ready in the shared framebuffer.
    FrameReady {
        sequence: u64,
        #[serde(skip)]
        snapshot: Option<FramebufferSnapshot>,
    },
    /// Clipboard text received from the remote host.
    ServerClipboard(String),
    /// File transfer received from the remote host.
    ServerFilesReceived {
        files: Vec<String>,
        target_dir: String,
    },
    /// Typed file transfer started event.
    TransferStarted {
        transfer_id: u64,
        direction: TransferDirection,
        file_count: usize,
        total_bytes: u64,
    },
    /// Typed file transfer progress update.
    TransferProgress {
        transfer_id: u64,
        file_index: usize,
        file_name: String,
        bytes_transferred: u64,
        file_total_bytes: u64,
    },
    /// Typed file transfer completed event.
    TransferCompleted {
        transfer_id: u64,
        direction: TransferDirection,
        files: Vec<String>,
        target_dir: Option<String>,
    },
    /// Typed file transfer failed event.
    TransferFailed {
        transfer_id: u64,
        direction: TransferDirection,
        reason: String,
    },
    /// Typed file transfer cancelled event.
    TransferCancelled {
        transfer_id: u64,
        direction: TransferDirection,
    },
    /// Remote audio output status and telemetry update.
    AudioStatus {
        active: bool,
        muted: bool,
        volume: u16,
        format: String,
        sample_rate: u32,
        channels: u16,
        bytes_received: u64,
    },
    /// Display control capabilities negotiated via MS-RDPEDISP extension.
    DisplayControlCaps { max_monitors: u32, max_area: u64 },
    /// Display resize submission or negotiation failed in the protocol worker.
    ResizeFailed {
        width: u32,
        height: u32,
        reason: String,
    },
    /// Display control capabilities are no longer available for this session.
    DisplayControlUnavailable { reason: String },
    /// Drive redirection status and telemetry update (RDPDR / MS-RDPEFS).
    DriveStatus {
        drive_name: String,
        local_path: String,
        mounted: bool,
        bytes_read: u64,
        bytes_written: u64,
        operations_count: u64,
    },
    /// Remote microphone input status and telemetry update (MS-RDPEAI).
    MicrophoneStatus {
        active: bool,
        muted: bool,
        volume: u16,
        format: String,
        sample_rate: u32,
        channels: u16,
        bytes_sent: u64,
        packets_sent: u64,
    },
    /// Remote terminal bell event.
    Bell,
    /// Connection or protocol error occurred.
    Error(String),
    /// Security alert or certificate warning.
    SecurityWarning(SecurityWarning),
    /// Measured performance and telemetry metrics snapshot.
    Metrics(SessionMetricsSnapshot),
}

#[derive(Debug)]
struct CommandQueueInner {
    queue: Mutex<VecDeque<SessionCommand>>,
    recovery_latch: Mutex<Option<RecoveryBarrier>>,
    min_valid_input_epoch: AtomicU64,
    capacity: usize,
    closed: AtomicBool,
    disconnect_requested: AtomicBool,
    coalesced_moves: AtomicU64,
    high_water: AtomicUsize,
    notify_tx: Sender<()>,
}

#[derive(Debug)]
struct EventQueueInner {
    control_queue: Mutex<VecDeque<SessionEvent>>,
    latest_frame: Mutex<Option<SessionEvent>>,
    legacy_event_rx: Receiver<SessionEvent>,
    control_capacity: usize,
    closed: AtomicBool,
    coalesced_frames: AtomicU64,
    notify_tx: Sender<()>,
}

/// UI-side session channel handle.
#[derive(Debug, Clone)]
pub struct SessionUiHandle {
    cmd_inner: Arc<CommandQueueInner>,
    ev_inner: Arc<EventQueueInner>,
    ev_notify_rx: Receiver<()>,
}

impl SessionUiHandle {
    /// Send a command to the background protocol task.
    pub async fn send_command(&self, cmd: SessionCommand) -> Result<(), String> {
        self.try_send_command(cmd)
    }

    /// Atomically advance the minimum valid input epoch, purge all queued interactive commands
    /// from abandoned epochs, and latch a `RecoveryBarrier` so the worker drains submitted
    /// held state before accepting any new-epoch input. Never fails even if `queue` is full.
    pub fn trigger_recovery(&self, barrier: RecoveryBarrier) {
        self.cmd_inner
            .min_valid_input_epoch
            .fetch_max(barrier.input_epoch, Ordering::AcqRel);
        {
            let mut q = self.cmd_inner.queue.lock().unwrap();
            let min_epoch = barrier.input_epoch;
            q.retain(|c| !is_stale_interactive_command(c, min_epoch));
        }
        {
            let mut latch = self.cmd_inner.recovery_latch.lock().unwrap();
            *latch = Some(barrier);
        }
        let _ = self.cmd_inner.notify_tx.try_send(());
    }

    /// Try to send a command without awaiting (non-blocking).
    ///
    /// - `Disconnect` is delivered out-of-band and cannot be blocked by a full queue.
    /// - Adjacent compatible `PointerMove` commands are coalesced in place without
    ///   crossing critical input or policy barriers.
    /// - No priority queue overtaking: all admitted commands remain strictly FIFO.
    pub fn try_send_command(&self, cmd: SessionCommand) -> Result<(), String> {
        if self.cmd_inner.closed.load(Ordering::Acquire) {
            return Err("Session worker has terminated".into());
        }

        if matches!(cmd, SessionCommand::Disconnect) {
            let mut q = self.cmd_inner.queue.lock().unwrap();
            if q.len() < self.cmd_inner.capacity {
                q.push_back(cmd);
                let len = q.len();
                self.cmd_inner.high_water.fetch_max(len, Ordering::Relaxed);
            } else {
                // Queue is full: deliver Disconnect out-of-band so it is never starved
                self.cmd_inner
                    .disconnect_requested
                    .store(true, Ordering::Release);
            }
            drop(q);
            let _ = self.cmd_inner.notify_tx.try_send(());
            return Ok(());
        }

        if let SessionCommand::RecoveryBarrier(barrier) = cmd {
            self.trigger_recovery(barrier);
            return Ok(());
        }

        let min_epoch = self.cmd_inner.min_valid_input_epoch.load(Ordering::Acquire);
        if is_stale_interactive_command(&cmd, min_epoch) {
            return Err("Stale input epoch rejected".into());
        }

        let is_policy_barrier = matches!(
            cmd,
            SessionCommand::SetViewOnly { .. }
                | SessionCommand::SetClipboardPolicy(_)
                | SessionCommand::ConfigureDrivePolicy { .. }
        );

        let mut q = self.cmd_inner.queue.lock().unwrap();
        match &cmd {
            SessionCommand::Input(RemoteInput::PointerMove { x, y }) => {
                if let Some(SessionCommand::Input(RemoteInput::PointerMove {
                    x: prev_x,
                    y: prev_y,
                })) = q.back_mut()
                {
                    *prev_x = *x;
                    *prev_y = *y;
                    self.cmd_inner
                        .coalesced_moves
                        .fetch_add(1, Ordering::Relaxed);
                    let _ = self.cmd_inner.notify_tx.try_send(());
                    return Ok(());
                }
            }
            SessionCommand::InputEnvelope(env) => {
                if let RemoteInput::PointerMove { x, y } = env.input {
                    if let Some(SessionCommand::InputEnvelope(prev)) = q.back_mut() {
                        if prev.transport_epoch == env.transport_epoch
                            && prev.input_epoch == env.input_epoch
                            && prev.policy_revision == env.policy_revision
                            && matches!(prev.input, RemoteInput::PointerMove { .. })
                        {
                            prev.input = RemoteInput::PointerMove { x, y };
                            prev.seq = env.seq;
                            self.cmd_inner
                                .coalesced_moves
                                .fetch_add(1, Ordering::Relaxed);
                            let _ = self.cmd_inner.notify_tx.try_send(());
                            return Ok(());
                        }
                    }
                }
            }
            _ => {}
        }

        if q.len() >= self.cmd_inner.capacity {
            // Evict only safely coalescible PointerMove commands if a non-move command arrives
            q.retain(|c| {
                !matches!(
                    c,
                    SessionCommand::Input(RemoteInput::PointerMove { .. })
                        | SessionCommand::InputEnvelope(InputEnvelope {
                            input: RemoteInput::PointerMove { .. },
                            ..
                        })
                )
            });
        }

        if q.len() >= self.cmd_inner.capacity {
            if is_policy_barrier {
                // Policy barrier under full interactive queue: purge interactive commands,
                // advance epoch, and latch a recovery barrier so no old press can follow policy.
                let next_epoch = self
                    .cmd_inner
                    .min_valid_input_epoch
                    .fetch_add(1, Ordering::AcqRel)
                    + 1;
                q.retain(|c| !is_stale_interactive_command(c, next_epoch));
                if q.len() < self.cmd_inner.capacity {
                    let mut latch = self.cmd_inner.recovery_latch.lock().unwrap();
                    if latch.is_none() {
                        *latch = Some(RecoveryBarrier {
                            identity: SessionIdentity::default(),
                            input_epoch: next_epoch,
                            barrier_id: next_epoch,
                            reason: "Policy barrier under queue saturation".into(),
                        });
                    }
                    q.push_back(cmd);
                    let len = q.len();
                    self.cmd_inner.high_water.fetch_max(len, Ordering::Relaxed);
                    drop(latch);
                    drop(q);
                    let _ = self.cmd_inner.notify_tx.try_send(());
                    return Ok(());
                }
            }
            return Err("Session command queue is full".into());
        }

        q.push_back(cmd);
        let len = q.len();
        self.cmd_inner.high_water.fetch_max(len, Ordering::Relaxed);
        drop(q);
        let _ = self.cmd_inner.notify_tx.try_send(());
        Ok(())
    }

    /// Return current command queue depth, high-water mark, and coalesced move count.
    pub fn command_queue_stats(&self) -> (usize, usize, u64) {
        let len = self.cmd_inner.queue.lock().unwrap().len();
        let hw = self.cmd_inner.high_water.load(Ordering::Relaxed);
        let coalesced = self.cmd_inner.coalesced_moves.load(Ordering::Relaxed);
        (len, hw, coalesced)
    }

    /// Try to receive the next pending event without blocking.
    /// Control events are prioritized over coalesced `FrameReady` snapshots.
    pub fn try_recv_event(&self) -> Result<Option<SessionEvent>, String> {
        while let Ok(ev) = self.ev_inner.legacy_event_rx.try_recv() {
            if matches!(ev, SessionEvent::FrameReady { .. }) {
                *self.ev_inner.latest_frame.lock().unwrap() = Some(ev);
            } else {
                let mut cq = self.ev_inner.control_queue.lock().unwrap();
                if cq.len() < self.ev_inner.control_capacity {
                    cq.push_back(ev);
                }
            }
        }
        {
            let mut cq = self.ev_inner.control_queue.lock().unwrap();
            if let Some(ev) = cq.pop_front() {
                return Ok(Some(ev));
            }
        }
        {
            let mut lf = self.ev_inner.latest_frame.lock().unwrap();
            if let Some(frame) = lf.take() {
                return Ok(Some(frame));
            }
        }
        if self.ev_inner.closed.load(Ordering::Acquire) {
            return Err("Session worker channel closed".into());
        }
        Ok(None)
    }

    /// Await the next event from the background protocol task.
    pub async fn recv_event(&self) -> Result<SessionEvent, String> {
        loop {
            if let Some(ev) = self.try_recv_event()? {
                return Ok(ev);
            }
            if self.ev_notify_rx.recv().await.is_err() {
                return self
                    .try_recv_event()?
                    .ok_or_else(|| "Session worker channel closed".to_string());
            }
        }
    }
}

/// Protocol worker session channel handle.
#[derive(Debug, Clone)]
pub struct SessionWorkerHandle {
    cmd_inner: Arc<CommandQueueInner>,
    cmd_notify_rx: Receiver<()>,
    ev_inner: Arc<EventQueueInner>,
    legacy_event_tx: Sender<SessionEvent>,
}

impl SessionWorkerHandle {
    /// Send an event from the protocol task to the UI.
    pub async fn send_event(&self, event: SessionEvent) -> Result<(), String> {
        self.try_send_event(event)
    }

    /// Try to send an event from the protocol task to the UI without awaiting.
    /// `FrameReady` events coalesce into a dedicated 1-slot buffer so they never
    /// starve critical control events (`StateChanged`, `Error`, `ServerClipboard`, etc.).
    pub fn try_send_event(&self, event: SessionEvent) -> Result<(), String> {
        if self.ev_inner.closed.load(Ordering::Acquire) {
            return Err("UI receiver has disconnected".into());
        }

        if matches!(event, SessionEvent::FrameReady { .. }) {
            let mut lf = self.ev_inner.latest_frame.lock().unwrap();
            if lf.is_some() {
                self.ev_inner
                    .coalesced_frames
                    .fetch_add(1, Ordering::Relaxed);
            }
            *lf = Some(event);
            drop(lf);
            let _ = self.ev_inner.notify_tx.try_send(());
            return Ok(());
        }

        let mut cq = self.ev_inner.control_queue.lock().unwrap();
        if cq.len() >= self.ev_inner.control_capacity {
            return Err("Session event queue is full".into());
        }
        cq.push_back(event);
        drop(cq);
        let _ = self.ev_inner.notify_tx.try_send(());
        Ok(())
    }

    /// Return a clone of a bridge sender for legacy backends; drains into `ev_inner` via `try_send_event`
    /// or use `worker.try_send_event` directly.
    pub fn event_sender(&self) -> Sender<SessionEvent> {
        self.legacy_event_tx.clone()
    }

    /// Drain any events sent through `event_sender()` into the ordered control queue.
    pub fn drain_legacy_events(&self) {
        while let Ok(ev) = self.ev_inner.legacy_event_rx.try_recv() {
            let _ = self.try_send_event(ev);
        }
    }

    /// Await the next command from the UI.
    pub async fn recv_command(&self) -> Result<SessionCommand, String> {
        loop {
            if let Some(cmd) = self.try_recv_command()? {
                return Ok(cmd);
            }
            if self.cmd_notify_rx.recv().await.is_err() {
                return self
                    .try_recv_command()?
                    .ok_or_else(|| "UI command channel closed".to_string());
            }
        }
    }

    /// Try to receive the next pending command without blocking.
    pub fn try_recv_command(&self) -> Result<Option<SessionCommand>, String> {
        if self
            .cmd_inner
            .disconnect_requested
            .swap(false, Ordering::AcqRel)
        {
            return Ok(Some(SessionCommand::Disconnect));
        }
        {
            let mut latch = self.cmd_inner.recovery_latch.lock().unwrap();
            if let Some(barrier) = latch.take() {
                return Ok(Some(SessionCommand::RecoveryBarrier(barrier)));
            }
        }
        let mut q = self.cmd_inner.queue.lock().unwrap();
        if let Some(cmd) = q.pop_front() {
            return Ok(Some(cmd));
        }
        drop(q);
        if self.cmd_inner.closed.load(Ordering::Acquire) {
            return Err("UI command channel closed".into());
        }
        Ok(None)
    }

    /// Close the worker side of the event channel.
    pub fn close(&self) {
        self.ev_inner.closed.store(true, Ordering::Release);
        self.ev_inner.notify_tx.close();
    }
}

fn is_stale_interactive_command(cmd: &SessionCommand, min_valid_input_epoch: u64) -> bool {
    match cmd {
        SessionCommand::InputEnvelope(env) => env.input_epoch < min_valid_input_epoch,
        SessionCommand::Input(_)
        | SessionCommand::Shortcut(_)
        | SessionCommand::Pointer(_)
        | SessionCommand::Wheel(_)
        | SessionCommand::Key(_) => min_valid_input_epoch > 0,
        _ => false,
    }
}

/// Create a paired UI handle and Worker handle with bounded queue capacity.
pub fn create_session_channels(
    command_cap: usize,
    event_cap: usize,
) -> (SessionUiHandle, SessionWorkerHandle) {
    let (cmd_notify_tx, cmd_notify_rx) = bounded(1);
    let (ev_notify_tx, ev_notify_rx) = bounded(1);
    let (legacy_event_tx, legacy_event_rx) = bounded::<SessionEvent>(event_cap.max(64));

    let cmd_inner = Arc::new(CommandQueueInner {
        queue: Mutex::new(VecDeque::with_capacity(command_cap.max(16))),
        recovery_latch: Mutex::new(None),
        min_valid_input_epoch: AtomicU64::new(0),
        capacity: command_cap.max(16),
        closed: AtomicBool::new(false),
        disconnect_requested: AtomicBool::new(false),
        coalesced_moves: AtomicU64::new(0),
        high_water: AtomicUsize::new(0),
        notify_tx: cmd_notify_tx,
    });

    let ev_inner = Arc::new(EventQueueInner {
        control_queue: Mutex::new(VecDeque::with_capacity(event_cap.max(64))),
        latest_frame: Mutex::new(None),
        legacy_event_rx,
        control_capacity: event_cap.max(64),
        closed: AtomicBool::new(false),
        coalesced_frames: AtomicU64::new(0),
        notify_tx: ev_notify_tx,
    });

    let ui_handle = SessionUiHandle {
        cmd_inner: Arc::clone(&cmd_inner),
        ev_inner: Arc::clone(&ev_inner),
        ev_notify_rx,
    };
    let worker_handle = SessionWorkerHandle {
        cmd_inner,
        cmd_notify_rx,
        ev_inner,
        legacy_event_tx,
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

        // UI sends SendFiles command
        let test_paths = vec![std::path::PathBuf::from("/tmp/test.txt")];
        ui.try_send_command(SessionCommand::SendFiles(test_paths.clone()))
            .unwrap();
        let cmd = worker.try_recv_command().unwrap();
        assert_eq!(cmd, Some(SessionCommand::SendFiles(test_paths)));

        // Worker sends ServerFilesReceived event
        let files = vec!["download.txt".to_string()];
        let target_dir = "/tmp/downloads".to_string();
        worker
            .try_send_event(SessionEvent::ServerFilesReceived {
                files: files.clone(),
                target_dir: target_dir.clone(),
            })
            .unwrap();
        let ev = ui.try_recv_event().unwrap();
        assert_eq!(
            ev,
            Some(SessionEvent::ServerFilesReceived { files, target_dir })
        );

        // UI sends audio mute and volume commands
        ui.try_send_command(SessionCommand::SetAudioMute(true))
            .unwrap();
        assert_eq!(
            worker.try_recv_command().unwrap(),
            Some(SessionCommand::SetAudioMute(true))
        );
        ui.try_send_command(SessionCommand::SetAudioVolume(80))
            .unwrap();
        assert_eq!(
            worker.try_recv_command().unwrap(),
            Some(SessionCommand::SetAudioVolume(80))
        );

        // Worker sends AudioStatus event
        let audio_status = SessionEvent::AudioStatus {
            active: true,
            muted: false,
            volume: 80,
            format: "PCM".to_string(),
            sample_rate: 44100,
            channels: 2,
            bytes_received: 1024,
        };
        worker.try_send_event(audio_status.clone()).unwrap();
        assert_eq!(ui.try_recv_event().unwrap(), Some(audio_status));

        // Worker sends DisplayControlCaps event
        let caps = SessionEvent::DisplayControlCaps {
            max_monitors: 16,
            max_area: 67108864,
        };
        worker.try_send_event(caps.clone()).unwrap();
        assert_eq!(ui.try_recv_event().unwrap(), Some(caps));
    }

    #[test]
    fn stresses_coalesced_moves_interleaved_with_critical_events_and_frame_non_starvation() {
        let (ui, worker) = create_session_channels(64, 64);

        // 10,000 synthetic moves interleaved with critical button down/up barriers
        ui.try_send_command(SessionCommand::Input(RemoteInput::PointerButton {
            button: crate::input::MouseButton::Left,
            down: true,
            x: 10,
            y: 20,
        }))
        .unwrap();

        for i in 0..10_000u32 {
            ui.try_send_command(SessionCommand::Input(RemoteInput::PointerMove {
                x: i % 1920,
                y: (i * 2) % 1080,
            }))
            .unwrap();
        }

        ui.try_send_command(SessionCommand::Input(RemoteInput::PointerButton {
            button: crate::input::MouseButton::Left,
            down: false,
            x: 1919,
            y: 1079,
        }))
        .unwrap();

        let (depth, _hw, coalesced) = ui.command_queue_stats();
        assert_eq!(depth, 3); // ButtonDown + coalesced Move + ButtonUp
        assert_eq!(coalesced, 9_999);

        assert!(matches!(
            worker.try_recv_command().unwrap(),
            Some(SessionCommand::Input(RemoteInput::PointerButton {
                down: true,
                ..
            }))
        ));
        assert_eq!(
            worker.try_recv_command().unwrap(),
            Some(SessionCommand::Input(RemoteInput::PointerMove {
                x: 9999 % 1920,
                y: (9999 * 2) % 1080,
            }))
        );
        assert!(matches!(
            worker.try_recv_command().unwrap(),
            Some(SessionCommand::Input(RemoteInput::PointerButton {
                down: false,
                ..
            }))
        ));

        // Flood FrameReady events; control event must still be delivered first without starvation
        for seq in 0..500u64 {
            worker
                .try_send_event(SessionEvent::FrameReady {
                    sequence: seq,
                    snapshot: None,
                })
                .unwrap();
        }
        worker
            .try_send_event(SessionEvent::StateChanged(SessionState::Connected))
            .unwrap();
        assert_eq!(
            ui.try_recv_event().unwrap(),
            Some(SessionEvent::StateChanged(SessionState::Connected))
        );
        assert_eq!(
            ui.try_recv_event().unwrap(),
            Some(SessionEvent::FrameReady {
                sequence: 499,
                snapshot: None
            })
        );
    }

    #[test]
    fn non_coalescible_queue_saturation_purges_abandoned_presses_and_latches_recovery() {
        let (ui, worker) = create_session_channels(16, 16);
        let identity = SessionIdentity::new(1, 1, 1);

        // Fill all 16 slots with non-coalescible PhysicalKey presses in input_epoch 1
        for i in 0..16u16 {
            ui.try_send_command(SessionCommand::InputEnvelope(InputEnvelope {
                tab_id: 1,
                connection_generation: 1,
                transport_epoch: 1,
                input_epoch: 1,
                policy_revision: 0,
                seq: i as u64 + 1,
                input: RemoteInput::PhysicalKey {
                    key: crate::input::RemoteKey {
                        keysym: 0x0061 + i as u32,
                        scancode: 0x1E + i,
                        extended: false,
                        is_modifier: false,
                        key_name: format!("k{i}"),
                    },
                    down: true,
                    repeat: false,
                },
            }))
            .unwrap();
        }
        // 17th ordinary key press must be rejected cleanly
        assert!(ui
            .try_send_command(SessionCommand::InputEnvelope(InputEnvelope {
                tab_id: 1,
                connection_generation: 1,
                transport_epoch: 1,
                input_epoch: 1,
                policy_revision: 0,
                seq: 17,
                input: RemoteInput::PhysicalKey {
                    key: crate::input::RemoteKey {
                        keysym: 0x007A,
                        scancode: 0x2C,
                        extended: false,
                        is_modifier: false,
                        key_name: "z".into(),
                    },
                    down: true,
                    repeat: false,
                },
            }))
            .is_err());

        // Trigger latched recovery barrier for input_epoch 2 + SetViewOnly + Disconnect
        ui.trigger_recovery(RecoveryBarrier {
            identity,
            input_epoch: 2,
            barrier_id: 1,
            reason: "Queue saturation".into(),
        });
        ui.try_send_command(SessionCommand::SetViewOnly {
            enabled: true,
            transition_id: 42,
        })
        .unwrap();
        ui.try_send_command(SessionCommand::Disconnect).unwrap();

        // Any stale input_epoch 1 command must be rejected immediately
        assert!(ui
            .try_send_command(SessionCommand::InputEnvelope(InputEnvelope {
                tab_id: 1,
                connection_generation: 1,
                transport_epoch: 1,
                input_epoch: 1,
                policy_revision: 0,
                seq: 18,
                input: RemoteInput::PointerMove { x: 5, y: 5 },
            }))
            .is_err());

        // Worker receives RecoveryBarrier, SetViewOnly, Disconnect, and ZERO abandoned epoch-1 presses
        let mut seen = Vec::new();
        while let Some(cmd) = worker.try_recv_command().unwrap() {
            assert!(
                !matches!(cmd, SessionCommand::InputEnvelope(_)),
                "abandoned epoch-1 press must never follow recovery barrier"
            );
            seen.push(cmd);
        }
        assert!(seen
            .iter()
            .any(|c| matches!(c, SessionCommand::RecoveryBarrier(b) if b.input_epoch == 2)));
        assert!(seen.iter().any(|c| matches!(
            c,
            SessionCommand::SetViewOnly {
                enabled: true,
                transition_id: 42
            }
        )));
        assert!(seen.iter().any(|c| matches!(c, SessionCommand::Disconnect)));
    }
}

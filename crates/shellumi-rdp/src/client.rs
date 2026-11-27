//! High-level asynchronous RDP client session driver powered by IronRDP.

use crate::config::RdpClientConfig;
use crate::input::translate_remote_input;
use ironrdp_client::rdp::{RdpClient, RdpInputEvent, RdpOutputEvent};
use shellumi_remote_desktop::{
    Framebuffer, Rect, SessionCommand, SessionConfig, SessionEvent, SessionMetrics, SessionState,
    SessionWorkerHandle,
};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc;

/// RDP client error conditions.
#[derive(Debug, Error)]
pub enum RdpError {
    #[error("Configuration error: {0}")]
    Config(#[from] crate::config::RdpConfigError),

    #[error("TLS certificate validation error: {0}")]
    Certificate(#[from] crate::security::CertificateError),

    #[error("Connection failure: {0}")]
    Connection(String),

    #[error("Session terminated: {0}")]
    Terminated(String),

    #[error("Connection timed out after {0:?}")]
    Timeout(Duration),
}

/// Run an authentic RDP session using IronRDP client engine with automatic network reconnection.
pub async fn run_rdp_session(
    config: SessionConfig,
    worker: SessionWorkerHandle,
    mut framebuffer: Framebuffer,
) -> Result<(), RdpError> {
    let policy = config.reconnect_policy;
    let mut attempt = 0;
    let mut view_only = config.view_only;
    let mut transport_epoch: u64 = 0;

    loop {
        transport_epoch = transport_epoch.wrapping_add(1);
        let _ = worker
            .send_event(SessionEvent::TransportEpochChanged {
                connection_generation: 0,
                transport_epoch,
            })
            .await;
        let (attempt_res, was_connected) = run_rdp_session_attempt(
            &config,
            &worker,
            &mut framebuffer,
            &mut view_only,
            transport_epoch,
        )
        .await;

        if was_connected {
            attempt = 0;
        }

        match attempt_res {
            Ok(()) => {
                // User disconnected cleanly
                let _ = worker
                    .send_event(SessionEvent::StateChanged(SessionState::Disconnected))
                    .await;
                return Ok(());
            }
            Err(err) => {
                let err_msg = err.to_string();
                tracing::error!(
                    host = %config.host,
                    port = config.port,
                    attempt = attempt + 1,
                    error = %err,
                    "RDP connection attempt failed"
                );

                // Authentication/CredSSP/Certificate errors or explicit protocol negotiation errors must not be retried blindly!
                let is_auth_or_fatal = err_msg.contains("CredSSP")
                    || err_msg.contains("authentication rejected")
                    || err_msg.contains("Authentication")
                    || err_msg.contains("Certificate")
                    || err_msg.contains("protocol negotiation failed")
                    || err_msg.contains("Configuration error");

                if is_auth_or_fatal || !policy.enabled || attempt >= policy.max_attempts {
                    let _ = worker
                        .send_event(SessionEvent::Error(err_msg.clone()))
                        .await;
                    let _ = worker
                        .send_event(SessionEvent::StateChanged(SessionState::Error(err_msg)))
                        .await;
                    return Err(err);
                }

                attempt += 1;
                let delay = policy.compute_delay(attempt);
                let delay_ms = delay.as_millis() as u64;

                let _ = worker
                    .send_event(SessionEvent::StateChanged(SessionState::Reconnecting {
                        attempt,
                        max_attempts: policy.max_attempts,
                        delay_ms,
                    }))
                    .await;

                let mut cancelled = false;
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    cmd = worker.recv_command() => {
                        if let Ok(SessionCommand::Disconnect) = cmd {
                            cancelled = true;
                        }
                    }
                }

                if cancelled {
                    let _ = worker
                        .send_event(SessionEvent::StateChanged(SessionState::Disconnected))
                        .await;
                    return Ok(());
                }
            }
        }
    }
}

/// Execute a single attempt of an RDP session.
/// Returns `(Result<(), RdpError>, bool /* was_connected */)`.
async fn run_rdp_session_attempt(
    config: &SessionConfig,
    worker: &SessionWorkerHandle,
    framebuffer: &mut Framebuffer,
    view_only: &mut bool,
    transport_epoch: u64,
) -> (Result<(), RdpError>, bool) {
    let _ = worker
        .send_event(SessionEvent::StateChanged(SessionState::Connecting))
        .await;

    crate::security::ensure_crypto_provider_installed();

    tracing::info!(
        host = %config.host,
        port = config.port,
        tls_strict = config.tls_strict,
        "Initiating RDP connection attempt"
    );

    let rdp_config = RdpClientConfig::from_session_config(config);
    let proxy = std::sync::Arc::new(std::sync::Mutex::new(None));
    let staging = match crate::clipboard::PrivateStaging::create() {
        Ok(dir) => Some(dir),
        Err(err) => {
            tracing::error!(error = %err, "Failed to create private RDP staging directory; file receive disabled");
            None
        }
    };
    let clipboard_backend = crate::clipboard::ShellumiCliprdrBackend::new_with_staging(
        proxy.clone(),
        worker.event_sender(),
        staging,
    );
    let sound_control = crate::sound::ShellumiRdpsndControl::new(false, 100);
    let display_control = crate::display::DisplayControlState::new();
    let microphone_control = crate::microphone::MicrophoneControlState::new(false, 100);
    // Explicit authorization only: never implicitly share Downloads or temp directories.
    let drive_control = rdp_config.shared_folder.as_ref().map(|path| {
        let drive_name = crate::drive::derive_drive_name(path);
        crate::drive::DriveControlState::new(
            crate::drive::DEFAULT_DRIVE_DEVICE_ID,
            drive_name,
            path.clone(),
        )
    });
    let ironrdp_cfg = match rdp_config.to_ironrdp_config_with_backends(
        Some(clipboard_backend.clone()),
        Some(sound_control.clone()),
        Some(display_control.clone()),
        drive_control.clone(),
        Some(microphone_control.clone()),
        Some(worker.event_sender().clone()),
    ) {
        Ok(c) => c,
        Err(e) => return (Err(e.into()), false),
    };

    let (out_tx, mut out_rx) = mpsc::channel(64);
    let client = RdpClient::new(ironrdp_cfg, out_tx);
    let input_sender = client.input_sender();
    *proxy.lock().unwrap() = Some(input_sender.clone());
    microphone_control.set_input_sender(input_sender.clone());

    // Spawn IronRDP background runner in a dedicated OS thread with current-thread runtime
    // to bypass higher-ranked lifetime Send limitations in ironrdp-client's internal futures.
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let rdp_thread = match std::thread::Builder::new()
        .name("shellumi-rdp-runner".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to initialize RDP runtime: {e}");
                    return;
                }
            };

            crate::security::ensure_crypto_provider_installed();

            let run_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rt.block_on(async move {
                    tokio::select! {
                        _ = client.run() => {}
                        _ = &mut stop_rx => {}
                    }
                });
            }));
            if let Err(panic_info) = run_res {
                let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Non-string panic payload in RDP engine".to_string()
                };
                tracing::error!(panic = %msg, "RDP runner thread panicked");
            }
        }) {
        Ok(t) => t,
        Err(e) => {
            return (
                Err(RdpError::Connection(format!(
                    "Failed to spawn RDP runner thread: {e}"
                ))),
                false,
            );
        }
    };

    let mut metrics = SessionMetrics::new();
    let mut metrics_interval = tokio::time::interval(Duration::from_millis(1000));
    let mut connected = false;
    let mut failure: Option<RdpError> = None;
    let mut ledger = shellumi_remote_desktop::SubmittedInputLedger::new();
    let mut clipboard_policy = shellumi_remote_desktop::ClipboardPolicy::default();
    let mut worker_min_input_epoch: u64 = 0;
    if *view_only {
        clipboard_backend.set_policy(false, false);
    }

    loop {
        worker.drain_legacy_events();
        tokio::select! {
            // Outbound UI commands
            cmd_res = worker.recv_command() => {
                match cmd_res {
                    Ok(cmd) => match cmd {
                        SessionCommand::RecoveryBarrier(barrier) => {
                            if barrier.identity.transport_epoch != 0
                                && barrier.identity.transport_epoch != transport_epoch
                            {
                                continue;
                            }
                            worker_min_input_epoch = worker_min_input_epoch.max(barrier.input_epoch);
                            let releases = ledger.drain_releases();
                            let released_count = releases.len();
                            let mut failed_reason: Option<String> = None;
                            for rel in releases {
                                let events = translate_remote_input(&rel);
                                if !events.is_empty() {
                                    if let Err(err) = input_sender.send(RdpInputEvent::FastPath(events.into())) {
                                        failed_reason = Some(err.to_string());
                                        break;
                                    }
                                }
                            }
                            if let Some(reason) = failed_reason {
                                let _ = worker
                                    .send_event(SessionEvent::RecoveryFailed {
                                        identity: barrier.identity,
                                        input_epoch: barrier.input_epoch,
                                        barrier_id: barrier.barrier_id,
                                        reason: reason.clone(),
                                    })
                                    .await;
                                let _ = input_sender.send(RdpInputEvent::Close);
                                let _ = stop_tx.send(());
                                return (
                                    Err(RdpError::Terminated(format!(
                                        "Input recovery barrier failed: {reason}"
                                    ))),
                                    connected,
                                );
                            }
                            let _ = worker
                                .send_event(SessionEvent::RecoveryCompleted {
                                    identity: barrier.identity,
                                    input_epoch: barrier.input_epoch,
                                    barrier_id: barrier.barrier_id,
                                    released_count,
                                    reason: barrier.reason,
                                })
                                .await;
                        }
                        SessionCommand::SetViewOnly {
                            identity,
                            enabled,
                            transition_id,
                        } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            if enabled && !*view_only {
                                let releases = ledger.drain_releases();
                                let released_count = releases.len();
                                for rel in releases {
                                    let events = translate_remote_input(&rel);
                                    if !events.is_empty() {
                                        if let Err(err) = input_sender.send(RdpInputEvent::FastPath(events.into())) {
                                            let reason = format!("View-only release submission failed: {err}");
                                            let _ = worker.send_event(SessionEvent::Error(reason)).await;
                                            let _ = input_sender.send(RdpInputEvent::Close);
                                            let _ = stop_tx.send(());
                                            return (
                                                Err(RdpError::Terminated("View-only release submission failed".into())),
                                                connected,
                                            );
                                        }
                                    }
                                }
                                if released_count > 0 {
                                    let _ = worker.send_event(SessionEvent::InputRecovery {
                                        transport_epoch,
                                        released_count,
                                        reason: "View-only enabled".into(),
                                    }).await;
                                }
                                microphone_control.revoke();
                                clipboard_backend.set_policy(false, false);
                                if let Some(ref dc) = drive_control {
                                    dc.set_revoked(true);
                                }
                            } else if !enabled {
                                let clip_on = clipboard_policy.mode != shellumi_remote_desktop::ClipboardSyncMode::Disabled;
                                clipboard_backend.set_policy(
                                    clip_on && clipboard_policy.allow_send,
                                    clip_on && clipboard_policy.allow_receive,
                                );
                            }
                            *view_only = enabled;
                            let _ = worker
                                .send_event(SessionEvent::ViewOnlyChanged {
                                    identity,
                                    enabled,
                                    transition_id,
                                })
                                .await;
                        }
                        SessionCommand::InputEnvelope(env) => {
                            if env.transport_epoch == 0
                                || env.transport_epoch != transport_epoch
                                || env.input_epoch < worker_min_input_epoch
                            {
                                tracing::debug!(
                                    expected_epoch = transport_epoch,
                                    got_epoch = env.transport_epoch,
                                    got_input_epoch = env.input_epoch,
                                    min_input_epoch = worker_min_input_epoch,
                                    "Dropped stale RDP input envelope"
                                );
                                continue;
                            }
                            if matches!(env.input, shellumi_remote_desktop::RemoteInput::ReleaseAll) {
                                let releases = ledger.drain_releases();
                                let released_count = releases.len();
                                for rel in releases {
                                    let events = translate_remote_input(&rel);
                                    if !events.is_empty() {
                                        if let Err(err) = input_sender.send(RdpInputEvent::FastPath(events.into())) {
                                            let _ = worker.send_event(SessionEvent::InputError {
                                                transport_epoch,
                                                seq: env.seq,
                                                reason: err.to_string(),
                                            }).await;
                                        }
                                    }
                                }
                                let _ = worker.send_event(SessionEvent::InputRecovery {
                                    transport_epoch,
                                    released_count,
                                    reason: "ReleaseAll".into(),
                                }).await;
                                continue;
                            }
                            let is_release = matches!(
                                env.input,
                                shellumi_remote_desktop::RemoteInput::PointerButton { down: false, .. }
                                    | shellumi_remote_desktop::RemoteInput::PhysicalKey { down: false, .. }
                            );
                            if !*view_only || is_release {
                                let events = translate_remote_input(&env.input);
                                if !events.is_empty() {
                                    match input_sender.send(RdpInputEvent::FastPath(events.into())) {
                                        Ok(()) => ledger.record_submitted(&env.input),
                                        Err(err) => {
                                            let _ = worker.send_event(SessionEvent::InputError {
                                                transport_epoch,
                                                seq: env.seq,
                                                reason: err.to_string(),
                                            }).await;
                                        }
                                    }
                                }
                            }
                        }
                        SessionCommand::SendClipboard { identity, text } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            let send_allowed = !*view_only
                                && clipboard_policy.mode != shellumi_remote_desktop::ClipboardSyncMode::Disabled
                                && clipboard_policy.allow_send;
                            if send_allowed {
                                clipboard_backend.set_local_text(text);
                            }
                        }
                        SessionCommand::SendFiles { identity, paths } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            let send_allowed = !*view_only
                                && clipboard_policy.mode != shellumi_remote_desktop::ClipboardSyncMode::Disabled
                                && clipboard_policy.allow_send;
                            if send_allowed {
                                if let Err(e) = clipboard_backend.stage_files(&paths) {
                                    tracing::warn!("Failed to stage files for RDP transfer: {e}");
                                }
                            }
                        }
                        SessionCommand::SetAudioMute { identity, muted } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            sound_control.set_muted(muted);
                            let _ = worker
                                .send_event(sound_control.current_status_snapshot())
                                .await;
                        }
                        SessionCommand::SetAudioVolume { identity, volume } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            sound_control.set_volume(volume);
                            let _ = worker
                                .send_event(sound_control.current_status_snapshot())
                                .await;
                        }
                        SessionCommand::SetMicrophoneMute { identity, muted } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            if !*view_only {
                                microphone_control.set_muted(muted);
                            }
                            let _ = worker.send_event(microphone_control.status_snapshot()).await;
                        }
                        SessionCommand::SetMicrophoneVolume { identity, volume } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            if !*view_only {
                                microphone_control.set_volume(volume);
                            }
                            let _ = worker.send_event(microphone_control.status_snapshot()).await;
                        }
                        SessionCommand::RequestResize {
                            identity,
                            width,
                            height,
                        } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            let (adj_w, adj_h) = crate::display::adjust_display_size(width, height);
                            tracing::debug!(
                                width,
                                height,
                                adj_w,
                                adj_h,
                                "Dispatching MS-RDPEDISP dynamic resize event to RDP engine"
                            );
                            if let Err(err) = input_sender.send(RdpInputEvent::Resize {
                                width: adj_w as u16,
                                height: adj_h as u16,
                                scale_factor: 100,
                                physical_size: None,
                            }) {
                                tracing::warn!(
                                    width = adj_w,
                                    height = adj_h,
                                    error = %err,
                                    "Failed to submit MS-RDPEDISP resize event to RDP worker"
                                );
                                let _ = worker
                                    .send_event(SessionEvent::ResizeFailed {
                                        width: adj_w,
                                        height: adj_h,
                                        reason: err.to_string(),
                                    })
                                    .await;
                            }
                        }
                        SessionCommand::ConfigureDrivePolicy {
                            identity,
                            path,
                            read_only,
                        } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            if *view_only {
                                continue;
                            }
                            if let Some(ref dc) = drive_control {
                                dc.set_read_only(read_only);
                                match path {
                                    Some(target_path) => {
                                        let d_name = crate::drive::derive_drive_name(&target_path);
                                        dc.update_folder(d_name.clone(), target_path.clone());
                                    }
                                    None => {
                                        dc.set_revoked(true);
                                    }
                                }
                                let _ = worker
                                    .send_event(SessionEvent::DriveStatus {
                                        drive_name: dc.drive_name(),
                                        local_path: dc.local_path().to_string_lossy().to_string(),
                                        mounted: dc.is_mounted() && !dc.is_revoked(),
                                        bytes_read: dc.bytes_read(),
                                        bytes_written: dc.bytes_written(),
                                        operations_count: dc.operations_count(),
                                    })
                                    .await;
                            }
                        }
                        SessionCommand::SetClipboardPolicy { identity, policy } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            clipboard_policy = policy;
                            let enabled = policy.mode != shellumi_remote_desktop::ClipboardSyncMode::Disabled && !*view_only;
                            clipboard_backend.set_policy(
                                enabled && policy.allow_send,
                                enabled && policy.allow_receive,
                            );
                        }
                        SessionCommand::CancelTransfer {
                            identity,
                            transfer_id,
                        } => {
                            if !identity.is_transport_bound() || identity.transport_epoch != transport_epoch {
                                continue;
                            }
                            let _ = clipboard_backend.cancel_transfer(transfer_id);
                        }
                        SessionCommand::Disconnect => {
                            for rel in ledger.drain_releases() {
                                let events = translate_remote_input(&rel);
                                if !events.is_empty() {
                                    let _ = input_sender.send(RdpInputEvent::FastPath(events.into()));
                                }
                            }
                            sound_control.set_active(false);
                            microphone_control.set_recording(false);
                            let _ = input_sender.send(RdpInputEvent::Close);
                            break;
                        }
                        _ => {}
                    },
                    Err(_) => {
                        sound_control.set_active(false);
                        microphone_control.set_recording(false);
                        let _ = input_sender.send(RdpInputEvent::Close);
                        break;
                    }
                }
            }

            // Periodic metrics reporting
            _ = metrics_interval.tick() => {
                let snap = metrics.snapshot(Instant::now());
                let _ = worker.send_event(SessionEvent::Metrics(snap)).await;
            }

            // Inbound events from IronRDP client runner
            out_res = out_rx.recv() => {
                match out_res {
                    Some(event) => match event {
                        RdpOutputEvent::Image { buffer, width, height } => {
                            let decode_start = Instant::now();
                            let w = width.get() as u32;
                            let h = height.get() as u32;

                            if !connected {
                                connected = true;
                                tracing::info!(
                                    host = %config.host,
                                    port = config.port,
                                    width = w,
                                    height = h,
                                    "RDP session connected"
                                );
                                let _ = worker.send_event(SessionEvent::StateChanged(SessionState::Connected)).await;
                                let _ = worker.send_event(SessionEvent::RemoteResize { width: w, height: h }).await;
                                if let Some(ref dc) = drive_control {
                                    let _ = worker
                                        .send_event(SessionEvent::DriveStatus {
                                            drive_name: dc.drive_name(),
                                            local_path: dc.local_path().to_string_lossy().to_string(),
                                            mounted: dc.is_mounted(),
                                            bytes_read: dc.bytes_read(),
                                            bytes_written: dc.bytes_written(),
                                            operations_count: dc.operations_count(),
                                        })
                                        .await;
                                }
                                let _ = worker.send_event(microphone_control.status_snapshot()).await;
                            }

                            if framebuffer.width() != w || framebuffer.height() != h {
                                let _ = framebuffer.resize(w, h);
                                let _ = worker.send_event(SessionEvent::RemoteResize { width: w, height: h }).await;
                            }

                            // Convert u32 [0, R, G, B] to RGBA bytes
                            let mut rgba_buf = vec![0u8; buffer.len() * 4];
                            for (i, &pixel) in buffer.iter().enumerate() {
                                let bytes = pixel.to_be_bytes();
                                let offset = i * 4;
                                rgba_buf[offset] = bytes[1];     // R
                                rgba_buf[offset + 1] = bytes[2]; // G
                                rgba_buf[offset + 2] = bytes[3]; // B
                                rgba_buf[offset + 3] = 255;      // A
                            }

                            if let Ok(()) = framebuffer.update_rect(Rect::new(0, 0, w, h), &rgba_buf) {
                                let dirty = framebuffer.take_dirty();
                                let decode_dur = decode_start.elapsed();
                                metrics.record_frame_decoded(decode_dur, Instant::now());
                                // Note: IronRDP does not expose raw encrypted wire byte counts here;
                                // do not count decoded RGBA framebuffer bytes as network traffic.

                                let _ = worker.send_event(SessionEvent::Damage {
                                    rects: dirty,
                                    sequence: framebuffer.sequence(),
                                }).await;
                                let _ = worker.send_event(SessionEvent::FrameReady {
                                    sequence: framebuffer.sequence(),
                                    snapshot: Some(framebuffer.snapshot()),
                                }).await;
                            }
                        }
                        RdpOutputEvent::ConnectionFailure(err) => {
                            let err_msg = format!("RDP connection failed: {err}");
                            tracing::warn!(error = %err, "IronRDP client reported connection failure");
                            sound_control.set_active(false);
                            microphone_control.set_recording(false);
                            let _ = input_sender.send(RdpInputEvent::Close);
                            failure = Some(RdpError::Connection(err_msg));
                            break;
                        }
                        RdpOutputEvent::Terminated(res) => {
                            let reason = match res {
                                Ok(r) => format!("Session terminated gracefully: {r:?}"),
                                Err(e) => format!("Session terminated with error: {e}"),
                            };
                            tracing::info!(reason = %reason, "IronRDP client reported termination");
                            sound_control.set_active(false);
                            microphone_control.set_recording(false);
                            let _ = input_sender.send(RdpInputEvent::Close);
                            failure = Some(RdpError::Terminated(reason));
                            break;
                        }
                        _ => {}
                    },
                    None => {
                        // RDP runner task closed
                        tracing::warn!("IronRDP output event channel closed (runner exited)");
                        if !connected && failure.is_none() {
                            failure = Some(RdpError::Connection(
                                "RDP server closed connection (authentication rejected or protocol negotiation failed)".into(),
                            ));
                        }
                        break;
                    }
                }
            }
        }
    }

    *proxy.lock().unwrap() = None;
    let _ = stop_tx.send(());
    match rdp_thread.join() {
        Ok(()) => {}
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "Non-string thread panic payload in RDP engine".to_string()
            };
            tracing::error!(panic = %msg, "RDP runner thread joined with panic");
            if failure.is_none() {
                failure = Some(RdpError::Connection(format!("RDP engine crashed: {msg}")));
            }
        }
    }

    match failure {
        Some(err) => (Err(err), connected),
        None => {
            if !connected {
                (
                    Err(RdpError::Connection(
                        "RDP session terminated before connection was established".into(),
                    )),
                    false,
                )
            } else {
                (Ok(()), true)
            }
        }
    }
}

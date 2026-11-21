//! High-level asynchronous RDP client session driver powered by IronRDP.

use crate::config::RdpClientConfig;
use crate::input::{
    build_ctrl_alt_del_sequence, translate_key_event, translate_pointer_event,
    translate_wheel_event,
};
use ironrdp_client::rdp::{RdpClient, RdpInputEvent, RdpOutputEvent};
use shellumi_remote_desktop::{
    Framebuffer, Rect, SessionCommand, SessionConfig, SessionEvent, SessionMetrics, SessionState,
    SessionWorkerHandle,
};
use smallvec::smallvec;
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

    loop {
        let (attempt_res, was_connected) =
            run_rdp_session_attempt(&config, &worker, &mut framebuffer).await;

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
) -> (Result<(), RdpError>, bool) {
    let _ = worker
        .send_event(SessionEvent::StateChanged(SessionState::Connecting))
        .await;

    tracing::info!(
        host = %config.host,
        port = config.port,
        tls_strict = config.tls_strict,
        "Initiating RDP connection attempt"
    );

    let rdp_config = RdpClientConfig::from_session_config(config);
    let proxy = std::sync::Arc::new(std::sync::Mutex::new(None));
    let temp_dir = std::env::temp_dir()
        .join("shellumi-transfers")
        .to_string_lossy()
        .to_string();
    let clipboard_backend = crate::clipboard::ShellumiCliprdrBackend::new(
        proxy.clone(),
        worker.event_sender(),
        temp_dir,
    );
    let sound_control = crate::sound::ShellumiRdpsndControl::new(false, 100);
    let display_control = crate::display::DisplayControlState::new();
    let microphone_control = crate::microphone::MicrophoneControlState::new(false, 100);
    let shared_folder_path = rdp_config
        .shared_folder
        .clone()
        .unwrap_or_else(crate::drive::default_shared_folder);
    let drive_name = crate::drive::derive_drive_name(&shared_folder_path);
    let drive_control = crate::drive::DriveControlState::new(
        crate::drive::DEFAULT_DRIVE_DEVICE_ID,
        drive_name,
        shared_folder_path,
    );
    let ironrdp_cfg = match rdp_config.to_ironrdp_config_with_backends(
        Some(clipboard_backend.clone()),
        Some(sound_control.clone()),
        Some(display_control.clone()),
        Some(drive_control.clone()),
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

            rt.block_on(async move {
                tokio::select! {
                    _ = client.run() => {}
                    _ = &mut stop_rx => {}
                }
            });
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

    loop {
        tokio::select! {
            // Outbound UI commands
            cmd_res = worker.recv_command() => {
                match cmd_res {
                    Ok(cmd) => match cmd {
                        SessionCommand::Pointer(p) => {
                            if !config.view_only {
                                let event = translate_pointer_event(&p);
                                let _ = input_sender.send(RdpInputEvent::FastPath(smallvec![event]));
                            }
                        }
                        SessionCommand::Wheel(w) => {
                            if !config.view_only {
                                let event = translate_wheel_event(&w);
                                let _ = input_sender.send(RdpInputEvent::FastPath(smallvec![event]));
                            }
                        }
                        SessionCommand::Key(k) => {
                            if !config.view_only {
                                let event = translate_key_event(&k);
                                let _ = input_sender.send(RdpInputEvent::FastPath(smallvec![event]));
                            }
                        }
                        SessionCommand::CtrlAltDel => {
                            if !config.view_only {
                                for chunk in build_ctrl_alt_del_sequence() {
                                    let _ = input_sender.send(RdpInputEvent::FastPath(chunk));
                                }
                            }
                        }
                        SessionCommand::SendClipboard(text) => {
                            if !config.view_only {
                                clipboard_backend.set_local_text(text);
                            }
                        }
                        SessionCommand::SendFiles(paths) => {
                            if !config.view_only {
                                if let Err(e) = clipboard_backend.stage_files(&paths) {
                                    tracing::warn!("Failed to stage files for RDP transfer: {e}");
                                }
                            }
                        }
                        SessionCommand::SetAudioMute(muted) => {
                            sound_control.set_muted(muted);
                            let _ = worker
                                .send_event(sound_control.status_snapshot("PCM", 44100, 2))
                                .await;
                        }
                        SessionCommand::SetAudioVolume(volume) => {
                            sound_control.set_volume(volume);
                            let _ = worker
                                .send_event(sound_control.status_snapshot("PCM", 44100, 2))
                                .await;
                        }
                        SessionCommand::SetMicrophoneMute(muted) => {
                            microphone_control.set_muted(muted);
                            let _ = worker.send_event(microphone_control.status_snapshot()).await;
                        }
                        SessionCommand::SetMicrophoneVolume(volume) => {
                            microphone_control.set_volume(volume);
                            let _ = worker.send_event(microphone_control.status_snapshot()).await;
                        }
                        SessionCommand::RequestResize { width, height } => {
                            let (adj_w, adj_h) = crate::display::adjust_display_size(width, height);
                            tracing::debug!(
                                width,
                                height,
                                adj_w,
                                adj_h,
                                "Dispatching MS-RDPEDISP dynamic resize event to RDP engine"
                            );
                            let _ = input_sender.send(RdpInputEvent::Resize {
                                width: adj_w as u16,
                                height: adj_h as u16,
                                scale_factor: 100,
                                physical_size: None,
                            });
                        }
                        SessionCommand::ConfigureDrive(opt_path) => {
                            let target_path = opt_path.unwrap_or_else(crate::drive::default_shared_folder);
                            let d_name = crate::drive::derive_drive_name(&target_path);
                            drive_control.update_folder(d_name.clone(), target_path.clone());
                            let _ = worker
                                .send_event(SessionEvent::DriveStatus {
                                    drive_name: d_name,
                                    local_path: target_path.to_string_lossy().to_string(),
                                    mounted: drive_control.is_mounted(),
                                    bytes_read: drive_control.bytes_read(),
                                    bytes_written: drive_control.bytes_written(),
                                    operations_count: drive_control.operations_count(),
                                })
                                .await;
                        }
                        SessionCommand::Disconnect => {
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
                                let _ = worker
                                    .send_event(SessionEvent::DriveStatus {
                                        drive_name: drive_control.drive_name(),
                                        local_path: drive_control.local_path().to_string_lossy().to_string(),
                                        mounted: drive_control.is_mounted(),
                                        bytes_read: drive_control.bytes_read(),
                                        bytes_written: drive_control.bytes_written(),
                                        operations_count: drive_control.operations_count(),
                                    })
                                    .await;
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
                                metrics.record_bytes_received((buffer.len() * 4) as u64);

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
    let _ = rdp_thread.join();

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

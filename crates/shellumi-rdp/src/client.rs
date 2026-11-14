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

/// Run an authentic RDP session using IronRDP client engine.
pub async fn run_rdp_session(
    config: SessionConfig,
    worker: SessionWorkerHandle,
    mut framebuffer: Framebuffer,
) -> Result<(), RdpError> {
    let _ = worker
        .send_event(SessionEvent::StateChanged(SessionState::Connecting))
        .await;

    let rdp_config = RdpClientConfig::from_session_config(&config);
    let ironrdp_cfg = rdp_config.to_ironrdp_config()?;

    let (out_tx, mut out_rx) = mpsc::channel(64);
    let client = RdpClient::new(ironrdp_cfg, out_tx);
    let input_sender = client.input_sender();

    // Spawn IronRDP background runner in a dedicated OS thread with current-thread runtime
    // to bypass higher-ranked lifetime Send limitations in ironrdp-client's internal futures.
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let rdp_thread = std::thread::Builder::new()
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
        })
        .map_err(|e| RdpError::Connection(format!("Failed to spawn RDP runner thread: {e}")))?;

    let mut metrics = SessionMetrics::new();
    let mut metrics_interval = tokio::time::interval(Duration::from_millis(1000));
    let mut connected = false;

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
                        SessionCommand::Disconnect => {
                            let _ = input_sender.send(RdpInputEvent::Close);
                            break;
                        }
                        _ => {}
                    },
                    Err(_) => {
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
                                let _ = worker.send_event(SessionEvent::StateChanged(SessionState::Connected)).await;
                                let _ = worker.send_event(SessionEvent::RemoteResize { width: w, height: h }).await;
                            }

                            if framebuffer.width() != w || framebuffer.height() != h {
                                let _ = framebuffer.resize(w, h);
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
                            let _ = worker.send_event(SessionEvent::Error(err_msg.clone())).await;
                            let _ = worker.send_event(SessionEvent::StateChanged(SessionState::Error(err_msg.clone()))).await;
                            return Err(RdpError::Connection(err_msg));
                        }
                        RdpOutputEvent::Terminated(res) => {
                            let reason = match res {
                                Ok(r) => format!("Session terminated gracefully: {r:?}"),
                                Err(e) => format!("Session terminated with error: {e}"),
                            };
                            let _ = worker.send_event(SessionEvent::StateChanged(SessionState::Disconnected)).await;
                            return Err(RdpError::Terminated(reason));
                        }
                        _ => {}
                    },
                    None => {
                        // RDP runner task closed
                        break;
                    }
                }
            }
        }
    }

    let _ = stop_tx.send(());
    let _ = rdp_thread.join();
    let _ = worker
        .send_event(SessionEvent::StateChanged(SessionState::Disconnected))
        .await;

    Ok(())
}

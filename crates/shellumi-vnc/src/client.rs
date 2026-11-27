//! High-level asynchronous RFB client event loop driving network I/O, frame decoding, and input events.

use crate::decoder::{decode_hextile_rectangle, TightDecoder, ZrleDecoder, MAX_CLIPBOARD_BYTES};
use crate::handshake::{client_and_server_init, negotiate_security, negotiate_version};
use crate::rectangles::{
    decode_copy_rect, decode_raw_rectangle, handle_desktop_size, ENCODING_COPYRECT,
    ENCODING_DESKTOP_SIZE, ENCODING_HEXTILE, ENCODING_RAW, ENCODING_TIGHT, ENCODING_ZRLE,
};
use crate::wire::{
    build_framebuffer_update_request, build_key_event, build_pointer_event, build_set_encodings,
    build_set_pixel_format, server_msg, PixelFormat, RectangleHeader,
};
use shellumi_remote_desktop::{
    Framebuffer, SessionCommand, SessionConfig, SessionEvent, SessionMetrics, SessionState,
    SessionWorkerHandle,
};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Client connection and protocol errors.
#[derive(Debug, Error)]
pub enum VncClientError {
    #[error("Handshake failed: {0}")]
    Handshake(#[from] crate::handshake::HandshakeError),

    #[error("Rectangle decoding failed: {0}")]
    Rectangle(#[from] crate::rectangles::RectangleError),

    #[error("Network connection error: {0}")]
    Network(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Connection timed out after {0:?}")]
    Timeout(Duration),
}

/// Asynchronous VNC client task runner with network resilience and automatic reconnection.
///
/// Drives the RFB session over TCP, blitting incoming rectangles into `framebuffer`
/// and transmitting outgoing input commands received from `worker`.
pub async fn run_vnc_session(
    config: SessionConfig,
    worker: SessionWorkerHandle,
    framebuffer: Framebuffer,
) -> Result<(), VncClientError> {
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
        let _ = worker
            .send_event(SessionEvent::StateChanged(SessionState::Connecting))
            .await;

        // 1. Establish TCP connection with 10-second timeout
        let endpoint = config.target_endpoint();
        tracing::info!(
            host = %config.host,
            port = config.port,
            attempt = attempt,
            "Connecting to VNC server"
        );
        let stream_result =
            tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(&endpoint)).await;

        let stream = match stream_result {
            Ok(Ok(stream)) => {
                tracing::debug!(host = %config.host, port = config.port, "VNC TCP connection established");
                stream
            }
            Ok(Err(err)) => {
                let err_msg = format!("Failed to connect to {endpoint}: {err}");
                tracing::error!(
                    host = %config.host,
                    port = config.port,
                    error = %err,
                    "Failed to connect to VNC endpoint"
                );
                if !policy.enabled || attempt >= policy.max_attempts {
                    let _ = worker
                        .send_event(SessionEvent::Error(err_msg.clone()))
                        .await;
                    let _ = worker
                        .send_event(SessionEvent::StateChanged(SessionState::Error(err_msg)))
                        .await;
                    return Err(VncClientError::Network(err));
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
                continue;
            }
            Err(_) => {
                let err_msg = format!("Connection to {endpoint} timed out after 10 seconds");
                if !policy.enabled || attempt >= policy.max_attempts {
                    let _ = worker
                        .send_event(SessionEvent::Error(err_msg.clone()))
                        .await;
                    let _ = worker
                        .send_event(SessionEvent::StateChanged(SessionState::Error(err_msg)))
                        .await;
                    return Err(VncClientError::Timeout(Duration::from_secs(10)));
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
                continue;
            }
        };

        // Run the connected session
        let mut fb_attempt = framebuffer.clone();
        let session_res = run_vnc_session_with_stream_inner(
            stream,
            &config,
            &worker,
            &mut fb_attempt,
            &mut view_only,
            transport_epoch,
        )
        .await;

        match session_res {
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
                    "VNC session error"
                );

                // Authentication, Handshake, and Connection rejection errors must not be retried blindly!
                let is_auth_or_fatal = matches!(
                    &err,
                    VncClientError::Handshake(
                        crate::handshake::HandshakeError::AuthFailed(_)
                            | crate::handshake::HandshakeError::ConnectionRejected(_)
                            | crate::handshake::HandshakeError::NoSecurityTypes
                            | crate::handshake::HandshakeError::UnsupportedVersion(_)
                            | crate::handshake::HandshakeError::InvalidVersionString(_)
                    )
                );

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

/// Run an RFB VNC session over an arbitrary asynchronous stream (e.g. TCP, TLS, or in-memory duplex).
pub async fn run_vnc_session_with_stream<S>(
    stream: S,
    config: SessionConfig,
    worker: SessionWorkerHandle,
    mut framebuffer: Framebuffer,
) -> Result<(), VncClientError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let mut view_only = config.view_only;
    run_vnc_session_with_stream_inner(
        stream,
        &config,
        &worker,
        &mut framebuffer,
        &mut view_only,
        1,
    )
    .await
}

async fn run_vnc_session_with_stream_inner<S>(
    mut stream: S,
    config: &SessionConfig,
    worker: &SessionWorkerHandle,
    framebuffer: &mut Framebuffer,
    view_only: &mut bool,
    transport_epoch: u64,
) -> Result<(), VncClientError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // 2. Negotiate protocol version
    let version = match negotiate_version(&mut stream).await {
        Ok(v) => {
            tracing::debug!(?v, "RFB protocol version negotiated");
            v
        }
        Err(err) => {
            let err_msg = err.to_string();
            tracing::error!(error = %err, "RFB version negotiation failed");
            let _ = worker
                .send_event(SessionEvent::Error(err_msg.clone()))
                .await;
            let _ = worker
                .send_event(SessionEvent::StateChanged(SessionState::Error(err_msg)))
                .await;
            return Err(VncClientError::Handshake(err));
        }
    };

    // 3. Negotiate security and authenticate
    if let Err(err) = negotiate_security(
        &mut stream,
        version,
        config.username.as_deref(),
        config.password.as_deref(),
    )
    .await
    {
        let err_msg = err.to_string();
        tracing::error!(error = %err, "VNC security authentication failed");
        let _ = worker
            .send_event(SessionEvent::Error(err_msg.clone()))
            .await;
        let _ = worker
            .send_event(SessionEvent::StateChanged(SessionState::Error(err_msg)))
            .await;
        return Err(VncClientError::Handshake(err));
    }
    tracing::debug!("VNC security handshake completed successfully");

    // 4. ClientInit and ServerInit
    let server_init = match client_and_server_init(&mut stream, true).await {
        Ok(init) => {
            tracing::info!(
                width = init.width,
                height = init.height,
                name = %init.name,
                "VNC server initialized"
            );
            init
        }
        Err(err) => {
            let err_msg = err.to_string();
            tracing::error!(error = %err, "VNC ServerInit failed");
            let _ = worker
                .send_event(SessionEvent::Error(err_msg.clone()))
                .await;
            let _ = worker
                .send_event(SessionEvent::StateChanged(SessionState::Error(err_msg)))
                .await;
            return Err(VncClientError::Handshake(err));
        }
    };

    // 5. Notify UI of remote screen geometry and resize local framebuffer
    let width = server_init.width as u32;
    let height = server_init.height as u32;
    let _ = worker
        .send_event(SessionEvent::RemoteResize { width, height })
        .await;

    if framebuffer.width() != width || framebuffer.height() != height {
        let _ = framebuffer.resize(width, height);
    }

    // 6. Set preferred pixel format (32bpp little-endian RGBA)
    let client_pf = PixelFormat::rgba_32();
    let set_pf_packet = build_set_pixel_format(&client_pf);
    stream.write_all(&set_pf_packet).await?;

    // 7. Request supported encodings (CopyRect, Hextile, ZRLE, Tight, Raw, DesktopSize, ExtendedClipboard)
    let supported_encodings = [
        ENCODING_COPYRECT,
        ENCODING_HEXTILE,
        ENCODING_ZRLE,
        ENCODING_TIGHT,
        ENCODING_RAW,
        ENCODING_DESKTOP_SIZE,
        crate::wire::ENCODING_EXTENDED_CLIPBOARD,
    ];
    let set_enc_packet = build_set_encodings(&supported_encodings);
    stream.write_all(&set_enc_packet).await?;

    // 8. Request initial full framebuffer update
    let initial_fbur = build_framebuffer_update_request(false, 0, 0, width as u16, height as u16);
    stream.write_all(&initial_fbur).await?;
    stream.flush().await?;

    // 9. Transition to Connected state
    tracing::info!(width = width, height = height, "VNC session connected");
    let _ = worker
        .send_event(SessionEvent::StateChanged(SessionState::Connected))
        .await;
    let _ = worker
        .send_event(SessionEvent::FrameReady {
            sequence: framebuffer.sequence(),
            snapshot: Some(framebuffer.snapshot()),
        })
        .await;

    let mut metrics = SessionMetrics::new();
    let mut metrics_interval = tokio::time::interval(Duration::from_millis(1000));
    let mut zrle_decoder = ZrleDecoder::new();
    let mut tight_decoder = TightDecoder::new();
    let mut last_sent_clipboard: Option<String> = None;
    let mut last_received_clipboard: Option<String> = None;
    let mut clipboard_policy = shellumi_remote_desktop::ClipboardPolicy::default();
    let mut ext_clipboard_utf8 = false;
    let mut peer_ext_max_bytes: usize = MAX_CLIPBOARD_BYTES;
    let mut local_clipboard_offer: Option<String> = None;
    let mut ledger = shellumi_remote_desktop::SubmittedInputLedger::new();
    let mut worker_min_input_epoch: u64 = 0;
    let mut held_buttons: u8 = 0;
    let mut last_pointer: (u16, u16) = (0, 0);
    let mut scroll_acc_x: i32 = 0;
    let mut scroll_acc_y: i32 = 0;

    // 10. Main RFB event loop
    loop {
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
                            let mut release_bytes = Vec::new();
                            for rel in &releases {
                                release_bytes.extend_from_slice(&translate_vnc_remote_input(
                                    rel,
                                    &mut held_buttons,
                                    &mut last_pointer,
                                    &mut scroll_acc_x,
                                    &mut scroll_acc_y,
                                    framebuffer.width(),
                                    framebuffer.height(),
                                ));
                            }
                            if !release_bytes.is_empty() {
                                if let Err(err) = stream.write_all(&release_bytes).await {
                                    let reason = format!("VNC recovery write failed: {err}");
                                    let _ = worker.send_event(SessionEvent::RecoveryFailed {
                                        identity: barrier.identity,
                                        input_epoch: barrier.input_epoch,
                                        barrier_id: barrier.barrier_id,
                                        reason: reason.clone(),
                                    }).await;
                                    let _ = worker.send_event(SessionEvent::Error(reason)).await;
                                    return Err(VncClientError::Network(err));
                                }
                                metrics.record_bytes_sent(release_bytes.len() as u64);
                            }
                            let _ = worker.send_event(SessionEvent::RecoveryCompleted {
                                identity: barrier.identity,
                                input_epoch: barrier.input_epoch,
                                barrier_id: barrier.barrier_id,
                                released_count,
                                reason: barrier.reason,
                            }).await;
                        }
                        SessionCommand::SetViewOnly { enabled, transition_id } => {
                            if enabled && !*view_only {
                                last_sent_clipboard = None;
                                last_received_clipboard = None;
                                let releases = ledger.drain_releases();
                                let released_count = releases.len();
                                let mut release_bytes = Vec::new();
                                for rel in &releases {
                                    release_bytes.extend_from_slice(&translate_vnc_remote_input(
                                        rel,
                                        &mut held_buttons,
                                        &mut last_pointer,
                                        &mut scroll_acc_x,
                                        &mut scroll_acc_y,
                                        framebuffer.width(),
                                        framebuffer.height(),
                                    ));
                                }
                                if !release_bytes.is_empty() {
                                    if let Err(err) = stream.write_all(&release_bytes).await {
                                        let err_msg = format!("VNC write failed: {err}");
                                        let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                        return Err(VncClientError::Network(err));
                                    }
                                    metrics.record_bytes_sent(release_bytes.len() as u64);
                                }
                                if released_count > 0 {
                                    let _ = worker.send_event(SessionEvent::InputRecovery {
                                        transport_epoch,
                                        released_count,
                                        reason: "View-only enabled".into(),
                                    }).await;
                                }
                            }
                            *view_only = enabled;
                            let _ = worker.send_event(SessionEvent::ViewOnlyChanged { enabled, transition_id }).await;
                        }
                        SessionCommand::InputEnvelope(env) => {
                            if env.transport_epoch == 0
                                || env.transport_epoch != transport_epoch
                                || env.input_epoch < worker_min_input_epoch
                            {
                                continue;
                            }
                            if matches!(env.input, shellumi_remote_desktop::RemoteInput::ReleaseAll) {
                                let releases = ledger.drain_releases();
                                let released_count = releases.len();
                                let mut bytes = Vec::new();
                                for rel in &releases {
                                    bytes.extend_from_slice(&translate_vnc_remote_input(
                                        rel,
                                        &mut held_buttons,
                                        &mut last_pointer,
                                        &mut scroll_acc_x,
                                        &mut scroll_acc_y,
                                        framebuffer.width(),
                                        framebuffer.height(),
                                    ));
                                }
                                if !bytes.is_empty() {
                                    if let Err(err) = stream.write_all(&bytes).await {
                                        let _ = worker.send_event(SessionEvent::InputError {
                                            transport_epoch,
                                            seq: env.seq,
                                            reason: err.to_string(),
                                        }).await;
                                        return Err(VncClientError::Network(err));
                                    }
                                    metrics.record_bytes_sent(bytes.len() as u64);
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
                                let bytes = translate_vnc_remote_input(
                                    &env.input,
                                    &mut held_buttons,
                                    &mut last_pointer,
                                    &mut scroll_acc_x,
                                    &mut scroll_acc_y,
                                    framebuffer.width(),
                                    framebuffer.height(),
                                );
                                if !bytes.is_empty() {
                                    if let Err(err) = stream.write_all(&bytes).await {
                                        let _ = worker.send_event(SessionEvent::InputError {
                                            transport_epoch,
                                            seq: env.seq,
                                            reason: err.to_string(),
                                        }).await;
                                        return Err(VncClientError::Network(err));
                                    }
                                    ledger.record_submitted(&env.input);
                                    metrics.record_bytes_sent(bytes.len() as u64);
                                }
                            }
                        }
                        SessionCommand::Input(input) => {
                            if matches!(input, shellumi_remote_desktop::RemoteInput::ReleaseAll) {
                                let releases = ledger.drain_releases();
                                let mut bytes = Vec::new();
                                for rel in &releases {
                                    bytes.extend_from_slice(&translate_vnc_remote_input(
                                        rel,
                                        &mut held_buttons,
                                        &mut last_pointer,
                                        &mut scroll_acc_x,
                                        &mut scroll_acc_y,
                                        framebuffer.width(),
                                        framebuffer.height(),
                                    ));
                                }
                                if !bytes.is_empty() {
                                    if let Err(err) = stream.write_all(&bytes).await {
                                        let err_msg = format!("VNC write failed: {err}");
                                        let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                        return Err(VncClientError::Network(err));
                                    }
                                    metrics.record_bytes_sent(bytes.len() as u64);
                                }
                                continue;
                            }
                            let is_release = matches!(
                                input,
                                shellumi_remote_desktop::RemoteInput::PointerButton { down: false, .. }
                                    | shellumi_remote_desktop::RemoteInput::PhysicalKey { down: false, .. }
                            );
                            if !*view_only || is_release {
                                let bytes = translate_vnc_remote_input(
                                    &input,
                                    &mut held_buttons,
                                    &mut last_pointer,
                                    &mut scroll_acc_x,
                                    &mut scroll_acc_y,
                                    framebuffer.width(),
                                    framebuffer.height(),
                                );
                                if !bytes.is_empty() {
                                    if let Err(err) = stream.write_all(&bytes).await {
                                        let err_msg = format!("VNC write failed: {err}");
                                        let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                        return Err(VncClientError::Network(err));
                                    }
                                    ledger.record_submitted(&input);
                                    metrics.record_bytes_sent(bytes.len() as u64);
                                }
                            }
                        }
                        SessionCommand::Shortcut(shortcut) => {
                            if !*view_only {
                                let bytes = build_vnc_shortcut_packets(shortcut);
                                if !bytes.is_empty() {
                                    if let Err(err) = stream.write_all(&bytes).await {
                                        let err_msg = format!("VNC write failed: {err}");
                                        let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                        return Err(VncClientError::Network(err));
                                    }
                                    metrics.record_bytes_sent(bytes.len() as u64);
                                }
                            }
                        }
                        SessionCommand::Pointer(p) => {
                            if !*view_only || !p.pressed {
                                let x = (p.x.min(framebuffer.width().saturating_sub(1))).min(u16::MAX as u32) as u16;
                                let y = (p.y.min(framebuffer.height().saturating_sub(1))).min(u16::MAX as u32) as u16;
                                last_pointer = (x, y);
                                if p.button_mask != 0 {
                                    if p.pressed {
                                        held_buttons |= p.button_mask;
                                    } else {
                                        held_buttons &= !p.button_mask;
                                    }
                                }
                                let packet = build_pointer_event(held_buttons, x, y);
                                if let Err(err) = stream.write_all(&packet).await {
                                    let err_msg = format!("VNC write failed: {err}");
                                    let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                    return Err(VncClientError::Network(err));
                                }
                                metrics.record_bytes_sent(packet.len() as u64);
                            }
                        }
                        SessionCommand::Wheel(w) => {
                            if !*view_only {
                                let input = shellumi_remote_desktop::RemoteInput::Scroll {
                                    x: w.x,
                                    y: w.y,
                                    horizontal: w.delta_x,
                                    vertical: w.delta_y,
                                };
                                let bytes = translate_vnc_remote_input(
                                    &input,
                                    &mut held_buttons,
                                    &mut last_pointer,
                                    &mut scroll_acc_x,
                                    &mut scroll_acc_y,
                                    framebuffer.width(),
                                    framebuffer.height(),
                                );
                                if !bytes.is_empty() {
                                    if let Err(err) = stream.write_all(&bytes).await {
                                        let err_msg = format!("VNC write failed: {err}");
                                        let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                        return Err(VncClientError::Network(err));
                                    }
                                    metrics.record_bytes_sent(bytes.len() as u64);
                                }
                            }
                        }
                        SessionCommand::Key(k) => {
                            if !*view_only || !k.pressed {
                                let packet = build_key_event(k.pressed, k.keysym);
                                if let Err(err) = stream.write_all(&packet).await {
                                    let err_msg = format!("VNC write failed: {err}");
                                    let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                    return Err(VncClientError::Network(err));
                                }
                                metrics.record_bytes_sent(packet.len() as u64);
                            }
                        }
                        SessionCommand::CtrlAltDel => {
                            if !*view_only {
                                let bytes = build_vnc_shortcut_packets(shellumi_remote_desktop::RemoteShortcut::CtrlAltDel);
                                if let Err(err) = stream.write_all(&bytes).await {
                                    let err_msg = format!("VNC write failed: {err}");
                                    let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                    return Err(VncClientError::Network(err));
                                }
                                metrics.record_bytes_sent(bytes.len() as u64);
                            }
                        }
                        SessionCommand::SetClipboardPolicy(policy) => {
                            clipboard_policy = policy;
                        }
                        SessionCommand::SendClipboard(text) => {
                            let send_allowed = !*view_only
                                && clipboard_policy.mode != shellumi_remote_desktop::ClipboardSyncMode::Disabled
                                && clipboard_policy.allow_send;
                            if !send_allowed {
                                continue;
                            }
                            if last_received_clipboard.as_deref() == Some(&text) {
                                last_received_clipboard = None;
                                continue;
                            }
                            last_sent_clipboard = Some(text.clone());
                            local_clipboard_offer = Some(text.clone());
                            if ext_clipboard_utf8 && text.len() > peer_ext_max_bytes {
                                let _ = worker
                                    .send_event(SessionEvent::Error(format!(
                                        "Clipboard text ({} bytes) exceeds peer extended clipboard limit ({peer_ext_max_bytes} bytes)",
                                        text.len()
                                    )))
                                    .await;
                                continue;
                            }
                            let packet = if ext_clipboard_utf8 {
                                match crate::wire::build_extended_clipboard_provide_utf8(&text) {
                                    Ok(p) => p,
                                    Err(err) => {
                                        let _ = worker.send_event(SessionEvent::Error(err)).await;
                                        continue;
                                    }
                                }
                            } else {
                                match crate::wire::encode_latin1_cut_text(&text) {
                                    Ok(p) => p,
                                    Err(err) => {
                                        let _ = worker.send_event(SessionEvent::Error(err)).await;
                                        continue;
                                    }
                                }
                            };
                            if let Err(err) = stream.write_all(&packet).await {
                                let err_msg = format!("VNC write failed: {err}");
                                let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                return Err(VncClientError::Network(err));
                            }
                            metrics.record_bytes_sent(packet.len() as u64);
                        }
                        SessionCommand::Refresh => {
                            let fbur = build_framebuffer_update_request(false, 0, 0, framebuffer.width() as u16, framebuffer.height() as u16);
                            if let Err(err) = stream.write_all(&fbur).await {
                                let err_msg = format!("VNC write failed: {err}");
                                let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                                return Err(VncClientError::Network(err));
                            }
                            metrics.record_bytes_sent(fbur.len() as u64);
                        }
                        SessionCommand::Disconnect => {
                            let releases = ledger.drain_releases();
                            let mut bytes = Vec::new();
                            for rel in &releases {
                                bytes.extend_from_slice(&translate_vnc_remote_input(
                                    rel,
                                    &mut held_buttons,
                                    &mut last_pointer,
                                    &mut scroll_acc_x,
                                    &mut scroll_acc_y,
                                    framebuffer.width(),
                                    framebuffer.height(),
                                ));
                            }
                            if !bytes.is_empty() {
                                let _ = stream.write_all(&bytes).await;
                            }
                            break;
                        }
                        _ => {}
                    },
                    Err(_) => {
                        // UI handle closed
                        break;
                    }
                }
            }

            // Periodic metrics telemetry reporting
            _ = metrics_interval.tick() => {
                let snap = metrics.snapshot(Instant::now());
                let _ = worker.send_event(SessionEvent::Metrics(snap)).await;
            }

            // Inbound RFB socket messages
            msg_type_res = stream.read_u8() => {
                let msg_type = match msg_type_res {
                    Ok(t) => t,
                    Err(err) => {
                        let err_msg = format!("Remote host closed connection: {err}");
                        let _ = worker.send_event(SessionEvent::Error(err_msg)).await;
                        return Err(VncClientError::Network(err));
                    }
                };
                metrics.record_bytes_received(1);

                match msg_type {
                    server_msg::FRAMEBUFFER_UPDATE => {
                        let decode_start = Instant::now();
                        let _pad = stream.read_u8().await?;
                        let num_rects = stream.read_u16().await?;
                        metrics.record_bytes_received(3);

                        for _ in 0..num_rects {
                            let mut header_buf = [0u8; 12];
                            stream.read_exact(&mut header_buf).await?;
                            metrics.record_bytes_received(12);

                            let header = RectangleHeader::from_bytes(&header_buf);

                            match header.encoding_type {
                                ENCODING_RAW => {
                                    decode_raw_rectangle(&mut stream, &header, &client_pf, framebuffer).await?;
                                }
                                ENCODING_COPYRECT => {
                                    decode_copy_rect(&mut stream, &header, framebuffer).await?;
                                    metrics.record_bytes_received(4);
                                }
                                ENCODING_HEXTILE => {
                                    decode_hextile_rectangle(&mut stream, &header, &client_pf, framebuffer).await?;
                                }
                                ENCODING_ZRLE => {
                                    zrle_decoder.decode_rectangle(&mut stream, &header, &client_pf, framebuffer).await?;
                                }
                                ENCODING_TIGHT => {
                                    tight_decoder.decode_rectangle(&mut stream, &header, &client_pf, framebuffer).await?;
                                }
                                ENCODING_DESKTOP_SIZE => {
                                    handle_desktop_size(&header, framebuffer)?;
                                    let _ = worker.send_event(SessionEvent::RemoteResize {
                                        width: header.width as u32,
                                        height: header.height as u32,
                                    }).await;
                                }
                                unsupported => {
                                    return Err(VncClientError::Rectangle(
                                        crate::rectangles::RectangleError::UnsupportedEncoding(unsupported),
                                    ));
                                }
                            }
                        }

                        let dirty = framebuffer.take_dirty();
                        let decode_dur = decode_start.elapsed();
                        metrics.record_frame_decoded(decode_dur, Instant::now());

                        let _ = worker.send_event(SessionEvent::Damage {
                            rects: dirty,
                            sequence: framebuffer.sequence(),
                        }).await;
                        let _ = worker.send_event(SessionEvent::FrameReady {
                            sequence: framebuffer.sequence(),
                            snapshot: Some(framebuffer.snapshot()),
                        }).await;

                        // Request next incremental update
                        let inc_fbur = build_framebuffer_update_request(
                            true,
                            0,
                            0,
                            framebuffer.width() as u16,
                            framebuffer.height() as u16,
                        );
                        stream.write_all(&inc_fbur).await?;
                        metrics.record_bytes_sent(inc_fbur.len() as u64);
                    }
                    server_msg::BELL => {
                        let _ = worker.send_event(SessionEvent::Bell).await;
                    }
                    server_msg::SERVER_CUT_TEXT => {
                        let mut pad = [0u8; 3];
                        stream.read_exact(&mut pad).await?;
                        let raw_len = stream.read_i32().await?;
                        metrics.record_bytes_received(7);

                        let receive_allowed = !*view_only
                            && clipboard_policy.mode != shellumi_remote_desktop::ClipboardSyncMode::Disabled
                            && clipboard_policy.allow_receive;

                        if raw_len < 0 {
                            let ext_len = raw_len.unsigned_abs() as usize;
                            if !(4..=MAX_CLIPBOARD_BYTES).contains(&ext_len) {
                                return Err(VncClientError::Protocol(format!(
                                    "Extended ServerCutText payload ({ext_len} bytes) out of bounds"
                                )));
                            }
                            let mut ext_buf = vec![0u8; ext_len];
                            stream.read_exact(&mut ext_buf).await?;
                            metrics.record_bytes_received(ext_len as u64);

                            let flags = u32::from_be_bytes([ext_buf[0], ext_buf[1], ext_buf[2], ext_buf[3]]);
                            let action = flags & 0xFF00_0000;
                            let send_allowed = !*view_only
                                && clipboard_policy.mode != shellumi_remote_desktop::ClipboardSyncMode::Disabled
                                && clipboard_policy.allow_send;
                            if action == crate::wire::ext_clipboard::ACTION_CAPS {
                                if ext_buf.len() >= 8 {
                                    let advertised_max = u32::from_be_bytes([ext_buf[4], ext_buf[5], ext_buf[6], ext_buf[7]]) as usize;
                                    if advertised_max > 0 {
                                        peer_ext_max_bytes = advertised_max.min(MAX_CLIPBOARD_BYTES);
                                    }
                                }
                                if (flags & crate::wire::ext_clipboard::FORMAT_UTF8) != 0 {
                                    ext_clipboard_utf8 = true;
                                    let caps_pkt = crate::wire::build_extended_clipboard_caps(MAX_CLIPBOARD_BYTES as u32);
                                    stream.write_all(&caps_pkt).await?;
                                    metrics.record_bytes_sent(caps_pkt.len() as u64);
                                }
                            } else if action == crate::wire::ext_clipboard::ACTION_PEEK {
                                let notify_flags = if send_allowed && local_clipboard_offer.is_some() {
                                    crate::wire::ext_clipboard::ACTION_NOTIFY
                                        | crate::wire::ext_clipboard::FORMAT_UTF8
                                } else {
                                    crate::wire::ext_clipboard::ACTION_NOTIFY
                                };
                                let pkt = crate::wire::build_extended_clipboard_request(notify_flags);
                                stream.write_all(&pkt).await?;
                                metrics.record_bytes_sent(pkt.len() as u64);
                            } else if action == crate::wire::ext_clipboard::ACTION_REQUEST {
                                if send_allowed && (flags & crate::wire::ext_clipboard::FORMAT_UTF8) != 0 {
                                    if let Some(ref offer) = local_clipboard_offer {
                                        if offer.len() <= peer_ext_max_bytes {
                                            if let Ok(provide_pkt) = crate::wire::build_extended_clipboard_provide_utf8(offer) {
                                                stream.write_all(&provide_pkt).await?;
                                                metrics.record_bytes_sent(provide_pkt.len() as u64);
                                            }
                                        }
                                    }
                                }
                            } else if action == crate::wire::ext_clipboard::ACTION_NOTIFY {
                                if receive_allowed && (flags & crate::wire::ext_clipboard::FORMAT_UTF8) != 0 {
                                    let req_pkt = crate::wire::build_extended_clipboard_request(crate::wire::ext_clipboard::FORMAT_UTF8);
                                    stream.write_all(&req_pkt).await?;
                                    metrics.record_bytes_sent(req_pkt.len() as u64);
                                }
                            } else if action == crate::wire::ext_clipboard::ACTION_PROVIDE
                                && receive_allowed
                                && (flags & crate::wire::ext_clipboard::FORMAT_UTF8) != 0
                            {
                                match crate::wire::decode_extended_clipboard_provide_utf8(&ext_buf[4..], MAX_CLIPBOARD_BYTES) {
                                    Ok(text) => {
                                        if last_sent_clipboard.as_deref() == Some(&text) {
                                            last_sent_clipboard = None;
                                        } else {
                                            last_received_clipboard = Some(text.clone());
                                            let _ = worker.send_event(SessionEvent::ServerClipboard(text)).await;
                                        }
                                    }
                                    Err(err) => {
                                        return Err(VncClientError::Protocol(err));
                                    }
                                }
                            }
                        } else {
                            let len = raw_len as usize;
                            if len > MAX_CLIPBOARD_BYTES {
                                return Err(VncClientError::Protocol(format!(
                                    "ServerCutText payload ({len} bytes) exceeds maximum limit ({MAX_CLIPBOARD_BYTES} bytes)"
                                )));
                            }
                            let mut text_buf = vec![0u8; len];
                            stream.read_exact(&mut text_buf).await?;
                            metrics.record_bytes_received(len as u64);

                            if receive_allowed {
                                let text = crate::wire::decode_server_cut_text_latin1(&text_buf);
                                if last_sent_clipboard.as_deref() == Some(&text) {
                                    last_sent_clipboard = None;
                                } else {
                                    last_received_clipboard = Some(text.clone());
                                    let _ = worker.send_event(SessionEvent::ServerClipboard(text)).await;
                                }
                            }
                        }
                    }
                    other => {
                        return Err(VncClientError::Protocol(format!(
                            "Unsupported RFB server message type: {other}"
                        )));
                    }
                }
            }
        }
    }

    let _ = worker
        .send_event(SessionEvent::StateChanged(SessionState::Disconnected))
        .await;
    Ok(())
}

/// Translate a normalized `RemoteInput` into RFB wire packets, maintaining held-button
/// bitmask and 120-unit wheel accumulators.
pub fn translate_vnc_remote_input(
    input: &shellumi_remote_desktop::RemoteInput,
    held_buttons: &mut u8,
    last_pointer: &mut (u16, u16),
    scroll_acc_x: &mut i32,
    scroll_acc_y: &mut i32,
    fb_width: u32,
    fb_height: u32,
) -> Vec<u8> {
    use shellumi_remote_desktop::RemoteInput;
    let max_x = fb_width.saturating_sub(1).min(u16::MAX as u32) as u16;
    let max_y = fb_height.saturating_sub(1).min(u16::MAX as u32) as u16;
    let mut out = Vec::new();

    match input {
        RemoteInput::PointerMove { x, y } => {
            let cx = (*x).min(max_x as u32) as u16;
            let cy = (*y).min(max_y as u32) as u16;
            *last_pointer = (cx, cy);
            out.extend_from_slice(&build_pointer_event(*held_buttons, cx, cy));
        }
        RemoteInput::PointerButton { button, down, x, y } => {
            let mask = button.bit_mask();
            if mask == 0 {
                return out;
            }
            if *down {
                *held_buttons |= mask;
            } else {
                *held_buttons &= !mask;
            }
            let cx = (*x).min(max_x as u32) as u16;
            let cy = (*y).min(max_y as u32) as u16;
            *last_pointer = (cx, cy);
            out.extend_from_slice(&build_pointer_event(*held_buttons, cx, cy));
        }
        RemoteInput::Scroll {
            x,
            y,
            horizontal,
            vertical,
        } => {
            let cx = (*x).min(max_x as u32) as u16;
            let cy = (*y).min(max_y as u32) as u16;
            *last_pointer = (cx, cy);

            *scroll_acc_y += *vertical;
            *scroll_acc_x += *horizontal;

            // RFB wheel pulses: positive vertical = wheel up (bit 3 = 1<<3), negative = down (bit 4 = 1<<4).
            let v_ticks = (*scroll_acc_y / 120).clamp(-16, 16);
            *scroll_acc_y -= v_ticks * 120;
            let v_bit = if v_ticks > 0 { 1u8 << 3 } else { 1u8 << 4 };
            for _ in 0..v_ticks.abs() {
                out.extend_from_slice(&build_pointer_event(*held_buttons | v_bit, cx, cy));
                out.extend_from_slice(&build_pointer_event(*held_buttons, cx, cy));
            }

            // Horizontal wheel: positive = right (bit 6 = 1<<6), negative = left (bit 5 = 1<<5).
            let h_ticks = (*scroll_acc_x / 120).clamp(-16, 16);
            *scroll_acc_x -= h_ticks * 120;
            let h_bit = if h_ticks > 0 { 1u8 << 6 } else { 1u8 << 5 };
            for _ in 0..h_ticks.abs() {
                out.extend_from_slice(&build_pointer_event(*held_buttons | h_bit, cx, cy));
                out.extend_from_slice(&build_pointer_event(*held_buttons, cx, cy));
            }
        }
        RemoteInput::PhysicalKey { key, down, .. } => {
            if key.keysym != 0 {
                out.extend_from_slice(&build_key_event(*down, key.keysym));
            }
        }
        RemoteInput::CommittedText(text) => {
            for ch in text.chars() {
                let cp = ch as u32;
                let keysym = if (0x0020..=0x007E).contains(&cp) || (0x00A0..=0x00FF).contains(&cp) {
                    cp
                } else {
                    0x0100_0000 | cp
                };
                out.extend_from_slice(&build_key_event(true, keysym));
                out.extend_from_slice(&build_key_event(false, keysym));
            }
        }
        RemoteInput::Shortcut(shortcut) => {
            out.extend_from_slice(&build_vnc_shortcut_packets(*shortcut));
        }
        RemoteInput::ReleaseAll => {
            if *held_buttons != 0 {
                *held_buttons = 0;
                out.extend_from_slice(&build_pointer_event(0, last_pointer.0, last_pointer.1));
            }
        }
    }
    out
}

/// Build an ordered RFB `KeyEvent` sequence for a `RemoteShortcut`.
pub fn build_vnc_shortcut_packets(shortcut: shellumi_remote_desktop::RemoteShortcut) -> Vec<u8> {
    use shellumi_remote_desktop::RemoteShortcut;
    const CTRL_L: u32 = 0xFFE3;
    const ALT_L: u32 = 0xFFE9;
    const SUPER_L: u32 = 0xFFEB;
    const DELETE: u32 = 0xFFFF;
    const TAB: u32 = 0xFF09;
    const ESCAPE: u32 = 0xFF1B;

    let mut out = Vec::new();
    match shortcut {
        RemoteShortcut::CtrlAltDel => {
            for (down, sym) in [
                (true, CTRL_L),
                (true, ALT_L),
                (true, DELETE),
                (false, DELETE),
                (false, ALT_L),
                (false, CTRL_L),
            ] {
                out.extend_from_slice(&build_key_event(down, sym));
            }
        }
        RemoteShortcut::AltTab => {
            for (down, sym) in [(true, ALT_L), (true, TAB), (false, TAB), (false, ALT_L)] {
                out.extend_from_slice(&build_key_event(down, sym));
            }
        }
        RemoteShortcut::WinKey => {
            out.extend_from_slice(&build_key_event(true, SUPER_L));
            out.extend_from_slice(&build_key_event(false, SUPER_L));
        }
        RemoteShortcut::CtrlEscape => {
            for (down, sym) in [
                (true, CTRL_L),
                (true, ESCAPE),
                (false, ESCAPE),
                (false, CTRL_L),
            ] {
                out.extend_from_slice(&build_key_event(down, sym));
            }
        }
        RemoteShortcut::CtrlC => emit_vnc_ctrl_chord(&mut out, 0x0063),
        RemoteShortcut::CtrlV => emit_vnc_ctrl_chord(&mut out, 0x0076),
        RemoteShortcut::CtrlX => emit_vnc_ctrl_chord(&mut out, 0x0078),
        RemoteShortcut::CtrlA => emit_vnc_ctrl_chord(&mut out, 0x0061),
        RemoteShortcut::CtrlZ => emit_vnc_ctrl_chord(&mut out, 0x007A),
        RemoteShortcut::CtrlShiftZ => {
            const CTRL_L: u32 = 0xFFE3;
            const SHIFT_L: u32 = 0xFFE1;
            out.extend_from_slice(&build_key_event(true, CTRL_L));
            out.extend_from_slice(&build_key_event(true, SHIFT_L));
            out.extend_from_slice(&build_key_event(true, 0x007A));
            out.extend_from_slice(&build_key_event(false, 0x007A));
            out.extend_from_slice(&build_key_event(false, SHIFT_L));
            out.extend_from_slice(&build_key_event(false, CTRL_L));
        }
    }
    out
}

fn emit_vnc_ctrl_chord(out: &mut Vec<u8>, keysym: u32) {
    const CTRL_L: u32 = 0xFFE3;
    out.extend_from_slice(&build_key_event(true, CTRL_L));
    out.extend_from_slice(&build_key_event(true, keysym));
    out.extend_from_slice(&build_key_event(false, keysym));
    out.extend_from_slice(&build_key_event(false, CTRL_L));
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use shellumi_remote_desktop::{MouseButton, RemoteInput, RemoteShortcut};

    #[test]
    fn vnc_pointer_maintains_held_buttons_and_wheel_pulses() {
        let mut held = 0u8;
        let mut last = (0, 0);
        let mut sx = 0;
        let mut sy = 0;

        // Left down
        let p1 = translate_vnc_remote_input(
            &RemoteInput::PointerButton {
                button: MouseButton::Left,
                down: true,
                x: 100,
                y: 200,
            },
            &mut held,
            &mut last,
            &mut sx,
            &mut sy,
            1920,
            1080,
        );
        assert_eq!(p1, build_pointer_event(1, 100, 200));

        // Move while left held -> mask stays 1
        let p2 = translate_vnc_remote_input(
            &RemoteInput::PointerMove { x: 150, y: 250 },
            &mut held,
            &mut last,
            &mut sx,
            &mut sy,
            1920,
            1080,
        );
        assert_eq!(p2, build_pointer_event(1, 150, 250));

        // Scroll up while left held -> pulses bit 3 (1 | 8 = 9), then restores 1
        let p3 = translate_vnc_remote_input(
            &RemoteInput::Scroll {
                x: 150,
                y: 250,
                horizontal: 0,
                vertical: 120,
            },
            &mut held,
            &mut last,
            &mut sx,
            &mut sy,
            1920,
            1080,
        );
        assert_eq!(p3.len(), 12);
        assert_eq!(&p3[..6], &build_pointer_event(9, 150, 250));
        assert_eq!(&p3[6..12], &build_pointer_event(1, 150, 250));

        // Left up -> mask becomes 0
        let p4 = translate_vnc_remote_input(
            &RemoteInput::PointerButton {
                button: MouseButton::Left,
                down: false,
                x: 150,
                y: 250,
            },
            &mut held,
            &mut last,
            &mut sx,
            &mut sy,
            1920,
            1080,
        );
        assert_eq!(p4, build_pointer_event(0, 150, 250));
    }

    #[test]
    fn vnc_ctrl_alt_del_emits_six_key_events() {
        let bytes = build_vnc_shortcut_packets(RemoteShortcut::CtrlAltDel);
        assert_eq!(bytes.len(), 48); // 6 * 8 bytes
        assert_eq!(bytes[1], 1); // Ctrl down
        assert_eq!(bytes[9], 1); // Alt down
        assert_eq!(bytes[17], 1); // Del down
        assert_eq!(bytes[25], 0); // Del up
        assert_eq!(bytes[33], 0); // Alt up
        assert_eq!(bytes[41], 0); // Ctrl up
    }
}

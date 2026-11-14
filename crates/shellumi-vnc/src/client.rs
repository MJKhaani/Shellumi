//! High-level asynchronous RFB client event loop driving network I/O, frame decoding, and input events.

use crate::decoder::{decode_hextile_rectangle, TightDecoder, ZrleDecoder, MAX_CLIPBOARD_BYTES};
use crate::handshake::{client_and_server_init, negotiate_security, negotiate_version};
use crate::rectangles::{
    decode_copy_rect, decode_raw_rectangle, handle_desktop_size, ENCODING_COPYRECT,
    ENCODING_DESKTOP_SIZE, ENCODING_HEXTILE, ENCODING_RAW, ENCODING_TIGHT, ENCODING_ZRLE,
};
use crate::wire::{
    build_client_cut_text, build_framebuffer_update_request, build_key_event, build_pointer_event,
    build_set_encodings, build_set_pixel_format, server_msg, PixelFormat, RectangleHeader,
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

    #[error("Connection timed out after {0:?}")]
    Timeout(Duration),
}

/// Asynchronous VNC client task runner.
///
/// Drives the RFB session over TCP, blitting incoming rectangles into `framebuffer`
/// and transmitting outgoing input commands received from `worker`.
pub async fn run_vnc_session(
    config: SessionConfig,
    worker: SessionWorkerHandle,
    framebuffer: Framebuffer,
) -> Result<(), VncClientError> {
    let _ = worker
        .send_event(SessionEvent::StateChanged(SessionState::Connecting))
        .await;

    // 1. Establish TCP connection with 10-second timeout
    let endpoint = config.target_endpoint();
    let stream_result =
        tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(&endpoint)).await;

    let stream = match stream_result {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) => {
            let err_msg = format!("Failed to connect to {endpoint}: {err}");
            let _ = worker
                .send_event(SessionEvent::Error(err_msg.clone()))
                .await;
            let _ = worker
                .send_event(SessionEvent::StateChanged(SessionState::Error(err_msg)))
                .await;
            return Err(VncClientError::Network(err));
        }
        Err(_) => {
            let err_msg = format!("Connection to {endpoint} timed out after 10 seconds");
            let _ = worker
                .send_event(SessionEvent::Error(err_msg.clone()))
                .await;
            let _ = worker
                .send_event(SessionEvent::StateChanged(SessionState::Error(err_msg)))
                .await;
            return Err(VncClientError::Timeout(Duration::from_secs(10)));
        }
    };

    run_vnc_session_with_stream(stream, config, worker, framebuffer).await
}

/// Run an RFB VNC session over an arbitrary asynchronous stream (e.g. TCP, TLS, or in-memory duplex).
pub async fn run_vnc_session_with_stream<S>(
    mut stream: S,
    config: SessionConfig,
    worker: SessionWorkerHandle,
    mut framebuffer: Framebuffer,
) -> Result<(), VncClientError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // 2. Negotiate protocol version
    let version = match negotiate_version(&mut stream).await {
        Ok(v) => v,
        Err(err) => {
            let err_msg = err.to_string();
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
    if let Err(err) = negotiate_security(&mut stream, version, config.password.as_deref()).await {
        let err_msg = err.to_string();
        let _ = worker
            .send_event(SessionEvent::Error(err_msg.clone()))
            .await;
        let _ = worker
            .send_event(SessionEvent::StateChanged(SessionState::Error(err_msg)))
            .await;
        return Err(VncClientError::Handshake(err));
    }

    // 4. ClientInit and ServerInit
    let server_init = match client_and_server_init(&mut stream, true).await {
        Ok(init) => init,
        Err(err) => {
            let err_msg = err.to_string();
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

    // 7. Request supported encodings (CopyRect, Hextile, ZRLE, Tight, Raw, DesktopSize)
    let supported_encodings = [
        ENCODING_COPYRECT,
        ENCODING_HEXTILE,
        ENCODING_ZRLE,
        ENCODING_TIGHT,
        ENCODING_RAW,
        ENCODING_DESKTOP_SIZE,
    ];
    let set_enc_packet = build_set_encodings(&supported_encodings);
    stream.write_all(&set_enc_packet).await?;

    // 8. Request initial full framebuffer update
    let initial_fbur = build_framebuffer_update_request(false, 0, 0, width as u16, height as u16);
    stream.write_all(&initial_fbur).await?;
    stream.flush().await?;

    // 9. Transition to Connected state
    let _ = worker
        .send_event(SessionEvent::StateChanged(SessionState::Connected))
        .await;
    let _ = worker
        .send_event(SessionEvent::FrameReady {
            sequence: framebuffer.sequence(),
        })
        .await;

    let mut metrics = SessionMetrics::new();
    let mut metrics_interval = tokio::time::interval(Duration::from_millis(1000));
    let mut zrle_decoder = ZrleDecoder::new();
    let mut tight_decoder = TightDecoder::new();
    let mut last_sent_clipboard: Option<String> = None;
    let mut last_received_clipboard: Option<String> = None;

    // 10. Main RFB event loop
    loop {
        tokio::select! {
            // Outbound UI commands
            cmd_res = worker.recv_command() => {
                match cmd_res {
                    Ok(cmd) => match cmd {
                        SessionCommand::Pointer(p) => {
                            let packet = build_pointer_event(p.button_mask, p.x as u16, p.y as u16);
                            let _ = stream.write_all(&packet).await;
                            metrics.record_bytes_sent(packet.len() as u64);
                        }
                        SessionCommand::Key(k) => {
                            let packet = build_key_event(k.pressed, k.keysym);
                            let _ = stream.write_all(&packet).await;
                            metrics.record_bytes_sent(packet.len() as u64);
                        }
                        SessionCommand::SendClipboard(text) => {
                            // Bidirectional loop suppression: do not echo back identical server clipboard
                            if last_received_clipboard.as_deref() != Some(&text) {
                                last_sent_clipboard = Some(text.clone());
                                let packet = build_client_cut_text(&text);
                                let _ = stream.write_all(&packet).await;
                                metrics.record_bytes_sent(packet.len() as u64);
                            }
                        }
                        SessionCommand::Refresh => {
                            let fbur = build_framebuffer_update_request(false, 0, 0, framebuffer.width() as u16, framebuffer.height() as u16);
                            let _ = stream.write_all(&fbur).await;
                            metrics.record_bytes_sent(fbur.len() as u64);
                        }
                        SessionCommand::Disconnect => {
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
                        let _ = worker.send_event(SessionEvent::StateChanged(SessionState::Disconnected)).await;
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
                                    decode_raw_rectangle(&mut stream, &header, &client_pf, &mut framebuffer).await?;
                                }
                                ENCODING_COPYRECT => {
                                    decode_copy_rect(&mut stream, &header, &mut framebuffer).await?;
                                    metrics.record_bytes_received(4);
                                }
                                ENCODING_HEXTILE => {
                                    decode_hextile_rectangle(&mut stream, &header, &client_pf, &mut framebuffer).await?;
                                }
                                ENCODING_ZRLE => {
                                    zrle_decoder.decode_rectangle(&mut stream, &header, &client_pf, &mut framebuffer).await?;
                                }
                                ENCODING_TIGHT => {
                                    tight_decoder.decode_rectangle(&mut stream, &header, &client_pf, &mut framebuffer).await?;
                                }
                                ENCODING_DESKTOP_SIZE => {
                                    handle_desktop_size(&header, &mut framebuffer)?;
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
                        let _ = worker.send_event(SessionEvent::FrameReady { sequence: framebuffer.sequence() }).await;

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
                        let len = stream.read_u32().await? as usize;
                        metrics.record_bytes_received(7);

                        if len > MAX_CLIPBOARD_BYTES {
                            // Defensive protection: discard oversized payload to prevent OOM
                            let mut remaining = len;
                            let mut discard_buf = [0u8; 4096];
                            while remaining > 0 {
                                let n = remaining.min(discard_buf.len());
                                stream.read_exact(&mut discard_buf[..n]).await?;
                                remaining -= n;
                            }
                            metrics.record_bytes_received(len as u64);
                        } else {
                            let mut text_buf = vec![0u8; len];
                            stream.read_exact(&mut text_buf).await?;
                            metrics.record_bytes_received(len as u64);

                            let text = String::from_utf8_lossy(&text_buf).into_owned();
                            // Loop suppression: only propagate if not an echo of our own sent clipboard
                            if last_sent_clipboard.as_deref() != Some(&text) {
                                last_received_clipboard = Some(text.clone());
                                let _ = worker.send_event(SessionEvent::ServerClipboard(text)).await;
                            }
                        }
                    }
                    _other => {
                        // Unknown or unhandled server message
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

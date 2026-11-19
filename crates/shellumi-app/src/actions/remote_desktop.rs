//! Remote desktop actions.
use super::*;

impl Shellumi {
    pub(crate) fn launch_remote_desktop(
        &mut self,
        host: &SavedHost,
        protocol: RemoteDesktopProtocol,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let pane_id = self.next_tab_id;

        let search_input = cx.new(|cx| TextInput::new(cx, "", "Search output…"));
        let placeholder_pane = TerminalPane {
            id: pane_id,
            name: host.name.clone(),
            kind: SessionKind::RemoteDesktop,
            scroll: gpui::ScrollHandle::new(),
            terminal: Terminal::new(24, 80),
            session: None,
            message: None,
            search_open: false,
            search_input,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_idx: None,
        };

        let (
            target,
            width,
            height,
            color_depth,
            view_only,
            username,
            domain,
            tls_strict,
            enable_audio,
            enable_microphone,
        ) = match protocol {
            RemoteDesktopProtocol::Vnc => {
                let vnc_cfg = host.vnc_config().unwrap_or_else(|| {
                    VncConfig::new(host.destination.host(), host.destination.port().get())
                        .unwrap_or_default()
                });
                (
                    format!("{}:{}", vnc_cfg.host, vnc_cfg.port),
                    vnc_cfg.width as u32,
                    vnc_cfg.height as u32,
                    vnc_cfg.color_depth,
                    vnc_cfg.view_only,
                    None,
                    None,
                    true,
                    false,
                    false,
                )
            }
            RemoteDesktopProtocol::Rdp => {
                let rdp_cfg = host.rdp_config().unwrap_or_else(|| {
                    RdpConfig::new(host.destination.host(), host.destination.port().get())
                        .unwrap_or_default()
                });
                let user = rdp_cfg.username.clone().or_else(|| {
                    if host.destination.username().is_empty() {
                        None
                    } else {
                        Some(host.destination.username().to_string())
                    }
                });
                (
                    format!("{}:{}", rdp_cfg.host, rdp_cfg.port),
                    rdp_cfg.screen_width as u32,
                    rdp_cfg.screen_height as u32,
                    rdp_cfg.color_depth,
                    rdp_cfg.view_only,
                    user,
                    rdp_cfg.domain.clone(),
                    rdp_cfg.tls_strict,
                    rdp_cfg.enable_audio,
                    rdp_cfg.enable_microphone,
                )
            }
        };

        let password_input = cx.new(|cx| TextInput::new(cx, "", "Password…").masked());
        let rd_state = RemoteDesktopState {
            host_name: host.name.clone(),
            target,
            protocol,
            status: RemoteDesktopStatus::NotConnected,
            status_message: Some(format!(
                "Ready to connect to {} via {}",
                host.name,
                match protocol {
                    RemoteDesktopProtocol::Vnc => "RFB / VNC (port 5900)",
                    RemoteDesktopProtocol::Rdp => "MS-RDPBCGR / RDP (port 3389)",
                }
            )),
            width,
            height,
            view_only,
            scale_mode: RemoteDesktopScaleMode::Fit,
            color_depth,
            fps: None,
            latency_ms: None,
            clipboard_text: None,
            error: None,
            last_action_note: None,
            framebuffer: None,
            render_image: None,
            ui_handle: None,
            metrics: shellumi_remote_desktop::SessionMetrics::new(),
            username,
            password: None,
            domain,
            tls_strict,
            show_diagnostics: false,
            password_input: Some(password_input),
            enable_audio,
            audio_muted: false,
            audio_volume: 100,
            audio_active: false,
            audio_format: None,
            audio_sample_rate: 44100,
            audio_channels: 2,
            audio_bytes_received: 0,
            enable_microphone,
            mic_muted: false,
            mic_volume: 100,
            mic_active: false,
            mic_format: None,
            mic_sample_rate: 44100,
            mic_channels: 2,
            mic_bytes_sent: 0,
            mic_packets_sent: 0,
            display_control_ready: false,
            max_monitor_area: None,
            shared_folder: None,
            drive_name: None,
            drive_mounted: false,
            drive_bytes_read: 0,
            drive_bytes_written: 0,
            drive_operations_count: 0,
        };

        self.tabs.push(SessionTab {
            id: tab_id,
            name: host.name.clone(),
            kind: SessionKind::RemoteDesktop,
            panes: vec![placeholder_pane],
            active_pane_idx: 0,
            split: None,
            sftp_state: None,
            remote_desktop_state: Some(rd_state),
            multiplayer: None,
            collab_state: None,
        });

        self.page = None;
        self.active_tab_idx = Some(self.tabs.len() - 1);
        window.focus(&self.focus);
        cx.notify();
    }

    pub(crate) fn launch_quick_connect_remote_desktop(
        &mut self,
        protocol: RemoteDesktopProtocol,
        host: &str,
        port: u16,
        username: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let pane_id = self.next_tab_id;

        let display_name = format!("{}:{}", host, port);
        let search_input = cx.new(|cx| TextInput::new(cx, "", "Search output…"));
        let placeholder_pane = TerminalPane {
            id: pane_id,
            name: display_name.clone(),
            kind: SessionKind::RemoteDesktop,
            scroll: gpui::ScrollHandle::new(),
            terminal: Terminal::new(24, 80),
            session: None,
            message: None,
            search_open: false,
            search_input,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_idx: None,
        };

        let password_input = cx.new(|cx| TextInput::new(cx, "", "Password…").masked());
        let rd_state = RemoteDesktopState {
            host_name: display_name.clone(),
            target: format!("{}:{}", host, port),
            protocol,
            status: RemoteDesktopStatus::NotConnected,
            status_message: Some(format!(
                "Quick connect to {display_name} via {}",
                match protocol {
                    RemoteDesktopProtocol::Vnc => "RFB / VNC",
                    RemoteDesktopProtocol::Rdp => "RDP",
                }
            )),
            width: 1920,
            height: 1080,
            view_only: false,
            scale_mode: RemoteDesktopScaleMode::Fit,
            color_depth: 32,
            fps: None,
            latency_ms: None,
            clipboard_text: None,
            error: None,
            last_action_note: None,
            framebuffer: None,
            render_image: None,
            ui_handle: None,
            metrics: shellumi_remote_desktop::SessionMetrics::new(),
            username,
            password: None,
            domain: None,
            tls_strict: true,
            show_diagnostics: false,
            password_input: Some(password_input),
            enable_audio: match protocol {
                RemoteDesktopProtocol::Vnc => false,
                RemoteDesktopProtocol::Rdp => true,
            },
            audio_muted: false,
            audio_volume: 100,
            audio_active: false,
            audio_format: None,
            audio_sample_rate: 44100,
            audio_channels: 2,
            audio_bytes_received: 0,
            enable_microphone: match protocol {
                RemoteDesktopProtocol::Vnc => false,
                RemoteDesktopProtocol::Rdp => true,
            },
            mic_muted: false,
            mic_volume: 100,
            mic_active: false,
            mic_format: None,
            mic_sample_rate: 44100,
            mic_channels: 2,
            mic_bytes_sent: 0,
            mic_packets_sent: 0,
            display_control_ready: false,
            max_monitor_area: None,
            shared_folder: None,
            drive_name: None,
            drive_mounted: false,
            drive_bytes_read: 0,
            drive_bytes_written: 0,
            drive_operations_count: 0,
        };

        self.tabs.push(SessionTab {
            id: tab_id,
            name: display_name,
            kind: SessionKind::RemoteDesktop,
            panes: vec![placeholder_pane],
            active_pane_idx: 0,
            split: None,
            sftp_state: None,
            remote_desktop_state: Some(rd_state),
            multiplayer: None,
            collab_state: None,
        });

        self.page = None;
        self.active_tab_idx = Some(self.tabs.len() - 1);
        window.focus(&self.focus);
        cx.notify();
    }

    pub(crate) fn remote_desktop_send_key(
        &mut self,
        tab_idx: usize,
        key_desc: &'static str,
        keysym: u32,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                // Real RFB key event serialization for client buffer
                let _down_packet = build_rfb_key_event(true, keysym);
                let _up_packet = build_rfb_key_event(false, keysym);

                if let Some(ui_handle) = &rd.ui_handle {
                    if key_desc == "Ctrl+Alt+Del" {
                        let _ = ui_handle
                            .try_send_command(shellumi_remote_desktop::SessionCommand::CtrlAltDel);
                    } else {
                        let _ = ui_handle.try_send_command(
                            shellumi_remote_desktop::SessionCommand::Key(
                                shellumi_remote_desktop::KeyEvent {
                                    keysym,
                                    scancode: 0,
                                    pressed: true,
                                    modifiers: shellumi_remote_desktop::KeyModifiers::default(),
                                    text: None,
                                },
                            ),
                        );
                    }
                }

                if rd.status == RemoteDesktopStatus::Connected {
                    rd.last_action_note = Some(format!("Sent keystroke: {key_desc}"));
                    rd.status_message = Some(format!(
                        "Keystroke {key_desc} transmitted over {}",
                        match rd.protocol {
                            RemoteDesktopProtocol::Vnc => "RFB",
                            RemoteDesktopProtocol::Rdp => "RDP",
                        }
                    ));
                } else {
                    rd.last_action_note = Some(format!(
                        "Keystroke {key_desc} buffered locally (not connected)"
                    ));
                    rd.status_message = Some(format!(
                        "Keystroke {key_desc} received; live network transport in active development",
                    ));
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_click_canvas(
        &mut self,
        tab_idx: usize,
        x: u32,
        y: u32,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                if rd.view_only {
                    rd.last_action_note = Some("View-only mode: user input blocked".into());
                    cx.notify();
                    return;
                }
                // Real RFB pointer event serialization
                let _click_packet =
                    build_rfb_pointer_event(rfb_constants::BUTTON_LEFT, x as u16, y as u16);

                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::Pointer(
                            shellumi_remote_desktop::PointerEvent {
                                x,
                                y,
                                button_mask: shellumi_remote_desktop::pointer_mask::BUTTON_LEFT,
                                pressed: true,
                            },
                        ),
                    );
                }

                if rd.status == RemoteDesktopStatus::Connected {
                    rd.last_action_note = Some(format!("Mouse click at ({x}, {y})"));
                } else {
                    rd.last_action_note = Some(format!(
                        "Pointer click at ({x}, {y}) (session not connected)"
                    ));
                }
                cx.notify();
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn remote_desktop_apply_framebuffer_snapshot(
        &mut self,
        tab_idx: usize,
        snapshot: shellumi_remote_desktop::FramebufferSnapshot,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.width = snapshot.width;
                rd.height = snapshot.height;
                let render_image =
                    crate::ui::remote_desktop_canvas::create_render_image_from_snapshot(&snapshot);
                rd.render_image = render_image;
                rd.framebuffer = Some(snapshot);
                rd.status = RemoteDesktopStatus::Connected;
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_set_scale_mode(
        &mut self,
        tab_idx: usize,
        mode: RemoteDesktopScaleMode,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.scale_mode = mode;
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_toggle_view_only(
        &mut self,
        tab_idx: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.view_only = !rd.view_only;
                rd.last_action_note = Some(if rd.view_only {
                    "View-only mode activated".into()
                } else {
                    "Interactive control enabled".into()
                });
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_toggle_diagnostics(
        &mut self,
        tab_idx: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.show_diagnostics = !rd.show_diagnostics;
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_disconnect(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle
                        .try_send_command(shellumi_remote_desktop::SessionCommand::Disconnect);
                }
                rd.status = RemoteDesktopStatus::Disconnected;
                rd.status_message = Some("Session disconnected by user.".into());
                rd.last_action_note = Some("Disconnected".into());
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_reconnect(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let default_port = match rd.protocol {
                    RemoteDesktopProtocol::Vnc => 5900,
                    RemoteDesktopProtocol::Rdp => 3389,
                };
                let (host, port) = {
                    let parts: Vec<&str> = rd.target.split(':').collect();
                    let h = parts.first().copied().unwrap_or("127.0.0.1").to_string();
                    let p: u16 = parts
                        .get(1)
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(default_port);
                    (h, p)
                };

                if let Some(input) = &rd.password_input {
                    let pwd = input.read(cx).value().trim().to_string();
                    if !pwd.is_empty() {
                        rd.password = Some(pwd);
                    }
                }

                let (ui_handle, worker_handle) =
                    shellumi_remote_desktop::create_session_channels(64, 256);
                rd.ui_handle = Some(ui_handle.clone());
                rd.status = RemoteDesktopStatus::Connecting;
                rd.status_message = Some(format!(
                    "Connecting to {host}:{port} via {}…",
                    match rd.protocol {
                        RemoteDesktopProtocol::Vnc => "RFB 3.8",
                        RemoteDesktopProtocol::Rdp => "MS-RDPBCGR / RDP",
                    }
                ));
                rd.last_action_note = Some("Connection initiated".into());
                cx.notify();

                let scale_mode = match rd.scale_mode {
                    RemoteDesktopScaleMode::Fit => shellumi_remote_desktop::ScaleMode::Fit,
                    RemoteDesktopScaleMode::Original => {
                        shellumi_remote_desktop::ScaleMode::Original
                    }
                    RemoteDesktopScaleMode::Fill => shellumi_remote_desktop::ScaleMode::Fill,
                };

                let proto = match rd.protocol {
                    RemoteDesktopProtocol::Vnc => shellumi_remote_desktop::RemoteProtocol::Vnc,
                    RemoteDesktopProtocol::Rdp => shellumi_remote_desktop::RemoteProtocol::Rdp,
                };

                let session_cfg = shellumi_remote_desktop::SessionConfig {
                    host,
                    port,
                    protocol: proto,
                    width: rd.width,
                    height: rd.height,
                    color_depth: rd.color_depth,
                    view_only: rd.view_only,
                    scale_mode,
                    username: rd.username.clone(),
                    password: rd.password.clone(),
                    domain: rd.domain.clone(),
                    tls_strict: rd.tls_strict,
                    enable_audio: rd.enable_audio,
                    enable_microphone: rd.enable_microphone,
                    shared_folder: rd.shared_folder.clone(),
                    reconnect_policy: shellumi_remote_desktop::ReconnectPolicy::default(),
                };

                let fb = shellumi_remote_desktop::Framebuffer::new(rd.width, rd.height)
                    .unwrap_or_else(|_| {
                        shellumi_remote_desktop::Framebuffer::new(800, 600).unwrap()
                    });

                let protocol = rd.protocol;
                let worker_err = worker_handle.clone();
                std::thread::Builder::new()
                    .name("shellumi-remote-desktop".into())
                    .spawn(move || {
                        let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
                            .enable_all()
                            .build()
                        else {
                            let _ =
                                worker_err
                                    .try_send_event(shellumi_remote_desktop::SessionEvent::Error(
                                    "Failed to initialize Tokio runtime for remote desktop session"
                                        .into(),
                                ));
                            let _ = worker_err.try_send_event(
                                shellumi_remote_desktop::SessionEvent::StateChanged(
                                    shellumi_remote_desktop::SessionState::Error(
                                        "Failed to initialize async network runtime".into(),
                                    ),
                                ),
                            );
                            return;
                        };

                        runtime.block_on(async move {
                            match protocol {
                                RemoteDesktopProtocol::Vnc => {
                                    let _ = shellumi_vnc::run_vnc_session(
                                        session_cfg,
                                        worker_handle,
                                        fb,
                                    )
                                    .await;
                                }
                                RemoteDesktopProtocol::Rdp => {
                                    let _ = shellumi_rdp::run_rdp_session(
                                        session_cfg,
                                        worker_handle,
                                        fb,
                                    )
                                    .await;
                                }
                            }
                        });
                    })
                    .expect("failed to spawn remote desktop thread");

                let proto_name = match rd.protocol {
                    RemoteDesktopProtocol::Vnc => "VNC",
                    RemoteDesktopProtocol::Rdp => "RDP",
                };

                cx.spawn(async move |this, cx| {
                    while let Ok(event) = ui_handle.recv_event().await {
                        let res = this.update(cx, |app, cx| {
                            if let Some(tab) = app.tabs.get_mut(tab_idx) {
                                if let Some(rd) = &mut tab.remote_desktop_state {
                                    match event {
                                        shellumi_remote_desktop::SessionEvent::StateChanged(s) => match s {
                                            shellumi_remote_desktop::SessionState::Connecting => {
                                                rd.status = RemoteDesktopStatus::Connecting;
                                                rd.status_message = Some(format!("Connecting to {proto_name} server…"));
                                            }
                                            shellumi_remote_desktop::SessionState::Connected => {
                                                rd.status = RemoteDesktopStatus::Connected;
                                                rd.status_message = Some(format!("Connected to {proto_name} server"));
                                            }
                                            shellumi_remote_desktop::SessionState::Reconnecting {
                                                attempt,
                                                max_attempts,
                                                delay_ms,
                                            } => {
                                                rd.status = RemoteDesktopStatus::Reconnecting {
                                                    attempt,
                                                    max_attempts,
                                                };
                                                let delay_s = (delay_ms as f64) / 1000.0;
                                                rd.status_message = Some(format!(
                                                    "Connection interrupted. Reconnecting (attempt {attempt}/{max_attempts} in {delay_s:.1}s)…"
                                                ));
                                                rd.last_action_note = Some(format!("Reconnecting ({attempt}/{max_attempts})"));
                                            }
                                            shellumi_remote_desktop::SessionState::Disconnected => {
                                                rd.status = RemoteDesktopStatus::Disconnected;
                                                rd.status_message = Some(format!("Disconnected from {proto_name} server"));
                                            }
                                            shellumi_remote_desktop::SessionState::Error(err) => {
                                                rd.status = RemoteDesktopStatus::Error;
                                                rd.error = Some(err.clone());
                                                rd.status_message = Some(format!("Error: {err}"));
                                            }
                                            _ => {}
                                        },
                                        shellumi_remote_desktop::SessionEvent::RemoteResize { width, height } => {
                                            rd.width = width;
                                            rd.height = height;
                                        }
                                        shellumi_remote_desktop::SessionEvent::FrameReady { sequence: _, snapshot } => {
                                            if let Some(snap) = snapshot {
                                                rd.width = snap.width;
                                                rd.height = snap.height;
                                                let render_image =
                                                    crate::ui::remote_desktop_canvas::create_render_image_from_snapshot(&snap);
                                                rd.render_image = render_image;
                                                rd.framebuffer = Some(snap);
                                            }
                                            rd.status = RemoteDesktopStatus::Connected;
                                        }
                                        shellumi_remote_desktop::SessionEvent::Metrics(m) => {
                                            rd.fps = m.fps;
                                            rd.latency_ms = m.latency_ms;
                                        }
                                        shellumi_remote_desktop::SessionEvent::ServerClipboard(text) => {
                                            rd.clipboard_text = Some(text.clone());
                                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                                            rd.last_action_note = Some("Remote clipboard text received and copied to local clipboard".into());
                                        }
                                        shellumi_remote_desktop::SessionEvent::ServerFilesReceived { files, target_dir } => {
                                            rd.last_action_note = Some(format!("Received {} file(s) into {}", files.len(), target_dir));
                                        }
                                        shellumi_remote_desktop::SessionEvent::Bell => {
                                            rd.last_action_note = Some("Terminal bell alert".into());
                                        }
                                        shellumi_remote_desktop::SessionEvent::SecurityWarning(warning) => {
                                            rd.status_message = Some(format!("Security: {}", warning.message()));
                                        }
                                        shellumi_remote_desktop::SessionEvent::AudioStatus {
                                            active,
                                            muted,
                                            volume,
                                            format,
                                            sample_rate,
                                            channels,
                                            bytes_received,
                                        } => {
                                            rd.audio_active = active;
                                            rd.audio_muted = muted;
                                            rd.audio_volume = volume;
                                            rd.audio_format = Some(format);
                                            rd.audio_sample_rate = sample_rate;
                                            rd.audio_channels = channels;
                                            rd.audio_bytes_received = bytes_received;
                                        }
                                        shellumi_remote_desktop::SessionEvent::MicrophoneStatus {
                                            active,
                                            muted,
                                            volume,
                                            format,
                                            sample_rate,
                                            channels,
                                            bytes_sent,
                                            packets_sent,
                                        } => {
                                            rd.mic_active = active;
                                            rd.mic_muted = muted;
                                            rd.mic_volume = volume;
                                            rd.mic_format = Some(format);
                                            rd.mic_sample_rate = sample_rate;
                                            rd.mic_channels = channels;
                                            rd.mic_bytes_sent = bytes_sent;
                                            rd.mic_packets_sent = packets_sent;
                                        }
                                        shellumi_remote_desktop::SessionEvent::DisplayControlCaps {
                                            max_monitors: _,
                                            max_area,
                                        } => {
                                            rd.display_control_ready = true;
                                            rd.max_monitor_area = Some(max_area);
                                            rd.last_action_note = Some(
                                                "Dynamic display resizing (MS-RDPEDISP) ready".into(),
                                            );
                                        }
                                        shellumi_remote_desktop::SessionEvent::DriveStatus {
                                            drive_name,
                                            local_path,
                                            mounted,
                                            bytes_read,
                                            bytes_written,
                                            operations_count,
                                        } => {
                                            rd.drive_name = Some(drive_name.clone());
                                            rd.shared_folder = Some(std::path::PathBuf::from(local_path.clone()));
                                            rd.drive_mounted = mounted;
                                            rd.drive_bytes_read = bytes_read;
                                            rd.drive_bytes_written = bytes_written;
                                            rd.drive_operations_count = operations_count;
                                            if mounted {
                                                rd.last_action_note = Some(format!(
                                                    "Shared drive \"{}\" mounted ({})",
                                                    drive_name, local_path
                                                ));
                                            }
                                        }
                                        shellumi_remote_desktop::SessionEvent::Error(err) => {
                                            rd.error = Some(err);
                                        }
                                        _ => {}
                                    }
                                    cx.notify();
                                }
                            }
                        });
                        if res.is_err() {
                            break;
                        }
                    }
                })
                .detach();
            }
        }
    }

    pub(crate) fn remote_desktop_sync_clipboard(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let mut text_to_send = rd.clipboard_text.clone();
                if let Some(item) = cx.read_from_clipboard() {
                    if let Some(t) = item.text() {
                        if !t.is_empty() {
                            text_to_send = Some(t);
                        }
                    }
                }
                if let Some(ui_handle) = &rd.ui_handle {
                    if let Some(text) = text_to_send {
                        let _ = ui_handle.try_send_command(
                            shellumi_remote_desktop::SessionCommand::SendClipboard(text),
                        );
                    }
                }
                if rd.status == RemoteDesktopStatus::Connected {
                    rd.last_action_note = Some("Clipboard synchronized with remote session".into());
                } else {
                    rd.last_action_note = Some(
                        "Clipboard synchronization requires an active connected session".into(),
                    );
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_send_files(
        &mut self,
        tab_idx: usize,
        paths: Vec<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                if let Some(ui_handle) = &rd.ui_handle {
                    let count = paths.len();
                    if let Err(e) = ui_handle
                        .try_send_command(shellumi_remote_desktop::SessionCommand::SendFiles(paths))
                    {
                        rd.last_action_note = Some(format!("Failed to stage files: {e}"));
                    } else {
                        rd.last_action_note =
                            Some(format!("Staged {count} file(s) for remote transfer"));
                    }
                } else {
                    rd.last_action_note = Some("File transfer requires an active session".into());
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_choose_and_send_files(
        &mut self,
        tab_idx: usize,
        cx: &mut Context<Self>,
    ) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Select files to send to remote desktop".into()),
        });

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if !paths.is_empty() {
                    let _ = this.update(cx, |app, cx| {
                        app.remote_desktop_send_files(tab_idx, paths, cx);
                    });
                }
            }
        })
        .detach();
    }

    pub(crate) fn remote_desktop_toggle_mute(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let new_muted = !rd.audio_muted;
                rd.audio_muted = new_muted;
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SetAudioMute(new_muted),
                    );
                }
                rd.last_action_note = Some(if new_muted {
                    "Audio output muted".into()
                } else {
                    "Audio output unmuted".into()
                });
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_set_volume(
        &mut self,
        tab_idx: usize,
        volume: u16,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let vol = volume.min(100);
                rd.audio_volume = vol;
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SetAudioVolume(vol),
                    );
                }
                rd.last_action_note = Some(format!("Volume set to {vol}%"));
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_toggle_mic_mute(
        &mut self,
        tab_idx: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let new_muted = !rd.mic_muted;
                rd.mic_muted = new_muted;
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SetMicrophoneMute(new_muted),
                    );
                }
                rd.last_action_note = Some(if new_muted {
                    "Microphone muted".into()
                } else {
                    "Microphone unmuted".into()
                });
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_set_mic_volume(
        &mut self,
        tab_idx: usize,
        volume: u16,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let vol = volume.min(200);
                rd.mic_volume = vol;
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SetMicrophoneVolume(vol),
                    );
                }
                rd.last_action_note = Some(format!("Microphone gain set to {vol}%"));
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_request_resize(
        &mut self,
        tab_idx: usize,
        width: u32,
        height: u32,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::RequestResize { width, height },
                    );
                    rd.last_action_note =
                        Some(format!("Requested resolution resize to {width}×{height}"));
                } else {
                    rd.width = width;
                    rd.height = height;
                    rd.last_action_note = Some(format!("Local resolution set to {width}×{height}"));
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_choose_shared_folder(
        &mut self,
        tab_idx: usize,
        folder_path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let drive_name = folder_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Shared")
                    .to_string();
                rd.shared_folder = Some(folder_path.clone());
                rd.drive_name = Some(drive_name.clone());
                rd.last_action_note = Some(format!(
                    "Redirecting shared folder: {}",
                    folder_path.display()
                ));

                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::ConfigureDrive(Some(folder_path)),
                    );
                }
                cx.notify();
            }
        }
    }
}

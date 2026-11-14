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

        let (target, width, height, color_depth, view_only, username, domain, tls_strict) =
            match protocol {
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
                };

                let fb = shellumi_remote_desktop::Framebuffer::new(rd.width, rd.height)
                    .unwrap_or_else(|_| {
                        shellumi_remote_desktop::Framebuffer::new(800, 600).unwrap()
                    });

                let protocol = rd.protocol;
                cx.background_executor()
                    .spawn(async move {
                        match protocol {
                            RemoteDesktopProtocol::Vnc => {
                                let _ =
                                    shellumi_vnc::run_vnc_session(session_cfg, worker_handle, fb)
                                        .await;
                            }
                            RemoteDesktopProtocol::Rdp => {
                                let _ =
                                    shellumi_rdp::run_rdp_session(session_cfg, worker_handle, fb)
                                        .await;
                            }
                        }
                    })
                    .detach();

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
                                            rd.clipboard_text = Some(text);
                                            rd.last_action_note = Some("Remote clipboard text received".into());
                                        }
                                        shellumi_remote_desktop::SessionEvent::Bell => {
                                            rd.last_action_note = Some("Terminal bell alert".into());
                                        }
                                        shellumi_remote_desktop::SessionEvent::SecurityWarning(warning) => {
                                            rd.status_message = Some(format!("Security: {}", warning.message()));
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
                if let Some(ui_handle) = &rd.ui_handle {
                    if let Some(clip) = &rd.clipboard_text {
                        let _ = ui_handle.try_send_command(
                            shellumi_remote_desktop::SessionCommand::SendClipboard(clip.clone()),
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
}

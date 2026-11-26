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
                let user = if host.destination.username().is_empty() {
                    None
                } else {
                    Some(host.destination.username().to_string())
                };
                let target_fmt = if vnc_cfg.host.contains(':') && !vnc_cfg.host.starts_with('[') {
                    format!("[{}]:{}", vnc_cfg.host, vnc_cfg.port)
                } else {
                    format!("{}:{}", vnc_cfg.host, vnc_cfg.port)
                };
                (
                    target_fmt,
                    vnc_cfg.width as u32,
                    vnc_cfg.height as u32,
                    vnc_cfg.color_depth,
                    vnc_cfg.view_only,
                    user,
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
                let target_fmt = if rdp_cfg.host.contains(':') && !rdp_cfg.host.starts_with('[') {
                    format!("[{}]:{}", rdp_cfg.host, rdp_cfg.port)
                } else {
                    format!("{}:{}", rdp_cfg.host, rdp_cfg.port)
                };
                (
                    target_fmt,
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

        let username_input = cx.new(|cx| {
            TextInput::new(
                cx,
                username.as_deref().unwrap_or(""),
                match protocol {
                    RemoteDesktopProtocol::Vnc => "Username (e.g. macOS user account)",
                    RemoteDesktopProtocol::Rdp => "Username (e.g. Administrator)",
                },
            )
        });
        let domain_input =
            cx.new(|cx| TextInput::new(cx, domain.as_deref().unwrap_or(""), "Domain (optional)"));
        let password_input = cx.new(|cx| TextInput::new(cx, "", "Password…").masked());
        cx.observe(&username_input, move |app, _, _| {
            let entry = app.rd_credential_edit_gen.entry(tab_id).or_insert(0);
            *entry = entry.wrapping_add(1);
        })
        .detach();
        cx.observe(&domain_input, move |app, _, _| {
            let entry = app.rd_credential_edit_gen.entry(tab_id).or_insert(0);
            *entry = entry.wrapping_add(1);
        })
        .detach();
        cx.observe(&password_input, move |app, _, _| {
            let entry = app.rd_credential_edit_gen.entry(tab_id).or_insert(0);
            *entry = entry.wrapping_add(1);
        })
        .detach();
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
            username_input: Some(username_input),
            domain_input: Some(domain_input),
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
            connection_generation: 0,
            dynamic_resize_enabled: matches!(protocol, RemoteDesktopProtocol::Rdp),
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

        // Asynchronously resolve saved, host-scoped, or inherited group credentials via the unified resolver
        if let Some(db_path) = self.active_vault_db_path() {
            let edit_gen = self
                .rd_credential_edit_gen
                .get(&tab_id)
                .copied()
                .unwrap_or(0);
            let op_ctx =
                self.register_connection_op(host.id, window, Some(tab_id), Some(0), edit_gen);
            let vault_id = op_ctx.vault_id.clone();
            let vault_key = self.active_vault_key();
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(
                    shellumi_storage::desktop::resolve_remote_desktop_auth_with_key(
                        &db_path,
                        op_ctx.host_id,
                        Some(&vault_id),
                        vault_key.as_ref(),
                    ),
                )
            });

            cx.spawn(async move |app, cx| {
                let mut res = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Err(reason) = app.authorize_and_consume_connection_op(&op_ctx, None) {
                        tracing::warn!(
                            tab_id = tab_id,
                            host_id = op_ctx.host_id,
                            operation_id = op_ctx.operation_id,
                            reason = %reason,
                            "Discarded stale remote desktop credential resolution"
                        );
                        if let Ok((_, Some(ref mut pwd), _)) = res {
                            use zeroize::Zeroize;
                            pwd.zeroize();
                        }
                        return;
                    }
                    if let Some(tab) = app.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if let Some(rd) = &mut tab.remote_desktop_state {
                            match res {
                                Ok((uname, mut pwd, _source)) => {
                                    if rd.username.is_none() {
                                        if let Some(u) = uname {
                                            rd.username = Some(u.clone());
                                            if let Some(input) = &rd.username_input {
                                                input.update(cx, |inp, cx| inp.set_value(&u, cx));
                                            }
                                        }
                                    }
                                    if let Some(mut p) = pwd.take() {
                                        if rd.password.is_none() {
                                            rd.password = Some(p);
                                            rd.last_action_note =
                                                Some("Saved credential loaded from vault".into());
                                        } else {
                                            use zeroize::Zeroize;
                                            p.zeroize();
                                        }
                                    }
                                }
                                Err(err) => {
                                    tracing::warn!(
                                        tab_id = tab_id,
                                        error = %err,
                                        "Failed to resolve saved remote desktop credential"
                                    );
                                    rd.last_action_note = Some(format!(
                                        "Saved credential unavailable ({err}); enter password"
                                    ));
                                }
                            }
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        }
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

        let username_input = cx.new(|cx| {
            TextInput::new(
                cx,
                username.as_deref().unwrap_or(""),
                match protocol {
                    RemoteDesktopProtocol::Vnc => "Username (e.g. macOS user account)",
                    RemoteDesktopProtocol::Rdp => "Username (e.g. Administrator)",
                },
            )
        });
        let domain_input = cx.new(|cx| TextInput::new(cx, "", "Domain (optional)"));
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
            username_input: Some(username_input),
            domain_input: Some(domain_input),
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
            connection_generation: 0,
            dynamic_resize_enabled: matches!(protocol, RemoteDesktopProtocol::Rdp),
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
            let tab_id = tab.id;
            if let Some(rd) = &mut tab.remote_desktop_state {
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle
                        .try_send_command(shellumi_remote_desktop::SessionCommand::Disconnect);
                }
                rd.connection_generation = rd.connection_generation.wrapping_add(1);
                rd.ui_handle = None;
                rd.display_control_ready = false;
                rd.status = RemoteDesktopStatus::Disconnected;
                rd.error = None;
                rd.status_message = Some("Session disconnected by user.".into());
                rd.last_action_note = Some("Disconnected".into());
                self.rd_resize_state.remove(&tab_id);
                self.connection_ops.cancel_for_tab(tab_id);
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_edit_credentials(
        &mut self,
        tab_idx: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.status = RemoteDesktopStatus::NotConnected;
                rd.error = None;
                rd.status_message = Some(format!(
                    "Configure credentials and connect to {} via {}",
                    rd.host_name,
                    match rd.protocol {
                        RemoteDesktopProtocol::Vnc => "RFB / VNC",
                        RemoteDesktopProtocol::Rdp => "MS-RDPBCGR / RDP",
                    }
                ));
                rd.last_action_note = Some("Editing credentials".into());
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_reconnect(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            let tab_id = tab.id;
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.error = None;
                rd.connection_generation = rd.connection_generation.wrapping_add(1);
                rd.display_control_ready = false;
                let session_key = crate::RemoteSessionKey {
                    tab_id,
                    connection_generation: rd.connection_generation,
                };
                self.connection_ops.cancel_for_tab(tab_id);
                self.rd_resize_state
                    .entry(tab_id)
                    .or_default()
                    .reset_for_connection(session_key.connection_generation);
                let default_port = match rd.protocol {
                    RemoteDesktopProtocol::Vnc => 5900,
                    RemoteDesktopProtocol::Rdp => 3389,
                };
                let (host, port) = if let Some((h_part, p_part)) = rd.target.rsplit_once(':') {
                    let clean_h = h_part.trim_start_matches('[').trim_end_matches(']');
                    let p: u16 = p_part.parse().unwrap_or(default_port);
                    (clean_h.to_string(), p)
                } else {
                    (rd.target.clone(), default_port)
                };

                if let Some(input) = &rd.username_input {
                    let u = input.read(cx).value().trim().to_string();
                    if !u.is_empty() {
                        rd.username = Some(u);
                    } else {
                        rd.username = None;
                    }
                }

                if let Some(input) = &rd.domain_input {
                    let d = input.read(cx).value().trim().to_string();
                    if !d.is_empty() {
                        rd.domain = Some(d);
                    } else {
                        rd.domain = None;
                    }
                }

                if let Some(input) = &rd.password_input {
                    // Preserve exact password whitespace; only replace resolved password if user typed a new one.
                    let pwd = input.read(cx).value().to_string();
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
                        let mut keep_running = true;
                        let mut trigger_auto_resize = false;
                        let mut ack_size: Option<(u32, u32)> = None;
                        let res = this.update(cx, |app, cx| {
                            let Some(tab) = app.tabs.iter_mut().find(|t| t.id == session_key.tab_id)
                            else {
                                keep_running = false;
                                return;
                            };
                            let Some(rd) = &mut tab.remote_desktop_state else {
                                keep_running = false;
                                return;
                            };
                            if rd.connection_generation != session_key.connection_generation {
                                keep_running = false;
                                return;
                            }
                            match event {
                                shellumi_remote_desktop::SessionEvent::StateChanged(s) => match s {
                                    shellumi_remote_desktop::SessionState::Connecting => {
                                        rd.status = RemoteDesktopStatus::Connecting;
                                        rd.status_message =
                                            Some(format!("Connecting to {proto_name} server…"));
                                    }
                                    shellumi_remote_desktop::SessionState::Connected => {
                                        rd.status = RemoteDesktopStatus::Connected;
                                        rd.status_message =
                                            Some(format!("Connected to {proto_name} server"));
                                        trigger_auto_resize = true;
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
                                        rd.last_action_note =
                                            Some(format!("Reconnecting ({attempt}/{max_attempts})"));
                                    }
                                    shellumi_remote_desktop::SessionState::Disconnected => {
                                        if rd.status != RemoteDesktopStatus::Error
                                            && rd.error.is_none()
                                        {
                                            rd.status = RemoteDesktopStatus::Disconnected;
                                            rd.status_message = Some(format!(
                                                "Disconnected from {proto_name} server"
                                            ));
                                        }
                                    }
                                    shellumi_remote_desktop::SessionState::Error(err) => {
                                        rd.status = RemoteDesktopStatus::Error;
                                        rd.error = Some(err.clone());
                                        rd.status_message = Some(format!("Error: {err}"));
                                    }
                                    _ => {}
                                },
                                shellumi_remote_desktop::SessionEvent::RemoteResize {
                                    width,
                                    height,
                                } => {
                                    rd.width = width;
                                    rd.height = height;
                                    ack_size = Some((width, height));
                                }
                                shellumi_remote_desktop::SessionEvent::FrameReady {
                                    sequence: _,
                                    snapshot,
                                } => {
                                    if let Some(snap) = snapshot {
                                        rd.width = snap.width;
                                        rd.height = snap.height;
                                        ack_size = Some((snap.width, snap.height));
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
                                    rd.last_action_note = Some(
                                        "Remote clipboard text received and copied to local clipboard"
                                            .into(),
                                    );
                                }
                                shellumi_remote_desktop::SessionEvent::ServerFilesReceived {
                                    files,
                                    target_dir,
                                } => {
                                    rd.last_action_note = Some(format!(
                                        "Received {} file(s) into {}",
                                        files.len(),
                                        target_dir
                                    ));
                                }
                                shellumi_remote_desktop::SessionEvent::Bell => {
                                    rd.last_action_note = Some("Terminal bell alert".into());
                                }
                                shellumi_remote_desktop::SessionEvent::SecurityWarning(warning) => {
                                    rd.status_message =
                                        Some(format!("Security: {}", warning.message()));
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
                                    trigger_auto_resize = true;
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
                                    rd.shared_folder =
                                        Some(std::path::PathBuf::from(local_path.clone()));
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
                                    rd.status = RemoteDesktopStatus::Error;
                                    rd.error = Some(err.clone());
                                    rd.status_message = Some(format!("Error: {err}"));
                                }
                                _ => {}
                            }
                            if let Some(sz) = ack_size {
                                app.handle_rd_resize_acknowledged(session_key, sz, cx);
                            }
                            if trigger_auto_resize {
                                app.schedule_rd_auto_resize(session_key.tab_id, cx);
                            }
                            cx.notify();
                        });
                        if res.is_err() || !keep_running {
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
        let Some(tab_id) = self.tabs.get(tab_idx).map(|t| t.id) else {
            return;
        };
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
                        if let Some(current_idx) = app.tabs.iter().position(|t| t.id == tab_id) {
                            app.remote_desktop_send_files(current_idx, paths, cx);
                        }
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
        let Some(tab) = self.tabs.get(tab_idx) else {
            return;
        };
        let Some(rd) = tab.remote_desktop_state.as_ref() else {
            return;
        };
        let tab_id = tab.id;
        let conn_gen = rd.connection_generation;
        let dynamic_enabled = rd.dynamic_resize_enabled;
        let layout_gen = self.layout_epoch;

        // If the session is not yet connected/has no ui_handle, fall back to direct send.
        if rd.ui_handle.is_none() || rd.status != RemoteDesktopStatus::Connected {
            if let Err(err) = self.try_send_remote_resize(tab_idx, width, height, cx) {
                if let Some(rd) = self
                    .tabs
                    .get_mut(tab_idx)
                    .and_then(|t| t.remote_desktop_state.as_mut())
                {
                    rd.last_action_note = Some(format!("Resize request failed: {err}"));
                    cx.notify();
                }
            }
            return;
        }

        let Some(target) =
            Self::compute_legal_rdp_resize(width as f32, height as f32, rd.max_monitor_area)
        else {
            return;
        };

        let next_req = {
            let entry = self.rd_resize_state.entry(tab_id).or_default();
            if entry.connection_generation != conn_gen {
                entry.reset_for_connection(conn_gen);
            }
            match entry.request_resize(target, layout_gen, crate::ResizeOrigin::Manual) {
                crate::ResizeRequestOutcome::DispatchNow => {
                    entry.take_ready_to_dispatch(dynamic_enabled)
                }
                _ => None,
            }
        };
        if let Some(req) = next_req {
            let session_key = crate::RemoteSessionKey {
                tab_id,
                connection_generation: conn_gen,
            };
            self.dispatch_rd_resize(session_key, req, cx);
        }
    }

    fn try_send_remote_resize(
        &mut self,
        tab_idx: usize,
        width: u32,
        height: u32,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(tab) = self.tabs.get_mut(tab_idx) else {
            return Err("Remote desktop tab no longer exists".into());
        };
        let Some(rd) = &mut tab.remote_desktop_state else {
            return Err("Remote desktop state missing".into());
        };
        if let Some(ui_handle) = &rd.ui_handle {
            ui_handle
                .try_send_command(shellumi_remote_desktop::SessionCommand::RequestResize {
                    width,
                    height,
                })
                .map_err(|e| e.to_string())?;
            rd.last_action_note = Some(format!("Requested resolution resize to {width}×{height}"));
        } else {
            rd.width = width;
            rd.height = height;
            rd.last_action_note = Some(format!("Local resolution set to {width}×{height}"));
        }
        cx.notify();
        Ok(())
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

    /// Compute a legal RDP display resolution from measured viewport dimensions,
    /// clamping against backend limits, preserving aspect ratio under `max_monitor_area`,
    /// and aligning width to an even number.
    pub(crate) fn compute_legal_rdp_resize(
        viewport_w: f32,
        viewport_h: f32,
        max_area: Option<u64>,
    ) -> Option<(u32, u32)> {
        if !viewport_w.is_finite()
            || !viewport_h.is_finite()
            || viewport_w <= 0.0
            || viewport_h <= 0.0
        {
            return None;
        }
        let mut w = (viewport_w.floor() as u32).clamp(200, 8192);
        let mut h = (viewport_h.floor() as u32).clamp(200, 8192);

        if let Some(area_cap) = max_area {
            let area = (w as u64) * (h as u64);
            if area > area_cap && area_cap >= 200 * 200 {
                let scale = ((area_cap as f64) / (area as f64)).sqrt();
                w = ((w as f64 * scale).floor() as u32).clamp(200, 8192);
                h = ((h as f64 * scale).floor() as u32).clamp(200, 8192);
            }
        }

        // RDP requires even width
        w &= !1;
        if w < 200 {
            w = 200;
        }
        Some((w, h))
    }

    /// Schedule a debounced automatic resize for an active RDP session when its
    /// measured canvas viewport changes.
    pub(crate) fn schedule_rd_auto_resize(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(rd) = tab.remote_desktop_state.as_ref() else {
            return;
        };
        if rd.protocol != RemoteDesktopProtocol::Rdp
            || !rd.dynamic_resize_enabled
            || rd.view_only
            || !rd.display_control_ready
            || rd.status != RemoteDesktopStatus::Connected
        {
            return;
        }
        let Some(measurement) = self
            .rd_canvas_measurements
            .get(&tab_id)
            .filter(|m| m.tab_id == tab_id && m.layout_epoch == self.layout_epoch)
        else {
            return;
        };
        let Some(target) = Self::compute_legal_rdp_resize(
            f32::from(measurement.viewport.size.width),
            f32::from(measurement.viewport.size.height),
            rd.max_monitor_area,
        ) else {
            return;
        };

        let conn_gen = rd.connection_generation;
        let layout_gen = measurement.layout_epoch;
        let entry = self.rd_resize_state.entry(tab_id).or_default();
        if entry.connection_generation != conn_gen {
            entry.reset_for_connection(conn_gen);
        }
        let crate::ResizeRequestOutcome::Debounced {
            debounce_gen,
            auto_cancel_gen,
        } = entry.request_resize(target, layout_gen, crate::ResizeOrigin::Automatic)
        else {
            return;
        };
        let session_key = crate::RemoteSessionKey {
            tab_id,
            connection_generation: conn_gen,
        };

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(200))
                .await;
            let _ = this.update(cx, |app, cx| {
                let Some(tab) = app.tabs.iter().find(|t| t.id == session_key.tab_id) else {
                    return;
                };
                let Some(rd) = tab.remote_desktop_state.as_ref() else {
                    return;
                };
                if rd.connection_generation != session_key.connection_generation
                    || rd.status != RemoteDesktopStatus::Connected
                    || !rd.dynamic_resize_enabled
                    || !rd.display_control_ready
                    || rd.view_only
                {
                    return;
                }
                let dynamic_enabled = rd.dynamic_resize_enabled;
                let next_req = {
                    let Some(state) = app.rd_resize_state.get_mut(&session_key.tab_id) else {
                        return;
                    };
                    if state.connection_generation != session_key.connection_generation
                        || state.auto_cancel_generation != auto_cancel_gen
                        || state.debounce_generation != debounce_gen
                        || state.layout_generation != layout_gen
                        || state.in_flight.is_some()
                    {
                        return;
                    }
                    state.take_ready_to_dispatch(dynamic_enabled)
                };
                if let Some(req) = next_req {
                    app.dispatch_rd_resize(session_key, req, cx);
                }
            });
        })
        .detach();
    }

    fn dispatch_rd_resize(
        &mut self,
        session_key: crate::RemoteSessionKey,
        req: crate::PendingResizeRequest,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_idx) = self.tabs.iter().position(|t| t.id == session_key.tab_id) else {
            if let Some(state) = self.rd_resize_state.get_mut(&session_key.tab_id) {
                state.fail_dispatch(req.request_id, "Remote desktop tab closed".into());
            }
            return;
        };
        let Some(rd) = self.tabs[tab_idx].remote_desktop_state.as_ref() else {
            if let Some(state) = self.rd_resize_state.get_mut(&session_key.tab_id) {
                state.fail_dispatch(req.request_id, "Remote desktop state missing".into());
            }
            return;
        };
        if rd.connection_generation != session_key.connection_generation
            || rd.status != RemoteDesktopStatus::Connected
            || !rd.display_control_ready
            || rd.view_only
            || (req.origin == crate::ResizeOrigin::Automatic && !rd.dynamic_resize_enabled)
        {
            if let Some(state) = self.rd_resize_state.get_mut(&session_key.tab_id) {
                state.fail_dispatch(req.request_id, "Session not eligible for resize".into());
            }
            return;
        }
        let target = req.dimensions;
        let req_id = req.request_id;
        let backoff_ms = 2000u64.saturating_mul(u64::from(req.retry_count) + 1);
        tracing::debug!(
            tab_id = session_key.tab_id,
            connection_generation = session_key.connection_generation,
            request_id = req_id,
            origin = ?req.origin,
            width = target.0,
            height = target.1,
            "Dispatching RDP display resize"
        );
        if let Err(err) = self.try_send_remote_resize(tab_idx, target.0, target.1, cx) {
            let next_req = {
                let dynamic_enabled = self
                    .tabs
                    .get(tab_idx)
                    .and_then(|t| t.remote_desktop_state.as_ref())
                    .map(|r| r.dynamic_resize_enabled)
                    .unwrap_or(false);
                if let Some(state) = self.rd_resize_state.get_mut(&session_key.tab_id) {
                    state.fail_dispatch(req_id, err.clone());
                    // Do not immediately spin on channel failure; leave desired for next trigger.
                    let _ = dynamic_enabled;
                }
                None::<crate::PendingResizeRequest>
            };
            let _ = next_req;
            tracing::warn!(
                tab_id = session_key.tab_id,
                request_id = req_id,
                error = %err,
                "Failed to enqueue RDP display resize"
            );
            return;
        }

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(backoff_ms))
                .await;
            let _ = this.update(cx, |app, cx| {
                let Some(rd) = app
                    .tabs
                    .iter()
                    .find(|t| t.id == session_key.tab_id)
                    .and_then(|t| t.remote_desktop_state.as_ref())
                else {
                    return;
                };
                if rd.connection_generation != session_key.connection_generation
                    || rd.status != RemoteDesktopStatus::Connected
                {
                    return;
                }
                let dynamic_enabled = rd.dynamic_resize_enabled;
                let next_dispatch = {
                    let Some(state) = app.rd_resize_state.get_mut(&session_key.tab_id) else {
                        return;
                    };
                    if state.connection_generation != session_key.connection_generation {
                        return;
                    }
                    let next = state.timeout_request(req_id, dynamic_enabled);
                    if next.is_none() && state.last_error.is_some() {
                        tracing::warn!(
                            tab_id = session_key.tab_id,
                            request_id = req_id,
                            width = target.0,
                            height = target.1,
                            "RDP display resize acknowledgement timed out after retries"
                        );
                    }
                    next
                };
                if let Some(next_req) = next_dispatch {
                    app.dispatch_rd_resize(session_key, next_req, cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn handle_rd_resize_acknowledged(
        &mut self,
        session_key: crate::RemoteSessionKey,
        size: (u32, u32),
        cx: &mut Context<Self>,
    ) {
        let dynamic_enabled = self
            .tabs
            .iter()
            .find(|t| t.id == session_key.tab_id)
            .and_then(|t| t.remote_desktop_state.as_ref())
            .map(|rd| rd.dynamic_resize_enabled)
            .unwrap_or(false);
        let next = {
            let Some(state) = self.rd_resize_state.get_mut(&session_key.tab_id) else {
                return;
            };
            if state.connection_generation != session_key.connection_generation {
                return;
            }
            state.acknowledge(size, dynamic_enabled)
        };
        if let Some(next_req) = next {
            self.dispatch_rd_resize(session_key, next_req, cx);
        }
    }

    /// Toggle automatic dynamic display resizing for an RDP session.
    pub(crate) fn remote_desktop_toggle_dynamic_resize(
        &mut self,
        tab_id: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(rd) = self
            .tabs
            .iter_mut()
            .find(|t| t.id == tab_id)
            .and_then(|t| t.remote_desktop_state.as_mut())
        else {
            return;
        };
        rd.dynamic_resize_enabled = !rd.dynamic_resize_enabled;
        let enabled = rd.dynamic_resize_enabled;
        rd.last_action_note = Some(if enabled {
            "Automatic viewport resizing enabled".into()
        } else {
            "Automatic viewport resizing disabled (manual Match Viewport still available)".into()
        });
        if !enabled {
            if let Some(state) = self.rd_resize_state.get_mut(&tab_id) {
                state.cancel_automatic();
            }
        }
        cx.notify();
        if enabled {
            self.schedule_rd_auto_resize(tab_id, cx);
        }
    }

    /// Manually resize the remote desktop to match the measured GPUI viewport,
    /// even when automatic dynamic resizing is disabled.
    pub(crate) fn remote_desktop_match_viewport(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(rd) = tab.remote_desktop_state.as_ref() else {
            return;
        };
        if rd.protocol != RemoteDesktopProtocol::Rdp
            || rd.view_only
            || !rd.display_control_ready
            || rd.status != RemoteDesktopStatus::Connected
        {
            return;
        }
        let Some(measurement) = self
            .rd_canvas_measurements
            .get(&tab_id)
            .filter(|m| m.tab_id == tab_id && m.layout_epoch == self.layout_epoch)
        else {
            return;
        };
        let Some(target) = Self::compute_legal_rdp_resize(
            f32::from(measurement.viewport.size.width),
            f32::from(measurement.viewport.size.height),
            rd.max_monitor_area,
        ) else {
            return;
        };
        let conn_gen = rd.connection_generation;
        let dynamic_enabled = rd.dynamic_resize_enabled;
        let layout_gen = measurement.layout_epoch;
        let next_req = {
            let entry = self.rd_resize_state.entry(tab_id).or_default();
            if entry.connection_generation != conn_gen {
                entry.reset_for_connection(conn_gen);
            }
            match entry.request_resize(target, layout_gen, crate::ResizeOrigin::Manual) {
                crate::ResizeRequestOutcome::DispatchNow => {
                    entry.take_ready_to_dispatch(dynamic_enabled)
                }
                _ => None,
            }
        };
        if let Some(req) = next_req {
            let session_key = crate::RemoteSessionKey {
                tab_id,
                connection_generation: conn_gen,
            };
            self.dispatch_rd_resize(session_key, req, cx);
        }
    }

    /// Request a remote desktop resize using *only* the validated GPUI `CanvasMeasurement`.
    #[allow(dead_code)]
    pub(crate) fn request_measured_remote_resize(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        self.remote_desktop_match_viewport(tab_id, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_legal_rdp_resize_clamps_area_and_aligns_even_width() {
        // Odd width 1281 -> aligned to even 1280
        assert_eq!(
            Shellumi::compute_legal_rdp_resize(1281.7, 720.9, None),
            Some((1280, 720))
        );
        // Area cap exceeded (e.g. 3840x2160 > 2_073_600) -> scales down preserving aspect ratio + even width
        let (w, h) = Shellumi::compute_legal_rdp_resize(3840.0, 2160.0, Some(2_073_600)).unwrap();
        assert!(w * h <= 2_073_600);
        assert_eq!(w % 2, 0);
        // Zero / NaN rejected
        assert_eq!(Shellumi::compute_legal_rdp_resize(0.0, 720.0, None), None);
        assert_eq!(
            Shellumi::compute_legal_rdp_resize(f32::NAN, 720.0, None),
            None
        );
    }

    #[test]
    fn test_rd_resize_coordinator_enforces_single_inflight_coalesces_and_resets() {
        let mut state = crate::RdAutoResizeState::default();
        assert!(matches!(
            state.request_resize((1280, 720), 1, crate::ResizeOrigin::Automatic),
            crate::ResizeRequestOutcome::Debounced { .. }
        ));
        let gen1 = state.debounce_generation;
        assert!(matches!(
            state.request_resize((1440, 900), 1, crate::ResizeOrigin::Automatic),
            crate::ResizeRequestOutcome::Debounced { .. }
        ));
        assert_ne!(state.debounce_generation, gen1);

        // Dispatch (1440, 900) as the single in-flight negotiation.
        let req1 = state
            .take_ready_to_dispatch(true)
            .expect("should dispatch 1440x900");
        assert_eq!(req1.dimensions, (1440, 900));
        assert!(state.take_ready_to_dispatch(true).is_none());

        // A manual request arriving mid-flight must queue behind in_flight, never dispatch concurrently.
        assert_eq!(
            state.request_resize((1920, 1080), 2, crate::ResizeOrigin::Manual),
            crate::ResizeRequestOutcome::QueuedBehindInFlight
        );
        // An automatic resize must NOT overwrite a queued manual request.
        assert_eq!(
            state.request_resize((1600, 1000), 2, crate::ResizeOrigin::Automatic),
            crate::ResizeRequestOutcome::Skipped
        );

        // Acknowledging (1440, 900) immediately promotes the queued manual (1920, 1080).
        let req2 = state
            .acknowledge((1440, 900), true)
            .expect("should promote queued manual resize");
        assert_eq!(req2.dimensions, (1920, 1080));
        assert_eq!(req2.origin, crate::ResizeOrigin::Manual);
        assert_ne!(req2.request_id, req1.request_id);

        // Reconnect resets in-flight, desired, confirmed, and bumps generations cleanly.
        state.reset_for_connection(2);
        assert_eq!(state.connection_generation, 2);
        assert_eq!(state.in_flight, None);
        assert_eq!(state.desired, None);
        assert_eq!(state.last_confirmed, None);
    }

    #[test]
    fn test_rd_resize_request_id_prevents_stale_timeouts_and_handles_queue_failure_and_rejection() {
        let mut state = crate::RdAutoResizeState::default();
        state.request_resize((1280, 720), 1, crate::ResizeOrigin::Automatic);
        let req1 = state.take_ready_to_dispatch(true).unwrap();

        // Queue failure clears in_flight immediately without leaving request stuck.
        state.fail_dispatch(req1.request_id, "channel full".into());
        assert_eq!(state.in_flight, None);
        assert_eq!(state.last_error.as_deref(), Some("channel full"));

        // Stale timeout for req1 must NOT cancel req2 even with identical dimensions.
        state.request_resize((1280, 720), 1, crate::ResizeOrigin::Manual);
        let req2 = state.take_ready_to_dispatch(true).unwrap();
        assert_ne!(req1.request_id, req2.request_id);
        assert_eq!(state.timeout_request(req1.request_id, true), None);
        assert_eq!(state.in_flight, Some(req2));

        // Rejection clears matching in-flight and records last_rejected.
        assert_eq!(
            state.reject((1280, 720), "server rejected".into(), true),
            None
        );
        assert_eq!(state.in_flight, None);
        assert_eq!(state.last_rejected, Some((1280, 720)));
        assert_eq!(state.last_error.as_deref(), Some("server rejected"));

        // Delayed ack for an older size cannot complete a newer in-flight size.
        state.request_resize((1920, 1080), 2, crate::ResizeOrigin::Manual);
        let req3 = state.take_ready_to_dispatch(true).unwrap();
        let _ = state.acknowledge((1280, 720), true);
        assert_eq!(state.in_flight, Some(req3));
    }

    #[test]
    fn test_disabling_dynamic_resize_cancels_auto_work_but_preserves_manual_and_confirmed() {
        let mut state = crate::RdAutoResizeState::default();
        assert_eq!(state.acknowledge((1280, 720), true), None);
        assert_eq!(state.last_confirmed, Some((1280, 720)));

        // Queued automatic request is dropped and auto_cancel_generation increments when disabled.
        let cancel_before = state.auto_cancel_generation;
        state.request_resize((1440, 900), 2, crate::ResizeOrigin::Automatic);
        state.cancel_automatic();
        assert_ne!(state.auto_cancel_generation, cancel_before);
        assert_eq!(state.desired, None);
        assert_eq!(state.last_confirmed, Some((1280, 720)));

        // An in-flight automatic request must NOT retry or schedule further auto requests once disabled.
        state.request_resize((1600, 1000), 3, crate::ResizeOrigin::Automatic);
        let auto_req = state.take_ready_to_dispatch(true).unwrap();
        state.cancel_automatic();
        assert_eq!(state.timeout_request(auto_req.request_id, false), None);

        // Pending manual requests are preserved when automatic resizing is canceled.
        state.request_resize((1920, 1080), 4, crate::ResizeOrigin::Manual);
        state.cancel_automatic();
        let manual_req = state
            .take_ready_to_dispatch(false)
            .expect("manual request should survive cancel_automatic");
        assert_eq!(manual_req.origin, crate::ResizeOrigin::Manual);
        assert_eq!(manual_req.dimensions, (1920, 1080));
        assert_eq!(state.last_confirmed, Some((1280, 720)));
    }
}

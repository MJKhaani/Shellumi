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
            search_case_sensitive: false,
            search_regex: false,
            search_error: None,
            session_uuid: uuid::Uuid::new_v4().to_string(),
            session_generation: 1,
            vault_id: self.active_vault_id.clone(),
            host_id: Some(host.id),
            recording_id: None,
            recording_started_at: None,
            recording_state: shellumi_core::RecordingLifecycleState::Idle,
            focus_handle: cx.focus_handle(),
            bell_flash_until: None,
            has_unread_bell: false,
            selection: None,
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
        self.ensure_rd_focus_handle(tab_id, window, cx);
        self.rd_input_controllers.insert(
            tab_id,
            shellumi_remote_desktop::RemoteInputController::new(
                tab_id,
                false,
                shellumi_remote_desktop::InputCapabilities::default(),
            ),
        );
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
            search_case_sensitive: false,
            search_regex: false,
            search_error: None,
            session_uuid: uuid::Uuid::new_v4().to_string(),
            session_generation: 1,
            vault_id: self.active_vault_id.clone(),
            host_id: None,
            recording_id: None,
            recording_started_at: None,
            recording_state: shellumi_core::RecordingLifecycleState::Idle,
            focus_handle: cx.focus_handle(),
            bell_flash_until: None,
            has_unread_bell: false,
            selection: None,
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
            enable_microphone: false,
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
        self.ensure_rd_focus_handle(tab_id, window, cx);
        self.rd_input_controllers.insert(
            tab_id,
            shellumi_remote_desktop::RemoteInputController::new(
                tab_id,
                false,
                shellumi_remote_desktop::InputCapabilities::default(),
            ),
        );
        window.focus(&self.focus);
        cx.notify();
    }

    pub(crate) fn remote_desktop_send_shortcut(
        &mut self,
        tab_idx: usize,
        shortcut: shellumi_remote_desktop::RemoteShortcut,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.get_mut(tab_idx) else {
            return;
        };
        let tab_id = tab.id;
        let Some(rd) = &mut tab.remote_desktop_state else {
            return;
        };
        if rd.view_only {
            rd.last_action_note = Some("View-only mode: shortcut blocked".into());
            cx.notify();
            return;
        }
        if rd.ui_handle.is_none() {
            rd.last_action_note = Some(format!(
                "Shortcut {} ignored (session not connected)",
                shortcut.label()
            ));
            cx.notify();
            return;
        }
        let proto = rd.protocol;
        let is_connected = rd.status == RemoteDesktopStatus::Connected;

        let inputs = match self.rd_input_controllers.get_mut(&tab_id) {
            Some(ctrl) => match ctrl.on_shortcut(shortcut) {
                Ok(inputs) => inputs,
                Err(reason) => {
                    if let Some(tab) = self.tabs.get_mut(tab_idx) {
                        if let Some(rd) = &mut tab.remote_desktop_state {
                            rd.last_action_note = Some(format!(
                                "Shortcut {} rejected ({reason:?})",
                                shortcut.label()
                            ));
                        }
                    }
                    cx.notify();
                    return;
                }
            },
            None => return,
        };

        self.dispatch_rd_inputs(tab_id, inputs);
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                if !self
                    .rd_input_controllers
                    .get(&tab_id)
                    .is_some_and(|c| c.recovery_frozen)
                {
                    rd.last_action_note = Some(format!("Queued shortcut: {}", shortcut.label()));
                    if is_connected {
                        rd.status_message = Some(format!(
                            "Shortcut {} queued for {}",
                            shortcut.label(),
                            match proto {
                                RemoteDesktopProtocol::Vnc => "RFB",
                                RemoteDesktopProtocol::Rdp => "RDP",
                            }
                        ));
                    }
                }
            }
        }
        cx.notify();
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
            let tab_id = tab.id;
            if let Some(rd) = &mut tab.remote_desktop_state {
                let next_enabled = !rd.view_only;
                self.rd_view_only_transition_seq = self.rd_view_only_transition_seq.wrapping_add(1);
                let transition_id = self.rd_view_only_transition_seq;

                let (identity, envelopes): (shellumi_remote_desktop::SessionIdentity, Vec<_>) =
                    if let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) {
                        let id = ctrl.identity();
                        let rels = ctrl.begin_view_only_transition(next_enabled, transition_id);
                        (
                            id,
                            rels.into_iter().map(|r| ctrl.wrap_envelope(r)).collect(),
                        )
                    } else {
                        (
                            shellumi_remote_desktop::SessionIdentity::default(),
                            Vec::new(),
                        )
                    };

                if let Some(ui_handle) = &rd.ui_handle {
                    if !identity.is_transport_bound() {
                        rd.last_action_note =
                            Some("View-only transition ignored: transport not active".into());
                        cx.notify();
                        return;
                    }
                    for env in envelopes {
                        if let Err(err) = ui_handle.try_send_command(
                            shellumi_remote_desktop::SessionCommand::InputEnvelope(env),
                        ) {
                            if let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) {
                                match ui_handle.request_recovery(
                                    ctrl.identity(),
                                    "View-only release admission failed",
                                ) {
                                    Ok(ticket) if ctrl.install_recovery(&ticket) => {}
                                    Ok(_) => {
                                        ctrl.block_recovery(
                                            "Recovery ticket could not be installed",
                                        );
                                        tracing::error!(
                                            "View-only recovery ticket could not be installed"
                                        );
                                    }
                                    Err(error) => {
                                        ctrl.block_recovery(&error);
                                        tracing::warn!(%error, "View-only recovery rejected");
                                    }
                                }
                            }
                            rd.last_action_note =
                                Some(format!("View-only release recovery triggered: {err}"));
                            break;
                        }
                    }
                    if let Err(err) = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SetViewOnly {
                            identity,
                            enabled: next_enabled,
                            transition_id,
                        },
                    ) {
                        rd.last_action_note =
                            Some(format!("Failed to update view-only policy: {err}"));
                        cx.notify();
                        return;
                    }
                }

                if next_enabled {
                    rd.view_only = true;
                    rd.last_action_note = Some("Enabling view-only mode…".into());
                } else {
                    rd.last_action_note = Some("Requesting interactive control…".into());
                }
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
                if let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) {
                    if let Some(ui_handle) = &rd.ui_handle {
                        let rels = ctrl.release_all();
                        for rel in rels {
                            let env = ctrl.wrap_envelope(rel);
                            let _ = ui_handle.try_send_command(
                                shellumi_remote_desktop::SessionCommand::InputEnvelope(env),
                            );
                        }
                    }
                    ctrl.connected = false;
                }
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
                self.invalidate_layout();
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
                self.layout_epoch = self.layout_epoch.wrapping_add(1);
                self.rd_canvas_measurements.clear();
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
                    shellumi_remote_desktop::create_session_channels(512, 256);
                rd.ui_handle = Some(ui_handle.clone());
                let ctrl = self.rd_input_controllers.entry(tab_id).or_insert_with(|| {
                    shellumi_remote_desktop::RemoteInputController::new(
                        tab_id,
                        rd.view_only,
                        shellumi_remote_desktop::InputCapabilities::default(),
                    )
                });
                ctrl.reset_for_transport(rd.connection_generation, rd.connection_generation);
                ctrl.view_only = rd.view_only;
                ctrl.connected = false;
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
                            let is_active_tab = app
                                .active_tab_idx
                                .and_then(|i| app.tabs.get(i))
                                .is_some_and(|t| t.id == session_key.tab_id);
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
                                        if let Some(ctrl) = app.rd_input_controllers.get_mut(&session_key.tab_id) {
                                            ctrl.connected = true;
                                        }
                                        trigger_auto_resize = true;
                                    }
                                    shellumi_remote_desktop::SessionState::Reconnecting {
                                        attempt,
                                        max_attempts,
                                        delay_ms,
                                    } => {
                                        if let Some(ctrl) = app.rd_input_controllers.get_mut(&session_key.tab_id) {
                                            ctrl.release_all();
                                            ctrl.connected = false;
                                        }
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
                                        if let Some(broker) = app.rd_clipboard_brokers.get_mut(&session_key.tab_id) {
                                            broker.reset();
                                        }
                                        if let Some(ctrl) = app.rd_input_controllers.get_mut(&session_key.tab_id) {
                                            ctrl.release_all();
                                            ctrl.connected = false;
                                        }
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
                                        if let Some(ctrl) = app.rd_input_controllers.get_mut(&session_key.tab_id) {
                                            ctrl.release_all();
                                            ctrl.connected = false;
                                        }
                                        rd.status = RemoteDesktopStatus::Error;
                                        rd.error = Some(err.clone());
                                        rd.status_message = Some(format!("Error: {err}"));
                                    }
                                    _ => {}
                                },
                                shellumi_remote_desktop::SessionEvent::ViewOnlyChanged {
                                    identity,
                                    enabled,
                                    transition_id,
                                } => {
                                    let accepted = app
                                        .rd_input_controllers
                                        .get_mut(&session_key.tab_id)
                                        .is_some_and(|ctrl| {
                                            identity.matches_transport(&ctrl.identity())
                                                && ctrl.ack_view_only_transition(enabled, transition_id)
                                        });
                                    if accepted {
                                        rd.view_only = enabled;
                                        rd.last_action_note = Some(if enabled {
                                            "View-only mode active".into()
                                        } else {
                                            "Interactive control enabled".into()
                                        });
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::TransportEpochChanged {
                                    connection_generation: _,
                                    transport_epoch,
                                } => {
                                    if let Some(ctrl) = app.rd_input_controllers.get_mut(&session_key.tab_id) {
                                        let duplicate = ctrl.identity().transport_epoch == transport_epoch
                                            && ctrl.identity().connection_generation == rd.connection_generation
                                            && ctrl.identity().is_transport_bound();
                                        if !duplicate {
                                            ctrl.reset_for_transport(rd.connection_generation, transport_epoch);
                                            ctrl.view_only = rd.view_only;
                                            if let Some(ui_handle) = &rd.ui_handle {
                                                ctrl.input_epoch = ui_handle.set_active_identity(ctrl.identity());
                                            }
                                        }
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::InputRecovery {
                                    transport_epoch: _,
                                    released_count,
                                    reason,
                                } => {
                                    if released_count > 0 {
                                        rd.last_action_note = Some(format!(
                                            "Released {released_count} held input(s) ({reason})"
                                        ));
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::RecoveryCompleted {
                                    identity,
                                    input_epoch,
                                    barrier_id,
                                    released_count,
                                    reason,
                                } => {
                                    if let Some(ctrl) = app.rd_input_controllers.get_mut(&session_key.tab_id) {
                                        if ctrl.ack_recovery(&identity, input_epoch, barrier_id, true, &reason) {
                                            rd.last_action_note = Some(format!(
                                                "Input recovery completed ({released_count} released: {reason})"
                                            ));
                                        }
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::RecoveryFailed {
                                    identity,
                                    input_epoch,
                                    barrier_id,
                                    reason,
                                } => {
                                    if let Some(ctrl) = app.rd_input_controllers.get_mut(&session_key.tab_id) {
                                        if ctrl.ack_recovery(&identity, input_epoch, barrier_id, false, &reason) {
                                            rd.view_only = true;
                                            rd.last_action_note = Some(format!(
                                                "Input recovery failed; session locked fail-closed: {reason}"
                                            ));
                                        }
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::InputError {
                                    transport_epoch: _,
                                    seq: _,
                                    reason,
                                } => {
                                    if let Some(ctrl) = app.rd_input_controllers.get_mut(&session_key.tab_id) {
                                        ctrl.counters.transport_failed += 1;
                                        ctrl.counters.last_error_category = Some(reason.clone());
                                    }
                                    rd.last_action_note = Some(format!("Input failed: {reason}"));
                                }
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
                                    if is_active_tab && !rd.view_only {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
                                        let pb_change = crate::native_clipboard::pasteboard_change_count();
                                        app.rd_clipboard_brokers
                                            .entry(session_key.tab_id)
                                            .or_default()
                                            .record_remote_write(pb_change, &text);
                                        rd.last_action_note = Some(
                                            "Remote clipboard text received and copied to local clipboard"
                                                .into(),
                                        );
                                    } else {
                                        rd.last_action_note = Some(
                                            "Remote clipboard received (held in session; inactive or view-only)"
                                                .into(),
                                        );
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::TransferStarted {
                                    transfer_id,
                                    direction,
                                    file_count,
                                    total_bytes,
                                } => {
                                    rd.last_action_note = Some(format!(
                                        "{direction:?} started: {file_count} file(s), {total_bytes} bytes"
                                    ));
                                    let jobs = app.rd_transfers.entry(session_key.tab_id).or_default();
                                    jobs.push(crate::RdTransferJob {
                                        transfer_id,
                                        direction,
                                        phase: crate::RdTransferPhase::Transferring,
                                        file_count,
                                        files: Vec::new(),
                                        target_dir: None,
                                        bytes_transferred: 0,
                                        total_bytes,
                                        error: None,
                                    });
                                    if jobs.len() > 10 {
                                        jobs.remove(0);
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::TransferProgress {
                                    transfer_id,
                                    file_index: _,
                                    file_name,
                                    bytes_transferred,
                                    file_total_bytes,
                                } => {
                                    rd.last_action_note = Some(format!(
                                        "Transferring {file_name}: {bytes_transferred}/{file_total_bytes} bytes"
                                    ));
                                    if let Some(jobs) = app.rd_transfers.get_mut(&session_key.tab_id) {
                                        if let Some(job) = jobs.iter_mut().find(|j| j.transfer_id == transfer_id) {
                                            job.bytes_transferred = bytes_transferred;
                                        }
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::TransferCompleted {
                                    transfer_id,
                                    direction,
                                    files,
                                    target_dir,
                                } => {
                                    if let Some(jobs) = app.rd_transfers.get_mut(&session_key.tab_id) {
                                        if let Some(job) = jobs.iter_mut().find(|j| j.transfer_id == transfer_id) {
                                            job.phase = crate::RdTransferPhase::Completed;
                                            job.files = files.clone();
                                            job.target_dir = target_dir.clone();
                                            job.bytes_transferred = job.total_bytes;
                                        }
                                    }
                                    if direction == shellumi_remote_desktop::TransferDirection::Download
                                        && is_active_tab
                                        && !rd.view_only
                                    {
                                        let mut clip_note = format!(
                                            "{direction:?} completed: {} file(s)",
                                            files.len()
                                        );
                                        if let Some(ref dir) = target_dir {
                                            let full_paths: Vec<std::path::PathBuf> = files
                                                .iter()
                                                .map(|f| std::path::Path::new(dir).join(f))
                                                .collect();
                                            if let Err(err) = crate::native_clipboard::write_clipboard_files(&full_paths) {
                                                clip_note = format!(
                                                    "{direction:?} completed ({} file(s)); native clipboard publish skipped: {err}",
                                                    files.len()
                                                );
                                            }
                                        }
                                        rd.last_action_note = Some(clip_note);
                                    } else {
                                        rd.last_action_note = Some(format!(
                                            "{direction:?} completed: {} file(s)",
                                            files.len()
                                        ));
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::TransferFailed {
                                    transfer_id,
                                    direction,
                                    reason,
                                } => {
                                    if let Some(jobs) = app.rd_transfers.get_mut(&session_key.tab_id) {
                                        if let Some(job) = jobs.iter_mut().find(|j| j.transfer_id == transfer_id) {
                                            job.phase = crate::RdTransferPhase::Failed;
                                            job.error = Some(reason.clone());
                                        }
                                    }
                                    rd.last_action_note = Some(format!(
                                        "{direction:?} failed: {reason}"
                                    ));
                                }
                                shellumi_remote_desktop::SessionEvent::TransferCancelled {
                                    transfer_id,
                                    direction,
                                } => {
                                    if let Some(jobs) = app.rd_transfers.get_mut(&session_key.tab_id) {
                                        if let Some(job) = jobs.iter_mut().find(|j| j.transfer_id == transfer_id) {
                                            job.phase = crate::RdTransferPhase::Cancelled;
                                        }
                                    }
                                    rd.last_action_note = Some(format!("{direction:?} cancelled"));
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
                                shellumi_remote_desktop::SessionEvent::ResizeFailed {
                                    width,
                                    height,
                                    reason,
                                } => {
                                    rd.last_action_note = Some(format!(
                                        "Display resize to {width}×{height} failed: {reason}"
                                    ));
                                    let dyn_en = rd.dynamic_resize_enabled;
                                    let epoch = app.layout_epoch;
                                    if let Some(state) =
                                        app.rd_resize_state.get_mut(&session_key.tab_id)
                                    {
                                        if state.connection_generation
                                            == session_key.connection_generation
                                        {
                                            if let Some(next_req) = state.handle_resize_failed(
                                                (width, height),
                                                reason,
                                                epoch,
                                                dyn_en,
                                            ) {
                                                let _ = next_req;
                                            }
                                        }
                                    }
                                }
                                shellumi_remote_desktop::SessionEvent::DisplayControlUnavailable {
                                    reason,
                                } => {
                                    rd.display_control_ready = false;
                                    rd.last_action_note = Some(reason);
                                    if let Some(state) = app.rd_resize_state.get_mut(&session_key.tab_id) {
                                        if state.connection_generation == session_key.connection_generation {
                                            state.cancel_automatic();
                                            state.in_flight = None;
                                        }
                                    }
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
                if rd.view_only {
                    rd.last_action_note = Some("View-only mode: clipboard sync blocked".into());
                    cx.notify();
                    return;
                }
                if rd.status != RemoteDesktopStatus::Connected {
                    rd.last_action_note = Some(
                        "Clipboard synchronization requires an active connected session".into(),
                    );
                    cx.notify();
                    return;
                }

                // Prefer native file references if present on the OS clipboard (RDP only)
                let tab_id = tab.id;
                let identity = self
                    .rd_input_controllers
                    .get(&tab_id)
                    .map(|c| c.identity())
                    .unwrap_or_default();
                if rd.protocol == RemoteDesktopProtocol::Rdp {
                    let native_files = crate::native_clipboard::read_clipboard_files();
                    if !native_files.is_empty() {
                        let count = native_files.len();
                        if let Some(ui_handle) = &rd.ui_handle {
                            match ui_handle.try_send_command(
                                shellumi_remote_desktop::SessionCommand::SendFiles {
                                    identity,
                                    paths: native_files,
                                },
                            ) {
                                Ok(()) => {
                                    rd.last_action_note = Some(format!(
                                        "Staged {count} local clipboard file(s) for remote paste"
                                    ));
                                }
                                Err(e) => {
                                    rd.last_action_note =
                                        Some(format!("Failed to stage clipboard files: {e}"));
                                }
                            }
                        }
                        cx.notify();
                        return;
                    }
                }

                // Never fall back to cached remote text when local clipboard has no text
                let local_text = cx.read_from_clipboard().and_then(|item| item.text());
                match local_text {
                    Some(text) => {
                        let pb_change = crate::native_clipboard::pasteboard_change_count();
                        let should_send = self
                            .rd_clipboard_brokers
                            .entry(tab_id)
                            .or_default()
                            .should_send_local_text(pb_change, &text);
                        if !should_send {
                            if let Some(tab) = self.tabs.get_mut(tab_idx) {
                                if let Some(rd) = &mut tab.remote_desktop_state {
                                    rd.last_action_note = Some(
                                        "Clipboard unchanged since remote write (echo suppressed)"
                                            .into(),
                                    );
                                }
                            }
                            cx.notify();
                            return;
                        }
                        if let Some(tab) = self.tabs.get_mut(tab_idx) {
                            if let Some(rd) = &mut tab.remote_desktop_state {
                                if let Some(ui_handle) = &rd.ui_handle {
                                    match ui_handle.try_send_command(
                                        shellumi_remote_desktop::SessionCommand::SendClipboard {
                                            identity,
                                            text,
                                        },
                                    ) {
                                        Ok(()) => {
                                            rd.last_action_note = Some(
                                                "Local clipboard text queued for remote session"
                                                    .into(),
                                            );
                                        }
                                        Err(e) => {
                                            rd.last_action_note = Some(format!(
                                                "Failed to queue clipboard text: {e}"
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    None => {
                        rd.last_action_note =
                            Some("Local clipboard contains no text or files".into());
                    }
                }
                cx.notify();
            }
        }
    }

    /// Ordered paste transaction: sync local clipboard offer (if session/policy permits)
    /// into the worker command FIFO *before* atomically queuing `Ctrl+V`.
    pub(crate) fn remote_desktop_paste_transaction(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(tab_idx) = self.tabs.iter().position(|t| t.id == tab_id) else {
            return;
        };
        let _pb_change = crate::native_clipboard::pasteboard_change_count();
        self.remote_desktop_sync_clipboard(tab_idx, cx);
        self.remote_desktop_send_shortcut(
            tab_idx,
            shellumi_remote_desktop::RemoteShortcut::CtrlV,
            cx,
        );
    }

    pub(crate) fn remote_desktop_cancel_transfer(
        &mut self,
        tab_idx: usize,
        transfer_id: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self.tabs.get(tab_idx).map(|t| t.id) else {
            return;
        };
        let identity = self
            .rd_input_controllers
            .get(&tab_id)
            .map(|c| c.identity())
            .unwrap_or_default();
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                if let Some(ui_handle) = &rd.ui_handle {
                    match ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::CancelTransfer {
                            identity,
                            transfer_id,
                        },
                    ) {
                        Ok(()) => {
                            rd.last_action_note = Some(format!(
                                "Cancellation requested for transfer #{transfer_id}"
                            ));
                        }
                        Err(e) => {
                            rd.last_action_note =
                                Some(format!("Failed to cancel transfer #{transfer_id}: {e}"));
                        }
                    }
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn remote_desktop_copy_transfer_to_clipboard(
        &mut self,
        tab_idx: usize,
        transfer_id: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.get(tab_idx) else {
            return;
        };
        let tab_id = tab.id;
        let paths = self
            .rd_transfers
            .get(&tab_id)
            .and_then(|jobs| jobs.iter().find(|j| j.transfer_id == transfer_id))
            .and_then(|job| {
                job.target_dir.as_ref().map(|dir| {
                    job.files
                        .iter()
                        .map(|f| std::path::Path::new(dir).join(f))
                        .collect::<Vec<_>>()
                })
            });
        let Some(paths) = paths else {
            return;
        };
        let res = crate::native_clipboard::write_clipboard_files(&paths);
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.last_action_note = Some(match res {
                    Ok(()) => format!("Copied {} downloaded file(s) to OS clipboard", paths.len()),
                    Err(e) => format!("Failed to copy files to OS clipboard: {e}"),
                });
            }
        }
        cx.notify();
    }

    pub(crate) fn remote_desktop_send_files(
        &mut self,
        tab_idx: usize,
        paths: Vec<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_id) = self.tabs.get(tab_idx).map(|t| t.id) else {
            return;
        };
        let identity = self
            .rd_input_controllers
            .get(&tab_id)
            .map(|c| c.identity())
            .unwrap_or_default();
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                if let Some(ui_handle) = &rd.ui_handle {
                    let count = paths.len();
                    if let Err(e) = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SendFiles { identity, paths },
                    ) {
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
        let captured_identity = self
            .rd_input_controllers
            .get(&tab_id)
            .map(|c| c.identity())
            .unwrap_or_default();
        if !captured_identity.is_transport_bound() {
            return;
        }
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
                        let current_identity = app
                            .rd_input_controllers
                            .get(&tab_id)
                            .map(|c| c.identity())
                            .unwrap_or_default();
                        if current_identity != captured_identity {
                            return;
                        }
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
        let Some(tab_id) = self.tabs.get(tab_idx).map(|t| t.id) else {
            return;
        };
        let identity = self
            .rd_input_controllers
            .get(&tab_id)
            .map(|c| c.identity())
            .unwrap_or_default();
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let new_muted = !rd.audio_muted;
                rd.audio_muted = new_muted;
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SetAudioMute {
                            identity,
                            muted: new_muted,
                        },
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
        let Some(tab_id) = self.tabs.get(tab_idx).map(|t| t.id) else {
            return;
        };
        let identity = self
            .rd_input_controllers
            .get(&tab_id)
            .map(|c| c.identity())
            .unwrap_or_default();
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let vol = volume.min(100);
                rd.audio_volume = vol;
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SetAudioVolume {
                            identity,
                            volume: vol,
                        },
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
        let Some(tab_id) = self.tabs.get(tab_idx).map(|t| t.id) else {
            return;
        };
        let identity = self
            .rd_input_controllers
            .get(&tab_id)
            .map(|c| c.identity())
            .unwrap_or_default();
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let new_muted = !rd.mic_muted;
                rd.mic_muted = new_muted;
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SetMicrophoneMute {
                            identity,
                            muted: new_muted,
                        },
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
        let Some(tab_id) = self.tabs.get(tab_idx).map(|t| t.id) else {
            return;
        };
        let identity = self
            .rd_input_controllers
            .get(&tab_id)
            .map(|c| c.identity())
            .unwrap_or_default();
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                let vol = volume.min(200);
                rd.mic_volume = vol;
                if let Some(ui_handle) = &rd.ui_handle {
                    let _ = ui_handle.try_send_command(
                        shellumi_remote_desktop::SessionCommand::SetMicrophoneVolume {
                            identity,
                            volume: vol,
                        },
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
                    entry.take_ready_to_dispatch(layout_gen, dynamic_enabled)
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
        let Some(tab_id) = self.tabs.get(tab_idx).map(|t| t.id) else {
            return Err("Remote desktop tab no longer exists".into());
        };
        let identity = self
            .rd_input_controllers
            .get(&tab_id)
            .map(|c| c.identity())
            .unwrap_or_default();
        let Some(tab) = self.tabs.get_mut(tab_idx) else {
            return Err("Remote desktop tab no longer exists".into());
        };
        let Some(rd) = &mut tab.remote_desktop_state else {
            return Err("Remote desktop state missing".into());
        };
        if let Some(ui_handle) = &rd.ui_handle {
            ui_handle
                .try_send_command(shellumi_remote_desktop::SessionCommand::RequestResize {
                    identity,
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
        let Some(tab_id) = self.tabs.get(tab_idx).map(|t| t.id) else {
            return;
        };
        let identity = self
            .rd_input_controllers
            .get(&tab_id)
            .map(|c| c.identity())
            .unwrap_or_default();
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
                        shellumi_remote_desktop::SessionCommand::ConfigureDrivePolicy {
                            identity,
                            path: Some(folder_path),
                            read_only: true,
                        },
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

    /// Advance `layout_epoch`, clear stale remote-desktop canvas measurements,
    /// and invalidate geometry-dependent pending resize state whenever container
    /// geometry or workspace visibility changes.
    pub(crate) fn invalidate_layout(&mut self) {
        Self::invalidate_layout_state_all(
            &mut self.layout_epoch,
            &mut self.rd_canvas_measurements,
            &mut self.rd_resize_state,
        );
    }

    pub(crate) fn invalidate_layout_state_all(
        layout_epoch: &mut u64,
        measurements: &mut std::collections::HashMap<u64, crate::CanvasMeasurement>,
        resize_states: &mut std::collections::HashMap<u64, crate::RdAutoResizeState>,
    ) {
        *layout_epoch = layout_epoch.wrapping_add(1);
        measurements.clear();
        for state in resize_states.values_mut() {
            state.invalidate_layout(*layout_epoch);
        }
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
                let current_epoch = app.layout_epoch;
                if current_epoch != layout_gen
                    || !app
                        .rd_canvas_measurements
                        .get(&session_key.tab_id)
                        .is_some_and(|m| m.layout_epoch == current_epoch)
                {
                    return;
                }
                let next_req = {
                    let Some(state) = app.rd_resize_state.get_mut(&session_key.tab_id) else {
                        return;
                    };
                    if state.connection_generation != session_key.connection_generation
                        || state.auto_cancel_generation != auto_cancel_gen
                        || state.debounce_generation != debounce_gen
                        || state.layout_generation != current_epoch
                        || state.in_flight.is_some()
                    {
                        return;
                    }
                    state.take_ready_to_dispatch(current_epoch, dynamic_enabled)
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
        let is_visible = self.page.is_none() && self.active_tab_idx == Some(tab_idx);
        let has_fresh_measurement = self
            .rd_canvas_measurements
            .get(&session_key.tab_id)
            .is_some_and(|m| m.layout_epoch == self.layout_epoch);
        let Some(rd) = self.tabs[tab_idx].remote_desktop_state.as_ref() else {
            if let Some(state) = self.rd_resize_state.get_mut(&session_key.tab_id) {
                state.fail_dispatch(req.request_id, "Remote desktop state missing".into());
            }
            return;
        };
        let cancel_gen_valid = req.origin != crate::ResizeOrigin::Automatic
            || self
                .rd_resize_state
                .get(&session_key.tab_id)
                .is_some_and(|s| s.auto_cancel_generation == req.auto_cancel_generation);
        if rd.connection_generation != session_key.connection_generation
            || rd.status != RemoteDesktopStatus::Connected
            || !rd.display_control_ready
            || rd.view_only
            || req.layout_generation != self.layout_epoch
            || !has_fresh_measurement
            || !is_visible
            || !cancel_gen_valid
            || (req.origin == crate::ResizeOrigin::Automatic && !rd.dynamic_resize_enabled)
        {
            if let Some(state) = self.rd_resize_state.get_mut(&session_key.tab_id) {
                state.fail_dispatch(
                    req.request_id,
                    "Session or layout no longer eligible for resize".into(),
                );
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
                let current_epoch = app.layout_epoch;
                let next_dispatch = {
                    let Some(state) = app.rd_resize_state.get_mut(&session_key.tab_id) else {
                        return;
                    };
                    if state.connection_generation != session_key.connection_generation {
                        return;
                    }
                    let next = state.timeout_request(req_id, current_epoch, dynamic_enabled);
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
        let current_epoch = self.layout_epoch;
        let next = {
            let Some(state) = self.rd_resize_state.get_mut(&session_key.tab_id) else {
                return;
            };
            if state.connection_generation != session_key.connection_generation {
                return;
            }
            state.acknowledge(size, current_epoch, dynamic_enabled)
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
                    entry.take_ready_to_dispatch(layout_gen, dynamic_enabled)
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

    pub(crate) fn ensure_rd_focus_handle(
        &mut self,
        tab_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> FocusHandle {
        if let Some(fh) = self.rd_focus_handles.get(&tab_id) {
            return fh.clone();
        }
        let fh = cx.focus_handle();
        cx.on_focus_out(&fh, window, move |app, _event, _window, cx| {
            app.remote_desktop_release_all_for_tab(tab_id);
            cx.notify();
        })
        .detach();
        cx.observe_window_activation(window, move |app, window, cx| {
            if !window.is_window_active() {
                app.remote_desktop_release_all_for_tab(tab_id);
                cx.notify();
            }
        })
        .detach();
        self.rd_focus_handles.insert(tab_id, fh.clone());
        fh
    }

    pub(crate) fn rd_canvas_transform_for_tab(
        &self,
        tab_id: u64,
    ) -> Option<shellumi_remote_desktop::CanvasTransform> {
        let tab = self.tabs.iter().find(|t| t.id == tab_id)?;
        let rd = tab.remote_desktop_state.as_ref()?;
        let m = self
            .rd_canvas_measurements
            .get(&tab_id)
            .filter(|m| m.tab_id == tab_id && m.layout_epoch == self.layout_epoch)?;
        let img = rd.render_image.as_ref()?;
        let sz = img.size(0);
        let fw = (sz.width.0.max(0) as u32).max(1);
        let fh = (sz.height.0.max(0) as u32).max(1);
        let mode = match rd.scale_mode {
            RemoteDesktopScaleMode::Fit => shellumi_remote_desktop::ScaleMode::Fit,
            RemoteDesktopScaleMode::Original => shellumi_remote_desktop::ScaleMode::Original,
            RemoteDesktopScaleMode::Fill => shellumi_remote_desktop::ScaleMode::Fill,
        };
        let pan = self.rd_pan_state.get(&tab_id).copied().unwrap_or_default();
        shellumi_remote_desktop::CanvasTransform::new(
            (
                f64::from(f32::from(m.viewport.origin.x)),
                f64::from(f32::from(m.viewport.origin.y)),
            ),
            (
                f64::from(f32::from(m.viewport.size.width)),
                f64::from(f32::from(m.viewport.size.height)),
            ),
            (fw, fh),
            mode,
            (pan.x, pan.y),
        )
    }

    fn dispatch_rd_inputs(
        &mut self,
        tab_id: u64,
        inputs: Vec<shellumi_remote_desktop::RemoteInput>,
    ) {
        if inputs.is_empty() {
            return;
        }
        let envelopes: Vec<_> = if let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) {
            inputs
                .into_iter()
                .map(|input| ctrl.wrap_envelope(input))
                .collect()
        } else {
            return;
        };
        let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) else {
            return;
        };
        let Some(rd) = tab.remote_desktop_state.as_mut() else {
            return;
        };
        let Some(ui_handle) = rd.ui_handle.as_ref() else {
            return;
        };
        for env in envelopes {
            match ui_handle
                .try_send_command(shellumi_remote_desktop::SessionCommand::InputEnvelope(env))
            {
                Ok(()) => {
                    if let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) {
                        ctrl.counters.enqueued += 1;
                    }
                }
                Err(err) => {
                    if let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) {
                        ctrl.counters.queue_full_errors += 1;
                        ctrl.counters.last_error_category = Some(err.clone());
                        match ui_handle
                            .request_recovery(ctrl.identity(), "Command queue admission failure")
                        {
                            Ok(ticket) if ctrl.install_recovery(&ticket) => {}
                            Ok(_) => {
                                ctrl.block_recovery("Recovery ticket could not be installed");
                                tracing::error!("Input recovery ticket could not be installed");
                            }
                            Err(error) => {
                                ctrl.block_recovery(&error);
                                tracing::warn!(%error, "Input recovery rejected");
                            }
                        }
                    }
                    rd.last_action_note = Some(format!("Input recovery triggered: {err}"));
                    break;
                }
            }
        }
    }

    pub(crate) fn remote_desktop_release_all_for_tab(&mut self, tab_id: u64) {
        let releases = if let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) {
            ctrl.focused = false;
            ctrl.release_all()
        } else {
            Vec::new()
        };
        self.dispatch_rd_inputs(tab_id, releases);
    }

    pub(crate) fn remote_desktop_handle_mouse_down(
        &mut self,
        tab_id: u64,
        event: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fh = self.ensure_rd_focus_handle(tab_id, window, cx);
        window.focus(&fh);

        let button = match gpui_button_to_domain(event.button) {
            Some(b) => b,
            None => return,
        };
        let tx = self.rd_canvas_transform_for_tab(tab_id);
        let (win_x, win_y) = (event.position.x.to_f64(), event.position.y.to_f64());

        let result = {
            let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) else {
                return;
            };
            ctrl.on_pointer_down(button, win_x, win_y, tx.as_ref())
        };
        if let Ok(inputs) = result {
            self.dispatch_rd_inputs(tab_id, inputs);
        }
    }

    pub(crate) fn remote_desktop_handle_mouse_up(
        &mut self,
        tab_id: u64,
        event: &gpui::MouseUpEvent,
        _cx: &mut Context<Self>,
    ) {
        let button = match gpui_button_to_domain(event.button) {
            Some(b) => b,
            None => return,
        };
        let tx = self.rd_canvas_transform_for_tab(tab_id);
        let (win_x, win_y) = (event.position.x.to_f64(), event.position.y.to_f64());

        let result = {
            let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) else {
                return;
            };
            ctrl.on_pointer_up(button, win_x, win_y, tx.as_ref())
        };
        if let Ok(inputs) = result {
            self.dispatch_rd_inputs(tab_id, inputs);
        }
    }

    pub(crate) fn remote_desktop_handle_mouse_move(
        &mut self,
        tab_id: u64,
        event: &gpui::MouseMoveEvent,
        _cx: &mut Context<Self>,
    ) {
        let tx = self.rd_canvas_transform_for_tab(tab_id);
        let (win_x, win_y) = (event.position.x.to_f64(), event.position.y.to_f64());

        let result = {
            let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) else {
                return;
            };
            ctrl.on_pointer_move(win_x, win_y, tx.as_ref())
        };
        if let Ok(inputs) = result {
            self.dispatch_rd_inputs(tab_id, inputs);
        }
    }

    pub(crate) fn remote_desktop_handle_scroll_wheel(
        &mut self,
        tab_id: u64,
        event: &gpui::ScrollWheelEvent,
        _cx: &mut Context<Self>,
    ) {
        let tx = self.rd_canvas_transform_for_tab(tab_id);
        let (win_x, win_y) = (event.position.x.to_f64(), event.position.y.to_f64());
        let (dx, dy, is_pixels) = match event.delta {
            gpui::ScrollDelta::Pixels(pt) => (pt.x.to_f64(), pt.y.to_f64(), true),
            gpui::ScrollDelta::Lines(pt) => (f64::from(pt.x), f64::from(pt.y), false),
        };

        let result = {
            let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) else {
                return;
            };
            ctrl.on_scroll(win_x, win_y, dx, dy, is_pixels, tx.as_ref())
        };
        if let Ok(inputs) = result {
            self.dispatch_rd_inputs(tab_id, inputs);
        }
    }

    pub(crate) fn remote_desktop_handle_key_down(
        &mut self,
        tab_id: u64,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;
        // Explicit local escape-focus chord: Ctrl+Alt+Escape releases remote keyboard capture
        if ks.key == "escape" && ks.modifiers.control && ks.modifiers.alt {
            self.remote_desktop_release_all_for_tab(tab_id);
            window.focus(&self.focus);
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                if let Some(rd) = &mut tab.remote_desktop_state {
                    rd.last_action_note = Some("Released remote keyboard focus".into());
                }
            }
            cx.notify();
            return;
        }

        // macOS convenience mapping for Cmd+C/V/X/A/Z and Shift+Cmd+Z when enabled
        if self.rd_keyboard_mapping
            == shellumi_remote_desktop::KeyboardMappingMode::MacToWindowsLinux
            && ks.modifiers.platform
            && !ks.modifiers.control
            && !ks.modifiers.alt
        {
            let shortcut = match (ks.key.as_str(), ks.modifiers.shift) {
                ("c", false) => Some(shellumi_remote_desktop::RemoteShortcut::CtrlC),
                ("v", false) => Some(shellumi_remote_desktop::RemoteShortcut::CtrlV),
                ("x", false) => Some(shellumi_remote_desktop::RemoteShortcut::CtrlX),
                ("a", false) => Some(shellumi_remote_desktop::RemoteShortcut::CtrlA),
                ("z", false) => Some(shellumi_remote_desktop::RemoteShortcut::CtrlZ),
                ("z", true) => Some(shellumi_remote_desktop::RemoteShortcut::CtrlShiftZ),
                _ => None,
            };
            if let Some(sc) = shortcut {
                // Temporarily release Super if it was already forwarded so remote OS
                // receives a clean Ctrl+* chord without Win+Ctrl+*.
                let release_super = self
                    .rd_input_controllers
                    .get_mut(&tab_id)
                    .and_then(|c| c.on_key_up("platform").ok())
                    .unwrap_or_default();
                if !release_super.is_empty() {
                    self.dispatch_rd_inputs(tab_id, release_super);
                }
                if sc == shellumi_remote_desktop::RemoteShortcut::CtrlV {
                    self.remote_desktop_paste_transaction(tab_id, cx);
                    return;
                }
                if let Some(tab_idx) = self.tabs.iter().position(|t| t.id == tab_id) {
                    self.remote_desktop_send_shortcut(tab_idx, sc, cx);
                }
                return;
            }
            // Non-convenience Cmd+key: flush deferred Super press if not yet transmitted
            let need_super = self
                .rd_input_controllers
                .get(&tab_id)
                .is_some_and(|c| !c.held_keys.contains_key("platform"));
            if need_super {
                let super_key = shellumi_remote_desktop::RemoteKey {
                    keysym: 0xFFEB,
                    scancode: 0x5B,
                    extended: true,
                    is_modifier: true,
                    key_name: "platform".into(),
                };
                if let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) {
                    if let Ok(inputs) = ctrl.on_key_down("platform", super_key, false) {
                        self.dispatch_rd_inputs(tab_id, inputs);
                    }
                }
            }
        }

        // Native Ctrl+V paste transaction (respecting ClipboardPolicy)
        if ks.modifiers.control && !ks.modifiers.alt && !ks.modifiers.platform && ks.key == "v" {
            self.remote_desktop_paste_transaction(tab_id, cx);
            return;
        }

        // Separate IME/composed text from physical shortcut keys
        if !ks.modifiers.control && !ks.modifiers.alt && !ks.modifiers.platform {
            if let Some(ch_str) = ks.key_char.as_deref().filter(|s| !s.is_empty()) {
                let is_composed_or_non_ascii = !ch_str.is_ascii() || ch_str != ks.key;
                if is_composed_or_non_ascii
                    && gpui_keystroke_to_remote_key(ks).is_none_or(|rk| rk.scancode == 0)
                {
                    let result = {
                        let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) else {
                            return;
                        };
                        ctrl.on_committed_text(ch_str)
                    };
                    if let Ok(inputs) = result {
                        self.dispatch_rd_inputs(tab_id, inputs);
                    }
                    return;
                }
            }
        }

        if let Some(remote_key) = gpui_keystroke_to_remote_key(ks) {
            let result = {
                let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) else {
                    return;
                };
                ctrl.on_key_down(ks.key.clone(), remote_key, event.is_held)
            };
            if let Ok(inputs) = result {
                self.dispatch_rd_inputs(tab_id, inputs);
            }
        } else if let Some(ch_str) = ks.key_char.as_deref().filter(|s| !s.is_empty()) {
            let result = {
                let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) else {
                    return;
                };
                ctrl.on_committed_text(ch_str)
            };
            if let Ok(inputs) = result {
                self.dispatch_rd_inputs(tab_id, inputs);
            }
        }
    }

    pub(crate) fn remote_desktop_handle_key_up(
        &mut self,
        tab_id: u64,
        event: &gpui::KeyUpEvent,
        _cx: &mut Context<Self>,
    ) {
        let result = {
            let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) else {
                return;
            };
            ctrl.on_key_up(&event.keystroke.key)
        };
        if let Ok(inputs) = result {
            self.dispatch_rd_inputs(tab_id, inputs);
        }
    }

    pub(crate) fn remote_desktop_handle_modifiers_changed(
        &mut self,
        tab_id: u64,
        event: &gpui::ModifiersChangedEvent,
        _cx: &mut Context<Self>,
    ) {
        let new_mods = shellumi_remote_desktop::KeyModifiers {
            shift: event.modifiers.shift,
            ctrl: event.modifiers.control,
            alt: event.modifiers.alt,
            meta: event.modifiers.platform,
        };
        let mut inputs = Vec::new();
        if let Some(ctrl) = self.rd_input_controllers.get_mut(&tab_id) {
            if !ctrl.connected || ctrl.view_only {
                return;
            }
            let old = ctrl.modifiers;
            ctrl.modifiers = new_mods;
            let defer_super = self.rd_keyboard_mapping
                == shellumi_remote_desktop::KeyboardMappingMode::MacToWindowsLinux;
            for (was, is, id, keysym, scancode, extended) in [
                (
                    old.shift,
                    new_mods.shift,
                    "shift",
                    0xFFE1u32,
                    0x2Au16,
                    false,
                ),
                (old.ctrl, new_mods.ctrl, "control", 0xFFE3, 0x1D, false),
                (old.alt, new_mods.alt, "alt", 0xFFE9, 0x38, false),
                (old.meta, new_mods.meta, "platform", 0xFFEB, 0x5B, true),
            ] {
                if was != is {
                    // In MacToWindowsLinux mode, defer sending Super down until a non-convenience
                    // key is pressed so Cmd+C/V/X/A/Z never emits Super+Ctrl+V.
                    if id == "platform" && is && defer_super {
                        continue;
                    }
                    let key = shellumi_remote_desktop::RemoteKey {
                        keysym,
                        scancode,
                        extended,
                        is_modifier: true,
                        key_name: id.to_string(),
                    };
                    if is {
                        if let Ok(ev) = ctrl.on_key_down(id, key, false) {
                            inputs.extend(ev);
                        }
                    } else if let Ok(ev) = ctrl.on_key_up(id) {
                        inputs.extend(ev);
                    }
                }
            }
        }
        self.dispatch_rd_inputs(tab_id, inputs);
    }
}

fn gpui_button_to_domain(
    button: gpui::MouseButton,
) -> Option<shellumi_remote_desktop::MouseButton> {
    match button {
        gpui::MouseButton::Left => Some(shellumi_remote_desktop::MouseButton::Left),
        gpui::MouseButton::Middle => Some(shellumi_remote_desktop::MouseButton::Middle),
        gpui::MouseButton::Right => Some(shellumi_remote_desktop::MouseButton::Right),
        gpui::MouseButton::Navigate(gpui::NavigationDirection::Back) => {
            Some(shellumi_remote_desktop::MouseButton::Button4)
        }
        gpui::MouseButton::Navigate(gpui::NavigationDirection::Forward) => {
            Some(shellumi_remote_desktop::MouseButton::Button5)
        }
    }
}

pub(crate) fn gpui_keystroke_to_remote_key(
    ks: &gpui::Keystroke,
) -> Option<shellumi_remote_desktop::RemoteKey> {
    let name = ks.key.as_str();
    let (keysym, scancode, extended, is_modifier) = match name {
        "escape" => (0xFF1B, 0x01, false, false),
        "backspace" => (0xFF08, 0x0E, false, false),
        "tab" => (0xFF09, 0x0F, false, false),
        "enter" | "return" => (0xFF0D, 0x1C, false, false),
        "space" | " " => (0x0020, 0x39, false, false),
        "left" => (0xFF51, 0x4B, true, false),
        "up" => (0xFF52, 0x48, true, false),
        "right" => (0xFF53, 0x4D, true, false),
        "down" => (0xFF54, 0x50, true, false),
        "home" => (0xFF50, 0x47, true, false),
        "end" => (0xFF57, 0x4F, true, false),
        "pageup" => (0xFF55, 0x49, true, false),
        "pagedown" => (0xFF56, 0x51, true, false),
        "insert" => (0xFF63, 0x52, true, false),
        "delete" => (0xFFFF, 0x53, true, false),
        "f1" => (0xFFBE, 0x3B, false, false),
        "f2" => (0xFFBF, 0x3C, false, false),
        "f3" => (0xFFC0, 0x3D, false, false),
        "f4" => (0xFFC1, 0x3E, false, false),
        "f5" => (0xFFC2, 0x3F, false, false),
        "f6" => (0xFFC3, 0x40, false, false),
        "f7" => (0xFFC4, 0x41, false, false),
        "f8" => (0xFFC5, 0x42, false, false),
        "f9" => (0xFFC6, 0x43, false, false),
        "f10" => (0xFFC7, 0x44, false, false),
        "f11" => (0xFFC8, 0x57, false, false),
        "f12" => (0xFFC9, 0x58, false, false),
        "shift" => (0xFFE1, 0x2A, false, true),
        "rshift" | "rightshift" => (0xFFE2, 0x36, false, true),
        "control" | "ctrl" => (0xFFE3, 0x1D, false, true),
        "rcontrol" | "rightcontrol" => (0xFFE4, 0x1D, true, true),
        "alt" => (0xFFE9, 0x38, false, true),
        "ralt" | "rightalt" => (0xFFEA, 0x38, true, true),
        "platform" | "cmd" | "super" | "win" => (0xFFEB, 0x5B, true, true),
        "rplatform" | "rightsuper" => (0xFFEC, 0x5C, true, true),
        "capslock" => (0xFFE5, 0x3A, false, false),
        "numlock" => (0xFF7F, 0x45, false, false),
        "scrolllock" => (0xFF14, 0x46, false, false),
        "printscreen" => (0xFF61, 0x37, true, false),
        "pause" => (0xFF13, 0x45, false, false),
        "kp_0" => (0xFFB0, 0x52, false, false),
        "kp_1" => (0xFFB1, 0x4F, false, false),
        "kp_2" => (0xFFB2, 0x50, false, false),
        "kp_3" => (0xFFB3, 0x51, false, false),
        "kp_4" => (0xFFB4, 0x4B, false, false),
        "kp_5" => (0xFFB5, 0x4C, false, false),
        "kp_6" => (0xFFB6, 0x4D, false, false),
        "kp_7" => (0xFFB7, 0x47, false, false),
        "kp_8" => (0xFFB8, 0x48, false, false),
        "kp_9" => (0xFFB9, 0x49, false, false),
        "kp_decimal" => (0xFFAE, 0x53, false, false),
        "kp_divide" => (0xFFAF, 0x35, true, false),
        "kp_multiply" => (0xFFAA, 0x37, false, false),
        "kp_subtract" => (0xFFAD, 0x4A, false, false),
        "kp_add" => (0xFFAB, 0x4E, false, false),
        "kp_enter" => (0xFF8D, 0x1C, true, false),
        _ => {
            let ch = name.chars().next().filter(|_| name.chars().count() == 1)?;
            let sc = ascii_char_to_scancode(ch);
            let cp = ch as u32;
            let keysym = if (0x0020..=0x007E).contains(&cp) || (0x00A0..=0x00FF).contains(&cp) {
                cp
            } else {
                0x0100_0000 | cp
            };
            if sc != 0 || keysym != 0 {
                (keysym, sc, false, false)
            } else {
                return None;
            }
        }
    };
    Some(shellumi_remote_desktop::RemoteKey {
        keysym,
        scancode,
        extended,
        is_modifier,
        key_name: name.to_string(),
    })
}

fn ascii_char_to_scancode(ch: char) -> u16 {
    match ch.to_ascii_lowercase() {
        '1' | '!' => 0x02,
        '2' | '@' => 0x03,
        '3' | '#' => 0x04,
        '4' | '$' => 0x05,
        '5' | '%' => 0x06,
        '6' | '^' => 0x07,
        '7' | '&' => 0x08,
        '8' | '*' => 0x09,
        '9' | '(' => 0x0A,
        '0' | ')' => 0x0B,
        '-' | '_' => 0x0C,
        '=' | '+' => 0x0D,
        'q' => 0x10,
        'w' => 0x11,
        'e' => 0x12,
        'r' => 0x13,
        't' => 0x14,
        'y' => 0x15,
        'u' => 0x16,
        'i' => 0x17,
        'o' => 0x18,
        'p' => 0x19,
        '[' | '{' => 0x1A,
        ']' | '}' => 0x1B,
        'a' => 0x1E,
        's' => 0x1F,
        'd' => 0x20,
        'f' => 0x21,
        'g' => 0x22,
        'h' => 0x23,
        'j' => 0x24,
        'k' => 0x25,
        'l' => 0x26,
        ';' | ':' => 0x27,
        '\'' | '"' => 0x28,
        '`' | '~' => 0x29,
        '\\' | '|' => 0x2B,
        'z' => 0x2C,
        'x' => 0x2D,
        'c' => 0x2E,
        'v' => 0x2F,
        'b' => 0x30,
        'n' => 0x31,
        'm' => 0x32,
        ',' | '<' => 0x33,
        '.' | '>' => 0x34,
        '/' | '?' => 0x35,
        ' ' => 0x39,
        _ => 0,
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
            .take_ready_to_dispatch(1, true)
            .expect("should dispatch 1440x900");
        assert_eq!(req1.dimensions, (1440, 900));
        assert!(state.take_ready_to_dispatch(1, true).is_none());

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

        // Acknowledging (1440, 900) immediately promotes the queued manual (1920, 1080) at epoch 2.
        let req2 = state
            .acknowledge((1440, 900), 2, true)
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
    fn test_rd_resize_request_id_prevents_stale_timeouts_and_handles_queue_failure_and_worker_rejection(
    ) {
        let mut state = crate::RdAutoResizeState::default();
        state.request_resize((1280, 720), 1, crate::ResizeOrigin::Automatic);
        let req1 = state.take_ready_to_dispatch(1, true).unwrap();

        // Queue failure clears in_flight immediately without leaving request stuck.
        state.fail_dispatch(req1.request_id, "channel full".into());
        assert_eq!(state.in_flight, None);
        assert_eq!(state.last_error.as_deref(), Some("channel full"));

        // Stale timeout for req1 must NOT cancel req2 even with identical dimensions.
        state.request_resize((1280, 720), 1, crate::ResizeOrigin::Manual);
        let req2 = state.take_ready_to_dispatch(1, true).unwrap();
        assert_ne!(req1.request_id, req2.request_id);
        assert_eq!(state.timeout_request(req1.request_id, 1, true), None);
        assert_eq!(state.in_flight, Some(req2));

        // Worker ResizeFailed clears matching in-flight and records last_rejected.
        assert_eq!(
            state.handle_resize_failed((1280, 720), "worker rejected".into(), 1, true),
            None
        );
        assert_eq!(state.in_flight, None);
        assert_eq!(state.last_rejected, Some((1280, 720)));
        assert_eq!(state.last_error.as_deref(), Some("worker rejected"));

        // Delayed ack for an older size cannot complete a newer in-flight size.
        state.request_resize((1920, 1080), 2, crate::ResizeOrigin::Manual);
        let req3 = state.take_ready_to_dispatch(2, true).unwrap();
        let _ = state.acknowledge((1280, 720), 2, true);
        assert_eq!(state.in_flight, Some(req3));
    }

    #[test]
    fn test_disabling_dynamic_resize_cancels_auto_work_and_prevents_reactivation_on_reenable() {
        let mut state = crate::RdAutoResizeState::default();
        assert_eq!(state.acknowledge((1280, 720), 1, true), None);
        assert_eq!(state.last_confirmed, Some((1280, 720)));

        // Queued automatic request is dropped and auto_cancel_generation increments when disabled.
        let cancel_before = state.auto_cancel_generation;
        state.request_resize((1440, 900), 2, crate::ResizeOrigin::Automatic);
        state.cancel_automatic();
        assert_ne!(state.auto_cancel_generation, cancel_before);
        assert_eq!(state.desired, None);
        assert_eq!(state.last_confirmed, Some((1280, 720)));

        // Disable then RE-ENABLE before an old in-flight automatic timeout executes:
        // the old request's auto_cancel_generation no longer matches, so it must NEVER retry!
        state.request_resize((1600, 1000), 3, crate::ResizeOrigin::Automatic);
        let auto_req = state.take_ready_to_dispatch(3, true).unwrap();
        state.cancel_automatic(); // disabled
                                  // Re-enabled (dynamic_resize_enabled = true again), but old auto_req has stale token:
        assert_eq!(state.timeout_request(auto_req.request_id, 3, true), None);
        assert_eq!(state.in_flight, None);

        // Pending manual requests are preserved when automatic resizing is canceled.
        state.request_resize((1920, 1080), 4, crate::ResizeOrigin::Manual);
        state.cancel_automatic();
        let manual_req = state
            .take_ready_to_dispatch(4, false)
            .expect("manual request should survive cancel_automatic");
        assert_eq!(manual_req.origin, crate::ResizeOrigin::Manual);
        assert_eq!(manual_req.dimensions, (1920, 1080));
        assert_eq!(state.last_confirmed, Some((1280, 720)));
    }

    #[test]
    fn test_invalidate_layout_discards_obsolete_geometry_and_flags_manual_recalc() {
        let mut layout_epoch = 1u64;
        let mut measurements = std::collections::HashMap::new();
        let mut states = std::collections::HashMap::new();
        measurements.insert(
            42,
            crate::CanvasMeasurement {
                tab_id: 42,
                layout_epoch,
                viewport: gpui::Bounds::default(),
            },
        );
        let mut st = crate::RdAutoResizeState::default();
        st.request_resize((1280, 720), layout_epoch, crate::ResizeOrigin::Automatic);
        let debounce_before = st.debounce_generation;
        states.insert(42, st);

        // Layout invalidation (e.g. inspector open, sidebar collapse, DPI change)
        Shellumi::invalidate_layout_state_all(&mut layout_epoch, &mut measurements, &mut states);
        assert_eq!(layout_epoch, 2);
        assert!(measurements.is_empty());
        let st_after = states.get(&42).unwrap();
        assert_eq!(st_after.layout_generation, 2);
        assert_ne!(st_after.debounce_generation, debounce_before);
        assert_eq!(st_after.desired, None);

        // Manual request invalidated before dispatch flags pending_manual_recalc instead of sending obsolete dimensions.
        states
            .get_mut(&42)
            .unwrap()
            .request_resize((1440, 900), 2, crate::ResizeOrigin::Manual);
        Shellumi::invalidate_layout_state_all(&mut layout_epoch, &mut measurements, &mut states);
        assert_eq!(layout_epoch, 3);
        let st_manual = states.get_mut(&42).unwrap();
        assert!(st_manual.pending_manual_recalc);
        assert_eq!(st_manual.desired, None);

        // In-flight request from epoch 3 must NOT retry if layout advances to epoch 4 before timeout.
        st_manual.pending_manual_recalc = false;
        st_manual.request_resize((1600, 900), 3, crate::ResizeOrigin::Automatic);
        let in_flight = st_manual.take_ready_to_dispatch(3, true).unwrap();
        Shellumi::invalidate_layout_state_all(&mut layout_epoch, &mut measurements, &mut states);
        assert_eq!(
            states
                .get_mut(&42)
                .unwrap()
                .timeout_request(in_flight.request_id, layout_epoch, true),
            None
        );
    }

    #[test]
    fn test_worker_channel_resize_submission_failure_and_confirmation_lifecycle() {
        let (cmd_tx, cmd_rx) = async_channel::bounded(4);
        let (evt_tx, evt_rx) = async_channel::bounded(4);
        let mut state = crate::RdAutoResizeState::default();

        // 1. Coordinator queues and submits RequestResize over SessionCommand channel.
        state.request_resize((1600, 900), 1, crate::ResizeOrigin::Automatic);
        let req1 = state.take_ready_to_dispatch(1, true).unwrap();
        cmd_tx
            .try_send(shellumi_remote_desktop::SessionCommand::RequestResize {
                identity: shellumi_remote_desktop::SessionIdentity::new(1, 1, 1),
                width: req1.dimensions.0,
                height: req1.dimensions.1,
            })
            .unwrap();
        match cmd_rx.try_recv().unwrap() {
            shellumi_remote_desktop::SessionCommand::RequestResize { width, height, .. } => {
                assert_eq!((width, height), (1600, 900));
            }
            other => panic!("unexpected command: {other:?}"),
        }

        // 2. Worker emits ResizeFailed, while a manual request is queued behind it.
        assert_eq!(
            state.request_resize((1920, 1080), 1, crate::ResizeOrigin::Manual),
            crate::ResizeRequestOutcome::QueuedBehindInFlight
        );
        evt_tx
            .try_send(shellumi_remote_desktop::SessionEvent::ResizeFailed {
                width: 1600,
                height: 900,
                reason: "worker input channel closed".to_string(),
            })
            .unwrap();
        let next = match evt_rx.try_recv().unwrap() {
            shellumi_remote_desktop::SessionEvent::ResizeFailed {
                width,
                height,
                reason,
            } => state.handle_resize_failed((width, height), reason, 1, true),
            _ => panic!("expected ResizeFailed"),
        };
        let req2 = next.expect("queued manual request should promote after ResizeFailed");
        assert_eq!(req2.dimensions, (1920, 1080));
        assert_eq!(req2.origin, crate::ResizeOrigin::Manual);

        // 3. Worker confirms 1920x1080 via RemoteResize.
        evt_tx
            .try_send(shellumi_remote_desktop::SessionEvent::RemoteResize {
                width: 1920,
                height: 1080,
            })
            .unwrap();
        match evt_rx.try_recv().unwrap() {
            shellumi_remote_desktop::SessionEvent::RemoteResize { width, height } => {
                assert_eq!(state.acknowledge((width, height), 1, true), None);
            }
            _ => panic!("expected RemoteResize"),
        }
        assert_eq!(state.in_flight, None);
        assert_eq!(state.last_confirmed, Some((1920, 1080)));
    }
}

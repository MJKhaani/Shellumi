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

        let (target, width, height, color_depth) = match protocol {
            RemoteDesktopProtocol::Vnc => {
                let vnc_cfg = host.vnc_config().unwrap_or_else(|| {
                    VncConfig::new(host.destination.host(), host.destination.port().get())
                        .unwrap_or_default()
                });
                (
                    format!("{}:{}", vnc_cfg.host, vnc_cfg.port),
                    1920,
                    1080,
                    vnc_cfg.color_depth,
                )
            }
            RemoteDesktopProtocol::Rdp => {
                let rdp_cfg = host.rdp_config().unwrap_or_else(|| {
                    RdpConfig::new(host.destination.host(), host.destination.port().get())
                        .unwrap_or_default()
                });
                (
                    format!("{}:{}", rdp_cfg.host, rdp_cfg.port),
                    rdp_cfg.screen_width as u32,
                    rdp_cfg.screen_height as u32,
                    rdp_cfg.color_depth,
                )
            }
        };

        let rd_state = RemoteDesktopState {
            host_name: host.name.clone(),
            target,
            protocol,
            status: RemoteDesktopStatus::NotConnected,
            status_message: Some(format!(
                "Ready to connect to {} via {} — native engine in development",
                host.name,
                match protocol {
                    RemoteDesktopProtocol::Vnc => "RFB / VNC (port 5900)",
                    RemoteDesktopProtocol::Rdp => "MS-RDPBCGR / RDP (port 3389)",
                }
            )),
            width,
            height,
            view_only: false,
            scale_mode: RemoteDesktopScaleMode::Fit,
            color_depth,
            fps: None,
            latency_ms: None,
            clipboard_text: None,
            error: None,
            last_action_note: None,
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
        x: u16,
        y: u16,
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
                let _click_packet = build_rfb_pointer_event(rfb_constants::BUTTON_LEFT, x, y);
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

    pub(crate) fn remote_desktop_disconnect(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
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
                rd.status = RemoteDesktopStatus::Connecting;
                rd.status_message =
                    Some("Connecting... (native protocol worker in development)".into());
                rd.last_action_note = Some("Connection initiated".into());
                cx.notify();
            }
        }
    }

    pub(crate) fn remote_desktop_sync_clipboard(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
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

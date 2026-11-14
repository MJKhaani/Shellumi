//! Terminal actions.
use super::*;

impl Shellumi {
    pub(crate) fn quick_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.quick_connect_input.read(cx).value().trim().to_string();
        if target.is_empty() {
            return;
        }

        if let Some((proto, host, port, username)) =
            shellumi_core::parse_graphical_quick_connect(&target)
        {
            self.quick_connect_error = None;
            self.quick_connect_input.update(cx, |input, cx| {
                input.set_value("", cx);
            });
            let rd_proto = match proto {
                ConnectionProtocol::Vnc => RemoteDesktopProtocol::Vnc,
                ConnectionProtocol::Rdp => RemoteDesktopProtocol::Rdp,
                _ => return,
            };
            self.launch_quick_connect_remote_desktop(rd_proto, &host, port, username, window, cx);
            return;
        }

        match shellumi_core::parse_quick_connect_target(&target) {
            Ok(destination) => {
                self.quick_connect_error = None;
                let command = TerminalCommand::new("ssh").args(destination.openssh_arguments());
                self.start_session(target, SessionKind::Ssh, command, window, cx);
            }
            Err(error) => {
                self.quick_connect_error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn bind_pane_events(
        &self,
        tab_id: u64,
        pane_id: u64,
        events: async_channel::Receiver<SessionEvent>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |app, cx| {
            while let Ok(event) = events.recv().await {
                let closed = matches!(event, SessionEvent::Closed | SessionEvent::Error(_));
                if app
                    .update(cx, |app, cx| {
                        if let Some(tab) = app.tabs.iter_mut().find(|t| t.id == tab_id) {
                            if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pane_id) {
                                match event {
                                    SessionEvent::Output(bytes) => {
                                        let follow = pane.scroll.max_offset().height
                                            + pane.scroll.offset().y
                                            <= px(24.0);
                                        pane.terminal.process_bytes(&bytes);
                                        if follow {
                                            pane.scroll.scroll_to_bottom();
                                        }
                                        if !pane.search_query.is_empty() {
                                            pane.search_matches =
                                                pane.terminal.search(&pane.search_query, false);
                                        }
                                    }
                                    SessionEvent::Error(error) => {
                                        pane.message = Some(format!("Session error: {error}"));
                                        pane.session = None;
                                    }
                                    SessionEvent::Closed => {
                                        pane.message = Some("Session ended.".into());
                                        pane.session = None;
                                    }
                                }
                                cx.notify();
                            }
                        }
                    })
                    .is_err()
                    || closed
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn start_session(
        &mut self,
        name: String,
        kind: SessionKind,
        command: TerminalCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_session_with_startup(name, kind, command, None, window, cx);
    }

    pub(crate) fn start_session_with_startup(
        &mut self,
        name: String,
        kind: SessionKind,
        command: TerminalCommand,
        startup_snippet_name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            return;
        }
        self.form = None;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let pane_id = self.next_tab_id;

        let session = Session::start(command);
        let events = session.events.clone();
        let search_input = cx.new(|cx| TextInput::new(cx, "", "Search output…"));
        let initial_message = startup_snippet_name.as_ref().map(|name| {
            format!(
                "Startup snippet '{}' was not run automatically; run it manually after authentication.",
                name
            )
        });
        let pane = TerminalPane {
            id: pane_id,
            name: name.clone(),
            kind,
            scroll: gpui::ScrollHandle::new(),
            terminal: {
                let mut terminal = Terminal::new(24, 80);
                terminal.set_scrollback_limit(self.desktop.preferences.scrollback_lines);
                terminal
            },
            session: Some(session),
            message: initial_message,
            search_open: false,
            search_input,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_idx: None,
        };

        let sftp_state = if kind == SessionKind::Sftp {
            let local_path = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("/"))
                .to_string_lossy()
                .to_string();
            let local_entries = list_local_directory(&local_path, false).unwrap_or_default();
            let remote_path = "/home".to_string();
            let remote_entries = Vec::new();
            Some(SftpBrowserState {
                listing_generation: 0,
                connection_options: SftpConnectOptions::default(),
                host_name: name.clone(),
                destination: None,
                jump_host: None,
                loading: false,
                local_path,
                remote_path,
                local_entries,
                remote_entries,
                selected_local: None,
                selected_remote: None,
                show_hidden: false,
                mode: SftpTabMode::Terminal,
                error: None,
                status_message: None,
                queue: TransferQueue::new(2),
            })
        } else {
            None
        };

        self.tabs.push(SessionTab {
            id: tab_id,
            name,
            kind,
            panes: vec![pane],
            active_pane_idx: 0,
            split: None,
            sftp_state,
            remote_desktop_state: None,
            multiplayer: None,
        });
        self.page = None;
        self.active_tab_idx = Some(self.tabs.len() - 1);
        window.focus(&self.focus);
        cx.notify();

        self.bind_pane_events(tab_id, pane_id, events, cx);
    }

    pub(crate) fn split_pane(
        &mut self,
        direction: SplitDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if tab.panes.len() >= 2 {
            return;
        }
        let tab_id = tab.id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let pane_id = self.next_tab_id;

        let session = Session::start(TerminalCommand::default_shell());
        let events = session.events.clone();
        let search_input = cx.new(|cx| TextInput::new(cx, "", "Search output…"));
        let pane = TerminalPane {
            id: pane_id,
            name: "Local terminal".into(),
            kind: SessionKind::Local,
            scroll: gpui::ScrollHandle::new(),
            terminal: {
                let mut terminal = Terminal::new(24, 80);
                terminal.set_scrollback_limit(self.desktop.preferences.scrollback_lines);
                terminal
            },
            session: Some(session),
            message: None,
            search_open: false,
            search_input,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_idx: None,
        };

        tab.panes.push(pane);
        tab.split = Some(direction);
        tab.active_pane_idx = tab.panes.len() - 1;
        window.focus(&self.focus);
        cx.notify();

        self.bind_pane_events(tab_id, pane_id, events, cx);
    }

    pub(crate) fn close_pane(&mut self, pane_id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if tab.panes.len() > 1 {
            tab.panes.retain(|p| p.id != pane_id);
            tab.split = None;
            tab.active_pane_idx = 0;
            window.focus(&self.focus);
            cx.notify();
        } else {
            let tab_id = tab.id;
            self.close_tab(tab_id, window, cx);
        }
    }

    pub(crate) fn toggle_search(&mut self, pane_id: u64, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pane_id) {
            pane.search_open = !pane.search_open;
            if !pane.search_open {
                pane.search_query.clear();
                pane.search_matches.clear();
                pane.current_match_idx = None;
            }
            cx.notify();
        }
    }

    pub(crate) fn execute_search(&mut self, pane_id: u64, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pane_id) {
            let query = pane.search_input.read(cx).value().trim().to_string();
            pane.search_query = query.clone();
            pane.search_matches = if query.is_empty() {
                Vec::new()
            } else {
                pane.terminal.search(&query, false)
            };
            pane.current_match_idx = if pane.search_matches.is_empty() {
                None
            } else {
                Some(0)
            };
            cx.notify();
        }
    }

    pub(crate) fn search_next(&mut self, pane_id: u64, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pane_id) {
            let count = pane.search_matches.len();
            if count > 0 {
                let next_idx = match pane.current_match_idx {
                    Some(i) => (i + 1) % count,
                    None => 0,
                };
                pane.current_match_idx = Some(next_idx);
                cx.notify();
            }
        }
    }

    pub(crate) fn search_prev(&mut self, pane_id: u64, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if let Some(pane) = tab.panes.iter_mut().find(|p| p.id == pane_id) {
            let count = pane.search_matches.len();
            if count > 0 {
                let prev_idx = match pane.current_match_idx {
                    Some(0) | None => count - 1,
                    Some(i) => i - 1,
                };
                pane.current_match_idx = Some(prev_idx);
                cx.notify();
            }
        }
    }

    pub(crate) fn close_tab(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.sftp_jobs.retain(|(tab_id, _), job| {
                if *tab_id == id {
                    job.control.cancel();
                    false
                } else {
                    true
                }
            });
            self.tabs.remove(pos);
            if self.tabs.is_empty() {
                self.active_tab_idx = None;
            } else if let Some(active) = self.active_tab_idx {
                if active == pos {
                    self.active_tab_idx = Some(pos.min(self.tabs.len() - 1));
                } else if active > pos {
                    self.active_tab_idx = Some(active - 1);
                }
            }
            window.focus(&self.focus);
            cx.notify();
        }
    }

    pub(crate) fn select_tab(
        &mut self,
        idx: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.page = None;
        self.active_tab_idx = idx;
        window.focus(&self.focus);
        cx.notify();
    }

    pub(crate) fn connect(
        &mut self,
        host: &SavedHost,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let proto = host.protocol();
        match proto {
            ConnectionProtocol::Ssh => self.launch_saved_connection(host, false, window, cx),
            ConnectionProtocol::Mosh => {
                let mosh_cfg = host
                    .protocol_config
                    .as_deref()
                    .and_then(|c| serde_json::from_str::<MoshConfig>(c).ok())
                    .unwrap_or_default();
                let mosh_args =
                    mosh_cfg.build_arguments(&host.destination, host.jump_host.as_deref());
                let command = TerminalCommand::new("mosh").args(mosh_args);
                self.start_session_with_startup(
                    host.name.clone(),
                    SessionKind::Mosh,
                    command,
                    None,
                    window,
                    cx,
                );
            }
            ConnectionProtocol::Telnet => {
                let telnet_cfg = host
                    .protocol_config
                    .as_deref()
                    .and_then(|c| serde_json::from_str::<TelnetConfig>(c).ok())
                    .unwrap_or_else(|| {
                        TelnetConfig::new(host.destination.host(), host.destination.port().get())
                            .unwrap_or_default()
                    });
                let telnet_args = telnet_cfg.build_arguments();
                let command = TerminalCommand::new("telnet").args(telnet_args);
                self.start_session_with_startup(
                    host.name.clone(),
                    SessionKind::Telnet,
                    command,
                    None,
                    window,
                    cx,
                );
            }
            ConnectionProtocol::Serial => {
                let serial_cfg = host
                    .protocol_config
                    .as_deref()
                    .and_then(|c| serde_json::from_str::<SerialConfig>(c).ok())
                    .unwrap_or_else(|| {
                        SerialConfig::new(host.destination.host(), 115200).unwrap_or_default()
                    });
                let serial_args = serial_cfg.build_arguments();
                let command = TerminalCommand::new("screen").args(serial_args);
                self.start_session_with_startup(
                    host.name.clone(),
                    SessionKind::Serial,
                    command,
                    None,
                    window,
                    cx,
                );
            }
            ConnectionProtocol::Local => {
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
                let command = TerminalCommand::new(shell);
                self.start_session_with_startup(
                    host.name.clone(),
                    SessionKind::Local,
                    command,
                    None,
                    window,
                    cx,
                );
            }
            ConnectionProtocol::Vnc => {
                self.launch_remote_desktop(host, RemoteDesktopProtocol::Vnc, window, cx);
            }
            ConnectionProtocol::Rdp => {
                self.launch_remote_desktop(host, RemoteDesktopProtocol::Rdp, window, cx);
            }
        }
    }
}

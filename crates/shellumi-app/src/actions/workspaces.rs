//! Workspaces actions.
use super::*;

impl Shellumi {
    fn load_workspaces(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store.list_workspaces().await.map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Ok(workspaces) = result {
                    app.workspaces = workspaces;
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn save_current_layout_as_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get(active_idx) else {
            return;
        };
        let Some(path) = self.database.clone() else {
            return;
        };

        let layout = match tab.split {
            Some(SplitDirection::Vertical) => WorkspaceLayout::SplitVertical,
            Some(SplitDirection::Horizontal) => WorkspaceLayout::SplitHorizontal,
            Some(SplitDirection::Grid) => WorkspaceLayout::DualGrid,
            None => {
                if tab.panes.len() >= 4 {
                    WorkspaceLayout::DualGrid
                } else if tab.panes.len() >= 2 {
                    WorkspaceLayout::SplitVertical
                } else {
                    WorkspaceLayout::Single
                }
            }
        };

        let sessions: Vec<WorkspaceSession> = tab
            .panes
            .iter()
            .enumerate()
            .map(|(idx, pane)| {
                let kind = match pane.kind {
                    SessionKind::Ssh | SessionKind::Mosh | SessionKind::Telnet => {
                        WorkspaceSessionKind::Ssh
                    }
                    SessionKind::Serial | SessionKind::Local => WorkspaceSessionKind::Local,
                    SessionKind::RemoteDesktop => {
                        if let Some(rd) = &tab.remote_desktop_state {
                            match rd.protocol {
                                RemoteDesktopProtocol::Vnc => WorkspaceSessionKind::Vnc,
                                RemoteDesktopProtocol::Rdp => WorkspaceSessionKind::Rdp,
                            }
                        } else {
                            WorkspaceSessionKind::Local
                        }
                    }
                    SessionKind::Sftp => WorkspaceSessionKind::Sftp,
                };
                let host = match &self.hosts {
                    HostState::Loaded(hosts) => hosts.iter().find(|host| host.name == pane.name),
                    _ => None,
                };
                let destination = host.map(|host| host.destination.clone()).or_else(|| {
                    if let Some(rd) = &tab.remote_desktop_state {
                        let (h, p) = rd
                            .target
                            .split_once(':')
                            .and_then(|(h, p)| p.parse::<u16>().ok().map(|port| (h, port)))
                            .unwrap_or((rd.target.as_str(), 5900));
                        SshDestination::new(h, rd.username.as_deref().unwrap_or(""), p).ok()
                    } else {
                        None
                    }
                });
                WorkspaceSession {
                    name: pane.name.clone(),
                    kind,
                    destination,
                    host_id: host.map(|host| host.id),
                    startup_command: None,
                    working_directory: None,
                    pane_index: idx,
                    excluded_from_broadcast: self.broadcast_manager.is_pane_excluded(idx),
                }
            })
            .collect();

        if sessions.iter().any(|session| {
            session.kind != WorkspaceSessionKind::Local && session.destination.is_none()
        }) {
            self.desktop.message = Some(
                "Save this layout in Workspaces and choose the remote hosts explicitly.".into(),
            );
            cx.notify();
            return;
        }
        let ws_name = format!("{} Workspace", tab.name);
        let layout_str = layout.as_str().to_string();
        let tags = vec!["workspace".to_string(), layout_str.clone()];

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .add_workspace(
                        &ws_name,
                        Some("Saved from active tab layout"),
                        &layout_str,
                        &sessions,
                        &tags,
                    )
                    .await
                    .map_err(|e| e.to_string())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if result.is_ok() {
                    app.load_workspaces(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn load_workspace(
        &mut self,
        workspace_id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ws) = self
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .cloned()
        else {
            return;
        };
        let Some(database) = self.database.clone() else {
            return;
        };
        let prepared = ws.clone();
        let handle = window.window_handle();
        self.desktop.message = Some("Resolving workspace identities…".into());
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async move {
                let mut commands = Vec::new();
                for session in &prepared.sessions {
                    if session.kind == WorkspaceSessionKind::Local {
                        commands.push(TerminalCommand::default_shell());
                        continue;
                    }
                    if session.kind == WorkspaceSessionKind::Vnc
                        || session.kind == WorkspaceSessionKind::Rdp
                    {
                        commands.push(TerminalCommand::new(""));
                        continue;
                    }
                    let plan = if let Some(id) = session.host_id {
                        shellumi_storage::desktop::resolve_connection(&database, id).await?
                    } else {
                        shellumi_core::connection::ConnectionPlan::new(
                            session
                                .destination
                                .clone()
                                .ok_or("Workspace session has no destination.")?,
                            None,
                            None,
                        )?
                    };
                    let (program, args) = if session.kind == WorkspaceSessionKind::Sftp {
                        ("sftp", plan.sftp_arguments.clone())
                    } else {
                        ("ssh", plan.ssh_arguments.clone())
                    };
                    commands.push(askpass::command(program, args, &plan)?);
                }
                Ok::<_, String>(commands)
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = app.update(cx, |app, cx| match result {
                    Ok(commands) => {
                        app.desktop.message = None;
                        app.start_workspace(ws, commands, window, cx);
                    }
                    Err(error) => {
                        app.desktop.message = Some(format!("Workspace could not start: {error}"));
                        cx.notify();
                    }
                });
            });
        })
        .detach();
    }

    fn start_workspace(
        &mut self,
        ws: SavedWorkspace,
        commands: Vec<TerminalCommand>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if ws.sessions.iter().any(|session| {
            session.kind != WorkspaceSessionKind::Local && session.destination.is_none()
        }) {
            self.desktop.message = Some("This workspace has a remote session without a destination. Edit the workspace and select its hosts again.".into());
            cx.notify();
            return;
        }
        let tab_id = self.next_tab_id.wrapping_add(1);
        self.next_tab_id = tab_id;

        let mut panes = Vec::new();
        let session_specs = if ws.sessions.is_empty() {
            vec![WorkspaceSession {
                name: ws.name.clone(),
                kind: WorkspaceSessionKind::Local,
                destination: None,
                host_id: None,
                startup_command: None,
                working_directory: None,
                pane_index: 0,
                excluded_from_broadcast: false,
            }]
        } else {
            ws.sessions.clone()
        };

        let mut events_to_bind = Vec::new();
        let mut first_rd_state = None;

        self.broadcast_manager = BroadcastManager::new(session_specs.len());

        for (idx, sess) in session_specs.iter().enumerate() {
            let pane_id = self.next_tab_id.wrapping_add(1);
            self.next_tab_id = pane_id;

            let rd_proto = match sess.kind {
                WorkspaceSessionKind::Vnc => Some(RemoteDesktopProtocol::Vnc),
                WorkspaceSessionKind::Rdp => Some(RemoteDesktopProtocol::Rdp),
                _ => None,
            };

            let kind = match sess.kind {
                WorkspaceSessionKind::Local => SessionKind::Local,
                WorkspaceSessionKind::Ssh => SessionKind::Ssh,
                WorkspaceSessionKind::Sftp => SessionKind::Sftp,
                WorkspaceSessionKind::Vnc | WorkspaceSessionKind::Rdp => SessionKind::RemoteDesktop,
            };

            let session = if sess.kind == WorkspaceSessionKind::Local {
                Some(Session::start(TerminalCommand::default_shell()))
            } else if sess.kind == WorkspaceSessionKind::Ssh
                || sess.kind == WorkspaceSessionKind::Sftp
            {
                Some(Session::start(commands[idx].clone()))
            } else {
                None
            };
            let events = session.as_ref().map(|s| s.events.clone());

            if let Some(proto) = rd_proto {
                let matched_host = match &self.hosts {
                    HostState::Loaded(hosts) => hosts.iter().find(|h| {
                        sess.host_id.map(|id| h.id == id).unwrap_or(false)
                            || sess
                                .destination
                                .as_ref()
                                .map(|d| &h.destination == d)
                                .unwrap_or(false)
                            || h.name == sess.name
                    }),
                    _ => None,
                };

                let (target, width, height, color_depth, view_only, username, domain, tls_strict) =
                    if let Some(host) = matched_host {
                        match proto {
                            RemoteDesktopProtocol::Vnc => {
                                let cfg = host.vnc_config().unwrap_or_default();
                                (
                                    format!("{}:{}", cfg.host, cfg.port),
                                    cfg.width as u32,
                                    cfg.height as u32,
                                    cfg.color_depth,
                                    cfg.view_only,
                                    None,
                                    None,
                                    true,
                                )
                            }
                            RemoteDesktopProtocol::Rdp => {
                                let cfg = host.rdp_config().unwrap_or_default();
                                (
                                    format!("{}:{}", cfg.host, cfg.port),
                                    cfg.screen_width as u32,
                                    cfg.screen_height as u32,
                                    cfg.color_depth,
                                    cfg.view_only,
                                    cfg.username.or_else(|| {
                                        if host.destination.username().is_empty() {
                                            None
                                        } else {
                                            Some(host.destination.username().to_string())
                                        }
                                    }),
                                    cfg.domain,
                                    cfg.tls_strict,
                                )
                            }
                        }
                    } else if let Some(dest) = &sess.destination {
                        (
                            format!("{}:{}", dest.host(), dest.port().get()),
                            1920,
                            1080,
                            32,
                            false,
                            if dest.username().is_empty() {
                                None
                            } else {
                                Some(dest.username().to_string())
                            },
                            None,
                            true,
                        )
                    } else {
                        (sess.name.clone(), 1920, 1080, 32, false, None, None, true)
                    };

                let password_input = cx.new(|cx| TextInput::new(cx, "", "Password…").masked());
                let rd = RemoteDesktopState {
                    host_name: sess.name.clone(),
                    target,
                    protocol: proto,
                    status: RemoteDesktopStatus::NotConnected,
                    status_message: Some(format!(
                        "Restored session to {} via {}",
                        sess.name,
                        match proto {
                            RemoteDesktopProtocol::Vnc => "RFB / VNC",
                            RemoteDesktopProtocol::Rdp => "RDP",
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
                if first_rd_state.is_none() {
                    first_rd_state = Some(rd);
                }
            }

            let search_input = cx.new(|cx| TextInput::new(cx, "", "Search output…"));
            let pane = TerminalPane {
                id: pane_id,
                name: sess.name.clone(),
                kind,
                scroll: gpui::ScrollHandle::new(),
                terminal: {
                    let mut terminal = Terminal::new(24, 80);
                    terminal.set_scrollback_limit(self.desktop.preferences.scrollback_lines);
                    terminal
                },
                session,
                message: None,
                search_open: false,
                search_input,
                search_query: String::new(),
                search_matches: Vec::new(),
                current_match_idx: None,
            };

            if sess.excluded_from_broadcast {
                self.broadcast_manager.exclude_pane(idx);
            }
            panes.push(pane);
            if let Some(events) = events {
                events_to_bind.push((pane_id, events));
            }
        }

        let split = match WorkspaceLayout::parse(&ws.layout).unwrap_or(WorkspaceLayout::Single) {
            WorkspaceLayout::SplitVertical => Some(SplitDirection::Vertical),
            WorkspaceLayout::SplitHorizontal => Some(SplitDirection::Horizontal),
            WorkspaceLayout::DualGrid => Some(SplitDirection::Grid),
            WorkspaceLayout::Single => None,
        };

        let tab = SessionTab {
            id: tab_id,
            name: ws.name,
            kind: panes.first().map(|p| p.kind).unwrap_or(SessionKind::Local),
            panes,
            active_pane_idx: 0,
            split,
            sftp_state: None,
            remote_desktop_state: first_rd_state,
            multiplayer: None,
        };

        self.tabs.push(tab);
        self.page = None;
        self.active_tab_idx = Some(self.tabs.len() - 1);

        window.focus(&self.focus);
        cx.notify();

        for (pane_id, events) in events_to_bind {
            self.bind_pane_events(tab_id, pane_id, events, cx);
        }
    }
}

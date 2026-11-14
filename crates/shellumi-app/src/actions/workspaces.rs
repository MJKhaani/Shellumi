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
                    SessionKind::Serial | SessionKind::Local | SessionKind::RemoteDesktop => {
                        WorkspaceSessionKind::Local
                    }
                    SessionKind::Sftp => WorkspaceSessionKind::Sftp,
                };
                let host = match &self.hosts {
                    HostState::Loaded(hosts) => hosts.iter().find(|host| host.name == pane.name),
                    _ => None,
                };
                WorkspaceSession {
                    name: pane.name.clone(),
                    kind,
                    destination: host.map(|host| host.destination.clone()),
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

        self.broadcast_manager = BroadcastManager::new(session_specs.len());

        for (idx, sess) in session_specs.iter().enumerate() {
            let pane_id = self.next_tab_id.wrapping_add(1);
            self.next_tab_id = pane_id;

            let kind = match sess.kind {
                WorkspaceSessionKind::Local => SessionKind::Local,
                WorkspaceSessionKind::Ssh => SessionKind::Ssh,
                WorkspaceSessionKind::Sftp => SessionKind::Sftp,
            };
            let cmd = if sess.kind == WorkspaceSessionKind::Local {
                TerminalCommand::default_shell()
            } else {
                commands[idx].clone()
            };

            let session = Session::start(cmd);
            let events = session.events.clone();
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
                session: Some(session),
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
            events_to_bind.push((pane_id, events));
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
            remote_desktop_state: None,
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

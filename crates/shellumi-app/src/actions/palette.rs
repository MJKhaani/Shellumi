//! Palette actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_command_palette(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let mut registry = PaletteRegistry::with_static_actions();

        // 1. Saved hosts
        if let HostState::Loaded(hosts) = &self.hosts {
            for h in hosts {
                let dest_str = format!(
                    "SSH: {}@{}:{}",
                    h.destination.username(),
                    h.destination.host(),
                    h.destination.port()
                );
                registry.register(
                    PaletteCommand::new(
                        format!("host:{}", h.id),
                        format!("Connect to {}", h.name),
                        dest_str,
                        PaletteCategory::Host,
                        PaletteAction::ConnectHost {
                            host_id: h.id,
                            host_name: h.name.clone(),
                        },
                    )
                    .with_keywords(h.tags.clone()),
                );

                registry.register(
                    PaletteCommand::new(
                        format!("sftp:{}", h.id),
                        format!("Open SFTP: {}", h.name),
                        format!("SFTP file transfer session with {}", h.name),
                        PaletteCategory::Sftp,
                        PaletteAction::ConnectSftp {
                            host_id: h.id,
                            host_name: h.name.clone(),
                        },
                    )
                    .with_keywords(h.tags.clone()),
                );
            }
        }

        // 2. Saved snippets
        for s in &self.snippets {
            let desc = s.description.clone().unwrap_or_else(|| s.command.clone());
            registry.register(
                PaletteCommand::new(
                    format!("snippet:{}", s.id),
                    format!("Run Snippet: {}", s.name),
                    desc,
                    PaletteCategory::Snippet,
                    PaletteAction::ExecuteSnippet {
                        snippet_id: s.id,
                        snippet_name: s.name.clone(),
                    },
                )
                .with_keywords(s.tags.clone()),
            );
            registry.register(PaletteCommand::batch_from_snippet(s.id, &s.name));
        }

        // 3. Tab switching
        for (i, tab) in self.tabs.iter().enumerate() {
            registry.register(
                PaletteCommand::new(
                    format!("tab:{}", tab.id),
                    format!("Switch to Tab {}: {}", i + 1, tab.name),
                    format!("Bring session '{}' to foreground", tab.name),
                    PaletteCategory::Navigation,
                    PaletteAction::SwitchToTab(i),
                )
                .with_shortcut(format!("Cmd+{}", (i + 2).min(9))),
            );
        }

        // 4. Saved workspaces
        for w in &self.workspaces {
            registry.register(
                PaletteCommand::workspace_from_saved(w.id, &w.name, w.layout.as_str())
                    .with_keywords(w.tags.clone()),
            );
        }

        let input =
            cx.new(|cx| TextInput::new(cx, "", "Type a command, host, snippet, or action…"));
        self.command_palette = Some(CommandPaletteState {
            input,
            registry,
            selected_idx: 0,
        });
        cx.notify();
    }

    pub(crate) fn dismiss_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette = None;
        cx.notify();
    }

    pub(crate) fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_palette.is_some() {
            self.dismiss_command_palette(cx);
        } else {
            self.open_command_palette(window, cx);
        }
    }

    pub(crate) fn command_palette_select_next(&mut self, cx: &mut Context<Self>) {
        if let Some(palette) = &mut self.command_palette {
            let query = palette.input.read(cx).value();
            let count = palette.registry.search(query).len();
            if count > 0 {
                palette.selected_idx = (palette.selected_idx + 1).min(count - 1);
                cx.notify();
            }
        }
    }

    pub(crate) fn command_palette_select_prev(&mut self, cx: &mut Context<Self>) {
        if let Some(palette) = &mut self.command_palette {
            palette.selected_idx = palette.selected_idx.saturating_sub(1);
            cx.notify();
        }
    }

    pub(crate) fn execute_command_palette_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(palette) = &self.command_palette else {
            return;
        };
        let query = palette.input.read(cx).value();
        let matches = palette.registry.search(query);
        let action = matches
            .get(palette.selected_idx)
            .map(|m| m.command.action.clone());
        if let Some(action) = action {
            self.execute_palette_action(action, window, cx);
        } else {
            self.dismiss_command_palette(cx);
        }
    }

    pub(crate) fn execute_palette_action(
        &mut self,
        action: PaletteAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_palette = None;
        cx.notify();

        match action {
            PaletteAction::ConnectHost { host_id, .. } => {
                if let HostState::Loaded(hosts) = &self.hosts {
                    if let Some(host) = hosts.iter().find(|h| h.id == host_id).cloned() {
                        self.connect(&host, window, cx);
                    }
                }
            }
            PaletteAction::ConnectSftp { host_id, .. } => {
                if let HostState::Loaded(hosts) = &self.hosts {
                    if let Some(host) = hosts.iter().find(|h| h.id == host_id).cloned() {
                        self.connect_sftp(&host, window, cx);
                    }
                }
            }
            PaletteAction::ExecuteSnippet { snippet_id, .. } => {
                self.insert_snippet(snippet_id, true, cx);
            }
            PaletteAction::ExecuteBatchSnippet { snippet_id, .. } => {
                self.open_batch_modal(snippet_id, cx);
            }
            PaletteAction::QuickConnect => {
                self.active_tab_idx = None;
                window.focus(&self.focus);
                cx.notify();
            }
            PaletteAction::TriggerSync => {
                self.trigger_sync(cx);
            }
            PaletteAction::OpenConflictResolver => {
                self.trigger_sync(cx);
            }
            PaletteAction::NewLocalTerminal => {
                self.open_local_terminal(window, cx);
            }
            PaletteAction::SplitVertical => {
                self.split_pane(SplitDirection::Vertical, window, cx);
            }
            PaletteAction::SplitHorizontal => {
                self.split_pane(SplitDirection::Horizontal, window, cx);
            }
            PaletteAction::CloseActiveTab => {
                if let Some(idx) = self.active_tab_idx {
                    if let Some(tab) = self.tabs.get(idx) {
                        let id = tab.id;
                        self.close_tab(id, window, cx);
                    }
                }
            }
            PaletteAction::ToggleBroadcastInput => {
                let total = self
                    .active_tab_idx
                    .and_then(|idx| self.tabs.get(idx))
                    .map(|t| t.panes.len())
                    .unwrap_or(0);
                self.broadcast_manager.set_total_panes(total);
                let on = self.broadcast_manager.toggle();
                if !on {
                    self.broadcast_warning = None;
                }
                cx.notify();
            }
            PaletteAction::SaveCurrentLayoutAsWorkspace => {
                self.save_current_layout_as_workspace(cx);
            }
            PaletteAction::LoadWorkspace { workspace_id, .. } => {
                self.load_workspace(workspace_id, window, cx);
            }
            PaletteAction::OpenSnippetsDrawer => {
                self.snippets_panel_open = true;
                cx.notify();
            }
            PaletteAction::AddNewHost => {
                self.active_tab_idx = None;
                self.edit_host(None, cx);
            }
            PaletteAction::AddNewSnippet => {
                self.snippets_panel_open = true;
                cx.notify();
            }
            PaletteAction::AddNewTunnel => {
                self.open_tunnel_modal(cx);
            }
            PaletteAction::NavigateToHosts => {
                self.active_tab_idx = None;
                cx.notify();
            }
            PaletteAction::SwitchToTab(tab_idx) => {
                if tab_idx < self.tabs.len() {
                    self.active_tab_idx = Some(tab_idx);
                    window.focus(&self.focus);
                    cx.notify();
                }
            }
            PaletteAction::OpenTeamCollaboration => {
                self.open_team_modal(cx);
            }
            PaletteAction::ToggleMultiplayerCollaboration => {
                self.toggle_multiplayer_on_active_tab(cx);
            }
            PaletteAction::OpenInfrastructureManager | PaletteAction::SyncCloudInfrastructure => {
                self.open_infra_modal(cx);
            }
            PaletteAction::ImportAnsibleInventory => {
                self.open_infra_modal(cx);
                if let Some(ref mut m) = self.infra_modal {
                    m.active_tab = InfraModalTab::Ansible;
                }
            }
            PaletteAction::ExportAnsibleInventory => {
                self.export_ansible_inventory_file(false, cx);
            }
            PaletteAction::RunDiagnostics => {
                self.open_diagnostics_modal(cx);
            }
            PaletteAction::RunBenchmarks => {
                self.open_benchmarks_modal(cx);
            }
            PaletteAction::OpenMobileSimulator => {
                self.open_mobile_modal(cx);
            }
            PaletteAction::OpenShellIntegrationModal => {
                self.open_shell_integration_modal(cx);
            }
            PaletteAction::OpenSessionRecordings => {
                self.open_session_replay_modal(cx);
            }
        }
    }
}

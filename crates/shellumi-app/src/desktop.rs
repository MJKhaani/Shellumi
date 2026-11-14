//! Desktop pages and editors; persistence is delegated to shellumi-storage.
use super::*;
use shellumi_storage::desktop::{Command, Preferences};

mod backup;
mod files;
mod overview;
mod public_inventory;
mod sessions;
mod settings;
mod snippets;
mod workspaces;

pub(super) struct DesktopUi {
    pub local_entries: Vec<FileEntry>,
    pub file_path: Entity<TextInput>,
    pub groups: Vec<shellumi_core::HostGroup>,
    pub preferences: Preferences,
    pub public_records: Vec<shellumi_storage::desktop::PublicSshRecord>,
    pub public_selected: usize,
    pub busy: bool,
    pub message: Option<String>,
    pub selected_snippet: Option<i64>,
    pub snippet_tag: Option<String>,
    pub search: Entity<TextInput>,
    pub editor: Option<Editor>,
    pub confirm: Option<(String, Command)>,
    pub settings_section: &'static str,
}
pub(super) struct Editor {
    pub kind: EditorKind,
    pub id: Option<i64>,
    pub name: Entity<TextInput>,
    pub description: Entity<TextInput>,
    pub tags: Entity<TextInput>,
    pub lines: Vec<Entity<TextInput>>,
    pub layout: &'static str,
    pub host_ids: Vec<i64>,
    pub local_shell: bool,
    pub identity_id: Option<i64>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorKind {
    Snippet,
    Workspace,
    Import,
    ImportHosts,
    Group,
    ExportBackup,
    RestoreBackup,
}
impl DesktopUi {
    pub fn new(cx: &mut Context<Shellumi>) -> Self {
        let search = cx.new(|cx| TextInput::new(cx, "", "Search snippets…"));
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        Self {
            local_entries: Vec::new(),
            file_path: cx.new(|cx| {
                TextInput::new(
                    cx,
                    &std::env::var("HOME")
                        .or_else(|_| std::env::var("USERPROFILE"))
                        .unwrap_or_else(|_| "/".into()),
                    "Local directory",
                )
            }),
            groups: Vec::new(),
            preferences: Preferences::default(),
            public_records: Vec::new(),
            public_selected: 0,
            busy: false,
            message: None,
            selected_snippet: None,
            snippet_tag: None,
            search,
            editor: None,
            confirm: None,
            settings_section: "General",
        }
    }
}
fn panel() -> gpui::Div {
    div()
        .rounded(px(MacTheme::PANEL_RADIUS))
        .border_1()
        .border_color(rgb(MacTheme::SEPARATOR))
        .bg(rgb(MacTheme::SURFACE))
        .p_4()
}
fn muted(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .text_sm()
        .text_color(rgb(MacTheme::TEXT_SECONDARY))
        .child(text.into())
}
fn heading(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(MacTheme::TEXT_PRIMARY))
        .child(text.into())
}
fn button_base(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_1p5()
        .rounded(px(MacTheme::CONTROL_RADIUS))
        .border_1()
        .text_sm()
        .cursor_pointer()
        .child(text.into())
}
fn button(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    button_base(id, text)
        .border_color(rgb(MacTheme::SEPARATOR))
        .bg(rgb(0x323236))
        .text_color(rgb(MacTheme::TEXT_PRIMARY))
        .hover(|style| style.bg(rgb(MacTheme::SURFACE_HOVER)))
}
pub(crate) fn button_icon(
    id: impl Into<gpui::ElementId>,
    icon_name: &str,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    let icon_el = if let Some(p) = crate::resolve_icon_path(icon_name, false, 48) {
        div()
            .w(px(22.0))
            .h(px(22.0))
            .child(gpui::img(p).size_full())
            .into_any_element()
    } else {
        div().into_any_element()
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .py_1p5()
        .rounded(px(MacTheme::CONTROL_RADIUS))
        .border_1()
        .border_color(rgb(MacTheme::SEPARATOR))
        .bg(rgb(0x323236))
        .text_color(rgb(MacTheme::TEXT_PRIMARY))
        .text_sm()
        .cursor_pointer()
        .hover(|style| style.bg(rgb(MacTheme::SURFACE_HOVER)))
        .child(icon_el)
        .child(text.into())
}
struct IconTooltip(&'static str);

impl Render for IconTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(MacTheme::SURFACE_HOVER))
            .text_sm()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .child(self.0)
    }
}

fn icon_button(id: &'static str, icon: &str, hint: &'static str) -> gpui::Stateful<gpui::Div> {
    let mut button = div()
        .id(id)
        .w(px(36.0))
        .h(px(36.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(MacTheme::CONTROL_RADIUS))
        .border_1()
        .border_color(rgb(MacTheme::SEPARATOR))
        .bg(rgb(MacTheme::SURFACE))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(MacTheme::SURFACE_HOVER)))
        .tooltip(move |_, cx| cx.new(|_| IconTooltip(hint)).into());
    if let Some(path) = crate::resolve_icon_path(icon, false, 48) {
        button = button.child(
            div()
                .w(px(32.0))
                .h(px(32.0))
                .child(gpui::img(path).size_full()),
        );
    }
    button
}

fn primary(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    button_base(id, text)
        .border_color(rgb(MacTheme::ACCENT_BLUE))
        .bg(rgb(MacTheme::ACCENT_BLUE))
        .text_color(rgb(0xffffff))
        .hover(|style| {
            style
                .bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                .border_color(rgb(MacTheme::ACCENT_BLUE_HOVER))
        })
}
fn danger(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    button_base(id, text)
        .border_color(rgb(MacTheme::DANGER))
        .bg(rgb(MacTheme::DANGER))
        .text_color(rgb(0xffffff))
        .hover(|style| style.bg(rgb(0xff6961)).border_color(rgb(0xff6961)))
}
fn empty(title: &str, text: &str) -> gpui::Div {
    panel()
        .flex_1()
        .flex()
        .flex_col()
        .gap_3()
        .p_6()
        .child(heading(title.to_owned()))
        .child(muted(text.to_owned()))
}

impl Shellumi {
    pub(super) fn desktop_command(&mut self, command: Command, cx: &mut Context<Self>) {
        if self.desktop.busy {
            return;
        }
        let Some(path) = self.database.clone() else {
            self.desktop.message = Some("Local storage is unavailable.".into());
            cx.notify();
            return;
        };
        let refreshing = matches!(command, Command::Refresh);
        self.desktop.busy = true;
        self.desktop.message = None;
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(shellumi_storage::desktop::execute(&path, command))
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                app.desktop.busy = false;
                match result {
                    Ok(snapshot) => {
                        app.hosts = HostState::Loaded(snapshot.hosts);
                        app.desktop.groups = snapshot.groups;
                        app.snippets = snapshot.snippets;
                        app.workspaces = snapshot.workspaces;
                        app.identities = snapshot.identities;
                        app.desktop.preferences = snapshot.preferences;
                        for tab in &mut app.tabs {
                            for pane in &mut tab.panes {
                                pane.terminal
                                    .set_scrollback_limit(app.desktop.preferences.scrollback_lines);
                            }
                            if let Some(browser) = &mut tab.sftp_state {
                                browser.show_hidden = app.desktop.preferences.show_hidden_files;
                            }
                        }
                        if refreshing
                            && app.desktop.preferences.startup_overview
                            && app.tabs.is_empty()
                            && app.page.is_none()
                        {
                            app.page = Some("Overview");
                        }
                        if !refreshing
                            && (app.page == Some("Known Hosts") || app.page == Some("Keychain"))
                        {
                            app.refresh_public_inventory(cx);
                        }
                        if !refreshing && app.page == Some("SFTP Files") {
                            app.refresh_local_files(cx);
                        }
                        if !refreshing {
                            app.desktop.editor = None;
                            app.desktop.confirm = None;
                            app.desktop.message = Some("Saved locally.".into());
                        }
                    }
                    Err(error) => app.desktop.message = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn navigate_page(
        &mut self,
        page: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.team_modal = None;
        self.infra_modal = None;
        self.session_replay_modal = None;
        self.tunnel_modal = None;
        self.shell_integration_modal = None;
        self.snippet_modal = None;
        self.batch_modal = None;
        self.diagnostics_modal = None;
        self.command_palette = None;
        self.form = None;
        self.desktop.editor = None;
        self.desktop.confirm = None;
        self.desktop.message = None;
        self.active_tab_idx = None;
        self.page = if page == "Hosts" {
            None
        } else if page == "Logs" {
            Some("Session Logs")
        } else {
            Some(page)
        };
        if page == "Terminal" {
            if let Some(index) = (!self.tabs.is_empty()).then_some(0) {
                self.select_tab(Some(index), window, cx);
            }
        }
        if page == "Keychain" || page == "Known Hosts" {
            self.refresh_public_inventory(cx);
        }
        if page == "SFTP Files" {
            self.refresh_sftp_pane(true, cx);
            self.refresh_sftp_pane(false, cx);
        }
        cx.notify();
    }
    fn edit_library_item(&mut self, kind: EditorKind, id: Option<i64>, cx: &mut Context<Self>) {
        let snippet = id.and_then(|id| self.snippets.iter().find(|item| item.id == id));
        let workspace = id.and_then(|id| self.workspaces.iter().find(|item| item.id == id));
        let (name, description, tags, command) = if kind == EditorKind::Snippet {
            snippet
                .map(|s| {
                    (
                        s.name.as_str(),
                        s.description.as_deref().unwrap_or(""),
                        s.tags.join(", "),
                        s.command.clone(),
                    )
                })
                .unwrap_or(("", "", String::new(), String::new()))
        } else if kind == EditorKind::Workspace {
            workspace
                .map(|w| {
                    (
                        w.name.as_str(),
                        w.description.as_deref().unwrap_or(""),
                        w.tags.join(", "),
                        String::new(),
                    )
                })
                .unwrap_or(("", "", String::new(), String::new()))
        } else {
            ("", "", String::new(), String::new())
        };
        let host_ids = workspace
            .map(|w| w.sessions.iter().filter_map(|s| s.host_id).collect())
            .unwrap_or_default();
        let layout = workspace
            .and_then(|w| WorkspaceLayout::parse(&w.layout))
            .unwrap_or(WorkspaceLayout::Single)
            .as_str();
        let local_shell = workspace.is_none_or(|w| {
            w.sessions
                .iter()
                .any(|s| s.kind == WorkspaceSessionKind::Local)
        });
        self.desktop.editor = Some(Editor {
            kind,
            id,
            name: cx.new(|cx| TextInput::new(cx, name, "Display name")),
            description: cx.new(|cx| {
                let input = TextInput::new(cx, description, "Description");
                if matches!(kind, EditorKind::ExportBackup | EditorKind::RestoreBackup) {
                    input.masked()
                } else {
                    input
                }
            }),
            tags: cx.new(|cx| TextInput::new(cx, &tags, "Tags, separated by commas")),
            lines: command
                .split('\n')
                .map(|line| cx.new(|cx| TextInput::new(cx, line, "Command or script line")))
                .collect(),
            layout,
            host_ids,
            local_shell,
            identity_id: None,
        });
        self.desktop.message = None;
        cx.notify();
    }
    fn save_library_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.desktop.editor else {
            return;
        };
        let name = editor.name.read(cx).value().trim().to_owned();
        let description = if matches!(
            editor.kind,
            EditorKind::ExportBackup | EditorKind::RestoreBackup
        ) {
            editor.description.read(cx).value().to_owned()
        } else {
            editor.description.read(cx).value().trim().to_owned()
        };
        let tags: Vec<String> = editor
            .tags
            .read(cx)
            .value()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let command = editor
            .lines
            .iter()
            .map(|line| line.read(cx).value().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        let action = match editor.kind {
            EditorKind::Snippet => {
                match shellumi_core::Snippet::new(name, command, Some(description), &tags) {
                    Ok(snippet) => Command::SaveSnippet {
                        id: editor.id,
                        snippet,
                    },
                    Err(error) => {
                        self.desktop.message = Some(error.to_string());
                        cx.notify();
                        return;
                    }
                }
            }
            EditorKind::Workspace => {
                if name.is_empty() {
                    self.desktop.message = Some("Give this workspace a name.".into());
                    cx.notify();
                    return;
                }
                let mut sessions = Vec::new();
                if editor.local_shell {
                    sessions.push(WorkspaceSession {
                        name: "Local shell".into(),
                        kind: WorkspaceSessionKind::Local,
                        destination: None,
                        host_id: None,
                        startup_command: None,
                        working_directory: None,
                        pane_index: 0,
                        excluded_from_broadcast: false,
                    });
                }
                if let HostState::Loaded(hosts) = &self.hosts {
                    for host in hosts
                        .iter()
                        .filter(|host| editor.host_ids.contains(&host.id))
                    {
                        sessions.push(WorkspaceSession {
                            name: host.name.clone(),
                            kind: WorkspaceSessionKind::Ssh,
                            destination: Some(host.destination.clone()),
                            host_id: Some(host.id),
                            startup_command: None,
                            working_directory: None,
                            pane_index: sessions.len(),
                            excluded_from_broadcast: false,
                        });
                    }
                }
                if sessions.is_empty() {
                    self.desktop.message = Some("Select at least one session.".into());
                    cx.notify();
                    return;
                }
                Command::SaveWorkspace {
                    id: editor.id,
                    name,
                    description,
                    layout: editor.layout.into(),
                    sessions,
                }
            }
            EditorKind::Import => Command::ImportSnippets(command),
            EditorKind::ImportHosts => Command::ImportHosts {
                config: command,
                default_user: name,
            },
            EditorKind::ExportBackup | EditorKind::RestoreBackup => {
                if name.trim().is_empty() {
                    self.desktop.message = Some("Enter a backup file path.".into());
                    cx.notify();
                    return;
                }
                let passphrase = shellumi_core::SecretString::new(description);
                if editor.kind == EditorKind::ExportBackup {
                    Command::ExportBackup {
                        path: name.into(),
                        passphrase,
                    }
                } else {
                    Command::RestoreBackup {
                        path: name.into(),
                        passphrase,
                    }
                }
            }
            EditorKind::Group => {
                let port = command.trim();
                let port = if port.is_empty() {
                    None
                } else {
                    match port.parse::<u16>() {
                        Ok(port) => Some(port),
                        Err(_) => {
                            self.desktop.message =
                                Some("Port must be a number from 1 to 65535.".into());
                            cx.notify();
                            return;
                        }
                    }
                };
                match shellumi_core::HostGroup::new(
                    &name,
                    if description.is_empty() {
                        None
                    } else {
                        Some(description.as_str())
                    },
                    port,
                    &tags,
                ) {
                    Ok(group) => Command::SaveGroup(group.with_identity_id(editor.identity_id)),
                    Err(error) => {
                        self.desktop.message = Some(error.to_string());
                        cx.notify();
                        return;
                    }
                }
            }
        };
        self.desktop_command(action, cx);
    }
    fn change_preference(&mut self, key: &'static str, delta: i32, cx: &mut Context<Self>) {
        let mut preferences = self.desktop.preferences.clone();
        match key {
            "font" => {
                preferences.terminal_font_size =
                    (i32::from(preferences.terminal_font_size) + delta).clamp(10, 24) as u16
            }
            "ui" => {
                preferences.ui_font_size =
                    (i32::from(preferences.ui_font_size) + delta).clamp(11, 18) as u16
            }
            "scrollback" => {
                preferences.scrollback_lines =
                    (preferences.scrollback_lines as i32 + delta).clamp(100, 50000) as usize
            }
            "hidden" => preferences.show_hidden_files = !preferences.show_hidden_files,
            "startup" => preferences.startup_overview = !preferences.startup_overview,
            "reset" => preferences = Preferences::default(),
            _ => return,
        }
        self.desktop_command(Command::SavePreferences(preferences), cx);
    }
    pub(super) fn render_desktop_page(
        &self,
        page: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        if page == "SFTP Files" {
            return self.render_sftp_redesigned_dual_pane(cx);
        }
        let (view, is_full_view) = match page {
            "Keychain" | "Known Hosts" => (self.public_inventory_page(page, cx), true),
            "Snippets" => (self.snippets_page(cx), true),
            "Settings" => (self.settings_page(cx), true),
            "Workspaces" => (self.workspaces_page(cx), true),
            "Port Forwarding" | "Session Logs" => (self.tools_page(page, cx), true),
            "Overview" => (self.overview_page(cx), false),
            "Terminal" => (self.sessions_page(false, cx), false),
            _ => (self.tools_page(page, cx), false),
        };
        let mut body = if is_full_view {
            view
        } else {
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .h_full()
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(
                    div()
                        .id("page-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p_6()
                        .child(view),
                )
        };
        if self.desktop.busy {
            body = body.child(div().px_6().py_2().text_sm().child("Saving…"));
        }
        if let Some(message) = &self.desktop.message {
            body = body.child(
                div()
                    .px_6()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(MacTheme::WARNING))
                    .child(message.clone()),
            );
        }
        body
    }
    fn tools_page(&self, page: &'static str, cx: &mut Context<Self>) -> gpui::Div {
        if page == "Vaults" {
            if let Some(modal) = &self.team_modal {
                return div().child(self.render_team_modal(modal, cx));
            }
        }
        if page == "Integrations" {
            if let Some(modal) = &self.infra_modal {
                return div().child(self.render_infra_modal(modal, cx));
            }
        }
        match page {
            "Port Forwarding" => {
                if let Some(modal) = &self.tunnel_modal {
                    return div().child(self.render_tunnel_modal(modal, cx));
                }
                let action_bar = div()
                    .id("port-forwarding-action-bar")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .child(heading("Port Forwarding"))
                    .child(
                        button_icon("new-tunnel-btn", "port_forwarding", "+ New Tunnel")
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .border_color(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.open_tunnel_modal(cx);
                            })),
                    );
                let content_scroll = div()
                    .id("port-forwarding-content-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_6()
                    .child(
                        empty("Port forwarding", "Create and manage saved SSH tunnel rules.")
                            .child(primary("open-tunnels-page", "Manage tunnels").on_click(cx.listener(|app, _, _, cx| app.open_tunnel_modal(cx)))),
                    );
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .child(action_bar)
                    .child(content_scroll)
            }
            "Session Logs" => {
                if let Some(modal) = &self.session_replay_modal {
                    return div().child(self.render_session_replay_modal(modal, cx));
                }
                let action_bar = div()
                    .id("session-logs-action-bar")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .child(heading("Session Recordings & Logs"))
                    .child(
                        button_icon("open-recordings-btn", "log", "Open recordings")
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .border_color(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.open_session_replay_modal(cx);
                            })),
                    );
                let content_scroll = div()
                    .id("session-logs-content-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_6()
                    .child(
                        empty("Session recordings", "Recordings are opt-in and can contain sensitive terminal output.")
                            .child(primary("open-recordings-page", "Open recordings").on_click(cx.listener(|app, _, _, cx| app.open_session_replay_modal(cx)))),
                    );
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .child(action_bar)
                    .child(content_scroll)
            }
            "Vaults"=>empty("Local vault records","Manage vault metadata and local team membership. Network sharing is not connected.").child(primary("vault-records","Manage vaults").on_click(cx.listener(|app,_,_,cx|app.open_team_modal(cx)))),
            "Collaboration"=>empty("Session access","Network terminal sharing is not implemented. Local membership records are available under Vaults."),
            "Integrations"=>empty("Infrastructure inventory","Manage provider records and inspect inventory with the existing infrastructure tools.").child(primary("integration-records","Manage infrastructure").on_click(cx.listener(|app,_,_,cx|app.open_infra_modal(cx)))),
            "Google Drive Sync" => self.backup_page(cx),
            "Keychain"|"Known Hosts"=>self.public_inventory_page(page,cx),
            _=>empty("Workspace","Select a section from the navigation."),
        }
    }
    pub(super) fn desktop_overlays(
        &self,
        mut root: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        if let Some(editor) = &self.desktop.editor {
            let title = match editor.kind {
                EditorKind::Snippet => {
                    if editor.id.is_some() {
                        "Edit snippet"
                    } else {
                        "New snippet"
                    }
                }
                EditorKind::Workspace => {
                    if editor.id.is_some() {
                        "Edit workspace"
                    } else {
                        "New workspace"
                    }
                }
                EditorKind::Import => "Import snippet package",
                EditorKind::ImportHosts => "Import OpenSSH config",
                EditorKind::Group => "Group settings",
                EditorKind::ExportBackup => "Export encrypted backup",
                EditorKind::RestoreBackup => "Restore encrypted backup",
            };
            let mut body = div()
                .id("library-editor-scroll")
                .max_h(px(480.0))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_3();
            if editor.kind == EditorKind::Snippet || editor.kind == EditorKind::Workspace {
                body = body
                    .child(host_form_field("Display name", editor.name.clone()))
                    .child(host_form_field("Description", editor.description.clone()));
            }
            if matches!(
                editor.kind,
                EditorKind::ExportBackup | EditorKind::RestoreBackup
            ) {
                body=body.child(host_form_field("Backup file path",editor.name.clone())).child(host_form_field("Backup passphrase",editor.description.clone())).child(muted(if editor.kind==EditorKind::ExportBackup{"Choose a new file path and a passphrase of at least 12 characters. Remember the passphrase; it cannot be recovered."}else{"Restoring merges backup records into this local database. Decryption errors leave the database unchanged."}));
            } else if editor.kind == EditorKind::Group {
                body = body
                    .child(host_form_field("Group name", editor.name.clone()))
                    .child(host_form_field(
                        "Default username (optional)",
                        editor.description.clone(),
                    ))
                    .child(host_form_field(
                        "Default port (optional)",
                        editor.lines[0].clone(),
                    ))
                    .child(host_form_field("Inherited tags", editor.tags.clone()))
                    .child(self.identity_picker(editor.identity_id, true, cx));
            } else if editor.kind == EditorKind::Workspace {
                let mut layouts = div().flex().gap_2();
                for layout in ["single", "vertical", "horizontal", "grid"] {
                    layouts = layouts.child(
                        button(gpui::SharedString::from(format!("layout-{layout}")), layout)
                            .bg(if editor.layout == layout {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_color(if editor.layout == layout {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(move |app, _, _, cx| {
                                if let Some(editor) = &mut app.desktop.editor {
                                    editor.layout = layout;
                                }
                                cx.notify();
                            })),
                    );
                }
                body = body.child(heading("Layout")).child(layouts);
                body = body.child(
                    button(
                        "workspace-local",
                        if editor.local_shell {
                            "✓ Local shell"
                        } else {
                            "Local shell"
                        },
                    )
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(editor) = &mut app.desktop.editor {
                            editor.local_shell = !editor.local_shell;
                        }
                        cx.notify();
                    })),
                );
                if let HostState::Loaded(hosts) = &self.hosts {
                    for host in hosts.iter().filter(|host| host.is_ssh()) {
                        let id = host.id;
                        body = body.child(
                            button(
                                ("workspace-host", id as u64),
                                format!(
                                    "{} {}",
                                    if editor.host_ids.contains(&id) {
                                        "✓"
                                    } else {
                                        "+"
                                    },
                                    host.name
                                ),
                            )
                            .on_click(cx.listener(
                                move |app, _, _, cx| {
                                    if let Some(editor) = &mut app.desktop.editor {
                                        if editor.host_ids.contains(&id) {
                                            editor.host_ids.retain(|host| *host != id);
                                        } else {
                                            editor.host_ids.push(id);
                                        }
                                    }
                                    cx.notify();
                                },
                            )),
                        );
                    }
                }
            } else {
                if editor.kind == EditorKind::ImportHosts {
                    body=body.child(host_form_field("Default username for entries without User",editor.name.clone())).child(muted("Only named hosts and connection destinations are imported; wildcard blocks are ignored."));
                }
                if editor.kind == EditorKind::Snippet {
                    body = body.child(host_form_field("Tags", editor.tags.clone()));
                }
                body = body.child(
                    div()
                        .flex()
                        .justify_between()
                        .child(heading(if editor.kind == EditorKind::Import {
                            "JSON package"
                        } else {
                            "Script · {{variable}} or {{variable:-default}}"
                        }))
                        .child(
                            button("paste-editor-script", "Paste from clipboard").on_click(
                                cx.listener(|app, _, _, cx| {
                                    if let Some(text) =
                                        cx.read_from_clipboard().and_then(|item| item.text())
                                    {
                                        if let Some(editor) = &mut app.desktop.editor {
                                            editor.lines = text
                                                .split('\n')
                                                .map(|line| {
                                                    cx.new(|cx| {
                                                        TextInput::new(cx, line, "Script line")
                                                    })
                                                })
                                                .collect();
                                        }
                                    }
                                    cx.notify();
                                }),
                            ),
                        ),
                );
                for (index, line) in editor.lines.iter().enumerate() {
                    body = body.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(24.0))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child((index + 1).to_string()),
                            )
                            .child(div().flex_1().child(line.clone()))
                            .child(button(("remove-script-line", index), "−").on_click(
                                cx.listener(move |app, _, _, cx| {
                                    if let Some(editor) = &mut app.desktop.editor {
                                        if editor.lines.len() > 1 {
                                            editor.lines.remove(index);
                                        }
                                    }
                                    cx.notify();
                                }),
                            )),
                    );
                }
                body = body.child(
                    button("add-script-line", "+ Add line").on_click(cx.listener(
                        |app, _, _, cx| {
                            let input = cx.new(|cx| TextInput::new(cx, "", "Script line"));
                            if let Some(editor) = &mut app.desktop.editor {
                                editor.lines.push(input);
                            }
                            cx.notify();
                        },
                    )),
                );
            }
            let mut modal = panel()
                .w(px(760.0))
                .max_w_full()
                .flex()
                .flex_col()
                .gap_4()
                .p_6()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child(title),
                )
                .child(body);
            if let Some(error) = &self.desktop.message {
                modal = modal.child(muted(error.clone()));
            }
            modal = modal.child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        button("cancel-library-edit", "Cancel").on_click(cx.listener(
                            |app, _, _, cx| {
                                app.desktop.editor = None;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        primary(
                            "save-library-edit",
                            if self.desktop.busy {
                                "Saving…"
                            } else {
                                "Save"
                            },
                        )
                        .on_click(cx.listener(|app, _, _, cx| app.save_library_editor(cx))),
                    ),
            );
            root = root.child(
                div()
                    .id("library-editor-overlay")
                    .occlude()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x030305dd))
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(modal),
            );
        }
        if let Some((message, command)) = &self.desktop.confirm {
            let command = command.clone();
            root = root.child(
                div()
                    .id("library-confirm-overlay")
                    .occlude()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x030305dd))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        panel()
                            .w(px(440.0))
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(heading(message.clone()))
                            .child(muted(
                                "Confirm this change to the local record. Existing terminal sessions are not affected.",
                            ))
                            .child(
                                div()
                                    .flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        button("cancel-library-delete", "Cancel").on_click(
                                            cx.listener(|app, _, _, cx| {
                                                app.desktop.confirm = None;
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .child(
                                        danger("confirm-library-delete", "Delete").on_click(
                                            cx.listener(move |app, _, _, cx| {
                                                app.desktop_command(command.clone(), cx)
                                            }),
                                        ),
                                    ),
                            ),
                    ),
            );
        }
        if let Some(modal) = &self.snippet_modal {
            root = root.child(self.render_snippet_modal(modal, cx));
        }
        if let Some(modal) = &self.batch_modal {
            root = root.child(self.render_batch_modal(modal, cx));
        }
        if let Some(palette) = &self.command_palette {
            root = root.child(self.render_command_palette(palette, cx));
        }
        if let Some(modal) = &self.team_modal {
            if self.page != Some("Vaults") {
                root = root.child(self.render_team_modal(modal, cx));
            }
        }
        if let Some(modal) = &self.infra_modal {
            if self.page != Some("Integrations") {
                root = root.child(self.render_infra_modal(modal, cx));
            }
        }
        if let Some(modal) = &self.diagnostics_modal {
            root = root.child(self.render_diagnostics_modal(modal, cx));
        }
        if let Some(modal) = &self.shell_integration_modal {
            root = root.child(self.render_shell_integration_modal(modal, cx));
        }
        if let Some(modal) = &self.session_replay_modal {
            if self.page != Some("Session Logs") {
                root = root.child(self.render_session_replay_modal(modal, cx));
            }
        }
        if let Some(modal) = &self.tunnel_modal {
            if self.page != Some("Port Forwarding") {
                root = root.child(self.render_tunnel_modal(modal, cx));
            }
        }
        if let Some(modal) = &self.create_vault_modal {
            root = root.child(self.render_create_vault_modal(modal, cx));
        }
        if let Some(modal) = &self.invite_modal {
            root = root.child(self.render_invite_team_modal(modal, cx));
        }
        if let Some(modal) = &self.identity_modal {
            root = root.child(self.render_identity_modal(modal, cx));
        }
        root
    }
}

impl Shellumi {
    pub(super) fn host_library_actions(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .gap_2()
            .child(
                icon_button("import-host-config", "import", "Import OpenSSH config").on_click(
                    cx.listener(|app, _, _, cx| {
                        app.edit_library_item(EditorKind::ImportHosts, None, cx)
                    }),
                ),
            )
            .child(
                icon_button("export-host-config", "export", "Copy OpenSSH config").on_click(
                    cx.listener(|app, _, _, cx| {
                        if let HostState::Loaded(hosts) = &app.hosts {
                            let entries = hosts
                                .iter()
                                .map(|host| {
                                    shellumi_core::HostConfigEntry::new(
                                        host.name.clone(),
                                        host.destination.clone(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                shellumi_core::export_ssh_config(&entries),
                            ));
                            app.desktop.message =
                                Some("OpenSSH config copied to clipboard.".into());
                            cx.notify();
                        }
                    }),
                ),
            )
    }
}

pub(super) fn terminal_dimensions(
    width: f32,
    height: f32,
    cell_width: f32,
    line_height: f32,
) -> (usize, usize) {
    let rows = (height.max(0.0) / line_height.max(1.0)).floor() as usize;
    let cols = (width.max(0.0) / cell_width.max(1.0)).floor() as usize;
    (rows.clamp(2, 500), cols.clamp(10, 500))
}

impl Shellumi {
    pub(super) fn identity_picker(
        &self,
        selected: Option<i64>,
        group: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut choices = div()
            .id(if group {
                "group-identities"
            } else {
                "host-identities"
            })
            .max_h(px(120.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        let mut entries = vec![(
            None,
            if group {
                "System OpenSSH defaults".to_string()
            } else {
                "Inherit group identity / OpenSSH defaults".to_string()
            },
        )];
        entries.extend(self.identities.iter().map(|identity| {
            let key_detail = identity
                .key_path
                .as_deref()
                .or(identity.certificate_path.as_deref())
                .unwrap_or(&identity.username);
            (
                Some(identity.id),
                format!(
                    "🔑 {} ({}) · {}",
                    identity.name,
                    identity.auth_method.as_str(),
                    key_detail
                ),
            )
        }));
        for (index, (id, label)) in entries.into_iter().enumerate() {
            choices = choices.child(
                button(
                    (
                        if group {
                            "group-identity"
                        } else {
                            "host-identity"
                        },
                        index,
                    ),
                    format!("{}{}", if selected == id { "✓ " } else { "" }, label),
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    if group {
                        if let Some(editor) = &mut app.desktop.editor {
                            editor.identity_id = id;
                        }
                    } else if let Some(form) = &mut app.form {
                        form.identity_id = id;
                    }
                    cx.notify();
                })),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(heading("Connection identity"))
            .child(muted("Identity username overrides host username. Saved secrets require a trusted host key."))
            .child(choices)
            .child(
                button(
                    if group {
                        "refresh-group-identities"
                    } else {
                        "refresh-host-identities"
                    },
                    "Refresh identities",
                )
                .on_click(cx.listener(|app, _, _, cx| app.desktop_command(Command::Refresh, cx))),
            )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn primary_button_registers_interaction_styles_once() {
        let _ = super::primary("regression-primary", "Save");
    }

    #[test]
    fn terminal_grid_fits_viewport_and_stays_bounded_during_resize() {
        assert_eq!(
            super::terminal_dimensions(800.0, 480.0, 10.0, 20.0),
            (24, 80)
        );
        assert_eq!(
            super::terminal_dimensions(400.0, 240.0, 10.0, 20.0),
            (12, 40)
        );
        assert_eq!(super::terminal_dimensions(-1.0, 0.0, 0.0, 0.0), (2, 10));
        assert_eq!(super::terminal_dimensions(1e9, 1e9, 1.0, 1.0), (500, 500));
    }
}

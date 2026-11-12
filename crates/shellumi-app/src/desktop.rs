//! Desktop pages and editors; persistence is delegated to shellumi-storage.
use super::*;
use shellumi_storage::desktop::{Command, Preferences};

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
    fn snippets_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let action_bar = div()
            .id("snippets-action-bar")
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(heading("Command snippets"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button_icon("import-snippets", "import", "Import package").on_click(
                            cx.listener(|app, _, _, cx| {
                                app.edit_library_item(EditorKind::Import, None, cx)
                            }),
                        ),
                    )
                    .child(
                        button_icon("new-snippet", "edit", "+ New snippet")
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .border_color(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.edit_library_item(EditorKind::Snippet, None, cx)
                            })),
                    ),
            );
        let mut packages = panel()
            .w(px(170.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        if let Some(p) = crate::resolve_icon_path("tags", false, 32) {
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        },
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("PACKAGES"),
                    ),
            )
            .child(
                button("all-snippets", "All snippets")
                    .bg(if self.desktop.snippet_tag.is_none() {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_color(if self.desktop.snippet_tag.is_none() {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.desktop.snippet_tag = None;
                        cx.notify();
                    })),
            );
        let mut tags: Vec<_> = self.snippets.iter().flat_map(|s| s.tags.iter()).collect();
        tags.sort();
        tags.dedup();
        for (index, tag) in tags.iter().enumerate() {
            let tag = (*tag).clone();
            let selected_tag = tag.clone();
            let is_sel = self.desktop.snippet_tag.as_deref() == Some(&tag);
            packages = packages.child(
                button(("snippet-tag", index), tag)
                    .bg(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.desktop.snippet_tag = Some(selected_tag.clone());
                        cx.notify();
                    })),
            );
        }
        let query = self.desktop.search.read(cx).value().trim().to_lowercase();
        let snippets: Vec<_> = self
            .snippets
            .iter()
            .filter(|s| {
                self.desktop
                    .snippet_tag
                    .as_ref()
                    .is_none_or(|tag| s.tags.contains(tag))
                    && (query.is_empty()
                        || s.name.to_lowercase().contains(&query)
                        || s.command.to_lowercase().contains(&query))
            })
            .collect();
        let selected = snippets
            .iter()
            .copied()
            .find(|s| Some(s.id) == self.desktop.selected_snippet)
            .or_else(|| snippets.first().copied());
        let mut list = panel()
            .w(px(220.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.desktop.search.clone());
        for snippet in snippets {
            let id = snippet.id;
            let is_sel = selected.is_some_and(|s| s.id == id);
            list = list.child(
                button(("snippet", id as u64), snippet.name.clone())
                    .bg(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.desktop.selected_snippet = Some(id);
                        cx.notify();
                    })),
            );
        }
        let mut editor = panel().flex_1().min_w_0().flex().flex_col().gap_3();
        if let Some(snippet) = selected {
            let id = snippet.id;
            let name = snippet.name.clone();
            let command = snippet.command.clone();
            let export = serde_json::to_string_pretty(&vec![shellumi_core::Snippet {
                name: snippet.name.clone(),
                command: snippet.command.clone(),
                description: snippet.description.clone(),
                tags: snippet.tags.clone(),
                variables: snippet.variables.clone(),
            }])
            .unwrap_or_default();
            editor =
                editor
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(heading(snippet.name.clone()))
                            .child(button_icon("edit-snippet", "edit", "Edit").on_click(
                                cx.listener(move |app, _, _, cx| {
                                    app.edit_library_item(EditorKind::Snippet, Some(id), cx)
                                }),
                            )),
                    )
                    .child(muted(snippet.description.clone().unwrap_or_default()))
                    .child(muted(snippet.tags.join(" · ")))
                    .child(
                        div()
                            .min_h(px(260.0))
                            .p_4()
                            .rounded_lg()
                            .bg(rgb(MacTheme::CODE_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .font_family(MONOSPACE_FONT)
                            .text_sm()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .children(snippet.command.lines().enumerate().map(|(i, line)| {
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(28.0))
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child(format!("{}", i + 1)),
                                    )
                                    .child(
                                        div()
                                            .text_color(if i == 0 && line.starts_with('#') {
                                                rgb(MacTheme::SUCCESS)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
                                            })
                                            .child(line.to_owned()),
                                    )
                            })),
                    )
                    .child(muted(format!(
                        "{} template parameters · execution always requires your action",
                        snippet.variables.len()
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(button("copy-snippet", "Copy").on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        command.clone(),
                                    ))
                                },
                            )))
                            .child(button("export-snippet", "Copy JSON package").on_click(
                                cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        export.clone(),
                                    ))
                                }),
                            ))
                            .child(button("paste-snippet", "Paste into terminal").on_click(
                                cx.listener(move |app, _, window, cx| {
                                    if app.active_tab_idx.is_none() {
                                        app.desktop.message =
                                            Some("Open a terminal session first.".into());
                                        cx.notify();
                                    } else {
                                        app.page = None;
                                        app.insert_snippet(id, false, cx);
                                        window.focus(&app.focus);
                                    }
                                }),
                            ))
                            .child(primary("run-snippet", "Run…").on_click(cx.listener(
                                move |app, _, _, cx| {
                                    if app.active_tab_idx.is_none() {
                                        app.desktop.message = Some(
                                            "Open a terminal session before running a snippet."
                                                .into(),
                                        );
                                        cx.notify();
                                    } else {
                                        app.open_snippet_modal_for_run(id, cx);
                                    }
                                },
                            )))
                            .child(
                                button("delete-snippet", "Delete")
                                    .border_color(rgb(MacTheme::DANGER))
                                    .text_color(rgb(MacTheme::DANGER))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.desktop.confirm = Some((
                                            format!("Delete snippet '{name}'?"),
                                            Command::DeleteSnippet(id),
                                        ));
                                        cx.notify();
                                    })),
                            ),
                    );
        } else {
            editor = editor.child(heading("Your command library")).child(muted(
                "Create a snippet to save commands, scripts and parameterized templates.",
            ));
        }
        let content_scroll = div()
            .id("snippets-content-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_3()
                    .child(packages)
                    .child(list)
                    .child(editor),
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
    fn open_snippet_modal_for_run(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(snippet) = self.snippets.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        self.page = None;
        self.open_snippet_modal(&snippet, true, cx);
    }
    fn settings_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut categories = panel()
            .w(px(190.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(muted("PREFERENCES"));
        for section in [
            "General",
            "Appearance",
            "Terminal",
            "Connections",
            "Files",
            "Security",
            "Synchronization",
            "Keyboard Shortcuts",
            "Advanced",
        ] {
            let is_active = self.desktop.settings_section == section;
            categories = categories.child(
                button(
                    gpui::SharedString::from(format!("setting-section-{section}")),
                    section,
                )
                .bg(if is_active {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::SURFACE)
                })
                .text_color(if is_active {
                    rgb(0xffffff)
                } else {
                    rgb(MacTheme::TEXT_PRIMARY)
                })
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.desktop.settings_section = section;
                    cx.notify();
                })),
            );
        }
        let prefs = &self.desktop.preferences;
        let mut content = panel()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(heading(self.desktop.settings_section))
                    .child(button("reset-prefs", "Reset preferences").on_click(
                        cx.listener(|app, _, _, cx| app.change_preference("reset", 0, cx)),
                    )),
            )
            .child(muted(
                "Changes are saved on this device and apply immediately.",
            ));
        let row = |title: &'static str, description: String, control: gpui::AnyElement| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .py_3()
                .border_b_1()
                .border_color(rgb(MacTheme::SEPARATOR))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(heading(title))
                        .child(muted(description)),
                )
                .child(control)
        };
        match self.desktop.settings_section {
            "General" => {
                content = content
                    .child(row(
                        "Default startup screen",
                        "Choose what appears when Shellumi opens.".into(),
                        button(
                            "startup-pref",
                            if prefs.startup_overview {
                                "Overview"
                            } else {
                                "Hosts"
                            },
                        )
                        .on_click(
                            cx.listener(|app, _, _, cx| app.change_preference("startup", 0, cx)),
                        )
                        .into_any_element(),
                    ))
                    .child(row(
                        "Local data",
                        self.database.clone().unwrap_or_default(),
                        muted("Local only").into_any_element(),
                    ));
            }
            "Appearance" => {
                content = content
                    .child(row(
                        "Interface text size",
                        "Adjust the desktop interface font size.".into(),
                        self.preference_stepper("ui", prefs.ui_font_size.to_string(), 1, cx)
                            .into_any_element(),
                    ))
                    .child(row(
                        "Color scheme",
                        "The current desktop design uses the supplied dark navy palette.".into(),
                        muted("Dark").into_any_element(),
                    ));
            }
            "Terminal" => {
                content = content
                    .child(row(
                        "Font size",
                        "Applies to terminal text; SSH and local shell use the same setting."
                            .into(),
                        self.preference_stepper(
                            "font",
                            prefs.terminal_font_size.to_string(),
                            1,
                            cx,
                        )
                        .into_any_element(),
                    ))
                    .child(row(
                        "Scrollback lines",
                        "Maximum retained terminal history per pane.".into(),
                        self.preference_stepper(
                            "scrollback",
                            prefs.scrollback_lines.to_string(),
                            1000,
                            cx,
                        )
                        .into_any_element(),
                    ))
                    .child(
                        button(
                            "open-integration-settings",
                            "Shell integration and autocomplete",
                        )
                        .on_click(
                            cx.listener(|app, _, _, cx| app.open_shell_integration_modal(cx)),
                        ),
                    );
            }
            "Files" => {
                content = content.child(row(
                    "Show hidden files",
                    "Used when opening the local SFTP file browser.".into(),
                    button(
                        "hidden-pref",
                        if prefs.show_hidden_files { "On" } else { "Off" },
                    )
                    .on_click(cx.listener(|app, _, _, cx| app.change_preference("hidden", 0, cx)))
                    .into_any_element(),
                ));
            }
            "Connections" | "Security" => {
                content=content.child(row("SSH host-key verification","Unknown servers require an explicit decision. Changed keys are rejected by OpenSSH.".into(),muted("Required").into_any_element())).child(row("Agent forwarding","SSH sessions do not forward your authentication agent.".into(),muted("Disabled").into_any_element())).child(row("Authentication","System OpenSSH uses your local SSH config, agent and interactive authentication prompts.".into(),muted("OpenSSH").into_any_element()));
            }
            "Synchronization" => {
                content=content.child(heading("Encrypted Google Drive Synchronization")).child(muted("End-to-end encrypted synchronization over HTTPS using system curl transport to your private Google Drive appDataFolder. Passphrases never leave your machine and Google cannot inspect payload ciphertext."));
            }
            "Keyboard Shortcuts" => {
                for (name, key) in [
                    ("Command palette", "Cmd/Ctrl+K"),
                    ("Refresh hosts", "Cmd/Ctrl+R"),
                    ("Paste into terminal", "Cmd+V / Ctrl+Shift+V"),
                    ("Terminal control keys", "Ctrl+A … Ctrl+Z"),
                ] {
                    content = content.child(row(name, "".into(), muted(key).into_any_element()));
                }
            }
            _ => {
                content=content.child(button("run-diagnostics-settings","Run diagnostics").on_click(cx.listener(|app,_,_,cx|app.open_diagnostics_modal(cx)))).child(muted("No telemetry. No mandatory account. Credentials are not stored in the host database."));
            }
        }
        let action_bar = div()
            .id("settings-action-bar")
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(heading("Settings"));

        let content_scroll = div()
            .id("settings-content-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_4()
                    .child(categories)
                    .child(content),
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
    fn preference_stepper(
        &self,
        key: &'static str,
        value: String,
        step: i32,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                button(gpui::SharedString::from(format!("pref-minus-{key}")), "−").on_click(
                    cx.listener(move |app, _, _, cx| app.change_preference(key, -step, cx)),
                ),
            )
            .child(muted(value))
            .child(
                button(gpui::SharedString::from(format!("pref-plus-{key}")), "+").on_click(
                    cx.listener(move |app, _, _, cx| app.change_preference(key, step, cx)),
                ),
            )
    }
    fn workspaces_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut grid = div().flex().flex_wrap().gap_3();
        for (index, workspace) in self.workspaces.iter().enumerate() {
            let id = workspace.id;
            let name = workspace.name.clone();
            let duplicate = format!("{} copy", workspace.name);
            let glyph = match index % 4 {
                0 => "◉",
                1 => "⌘",
                2 => "▣",
                _ => "▧",
            };
            let mut preview = div()
                .flex()
                .flex_wrap()
                .gap_1()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .bg(rgb(MacTheme::CONTENT_BG));
            for session in &workspace.sessions {
                preview = preview.child(
                    div()
                        .w(px(140.0))
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .bg(rgb(MacTheme::SURFACE))
                        .text_xs()
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                        .child(session.name.clone()),
                );
            }
            grid = grid.child(
                panel()
                    .w(px(320.0))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .w(px(35.0))
                                    .h(px(35.0))
                                    .rounded_md()
                                    .bg(rgb(MacTheme::BADGE_DEEP_BLUE))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                    .font_weight(FontWeight::BOLD)
                                    .child(glyph),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(heading(workspace.name.clone()))
                                    .child(muted(workspace.description.clone().unwrap_or_else(
                                        || {
                                            format!(
                                                "{} sessions · {}",
                                                workspace.sessions.len(),
                                                workspace.layout
                                            )
                                        },
                                    ))),
                            ),
                    )
                    .child(preview)
                    .child(muted(format!(
                        "{} sessions · {}",
                        workspace.sessions.len(),
                        workspace.layout
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(primary(("open-workspace", id as u64), "Open").on_click(
                                cx.listener(move |app, _, window, cx| {
                                    app.load_workspace(id, window, cx)
                                }),
                            ))
                            .child(button(("edit-workspace", id as u64), "Edit").on_click(
                                cx.listener(move |app, _, _, cx| {
                                    app.edit_library_item(EditorKind::Workspace, Some(id), cx)
                                }),
                            ))
                            .child(
                                button(("duplicate-workspace", id as u64), "Duplicate").on_click(
                                    cx.listener(move |app, _, _, cx| {
                                        app.desktop_command(
                                            Command::DuplicateWorkspace {
                                                id,
                                                name: duplicate.clone(),
                                            },
                                            cx,
                                        )
                                    }),
                                ),
                            )
                            .child(button(("delete-workspace", id as u64), "Delete").on_click(
                                cx.listener(move |app, _, _, cx| {
                                    app.desktop.confirm = Some((
                                        format!("Delete workspace '{name}'?"),
                                        Command::DeleteWorkspace(id),
                                    ));
                                    cx.notify();
                                }),
                            )),
                    ),
            );
        }
        if self.workspaces.is_empty() {
            grid=grid.child(empty("Build your workspace","Save named layouts containing local shells and SSH hosts. Opening a workspace starts new sessions."));
        }
        let action_bar = div()
            .id("workspaces-action-bar")
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(heading("Saved workspaces"))
            .child(primary("new-workspace", "+ New workspace").on_click(
                cx.listener(|app, _, _, cx| app.edit_library_item(EditorKind::Workspace, None, cx)),
            ));

        let content_scroll = div()
            .id("workspaces-content-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .child(grid);

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
    fn overview_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let hosts = match &self.hosts {
            HostState::Loaded(hosts) => hosts.as_slice(),
            _ => &[],
        };
        let active = self
            .tabs
            .iter()
            .flat_map(|tab| &tab.panes)
            .filter(|p| p.session.is_some())
            .count();
        let metric_defs = [
            (
                "Saved hosts",
                hosts.len(),
                "⌘",
                MacTheme::BADGE_NAVY,
                MacTheme::ACCENT_BLUE,
            ),
            ("Open sessions", active, "●", 0x1a3a2a, MacTheme::SUCCESS),
            (
                "Saved workspaces",
                self.workspaces.len(),
                "▣",
                MacTheme::BADGE_DEEP_BLUE,
                MacTheme::ACCENT_BLUE,
            ),
            (
                "Command snippets",
                self.snippets.len(),
                "⚡",
                0x3d3010,
                MacTheme::WARNING,
            ),
        ];
        let mut metrics = div().flex().flex_wrap().gap_3();
        for (label, value, glyph, bg, fg) in metric_defs {
            metrics = metrics.child(
                panel().flex_1().min_w(px(140.0)).child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(36.0))
                                .rounded_md()
                                .bg(rgb(bg))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(fg))
                                .font_weight(FontWeight::BOLD)
                                .child(glyph),
                        )
                        .child(
                            div().flex().flex_col().child(muted(label)).child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child(value.to_string()),
                            ),
                        ),
                ),
            );
        }
        let mut host_list = panel()
            .flex_1()
            .flex()
            .flex_col()
            .gap_3()
            .child(heading("Saved connections"));
        for host in hosts.iter().take(6) {
            let host = host.clone();
            host_list = host_list.child(
                button(
                    ("overview-host", host.id as u64),
                    format!("⌘  {}", host.name),
                )
                .on_click(cx.listener(move |app, _, window, cx| app.connect(&host, window, cx))),
            );
        }
        if hosts.is_empty() {
            host_list = host_list.child(muted("Add your first host to get started."));
        }
        let actions = panel()
            .flex_1()
            .flex()
            .flex_col()
            .gap_3()
            .child(heading("Quick actions"))
            .child(
                primary("overview-new-host", "New host").on_click(cx.listener(|app, _, _, cx| {
                    app.page = None;
                    app.edit_host(None, cx);
                })),
            )
            .child(
                button("overview-local", "Open local terminal").on_click(
                    cx.listener(|app, _, window, cx| app.open_local_terminal(window, cx)),
                ),
            )
            .child(
                button("overview-new-snippet", "New snippet").on_click(cx.listener(
                    |app, _, _, cx| {
                        app.page = Some("Snippets");
                        app.edit_library_item(EditorKind::Snippet, None, cx);
                    },
                )),
            )
            .child(button("overview-workspaces", "Manage workspaces").on_click(
                cx.listener(|app, _, window, cx| app.navigate_page("Workspaces", window, cx)),
            ));
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Your infrastructure, in focus."),
            )
            .child(muted(
                "Manage connections, files and workflows from your local workspace.",
            ))
            .child(metrics)
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_start()
                    .child(host_list)
                    .child(actions),
            )
    }
    fn sessions_page(&self, sftp: bool, cx: &mut Context<Self>) -> gpui::Div {
        let mut content = div().flex().flex_col().gap_3();
        if !sftp {
            content =
                content.child(primary("new-local-session", "+ Local terminal").on_click(
                    cx.listener(|app, _, window, cx| app.open_local_terminal(window, cx)),
                ));
        }
        for (index, tab) in self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| !sftp || tab.kind == SessionKind::Sftp)
        {
            content = content.child(button(("session-nav", index), tab.name.clone()).on_click(
                cx.listener(move |app, _, window, cx| app.select_tab(Some(index), window, cx)),
            ));
        }
        if let HostState::Loaded(hosts) = &self.hosts {
            for host in hosts.iter().filter(|host| host.is_ssh()) {
                let host = host.clone();
                content = content.child(
                    panel()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(heading(host.name.clone()))
                        .child(
                            button(
                                ("file-host", host.id as u64),
                                if sftp { "Connect SFTP" } else { "Connect SSH" },
                            )
                            .on_click(cx.listener(
                                move |app, _, window, cx| {
                                    if sftp {
                                        app.connect_sftp(&host, window, cx);
                                    } else {
                                        app.connect(&host, window, cx);
                                    }
                                },
                            )),
                        ),
                );
            }
        }
        content
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
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
    fn refresh_public_inventory(&mut self, cx: &mut Context<Self>) {
        let keys = self.page == Some("Keychain");
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from);
        let Some(home) = home else {
            self.desktop.message = Some("Home directory is unavailable.".into());
            cx.notify();
            return;
        };
        self.desktop.public_records.clear();
        self.desktop.public_selected = 0;
        if keys {
            self.load_identities(cx);
        }
        let task = cx
            .background_executor()
            .spawn(async move { shellumi_storage::desktop::public_ssh_records(&home, keys) });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok(records) => app.desktop.public_records = records,
                    Err(error) => app.desktop.message = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn public_inventory_page(&self, page: &'static str, cx: &mut Context<Self>) -> gpui::Div {
        let keys = page == "Keychain";
        if keys {
            let action_bar = div()
                .id("keychain-action-bar")
                .flex()
                .items_center()
                .justify_between()
                .px_6()
                .py_3()
                .border_b_1()
                .border_color(rgb(MacTheme::BORDER))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            button_icon("generate-ssh-key", "private_key", "+ New key ⌵")
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                                .text_color(rgb(0xffffff))
                                .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                .on_click(
                                    cx.listener(|app, _, window, cx| {
                                        app.start_session(
                                            "Generate SSH identity".into(),
                                            SessionKind::Local,
                                            TerminalCommand::new("ssh-keygen").args(["-t", "ed25519"]),
                                            window,
                                            cx,
                                        )
                                    }),
                                ),
                        )
                        .child(
                            button_icon("new-identity-btn", "identity_card", "+ New Identity").on_click(
                                cx.listener(|app, _, _, cx| {
                                    app.open_identity_modal(None, cx);
                                }),
                            ),
                        )
                        .child(
                            button_icon("cert-btn", "certificate", "Certificate").on_click(cx.listener(|app, _, _, cx| {
                                app.desktop.message = Some("Certificate management: add certificates to identities or OpenSSH config.".into());
                                cx.notify();
                            })),
                        )
                        .child(
                            button_icon("fido2-btn", "fido2", "FIDO2").on_click(cx.listener(|app, _, _, cx| {
                                app.desktop.message = Some("FIDO2 hardware keys supported via OpenSSH sk-ecdsa/sk-ed25519 keys.".into());
                                cx.notify();
                            })),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("keychain-search-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.open_command_palette(window, cx)
                                }))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("search", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("🔍").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .id("keychain-grid-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("grid_view", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("⊞").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .id("keychain-refresh-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.refresh_public_inventory(cx)
                                }))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("calendar_sort", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("📅").into_any_element()
                                    },
                                ),
                        ),
                );

            let mut keys_grid = div().flex().flex_wrap().gap_3();
            for (idx, record) in self.desktop.public_records.iter().enumerate() {
                let is_sel = idx == self.desktop.public_selected;
                keys_grid = keys_grid.child(
                    div()
                        .id(("key-card", idx))
                        .w(px(250.0))
                        .p_3p5()
                        .rounded_xl()
                        .bg(rgb(if is_sel {
                            MacTheme::SURFACE_ACTIVE
                        } else {
                            MacTheme::SURFACE
                        }))
                        .border_1()
                        .border_color(rgb(if is_sel {
                            MacTheme::ACCENT_BLUE
                        } else {
                            MacTheme::BORDER
                        }))
                        .flex()
                        .items_center()
                        .gap_3()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.desktop.public_selected = idx;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(36.0))
                                .rounded_lg()
                                .bg(rgb(MacTheme::BADGE_NAVY))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_base()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("public_key", false, 48)
                                    {
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("🔑").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .truncate()
                                        .child(record.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(format!("Type {}", record.key_type)),
                                ),
                        ),
                );
            }
            if self.desktop.public_records.is_empty() {
                keys_grid = keys_grid.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("No SSH keys found in ~/.ssh."),
                );
            }

            let mut identities_grid = div().flex().flex_wrap().gap_3();
            for (idx, id) in self.identities.iter().enumerate() {
                let edit_id = id.id;
                identities_grid = identities_grid.child(
                    div()
                        .id(("identity-card", idx))
                        .w(px(260.0))
                        .p_3p5()
                        .rounded_xl()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .cursor_pointer()
                        .hover(|s| {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                        })
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.open_identity_modal(Some(edit_id), cx);
                        }))
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(36.0))
                                .rounded_lg()
                                .bg(rgb(MacTheme::BADGE_NAVY))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_base()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("identity_card", false, 48)
                                    {
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("🪪").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .truncate()
                                        .child(id.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(format!("User: {}", id.username)),
                                ),
                        )
                        .child(
                            div()
                                .id(("edit-identity-badge", idx))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::INPUT_BG))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    if let Some(p) = crate::resolve_icon_path("edit", false, 32) {
                                        div()
                                            .w(px(14.0))
                                            .h(px(14.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("✏️").into_any_element()
                                    },
                                )
                                .child("Edit"),
                        ),
                );
            }
            if self.identities.is_empty() {
                identities_grid = identities_grid.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("No saved reusable identities yet."),
                );
            }

            let daemon = shellumi_core::probe_keychain_daemon();
            let daemon_card = div()
                .p_3()
                .rounded_lg()
                .bg(rgb(MacTheme::SURFACE))
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(format!("🔐 OS Keychain Daemon: {}", daemon.backend_name)),
                        )
                        .child(
                            div()
                                .p_1()
                                .rounded_md()
                                .text_xs()
                                .bg(if daemon.is_available {
                                    rgba(0x30d15826)
                                } else {
                                    rgba(0xffd60a26)
                                })
                                .text_color(if daemon.is_available {
                                    rgb(MacTheme::SUCCESS)
                                } else {
                                    rgb(MacTheme::WARNING)
                                })
                                .child(if daemon.is_available {
                                    "ACTIVE"
                                } else {
                                    "FALLBACK"
                                }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child(format!("Daemon: {}", daemon.daemon_identifier)),
                );

            let content_scroll = div()
                .id("keychain-content-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_6()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Keys"),
                )
                .child(keys_grid)
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Identities"),
                )
                .child(identities_grid)
                .child(div().h(px(8.0)))
                .child(daemon_card);

            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .bg(rgb(MacTheme::CONTENT_BG))
                .child(action_bar)
                .child(content_scroll)
        } else {
            // Known Hosts view
            let action_bar = div()
                .id("known-hosts-action-bar")
                .flex()
                .items_center()
                .justify_between()
                .px_6()
                .py_3()
                .border_b_1()
                .border_color(rgb(MacTheme::BORDER))
                .child(
                    button_icon("import-known-hosts", "import", "Import").on_click(cx.listener(
                        |app, _, _, cx| {
                            app.refresh_public_inventory(cx);
                        },
                    )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("known-hosts-search-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.open_command_palette(window, cx)
                                }))
                                .child(
                                    if let Some(p) = crate::resolve_icon_path("search", false, 48) {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("🔍").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .id("known-hosts-grid-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("grid_view", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("⊞").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div()
                                .id("known-hosts-refresh-btn")
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                })
                                .on_click(
                                    cx.listener(|app, _, _, cx| app.refresh_public_inventory(cx)),
                                )
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("calendar_sort", false, 48)
                                    {
                                        div()
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().text_xs().child("📅").into_any_element()
                                    },
                                ),
                        ),
                );

            let mut hosts_grid = div().flex().flex_wrap().gap_3();
            for (idx, record) in self.desktop.public_records.iter().enumerate() {
                hosts_grid = hosts_grid.child(
                    div()
                        .id(("known-host-card", idx))
                        .min_w(px(250.0))
                        .flex_1()
                        .p_3p5()
                        .rounded_xl()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .flex()
                        .items_center()
                        .gap_3()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .child(
                            div()
                                .w(px(36.0))
                                .h(px(36.0))
                                .rounded_lg()
                                .bg(rgb(MacTheme::BADGE_NAVY))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_base()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child(
                                    if let Some(p) =
                                        crate::resolve_icon_path("fingerprint", false, 48)
                                    {
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("🛡").into_any_element()
                                    },
                                ),
                        )
                        .child(
                            div().flex().flex_col().gap_0p5().min_w_0().child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .truncate()
                                    .child(record.name.clone()),
                            ),
                        ),
                );
            }
            if self.desktop.public_records.is_empty() {
                hosts_grid = hosts_grid.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("No known-host records found in ~/.ssh/known_hosts."),
                );
            }

            let content_scroll = div()
                .id("known-hosts-content-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_6()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Known Hosts"),
                )
                .child(hosts_grid);

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
    }
}

impl Shellumi {
    pub(super) fn host_library_actions(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .gap_2()
            .child(
                button_icon("import-host-config", "import", "Import").on_click(cx.listener(
                    |app, _, _, cx| app.edit_library_item(EditorKind::ImportHosts, None, cx),
                )),
            )
            .child(
                button_icon("new-group", "group_settings", "Group settings").on_click(cx.listener(
                    |app, _, _, cx| {
                        app.edit_library_item(EditorKind::Group, None, cx);
                        let group = app
                            .selected_group
                            .as_ref()
                            .and_then(|name| {
                                app.desktop.groups.iter().find(|group| &group.name == name)
                            })
                            .cloned();
                        if let (Some(group), Some(editor)) = (group, app.desktop.editor.as_mut()) {
                            editor.identity_id = group.identity_id;
                            editor
                                .name
                                .update(cx, |input, cx| input.set_value(&group.name, cx));
                            editor.description.update(cx, |input, cx| {
                                input.set_value(group.default_username.as_deref().unwrap_or(""), cx)
                            });
                            editor.tags.update(cx, |input, cx| {
                                input.set_value(&group.tags.join(", "), cx)
                            });
                            editor.lines[0].update(cx, |input, cx| {
                                input.set_value(
                                    &group
                                        .default_port
                                        .map(|port| port.to_string())
                                        .unwrap_or_default(),
                                    cx,
                                )
                            });
                        }
                    },
                )),
            )
            .child(
                button_icon("export-host-config", "export", "Export").on_click(cx.listener(
                    |app, _, _, cx| {
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
                    },
                )),
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
    fn refresh_local_files(&mut self, cx: &mut Context<Self>) {
        let path = self.desktop.file_path.read(cx).value().to_owned();
        let hidden = self.desktop.preferences.show_hidden_files;
        let task = cx.background_executor().spawn(async move {
            shellumi_core::list_local_directory(&path, hidden).map_err(|e| e.to_string())
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok(entries) => {
                        app.desktop.local_entries = entries;
                        app.desktop.message = None;
                    }
                    Err(error) => app.desktop.message = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
    #[allow(dead_code)]
    fn files_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut local = panel()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(heading("Local files"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(self.desktop.file_path.clone()))
                    .child(
                        button("local-path-open", "Open")
                            .on_click(cx.listener(|app, _, _, cx| app.refresh_local_files(cx))),
                    )
                    .child(button("local-path-parent", "↑").on_click(cx.listener(
                        |app, _, _, cx| {
                            let path =
                                std::path::PathBuf::from(app.desktop.file_path.read(cx).value());
                            if let Some(parent) = path.parent() {
                                let parent = parent.to_string_lossy().into_owned();
                                app.desktop
                                    .file_path
                                    .update(cx, |input, cx| input.set_value(&parent, cx));
                                app.refresh_local_files(cx);
                            }
                        },
                    ))),
            )
            .child(
                button(
                    "local-files-hidden",
                    if self.desktop.preferences.show_hidden_files {
                        "Hide hidden files"
                    } else {
                        "Show hidden files"
                    },
                )
                .on_click(cx.listener(|app, _, _, cx| app.change_preference("hidden", 0, cx))),
            );
        for (index, entry) in self.desktop.local_entries.iter().enumerate() {
            let path = entry.path.clone();
            let directory = entry.kind == FileKind::Directory;
            local = local.child(
                button(
                    ("local-file", index),
                    format!(
                        "{} {}     {}",
                        if directory { "📁" } else { "◧" },
                        entry.name,
                        if directory {
                            "—".to_string()
                        } else {
                            format!("{} B", entry.size)
                        }
                    ),
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    if directory {
                        app.desktop
                            .file_path
                            .update(cx, |input, cx| input.set_value(&path, cx));
                        app.refresh_local_files(cx);
                    } else {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(path.clone()));
                        app.desktop.message = Some(
                            "Local file path copied. Use put in the SFTP session to upload.".into(),
                        );
                        cx.notify();
                    }
                })),
            );
        }
        let remote = panel()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(heading("Remote connections"))
            .child(muted(
                "Select a host to open its live SFTP terminal and graphical dual-pane browser.",
            ))
            .child(self.sessions_page(true, cx));
        div()
            .flex()
            .items_start()
            .gap_4()
            .child(local)
            .child(remote)
    }
}

impl Shellumi {
    fn backup_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let sync_badge = match &self.sync_status {
            AppSyncStatus::LocalOnly => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgb(0x38383a))
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child("Local only"),
            AppSyncStatus::UpToDate { .. } => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgba(0x30d15826))
                .text_xs()
                .text_color(rgb(MacTheme::SUCCESS))
                .child("Up to date"),
            AppSyncStatus::Syncing => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgba(0x0a84ff26))
                .text_xs()
                .text_color(rgb(MacTheme::ACCENT_BLUE))
                .child("Syncing…"),
            AppSyncStatus::Conflict { message } => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgba(0xffd60a26))
                .text_xs()
                .text_color(rgb(MacTheme::WARNING))
                .child(format!("Conflict: {message}")),
            AppSyncStatus::Error(e) => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgba(0xff453a26))
                .text_xs()
                .text_color(rgb(MacTheme::DANGER))
                .child(format!("Error: {e}")),
        };

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                panel()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(heading("Google Drive Cloud Synchronization"))
                            .child(sync_badge),
                    )
                    .child(muted(
                        "End-to-end encrypted synchronization over HTTPS directly to your private Google Drive appDataFolder via system HTTP transport. All records are encrypted with AES-256-GCM before transport; Google cannot inspect your data.",
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                primary("trigger-gdrive-sync", "Sync with Google Drive now")
                                    .on_click(cx.listener(|app, _, _, cx| app.trigger_sync(cx))),
                            ),
                    ),
            )
            .child(
                panel()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(heading("Encrypted Backups"))
                    .child(muted(
                        "Export your local vault, or restore an existing encrypted backup. Only ciphertext is written to the backup file.",
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                primary("export-vault-backup", "Export encrypted backup…")
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.edit_library_item(EditorKind::ExportBackup, None, cx)
                                    })),
                            )
                            .child(
                                button("restore-vault-backup", "Restore encrypted backup…")
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.edit_library_item(EditorKind::RestoreBackup, None, cx)
                                    })),
                            ),
                    ),
            )
    }
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

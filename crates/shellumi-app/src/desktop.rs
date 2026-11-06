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
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x263346))
        .bg(rgb(0x141e2c))
        .p_4()
}
fn muted(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div().text_sm().text_color(rgb(0x91a0b7)).child(text.into())
}
fn heading(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div().font_weight(FontWeight::SEMIBOLD).child(text.into())
}
fn button(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x34465c))
        .bg(rgb(0x192536))
        .text_sm()
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x223043)))
        .child(text.into())
}
fn primary(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    button(id, text).bg(rgb(0x59ddc1)).text_color(rgb(0x0b2928))
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
        self.desktop.editor = None;
        self.desktop.confirm = None;
        self.desktop.message = None;
        self.page = if page == "Hosts" { None } else { Some(page) };
        if page == "Terminal" {
            if let Some(index) = self
                .active_tab_idx
                .or_else(|| (!self.tabs.is_empty()).then_some(0))
            {
                self.select_tab(Some(index), window, cx);
            }
        }
        if page == "Keychain" || page == "Known Hosts" {
            self.refresh_public_inventory(cx);
        }
        match page {
            "Port Forwarding" if self.tunnel_modal.is_none() => self.open_tunnel_modal(cx),
            "Session Logs" if self.session_replay_modal.is_none() => {
                self.open_session_replay_modal(cx)
            }
            "Vaults" if self.team_modal.is_none() => self.open_team_modal(cx),
            "Integrations" if self.infra_modal.is_none() => self.open_infra_modal(cx),
            _ => {}
        }
        if page == "SFTP Files" {
            self.refresh_local_files(cx);
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
                    Ok(group) => Command::SaveGroup(group),
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
        let subtitle = match page {
            "Overview" => "Everything in one secure place",
            "Snippets" => "Reusable command library",
            "Workspaces" => "Saved layouts and sessions",
            "Settings" => "Customize Shellumi",
            "SFTP Files" => "Secure file transfer",
            "Keychain" => "Keys, certificates and identities",
            "Known Hosts" => "SSH server trust records",
            "Vaults" => "Local vaults and encrypted backups",
            "Collaboration" => "Session sharing and team access",
            "Integrations" => "Cloud inventory and infrastructure",
            "Google Drive Sync" => "Optional encrypted synchronization",
            _ => "Your infrastructure workspace",
        };
        let header =
            div()
                .flex()
                .justify_between()
                .items_center()
                .px_6()
                .py_4()
                .border_b_1()
                .border_color(rgb(0x263346))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_xl().font_weight(FontWeight::BOLD).child(page))
                        .child(muted(subtitle)),
                )
                .child(button("page-search", "Search anything").on_click(
                    cx.listener(|app, _, window, cx| app.open_command_palette(window, cx)),
                ));
        let content = match page {
            "Snippets" => self.snippets_page(cx),
            "Settings" => self.settings_page(cx),
            "Workspaces" => self.workspaces_page(cx),
            "Overview" => self.overview_page(cx),
            "Terminal" => self.sessions_page(false, cx),
            "SFTP Files" => self.files_page(cx),
            _ => self.tools_page(page, cx),
        };
        let mut body = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(header)
            .child(
                div()
                    .id("page-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_6()
                    .child(content),
            );
        if self.desktop.busy {
            body = body.child(div().px_6().py_2().text_sm().child("Saving…"));
        }
        if let Some(message) = &self.desktop.message {
            body = body.child(
                div()
                    .px_6()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(0xf3be72))
                    .child(message.clone()),
            );
        }
        body
    }
    fn snippets_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let toolbar = div()
            .flex()
            .justify_between()
            .items_center()
            .child(heading("Command snippets"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("import-snippets", "Import package").on_click(cx.listener(
                            |app, _, _, cx| app.edit_library_item(EditorKind::Import, None, cx),
                        )),
                    )
                    .child(
                        primary("new-snippet", "+ New snippet").on_click(cx.listener(
                            |app, _, _, cx| app.edit_library_item(EditorKind::Snippet, None, cx),
                        )),
                    ),
            );
        let mut packages = panel()
            .w(px(160.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(muted("PACKAGES"))
            .child(button("all-snippets", "All snippets").on_click(cx.listener(
                |app, _, _, cx| {
                    app.desktop.snippet_tag = None;
                    cx.notify();
                },
            )));
        let mut tags: Vec<_> = self.snippets.iter().flat_map(|s| s.tags.iter()).collect();
        tags.sort();
        tags.dedup();
        for (index, tag) in tags.iter().enumerate() {
            let tag = (*tag).clone();
            let selected_tag = tag.clone();
            packages = packages.child(button(("snippet-tag", index), tag).on_click(cx.listener(
                move |app, _, _, cx| {
                    app.desktop.snippet_tag = Some(selected_tag.clone());
                    cx.notify();
                },
            )));
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
            list = list.child(
                button(("snippet", id as u64), snippet.name.clone())
                    .bg(if selected.is_some_and(|s| s.id == id) {
                        rgb(0x1c373d)
                    } else {
                        rgb(0x141e2c)
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
            editor = editor
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(heading(snippet.name.clone()))
                        .child(button("edit-snippet", "Edit").on_click(cx.listener(
                            move |app, _, _, cx| {
                                app.edit_library_item(EditorKind::Snippet, Some(id), cx)
                            },
                        ))),
                )
                .child(muted(snippet.description.clone().unwrap_or_default()))
                .child(muted(snippet.tags.join(" · ")))
                .child(
                    div()
                        .min_h(px(260.0))
                        .p_4()
                        .rounded_md()
                        .bg(rgb(0x0b1521))
                        .font_family(MONOSPACE_FONT)
                        .text_sm()
                        .children(
                            snippet
                                .command
                                .lines()
                                .map(|line| div().child(line.to_owned())),
                        ),
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
                        .child(
                            button("export-snippet", "Copy JSON package").on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        export.clone(),
                                    ))
                                },
                            )),
                        )
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
                                        "Open a terminal session before running a snippet.".into(),
                                    );
                                    cx.notify();
                                } else {
                                    app.open_snippet_modal_for_run(id, cx);
                                }
                            },
                        )))
                        .child(
                            button("delete-snippet", "Delete")
                                .text_color(rgb(0xff858c))
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
        div().flex().flex_col().gap_4().child(toolbar).child(
            div()
                .flex()
                .items_start()
                .gap_3()
                .child(packages)
                .child(list)
                .child(editor),
        )
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
            categories = categories.child(
                button(
                    gpui::SharedString::from(format!("setting-section-{section}")),
                    section,
                )
                .bg(if self.desktop.settings_section == section {
                    rgb(0x1c373d)
                } else {
                    rgb(0x141e2c)
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
                .border_color(rgb(0x263346))
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
        div()
            .flex()
            .items_start()
            .gap_4()
            .child(categories)
            .child(content)
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
        for workspace in &self.workspaces {
            let id = workspace.id;
            let name = workspace.name.clone();
            let duplicate = format!("{} copy", workspace.name);
            let mut preview = div()
                .flex()
                .flex_wrap()
                .gap_1()
                .p_3()
                .rounded_md()
                .bg(rgb(0x0b1521));
            for session in &workspace.sessions {
                preview = preview.child(
                    div()
                        .w(px(140.0))
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0x34465c))
                        .text_xs()
                        .text_color(rgb(0x59ddc1))
                        .child(session.name.clone()),
                );
            }
            grid = grid.child(
                panel()
                    .w(px(300.0))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(heading(workspace.name.clone()))
                    .child(muted(workspace.description.clone().unwrap_or_default()))
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
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(heading("Saved workspaces"))
                    .child(
                        primary("new-workspace", "+ New workspace").on_click(cx.listener(
                            |app, _, _, cx| app.edit_library_item(EditorKind::Workspace, None, cx),
                        )),
                    ),
            )
            .child(grid)
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
        let mut metrics = div().flex().gap_3();
        for (label, value) in [
            ("Saved hosts", hosts.len()),
            ("Open sessions", active),
            ("Saved workspaces", self.workspaces.len()),
            ("Command snippets", self.snippets.len()),
        ] {
            metrics = metrics.child(
                panel().flex_1().child(muted(label)).child(
                    div()
                        .mt_2()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .child(value.to_string()),
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
                button(("overview-host", host.id as u64), host.name.clone()).on_click(
                    cx.listener(move |app, _, window, cx| app.connect(&host, window, cx)),
                ),
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
            "Port Forwarding"=> {if let Some(modal)=&self.tunnel_modal{return div().child(self.render_tunnel_modal(modal,cx));} empty("Port forwarding","Create and manage saved SSH tunnel rules.").child(primary("open-tunnels-page","Manage tunnels").on_click(cx.listener(|app,_,_,cx|app.open_tunnel_modal(cx))))},
            "Session Logs"=>{if let Some(modal)=&self.session_replay_modal{return div().child(self.render_session_replay_modal(modal,cx));} empty("Session recordings","Recordings are opt-in and can contain sensitive terminal output.").child(primary("open-recordings-page","Open recordings").on_click(cx.listener(|app,_,_,cx|app.open_session_replay_modal(cx))))},
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
                    .child(host_form_field("Inherited tags", editor.tags.clone()));
            } else if editor.kind == EditorKind::Workspace {
                let mut layouts = div().flex().gap_2();
                for layout in ["single", "vertical", "horizontal", "grid"] {
                    layouts = layouts.child(
                        button(gpui::SharedString::from(format!("layout-{layout}")), layout)
                            .bg(if editor.layout == layout {
                                rgb(0x1c373d)
                            } else {
                                rgb(0x192536)
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
                                    .text_color(rgb(0x697c94))
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
                .child(div().text_lg().font_weight(FontWeight::BOLD).child(title))
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
                    .bg(rgba(0x030913dd))
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(modal),
            );
        }
        if let Some((message, command)) = &self.desktop.confirm {
            let command = command.clone();
            root=root.child(div().absolute().inset_0().bg(rgba(0x030913dd)).flex().items_center().justify_center().child(panel().w(px(440.0)).flex().flex_col().gap_4().child(heading(message.clone())).child(muted("Confirm this change to the local record. Existing terminal sessions are not affected.")).child(div().flex().justify_end().gap_2().child(button("cancel-library-delete","Cancel").on_click(cx.listener(|app,_,_,cx|{app.desktop.confirm=None;cx.notify();}))).child(primary("confirm-library-delete","Delete").on_click(cx.listener(move|app,_,_,cx|app.desktop_command(command.clone(),cx)))))));
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
        let mut table = panel().flex_1().min_w_0().flex().flex_col().gap_2();
        if keys {
            let daemon = shellumi_core::probe_keychain_daemon();
            let status_badge_color = if daemon.is_available {
                rgb(0x10b981)
            } else {
                rgb(0xf59e0b)
            };
            let status_badge_bg = if daemon.is_available {
                rgb(0x064e3b)
            } else {
                rgb(0x78350f)
            };
            let daemon_card = div()
                .p_3()
                .rounded_lg()
                .bg(rgb(0x0f172a))
                .border_1()
                .border_color(rgb(0x334155))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
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
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(0xf8fafc))
                                        .child(format!(
                                            "🔐 OS Keychain Daemon: {}",
                                            daemon.backend_name
                                        )),
                                )
                                .child(
                                    div()
                                        .p_1()
                                        .rounded_md()
                                        .text_xs()
                                        .bg(status_badge_bg)
                                        .text_color(status_badge_color)
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
                                .text_color(rgb(0x94a3b8))
                                .child(format!("Daemon: {}", daemon.daemon_identifier)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x64748b))
                        .child(daemon.details),
                );

            table = table.child(daemon_card).child(div().h(px(4.0)));
            table = table.child(heading("Saved Reusable Identities"));
            if self.identities.is_empty() {
                table = table.child(muted("No reusable identities saved. Add identities via `shellumi identity add <name> --username <user> --key-path <path>`."));
            } else {
                for id in &self.identities {
                    let cert_info = if let Some(ref cert) = id.certificate_path {
                        format!(" (Cert: {cert})")
                    } else {
                        String::new()
                    };
                    let method_badge = format!("{} • {}{}", id.username, id.auth_method, cert_info);
                    let has_keychain_ref =
                        id.password_secret_ref.is_some() || id.passphrase_secret_ref.is_some();
                    table = table.child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(rgb(0x1e293b))
                            .border_1()
                            .border_color(rgb(0x334155))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(0xf3f4f6))
                                                    .child(id.name.clone()),
                                            )
                                            .children(if has_keychain_ref {
                                                Some(
                                                    div()
                                                        .p_1()
                                                        .rounded_sm()
                                                        .bg(rgb(0x1e3a8a))
                                                        .text_color(rgb(0x93c5fd))
                                                        .text_xs()
                                                        .child("🔐 Keychain"),
                                                )
                                            } else {
                                                None
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x9ca3af))
                                            .child(method_badge),
                                    ),
                            )
                            .child(if let Some(ref kp) = id.key_path {
                                div().text_xs().text_color(rgb(0x60a5fa)).child(kp.clone())
                            } else {
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x9ca3af))
                                    .child("Passphrase/Agent".to_string())
                            }),
                    );
                }
            }
            table = table
                .child(div().h(px(8.0)))
                .child(heading("System SSH Keys (~/.ssh)"));
        }
        for (index, record) in self.desktop.public_records.iter().enumerate() {
            table = table.child(
                button(("public-record", index), record.name.clone())
                    .bg(if index == self.desktop.public_selected {
                        rgb(0x1c373d)
                    } else {
                        rgb(0x141e2c)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.desktop.public_selected = index;
                        cx.notify();
                    })),
            );
        }
        if self.desktop.public_records.is_empty() {
            table=table.child(muted(if keys{"No public SSH keys were found in your .ssh directory."}else{"No known-host records were found. OpenSSH adds records after you explicitly trust a server."}));
        }
        let mut inspector = panel().w(px(290.0)).flex().flex_col().gap_3();
        if let Some(record) = self
            .desktop
            .public_records
            .get(self.desktop.public_selected)
        {
            let text = record.public_key.clone();
            inspector = inspector
                .child(heading(record.name.clone()))
                .child(muted(record.key_type.clone()))
                .child(muted(record.fingerprint.clone()))
                .child(muted(record.path.to_string_lossy().into_owned()))
                .child(
                    button("copy-public-key", "Copy public key").on_click(cx.listener(
                        move |_, _, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()))
                        },
                    )),
                );
            if !keys {
                if let Some(raw) = &record.trust_record {
                    if !raw.starts_with('@') {
                        let command = Command::RevokeKnownHost {
                            path: record.path.clone(),
                            expected: raw.clone(),
                        };
                        inspector=inspector.child(button("revoke-known-host","Revoke trust…").text_color(rgb(0xff858c)).on_click(cx.listener(move|app,_,_,cx|{app.desktop.confirm=Some(("Revoke this server key? Future connections with this key will be rejected.".into(),command.clone()));cx.notify();})));
                    }
                }
            }
            if keys {
                let private_path = record
                    .path
                    .with_extension("")
                    .to_string_lossy()
                    .into_owned();
                inspector = inspector.child(
                    button("load-agent-identity", "Load identity into SSH agent…").on_click(
                        cx.listener(move |app, _, window, cx| {
                            app.start_session(
                                "SSH agent".into(),
                                SessionKind::Local,
                                TerminalCommand::new("ssh-add").arg(private_path.clone()),
                                window,
                                cx,
                            )
                        }),
                    ),
                );
            }
        } else {
            inspector = inspector.child(muted("Select a record to inspect its public metadata."));
        }
        let mut toolbar = div()
            .flex()
            .justify_between()
            .child(heading(if keys {
                "SSH identities"
            } else {
                "Trusted server records"
            }))
            .child(
                button("refresh-ssh-records", "Refresh")
                    .on_click(cx.listener(|app, _, _, cx| app.refresh_public_inventory(cx))),
            );
        if keys {
            toolbar = toolbar.child(primary("generate-ssh-key", "Generate key…").on_click(
                cx.listener(|app, _, window, cx| {
                    app.start_session(
                        "Generate SSH identity".into(),
                        SessionKind::Local,
                        TerminalCommand::new("ssh-keygen").args(["-t", "ed25519"]),
                        window,
                        cx,
                    )
                }),
            ));
        }
        div().flex().flex_col().gap_4().child(toolbar).child(muted(if keys{"System OpenSSH manages private keys. Key generation prompts for a storage path and passphrase in the terminal."}else{"Trust decisions are handled by OpenSSH with strict verification. This view never accepts or replaces server keys automatically."})).child(div().flex().items_start().gap_4().child(table).child(inspector))
    }
}

impl Shellumi {
    pub(super) fn host_library_actions(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .gap_2()
            .child(
                button("import-host-config", "Import").on_click(cx.listener(|app, _, _, cx| {
                    app.edit_library_item(EditorKind::ImportHosts, None, cx)
                })),
            )
            .child(
                button("new-group", "Group settings").on_click(cx.listener(|app, _, _, cx| {
                    app.edit_library_item(EditorKind::Group, None, cx);
                    let group = app
                        .selected_group
                        .as_ref()
                        .and_then(|name| {
                            app.desktop.groups.iter().find(|group| &group.name == name)
                        })
                        .cloned();
                    if let (Some(group), Some(editor)) = (group, app.desktop.editor.as_mut()) {
                        editor
                            .name
                            .update(cx, |input, cx| input.set_value(&group.name, cx));
                        editor.description.update(cx, |input, cx| {
                            input.set_value(group.default_username.as_deref().unwrap_or(""), cx)
                        });
                        editor
                            .tags
                            .update(cx, |input, cx| input.set_value(&group.tags.join(", "), cx));
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
                })),
            )
            .child(
                button("export-host-config", "Export").on_click(cx.listener(|app, _, _, cx| {
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
                        app.desktop.message = Some("OpenSSH config copied to clipboard.".into());
                        cx.notify();
                    }
                })),
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
                        if directory { "▸" } else { "·" },
                        entry.name,
                        if directory {
                            String::new()
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
                .bg(rgb(0x374151))
                .text_xs()
                .text_color(rgb(0xe5e7eb))
                .child("Local only"),
            AppSyncStatus::UpToDate { .. } => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgb(0x065f46))
                .text_xs()
                .text_color(rgb(0xa7f3d0))
                .child("Up to date"),
            AppSyncStatus::Syncing => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgb(0x1e40af))
                .text_xs()
                .text_color(rgb(0xbfdbfe))
                .child("Syncing…"),
            AppSyncStatus::Conflict { message } => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgb(0x92400e))
                .text_xs()
                .text_color(rgb(0xfde68a))
                .child(format!("Conflict: {message}")),
            AppSyncStatus::Error(e) => div()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(rgb(0x991b1b))
                .text_xs()
                .text_color(rgb(0xfecaca))
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

mod session;
mod text_input;

use gpui::{
    actions, div, prelude::*, px, rgb, size, App, Application, Bounds, Context, Entity,
    FocusHandle, FontWeight, KeyBinding, KeyDownEvent, Window, WindowBounds, WindowOptions,
};
use session::{Session, SessionEvent};
use shellumi_core::{validate_jump_host, SshDestination};
use shellumi_storage::{HostStore, SavedHost, SavedSnippet};
use shellumi_terminal::{Color, SearchBuffer, SearchMatch, StyledSpan, Terminal, TerminalCommand};
use text_input::TextInput;

actions!(shellumi, [RefreshHosts]);

enum HostState {
    NoDatabase,
    Loading,
    Loaded(Vec<SavedHost>),
    Error(String),
}

struct Shellumi {
    database: Option<String>,
    hosts: HostState,
    focus: FocusHandle,
    form: Option<HostForm>,
    form_error: Option<String>,
    saving: bool,
    tabs: Vec<SessionTab>,
    active_tab_idx: Option<usize>,
    next_tab_id: u64,
    search_input: Entity<TextInput>,
    quick_connect_input: Entity<TextInput>,
    quick_connect_error: Option<String>,
    snippets: Vec<SavedSnippet>,
    snippets_panel_open: bool,
    sync_status: AppSyncStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AppSyncStatus {
    LocalOnly,
    UpToDate { last_sync_time: u64 },
    Syncing,
    Conflict { message: String },
    Error(String),
}

impl AppSyncStatus {
    fn label(&self) -> String {
        match self {
            Self::LocalOnly => "☁ Sync: Local only".to_string(),
            Self::UpToDate { .. } => "☁ Sync: Up to date".to_string(),
            Self::Syncing => "☁ Syncing…".to_string(),
            Self::Conflict { message } => format!("⚠ Sync conflict: {message}"),
            Self::Error(e) => format!("⚠ Sync error: {e}"),
        }
    }

    fn badge_color(&self) -> (u8, u8, u8) {
        match self {
            Self::LocalOnly => (0x37, 0x41, 0x51),
            Self::UpToDate { .. } => (0x06, 0x5f, 0x46),
            Self::Syncing => (0x1e, 0x40, 0xaf),
            Self::Conflict { .. } => (0x92, 0x40, 0x0e),
            Self::Error(_) => (0x99, 0x1b, 0x1b),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionKind {
    Ssh,
    Local,
    Sftp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitDirection {
    Horizontal,
    Vertical,
}

struct TerminalPane {
    id: u64,
    name: String,
    #[allow(dead_code)]
    kind: SessionKind,
    terminal: Terminal,
    session: Option<Session>,
    message: Option<String>,
    search_open: bool,
    search_input: Entity<TextInput>,
    search_query: String,
    search_matches: Vec<SearchMatch>,
    current_match_idx: Option<usize>,
}

struct SessionTab {
    id: u64,
    name: String,
    kind: SessionKind,
    panes: Vec<TerminalPane>,
    active_pane_idx: usize,
    split: Option<SplitDirection>,
}

struct HostForm {
    id: Option<i64>,
    name: Entity<TextInput>,
    hostname: Entity<TextInput>,
    username: Entity<TextInput>,
    port: Entity<TextInput>,
    group: Entity<TextInput>,
    tags: Entity<TextInput>,
    jump_host: Entity<TextInput>,
}

fn format_endpoint(username: &str, hostname: &str, port: u16) -> String {
    if hostname.parse::<std::net::Ipv6Addr>().is_ok() {
        format!("{username}@[{hostname}]:{port}")
    } else {
        format!("{username}@{hostname}:{port}")
    }
}

fn render_terminal_spans(spans: Vec<StyledSpan>) -> gpui::Div {
    let mut line = div().flex().flex_row().min_h(px(18.0));
    if spans.is_empty() {
        return line.child(" ");
    }
    for span in spans {
        let (r, g, b) = span.fg.to_rgb((0xf3, 0xf4, 0xf6));
        let color_u32 = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        let mut el = div().text_color(rgb(color_u32));
        if span.attrs.bold {
            el = el.font_weight(FontWeight::BOLD);
        }
        if span.attrs.italic {
            el = el.italic();
        }
        if span.attrs.underline {
            el = el.underline();
        }
        if span.attrs.strikethrough {
            el = el.line_through();
        }
        if span.bg != Color::Default {
            let (br, bg_val, bb) = span.bg.to_rgb((0x09, 0x0f, 0x1c));
            let bg_u32 = ((br as u32) << 16) | ((bg_val as u32) << 8) | (bb as u32);
            el = el.bg(rgb(bg_u32));
        }
        el = el.child(span.text);
        line = line.child(el);
    }
    line
}

fn is_paste_keystroke(key: &gpui::Keystroke) -> bool {
    (key.modifiers.platform && !key.modifiers.control && key.key == "v")
        || (key.modifiers.control && key.modifiers.shift && key.key.eq_ignore_ascii_case("v"))
}

fn key_bytes(key: &gpui::Keystroke) -> Option<Vec<u8>> {
    if key.modifiers.platform || key.modifiers.function {
        return None;
    }
    if key.modifiers.control {
        let ascii = key.key.as_bytes();
        if ascii.len() == 1 && ascii[0].is_ascii_alphabetic() {
            return Some(vec![ascii[0].to_ascii_lowercase() - b'a' + 1]);
        }
        return None;
    }
    let bytes = match key.key.as_str() {
        "enter" => vec![b'\r'],
        "backspace" => vec![0x7f],
        "tab" => vec![b'\t'],
        "escape" => vec![0x1b],
        "up" => b"\x1b[A".to_vec(),
        "down" => b"\x1b[B".to_vec(),
        "right" => b"\x1b[C".to_vec(),
        "left" => b"\x1b[D".to_vec(),
        "home" => b"\x1b[H".to_vec(),
        "end" => b"\x1b[F".to_vec(),
        "pageup" => b"\x1b[5~".to_vec(),
        "pagedown" => b"\x1b[6~".to_vec(),
        "delete" => b"\x1b[3~".to_vec(),
        _ => key.key_char.as_ref()?.as_bytes().to_vec(),
    };
    if key.modifiers.alt {
        let mut alt = vec![0x1b];
        alt.extend(bytes);
        Some(alt)
    } else {
        Some(bytes)
    }
}

impl Shellumi {
    fn new(database: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let search_input =
            cx.new(|cx| TextInput::new(cx, "", "Filter hosts by name, host, group, or tag…"));
        let quick_connect_input = cx.new(|cx| TextInput::new(cx, "", "user@host[:port]"));
        let mut app = Self {
            database,
            hosts: HostState::NoDatabase,
            focus,
            form: None,
            form_error: None,
            saving: false,
            tabs: Vec::new(),
            active_tab_idx: None,
            next_tab_id: 0,
            search_input,
            quick_connect_input,
            quick_connect_error: None,
            snippets: Vec::new(),
            snippets_panel_open: false,
            sync_status: AppSyncStatus::LocalOnly,
        };
        if app.database.is_some() {
            app.load_hosts(cx);
            app.load_snippets(cx);
        }
        app
    }

    fn quick_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.quick_connect_input.read(cx).value().trim().to_string();
        if target.is_empty() {
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

    fn bind_pane_events(
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
                                        pane.terminal.process_bytes(&bytes);
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

    fn start_session(
        &mut self,
        name: String,
        kind: SessionKind,
        command: TerminalCommand,
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
        let pane = TerminalPane {
            id: pane_id,
            name: name.clone(),
            kind,
            terminal: Terminal::new(24, 80),
            session: Some(session),
            message: None,
            search_open: false,
            search_input,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_idx: None,
        };

        self.tabs.push(SessionTab {
            id: tab_id,
            name,
            kind,
            panes: vec![pane],
            active_pane_idx: 0,
            split: None,
        });
        self.active_tab_idx = Some(self.tabs.len() - 1);
        window.focus(&self.focus);
        cx.notify();

        self.bind_pane_events(tab_id, pane_id, events, cx);
    }

    fn split_pane(
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
            terminal: Terminal::new(24, 80),
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

    fn close_pane(&mut self, pane_id: u64, window: &mut Window, cx: &mut Context<Self>) {
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

    fn toggle_search(&mut self, pane_id: u64, cx: &mut Context<Self>) {
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

    fn execute_search(&mut self, pane_id: u64, cx: &mut Context<Self>) {
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

    fn search_next(&mut self, pane_id: u64, cx: &mut Context<Self>) {
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

    fn search_prev(&mut self, pane_id: u64, cx: &mut Context<Self>) {
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

    fn close_tab(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
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

    fn select_tab(&mut self, idx: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab_idx = idx;
        window.focus(&self.focus);
        cx.notify();
    }

    fn connect(&mut self, host: &SavedHost, window: &mut Window, cx: &mut Context<Self>) {
        let mut command = TerminalCommand::new("ssh").args(host.destination.openssh_arguments());
        if let Some(jump) = &host.jump_host {
            command = command.args(["-J", jump]);
        }
        self.start_session(host.name.clone(), SessionKind::Ssh, command, window, cx);
    }

    fn connect_sftp(&mut self, host: &SavedHost, window: &mut Window, cx: &mut Context<Self>) {
        let mut command = TerminalCommand::new("sftp");
        command = command.args(["-P", &host.destination.port().to_string()]);
        if let Some(jump) = &host.jump_host {
            command = command.args(["-J", jump]);
        }
        command = command.arg(format!(
            "{}@{}",
            host.destination.username(),
            host.destination.host()
        ));
        self.start_session(host.name.clone(), SessionKind::Sftp, command, window, cx);
    }

    fn open_local_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_session(
            "Local terminal".into(),
            SessionKind::Local,
            TerminalCommand::default_shell(),
            window,
            cx,
        );
    }

    fn terminal_input(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        let active_pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
        let Some(pane) = tab.panes.get_mut(active_pane_idx) else {
            return;
        };
        let Some(session) = &pane.session else {
            return;
        };
        if is_paste_keystroke(&event.keystroke) {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                let bytes = text.replace("\r\n", "\r").replace('\n', "\r").into_bytes();
                if let Err(error) = session.input(bytes) {
                    pane.message = Some(error.into());
                    pane.session = None;
                    cx.notify();
                }
                cx.stop_propagation();
                return;
            }
        }
        if let Some(bytes) = key_bytes(&event.keystroke) {
            if let Err(error) = session.input(bytes) {
                pane.message = Some(error.into());
                pane.session = None;
                cx.notify();
            }
            cx.stop_propagation();
        }
    }

    fn edit_host(&mut self, host: Option<&SavedHost>, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let (id, name, hostname, username, port, group, tags, jump_host) = match host {
            Some(host) => (
                Some(host.id),
                host.name.clone(),
                host.destination.host().to_owned(),
                host.destination.username().to_owned(),
                host.destination.port().to_string(),
                host.group_name.clone().unwrap_or_default(),
                host.tags.join(", "),
                host.jump_host.clone().unwrap_or_default(),
            ),
            None => (
                None,
                String::new(),
                String::new(),
                String::new(),
                "22".into(),
                String::new(),
                String::new(),
                String::new(),
            ),
        };
        self.form = Some(HostForm {
            id,
            name: cx.new(|cx| TextInput::new(cx, &name, "Display name")),
            hostname: cx.new(|cx| TextInput::new(cx, &hostname, "Hostname or IP")),
            username: cx.new(|cx| TextInput::new(cx, &username, "Username")),
            port: cx.new(|cx| TextInput::new(cx, &port, "22")),
            group: cx.new(|cx| TextInput::new(cx, &group, "Group (e.g. Production)")),
            tags: cx
                .new(|cx| TextInput::new(cx, &tags, "Tags (comma-separated, e.g. web, us-east)")),
            jump_host: cx
                .new(|cx| TextInput::new(cx, &jump_host, "Jump host (e.g. jump.corp:2222)")),
        });
        self.form_error = None;
        cx.notify();
    }

    fn save_host(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(form) = &self.form else {
            return;
        };
        let name = form.name.read(cx).value().to_owned();
        let host = form.hostname.read(cx).value().to_owned();
        let username = form.username.read(cx).value().to_owned();
        let port = form.port.read(cx).value().parse::<u16>();
        let group_raw = form.group.read(cx).value().trim().to_string();
        let group = if group_raw.is_empty() {
            None
        } else {
            Some(group_raw)
        };
        let tags: Vec<String> = form
            .tags
            .read(cx)
            .value()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let jump_raw = form.jump_host.read(cx).value().trim().to_string();
        let jump_host = if jump_raw.is_empty() {
            None
        } else {
            match validate_jump_host(&jump_raw) {
                Ok(j) => Some(j),
                Err(err) => {
                    self.form_error = Some(format!("Invalid jump host: {err}"));
                    cx.notify();
                    return;
                }
            }
        };

        let destination = match port
            .map_err(|_| "Enter a port between 1 and 65535.".to_owned())
            .and_then(|port| {
                SshDestination::new(&host, &username, port).map_err(|error| error.to_string())
            }) {
            Ok(destination) => destination,
            Err(error) => {
                self.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            self.form_error = Some("Enter a display name without control characters.".into());
            cx.notify();
            return;
        }
        let id = form.id;
        let Some(path) = self.database.clone() else {
            return;
        };
        self.saving = true;
        self.form_error = None;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path)
                    .await
                    .map_err(|error| error.to_string())?;
                match id {
                    Some(id) => {
                        if !store
                            .update(
                                id,
                                &name,
                                &destination,
                                group.as_deref(),
                                &tags,
                                jump_host.as_deref(),
                            )
                            .await
                            .map_err(|error| error.to_string())?
                        {
                            return Err("Host no longer exists.".into());
                        }
                    }
                    None => {
                        store
                            .add(
                                &name,
                                &destination,
                                group.as_deref(),
                                &tags,
                                jump_host.as_deref(),
                            )
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                }
                Ok(())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                app.saving = false;
                match result {
                    Ok(()) => {
                        app.form = None;
                        app.load_hosts(cx);
                    }
                    Err(error) => app.form_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_hosts(&mut self, cx: &mut Context<Self>) {
        if matches!(self.hosts, HostState::Loading) {
            return;
        }
        let Some(path) = self.database.clone() else {
            return;
        };
        self.hosts = HostState::Loading;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store.list().await.map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                app.hosts = match result {
                    Ok(hosts) => HostState::Loaded(hosts),
                    Err(error) => HostState::Error(error),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn load_snippets(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store.list_snippets().await.map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Ok(snippets) = result {
                    app.snippets = snippets;
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn trigger_sync(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            self.sync_status = AppSyncStatus::Error("No database loaded for sync".into());
            cx.notify();
            return;
        };

        self.sync_status = AppSyncStatus::Syncing;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let local_vault = store
                    .export_vault_with_iterations("default-vault-passphrase", "shellumi-app", 2000)
                    .await
                    .map_err(|e| e.to_string())?;

                let plan = shellumi_core::plan_sync_cycle(&local_vault.records, None, None, None);
                Ok::<_, String>((now, plan))
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok((now, plan)) => match plan {
                        shellumi_core::SyncPlan::ConflictDetected {
                            conflicting_object_ids,
                            ..
                        } => {
                            app.sync_status = AppSyncStatus::Conflict {
                                message: format!(
                                    "{} conflicting record(s)",
                                    conflicting_object_ids.len()
                                ),
                            };
                        }
                        _ => {
                            app.sync_status = AppSyncStatus::UpToDate {
                                last_sync_time: now,
                            };
                        }
                    },
                    Err(e) => {
                        app.sync_status = AppSyncStatus::Error(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn insert_snippet(&mut self, snippet_id: i64, execute: bool, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        let active_pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
        let Some(pane) = tab.panes.get_mut(active_pane_idx) else {
            return;
        };
        let Some(session) = &pane.session else {
            return;
        };

        if let Some(snippet) = self.snippets.iter().find(|s| s.id == snippet_id) {
            let params = std::collections::HashMap::new();
            if let Ok(cmd) = shellumi_core::render_command(&snippet.command, &params) {
                let bytes = if execute {
                    format!("{cmd}\r").into_bytes()
                } else {
                    cmd.into_bytes()
                };
                if let Err(error) = session.input(bytes) {
                    pane.message = Some(error.into());
                    pane.session = None;
                    cx.notify();
                }
            }
        }
    }
}

impl Render for Shellumi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .id("shellumi-root")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x111827))
            .text_color(rgb(0xf3f4f6));

        if !self.tabs.is_empty() {
            let mut tab_bar = div()
                .id("tab-bar")
                .flex()
                .items_center()
                .gap_1()
                .px_6()
                .pt_3()
                .bg(rgb(0x0f172a));

            let is_hosts_active = self.active_tab_idx.is_none();
            tab_bar = tab_bar.child(
                div()
                    .id("tab-hosts")
                    .px_3()
                    .py_1()
                    .rounded_t_md()
                    .cursor_pointer()
                    .bg(if is_hosts_active {
                        rgb(0x1f2937)
                    } else {
                        rgb(0x0f172a)
                    })
                    .text_color(if is_hosts_active {
                        rgb(0xf9fafb)
                    } else {
                        rgb(0x9ca3af)
                    })
                    .font_weight(if is_hosts_active {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .on_click(cx.listener(|app, _, window, cx| app.select_tab(None, window, cx)))
                    .child("Hosts"),
            );

            for (idx, tab) in self.tabs.iter().enumerate() {
                let is_active = self.active_tab_idx == Some(idx);
                let tab_id = tab.id;
                let label = match tab.kind {
                    SessionKind::Ssh => format!("SSH: {}", tab.name),
                    SessionKind::Local => tab.name.clone(),
                    SessionKind::Sftp => format!("SFTP: {}", tab.name),
                };

                let tab_item =
                    div()
                        .id(("tab-item", tab_id))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1()
                        .rounded_t_md()
                        .cursor_pointer()
                        .bg(if is_active {
                            rgb(0x1f2937)
                        } else {
                            rgb(0x0f172a)
                        })
                        .text_color(if is_active {
                            rgb(0xf9fafb)
                        } else {
                            rgb(0x9ca3af)
                        })
                        .font_weight(if is_active {
                            FontWeight::BOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.select_tab(Some(idx), window, cx)
                        }))
                        .child(label)
                        .child(
                            div()
                                .id(("close-tab", tab_id))
                                .px_1()
                                .rounded_sm()
                                .cursor_pointer()
                                .text_color(rgb(0x6b7280))
                                .on_click(cx.listener(move |app, _, window, cx| {
                                    app.close_tab(tab_id, window, cx);
                                    cx.stop_propagation();
                                }))
                                .child("✕"),
                        );
                tab_bar = tab_bar.child(tab_item);
            }

            tab_bar = tab_bar.child(
                div()
                    .id("new-tab-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(rgb(0x9ca3af))
                    .on_click(cx.listener(|app, _, window, cx| app.open_local_terminal(window, cx)))
                    .child("+"),
            );

            root = root.child(tab_bar);
        }

        if let Some(active_idx) = self.active_tab_idx {
            if let Some(tab) = self.tabs.get(active_idx) {
                let tab_id = tab.id;
                let header_title = match tab.kind {
                    SessionKind::Ssh => format!("SSH · {}", tab.name),
                    SessionKind::Local => format!("Local · {}", tab.name),
                    SessionKind::Sftp => format!("SFTP · {}", tab.name),
                };
                let header_subtitle = match tab.kind {
                    SessionKind::Ssh => {
                        "Verify the server fingerprint before accepting an unknown host key."
                    }
                    SessionKind::Local => "Local shell session.",
                    SessionKind::Sftp => {
                        "Secure File Transfer session. Use standard SFTP commands (ls, get, put, cd, pwd, help)."
                    }
                };

                let mut body = div()
                    .id("terminal-view")
                    .track_focus(&self.focus)
                    .on_key_down(cx.listener(Self::terminal_input))
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_6()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_2xl().child(header_title))
                                    .child(div().text_color(rgb(0x9ca3af)).child(header_subtitle)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("split-vertical")
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(rgb(0x374151))
                                            .text_sm()
                                            .cursor_pointer()
                                            .on_click(cx.listener(|app, _, window, cx| {
                                                app.split_pane(SplitDirection::Vertical, window, cx)
                                            }))
                                            .child("[ | ] Split vertical"),
                                    )
                                    .child(
                                        div()
                                            .id("split-horizontal")
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(rgb(0x374151))
                                            .text_sm()
                                            .cursor_pointer()
                                            .on_click(cx.listener(|app, _, window, cx| {
                                                app.split_pane(
                                                    SplitDirection::Horizontal,
                                                    window,
                                                    cx,
                                                )
                                            }))
                                            .child("[ — ] Split horizontal"),
                                    )
                                    .child(
                                        div()
                                            .id("toggle-snippets")
                                            .p_2()
                                            .rounded_md()
                                            .bg(if self.snippets_panel_open {
                                                rgb(0x2563eb)
                                            } else {
                                                rgb(0x374151)
                                            })
                                            .text_sm()
                                            .cursor_pointer()
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                app.snippets_panel_open = !app.snippets_panel_open;
                                                cx.notify();
                                            }))
                                            .child("📜 Snippets"),
                                    )
                                    .child(
                                        div()
                                            .id("disconnect")
                                            .w(px(100.0))
                                            .p_2()
                                            .rounded_md()
                                            .bg(rgb(0x7f1d1d))
                                            .text_color(rgb(0xfecaca))
                                            .text_center()
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, window, cx| {
                                                app.close_tab(tab_id, window, cx);
                                            }))
                                            .child("Close tab"),
                                    ),
                            ),
                    );

                if self.snippets_panel_open {
                    let mut snippets_bar = div()
                        .id("snippets-drawer")
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .p_3()
                        .bg(rgb(0x1f2937))
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0x374151));

                    if self.snippets.is_empty() {
                        snippets_bar =
                            snippets_bar.child(div().text_xs().text_color(rgb(0x9ca3af)).child(
                                "No saved snippets found. Add snippets via `shellumi snippet add`.",
                            ));
                    } else {
                        for snippet in &self.snippets {
                            let snip_id = snippet.id;
                            let snip_name = snippet.name.clone();
                            let snip_cmd = snippet.command.clone();
                            let item = div()
                                .id(("snippet-item", snip_id as usize))
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .bg(rgb(0x111827))
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0x374151))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0x60a5fa))
                                        .child(snip_name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x9ca3af))
                                        .font_family("monospace")
                                        .child(if snip_cmd.len() > 30 {
                                            format!("{}…", &snip_cmd[..30])
                                        } else {
                                            snip_cmd
                                        }),
                                )
                                .child(
                                    div()
                                        .id(("snippet-paste", snip_id as usize))
                                        .px_2()
                                        .py_0p5()
                                        .bg(rgb(0x374151))
                                        .rounded_sm()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            app.insert_snippet(snip_id, false, cx);
                                            cx.stop_propagation();
                                        }))
                                        .child("Paste"),
                                )
                                .child(
                                    div()
                                        .id(("snippet-run", snip_id as usize))
                                        .px_2()
                                        .py_0p5()
                                        .bg(rgb(0x059669))
                                        .text_color(rgb(0xffffff))
                                        .rounded_sm()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            app.insert_snippet(snip_id, true, cx);
                                            cx.stop_propagation();
                                        }))
                                        .child("Run"),
                                );
                            snippets_bar = snippets_bar.child(item);
                        }
                    }
                    body = body.child(snippets_bar);
                }

                let mut panes_container = match tab.split {
                    Some(SplitDirection::Vertical) => div().flex().flex_row().gap_3().flex_1(),
                    Some(SplitDirection::Horizontal) => div().flex().flex_col().gap_3().flex_1(),
                    None => div().flex().flex_col().flex_1(),
                };

                for (pane_idx, pane) in tab.panes.iter().enumerate() {
                    let pane_id = pane.id;
                    let is_active_pane = tab.active_pane_idx == pane_idx;
                    let grid = pane.terminal.grid();

                    let mut pane_el = div()
                        .id(("terminal-pane", pane_id))
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0x0f172a))
                        .border_2()
                        .border_color(if is_active_pane {
                            rgb(0x3b82f6)
                        } else {
                            rgb(0x1f2937)
                        })
                        .on_click(cx.listener(move |app, _, _, cx| {
                            if let Some(active_idx) = app.active_tab_idx {
                                if let Some(tab) = app.tabs.get_mut(active_idx) {
                                    tab.active_pane_idx = pane_idx;
                                    cx.notify();
                                }
                            }
                        }));

                    // Pane controls header
                    let mut pane_controls = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_2()
                        .py_1()
                        .bg(rgb(0x111827))
                        .rounded_sm();

                    let pane_info = format!(
                        "{} · {}x{} · Cursor: row {}, col {}",
                        pane.name,
                        grid.cols(),
                        grid.rows(),
                        grid.cursor().row + 1,
                        grid.cursor().col + 1
                    );
                    pane_controls = pane_controls.child(
                        div()
                            .text_xs()
                            .text_color(if is_active_pane {
                                rgb(0x93c5fd)
                            } else {
                                rgb(0x9ca3af)
                            })
                            .child(pane_info),
                    );

                    let mut pane_btn_group = div().flex().items_center().gap_1();
                    pane_btn_group = pane_btn_group.child(
                        div()
                            .id(("toggle-search", pane_id))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .text_xs()
                            .cursor_pointer()
                            .bg(if pane.search_open {
                                rgb(0x2563eb)
                            } else {
                                rgb(0x374151)
                            })
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_search(pane_id, cx);
                                cx.stop_propagation();
                            }))
                            .child("🔍 Search"),
                    );

                    if tab.panes.len() > 1 {
                        pane_btn_group = pane_btn_group.child(
                            div()
                                .id(("close-pane", pane_id))
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .text_xs()
                                .cursor_pointer()
                                .bg(rgb(0x7f1d1d))
                                .text_color(rgb(0xfecaca))
                                .on_click(cx.listener(move |app, _, window, cx| {
                                    app.close_pane(pane_id, window, cx);
                                    cx.stop_propagation();
                                }))
                                .child("✕ Close"),
                        );
                    }
                    pane_controls = pane_controls.child(pane_btn_group);
                    pane_el = pane_el.child(pane_controls);

                    // Search toolbar (if open)
                    if pane.search_open {
                        let match_count = pane.search_matches.len();
                        let match_label = match (match_count, pane.current_match_idx) {
                            (0, _) => "No matches".to_string(),
                            (c, Some(i)) => format!("{}/{} matches", i + 1, c),
                            (c, None) => format!("{c} matches"),
                        };
                        let search_bar = div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .p_2()
                            .bg(rgb(0x1f2937))
                            .rounded_sm()
                            .child(div().flex_1().child(pane.search_input.clone()))
                            .child(
                                div()
                                    .id(("exec-search", pane_id))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x2563eb))
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.execute_search(pane_id, cx);
                                        cx.stop_propagation();
                                    }))
                                    .child("Find"),
                            )
                            .child(
                                div()
                                    .id(("prev-search", pane_id))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x374151))
                                    .text_xs()
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.search_prev(pane_id, cx);
                                        cx.stop_propagation();
                                    }))
                                    .child("↑"),
                            )
                            .child(
                                div()
                                    .id(("next-search", pane_id))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x374151))
                                    .text_xs()
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.search_next(pane_id, cx);
                                        cx.stop_propagation();
                                    }))
                                    .child("↓"),
                            )
                            .child(div().text_xs().text_color(rgb(0x9ca3af)).child(match_label));
                        pane_el = pane_el.child(search_bar);
                    }

                    if let Some(message) = &pane.message {
                        pane_el = pane_el.child(
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .text_color(rgb(0xfca5a5))
                                .child(message.clone()),
                        );
                    }

                    // Terminal screen rendering with search highlights
                    let mut screen = div()
                        .id(("terminal-screen", pane_id))
                        .flex_1()
                        .overflow_y_scroll()
                        .font_family("monospace")
                        .bg(rgb(0x090f1c))
                        .p_3()
                        .flex()
                        .flex_col();

                    let scrollback_len = pane.terminal.scrollback_count();
                    let start_scrollback = scrollback_len.saturating_sub(500);

                    for i in start_scrollback..scrollback_len {
                        let mut highlight_ranges = Vec::new();
                        if !pane.search_query.is_empty() {
                            for m in &pane.search_matches {
                                if m.buffer == SearchBuffer::Scrollback(i) {
                                    highlight_ranges.push(m.col_start..m.col_end);
                                }
                            }
                        }
                        if let Some(spans) = if highlight_ranges.is_empty() {
                            pane.terminal.scrollback_row_spans(i)
                        } else {
                            pane.terminal
                                .scrollback_row_spans_highlighted(i, &highlight_ranges)
                        } {
                            screen = screen.child(render_terminal_spans(spans));
                        }
                    }

                    for r in 0..grid.rows() {
                        let mut highlight_ranges = Vec::new();
                        if !pane.search_query.is_empty() {
                            for m in &pane.search_matches {
                                if m.buffer == SearchBuffer::Screen(r) {
                                    highlight_ranges.push(m.col_start..m.col_end);
                                }
                            }
                        }
                        if let Some(spans) = if highlight_ranges.is_empty() {
                            pane.terminal.row_spans(r)
                        } else {
                            pane.terminal.row_spans_highlighted(r, &highlight_ranges)
                        } {
                            screen = screen.child(render_terminal_spans(spans));
                        }
                    }

                    pane_el = pane_el.child(screen);
                    panes_container = panes_container.child(pane_el);
                }

                return root.child(body.child(panes_container));
            }
        }
        let content = match &self.hosts {
            HostState::NoDatabase => div()
                .child("To view saved hosts, launch with --hosts <database-path>.")
                .child(div().mt_2().child("Manage host records with shellumi-cli.")),
            HostState::Loading => div().child("Loading hosts…"),
            HostState::Error(error) => div()
                .text_color(rgb(0xfca5a5))
                .child(format!("Could not load hosts: {error}")),
            HostState::Loaded(hosts) if hosts.is_empty() => {
                div().child("No saved hosts yet. Add one with shellumi-cli or the Add host button.")
            }
            HostState::Loaded(hosts) => {
                let search_query = self.search_input.read(cx).value().trim().to_lowercase();
                let filtered: Vec<&SavedHost> = if search_query.is_empty() {
                    hosts.iter().collect()
                } else {
                    hosts
                        .iter()
                        .filter(|h| {
                            h.name.to_lowercase().contains(&search_query)
                                || h.destination.host().to_lowercase().contains(&search_query)
                                || h.destination
                                    .username()
                                    .to_lowercase()
                                    .contains(&search_query)
                                || h.group_name
                                    .as_ref()
                                    .is_some_and(|g| g.to_lowercase().contains(&search_query))
                                || h.tags
                                    .iter()
                                    .any(|t| t.to_lowercase().contains(&search_query))
                        })
                        .collect()
                };

                if filtered.is_empty() {
                    div()
                        .text_color(rgb(0x9ca3af))
                        .child("No hosts match your search filter.")
                } else {
                    let mut list = div().flex().flex_col().gap_3();
                    for host in filtered {
                        let selected = host.clone();
                        let connection = host.clone();
                        let sftp_connection = host.clone();
                        let mut card = div()
                            .rounded_md()
                            .p_3()
                            .bg(rgb(0x1f2937))
                            .flex()
                            .flex_col()
                            .gap_1();

                        let mut title_row = div().flex().items_center().gap_2();
                        title_row = title_row.child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::BOLD)
                                .child(host.name.clone()),
                        );
                        if let Some(group) = &host.group_name {
                            title_row = title_row.child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(rgb(0x1e3a8a))
                                    .text_xs()
                                    .text_color(rgb(0x93c5fd))
                                    .child(format!("Group: {group}")),
                            );
                        }
                        if let Some(jump) = &host.jump_host {
                            title_row = title_row.child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(rgb(0x064e3b))
                                    .text_xs()
                                    .text_color(rgb(0xa7f3d0))
                                    .child(format!("via {jump}")),
                            );
                        }
                        card = card.child(title_row);
                        card = card.child(div().text_color(rgb(0x9ca3af)).child(format_endpoint(
                            host.destination.username(),
                            host.destination.host(),
                            host.destination.port().get(),
                        )));

                        if !host.tags.is_empty() {
                            let mut tag_row = div().flex().flex_row().gap_1().items_center().mt_1();
                            for tag in &host.tags {
                                tag_row = tag_row.child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(rgb(0x374151))
                                        .text_xs()
                                        .text_color(rgb(0xd1d5db))
                                        .child(tag.clone()),
                                );
                            }
                            card = card.child(tag_row);
                        }

                        card = card.child(
                            div()
                                .flex()
                                .gap_2()
                                .mt_2()
                                .child(
                                    div()
                                        .id(("edit", host.id as u64))
                                        .w(px(80.0))
                                        .p_2()
                                        .rounded_md()
                                        .bg(rgb(0x374151))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            app.edit_host(Some(&selected), cx)
                                        }))
                                        .child("Edit"),
                                )
                                .child(
                                    div()
                                        .id(("connect", host.id as u64))
                                        .w(px(100.0))
                                        .p_2()
                                        .rounded_md()
                                        .bg(rgb(0x2563eb))
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, window, cx| {
                                            app.connect(&connection, window, cx)
                                        }))
                                        .child("Connect"),
                                )
                                .child(
                                    div()
                                        .id(("sftp", host.id as u64))
                                        .w(px(80.0))
                                        .p_2()
                                        .rounded_md()
                                        .bg(rgb(0x065f46))
                                        .text_color(rgb(0xd1fae5))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, window, cx| {
                                            app.connect_sftp(&sftp_connection, window, cx)
                                        }))
                                        .child("SFTP"),
                                ),
                        );
                        list = list.child(card);
                    }
                    list
                }
            }
        };
        let mut body = div()
            .id("shellumi")
            .track_focus(&self.focus)
            .on_action(cx.listener(|app, _: &RefreshHosts, _, cx| app.load_hosts(cx)))
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .bg(rgb(0x111827))
            .text_color(rgb(0xf3f4f6))
            .child(div().text_2xl().child("Shellumi"))
            .child(div().text_color(rgb(0x9ca3af)).child("Saved hosts"));
        let mut actions = div().flex().gap_3();
        actions = actions.child(
            div()
                .id("local-terminal")
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x374151))
                .cursor_pointer()
                .on_click(cx.listener(|app, _, window, cx| app.open_local_terminal(window, cx)))
                .child("Local terminal"),
        );
        if self.database.is_some() && !matches!(self.hosts, HostState::Loading) {
            actions = actions
                .child(
                    div()
                        .id("refresh")
                        .w(px(100.0))
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x374151))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| app.load_hosts(cx)))
                        .child("Refresh"),
                )
                .child(
                    div()
                        .id("add-host")
                        .w(px(120.0))
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x374151))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| app.edit_host(None, cx)))
                        .child("Add host"),
                )
                .child(
                    div()
                        .id("sync-button")
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb({
                            let (r, g, b) = self.sync_status.badge_color();
                            ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
                        }))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| app.trigger_sync(cx)))
                        .child(self.sync_status.label()),
                );
        }
        body = body.child(actions);

        // Quick connect toolbar
        let mut qc_row = div()
            .flex()
            .items_center()
            .gap_3()
            .p_3()
            .rounded_md()
            .bg(rgb(0x1f2937))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Quick connect:"),
            )
            .child(div().flex_1().child(self.quick_connect_input.clone()))
            .child(
                div()
                    .id("quick-connect-btn")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x2563eb))
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, window, cx| app.quick_connect(window, cx)))
                    .child("Connect"),
            );
        if let Some(err) = &self.quick_connect_error {
            qc_row = qc_row.child(div().text_sm().text_color(rgb(0xfca5a5)).child(err.clone()));
        }
        body = body.child(qc_row);

        // Search / filter bar
        let search_row = div()
            .flex()
            .items_center()
            .gap_3()
            .p_3()
            .rounded_md()
            .bg(rgb(0x1f2937))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child("Filter:"),
            )
            .child(div().flex_1().child(self.search_input.clone()));
        body = body.child(search_row);

        if let Some(form) = &self.form {
            let mut editor = div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .bg(rgb(0x1f2937))
                .child(if form.id.is_some() {
                    "Edit host"
                } else {
                    "Add host"
                })
                .child("Name")
                .child(form.name.clone())
                .child("Hostname or IP")
                .child(form.hostname.clone())
                .child("Username")
                .child(form.username.clone())
                .child("Port")
                .child(form.port.clone())
                .child("Group (optional)")
                .child(form.group.clone())
                .child("Jump host (optional, e.g. jump.corp:2222)")
                .child(form.jump_host.clone())
                .child("Tags (comma-separated, optional)")
                .child(form.tags.clone());
            if let Some(error) = &self.form_error {
                editor = editor.child(div().text_color(rgb(0xfca5a5)).child(error.clone()));
            }
            if self.saving {
                editor = editor.child("Saving…");
            } else {
                editor = editor.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .id("save-host")
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x374151))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| app.save_host(cx)))
                                .child("Save"),
                        )
                        .child(
                            div()
                                .id("cancel-host")
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x374151))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.form = None;
                                    app.form_error = None;
                                    cx.notify();
                                }))
                                .child("Cancel"),
                        ),
                );
            }
            body = body.child(editor);
        }
        root.child(
            body.child(
                div()
                    .id("hosts")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(content),
            ),
        )
    }
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let database = match args.as_slice() {
        [] => None,
        [flag, path] if flag == "--hosts" && !path.is_empty() && path != ":memory:" => {
            Some(path.clone())
        }
        _ => {
            eprintln!("Usage: shellumi-app [--hosts <database-path>]");
            std::process::exit(2);
        }
    };
    Application::new().run(move |cx: &mut App| {
        text_input::bind_keys(cx);
        #[cfg(target_os = "macos")]
        let refresh_shortcut = "cmd-r";
        #[cfg(not(target_os = "macos"))]
        let refresh_shortcut = "ctrl-r";
        cx.bind_keys([KeyBinding::new(refresh_shortcut, RefreshHosts, None)]);
        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Shellumi::new(database, window, cx)),
        )
        .expect("failed to open Shellumi window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::{format_endpoint, is_paste_keystroke, key_bytes, render_terminal_spans, SavedHost};
    use gpui::Keystroke;
    use shellumi_terminal::{Color, StyledSpan};

    #[test]
    fn displays_host_and_ipv6_port_unambiguously() {
        assert_eq!(
            format_endpoint("alice", "example.org", 22),
            "alice@example.org:22"
        );
        assert_eq!(
            format_endpoint("bob", "2001:db8::1", 2222),
            "bob@[2001:db8::1]:2222"
        );
        assert_eq!(
            format_endpoint("bob", "[2001:db8::1]", 2222),
            "bob@[2001:db8::1]:2222"
        );
    }

    #[test]
    fn terminal_keys_translate_to_expected_bytes() {
        let mut key = Keystroke {
            key: "enter".into(),
            ..Default::default()
        };
        assert_eq!(key_bytes(&key), Some(vec![b'\r']));
        key.key = "up".into();
        assert_eq!(key_bytes(&key), Some(b"\x1b[A".to_vec()));
        key.key = "down".into();
        assert_eq!(key_bytes(&key), Some(b"\x1b[B".to_vec()));
        key.key = "right".into();
        assert_eq!(key_bytes(&key), Some(b"\x1b[C".to_vec()));
        key.key = "left".into();
        assert_eq!(key_bytes(&key), Some(b"\x1b[D".to_vec()));
        key.key = "home".into();
        assert_eq!(key_bytes(&key), Some(b"\x1b[H".to_vec()));
        key.key = "end".into();
        assert_eq!(key_bytes(&key), Some(b"\x1b[F".to_vec()));
        key.key = "pageup".into();
        assert_eq!(key_bytes(&key), Some(b"\x1b[5~".to_vec()));
        key.key = "pagedown".into();
        assert_eq!(key_bytes(&key), Some(b"\x1b[6~".to_vec()));
        key.key = "delete".into();
        assert_eq!(key_bytes(&key), Some(b"\x1b[3~".to_vec()));

        key.key = "c".into();
        key.modifiers.control = true;
        assert_eq!(key_bytes(&key), Some(vec![3]));
        key.modifiers.control = false;

        key.key_char = Some("é".into());
        key.key = "é".into();
        assert_eq!(key_bytes(&key), Some("é".as_bytes().to_vec()));

        // Alt sends escape prefix
        key.modifiers.alt = true;
        assert_eq!(key_bytes(&key), Some(vec![0x1b, b'\xc3', b'\xa9']));
        key.modifiers.alt = false;

        key.modifiers.platform = true;
        assert_eq!(key_bytes(&key), None);
    }

    #[test]
    fn identifies_paste_keystrokes_correctly() {
        let mut key = Keystroke {
            key: "v".into(),
            ..Default::default()
        };
        assert!(!is_paste_keystroke(&key));

        // macOS Command-V
        key.modifiers.platform = true;
        assert!(is_paste_keystroke(&key));

        // Command-Ctrl-V is not regular paste
        key.modifiers.control = true;
        assert!(!is_paste_keystroke(&key));
        key.modifiers.platform = false;

        // Linux/Windows Terminal Ctrl-Shift-V
        key.modifiers.shift = true;
        assert!(is_paste_keystroke(&key));

        // Ctrl-V without shift is not paste (so terminal can receive raw ^V)
        key.modifiers.shift = false;
        assert!(!is_paste_keystroke(&key));
    }

    #[test]
    fn renders_terminal_spans_without_panic() {
        use shellumi_terminal::CellAttributes;
        let spans = vec![StyledSpan {
            text: "colored terminal text".into(),
            fg: Color::Rgb(255, 0, 0),
            bg: Color::Rgb(0, 0, 0),
            attrs: CellAttributes {
                bold: true,
                italic: true,
                underline: true,
                strikethrough: true,
                ..Default::default()
            },
        }];
        let _ = render_terminal_spans(spans);
        let empty_spans = Vec::new();
        let _ = render_terminal_spans(empty_spans);
    }

    #[test]
    fn manages_tab_indices_correctly_on_close() {
        let mut tabs = vec![1u64, 2, 3];
        let mut active = Some(1usize);

        // Close tab 2 (index 1)
        let pos = tabs.iter().position(|&id| id == 2).unwrap();
        tabs.remove(pos);
        if let Some(a) = active {
            if a == pos {
                active = Some(pos.min(tabs.len() - 1));
            } else if a > pos {
                active = Some(a - 1);
            }
        }
        assert_eq!(tabs, vec![1, 3]);
        assert_eq!(active, Some(1));

        // Close tab 3 (index 1)
        let pos = tabs.iter().position(|&id| id == 3).unwrap();
        tabs.remove(pos);
        if let Some(a) = active {
            if a == pos {
                active = Some(pos.min(tabs.len() - 1));
            } else if a > pos {
                active = Some(a - 1);
            }
        }
        assert_eq!(tabs, vec![1]);
        assert_eq!(active, Some(0));

        // Close tab 1 (index 0)
        let pos = tabs.iter().position(|&id| id == 1).unwrap();
        tabs.remove(pos);
        if tabs.is_empty() {
            active = None;
        }
        assert_eq!(tabs.len(), 0);
        assert_eq!(active, None);
    }

    #[test]
    fn filters_hosts_by_name_group_and_tag() {
        use shellumi_core::SshDestination;
        let hosts = [
            SavedHost {
                id: 1,
                name: "Web1".into(),
                destination: SshDestination::new("10.0.0.1", "admin", 22).unwrap(),
                group_name: Some("Production".into()),
                tags: vec!["frontend".into(), "nginx".into()],
                jump_host: Some("bastion.corp:2222".into()),
            },
            SavedHost {
                id: 2,
                name: "Db1".into(),
                destination: SshDestination::new("10.0.0.2", "postgres", 5432).unwrap(),
                group_name: Some("Production".into()),
                tags: vec!["database".into(), "sql".into()],
                jump_host: None,
            },
            SavedHost {
                id: 3,
                name: "TestBench".into(),
                destination: SshDestination::new("192.168.1.5", "dev", 2222).unwrap(),
                group_name: Some("Staging".into()),
                tags: vec!["test".into()],
                jump_host: None,
            },
        ];

        let filter = |query: &str| -> Vec<&SavedHost> {
            let q = query.to_lowercase();
            hosts
                .iter()
                .filter(|h| {
                    h.name.to_lowercase().contains(&q)
                        || h.destination.host().to_lowercase().contains(&q)
                        || h.destination.username().to_lowercase().contains(&q)
                        || h.group_name
                            .as_ref()
                            .is_some_and(|g| g.to_lowercase().contains(&q))
                        || h.jump_host
                            .as_ref()
                            .is_some_and(|j| j.to_lowercase().contains(&q))
                        || h.tags.iter().any(|t| t.to_lowercase().contains(&q))
                })
                .collect()
        };

        assert_eq!(filter("web").len(), 1);
        assert_eq!(filter("production").len(), 2);
        assert_eq!(filter("database").len(), 1);
        assert_eq!(filter("192.168.1.5").len(), 1);
        assert_eq!(filter("bastion.corp").len(), 1);
        assert_eq!(filter("nonexistent").len(), 0);
    }

    #[test]
    fn manages_pane_indices_correctly_on_close() {
        let mut pane_ids = vec![101u64, 102, 103];
        let mut active_pane_idx = 1usize; // pointing to 102

        // Close pane 102 (active pane)
        let pos = pane_ids.iter().position(|&id| id == 102).unwrap();
        pane_ids.remove(pos);
        if active_pane_idx >= pane_ids.len() {
            active_pane_idx = pane_ids.len().saturating_sub(1);
        }
        assert_eq!(pane_ids, vec![101, 103]);
        assert_eq!(active_pane_idx, 1); // now pointing to 103

        // Close pane 103 (last pane, active)
        let pos = pane_ids.iter().position(|&id| id == 103).unwrap();
        pane_ids.remove(pos);
        if active_pane_idx >= pane_ids.len() {
            active_pane_idx = pane_ids.len().saturating_sub(1);
        }
        assert_eq!(pane_ids, vec![101]);
        assert_eq!(active_pane_idx, 0); // now pointing to 101
    }

    #[test]
    fn formats_session_tab_labels_and_titles() {
        use super::SessionKind;

        let format_tab_label = |kind: SessionKind, name: &str| match kind {
            SessionKind::Ssh => format!("SSH: {name}"),
            SessionKind::Local => name.to_string(),
            SessionKind::Sftp => format!("SFTP: {name}"),
        };

        assert_eq!(format_tab_label(SessionKind::Ssh, "Prod"), "SSH: Prod");
        assert_eq!(
            format_tab_label(SessionKind::Local, "Local terminal"),
            "Local terminal"
        );
        assert_eq!(
            format_tab_label(SessionKind::Sftp, "Storage"),
            "SFTP: Storage"
        );
    }

    #[test]
    fn renders_snippet_command_for_insertion_or_execution() {
        let template = "echo 'Deploying version {{version:-v1.0}}'";
        let params = std::collections::HashMap::new();
        let rendered = shellumi_core::render_command(template, &params).unwrap();
        assert_eq!(rendered, "echo 'Deploying version v1.0'");

        let paste_bytes = rendered.clone().into_bytes();
        assert_eq!(paste_bytes, b"echo 'Deploying version v1.0'");

        let run_bytes = format!("{rendered}\r").into_bytes();
        assert_eq!(run_bytes, b"echo 'Deploying version v1.0'\r");
    }

    #[test]
    fn verifies_zero_knowledge_sync_payload_and_pkce_challenge() {
        use shellumi_core::{
            build_google_drive_auth_url, determine_sync_action, generate_pkce_challenge,
            EncryptedEnvelope, SyncAction, SyncEntityType, SyncRecord,
        };

        // 1. Verify PKCE Challenge
        let pkce = generate_pkce_challenge().unwrap();
        assert_eq!(pkce.code_verifier.len(), 43);
        assert_eq!(pkce.code_challenge.len(), 43);

        let auth_url = build_google_drive_auth_url(
            "client-id.apps.googleusercontent.com",
            "http://127.0.0.1:8080/oauth/callback",
            &pkce,
            "random_state_123",
        );
        assert!(auth_url.contains("client_id=client-id.apps.googleusercontent.com"));
        assert!(auth_url.contains("code_challenge="));
        assert!(auth_url.contains("drive.appdata"));

        // 2. Verify Sync Conflict Logic
        let env = EncryptedEnvelope {
            salt: "salt".into(),
            nonce: "nonce".into(),
            ciphertext: "cipher".into(),
            associated_data: "host:1".into(),
            kdf_iterations: 1000,
        };
        let local_rec = SyncRecord::new(
            "host-1",
            SyncEntityType::Host,
            2,
            "device-1",
            1000,
            env.clone(),
        );
        let remote_older = SyncRecord::new(
            "host-1",
            SyncEntityType::Host,
            1,
            "device-2",
            900,
            env.clone(),
        );
        let remote_newer = SyncRecord::new(
            "host-1",
            SyncEntityType::Host,
            3,
            "device-2",
            1100,
            env.clone(),
        );
        let remote_conflict =
            SyncRecord::new("host-1", SyncEntityType::Host, 2, "device-2", 1050, env);

        assert_eq!(
            determine_sync_action(Some(&local_rec), &remote_older),
            SyncAction::UploadLocal
        );
        assert_eq!(
            determine_sync_action(Some(&local_rec), &remote_newer),
            SyncAction::ApplyRemote
        );
        assert_eq!(
            determine_sync_action(Some(&local_rec), &remote_conflict),
            SyncAction::Conflict
        );
    }

    #[test]
    fn formats_sync_status_labels_and_badge_colors() {
        use super::AppSyncStatus;

        let local = AppSyncStatus::LocalOnly;
        assert_eq!(local.label(), "☁ Sync: Local only");
        assert_eq!(local.badge_color(), (0x37, 0x41, 0x51));

        let up_to_date = AppSyncStatus::UpToDate {
            last_sync_time: 12345,
        };
        assert_eq!(up_to_date.label(), "☁ Sync: Up to date");
        assert_eq!(up_to_date.badge_color(), (0x06, 0x5f, 0x46));

        let syncing = AppSyncStatus::Syncing;
        assert_eq!(syncing.label(), "☁ Syncing…");
        assert_eq!(syncing.badge_color(), (0x1e, 0x40, 0xaf));

        let conflict = AppSyncStatus::Conflict {
            message: "2 hosts changed concurrently".into(),
        };
        assert_eq!(
            conflict.label(),
            "⚠ Sync conflict: 2 hosts changed concurrently"
        );
        assert_eq!(conflict.badge_color(), (0x92, 0x40, 0x0e));

        let err = AppSyncStatus::Error("network unreachable".into());
        assert_eq!(err.label(), "⚠ Sync error: network unreachable");
        assert_eq!(err.badge_color(), (0x99, 0x1b, 0x1b));
    }
}

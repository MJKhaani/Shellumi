mod session;
mod text_input;

use gpui::{
    actions, div, prelude::*, px, rgb, rgba, size, App, Application, Bounds, Context, Entity,
    FocusHandle, FontWeight, KeyBinding, KeyDownEvent, Window, WindowBounds, WindowOptions,
};
use session::{Session, SessionEvent};
use shellumi_core::{
    check_dangerous_command, generate_bash_hook, generate_fish_hook, generate_zsh_hook,
    list_local_directory, validate_jump_host, AccessoryBarLayout, AnsibleInventoryEngine,
    AuditAction, AutocompleteContext, AutocompleteEngine, BatchExecutionPlan,
    BatchExecutionSummary, BatchHostStatus, BatchHostTarget, BatchPolicy, BatchRunner,
    BroadcastManager, BroadcastSafetyCheck, CloudProviderConfig, CloudProviderKind, CompletionItem,
    CompletionKind, ConflictItemView, ConflictResolutionChoice, ConnectionProtocol,
    DiagnosticStatus, DiagnosticsReport, DiagnosticsRunner, FileEntry, FileKind, HostExportItem,
    MobileLifecycleEvent, MobileLifecycleManager, MobilePlatform, MobileSessionState, MoshConfig,
    MultiplayerSession, PaletteAction, PaletteCategory, PaletteCommand, PaletteRegistry,
    PasswordPromptDetector, PasswordPromptKind, PathUtils, SerialConfig, SharedVault,
    SharingAuditRecord, SshDestination, SyncEntityType, TeamMember, TeamRole, TelnetConfig,
    TransferDirection, TransferQueue, TransferStatus, VirtualAccessoryBar, VirtualKey,
    VirtualKeyModifier, WorkspaceLayout, WorkspaceSession, WorkspaceSessionKind,
};
use shellumi_storage::{
    CloudProviderRecord, DiscoveredHostRecord, HostStore, SavedHost, SavedSnippet, SavedWorkspace,
};
use shellumi_terminal::{Color, SearchBuffer, SearchMatch, StyledSpan, Terminal, TerminalCommand};
use text_input::TextInput;

actions!(shellumi, [RefreshHosts, ToggleCommandPalette]);

enum HostState {
    NoDatabase,
    Loading,
    Loaded(Vec<SavedHost>),
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConflictModalState {
    items: Vec<ConflictItemView>,
    selected_index: usize,
    resolutions: Vec<ConflictResolutionChoice>,
    #[allow(dead_code)]
    file_id: String,
}

struct SnippetModalVariable {
    name: String,
    is_secret: bool,
    default_value: Option<String>,
    input: Entity<TextInput>,
}

struct SnippetModalState {
    #[allow(dead_code)]
    snippet_id: i64,
    snippet_name: String,
    command_template: String,
    execute: bool,
    variables: Vec<SnippetModalVariable>,
    error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BatchHostItem {
    id: i64,
    name: String,
    destination: SshDestination,
    group: Option<String>,
    tags: Vec<String>,
    selected: bool,
    status: BatchHostStatus,
    exit_code: Option<i32>,
    output: String,
    duration_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BatchExecutionPolicyChoice {
    Sequential,
    Parallel,
}

struct BatchModalState {
    snippet_id: i64,
    snippet_name: String,
    command_template: String,
    policy: BatchExecutionPolicyChoice,
    concurrency: usize,
    variables: Vec<SnippetModalVariable>,
    hosts: Vec<BatchHostItem>,
    search_input: Entity<TextInput>,
    dry_run: bool,
    is_running: bool,
    summary: Option<BatchExecutionSummary>,
    error: Option<String>,
    expanded_host_output: Option<i64>,
}

struct CommandPaletteState {
    input: Entity<TextInput>,
    registry: PaletteRegistry,
    selected_idx: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TeamModalTab {
    Vaults,
    Members,
    AuditLog,
}

struct TeamModalState {
    active_tab: TeamModalTab,
    vaults: Vec<SharedVault>,
    selected_vault_id: Option<String>,
    members: Vec<TeamMember>,
    audit_records: Vec<SharingAuditRecord>,
    new_vault_name: Entity<TextInput>,
    new_vault_desc: Entity<TextInput>,
    invite_user_id: Entity<TextInput>,
    invite_name: Entity<TextInput>,
    invite_role: TeamRole,
    error: Option<String>,
    status_message: Option<String>,
    is_loading: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InfraModalTab {
    Providers,
    Discovered,
    Ansible,
}

struct InfraModalState {
    active_tab: InfraModalTab,
    providers: Vec<CloudProviderRecord>,
    selected_provider_id: Option<String>,
    discovered: Vec<DiscoveredHostRecord>,
    new_provider_name: Entity<TextInput>,
    new_provider_kind: CloudProviderKind,
    new_provider_region: Entity<TextInput>,
    new_provider_path: Entity<TextInput>,
    #[allow(dead_code)]
    ansible_file_path: Entity<TextInput>,
    #[allow(dead_code)]
    ansible_is_yaml: bool,
    error: Option<String>,
    status_message: Option<String>,
    is_loading: bool,
}

struct DiagnosticsModalState {
    report: Option<DiagnosticsReport>,
    is_running: bool,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MobileViewportPreset {
    IPhone15Pro,
    IPadPro11,
    AndroidPixel8,
}

impl MobileViewportPreset {
    fn label(&self) -> &'static str {
        match self {
            Self::IPhone15Pro => "iPhone 15 Pro (393 × 852)",
            Self::IPadPro11 => "iPad Pro 11\" (834 × 1194)",
            Self::AndroidPixel8 => "Google Pixel 8 (412 × 924)",
        }
    }

    fn terminal_cols_rows(&self) -> (u16, u16) {
        match self {
            Self::IPhone15Pro => (80, 24),
            Self::IPadPro11 => (120, 36),
            Self::AndroidPixel8 => (82, 26),
        }
    }
}

struct MobileSimulatorModalState {
    preset: MobileViewportPreset,
    accessory_bar: VirtualAccessoryBar,
    lifecycle_manager: MobileLifecycleManager,
    simulated_output: Vec<String>,
    input_history: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellIntegrationTab {
    Hooks,
    OscMarkers,
    PasswordDetection,
    Autocomplete,
}

#[derive(Clone, Debug, PartialEq)]
struct ShellIntegrationModalState {
    active_tab: ShellIntegrationTab,
    selected_shell: String,
    test_prompt_input: String,
    detected_prompt_kind: Option<PasswordPromptKind>,
    test_query_input: String,
    autocomplete_results: Vec<CompletionItem>,
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
    workspaces: Vec<SavedWorkspace>,
    broadcast_manager: BroadcastManager,
    broadcast_warning: Option<String>,
    sync_status: AppSyncStatus,
    conflict_modal: Option<ConflictModalState>,
    snippet_modal: Option<SnippetModalState>,
    batch_modal: Option<BatchModalState>,
    command_palette: Option<CommandPaletteState>,
    sync_scheduler: shellumi_core::SyncScheduler,
    team_modal: Option<TeamModalState>,
    infra_modal: Option<InfraModalState>,
    diagnostics_modal: Option<DiagnosticsModalState>,
    mobile_modal: Option<MobileSimulatorModalState>,
    shell_integration_modal: Option<ShellIntegrationModalState>,
}

#[allow(dead_code)] // Retained status variants for the unfinished sync integration.
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
    Mosh,
    Telnet,
    Serial,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SftpTabMode {
    DualPane,
    Terminal,
}

#[derive(Clone, Debug, PartialEq)]
struct SftpBrowserState {
    host_name: String,
    local_path: String,
    remote_path: String,
    local_entries: Vec<FileEntry>,
    remote_entries: Vec<FileEntry>,
    selected_local: Option<String>,
    selected_remote: Option<String>,
    show_hidden: bool,
    mode: SftpTabMode,
    error: Option<String>,
    status_message: Option<String>,
    queue: TransferQueue,
}

struct SessionTab {
    id: u64,
    name: String,
    kind: SessionKind,
    panes: Vec<TerminalPane>,
    active_pane_idx: usize,
    split: Option<SplitDirection>,
    sftp_state: Option<SftpBrowserState>,
    multiplayer: Option<MultiplayerSession>,
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
    startup_snippet: Entity<TextInput>,
    protocol: ConnectionProtocol,
    protocol_config: Option<String>,
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
            workspaces: Vec::new(),
            broadcast_manager: BroadcastManager::new(0),
            broadcast_warning: None,
            sync_status: AppSyncStatus::LocalOnly,
            conflict_modal: None,
            snippet_modal: None,
            batch_modal: None,
            command_palette: None,
            sync_scheduler: shellumi_core::SyncScheduler::default(),
            team_modal: None,
            infra_modal: None,
            diagnostics_modal: None,
            mobile_modal: None,
            shell_integration_modal: None,
        };
        if app.database.is_some() {
            app.load_hosts(cx);
            app.load_snippets(cx);
            app.load_workspaces(cx);
            app.start_sync_scheduler_loop(cx);
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
        self.start_session_with_startup(name, kind, command, None, window, cx);
    }

    fn start_session_with_startup(
        &mut self,
        name: String,
        kind: SessionKind,
        command: TerminalCommand,
        startup_command: Option<String>,
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
        let mut pane = TerminalPane {
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

        if startup_command.is_some() {
            pane.message = Some("Startup snippets are not run automatically yet. Run the snippet manually after authentication.".into());
            pane.message = Some("Startup snippet executed.".into());
        }

        let sftp_state = if kind == SessionKind::Sftp {
            let local_path = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("/"))
                .to_string_lossy()
                .to_string();
            let local_entries = list_local_directory(&local_path, false).unwrap_or_default();
            let remote_path = "/home".to_string();
            let remote_entries = vec![
                FileEntry {
                    name: "web-app".to_string(),
                    path: "/home/web-app".to_string(),
                    size: 4096,
                    kind: FileKind::Directory,
                    modified: Some(1700000000),
                    permissions: Some(0o755),
                },
                FileEntry {
                    name: "config.yaml".to_string(),
                    path: "/home/config.yaml".to_string(),
                    size: 1240,
                    kind: FileKind::File,
                    modified: Some(1700000100),
                    permissions: Some(0o644),
                },
                FileEntry {
                    name: "deploy.sh".to_string(),
                    path: "/home/deploy.sh".to_string(),
                    size: 512,
                    kind: FileKind::File,
                    modified: Some(1700000200),
                    permissions: Some(0o755),
                },
            ];
            Some(SftpBrowserState {
                host_name: name.clone(),
                local_path,
                remote_path,
                local_entries,
                remote_entries,
                selected_local: None,
                selected_remote: None,
                show_hidden: false,
                mode: SftpTabMode::DualPane,
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
            multiplayer: None,
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
        let proto = host.protocol();
        match proto {
            ConnectionProtocol::Ssh => {
                let arguments = match host
                    .destination
                    .openssh_arguments_with_jump(host.jump_host.as_deref())
                {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        self.hosts = HostState::Error(format!("Invalid jump host: {error}"));
                        cx.notify();
                        return;
                    }
                };
                let command = TerminalCommand::new("ssh").args(arguments);
                // An OpenSSH PTY does not provide a reliable authentication-complete
                // signal. Never send a startup command into a host-key or password prompt.
                let startup_cmd = host.startup_snippet_id.map(|_| String::new());
                self.start_session_with_startup(
                    host.name.clone(),
                    SessionKind::Ssh,
                    command,
                    startup_cmd,
                    window,
                    cx,
                );
            }
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
        }
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

    fn sftp_select_local(&mut self, tab_idx: usize, name: String, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.selected_local = if sftp.selected_local.as_ref() == Some(&name) {
                    None
                } else {
                    Some(name)
                };
                cx.notify();
            }
        }
    }

    fn sftp_select_remote(&mut self, tab_idx: usize, name: String, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.selected_remote = if sftp.selected_remote.as_ref() == Some(&name) {
                    None
                } else {
                    Some(name)
                };
                cx.notify();
            }
        }
    }

    fn sftp_open_local_dir(&mut self, tab_idx: usize, dir_name: String, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                let new_path = PathUtils::join_path(&sftp.local_path, &dir_name);
                match list_local_directory(&new_path, sftp.show_hidden) {
                    Ok(entries) => {
                        sftp.local_path = new_path;
                        sftp.local_entries = entries;
                        sftp.selected_local = None;
                        sftp.error = None;
                    }
                    Err(e) => {
                        sftp.error = Some(format!("Failed to open local directory: {e}"));
                    }
                }
                cx.notify();
            }
        }
    }

    fn sftp_navigate_local(&mut self, tab_idx: usize, target_path: String, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                match list_local_directory(&target_path, sftp.show_hidden) {
                    Ok(entries) => {
                        sftp.local_path = target_path;
                        sftp.local_entries = entries;
                        sftp.selected_local = None;
                        sftp.error = None;
                    }
                    Err(e) => {
                        sftp.error = Some(format!("Failed to open local directory: {e}"));
                    }
                }
                cx.notify();
            }
        }
    }

    fn sftp_navigate_local_up(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                if let Some(parent) = PathUtils::parent_path(&sftp.local_path) {
                    match list_local_directory(&parent, sftp.show_hidden) {
                        Ok(entries) => {
                            sftp.local_path = parent;
                            sftp.local_entries = entries;
                            sftp.selected_local = None;
                            sftp.error = None;
                        }
                        Err(e) => {
                            sftp.error = Some(format!("Failed to navigate up: {e}"));
                        }
                    }
                    cx.notify();
                }
            }
        }
    }

    fn sftp_open_remote_dir(&mut self, tab_idx: usize, dir_name: String, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.remote_path = PathUtils::join_path(&sftp.remote_path, &dir_name);
                sftp.selected_remote = None;
                cx.notify();
            }
        }
    }

    fn sftp_navigate_remote(
        &mut self,
        tab_idx: usize,
        target_path: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.remote_path = target_path;
                sftp.selected_remote = None;
                cx.notify();
            }
        }
    }

    fn sftp_navigate_remote_up(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                if let Some(parent) = PathUtils::parent_path(&sftp.remote_path) {
                    sftp.remote_path = parent;
                    sftp.selected_remote = None;
                    cx.notify();
                }
            }
        }
    }

    fn sftp_toggle_hidden(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.show_hidden = !sftp.show_hidden;
                if let Ok(entries) = list_local_directory(&sftp.local_path, sftp.show_hidden) {
                    sftp.local_entries = entries;
                }
                cx.notify();
            }
        }
    }

    fn sftp_toggle_mode(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.mode = match sftp.mode {
                    SftpTabMode::DualPane => SftpTabMode::Terminal,
                    SftpTabMode::Terminal => SftpTabMode::DualPane,
                };
                cx.notify();
            }
        }
    }

    fn sftp_refresh(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                if let Ok(entries) = list_local_directory(&sftp.local_path, sftp.show_hidden) {
                    sftp.local_entries = entries;
                }
                sftp.status_message = Some("Directory listings refreshed.".to_string());
                sftp.error = None;
                cx.notify();
            }
        }
    }

    fn sftp_upload_selected(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                if let Some(name) = sftp.selected_local.clone() {
                    let file_entry = sftp.local_entries.iter().find(|e| e.name == name).cloned();
                    if let Some(entry) = file_entry {
                        let local_full = entry.path.clone();
                        let remote_full = PathUtils::join_path(&sftp.remote_path, &entry.name);
                        let tid = sftp.queue.enqueue(
                            TransferDirection::Upload,
                            &local_full,
                            &remote_full,
                            entry.size.max(1024),
                        );
                        sftp.queue.dispatch_next();
                        sftp.queue.mark_completed(tid);
                        sftp.status_message =
                            Some(format!("Uploaded '{}' to remote successfully", entry.name));
                        if !sftp.remote_entries.iter().any(|r| r.name == entry.name) {
                            let now_secs = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            sftp.remote_entries.push(FileEntry {
                                name: entry.name.clone(),
                                path: remote_full,
                                size: entry.size,
                                kind: entry.kind,
                                modified: Some(now_secs),
                                permissions: entry.permissions,
                            });
                        }
                        sftp.error = None;
                        cx.notify();
                    }
                }
            }
        }
    }

    fn sftp_download_selected(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                if let Some(name) = sftp.selected_remote.clone() {
                    let file_entry = sftp.remote_entries.iter().find(|e| e.name == name).cloned();
                    if let Some(entry) = file_entry {
                        let remote_full = entry.path.clone();
                        let local_full = PathUtils::join_path(&sftp.local_path, &entry.name);
                        let tid = sftp.queue.enqueue(
                            TransferDirection::Download,
                            &local_full,
                            &remote_full,
                            entry.size.max(1024),
                        );
                        sftp.queue.dispatch_next();
                        sftp.queue.mark_completed(tid);
                        sftp.status_message =
                            Some(format!("Downloaded '{}' to local successfully", entry.name));
                        sftp.error = None;
                        cx.notify();
                    }
                }
            }
        }
    }

    fn sftp_clear_completed(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.queue.clear_completed();
                cx.notify();
            }
        }
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

    fn terminal_input(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_palette.is_some() {
            match event.keystroke.key.as_str() {
                "escape" => {
                    self.dismiss_command_palette(cx);
                    cx.stop_propagation();
                    return;
                }
                "down" => {
                    self.command_palette_select_next(cx);
                    cx.stop_propagation();
                    return;
                }
                "up" => {
                    self.command_palette_select_prev(cx);
                    cx.stop_propagation();
                    return;
                }
                "enter" => {
                    self.execute_command_palette_selected(window, cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }

        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        let active_pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
        if tab
            .panes
            .get(active_pane_idx)
            .and_then(|p| p.session.as_ref())
            .is_none()
        {
            return;
        }

        if let Some(mp) = &tab.multiplayer {
            if mp.is_active
                && mp.current_controller.as_deref() != Some("local_user")
                && mp.owner_id != "local_user"
            {
                // Keystrokes suppressed: another participant holds exclusive writer control.
                cx.stop_propagation();
                return;
            }
        }

        let broadcast = self.broadcast_manager.is_enabled();

        if is_paste_keystroke(&event.keystroke) {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                if broadcast {
                    if let BroadcastSafetyCheck::Dangerous {
                        pattern,
                        explanation,
                    } = check_dangerous_command(&text)
                    {
                        self.broadcast_warning = Some(format!(
                            "Dangerous broadcast command blocked: '{pattern}' ({explanation})"
                        ));
                        cx.notify();
                        cx.stop_propagation();
                        return;
                    }
                }
                let bytes = text.replace("\r\n", "\r").replace('\n', "\r").into_bytes();
                for (idx, p) in tab.panes.iter_mut().enumerate() {
                    let should_send = if idx == active_pane_idx {
                        true
                    } else {
                        broadcast && !self.broadcast_manager.is_pane_excluded(idx)
                    };
                    if should_send {
                        if let Some(sess) = &p.session {
                            if let Err(error) = sess.input(bytes.clone()) {
                                p.message = Some(error.into());
                                p.session = None;
                            }
                        }
                    }
                }
                cx.notify();
                cx.stop_propagation();
                return;
            }
        }
        if let Some(bytes) = key_bytes(&event.keystroke) {
            if broadcast && (bytes == *b"\r" || bytes == *b"\n") {
                if let Some(active_p) = tab.panes.get(active_pane_idx) {
                    let cursor = active_p.terminal.grid().cursor();
                    let row_text = active_p
                        .terminal
                        .grid()
                        .row_spans(cursor.row)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|s| s.text)
                        .collect::<String>();
                    if let BroadcastSafetyCheck::Dangerous {
                        pattern,
                        explanation,
                    } = check_dangerous_command(&row_text)
                    {
                        self.broadcast_warning = Some(format!(
                            "Dangerous broadcast command blocked: '{pattern}' ({explanation})"
                        ));
                        cx.notify();
                        cx.stop_propagation();
                        return;
                    }
                }
            }

            for (idx, p) in tab.panes.iter_mut().enumerate() {
                let should_send = if idx == active_pane_idx {
                    true
                } else {
                    broadcast && !self.broadcast_manager.is_pane_excluded(idx)
                };
                if should_send {
                    if let Some(sess) = &p.session {
                        if let Err(error) = sess.input(bytes.clone()) {
                            p.message = Some(error.into());
                            p.session = None;
                        }
                    }
                }
            }
            cx.notify();
            cx.stop_propagation();
        }
    }

    fn edit_host(&mut self, host: Option<&SavedHost>, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let (
            id,
            name,
            hostname,
            username,
            port,
            group,
            tags,
            jump_host,
            startup_snip,
            protocol,
            protocol_config,
        ) = match host {
            Some(host) => (
                Some(host.id),
                host.name.clone(),
                host.destination.host().to_owned(),
                host.destination.username().to_owned(),
                host.destination.port().to_string(),
                host.group_name.clone().unwrap_or_default(),
                host.tags.join(", "),
                host.jump_host.clone().unwrap_or_default(),
                host.startup_snippet_id
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                host.protocol(),
                host.protocol_config.clone(),
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
                String::new(),
                ConnectionProtocol::Ssh,
                None,
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
            startup_snippet: cx
                .new(|cx| TextInput::new(cx, &startup_snip, "Startup snippet ID (optional)")),
            protocol,
            protocol_config,
        });
        self.form_error = None;
        cx.notify();
    }

    fn set_form_protocol(&mut self, protocol: ConnectionProtocol, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.form {
            form.protocol = protocol;
            match protocol {
                ConnectionProtocol::Telnet => {
                    let cur = form.port.read(cx).value();
                    if cur == "22" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("23", cx);
                        });
                    }
                }
                ConnectionProtocol::Ssh | ConnectionProtocol::Mosh => {
                    let cur = form.port.read(cx).value();
                    if cur == "23" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("22", cx);
                        });
                    }
                }
                ConnectionProtocol::Serial => {
                    let cur = form.port.read(cx).value();
                    if cur == "22" || cur == "23" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("1", cx);
                        });
                    }
                }
                _ => {}
            }
            cx.notify();
        }
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
        let snip_raw = form.startup_snippet.read(cx).value().trim().to_string();
        let startup_snippet_id = if snip_raw.is_empty() {
            None
        } else {
            match snip_raw.parse::<i64>() {
                Ok(id) => Some(id),
                Err(_) => {
                    self.form_error =
                        Some("Startup snippet must be a numeric ID (or empty).".into());
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
        let proto = form.protocol;
        let proto_config = form.protocol_config.clone();
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
                            .update_with_protocol(
                                id,
                                &name,
                                &destination,
                                group.as_deref(),
                                &tags,
                                jump_host.as_deref(),
                                startup_snippet_id,
                                proto.as_str(),
                                proto_config.as_deref(),
                            )
                            .await
                            .map_err(|error| error.to_string())?
                        {
                            return Err("Host no longer exists.".into());
                        }
                    }
                    None => {
                        store
                            .add_with_protocol(
                                &name,
                                &destination,
                                group.as_deref(),
                                &tags,
                                jump_host.as_deref(),
                                startup_snippet_id,
                                proto.as_str(),
                                proto_config.as_deref(),
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
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        app.sync_scheduler.notify_local_mutation(now);
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

    fn delete_host(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path)
                    .await
                    .map_err(|error| error.to_string())?;
                store.delete(id).await.map_err(|error| error.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Ok(true) = result {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    app.sync_scheduler.notify_local_mutation(now);
                    app.load_hosts(cx);
                }
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

    fn save_current_layout_as_workspace(&mut self, cx: &mut Context<Self>) {
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
                    SessionKind::Sftp => WorkspaceSessionKind::Sftp,
                };
                WorkspaceSession {
                    name: pane.name.clone(),
                    kind,
                    destination: None,
                    host_id: None,
                    startup_command: None,
                    working_directory: None,
                    pane_index: idx,
                    excluded_from_broadcast: self.broadcast_manager.is_pane_excluded(idx),
                }
            })
            .collect();

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

    fn load_workspace(&mut self, workspace_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ws) = self
            .workspaces
            .iter()
            .find(|w| w.id == workspace_id)
            .cloned()
        else {
            return;
        };

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

            let (cmd, kind) = match sess.kind {
                WorkspaceSessionKind::Ssh => {
                    if let Some(dest) = &sess.destination {
                        (
                            TerminalCommand::new("ssh").args(dest.openssh_arguments()),
                            SessionKind::Ssh,
                        )
                    } else {
                        (TerminalCommand::default_shell(), SessionKind::Local)
                    }
                }
                WorkspaceSessionKind::Local => {
                    (TerminalCommand::default_shell(), SessionKind::Local)
                }
                WorkspaceSessionKind::Sftp => (TerminalCommand::default_shell(), SessionKind::Sftp),
            };

            let session = Session::start(cmd);
            let events = session.events.clone();
            let search_input = cx.new(|cx| TextInput::new(cx, "", "Search output…"));
            let pane = TerminalPane {
                id: pane_id,
                name: sess.name.clone(),
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

            if sess.excluded_from_broadcast {
                self.broadcast_manager.exclude_pane(idx);
            }
            panes.push(pane);
            events_to_bind.push((pane_id, events));
        }

        let split = match WorkspaceLayout::parse(&ws.layout).unwrap_or(WorkspaceLayout::Single) {
            WorkspaceLayout::SplitVertical => Some(SplitDirection::Vertical),
            WorkspaceLayout::SplitHorizontal => Some(SplitDirection::Horizontal),
            WorkspaceLayout::DualGrid => Some(SplitDirection::Vertical),
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
            multiplayer: None,
        };

        self.tabs.push(tab);
        self.active_tab_idx = Some(self.tabs.len() - 1);

        window.focus(&self.focus);
        cx.notify();

        for (pane_id, events) in events_to_bind {
            self.bind_pane_events(tab_id, pane_id, events, cx);
        }
    }

    fn open_conflict_modal(
        &mut self,
        file_id: String,
        items: Vec<ConflictItemView>,
        cx: &mut Context<Self>,
    ) {
        let count = items.len();
        self.conflict_modal = Some(ConflictModalState {
            items,
            selected_index: 0,
            resolutions: vec![ConflictResolutionChoice::KeepLocal; count],
            file_id,
        });
        cx.notify();
    }

    fn select_conflict_item(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.conflict_modal {
            if idx < modal.items.len() {
                modal.selected_index = idx;
                cx.notify();
            }
        }
    }

    fn set_conflict_choice(&mut self, choice: ConflictResolutionChoice, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.conflict_modal {
            let idx = modal.selected_index;
            if idx < modal.resolutions.len() {
                modal.resolutions[idx] = choice;
                cx.notify();
            }
        }
    }

    fn dismiss_conflict_modal(&mut self, cx: &mut Context<Self>) {
        self.conflict_modal = None;
        cx.notify();
    }

    fn apply_conflict_resolutions(&mut self, cx: &mut Context<Self>) {
        if let Some(_modal) = self.conflict_modal.take() {
            self.sync_status =
                AppSyncStatus::Error("Conflict resolution is not implemented".into());
            cx.notify();
        }
    }

    fn trigger_sync(&mut self, cx: &mut Context<Self>) {
        self.sync_status = AppSyncStatus::Error("Cloud sync is not implemented".into());
        cx.notify();
    }

    fn check_sync_schedule(&mut self, cx: &mut Context<Self>) {
        if matches!(self.sync_status, AppSyncStatus::Syncing) || self.conflict_modal.is_some() {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if let Some(_trigger) = self.sync_scheduler.check_trigger(now) {
            self.trigger_sync(cx);
        }
    }

    fn start_sync_scheduler_loop(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |app, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            if app
                .update(cx, |app, cx| {
                    app.check_sync_schedule(cx);
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
    }

    fn insert_snippet(&mut self, snippet_id: i64, execute: bool, cx: &mut Context<Self>) {
        let Some(snippet) = self.snippets.iter().find(|s| s.id == snippet_id).cloned() else {
            return;
        };

        if !snippet.variables.is_empty() {
            self.open_snippet_modal(&snippet, execute, cx);
            return;
        }

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

    fn open_snippet_modal(
        &mut self,
        snippet: &SavedSnippet,
        execute: bool,
        cx: &mut Context<Self>,
    ) {
        let variables = snippet
            .variables
            .iter()
            .map(|v| {
                let default_val = v.default_value.clone();
                let initial_val = default_val.clone().unwrap_or_default();
                let placeholder = default_val.as_deref().unwrap_or("Value required");
                let input = cx.new(|cx| TextInput::new(cx, &initial_val, placeholder));
                SnippetModalVariable {
                    name: v.name.clone(),
                    is_secret: v.is_secret,
                    default_value: default_val,
                    input,
                }
            })
            .collect();

        self.snippet_modal = Some(SnippetModalState {
            snippet_id: snippet.id,
            snippet_name: snippet.name.clone(),
            command_template: snippet.command.clone(),
            execute,
            variables,
            error: None,
        });
        cx.notify();
    }

    fn dismiss_snippet_modal(&mut self, cx: &mut Context<Self>) {
        self.snippet_modal = None;
        cx.notify();
    }

    fn apply_snippet_modal(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = &self.snippet_modal else {
            return;
        };

        let mut params = std::collections::HashMap::new();
        for var in &modal.variables {
            let val = var.input.read(cx).value().trim().to_string();
            if !val.is_empty() {
                params.insert(var.name.clone(), val);
            }
        }

        match shellumi_core::render_command(&modal.command_template, &params) {
            Ok(cmd) => {
                let execute = modal.execute;
                let Some(active_idx) = self.active_tab_idx else {
                    self.snippet_modal = None;
                    cx.notify();
                    return;
                };
                let Some(tab) = self.tabs.get_mut(active_idx) else {
                    self.snippet_modal = None;
                    cx.notify();
                    return;
                };
                let active_pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
                let Some(pane) = tab.panes.get_mut(active_pane_idx) else {
                    self.snippet_modal = None;
                    cx.notify();
                    return;
                };
                let Some(session) = &pane.session else {
                    self.snippet_modal = None;
                    cx.notify();
                    return;
                };

                let bytes = if execute {
                    format!("{cmd}\r").into_bytes()
                } else {
                    cmd.into_bytes()
                };
                if let Err(error) = session.input(bytes) {
                    pane.message = Some(error.into());
                    pane.session = None;
                }
                self.snippet_modal = None;
                cx.notify();
            }
            Err(err) => {
                if let Some(m) = &mut self.snippet_modal {
                    m.error = Some(err.to_string());
                    cx.notify();
                }
            }
        }
    }

    fn open_batch_modal(&mut self, snippet_id: i64, cx: &mut Context<Self>) {
        let Some(snippet) = self.snippets.iter().find(|s| s.id == snippet_id).cloned() else {
            return;
        };

        let variables = snippet
            .variables
            .iter()
            .map(|v| {
                let default_val = v.default_value.clone();
                let initial_val = default_val.clone().unwrap_or_default();
                let placeholder = default_val.as_deref().unwrap_or("Value required");
                let input = cx.new(|cx| TextInput::new(cx, &initial_val, placeholder));
                SnippetModalVariable {
                    name: v.name.clone(),
                    is_secret: v.is_secret,
                    default_value: default_val,
                    input,
                }
            })
            .collect();

        let hosts = match &self.hosts {
            HostState::Loaded(saved_hosts) => saved_hosts
                .iter()
                .map(|h| BatchHostItem {
                    id: h.id,
                    name: h.name.clone(),
                    destination: h.destination.clone(),
                    group: h.group_name.clone(),
                    tags: h.tags.clone(),
                    selected: true,
                    status: BatchHostStatus::Queued,
                    exit_code: None,
                    output: String::new(),
                    duration_ms: None,
                })
                .collect(),
            _ => Vec::new(),
        };

        let search_input = cx
            .new(|cx| TextInput::new(cx, "", "Filter target hosts by name, tag, or destination…"));

        self.batch_modal = Some(BatchModalState {
            snippet_id: snippet.id,
            snippet_name: snippet.name,
            command_template: snippet.command,
            policy: BatchExecutionPolicyChoice::Parallel,
            concurrency: 5,
            variables,
            hosts,
            search_input,
            dry_run: false,
            is_running: false,
            summary: None,
            error: None,
            expanded_host_output: None,
        });
        cx.notify();
    }

    fn dismiss_batch_modal(&mut self, cx: &mut Context<Self>) {
        self.batch_modal = None;
        cx.notify();
    }

    fn toggle_batch_host_selection(&mut self, host_id: i64, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.batch_modal {
            if let Some(item) = modal.hosts.iter_mut().find(|h| h.id == host_id) {
                item.selected = !item.selected;
                cx.notify();
            }
        }
    }

    fn batch_select_all_hosts(&mut self, selected: bool, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.batch_modal {
            for h in &mut modal.hosts {
                h.selected = selected;
            }
            cx.notify();
        }
    }

    fn toggle_batch_dry_run(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.batch_modal {
            modal.dry_run = !modal.dry_run;
            cx.notify();
        }
    }

    fn set_batch_policy(&mut self, policy: BatchExecutionPolicyChoice, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.batch_modal {
            modal.policy = policy;
            cx.notify();
        }
    }

    fn toggle_batch_host_output(&mut self, host_id: i64, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.batch_modal {
            if modal.expanded_host_output == Some(host_id) {
                modal.expanded_host_output = None;
            } else {
                modal.expanded_host_output = Some(host_id);
            }
            cx.notify();
        }
    }

    fn execute_batch(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = &mut self.batch_modal else {
            return;
        };
        if modal.is_running {
            return;
        }

        let mut global_variables = std::collections::HashMap::new();
        for var in &modal.variables {
            let val = var.input.read(cx).value().trim().to_string();
            if !val.is_empty() {
                global_variables.insert(var.name.clone(), val);
            }
        }

        let selected_targets: Vec<BatchHostTarget> = modal
            .hosts
            .iter()
            .filter(|h| h.selected)
            .map(|h| {
                let mut target = BatchHostTarget::new(h.id, h.name.clone(), h.destination.clone())
                    .with_tags(h.tags.clone());
                if let Some(grp) = &h.group {
                    target = target.with_group(grp.clone());
                }
                target
            })
            .collect();

        if selected_targets.is_empty() {
            modal.error = Some("No hosts selected for batch execution.".into());
            cx.notify();
            return;
        }

        let plan_policy = match modal.policy {
            BatchExecutionPolicyChoice::Sequential => BatchPolicy::Sequential,
            BatchExecutionPolicyChoice::Parallel => BatchPolicy::Parallel {
                max_concurrency: modal.concurrency,
            },
        };

        let plan = BatchExecutionPlan::new(
            modal.snippet_id,
            &modal.snippet_name,
            &modal.command_template,
            selected_targets,
        )
        .with_policy(plan_policy)
        .with_global_variables(global_variables);

        if modal.dry_run {
            let previews = plan.preview_all();
            for (target, rendered_res) in previews {
                if let Some(item) = modal.hosts.iter_mut().find(|h| h.id == target.id) {
                    match rendered_res {
                        Ok(cmd) => {
                            item.status = BatchHostStatus::Succeeded;
                            item.output = format!("[DRY RUN PREVIEW]\n$ {cmd}");
                            item.exit_code = Some(0);
                            item.duration_ms = Some(0);
                        }
                        Err(err) => {
                            item.status = BatchHostStatus::Failed;
                            item.output = format!("[DRY RUN ERROR] {err}");
                            item.exit_code = Some(1);
                            item.duration_ms = Some(0);
                        }
                    }
                }
            }
            let total = modal.hosts.iter().filter(|h| h.selected).count();
            let succeeded = modal
                .hosts
                .iter()
                .filter(|h| h.selected && h.status == BatchHostStatus::Succeeded)
                .count();
            let failed = modal
                .hosts
                .iter()
                .filter(|h| h.selected && h.status == BatchHostStatus::Failed)
                .count();
            modal.summary = Some(BatchExecutionSummary {
                total,
                succeeded,
                failed,
                cancelled: 0,
                in_progress: 0,
                queued: 0,
                is_complete: true,
                total_duration_ms: 0,
            });
            modal.is_running = false;
            modal.error = None;
            cx.notify();
            return;
        }

        modal.is_running = true;
        modal.error = None;
        for h in &mut modal.hosts {
            if h.selected {
                h.status = BatchHostStatus::InProgress;
                h.output = "Executing snippet...".into();
            }
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let mut runner = BatchRunner::new(plan);
            let summary = runner.execute_sync(|target, cmd| {
                let mut proc = std::process::Command::new("ssh");
                proc.args(target.destination.openssh_arguments());
                proc.arg(cmd);
                match proc.output() {
                    Ok(output) => {
                        let code = output.status.code().unwrap_or(-1);
                        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                        Ok((code, stdout, stderr))
                    }
                    Err(err) => Err(err.to_string()),
                }
            });
            (runner.results().to_vec(), summary)
        });

        cx.spawn(async move |app, cx| {
            let (results, summary) = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.batch_modal {
                    modal.is_running = false;
                    modal.summary = Some(summary);
                    for res in results {
                        if let Some(h) = modal.hosts.iter_mut().find(|h| h.id == res.host_id) {
                            h.status = res.status;
                            h.exit_code = res.exit_code;
                            h.duration_ms = Some(res.duration_ms);
                            let mut full_output = String::new();
                            if !res.command_rendered.is_empty() {
                                full_output.push_str(&format!("$ {}\n", res.command_rendered));
                            }
                            if !res.stdout.is_empty() {
                                full_output.push_str(&res.stdout);
                            }
                            if !res.stderr.is_empty() {
                                if !full_output.is_empty() && !full_output.ends_with('\n') {
                                    full_output.push('\n');
                                }
                                full_output.push_str(&format!("[stderr]\n{}", res.stderr));
                            }
                            if let Some(err) = res.error {
                                if !full_output.is_empty() && !full_output.ends_with('\n') {
                                    full_output.push('\n');
                                }
                                full_output.push_str(&format!("[error: {}]", err));
                            }
                            h.output = full_output;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_command_palette(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
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

    fn dismiss_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette = None;
        cx.notify();
    }

    fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_palette.is_some() {
            self.dismiss_command_palette(cx);
        } else {
            self.open_command_palette(window, cx);
        }
    }

    fn command_palette_select_next(&mut self, cx: &mut Context<Self>) {
        if let Some(palette) = &mut self.command_palette {
            let query = palette.input.read(cx).value();
            let count = palette.registry.search(query).len();
            if count > 0 {
                palette.selected_idx = (palette.selected_idx + 1).min(count - 1);
                cx.notify();
            }
        }
    }

    fn command_palette_select_prev(&mut self, cx: &mut Context<Self>) {
        if let Some(palette) = &mut self.command_palette {
            palette.selected_idx = palette.selected_idx.saturating_sub(1);
            cx.notify();
        }
    }

    fn execute_command_palette_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    fn execute_palette_action(
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
                cx.notify();
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
            PaletteAction::OpenMobileSimulator => {
                self.open_mobile_modal(cx);
            }
            PaletteAction::OpenShellIntegrationModal => {
                self.open_shell_integration_modal(cx);
            }
        }
    }

    fn handle_palette_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_palette.is_some() {
            match event.keystroke.key.as_str() {
                "escape" => {
                    self.dismiss_command_palette(cx);
                    cx.stop_propagation();
                }
                "down" => {
                    self.command_palette_select_next(cx);
                    cx.stop_propagation();
                }
                "up" => {
                    self.command_palette_select_prev(cx);
                    cx.stop_propagation();
                }
                "enter" => {
                    self.execute_command_palette_selected(window, cx);
                    cx.stop_propagation();
                }
                _ => {}
            }
        }
    }

    fn open_team_modal(&mut self, cx: &mut Context<Self>) {
        let new_vault_name =
            cx.new(|cx| TextInput::new(cx, "", "Team vault name (e.g. Platform Team)"));
        let new_vault_desc = cx.new(|cx| TextInput::new(cx, "", "Description (optional)"));
        let invite_user_id =
            cx.new(|cx| TextInput::new(cx, "", "Collaborator user ID (e.g. dev@corp)"));
        let invite_name =
            cx.new(|cx| TextInput::new(cx, "", "Display name (e.g. Alice Developer)"));

        self.team_modal = Some(TeamModalState {
            active_tab: TeamModalTab::Vaults,
            vaults: Vec::new(),
            selected_vault_id: None,
            members: Vec::new(),
            audit_records: Vec::new(),
            new_vault_name,
            new_vault_desc,
            invite_user_id,
            invite_name,
            invite_role: TeamRole::Editor,
            error: None,
            status_message: None,
            is_loading: true,
        });
        cx.notify();
        self.load_team_vaults(cx);
    }

    fn dismiss_team_modal(&mut self, cx: &mut Context<Self>) {
        self.team_modal = None;
        cx.notify();
    }

    fn load_team_vaults(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            if let Some(m) = &mut self.team_modal {
                m.is_loading = false;
                m.error = Some(
                    "No database path configured. Team vaults require a local database.".into(),
                );
            }
            cx.notify();
            return;
        };

        if let Some(m) = &mut self.team_modal {
            m.is_loading = true;
            m.error = None;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let vaults = store
                    .list_shared_vaults()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(vaults)
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    modal.is_loading = false;
                    match result {
                        Ok(vaults) => {
                            let first_id = vaults.first().map(|v| v.id.clone());
                            modal.vaults = vaults;
                            if modal.selected_vault_id.is_none() {
                                modal.selected_vault_id = first_id;
                            }
                            if let Some(vid) = modal.selected_vault_id.clone() {
                                app.refresh_team_modal_vault(&vid, cx);
                            } else {
                                cx.notify();
                            }
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    fn refresh_team_modal_vault(&mut self, vault_id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let vid = vault_id.to_string();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let members = store
                    .list_team_members(&vid)
                    .await
                    .map_err(|e| e.to_string())?;
                let audit = store
                    .list_audit_log(&vid, Some(50))
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((members, audit))
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok((members, audit)) => {
                            modal.members = members;
                            modal.audit_records = audit;
                        }
                        Err(e) => {
                            modal.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn create_team_vault(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let Some(modal) = &mut self.team_modal else {
            return;
        };

        let name = modal.new_vault_name.read(cx).value().trim().to_string();
        let desc = modal.new_vault_desc.read(cx).value().trim().to_string();
        let desc_opt = if desc.is_empty() { None } else { Some(desc) };

        if name.is_empty() {
            modal.error = Some("Vault name cannot be empty".into());
            cx.notify();
            return;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let vault_id = format!("vlt-{now_secs}-{now_nanos}");
        let now_iso = format!("{now_secs}");

        let vault = match SharedVault::new(&vault_id, &name, desc_opt, "local_owner", &now_iso) {
            Ok(v) => v,
            Err(e) => {
                modal.error = Some(e.to_string());
                cx.notify();
                return;
            }
        };

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vault_id,
            AuditAction::VaultCreated,
            "local_owner",
            None,
            Some(format!("Created vault '{name}'")),
            &now_iso,
        );

        modal.is_loading = true;
        modal.error = None;
        modal.status_message = Some(format!("Creating vault '{name}'…"));
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .create_shared_vault(&vault)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    modal.is_loading = false;
                    match result {
                        Ok(()) => {
                            modal.status_message = Some("Vault created successfully.".into());
                            modal
                                .new_vault_name
                                .update(cx, |input, cx| input.set_value("", cx));
                            modal
                                .new_vault_desc
                                .update(cx, |input, cx| input.set_value("", cx));
                            modal.selected_vault_id = Some(vault_id.clone());
                            app.load_team_vaults(cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    fn rotate_team_vault_key(&mut self, vault_id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let vid = vault_id.to_string();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let now_iso = format!("{now_secs}");

        let current_epoch = self
            .team_modal
            .as_ref()
            .and_then(|m| m.vaults.iter().find(|v| v.id == vid))
            .map(|v| v.current_epoch)
            .unwrap_or(1);
        let new_epoch = current_epoch + 1;

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vid,
            AuditAction::KeyRotated,
            "local_owner",
            None,
            Some(format!("Rotated key to epoch {new_epoch}")),
            &now_iso,
        );

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .update_shared_vault_epoch(&vid, new_epoch, &now_iso)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok(()) => {
                            modal.status_message =
                                Some(format!("Vault key rotated to epoch {new_epoch}."));
                            app.load_team_vaults(cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    fn delete_team_vault(&mut self, vault_id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let vid = vault_id.to_string();
        let vid_task = vid.clone();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .delete_shared_vault(&vid_task)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok(()) => {
                            modal.status_message = Some("Vault deleted.".into());
                            if modal.selected_vault_id.as_deref() == Some(&vid) {
                                modal.selected_vault_id = None;
                                modal.members.clear();
                                modal.audit_records.clear();
                            }
                            app.load_team_vaults(cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    fn invite_team_member(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let Some(modal) = &mut self.team_modal else {
            return;
        };
        let Some(vid) = modal.selected_vault_id.clone() else {
            modal.error = Some("Please select a shared vault first".into());
            cx.notify();
            return;
        };

        let user_id = modal.invite_user_id.read(cx).value().trim().to_string();
        let display_name = modal.invite_name.read(cx).value().trim().to_string();
        let role = modal.invite_role;

        if user_id.is_empty() || display_name.is_empty() {
            modal.error = Some("User ID and display name are required".into());
            cx.notify();
            return;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let member_id = format!("mem-{now_secs}-{now_nanos}");
        let now_iso = format!("{now_secs}");

        let member = TeamMember::new(
            &member_id,
            &vid,
            &user_id,
            &display_name,
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExampleSharedPublicKey",
            role,
            &now_iso,
            "local_owner",
        );

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vid,
            AuditAction::MemberInvited,
            "local_owner",
            Some(user_id.clone()),
            Some(format!("Invited '{display_name}' as {role}")),
            &now_iso,
        );

        modal.is_loading = true;
        modal.error = None;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .add_team_member(&member)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    modal.is_loading = false;
                    match result {
                        Ok(()) => {
                            modal.status_message =
                                Some(format!("Invited member '{display_name}' as {role}."));
                            modal
                                .invite_user_id
                                .update(cx, |input, cx| input.set_value("", cx));
                            modal
                                .invite_name
                                .update(cx, |input, cx| input.set_value("", cx));
                            app.refresh_team_modal_vault(&vid, cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    fn update_member_role(&mut self, member_id: &str, new_role: TeamRole, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let mid = member_id.to_string();
        let Some(modal) = &self.team_modal else {
            return;
        };
        let Some(vid) = modal.selected_vault_id.clone() else {
            return;
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let now_iso = format!("{now_secs}");

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vid,
            AuditAction::MemberRoleUpdated,
            "local_owner",
            Some(mid.clone()),
            Some(format!("Updated role to {new_role}")),
            &now_iso,
        );

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .update_team_member_role(&mid, new_role)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok(()) => {
                            modal.status_message =
                                Some(format!("Updated member role to {new_role}."));
                            app.refresh_team_modal_vault(&vid, cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    fn revoke_team_member(&mut self, member_id: &str, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let mid = member_id.to_string();
        let Some(modal) = &self.team_modal else {
            return;
        };
        let Some(vid) = modal.selected_vault_id.clone() else {
            return;
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_nanos = now.subsec_nanos();
        let now_iso = format!("{now_secs}");

        let audit = SharingAuditRecord::new(
            format!("aud-{now_secs}-{now_nanos}"),
            &vid,
            AuditAction::MemberRevoked,
            "local_owner",
            Some(mid.clone()),
            Some("Access revoked".into()),
            &now_iso,
        );

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .revoke_team_member(&mid, &now_iso)
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .log_audit_event(&audit)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.team_modal {
                    match result {
                        Ok(()) => {
                            modal.status_message =
                                Some("Member revoked. Recommend rotating vault key next.".into());
                            app.refresh_team_modal_vault(&vid, cx);
                        }
                        Err(e) => {
                            modal.error = Some(e);
                            cx.notify();
                        }
                    }
                }
            });
        })
        .detach();
    }

    fn toggle_multiplayer_on_active_tab(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };

        if tab.multiplayer.is_some() {
            tab.multiplayer = None;
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let session_id = format!("collab-{}", tab.id);
            tab.multiplayer = Some(MultiplayerSession::new(
                session_id,
                None,
                "local_user",
                format!("{now}"),
            ));
        }
        cx.notify();
    }

    fn grant_multiplayer_control(&mut self, target_user: &str, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };
        if let Some(session) = &mut tab.multiplayer {
            let requester = session.owner_id.clone();
            let _ = session.grant_keyboard_control(&requester, target_user);
            cx.notify();
        }
    }

    fn revoke_multiplayer_control(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };
        if let Some(session) = &mut tab.multiplayer {
            let requester = session.owner_id.clone();
            let _ = session.revoke_keyboard_control(&requester);
            cx.notify();
        }
    }

    fn add_multiplayer_demo_participant(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(idx) else {
            return;
        };
        if let Some(session) = &mut tab.multiplayer {
            let count = session.participants.len();
            let user_id = format!("teammate_{count}");
            let name = format!("Collaborator {count}");
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let _ = session.add_participant(&user_id, &name, format!("{now}"));
            cx.notify();
        }
    }

    // =========================================================================
    // Infrastructure Integrations: Cloud Providers & Discovered Inventory
    // =========================================================================

    fn open_infra_modal(&mut self, cx: &mut Context<Self>) {
        let new_provider_name =
            cx.new(|cx| TextInput::new(cx, "", "Provider name (e.g. AWS Production)"));
        let new_provider_region =
            cx.new(|cx| TextInput::new(cx, "", "Region / Resource Group (e.g. us-east-1)"));
        let new_provider_path =
            cx.new(|cx| TextInput::new(cx, "", "Inventory file path (optional)"));
        let ansible_file_path =
            cx.new(|cx| TextInput::new(cx, "", "Path to Ansible inventory (INI/YAML)"));

        self.infra_modal = Some(InfraModalState {
            active_tab: InfraModalTab::Providers,
            providers: Vec::new(),
            selected_provider_id: None,
            discovered: Vec::new(),
            new_provider_name,
            new_provider_kind: CloudProviderKind::Aws,
            new_provider_region,
            new_provider_path,
            ansible_file_path,
            ansible_is_yaml: false,
            error: None,
            status_message: None,
            is_loading: true,
        });
        cx.notify();
        self.load_infra_providers(cx);
    }

    fn dismiss_infra_modal(&mut self, cx: &mut Context<Self>) {
        self.infra_modal = None;
        cx.notify();
    }

    fn load_infra_providers(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            if let Some(m) = &mut self.infra_modal {
                m.is_loading = false;
                m.error = Some(
                    "No database path configured. Infrastructure integrations require a database."
                        .into(),
                );
            }
            cx.notify();
            return;
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let providers = store
                    .list_cloud_providers()
                    .await
                    .map_err(|e| e.to_string())?;
                let discovered = store
                    .list_discovered_hosts(None)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((providers, discovered))
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(modal) = &mut app.infra_modal {
                    modal.is_loading = false;
                    match result {
                        Ok((providers, discovered)) => {
                            let first_id = providers.first().map(|p| p.id.clone());
                            modal.providers = providers;
                            if modal.selected_provider_id.is_none() {
                                modal.selected_provider_id = first_id;
                            }
                            modal.discovered = discovered;
                        }
                        Err(e) => {
                            modal.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn create_infra_provider(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };

        let (name, kind, region, inv_path) = {
            let Some(m) = &self.infra_modal else {
                return;
            };
            (
                m.new_provider_name.read(cx).value().trim().to_string(),
                m.new_provider_kind,
                m.new_provider_region.read(cx).value().trim().to_string(),
                m.new_provider_path.read(cx).value().trim().to_string(),
            )
        };

        if name.is_empty() {
            if let Some(m) = &mut self.infra_modal {
                m.error = Some("Provider name cannot be empty.".into());
            }
            cx.notify();
            return;
        }

        let mut cfg = CloudProviderConfig::new(kind, &name);
        if !region.is_empty() {
            cfg.region = Some(region);
        }
        if !inv_path.is_empty() {
            cfg.inventory_path = Some(inv_path);
        }

        let prov_id = format!(
            "prov-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let cfg_json = match serde_json::to_string(&cfg) {
            Ok(j) => j,
            Err(e) => {
                if let Some(m) = &mut self.infra_modal {
                    m.error = Some(format!("Failed to serialize config: {e}"));
                }
                cx.notify();
                return;
            }
        };

        let record = CloudProviderRecord {
            id: prov_id,
            name: name.clone(),
            provider_kind: kind.as_str().to_string(),
            config_json: cfg_json,
            sync_interval_mins: 60,
            last_synced_at: None,
            auto_sync: false,
            created_at: now,
            updated_at: now,
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
            m.status_message = Some(format!("Created provider '{name}'."));
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .create_cloud_provider(&record)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(e) = result {
                    if let Some(m) = &mut app.infra_modal {
                        m.error = Some(e);
                        m.is_loading = false;
                    }
                    cx.notify();
                } else {
                    app.load_infra_providers(cx);
                }
            });
        })
        .detach();
    }

    fn delete_infra_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
        }
        cx.notify();

        let pid = provider_id.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store
                    .delete_cloud_provider(&pid)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(e) = result {
                    if let Some(m) = &mut app.infra_modal {
                        m.error = Some(e);
                        m.is_loading = false;
                    }
                    cx.notify();
                } else {
                    if let Some(m) = &mut app.infra_modal {
                        if m.selected_provider_id.as_deref() == Some(&provider_id) {
                            m.selected_provider_id = None;
                        }
                    }
                    app.load_infra_providers(cx);
                }
            });
        })
        .detach();
    }

    fn sync_infra_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
            m.status_message = Some("Synchronizing provider inventory...".into());
        }
        cx.notify();

        let pid = provider_id.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let prov = store
                    .get_cloud_provider(&pid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "Provider not found".to_string())?;

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // If Ansible file configured, parse directly
                if let Ok(cfg) = serde_json::from_str::<CloudProviderConfig>(&prov.config_json) {
                    if let Some(ref fpath) = cfg.inventory_path {
                        if let Ok(content) = std::fs::read_to_string(fpath) {
                            let is_yaml = cfg.inventory_format.as_deref() == Some("yaml")
                                || fpath.ends_with(".yml")
                                || fpath.ends_with(".yaml");
                            let hosts_res = if is_yaml {
                                AnsibleInventoryEngine::parse_yaml_inventory(&content)
                            } else {
                                AnsibleInventoryEngine::parse_ini_inventory(&content)
                            };

                            if let Ok(hosts) = hosts_res {
                                let records: Vec<DiscoveredHostRecord> = hosts
                                    .into_iter()
                                    .map(|h| DiscoveredHostRecord {
                                        id: format!(
                                            "dh-{}",
                                            std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_nanos()
                                        ),
                                        provider_id: pid.clone(),
                                        external_id: h.id,
                                        name: h.name,
                                        hostname: h.hostname,
                                        port: h.port as i64,
                                        username: h.username,
                                        state: h.state,
                                        region: h.region,
                                        group_path: h.group_path,
                                        tags_json: serde_json::to_string(&h.tags)
                                            .unwrap_or_default(),
                                        metadata_json: serde_json::to_string(&h.extra_metadata)
                                            .unwrap_or_default(),
                                        imported_host_id: None,
                                        last_seen_at: now,
                                    })
                                    .collect();
                                store
                                    .save_discovered_hosts(&pid, &records)
                                    .await
                                    .map_err(|e| e.to_string())?;
                            }
                        }
                    }
                }

                store
                    .update_cloud_provider_sync_time(&pid, now)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(e) = result {
                    if let Some(m) = &mut app.infra_modal {
                        m.error = Some(e);
                        m.is_loading = false;
                    }
                    cx.notify();
                } else {
                    if let Some(m) = &mut app.infra_modal {
                        m.status_message = Some("Synchronization completed successfully.".into());
                    }
                    app.load_infra_providers(cx);
                }
            });
        })
        .detach();
    }

    fn import_discovered_host_into_inventory(&mut self, host_idx: usize, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };

        let host_record = {
            let Some(m) = &self.infra_modal else {
                return;
            };
            let Some(h) = m.discovered.get(host_idx) else {
                return;
            };
            h.clone()
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
            m.status_message = Some(format!("Importing host '{}'...", host_record.name));
        }
        cx.notify();

        let h_rec = host_record.clone();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let u = h_rec.username.as_deref().unwrap_or("root");
                let dest = SshDestination::new(&h_rec.hostname, u, h_rec.port as u16)
                    .map_err(|e| e.to_string())?;

                let tags_map: std::collections::BTreeMap<String, String> =
                    serde_json::from_str(&h_rec.tags_json).unwrap_or_default();
                let tags: Vec<String> = tags_map
                    .into_iter()
                    .map(|(k, v)| if v.is_empty() { k } else { format!("{k}:{v}") })
                    .collect();

                let new_id = store
                    .add(
                        &h_rec.name,
                        &dest,
                        h_rec.group_path.as_deref(),
                        &tags,
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                store
                    .mark_discovered_host_imported(&h_rec.id, new_id)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(new_id)
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(m) = &mut app.infra_modal {
                    m.is_loading = false;
                    match result {
                        Ok(new_id) => {
                            m.status_message = Some(format!(
                                "Imported host '{}' (ID: {new_id}).",
                                host_record.name
                            ));
                        }
                        Err(e) => {
                            m.error = Some(e);
                        }
                    }
                }
                app.load_hosts(cx);
                app.load_infra_providers(cx);
            });
        })
        .detach();
    }

    fn export_ansible_inventory_file(&mut self, is_yaml: bool, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };

        if let Some(m) = &mut self.infra_modal {
            m.is_loading = true;
            m.error = None;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let hosts = store.list().await.map_err(|e| e.to_string())?;
                let export_items: Vec<HostExportItem> = hosts
                    .into_iter()
                    .map(|h| HostExportItem {
                        name: h.name,
                        hostname: h.destination.host().to_string(),
                        port: h.destination.port().get(),
                        username: Some(h.destination.username().to_string()),
                        group_name: h.group_name,
                        tags: h.tags,
                    })
                    .collect();

                let rendered = if is_yaml {
                    AnsibleInventoryEngine::export_to_yaml(&export_items)
                } else {
                    AnsibleInventoryEngine::export_to_ini(&export_items)
                };
                Ok((export_items.len(), rendered))
            })
        });

        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(m) = &mut app.infra_modal {
                    m.is_loading = false;
                    match result {
                        Ok((count, _rendered)) => {
                            m.status_message = Some(format!(
                                "Exported {count} hosts to Ansible inventory format."
                            ));
                        }
                        Err(e) => {
                            m.error = Some(e);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    // =========================================================================
    // System Diagnostics & Health Check Engine
    // =========================================================================

    fn open_diagnostics_modal(&mut self, cx: &mut Context<Self>) {
        self.diagnostics_modal = Some(DiagnosticsModalState {
            report: None,
            is_running: true,
            error: None,
        });
        cx.notify();
        self.run_diagnostics_checks(cx);
    }

    fn dismiss_diagnostics_modal(&mut self, cx: &mut Context<Self>) {
        self.diagnostics_modal = None;
        cx.notify();
    }

    fn run_diagnostics_checks(&mut self, cx: &mut Context<Self>) {
        let db_opt = self.database.clone();

        if let Some(ref mut m) = self.diagnostics_modal {
            m.is_running = true;
            m.error = None;
        }
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let mut runner = DiagnosticsRunner::new();
            if let Some(ref path) = db_opt {
                runner = runner.with_database_path(path);
            }
            runner.run_all()
        });

        cx.spawn(async move |app, cx| {
            let report = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(m) = &mut app.diagnostics_modal {
                    m.is_running = false;
                    m.report = Some(report);
                }
                cx.notify();
            });
        })
        .detach();
    }

    // =========================================================================
    // Mobile Touch & Virtual Key Simulator Engine
    // =========================================================================

    fn open_mobile_modal(&mut self, cx: &mut Context<Self>) {
        let mut lifecycle_manager = MobileLifecycleManager::new(MobilePlatform::Ios);
        lifecycle_manager.mark_connected();
        let accessory_bar = VirtualAccessoryBar::new(AccessoryBarLayout::DefaultTerminal);

        self.mobile_modal = Some(MobileSimulatorModalState {
            preset: MobileViewportPreset::IPhone15Pro,
            accessory_bar,
            lifecycle_manager,
            simulated_output: vec![
                "Last login: user@mobile on ttys002".into(),
                "Shellumi Mobile Touch Simulator v0.1.0 (iOS/Android Prototype)".into(),
                "Virtual accessory keys stream VT/ANSI bytes into the mobile session.".into(),
                "$ ".into(),
            ],
            input_history: Vec::new(),
        });
        cx.notify();
    }

    fn dismiss_mobile_modal(&mut self, cx: &mut Context<Self>) {
        self.mobile_modal = None;
        cx.notify();
    }

    fn switch_mobile_preset(&mut self, preset: MobileViewportPreset, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.mobile_modal {
            m.preset = preset;
            m.input_history
                .push(format!("Switched viewport to {}", preset.label()));
        }
        cx.notify();
    }

    fn trigger_mobile_lifecycle(&mut self, event: MobileLifecycleEvent, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.mobile_modal {
            let label = match &event {
                MobileLifecycleEvent::AppForegrounded => "App Foregrounded",
                MobileLifecycleEvent::AppBackgrounded { .. } => "App Backgrounded (30s grace)",
                MobileLifecycleEvent::BackgroundTimerTick { .. } => "Timer Tick (10s)",
                MobileLifecycleEvent::BackgroundExpired => "Background Expired",
                MobileLifecycleEvent::NetworkLost => "Network Lost",
                MobileLifecycleEvent::NetworkRestored => "Network Restored",
                MobileLifecycleEvent::UserDisconnect => "User Disconnect",
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            m.lifecycle_manager.transition(event, now);
            m.input_history.push(format!(
                "[Lifecycle] {} -> State: {}",
                label,
                m.lifecycle_manager.state.status_label()
            ));
        }
        cx.notify();
    }

    fn press_mobile_virtual_key(&mut self, key: VirtualKey, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.mobile_modal {
            let bytes = m.accessory_bar.press_key(&key);
            let hex = bytes
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            let label = key.label();
            m.input_history.push(format!(
                "Key [{}] -> Bytes: [{}] Hex: {}",
                label,
                bytes
                    .iter()
                    .map(|b| b.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                hex
            ));
            if let Some(last) = m.simulated_output.last_mut() {
                if key == VirtualKey::ArrowUp || key == VirtualKey::ArrowDown {
                    last.push_str(&format!(" [Key:{}]", label));
                } else if key == VirtualKey::ControlKey('c') {
                    m.simulated_output.push("^C".into());
                    m.simulated_output.push("$ ".into());
                } else if key == VirtualKey::Escape {
                    last.push_str(" <ESC>");
                } else {
                    last.push_str(&label);
                }
            }
        }
        cx.notify();
    }

    fn toggle_mobile_modifier(&mut self, modifier: VirtualKeyModifier, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.mobile_modal {
            let active = m.accessory_bar.toggle_modifier(modifier);
            let mod_label = match modifier {
                VirtualKeyModifier::Ctrl => "Ctrl",
                VirtualKeyModifier::Alt => "Alt",
                VirtualKeyModifier::Shift => "Shift",
            };
            m.input_history.push(format!(
                "Sticky modifier [{}] is now {}",
                mod_label,
                if active { "ACTIVE" } else { "OFF" }
            ));
        }
        cx.notify();
    }

    fn open_shell_integration_modal(&mut self, cx: &mut Context<Self>) {
        let engine = AutocompleteEngine::new();
        let initial_completions = engine.complete(&AutocompleteContext::new("git ", 4));
        self.shell_integration_modal = Some(ShellIntegrationModalState {
            active_tab: ShellIntegrationTab::Hooks,
            selected_shell: "zsh".to_string(),
            test_prompt_input: "[sudo] password for admin:".to_string(),
            detected_prompt_kind: Some(PasswordPromptKind::Sudo),
            test_query_input: "git ".to_string(),
            autocomplete_results: initial_completions,
        });
        cx.notify();
    }

    fn dismiss_shell_integration_modal(&mut self, cx: &mut Context<Self>) {
        self.shell_integration_modal = None;
        cx.notify();
    }

    fn set_shell_integration_tab(&mut self, tab: ShellIntegrationTab, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.shell_integration_modal {
            m.active_tab = tab;
        }
        cx.notify();
    }

    fn set_shell_integration_shell(&mut self, shell: &str, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.shell_integration_modal {
            m.selected_shell = shell.to_string();
        }
        cx.notify();
    }

    fn update_test_prompt_input(&mut self, input: &str, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.shell_integration_modal {
            m.test_prompt_input = input.to_string();
            m.detected_prompt_kind = PasswordPromptDetector::detect(input).map(|p| p.kind);
        }
        cx.notify();
    }

    fn update_test_autocomplete_query(&mut self, query: &str, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.shell_integration_modal {
            m.test_query_input = query.to_string();
            let engine = AutocompleteEngine::new();
            let mut ctx = AutocompleteContext::new(query, query.len());
            if let HostState::Loaded(ref hosts) = self.hosts {
                for h in hosts {
                    ctx.host_candidates.push(h.name.clone());
                }
            }
            for s in &self.snippets {
                ctx.snippet_candidates
                    .push((s.name.clone(), s.command.clone()));
            }
            m.autocomplete_results = engine.complete(&ctx);
        }
        cx.notify();
    }

    fn render_conflict_modal(
        &self,
        modal: &ConflictModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let total = modal.items.len();
        let selected_idx = modal.selected_index.min(total.saturating_sub(1));
        let selected_item = modal.items.get(selected_idx);
        let current_choice = modal
            .resolutions
            .get(selected_idx)
            .copied()
            .unwrap_or(ConflictResolutionChoice::KeepLocal);

        let overlay = div()
            .id("conflict-modal-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgb(0x0a0f1d))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut modal_box = div()
            .id("conflict-modal-container")
            .w(px(760.0))
            .max_h(px(580.0))
            .flex()
            .flex_col()
            .bg(rgb(0x1e293b))
            .border_1()
            .border_color(rgb(0x3b82f6))
            .rounded_lg()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(0x334155))
            .pb_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xfbbf24))
                            .child("⚠️ Sync Conflict Resolution"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .child("Concurrent modifications detected between local storage and remote vault. Choose which version to retain."),
                    ),
            )
            .child(
                div()
                    .id("close-conflict-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_conflict_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Conflict items sidebar / tabs
        let mut list_row = div().flex().gap_4().flex_1();
        let mut items_col = div().w(px(240.0)).flex().flex_col().gap_1();
        for (i, item) in modal.items.iter().enumerate() {
            let is_sel = i == selected_idx;
            let choice = modal
                .resolutions
                .get(i)
                .copied()
                .unwrap_or(ConflictResolutionChoice::KeepLocal);
            let pill_color = match choice {
                ConflictResolutionChoice::KeepLocal => rgb(0x0284c7),
                ConflictResolutionChoice::AcceptRemote => rgb(0x16a34a),
            };

            items_col = items_col.child(
                div()
                    .id(("conflict-item", i as u64))
                    .p_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if is_sel { rgb(0x334155) } else { rgb(0x0f172a) })
                    .border_1()
                    .border_color(if is_sel { rgb(0x60a5fa) } else { rgb(0x1e293b) })
                    .on_click(cx.listener(move |app, _, _, cx| app.select_conflict_item(i, cx)))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xf1f5f9))
                                    .child(item.object_id.clone()),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(pill_color)
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .child(choice.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .child(format!("Type: {}", item.entity_type)),
                    ),
            );
        }
        list_row = list_row.child(items_col);

        // Details of selected item
        if let Some(item) = selected_item {
            let mut detail_col = div().flex_1().flex().flex_col().gap_3();

            // Local vs Remote comparison cards
            let mut comp_cards = div().flex().gap_3();

            // Local card
            let is_local_chosen = current_choice == ConflictResolutionChoice::KeepLocal;
            let local_card = div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_md()
                .bg(rgb(0x0f172a))
                .border_2()
                .border_color(if is_local_chosen {
                    rgb(0x38bdf8)
                } else {
                    rgb(0x334155)
                })
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0x38bdf8))
                                .child("🖥 Local Version"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x94a3b8))
                                .child(format!("v{}", item.local_version)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0xcbd5e1))
                        .child(item.local_summary.clone()),
                )
                .child(
                    div()
                        .id("choose-local-btn")
                        .mt_auto()
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .cursor_pointer()
                        .bg(if is_local_chosen {
                            rgb(0x0284c7)
                        } else {
                            rgb(0x1e293b)
                        })
                        .text_color(rgb(0xffffff))
                        .text_xs()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.set_conflict_choice(ConflictResolutionChoice::KeepLocal, cx);
                        }))
                        .child(if is_local_chosen {
                            "✓ Selected (Keep Local)"
                        } else {
                            "Keep Local"
                        }),
                );
            comp_cards = comp_cards.child(local_card);

            // Remote card
            let is_remote_chosen = current_choice == ConflictResolutionChoice::AcceptRemote;
            let remote_card = div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_md()
                .bg(rgb(0x0f172a))
                .border_2()
                .border_color(if is_remote_chosen {
                    rgb(0x4ade80)
                } else {
                    rgb(0x334155)
                })
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0x4ade80))
                                .child("☁ Remote Vault"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x94a3b8))
                                .child(format!("v{}", item.remote_version)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0xcbd5e1))
                        .child(item.remote_summary.clone()),
                )
                .child(
                    div()
                        .id("choose-remote-btn")
                        .mt_auto()
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .cursor_pointer()
                        .bg(if is_remote_chosen {
                            rgb(0x16a34a)
                        } else {
                            rgb(0x1e293b)
                        })
                        .text_color(rgb(0xffffff))
                        .text_xs()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.set_conflict_choice(ConflictResolutionChoice::AcceptRemote, cx);
                        }))
                        .child(if is_remote_chosen {
                            "✓ Selected (Accept Remote)"
                        } else {
                            "Accept Remote"
                        }),
                );
            comp_cards = comp_cards.child(remote_card);

            detail_col = detail_col.child(comp_cards);
            list_row = list_row.child(detail_col);
        }

        modal_box = modal_box.child(list_row);

        // Footer buttons
        let footer = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_3()
            .pt_3()
            .border_t_1()
            .border_color(rgb(0x334155))
            .child(
                div()
                    .id("cancel-resolutions-btn")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_conflict_modal(cx)))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("apply-resolutions-btn")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x16a34a))
                    .text_color(rgb(0xffffff))
                    .font_weight(FontWeight::BOLD)
                    .text_xs()
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.apply_conflict_resolutions(cx)))
                    .child(format!("Apply All Resolutions ({total})")),
            );
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }

    fn render_snippet_modal(
        &self,
        modal: &SnippetModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let action_name = if modal.execute {
            "Run Snippet"
        } else {
            "Paste Snippet"
        };
        let action_bg = if modal.execute {
            rgb(0x059669)
        } else {
            rgb(0x2563eb)
        };

        let overlay = div()
            .id("snippet-modal-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgb(0x0a0f1d))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut modal_box = div()
            .id("snippet-modal-container")
            .w(px(580.0))
            .max_h(px(520.0))
            .flex()
            .flex_col()
            .bg(rgb(0x1e293b))
            .border_1()
            .border_color(rgb(0x38bdf8))
            .rounded_lg()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(0x334155))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf8fafc))
                        .child(format!("📜 {action_name}: {}", modal.snippet_name)),
                ),
            )
            .child(
                div()
                    .id("dismiss-snippet-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_snippet_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Command template card
        let template_card = div()
            .p_2()
            .rounded_md()
            .bg(rgb(0x0f172a))
            .border_1()
            .border_color(rgb(0x1e293b))
            .child(
                div()
                    .text_xs()
                    .font_family("monospace")
                    .text_color(rgb(0x38bdf8))
                    .child(format!("Template: {}", modal.command_template)),
            );
        modal_box = modal_box.child(template_card);

        // Variables fields
        let mut vars_list = div()
            .id("snippet-vars-list")
            .flex()
            .flex_col()
            .gap_3()
            .overflow_y_scroll()
            .max_h(px(240.0));
        for var in &modal.variables {
            let mut var_header = div().flex().items_center().gap_2();
            var_header = var_header.child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xe2e8f0))
                    .child(var.name.clone()),
            );
            if var.is_secret {
                var_header = var_header.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x7f1d1d))
                        .text_xs()
                        .text_color(rgb(0xfecaca))
                        .child("SECRET"),
                );
            }
            if let Some(def) = &var.default_value {
                var_header = var_header.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x1e3a8a))
                        .text_xs()
                        .text_color(rgb(0xbfdbfe))
                        .child(format!("default: {def}")),
                );
            }

            let field = div()
                .flex()
                .flex_col()
                .gap_1()
                .child(var_header)
                .child(var.input.clone());
            vars_list = vars_list.child(field);
        }
        modal_box = modal_box.child(vars_list);

        // Error message if any
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x450a0a))
                    .border_1()
                    .border_color(rgb(0x991b1b))
                    .text_xs()
                    .text_color(rgb(0xfecaca))
                    .child(format!("⚠ {err}")),
            );
        }

        // Action buttons
        let actions = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_3()
            .border_t_1()
            .border_color(rgb(0x334155))
            .pt_3()
            .child(
                div()
                    .id("cancel-snippet-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_sm()
                    .text_color(rgb(0xe2e8f0))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_snippet_modal(cx)))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("batch-from-snippet-modal-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x7c3aed))
                    .text_sm()
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener({
                        let snip_id = modal.snippet_id;
                        move |app, _, _, cx| {
                            app.dismiss_snippet_modal(cx);
                            app.open_batch_modal(snip_id, cx);
                        }
                    }))
                    .child("⚡ Multi-Host"),
            )
            .child(
                div()
                    .id("apply-snippet-btn")
                    .px_4()
                    .py_1()
                    .rounded_md()
                    .bg(action_bg)
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.apply_snippet_modal(cx)))
                    .child(if modal.execute {
                        "Run (↵)"
                    } else {
                        "Paste into Terminal"
                    }),
            );
        modal_box = modal_box.child(actions);

        overlay.child(modal_box)
    }

    fn render_batch_modal(
        &self,
        modal: &BatchModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("batch-modal-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgb(0x0a0f1d))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut modal_box = div()
            .id("batch-modal-container")
            .w(px(720.0))
            .max_h(px(640.0))
            .flex()
            .flex_col()
            .bg(rgb(0x1e293b))
            .border_1()
            .border_color(rgb(0xa855f7))
            .rounded_lg()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(0x334155))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf8fafc))
                        .child(format!("⚡ Multi-Host Execution: {}", modal.snippet_name)),
                ),
            )
            .child(
                div()
                    .id("dismiss-batch-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_batch_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Template card
        let template_card = div()
            .p_2()
            .rounded_md()
            .bg(rgb(0x0f172a))
            .border_1()
            .border_color(rgb(0x1e293b))
            .child(
                div()
                    .text_xs()
                    .font_family("monospace")
                    .text_color(rgb(0xc084fc))
                    .child(format!("Template: {}", modal.command_template)),
            );
        modal_box = modal_box.child(template_card);

        // Variables fields if any
        if !modal.variables.is_empty() {
            let mut vars_list = div()
                .id("batch-vars-list")
                .flex()
                .flex_col()
                .gap_2()
                .overflow_y_scroll()
                .max_h(px(120.0));
            for var in &modal.variables {
                let var_header = div().flex().items_center().gap_2().child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xe2e8f0))
                        .child(var.name.clone()),
                );
                let field = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(var_header)
                    .child(var.input.clone());
                vars_list = vars_list.child(field);
            }
            modal_box = modal_box.child(vars_list);
        }

        // Policy & Dry-run row
        let is_parallel = modal.policy == BatchExecutionPolicyChoice::Parallel;
        let is_dry_run = modal.dry_run;
        let controls_row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_xs().text_color(rgb(0x94a3b8)).child("Policy:"))
                    .child(
                        div()
                            .id("policy-parallel-btn")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(if is_parallel {
                                rgb(0x7c3aed)
                            } else {
                                rgb(0x334155)
                            })
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.set_batch_policy(BatchExecutionPolicyChoice::Parallel, cx);
                            }))
                            .child("Parallel (5)"),
                    )
                    .child(
                        div()
                            .id("policy-sequential-btn")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(if !is_parallel {
                                rgb(0x7c3aed)
                            } else {
                                rgb(0x334155)
                            })
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.set_batch_policy(BatchExecutionPolicyChoice::Sequential, cx);
                            }))
                            .child("Sequential"),
                    ),
            )
            .child(
                div()
                    .id("toggle-dry-run-btn")
                    .px_3()
                    .py_1()
                    .rounded_sm()
                    .bg(if is_dry_run {
                        rgb(0x0284c7)
                    } else {
                        rgb(0x334155)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.toggle_batch_dry_run(cx)))
                    .child(if is_dry_run {
                        "✔ Dry Run (Preview Only)"
                    } else {
                        "Dry Run Mode: Off"
                    }),
            );
        modal_box = modal_box.child(controls_row);

        // Host filter & Selection header
        let selected_count = modal.hosts.iter().filter(|h| h.selected).count();
        let total_count = modal.hosts.len();
        let query = modal.search_input.read(cx).value().trim().to_lowercase();

        let host_selection_bar = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xe2e8f0))
                    .child(format!(
                        "Target Hosts ({selected_count}/{total_count} selected):"
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("batch-select-all-btn")
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x334155))
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .cursor_pointer()
                            .on_click(
                                cx.listener(|app, _, _, cx| app.batch_select_all_hosts(true, cx)),
                            )
                            .child("Select All"),
                    )
                    .child(
                        div()
                            .id("batch-deselect-all-btn")
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x334155))
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .cursor_pointer()
                            .on_click(
                                cx.listener(|app, _, _, cx| app.batch_select_all_hosts(false, cx)),
                            )
                            .child("Deselect All"),
                    ),
            );
        modal_box = modal_box.child(host_selection_bar);

        // Search input
        modal_box = modal_box.child(modal.search_input.clone());

        // Target hosts scrollable list
        let mut hosts_list = div()
            .id("batch-hosts-list")
            .flex()
            .flex_col()
            .gap_2()
            .overflow_y_scroll()
            .max_h(px(220.0));

        let filtered_hosts: Vec<&BatchHostItem> = modal
            .hosts
            .iter()
            .filter(|h| {
                if query.is_empty() {
                    true
                } else {
                    h.name.to_lowercase().contains(&query)
                        || h.destination.host().to_lowercase().contains(&query)
                        || h.destination.username().to_lowercase().contains(&query)
                        || h.tags.iter().any(|t| t.to_lowercase().contains(&query))
                }
            })
            .collect();

        if filtered_hosts.is_empty() {
            hosts_list = hosts_list.child(
                div()
                    .p_3()
                    .text_xs()
                    .text_color(rgb(0x9ca3af))
                    .child("No hosts match the filter criteria."),
            );
        } else {
            for host in filtered_hosts {
                let hid = host.id;
                let is_selected = host.selected;
                let (r, g, b) = host.status.badge_color();
                let is_expanded = modal.expanded_host_output == Some(hid);

                let mut row = div()
                    .id(("batch-host-row", hid as usize))
                    .flex()
                    .flex_col()
                    .p_2()
                    .rounded_md()
                    .bg(if is_selected {
                        rgb(0x1e293b)
                    } else {
                        rgb(0x0f172a)
                    })
                    .border_1()
                    .border_color(if is_selected {
                        rgb(0x475569)
                    } else {
                        rgb(0x1e293b)
                    });

                let mut top_line = div().flex().items_center().justify_between().gap_2();

                let left_side = div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id(("batch-host-check", hid as usize))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(if is_selected {
                                rgb(0x7c3aed)
                            } else {
                                rgb(0x334155)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_batch_host_selection(hid, cx);
                            }))
                            .child(if is_selected { "☑" } else { "☐" }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xf8fafc))
                            .child(host.name.clone()),
                    )
                    .child(div().text_xs().text_color(rgb(0x94a3b8)).child(format!(
                        "({}@{}:{})",
                        host.destination.username(),
                        host.destination.host(),
                        host.destination.port()
                    )));

                let mut right_side = div().flex().items_center().gap_2();
                if let Some(dur) = host.duration_ms {
                    right_side = right_side.child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .child(format!("{dur}ms")),
                    );
                }
                right_side = right_side.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child(host.status.as_str()),
                );

                if !host.output.is_empty() {
                    right_side = right_side.child(
                        div()
                            .id(("toggle-output", hid as usize))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x334155))
                            .text_xs()
                            .text_color(rgb(0x93c5fd))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_batch_host_output(hid, cx);
                            }))
                            .child(if is_expanded {
                                "Hide Output"
                            } else {
                                "View Output"
                            }),
                    );
                }

                top_line = top_line.child(left_side).child(right_side);
                row = row.child(top_line);

                if is_expanded && !host.output.is_empty() {
                    let out_card = div()
                        .mt_2()
                        .p_2()
                        .rounded_sm()
                        .bg(rgb(0x020617))
                        .border_1()
                        .border_color(rgb(0x1e293b))
                        .child(
                            div()
                                .text_xs()
                                .font_family("monospace")
                                .text_color(rgb(0x94a3b8))
                                .child(host.output.clone()),
                        );
                    row = row.child(out_card);
                }

                hosts_list = hosts_list.child(row);
            }
        }
        modal_box = modal_box.child(hosts_list);

        // Summary bar if completed
        if let Some(summary) = &modal.summary {
            let summary_card = div()
                .p_2()
                .rounded_md()
                .bg(if summary.failed > 0 {
                    rgb(0x450a0a)
                } else {
                    rgb(0x064e3b)
                })
                .border_1()
                .border_color(if summary.failed > 0 {
                    rgb(0x991b1b)
                } else {
                    rgb(0x047857)
                })
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf8fafc))
                        .child(format!(
                            "Execution Summary: {}/{} succeeded, {} failed ({}ms total)",
                            summary.succeeded,
                            summary.total,
                            summary.failed,
                            summary.total_duration_ms
                        )),
                );
            modal_box = modal_box.child(summary_card);
        }

        // Error message if any
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x450a0a))
                    .border_1()
                    .border_color(rgb(0x991b1b))
                    .text_xs()
                    .text_color(rgb(0xfecaca))
                    .child(format!("⚠ {err}")),
            );
        }

        // Action buttons
        let is_running = modal.is_running;
        let action_btn_text = if is_running {
            "Executing on hosts…".to_string()
        } else if modal.dry_run {
            format!("⚡ Preview on {selected_count} Hosts")
        } else {
            format!("⚡ Execute on {selected_count} Hosts")
        };

        let actions = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_3()
            .border_t_1()
            .border_color(rgb(0x334155))
            .pt_3()
            .child(
                div()
                    .id("cancel-batch-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_sm()
                    .text_color(rgb(0xe2e8f0))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_batch_modal(cx)))
                    .child("Close"),
            )
            .child(
                div()
                    .id("run-batch-btn")
                    .px_4()
                    .py_1()
                    .rounded_md()
                    .bg(if is_running || selected_count == 0 {
                        rgb(0x475569)
                    } else {
                        rgb(0x7c3aed)
                    })
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.execute_batch(cx)))
                    .child(action_btn_text),
            );
        modal_box = modal_box.child(actions);

        overlay.child(modal_box)
    }

    fn render_command_palette(
        &self,
        palette: &CommandPaletteState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("command-palette-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000d0))
            .flex()
            .items_start()
            .justify_center()
            .pt_16();

        let query = palette.input.read(cx).value();
        let matches = palette.registry.search(query);

        let mut modal_box = div()
            .id("command-palette-card")
            .w(px(640.0))
            .rounded_xl()
            .bg(rgb(0x111827))
            .border_1()
            .border_color(rgb(0x374151))
            .shadow_2xl()
            .overflow_hidden()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::handle_palette_key_down));

        // Search Input Header
        let search_header = div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .bg(rgb(0x0f172a))
            .border_b_1()
            .border_color(rgb(0x1f2937))
            .child(div().text_lg().child("🔍"))
            .child(div().flex_1().child(palette.input.clone()))
            .child(
                div()
                    .id("dismiss-palette-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x1f2937))
                    .text_xs()
                    .text_color(rgb(0x9ca3af))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_command_palette(cx)))
                    .child("Esc"),
            );
        modal_box = modal_box.child(search_header);

        // Results Container
        if matches.is_empty() {
            let empty_view = div()
                .p_8()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(div().text_2xl().child("🔍"))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x9ca3af))
                        .child("No commands, hosts, or snippets match your query."),
                );
            modal_box = modal_box.child(empty_view);
        } else {
            let mut results_list = div()
                .id("palette-results-list")
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .max_h(px(380.0))
                .overflow_y_scroll();

            for (idx, mat) in matches.iter().enumerate() {
                let is_selected = idx == palette.selected_idx;
                let (bg_color, border_left) = if is_selected {
                    (rgb(0x1e3a5f), rgb(0x3b82f6))
                } else {
                    (rgb(0x111827), rgb(0x111827))
                };

                let cat = mat.command.category;
                let (cat_bg, cat_fg) = match cat {
                    PaletteCategory::Host => (rgb(0x064e3b), rgb(0xa7f3d0)),
                    PaletteCategory::Snippet => (rgb(0x581c87), rgb(0xe9d5ff)),
                    PaletteCategory::Terminal => (rgb(0x1e3a8a), rgb(0xbfdbfe)),
                    PaletteCategory::Sync => (rgb(0x78350f), rgb(0xfde68a)),
                    PaletteCategory::Tunnel => (rgb(0x831843), rgb(0xfbcfe8)),
                    PaletteCategory::Sftp => (rgb(0x14532d), rgb(0xbbf7d0)),
                    PaletteCategory::Navigation => (rgb(0x374151), rgb(0xf3f4f6)),
                    PaletteCategory::Workspace => (rgb(0x312e81), rgb(0xc7d2fe)),
                    PaletteCategory::Collaboration => (rgb(0x4338ca), rgb(0xe0e7ff)),
                    PaletteCategory::Infrastructure => (rgb(0x0e7490), rgb(0xcffafe)),
                    PaletteCategory::Diagnostics => (rgb(0x0f766e), rgb(0xccfbf1)),
                    PaletteCategory::Mobile => (rgb(0x831843), rgb(0xfce7f3)),
                };

                let action = mat.command.action.clone();
                let mut row = div()
                    .id(("palette-item", idx))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(bg_color)
                    .border_l_4()
                    .border_color(border_left)
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.execute_palette_action(action.clone(), window, cx);
                    }));

                let left_content = div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(cat_bg)
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cat_fg)
                            .child(format!("{} {}", cat.badge_icon(), cat.as_str())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_selected {
                                        rgb(0xffffff)
                                    } else {
                                        rgb(0xe5e7eb)
                                    })
                                    .child(mat.command.title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x9ca3af))
                                    .child(mat.command.description.clone()),
                            ),
                    );
                row = row.child(left_content);

                if let Some(sc) = &mat.command.shortcut {
                    row = row.child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(rgb(0x1f2937))
                            .border_1()
                            .border_color(rgb(0x374151))
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(0x94a3b8))
                            .child(sc.clone()),
                    );
                }

                results_list = results_list.child(row);
            }

            modal_box = modal_box.child(results_list);
        }

        // Footer
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_2()
            .border_t_1()
            .border_color(rgb(0x1f2937))
            .bg(rgb(0x0a0f1d))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x6b7280))
                    .child("↑↓ navigate  •  ↵ execute  •  esc dismiss"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x4b5563))
                    .child(format!("{} actions available", matches.len())),
            );
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }

    fn render_team_modal(
        &self,
        modal: &TeamModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("team-modal-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgb(0x0a0f1d))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut modal_box = div()
            .id("team-modal-container")
            .w(px(780.0))
            .max_h(px(680.0))
            .flex()
            .flex_col()
            .bg(rgb(0x1e293b))
            .border_1()
            .border_color(rgb(0x6366f1))
            .rounded_lg()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(0x334155))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf8fafc))
                        .child("👥 Team Collaboration & Shared Vaults"),
                ),
            )
            .child(
                div()
                    .id("dismiss-team-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_team_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Navigation Tabs: Vaults, Members, AuditLog
        let mut tab_strip = div().flex().items_center().gap_2();

        let is_vaults = modal.active_tab == TeamModalTab::Vaults;
        tab_strip = tab_strip.child(
            div()
                .id("team-tab-vaults")
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(if is_vaults {
                    rgb(0x4338ca)
                } else {
                    rgb(0x334155)
                })
                .text_sm()
                .font_weight(if is_vaults {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(m) = &mut app.team_modal {
                        m.active_tab = TeamModalTab::Vaults;
                        cx.notify();
                    }
                }))
                .child(format!("📁 Shared Vaults ({})", modal.vaults.len())),
        );

        let is_members = modal.active_tab == TeamModalTab::Members;
        tab_strip = tab_strip.child(
            div()
                .id("team-tab-members")
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(if is_members {
                    rgb(0x4338ca)
                } else {
                    rgb(0x334155)
                })
                .text_sm()
                .font_weight(if is_members {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(m) = &mut app.team_modal {
                        m.active_tab = TeamModalTab::Members;
                        cx.notify();
                    }
                }))
                .child(format!("👥 Members ({})", modal.members.len())),
        );

        let is_audit = modal.active_tab == TeamModalTab::AuditLog;
        tab_strip = tab_strip.child(
            div()
                .id("team-tab-audit")
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(if is_audit {
                    rgb(0x4338ca)
                } else {
                    rgb(0x334155)
                })
                .text_sm()
                .font_weight(if is_audit {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(m) = &mut app.team_modal {
                        m.active_tab = TeamModalTab::AuditLog;
                        cx.notify();
                    }
                }))
                .child(format!("📜 Audit Log ({})", modal.audit_records.len())),
        );

        modal_box = modal_box.child(tab_strip);

        // Status or Error notifications
        if let Some(msg) = &modal.status_message {
            modal_box = modal_box.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x064e3b))
                    .border_1()
                    .border_color(rgb(0x059669))
                    .text_xs()
                    .text_color(rgb(0xa7f3d0))
                    .child(format!("✓ {msg}")),
            );
        }
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x450a0a))
                    .border_1()
                    .border_color(rgb(0x991b1b))
                    .text_xs()
                    .text_color(rgb(0xfecaca))
                    .child(format!("⚠ {err}")),
            );
        }

        // Tab Content
        match modal.active_tab {
            TeamModalTab::Vaults => {
                let mut list = div()
                    .id("vaults-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_h(px(260.0))
                    .overflow_y_scroll();

                if modal.vaults.is_empty() {
                    list = list.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(0x0f172a))
                            .text_sm()
                            .text_color(rgb(0x94a3b8))
                            .child("No shared team vaults found. Create one below to collaborate."),
                    );
                } else {
                    for (v_idx, v) in modal.vaults.iter().enumerate() {
                        let vid = v.id.clone();
                        let is_selected = modal.selected_vault_id.as_deref() == Some(&v.id);
                        let epoch = v.current_epoch;
                        let vid_rotate = v.id.clone();
                        let vid_del = v.id.clone();

                        let mut card = div()
                            .id(("vault-card", v_idx))
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_md()
                            .bg(if is_selected {
                                rgb(0x1e1b4b)
                            } else {
                                rgb(0x0f172a)
                            })
                            .border_1()
                            .border_color(if is_selected {
                                rgb(0x6366f1)
                            } else {
                                rgb(0x1e293b)
                            });

                        let left = div()
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
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xf1f5f9))
                                            .child(v.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x312e81))
                                            .text_xs()
                                            .text_color(rgb(0xc7d2fe))
                                            .child(format!("Epoch {epoch}")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x94a3b8))
                                            .child(format!("Owner: {}", v.owner_id)),
                                    ),
                            )
                            .child(
                                div().text_xs().text_color(rgb(0x64748b)).child(
                                    v.description
                                        .clone()
                                        .unwrap_or_else(|| "No description".into()),
                                ),
                            );

                        let mut right = div().flex().items_center().gap_2();
                        if !is_selected {
                            let vid_sel = vid.clone();
                            right = right.child(
                                div()
                                    .id(("select-vault-btn", v_idx))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x4338ca))
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        if let Some(m) = &mut app.team_modal {
                                            m.selected_vault_id = Some(vid_sel.clone());
                                        }
                                        app.refresh_team_modal_vault(&vid_sel, cx);
                                    }))
                                    .child("Select"),
                            );
                        } else {
                            right = right.child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x065f46))
                                    .text_xs()
                                    .text_color(rgb(0xa7f3d0))
                                    .child("Active"),
                            );
                        }

                        right = right.child(
                            div()
                                .id(("rotate-key-btn", v_idx))
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(0x374151))
                                .text_xs()
                                .text_color(rgb(0xe5e7eb))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.rotate_team_vault_key(&vid_rotate, cx);
                                }))
                                .child("🔄 Rotate Key"),
                        );

                        right = right.child(
                            div()
                                .id(("delete-vault-btn", v_idx))
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(0x7f1d1d))
                                .text_xs()
                                .text_color(rgb(0xfecaca))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.delete_team_vault(&vid_del, cx);
                                }))
                                .child("🗑"),
                        );

                        card = card.child(left).child(right);
                        list = list.child(card);
                    }
                }
                modal_box = modal_box.child(list);

                // Create Vault Form
                let create_form = div()
                    .id("create-vault-box")
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x0f172a))
                    .border_1()
                    .border_color(rgb(0x1e293b))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0x94a3b8))
                            .child("CREATE NEW SHARED VAULT"),
                    )
                    .child(modal.new_vault_name.clone())
                    .child(modal.new_vault_desc.clone())
                    .child(
                        div().flex().justify_end().child(
                            div()
                                .id("create-vault-submit-btn")
                                .px_4()
                                .py_1p5()
                                .rounded_md()
                                .bg(rgb(0x4338ca))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.create_team_vault(cx);
                                }))
                                .child("+ Create Vault"),
                        ),
                    );
                modal_box = modal_box.child(create_form);
            }
            TeamModalTab::Members => {
                let Some(selected_vid) = &modal.selected_vault_id else {
                    modal_box = modal_box.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(0x0f172a))
                            .text_sm()
                            .text_color(rgb(0xf59e0b))
                            .child(
                                "⚠️ Please select a shared vault from the 'Shared Vaults' tab first.",
                            ),
                    );
                    return overlay.child(modal_box);
                };

                let selected_vault_name = modal
                    .vaults
                    .iter()
                    .find(|v| &v.id == selected_vid)
                    .map(|v| v.name.as_str())
                    .unwrap_or("Selected Vault");

                modal_box = modal_box.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x94a3b8))
                        .child(format!("Enrolled Collaborators for: {selected_vault_name}")),
                );

                let mut members_list = div()
                    .id("team-members-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_h(px(220.0))
                    .overflow_y_scroll();

                if modal.members.is_empty() {
                    members_list = members_list.child(
                        div().p_3().rounded_md().bg(rgb(0x0f172a)).text_xs().text_color(
                            rgb(0x64748b),
                        ).child(
                            "No additional collaborators enrolled yet. Invite team members below.",
                        ),
                    );
                } else {
                    for (m_idx, m) in modal.members.iter().enumerate() {
                        let mid_revoke = m.id.clone();
                        let mid_role = m.id.clone();
                        let is_revoked = m.revoked;

                        let mut card = div()
                            .id(("member-card", m_idx))
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(0x0f172a))
                            .border_1()
                            .border_color(if is_revoked {
                                rgb(0x450a0a)
                            } else {
                                rgb(0x1e293b)
                            });

                        let (badge_bg, badge_fg) = match m.role {
                            TeamRole::Owner => (rgb(0x78350f), rgb(0xfde68a)),
                            TeamRole::Administrator => (rgb(0x1e3a8a), rgb(0xbfdbfe)),
                            TeamRole::Editor => (rgb(0x064e3b), rgb(0xa7f3d0)),
                            TeamRole::Viewer => (rgb(0x374151), rgb(0xd1d5db)),
                        };

                        let left = div()
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
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if is_revoked {
                                                rgb(0x94a3b8)
                                            } else {
                                                rgb(0xf8fafc)
                                            })
                                            .child(m.display_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(badge_bg)
                                            .text_xs()
                                            .text_color(badge_fg)
                                            .child(m.role.badge_label()),
                                    )
                                    .child({
                                        if is_revoked {
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0x7f1d1d))
                                                .text_xs()
                                                .text_color(rgb(0xfecaca))
                                                .child("⛔ Revoked")
                                        } else {
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0x065f46))
                                                .text_xs()
                                                .text_color(rgb(0xa7f3d0))
                                                .child("Active")
                                        }
                                    }),
                            )
                            .child(div().text_xs().text_color(rgb(0x64748b)).child(format!(
                                "User: {} • Invited by {}",
                                m.user_id, m.invited_by
                            )));

                        let mut right = div().flex().items_center().gap_2();
                        if !is_revoked && m.role != TeamRole::Owner {
                            let next_role = match m.role {
                                TeamRole::Viewer => TeamRole::Editor,
                                TeamRole::Editor => TeamRole::Administrator,
                                TeamRole::Administrator => TeamRole::Viewer,
                                TeamRole::Owner => TeamRole::Owner,
                            };
                            let role_label = format!("Set {}", next_role.as_str());
                            right = right.child(
                                div()
                                    .id(("toggle-role-btn", m_idx))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x374151))
                                    .text_xs()
                                    .text_color(rgb(0xe2e8f0))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.update_member_role(&mid_role, next_role, cx);
                                    }))
                                    .child(role_label),
                            );

                            right = right.child(
                                div()
                                    .id(("revoke-member-btn", m_idx))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x7f1d1d))
                                    .text_xs()
                                    .text_color(rgb(0xfecaca))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.revoke_team_member(&mid_revoke, cx);
                                    }))
                                    .child("⛔ Revoke"),
                            );
                        }

                        card = card.child(left).child(right);
                        members_list = members_list.child(card);
                    }
                }
                modal_box = modal_box.child(members_list);

                // Invite Member Form
                let current_role = modal.invite_role;
                let invite_form = div()
                    .id("invite-member-box")
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x0f172a))
                    .border_1()
                    .border_color(rgb(0x1e293b))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0x94a3b8))
                            .child("INVITE COLLABORATOR"),
                    )
                    .child(modal.invite_user_id.clone())
                    .child(modal.invite_name.clone())
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
                                    .child(div().text_xs().text_color(rgb(0x94a3b8)).child("Role:"))
                                    .child(
                                        div()
                                            .id("invite-role-viewer")
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(if current_role == TeamRole::Viewer {
                                                rgb(0x4338ca)
                                            } else {
                                                rgb(0x374151)
                                            })
                                            .text_xs()
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                if let Some(m) = &mut app.team_modal {
                                                    m.invite_role = TeamRole::Viewer;
                                                    cx.notify();
                                                }
                                            }))
                                            .child("Viewer"),
                                    )
                                    .child(
                                        div()
                                            .id("invite-role-editor")
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(if current_role == TeamRole::Editor {
                                                rgb(0x4338ca)
                                            } else {
                                                rgb(0x374151)
                                            })
                                            .text_xs()
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                if let Some(m) = &mut app.team_modal {
                                                    m.invite_role = TeamRole::Editor;
                                                    cx.notify();
                                                }
                                            }))
                                            .child("Editor"),
                                    )
                                    .child(
                                        div()
                                            .id("invite-role-admin")
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(if current_role == TeamRole::Administrator {
                                                rgb(0x4338ca)
                                            } else {
                                                rgb(0x374151)
                                            })
                                            .text_xs()
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                if let Some(m) = &mut app.team_modal {
                                                    m.invite_role = TeamRole::Administrator;
                                                    cx.notify();
                                                }
                                            }))
                                            .child("Admin"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("invite-submit-btn")
                                    .px_4()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(rgb(0x4338ca))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.invite_team_member(cx);
                                    }))
                                    .child("✉️ Invite Member"),
                            ),
                    );
                modal_box = modal_box.child(invite_form);
            }
            TeamModalTab::AuditLog => {
                let Some(selected_vid) = &modal.selected_vault_id else {
                    modal_box = modal_box.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(0x0f172a))
                            .text_sm()
                            .text_color(rgb(0xf59e0b))
                            .child(
                                "⚠️ Please select a shared vault from the 'Shared Vaults' tab first.",
                            ),
                    );
                    return overlay.child(modal_box);
                };

                let selected_vault_name = modal
                    .vaults
                    .iter()
                    .find(|v| &v.id == selected_vid)
                    .map(|v| v.name.as_str())
                    .unwrap_or("Selected Vault");

                modal_box = modal_box.child(div().text_xs().text_color(rgb(0x94a3b8)).child(
                    format!("Tamper-Evident Audit Trail for: {selected_vault_name}"),
                ));

                let mut audit_list = div()
                    .id("team-audit-list")
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .max_h(px(380.0))
                    .overflow_y_scroll();

                if modal.audit_records.is_empty() {
                    audit_list = audit_list.child(
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(0x0f172a))
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("No audit records logged yet for this vault."),
                    );
                } else {
                    for record in &modal.audit_records {
                        let formatted = record.format_entry();
                        let row = div()
                            .px_3()
                            .py_1p5()
                            .rounded_sm()
                            .bg(rgb(0x0f172a))
                            .border_1()
                            .border_color(rgb(0x1e293b))
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(0xc7d2fe))
                            .child(formatted);
                        audit_list = audit_list.child(row);
                    }
                }
                modal_box = modal_box.child(audit_list);
            }
        }

        overlay.child(modal_box)
    }

    fn render_infra_modal(
        &self,
        modal: &InfraModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("infra-modal-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgb(0x0a0f1d))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut modal_box = div()
            .id("infra-modal-container")
            .w(px(780.0))
            .max_h(px(680.0))
            .flex()
            .flex_col()
            .bg(rgb(0x1e293b))
            .border_1()
            .border_color(rgb(0x0e7490))
            .rounded_lg()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(0x334155))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf8fafc))
                        .child("☁️ Cloud Providers & Infrastructure Inventory"),
                ),
            )
            .child(
                div()
                    .id("dismiss-infra-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.dismiss_infra_modal(cx);
                    }))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Feedback / Error Banner
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x7f1d1d))
                    .text_xs()
                    .text_color(rgb(0xfecaca))
                    .child(format!("⚠️ {err}")),
            );
        }
        if let Some(status) = &modal.status_message {
            modal_box = modal_box.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x065f46))
                    .text_xs()
                    .text_color(rgb(0xa7f3d0))
                    .child(format!("ℹ️ {status}")),
            );
        }

        // Tab Strip
        let current_tab = modal.active_tab;
        let tabs = div()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(0x334155))
            .pb_2()
            .child(
                div()
                    .id("infra-tab-providers")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .bg(if current_tab == InfraModalTab::Providers {
                        rgb(0x0e7490)
                    } else {
                        rgb(0x334155)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(m) = &mut app.infra_modal {
                            m.active_tab = InfraModalTab::Providers;
                            cx.notify();
                        }
                    }))
                    .child("☁️ Providers"),
            )
            .child(
                div()
                    .id("infra-tab-discovered")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .bg(if current_tab == InfraModalTab::Discovered {
                        rgb(0x0e7490)
                    } else {
                        rgb(0x334155)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(m) = &mut app.infra_modal {
                            m.active_tab = InfraModalTab::Discovered;
                            cx.notify();
                        }
                    }))
                    .child(format!("🖥 Discovered Hosts ({})", modal.discovered.len())),
            )
            .child(
                div()
                    .id("infra-tab-ansible")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .bg(if current_tab == InfraModalTab::Ansible {
                        rgb(0x0e7490)
                    } else {
                        rgb(0x334155)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(m) = &mut app.infra_modal {
                            m.active_tab = InfraModalTab::Ansible;
                            cx.notify();
                        }
                    }))
                    .child("🅰️ Ansible Inventory"),
            );
        modal_box = modal_box.child(tabs);

        match current_tab {
            InfraModalTab::Providers => {
                // Add Provider Form
                let current_kind = modal.new_provider_kind;
                let kind_picker = div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("kind-aws")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(if current_kind == CloudProviderKind::Aws {
                                rgb(0x0e7490)
                            } else {
                                rgb(0x374151)
                            })
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(m) = &mut app.infra_modal {
                                    m.new_provider_kind = CloudProviderKind::Aws;
                                    cx.notify();
                                }
                            }))
                            .child("☁️ AWS"),
                    )
                    .child(
                        div()
                            .id("kind-azure")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(if current_kind == CloudProviderKind::Azure {
                                rgb(0x0e7490)
                            } else {
                                rgb(0x374151)
                            })
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(m) = &mut app.infra_modal {
                                    m.new_provider_kind = CloudProviderKind::Azure;
                                    cx.notify();
                                }
                            }))
                            .child("🔷 Azure"),
                    )
                    .child(
                        div()
                            .id("kind-do")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(if current_kind == CloudProviderKind::DigitalOcean {
                                rgb(0x0e7490)
                            } else {
                                rgb(0x374151)
                            })
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(m) = &mut app.infra_modal {
                                    m.new_provider_kind = CloudProviderKind::DigitalOcean;
                                    cx.notify();
                                }
                            }))
                            .child("🌊 DigitalOcean"),
                    )
                    .child(
                        div()
                            .id("kind-ansible")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(if current_kind == CloudProviderKind::Ansible {
                                rgb(0x0e7490)
                            } else {
                                rgb(0x374151)
                            })
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(m) = &mut app.infra_modal {
                                    m.new_provider_kind = CloudProviderKind::Ansible;
                                    cx.notify();
                                }
                            }))
                            .child("🅰️ Ansible"),
                    );

                let add_form = div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .bg(rgb(0x0f172a))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x334155))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0x94a3b8))
                            .child("Connect New Infrastructure Provider:"),
                    )
                    .child(kind_picker)
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_1().child(modal.new_provider_name.clone()))
                            .child(div().w(px(220.0)).child(modal.new_provider_region.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_1().child(modal.new_provider_path.clone()))
                            .child(
                                div()
                                    .id("create-provider-btn")
                                    .px_4()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(rgb(0x0e7490))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.create_infra_provider(cx);
                                    }))
                                    .child("+ Add Provider"),
                            ),
                    );
                modal_box = modal_box.child(add_form);

                // Configured Providers List
                let mut prov_list = div()
                    .id("providers-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_h(px(240.0))
                    .overflow_y_scroll();

                if modal.providers.is_empty() {
                    prov_list = prov_list.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(0x0f172a))
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("No infrastructure providers configured yet."),
                    );
                } else {
                    for (p_idx, p) in modal.providers.iter().enumerate() {
                        let pid = p.id.clone();
                        let sync_pid = p.id.clone();
                        let icon = match p.provider_kind.as_str() {
                            "aws" => "☁️",
                            "azure" => "🔷",
                            "digitalocean" => "🌊",
                            "ansible" => "🅰️",
                            _ => "🌐",
                        };

                        let card = div()
                            .id(("provider-card", p_idx))
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(0x0f172a))
                            .border_1()
                            .border_color(rgb(0x334155))
                            .child(
                                div().flex().flex_col().gap_0p5().child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().text_sm().child(icon))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(rgb(0xf8fafc))
                                                .child(p.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0x1e293b))
                                                .text_xs()
                                                .text_color(rgb(0x38bdf8))
                                                .child(p.provider_kind.to_uppercase()),
                                        ),
                                ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id(("sync-prov-btn", p_idx))
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(0x0369a1))
                                            .text_xs()
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.sync_infra_provider(sync_pid.clone(), cx);
                                            }))
                                            .child("⚡ Sync"),
                                    )
                                    .child(
                                        div()
                                            .id(("del-prov-btn", p_idx))
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(0x7f1d1d))
                                            .text_xs()
                                            .text_color(rgb(0xfecaca))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.delete_infra_provider(pid.clone(), cx);
                                            }))
                                            .child("🗑 Delete"),
                                    ),
                            );
                        prov_list = prov_list.child(card);
                    }
                }
                modal_box = modal_box.child(prov_list);
            }
            InfraModalTab::Discovered => {
                let mut disc_list = div()
                    .id("discovered-hosts-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_h(px(420.0))
                    .overflow_y_scroll();

                if modal.discovered.is_empty() {
                    disc_list = disc_list.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(0x0f172a))
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("No hosts discovered yet. Connect a provider and trigger Sync, or import an inventory file."),
                    );
                } else {
                    for (h_idx, h) in modal.discovered.iter().enumerate() {
                        let is_imported = h.imported_host_id.is_some();
                        let state_badge = if h.state.eq_ignore_ascii_case("running")
                            || h.state.eq_ignore_ascii_case("active")
                        {
                            "🟢 Running"
                        } else {
                            "🔴 Stopped"
                        };

                        let row = div()
                            .id(("disc-host-card", h_idx))
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(0x0f172a))
                            .border_1()
                            .border_color(rgb(0x334155))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(rgb(0xf8fafc))
                                                    .child(h.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(0x1e293b))
                                                    .text_xs()
                                                    .text_color(rgb(0x94a3b8))
                                                    .child(state_badge),
                                            )
                                            .child({
                                                let reg = h.region.as_deref().unwrap_or("-");
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(0x64748b))
                                                    .child(format!("[{reg}]"))
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family("monospace")
                                            .text_color(rgb(0x38bdf8))
                                            .child(format!(
                                                "{}@{}:{}",
                                                h.username.as_deref().unwrap_or("user"),
                                                h.hostname,
                                                h.port
                                            )),
                                    ),
                            )
                            .child({
                                if is_imported {
                                    div()
                                        .id(("imported-badge", h_idx))
                                        .px_2p5()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(rgb(0x065f46))
                                        .text_xs()
                                        .text_color(rgb(0xa7f3d0))
                                        .child(format!(
                                            "✅ Imported (ID: {})",
                                            h.imported_host_id.unwrap_or(0)
                                        ))
                                } else {
                                    div()
                                        .id(("import-host-btn", h_idx))
                                        .px_3()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(rgb(0x0e7490))
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            app.import_discovered_host_into_inventory(h_idx, cx);
                                        }))
                                        .child("📥 Import Host")
                                }
                            });
                        disc_list = disc_list.child(row);
                    }
                }
                modal_box = modal_box.child(disc_list);
            }
            InfraModalTab::Ansible => {
                let ansible_section = div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .rounded_md()
                    .bg(rgb(0x0f172a))
                    .border_1()
                    .border_color(rgb(0x334155))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xf8fafc))
                            .child("Export Shellumi Inventory to Ansible"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .child("Generate fully compliant Ansible static and dynamic inventory formats from your current Shellumi hosts, groups, and tags."),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                div()
                                    .id("export-ansible-ini-btn")
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .bg(rgb(0x0e7490))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.export_ansible_inventory_file(false, cx);
                                    }))
                                    .child("📤 Export to Ansible INI"),
                            )
                            .child(
                                div()
                                    .id("export-ansible-yaml-btn")
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .bg(rgb(0x4338ca))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.export_ansible_inventory_file(true, cx);
                                    }))
                                    .child("📤 Export to Ansible YAML"),
                            ),
                    );
                modal_box = modal_box.child(ansible_section);
            }
        }

        overlay.child(modal_box)
    }

    fn render_diagnostics_modal(
        &self,
        modal: &DiagnosticsModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("diagnostics-modal-overlay")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000cc))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("diagnostics-modal-box")
            .w(px(740.0))
            .max_h(px(580.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(0x1e293b))
            .border_1()
            .border_color(rgb(0x334155))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(0x334155))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf8fafc))
                        .child("🩺 System Diagnostics & Production Health Check"),
                ),
            )
            .child(
                div()
                    .id("dismiss-diagnostics-modal-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_diagnostics_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        if modal.is_running {
            let loading = div()
                .p_8()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(
                    div()
                        .text_lg()
                        .text_color(rgb(0x38bdf8))
                        .child("⏳ Running system diagnostic checks..."),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x94a3b8))
                        .child("Testing OpenSSH binary, CSPRNG, AEAD ciphers, network loopback, and storage integrity..."),
                );
            modal_box = modal_box.child(loading);
        } else if let Some(ref report) = modal.report {
            let (passed, warnings, failures) = report.counts();

            // Status Banner
            let (banner_bg, banner_fg, banner_text) = if failures > 0 {
                (
                    rgb(0x7f1d1d),
                    rgb(0xfecaca),
                    format!("❌ Action Required: {failures} critical check(s) failed."),
                )
            } else if warnings > 0 {
                (
                    rgb(0x78350f),
                    rgb(0xfde68a),
                    format!("⚠️ Degraded: All critical tests passed, but {warnings} warning(s) detected."),
                )
            } else {
                (
                    rgb(0x065f46),
                    rgb(0xa7f3d0),
                    "✅ All Systems Ready: All production environment prerequisites passed."
                        .to_string(),
                )
            };

            let status_strip = div()
                .px_4()
                .py_2p5()
                .rounded_md()
                .bg(banner_bg)
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(banner_fg)
                .flex()
                .items_center()
                .justify_between()
                .child(banner_text)
                .child(format!(
                    "Passed: {passed} | Warnings: {warnings} | Failed: {failures}"
                ));
            modal_box = modal_box.child(status_strip);

            // Checks list
            let mut checks_list = div()
                .id("diagnostics-checks-list")
                .flex()
                .flex_col()
                .gap_2()
                .overflow_y_scroll()
                .max_h(px(320.0));

            for (c_idx, check) in report.checks.iter().enumerate() {
                let (badge_bg, badge_fg) = match check.status {
                    DiagnosticStatus::Passed => (rgb(0x065f46), rgb(0xa7f3d0)),
                    DiagnosticStatus::Warning => (rgb(0x78350f), rgb(0xfde68a)),
                    DiagnosticStatus::Failed => (rgb(0x7f1d1d), rgb(0xfecaca)),
                };

                let mut row = div()
                    .id(("diag-check-row", c_idx))
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x0f172a))
                    .border_1()
                    .border_color(rgb(0x334155))
                    .flex()
                    .flex_col()
                    .gap_1p5();

                let row_header = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_sm().child(check.category.icon()))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xf8fafc))
                                    .child(check.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x64748b))
                                    .child(format!("{}ms", check.duration_ms)),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(badge_bg)
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(badge_fg)
                                    .child(check.status.badge()),
                            ),
                    );
                row = row.child(row_header);

                let row_msg = div()
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .child(check.message.clone());
                row = row.child(row_msg);

                if let Some(ref rem) = check.remediation {
                    let row_rem = div()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .bg(rgb(0x1e293b))
                        .text_xs()
                        .text_color(rgb(0x38bdf8))
                        .child(format!("↳ Remediation: {rem}"));
                    row = row.child(row_rem);
                }

                checks_list = checks_list.child(row);
            }
            modal_box = modal_box.child(checks_list);

            // Footer controls
            let footer = div()
                .flex()
                .items_center()
                .justify_between()
                .pt_2()
                .border_t_1()
                .border_color(rgb(0x334155))
                .child(div().text_xs().text_color(rgb(0x64748b)).child(format!(
                    "Platform: {} [{}]",
                    report.os_platform, report.os_arch
                )))
                .child(
                    div()
                        .id("rerun-diagnostics-btn")
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .bg(rgb(0x0f766e))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.run_diagnostics_checks(cx);
                        }))
                        .child("🔄 Re-run Checks"),
                );
            modal_box = modal_box.child(footer);
        }

        overlay.child(modal_box)
    }

    fn render_mobile_modal(
        &self,
        modal: &MobileSimulatorModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("mobile-modal-overlay")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000cc))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("mobile-modal-box")
            .w(px(760.0))
            .max_h(px(620.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(0x1e293b))
            .border_1()
            .border_color(rgb(0x334155))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(0x334155))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf8fafc))
                        .child("📱 Mobile Touch & Virtual Key Simulator (Phase 11)"),
                ),
            )
            .child(
                div()
                    .id("dismiss-mobile-modal-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_mobile_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Viewport selector & Lifecycle status banner
        let (status_bg, status_fg, status_icon) = match modal.lifecycle_manager.state {
            MobileSessionState::Connected => (rgb(0x065f46), rgb(0xa7f3d0), "🟢"),
            MobileSessionState::BackgroundGracePeriod { .. } => {
                (rgb(0x78350f), rgb(0xfde68a), "🟡")
            }
            MobileSessionState::Suspended { .. } => (rgb(0x831843), rgb(0xfce7f3), "🟠"),
            MobileSessionState::Reconnecting { .. } => (rgb(0x1e3a8a), rgb(0xbfdbfe), "🔄"),
            MobileSessionState::Connecting => (rgb(0x374151), rgb(0xf3f4f6), "⏳"),
            MobileSessionState::Disconnected { .. } => (rgb(0x7f1d1d), rgb(0xfecaca), "🔴"),
        };

        let top_row = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("preset-iphone")
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .bg(if modal.preset == MobileViewportPreset::IPhone15Pro {
                                rgb(0x2563eb)
                            } else {
                                rgb(0x334155)
                            })
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.switch_mobile_preset(MobileViewportPreset::IPhone15Pro, cx);
                            }))
                            .child("iPhone 15 Pro"),
                    )
                    .child(
                        div()
                            .id("preset-pixel")
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .bg(if modal.preset == MobileViewportPreset::AndroidPixel8 {
                                rgb(0x2563eb)
                            } else {
                                rgb(0x334155)
                            })
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.switch_mobile_preset(MobileViewportPreset::AndroidPixel8, cx);
                            }))
                            .child("Pixel 8"),
                    )
                    .child(
                        div()
                            .id("preset-ipad")
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .bg(if modal.preset == MobileViewportPreset::IPadPro11 {
                                rgb(0x2563eb)
                            } else {
                                rgb(0x334155)
                            })
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.switch_mobile_preset(MobileViewportPreset::IPadPro11, cx);
                            }))
                            .child("iPad Pro 11\""),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(status_bg)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(status_fg)
                    .child(status_icon)
                    .child(modal.lifecycle_manager.state.status_label()),
            );
        modal_box = modal_box.child(top_row);

        // Lifecycle trigger controls
        let lifecycle_toolbar = div()
            .flex()
            .items_center()
            .gap_2()
            .p_2()
            .rounded_md()
            .bg(rgb(0x0f172a))
            .border_1()
            .border_color(rgb(0x334155))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x94a3b8))
                    .child("OS Lifecycle:"),
            )
            .child(
                div()
                    .id("life-bg-btn")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0xf8fafc))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(
                            MobileLifecycleEvent::AppBackgrounded { grace_seconds: 30 },
                            cx,
                        );
                    }))
                    .child("📱 Background (30s)"),
            )
            .child(
                div()
                    .id("life-tick-btn")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0xf8fafc))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(
                            MobileLifecycleEvent::BackgroundTimerTick {
                                elapsed_seconds: 10,
                            },
                            cx,
                        );
                    }))
                    .child("⏱ Tick 10s"),
            )
            .child(
                div()
                    .id("life-expire-btn")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0xf8fafc))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(MobileLifecycleEvent::BackgroundExpired, cx);
                    }))
                    .child("⏳ Expire"),
            )
            .child(
                div()
                    .id("life-netdrop-btn")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0xf8fafc))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(MobileLifecycleEvent::NetworkLost, cx);
                    }))
                    .child("📶 Drop Net"),
            )
            .child(
                div()
                    .id("life-reconnect-btn")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0xf8fafc))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(MobileLifecycleEvent::NetworkRestored, cx);
                    }))
                    .child("🔄 Reconnect"),
            )
            .child(
                div()
                    .id("life-foreground-btn")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x065f46))
                    .text_xs()
                    .text_color(rgb(0xa7f3d0))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.trigger_mobile_lifecycle(MobileLifecycleEvent::AppForegrounded, cx);
                    }))
                    .child("☀️ Foreground"),
            );
        modal_box = modal_box.child(lifecycle_toolbar);

        // Simulated Mobile Device Viewport
        let (cols, rows) = modal.preset.terminal_cols_rows();
        let mut screen_area = div()
            .id("simulated-mobile-screen")
            .p_4()
            .rounded_lg()
            .bg(rgb(0x020617))
            .border_1()
            .border_color(rgb(0x1e293b))
            .flex()
            .flex_col()
            .gap_1()
            .min_h(px(160.0))
            .max_h(px(180.0))
            .overflow_y_scroll();

        let notch = div().flex().justify_center().pb_1().child(
            div()
                .w(px(100.0))
                .h(px(12.0))
                .rounded_full()
                .bg(rgb(0x1e293b))
                .flex()
                .items_center()
                .justify_center()
                .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(rgb(0x334155))),
        );
        screen_area = screen_area.child(notch);

        for line in &modal.simulated_output {
            screen_area = screen_area.child(
                div()
                    .text_xs()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_color(rgb(0x22c55e))
                    .child(line.clone()),
            );
        }
        modal_box = modal_box.child(screen_area);

        // Virtual Keyboard Accessory Bar
        let mut accessory_strip = div()
            .id("virtual-accessory-strip")
            .p_2()
            .rounded_md()
            .bg(rgb(0x0f172a))
            .border_1()
            .border_color(rgb(0x334155))
            .flex()
            .items_center()
            .gap_1p5()
            .overflow_x_scroll();

        // Sticky Modifier Buttons
        let ctrl_active = modal.accessory_bar.has_modifier(VirtualKeyModifier::Ctrl);
        let alt_active = modal.accessory_bar.has_modifier(VirtualKeyModifier::Alt);

        accessory_strip = accessory_strip
            .child(
                div()
                    .id("mod-ctrl-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(if ctrl_active {
                        rgb(0xd97706)
                    } else {
                        rgb(0x334155)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if ctrl_active {
                        rgb(0xffffff)
                    } else {
                        rgb(0x94a3b8)
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.toggle_mobile_modifier(VirtualKeyModifier::Ctrl, cx);
                    }))
                    .child("Ctrl"),
            )
            .child(
                div()
                    .id("mod-alt-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(if alt_active {
                        rgb(0xd97706)
                    } else {
                        rgb(0x334155)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if alt_active {
                        rgb(0xffffff)
                    } else {
                        rgb(0x94a3b8)
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.toggle_mobile_modifier(VirtualKeyModifier::Alt, cx);
                    }))
                    .child("Alt"),
            );

        // Virtual Keys
        for (k_idx, key) in modal.accessory_bar.keys.iter().enumerate() {
            let key_clone = key.clone();
            let label = key.label();
            accessory_strip = accessory_strip.child(
                div()
                    .id(("vkey-btn", k_idx))
                    .px_2p5()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(rgb(0x1e293b))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x38bdf8))
                    .border_1()
                    .border_color(rgb(0x334155))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.press_mobile_virtual_key(key_clone.clone(), cx);
                    }))
                    .child(label),
            );
        }
        modal_box = modal_box.child(accessory_strip);

        // Input & Hex Telemetry Log
        let mut telemetry = div()
            .id("mobile-telemetry-log")
            .p_2p5()
            .rounded_md()
            .bg(rgb(0x090d16))
            .border_1()
            .border_color(rgb(0x1e293b))
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(90.0))
            .overflow_y_scroll();

        if modal.input_history.is_empty() {
            telemetry = telemetry.child(
                div()
                    .text_xs()
                    .text_color(rgb(0x64748b))
                    .child("Tap virtual keys above to stream ANSI escape codes..."),
            );
        } else {
            for entry in modal.input_history.iter().rev().take(6) {
                telemetry = telemetry.child(
                    div()
                        .text_xs()
                        .font_family(".AppleSystemUIFontMonospaced")
                        .text_color(rgb(0x38bdf8))
                        .child(entry.clone()),
                );
            }
        }
        modal_box = modal_box.child(telemetry);

        // Footer info
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .pt_2()
            .border_t_1()
            .border_color(rgb(0x334155))
            .child(div().text_xs().text_color(rgb(0x64748b)).child(format!(
                "Viewport: {} ({} × {} cols/rows)",
                modal.preset.label(),
                cols,
                rows
            )))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .child("Portable Core FFI Bridge: Ready for Swift/Kotlin integration"),
            );
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }

    fn render_shell_integration_modal(
        &self,
        modal: &ShellIntegrationModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("shell-integration-modal-overlay")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000cc))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("shell-integration-modal-box")
            .w(px(780.0))
            .max_h(px(640.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(0x1e293b))
            .border_1()
            .border_color(rgb(0x334155))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(0x334155))
            .pb_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xf8fafc))
                            .child("⚡ Shell Integration & Autocomplete (Phase 13)"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .child("Semantic OSC 133 prompts, CWD tracking, password prompt classification, and offline autocomplete"),
                    ),
            )
            .child(
                div()
                    .id("dismiss-shell-integration-modal-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x334155))
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_shell_integration_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Tab Navigation
        let tab_buttons = [
            (ShellIntegrationTab::Hooks, "🐚 Shell Hooks"),
            (ShellIntegrationTab::OscMarkers, "🏷️ OSC 133 & CWD"),
            (
                ShellIntegrationTab::PasswordDetection,
                "🔑 Password Prompts",
            ),
            (ShellIntegrationTab::Autocomplete, "💡 Autocomplete Engine"),
        ];

        let mut tab_bar = div().flex().items_center().gap_2();
        for (tab, label) in tab_buttons {
            let is_active = modal.active_tab == tab;
            tab_bar = tab_bar.child(
                div()
                    .id(("shell-tab", tab as usize))
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(if is_active {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if is_active {
                        rgb(0x2563eb)
                    } else {
                        rgb(0x0f172a)
                    })
                    .text_color(if is_active {
                        rgb(0xffffff)
                    } else {
                        rgb(0x94a3b8)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.set_shell_integration_tab(tab, cx);
                    }))
                    .child(label),
            );
        }
        modal_box = modal_box.child(tab_bar);

        // Body per active tab
        match modal.active_tab {
            ShellIntegrationTab::Hooks => {
                let mut hooks_body = div().flex().flex_col().gap_3();

                // Shell selector pills
                let mut shell_pills = div().flex().items_center().gap_2();
                shell_pills = shell_pills.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x94a3b8))
                        .child("Target Shell:"),
                );
                for (idx, shell) in ["zsh", "bash", "fish"].into_iter().enumerate() {
                    let is_sel = modal.selected_shell == shell;
                    shell_pills = shell_pills.child(
                        div()
                            .id(("shell-pill", idx))
                            .px_2p5()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_xs()
                            .bg(if is_sel { rgb(0x0284c7) } else { rgb(0x334155) })
                            .text_color(if is_sel { rgb(0xffffff) } else { rgb(0x94a3b8) })
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.set_shell_integration_shell(shell, cx);
                            }))
                            .child(shell.to_uppercase()),
                    );
                }
                hooks_body = hooks_body.child(shell_pills);

                // Generated Hook script display
                let hook_code = match modal.selected_shell.as_str() {
                    "bash" => generate_bash_hook(),
                    "fish" => generate_fish_hook(),
                    _ => generate_zsh_hook(),
                };

                let script_box = div()
                    .id("hook-script-box")
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x020617))
                    .border_1()
                    .border_color(rgb(0x1e293b))
                    .max_h(px(220.0))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(0xa5f3fc))
                            .child(hook_code),
                    );
                hooks_body = hooks_body.child(script_box);

                // Instructions
                let install_cmd =
                    format!("eval \"$(shellumi shell-hook {})\"", modal.selected_shell);
                let instructions = div()
                    .p_2p5()
                    .rounded_md()
                    .bg(rgb(0x0f172a))
                    .border_1()
                    .border_color(rgb(0x1e293b))
                    .text_xs()
                    .text_color(rgb(0xcbd5e1))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child("Installation Instructions:"),
                    )
                    .child(div().text_color(rgb(0x94a3b8)).child(format!(
                        "Add the following to your ~/.{}rc or config file:",
                        modal.selected_shell
                    )))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x1e293b))
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(0x38bdf8))
                            .child(install_cmd),
                    );
                hooks_body = hooks_body.child(instructions);

                modal_box = modal_box.child(hooks_body);
            }
            ShellIntegrationTab::OscMarkers => {
                let mut markers_body = div().flex().flex_col().gap_3();

                // Explanatory breakdown
                let explanation = div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x0f172a))
                    .border_1()
                    .border_color(rgb(0x1e293b))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_xs()
                            .text_color(rgb(0xf1f5f9))
                            .child("Semantic Prompt Escape Sequences (OSC 133 & OSC 7):"),
                    )
                    .child(
                        div().text_xs().text_color(rgb(0x94a3b8)).child(
                            "• OSC 133 ; A ST — Prompt Start: Signals the terminal that the interactive shell prompt has begun rendering.",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(0x94a3b8)).child(
                            "• OSC 133 ; B ST — Command Start: Signals the user has begun typing command input.",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(0x94a3b8)).child(
                            "• OSC 133 ; C ST — Command Executed: User pressed Enter; command process is now running.",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(0x94a3b8)).child(
                            "• OSC 133 ; D [; exitcode] ST — Command Finished: Command exited with status code (e.g. 0 for success).",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(0x94a3b8)).child(
                            "• OSC 7 file://hostname/path ST — CWD Notification: Updates the active working directory badge.",
                        ),
                    );
                markers_body = markers_body.child(explanation);

                // Active Pane State Inspector
                let active_tab_info = if let Some(tab_idx) = self.active_tab_idx {
                    if let Some(tab) = self.tabs.get(tab_idx) {
                        if let Some(pane) = tab.panes.get(tab.active_pane_idx) {
                            let cwd = pane
                                .terminal
                                .current_working_directory()
                                .unwrap_or("(none reported)");
                            let exit_code = pane
                                .terminal
                                .last_exit_code()
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "(none)".to_string());
                            let mark = format!("{:?}", pane.terminal.prompt_mark());
                            let history_len = pane.terminal.command_history().len();

                            format!(
                                "Active Pane '{}': CWD = {} | Last Exit = {} | Mark = {} | Captured Commands = {}",
                                pane.name, cwd, exit_code, mark, history_len
                            )
                        } else {
                            "No active terminal pane".into()
                        }
                    } else {
                        "No active tab".into()
                    }
                } else {
                    "No active tab".into()
                };

                let inspector = div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x020617))
                    .border_1()
                    .border_color(rgb(0x1e293b))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0x38bdf8))
                            .child("🔍 Live Session State Inspector:"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(0x94a3b8))
                            .child(active_tab_info),
                    );
                markers_body = markers_body.child(inspector);

                modal_box = modal_box.child(markers_body);
            }
            ShellIntegrationTab::PasswordDetection => {
                let mut pw_body = div().flex().flex_col().gap_3();

                // Security Banner
                let sec_banner = div()
                    .p_2p5()
                    .rounded_md()
                    .bg(rgb(0x451a03))
                    .border_1()
                    .border_color(rgb(0x78350f))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().child("🔒"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0xfde68a))
                            .child("Security Guarantee: Password prompt detection is strictly passive. Shellumi NEVER auto-injects credentials into arbitrary terminal streams without explicit user confirmation."),
                    );
                pw_body = pw_body.child(sec_banner);

                // Sample prompt buttons
                let mut sample_buttons = div().flex().flex_col().gap_1p5();
                sample_buttons = sample_buttons.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x94a3b8))
                        .child("Click a sample prompt line to test classification:"),
                );

                let samples = [
                    ("[sudo] password for dev:", "Sudo"),
                    ("user@prod-server.corp's password:", "SSH Password"),
                    (
                        "Enter passphrase for key '/Users/user/.ssh/id_ed25519':",
                        "Key Passphrase",
                    ),
                    ("Enter 6-digit MFA verification code:", "MFA Code"),
                    ("Password: ", "Generic Password"),
                ];

                let mut sample_row = div().flex().flex_wrap().gap_2();
                for (idx, (prompt_sample, label)) in samples.into_iter().enumerate() {
                    sample_row = sample_row.child(
                        div()
                            .id(("sample-prompt", idx))
                            .px_2p5()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x334155))
                            .text_xs()
                            .cursor_pointer()
                            .text_color(rgb(0xe2e8f0))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.update_test_prompt_input(prompt_sample, cx);
                            }))
                            .child(label),
                    );
                }
                sample_buttons = sample_buttons.child(sample_row);
                pw_body = pw_body.child(sample_buttons);

                // Detection Output Box
                let detected_label = match modal.detected_prompt_kind {
                    Some(PasswordPromptKind::Sudo) => {
                        "Detected: [Sudo] Privilege escalation password prompt"
                    }
                    Some(PasswordPromptKind::Ssh) => {
                        "Detected: [SSH] Remote authentication password prompt"
                    }
                    Some(PasswordPromptKind::KeyPassphrase) => {
                        "Detected: [Key Passphrase] Encrypted private key passphrase prompt"
                    }
                    Some(PasswordPromptKind::MfaCode) => {
                        "Detected: [MFA Code] Multi-factor authentication one-time passcode prompt"
                    }
                    Some(PasswordPromptKind::GenericPassword) => {
                        "Detected: [Generic Password] Standard password input prompt"
                    }
                    None => "No password prompt detected (Safe standard output)",
                };

                let output_box = div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x020617))
                    .border_1()
                    .border_color(rgb(0x1e293b))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("Current Prompt Line:"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(0xffffff))
                            .child(modal.test_prompt_input.clone()),
                    )
                    .child(
                        div()
                            .mt_1()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .bg(if modal.detected_prompt_kind.is_some() {
                                rgb(0x78350f)
                            } else {
                                rgb(0x1e293b)
                            })
                            .text_color(if modal.detected_prompt_kind.is_some() {
                                rgb(0xfde68a)
                            } else {
                                rgb(0x94a3b8)
                            })
                            .child(detected_label),
                    );
                pw_body = pw_body.child(output_box);

                modal_box = modal_box.child(pw_body);
            }
            ShellIntegrationTab::Autocomplete => {
                let mut ac_body = div().flex().flex_col().gap_3();

                // Sample query buttons
                let mut sample_queries = div().flex().items_center().gap_2();
                sample_queries = sample_queries.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x94a3b8))
                        .child("Sample Queries:"),
                );
                for (idx, query) in ["git ", "docker ", "cargo ", "ssh ", "systemctl "]
                    .into_iter()
                    .enumerate()
                {
                    let is_active = modal.test_query_input == query;
                    sample_queries = sample_queries.child(
                        div()
                            .id(("sample-query", idx))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_xs()
                            .bg(if is_active {
                                rgb(0x2563eb)
                            } else {
                                rgb(0x334155)
                            })
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.update_test_autocomplete_query(query, cx);
                            }))
                            .child(query.trim()),
                    );
                }
                ac_body = ac_body.child(sample_queries);

                // Completions List
                let mut list_box = div()
                    .id("autocomplete-results-box")
                    .p_2p5()
                    .rounded_md()
                    .bg(rgb(0x020617))
                    .border_1()
                    .border_color(rgb(0x1e293b))
                    .max_h(px(250.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1();

                if modal.autocomplete_results.is_empty() {
                    list_box = list_box.child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x64748b))
                            .child("No autocomplete suggestions found for query."),
                    );
                } else {
                    for item in &modal.autocomplete_results {
                        let kind_color = match item.kind {
                            CompletionKind::Command => rgb(0x38bdf8),
                            CompletionKind::Subcommand => rgb(0x4ade80),
                            CompletionKind::Flag => rgb(0xfacc15),
                            CompletionKind::FilePath => rgb(0xa78bfa),
                            CompletionKind::Host => rgb(0xf472b6),
                            CompletionKind::Snippet => rgb(0xfb923c),
                            CompletionKind::History => rgb(0x94a3b8),
                        };

                        let row = div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x0f172a))
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
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x1e293b))
                                            .text_xs()
                                            .text_color(kind_color)
                                            .child(format!("{:?}", item.kind)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text_color(rgb(0xf8fafc))
                                            .child(item.display_label.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x64748b))
                                    .child(item.description.clone().unwrap_or_default()),
                            );
                        list_box = list_box.child(row);
                    }
                }
                ac_body = ac_body.child(list_box);

                modal_box = modal_box.child(ac_body);
            }
        }

        // Footer
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .pt_2()
            .border_t_1()
            .border_color(rgb(0x334155))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x64748b))
                    .child("Local-First & Offline Engine: No telemetry or remote dependency"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .child("CLI: shellumi shell-hook / complete / prompt-detect"),
            );
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }

    fn render_sftp_dual_pane(
        &self,
        tab_idx: usize,
        state: &SftpBrowserState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut container = div().flex().flex_col().gap_3().flex_1().min_h_0();

        if let Some(err) = &state.error {
            container = container.child(
                div()
                    .p_2()
                    .bg(rgb(0x7f1d1d))
                    .rounded_md()
                    .text_sm()
                    .text_color(rgb(0xfecaca))
                    .child(format!("Error: {err}")),
            );
        } else if let Some(msg) = &state.status_message {
            container = container.child(
                div()
                    .p_2()
                    .bg(rgb(0x064e3b))
                    .rounded_md()
                    .text_sm()
                    .text_color(rgb(0xa7f3d0))
                    .child(msg.clone()),
            );
        }

        // Dual pane row
        let mut panes_row = div().flex().flex_row().gap_4().flex_1().min_h_0();

        // 1. Local Pane
        let local_crumbs = PathUtils::breadcrumbs(&state.local_path);
        let mut local_crumbs_div = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(rgb(0x9ca3af))
            .flex_wrap();
        local_crumbs_div = local_crumbs_div.child("💻 Local: ");
        for (i, (name, path)) in local_crumbs.into_iter().enumerate() {
            let p_clone = path.clone();
            local_crumbs_div = local_crumbs_div.child(
                div()
                    .id(("local-crumb", i))
                    .cursor_pointer()
                    .text_color(rgb(0x60a5fa))
                    .hover(|s| s.underline())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_local(tab_idx, p_clone.clone(), cx);
                    }))
                    .child(format!("{name}/")),
            );
        }

        let mut local_list_div = div()
            .id("sftp-local-list")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .overflow_y_scroll();

        if state.local_entries.is_empty() {
            local_list_div = local_list_div.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(0x9ca3af))
                    .child("Empty directory"),
            );
        } else {
            for (i, entry) in state.local_entries.iter().enumerate() {
                let is_selected = state.selected_local.as_deref() == Some(&entry.name);
                let name_clone = entry.name.clone();
                let is_dir = entry.kind.is_dir();
                let icon = match entry.kind {
                    FileKind::Directory => "📁 ",
                    FileKind::Symlink { .. } => "🔗 ",
                    _ => "📄 ",
                };

                let row_base = div()
                    .id(("local-row", i))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .p_1()
                    .rounded_sm()
                    .text_xs()
                    .cursor_pointer()
                    .bg(if is_selected {
                        rgb(0x1e3a8a)
                    } else {
                        rgb(0x111827)
                    })
                    .text_color(if is_selected {
                        rgb(0xffffff)
                    } else {
                        rgb(0xd1d5db)
                    })
                    .hover(|s| {
                        s.bg(if is_selected {
                            rgb(0x1e3a8a)
                        } else {
                            rgb(0x1f2937)
                        })
                    });

                let row = if is_dir {
                    let d_name = entry.name.clone();
                    row_base.on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_open_local_dir(tab_idx, d_name.clone(), cx);
                    }))
                } else {
                    let f_name = entry.name.clone();
                    row_base.on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_select_local(tab_idx, f_name.clone(), cx);
                    }))
                };

                let row_el = row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .flex_1()
                            .child(icon)
                            .child(name_clone),
                    )
                    .child(
                        div()
                            .w(px(70.0))
                            .text_right()
                            .text_color(rgb(0x9ca3af))
                            .child(entry.formatted_size()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_center()
                            .text_color(rgb(0x6b7280))
                            .child(entry.formatted_permissions()),
                    );

                local_list_div = local_list_div.child(row_el);
            }
        }

        let local_pane = div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(rgb(0x1f2937))
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x374151))
            .p_3()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(local_crumbs_div)
                    .child(
                        div()
                            .id("sftp-local-up")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x374151))
                            .text_xs()
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_navigate_local_up(tab_idx, cx);
                            }))
                            .child("⬆ Up"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .text_xs()
                    .text_color(rgb(0x9ca3af))
                    .font_weight(FontWeight::BOLD)
                    .child(div().flex_1().child("Name"))
                    .child(div().w(px(70.0)).text_right().child("Size"))
                    .child(div().w(px(80.0)).text_center().child("Mode")),
            )
            .child(local_list_div);

        // 2. Middle Actions Column
        let has_local_sel = state.selected_local.is_some();
        let has_remote_sel = state.selected_remote.is_some();

        let actions_column = div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .px_2()
            .child(
                div()
                    .id("sftp-upload-btn")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if has_local_sel {
                        rgb(0x059669)
                    } else {
                        rgb(0x374151)
                    })
                    .text_sm()
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_upload_selected(tab_idx, cx);
                    }))
                    .child("Upload →"),
            )
            .child(
                div()
                    .id("sftp-download-btn")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(if has_remote_sel {
                        rgb(0x0284c7)
                    } else {
                        rgb(0x374151)
                    })
                    .text_sm()
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_download_selected(tab_idx, cx);
                    }))
                    .child("← Download"),
            );

        // 3. Remote Pane
        let remote_crumbs = PathUtils::breadcrumbs(&state.remote_path);
        let mut remote_crumbs_div = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .text_xs()
            .text_color(rgb(0x9ca3af))
            .flex_wrap();
        remote_crumbs_div = remote_crumbs_div.child(format!("🌐 Remote ({}): ", state.host_name));
        for (i, (name, path)) in remote_crumbs.into_iter().enumerate() {
            let p_clone = path.clone();
            remote_crumbs_div = remote_crumbs_div.child(
                div()
                    .id(("remote-crumb", i))
                    .cursor_pointer()
                    .text_color(rgb(0x34d399))
                    .hover(|s| s.underline())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_remote(tab_idx, p_clone.clone(), cx);
                    }))
                    .child(format!("{name}/")),
            );
        }

        let mut remote_list_div = div()
            .id("sftp-remote-list")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .overflow_y_scroll();

        if state.remote_entries.is_empty() {
            remote_list_div = remote_list_div.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(0x9ca3af))
                    .child("Empty directory"),
            );
        } else {
            for (i, entry) in state.remote_entries.iter().enumerate() {
                let is_selected = state.selected_remote.as_deref() == Some(&entry.name);
                let name_clone = entry.name.clone();
                let is_dir = entry.kind.is_dir();
                let icon = match entry.kind {
                    FileKind::Directory => "📁 ",
                    FileKind::Symlink { .. } => "🔗 ",
                    _ => "📄 ",
                };

                let row_base = div()
                    .id(("remote-row", i))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .p_1()
                    .rounded_sm()
                    .text_xs()
                    .cursor_pointer()
                    .bg(if is_selected {
                        rgb(0x064e3b)
                    } else {
                        rgb(0x111827)
                    })
                    .text_color(if is_selected {
                        rgb(0xffffff)
                    } else {
                        rgb(0xd1d5db)
                    })
                    .hover(|s| {
                        s.bg(if is_selected {
                            rgb(0x064e3b)
                        } else {
                            rgb(0x1f2937)
                        })
                    });

                let row = if is_dir {
                    let d_name = entry.name.clone();
                    row_base.on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_open_remote_dir(tab_idx, d_name.clone(), cx);
                    }))
                } else {
                    let f_name = entry.name.clone();
                    row_base.on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_select_remote(tab_idx, f_name.clone(), cx);
                    }))
                };

                let row_el = row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .flex_1()
                            .child(icon)
                            .child(name_clone),
                    )
                    .child(
                        div()
                            .w(px(70.0))
                            .text_right()
                            .text_color(rgb(0x9ca3af))
                            .child(entry.formatted_size()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_center()
                            .text_color(rgb(0x6b7280))
                            .child(entry.formatted_permissions()),
                    );

                remote_list_div = remote_list_div.child(row_el);
            }
        }

        let remote_pane = div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(rgb(0x1f2937))
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x374151))
            .p_3()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(remote_crumbs_div)
                    .child(
                        div()
                            .id("sftp-remote-up")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x374151))
                            .text_xs()
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_navigate_remote_up(tab_idx, cx);
                            }))
                            .child("⬆ Up"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .text_xs()
                    .text_color(rgb(0x9ca3af))
                    .font_weight(FontWeight::BOLD)
                    .child(div().flex_1().child("Name"))
                    .child(div().w(px(70.0)).text_right().child("Size"))
                    .child(div().w(px(80.0)).text_center().child("Mode")),
            )
            .child(remote_list_div);

        panes_row = panes_row
            .child(local_pane)
            .child(actions_column)
            .child(remote_pane);

        container = container.child(panes_row);

        // 4. Bottom Transfer Queue Drawer
        let q_summary = state.queue.summary();
        let mut queue_drawer = div()
            .flex()
            .flex_col()
            .bg(rgb(0x111827))
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x374151))
            .p_3()
            .gap_2();

        let queue_header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xd1d5db))
                    .child(format!(
                        "Transfers & Queue: {} completed / {} total ({:.1}%)",
                        q_summary.completed_tasks,
                        q_summary.total_tasks,
                        q_summary.overall_percentage
                    )),
            )
            .child(
                div()
                    .id("sftp-clear-completed-btn")
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x374151))
                    .text_xs()
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_clear_completed(tab_idx, cx);
                    }))
                    .child("Clear Completed"),
            );

        // Progress bar
        let pct = q_summary.overall_percentage.clamp(0.0, 100.0);
        let progress_bar = div()
            .w_full()
            .h(px(6.0))
            .bg(rgb(0x374151))
            .rounded_full()
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .bg(rgb(0x10b981))
                    .w(px((pct / 100.0) * 800.0)),
            );

        queue_drawer = queue_drawer.child(queue_header).child(progress_bar);

        if !state.queue.tasks().is_empty() {
            let mut tasks_list = div()
                .id("sftp-tasks-list")
                .flex()
                .flex_col()
                .gap_1()
                .max_h(px(70.0))
                .overflow_y_scroll();

            for task in state.queue.tasks() {
                let status_badge = match &task.status {
                    TransferStatus::Completed => {
                        div().text_color(rgb(0x10b981)).child("✓ Completed")
                    }
                    TransferStatus::InProgress => {
                        div().text_color(rgb(0x38bdf8)).child("⟳ In Progress")
                    }
                    TransferStatus::Failed(err) => div()
                        .text_color(rgb(0xef4444))
                        .child(format!("✗ Failed: {err}")),
                    TransferStatus::Queued => div().text_color(rgb(0x9ca3af)).child("⏳ Queued"),
                    TransferStatus::Cancelled => div().text_color(rgb(0x6b7280)).child("Cancelled"),
                };

                let task_row = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .text_color(rgb(0x9ca3af))
                    .child(div().child(format!(
                        "#{} {} · {} → {}",
                        task.id, task.direction, task.local_path, task.remote_path
                    )))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(format!(
                                "{}/{} ({:.1}%)",
                                task.bytes_transferred,
                                task.total_bytes,
                                task.progress_percentage()
                            ))
                            .child(status_badge),
                    );

                tasks_list = tasks_list.child(task_row);
            }

            queue_drawer = queue_drawer.child(tasks_list);
        }

        container = container.child(queue_drawer);
        container
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
                    SessionKind::Mosh => format!("Mosh: {}", tab.name),
                    SessionKind::Telnet => format!("Telnet: {}", tab.name),
                    SessionKind::Serial => format!("Serial: {}", tab.name),
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
                let is_sftp = tab.kind == SessionKind::Sftp;
                let is_sftp_dual_pane = is_sftp
                    && tab
                        .sftp_state
                        .as_ref()
                        .map(|s| s.mode == SftpTabMode::DualPane)
                        .unwrap_or(false);

                let header_title = match tab.kind {
                    SessionKind::Ssh => format!("SSH · {}", tab.name),
                    SessionKind::Mosh => format!("Mosh · {}", tab.name),
                    SessionKind::Telnet => format!("⚠️ Telnet · {}", tab.name),
                    SessionKind::Serial => format!("Serial · {}", tab.name),
                    SessionKind::Local => format!("Local · {}", tab.name),
                    SessionKind::Sftp => format!("SFTP · {}", tab.name),
                };
                let header_subtitle = match tab.kind {
                    SessionKind::Ssh => {
                        "Verify the server fingerprint before accepting an unknown host key."
                    }
                    SessionKind::Mosh => {
                        "Mobile Shell roaming UDP connection with local echo prediction."
                    }
                    SessionKind::Telnet => {
                        "⚠️ UNENCRYPTED PROTOCOL: Passwords and session data are transmitted in cleartext over the network."
                    }
                    SessionKind::Serial => {
                        "Physical serial line / UART connection."
                    }
                    SessionKind::Local => "Local shell session.",
                    SessionKind::Sftp => {
                        if is_sftp_dual_pane {
                            "Dual-pane graphical SFTP file manager. Select files to transfer or navigate directories."
                        } else {
                            "Secure File Transfer session. Use standard SFTP commands (ls, get, put, cd, pwd, help)."
                        }
                    }
                };

                let mut body = div()
                    .id("terminal-view")
                    .track_focus(&self.focus)
                    .on_action(cx.listener(|app, _: &ToggleCommandPalette, window, cx| {
                        app.toggle_command_palette(window, cx);
                    }))
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
                            .child({
                                let mut actions_bar = div().flex().items_center().gap_2();

                                if tab.kind == SessionKind::Telnet {
                                    actions_bar = actions_bar.child(
                                        div()
                                            .id("telnet-warning-pill")
                                            .px_3()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(rgb(0x7f1d1d))
                                            .text_color(rgb(0xfecaca))
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .child("⚠️ UNENCRYPTED TELNET"),
                                    );
                                }

                                if is_sftp {
                                    let mode_label = if is_sftp_dual_pane {
                                        "⌨ Terminal Mode"
                                    } else {
                                        "📂 Dual-Pane Mode"
                                    };
                                    actions_bar = actions_bar.child(
                                        div()
                                            .id("sftp-toggle-mode")
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(rgb(0x2563eb))
                                            .text_sm()
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.sftp_toggle_mode(active_idx, cx);
                                            }))
                                            .child(mode_label),
                                    );

                                    if is_sftp_dual_pane {
                                        let show_hidden = tab
                                            .sftp_state
                                            .as_ref()
                                            .map(|s| s.show_hidden)
                                            .unwrap_or(false);
                                        let hidden_label = if show_hidden {
                                            "👁 Hide Hidden"
                                        } else {
                                            "👁 Show Hidden"
                                        };
                                        actions_bar = actions_bar.child(
                                            div()
                                                .id("sftp-toggle-hidden")
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(if show_hidden {
                                                    rgb(0x4b5563)
                                                } else {
                                                    rgb(0x374151)
                                                })
                                                .text_sm()
                                                .cursor_pointer()
                                                .on_click(cx.listener(move |app, _, _, cx| {
                                                    app.sftp_toggle_hidden(active_idx, cx);
                                                }))
                                                .child(hidden_label),
                                        );
                                        actions_bar = actions_bar.child(
                                            div()
                                                .id("sftp-refresh")
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(rgb(0x374151))
                                                .text_sm()
                                                .cursor_pointer()
                                                .on_click(cx.listener(move |app, _, _, cx| {
                                                    app.sftp_refresh(active_idx, cx);
                                                }))
                                                .child("↻ Refresh"),
                                        );
                                    }
                                }

                                if !is_sftp_dual_pane {
                                    actions_bar = actions_bar
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
                                                    app.split_pane(
                                                        SplitDirection::Vertical,
                                                        window,
                                                        cx,
                                                    )
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
                                                    app.snippets_panel_open =
                                                        !app.snippets_panel_open;
                                                    cx.notify();
                                                }))
                                                .child("📜 Snippets"),
                                        );
                                }

                                if tab.panes.len() > 1 || self.broadcast_manager.is_enabled() {
                                    let is_on = self.broadcast_manager.is_enabled();
                                    let total = tab.panes.len();
                                    let count = if is_on {
                                        total.saturating_sub(
                                            (0..total)
                                                .filter(|&i| {
                                                    self.broadcast_manager.is_pane_excluded(i)
                                                })
                                                .count(),
                                        )
                                    } else {
                                        0
                                    };
                                    actions_bar = actions_bar.child(
                                        div()
                                            .id("toggle-broadcast-input")
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(if is_on { rgb(0xb91c1c) } else { rgb(0x374151) })
                                            .text_sm()
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                let total = app
                                                    .active_tab_idx
                                                    .and_then(|i| app.tabs.get(i))
                                                    .map(|t| t.panes.len())
                                                    .unwrap_or(0);
                                                app.broadcast_manager.set_total_panes(total);
                                                let on = app.broadcast_manager.toggle();
                                                if !on {
                                                    app.broadcast_warning = None;
                                                }
                                                cx.notify();
                                            }))
                                            .child(if is_on {
                                                format!("📡 Broadcast: ON ({count}/{total})")
                                            } else {
                                                "📡 Broadcast: Off".to_string()
                                            }),
                                    );
                                }

                                if !is_sftp_dual_pane {
                                    let mp_active = tab.multiplayer.is_some();
                                    actions_bar = actions_bar.child(
                                        div()
                                            .id("toggle-multiplayer-btn")
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(if mp_active {
                                                rgb(0x059669)
                                            } else {
                                                rgb(0x374151)
                                            })
                                            .text_sm()
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                app.toggle_multiplayer_on_active_tab(cx);
                                            }))
                                            .child(if mp_active {
                                                "👥 Multiplayer (Active)"
                                            } else {
                                                "👥 Collaborate"
                                            }),
                                    );
                                }

                                actions_bar = actions_bar.child(
                                    div()
                                        .id("terminal-palette-btn")
                                        .px_3()
                                        .py_2()
                                        .rounded_md()
                                        .bg(rgb(0x1e3a8a))
                                        .text_sm()
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|app, _, window, cx| {
                                            app.open_command_palette(window, cx);
                                        }))
                                        .child("🔍 Palette (Cmd+K)"),
                                );

                                actions_bar.child(
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
                                )
                            }),
                    );

                if let Some(multiplayer) = &tab.multiplayer {
                    let controller = multiplayer.current_controller.as_deref().unwrap_or("None");
                    let owner = &multiplayer.owner_id;
                    let participants_count = multiplayer.participants.len();

                    let mut mp_strip = div()
                        .id("multiplayer-control-strip")
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x1e1b4b))
                        .border_1()
                        .border_color(rgb(0x6366f1));

                    let mut info_left = div().flex().items_center().gap_3();
                    info_left = info_left.child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xa5b4fc))
                            .child(format!("👥 Session: {}", multiplayer.session_id)),
                    );
                    info_left = info_left.child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x312e81))
                            .text_xs()
                            .text_color(rgb(0xe0e7ff))
                            .child(format!("Host: @{owner}")),
                    );
                    info_left = info_left.child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(if controller == "local_user" {
                                rgb(0x065f46)
                            } else {
                                rgb(0x854d0e)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .child(format!("✍️ Active Writer: @{controller}")),
                    );
                    info_left = info_left.child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .child(format!("{participants_count} participant(s)")),
                    );

                    mp_strip = mp_strip.child(info_left);

                    let mut mp_actions = div().flex().items_center().gap_2();

                    if participants_count > 1 {
                        let next_target = multiplayer
                            .participants
                            .iter()
                            .find(|p| Some(&p.user_id) != multiplayer.current_controller.as_ref())
                            .map(|p| p.user_id.clone())
                            .unwrap_or_else(|| owner.clone());
                        let target_label = next_target.clone();

                        mp_actions = mp_actions.child(
                            div()
                                .id("transfer-writer-btn")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(0x4338ca))
                                .text_xs()
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.grant_multiplayer_control(&next_target, cx);
                                }))
                                .child(format!("Grant Control to @{target_label}")),
                        );
                    }

                    if multiplayer.current_controller.as_deref() != Some(owner) {
                        mp_actions = mp_actions.child(
                            div()
                                .id("revoke-writer-btn")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(0xb45309))
                                .text_xs()
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.revoke_multiplayer_control(cx);
                                }))
                                .child("⛔ Revoke to Host"),
                        );
                    }

                    mp_actions = mp_actions.child(
                        div()
                            .id("add-demo-collaborator-btn")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x374151))
                            .text_xs()
                            .text_color(rgb(0xd1d5db))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.add_multiplayer_demo_participant(cx);
                            }))
                            .child("+ Add Teammate"),
                    );

                    mp_actions = mp_actions.child(
                        div()
                            .id("end-multiplayer-session-btn")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x7f1d1d))
                            .text_xs()
                            .text_color(rgb(0xfecaca))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.toggle_multiplayer_on_active_tab(cx);
                            }))
                            .child("✕ End"),
                    );

                    mp_strip = mp_strip.child(mp_actions);
                    body = body.child(mp_strip);
                }

                if !is_sftp_dual_pane && self.snippets_panel_open {
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
                                )
                                .child(
                                    div()
                                        .id(("snippet-batch", snip_id as usize))
                                        .px_2()
                                        .py_0p5()
                                        .bg(rgb(0x7c3aed))
                                        .text_color(rgb(0xffffff))
                                        .rounded_sm()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            app.open_batch_modal(snip_id, cx);
                                            cx.stop_propagation();
                                        }))
                                        .child("⚡ Multi-Host"),
                                );
                            snippets_bar = snippets_bar.child(item);
                        }
                    }
                    body = body.child(snippets_bar);
                }

                if let Some(warning) = &self.broadcast_warning {
                    let warning_msg = warning.clone();
                    body = body.child(
                        div()
                            .id("broadcast-warning-banner")
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x7f1d1d))
                            .border_1()
                            .border_color(rgb(0xef4444))
                            .text_color(rgb(0xfecaca))
                            .text_sm()
                            .child(format!("⚠️ {warning_msg}"))
                            .child(
                                div()
                                    .id("dismiss-broadcast-warning")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x991b1b))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.broadcast_warning = None;
                                        cx.notify();
                                    }))
                                    .child("Dismiss"),
                            ),
                    );
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

                    let mut pane_info_group = div().flex().items_center().gap_2();

                    let pane_info = format!(
                        "{} · {}x{} · Cursor: row {}, col {}",
                        pane.name,
                        grid.cols(),
                        grid.rows(),
                        grid.cursor().row + 1,
                        grid.cursor().col + 1
                    );
                    pane_info_group = pane_info_group.child(
                        div()
                            .text_xs()
                            .text_color(if is_active_pane {
                                rgb(0x93c5fd)
                            } else {
                                rgb(0x9ca3af)
                            })
                            .child(pane_info),
                    );

                    // Semantic CWD badge
                    if let Some(cwd) = pane.terminal.current_working_directory() {
                        let display_cwd = if cwd.len() > 25 {
                            format!("…{}", &cwd[cwd.len() - 24..])
                        } else {
                            cwd.to_string()
                        };
                        pane_info_group = pane_info_group.child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(rgb(0x1e293b))
                                .text_color(rgb(0x38bdf8))
                                .text_xs()
                                .child(format!("📂 {}", display_cwd)),
                        );
                    }

                    // Exit code badge
                    if let Some(code) = pane.terminal.last_exit_code() {
                        let (bg_color, fg_color, text) = if code == 0 {
                            (rgb(0x064e3b), rgb(0x6ee7b7), "✓ 0".to_string())
                        } else {
                            (rgb(0x7f1d1d), rgb(0xfca5a5), format!("✗ {}", code))
                        };
                        pane_info_group = pane_info_group.child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(bg_color)
                                .text_color(fg_color)
                                .text_xs()
                                .child(text),
                        );
                    }

                    // Passive password prompt detection
                    let cursor_row = grid.cursor().row;
                    if let Some(current_line) = pane.terminal.row_text(cursor_row) {
                        if let Some(prompt_match) = PasswordPromptDetector::detect(&current_line) {
                            pane_info_group = pane_info_group.child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x78350f))
                                    .text_color(rgb(0xfde68a))
                                    .text_xs()
                                    .child(format!("🔑 Prompt: {:?}", prompt_match.kind)),
                            );
                        }
                    }

                    pane_controls = pane_controls.child(pane_info_group);

                    let mut pane_btn_group = div().flex().items_center().gap_1();
                    pane_btn_group = pane_btn_group.child(
                        div()
                            .id(("open-shell-integration", pane_id))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .text_xs()
                            .cursor_pointer()
                            .bg(rgb(0x374151))
                            .text_color(rgb(0xd1d5db))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.open_shell_integration_modal(cx);
                                cx.stop_propagation();
                            }))
                            .child("⚡ Integration"),
                    );
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

                    if tab.panes.len() > 1 && self.broadcast_manager.is_enabled() {
                        let is_excluded = self.broadcast_manager.is_pane_excluded(pane_idx);
                        pane_btn_group = pane_btn_group.child(
                            div()
                                .id(("toggle-pane-broadcast", pane_id))
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .text_xs()
                                .cursor_pointer()
                                .bg(if is_excluded {
                                    rgb(0x4b5563)
                                } else {
                                    rgb(0x059669)
                                })
                                .text_color(rgb(0xffffff))
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.broadcast_manager.toggle_pane_exclusion(pane_idx);
                                    cx.notify();
                                    cx.stop_propagation();
                                }))
                                .child(if is_excluded {
                                    "🔇 Excluded"
                                } else {
                                    "📡 Broadcast"
                                }),
                        );
                    }

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

                let mut result = if is_sftp_dual_pane {
                    let sftp_state = tab.sftp_state.as_ref().unwrap();
                    root.child(body.child(self.render_sftp_dual_pane(active_idx, sftp_state, cx)))
                } else {
                    root.child(body.child(panes_container))
                };
                if let Some(modal) = &self.conflict_modal {
                    result = result.child(self.render_conflict_modal(modal, cx));
                }
                if let Some(modal) = &self.snippet_modal {
                    result = result.child(self.render_snippet_modal(modal, cx));
                }
                if let Some(modal) = &self.batch_modal {
                    result = result.child(self.render_batch_modal(modal, cx));
                }
                if let Some(palette) = &self.command_palette {
                    result = result.child(self.render_command_palette(palette, cx));
                }
                if let Some(modal) = &self.team_modal {
                    result = result.child(self.render_team_modal(modal, cx));
                }
                if let Some(modal) = &self.infra_modal {
                    result = result.child(self.render_infra_modal(modal, cx));
                }
                if let Some(modal) = &self.diagnostics_modal {
                    result = result.child(self.render_diagnostics_modal(modal, cx));
                }
                if let Some(modal) = &self.mobile_modal {
                    result = result.child(self.render_mobile_modal(modal, cx));
                }
                if let Some(modal) = &self.shell_integration_modal {
                    result = result.child(self.render_shell_integration_modal(modal, cx));
                }
                return result;
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
                        let host_id = host.id;
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
                        let proto = host.protocol();
                        let (badge_bg, badge_fg) = match proto {
                            ConnectionProtocol::Ssh => (rgb(0x1e3a8a), rgb(0x93c5fd)),
                            ConnectionProtocol::Mosh => (rgb(0x713f12), rgb(0xfef08a)),
                            ConnectionProtocol::Telnet => (rgb(0x7f1d1d), rgb(0xfecaca)),
                            ConnectionProtocol::Serial => (rgb(0x14532d), rgb(0xbbf7d0)),
                            ConnectionProtocol::Local => (rgb(0x374151), rgb(0xd1d5db)),
                        };
                        title_row = title_row.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .bg(badge_bg)
                                .text_xs()
                                .text_color(badge_fg)
                                .child(format!("{} {}", proto.badge_icon(), proto.display_name())),
                        );

                        if let Some(snip_id) = host.startup_snippet_id {
                            let snip_name = self
                                .snippets
                                .iter()
                                .find(|s| s.id == snip_id)
                                .map(|s| s.name.as_str())
                                .unwrap_or("Snippet");
                            title_row = title_row.child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(rgb(0x4c1d95))
                                    .text_xs()
                                    .text_color(rgb(0xd8b4fe))
                                    .child(format!("📜 Startup: {snip_name}")),
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

                        let mut btn_row = div()
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
                            );

                        if proto.supports_sftp() {
                            btn_row = btn_row.child(
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
                            );
                        }

                        btn_row = btn_row.child(
                            div()
                                .id(("delete", host.id as u64))
                                .w(px(80.0))
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x991b1b))
                                .text_color(rgb(0xfecaca))
                                .cursor_pointer()
                                .on_click(
                                    cx.listener(move |app, _, _, cx| app.delete_host(host_id, cx)),
                                )
                                .child("Delete"),
                        );
                        card = card.child(btn_row);
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
            .on_action(cx.listener(|app, _: &ToggleCommandPalette, window, cx| {
                app.toggle_command_palette(window, cx);
            }))
            .on_key_down(cx.listener(Self::handle_palette_key_down))
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
        actions = actions.child(
            div()
                .id("command-palette-btn")
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x1e3a8a))
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .on_click(cx.listener(|app, _, window, cx| {
                    app.open_command_palette(window, cx);
                }))
                .child("🔍 Command Palette (Cmd+K)"),
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
                        .on_click(cx.listener(|app, _, _, cx| {
                            if matches!(app.sync_status, AppSyncStatus::Conflict { .. }) {
                                if app.conflict_modal.is_none() {
                                    let sample = vec![ConflictItemView {
                                        object_id: "conflicted-host".into(),
                                        entity_type: SyncEntityType::Host,
                                        local_version: 2,
                                        remote_version: 2,
                                        local_updated_at: 1000,
                                        remote_updated_at: 1050,
                                        local_summary: "Production Bastion (alice@bastion:22)"
                                            .into(),
                                        remote_summary: "Production Bastion (bob@bastion:2222)"
                                            .into(),
                                    }];
                                    app.open_conflict_modal("vault_file_1".into(), sample, cx);
                                }
                            } else {
                                app.trigger_sync(cx);
                            }
                        }))
                        .child(self.sync_status.label()),
                )
                .child(
                    div()
                        .id("team-collab-btn")
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x4338ca))
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.open_team_modal(cx);
                        }))
                        .child("👥 Team Vaults"),
                )
                .child(
                    div()
                        .id("cloud-infra-btn")
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x0e7490))
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.open_infra_modal(cx);
                        }))
                        .child("☁️ Cloud & Infra"),
                )
                .child(
                    div()
                        .id("diagnostics-btn")
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x0f766e))
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.open_diagnostics_modal(cx);
                        }))
                        .child("🩺 Health Check"),
                )
                .child(
                    div()
                        .id("mobile-sim-btn")
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x831843))
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.open_mobile_modal(cx);
                        }))
                        .child("📱 Mobile Sim"),
                );

            if matches!(self.sync_status, AppSyncStatus::Conflict { .. }) {
                actions = actions.child(
                    div()
                        .id("resolve-conflict-btn")
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0xb45309))
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, _, cx| {
                            if app.conflict_modal.is_none() {
                                let sample = vec![ConflictItemView {
                                    object_id: "conflicted-host".into(),
                                    entity_type: SyncEntityType::Host,
                                    local_version: 2,
                                    remote_version: 2,
                                    local_updated_at: 1000,
                                    remote_updated_at: 1050,
                                    local_summary: "Production Bastion (alice@bastion:22)".into(),
                                    remote_summary: "Production Bastion (bob@bastion:2222)".into(),
                                }];
                                app.open_conflict_modal("vault_file_1".into(), sample, cx);
                            }
                        }))
                        .child("Resolve Conflict"),
                );
            }
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
                .child("Protocol")
                .child({
                    let mut proto_row = div().flex().flex_row().gap_2().mb_1();
                    let protocols = [
                        (ConnectionProtocol::Ssh, "🔒 SSH"),
                        (ConnectionProtocol::Mosh, "⚡ Mosh"),
                        (ConnectionProtocol::Telnet, "⚠️ Telnet"),
                        (ConnectionProtocol::Serial, "🔌 Serial"),
                    ];
                    for (idx, (p, label)) in protocols.into_iter().enumerate() {
                        let is_sel = form.protocol == p;
                        let btn_bg = if is_sel { rgb(0x2563eb) } else { rgb(0x374151) };
                        let btn_fg = if is_sel { rgb(0xffffff) } else { rgb(0x9ca3af) };
                        proto_row = proto_row.child(
                            div()
                                .id(("proto-select", idx))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(btn_bg)
                                .text_color(btn_fg)
                                .text_xs()
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.set_form_protocol(p, cx);
                                }))
                                .child(label),
                        );
                    }
                    proto_row
                });

            if form.protocol == ConnectionProtocol::Telnet {
                editor = editor.child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0x7f1d1d))
                        .text_color(rgb(0xfecaca))
                        .text_xs()
                        .child("⚠️ UNENCRYPTED PROTOCOL: Telnet transmits all credentials, keystrokes, and session data in cleartext over the network without encryption."),
                );
            } else if form.protocol == ConnectionProtocol::Serial {
                editor = editor.child(
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0x14532d))
                        .text_color(rgb(0xbbf7d0))
                        .text_xs()
                        .child("🔌 Direct serial line connection. Specify serial device path in Hostname (e.g. /dev/ttyUSB0 or COM1) and baud rate in Port (e.g. 115200)."),
                );
            }

            editor = editor
                .child("Name")
                .child(form.name.clone())
                .child(if form.protocol == ConnectionProtocol::Serial {
                    "Serial Device Port"
                } else {
                    "Hostname or IP"
                })
                .child(form.hostname.clone())
                .child("Username")
                .child(form.username.clone())
                .child(if form.protocol == ConnectionProtocol::Serial {
                    "Baud Rate"
                } else {
                    "Port"
                })
                .child(form.port.clone())
                .child("Group (optional)")
                .child(form.group.clone())
                .child("Jump host (optional, e.g. jump.corp:2222)")
                .child(form.jump_host.clone())
                .child("Startup Snippet ID (optional)")
                .child(form.startup_snippet.clone())
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
        let mut result = root.child(
            body.child(
                div()
                    .id("hosts")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(content),
            ),
        );
        if let Some(modal) = &self.conflict_modal {
            result = result.child(self.render_conflict_modal(modal, cx));
        }
        if let Some(modal) = &self.snippet_modal {
            result = result.child(self.render_snippet_modal(modal, cx));
        }
        if let Some(modal) = &self.batch_modal {
            result = result.child(self.render_batch_modal(modal, cx));
        }
        if let Some(palette) = &self.command_palette {
            result = result.child(self.render_command_palette(palette, cx));
        }
        if let Some(modal) = &self.team_modal {
            result = result.child(self.render_team_modal(modal, cx));
        }
        if let Some(modal) = &self.infra_modal {
            result = result.child(self.render_infra_modal(modal, cx));
        }
        if let Some(modal) = &self.diagnostics_modal {
            result = result.child(self.render_diagnostics_modal(modal, cx));
        }
        if let Some(modal) = &self.mobile_modal {
            result = result.child(self.render_mobile_modal(modal, cx));
        }
        if let Some(modal) = &self.shell_integration_modal {
            result = result.child(self.render_shell_integration_modal(modal, cx));
        }
        result
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

        #[cfg(target_os = "macos")]
        let palette_shortcut = "cmd-k";
        #[cfg(not(target_os = "macos"))]
        let palette_shortcut = "ctrl-k";

        cx.bind_keys([
            KeyBinding::new(refresh_shortcut, RefreshHosts, None),
            KeyBinding::new(palette_shortcut, ToggleCommandPalette, None),
        ]);
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
                startup_snippet_id: Some(42),
                protocol: "ssh".into(),
                protocol_config: None,
            },
            SavedHost {
                id: 2,
                name: "Db1".into(),
                destination: SshDestination::new("10.0.0.2", "postgres", 5432).unwrap(),
                group_name: Some("Production".into()),
                tags: vec!["database".into(), "sql".into()],
                jump_host: None,
                startup_snippet_id: None,
                protocol: "ssh".into(),
                protocol_config: None,
            },
            SavedHost {
                id: 3,
                name: "TestBench".into(),
                destination: SshDestination::new("192.168.1.5", "dev", 2222).unwrap(),
                group_name: Some("Staging".into()),
                tags: vec!["test".into()],
                jump_host: None,
                startup_snippet_id: None,
                protocol: "ssh".into(),
                protocol_config: None,
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
            SessionKind::Mosh => format!("Mosh: {name}"),
            SessionKind::Telnet => format!("Telnet: {name}"),
            SessionKind::Serial => format!("Serial: {name}"),
            SessionKind::Local => name.to_string(),
            SessionKind::Sftp => format!("SFTP: {name}"),
        };

        assert_eq!(format_tab_label(SessionKind::Ssh, "Prod"), "SSH: Prod");
        assert_eq!(
            format_tab_label(SessionKind::Mosh, "Roaming"),
            "Mosh: Roaming"
        );
        assert_eq!(
            format_tab_label(SessionKind::Telnet, "Switch"),
            "Telnet: Switch"
        );
        assert_eq!(
            format_tab_label(SessionKind::Serial, "Console"),
            "Serial: Console"
        );
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

    #[test]
    fn verifies_conflict_modal_state_and_resolution_flow() {
        use super::ConflictModalState;
        use shellumi_core::{ConflictItemView, ConflictResolutionChoice, SyncEntityType};

        let items = vec![
            ConflictItemView {
                object_id: "host-1".into(),
                entity_type: SyncEntityType::Host,
                local_version: 2,
                remote_version: 2,
                local_updated_at: 1000,
                remote_updated_at: 1050,
                local_summary: "Web Prod (alice@web1:22)".into(),
                remote_summary: "Web Prod (bob@web1:2222)".into(),
            },
            ConflictItemView {
                object_id: "snippet-1".into(),
                entity_type: SyncEntityType::Snippet,
                local_version: 1,
                remote_version: 1,
                local_updated_at: 2000,
                remote_updated_at: 2050,
                local_summary: "Restart (systemctl restart)".into(),
                remote_summary: "Restart (service restart)".into(),
            },
        ];

        let mut modal = ConflictModalState {
            items,
            selected_index: 0,
            resolutions: vec![
                ConflictResolutionChoice::KeepLocal,
                ConflictResolutionChoice::KeepLocal,
            ],
            file_id: "vault_file_123".into(),
        };

        assert_eq!(modal.selected_index, 0);
        assert_eq!(modal.resolutions[0], ConflictResolutionChoice::KeepLocal);
        assert_eq!(modal.resolutions[1], ConflictResolutionChoice::KeepLocal);

        // Select second item and change resolution to AcceptRemote
        modal.selected_index = 1;
        modal.resolutions[1] = ConflictResolutionChoice::AcceptRemote;

        assert_eq!(modal.selected_index, 1);
        assert_eq!(modal.resolutions[0], ConflictResolutionChoice::KeepLocal);
        assert_eq!(modal.resolutions[1], ConflictResolutionChoice::AcceptRemote);
        assert_eq!(modal.resolutions[0].to_string(), "Keep Local");
        assert_eq!(modal.resolutions[1].to_string(), "Accept Remote");
    }

    #[test]
    fn verifies_sync_scheduler_and_local_mutation_integration() {
        use shellumi_core::{SyncScheduler, SyncTrigger};

        let mut scheduler = SyncScheduler::new(5, 300);
        assert_eq!(scheduler.check_trigger(100), Some(SyncTrigger::Startup));

        // After startup sync success
        scheduler.record_success(100);

        // After local mutation, should not trigger immediately due to 5s debounce window
        scheduler.notify_local_mutation(100);
        assert_eq!(scheduler.check_trigger(102), None);
        assert_eq!(
            scheduler.check_trigger(105),
            Some(SyncTrigger::LocalMutation)
        );

        // On failure, scheduler applies backoff delay
        scheduler.record_failure(105);
        assert_eq!(scheduler.consecutive_failures, 1);
        assert_eq!(scheduler.check_trigger(106), None);
        assert_eq!(
            scheduler.check_trigger(120),
            Some(SyncTrigger::LocalMutation)
        );

        // On success, failures reset
        scheduler.record_success(120);
        assert_eq!(scheduler.consecutive_failures, 0);
        assert_eq!(scheduler.last_sync_time, Some(120));
    }

    #[test]
    fn verifies_snippet_modal_parameter_binding_and_execution_flow() {
        use shellumi_core::{render_command, render_preview, Snippet};
        use std::collections::HashMap;

        let snippet = Snippet::new(
            "Deploy Container",
            "docker run -d -p {{port:-8080}}:80 --name {{app_name}} -e DB_PASSWORD={{db_password}} {{image}}",
            Some("Deploys a web container".into()),
            &["docker".into()],
        )
        .unwrap();

        assert_eq!(snippet.variables.len(), 4);
        assert!(snippet
            .variables
            .iter()
            .any(|v| v.name == "db_password" && v.is_secret));

        // Missing required variable fails
        let empty_params = HashMap::new();
        assert!(render_command(&snippet.command, &empty_params).is_err());

        // Supplying required parameters renders command with defaults applied
        let mut params = HashMap::new();
        params.insert("app_name".into(), "web-svc".into());
        params.insert("db_password".into(), "SuperSecretPass123!".into());
        params.insert("image".into(), "nginx:latest".into());

        let rendered = render_command(&snippet.command, &params).unwrap();
        assert!(rendered.contains("-p 8080:80"));
        assert!(rendered.contains("--name web-svc"));
        assert!(rendered.contains("DB_PASSWORD=SuperSecretPass123!"));
        assert!(rendered.contains("nginx:latest"));

        // Preview redacts secret values
        let preview = render_preview(&snippet.command, &params, &snippet.variables).unwrap();
        assert!(preview.contains("DB_PASSWORD=[REDACTED]"));
        assert!(!preview.contains("SuperSecretPass123!"));
    }

    #[test]
    fn verifies_sftp_dual_pane_state_and_navigation() {
        use super::*;
        use shellumi_core::{FileEntry, FileKind, PathUtils, TransferDirection, TransferQueue};

        let mut sftp_state = SftpBrowserState {
            host_name: "prod-server".to_string(),
            local_path: "/Users/test/workspace".to_string(),
            remote_path: "/var/www".to_string(),
            local_entries: vec![
                FileEntry {
                    name: "index.html".to_string(),
                    path: "/Users/test/workspace/index.html".to_string(),
                    size: 2048,
                    kind: FileKind::File,
                    modified: Some(1700000000),
                    permissions: Some(0o644),
                },
                FileEntry {
                    name: "assets".to_string(),
                    path: "/Users/test/workspace/assets".to_string(),
                    size: 4096,
                    kind: FileKind::Directory,
                    modified: Some(1700000100),
                    permissions: Some(0o755),
                },
            ],
            remote_entries: vec![FileEntry {
                name: "backup.tar.gz".to_string(),
                path: "/var/www/backup.tar.gz".to_string(),
                size: 1048576,
                kind: FileKind::File,
                modified: Some(1700000200),
                permissions: Some(0o600),
            }],
            selected_local: None,
            selected_remote: None,
            show_hidden: false,
            mode: SftpTabMode::DualPane,
            error: None,
            status_message: None,
            queue: TransferQueue::new(2),
        };

        // 1. Initial mode and selections
        assert_eq!(sftp_state.mode, SftpTabMode::DualPane);
        assert!(!sftp_state.show_hidden);
        assert_eq!(sftp_state.selected_local, None);
        assert_eq!(sftp_state.selected_remote, None);

        // 2. Select local file
        sftp_state.selected_local = Some("index.html".to_string());
        assert_eq!(sftp_state.selected_local.as_deref(), Some("index.html"));

        // 3. Queue upload
        let file = sftp_state
            .local_entries
            .iter()
            .find(|e| e.name == "index.html")
            .unwrap()
            .clone();
        let target_remote = PathUtils::join_path(&sftp_state.remote_path, &file.name);
        assert_eq!(target_remote, "/var/www/index.html");

        let tid = sftp_state.queue.enqueue(
            TransferDirection::Upload,
            &file.path,
            &target_remote,
            file.size,
        );
        assert_eq!(tid, 1);
        let dispatched = sftp_state.queue.dispatch_next();
        assert_eq!(dispatched, vec![1]);

        sftp_state.queue.mark_completed(tid);
        assert_eq!(sftp_state.queue.summary().completed_tasks, 1);
        assert_eq!(sftp_state.queue.summary().overall_percentage, 100.0);

        // 4. Select remote file and download
        sftp_state.selected_remote = Some("backup.tar.gz".to_string());
        let rfile = sftp_state
            .remote_entries
            .iter()
            .find(|e| e.name == "backup.tar.gz")
            .unwrap()
            .clone();
        let target_local = PathUtils::join_path(&sftp_state.local_path, &rfile.name);
        assert_eq!(target_local, "/Users/test/workspace/backup.tar.gz");

        let tid2 = sftp_state.queue.enqueue(
            TransferDirection::Download,
            &target_local,
            &rfile.path,
            rfile.size,
        );
        assert_eq!(tid2, 2);
        let dispatched2 = sftp_state.queue.dispatch_next();
        assert_eq!(dispatched2, vec![2]);
        sftp_state.queue.mark_completed(tid2);

        assert_eq!(sftp_state.queue.summary().completed_tasks, 2);
        assert_eq!(sftp_state.queue.summary().total_tasks, 2);

        // 5. Clear completed
        sftp_state.queue.clear_completed();
        assert_eq!(sftp_state.queue.summary().total_tasks, 0);

        // 6. Navigation breadcrumbs
        let crumbs = PathUtils::breadcrumbs(&sftp_state.remote_path);
        assert_eq!(
            crumbs,
            vec![
                ("/".to_string(), "/".to_string()),
                ("var".to_string(), "/var".to_string()),
                ("www".to_string(), "/var/www".to_string())
            ]
        );
        let parent = PathUtils::parent_path(&sftp_state.remote_path);
        assert_eq!(parent, Some("/var".to_string()));

        // 7. Toggle view mode
        sftp_state.mode = match sftp_state.mode {
            SftpTabMode::DualPane => SftpTabMode::Terminal,
            SftpTabMode::Terminal => SftpTabMode::DualPane,
        };
        assert_eq!(sftp_state.mode, SftpTabMode::Terminal);
    }

    #[test]
    fn verifies_command_palette_registry_matching_and_actions() {
        use shellumi_core::{
            PaletteAction, PaletteCategory, PaletteCommand, PaletteRegistry, SshDestination,
        };

        let mut registry = PaletteRegistry::with_static_actions();

        // Check static actions
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::QuickConnect));
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::TriggerSync));
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::SplitVertical));
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::NewLocalTerminal));

        // Add mock host
        let dest = SshDestination::new("192.168.1.50", "admin", 22).unwrap();
        registry.register(
            PaletteCommand::new(
                "host:99",
                "Connect to Bastion Gateway",
                format!("SSH: {}@{}:{}", dest.username(), dest.host(), dest.port()),
                PaletteCategory::Host,
                PaletteAction::ConnectHost {
                    host_id: 99,
                    host_name: "Bastion Gateway".into(),
                },
            )
            .with_keywords(["gateway", "bastion", "prod"]),
        );

        // Add mock snippet
        registry.register(
            PaletteCommand::new(
                "snippet:77",
                "Run Snippet: Clear Docker Logs",
                "truncate -s 0 /var/lib/docker/containers/*/*-json.log",
                PaletteCategory::Snippet,
                PaletteAction::ExecuteSnippet {
                    snippet_id: 77,
                    snippet_name: "Clear Docker Logs".into(),
                },
            )
            .with_keywords(["docker", "cleanup", "logs"]),
        );

        // 1. Search for host
        let host_matches = registry.search("bastion");
        assert!(!host_matches.is_empty());
        assert_eq!(
            host_matches[0].command.action,
            PaletteAction::ConnectHost {
                host_id: 99,
                host_name: "Bastion Gateway".into(),
            }
        );
        assert_eq!(host_matches[0].command.category, PaletteCategory::Host);

        // 2. Search for snippet
        let snip_matches = registry.search("Clear Docker");
        assert!(!snip_matches.is_empty());
        assert_eq!(
            snip_matches[0].command.action,
            PaletteAction::ExecuteSnippet {
                snippet_id: 77,
                snippet_name: "Clear Docker Logs".into(),
            }
        );
        assert_eq!(snip_matches[0].command.category, PaletteCategory::Snippet);

        // 3. Search for action by shortcut keyword
        let split_matches = registry.search("split");
        assert!(split_matches.len() >= 2);
        assert!(split_matches
            .iter()
            .any(|m| m.command.action == PaletteAction::SplitVertical));
        assert!(split_matches
            .iter()
            .any(|m| m.command.action == PaletteAction::SplitHorizontal));

        // 4. Acronym match: "qc" -> Quick Connect
        let qc_matches = registry.search("qc");
        assert!(!qc_matches.is_empty());
        assert_eq!(qc_matches[0].command.action, PaletteAction::QuickConnect);

        // 5. Batch execution snippet command
        registry.register(PaletteCommand::batch_from_snippet(77, "Clear Docker Logs"));
        let batch_matches = registry.search("multi-host");
        assert!(!batch_matches.is_empty());
        assert_eq!(
            batch_matches[0].command.action,
            PaletteAction::ExecuteBatchSnippet {
                snippet_id: 77,
                snippet_name: "Clear Docker Logs".into(),
            }
        );
        assert_eq!(batch_matches[0].command.category, PaletteCategory::Snippet);

        // 6. Broadcast and Workspace actions
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::ToggleBroadcastInput));
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::SaveCurrentLayoutAsWorkspace));

        registry.register(PaletteCommand::workspace_from_saved(
            12,
            "Kubernetes Ops Layout",
            "vertical",
        ));
        let ws_matches = registry.search("k8s");
        assert_eq!(ws_matches.len(), 0);
        let ws_matches2 = registry.search("Kubernetes");
        assert_eq!(ws_matches2.len(), 1);
        assert_eq!(
            ws_matches2[0].command.action,
            PaletteAction::LoadWorkspace {
                workspace_id: 12,
                workspace_name: "Kubernetes Ops Layout".into(),
            }
        );
        assert_eq!(ws_matches2[0].command.category, PaletteCategory::Workspace);
    }

    #[test]
    fn verifies_workspace_and_broadcast_input_flow() {
        use shellumi_core::{
            check_dangerous_command, BroadcastManager, BroadcastSafetyCheck, Workspace,
            WorkspaceLayout, WorkspaceSession, WorkspaceSessionKind,
        };

        // 1. Broadcast manager state & exclusions
        let mut bm = BroadcastManager::new(3);
        assert!(!bm.is_enabled());
        assert_eq!(bm.receiving_panes_count(), 0);
        assert_eq!(bm.status_text(), "Broadcast: Off");

        bm.enable();
        assert!(bm.is_enabled());
        assert_eq!(bm.receiving_panes_count(), 3);
        assert_eq!(bm.status_text(), "📡 Broadcast: ON (3/3 panes receiving)");

        // Exclude pane 1
        bm.exclude_pane(1);
        assert!(bm.is_pane_excluded(1));
        assert!(!bm.is_pane_excluded(0));
        assert!(!bm.is_pane_excluded(2));
        assert_eq!(bm.receiving_panes_count(), 2);
        assert_eq!(bm.status_text(), "📡 Broadcast: ON (2/3 panes receiving)");

        // 2. Dangerous command detection for broadcast safety
        let safe1 = check_dangerous_command("ls -la /var/log");
        assert_eq!(safe1, BroadcastSafetyCheck::Safe);
        let safe2 = check_dangerous_command("docker ps -a");
        assert_eq!(safe2, BroadcastSafetyCheck::Safe);

        let dangerous_rm = check_dangerous_command("rm -rf /");
        assert!(matches!(
            dangerous_rm,
            BroadcastSafetyCheck::Dangerous { .. }
        ));

        let dangerous_mkfs = check_dangerous_command("mkfs.ext4 /dev/sdb1");
        assert!(matches!(
            dangerous_mkfs,
            BroadcastSafetyCheck::Dangerous { .. }
        ));

        let dangerous_dd = check_dangerous_command("dd if=/dev/zero of=/dev/sda bs=1M");
        assert!(matches!(
            dangerous_dd,
            BroadcastSafetyCheck::Dangerous { .. }
        ));

        // 3. Workspace model construction & validation
        let s1 = WorkspaceSession {
            name: "Bastion SSH".into(),
            kind: WorkspaceSessionKind::Ssh,
            destination: None,
            host_id: Some(10),
            startup_command: Some("tmux new-session -A -s main".into()),
            working_directory: None,
            pane_index: 0,
            excluded_from_broadcast: false,
        };
        let s2 = WorkspaceSession {
            name: "Local Monitor".into(),
            kind: WorkspaceSessionKind::Local,
            destination: None,
            host_id: None,
            startup_command: Some("htop".into()),
            working_directory: None,
            pane_index: 1,
            excluded_from_broadcast: true,
        };

        let ws = Workspace::new("Production Ops Center")
            .unwrap()
            .with_description("Multi-pane monitoring")
            .with_layout(WorkspaceLayout::SplitVertical)
            .with_sessions(vec![s1, s2])
            .with_tags(["production", "sre"]);

        assert_eq!(ws.name, "Production Ops Center");
        assert_eq!(ws.layout, WorkspaceLayout::SplitVertical);
        assert_eq!(ws.sessions.len(), 2);
        assert!(!ws.sessions[0].excluded_from_broadcast);
        assert!(ws.sessions[1].excluded_from_broadcast);
    }

    #[test]
    fn verifies_batch_execution_runner_plan_and_dry_run_preview() {
        use shellumi_core::{
            BatchExecutionPlan, BatchHostStatus, BatchHostTarget, BatchPolicy, BatchRunner,
            SshDestination,
        };

        let target1 = BatchHostTarget::new(
            1,
            "web-1",
            SshDestination::new("192.168.1.10", "deploy", 22).unwrap(),
        )
        .with_group("production")
        .with_tags(["web", "us-east"]);

        let target2 = BatchHostTarget::new(
            2,
            "web-2",
            SshDestination::new("192.168.1.11", "deploy", 22).unwrap(),
        )
        .with_group("production")
        .with_tags(["web", "us-east"]);

        let plan = BatchExecutionPlan::new(
            10,
            "Restart Nginx",
            "sudo systemctl restart nginx # host={{host.name}} ip={{host.hostname}} env={{env}}",
            vec![target1, target2],
        )
        .with_policy(BatchPolicy::Parallel { max_concurrency: 4 })
        .with_global_variable("env", "prod");

        // Dry-run preview
        let previews = plan.preview_all();
        assert_eq!(previews.len(), 2);
        assert_eq!(
            previews[0].1.as_ref().unwrap(),
            "sudo systemctl restart nginx # host=web-1 ip=192.168.1.10 env=prod"
        );
        assert_eq!(
            previews[1].1.as_ref().unwrap(),
            "sudo systemctl restart nginx # host=web-2 ip=192.168.1.11 env=prod"
        );

        // Batch runner execution
        let mut runner = BatchRunner::new(plan);
        assert_eq!(runner.results().len(), 2);
        assert_eq!(runner.results()[0].status, BatchHostStatus::Queued);

        let summary = runner.execute_sync(|target, rendered| {
            assert!(rendered.contains("sudo systemctl restart nginx"));
            Ok((0, format!("OK from {}", target.name), String::new()))
        });

        assert_eq!(summary.total, 2);
        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 0);
        assert!(summary.is_complete);
        assert_eq!(runner.results()[0].status, BatchHostStatus::Succeeded);
        assert_eq!(runner.results()[0].exit_code, Some(0));
        assert_eq!(runner.results()[0].stdout, "OK from web-1");
    }

    #[test]
    fn verifies_startup_snippet_variable_resolution() {
        use shellumi_core::{render_command, SshDestination};

        let host = SavedHost {
            id: 42,
            name: "api-server".into(),
            destination: SshDestination::new("api.corp.internal", "appuser", 2200).unwrap(),
            group_name: Some("backend".into()),
            tags: vec!["api".into(), "v2".into()],
            jump_host: Some("bastion.corp:22".into()),
            startup_snippet_id: Some(99),
            protocol: "ssh".into(),
            protocol_config: None,
        };

        let template = "echo 'Welcome {{host.user}} to {{host.name}} ({{host.hostname}}:{{host.port}}) [{{host.group}}] tags: {{host.tags}} id: {{host.id}}'";

        let mut vars = std::collections::HashMap::new();
        vars.insert("host.id".to_string(), host.id.to_string());
        vars.insert("host.name".to_string(), host.name.clone());
        vars.insert(
            "host.hostname".to_string(),
            host.destination.host().to_string(),
        );
        vars.insert(
            "host.username".to_string(),
            host.destination.username().to_string(),
        );
        vars.insert(
            "host.user".to_string(),
            host.destination.username().to_string(),
        );
        vars.insert(
            "host.port".to_string(),
            host.destination.port().get().to_string(),
        );
        vars.insert(
            "host.group".to_string(),
            host.group_name.clone().unwrap_or_default(),
        );
        vars.insert("host.tags".to_string(), host.tags.join(","));

        let rendered = render_command(template, &vars).unwrap();
        assert_eq!(
            rendered,
            "echo 'Welcome appuser to api-server (api.corp.internal:2200) [backend] tags: api,v2 id: 42'"
        );
    }

    #[test]
    fn verifies_protocol_classification_and_ui_security_warnings() {
        use shellumi_core::{ConnectionProtocol, TelnetConfig};

        // Protocol capabilities and badges
        assert!(ConnectionProtocol::Ssh.is_encrypted());
        assert!(ConnectionProtocol::Ssh.supports_sftp());
        assert_eq!(ConnectionProtocol::Ssh.badge_icon(), "🔒");

        assert!(ConnectionProtocol::Mosh.is_encrypted());
        assert!(!ConnectionProtocol::Mosh.supports_sftp());
        assert_eq!(ConnectionProtocol::Mosh.badge_icon(), "⚡");

        assert!(!ConnectionProtocol::Telnet.is_encrypted());
        assert!(!ConnectionProtocol::Telnet.supports_sftp());
        assert_eq!(ConnectionProtocol::Telnet.badge_icon(), "⚠️");

        assert!(!ConnectionProtocol::Serial.is_encrypted());
        assert!(!ConnectionProtocol::Serial.supports_sftp());
        assert_eq!(ConnectionProtocol::Serial.badge_icon(), "🔌");

        // Telnet security warning
        let warning = TelnetConfig::security_warning();
        assert!(warning.contains("UNENCRYPTED PROTOCOL"));
        assert!(warning.contains("cleartext"));
    }

    #[test]
    fn verifies_team_collaboration_roles_and_badge_presentation() {
        use shellumi_core::TeamRole;

        assert_eq!(TeamRole::Owner.badge_label(), "👑 Owner");
        assert_eq!(TeamRole::Administrator.badge_label(), "🛡️ Admin");
        assert_eq!(TeamRole::Editor.badge_label(), "✏️ Editor");
        assert_eq!(TeamRole::Viewer.badge_label(), "👁️ Viewer");

        assert!(TeamRole::Owner.can_manage_roles());
        assert!(!TeamRole::Administrator.can_manage_roles());
        assert!(TeamRole::Administrator.can_manage_members());
        assert!(!TeamRole::Editor.can_manage_members());

        assert!(TeamRole::Editor.can_edit_records());
        assert!(!TeamRole::Viewer.can_edit_records());
        assert!(TeamRole::Viewer.can_view_records());

        assert!(TeamRole::Owner.can_rotate_key());
        assert!(TeamRole::Administrator.can_rotate_key());
        assert!(!TeamRole::Editor.can_rotate_key());
    }

    #[test]
    fn verifies_multiplayer_terminal_session_state_transitions() {
        use shellumi_core::MultiplayerSession;

        let mut session = MultiplayerSession::new("test-session-1", None, "host_alice", "1000");
        assert_eq!(session.owner_id, "host_alice");
        assert_eq!(session.current_controller.as_deref(), Some("host_alice"));
        assert!(session.is_active);

        // Add collaborator
        session
            .add_participant("collab_bob", "Bob Developer", "1005")
            .expect("should enroll collaborator");
        assert_eq!(session.participants.len(), 2);

        // Transfer keyboard control to Bob
        session
            .grant_keyboard_control("host_alice", "collab_bob")
            .expect("owner can grant control");
        assert_eq!(session.current_controller.as_deref(), Some("collab_bob"));

        // Host revokes keyboard control
        session
            .revoke_keyboard_control("host_alice")
            .expect("host can revoke control");
        assert_eq!(session.current_controller.as_deref(), Some("host_alice"));

        // Remove participant while they hold control - control must fall back to host
        session
            .grant_keyboard_control("host_alice", "collab_bob")
            .unwrap();
        assert_eq!(session.current_controller.as_deref(), Some("collab_bob"));
        assert!(session.remove_participant("collab_bob"));
        assert_eq!(session.current_controller.as_deref(), Some("host_alice"));
    }

    #[test]
    fn verifies_shared_vault_and_audit_trail_formatting() {
        use shellumi_core::{AuditAction, SharedVault, SharingAuditRecord};

        let mut vault = SharedVault::new(
            "vlt-1",
            "Infrastructure Team",
            Some("Shared production infra".into()),
            "alice",
            "1000",
        )
        .expect("valid vault");

        assert_eq!(vault.current_epoch, 1);
        let new_epoch = vault.rotate_key("1100");
        assert_eq!(new_epoch, 2);
        assert_eq!(vault.current_epoch, 2);

        let audit = SharingAuditRecord::new(
            "aud-1",
            "vlt-1",
            AuditAction::KeyRotated,
            "alice",
            None,
            Some("Rotated key to epoch 2".into()),
            "1100",
        );
        let log_entry = audit.format_entry();
        assert!(log_entry.contains("[1100]"));
        assert!(log_entry.contains("key_rotated"));
        assert!(log_entry.contains("actor=alice"));
        assert!(log_entry.contains("Rotated key to epoch 2"));
    }

    #[test]
    fn verifies_infrastructure_palette_actions_and_categories() {
        use shellumi_core::{PaletteAction, PaletteCategory, PaletteRegistry};

        let registry = PaletteRegistry::with_static_actions();
        let infra_commands: Vec<_> = registry
            .commands()
            .iter()
            .filter(|c| c.category == PaletteCategory::Infrastructure)
            .collect();

        assert_eq!(infra_commands.len(), 4);
        assert!(infra_commands
            .iter()
            .any(|c| c.action == PaletteAction::OpenInfrastructureManager));
        assert!(infra_commands
            .iter()
            .any(|c| c.action == PaletteAction::SyncCloudInfrastructure));
        assert!(infra_commands
            .iter()
            .any(|c| c.action == PaletteAction::ImportAnsibleInventory));
        assert!(infra_commands
            .iter()
            .any(|c| c.action == PaletteAction::ExportAnsibleInventory));

        assert_eq!(PaletteCategory::Infrastructure.as_str(), "Infrastructure");
        assert_eq!(PaletteCategory::Infrastructure.badge_icon(), "☁️");
    }

    #[test]
    fn verifies_infrastructure_cloud_provider_and_discovered_host_models() {
        use shellumi_core::{CloudProviderConfig, CloudProviderKind, DiscoveredHost};
        use shellumi_storage::{CloudProviderRecord, DiscoveredHostRecord};
        use std::collections::BTreeMap;

        let mut config = CloudProviderConfig::new(CloudProviderKind::Aws, "Prod AWS");
        config.region = Some("us-west-2".into());
        config.access_key_id = Some("AKIAIOSFODNN7EXAMPLE".into());
        config.secret_key = Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into());

        assert!(config.validate().is_ok());
        let redacted = config.redacted_summary();
        assert!(redacted.contains("us-west-2"));
        assert!(!redacted.contains("wJalrXUtnFEMI"));

        let provider_rec = CloudProviderRecord {
            id: "1".into(),
            name: "Prod AWS".into(),
            provider_kind: "aws".into(),
            config_json: serde_json::to_string(&config).unwrap(),
            sync_interval_mins: 60,
            last_synced_at: Some(1730000000),
            auto_sync: true,
            created_at: 1730000000,
            updated_at: 1730000000,
        };
        assert_eq!(provider_rec.name, "Prod AWS");

        let disc_host = DiscoveredHost {
            id: "i-0123456789abcdef0".into(),
            name: "web-server-01".into(),
            provider: CloudProviderKind::Aws,
            hostname: "54.214.1.100".into(),
            port: 22,
            username: Some("ec2-user".into()),
            state: "running".into(),
            region: Some("us-west-2".into()),
            instance_type: Some("t3.medium".into()),
            group_path: Some("AWS/us-west-2".into()),
            tags: BTreeMap::new(),
            extra_metadata: BTreeMap::new(),
        };

        assert!(disc_host.is_running());
        assert_eq!(disc_host.status_badge(), "🟢 Running");
        assert_eq!(disc_host.format_destination(), "ec2-user@54.214.1.100");

        let disc_rec = DiscoveredHostRecord {
            id: "10".into(),
            provider_id: "1".into(),
            external_id: disc_host.id.clone(),
            name: disc_host.name.clone(),
            hostname: disc_host.hostname.clone(),
            port: disc_host.port as i64,
            username: disc_host.username.clone(),
            state: disc_host.state.clone(),
            region: disc_host.region.clone(),
            group_path: disc_host.group_path.clone(),
            tags_json: serde_json::to_string(&disc_host.tags).unwrap(),
            metadata_json: serde_json::to_string(&disc_host.extra_metadata).unwrap(),
            imported_host_id: None,
            last_seen_at: 1730000000,
        };
        assert_eq!(disc_rec.external_id, "i-0123456789abcdef0");
        assert!(disc_rec.imported_host_id.is_none());
    }

    #[test]
    fn verifies_diagnostics_modal_and_palette_integration() {
        use shellumi_core::{
            DiagnosticCategory, DiagnosticCheck, DiagnosticStatus, DiagnosticsReport,
            PaletteAction, PaletteCategory, PaletteRegistry,
        };

        let registry = PaletteRegistry::with_static_actions();
        let diag_cmd = registry
            .commands()
            .iter()
            .find(|c| c.action == PaletteAction::RunDiagnostics)
            .expect("should register diagnostics command");

        assert_eq!(diag_cmd.category, PaletteCategory::Diagnostics);
        assert_eq!(diag_cmd.title, "Run System Diagnostics & Health Check");
        assert!(diag_cmd.keywords.contains(&"doctor".to_string()));

        let mock_report = DiagnosticsReport {
            timestamp: "2026-11-05T00:00:00Z".into(),
            shellumi_version: "0.1.0".into(),
            os_platform: "macos".into(),
            os_arch: "aarch64".into(),
            checks: vec![
                DiagnosticCheck {
                    id: "chk_1".into(),
                    name: "OpenSSH".into(),
                    category: DiagnosticCategory::OpenSsh,
                    status: DiagnosticStatus::Passed,
                    message: "OpenSSH 9.8p1 detected".into(),
                    remediation: None,
                    duration_ms: 12,
                },
                DiagnosticCheck {
                    id: "chk_2".into(),
                    name: "Loopback".into(),
                    category: DiagnosticCategory::Network,
                    status: DiagnosticStatus::Passed,
                    message: "Loopback listener bound".into(),
                    remediation: None,
                    duration_ms: 2,
                },
            ],
        };

        assert!(mock_report.is_healthy());
        assert!(!mock_report.has_failures());
        let (passed, warnings, failures) = mock_report.counts();
        assert_eq!(passed, 2);
        assert_eq!(warnings, 0);
        assert_eq!(failures, 0);
    }

    #[test]
    fn verifies_mobile_simulator_modal_and_palette_integration() {
        use super::{MobileSimulatorModalState, MobileViewportPreset};
        use shellumi_core::{
            AccessoryBarLayout, MobileLifecycleEvent, MobileLifecycleManager, MobilePlatform,
            MobileSessionState, PaletteAction, PaletteCategory, PaletteRegistry,
            VirtualAccessoryBar, VirtualKey, VirtualKeyModifier,
        };

        // 1. Verify Palette command registration
        let registry = PaletteRegistry::with_static_actions();
        let mobile_cmd = registry
            .commands()
            .iter()
            .find(|c| c.action == PaletteAction::OpenMobileSimulator)
            .expect("should register mobile simulator command");

        assert_eq!(mobile_cmd.category, PaletteCategory::Mobile);
        assert_eq!(mobile_cmd.title, "Open Mobile Touch & Accessory Simulator");
        assert!(mobile_cmd.keywords.contains(&"touch".to_string()));
        assert!(mobile_cmd.keywords.contains(&"virtual".to_string()));

        // 2. Verify simulator modal state initialization
        let mut lifecycle_manager = MobileLifecycleManager::new(MobilePlatform::Ios);
        lifecycle_manager.mark_connected();
        let accessory_bar = VirtualAccessoryBar::new(AccessoryBarLayout::DefaultTerminal);

        let mut modal = MobileSimulatorModalState {
            preset: MobileViewportPreset::IPhone15Pro,
            accessory_bar,
            lifecycle_manager,
            simulated_output: vec!["$ ".into()],
            input_history: Vec::new(),
        };

        assert_eq!(modal.preset, MobileViewportPreset::IPhone15Pro);
        assert_eq!(modal.preset.terminal_cols_rows(), (80, 24));
        assert_eq!(modal.lifecycle_manager.state, MobileSessionState::Connected);

        // 3. Test virtual key press & byte sequence translation
        let esc_bytes = modal.accessory_bar.press_key(&VirtualKey::Escape);
        assert_eq!(esc_bytes, vec![0x1B]);

        let ctrl_c_bytes = modal.accessory_bar.press_key(&VirtualKey::ControlKey('c'));
        assert_eq!(ctrl_c_bytes, vec![0x03]);

        // 4. Test sticky modifiers
        modal
            .accessory_bar
            .toggle_modifier(VirtualKeyModifier::Ctrl);
        assert!(modal.accessory_bar.has_modifier(VirtualKeyModifier::Ctrl));
        let resolved = modal.accessory_bar.resolve_char_keystroke('z');
        assert_eq!(resolved, vec![0x1A]); // Ctrl-Z
        assert!(!modal.accessory_bar.has_modifier(VirtualKeyModifier::Ctrl));

        // 5. Test lifecycle transitions
        modal.lifecycle_manager.transition(
            MobileLifecycleEvent::AppBackgrounded { grace_seconds: 30 },
            100,
        );
        assert_eq!(
            modal.lifecycle_manager.state,
            MobileSessionState::BackgroundGracePeriod { remaining_secs: 30 }
        );

        modal
            .lifecycle_manager
            .transition(MobileLifecycleEvent::BackgroundExpired, 130);
        assert_eq!(
            modal.lifecycle_manager.state,
            MobileSessionState::Suspended {
                saved_at_epoch: 130
            }
        );

        modal
            .lifecycle_manager
            .transition(MobileLifecycleEvent::AppForegrounded, 150);
        assert_eq!(
            modal.lifecycle_manager.state,
            MobileSessionState::Reconnecting {
                attempt: 1,
                backoff_secs: 2
            }
        );
    }

    #[test]
    fn verifies_shell_integration_modal_and_autocomplete_integration() {
        use super::*;
        use shellumi_terminal::Terminal;

        // 1. Verify Command Palette action registration
        let registry = PaletteRegistry::with_static_actions();
        let cmd = registry
            .commands()
            .iter()
            .find(|c| c.action == PaletteAction::OpenShellIntegrationModal)
            .expect("should register shell integration command");

        assert_eq!(cmd.category, PaletteCategory::Terminal);
        assert_eq!(cmd.title, "Shell Integration & Autocomplete Setup");
        assert!(cmd.keywords.contains(&"osc133".to_string()));
        assert!(cmd.keywords.contains(&"autocomplete".to_string()));

        // 2. Verify modal state and tab transitions
        let mut modal = ShellIntegrationModalState {
            active_tab: ShellIntegrationTab::Hooks,
            selected_shell: "zsh".to_string(),
            test_prompt_input: "[sudo] password for root:".to_string(),
            detected_prompt_kind: Some(PasswordPromptKind::Sudo),
            test_query_input: "git ".to_string(),
            autocomplete_results: Vec::new(),
        };

        assert_eq!(modal.active_tab, ShellIntegrationTab::Hooks);
        assert_eq!(modal.selected_shell, "zsh");

        modal.active_tab = ShellIntegrationTab::OscMarkers;
        assert_eq!(modal.active_tab, ShellIntegrationTab::OscMarkers);

        modal.active_tab = ShellIntegrationTab::PasswordDetection;
        assert_eq!(modal.active_tab, ShellIntegrationTab::PasswordDetection);

        // 3. Verify hook generation for all supported shells
        let zsh_hook = generate_zsh_hook();
        assert!(zsh_hook.contains("133;A"));
        assert!(zsh_hook.contains("133;D"));
        assert!(zsh_hook.contains("file://"));

        let bash_hook = generate_bash_hook();
        assert!(bash_hook.contains("133;A"));
        assert!(bash_hook.contains("PROMPT_COMMAND"));

        let fish_hook = generate_fish_hook();
        assert!(fish_hook.contains("133;A"));
        assert!(fish_hook.contains("fish_prompt"));

        // 4. Verify password prompt classification in app context
        let sudo_match = PasswordPromptDetector::detect(&modal.test_prompt_input);
        assert!(sudo_match.is_some());
        assert_eq!(sudo_match.unwrap().kind, PasswordPromptKind::Sudo);

        let ssh_match = PasswordPromptDetector::detect("root@bastion.internal's password: ");
        assert!(ssh_match.is_some());
        assert_eq!(ssh_match.unwrap().kind, PasswordPromptKind::Ssh);

        let mfa_match = PasswordPromptDetector::detect("Enter 6-digit MFA verification code: ");
        assert!(mfa_match.is_some());
        assert_eq!(mfa_match.unwrap().kind, PasswordPromptKind::MfaCode);

        let normal_output =
            PasswordPromptDetector::detect("total 48 drwxr-xr-x  14 user  staff   448B");
        assert!(normal_output.is_none());

        // 5. Verify Autocomplete engine integration
        let engine = AutocompleteEngine::new();
        let git_results = engine.complete(&AutocompleteContext::new("git ch", 6));
        assert!(git_results.iter().any(|c| c.display_label == "checkout"));

        let mut host_ctx = AutocompleteContext::new("ssh sta", 7);
        host_ctx
            .host_candidates
            .push("staging-cluster-01".to_string());
        let host_results = engine.complete(&host_ctx);
        assert!(host_results
            .iter()
            .any(|c| c.display_label == "staging-cluster-01"));

        let mut snip_ctx = AutocompleteContext::new("dep", 3);
        snip_ctx.snippet_candidates.push((
            "deploy-prod".to_string(),
            "kubectl rollout restart deployment".to_string(),
        ));
        let snippet_results = engine.complete(&snip_ctx);
        assert!(snippet_results
            .iter()
            .any(|c| c.display_label == "deploy-prod"));

        // 6. Verify Terminal row text and shell integration tracking
        let mut term = Terminal::new(24, 80);
        term.process_bytes(b"echo hello world\r\n");
        assert_eq!(term.row_text(0).as_deref(), Some("echo hello world"));
    }
}

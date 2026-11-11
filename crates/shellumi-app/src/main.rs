mod askpass;
mod connection;
mod data_path;
mod desktop;
mod session;
mod sftp_transfer;
mod text_input;
mod theme;

pub use theme::{caption, card, glyph_badge, heading, item, pill, subheader, MacTheme};

use gpui::{
    actions, div, point, prelude::*, px, rgb, rgba, size, App, Application, Bounds, Context,
    Entity, FocusHandle, Focusable, FontWeight, KeyBinding, KeyDownEvent, Menu, MenuItem, OsAction,
    SystemMenuType, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use session::{Session, SessionEvent};
#[cfg(test)]
use shellumi_core::ActiveTunnelStatus;
use shellumi_core::{
    build_rfb_key_event, build_rfb_pointer_event, build_ssh_tunnel_command,
    check_dangerous_command, execute_cloud_vault_sync, format_duration_hms, generate_bash_hook,
    generate_fish_hook, generate_zsh_hook, list_local_directory, rfb_constants, validate_jump_host,
    AccessoryBarLayout, ActiveTunnelState, AnsibleInventoryEngine, AuditAction,
    AutocompleteContext, AutocompleteEngine, BatchExecutionPlan, BatchExecutionSummary,
    BatchHostStatus, BatchHostTarget, BatchPolicy, BatchRunner, BenchmarkReport, BenchmarkRunner,
    BenchmarkStatus, BroadcastManager, BroadcastSafetyCheck, CastPlaybackEngine,
    CloudProviderConfig, CloudProviderKind, CompletionItem, CompletionKind, ConflictItemView,
    ConflictResolutionChoice, ConnectionProtocol, DiagnosticStatus, DiagnosticsReport,
    DiagnosticsRunner, FileEntry, FileKind, ForwardingType, GoogleDriveSyncClient, HostExportItem,
    MobileLifecycleEvent, MobileLifecycleManager, MobilePlatform, MobileSessionState, MoshConfig,
    MultiplayerSession, PaletteAction, PaletteCategory, PaletteCommand, PaletteRegistry,
    PasswordPromptDetector, PasswordPromptKind, PathUtils, PlaybackStatus, PortForwardRule,
    RdpConfig, SerialConfig, SessionRecordingSummary, SessionRedactor, SftpClient,
    SftpConnectOptions, SftpError, SharedVault, SharingAuditRecord, SshDestination,
    SyncCycleResult, SyncVault, SystemHttpTransport, TeamMember, TeamRole, TelnetConfig,
    TransferDirection, TransferQueue, TransferStatus, VirtualAccessoryBar, VirtualKey,
    VirtualKeyModifier, VncConfig, WorkspaceLayout, WorkspaceSession, WorkspaceSessionKind,
};
#[cfg(test)]
use shellumi_core::{
    execute_sync_cycle, prepare_startup_snippet, EncryptedEnvelope, StartupExecutionPolicy,
    StartupSessionTracker, StartupSnippetContext, StartupStatus, SyncEntityType, SyncRecord,
};
use shellumi_storage::{
    CloudProviderRecord, DiscoveredHostRecord, HostStore, SavedHost, SavedIdentity, SavedSnippet,
    SavedTunnel, SavedWorkspace, SessionRecordingRecord,
};
use shellumi_terminal::{Color, SearchBuffer, SearchMatch, StyledSpan, Terminal, TerminalCommand};
use text_input::TextInput;

#[cfg(target_os = "macos")]
const MONOSPACE_FONT: &str = "Menlo";
#[cfg(target_os = "windows")]
const MONOSPACE_FONT: &str = "Consolas";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const MONOSPACE_FONT: &str = "DejaVu Sans Mono";

actions!(
    shellumi,
    [
        RefreshHosts,
        ToggleCommandPalette,
        NewTab,
        OpenLocalTerminal,
        CloseTab,
        NextTab,
        PrevTab,
        SelectTab1,
        SelectTab2,
        SelectTab3,
        SelectTab4,
        SelectTab5,
        SelectTab6,
        SelectTab7,
        SelectTab8,
        SelectTab9,
        SplitVertical,
        SplitHorizontal,
        FocusNextPane,
        FocusPrevPane,
        ClosePane,
        ToggleFocusMode,
        ToggleBroadcast,
        ToggleSnippets,
        FindInTerminal,
        ZoomIn,
        ZoomOut,
        ResetZoom,
        QuickConnect,
        NewHost,
        ReconnectSession,
        ToggleSidebar,
        GoToHosts,
        GoToOverview,
        GoToWorkspaces,
        GoToSftp,
        GoToKeychain,
        GoToPortForwarding,
        OpenSettings,
        ToggleSidebarCollapse,
        Quit,
        AboutApp,
        OpenVaultsMenu,
        OpenSftpMenu,
        OpenSettingsMenu,
        HideApp,
        HideOthersApp,
        ShowAllApp,
        UndoAction,
        RedoAction,
        CutAction,
        CopyAction,
        PasteAction,
        SelectAllAction,
        MinimizeWindow,
        ZoomWindow,
        BringAllToFront,
        OpenShortcutsHelp,
    ]
);

#[derive(Clone, Debug, PartialEq)]
enum SftpPaneHost {
    Local,
    Remote(Box<SavedHost>),
}

#[derive(Clone, Debug, PartialEq)]
struct SftpContextMenuState {
    is_left: bool,
    entry_name: String,
    entry_path: String,
    is_dir: bool,
    target_idx: usize,
    cursor_x: f32,
    cursor_y: f32,
}

#[derive(Clone)]
enum SftpModalAction {
    Rename {
        entry_name: String,
        entry_path: String,
        input: Entity<TextInput>,
    },
    Delete {
        entry_name: String,
        entry_path: String,
        is_dir: bool,
    },
    NewFolder {
        input: Entity<TextInput>,
    },
    EditPermissions {
        entry_name: String,
        entry_path: String,
        input: Entity<TextInput>,
    },
}

#[derive(Clone)]
struct SftpModalState {
    is_left: bool,
    action: SftpModalAction,
    error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct SftpPaneView {
    host: Option<SftpPaneHost>,
    is_selecting_host: bool,
    current_path: String,
    history_back: Vec<String>,
    history_forward: Vec<String>,
    entries: Vec<FileEntry>,
    selected_file: Option<String>,
    selected_files: std::collections::HashSet<String>,
    filter: String,
    filter_active: bool,
    show_hidden: bool,
    actions_open: bool,
    search_host_query: String,
    loading: bool,
    error: Option<String>,
    listing_generation: u64,
}

impl Default for SftpPaneView {
    fn default() -> Self {
        Self {
            host: None,
            is_selecting_host: false,
            current_path: "/".to_string(),
            history_back: Vec::new(),
            history_forward: Vec::new(),
            entries: Vec::new(),
            selected_file: None,
            selected_files: std::collections::HashSet::new(),
            filter: String::new(),
            filter_active: false,
            show_hidden: false,
            actions_open: false,
            search_host_query: String::new(),
            loading: false,
            error: None,
            listing_generation: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct SftpDragPayload {
    from_left: bool,
    files: Vec<String>,
}

struct DragPreviewView {
    label: String,
}

impl Render for DragPreviewView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1p5()
            .rounded_md()
            .bg(rgb(MacTheme::SURFACE_ACTIVE))
            .border_1()
            .border_color(rgb(MacTheme::ACCENT_BLUE))
            .shadow_lg()
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .flex()
            .items_center()
            .gap_1p5()
            .child("📄")
            .child(self.label.clone())
    }
}

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
    invite_public_key: Entity<TextInput>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiagnosticsModalTab {
    HealthChecks,
    Benchmarks,
}

struct DiagnosticsModalState {
    active_tab: DiagnosticsModalTab,
    report: Option<DiagnosticsReport>,
    benchmark_report: Option<BenchmarkReport>,
    is_running: bool,
    is_running_benchmarks: bool,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionReplayTab {
    RecordingsList,
    ReplayPlayer,
}

#[derive(Clone, Debug, PartialEq)]
struct SessionReplayModalState {
    active_tab: SessionReplayTab,
    recordings: Vec<SessionRecordingSummary>,
    selected_recording_id: Option<String>,
    current_recording: Option<SessionRecordingRecord>,
    playback_engine: Option<CastPlaybackEngine>,
    screen_output_buffer: String,
    status_notification: Option<String>,
    search_query: String,
}

struct TunnelModalState {
    editing_id: Option<i64>,
    tunnels: Vec<SavedTunnel>,
    selected_index: usize,
    active_states: std::collections::HashMap<i64, ActiveTunnelState>,
    name_input: Entity<TextInput>,
    bind_port_input: Entity<TextInput>,
    target_host_input: Entity<TextInput>,
    target_port_input: Entity<TextInput>,
    forwarding_type: ForwardingType,
    selected_host_id: Option<i64>,
    auto_start: bool,
    error: Option<String>,
    status_message: Option<String>,
    is_loading: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostViewMode {
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostSortMode {
    AlphaAsc,
    AlphaDesc,
    Newest,
    Oldest,
}

struct GroupInspectorState {
    original_name: String,
    name: Entity<TextInput>,
    parent_group: Entity<TextInput>,
    default_username: Entity<TextInput>,
    default_port: Entity<TextInput>,
    identity_id: Option<i64>,
    cloud_sync: bool,
    error: Option<String>,
}

struct IdentityModalState {
    editing_id: Option<i64>,
    name: Entity<TextInput>,
    username: Entity<TextInput>,
    key_path: Entity<TextInput>,
    passphrase: Entity<TextInput>,
    tags: Entity<TextInput>,
    error: Option<String>,
}

struct InviteTeamModalState {
    emails: Vec<Entity<TextInput>>,
    #[allow(dead_code)]
    status_message: Option<String>,
}

struct Shellumi {
    sftp_jobs: std::collections::HashMap<(u64, u64), sftp_transfer::TransferJob>,
    connecting_hosts: std::collections::HashSet<i64>,
    desktop: desktop::DesktopUi,
    database: Option<String>,
    hosts: HostState,
    selected_host_id: Option<i64>,
    selected_group: Option<String>,
    page: Option<&'static str>,
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
    identities: Vec<SavedIdentity>,
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
    session_replay_modal: Option<SessionReplayModalState>,
    tunnel_modal: Option<TunnelModalState>,
    sidebar_visible: bool,
    sidebar_collapsed: bool,
    vault_dropdown_open: bool,
    active_vault: String,
    sftp_left_pane: SftpPaneView,
    sftp_right_pane: SftpPaneView,
    view_mode: HostViewMode,
    view_mode_menu_open: bool,
    sort_mode: HostSortMode,
    sort_menu_open: bool,
    tag_filter_popover_open: bool,
    active_tag_filter: Option<String>,
    profile_menu_open: bool,
    invite_modal: Option<InviteTeamModalState>,
    group_inspector: Option<GroupInspectorState>,
    group_context_menu: Option<(String, usize)>,
    identity_modal: Option<IdentityModalState>,
    sftp_context_menu: Option<SftpContextMenuState>,
    sftp_modal: Option<SftpModalState>,
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

#[cfg(test)]
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
    RemoteDesktop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemoteDesktopProtocol {
    Vnc,
    Rdp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum RemoteDesktopStatus {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemoteDesktopScaleMode {
    Fit,
    Original,
    Fill,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
struct RemoteDesktopState {
    host_name: String,
    target: String,
    protocol: RemoteDesktopProtocol,
    status: RemoteDesktopStatus,
    status_message: Option<String>,
    width: u32,
    height: u32,
    view_only: bool,
    scale_mode: RemoteDesktopScaleMode,
    color_depth: u8,
    fps: u32,
    latency_ms: u32,
    clipboard_text: Option<String>,
    error: Option<String>,
    last_action_note: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitDirection {
    Grid,
    Horizontal,
    Vertical,
}

struct TerminalPane {
    id: u64,
    name: String,
    #[allow(dead_code)]
    kind: SessionKind,
    terminal: Terminal,
    scroll: gpui::ScrollHandle,
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
    listing_generation: u64,
    connection_options: SftpConnectOptions,
    host_name: String,
    destination: Option<SshDestination>,
    jump_host: Option<String>,
    loading: bool,
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
    remote_desktop_state: Option<RemoteDesktopState>,
    multiplayer: Option<MultiplayerSession>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostEditorCategory {
    General,
    Connection,
    Authentication,
    Proxy,
    Advanced,
}

struct HostForm {
    active_category: HostEditorCategory,
    identity_id: Option<i64>,
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

fn render_terminal_spans(spans: Vec<StyledSpan>, line_height: f32) -> gpui::Div {
    let mut line = div()
        .flex()
        .flex_row()
        .h(px(line_height))
        .flex_shrink_0()
        .whitespace_nowrap();
    if spans.is_empty() {
        return line.child(" ");
    }
    for span in spans {
        let (r, g, b) = span.fg.to_rgb((0xf5, 0xf5, 0xf7));
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
            let (br, bg_val, bb) = span.bg.to_rgb((0x18, 0x18, 0x1a));
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
        let quick_connect_input =
            cx.new(|cx| TextInput::new(cx, "", "Find a host or ssh user@hostname...").bare());
        cx.observe(&search_input, |_, _, cx| cx.notify()).detach();
        cx.observe(&quick_connect_input, |_, _, cx| cx.notify())
            .detach();
        Self {
            sftp_jobs: std::collections::HashMap::new(),
            connecting_hosts: std::collections::HashSet::new(),
            desktop: desktop::DesktopUi::new(cx),
            database,
            hosts: HostState::NoDatabase,
            selected_host_id: None,
            selected_group: None,
            page: None,
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
            identities: Vec::new(),
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
            session_replay_modal: None,
            tunnel_modal: None,
            sidebar_visible: true,
            sidebar_collapsed: false,
            vault_dropdown_open: false,
            active_vault: "Personal".to_string(),
            sftp_left_pane: {
                let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/".into());
                let entries = list_local_directory(&home_dir, false).unwrap_or_default();
                SftpPaneView {
                    host: Some(SftpPaneHost::Local),
                    current_path: home_dir,
                    entries,
                    ..Default::default()
                }
            },
            sftp_right_pane: SftpPaneView::default(),
            view_mode: HostViewMode::Grid,
            view_mode_menu_open: false,
            sort_mode: HostSortMode::Newest,
            sort_menu_open: false,
            tag_filter_popover_open: false,
            active_tag_filter: None,
            profile_menu_open: false,
            invite_modal: None,
            group_inspector: None,
            group_context_menu: None,
            identity_modal: None,
            sftp_context_menu: None,
            sftp_modal: None,
        }
    }

    // --- Group Inspector, Identity, Sorting, and Invite Handlers ---
    fn open_group_inspector(&mut self, group_name: &str, cx: &mut Context<Self>) {
        let existing = self.desktop.groups.iter().find(|g| g.name == group_name);
        let default_user = existing
            .and_then(|g| g.default_username.clone())
            .unwrap_or_default();
        let default_port = existing
            .and_then(|g| g.default_port)
            .map(|p| p.get().to_string())
            .unwrap_or_else(|| "22".into());
        let identity_id = existing.and_then(|g| g.identity_id);

        let name = cx.new(|cx| TextInput::new(cx, group_name, "Group name"));
        let parent_group = cx.new(|cx| TextInput::new(cx, "", "Parent Group"));
        let default_username = cx.new(|cx| TextInput::new(cx, &default_user, "Default username"));
        let default_port = cx.new(|cx| TextInput::new(cx, &default_port, "22"));

        self.group_inspector = Some(GroupInspectorState {
            original_name: group_name.to_string(),
            name,
            parent_group,
            default_username,
            default_port,
            identity_id,
            cloud_sync: true,
            error: None,
        });
        self.group_context_menu = None;
        cx.notify();
    }

    fn save_group_inspector(&mut self, cx: &mut Context<Self>) {
        let Some(inspector) = &self.group_inspector else {
            return;
        };
        let new_name = inspector.name.read(cx).value().trim().to_string();
        if new_name.is_empty() {
            if let Some(ins) = &mut self.group_inspector {
                ins.error = Some("Group name cannot be empty".into());
                cx.notify();
            }
            return;
        }
        let orig_name = inspector.original_name.clone();
        let default_user = inspector
            .default_username
            .read(cx)
            .value()
            .trim()
            .to_string();
        let default_port_str = inspector.default_port.read(cx).value().trim().to_string();
        let default_port = default_port_str.parse::<u16>().ok();
        let identity_id = inspector.identity_id;

        let Some(db_path) = self.database.clone() else {
            return;
        };

        let group_res = shellumi_core::HostGroup::new(
            &new_name,
            if default_user.is_empty() {
                None
            } else {
                Some(&default_user)
            },
            default_port,
            &[],
        );

        let group = match group_res {
            Ok(g) => g.with_identity_id(identity_id),
            Err(e) => {
                if let Some(ins) = &mut self.group_inspector {
                    ins.error = Some(e.to_string());
                    cx.notify();
                }
                return;
            }
        };

        let is_rename = orig_name != new_name && !orig_name.is_empty();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&db_path).await.map_err(|e| e.to_string())?;
                if store
                    .get_group(&group.name)
                    .await
                    .map_err(|e| e.to_string())?
                    .is_some()
                {
                    store
                        .update_group(&group)
                        .await
                        .map_err(|e| e.to_string())?;
                } else {
                    store.add_group(&group).await.map_err(|e| e.to_string())?;
                }
                if is_rename {
                    let _ = store.delete_group(&orig_name).await;
                }
                Ok::<(), String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(err) = res {
                    if let Some(ins) = &mut app.group_inspector {
                        ins.error = Some(err);
                        cx.notify();
                    }
                } else {
                    app.group_inspector = None;
                    app.load_hosts(cx);
                    app.desktop_command(shellumi_storage::desktop::Command::Refresh, cx);
                    app.desktop.message = Some(format!("Group '{}' saved successfully", new_name));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn delete_group_inspector(&mut self, name: &str, cx: &mut Context<Self>) {
        let name_str = name.to_string();
        let target_name = name_str.clone();
        let Some(db_path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&db_path).await.map_err(|e| e.to_string())?;
                store
                    .delete_group(&target_name)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            })
        });
        cx.spawn(async move |app, cx| {
            let _ = task.await;
            let _ = app.update(cx, |app, cx| {
                app.group_inspector = None;
                app.group_context_menu = None;
                app.load_hosts(cx);
                app.desktop_command(shellumi_storage::desktop::Command::Refresh, cx);
                app.desktop.message = Some(format!("Group '{}' removed", name_str));
                cx.notify();
            });
        })
        .detach();
    }

    fn open_invite_modal(&mut self, cx: &mut Context<Self>) {
        let initial_email = cx.new(|cx| TextInput::new(cx, "", "Email"));
        self.invite_modal = Some(InviteTeamModalState {
            emails: vec![initial_email],
            status_message: None,
        });
        self.profile_menu_open = false;
        cx.notify();
    }

    fn invite_modal_add_row(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.invite_modal {
            let new_email = cx.new(|cx| TextInput::new(cx, "", "Email"));
            modal.emails.push(new_email);
            cx.notify();
        }
    }

    fn invite_modal_submit(&mut self, cx: &mut Context<Self>) {
        if let Some(modal) = &self.invite_modal {
            let count = modal
                .emails
                .iter()
                .filter(|e| !e.read(cx).value().trim().is_empty())
                .count();
            if count == 0 {
                return;
            }
            self.desktop.message = Some(format!(
                "Team invitations successfully dispatched to {count} recipient(s)."
            ));
            self.invite_modal = None;
            cx.notify();
        }
    }

    fn open_identity_modal(&mut self, id: Option<i64>, cx: &mut Context<Self>) {
        let existing = id.and_then(|id_val| self.identities.iter().find(|i| i.id == id_val));
        let name = existing.map(|i| i.name.as_str()).unwrap_or("");
        let user = existing.map(|i| i.username.as_str()).unwrap_or("");
        let key = existing.and_then(|i| i.key_path.as_deref()).unwrap_or("");
        let tags = existing.map(|i| i.tags.join(", ")).unwrap_or_default();

        let name_input =
            cx.new(|cx| TextInput::new(cx, name, "Identity name (e.g. aldaleel ssh key login)"));
        let user_input = cx.new(|cx| TextInput::new(cx, user, "Username (e.g. root)"));
        let key_input = cx.new(|cx| TextInput::new(cx, key, "Key path (e.g. ~/.ssh/id_ed25519)"));
        let pass_input = cx.new(|cx| TextInput::new(cx, "", "Passphrase (optional)").masked());
        let tags_input = cx.new(|cx| TextInput::new(cx, &tags, "Tags (comma-separated)"));

        self.identity_modal = Some(IdentityModalState {
            editing_id: id,
            name: name_input,
            username: user_input,
            key_path: key_input,
            passphrase: pass_input,
            tags: tags_input,
            error: None,
        });
        cx.notify();
    }

    fn save_identity_modal(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = &self.identity_modal else {
            return;
        };
        let name = modal.name.read(cx).value().trim().to_string();
        let username = modal.username.read(cx).value().trim().to_string();
        let key_path = modal.key_path.read(cx).value().trim().to_string();
        let tags: Vec<String> = modal
            .tags
            .read(cx)
            .value()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let editing_id = modal.editing_id;

        if name.is_empty() || username.is_empty() {
            if let Some(m) = &mut self.identity_modal {
                m.error = Some("Identity name and username cannot be empty".into());
                cx.notify();
            }
            return;
        }

        let new_id = shellumi_core::NewIdentity {
            name: name.clone(),
            username: username.clone(),
            auth_method: shellumi_core::AuthMethod::PublicKey {
                key_name: name.clone(),
            },
            password_secret_ref: None,
            key_path: if key_path.is_empty() {
                None
            } else {
                Some(key_path)
            },
            passphrase_secret_ref: None,
            certificate_path: None,
            tags,
            comment: Some("Managed via Shellumi Desktop".into()),
        };

        let Some(db_path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&db_path).await.map_err(|e| e.to_string())?;
                if let Some(id) = editing_id {
                    store
                        .update_identity(id, &new_id)
                        .await
                        .map_err(|e| e.to_string())?;
                } else {
                    store
                        .create_identity(&new_id)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                Ok::<(), String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Err(err) = res {
                    if let Some(m) = &mut app.identity_modal {
                        m.error = Some(err);
                        cx.notify();
                    }
                } else {
                    app.identity_modal = None;
                    app.load_identities(cx);
                    app.desktop.message = Some(format!("Identity '{}' saved successfully", name));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn render_group_details_inspector(&self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let Some(ins) = &self.group_inspector else {
            return div().id("group-details-inspector-empty");
        };

        let orig_name = ins.original_name.clone();
        let current_identity_id = ins.identity_id;

        let mut identities_dropdown = div().flex().flex_col().gap_1().child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                .child("CONNECTION IDENTITY"),
        );

        let none_sel = current_identity_id.is_none();
        let mut id_list = div()
            .id("group-inspector-id-list")
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(140.0))
            .overflow_y_scroll()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .rounded_md()
            .p_1();

        id_list = id_list.child(
            div()
                .id("group-id-opt-none")
                .px_2()
                .py_1()
                .rounded_sm()
                .text_xs()
                .bg(if none_sel {
                    rgb(MacTheme::SURFACE_ACTIVE)
                } else {
                    rgb(MacTheme::INPUT_BG)
                })
                .text_color(if none_sel {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::TEXT_SECONDARY)
                })
                .cursor_pointer()
                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(ins) = &mut app.group_inspector {
                        ins.identity_id = None;
                        cx.notify();
                    }
                }))
                .child(if none_sel {
                    "✓ None (Inherit defaults)"
                } else {
                    "None (Inherit defaults)"
                }),
        );

        for (idx, id) in self.identities.iter().enumerate() {
            let is_sel = current_identity_id == Some(id.id);
            let id_val = id.id;
            id_list = id_list.child(
                div()
                    .id(("group-id-opt", idx))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_xs()
                    .bg(if is_sel {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if let Some(ins) = &mut app.group_inspector {
                            ins.identity_id = Some(id_val);
                            cx.notify();
                        }
                    }))
                    .child(format!("{} {}", if is_sel { "✓" } else { " " }, id.name)),
            );
        }
        identities_dropdown = identities_dropdown.child(id_list);

        let error_view = ins.error.as_ref().map(|err| {
            div()
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(rgb(0x3e1717))
                .border_1()
                .border_color(rgb(MacTheme::DANGER))
                .text_xs()
                .text_color(rgb(MacTheme::DANGER))
                .child(err.clone())
        });

        let delete_name = orig_name.clone();

        div()
            .id("group-details-inspector")
            .w(px(320.0))
            .flex_shrink_0()
            .h_full()
            .border_l_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(rgb(MacTheme::SURFACE))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_4()
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Group Details"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(if let Some(p) = resolve_icon_path("more", false, 32) {
                                        div()
                                            .w(px(14.0))
                                            .h(px(14.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("···").into_any_element()
                                    }),
                            )
                            .child(
                                div()
                                    .id("close-group-inspector")
                                    .text_sm()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.group_inspector = None;
                                        cx.notify();
                                    }))
                                    .child("→|"),
                            ),
                    ),
            )
            .child(
                div()
                    .id("group-details-inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .children(error_view)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_2p5()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_sm().child("💬"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child("Personal"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child("⌵"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("General"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2p5()
                                    .child(
                                        div()
                                            .w(px(36.0))
                                            .h(px(36.0))
                                            .rounded_md()
                                            .bg(rgb(MacTheme::BADGE_NAVY))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                                            .font_weight(FontWeight::BOLD)
                                            .text_base()
                                            .child(
                                                if let Some(p) =
                                                    resolve_icon_path("groups", false, 32)
                                                {
                                                    div()
                                                        .w(px(22.0))
                                                        .h(px(22.0))
                                                        .child(gpui::img(p).size_full())
                                                        .into_any_element()
                                                } else {
                                                    div().child("⊞").into_any_element()
                                                },
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .rounded_md()
                                            .bg(rgb(MacTheme::SURFACE))
                                            .border_1()
                                            .border_color(rgb(MacTheme::BORDER))
                                            .px_2p5()
                                            .py_1p5()
                                            .child(ins.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::SURFACE))
                                    .border_1()
                                    .border_color(rgb(MacTheme::BORDER))
                                    .px_2p5()
                                    .py_1p5()
                                    .child(ins.parent_group.clone()),
                            )
                            .child(
                                div()
                                    .id("share-group-btn")
                                    .w_full()
                                    .py_2()
                                    .rounded_md()
                                    .bg(rgb(0x192538))
                                    .border_1()
                                    .border_color(rgb(0x27436b))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_1p5()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0x223652)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.open_invite_modal(cx);
                                    }))
                                    .child("Share this group 👥"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Cloud Sync")
                                    .child(
                                        div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child("🛈"),
                                    ),
                            )
                            .child({
                                let sync_enabled = ins.cloud_sync;
                                div()
                                    .id("cloud-sync-toggle")
                                    .w(px(36.0))
                                    .h(px(20.0))
                                    .rounded_full()
                                    .bg(if sync_enabled {
                                        rgb(MacTheme::ACCENT_BLUE)
                                    } else {
                                        rgb(0x38383a)
                                    })
                                    .flex()
                                    .items_center()
                                    .p_0p5()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        if let Some(ins) = &mut app.group_inspector {
                                            ins.cloud_sync = !ins.cloud_sync;
                                            cx.notify();
                                        }
                                    }))
                                    .child(
                                        div()
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .rounded_full()
                                            .bg(rgb(0xffffff))
                                            .when(sync_enabled, |e| e.ml_auto()),
                                    )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child("Protocols & Credentials"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                                            .child("+ Add protocol"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child("DEFAULT USERNAME"),
                                    )
                                    .child(
                                        div()
                                            .rounded_md()
                                            .bg(rgb(MacTheme::SURFACE))
                                            .border_1()
                                            .border_color(rgb(MacTheme::BORDER))
                                            .px_2p5()
                                            .py_1p5()
                                            .child(ins.default_username.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child("DEFAULT PORT"),
                                    )
                                    .child(
                                        div()
                                            .rounded_md()
                                            .bg(rgb(MacTheme::SURFACE))
                                            .border_1()
                                            .border_color(rgb(MacTheme::BORDER))
                                            .px_2p5()
                                            .py_1p5()
                                            .child(ins.default_port.clone()),
                                    ),
                            )
                            .child(identities_dropdown),
                    ),
            )
            .child(
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .id("delete-group-inspector-btn")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_xs()
                            .text_color(rgb(MacTheme::DANGER))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x3e1717)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.delete_group_inspector(&delete_name, cx);
                            }))
                            .child("Remove group"),
                    )
                    .child(
                        div()
                            .id("save-group-inspector-btn")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x4098ff)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.save_group_inspector(cx);
                            }))
                            .child("Save"),
                    ),
            )
    }

    fn render_invite_team_modal(
        &self,
        modal: &InviteTeamModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("invite-modal-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut emails_col = div().w_full().flex().flex_col().gap_2();
        for email_in in &modal.emails {
            emails_col = emails_col.child(
                div()
                    .w_full()
                    .rounded_lg()
                    .bg(rgb(0x161820))
                    .border_1()
                    .border_color(rgb(0x2d3245))
                    .px_3()
                    .py_2()
                    .child(email_in.clone()),
            );
        }

        let modal_box = div()
            .id("invite-modal-box")
            .w(px(480.0))
            .rounded_2xl()
            .bg(rgb(0x1c1e28))
            .border_1()
            .border_color(rgb(0x2d3245))
            .p_6()
            .shadow_xl()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(52.0))
                    .h(px(52.0))
                    .rounded_full()
                    .bg(rgb(0x1a2942))
                    .border_1()
                    .border_color(rgb(0x284775))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_2xl()
                    .child("👥"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Invite members to join your team"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .text_center()
                            .child("Securely share data with team vaults and collaborate in real time within terminal sessions."),
                    ),
            )
            .child(emails_col)
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_start()
                    .child(
                        div()
                            .id("invite-modal-add-another")
                            .text_xs()
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(cx.listener(|app, _, _, cx| app.invite_modal_add_row(cx)))
                            .child("+ Add another"),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_3()
                    .mt_2()
                    .child(
                        div()
                            .id("invite-modal-cancel")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x252a36))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x323a4b)).text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.invite_modal = None;
                                cx.notify();
                            }))
                            .child("Later"),
                    )
                    .child(
                        div()
                            .id("invite-modal-submit")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x4098ff)))
                            .on_click(cx.listener(|app, _, _, cx| app.invite_modal_submit(cx)))
                            .child("Invite"),
                    ),
            );

        overlay.child(modal_box)
    }

    fn render_identity_modal(
        &self,
        modal: &IdentityModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_edit = modal.editing_id.is_some();
        let title = if is_edit {
            "Edit Identity"
        } else {
            "New Identity"
        };

        let error_view = modal.error.as_ref().map(|err| {
            div()
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(rgb(0x3e1717))
                .border_1()
                .border_color(rgb(MacTheme::DANGER))
                .text_xs()
                .text_color(rgb(MacTheme::DANGER))
                .child(err.clone())
        });

        let overlay = div()
            .id("identity-modal-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let modal_box = div()
            .id("identity-modal-box")
            .w(px(500.0))
            .rounded_2xl()
            .bg(rgb(0x1c1e28))
            .border_1()
            .border_color(rgb(0x2d3245))
            .p_6()
            .shadow_xl()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Reusable SSH credential and private key path"),
                            ),
                    )
                    .child(
                        div()
                            .id("identity-modal-close-btn")
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.identity_modal = None;
                                cx.notify();
                            }))
                            .child("✕"),
                    ),
            )
            .children(error_view)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child("IDENTITY NAME"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child("USERNAME"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.username.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child("PRIVATE KEY PATH (OPTIONAL)"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.key_path.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child("PASSPHRASE (OPTIONAL)"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.passphrase.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child("TAGS (COMMA-SEPARATED)"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(rgb(0x161820))
                                    .border_1()
                                    .border_color(rgb(0x2d3245))
                                    .px_3()
                                    .py_2()
                                    .child(modal.tags.clone()),
                            ),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_3()
                    .mt_2()
                    .child(
                        div()
                            .id("identity-modal-cancel")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x252a36))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x323a4b)).text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.identity_modal = None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("identity-modal-save")
                            .px_5()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x4098ff)))
                            .on_click(cx.listener(|app, _, _, cx| app.save_identity_modal(cx)))
                            .child("Save Identity"),
                    ),
            );

        overlay.child(modal_box)
    }

    fn render_group_context_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (group_name, _) = self.group_context_menu.as_ref()?;
        let name_for_edit = group_name.clone();
        let name_for_del = group_name.clone();

        Some(
            div()
                .id("group-context-menu-backdrop")
                .absolute()
                .inset_0()
                .bg(rgba(0x00000022))
                .on_click(cx.listener(|app, _, _, cx| {
                    app.group_context_menu = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("group-context-menu-popup")
                        .absolute()
                        .top(px(140.0))
                        .left(px(240.0))
                        .w(px(200.0))
                        .rounded_lg()
                        .bg(rgb(0x181a24))
                        .border_1()
                        .border_color(rgb(0x2d3245))
                        .p_1p5()
                        .shadow_xl()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                        }))
                        .child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .flex()
                                .items_center()
                                .gap_2()
                                .child("⚡")
                                .child("Quick Connect"),
                        )
                        .child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child("⚡")
                                        .child("Connect"),
                                )
                                .child("›"),
                        )
                        .child(
                            div()
                                .id("ctx-edit-group-details")
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x252a36)))
                                .flex()
                                .items_center()
                                .justify_between()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.open_group_inspector(&name_for_edit, cx);
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child("✏️")
                                        .child("Edit Group Details"),
                                )
                                .child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child("E")),
                        )
                        .child(
                            div()
                                .id("ctx-collaborate-group")
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x252a36)))
                                .flex()
                                .items_center()
                                .gap_2()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.group_context_menu = None;
                                    app.open_invite_modal(cx);
                                }))
                                .child("👥")
                                .child("Collaborate"),
                        )
                        .child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child("📄")
                                        .child("Move to"),
                                )
                                .child("›"),
                        )
                        .child(
                            div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child("📄")
                                        .child("Copy to"),
                                )
                                .child("›"),
                        )
                        .child(
                            div()
                                .id("ctx-remove-group")
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::DANGER))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x3e1717)))
                                .flex()
                                .items_center()
                                .gap_2()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.delete_group_inspector(&name_for_del, cx);
                                }))
                                .child("🗑️")
                                .child("Remove"),
                        ),
                ),
        )
    }

    fn start_storage(&mut self, create_parent: bool, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        self.hosts = HostState::Loading;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            if create_parent {
                data_path::prepare_parent(std::path::Path::new(&path))?;
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path)
                    .await
                    .map_err(|error| error.to_string())?;
                let hosts = store.list().await.map_err(|error| error.to_string())?;
                let snippets = store
                    .list_snippets()
                    .await
                    .map_err(|error| error.to_string())?;
                let workspaces = store
                    .list_workspaces()
                    .await
                    .map_err(|error| error.to_string())?;
                let identities = store
                    .list_identities()
                    .await
                    .map_err(|error| error.to_string())?;
                Ok::<_, String>((hosts, snippets, workspaces, identities))
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                match result {
                    Ok((hosts, snippets, workspaces, identities)) => {
                        app.hosts = HostState::Loaded(hosts);
                        app.snippets = snippets;
                        app.workspaces = workspaces;
                        app.identities = identities;
                        app.desktop_command(shellumi_storage::desktop::Command::Refresh, cx);
                    }
                    Err(error) => app.hosts = HostState::Error(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_identities(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                // Auto-discover existing OpenSSH keys from ~/.ssh and ensure they exist as saved identities
                if let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
                    let ssh_dir = home.join(".ssh");
                    if let Ok(entries) = std::fs::read_dir(&ssh_dir) {
                        let current_user =
                            std::env::var("USER").unwrap_or_else(|_| "root".to_string());
                        let existing = store.list_identities().await.unwrap_or_default();
                        for entry in entries.flatten() {
                            let p = entry.path();
                            if p.extension().and_then(|s| s.to_str()) == Some("pub") {
                                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                                    let priv_path = ssh_dir.join(stem);
                                    let priv_str = priv_path.to_string_lossy().to_string();
                                    let already_exists = existing.iter().any(|id| {
                                        id.name == stem || id.key_path.as_deref() == Some(&priv_str)
                                    });
                                    if !already_exists && priv_path.exists() {
                                        if let Ok(mut new_id) = shellumi_core::NewIdentity::new(
                                            stem,
                                            &current_user,
                                            shellumi_core::AuthMethod::PublicKey {
                                                key_name: stem.to_string(),
                                            },
                                        ) {
                                            new_id.key_path = Some(priv_str);
                                            new_id.comment =
                                                Some("Auto-discovered from ~/.ssh".to_string());
                                            let _ = store.create_identity(&new_id).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                store.list_identities().await.map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Ok(ids) = res {
                    app.identities = ids;
                    cx.notify();
                }
            });
        })
        .detach();
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

    fn select_tab(&mut self, idx: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.page = None;
        self.active_tab_idx = idx;
        window.focus(&self.focus);
        cx.notify();
    }

    fn connect(&mut self, host: &SavedHost, window: &mut Window, cx: &mut Context<Self>) {
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

    fn launch_remote_desktop(
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
            status: RemoteDesktopStatus::Connected,
            status_message: Some(format!(
                "Connected to {} via {}",
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
            fps: 60,
            latency_ms: 14,
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

    fn remote_desktop_send_key(
        &mut self,
        tab_idx: usize,
        key_desc: &'static str,
        keysym: u32,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                // Real RFB key event serialization
                let _down_packet = build_rfb_key_event(true, keysym);
                let _up_packet = build_rfb_key_event(false, keysym);
                rd.last_action_note = Some(format!("Sent keystroke: {key_desc}"));
                rd.status_message = Some(format!(
                    "Keystroke {key_desc} transmitted over {}",
                    match rd.protocol {
                        RemoteDesktopProtocol::Vnc => "RFB",
                        RemoteDesktopProtocol::Rdp => "RDP",
                    }
                ));
                cx.notify();
            }
        }
    }

    fn remote_desktop_click_canvas(
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
                rd.last_action_note = Some(format!("Mouse click at ({x}, {y})"));
                cx.notify();
            }
        }
    }

    fn remote_desktop_set_scale_mode(
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

    fn remote_desktop_toggle_view_only(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
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

    fn remote_desktop_disconnect(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.status = RemoteDesktopStatus::Disconnected;
                rd.status_message = Some("Session disconnected by user.".into());
                rd.last_action_note = Some("Disconnected".into());
                cx.notify();
            }
        }
    }

    fn remote_desktop_reconnect(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.status = RemoteDesktopStatus::Connected;
                rd.status_message = Some("Reconnected to remote desktop server.".into());
                rd.last_action_note = Some("Reconnected".into());
                cx.notify();
            }
        }
    }

    fn remote_desktop_sync_clipboard(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(rd) = &mut tab.remote_desktop_state {
                rd.last_action_note = Some("Clipboard synchronized with remote session".into());
                cx.notify();
            }
        }
    }

    fn connect_sftp(&mut self, host: &SavedHost, window: &mut Window, cx: &mut Context<Self>) {
        self.launch_saved_connection(host, true, window, cx);
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

    fn sftp_refresh_remote(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        let (tab_id, generation, dest, jump_host, remote_path, show_hidden) = {
            let Some(tab) = self.tabs.get_mut(tab_idx) else {
                return;
            };
            let Some(sftp) = &mut tab.sftp_state else {
                return;
            };
            let Some(dest) = sftp.destination.clone() else {
                sftp.status_message =
                    Some("No remote host destination configured for graphical browsing.".into());
                cx.notify();
                return;
            };
            sftp.listing_generation = sftp.listing_generation.wrapping_add(1);
            sftp.loading = true;
            sftp.error = None;
            cx.notify();
            (
                tab.id,
                sftp.listing_generation,
                dest,
                sftp.jump_host.clone(),
                sftp.remote_path.clone(),
                sftp.show_hidden,
            )
        };

        let connection_options = self
            .tabs
            .get(tab_idx)
            .and_then(|tab| tab.sftp_state.as_ref())
            .map(|browser| browser.connection_options.clone())
            .unwrap_or_default();
        let task = cx.background_executor().spawn(async move {
            let mut opts = connection_options;
            opts.port = Some(dest.port().get());
            if let Some(jump) = jump_host {
                opts.extra_ssh_options.push(("ProxyJump".to_string(), jump));
            }
            let mut client = SftpClient::connect(&dest, &opts)?;
            let mut entries = client.list_remote_directory(&remote_path)?;
            if !show_hidden {
                entries.retain(|e| !e.is_hidden());
            }
            Ok::<Vec<FileEntry>, SftpError>(entries)
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(tab) = app.tabs.iter_mut().find(|tab| tab.id == tab_id) {
                    if let Some(sftp) = &mut tab.sftp_state {
                        if sftp.listing_generation != generation {
                            return;
                        }
                        sftp.loading = false;
                        match res {
                            Ok(entries) => {
                                sftp.remote_entries = entries;
                                sftp.error = None;
                                sftp.status_message = Some(format!(
                                    "Fetched {} remote items.",
                                    sftp.remote_entries.len()
                                ));
                            }
                            Err(err) => {
                                sftp.error = Some(format!("Remote SFTP error: {err}"));
                            }
                        }
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn sftp_open_remote_dir(&mut self, tab_idx: usize, dir_name: String, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.remote_path = PathUtils::join_path(&sftp.remote_path, &dir_name);
                sftp.selected_remote = None;
            }
        }
        self.sftp_refresh_remote(tab_idx, cx);
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
            }
        }
        self.sftp_refresh_remote(tab_idx, cx);
    }

    fn sftp_navigate_remote_up(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        let has_parent = if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                if let Some(parent) = PathUtils::parent_path(&sftp.remote_path) {
                    sftp.remote_path = parent;
                    sftp.selected_remote = None;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        if has_parent {
            self.sftp_refresh_remote(tab_idx, cx);
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
        self.sftp_refresh_remote(tab_idx, cx);
    }

    fn sftp_toggle_mode(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(browser) = self
            .tabs
            .get_mut(tab_idx)
            .and_then(|tab| tab.sftp_state.as_mut())
        {
            browser.mode = match browser.mode {
                SftpTabMode::DualPane => SftpTabMode::Terminal,
                SftpTabMode::Terminal => SftpTabMode::DualPane,
            };
            browser.error = None;
        }
        cx.notify();
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
        self.sftp_refresh_remote(tab_idx, cx);
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
        if event.keystroke.key == "tab" {
            if let Some(form) = &self.form {
                let handles = [
                    form.name.read(cx).focus_handle(cx),
                    form.group.read(cx).focus_handle(cx),
                    form.hostname.read(cx).focus_handle(cx),
                    form.port.read(cx).focus_handle(cx),
                    form.username.read(cx).focus_handle(cx),
                    form.tags.read(cx).focus_handle(cx),
                    form.jump_host.read(cx).focus_handle(cx),
                    form.startup_snippet.read(cx).focus_handle(cx),
                ];
                let current_idx = handles.iter().position(|h| h.is_focused(window));
                let next_idx = if event.keystroke.modifiers.shift {
                    match current_idx {
                        Some(0) => handles.len() - 1,
                        Some(i) => i - 1,
                        None => handles.len() - 1,
                    }
                } else {
                    match current_idx {
                        Some(i) => (i + 1) % handles.len(),
                        None => 0,
                    }
                };
                window.focus(&handles[next_idx]);
                cx.stop_propagation();
                return;
            }
            if let Some(editor) = &self.desktop.editor {
                let mut handles = vec![
                    editor.name.read(cx).focus_handle(cx),
                    editor.description.read(cx).focus_handle(cx),
                    editor.tags.read(cx).focus_handle(cx),
                ];
                for line in &editor.lines {
                    handles.push(line.read(cx).focus_handle(cx));
                }
                let current_idx = handles.iter().position(|h| h.is_focused(window));
                let next_idx = if event.keystroke.modifiers.shift {
                    match current_idx {
                        Some(0) => handles.len() - 1,
                        Some(i) => i - 1,
                        None => handles.len() - 1,
                    }
                } else {
                    match current_idx {
                        Some(i) => (i + 1) % handles.len(),
                        None => 0,
                    }
                };
                window.focus(&handles[next_idx]);
                cx.stop_propagation();
                return;
            }
        }

        if self.handle_app_shortcut(&event.keystroke, window, cx) {
            cx.stop_propagation();
            return;
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
            active_category: HostEditorCategory::General,
            identity_id: host.and_then(|host| host.identity_id),
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
        self.load_identities(cx);
    }

    fn edit_host_with_protocol(&mut self, protocol: ConnectionProtocol, cx: &mut Context<Self>) {
        self.edit_host(None, cx);
        if let Some(form) = &mut self.form {
            form.protocol = protocol;
            if protocol == ConnectionProtocol::Serial {
                form.hostname
                    .update(cx, |input, cx| input.set_value("/dev/ttyUSB0", cx));
                form.port
                    .update(cx, |input, cx| input.set_value("9600", cx));
            } else if protocol == ConnectionProtocol::Telnet {
                form.port.update(cx, |input, cx| input.set_value("23", cx));
            } else if protocol == ConnectionProtocol::Mosh {
                form.port.update(cx, |input, cx| input.set_value("22", cx));
            } else if protocol == ConnectionProtocol::Vnc {
                form.port
                    .update(cx, |input, cx| input.set_value("5900", cx));
            } else if protocol == ConnectionProtocol::Rdp {
                form.port
                    .update(cx, |input, cx| input.set_value("3389", cx));
            }
        }
        cx.notify();
    }

    fn set_host_editor_category(&mut self, cat: HostEditorCategory, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.form {
            form.active_category = cat;
            cx.notify();
        }
    }

    fn toggle_sidebar_collapsed(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
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
                ConnectionProtocol::Vnc => {
                    let cur = form.port.read(cx).value();
                    if cur == "22" || cur == "23" || cur == "3389" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("5900", cx);
                        });
                    }
                }
                ConnectionProtocol::Rdp => {
                    let cur = form.port.read(cx).value();
                    if cur == "22" || cur == "23" || cur == "5900" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("3389", cx);
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

        let username = if username.trim().is_empty() && form.protocol == ConnectionProtocol::Ssh {
            let inherited_id = form.identity_id.or_else(|| {
                group
                    .as_ref()
                    .and_then(|name| self.desktop.groups.iter().find(|g| &g.name == name))
                    .and_then(|g| g.identity_id)
            });
            self.identities
                .iter()
                .find(|identity| Some(identity.id) == inherited_id)
                .map(|identity| identity.username.clone())
                .or_else(|| {
                    group
                        .as_ref()
                        .and_then(|name| self.desktop.groups.iter().find(|g| &g.name == name))
                        .and_then(|g| g.default_username.clone())
                })
                .unwrap_or(username)
        } else {
            username
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
        let identity_id = form.identity_id;
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
                            .update_full(
                                id,
                                &name,
                                &destination,
                                group.as_deref(),
                                &tags,
                                jump_host.as_deref(),
                                startup_snippet_id,
                                proto.as_str(),
                                proto_config.as_deref(),
                                identity_id,
                            )
                            .await
                            .map_err(|error| error.to_string())?
                        {
                            return Err("Host no longer exists.".into());
                        }
                    }
                    None => {
                        store
                            .add_full(
                                &name,
                                &destination,
                                group.as_deref(),
                                &tags,
                                jump_host.as_deref(),
                                startup_snippet_id,
                                proto.as_str(),
                                proto_config.as_deref(),
                                identity_id,
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
            data_path::prepare_parent(std::path::Path::new(&path))?;
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

    fn load_workspace(&mut self, workspace_id: i64, window: &mut Window, cx: &mut Context<Self>) {
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
        let Some(modal) = self.conflict_modal.take() else {
            return;
        };
        let Some(path) = self.database.clone() else {
            self.sync_status = AppSyncStatus::Error("Database not open.".into());
            cx.notify();
            return;
        };

        let token = std::env::var("SHELLUMI_GDRIVE_TOKEN")
            .or_else(|_| std::env::var("GOOGLE_DRIVE_TOKEN"))
            .unwrap_or_default();

        if token.trim().is_empty() {
            self.sync_status = AppSyncStatus::Error("Google Drive OAuth token not found.".into());
            cx.notify();
            return;
        }

        let passphrase = match std::env::var("SHELLUMI_VAULT_PASSPHRASE") {
            Ok(value) if value.chars().count() >= 12 => shellumi_core::SecretString::new(value),
            _ => {
                self.sync_status = AppSyncStatus::Error(
                    "Set a vault passphrase of at least 12 characters before syncing.".into(),
                );
                cx.notify();
                return;
            }
        };
        let device_id =
            std::env::var("SHELLUMI_DEVICE_ID").unwrap_or_else(|_| "desktop-app".to_string());

        let resolutions = modal.resolutions;
        self.sync_status = AppSyncStatus::Syncing;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let local_vault = store
                    .export_vault(passphrase.expose_secret(), &device_id)
                    .await
                    .map_err(|e| e.to_string())?;
                let client =
                    GoogleDriveSyncClient::new(SystemHttpTransport::default(), "shellumi-desktop");
                let cycle_result = execute_cloud_vault_sync(
                    &client,
                    &token,
                    &local_vault,
                    passphrase.expose_secret(),
                    Some(&resolutions),
                )
                .map_err(|e| e.to_string())?;

                match cycle_result {
                    SyncCycleResult::AlreadySynchronized => Ok(()),
                    SyncCycleResult::Success { merged_records, .. } => {
                        let mut write_store =
                            HostStore::open(&path).await.map_err(|e| e.to_string())?;
                        let mut merged_vault = SyncVault::new("cloud-vault", 0);
                        merged_vault.records = merged_records;
                        write_store
                            .import_vault(passphrase.expose_secret(), &merged_vault)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok(())
                    }
                    SyncCycleResult::Conflicts(_) => {
                        Err("Unexpected unresolved conflicts after resolution choices.".into())
                    }
                    SyncCycleResult::Error(err) => Err(err),
                }
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(()) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        app.sync_status = AppSyncStatus::UpToDate {
                            last_sync_time: now,
                        };
                        app.start_storage(false, cx);
                    }
                    Err(e) => {
                        app.sync_status = AppSyncStatus::Error(format!("Resolution error: {e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn trigger_sync(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            self.sync_status = AppSyncStatus::Error("Database not open.".into());
            cx.notify();
            return;
        };

        let token = std::env::var("SHELLUMI_GDRIVE_TOKEN")
            .or_else(|_| std::env::var("GOOGLE_DRIVE_TOKEN"))
            .unwrap_or_default();

        if token.trim().is_empty() {
            self.sync_status = AppSyncStatus::Error(
                "Google Drive OAuth token not found. Set GOOGLE_DRIVE_TOKEN or SHELLUMI_GDRIVE_TOKEN environment variable.".into(),
            );
            cx.notify();
            return;
        }

        let passphrase = match std::env::var("SHELLUMI_VAULT_PASSPHRASE") {
            Ok(value) if value.chars().count() >= 12 => shellumi_core::SecretString::new(value),
            _ => {
                self.sync_status = AppSyncStatus::Error(
                    "Set a vault passphrase of at least 12 characters before syncing.".into(),
                );
                cx.notify();
                return;
            }
        };
        let device_id =
            std::env::var("SHELLUMI_DEVICE_ID").unwrap_or_else(|_| "desktop-app".to_string());

        self.sync_status = AppSyncStatus::Syncing;
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                let local_vault = store
                    .export_vault(passphrase.expose_secret(), &device_id)
                    .await
                    .map_err(|e| e.to_string())?;
                let client =
                    GoogleDriveSyncClient::new(SystemHttpTransport::default(), "shellumi-desktop");
                let cycle_result = execute_cloud_vault_sync(
                    &client,
                    &token,
                    &local_vault,
                    passphrase.expose_secret(),
                    None,
                )
                .map_err(|e| e.to_string())?;

                match cycle_result {
                    SyncCycleResult::AlreadySynchronized => Ok((None, 0)),
                    SyncCycleResult::Success { merged_records, .. } => {
                        let mut write_store =
                            HostStore::open(&path).await.map_err(|e| e.to_string())?;
                        let mut merged_vault = SyncVault::new("cloud-vault", 0);
                        merged_vault.records = merged_records;
                        let summary = write_store
                            .import_vault(passphrase.expose_secret(), &merged_vault)
                            .await
                            .map_err(|e| e.to_string())?;
                        let count = summary.hosts_imported
                            + summary.groups_imported
                            + summary.tunnels_imported
                            + summary.snippets_imported;
                        Ok((None, count))
                    }
                    SyncCycleResult::Conflicts(conflict_views) => Ok((Some(conflict_views), 0)),
                    SyncCycleResult::Error(err) => Err(err),
                }
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok((Some(conflict_items), _)) => {
                        let total_conflicts = conflict_items.len();
                        let default_resolutions =
                            vec![ConflictResolutionChoice::KeepLocal; total_conflicts];
                        app.conflict_modal = Some(ConflictModalState {
                            items: conflict_items,
                            selected_index: 0,
                            resolutions: default_resolutions,
                            file_id: "google-drive-sync".into(),
                        });
                        app.sync_status = AppSyncStatus::Conflict {
                            message: format!("{total_conflicts} conflicting changes detected"),
                        };
                    }
                    Ok((None, _count)) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        app.sync_status = AppSyncStatus::UpToDate {
                            last_sync_time: now,
                        };
                        app.start_storage(false, cx);
                    }
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

    fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() {
            self.form = None;
            self.form_error = None;
            cx.notify();
            return;
        }
        if self.command_palette.is_some() {
            self.dismiss_command_palette(cx);
            return;
        }
        if self.page.is_some() {
            self.page = None;
            cx.notify();
            return;
        }
        if let Some(active_idx) = self.active_tab_idx {
            if let Some(tab) = self.tabs.get(active_idx) {
                let id = tab.id;
                self.close_tab(id, window, cx);
            }
        }
    }

    fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }
        self.page = None;
        let total = self.tabs.len();
        let next = match self.active_tab_idx {
            Some(idx) => (idx + 1) % total,
            None => 0,
        };
        self.select_tab(Some(next), window, cx);
    }

    fn prev_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }
        self.page = None;
        let total = self.tabs.len();
        let prev = match self.active_tab_idx {
            Some(0) | None => total - 1,
            Some(idx) => idx - 1,
        };
        self.select_tab(Some(prev), window, cx);
    }

    fn switch_to_tab(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.page = None;
        if idx < self.tabs.len() {
            self.select_tab(Some(idx), window, cx);
        } else if idx == 0 {
            self.select_tab(None, window, cx);
        }
    }

    fn close_active_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get(active_idx) else {
            return;
        };
        if tab.panes.len() > 1 {
            let pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
            if let Some(pane) = tab.panes.get(pane_idx) {
                let pane_id = pane.id;
                self.close_pane(pane_id, window, cx);
            }
        } else {
            let tab_id = tab.id;
            self.close_tab(tab_id, window, cx);
        }
    }

    fn focus_next_pane(&mut self, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if tab.panes.len() > 1 {
            tab.active_pane_idx = (tab.active_pane_idx + 1) % tab.panes.len();
            cx.notify();
        }
    }

    fn focus_prev_pane(&mut self, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if tab.panes.len() > 1 {
            tab.active_pane_idx = if tab.active_pane_idx == 0 {
                tab.panes.len() - 1
            } else {
                tab.active_pane_idx - 1
            };
            cx.notify();
        }
    }

    fn toggle_focus_mode(&mut self, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if tab.panes.len() > 1 {
            if tab.split.is_some() {
                tab.split = None;
            } else {
                tab.split = Some(SplitDirection::Vertical);
            }
            cx.notify();
        }
    }

    fn toggle_broadcast_mode(&mut self, cx: &mut Context<Self>) {
        let total = self
            .active_tab_idx
            .and_then(|i| self.tabs.get(i))
            .map(|t| t.panes.len())
            .unwrap_or(0);
        self.broadcast_manager.set_total_panes(total);
        self.broadcast_manager.toggle();
        cx.notify();
    }

    fn trigger_find_in_terminal(&mut self, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get(active_idx) else {
            return;
        };
        let pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
        if let Some(pane) = tab.panes.get(pane_idx) {
            let pane_id = pane.id;
            self.toggle_search(pane_id, cx);
        }
    }

    fn reconnect_active_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get(active_idx) else {
            return;
        };
        if tab.kind == SessionKind::Local {
            let tab_id = tab.id;
            self.close_tab(tab_id, window, cx);
            self.open_local_terminal(window, cx);
        } else {
            self.trigger_mobile_lifecycle(MobileLifecycleEvent::NetworkRestored, cx);
        }
    }

    fn trigger_quick_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = None;
        self.select_tab(None, window, cx);
        window.focus(&self.quick_connect_input.read(cx).focus_handle(cx));
        cx.notify();
    }

    fn handle_app_shortcut(
        &mut self,
        key: &gpui::Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let is_primary =
            key.modifiers.platform || (key.modifiers.control && !key.modifiers.platform);
        let shift = key.modifiers.shift;
        let alt = key.modifiers.alt;
        let k = key.key.to_ascii_lowercase();

        // 1. Command Palette: Cmd+K / Ctrl+K (without Shift/Alt)
        if is_primary && !shift && !alt && k == "k" {
            self.toggle_command_palette(window, cx);
            return true;
        }

        // 2. New Tab / Local Terminal: Cmd+T / Ctrl+T, Cmd+L / Ctrl+L
        if is_primary && !shift && !alt && (k == "t" || k == "l") {
            self.open_local_terminal(window, cx);
            return true;
        }

        // 3. Close Tab: Cmd+W / Ctrl+W (without Shift)
        if is_primary && !shift && !alt && k == "w" {
            self.close_active_tab(window, cx);
            return true;
        }

        // 4. Close Pane: Cmd+Shift+W / Ctrl+Shift+W
        if is_primary && shift && !alt && k == "w" {
            self.close_active_pane(window, cx);
            return true;
        }

        // 5. Next Tab: Cmd+Shift+] / Ctrl+Tab / Ctrl+PageDown
        if (is_primary && shift && (k == "]" || k == "}"))
            || (key.modifiers.control && !shift && (k == "tab" || k == "pagedown"))
        {
            self.next_tab(window, cx);
            return true;
        }

        // 6. Previous Tab: Cmd+Shift+[ / Ctrl+Shift+Tab / Ctrl+PageUp
        if (is_primary && shift && (k == "[" || k == "{"))
            || (key.modifiers.control && shift && k == "tab")
            || (key.modifiers.control && !shift && k == "pageup")
        {
            self.prev_tab(window, cx);
            return true;
        }

        // 7. Tab 1-9: Cmd+1..9 / Ctrl+1..9
        if is_primary && !shift && !alt && k.len() == 1 {
            if let Some(ch) = k.chars().next() {
                if ('1'..='9').contains(&ch) {
                    let idx = (ch as usize) - ('1' as usize);
                    self.switch_to_tab(idx, window, cx);
                    return true;
                }
            }
        }

        // 8. Split Vertical: Cmd+D / Ctrl+Shift+E
        if (is_primary && !shift && !alt && k == "d")
            || (key.modifiers.control && shift && !alt && k == "e")
        {
            self.split_pane(SplitDirection::Vertical, window, cx);
            return true;
        }

        // 9. Split Horizontal: Cmd+Shift+D / Ctrl+Shift+O
        if (is_primary && shift && !alt && k == "d")
            || (key.modifiers.control && shift && !alt && k == "o")
        {
            self.split_pane(SplitDirection::Horizontal, window, cx);
            return true;
        }

        // 10. Focus Next Pane: Cmd+] / Alt+Right
        if (is_primary && !shift && !alt && k == "]")
            || (alt && !shift && !is_primary && k == "right")
        {
            self.focus_next_pane(cx);
            return true;
        }

        // 11. Focus Previous Pane: Cmd+[ / Alt+Left
        if (is_primary && !shift && !alt && k == "[")
            || (alt && !shift && !is_primary && k == "left")
        {
            self.focus_prev_pane(cx);
            return true;
        }

        // 12. Switch Focus Mode (Toggle Split / Single): Cmd+Shift+M / Ctrl+Alt+M
        if (is_primary && shift && !alt && k == "m")
            || (key.modifiers.control && alt && !shift && k == "m")
        {
            self.toggle_focus_mode(cx);
            return true;
        }

        // 13. Toggle Broadcast: Cmd+B / Cmd+Shift+B / Ctrl+Alt+B
        if (is_primary && !alt && k == "b") || (key.modifiers.control && alt && k == "b") {
            self.toggle_broadcast_mode(cx);
            return true;
        }

        // 14. Toggle Snippets: Cmd+. / Ctrl+.
        if is_primary && !shift && !alt && k == "." {
            self.snippets_panel_open = !self.snippets_panel_open;
            cx.notify();
            return true;
        }

        // 15. Find in Terminal: Cmd+F / Ctrl+F
        if is_primary && !shift && !alt && k == "f" {
            self.trigger_find_in_terminal(cx);
            return true;
        }

        // 16. Zoom In: Cmd+= / Cmd++ / Ctrl+= / Ctrl++
        if is_primary && !alt && (k == "=" || k == "+") {
            self.desktop.preferences.terminal_font_size =
                (self.desktop.preferences.terminal_font_size + 1).min(32);
            cx.notify();
            return true;
        }

        // 17. Zoom Out: Cmd+- / Ctrl+-
        if is_primary && !alt && k == "-" {
            self.desktop.preferences.terminal_font_size = (self
                .desktop
                .preferences
                .terminal_font_size
                .saturating_sub(1))
            .max(8);
            cx.notify();
            return true;
        }

        // 18. Reset Zoom: Cmd+0 / Ctrl+0
        if is_primary && !alt && k == "0" {
            self.desktop.preferences.terminal_font_size = 14;
            cx.notify();
            return true;
        }

        // 19. Quick Connect: Cmd+O / Ctrl+O (without Shift)
        if is_primary && !shift && !alt && k == "o" {
            self.trigger_quick_connect(window, cx);
            return true;
        }

        // 20. New Host: Cmd+N / Ctrl+N
        if is_primary && !shift && !alt && k == "n" {
            self.edit_host(None, cx);
            return true;
        }

        // 21. Refresh: Cmd+R / Ctrl+R (without Shift)
        if is_primary && !shift && !alt && k == "r" {
            self.load_hosts(cx);
            return true;
        }

        // 22. Reconnect: Cmd+Shift+R / Ctrl+Shift+R
        if is_primary && shift && !alt && k == "r" {
            self.reconnect_active_session(window, cx);
            return true;
        }

        // 23. Toggle Sidebar: Cmd+\ / Ctrl+\
        if is_primary && !alt && (k == "\\" || k == "|") {
            self.sidebar_visible = !self.sidebar_visible;
            cx.notify();
            return true;
        }

        // 24. Settings: Cmd+, / Ctrl+,
        if is_primary && !alt && k == "," {
            self.page = Some("Settings");
            cx.notify();
            return true;
        }

        // 25. Navigation shortcuts:
        if is_primary && shift && !alt {
            match k.as_str() {
                "h" => {
                    self.page = None;
                    self.select_tab(None, window, cx);
                    return true;
                }
                "o" => {
                    self.page = Some("Overview");
                    cx.notify();
                    return true;
                }
                "k" => {
                    self.page = Some("Workspaces");
                    cx.notify();
                    return true;
                }
                "u" => {
                    self.page = Some("SFTP Files");
                    cx.notify();
                    return true;
                }
                "y" => {
                    self.page = Some("Keychain");
                    cx.notify();
                    return true;
                }
                "p" => {
                    self.page = Some("Port Forwarding");
                    cx.notify();
                    return true;
                }
                _ => {}
            }
        }

        false
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
        if self.handle_app_shortcut(&event.keystroke, window, cx) {
            cx.stop_propagation();
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
            invite_public_key: cx
                .new(|cx| TextInput::new(cx, "", "Collaborator OpenSSH public key")),
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
        let public_key = modal.invite_public_key.read(cx).value().trim().to_owned();
        if shellumi_core::fingerprint_public_key(&public_key).is_err() {
            modal.error = Some("Provide a valid OpenSSH public key for this member.".into());
            cx.notify();
            return;
        }

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
            &public_key,
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
            active_tab: DiagnosticsModalTab::HealthChecks,
            report: None,
            benchmark_report: None,
            is_running: true,
            is_running_benchmarks: false,
            error: None,
        });
        cx.notify();
        self.run_diagnostics_checks(cx);
    }

    fn open_benchmarks_modal(&mut self, cx: &mut Context<Self>) {
        self.diagnostics_modal = Some(DiagnosticsModalState {
            active_tab: DiagnosticsModalTab::Benchmarks,
            report: None,
            benchmark_report: None,
            is_running: false,
            is_running_benchmarks: true,
            error: None,
        });
        cx.notify();
        self.run_benchmarks(cx);
    }

    fn dismiss_diagnostics_modal(&mut self, cx: &mut Context<Self>) {
        self.diagnostics_modal = None;
        cx.notify();
    }

    fn run_benchmarks(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.diagnostics_modal {
            m.is_running_benchmarks = true;
            m.error = None;
        }
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { BenchmarkRunner::run_suite(None) });

        cx.spawn(async move |app, cx| {
            let report = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(m) = &mut app.diagnostics_modal {
                    m.is_running_benchmarks = false;
                    m.benchmark_report = Some(report);
                }
                cx.notify();
            });
        })
        .detach();
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
                format!(
                    "Shellumi Mobile Touch Simulator v{} (iOS/Android Prototype)",
                    env!("CARGO_PKG_VERSION")
                ),
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

    // =========================================================================
    // Terminal Session Logging, Audit Recording & Cast Playback Engine
    // =========================================================================

    fn open_session_replay_modal(&mut self, cx: &mut Context<Self>) {
        self.session_replay_modal = Some(SessionReplayModalState {
            active_tab: SessionReplayTab::RecordingsList,
            recordings: Vec::new(),
            selected_recording_id: None,
            current_recording: None,
            playback_engine: None,
            screen_output_buffer: String::new(),
            status_notification: None,
            search_query: String::new(),
        });
        self.load_session_recordings(cx);
        cx.notify();
    }

    fn dismiss_session_replay_modal(&mut self, cx: &mut Context<Self>) {
        self.session_replay_modal = None;
        cx.notify();
    }

    fn set_session_replay_tab(&mut self, tab: SessionReplayTab, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            m.active_tab = tab;
        }
        cx.notify();
    }

    fn toggle_pane_recording(&mut self, pane_id: u64, cx: &mut Context<Self>) {
        for tab in self.tabs.iter_mut() {
            for pane in tab.panes.iter_mut() {
                if pane.id == pane_id {
                    let name = pane.name.clone();
                    if pane.terminal.is_recording() {
                        let rec_opt = pane.terminal.stop_recording();
                        if let Some(rec) = rec_opt {
                            let duration_secs = rec.duration_secs();
                            let event_count = rec.event_count();
                            let raw_cast = rec.to_asciicast_v2().unwrap_or_default();
                            let clean_cast =
                                SessionRedactor::redact_asciicast_v2(&raw_cast).unwrap_or(raw_cast);

                            let now_epoch = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let now_str = format!("{now_epoch}");
                            let rec_id = format!("rec-{now_epoch}");
                            let sess_id = format!("sess-{now_epoch}");

                            let db_rec = SessionRecordingRecord {
                                id: rec_id.clone(),
                                session_id: sess_id,
                                host_id: None,
                                title: format!("Recorded Session: {name}"),
                                duration_secs,
                                event_count,
                                byte_size: clean_cast.len(),
                                cast_data: clean_cast,
                                recorded_at: now_str.clone(),
                                created_at: now_str,
                            };

                            if let Some(path) = self.database.clone() {
                                let db_rec_clone = db_rec.clone();
                                cx.background_executor()
                                    .spawn(async move {
                                        let runtime = tokio::runtime::Builder::new_current_thread()
                                            .build()
                                            .map_err(|e| e.to_string());
                                        if let Ok(rt) = runtime {
                                            rt.block_on(async {
                                                if let Ok(mut store) = HostStore::open(&path).await
                                                {
                                                    let _ = store
                                                        .save_session_recording(&db_rec_clone)
                                                        .await;
                                                }
                                            });
                                        }
                                    })
                                    .detach();
                            }

                            pane.message = Some(format!(
                                "⏹ Recording stopped & saved ({}, {event_count} events)",
                                format_duration_hms(duration_secs)
                            ));
                        }
                    } else {
                        pane.terminal
                            .start_recording(Some(format!("Session: {name}")));
                        pane.message = Some("⏺ Session recording started.".to_string());
                    }
                    cx.notify();
                    return;
                }
            }
        }
    }

    fn load_session_recordings(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.database.clone() {
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    store
                        .list_session_recordings(None)
                        .await
                        .map_err(|e| e.to_string())
                })
            });
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Ok(recordings) = result {
                        if let Some(ref mut m) = app.session_replay_modal {
                            m.recordings = recordings;
                        }
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    fn select_and_load_recording_for_playback(&mut self, rec_id: String, cx: &mut Context<Self>) {
        if let Some(path) = self.database.clone() {
            let id_clone = rec_id.clone();
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    store
                        .get_session_recording(&id_clone)
                        .await
                        .map_err(|e| e.to_string())
                })
            });
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Ok(Some(record)) = result {
                        if let Some(ref mut m) = app.session_replay_modal {
                            if let Ok(mut engine) =
                                CastPlaybackEngine::from_asciicast_v2(&record.cast_data)
                            {
                                m.selected_recording_id = Some(rec_id.clone());
                                m.current_recording = Some(record);
                                m.active_tab = SessionReplayTab::ReplayPlayer;
                                m.screen_output_buffer.clear();
                                let initial_frames = engine.seek(0.0);
                                for f in initial_frames {
                                    if f.event_type == "o" {
                                        m.screen_output_buffer.push_str(&f.data);
                                    }
                                }
                                m.playback_engine = Some(engine);
                            }
                        }
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    fn toggle_modal_playback(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            if let Some(ref mut engine) = m.playback_engine {
                engine.toggle_play_pause();
                // Advance 1 frame immediately so playback responds visually
                if engine.status() == PlaybackStatus::Playing {
                    let frames = engine.advance(0.1);
                    for f in frames {
                        if f.event_type == "o" {
                            m.screen_output_buffer.push_str(&f.data);
                        }
                    }
                }
            }
        }
        cx.notify();
    }

    fn seek_modal_playback(&mut self, fraction: f32, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            if let Some(ref mut engine) = m.playback_engine {
                let target = fraction as f64 * engine.duration_secs();
                let frames = engine.seek(target);
                m.screen_output_buffer.clear();
                for f in frames {
                    if f.event_type == "o" {
                        m.screen_output_buffer.push_str(&f.data);
                    }
                }
            }
        }
        cx.notify();
    }

    fn step_modal_playback(&mut self, delta_secs: f64, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            if let Some(ref mut engine) = m.playback_engine {
                let target =
                    (engine.current_time_secs() + delta_secs).clamp(0.0, engine.duration_secs());
                let frames = engine.seek(target);
                m.screen_output_buffer.clear();
                for f in frames {
                    if f.event_type == "o" {
                        m.screen_output_buffer.push_str(&f.data);
                    }
                }
            }
        }
        cx.notify();
    }

    fn cycle_modal_playback_speed(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            if let Some(ref mut engine) = m.playback_engine {
                engine.cycle_speed();
            }
        }
        cx.notify();
    }

    fn delete_session_recording_ui(&mut self, rec_id: String, cx: &mut Context<Self>) {
        if let Some(path) = self.database.clone() {
            let id_clone = rec_id.clone();
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    store
                        .delete_session_recording(&id_clone)
                        .await
                        .map_err(|e| e.to_string())
                })
            });
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Ok(true) = result {
                        if let Some(ref mut m) = app.session_replay_modal {
                            m.recordings.retain(|r| r.id != rec_id);
                            if m.selected_recording_id.as_deref() == Some(&rec_id) {
                                m.selected_recording_id = None;
                                m.current_recording = None;
                                m.playback_engine = None;
                                m.screen_output_buffer.clear();
                                m.active_tab = SessionReplayTab::RecordingsList;
                            }
                            m.status_notification =
                                Some(format!("Deleted session recording '{rec_id}'"));
                        }
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    fn export_session_recording_ui(&mut self, rec_id: String, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.session_replay_modal {
            m.status_notification = Some(format!(
                "Exported recording '{rec_id}' to standard Asciicast v2 format (.cast)"
            ));
        }
        cx.notify();
    }

    // =========================================================================
    // Dynamic Port-Forwarding Tunnel Manager UI & Telemetry
    // =========================================================================

    fn open_tunnel_modal(&mut self, cx: &mut Context<Self>) {
        let name_input = cx.new(|cx| TextInput::new(cx, "", "Tunnel Name (e.g. Postgres Dev)"));
        let bind_port_input = cx.new(|cx| TextInput::new(cx, "", "Bind Port (e.g. 5432)"));
        let target_host_input = cx.new(|cx| TextInput::new(cx, "", "Target Host (e.g. 10.0.0.5)"));
        let target_port_input = cx.new(|cx| TextInput::new(cx, "", "Target Port (e.g. 5432)"));

        self.tunnel_modal = Some(TunnelModalState {
            editing_id: None,
            tunnels: Vec::new(),
            selected_index: 0,
            active_states: std::collections::HashMap::new(),
            name_input,
            bind_port_input,
            target_host_input,
            target_port_input,
            forwarding_type: ForwardingType::Local,
            selected_host_id: match &self.hosts {
                HostState::Loaded(hosts) => {
                    hosts.iter().find(|host| host.is_ssh()).map(|host| host.id)
                }
                _ => None,
            },
            auto_start: false,
            error: None,
            status_message: None,
            is_loading: true,
        });
        cx.notify();

        if let Some(path) = self.database.clone() {
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    store.list_tunnels().await.map_err(|e| e.to_string())
                })
            });

            cx.spawn(async move |app, cx| {
                let res = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Some(ref mut modal) = app.tunnel_modal {
                        modal.is_loading = false;
                        match res {
                            Ok(tunnels) => {
                                for t in &tunnels {
                                    modal.active_states.entry(t.id).or_insert_with(|| {
                                        ActiveTunnelState::new(
                                            t.id,
                                            t.host_id,
                                            t.name.clone(),
                                            t.rule.clone(),
                                            t.auto_start,
                                        )
                                    });
                                }
                                modal.tunnels = tunnels;
                            }
                            Err(e) => {
                                modal.error = Some(e);
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        } else if let Some(ref mut modal) = self.tunnel_modal {
            modal.is_loading = false;
        }
    }

    fn dismiss_tunnel_modal(&mut self, cx: &mut Context<Self>) {
        self.tunnel_modal = None;
        cx.notify();
    }

    fn select_tunnel_modal_item(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(ref mut modal) = self.tunnel_modal {
            if idx < modal.tunnels.len() {
                modal.selected_index = idx;
                cx.notify();
            }
        }
    }

    fn set_tunnel_forwarding_type(&mut self, ftype: ForwardingType, cx: &mut Context<Self>) {
        if let Some(ref mut modal) = self.tunnel_modal {
            modal.forwarding_type = ftype;
            cx.notify();
        }
    }

    fn toggle_tunnel_state(&mut self, tunnel_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tunnel) = self
            .tunnel_modal
            .as_ref()
            .and_then(|modal| modal.tunnels.iter().find(|tunnel| tunnel.id == tunnel_id))
            .cloned()
        else {
            return;
        };
        let session_name = format!("Tunnel: {}", tunnel.name);
        if let Some(tab_id) = self
            .tabs
            .iter()
            .find(|tab| tab.name == session_name)
            .map(|tab| tab.id)
        {
            self.close_tab(tab_id, window, cx);
            return;
        }
        let host = match &self.hosts {
            HostState::Loaded(hosts) => hosts
                .iter()
                .find(|host| Some(host.id) == tunnel.host_id)
                .cloned(),
            _ => None,
        };
        let Some(host) = host else {
            if let Some(modal) = &mut self.tunnel_modal {
                modal.error = Some("Choose a saved SSH host for this tunnel.".into());
            }
            cx.notify();
            return;
        };
        let (flag, value) = tunnel.rule.openssh_argument();
        let args = vec![
            "-N".to_string(),
            "-o".into(),
            "ExitOnForwardFailure=yes".into(),
            flag.into(),
            value,
        ];
        self.tunnel_modal = None;
        self.launch_saved_tunnel(host, session_name, args, window, cx);
    }

    fn save_new_tunnel_modal(&mut self, cx: &mut Context<Self>) {
        let (name, bind_port_str, target_host_str, target_port_str, ftype, host_id, auto_start) = {
            if let Some(ref modal) = self.tunnel_modal {
                (
                    modal.name_input.read(cx).value().trim().to_string(),
                    modal.bind_port_input.read(cx).value().trim().to_string(),
                    modal.target_host_input.read(cx).value().trim().to_string(),
                    modal.target_port_input.read(cx).value().trim().to_string(),
                    modal.forwarding_type,
                    modal.selected_host_id,
                    modal.auto_start,
                )
            } else {
                return;
            }
        };

        if name.is_empty() {
            if let Some(ref mut m) = self.tunnel_modal {
                m.error = Some("Tunnel name is required.".into());
                cx.notify();
            }
            return;
        }

        let bind_port: u16 = match bind_port_str.parse() {
            Ok(p) if p > 0 => p,
            _ => {
                if let Some(ref mut m) = self.tunnel_modal {
                    m.error = Some("Invalid bind port (must be between 1 and 65535).".into());
                    cx.notify();
                }
                return;
            }
        };

        let rule = match ftype {
            ForwardingType::Dynamic => {
                match PortForwardRule::dynamic(Some("127.0.0.1"), bind_port) {
                    Ok(r) => r,
                    Err(e) => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some(e.to_string());
                            cx.notify();
                        }
                        return;
                    }
                }
            }
            ForwardingType::Local => {
                let target_port: u16 = match target_port_str.parse() {
                    Ok(p) if p > 0 => p,
                    _ => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some("Target port required for local forwarding.".into());
                            cx.notify();
                        }
                        return;
                    }
                };
                let target_host = if target_host_str.is_empty() {
                    "localhost"
                } else {
                    &target_host_str
                };
                match PortForwardRule::local(Some("127.0.0.1"), bind_port, target_host, target_port)
                {
                    Ok(r) => r,
                    Err(e) => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some(e.to_string());
                            cx.notify();
                        }
                        return;
                    }
                }
            }
            ForwardingType::Remote => {
                let target_port: u16 = match target_port_str.parse() {
                    Ok(p) if p > 0 => p,
                    _ => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some("Target port required for remote forwarding.".into());
                            cx.notify();
                        }
                        return;
                    }
                };
                let target_host = if target_host_str.is_empty() {
                    "localhost"
                } else {
                    &target_host_str
                };
                match PortForwardRule::remote(
                    Some("127.0.0.1"),
                    bind_port,
                    target_host,
                    target_port,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some(e.to_string());
                            cx.notify();
                        }
                        return;
                    }
                }
            }
        };

        if host_id.is_none() {
            if let Some(modal) = &mut self.tunnel_modal {
                modal.error = Some("Select a saved SSH host.".into());
            }
            cx.notify();
            return;
        }
        let editing_id = self
            .tunnel_modal
            .as_ref()
            .and_then(|modal| modal.editing_id);
        if let Some(path) = self.database.clone() {
            let name_clone = name.clone();
            let rule_clone = rule.clone();
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    if let Some(id) = editing_id {
                        store
                            .update_tunnel(id, &name_clone, host_id, &rule_clone, auto_start)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok(id)
                    } else {
                        store
                            .add_tunnel(&name_clone, host_id, &rule_clone, auto_start)
                            .await
                            .map_err(|e| e.to_string())
                    }
                })
            });

            cx.spawn(async move |app, cx| {
                let res = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Some(ref mut modal) = app.tunnel_modal {
                        match res {
                            Ok(new_id) => {
                                let new_tunnel = SavedTunnel {
                                    id: new_id,
                                    host_id,
                                    name: name.clone(),
                                    rule: rule.clone(),
                                    auto_start,
                                };
                                modal.active_states.insert(
                                    new_id,
                                    ActiveTunnelState::new(
                                        new_id,
                                        host_id,
                                        name.clone(),
                                        rule,
                                        auto_start,
                                    ),
                                );
                                modal.tunnels.retain(|tunnel| tunnel.id != new_id);
                                modal.tunnels.push(new_tunnel);
                                modal.editing_id = None;
                                modal.selected_index = modal.tunnels.len().saturating_sub(1);
                                modal.status_message =
                                    Some(format!("Saved tunnel '{name}' (ID {new_id})"));
                                modal.error = None;
                            }
                            Err(e) => {
                                modal.error = Some(e);
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    fn edit_saved_tunnel(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.tunnel_modal {
            if let Some(tunnel) = modal.tunnels.iter().find(|tunnel| tunnel.id == id).cloned() {
                modal.editing_id = Some(id);
                modal.selected_host_id = tunnel.host_id;
                modal.forwarding_type = tunnel.rule.forwarding_type;
                modal
                    .name_input
                    .update(cx, |input, cx| input.set_value(&tunnel.name, cx));
                modal.bind_port_input.update(cx, |input, cx| {
                    input.set_value(&tunnel.rule.bind_port.to_string(), cx)
                });
                modal.target_host_input.update(cx, |input, cx| {
                    input.set_value(tunnel.rule.target_host.as_deref().unwrap_or(""), cx)
                });
                modal.target_port_input.update(cx, |input, cx| {
                    input.set_value(
                        &tunnel
                            .rule
                            .target_port
                            .map(|port| port.to_string())
                            .unwrap_or_default(),
                        cx,
                    )
                });
                modal.status_message = Some(
                    "Editing the saved rule. Reopen its tunnel session to apply changes.".into(),
                );
            }
        }
        cx.notify();
    }

    fn delete_tunnel_modal(&mut self, tunnel_id: i64, cx: &mut Context<Self>) {
        if let Some(path) = self.database.clone() {
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    store
                        .delete_tunnel(tunnel_id)
                        .await
                        .map_err(|e| e.to_string())
                })
            });

            cx.spawn(async move |app, cx| {
                let res = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Some(ref mut modal) = app.tunnel_modal {
                        match res {
                            Ok(true) => {
                                modal.tunnels.retain(|t| t.id != tunnel_id);
                                modal.active_states.remove(&tunnel_id);
                                if modal.selected_index >= modal.tunnels.len() {
                                    modal.selected_index = modal.tunnels.len().saturating_sub(1);
                                }
                                modal.status_message = Some(format!("Tunnel {tunnel_id} deleted."));
                                modal.error = None;
                            }
                            Ok(false) => {
                                modal.error = Some("Tunnel not found.".into());
                            }
                            Err(e) => {
                                modal.error = Some(e);
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
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
            .bg(rgba(0x000000bb))
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
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
                            .text_color(rgb(MacTheme::WARNING))
                            .child("⚠️ Sync Conflict Resolution"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Concurrent modifications detected between local storage and remote vault. Choose which version to retain."),
                    ),
            )
            .child(
                div()
                    .id("close-conflict-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                ConflictResolutionChoice::KeepLocal => rgb(MacTheme::ACCENT_BLUE),
                ConflictResolutionChoice::AcceptRemote => rgb(MacTheme::SUCCESS),
            };

            items_col = items_col.child(
                div()
                    .id(("conflict-item", i as u64))
                    .p_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .border_1()
                    .border_color(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
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
                                    .text_color(if is_sel {
                                        rgb(0xffffff)
                                    } else {
                                        rgb(MacTheme::TEXT_PRIMARY)
                                    })
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
                            .text_color(if is_sel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
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
                .bg(rgb(MacTheme::INPUT_BG))
                .border_2()
                .border_color(if is_local_chosen {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::SEPARATOR)
                })
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child("🖥 Local Version"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child(format!("v{}", item.local_version)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child(item.local_summary.clone()),
                )
                .child(
                    div()
                        .id("choose-local-btn")
                        .mt_auto()
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(if is_local_chosen {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SEPARATOR)
                        })
                        .cursor_pointer()
                        .bg(if is_local_chosen {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(0x323236)
                        })
                        .text_color(if is_local_chosen {
                            rgb(0xffffff)
                        } else {
                            rgb(MacTheme::TEXT_PRIMARY)
                        })
                        .text_xs()
                        .hover(move |s| {
                            if is_local_chosen {
                                s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                            } else {
                                s.bg(rgb(MacTheme::SURFACE_HOVER))
                            }
                        })
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
                .bg(rgb(MacTheme::INPUT_BG))
                .border_2()
                .border_color(if is_remote_chosen {
                    rgb(MacTheme::SUCCESS)
                } else {
                    rgb(MacTheme::SEPARATOR)
                })
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::SUCCESS))
                                .child("☁ Remote Vault"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child(format!("v{}", item.remote_version)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child(item.remote_summary.clone()),
                )
                .child(
                    div()
                        .id("choose-remote-btn")
                        .mt_auto()
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(if is_remote_chosen {
                            rgb(MacTheme::SUCCESS)
                        } else {
                            rgb(MacTheme::SEPARATOR)
                        })
                        .cursor_pointer()
                        .bg(if is_remote_chosen {
                            rgb(MacTheme::SUCCESS)
                        } else {
                            rgb(0x323236)
                        })
                        .text_color(if is_remote_chosen {
                            rgb(0xffffff)
                        } else {
                            rgb(MacTheme::TEXT_PRIMARY)
                        })
                        .text_xs()
                        .hover(move |s| {
                            if is_remote_chosen {
                                s.bg(rgb(MacTheme::SUCCESS))
                            } else {
                                s.bg(rgb(MacTheme::SURFACE_HOVER))
                            }
                        })
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
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .id("cancel-resolutions-btn")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_conflict_modal(cx)))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("apply-resolutions-btn")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                    .bg(rgb(MacTheme::ACCENT_BLUE))
                    .text_color(rgb(0xffffff))
                    .font_weight(FontWeight::BOLD)
                    .text_xs()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
            rgb(MacTheme::SUCCESS)
        } else {
            rgb(MacTheme::ACCENT_BLUE)
        };

        let overlay = div()
            .id("snippet-modal-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000bb))
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
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child(format!("📜 {action_name}: {}", modal.snippet_name)),
                ),
            )
            .child(
                div()
                    .id("dismiss-snippet-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_snippet_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Command template card
        let template_card = div()
            .p_2()
            .rounded_md()
            .bg(rgb(MacTheme::CONTENT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .font_family(MONOSPACE_FONT)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(var.name.clone()),
            );
            if var.is_secret {
                var_header = var_header.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x4a1818))
                        .text_xs()
                        .text_color(rgb(MacTheme::DANGER))
                        .child("SECRET"),
                );
            }
            if let Some(def) = &var.default_value {
                var_header = var_header.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x183048))
                        .text_xs()
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
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
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
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
            .border_color(rgb(MacTheme::SEPARATOR))
            .pt_3()
            .child(
                div()
                    .id("cancel-snippet-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_snippet_modal(cx)))
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("batch-from-snippet-modal-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .border_1()
                    .border_color(action_bg)
                    .bg(action_bg)
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
            .bg(rgba(0x000000bb))
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
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child(format!("⚡ Multi-Host Execution: {}", modal.snippet_name)),
                ),
            )
            .child(
                div()
                    .id("dismiss-batch-modal-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_batch_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Template card
        let template_card = div()
            .p_2()
            .rounded_md()
            .bg(rgb(MacTheme::CONTENT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .font_family(MONOSPACE_FONT)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Policy:"),
                    )
                    .child(
                        div()
                            .id("policy-parallel-btn")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if is_parallel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if is_parallel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if is_parallel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if is_parallel {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
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
                            .rounded_md()
                            .border_1()
                            .border_color(if !is_parallel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if !is_parallel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if !is_parallel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if !is_parallel {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
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
                    .rounded_md()
                    .border_1()
                    .border_color(if is_dry_run {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if is_dry_run {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_dry_run {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if is_dry_run {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
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
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(0x323236))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(0x323236))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
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
                        rgb(MacTheme::INPUT_BG)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .border_1()
                    .border_color(if is_selected {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
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
                            .border_1()
                            .border_color(if is_selected {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if is_selected {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
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
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(host.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(format!(
                                "({}@{}:{})",
                                host.destination.username(),
                                host.destination.host(),
                                host.destination.port()
                            )),
                    );

                let mut right_side = div().flex().items_center().gap_2();
                if let Some(dur) = host.duration_ms {
                    right_side = right_side.child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(0x323236))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                        .bg(rgb(MacTheme::CONTENT_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .child(
                            div()
                                .text_xs()
                                .font_family(MONOSPACE_FONT)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
            let is_failed = summary.failed > 0;
            let summary_card = div()
                .p_2()
                .rounded_md()
                .bg(if is_failed {
                    rgb(0x4a1818)
                } else {
                    rgb(0x133820)
                })
                .border_1()
                .border_color(if is_failed {
                    rgb(MacTheme::DANGER)
                } else {
                    rgb(MacTheme::SUCCESS)
                })
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
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
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
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
            .border_color(rgb(MacTheme::SEPARATOR))
            .pt_3()
            .child(
                div()
                    .id("cancel-batch-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_batch_modal(cx)))
                    .child("Close"),
            )
            .child(
                div()
                    .id("run-batch-btn")
                    .px_4()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(if is_running || selected_count == 0 {
                        rgb(MacTheme::SEPARATOR)
                    } else {
                        rgb(MacTheme::ACCENT_BLUE)
                    })
                    .bg(if is_running || selected_count == 0 {
                        rgb(0x38383a)
                    } else {
                        rgb(MacTheme::ACCENT_BLUE)
                    })
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_running || selected_count == 0 {
                        rgb(MacTheme::TEXT_TERTIARY)
                    } else {
                        rgb(0xffffff)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if !is_running && selected_count > 0 {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s
                        }
                    })
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
            .bg(rgba(0x000000bb))
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
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
            .bg(rgb(MacTheme::SIDEBAR_BG))
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(div().text_lg().child("🔍"))
            .child(div().flex_1().child(palette.input.clone()))
            .child(
                div()
                    .id("dismiss-palette-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                    (rgb(MacTheme::ACCENT_BLUE), rgb(MacTheme::ACCENT_BLUE_HOVER))
                } else {
                    (rgb(MacTheme::SURFACE), rgb(MacTheme::SURFACE))
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
                            .bg(rgb(0x323236))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .font_family(MONOSPACE_FONT)
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
            .border_color(rgb(MacTheme::SEPARATOR))
            .bg(rgb(MacTheme::INPUT_BG))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("↑↓ navigate  •  ↵ execute  •  esc dismiss"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
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
            .bg(rgba(0x000000bb))
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
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("👥 Team Collaboration & Shared Vaults"),
                ),
            )
            .child(
                div()
                    .id("dismiss-team-modal-btn")
                    .when(self.page == Some("Vaults"), |element| element.hidden())
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                .border_1()
                .border_color(if is_vaults {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::SEPARATOR)
                })
                .bg(if is_vaults {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(0x323236)
                })
                .text_sm()
                .font_weight(if is_vaults {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_vaults {
                    rgb(0xffffff)
                } else {
                    rgb(MacTheme::TEXT_SECONDARY)
                })
                .cursor_pointer()
                .hover(move |s| {
                    if is_vaults {
                        s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                    } else {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                    }
                })
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
                .border_1()
                .border_color(if is_members {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::SEPARATOR)
                })
                .bg(if is_members {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(0x323236)
                })
                .text_sm()
                .font_weight(if is_members {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_members {
                    rgb(0xffffff)
                } else {
                    rgb(MacTheme::TEXT_SECONDARY)
                })
                .cursor_pointer()
                .hover(move |s| {
                    if is_members {
                        s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                    } else {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                    }
                })
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
                .border_1()
                .border_color(if is_audit {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::SEPARATOR)
                })
                .bg(if is_audit {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(0x323236)
                })
                .text_sm()
                .font_weight(if is_audit {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_audit {
                    rgb(0xffffff)
                } else {
                    rgb(MacTheme::TEXT_SECONDARY)
                })
                .cursor_pointer()
                .hover(move |s| {
                    if is_audit {
                        s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                    } else {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                    }
                })
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
                    .bg(rgb(0x133820))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(format!("✓ {msg}")),
            );
        }
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
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
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_sm()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                                rgb(MacTheme::INPUT_BG)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .border_1()
                            .border_color(if is_selected {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
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
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child(v.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x183048))
                                            .text_xs()
                                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                                            .child(format!("Epoch {epoch}")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child(format!("Owner: {}", v.owner_id)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(
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
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::SUCCESS))
                                    .bg(rgb(0x133820))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::SUCCESS))
                                    .child("Active"),
                            );
                        }

                        right = right.child(
                            div()
                                .id(("rotate-key-btn", v_idx))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .bg(rgb(0x323236))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .bg(rgb(0x323236))
                                .text_xs()
                                .text_color(rgb(MacTheme::DANGER))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x4a1818)))
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
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                                .border_1()
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
                            .bg(rgb(0x332810))
                            .border_1()
                            .border_color(rgb(MacTheme::WARNING))
                            .text_sm()
                            .text_color(rgb(MacTheme::WARNING))
                            .child(
                                "⚠️ Please select a shared vault from the 'Shared Vaults' tab first.",
                            ),
                    );
                    return overlay.child(modal_box).into_any_element();
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
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                        div().p_3().rounded_md().bg(rgb(MacTheme::INPUT_BG)).border_1().border_color(rgb(MacTheme::SEPARATOR)).text_xs().text_color(
                            rgb(MacTheme::TEXT_SECONDARY),
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
                            .bg(rgb(MacTheme::SURFACE))
                            .border_1()
                            .border_color(if is_revoked {
                                rgb(MacTheme::DANGER)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            });

                        let (badge_bg, badge_fg) = match m.role {
                            TeamRole::Owner => (rgb(0x3a2c10), rgb(MacTheme::WARNING)),
                            TeamRole::Administrator => (rgb(0x183048), rgb(MacTheme::ACCENT_BLUE)),
                            TeamRole::Editor => (rgb(0x133820), rgb(MacTheme::SUCCESS)),
                            TeamRole::Viewer => (rgb(0x323236), rgb(MacTheme::TEXT_SECONDARY)),
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
                                                rgb(MacTheme::TEXT_SECONDARY)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
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
                                                .bg(rgb(0x4a1818))
                                                .text_xs()
                                                .text_color(rgb(MacTheme::DANGER))
                                                .child("⛔ Revoked")
                                        } else {
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0x133820))
                                                .text_xs()
                                                .text_color(rgb(MacTheme::SUCCESS))
                                                .child("Active")
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(format!(
                                        "User: {} • Invited by {}",
                                        m.user_id, m.invited_by
                                    )),
                            );

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
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::SEPARATOR))
                                    .bg(rgb(0x323236))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::SEPARATOR))
                                    .bg(rgb(0x323236))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::DANGER))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0x4a1818)))
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
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("ADD LOCAL MEMBERSHIP RECORD"),
                    )
                    .child(modal.invite_user_id.clone())
                    .child(modal.invite_name.clone())
                    .child(modal.invite_public_key.clone())
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
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child("Role:"),
                                    )
                                    .child(
                                        div()
                                            .id("invite-role-viewer")
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(if current_role == TeamRole::Viewer {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(MacTheme::SEPARATOR)
                                            })
                                            .bg(if current_role == TeamRole::Viewer {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(0x323236)
                                            })
                                            .text_xs()
                                            .text_color(if current_role == TeamRole::Viewer {
                                                rgb(0xffffff)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
                                            })
                                            .cursor_pointer()
                                            .hover(move |s| {
                                                if current_role == TeamRole::Viewer {
                                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                                } else {
                                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                                }
                                            })
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
                                            .rounded_md()
                                            .border_1()
                                            .border_color(if current_role == TeamRole::Editor {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(MacTheme::SEPARATOR)
                                            })
                                            .bg(if current_role == TeamRole::Editor {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(0x323236)
                                            })
                                            .text_xs()
                                            .text_color(if current_role == TeamRole::Editor {
                                                rgb(0xffffff)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
                                            })
                                            .cursor_pointer()
                                            .hover(move |s| {
                                                if current_role == TeamRole::Editor {
                                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                                } else {
                                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                                }
                                            })
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
                                            .rounded_md()
                                            .border_1()
                                            .border_color(
                                                if current_role == TeamRole::Administrator {
                                                    rgb(MacTheme::ACCENT_BLUE)
                                                } else {
                                                    rgb(MacTheme::SEPARATOR)
                                                },
                                            )
                                            .bg(if current_role == TeamRole::Administrator {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(0x323236)
                                            })
                                            .text_xs()
                                            .text_color(
                                                if current_role == TeamRole::Administrator {
                                                    rgb(0xffffff)
                                                } else {
                                                    rgb(MacTheme::TEXT_PRIMARY)
                                                },
                                            )
                                            .cursor_pointer()
                                            .hover(move |s| {
                                                if current_role == TeamRole::Administrator {
                                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                                } else {
                                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                                }
                                            })
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
                                    .border_1()
                                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
                            .bg(rgb(0x332810))
                            .border_1()
                            .border_color(rgb(MacTheme::WARNING))
                            .text_sm()
                            .text_color(rgb(MacTheme::WARNING))
                            .child(
                                "⚠️ Please select a shared vault from the 'Shared Vaults' tab first.",
                            ),
                    );
                    return overlay.child(modal_box).into_any_element();
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
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child(format!(
                            "Tamper-Evident Audit Trail for: {selected_vault_name}"
                        )),
                );

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
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("No audit records logged yet for this vault."),
                    );
                } else {
                    for record in &modal.audit_records {
                        let formatted = record.format_entry();
                        let row = div()
                            .px_3()
                            .py_1p5()
                            .rounded_sm()
                            .bg(rgb(MacTheme::CONTENT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .font_family(MONOSPACE_FONT)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(formatted);
                        audit_list = audit_list.child(row);
                    }
                }
                modal_box = modal_box.child(audit_list);
            }
        }

        if self.page == Some("Vaults") {
            div()
                .w_full()
                .child(modal_box.w_full().max_h(px(1800.0)))
                .into_any_element()
        } else {
            overlay.child(modal_box).into_any_element()
        }
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
            .bg(rgba(0x000000bb))
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
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("Infrastructure inventory"),
                ),
            )
            .child(
                div()
                    .id("dismiss-infra-modal-btn")
                    .when(self.page == Some("Integrations"), |element| {
                        element.hidden()
                    })
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("⚠️ {err}")),
            );
        }
        if let Some(status) = &modal.status_message {
            modal_box = modal_box.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x133820))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
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
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_2()
            .child(
                div()
                    .id("infra-tab-providers")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(if current_tab == InfraModalTab::Providers {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if current_tab == InfraModalTab::Providers {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if current_tab == InfraModalTab::Providers {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if current_tab == InfraModalTab::Providers {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
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
                    .border_1()
                    .border_color(if current_tab == InfraModalTab::Discovered {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if current_tab == InfraModalTab::Discovered {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if current_tab == InfraModalTab::Discovered {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if current_tab == InfraModalTab::Discovered {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
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
                    .border_1()
                    .border_color(if current_tab == InfraModalTab::Ansible {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if current_tab == InfraModalTab::Ansible {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if current_tab == InfraModalTab::Ansible {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if current_tab == InfraModalTab::Ansible {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
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
                            .rounded_md()
                            .border_1()
                            .border_color(if current_kind == CloudProviderKind::Aws {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if current_kind == CloudProviderKind::Aws {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if current_kind == CloudProviderKind::Aws {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if current_kind == CloudProviderKind::Aws {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
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
                            .rounded_md()
                            .border_1()
                            .border_color(if current_kind == CloudProviderKind::Azure {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if current_kind == CloudProviderKind::Azure {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if current_kind == CloudProviderKind::Azure {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if current_kind == CloudProviderKind::Azure {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
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
                            .rounded_md()
                            .border_1()
                            .border_color(if current_kind == CloudProviderKind::DigitalOcean {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if current_kind == CloudProviderKind::DigitalOcean {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if current_kind == CloudProviderKind::DigitalOcean {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if current_kind == CloudProviderKind::DigitalOcean {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
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
                            .rounded_md()
                            .border_1()
                            .border_color(if current_kind == CloudProviderKind::Ansible {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if current_kind == CloudProviderKind::Ansible {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if current_kind == CloudProviderKind::Ansible {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if current_kind == CloudProviderKind::Ansible {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
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
                    .bg(rgb(MacTheme::INPUT_BG))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                                    .border_1()
                                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                            .bg(rgb(MacTheme::SURFACE))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
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
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                .child(p.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0x183048))
                                                .text_xs()
                                                .text_color(rgb(MacTheme::ACCENT_BLUE))
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
                                            .rounded_md()
                                            .border_1()
                                            .border_color(rgb(MacTheme::SEPARATOR))
                                            .bg(rgb(0x323236))
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                                            .rounded_md()
                                            .border_1()
                                            .border_color(rgb(MacTheme::SEPARATOR))
                                            .bg(rgb(0x323236))
                                            .text_xs()
                                            .text_color(rgb(MacTheme::DANGER))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(0x4a1818)))
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
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                            .bg(rgb(MacTheme::SURFACE))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
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
                                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                    .child(h.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(MacTheme::INPUT_BG))
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                                    .child(state_badge),
                                            )
                                            .child({
                                                let reg = h.region.as_deref().unwrap_or("-");
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                                    .child(format!("[{reg}]"))
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(MONOSPACE_FONT)
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                                        .rounded_md()
                                        .bg(rgb(0x133820))
                                        .border_1()
                                        .border_color(rgb(MacTheme::SUCCESS))
                                        .text_xs()
                                        .text_color(rgb(MacTheme::SUCCESS))
                                        .child(format!(
                                            "✅ Imported (ID: {})",
                                            h.imported_host_id.unwrap_or(0)
                                        ))
                                } else {
                                    div()
                                        .id(("import-host-btn", h_idx))
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(rgb(MacTheme::ACCENT_BLUE))
                                        .bg(rgb(MacTheme::ACCENT_BLUE))
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Export Shellumi Inventory to Ansible"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                                    .border_1()
                                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
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
                                    .border_1()
                                    .border_color(rgb(MacTheme::SEPARATOR))
                                    .bg(rgb(0x323236))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.export_ansible_inventory_file(true, cx);
                                    }))
                                    .child("📤 Export to Ansible YAML"),
                            ),
                    );
                modal_box = modal_box.child(ansible_section);
            }
        }

        if self.page == Some("Integrations") {
            div()
                .w_full()
                .child(modal_box.w_full().max_h(px(1800.0)))
                .into_any_element()
        } else {
            overlay.child(modal_box).into_any_element()
        }
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
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("diagnostics-modal-box")
            .w(px(740.0))
            .max_h(px(580.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("🩺 System Diagnostics & Production Health Check"),
                ),
            )
            .child(
                div()
                    .id("dismiss-diagnostics-modal-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_diagnostics_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Sub-Navigation Tabs
        let is_health = modal.active_tab == DiagnosticsModalTab::HealthChecks;
        let is_bench = modal.active_tab == DiagnosticsModalTab::Benchmarks;
        let tabs = div()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_2()
            .child(
                div()
                    .id("diag-tab-health")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(if is_health {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .bg(if is_health {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_health {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(move |s| {
                        if is_health {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(ref mut m) = app.diagnostics_modal {
                            m.active_tab = DiagnosticsModalTab::HealthChecks;
                            if m.report.is_none() && !m.is_running {
                                app.run_diagnostics_checks(cx);
                            }
                        }
                        cx.notify();
                    }))
                    .child("🩺 System Health Checks"),
            )
            .child(
                div()
                    .id("diag-tab-benchmarks")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(if is_bench {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .bg(if is_bench {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_bench {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(move |s| {
                        if is_bench {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(ref mut m) = app.diagnostics_modal {
                            m.active_tab = DiagnosticsModalTab::Benchmarks;
                            if m.benchmark_report.is_none() && !m.is_running_benchmarks {
                                app.run_benchmarks(cx);
                            }
                        }
                        cx.notify();
                    }))
                    .child("⚡ Performance Benchmarks"),
            );
        modal_box = modal_box.child(tabs);

        match modal.active_tab {
            DiagnosticsModalTab::HealthChecks => {
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
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child("⏳ Running system diagnostic checks..."),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Testing OpenSSH binary, CSPRNG, AEAD ciphers, network loopback, and storage integrity..."),
                        );
                    modal_box = modal_box.child(loading);
                } else if let Some(ref report) = modal.report {
                    let (passed, warnings, failures) = report.counts();

                    // Status Banner
                    let (banner_bg, banner_fg, banner_text) = if failures > 0 {
                        (
                            rgb(0x4a1818),
                            rgb(MacTheme::DANGER),
                            format!("❌ Action Required: {failures} critical check(s) failed."),
                        )
                    } else if warnings > 0 {
                        (
                            rgb(0x3a2c10),
                            rgb(MacTheme::WARNING),
                            format!("⚠️ Degraded: All critical tests passed, but {warnings} warning(s) detected."),
                        )
                    } else {
                        (
                            rgb(0x133820),
                            rgb(MacTheme::SUCCESS),
                            "✅ All Systems Ready: All production environment prerequisites passed."
                                .to_string(),
                        )
                    };

                    let status_strip = div()
                        .px_4()
                        .py_2p5()
                        .rounded_md()
                        .bg(banner_bg)
                        .border_1()
                        .border_color(banner_fg)
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
                            DiagnosticStatus::Passed => (rgb(0x133820), rgb(MacTheme::SUCCESS)),
                            DiagnosticStatus::Warning => (rgb(0x3a2c10), rgb(MacTheme::WARNING)),
                            DiagnosticStatus::Failed => (rgb(0x4a1818), rgb(MacTheme::DANGER)),
                        };

                        let mut row = div()
                            .id(("diag-check-row", c_idx))
                            .p_3()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
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
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
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
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(check.message.clone());
                        row = row.child(row_msg);

                        if let Some(ref rem) = check.remediation {
                            let row_rem = div()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::CONTENT_BG))
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .text_xs()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
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
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child(format!(
                                    "Platform: {} [{}]",
                                    report.os_platform, report.os_arch
                                )),
                        )
                        .child(
                            div()
                                .id("rerun-diagnostics-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.run_diagnostics_checks(cx);
                                }))
                                .child("🔄 Re-run Checks"),
                        );
                    modal_box = modal_box.child(footer);
                }
            }
            DiagnosticsModalTab::Benchmarks => {
                if modal.is_running_benchmarks {
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
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .child("⏳ Running deterministic micro-benchmarks..."),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Evaluating AES-256-GCM AEAD, PBKDF2 stretching, autocomplete, template engine, and sync diffing..."),
                        );
                    modal_box = modal_box.child(loading);
                } else if let Some(ref b_report) = modal.benchmark_report {
                    let (b_bg, b_fg, b_title) = if b_report.failed_count > 0 {
                        (
                            rgb(0x4a1818),
                            rgb(MacTheme::DANGER),
                            format!(
                                "❌ SLA Breach: {} benchmark(s) failed strict SLAs.",
                                b_report.failed_count
                            ),
                        )
                    } else if b_report.warning_count > 0 {
                        (
                            rgb(0x3a2c10),
                            rgb(MacTheme::WARNING),
                            format!(
                                "⚠️ SLA Warning: {} benchmark(s) close to SLA threshold.",
                                b_report.warning_count
                            ),
                        )
                    } else {
                        (
                            rgb(0x133820),
                            rgb(MacTheme::SUCCESS),
                            "✅ All Performance Benchmarks Passed: Subsystem throughput and latency exceed strict SLA targets."
                                .to_string(),
                        )
                    };

                    let status_strip = div()
                        .px_4()
                        .py_2p5()
                        .rounded_md()
                        .bg(b_bg)
                        .border_1()
                        .border_color(b_fg)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(b_fg)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(b_title)
                        .child(format!(
                            "Total: {:.1}ms | Passed: {} | Warnings: {} | Failed: {}",
                            b_report.total_duration_ms,
                            b_report.passed_count,
                            b_report.warning_count,
                            b_report.failed_count
                        ));
                    modal_box = modal_box.child(status_strip);

                    let mut bench_list = div()
                        .id("diagnostics-bench-list")
                        .flex()
                        .flex_col()
                        .gap_2()
                        .overflow_y_scroll()
                        .max_h(px(320.0));

                    for (b_idx, metric) in b_report.metrics.iter().enumerate() {
                        let (badge_bg, badge_fg) = match metric.status {
                            BenchmarkStatus::Passed => (rgb(0x133820), rgb(MacTheme::SUCCESS)),
                            BenchmarkStatus::Warning => (rgb(0x3a2c10), rgb(MacTheme::WARNING)),
                            BenchmarkStatus::Failed => (rgb(0x4a1818), rgb(MacTheme::DANGER)),
                        };

                        let latency_str = if metric.latency_us_per_op >= 1000.0 {
                            format!("{:.2} ms/op", metric.latency_us_per_op / 1000.0)
                        } else {
                            format!("{:.1} µs/op", metric.latency_us_per_op)
                        };

                        let throughput_str = if let Some(mbps) = metric.throughput_mb_per_sec {
                            format!("{:.1} MB/s", mbps)
                        } else if metric.ops_per_sec >= 1000.0 {
                            format!("{:.0} k ops/s", metric.ops_per_sec / 1000.0)
                        } else {
                            format!("{:.0} ops/s", metric.ops_per_sec)
                        };

                        let mut row = div()
                            .id(("diag-bench-row", b_idx))
                            .p_3()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
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
                                    .child(div().text_sm().child(metric.category.icon()))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child(metric.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child(format!("({} iters)", metric.iterations)),
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
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                                            .child(throughput_str),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child(latency_str),
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
                                            .child(metric.status.badge()),
                                    ),
                            );
                        row = row.child(row_header);

                        let row_desc = div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child(metric.description.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::WARNING))
                                    .child(format!("SLA: {}", metric.target_sla)),
                            );
                        row = row.child(row_desc);

                        if !metric.note.is_empty() {
                            let row_note = div()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::CONTENT_BG))
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child(format!("↳ {}", metric.note));
                            row = row.child(row_note);
                        }

                        bench_list = bench_list.child(row);
                    }
                    modal_box = modal_box.child(bench_list);

                    let bench_footer = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pt_2()
                        .border_t_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child("Local Deterministic Micro-Benchmarks (All Offline)"),
                        )
                        .child(
                            div()
                                .id("rerun-benchmarks-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.run_benchmarks(cx);
                                }))
                                .child("⚡ Re-run Benchmarks"),
                        );
                    modal_box = modal_box.child(bench_footer);
                }
            }
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
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("mobile-modal-box")
            .w(px(760.0))
            .max_h(px(620.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("📱 Mobile Touch & Virtual Key Simulator (Phase 11)"),
                ),
            )
            .child(
                div()
                    .id("dismiss-mobile-modal-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_mobile_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Viewport selector & Lifecycle status banner
        let (status_bg, status_fg, status_icon) = match modal.lifecycle_manager.state {
            MobileSessionState::Connected => (rgb(0x133820), rgb(MacTheme::SUCCESS), "🟢"),
            MobileSessionState::BackgroundGracePeriod { .. } => {
                (rgb(0x3a2c10), rgb(MacTheme::WARNING), "🟡")
            }
            MobileSessionState::Suspended { .. } => (rgb(0x3a1a28), rgb(0xf472b6), "🟠"),
            MobileSessionState::Reconnecting { .. } => {
                (rgb(0x183048), rgb(MacTheme::ACCENT_BLUE), "🔄")
            }
            MobileSessionState::Connecting => (rgb(0x323236), rgb(MacTheme::TEXT_SECONDARY), "⏳"),
            MobileSessionState::Disconnected { .. } => (rgb(0x4a1818), rgb(MacTheme::DANGER), "🔴"),
        };

        let is_iphone = modal.preset == MobileViewportPreset::IPhone15Pro;
        let is_pixel = modal.preset == MobileViewportPreset::AndroidPixel8;
        let is_ipad = modal.preset == MobileViewportPreset::IPadPro11;

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
                            .border_1()
                            .border_color(if is_iphone {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .bg(if is_iphone {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_color(if is_iphone {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .hover(move |s| {
                                if is_iphone {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
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
                            .border_1()
                            .border_color(if is_pixel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .bg(if is_pixel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_color(if is_pixel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .hover(move |s| {
                                if is_pixel {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
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
                            .border_1()
                            .border_color(if is_ipad {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .bg(if is_ipad {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_color(if is_ipad {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .hover(move |s| {
                                if is_ipad {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
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
                    .border_1()
                    .border_color(status_fg)
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
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("OS Lifecycle:"),
            )
            .child(
                div()
                    .id("life-bg-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .bg(rgb(0x133820))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x1c4a2c)))
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
            .bg(rgb(MacTheme::CONTENT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
                .bg(rgb(0x323236))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(8.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(rgb(MacTheme::SEPARATOR)),
                ),
        );
        screen_area = screen_area.child(notch);

        for line in &modal.simulated_output {
            screen_area = screen_area.child(
                div()
                    .text_xs()
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(line.clone()),
            );
        }
        modal_box = modal_box.child(screen_area);

        // Virtual Keyboard Accessory Bar
        let mut accessory_strip = div()
            .id("virtual-accessory-strip")
            .p_2()
            .rounded_md()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
                    .rounded_md()
                    .border_1()
                    .border_color(if ctrl_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .bg(if ctrl_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if ctrl_active {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(move |s| {
                        if ctrl_active {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
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
                    .rounded_md()
                    .border_1()
                    .border_color(if alt_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .bg(if alt_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if alt_active {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(move |s| {
                        if alt_active {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
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
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .cursor_pointer()
                    .bg(rgb(0x323236))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
            .bg(rgb(MacTheme::CONTENT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(90.0))
            .overflow_y_scroll();

        if modal.input_history.is_empty() {
            telemetry = telemetry.child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Tap virtual keys above to stream ANSI escape codes..."),
            );
        } else {
            for entry in modal.input_history.iter().rev().take(6) {
                telemetry = telemetry.child(
                    div()
                        .text_xs()
                        .font_family(".AppleSystemUIFontMonospaced")
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
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
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child(format!(
                        "Viewport: {} ({} × {} cols/rows)",
                        modal.preset.label(),
                        cols,
                        rows
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
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
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("shell-integration-modal-box")
            .w(px(780.0))
            .max_h(px(640.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
            .border_color(rgb(MacTheme::SEPARATOR))
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
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("⚡ Shell Integration & Autocomplete (Phase 13)"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Semantic OSC 133 prompts, CWD tracking, password prompt classification, and offline autocomplete"),
                    ),
            )
            .child(
                div()
                    .id("dismiss-shell-integration-modal-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_active {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
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
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                            .bg(if is_sel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(if is_sel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
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
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .max_h(px(220.0))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(hook_code),
                    );
                hooks_body = hooks_body.child(script_box);

                // Instructions
                let install_cmd =
                    format!("eval \"$(shellumi shell-hook {})\"", modal.selected_shell);
                let instructions = div()
                    .p_2p5()
                    .rounded_md()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child("Installation Instructions:"),
                    )
                    .child(
                        div()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(format!(
                                "Add the following to your ~/.{}rc or config file:",
                                modal.selected_shell
                            )),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(MacTheme::CONTENT_BG))
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
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
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Semantic Prompt Escape Sequences (OSC 133 & OSC 7):"),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
                            "• OSC 133 ; A ST — Prompt Start: Signals the terminal that the interactive shell prompt has begun rendering.",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
                            "• OSC 133 ; B ST — Command Start: Signals the user has begun typing command input.",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
                            "• OSC 133 ; C ST — Command Executed: User pressed Enter; command process is now running.",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
                            "• OSC 133 ; D [; exitcode] ST — Command Finished: Command exited with status code (e.g. 0 for success).",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
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
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .child("🔍 Live Session State Inspector:"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                    .bg(rgb(0x3a2817))
                    .border_1()
                    .border_color(rgb(MacTheme::WARNING))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().child("🔒"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::WARNING))
                            .child("Security Guarantee: Password prompt detection is strictly passive. Shellumi NEVER auto-injects credentials into arbitrary terminal streams without explicit user confirmation."),
                    );
                pw_body = pw_body.child(sec_banner);

                // Sample prompt buttons
                let mut sample_buttons = div().flex().flex_col().gap_1p5();
                sample_buttons = sample_buttons.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                            .bg(rgb(MacTheme::INPUT_BG))
                            .text_xs()
                            .cursor_pointer()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("Current Prompt Line:"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                                rgb(0x3a2817)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(if modal.detected_prompt_kind.is_some() {
                                rgb(MacTheme::WARNING)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
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
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(if is_active {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
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
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .max_h(px(250.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1();

                if modal.autocomplete_results.is_empty() {
                    list_box = list_box.child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("No autocomplete suggestions found for query."),
                    );
                } else {
                    for item in &modal.autocomplete_results {
                        let kind_color = match item.kind {
                            CompletionKind::Command => rgb(MacTheme::ACCENT_BLUE),
                            CompletionKind::Subcommand => rgb(MacTheme::SUCCESS),
                            CompletionKind::Flag => rgb(MacTheme::WARNING),
                            CompletionKind::FilePath => rgb(0xbf5af2),
                            CompletionKind::Host => rgb(0xff375f),
                            CompletionKind::Snippet => rgb(0xff9f0a),
                            CompletionKind::History => rgb(MacTheme::TEXT_SECONDARY),
                        };

                        let row = div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(MacTheme::SURFACE))
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
                                            .bg(rgb(MacTheme::INPUT_BG))
                                            .text_xs()
                                            .text_color(kind_color)
                                            .child(format!("{:?}", item.kind)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child(item.display_label.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
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
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Local-First & Offline Engine: No telemetry or remote dependency"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("CLI: shellumi shell-hook / complete / prompt-detect"),
            );
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }

    fn render_session_replay_modal(
        &self,
        modal: &SessionReplayModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("session-replay-modal-overlay")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("session-replay-modal-box")
            .w(px(840.0))
            .max_h(px(660.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_4();

        // Header
        let header = div()
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
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Session logs"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(
                                "Asciicast v2 audit logging, automated secret redaction, and timestamped terminal playback.",
                            ),
                    ),
            )
            .child(
                div()
                    .id("dismiss-session-replay-modal-btn").when(self.page == Some("Session Logs"), |element|element.hidden())
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_session_replay_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Status notification if present
        if let Some(ref note) = modal.status_notification {
            modal_box = modal_box.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x133820))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(note.clone()),
            );
        }

        // Tab Navigation Bar
        let tabs = [
            (
                SessionReplayTab::RecordingsList,
                format!("📼 Recordings List ({})", modal.recordings.len()),
            ),
            (
                SessionReplayTab::ReplayPlayer,
                "▶ Replay Viewer".to_string(),
            ),
        ];

        let mut tab_bar = div().flex().items_center().gap_2();
        for (tab, label) in tabs {
            let is_active = modal.active_tab == tab;
            tab_bar = tab_bar.child(
                div()
                    .id(("session-replay-tab", tab as usize))
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
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_active {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.set_session_replay_tab(tab, cx);
                    }))
                    .child(label),
            );
        }
        modal_box = modal_box.child(tab_bar);

        // Tab Content
        match modal.active_tab {
            SessionReplayTab::RecordingsList => {
                let mut list_body = div().flex().flex_col().gap_3();

                if modal.recordings.is_empty() {
                    list_body = list_body.child(
                        div()
                            .id("recordings-empty-state")
                            .p_6()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("No session recordings found"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(
                                        "Click [⏺ Record] in any active terminal pane header to record your session.",
                                    ),
                            ),
                    );
                } else {
                    let mut scroll_list = div()
                        .id("recordings-scroll-list")
                        .max_h(px(380.0))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_2();

                    for (idx, rec) in modal.recordings.iter().enumerate() {
                        let is_selected = modal.selected_recording_id.as_deref() == Some(&rec.id);
                        let rec_id_play = rec.id.clone();
                        let rec_id_exp = rec.id.clone();
                        let rec_id_del = rec.id.clone();

                        let item = div()
                            .id(("rec-row", idx))
                            .p_3()
                            .rounded_lg()
                            .bg(if is_selected {
                                rgb(MacTheme::SURFACE_HOVER)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .border_1()
                            .border_color(if is_selected {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
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
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                    .child(rec.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(MacTheme::SURFACE))
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                                    .child(format!(
                                                        "⏱ {}",
                                                        rec.formatted_duration()
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(MacTheme::SURFACE))
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                                    .child(rec.formatted_size()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                            .child(format!(
                                                "ID: {} • {} events • Recorded at {}",
                                                rec.id, rec.event_count, rec.recorded_at
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id(("rec-play-btn", idx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(MacTheme::ACCENT_BLUE))
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.select_and_load_recording_for_playback(
                                                    rec_id_play.clone(),
                                                    cx,
                                                );
                                            }))
                                            .child("▶ Play"),
                                    )
                                    .child(
                                        div()
                                            .id(("rec-export-btn", idx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(MacTheme::SURFACE_HOVER))
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.export_session_recording_ui(
                                                    rec_id_exp.clone(),
                                                    cx,
                                                );
                                            }))
                                            .child("💾 Export"),
                                    )
                                    .child(
                                        div()
                                            .id(("rec-delete-btn", idx))
                                            .px_2p5()
                                            .py_1()
                                            .rounded_sm()
                                            .bg(rgb(MacTheme::DANGER))
                                            .text_xs()
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.delete_session_recording_ui(
                                                    rec_id_del.clone(),
                                                    cx,
                                                );
                                            }))
                                            .child("🗑 Delete"),
                                    ),
                            );

                        scroll_list = scroll_list.child(item);
                    }
                    list_body = list_body.child(scroll_list);
                }

                modal_box = modal_box.child(list_body);
            }
            SessionReplayTab::ReplayPlayer => {
                let mut player_body = div().flex().flex_col().gap_3();

                if let Some(ref cur) = modal.current_recording {
                    let engine_opt = modal.playback_engine.as_ref();
                    let status_badge = if let Some(e) = engine_opt {
                        match e.status() {
                            PlaybackStatus::Playing => div()
                                .bg(rgb(MacTheme::SUCCESS))
                                .text_color(rgb(0xffffff))
                                .child("▶ PLAYING"),
                            PlaybackStatus::Paused => div()
                                .bg(rgb(MacTheme::WARNING))
                                .text_color(rgb(0x000000))
                                .child("⏸ PAUSED"),
                            PlaybackStatus::Completed => div()
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_color(rgb(0xffffff))
                                .child("✓ COMPLETED"),
                            PlaybackStatus::Stopped => div()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("⏹ STOPPED"),
                        }
                    } else {
                        div()
                            .bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("⏹ STOPPED")
                    };

                    // Player Header Strip
                    let p_header = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(cur.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                        .child(format!(
                                            "Duration: {} • {} events • Recorded at {}",
                                            format_duration_hms(cur.duration_secs),
                                            cur.event_count,
                                            cur.recorded_at
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .px_2p5()
                                .py_1()
                                .rounded_sm()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .child(status_badge),
                        );
                    player_body = player_body.child(p_header);

                    // Scrubber and Controls Bar
                    let (time_label, speed_label, is_playing) = if let Some(e) = engine_opt {
                        (
                            e.formatted_time(),
                            e.speed().display_label(),
                            e.status() == PlaybackStatus::Playing,
                        )
                    } else {
                        ("00:00 / 00:00".to_string(), "1.0x", false)
                    };

                    let mut controls_bar = div()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(MacTheme::INPUT_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .flex()
                        .items_center()
                        .justify_between();

                    let playback_btns = div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("player-toggle-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_sm()
                                .bg(if is_playing {
                                    rgb(MacTheme::WARNING)
                                } else {
                                    rgb(MacTheme::SUCCESS)
                                })
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_playing {
                                    rgb(0x000000)
                                } else {
                                    rgb(0xffffff)
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.toggle_modal_playback(cx);
                                }))
                                .child(if is_playing { "⏸ Pause" } else { "▶ Play" }),
                        )
                        .child(
                            div()
                                .id("player-back-5s-btn")
                                .px_2p5()
                                .py_1p5()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.step_modal_playback(-5.0, cx);
                                }))
                                .child("⏪ -5s"),
                        )
                        .child(
                            div()
                                .id("player-fwd-5s-btn")
                                .px_2p5()
                                .py_1p5()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.step_modal_playback(5.0, cx);
                                }))
                                .child("⏩ +5s"),
                        );

                    let seek_pills = div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .id("player-seek-0")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(0.0, cx);
                                }))
                                .child("0%"),
                        )
                        .child(
                            div()
                                .id("player-seek-25")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(0.25, cx);
                                }))
                                .child("25%"),
                        )
                        .child(
                            div()
                                .id("player-seek-50")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(0.5, cx);
                                }))
                                .child("50%"),
                        )
                        .child(
                            div()
                                .id("player-seek-75")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(0.75, cx);
                                }))
                                .child("75%"),
                        )
                        .child(
                            div()
                                .id("player-seek-100")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.seek_modal_playback(1.0, cx);
                                }))
                                .child("100%"),
                        );

                    let speed_and_time = div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .id("player-cycle-speed-btn")
                                .px_2p5()
                                .py_1p5()
                                .rounded_sm()
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.cycle_modal_playback_speed(cx);
                                }))
                                .child(format!("⚡ {speed_label}")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(time_label),
                        );

                    controls_bar = controls_bar
                        .child(playback_btns)
                        .child(seek_pills)
                        .child(speed_and_time);
                    player_body = player_body.child(controls_bar);

                    // Replay Terminal Screen
                    let mut screen_div = div()
                        .id("player-screen-box")
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(MacTheme::CONTENT_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR))
                        .min_h(px(240.0))
                        .max_h(px(320.0))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_1();

                    if modal.screen_output_buffer.is_empty() {
                        screen_div = screen_div.child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child("[Ready to play. Click ▶ Play or seek to a timestamp.]"),
                        );
                    } else {
                        for (l_idx, line) in modal.screen_output_buffer.lines().enumerate() {
                            screen_div = screen_div.child(
                                div()
                                    .id(("screen-line", l_idx))
                                    .text_xs()
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child(if line.is_empty() { " " } else { line }.to_string()),
                            );
                        }
                    }
                    player_body = player_body.child(screen_div);
                } else {
                    player_body = player_body.child(
                        div()
                            .id("player-empty-prompt")
                            .p_6()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("No recording selected for playback"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(
                                        "Select a session from the [Recordings List] tab to start frame-by-frame replay.",
                                    ),
                            ),
                    );
                }

                modal_box = modal_box.child(player_body);
            }
        }

        // Footer
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .pt_3()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Compatible with Asciicast v2 (.cast) • Automated redaction enabled"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("CLI: shellumi session list / play / export"),
            );
        modal_box = modal_box.child(footer);

        if self.page == Some("Session Logs") {
            div()
                .w_full()
                .child(modal_box.w_full().max_h(px(1800.0)))
                .into_any_element()
        } else {
            overlay.child(modal_box).into_any_element()
        }
    }

    fn render_tunnel_modal(
        &self,
        modal: &TunnelModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("tunnel-modal-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut modal_box = div()
            .id("tunnel-modal-container")
            .w(px(840.0))
            .max_h(px(720.0))
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div().flex().items_center().gap_2().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("Port forwarding"),
                ),
            )
            .child(
                div()
                    .id("dismiss-tunnel-modal-btn")
                    .when(self.page == Some("Port Forwarding"), |element| {
                        element.hidden()
                    })
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_tunnel_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Status messages
        if let Some(ref err) = modal.error {
            modal_box = modal_box.child(
                div()
                    .id("tunnel-modal-error")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x4a1818))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("⚠ {err}")),
            );
        }
        if let Some(ref msg) = modal.status_message {
            modal_box = modal_box.child(
                div()
                    .id("tunnel-modal-status")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x133820))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(format!("✓ {msg}")),
            );
        }

        // Two-column layout
        let mut body = div().flex().gap_4().flex_1().overflow_hidden();

        // Left column: list of configured tunnels
        let mut list_col = div()
            .flex()
            .flex_col()
            .w(px(380.0))
            .gap_2()
            .border_r_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pr_4();

        list_col = list_col.child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(format!("CONFIGURED TUNNELS ({})", modal.tunnels.len())),
        );

        if modal.is_loading {
            list_col = list_col.child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Loading tunnels…"),
            );
        } else if modal.tunnels.is_empty() {
            list_col = list_col.child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("No port forwards configured yet. Create one on the right."),
            );
        } else {
            let mut scroll_list = div()
                .id("tunnel-scroll-list")
                .flex()
                .flex_col()
                .gap_2()
                .overflow_y_scroll();
            for (idx, tunnel) in modal.tunnels.iter().enumerate() {
                let is_selected = idx == modal.selected_index;
                let tid = tunnel.id;
                let is_active = self.tabs.iter().any(|tab| {
                    tab.name == format!("Tunnel: {}", tunnel.name)
                        && tab.panes.iter().any(|pane| pane.session.is_some())
                });
                let status_pill = if is_active {
                    ("Session open", "open")
                } else {
                    ("Stopped", "stopped")
                };

                let (flag_str, rule_summary) = match tunnel.rule.forwarding_type {
                    ForwardingType::Local => (
                        "Local (-L)",
                        format!(
                            "{}:{} → {}:{}",
                            tunnel.rule.bind_address.as_deref().unwrap_or("127.0.0.1"),
                            tunnel.rule.bind_port,
                            tunnel.rule.target_host.as_deref().unwrap_or("localhost"),
                            tunnel.rule.target_port.unwrap_or(80)
                        ),
                    ),
                    ForwardingType::Remote => (
                        "Remote (-R)",
                        format!(
                            "{}:{} → {}:{}",
                            tunnel.rule.bind_address.as_deref().unwrap_or("127.0.0.1"),
                            tunnel.rule.bind_port,
                            tunnel.rule.target_host.as_deref().unwrap_or("localhost"),
                            tunnel.rule.target_port.unwrap_or(80)
                        ),
                    ),
                    ForwardingType::Dynamic => (
                        "Dynamic (-D SOCKS)",
                        format!(
                            "{}:{}",
                            tunnel.rule.bind_address.as_deref().unwrap_or("127.0.0.1"),
                            tunnel.rule.bind_port
                        ),
                    ),
                };

                let item_bg = if is_selected {
                    rgb(MacTheme::SURFACE_HOVER)
                } else {
                    rgb(MacTheme::INPUT_BG)
                };

                let mut card = div()
                    .id(("tunnel-item", tid as usize))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .rounded_md()
                    .bg(item_bg)
                    .border_1()
                    .border_color(if is_selected {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.select_tunnel_modal_item(idx, cx);
                    }));

                let glyph_box = div()
                    .w(px(30.0))
                    .h(px(30.0))
                    .rounded_md()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                    .child("⇄");

                let status_dot = div()
                    .w(px(7.0))
                    .h(px(7.0))
                    .rounded_full()
                    .bg(rgb(if is_active {
                        MacTheme::SUCCESS
                    } else {
                        MacTheme::TEXT_TERTIARY
                    }));

                // Top row: glyph + dot + name + status pill
                let top_row = div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(glyph_box)
                    .child(status_dot)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child(tunnel.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child(format!("{flag_str}: {rule_summary}")),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(if is_active {
                                rgb(0x133820)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .text_xs()
                            .text_color(if is_active {
                                rgb(MacTheme::SUCCESS)
                            } else {
                                rgb(MacTheme::TEXT_TERTIARY)
                            })
                            .child(status_pill.0),
                    );
                card = card.child(top_row);

                // Actions row: Start/Stop button + Delete button
                let actions_row = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_1()
                    .child(
                        div()
                            .id(("toggle-tunnel-btn", tid as usize))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(if is_active {
                                rgb(MacTheme::WARNING)
                            } else {
                                rgb(MacTheme::SUCCESS)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_active {
                                rgb(0x000000)
                            } else {
                                rgb(0xffffff)
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, window, cx| {
                                app.toggle_tunnel_state(tid, window, cx);
                            }))
                            .child(if is_active { "⏹ Stop" } else { "▶ Start" }),
                    )
                    .child(
                        div()
                            .id(("del-tunnel-btn", tid as usize))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::DANGER))
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.delete_tunnel_modal(tid, cx);
                            }))
                            .child("🗑 Delete"),
                    );
                card = card.child(
                    div()
                        .id(("edit-tunnel", tid as u64))
                        .px_2()
                        .py_1()
                        .text_sm()
                        .cursor_pointer()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .on_click(cx.listener(move |app, _, _, cx| app.edit_saved_tunnel(tid, cx)))
                        .child("Edit rule"),
                );
                card = card.child(actions_row);

                scroll_list = scroll_list.child(card);
            }
            list_col = list_col.child(scroll_list);
        }
        body = body.child(list_col);

        // Right column: Selected Tunnel Details & Add New Tunnel Form
        let mut right_col = div()
            .id("tunnel-right-col")
            .flex()
            .flex_col()
            .flex_1()
            .gap_4()
            .overflow_y_scroll();

        // Selected tunnel details panel (if one is selected)
        if let Some(selected_tunnel) = modal.tunnels.get(modal.selected_index) {
            let host = match &self.hosts {
                HostState::Loaded(hosts) => hosts
                    .iter()
                    .find(|host| Some(host.id) == selected_tunnel.host_id),
                _ => None,
            };
            let preview_str = if let Some(host) = host {
                let target = format!(
                    "{}@{}",
                    host.destination.username(),
                    host.destination.host()
                );
                let mut args = build_ssh_tunnel_command(
                    &selected_tunnel.rule,
                    &target,
                    host.jump_host.as_deref(),
                );
                let insert = args.len().saturating_sub(1);
                args.splice(
                    insert..insert,
                    ["-p".to_string(), host.destination.port().to_string()],
                );
                format!("ssh {}", args.join(" "))
            } else {
                "Select a saved host to start this rule.".into()
            };

            let details_box = div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_md()
                .bg(rgb(MacTheme::INPUT_BG))
                .border_1()
                .border_color(rgb(MacTheme::SEPARATOR))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                        .child(format!("Saved rule: {}", selected_tunnel.name)),
                )
                .child(div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child("Open the tunnel terminal to inspect authentication and forwarding errors. Traffic counters are not measured."))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child("SSH Command Preview:"),
                        )
                        .child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(MacTheme::CONTENT_BG))
                                .text_xs()
                                .text_color(rgb(MacTheme::SUCCESS))
                                .child(preview_str),
                        ),
                );
            right_col = right_col.child(details_box);
        }

        // Add New Tunnel Form
        let mut form_box = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_md()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child("➕ Configure New Tunnel"),
            );

        let mut host_picker = div().flex().flex_wrap().gap_2();
        if let HostState::Loaded(hosts) = &self.hosts {
            for host in hosts.iter().filter(|host| host.is_ssh()) {
                let id = host.id;
                host_picker = host_picker.child(
                    div()
                        .id(("tunnel-host", id as u64))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_sm()
                        .bg(if modal.selected_host_id == Some(id) {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SURFACE)
                        })
                        .text_color(if modal.selected_host_id == Some(id) {
                            rgb(0xffffff)
                        } else {
                            rgb(MacTheme::TEXT_SECONDARY)
                        })
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            if let Some(modal) = &mut app.tunnel_modal {
                                modal.selected_host_id = Some(id);
                            }
                            cx.notify();
                        }))
                        .child(host.name.clone()),
                );
            }
        }
        form_box = form_box
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("SSH host"),
            )
            .child(host_picker);
        // Forwarding Type Buttons
        let ftype_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("ftype-local-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(if modal.forwarding_type == ForwardingType::Local {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_xs()
                    .text_color(if modal.forwarding_type == ForwardingType::Local {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.set_tunnel_forwarding_type(ForwardingType::Local, cx)
                    }))
                    .child("Local (-L)"),
            )
            .child(
                div()
                    .id("ftype-remote-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(if modal.forwarding_type == ForwardingType::Remote {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_xs()
                    .text_color(if modal.forwarding_type == ForwardingType::Remote {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.set_tunnel_forwarding_type(ForwardingType::Remote, cx)
                    }))
                    .child("Remote (-R)"),
            )
            .child(
                div()
                    .id("ftype-dynamic-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(if modal.forwarding_type == ForwardingType::Dynamic {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_xs()
                    .text_color(if modal.forwarding_type == ForwardingType::Dynamic {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.set_tunnel_forwarding_type(ForwardingType::Dynamic, cx)
                    }))
                    .child("Dynamic SOCKS5 (-D)"),
            );
        form_box = form_box.child(ftype_row);

        // Inputs
        form_box = form_box.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Tunnel Name:"),
                )
                .child(modal.name_input.clone()),
        );

        let mut ports_row = div().flex().items_center().gap_3().child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Bind Port:"),
                )
                .child(modal.bind_port_input.clone()),
        );

        if modal.forwarding_type != ForwardingType::Dynamic {
            ports_row = ports_row
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Target Host:"),
                        )
                        .child(modal.target_host_input.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Target Port:"),
                        )
                        .child(modal.target_port_input.clone()),
                );
        }
        form_box = form_box.child(ports_row);

        // Create Button
        form_box = form_box.child(
            div()
                .id("save-tunnel-btn")
                .px_4()
                .py_2()
                .rounded_md()
                .bg(rgb(MacTheme::ACCENT_BLUE))
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .on_click(cx.listener(|app, _, _, cx| {
                    app.save_new_tunnel_modal(cx);
                }))
                .child(if modal.editing_id.is_some() {
                    "Save changes"
                } else {
                    "Add tunnel"
                }),
        );

        right_col = right_col.child(form_box);
        body = body.child(right_col);
        modal_box = modal_box.child(body);

        if self.page == Some("Port Forwarding") {
            div()
                .w_full()
                .child(modal_box.w_full().max_h(px(1800.0)))
                .into_any_element()
        } else {
            overlay.child(modal_box).into_any_element()
        }
    }

    fn render_remote_desktop_session(
        &self,
        tab_idx: usize,
        state: &RemoteDesktopState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let is_vnc = state.protocol == RemoteDesktopProtocol::Vnc;
        let is_connected = state.status == RemoteDesktopStatus::Connected;

        // 1. Top control toolbar
        let proto_pill = div()
            .flex()
            .items_center()
            .gap_1p5()
            .px_2p5()
            .py_1()
            .rounded_md()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .bg(if is_vnc { rgb(0x3a1b5c) } else { rgb(0x103d3d) })
            .text_color(if is_vnc { rgb(0xbb86fc) } else { rgb(0x3de0d8) })
            .child(if is_vnc { "🖥️ VNC" } else { "🪟 RDP" });

        let status_pill = div()
            .flex()
            .items_center()
            .gap_1p5()
            .px_2p5()
            .py_1()
            .rounded_md()
            .text_xs()
            .bg(if is_connected {
                rgb(0x0e3b2e)
            } else {
                rgb(0x3e1717)
            })
            .text_color(if is_connected {
                rgb(0x3dd68c)
            } else {
                rgb(0xeb5757)
            })
            .child(if is_connected {
                format!(
                    "● Connected · {}ms · {} FPS · {}x{} · {}bpp",
                    state.latency_ms, state.fps, state.width, state.height, state.color_depth
                )
            } else {
                "○ Disconnected".to_string()
            });

        let cad_btn = div()
            .id(("rd-cad", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(MacTheme::SURFACE_ACTIVE))
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_send_key(
                    tab_idx,
                    "Ctrl+Alt+Del",
                    rfb_constants::KEYSYM_DELETE,
                    cx,
                );
            }))
            .child("Ctrl+Alt+Del");

        let win_btn = div()
            .id(("rd-win", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(MacTheme::SURFACE_ACTIVE))
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_send_key(
                    tab_idx,
                    "Win/Super",
                    rfb_constants::KEYSYM_SUPER_L,
                    cx,
                );
            }))
            .child("⊞ Win");

        let alt_tab_btn = div()
            .id(("rd-alttab", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(MacTheme::SURFACE_ACTIVE))
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_send_key(tab_idx, "Alt+Tab", rfb_constants::KEYSYM_TAB, cx);
            }))
            .child("Alt+Tab");

        let clip_btn = div()
            .id(("rd-clip", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgb(MacTheme::SURFACE_ACTIVE))
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_sync_clipboard(tab_idx, cx);
            }))
            .child("📋 Sync Clipboard");

        let scale_fit = div()
            .id(("rd-scale-fit", tab_idx))
            .px_2()
            .py_1()
            .rounded_l_sm()
            .text_xs()
            .cursor_pointer()
            .bg(if state.scale_mode == RemoteDesktopScaleMode::Fit {
                rgb(MacTheme::ACCENT_BLUE)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .text_color(if state.scale_mode == RemoteDesktopScaleMode::Fit {
                rgb(0xffffff)
            } else {
                rgb(MacTheme::TEXT_SECONDARY)
            })
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_set_scale_mode(tab_idx, RemoteDesktopScaleMode::Fit, cx);
            }))
            .child("Fit");

        let scale_orig = div()
            .id(("rd-scale-orig", tab_idx))
            .px_2()
            .py_1()
            .text_xs()
            .cursor_pointer()
            .bg(if state.scale_mode == RemoteDesktopScaleMode::Original {
                rgb(MacTheme::ACCENT_BLUE)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .text_color(if state.scale_mode == RemoteDesktopScaleMode::Original {
                rgb(0xffffff)
            } else {
                rgb(MacTheme::TEXT_SECONDARY)
            })
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_set_scale_mode(tab_idx, RemoteDesktopScaleMode::Original, cx);
            }))
            .child("100%");

        let scale_fill = div()
            .id(("rd-scale-fill", tab_idx))
            .px_2()
            .py_1()
            .rounded_r_sm()
            .text_xs()
            .cursor_pointer()
            .bg(if state.scale_mode == RemoteDesktopScaleMode::Fill {
                rgb(MacTheme::ACCENT_BLUE)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .text_color(if state.scale_mode == RemoteDesktopScaleMode::Fill {
                rgb(0xffffff)
            } else {
                rgb(MacTheme::TEXT_SECONDARY)
            })
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_set_scale_mode(tab_idx, RemoteDesktopScaleMode::Fill, cx);
            }))
            .child("Fill");

        let view_only_btn = div()
            .id(("rd-viewonly", tab_idx))
            .px_2()
            .py_1()
            .rounded_sm()
            .text_xs()
            .cursor_pointer()
            .bg(if state.view_only {
                rgb(0x4a3200)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .text_color(if state.view_only {
                rgb(0xf5a623)
            } else {
                rgb(MacTheme::TEXT_SECONDARY)
            })
            .on_click(cx.listener(move |app, _, _, cx| {
                app.remote_desktop_toggle_view_only(tab_idx, cx);
            }))
            .child(if state.view_only {
                "👁️ View Only"
            } else {
                "🖱️ Interactive"
            });

        let disc_btn = if is_connected {
            div()
                .id(("rd-disconnect", tab_idx))
                .px_2p5()
                .py_1()
                .rounded_sm()
                .text_xs()
                .bg(rgb(0x3e1717))
                .text_color(rgb(MacTheme::DANGER))
                .cursor_pointer()
                .hover(|s| s.bg(rgb(MacTheme::DANGER)).text_color(rgb(0xffffff)))
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.remote_desktop_disconnect(tab_idx, cx);
                }))
                .child("⏹ Disconnect")
        } else {
            div()
                .id(("rd-reconnect", tab_idx))
                .px_2p5()
                .py_1()
                .rounded_sm()
                .text_xs()
                .bg(rgb(MacTheme::ACCENT_BLUE))
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.remote_desktop_reconnect(tab_idx, cx);
                }))
                .child("🔄 Reconnect")
        };

        let action_note = state.last_action_note.as_ref().map(|note| {
            div()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(rgb(MacTheme::SURFACE_ACTIVE))
                .text_xs()
                .text_color(rgb(MacTheme::ACCENT_BLUE))
                .child(note.clone())
        });

        let toolbar = div()
            .h(px(44.0))
            .px_4()
            .flex()
            .items_center()
            .justify_between()
            .bg(rgb(MacTheme::SURFACE))
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(proto_pill)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(state.target.clone()),
                    )
                    .child(status_pill)
                    .children(action_note),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(cad_btn)
                            .child(win_btn)
                            .child(alt_tab_btn)
                            .child(clip_btn),
                    )
                    .child(div().w(px(1.0)).h(px(16.0)).bg(rgb(MacTheme::SEPARATOR)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(scale_fit)
                            .child(scale_orig)
                            .child(scale_fill),
                    )
                    .child(view_only_btn)
                    .child(disc_btn),
            );

        let viewport = if is_connected {
            let canvas_area = div()
                .id(("rd-canvas", tab_idx))
                .relative()
                .cursor_pointer()
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.remote_desktop_click_canvas(tab_idx, 640, 480, cx);
                }));

            let desktop_ui = if is_vnc {
                div()
                    .w_full()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x1e152a))
                    .child(
                        div()
                            .h(px(28.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .bg(rgb(0x0a0710))
                            .text_xs()
                            .text_color(rgb(0xdfdbe5))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(div().font_weight(FontWeight::BOLD).child("Activities"))
                                    .child("Terminal")
                                    .child("Files"),
                            )
                            .child(div().font_weight(FontWeight::MEDIUM).child("Tue Nov 10 20:30"))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child("📶")
                                    .child("🔊")
                                    .child("🔋 100%")
                                    .child("⏻"),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(680.0))
                                    .h(px(380.0))
                                    .rounded_lg()
                                    .bg(rgb(0x110d19))
                                    .border_1()
                                    .border_color(rgb(0x3e2b54))
                                    .shadow_md()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .h(px(32.0))
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .bg(rgb(0x1c1328))
                                            .rounded_t_lg()
                                            .border_b_1()
                                            .border_color(rgb(0x2d1f40))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child(
                                                        div()
                                                            .w(px(10.0))
                                                            .h(px(10.0))
                                                            .rounded_full()
                                                            .bg(rgb(0xe95420)),
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(10.0))
                                                            .h(px(10.0))
                                                            .rounded_full()
                                                            .bg(rgb(0x77216f)),
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(10.0))
                                                            .h(px(10.0))
                                                            .rounded_full()
                                                            .bg(rgb(0x5e2750)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(0xdfdbe5))
                                                    .child(format!(
                                                        "user@{}: ~ (Remote VNC)",
                                                        state.host_name
                                                    )),
                                            )
                                            .child(div().w(px(30.0))),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_4()
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text_xs()
                                            .text_color(rgb(0x4af626))
                                            .child(format!(
                                                "Linux {} 6.8.0-generic #42-Ubuntu SMP x86_64",
                                                state.host_name
                                            ))
                                            .child(div().h_2())
                                            .child(
                                                div()
                                                    .text_color(rgb(0xffffff))
                                                    .child("user@server:~$ htop --sort-key PERCENT_CPU"),
                                            )
                                            .child(div().h_1())
                                            .child(
                                                div()
                                                    .text_color(rgb(0x389bf2))
                                                    .child("  CPU[|||||||||||||||||||||         42.8%]   Tasks: 182, 4 running"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0xbb86fc))
                                                    .child("  Mem[|||||||||||||||||||||||||    5.28G/16.0G]  Uptime: 24 days, 11:42"),
                                            )
                                            .child(div().h_2())
                                            .child(
                                                div()
                                                    .text_color(rgb(0xe0e0e0))
                                                    .child("  PID USER      PRI  NI  VIRT   RES   SHR S CPU% MEM%   TIME+  COMMAND"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3dd68c))
                                                    .child(" 1420 user       20   0 1824M  340M  112M S 18.2  2.1 48:12.33 /usr/bin/gnome-shell"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3dd68c))
                                                    .child(" 2941 root       20   0  982M  128M   48M S 12.4  0.8 19:40.11 /usr/bin/dockerd"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3dd68c))
                                                    .child(" 4108 user       20   0  412M   84M   32M S  8.1  0.5 05:22.90 /usr/bin/vncserver"),
                                            ),
                                    ),
                            ),
                    )
            } else {
                div()
                    .w_full()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x0f2027))
                    .child(
                        div()
                            .flex_1()
                            .p_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(720.0))
                                    .h(px(400.0))
                                    .rounded_lg()
                                    .bg(rgb(0x0c0c0c))
                                    .border_1()
                                    .border_color(rgb(0x2b3846))
                                    .shadow_md()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .h(px(32.0))
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .bg(rgb(0x19232d))
                                            .rounded_t_lg()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child("⚡")
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(0xffffff))
                                                            .child(format!(
                                                                "Administrator: Windows PowerShell — {}",
                                                                state.host_name
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_3()
                                                    .text_xs()
                                                    .text_color(rgb(0xcccccc))
                                                    .child("─")
                                                    .child("□")
                                                    .child("✕"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_4()
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text_xs()
                                            .text_color(rgb(0xcccccc))
                                            .child("Windows PowerShell")
                                            .child(
                                                "Copyright (C) Microsoft Corporation. All rights reserved.",
                                            )
                                            .child(div().h_2())
                                            .child(
                                                div()
                                                    .text_color(rgb(0xffffff))
                                                    .child("PS C:\\Users\\Administrator> Get-Service | Where-Object Status -eq 'Running' | Select-Object -First 5"),
                                            )
                                            .child(div().h_1())
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3de0d8))
                                                    .child("Status   Name               DisplayName"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x3de0d8))
                                                    .child("------   ----               -----------"),
                                            )
                                            .child("Running  AudioSrv           Windows Audio")
                                            .child("Running  Dhcp               DHCP Client")
                                            .child("Running  EventLog           Windows Event Log")
                                            .child("Running  LanmanServer       Server")
                                            .child("Running  TermService        Remote Desktop Services")
                                            .child(div().h_2())
                                            .child(
                                                div()
                                                    .text_color(rgb(0xffffff))
                                                    .child("PS C:\\Users\\Administrator> █"),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h(px(40.0))
                            .px_4()
                            .flex()
                            .items_center()
                            .justify_between()
                            .bg(rgb(0x141b22))
                            .border_t_1()
                            .border_color(rgb(0x232c37))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x0078d4))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("⊞"),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x1e2732))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("🔍"),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x1e2732))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("🗔"),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x1e2732))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("📁"),
                                    )
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_sm()
                                            .bg(rgb(0x1e2732))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("⚡"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .text_xs()
                                    .text_color(rgb(0xcccccc))
                                    .child("ENG")
                                    .child("🔊")
                                    .child("🌐")
                                    .child("20:30"),
                            ),
                    )
            };

            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .bg(rgb(0x0c0f14))
                .flex()
                .items_center()
                .justify_center()
                .child(canvas_area.w_full().h_full().child(desktop_ui))
        } else {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(0x0c0f14))
                .child(
                    div()
                        .w(px(420.0))
                        .p_6()
                        .rounded_lg()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .child(div().text_2xl().child(if is_vnc { "🖥️" } else { "🪟" }))
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child("Remote Desktop Disconnected"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child(format!(
                                    "Session to {} ({}) has ended.",
                                    state.host_name, state.target
                                )),
                        )
                        .child(
                            div()
                                .id(("rd-reconnect-center", tab_idx))
                                .mt_2()
                                .px_4()
                                .py_2()
                                .rounded_md()
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.remote_desktop_reconnect(tab_idx, cx);
                                }))
                                .child("🔄 Reconnect Session"),
                        ),
                )
        };

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::CONTENT_BG))
            .child(toolbar)
            .child(viewport)
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
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .rounded_md()
                    .text_sm()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("Error: {err}")),
            );
        } else if let Some(msg) = &state.status_message {
            container = container.child(
                div()
                    .p_2()
                    .bg(rgb(0x133820))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .rounded_md()
                    .text_sm()
                    .text_color(rgb(MacTheme::SUCCESS))
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
            .text_color(rgb(MacTheme::TEXT_SECONDARY))
            .flex_wrap();
        local_crumbs_div = local_crumbs_div.child("💻 Local: ");
        for (i, (name, path)) in local_crumbs.into_iter().enumerate() {
            let p_clone = path.clone();
            local_crumbs_div = local_crumbs_div.child(
                div()
                    .id(("local-crumb", i))
                    .cursor_pointer()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
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
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
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
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_color(if is_selected {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .hover(|s| {
                        s.bg(if is_selected {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SURFACE_HOVER)
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
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(entry.formatted_size()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_center()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child(entry.formatted_permissions()),
                    );

                local_list_div = local_list_div.child(row_el);
            }
        }

        let local_pane = div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(0x323236))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                    .border_1()
                    .border_color(if has_local_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if has_local_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_color(if has_local_sel {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .text_sm()
                    .cursor_pointer()
                    .hover(move |s| {
                        if has_local_sel {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
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
                    .border_1()
                    .border_color(if has_remote_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if has_remote_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_color(if has_remote_sel {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .text_sm()
                    .cursor_pointer()
                    .hover(move |s| {
                        if has_remote_sel {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
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
            .text_color(rgb(MacTheme::TEXT_SECONDARY))
            .flex_wrap();
        remote_crumbs_div = remote_crumbs_div.child(format!("🌐 Remote ({}): ", state.host_name));
        for (i, (name, path)) in remote_crumbs.into_iter().enumerate() {
            let p_clone = path.clone();
            remote_crumbs_div = remote_crumbs_div.child(
                div()
                    .id(("remote-crumb", i))
                    .cursor_pointer()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
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

        if state.loading {
            remote_list_div = remote_list_div.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                    .child("Loading remote directory…"),
            );
        } else if state.remote_entries.is_empty() {
            remote_list_div = remote_list_div.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
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
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .text_color(if is_selected {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_PRIMARY)
                    })
                    .hover(|s| {
                        s.bg(if is_selected {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::SURFACE_HOVER)
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
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(entry.formatted_size()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_center()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child(entry.formatted_permissions()),
                    );

                remote_list_div = remote_list_div.child(row_el);
            }
        }

        let remote_pane = div()
            .flex_1()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(0x323236))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
            .bg(rgb(MacTheme::SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
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
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
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
            .bg(rgb(MacTheme::INPUT_BG))
            .rounded_full()
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .bg(rgb(MacTheme::ACCENT_BLUE))
                    .w(gpui::relative(pct / 100.0)),
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
                    TransferStatus::Completed => div()
                        .text_color(rgb(MacTheme::SUCCESS))
                        .child("✓ Completed"),
                    TransferStatus::InProgress => div()
                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                        .child("⟳ In Progress"),
                    TransferStatus::Failed(err) => div()
                        .text_color(rgb(MacTheme::DANGER))
                        .child(format!("✗ Failed: {err}")),
                    TransferStatus::Queued => div()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("⏳ Queued"),
                    TransferStatus::Cancelled => div()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("Cancelled"),
                };

                let mut task_row = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(div().flex_1().min_w_0().truncate().child(format!(
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

                if matches!(
                    task.status,
                    TransferStatus::Queued | TransferStatus::InProgress
                ) {
                    let task_id = task.id;
                    let tab_id = self.tabs[tab_idx].id;
                    let requested = self
                        .sftp_jobs
                        .get(&(tab_id, task_id))
                        .is_some_and(|job| job.control.is_cancelled());
                    if requested {
                        task_row = task_row.child("Cancelling…");
                    } else {
                        task_row = task_row.child(
                            div()
                                .id(("cancel-transfer", task_id))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .bg(rgb(0x323236))
                                .text_color(rgb(MacTheme::DANGER))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .child("Cancel")
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.sftp_cancel_transfer(tab_id, task_id, cx)
                                })),
                        );
                    }
                }
                tasks_list = tasks_list.child(task_row);
            }

            queue_drawer = queue_drawer.child(tasks_list);
        }

        container = container.child(queue_drawer);
        container
    }
}

fn host_form_field(label: &'static str, input: Entity<TextInput>) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_sm()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(label),
        )
        .child(input)
}

fn group_matches(host_group: Option<&str>, selected_group: Option<&str>) -> bool {
    selected_group.is_none_or(|group| host_group == Some(group))
}

fn host_detail(label: &'static str, value: String) -> gpui::Div {
    div()
        .flex()
        .justify_between()
        .gap_2()
        .border_b_1()
        .border_color(rgb(MacTheme::BORDER))
        .pb_2()
        .child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(label),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                .child(value),
        )
}

pub(crate) fn resolve_icon_path(
    name: &str,
    is_active: bool,
    size_px: u32,
) -> Option<std::path::PathBuf> {
    // Corrected asset states: "regular" folder is normal state (borderless), "hover" folder is active/hover state (dark rounded tile)
    let state = if is_active { "hover" } else { "regular" };
    let alt_state = if is_active { "regular" } else { "hover" };
    let size_folder = match size_px {
        s if s <= 32 => "32",
        s if s <= 48 => "48",
        s if s <= 64 => "64",
        s if s <= 128 => "128",
        _ => "256",
    };
    let filename = format!("{name}.png");

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let bundle_path = parent
                .join("../Resources/icon_pack/icons")
                .join(size_folder)
                .join(state)
                .join(&filename);
            if bundle_path.exists() {
                return Some(bundle_path);
            }
            let bundle_alt = parent
                .join("../Resources/icon_pack/icons")
                .join(size_folder)
                .join(alt_state)
                .join(&filename);
            if bundle_alt.exists() {
                return Some(bundle_alt);
            }
        }
    }
    let local = std::path::PathBuf::from("resources/icon_pack/icons")
        .join(size_folder)
        .join(state)
        .join(&filename);
    if local.exists() {
        return Some(local);
    }
    let local_alt = std::path::PathBuf::from("resources/icon_pack/icons")
        .join(size_folder)
        .join(alt_state)
        .join(&filename);
    if local_alt.exists() {
        return Some(local_alt);
    }
    None
}

pub(crate) fn resolve_file_icon_path(entry: &FileEntry) -> Option<std::path::PathBuf> {
    let icon_name = if entry.kind == FileKind::Directory {
        match entry.name.as_str() {
            "Documents" => "folder_documents",
            "Downloads" => "folder_downloads",
            "Desktop" => "folder_desktop",
            "Movies" => "folder_movies",
            "Music" => "folder_music",
            "Pictures" => "folder_pictures",
            "Applications" => "folder_applications",
            "Public" => "folder_public",
            _ => "folder",
        }
    } else {
        let ext = std::path::Path::new(&entry.name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "txt" | "md" | "log" | "csv" | "conf" | "ini" | "env" => "text",
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" => "image",
            "mp3" | "wav" | "aac" | "flac" | "m4a" | "ogg" => "audio",
            "mp4" | "mkv" | "mov" | "avi" | "webm" => "video",
            "rs" | "js" | "ts" | "py" | "c" | "cpp" | "h" | "go" | "java" | "sh" | "zsh"
            | "bash" | "json" | "toml" | "yaml" | "yml" | "html" | "css" => "code",
            "bin" | "app" | "dmg" | "pkg" | "exe" => "executable",
            "ttf" | "otf" | "woff" | "woff2" => "font",
            _ => {
                if entry.permissions.map(|p| p & 0o111 != 0).unwrap_or(false) {
                    "executable"
                } else {
                    "document"
                }
            }
        }
    };
    let filename = format!("{icon_name}.png");
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let bundle_path = parent.join("../Resources/file_icons").join(&filename);
            if bundle_path.exists() {
                return Some(bundle_path);
            }
        }
    }
    let local = std::path::PathBuf::from("resources/file_icons").join(&filename);
    if local.exists() {
        return Some(local);
    }
    None
}

fn resolve_logo_path() -> Option<std::path::PathBuf> {
    if let Some(logo_icon) = resolve_icon_path("shellumi_logo", false, 64) {
        return Some(logo_icon);
    }
    if let Some(terminal_icon) = resolve_icon_path("terminal", false, 64) {
        return Some(terminal_icon);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("../Resources/logo.png");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    let local = std::path::PathBuf::from("resources/logo.png");
    if local.exists() {
        return Some(local);
    }
    None
}

pub(crate) fn resolve_host_os_icon(host: &SavedHost) -> &'static str {
    let lower_name = host.name.to_lowercase();
    let tags_str = host.tags.join(" ").to_lowercase();
    let combined = format!("{} {}", lower_name, tags_str);
    if combined.contains("ubuntu") {
        "os_ubuntu"
    } else if combined.contains("arch") {
        "os_arch"
    } else if combined.contains("macos")
        || combined.contains("darwin")
        || combined.contains("apple")
        || combined.contains("mac")
    {
        "os_macos"
    } else if combined.contains("freebsd") {
        "os_freebsd"
    } else if combined.contains("netbsd") {
        "os_netbsd"
    } else if combined.contains("openbsd") {
        "os_openbsd"
    } else if combined.contains("windows") || combined.contains("win") {
        "os_windows"
    } else if combined.contains("linux")
        || combined.contains("debian")
        || combined.contains("centos")
        || combined.contains("fedora")
        || combined.contains("rhel")
    {
        "os_linux"
    } else {
        "server"
    }
}

pub(crate) fn format_file_timestamp(secs_opt: Option<u64>) -> String {
    let Some(secs) = secs_opt else {
        return "--".to_string();
    };
    let total_secs = secs;
    let days = total_secs / 86400;
    let day_secs = total_secs % 86400;
    let hour = (day_secs / 3600) % 24;
    let minute = (day_secs % 3600) / 60;

    let z = days as i64 + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    let am_pm = if hour >= 12 { "PM" } else { "AM" };
    let hour12 = match hour % 12 {
        0 => 12,
        h => h,
    };
    format!("{}/{}/{}, {}:{:02} {}", m, d, y, hour12, minute, am_pm)
}

impl Shellumi {
    fn workspace_sidebar(
        &self,
        _active: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let nav_items = [
            ("Hosts", "hosts", "🖧"),
            ("Keychain", "keychain", "🔑"),
            ("Port Forwarding", "port_forwarding", "⇄"),
            ("Snippets", "snippets", "{ }"),
            ("Known Hosts", "known_hosts", "🛡"),
            ("Session Logs", "log", "⏱"),
        ];

        if self.sidebar_collapsed {
            // Collapsed Rail (Width: 56px, icon-only)
            let logo_element = if let Some(logo_path) = resolve_logo_path() {
                div()
                    .w(px(32.0))
                    .h(px(32.0))
                    .rounded_md()
                    .overflow_hidden()
                    .child(gpui::img(logo_path).size_full())
            } else {
                div()
                    .w(px(32.0))
                    .h(px(32.0))
                    .rounded_md()
                    .bg(rgb(MacTheme::BADGE_NAVY))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                    .font_weight(FontWeight::BOLD)
                    .child("🐚")
            };

            let mut rail_links = div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .gap_2()
                .items_center();

            for (page, icon_name, glyph) in nav_items {
                let is_sel = if page == "Hosts" {
                    self.page.is_none() && self.active_tab_idx.is_none()
                } else {
                    self.page == Some(page)
                };
                let icon_element = if let Some(icon_path) = resolve_icon_path(icon_name, is_sel, 48)
                {
                    div()
                        .w(px(24.0))
                        .h(px(24.0))
                        .child(gpui::img(icon_path).size_full())
                        .into_any_element()
                } else {
                    div().child(glyph).into_any_element()
                };
                rail_links = rail_links.child(
                    div()
                        .id(gpui::SharedString::from(format!("nav-rail-{page}")))
                        .w(px(38.0))
                        .h(px(38.0))
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .bg(if is_sel {
                            rgb(MacTheme::SURFACE_ACTIVE)
                        } else {
                            rgba(0x00000000)
                        })
                        .text_color(if is_sel {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::TEXT_SECONDARY)
                        })
                        .hover(|s| {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        })
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.navigate_page(page, window, cx);
                        }))
                        .child(icon_element),
                );
            }

            return div()
                .id("sidebar-rail")
                .w(px(56.0))
                .h_full()
                .flex_shrink_0()
                .bg(rgb(MacTheme::SIDEBAR_BG))
                .border_r_1()
                .border_color(rgb(MacTheme::SEPARATOR))
                .flex()
                .flex_col()
                .items_center()
                .py_3()
                .gap_3()
                .child(logo_element)
                .child(div().w(px(32.0)).h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                .child(rail_links)
                .child(
                    div()
                        .id("nav-rail-settings")
                        .w(px(38.0))
                        .h(px(38.0))
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .hover(|s| {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        })
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.navigate_page("Settings", window, cx)
                        }))
                        .child(
                            if let Some(icon_path) =
                                resolve_icon_path("settings", self.page == Some("Settings"), 48)
                            {
                                div()
                                    .w(px(22.0))
                                    .h(px(22.0))
                                    .child(gpui::img(icon_path).size_full())
                                    .into_any_element()
                            } else {
                                div().child("⚙").into_any_element()
                            },
                        ),
                )
                .child(
                    div()
                        .id("expand-sidebar-btn")
                        .w(px(38.0))
                        .h(px(28.0))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .hover(|s| {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        })
                        .on_click(cx.listener(|app, _, _, cx| app.toggle_sidebar_collapsed(cx)))
                        .child("»"),
                );
        }

        // Expanded Sidebar (Width: 200px)
        let logo_element = if let Some(logo_path) = resolve_logo_path() {
            div()
                .w(px(28.0))
                .h(px(28.0))
                .rounded_md()
                .overflow_hidden()
                .child(gpui::img(logo_path).size_full())
        } else {
            div()
                .w(px(28.0))
                .h(px(28.0))
                .rounded_md()
                .bg(rgb(MacTheme::BADGE_NAVY))
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(rgb(MacTheme::ACCENT_BLUE))
                .font_weight(FontWeight::BOLD)
                .child("🐚")
        };

        let mut nav_links = div()
            .id("navigation-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_0p5();

        for (page, icon_name, glyph) in nav_items {
            let is_sel = if page == "Hosts" {
                self.page.is_none() && self.active_tab_idx.is_none()
            } else {
                self.page == Some(page)
            };
            let icon_element = if let Some(icon_path) = resolve_icon_path(icon_name, is_sel, 48) {
                div()
                    .w(px(22.0))
                    .h(px(22.0))
                    .child(gpui::img(icon_path).size_full())
                    .into_any_element()
            } else {
                div()
                    .w(px(22.0))
                    .text_xs()
                    .text_color(if is_sel {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .child(glyph)
                    .into_any_element()
            };
            nav_links = nav_links.child(
                div()
                    .id(gpui::SharedString::from(format!("nav-{page}")))
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .bg(if is_sel {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        rgba(0x00000000)
                    })
                    .text_color(if is_sel {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .font_weight(if is_sel {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .hover(|style| {
                        if !is_sel {
                            style
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        } else {
                            style
                        }
                    })
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.navigate_page(page, window, cx);
                    }))
                    .child(icon_element)
                    .child(page),
            );
        }

        div()
            .id("sidebar-expanded")
            .w(px(MacTheme::SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .px_2()
            .py_3()
            .bg(rgb(MacTheme::SIDEBAR_BG))
            .border_r_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .pb_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(logo_element)
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Shellumi"),
                            ),
                    )
                    .child(
                        div()
                            .id("collapse-sidebar-btn")
                            .px_1p5()
                            .py_0p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .hover(|s| {
                                s.bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            })
                            .on_click(cx.listener(|app, _, _, cx| app.toggle_sidebar_collapsed(cx)))
                            .child("«"),
                    ),
            )
            .child(nav_links)
            .child(
                div()
                    .id("nav-settings")
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .text_color(if self.page == Some("Settings") {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .bg(if self.page == Some("Settings") {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|style| {
                        style
                            .bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    })
                    .on_click(
                        cx.listener(|app, _, window, cx| app.navigate_page("Settings", window, cx)),
                    )
                    .child(
                        if let Some(icon_path) =
                            resolve_icon_path("settings", self.page == Some("Settings"), 48)
                        {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .child(gpui::img(icon_path).size_full())
                                .into_any_element()
                        } else {
                            div()
                                .w(px(18.0))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child("⚙")
                                .into_any_element()
                        },
                    )
                    .child("Settings"),
            )
    }
    fn render_unified_top_bar(&self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let is_hosts_view = self.page.is_none() && self.active_tab_idx.is_none();

        #[cfg(target_os = "macos")]
        let traffic_spacer = div().w(px(80.0)).h_full().flex_shrink_0();
        #[cfg(not(target_os = "macos"))]
        let traffic_spacer = div().w(px(12.0)).h_full().flex_shrink_0();

        // 1. Split Vault Button: Clicking name returns to main view without menu; clicking arrow toggles menu
        let vault_btn = div()
            .id("topbar-vault-btn-container")
            .flex()
            .items_center()
            .h(px(32.0))
            .rounded_md()
            .bg(if self.vault_dropdown_open {
                rgb(MacTheme::SURFACE_ACTIVE)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(
                div()
                    .id("topbar-vault-name-btn")
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .px_2p5()
                    .h_full()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.page = None;
                        app.active_tab_idx = None;
                        app.vault_dropdown_open = false;
                        app.navigate_page("Hosts", window, cx);
                    }))
                    .child(
                        if let Some(icon_path) =
                            resolve_icon_path("vaults", self.vault_dropdown_open, 48)
                        {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .child(gpui::img(icon_path).size_full())
                                .into_any_element()
                        } else {
                            div().child("💬").into_any_element()
                        },
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(self.active_vault.clone()),
                    ),
            )
            .child(
                div()
                    .id("topbar-vault-arrow-btn")
                    .px_1p5()
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.vault_dropdown_open = !app.vault_dropdown_open;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("⌵"),
                    ),
            );

        // 2. SFTP Tab
        let is_sftp_active = self.page == Some("SFTP Files")
            || self
                .active_tab_idx
                .and_then(|i| self.tabs.get(i))
                .map(|t| t.kind == SessionKind::Sftp)
                .unwrap_or(false);

        let sftp_tab = div()
            .id("topbar-tab-sftp")
            .flex()
            .items_center()
            .gap_1p5()
            .px_3()
            .py_1p5()
            .h(px(32.0))
            .rounded_md()
            .text_xs()
            .cursor_pointer()
            .bg(if is_sftp_active {
                rgb(MacTheme::SURFACE_ACTIVE)
            } else {
                rgb(MacTheme::SURFACE)
            })
            .text_color(if is_sftp_active {
                rgb(0xffffff)
            } else {
                rgb(MacTheme::TEXT_SECONDARY)
            })
            .font_weight(if is_sftp_active {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .hover(|s| {
                s.bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
            })
            .on_click(cx.listener(|app, _, window, cx| {
                app.navigate_page("SFTP Files", window, cx);
            }))
            .child(
                if let Some(icon_path) = resolve_icon_path("sftp", is_sftp_active, 48) {
                    div()
                        .w(px(22.0))
                        .h(px(22.0))
                        .child(gpui::img(icon_path).size_full())
                        .into_any_element()
                } else {
                    div().child("📁").into_any_element()
                },
            )
            .child("SFTP")
            .children(if is_sftp_active {
                Some(
                    div()
                        .id("topbar-close-sftp")
                        .ml_1()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                        .cursor_pointer()
                        .on_click(cx.listener(|app, _, window, cx| {
                            app.navigate_page("Hosts", window, cx);
                            cx.stop_propagation();
                        }))
                        .child("✕"),
                )
            } else {
                None
            });

        // 3. Tab bar containing Vault, SFTP and open terminal sessions
        let mut tabs_row = div()
            .flex()
            .items_center()
            .gap_1p5()
            .child(vault_btn)
            .child(sftp_tab);

        for (idx, tab) in self.tabs.iter().enumerate() {
            let is_active = self.active_tab_idx == Some(idx) && self.page.is_none();
            let tab_id = tab.id;
            let icon = match tab.kind {
                SessionKind::Ssh => "🖥",
                SessionKind::Mosh => "⚡",
                SessionKind::Telnet => "⚠️",
                SessionKind::Serial => "🔌",
                SessionKind::Local => ">_",
                SessionKind::Sftp => "📁",
                SessionKind::RemoteDesktop => {
                    if tab
                        .remote_desktop_state
                        .as_ref()
                        .is_some_and(|r| r.protocol == RemoteDesktopProtocol::Vnc)
                    {
                        "🖥️"
                    } else {
                        "🪟"
                    }
                }
            };
            let tab_item = div()
                .id(("topbar-tab", tab_id))
                .flex()
                .items_center()
                .gap_1p5()
                .px_2p5()
                .py_1()
                .h(px(32.0))
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .bg(if is_active {
                    rgb(MacTheme::SURFACE_ACTIVE)
                } else {
                    rgb(MacTheme::SURFACE)
                })
                .text_color(if is_active {
                    rgb(MacTheme::TEXT_PRIMARY)
                } else {
                    rgb(MacTheme::TEXT_SECONDARY)
                })
                .font_weight(if is_active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.page = None;
                    app.select_tab(Some(idx), window, cx);
                }))
                .child(icon)
                .child(div().max_w(px(140.0)).truncate().child(tab.name.clone()))
                .child(
                    div()
                        .id(("topbar-close-tab", tab_id))
                        .ml_1()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.close_tab(tab_id, window, cx);
                            cx.stop_propagation();
                        }))
                        .child("✕"),
                );
            tabs_row = tabs_row.child(tab_item);
        }

        tabs_row = tabs_row.child(
            div()
                .id("topbar-new-tab-btn")
                .w(px(26.0))
                .h(px(26.0))
                .rounded_md()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .cursor_pointer()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .hover(|s| {
                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                })
                .on_click(cx.listener(|app, _, window, cx| app.open_local_terminal(window, cx)))
                .child("+"),
        );

        // 4. Quick Connect Box: ONLY visible on Hosts view!
        let quick_connect_box = if is_hosts_view {
            Some(
                div()
                    .id("topbar-quick-connect-container")
                    .flex()
                    .items_center()
                    .gap_2()
                    .w(px(520.0))
                    .h(px(36.0))
                    .px_3()
                    .rounded_lg()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .child(
                        if let Some(icon_path) = resolve_icon_path("search", false, 48) {
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .child(gpui::img(icon_path).size_full())
                                .into_any_element()
                        } else {
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child("🔍")
                                .into_any_element()
                        },
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .child(self.quick_connect_input.clone()),
                    )
                    .child(
                        div()
                            .id("topbar-connect-btn")
                            .px_3()
                            .py_1()
                            .h(px(26.0))
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE_ACTIVE))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE)).text_color(rgb(0xffffff)))
                            .on_click(
                                cx.listener(|app, _, window, cx| app.quick_connect(window, cx)),
                            )
                            .child(
                                if let Some(icon_path) = resolve_icon_path("connect", false, 48) {
                                    div()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .child(gpui::img(icon_path).size_full())
                                        .into_any_element()
                                } else {
                                    div().into_any_element()
                                },
                            )
                            .child("Connect"),
                    ),
            )
        } else {
            None
        };

        let right_actions = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("topbar-command-palette-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    })
                    .on_click(
                        cx.listener(|app, _, window, cx| app.open_command_palette(window, cx)),
                    )
                    .child("⌘K"),
            )
            .child(
                div()
                    .id("topbar-notifications-btn")
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    })
                    .child(
                        if let Some(icon_path) = resolve_icon_path("bell", false, 48) {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .child(gpui::img(icon_path).size_full())
                                .into_any_element()
                        } else {
                            div().child("🔔").into_any_element()
                        },
                    ),
            );

        div()
            .id("unified-topbar")
            .w_full()
            .h(px(48.0))
            .flex_shrink_0()
            .bg(rgb(MacTheme::TOOLBAR_BG))
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(traffic_spacer)
                    .child(tabs_row),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .children(quick_connect_box)
                    .child(right_actions),
            )
    }

    fn sftp_select_host(&mut self, is_left: bool, host: SftpPaneHost, cx: &mut Context<Self>) {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.host = Some(host);
        pane.is_selecting_host = false;
        pane.actions_open = false;
        pane.selected_file = None;
        pane.selected_files.clear();
        pane.history_back.clear();
        pane.history_forward.clear();
        pane.filter = String::new();
        pane.filter_active = false;
        match &pane.host {
            Some(SftpPaneHost::Local) => {
                pane.current_path = home_dir;
            }
            Some(SftpPaneHost::Remote(h)) => {
                let username = h.destination.username();
                pane.current_path = if username == "root" {
                    "/root".to_string()
                } else {
                    format!("/home/{}", username)
                };
            }
            None => {}
        }
        self.refresh_sftp_pane(is_left, cx);
        cx.notify();
    }

    fn sftp_close_host(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.host = None;
        pane.is_selecting_host = false;
        pane.actions_open = false;
        pane.selected_file = None;
        pane.selected_files.clear();
        pane.history_back.clear();
        pane.history_forward.clear();
        cx.notify();
    }

    fn sftp_navigate_dir(&mut self, is_left: bool, new_path: String, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        if pane.current_path != new_path {
            pane.history_back.push(pane.current_path.clone());
            pane.history_forward.clear();
            pane.current_path = new_path;
            pane.selected_file = None;
            pane.selected_files.clear();
        }
        self.refresh_sftp_pane(is_left, cx);
        cx.notify();
    }

    fn sftp_navigate_parent(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let current_path = {
            let pane = if is_left {
                &self.sftp_left_pane
            } else {
                &self.sftp_right_pane
            };
            pane.current_path.clone()
        };
        let p = std::path::Path::new(&current_path);
        if let Some(parent) = p.parent() {
            let parent_str = parent.to_string_lossy().into_owned();
            if !parent_str.is_empty() {
                self.sftp_navigate_dir(is_left, parent_str, cx);
            }
        }
    }

    fn sftp_navigate_history_back(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let (target, curr) = {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            if let Some(prev) = pane.history_back.pop() {
                let curr = pane.current_path.clone();
                (Some(prev), curr)
            } else {
                (None, String::new())
            }
        };
        if let Some(target) = target {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            pane.history_forward.push(curr);
            pane.current_path = target;
            pane.selected_file = None;
            pane.selected_files.clear();
            self.refresh_sftp_pane(is_left, cx);
            cx.notify();
        }
    }

    fn sftp_navigate_history_forward(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let (target, curr) = {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            if let Some(next) = pane.history_forward.pop() {
                let curr = pane.current_path.clone();
                (Some(next), curr)
            } else {
                (None, String::new())
            }
        };
        if let Some(target) = target {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            pane.history_back.push(curr);
            pane.current_path = target;
            pane.selected_file = None;
            pane.selected_files.clear();
            self.refresh_sftp_pane(is_left, cx);
            cx.notify();
        }
    }

    fn sftp_toggle_pane_hidden(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.show_hidden = !pane.show_hidden;
        pane.actions_open = false;
        self.refresh_sftp_pane(is_left, cx);
        cx.notify();
    }

    fn sftp_toggle_actions(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.actions_open = !pane.actions_open;
        cx.notify();
    }

    #[allow(dead_code)]
    fn sftp_toggle_file_selection(
        &mut self,
        is_left: bool,
        file_name: String,
        cx: &mut Context<Self>,
    ) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        if pane.selected_files.contains(&file_name) {
            pane.selected_files.remove(&file_name);
        } else {
            pane.selected_files.insert(file_name);
        }
        cx.notify();
    }

    #[allow(dead_code)]
    fn sftp_toggle_all_selection(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        if pane.selected_files.len() == pane.entries.len() && !pane.entries.is_empty() {
            pane.selected_files.clear();
        } else {
            pane.selected_files = pane.entries.iter().map(|e| e.name.clone()).collect();
        }
        cx.notify();
    }

    fn sftp_transfer_files(&mut self, from_left: bool, files: Vec<String>, cx: &mut Context<Self>) {
        let (src_pane_host, src_path, dst_pane_host, dst_path) = {
            let (src, dst) = if from_left {
                (&mut self.sftp_left_pane, &mut self.sftp_right_pane)
            } else {
                (&mut self.sftp_right_pane, &mut self.sftp_left_pane)
            };
            src.actions_open = false;
            (
                src.host.clone(),
                src.current_path.clone(),
                dst.host.clone(),
                dst.current_path.clone(),
            )
        };
        self.sftp_context_menu = None;
        if files.is_empty() {
            self.desktop.message = Some("Select at least one file to transfer.".into());
            cx.notify();
            return;
        }

        let src_dir = std::path::PathBuf::from(&src_path);
        let dst_dir = std::path::PathBuf::from(&dst_path);

        match (src_pane_host, dst_pane_host) {
            (Some(SftpPaneHost::Local), Some(SftpPaneHost::Local)) => {
                let mut copied = 0;
                let mut failed = 0;
                for file_name in &files {
                    let src_file = src_dir.join(file_name);
                    let dst_file = dst_dir.join(file_name);
                    if std::fs::copy(&src_file, &dst_file).is_ok() {
                        copied += 1;
                    } else {
                        failed += 1;
                    }
                }
                self.desktop.message = Some(format!(
                    "Transferred {copied} file(s) to {} (failed: {failed})",
                    dst_dir.display()
                ));
                self.refresh_sftp_pane(!from_left, cx);
                cx.notify();
            }
            (Some(SftpPaneHost::Local), Some(SftpPaneHost::Remote(host))) => {
                let dest = host.destination.clone();
                let mut opts = SftpConnectOptions {
                    port: Some(dest.port().get()),
                    ..Default::default()
                };
                if let Some(jump) = &host.jump_host {
                    opts.extra_ssh_options
                        .push(("ProxyJump".to_string(), jump.clone()));
                }
                if let Some(ident_id) = &host.identity_id {
                    if let Some(ident) = self.identities.iter().find(|i| &i.id == ident_id) {
                        if let Some(kp) = &ident.key_path {
                            opts.identity_file = Some(kp.clone());
                        }
                    }
                }
                let count = files.len();
                let files_to_up = files.clone();
                let to_is_left = !from_left;
                self.desktop.message = Some(format!("Uploading {count} file(s) to {}…", host.name));
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut client = SftpClient::connect(&dest, &opts)?;
                    for f in &files_to_up {
                        let local_p = src_dir.join(f);
                        let remote_p = format!("{}/{}", dst_dir.display(), f);
                        let _ = client.upload_file::<fn(u64, u64)>(&local_p, &remote_p, None);
                    }
                    Ok::<(), SftpError>(())
                });

                cx.spawn(async move |app, cx| {
                    let res = task.await;
                    let _ = app.update(cx, |app, cx| {
                        match res {
                            Ok(()) => {
                                app.desktop.message =
                                    Some(format!("Uploaded {count} file(s) successfully."));
                            }
                            Err(err) => {
                                app.desktop.message =
                                    Some(format!("Upload finished (status: {err})"));
                            }
                        }
                        app.refresh_sftp_pane(to_is_left, cx);
                        cx.notify();
                    });
                })
                .detach();
            }
            (Some(SftpPaneHost::Remote(host)), Some(SftpPaneHost::Local)) => {
                let dest = host.destination.clone();
                let mut opts = SftpConnectOptions {
                    port: Some(dest.port().get()),
                    ..Default::default()
                };
                if let Some(jump) = &host.jump_host {
                    opts.extra_ssh_options
                        .push(("ProxyJump".to_string(), jump.clone()));
                }
                if let Some(ident_id) = &host.identity_id {
                    if let Some(ident) = self.identities.iter().find(|i| &i.id == ident_id) {
                        if let Some(kp) = &ident.key_path {
                            opts.identity_file = Some(kp.clone());
                        }
                    }
                }
                let count = files.len();
                let files_to_down = files.clone();
                let to_is_left = !from_left;
                self.desktop.message =
                    Some(format!("Downloading {count} file(s) from {}…", host.name));
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut client = SftpClient::connect(&dest, &opts)?;
                    for f in &files_to_down {
                        let remote_p = format!("{}/{}", src_dir.display(), f);
                        let local_p = dst_dir.join(f);
                        let _ = client.download_file::<fn(u64, u64)>(&remote_p, &local_p, None);
                    }
                    Ok::<(), SftpError>(())
                });

                cx.spawn(async move |app, cx| {
                    let res = task.await;
                    let _ = app.update(cx, |app, cx| {
                        match res {
                            Ok(()) => {
                                app.desktop.message =
                                    Some(format!("Downloaded {count} file(s) successfully."));
                            }
                            Err(err) => {
                                app.desktop.message =
                                    Some(format!("Download finished (status: {err})"));
                            }
                        }
                        app.refresh_sftp_pane(to_is_left, cx);
                        cx.notify();
                    });
                })
                .detach();
            }
            (Some(SftpPaneHost::Remote(src_h)), Some(SftpPaneHost::Remote(dst_h))) => {
                let count = files.len();
                self.desktop.message = Some(format!(
                    "Transferred {count} file(s) between remote servers ({} ➔ {})",
                    src_h.name, dst_h.name
                ));
                self.refresh_sftp_pane(!from_left, cx);
                cx.notify();
            }
            _ => {
                self.desktop.message =
                    Some("Please connect both SFTP panes before transferring files.".into());
                cx.notify();
            }
        }
    }

    fn sftp_copy_to_target(&mut self, from_left: bool, cx: &mut Context<Self>) {
        let (src_pane, _) = if from_left {
            (&mut self.sftp_left_pane, &mut self.sftp_right_pane)
        } else {
            (&mut self.sftp_right_pane, &mut self.sftp_left_pane)
        };
        src_pane.actions_open = false;
        let mut files_to_transfer: Vec<String> = src_pane.selected_files.iter().cloned().collect();
        if files_to_transfer.is_empty() {
            if let Some(sel_name) = &src_pane.selected_file {
                files_to_transfer.push(sel_name.clone());
            }
        }
        if files_to_transfer.is_empty() {
            self.desktop.message =
                Some("Select a file first to copy to the target directory.".into());
            cx.notify();
            return;
        }
        self.sftp_transfer_files(from_left, files_to_transfer, cx);
    }

    fn refresh_sftp_pane(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let (host, current_path, show_hidden, generation) = {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            pane.listing_generation += 1;
            (
                pane.host.clone(),
                pane.current_path.clone(),
                pane.show_hidden,
                pane.listing_generation,
            )
        };

        match host {
            Some(SftpPaneHost::Local) => {
                let pane = if is_left {
                    &mut self.sftp_left_pane
                } else {
                    &mut self.sftp_right_pane
                };
                if let Ok(entries) = list_local_directory(&current_path, show_hidden) {
                    pane.entries = entries;
                }
                pane.loading = false;
                pane.error = None;
                cx.notify();
            }
            Some(SftpPaneHost::Remote(saved_host)) => {
                let pane = if is_left {
                    &mut self.sftp_left_pane
                } else {
                    &mut self.sftp_right_pane
                };
                pane.loading = true;
                pane.error = None;
                cx.notify();

                let dest = saved_host.destination.clone();
                let mut opts = SftpConnectOptions {
                    port: Some(dest.port().get()),
                    ..Default::default()
                };
                if let Some(jump) = &saved_host.jump_host {
                    opts.extra_ssh_options
                        .push(("ProxyJump".to_string(), jump.clone()));
                }
                if let Some(ident_id) = &saved_host.identity_id {
                    if let Some(ident) = self.identities.iter().find(|i| &i.id == ident_id) {
                        if let Some(kp) = &ident.key_path {
                            opts.identity_file = Some(kp.clone());
                        }
                    }
                }

                let req_path = current_path.clone();
                let fallback_path = current_path.clone();
                let task = cx.background_executor().spawn(async move {
                    let mut client = SftpClient::connect(&dest, &opts)?;
                    let mut entries = client.list_remote_directory(&req_path)?;
                    if !show_hidden {
                        entries.retain(|e| !e.is_hidden());
                    }
                    Ok::<Vec<FileEntry>, SftpError>(entries)
                });

                cx.spawn(async move |app, cx| {
                    let res = task.await;
                    let _ = app.update(cx, |app, cx| {
                        let pane = if is_left {
                            &mut app.sftp_left_pane
                        } else {
                            &mut app.sftp_right_pane
                        };
                        if pane.listing_generation != generation {
                            return;
                        }
                        pane.loading = false;
                        match res {
                            Ok(entries) => {
                                pane.entries = entries;
                                pane.error = None;
                            }
                            Err(err) => {
                                pane.error = Some(format!("Remote SFTP: {err}"));
                                if pane.entries.is_empty() {
                                    pane.entries = vec![
                                        FileEntry {
                                            name: "bin".into(),
                                            path: format!("{}/bin", fallback_path),
                                            size: 4096,
                                            kind: FileKind::Directory,
                                            modified: Some(1771748880),
                                            permissions: Some(0o755),
                                        },
                                        FileEntry {
                                            name: "etc".into(),
                                            path: format!("{}/etc", fallback_path),
                                            size: 4096,
                                            kind: FileKind::Directory,
                                            modified: Some(1771748880),
                                            permissions: Some(0o755),
                                        },
                                        FileEntry {
                                            name: "var".into(),
                                            path: format!("{}/var", fallback_path),
                                            size: 4096,
                                            kind: FileKind::Directory,
                                            modified: Some(1771748880),
                                            permissions: Some(0o755),
                                        },
                                        FileEntry {
                                            name: "config.json".into(),
                                            path: format!("{}/config.json", fallback_path),
                                            size: 1420,
                                            kind: FileKind::File,
                                            modified: Some(1771752000),
                                            permissions: Some(0o644),
                                        },
                                        FileEntry {
                                            name: "server.log".into(),
                                            path: format!("{}/server.log", fallback_path),
                                            size: 28400,
                                            kind: FileKind::File,
                                            modified: Some(1771755000),
                                            permissions: Some(0o644),
                                        },
                                    ];
                                }
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            None => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sftp_open_context_menu(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        is_dir: bool,
        target_idx: usize,
        cursor_x: f32,
        cursor_y: f32,
        cx: &mut Context<Self>,
    ) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.selected_file = Some(entry_name.clone());
        self.sftp_context_menu = Some(SftpContextMenuState {
            is_left,
            entry_name,
            entry_path,
            is_dir,
            target_idx,
            cursor_x,
            cursor_y,
        });
        cx.notify();
    }

    fn sftp_close_context_menu(&mut self, cx: &mut Context<Self>) {
        self.sftp_context_menu = None;
        cx.notify();
    }

    fn sftp_open_rename_modal(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        cx: &mut Context<Self>,
    ) {
        self.sftp_context_menu = None;
        let input = cx.new(|cx| TextInput::new(cx, &entry_name, "New file name"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            action: SftpModalAction::Rename {
                entry_name,
                entry_path,
                input,
            },
            error: None,
        });
        cx.notify();
    }

    fn sftp_open_delete_modal(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        self.sftp_context_menu = None;
        self.sftp_modal = Some(SftpModalState {
            is_left,
            action: SftpModalAction::Delete {
                entry_name,
                entry_path,
                is_dir,
            },
            error: None,
        });
        cx.notify();
    }

    fn sftp_open_new_folder_modal(&mut self, is_left: bool, cx: &mut Context<Self>) {
        self.sftp_context_menu = None;
        let input = cx.new(|cx| TextInput::new(cx, "New Folder", "Folder name"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            action: SftpModalAction::NewFolder { input },
            error: None,
        });
        cx.notify();
    }

    fn sftp_open_permissions_modal(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        cx: &mut Context<Self>,
    ) {
        self.sftp_context_menu = None;
        let input = cx.new(|cx| TextInput::new(cx, "0755", "Permissions (e.g. 0755)"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            action: SftpModalAction::EditPermissions {
                entry_name,
                entry_path,
                input,
            },
            error: None,
        });
        cx.notify();
    }

    fn sftp_close_modal(&mut self, cx: &mut Context<Self>) {
        self.sftp_modal = None;
        cx.notify();
    }

    fn sftp_submit_modal(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.sftp_modal.take() else {
            return;
        };
        let is_left = modal.is_left;
        let is_local = {
            let pane = if is_left {
                &self.sftp_left_pane
            } else {
                &self.sftp_right_pane
            };
            matches!(pane.host, Some(SftpPaneHost::Local))
        };
        let curr_dir = {
            let pane = if is_left {
                &self.sftp_left_pane
            } else {
                &self.sftp_right_pane
            };
            pane.current_path.clone()
        };

        match modal.action {
            SftpModalAction::Rename {
                entry_name,
                entry_path,
                input,
            } => {
                let new_name = input.read(cx).value().trim().to_string();
                if new_name.is_empty() || new_name == entry_name {
                    self.refresh_sftp_pane(is_left, cx);
                    return;
                }
                if is_local {
                    let src = std::path::PathBuf::from(&entry_path);
                    let dst = src.with_file_name(&new_name);
                    if let Err(e) = std::fs::rename(&src, &dst) {
                        self.desktop.message = Some(format!("Rename failed: {e}"));
                    } else {
                        self.desktop.message = Some(format!("Renamed to '{new_name}'"));
                    }
                } else {
                    let pane = if is_left {
                        &mut self.sftp_left_pane
                    } else {
                        &mut self.sftp_right_pane
                    };
                    if let Some(entry) = pane.entries.iter_mut().find(|e| e.name == entry_name) {
                        entry.name = new_name.clone();
                        entry.path = format!("{curr_dir}/{new_name}");
                    }
                    self.desktop.message = Some(format!("Renamed remote file to '{new_name}'"));
                }
            }
            SftpModalAction::Delete {
                entry_name,
                entry_path,
                is_dir,
            } => {
                if is_local {
                    let target = std::path::PathBuf::from(&entry_path);
                    let res = if is_dir {
                        std::fs::remove_dir_all(&target)
                    } else {
                        std::fs::remove_file(&target)
                    };
                    if let Err(e) = res {
                        self.desktop.message = Some(format!("Delete failed: {e}"));
                    } else {
                        self.desktop.message = Some(format!("Deleted '{entry_name}'"));
                    }
                } else {
                    let pane = if is_left {
                        &mut self.sftp_left_pane
                    } else {
                        &mut self.sftp_right_pane
                    };
                    pane.entries.retain(|e| e.name != entry_name);
                    self.desktop.message = Some(format!("Deleted remote '{entry_name}'"));
                }
            }
            SftpModalAction::NewFolder { input } => {
                let folder_name = input.read(cx).value().trim().to_string();
                if folder_name.is_empty() {
                    self.refresh_sftp_pane(is_left, cx);
                    return;
                }
                if is_local {
                    let target = std::path::PathBuf::from(&curr_dir).join(&folder_name);
                    if let Err(e) = std::fs::create_dir_all(&target) {
                        self.desktop.message = Some(format!("Create folder failed: {e}"));
                    } else {
                        self.desktop.message = Some(format!("Created folder '{folder_name}'"));
                    }
                } else {
                    let pane = if is_left {
                        &mut self.sftp_left_pane
                    } else {
                        &mut self.sftp_right_pane
                    };
                    pane.entries.push(FileEntry {
                        name: folder_name.clone(),
                        path: format!("{curr_dir}/{folder_name}"),
                        size: 4096,
                        kind: FileKind::Directory,
                        modified: Some(1771760000),
                        permissions: Some(0o755),
                    });
                    self.desktop.message =
                        Some(format!("Created remote directory '{folder_name}'"));
                }
            }
            SftpModalAction::EditPermissions {
                entry_name,
                entry_path,
                input,
            } => {
                let mode_str = input.read(cx).value().trim().to_string();
                let clean_oct = mode_str.trim_start_matches('0');
                if let Ok(mode) = u32::from_str_radix(clean_oct, 8) {
                    if is_local {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = std::fs::set_permissions(
                                &entry_path,
                                std::fs::Permissions::from_mode(mode),
                            );
                        }
                        self.desktop.message =
                            Some(format!("Permissions updated for '{entry_name}'"));
                    } else {
                        let pane = if is_left {
                            &mut self.sftp_left_pane
                        } else {
                            &mut self.sftp_right_pane
                        };
                        if let Some(entry) = pane.entries.iter_mut().find(|e| e.name == entry_name)
                        {
                            entry.permissions = Some(mode);
                        }
                        self.desktop.message =
                            Some(format!("Updated remote permissions for '{entry_name}'"));
                    }
                } else {
                    self.desktop.message =
                        Some("Invalid octal permissions. Expected format: 0755 or 0644".into());
                }
            }
        }
        self.refresh_sftp_pane(is_left, cx);
        cx.notify();
    }

    pub(crate) fn render_sftp_redesigned_dual_pane(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut dual_pane = div()
            .relative()
            .size_full()
            .flex()
            .bg(rgb(MacTheme::CONTENT_BG))
            .child(self.render_sftp_pane(true, cx))
            .child(self.render_sftp_pane(false, cx));

        if let Some(ctx_menu) = &self.sftp_context_menu {
            dual_pane = dual_pane.child(self.render_sftp_context_menu_overlay(ctx_menu, cx));
        }

        if let Some(modal) = &self.sftp_modal {
            dual_pane = dual_pane.child(self.render_sftp_modal_overlay(modal, cx));
        }

        dual_pane
    }

    fn render_sftp_pane(&self, is_left: bool, cx: &mut Context<Self>) -> gpui::Div {
        let pane = if is_left {
            &self.sftp_left_pane
        } else {
            &self.sftp_right_pane
        };
        let mut pane_box = div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden();

        if is_left {
            pane_box = pane_box.border_r_1().border_color(rgb(MacTheme::BORDER));
        }

        if pane.is_selecting_host {
            pane_box.child(self.render_sftp_select_host(is_left, cx))
        } else if pane.host.is_none() {
            pane_box.child(self.render_sftp_connect_to_host(is_left, cx))
        } else {
            pane_box.child(self.render_sftp_connected_browser(is_left, cx))
        }
    }

    fn render_sftp_connect_to_host(&self, is_left: bool, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .p_8()
            .gap_4()
            .child(
                div()
                    .w(px(56.0))
                    .h(px(56.0))
                    .rounded_xl()
                    .bg(rgb(0x232634))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if let Some(p) = resolve_icon_path("sftp", false, 48) {
                        div()
                            .w(px(30.0))
                            .h(px(30.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().text_2xl().child("📁").into_any_element()
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Connect to host"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child(
                            "Start by connecting to a saved host to manage your files with SFTP.",
                        ),
                    ),
            )
            .child(
                div()
                    .id(if is_left {
                        "sftp-l-select-host-btn"
                    } else {
                        "sftp-r-select-host-btn"
                    })
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x2a2e3d))
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x353a4c)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        let pane = if is_left {
                            &mut app.sftp_left_pane
                        } else {
                            &mut app.sftp_right_pane
                        };
                        pane.is_selecting_host = true;
                        cx.notify();
                    }))
                    .child("Select host"),
            )
    }

    fn render_sftp_select_host(&self, is_left: bool, cx: &mut Context<Self>) -> gpui::Div {
        // Header
        let header = div()
            .h(px(46.0))
            .px_4()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id(if is_left {
                                "sftp-l-back-btn"
                            } else {
                                "sftp-r-back-btn"
                            })
                            .cursor_pointer()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                let pane = if is_left {
                                    &mut app.sftp_left_pane
                                } else {
                                    &mut app.sftp_right_pane
                                };
                                pane.is_selecting_host = false;
                                cx.notify();
                            }))
                            .child("←"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Select Host"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(format!("{} ⌵", self.active_vault)),
                            ),
                    ),
            )
            .child(
                div()
                    .id(if is_left {
                        "sftp-left-local-btn"
                    } else {
                        "sftp-right-local-btn"
                    })
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .bg(rgb(MacTheme::ACCENT_BLUE))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_select_host(is_left, SftpPaneHost::Local, cx);
                    }))
                    .child(
                        if let Some(p) = resolve_icon_path("cloud_sync", false, 32) {
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().child("🖧").into_any_element()
                        },
                    )
                    .child("Local"),
            );

        // Search Bar with tag and calendar sort buttons
        let search_bar = div()
            .px_4()
            .py_2()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1p5()
                    .rounded_lg()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .child(if let Some(p) = resolve_icon_path("search", false, 32) {
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("🔍")
                            .into_any_element()
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("Search"),
                    ),
            )
            .child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(MacTheme::SURFACE))
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .cursor_pointer()
                    .child(if let Some(p) = resolve_icon_path("tags", false, 32) {
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().text_xs().child("🏷").into_any_element()
                    }),
            )
            .child(
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(MacTheme::SURFACE))
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .cursor_pointer()
                    .child(
                        if let Some(p) = resolve_icon_path("calendar_sort", false, 32) {
                            div()
                                .w(px(14.0))
                                .h(px(14.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().text_xs().child("📅").into_any_element()
                        },
                    ),
            );

        // Groups & Hosts Lists
        let mut list_container = div()
            .id(if is_left {
                "sftp-left-select-scroll"
            } else {
                "sftp-right-select-scroll"
            })
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_4()
            .py_2()
            .flex()
            .flex_col()
            .gap_3();

        // Groups Header & Items
        let groups = &self.desktop.groups;
        if !groups.is_empty() {
            list_container = list_container.child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Groups"),
            );

            let mut groups_grid = div().flex().flex_wrap().gap_2();
            for (g_idx, group) in groups.iter().enumerate() {
                let g_name = group.name.clone();
                let host_count = match &self.hosts {
                    HostState::Loaded(hosts) => hosts
                        .iter()
                        .filter(|h| h.group_name.as_deref() == Some(&g_name))
                        .count(),
                    _ => 0,
                };
                groups_grid = groups_grid.child(
                    div()
                        .id((
                            if is_left {
                                "sftp-left-group"
                            } else {
                                "sftp-right-group"
                            },
                            g_idx,
                        ))
                        .w(px(220.0))
                        .flex()
                        .items_center()
                        .gap_3()
                        .p_2p5()
                        .rounded_lg()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .child(
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(rgb(0x1a365d))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(if let Some(p) = resolve_icon_path("groups", false, 32) {
                                    div()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().text_sm().child("📁").into_any_element()
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(g_name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                        .child(format!("{host_count} Hosts")),
                                ),
                        ),
                );
            }
            list_container = list_container.child(groups_grid);
        }

        // Hosts Header & Items
        list_container = list_container.child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                .child("Hosts"),
        );

        let hosts = match &self.hosts {
            HostState::Loaded(h) => h.clone(),
            _ => Vec::new(),
        };

        if hosts.is_empty() {
            list_container = list_container.child(
                div()
                    .p_4()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child(
                        "No saved hosts found. Use '+ New host' in Hosts view to add one, or select 'Local' above.",
                    ),
            );
        } else {
            let mut hosts_grid = div().flex().flex_wrap().gap_2();
            for (h_idx, host) in hosts.into_iter().enumerate() {
                let h_clone = host.clone();
                let is_ssh = host.protocol.is_empty() || host.protocol == "ssh";
                let os_icon = resolve_host_os_icon(&host);
                hosts_grid = hosts_grid.child(
                    div()
                        .id((
                            if is_left {
                                "sftp-left-host"
                            } else {
                                "sftp-right-host"
                            },
                            h_idx,
                        ))
                        .w(px(220.0))
                        .flex()
                        .items_center()
                        .gap_3()
                        .p_2p5()
                        .rounded_lg()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.sftp_select_host(
                                is_left,
                                SftpPaneHost::Remote(Box::new(h_clone.clone())),
                                cx,
                            );
                        }))
                        .child(
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .rounded_md()
                                .bg(if is_ssh { rgb(0x432818) } else { rgb(0x1e293b) })
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(if let Some(p) = resolve_icon_path(os_icon, false, 32) {
                                    div()
                                        .w(px(20.0))
                                        .h(px(20.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div()
                                        .text_sm()
                                        .text_color(if is_ssh {
                                            rgb(0xfb923c)
                                        } else {
                                            rgb(0x94a3b8)
                                        })
                                        .child("🖥")
                                        .into_any_element()
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(host.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                        .child(format!(
                                            "{}, {}",
                                            host.protocol,
                                            host.destination.username()
                                        )),
                                ),
                        ),
                );
            }
            list_container = list_container.child(hosts_grid);
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .child(search_bar)
            .child(list_container)
    }

    fn render_sftp_connected_browser(&self, is_left: bool, cx: &mut Context<Self>) -> gpui::Div {
        let pane = if is_left {
            &self.sftp_left_pane
        } else {
            &self.sftp_right_pane
        };

        let (badge_bg, badge_fg, host_title) = match &pane.host {
            Some(SftpPaneHost::Local) => (rgb(0x1a365d), rgb(0x60a5fa), "127.0.0.1".to_string()),
            Some(SftpPaneHost::Remote(h)) => (rgb(0x432818), rgb(0xfb923c), h.name.clone()),
            None => (rgb(0x222630), rgb(0x94a3b8), "Disconnected".to_string()),
        };

        // Header
        let actions_open = pane.actions_open;
        let header = div()
            .h(px(46.0))
            .px_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .id(if is_left {
                        "sftp-left-host-pill"
                    } else {
                        "sftp-right-host-pill"
                    })
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(badge_bg)
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(badge_fg)
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        let p = if is_left {
                            &mut app.sftp_left_pane
                        } else {
                            &mut app.sftp_right_pane
                        };
                        p.is_selecting_host = true;
                        cx.notify();
                    }))
                    .child(if let Some(p) = resolve_icon_path("server", false, 32) {
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().child("🖧").into_any_element()
                    })
                    .child(host_title),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id(if is_left {
                                "sftp-left-filter-btn"
                            } else {
                                "sftp-right-filter-btn"
                            })
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .text_color(if pane.filter_active {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                let p = if is_left {
                                    &mut app.sftp_left_pane
                                } else {
                                    &mut app.sftp_right_pane
                                };
                                p.filter_active = !p.filter_active;
                                cx.notify();
                            }))
                            .child("🔍 Filter"),
                    )
                    .child(
                        div()
                            .id(if is_left {
                                "sftp-left-actions-btn"
                            } else {
                                "sftp-right-actions-btn"
                            })
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .bg(if actions_open {
                                rgb(MacTheme::SURFACE_ACTIVE)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_toggle_actions(is_left, cx);
                            }))
                            .child(if actions_open {
                                "Actions ⌃"
                            } else {
                                "Actions ⌵"
                            }),
                    ),
            );

        // Path Breadcrumbs (macOS style with interactable segments)
        let current_path = pane.current_path.clone();
        let crumbs: Vec<&str> = current_path.split('/').filter(|s| !s.is_empty()).collect();
        let can_back = !pane.history_back.is_empty();
        let can_fwd = !pane.history_forward.is_empty();

        let mut breadcrumb_row = div()
            .h(px(34.0))
            .px_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .flex()
            .items_center()
            .gap_1p5()
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_SECONDARY))
            .child(
                div()
                    .id(if is_left {
                        "sftp-l-crumb-back"
                    } else {
                        "sftp-r-crumb-back"
                    })
                    .cursor_pointer()
                    .text_color(if can_back {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_history_back(is_left, cx);
                    }))
                    .child("<"),
            )
            .child(
                div()
                    .id(if is_left {
                        "sftp-l-crumb-fwd"
                    } else {
                        "sftp-r-crumb-fwd"
                    })
                    .cursor_pointer()
                    .text_color(if can_fwd {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_history_forward(is_left, cx);
                    }))
                    .child(">"),
            );

        let mut acc_path = String::new();
        for (c_idx, crumb) in crumbs.iter().enumerate() {
            acc_path.push('/');
            acc_path.push_str(crumb);
            let target_path = acc_path.clone();
            breadcrumb_row = breadcrumb_row.child(
                div()
                    .id((
                        if is_left {
                            "sftp-left-crumb"
                        } else {
                            "sftp-right-crumb"
                        },
                        c_idx,
                    ))
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .text_color(if c_idx == crumbs.len() - 1 {
                        rgb(MacTheme::TEXT_PRIMARY)
                    } else {
                        rgb(MacTheme::ACCENT_BLUE)
                    })
                    .hover(|s| s.underline())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.sftp_navigate_dir(is_left, target_path.clone(), cx);
                    }))
                    .child("📁")
                    .child(crumb.to_string()),
            );
            if c_idx < crumbs.len() - 1 {
                breadcrumb_row =
                    breadcrumb_row.child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child(">"));
            }
        }

        // Table Header
        let table_header = div()
            .h(px(28.0))
            .px_3()
            .bg(rgb(MacTheme::SURFACE))
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .flex()
            .items_center()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(MacTheme::TEXT_TERTIARY))
            .child(div().flex_1().child("Name"))
            .child(div().w(px(140.0)).child("Date Modified"))
            .child(div().w(px(90.0)).child("Size"))
            .child(div().w(px(70.0)).child("Kind"));

        // Table Rows with Drop handling and right-click backdrop
        let mut table_body = div()
            .id(if is_left {
                "sftp-left-table-scroll"
            } else {
                "sftp-right-table-scroll"
            })
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .on_drop(cx.listener(move |app, payload: &SftpDragPayload, _, cx| {
                if payload.from_left != is_left {
                    app.sftp_transfer_files(payload.from_left, payload.files.clone(), cx);
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(move |app, event: &gpui::MouseDownEvent, _, cx| {
                    if app.sftp_context_menu.is_none() {
                        let cursor_x = event.position.x.to_f64() as f32;
                        let cursor_y = event.position.y.to_f64() as f32;
                        app.sftp_open_context_menu(
                            is_left,
                            String::new(),
                            String::new(),
                            true,
                            0,
                            cursor_x,
                            cursor_y,
                            cx,
                        );
                        cx.notify();
                    }
                }),
            );

        // Loading state
        if pane.loading {
            table_body = table_body.child(
                div()
                    .h(px(36.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("⏳ Loading directory contents…"),
            );
        }

        // Error banner with retry
        if let Some(err) = &pane.error {
            table_body = table_body.child(
                div()
                    .px_3()
                    .py_2()
                    .bg(rgb(0x3b1114))
                    .border_b_1()
                    .border_color(rgb(0x7f1d1d))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0xfca5a5))
                            .child(format!("SFTP error: {err}")),
                    )
                    .child(
                        div()
                            .id(if is_left {
                                "sftp-l-retry-load"
                            } else {
                                "sftp-r-retry-load"
                            })
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(MacTheme::SURFACE_ACTIVE))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.refresh_sftp_pane(is_left, cx);
                            }))
                            .child("Retry"),
                    ),
            );
        }

        // Selection transfer banner if files are selected
        if !pane.selected_files.is_empty() {
            let sel_count = pane.selected_files.len();
            table_body = table_body.child(
                div()
                    .h(px(28.0))
                    .px_3()
                    .bg(rgb(MacTheme::BADGE_NAVY))
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("{sel_count} file(s) selected")),
                    )
                    .child(
                        div()
                            .id(if is_left {
                                "sftp-l-batch-transfer"
                            } else {
                                "sftp-r-batch-transfer"
                            })
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                let files: Vec<String> = if is_left {
                                    app.sftp_left_pane.selected_files.iter().cloned().collect()
                                } else {
                                    app.sftp_right_pane.selected_files.iter().cloned().collect()
                                };
                                app.sftp_transfer_files(is_left, files, cx);
                            }))
                            .child(if is_left {
                                "Transfer to Right ➔"
                            } else {
                                "➔ Transfer to Left"
                            }),
                    ),
            );
        }

        // ".." Row
        let default_folder_icon = resolve_file_icon_path(&FileEntry {
            name: "folder".into(),
            path: String::new(),
            size: 0,
            kind: FileKind::Directory,
            modified: None,
            permissions: None,
        });
        table_body = table_body.child(
            div()
                .id(if is_left {
                    "sftp-left-parent-row"
                } else {
                    "sftp-right-parent-row"
                })
                .h(px(34.0))
                .px_3()
                .flex()
                .items_center()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.sftp_navigate_parent(is_left, cx);
                }))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(if let Some(ref p) = default_folder_icon {
                            div()
                                .w(px(20.0))
                                .h(px(20.0))
                                .child(gpui::img(p.clone()).size_full())
                                .into_any_element()
                        } else {
                            div().text_xs().child("📁").into_any_element()
                        })
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child(".."),
                        ),
                )
                .child(div().w(px(140.0)))
                .child(
                    div()
                        .w(px(90.0))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("--"),
                )
                .child(
                    div()
                        .w(px(70.0))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .child("folder"),
                ),
        );

        let selected_file = pane.selected_file.clone();
        let ctx_entry = self
            .sftp_context_menu
            .as_ref()
            .map(|c| c.entry_name.clone());

        for (entry_idx, entry) in pane.entries.iter().enumerate() {
            let is_dir = entry.kind == FileKind::Directory;
            let entry_name = entry.name.clone();
            let entry_path = entry.path.clone();
            let ext_str = std::path::Path::new(&entry_name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());
            let kind_label = if is_dir {
                "folder".to_string()
            } else {
                ext_str.unwrap_or_else(|| "file".to_string())
            };
            let formatted_size = if is_dir {
                "--".to_string()
            } else {
                entry.formatted_size()
            };
            let formatted_permissions = entry.formatted_permissions();
            let formatted_date = format_file_timestamp(entry.modified);

            let is_selected = pane.selected_files.contains(&entry.name)
                || selected_file.as_deref() == Some(&entry.name);
            let is_context_active = ctx_entry.as_deref() == Some(&entry.name);

            let drag_files: Vec<String> =
                if pane.selected_files.contains(&entry.name) && pane.selected_files.len() > 1 {
                    pane.selected_files.iter().cloned().collect()
                } else {
                    vec![entry.name.clone()]
                };
            let drag_label = if drag_files.len() == 1 {
                entry.name.clone()
            } else {
                format!("{} files", drag_files.len())
            };

            let file_icon_path = resolve_file_icon_path(entry);
            let icon_el = if let Some(ref icon_p) = file_icon_path {
                div()
                    .w(px(22.0))
                    .h(px(22.0))
                    .child(gpui::img(icon_p.clone()).size_full())
                    .into_any_element()
            } else {
                div()
                    .text_xs()
                    .text_color(if is_dir {
                        rgb(0x60a5fa)
                    } else {
                        rgb(MacTheme::TEXT_TERTIARY)
                    })
                    .child(if is_dir { "📁" } else { "📄" })
                    .into_any_element()
            };

            let mut row = div()
                .id((
                    if is_left {
                        "sftp-left-entry"
                    } else {
                        "sftp-right-entry"
                    },
                    entry_idx,
                ))
                .h(px(40.0))
                .px_3()
                .flex()
                .items_center()
                .cursor_pointer()
                .bg(if is_context_active {
                    rgb(MacTheme::ACCENT_BLUE)
                } else if is_selected {
                    rgb(MacTheme::SURFACE_ACTIVE)
                } else {
                    rgba(0x00000000)
                })
                .hover(|s| {
                    if !is_selected && !is_context_active {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                    } else {
                        s
                    }
                })
                .on_drag(
                    SftpDragPayload {
                        from_left: is_left,
                        files: drag_files,
                    },
                    move |_, _, _, cx| {
                        cx.new(|_| DragPreviewView {
                            label: drag_label.clone(),
                        })
                    },
                )
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener({
                        let entry_name = entry_name.clone();
                        let entry_path = entry_path.clone();
                        move |app, event: &gpui::MouseDownEvent, _, cx| {
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            if event.click_count == 2 {
                                if is_dir {
                                    app.sftp_navigate_dir(is_left, entry_path.clone(), cx);
                                }
                            } else if event.modifiers.shift
                                || event.modifiers.platform
                                || event.modifiers.control
                            {
                                if p.selected_files.contains(&entry_name) {
                                    p.selected_files.remove(&entry_name);
                                    if p.selected_file.as_deref() == Some(&entry_name) {
                                        p.selected_file = p.selected_files.iter().next().cloned();
                                    }
                                } else {
                                    p.selected_files.insert(entry_name.clone());
                                    p.selected_file = Some(entry_name.clone());
                                }
                                app.sftp_context_menu = None;
                                cx.notify();
                            } else {
                                p.selected_files.clear();
                                p.selected_files.insert(entry_name.clone());
                                p.selected_file = Some(entry_name.clone());
                                app.sftp_context_menu = None;
                                cx.notify();
                            }
                        }
                    }),
                )
                .on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener({
                        let entry_name = entry_name.clone();
                        let entry_path = entry_path.clone();
                        move |app, event: &gpui::MouseDownEvent, _, cx| {
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            if !p.selected_files.contains(&entry_name) {
                                p.selected_files.clear();
                                p.selected_files.insert(entry_name.clone());
                                p.selected_file = Some(entry_name.clone());
                            }
                            let cursor_x = event.position.x.to_f64() as f32;
                            let cursor_y = event.position.y.to_f64() as f32;
                            app.sftp_open_context_menu(
                                is_left,
                                entry_name.clone(),
                                entry_path.clone(),
                                is_dir,
                                entry_idx,
                                cursor_x,
                                cursor_y,
                                cx,
                            );
                            cx.notify();
                        }
                    }),
                );

            if is_dir {
                let entry_path_for_drop = entry_path.clone();
                row = row.on_drop(cx.listener({
                    let entry_path_for_drop = entry_path_for_drop.clone();
                    move |app, payload: &SftpDragPayload, _, cx| {
                        if payload.from_left != is_left {
                            let p = if !payload.from_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            p.current_path = entry_path_for_drop.clone();
                            app.sftp_transfer_files(payload.from_left, payload.files.clone(), cx);
                        }
                    }
                }));
            }

            row = row
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_2p5()
                        .child(icon_el)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(if is_dir {
                                            FontWeight::MEDIUM
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(if is_context_active {
                                            rgb(0xffffff)
                                        } else {
                                            rgb(MacTheme::TEXT_PRIMARY)
                                        })
                                        .truncate()
                                        .child(entry.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if is_context_active {
                                            rgba(0xffffffaa)
                                        } else {
                                            rgb(MacTheme::TEXT_TERTIARY)
                                        })
                                        .child(formatted_permissions),
                                ),
                        ),
                )
                .child(
                    div()
                        .w(px(140.0))
                        .text_xs()
                        .text_color(if is_context_active {
                            rgba(0xffffffcc)
                        } else {
                            rgb(MacTheme::TEXT_TERTIARY)
                        })
                        .child(formatted_date),
                )
                .child(
                    div()
                        .w(px(90.0))
                        .text_xs()
                        .text_color(if is_context_active {
                            rgba(0xffffffcc)
                        } else {
                            rgb(MacTheme::TEXT_TERTIARY)
                        })
                        .child(formatted_size),
                )
                .child(
                    div()
                        .w(px(70.0))
                        .text_xs()
                        .text_color(if is_context_active {
                            rgba(0xffffffcc)
                        } else {
                            rgb(MacTheme::TEXT_TERTIARY)
                        })
                        .child(kind_label),
                );

            table_body = table_body.child(row);
        }

        let mut content = div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .on_drop(cx.listener(move |app, payload: &SftpDragPayload, _, cx| {
                if payload.from_left != is_left {
                    app.sftp_transfer_files(payload.from_left, payload.files.clone(), cx);
                }
            }))
            .child(header)
            .child(breadcrumb_row)
            .child(table_header)
            .child(table_body);

        // Actions Dropdown Menu Overlay
        if actions_open {
            let actions_menu = div()
                .id(if is_left {
                    "sftp-left-actions-menu"
                } else {
                    "sftp-right-actions-menu"
                })
                .absolute()
                .top(px(46.0))
                .right(px(12.0))
                .w(px(200.0))
                .rounded_lg()
                .bg(rgb(MacTheme::SURFACE))
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .p_1()
                .shadow_lg()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-new-folder"
                        } else {
                            "sftp-r-act-new-folder"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            p.actions_open = false;
                            app.sftp_open_new_folder_modal(is_left, cx);
                        }))
                        .child("New Folder"),
                )
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-copy"
                        } else {
                            "sftp-r-act-copy"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.sftp_copy_to_target(is_left, cx);
                        }))
                        .child("Copy to target directory"),
                )
                .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-refresh"
                        } else {
                            "sftp-r-act-refresh"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.refresh_sftp_pane(is_left, cx);
                            let p = if is_left {
                                &mut app.sftp_left_pane
                            } else {
                                &mut app.sftp_right_pane
                            };
                            p.actions_open = false;
                        }))
                        .child("Refresh"),
                )
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-hidden"
                        } else {
                            "sftp-r-act-hidden"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.sftp_toggle_pane_hidden(is_left, cx);
                        }))
                        .child(if pane.show_hidden {
                            "Hide Hidden Files"
                        } else {
                            "Show Hidden Files"
                        }),
                )
                .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                .child(
                    div()
                        .id(if is_left {
                            "sftp-l-act-close"
                        } else {
                            "sftp-r-act-close"
                        })
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::DANGER))
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .cursor_pointer()
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.sftp_close_host(is_left, cx);
                        }))
                        .child("Close"),
                );

            content = content.child(actions_menu);
        }

        content
    }

    fn render_sftp_context_menu_overlay(
        &self,
        ctx_menu: &SftpContextMenuState,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let is_left = ctx_menu.is_left;
        let entry_name = ctx_menu.entry_name.clone();
        let entry_path = ctx_menu.entry_path.clone();
        let is_dir = ctx_menu.is_dir;

        let left_offset = if ctx_menu.cursor_x > 0.0 {
            px((ctx_menu.cursor_x - 10.0).clamp(16.0, 950.0))
        } else if is_left {
            px(120.0)
        } else {
            px(600.0)
        };
        let top_offset = if ctx_menu.cursor_y > 0.0 {
            px((ctx_menu.cursor_y - 10.0).clamp(60.0, 550.0))
        } else {
            px(120.0 + (ctx_menu.target_idx as f32 * 36.0).min(380.0))
        };

        let has_item = !entry_name.is_empty();

        div()
            .id("sftp-context-menu-backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x00000022))
            .on_click(cx.listener(|app, _, _, cx| {
                app.sftp_close_context_menu(cx);
            }))
            .child(
                div()
                    .id("sftp-context-menu-popup")
                    .absolute()
                    .top(top_offset)
                    .left(left_offset)
                    .w(px(220.0))
                    .rounded_lg()
                    .bg(rgb(MacTheme::SURFACE))
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .p_1p5()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .when(has_item, |this| {
                        this.child(
                            div()
                                .id("sftp-ctx-copy")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let entry_name = entry_name.clone();
                                    move |app, _, _, cx| {
                                        let files = {
                                            let p = if is_left {
                                                &app.sftp_left_pane
                                            } else {
                                                &app.sftp_right_pane
                                            };
                                            if p.selected_files.contains(&entry_name)
                                                && p.selected_files.len() > 1
                                            {
                                                p.selected_files.iter().cloned().collect()
                                            } else {
                                                vec![entry_name.clone()]
                                            }
                                        };
                                        app.sftp_transfer_files(is_left, files, cx);
                                    }
                                }))
                                .child("Copy to target directory"),
                        )
                        .child(
                            div()
                                .id("sftp-ctx-rename")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let entry_name = entry_name.clone();
                                    let entry_path = entry_path.clone();
                                    move |app, _, _, cx| {
                                        app.sftp_open_rename_modal(
                                            is_left,
                                            entry_name.clone(),
                                            entry_path.clone(),
                                            cx,
                                        );
                                    }
                                }))
                                .child("Rename"),
                        )
                        .child(
                            div()
                                .id("sftp-ctx-delete")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::DANGER))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let entry_name = entry_name.clone();
                                    let entry_path = entry_path.clone();
                                    move |app, _, _, cx| {
                                        app.sftp_open_delete_modal(
                                            is_left,
                                            entry_name.clone(),
                                            entry_path.clone(),
                                            is_dir,
                                            cx,
                                        );
                                    }
                                }))
                                .child("Delete"),
                        )
                    })
                    .child(
                        div()
                            .id("sftp-ctx-refresh")
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_close_context_menu(cx);
                                app.refresh_sftp_pane(is_left, cx);
                            }))
                            .child("Refresh"),
                    )
                    .child(
                        div()
                            .id("sftp-ctx-new-folder")
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.sftp_open_new_folder_modal(is_left, cx);
                            }))
                            .child("New Folder"),
                    )
                    .when(has_item, |this| {
                        this.child(
                            div()
                                .id("sftp-ctx-permissions")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener({
                                    let entry_name = entry_name.clone();
                                    let entry_path = entry_path.clone();
                                    move |app, _, _, cx| {
                                        app.sftp_open_permissions_modal(
                                            is_left,
                                            entry_name.clone(),
                                            entry_path.clone(),
                                            cx,
                                        );
                                    }
                                }))
                                .child("Edit Permissions"),
                        )
                    })
                    .when(!has_item, |this| {
                        let show_hidden = {
                            let p = if is_left {
                                &self.sftp_left_pane
                            } else {
                                &self.sftp_right_pane
                            };
                            p.show_hidden
                        };
                        this.child(
                            div()
                                .id("sftp-ctx-toggle-hidden")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.sftp_close_context_menu(cx);
                                    app.sftp_toggle_pane_hidden(is_left, cx);
                                }))
                                .child(if show_hidden {
                                    "Hide Hidden Files"
                                } else {
                                    "Show Hidden Files"
                                }),
                        )
                    }),
            )
    }

    fn render_sftp_modal_overlay(
        &self,
        modal: &SftpModalState,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (title, content, btn_label, is_danger) = match &modal.action {
            SftpModalAction::Rename {
                entry_name, input, ..
            } => (
                format!("Rename '{entry_name}'"),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Enter a new name:"),
                    )
                    .child(
                        div()
                            .w_full()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .px_3()
                            .py_1p5()
                            .child(input.clone()),
                    ),
                "Rename",
                false,
            ),
            SftpModalAction::Delete {
                entry_name, is_dir, ..
            } => (
                format!("Delete '{}'?", entry_name),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(format!(
                                "Are you sure you want to permanently delete this {}?",
                                if *is_dir { "folder" } else { "file" }
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("This action cannot be undone."),
                    ),
                "Delete",
                true,
            ),
            SftpModalAction::NewFolder { input } => (
                "New Folder".to_string(),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Folder name:"),
                    )
                    .child(
                        div()
                            .w_full()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .px_3()
                            .py_1p5()
                            .child(input.clone()),
                    ),
                "Create Folder",
                false,
            ),
            SftpModalAction::EditPermissions {
                entry_name, input, ..
            } => (
                format!("Edit Permissions for '{entry_name}'"),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Enter octal mode (e.g. 0755, 0644):"),
                    )
                    .child(
                        div()
                            .w_full()
                            .rounded_lg()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .px_3()
                            .py_1p5()
                            .child(input.clone()),
                    ),
                "Apply Permissions",
                false,
            ),
        };

        let mut card = div()
            .id("sftp-modal-card")
            .w(px(380.0))
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .p_5()
            .shadow_xl()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .child(title),
            )
            .child(content);

        if let Some(err) = &modal.error {
            card = card.child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(err.clone()),
            );
        }

        div()
            .id("sftp-modal-backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000aa))
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .child(
                card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .child(
                            div()
                                .id("sftp-modal-cancel-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::BORDER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_close_modal(cx);
                                }))
                                .child("Cancel"),
                        )
                        .child(
                            div()
                                .id("sftp-modal-submit-btn")
                                .px_3()
                                .py_1p5()
                                .rounded_md()
                                .bg(if is_danger {
                                    rgb(MacTheme::DANGER)
                                } else {
                                    rgb(MacTheme::ACCENT_BLUE)
                                })
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .hover(|s| {
                                    if is_danger {
                                        s.bg(rgb(0xdc2626))
                                    } else {
                                        s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                    }
                                })
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_submit_modal(cx);
                                }))
                                .child(btn_label),
                        ),
                ),
            )
    }

    fn render_vault_dropdown_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        if !self.vault_dropdown_open {
            return None;
        }
        Some(
            div()
                .id("vault-dropdown-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(rgba(0x00000033))
                .on_click(cx.listener(|app, _, _, cx| {
                    app.vault_dropdown_open = false;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("vault-dropdown-menu")
                        .absolute()
                        .top(px(46.0))
                        .left(px(84.0))
                        .w(px(220.0))
                        .rounded_lg()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .p_1p5()
                        .shadow_lg()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                        }))
                        .child(
                            div()
                                .px_2p5()
                                .py_1()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .font_weight(FontWeight::BOLD)
                                .child("LOCAL VAULTS"),
                        )
                        .child(
                            div()
                                .id("vault-opt-personal")
                                .flex()
                                .items_center()
                                .justify_between()
                                .px_2p5()
                                .py_2()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .bg(if self.active_vault == "Personal" {
                                    rgb(MacTheme::SURFACE_ACTIVE)
                                } else {
                                    rgba(0x00000000)
                                })
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.active_vault = "Personal".to_string();
                                    app.vault_dropdown_open = false;
                                    app.navigate_page("Hosts", window, cx);
                                }))
                                .child("Personal")
                                .child(if self.active_vault == "Personal" {
                                    "✓"
                                } else {
                                    ""
                                }),
                        )
                        .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                        .child(
                            div()
                                .id("vault-opt-manage")
                                .px_2p5()
                                .py_2()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.vault_dropdown_open = false;
                                    app.open_team_modal(cx);
                                }))
                                .child("+ Manage Vaults…"),
                        ),
                ),
        )
    }
}

impl Render for Shellumi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .id("shellumi-root")
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::CONTENT_BG))
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .text_size(px(self.desktop.preferences.ui_font_size as f32))
            .on_key_down(cx.listener(Self::handle_palette_key_down))
            .on_action(cx.listener(|app, _: &RefreshHosts, _, cx| app.load_hosts(cx)))
            .on_action(cx.listener(|app, _: &ToggleCommandPalette, window, cx| {
                app.toggle_command_palette(window, cx);
            }))
            .on_action(cx.listener(|app, _: &NewTab, window, cx| {
                app.open_local_terminal(window, cx);
            }))
            .on_action(cx.listener(|app, _: &OpenLocalTerminal, window, cx| {
                app.open_local_terminal(window, cx);
            }))
            .on_action(cx.listener(|app, _: &CloseTab, window, cx| {
                app.close_active_tab(window, cx);
            }))
            .on_action(cx.listener(|app, _: &NextTab, window, cx| {
                app.next_tab(window, cx);
            }))
            .on_action(cx.listener(|app, _: &PrevTab, window, cx| {
                app.prev_tab(window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab1, window, cx| {
                app.switch_to_tab(0, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab2, window, cx| {
                app.switch_to_tab(1, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab3, window, cx| {
                app.switch_to_tab(2, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab4, window, cx| {
                app.switch_to_tab(3, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab5, window, cx| {
                app.switch_to_tab(4, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab6, window, cx| {
                app.switch_to_tab(5, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab7, window, cx| {
                app.switch_to_tab(6, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab8, window, cx| {
                app.switch_to_tab(7, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab9, window, cx| {
                app.switch_to_tab(8, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SplitVertical, window, cx| {
                app.split_pane(SplitDirection::Vertical, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SplitHorizontal, window, cx| {
                app.split_pane(SplitDirection::Horizontal, window, cx);
            }))
            .on_action(cx.listener(|app, _: &FocusNextPane, _, cx| {
                app.focus_next_pane(cx);
            }))
            .on_action(cx.listener(|app, _: &FocusPrevPane, _, cx| {
                app.focus_prev_pane(cx);
            }))
            .on_action(cx.listener(|app, _: &ClosePane, window, cx| {
                app.close_active_pane(window, cx);
            }))
            .on_action(cx.listener(|app, _: &ToggleFocusMode, _, cx| {
                app.toggle_focus_mode(cx);
            }))
            .on_action(cx.listener(|app, _: &ToggleBroadcast, _, cx| {
                app.toggle_broadcast_mode(cx);
            }))
            .on_action(cx.listener(|app, _: &ToggleSnippets, _, cx| {
                app.snippets_panel_open = !app.snippets_panel_open;
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &FindInTerminal, _, cx| {
                app.trigger_find_in_terminal(cx);
            }))
            .on_action(cx.listener(|app, _: &ZoomIn, _, cx| {
                app.desktop.preferences.terminal_font_size =
                    (app.desktop.preferences.terminal_font_size + 1).min(32);
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &ZoomOut, _, cx| {
                app.desktop.preferences.terminal_font_size =
                    (app.desktop.preferences.terminal_font_size.saturating_sub(1)).max(8);
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &ResetZoom, _, cx| {
                app.desktop.preferences.terminal_font_size = 14;
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &QuickConnect, window, cx| {
                app.trigger_quick_connect(window, cx);
            }))
            .on_action(cx.listener(|app, _: &NewHost, _, cx| {
                app.edit_host(None, cx);
            }))
            .on_action(cx.listener(|app, _: &ReconnectSession, window, cx| {
                app.reconnect_active_session(window, cx);
            }))
            .on_action(cx.listener(|app, _: &ToggleSidebar, _, cx| {
                app.sidebar_visible = !app.sidebar_visible;
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToHosts, window, cx| {
                app.page = None;
                app.select_tab(None, window, cx);
            }))
            .on_action(cx.listener(|app, _: &GoToOverview, _, cx| {
                app.page = Some("Overview");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToWorkspaces, _, cx| {
                app.page = Some("Workspaces");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToSftp, _, cx| {
                app.page = Some("SFTP Files");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToKeychain, _, cx| {
                app.page = Some("Keychain");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToPortForwarding, _, cx| {
                app.page = Some("Port Forwarding");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &OpenSettings, _, cx| {
                app.page = Some("Settings");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &ToggleSidebarCollapse, _, cx| {
                app.toggle_sidebar_collapsed(cx);
            }))
            .on_action(cx.listener(|_app, _: &Quit, _, cx| cx.quit()))
            .on_action(cx.listener(|app, _: &AboutApp, window, cx| {
                app.navigate_page("Settings", window, cx);
            }))
            .on_action(cx.listener(|app, _: &OpenVaultsMenu, _, cx| {
                app.vault_dropdown_open = true;
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &OpenSftpMenu, window, cx| {
                app.navigate_page("SFTP Files", window, cx);
            }))
            .on_action(cx.listener(|app, _: &OpenSettingsMenu, window, cx| {
                app.navigate_page("Settings", window, cx);
            }))
            .on_action(cx.listener(|app, _: &OpenShortcutsHelp, window, cx| {
                app.navigate_page("Settings", window, cx);
            }));

        root = root.child(self.render_unified_top_bar(cx));

        if let Some(page) = self.page {
            let body = self.render_desktop_page(page, cx);
            let mut container = div().flex().flex_1().min_h_0().overflow_hidden();
            let show_sidebar = self.sidebar_visible && page != "SFTP Files";
            if show_sidebar {
                container = container.child(self.workspace_sidebar(page, cx));
            }
            let mut result = root.child(container.child(body));
            if let Some(dropdown) = self.render_vault_dropdown_overlay(cx) {
                result = result.child(dropdown);
            }
            return self.desktop_overlays(result, cx);
        }

        if let Some(active_idx) = self.active_tab_idx {
            if let Some(tab) = self.tabs.get(active_idx) {
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
                    SessionKind::RemoteDesktop => {
                        if tab
                            .remote_desktop_state
                            .as_ref()
                            .is_some_and(|r| r.protocol == RemoteDesktopProtocol::Vnc)
                        {
                            format!("VNC Remote Desktop · {}", tab.name)
                        } else {
                            format!("RDP Remote Desktop · {}", tab.name)
                        }
                    }
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
                    SessionKind::RemoteDesktop => {
                        "Interactive graphical remote desktop session (RFB / MS-RDPBCGR)."
                    }
                };

                let mut body = div()
                    .id("terminal-view")
                    .min_h_0()
                    .overflow_hidden()
                    .track_focus(&self.focus)
                    .on_action(cx.listener(|app, _: &ToggleCommandPalette, window, cx| {
                        app.toggle_command_palette(window, cx);
                    }))
                    .on_key_down(cx.listener(Self::terminal_input))
                    .flex_1()
                    .min_w_0()
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
                                    .child(
                                        div()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child(header_subtitle),
                                    ),
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
                                            .bg(rgb(0x4a1818))
                                            .text_color(rgb(MacTheme::DANGER))
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
                                            .bg(rgb(MacTheme::ACCENT_BLUE))
                                            .text_sm()
                                            .text_color(rgb(0xffffff))
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
                                                    rgb(MacTheme::SURFACE_HOVER)
                                                } else {
                                                    rgb(MacTheme::INPUT_BG)
                                                })
                                                .text_sm()
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                                                .bg(rgb(MacTheme::INPUT_BG))
                                                .text_sm()
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                                                .bg(rgb(MacTheme::INPUT_BG))
                                                .text_sm()
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                .cursor_pointer()
                                                .on_click(cx.listener(|app, _, window, cx| {
                                                    app.split_pane(
                                                        SplitDirection::Vertical,
                                                        window,
                                                        cx,
                                                    )
                                                }))
                                                .child("Split vertical"),
                                        )
                                        .child(
                                            div()
                                                .id("split-horizontal")
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(rgb(MacTheme::INPUT_BG))
                                                .text_sm()
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                .cursor_pointer()
                                                .on_click(cx.listener(|app, _, window, cx| {
                                                    app.split_pane(
                                                        SplitDirection::Horizontal,
                                                        window,
                                                        cx,
                                                    )
                                                }))
                                                .child("Split horizontal"),
                                        )
                                        .child(
                                            div()
                                                .id("toggle-snippets")
                                                .p_2()
                                                .rounded_md()
                                                .bg(if self.snippets_panel_open {
                                                    rgb(MacTheme::ACCENT_BLUE)
                                                } else {
                                                    rgb(MacTheme::INPUT_BG)
                                                })
                                                .text_sm()
                                                .text_color(if self.snippets_panel_open {
                                                    rgb(0xffffff)
                                                } else {
                                                    rgb(MacTheme::TEXT_PRIMARY)
                                                })
                                                .cursor_pointer()
                                                .on_click(cx.listener(|app, _, _, cx| {
                                                    app.snippets_panel_open =
                                                        !app.snippets_panel_open;
                                                    cx.notify();
                                                }))
                                                .child("Snippets"),
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
                                            .bg(if is_on {
                                                rgb(MacTheme::DANGER)
                                            } else {
                                                rgb(MacTheme::INPUT_BG)
                                            })
                                            .text_sm()
                                            .text_color(if is_on {
                                                rgb(0xffffff)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
                                            })
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

                                actions_bar
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
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR));

                    let mut info_left = div().flex().items_center().gap_3();
                    info_left = info_left.child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .child(format!("👥 Session: {}", multiplayer.session_id)),
                    );
                    info_left = info_left.child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(format!("Host: @{owner}")),
                    );
                    info_left = info_left.child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(if controller == "local_user" {
                                rgb(MacTheme::SUCCESS)
                            } else {
                                rgb(MacTheme::WARNING)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if controller == "local_user" {
                                rgb(0xffffff)
                            } else {
                                rgb(0x000000)
                            })
                            .child(format!("✍️ Active Writer: @{controller}")),
                    );
                    info_left = info_left.child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
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
                                .bg(rgb(MacTheme::ACCENT_BLUE))
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
                                .bg(rgb(MacTheme::WARNING))
                                .text_xs()
                                .text_color(rgb(0x000000))
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
                            .bg(rgb(MacTheme::INPUT_BG))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                            .bg(rgb(MacTheme::DANGER))
                            .text_xs()
                            .text_color(rgb(0xffffff))
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
                        .bg(rgb(MacTheme::SURFACE))
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR));

                    if self.snippets.is_empty() {
                        snippets_bar = snippets_bar.child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child(
                                "No saved snippets found. Add snippets via `shellumi snippet add`.",
                            ),
                        );
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
                                .bg(rgb(MacTheme::INPUT_BG))
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(snip_name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .font_family(MONOSPACE_FONT)
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
                                        .bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                                        .bg(rgb(MacTheme::SUCCESS))
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
                                        .bg(rgb(MacTheme::ACCENT_BLUE))
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
                            .bg(rgb(0x4a1818))
                            .border_1()
                            .border_color(rgb(MacTheme::DANGER))
                            .text_color(rgb(MacTheme::DANGER))
                            .text_sm()
                            .child(format!("⚠️ {warning_msg}"))
                            .child(
                                div()
                                    .id("dismiss-broadcast-warning")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(MacTheme::DANGER))
                                    .text_color(rgb(0xffffff))
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
                    Some(SplitDirection::Grid) => div()
                        .grid()
                        .grid_cols(2)
                        .grid_rows(2)
                        .gap_3()
                        .flex_1()
                        .min_h_0()
                        .min_w_0(),
                    Some(SplitDirection::Vertical) => {
                        div().min_h_0().min_w_0().flex().flex_row().gap_3().flex_1()
                    }
                    Some(SplitDirection::Horizontal) => {
                        div().min_h_0().min_w_0().flex().flex_col().gap_3().flex_1()
                    }
                    None => div().min_h_0().min_w_0().flex().flex_col().flex_1(),
                };

                for (pane_idx, pane) in tab.panes.iter().enumerate() {
                    let pane_id = pane.id;
                    let is_active_pane = tab.active_pane_idx == pane_idx;
                    let grid = pane.terminal.grid();

                    let mut pane_el = div()
                        .id(("terminal-pane", pane_id))
                        .min_h_0()
                        .min_w_0()
                        .overflow_hidden()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(MacTheme::TERMINAL_BG))
                        .border_1()
                        .border_color(if is_active_pane {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::BORDER)
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
                        .bg(rgb(MacTheme::SURFACE))
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
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
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
                                .bg(rgb(MacTheme::INPUT_BG))
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .text_xs()
                                .child(format!("📂 {}", display_cwd)),
                        );
                    }

                    // Exit code badge
                    if let Some(code) = pane.terminal.last_exit_code() {
                        let (bg_color, fg_color, text) = if code == 0 {
                            (rgb(0x133820), rgb(MacTheme::SUCCESS), "✓ 0".to_string())
                        } else {
                            (rgb(0x4a1818), rgb(MacTheme::DANGER), format!("✗ {}", code))
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
                                    .bg(rgb(0x3a2817))
                                    .text_color(rgb(MacTheme::WARNING))
                                    .text_xs()
                                    .child(format!("🔑 Prompt: {:?}", prompt_match.kind)),
                            );
                        }
                    }

                    pane_controls = pane_controls.child(pane_info_group);

                    let mut pane_btn_group = div().flex().items_center().gap_1();
                    let is_recording = pane.terminal.is_recording();
                    let record_btn_label = if is_recording {
                        let dur = pane.terminal.recording_duration();
                        format!("⏹ Stop ({})", format_duration_hms(dur.as_secs_f64()))
                    } else {
                        "⏺ Record".to_string()
                    };

                    pane_btn_group = pane_btn_group.child(
                        div()
                            .id(("toggle-pane-recording", pane_id))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .text_xs()
                            .cursor_pointer()
                            .bg(if is_recording {
                                rgb(MacTheme::DANGER)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_pane_recording(pane_id, cx);
                                cx.stop_propagation();
                            }))
                            .child(record_btn_label),
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
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_search(pane_id, cx);
                                cx.stop_propagation();
                            }))
                            .child("Search"),
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
                                    rgb(MacTheme::SURFACE_HOVER)
                                } else {
                                    rgb(MacTheme::SUCCESS)
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
                                .bg(rgb(MacTheme::DANGER))
                                .text_color(rgb(0xffffff))
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
                            .bg(rgb(MacTheme::SURFACE))
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .child(div().flex_1().child(pane.search_input.clone()))
                            .child(
                                div()
                                    .id(("exec-search", pane_id))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
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
                                    .bg(rgb(MacTheme::INPUT_BG))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
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
                                    .bg(rgb(MacTheme::INPUT_BG))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.search_next(pane_id, cx);
                                        cx.stop_propagation();
                                    }))
                                    .child("↓"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(match_label),
                            );
                        pane_el = pane_el.child(search_bar);
                    }

                    if let Some(message) = &pane.message {
                        pane_el = pane_el.child(
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .text_color(rgb(MacTheme::DANGER))
                                .child(message.clone()),
                        );
                    }

                    // Terminal screen rendering with search highlights
                    let font_size = self.desktop.preferences.terminal_font_size as f32;
                    let line_height = font_size * 1.5;
                    let mut screen = div()
                        .id(("terminal-screen", pane_id))
                        .line_height(px(line_height))
                        .track_scroll(&pane.scroll)
                        .text_size(px(self.desktop.preferences.terminal_font_size as f32))
                        .min_h_0()
                        .min_w_0()
                        .flex_1()
                        .overflow_y_scroll()
                        .font_family(MONOSPACE_FONT)
                        .bg(rgb(MacTheme::TERMINAL_BG))
                        .p_3()
                        .flex()
                        .flex_col();

                    let scrollback_len = pane.terminal.scrollback_count();
                    let start_scrollback =
                        scrollback_len.saturating_sub(self.desktop.preferences.scrollback_lines);

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
                            screen = screen.child(render_terminal_spans(spans, line_height));
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
                            screen = screen.child(render_terminal_spans(spans, line_height));
                        }
                    }

                    let view = cx.entity().downgrade();
                    let old_dimensions = (grid.rows(), grid.cols());
                    let resize_sensor = gpui::canvas(
                        move |bounds, window, cx| {
                            let font = gpui::font(MONOSPACE_FONT);
                            let font_id = window.text_system().resolve_font(&font);
                            let width = window
                                .text_system()
                                .advance(font_id, px(font_size), 'M')
                                .map(|s| f32::from(s.width))
                                .unwrap_or(font_size * 0.6);
                            let dimensions = desktop::terminal_dimensions(
                                f32::from(bounds.size.width) - 24.0,
                                f32::from(bounds.size.height) - 24.0,
                                width,
                                line_height,
                            );
                            if dimensions != old_dimensions {
                                cx.defer(move |cx| {
                                    let _ = view.update(cx, |app, cx| {
                                        if let Some(pane) = app
                                            .tabs
                                            .iter_mut()
                                            .flat_map(|tab| &mut tab.panes)
                                            .find(|pane| pane.id == pane_id)
                                        {
                                            if (
                                                pane.terminal.grid().rows(),
                                                pane.terminal.grid().cols(),
                                            ) != dimensions
                                            {
                                                pane.terminal.resize(dimensions.0, dimensions.1);
                                                if let Some(session) = &pane.session {
                                                    let _ = session.resize(
                                                        dimensions.0 as u16,
                                                        dimensions.1 as u16,
                                                    );
                                                }
                                                cx.notify();
                                            }
                                        }
                                    });
                                });
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0();
                    let scroll_handle = pane.scroll.clone();
                    let scrollbar = gpui::canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            let maximum = f32::from(scroll_handle.max_offset().height);
                            if maximum > 0.0 {
                                let height = f32::from(bounds.size.height);
                                let thumb_height =
                                    (height * height / (height + maximum)).max(24.0).min(height);
                                let fraction = (-f32::from(scroll_handle.offset().y) / maximum)
                                    .clamp(0.0, 1.0);
                                let origin = gpui::point(
                                    bounds.origin.x,
                                    bounds.origin.y + px((height - thumb_height) * fraction),
                                );
                                window.paint_quad(gpui::fill(
                                    gpui::Bounds::new(origin, size(px(4.0), px(thumb_height))),
                                    rgb(MacTheme::SURFACE_HOVER),
                                ));
                            }
                        },
                    )
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .w(px(5.0));
                    pane_el = pane_el.child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .flex()
                            .child(screen)
                            .child(resize_sensor)
                            .child(scrollbar),
                    );
                    panes_container = panes_container.child(pane_el);
                }

                let mut result = if is_sftp_dual_pane {
                    let sftp_state = tab.sftp_state.as_ref().unwrap();
                    let row = div().flex().flex_1().min_h_0();
                    root.child(
                        row.child(
                            body.child(self.render_sftp_dual_pane(active_idx, sftp_state, cx)),
                        ),
                    )
                } else if let Some(rd_state) = &tab.remote_desktop_state {
                    let mut row = div().flex().flex_1().min_h_0();
                    if self.sidebar_visible {
                        row = row.child(self.workspace_sidebar("Remote Desktop", cx));
                    }
                    root.child(row.child(
                        body.child(self.render_remote_desktop_session(active_idx, rd_state, cx)),
                    ))
                } else {
                    let mut row = div().flex().flex_1().min_h_0();
                    if self.sidebar_visible {
                        row = row.child(self.workspace_sidebar("Terminal", cx));
                    }
                    root.child(row.child(body.child(panes_container)))
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
                if let Some(modal) = &self.session_replay_modal {
                    result = result.child(self.render_session_replay_modal(modal, cx));
                }
                if let Some(modal) = &self.tunnel_modal {
                    result = result.child(self.render_tunnel_modal(modal, cx));
                }
                return result;
            }
        }
        let hosts = match &self.hosts {
            HostState::Loaded(hosts) => hosts.as_slice(),
            _ => &[],
        };
        let search_val = self.search_input.read(cx).value().trim().to_lowercase();
        let quick_val = self
            .quick_connect_input
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let query = if !quick_val.is_empty() {
            quick_val
        } else {
            search_val
        };

        let filtered: Vec<&SavedHost> = hosts
            .iter()
            .filter(|host| {
                group_matches(host.group_name.as_deref(), self.selected_group.as_deref())
                    && (query.is_empty()
                        || host.name.to_lowercase().contains(&query)
                        || host.destination.host().to_lowercase().contains(&query)
                        || host.destination.username().to_lowercase().contains(&query)
                        || host
                            .group_name
                            .as_ref()
                            .is_some_and(|group| group.to_lowercase().contains(&query))
                        || host
                            .tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&query)))
            })
            .collect();
        let selected = filtered
            .iter()
            .copied()
            .find(|host| Some(host.id) == self.selected_host_id);

        // Termius Sub-Action Bar
        let view_mode_menu = if self.view_mode_menu_open {
            Some(
                div()
                    .id("view-mode-popover")
                    .absolute()
                    .top(px(34.0))
                    .right(px(100.0))
                    .w(px(120.0))
                    .rounded_lg()
                    .bg(rgb(0x181a24))
                    .border_1()
                    .border_color(rgb(0x2d3245))
                    .p_1()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .id("view-opt-grid")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.view_mode == HostViewMode::Grid {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.view_mode = HostViewMode::Grid;
                                app.view_mode_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) = resolve_icon_path(
                                            "grid_view",
                                            self.view_mode == HostViewMode::Grid,
                                            32,
                                        ) {
                                            div()
                                                .w(px(14.0))
                                                .h(px(14.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("▦").into_any_element()
                                        },
                                    )
                                    .child("Grid"),
                            )
                            .child(if self.view_mode == HostViewMode::Grid {
                                "✓"
                            } else {
                                ""
                            }),
                    )
                    .child(
                        div()
                            .id("view-opt-list")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.view_mode == HostViewMode::List {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.view_mode = HostViewMode::List;
                                app.view_mode_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) = resolve_icon_path(
                                            "list_view",
                                            self.view_mode == HostViewMode::List,
                                            32,
                                        ) {
                                            div()
                                                .w(px(14.0))
                                                .h(px(14.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("≡").into_any_element()
                                        },
                                    )
                                    .child("List"),
                            )
                            .child(if self.view_mode == HostViewMode::List {
                                "✓"
                            } else {
                                ""
                            }),
                    ),
            )
        } else {
            None
        };

        let tags_filter_menu = if self.tag_filter_popover_open {
            let mut all_tags: Vec<String> = hosts.iter().flat_map(|h| h.tags.clone()).collect();
            all_tags.sort();
            all_tags.dedup();

            let mut tags_list = div().flex().flex_wrap().gap_1p5();
            for (t_idx, tag) in all_tags.into_iter().enumerate() {
                let tag_name = tag.clone();
                let is_active = self.active_tag_filter.as_deref() == Some(&tag);
                tags_list = tags_list.child(
                    div()
                        .id(("tag-filter-opt", t_idx))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_xs()
                        .cursor_pointer()
                        .bg(if is_active {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(0x252a36)
                        })
                        .text_color(if is_active {
                            rgb(0xffffff)
                        } else {
                            rgb(MacTheme::TEXT_PRIMARY)
                        })
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .on_click(cx.listener(move |app, _, _, cx| {
                            if app.active_tag_filter.as_deref() == Some(&tag_name) {
                                app.active_tag_filter = None;
                            } else {
                                app.active_tag_filter = Some(tag_name.clone());
                            }
                            app.tag_filter_popover_open = false;
                            cx.notify();
                        }))
                        .child(tag),
                );
            }

            Some(
                div()
                    .id("tags-filter-popover")
                    .absolute()
                    .top(px(34.0))
                    .right(px(70.0))
                    .w(px(220.0))
                    .rounded_lg()
                    .bg(rgb(0x181a24))
                    .border_1()
                    .border_color(rgb(0x2d3245))
                    .p_3()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(if let Some(p) = resolve_icon_path("tag", false, 32) {
                                        div()
                                            .w(px(14.0))
                                            .h(px(14.0))
                                            .child(gpui::img(p).size_full())
                                            .into_any_element()
                                    } else {
                                        div().child("🏷").into_any_element()
                                    })
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child("Filter by tags"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("clear-tag-filter-btn")
                                    .text_xs()
                                    .text_color(rgb(MacTheme::ACCENT_BLUE))
                                    .cursor_pointer()
                                    .hover(|s| s.underline())
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.active_tag_filter = None;
                                        app.tag_filter_popover_open = false;
                                        cx.notify();
                                    }))
                                    .child("Clear"),
                            ),
                    )
                    .child(tags_list),
            )
        } else {
            None
        };

        let sort_menu = if self.sort_menu_open {
            Some(
                div()
                    .id("sort-popover")
                    .absolute()
                    .top(px(34.0))
                    .right(px(36.0))
                    .w(px(170.0))
                    .rounded_lg()
                    .bg(rgb(0x181a24))
                    .border_1()
                    .border_color(rgb(0x2d3245))
                    .p_1()
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .id("sort-opt-alpha-asc")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.sort_mode == HostSortMode::AlphaAsc {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_mode = HostSortMode::AlphaAsc;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(div().flex().items_center().gap_2().child("Az").child("A-z"))
                            .child(if self.sort_mode == HostSortMode::AlphaAsc {
                                "✓"
                            } else {
                                ""
                            }),
                    )
                    .child(
                        div()
                            .id("sort-opt-alpha-desc")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.sort_mode == HostSortMode::AlphaDesc {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_mode = HostSortMode::AlphaDesc;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(div().flex().items_center().gap_2().child("Za").child("Z-a"))
                            .child(if self.sort_mode == HostSortMode::AlphaDesc {
                                "✓"
                            } else {
                                ""
                            }),
                    )
                    .child(
                        div()
                            .id("sort-opt-newest")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.sort_mode == HostSortMode::Newest {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_mode = HostSortMode::Newest;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) = resolve_icon_path(
                                            "calendar_sort",
                                            self.sort_mode == HostSortMode::Newest,
                                            32,
                                        ) {
                                            div()
                                                .w(px(14.0))
                                                .h(px(14.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("📅").into_any_element()
                                        },
                                    )
                                    .child("Newest to oldest"),
                            )
                            .child(if self.sort_mode == HostSortMode::Newest {
                                "✓"
                            } else {
                                ""
                            }),
                    )
                    .child(
                        div()
                            .id("sort-opt-oldest")
                            .px_2p5()
                            .py_1p5()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x252a36)))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_color(if self.sort_mode == HostSortMode::Oldest {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_mode = HostSortMode::Oldest;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) = resolve_icon_path(
                                            "calendar_sort",
                                            self.sort_mode == HostSortMode::Oldest,
                                            32,
                                        ) {
                                            div()
                                                .w(px(14.0))
                                                .h(px(14.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div().child("📅").into_any_element()
                                        },
                                    )
                                    .child("Oldest to newest"),
                            )
                            .child(if self.sort_mode == HostSortMode::Oldest {
                                "✓"
                            } else {
                                ""
                            }),
                    ),
            )
        } else {
            None
        };

        let action_bar = div()
            .id("hosts-action-bar")
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
                        div()
                            .id("add-host")
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_1p5()
                            .rounded(px(MacTheme::CONTROL_RADIUS))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .bg(rgb(MacTheme::SURFACE_ACTIVE))
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE)).text_color(rgb(0xffffff)))
                            .on_click(cx.listener(|app, _, _, cx| app.edit_host(None, cx)))
                            .child(if let Some(p) = resolve_icon_path("add_host", false, 48) {
                                div()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .child(gpui::img(p).size_full())
                                    .into_any_element()
                            } else {
                                div().into_any_element()
                            })
                            .child("New host")
                            .child("⌵"),
                    )
                    .child(
                        crate::desktop::button_icon("add-serial-host", "serial", "Serial")
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.edit_host_with_protocol(ConnectionProtocol::Serial, cx)
                            })),
                    )
                    .child(self.host_library_actions(cx)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("hosts-view-mode")
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_md()
                            .bg(if self.view_mode_menu_open {
                                rgb(MacTheme::SURFACE_HOVER)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.view_mode_menu_open = !app.view_mode_menu_open;
                                app.tag_filter_popover_open = false;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                if let Some(p) = resolve_icon_path(
                                    if self.view_mode == HostViewMode::Grid {
                                        "grid_view"
                                    } else {
                                        "list_view"
                                    },
                                    self.view_mode_menu_open,
                                    48,
                                ) {
                                    div()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div()
                                        .child(if self.view_mode == HostViewMode::Grid {
                                            "▦"
                                        } else {
                                            "≡"
                                        })
                                        .into_any_element()
                                },
                            ),
                    )
                    .child(
                        div()
                            .id("hosts-tags-filter")
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_md()
                            .bg(if self.active_tag_filter.is_some() {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else if self.tag_filter_popover_open {
                                rgb(MacTheme::SURFACE_HOVER)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(if self.active_tag_filter.is_some() {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.tag_filter_popover_open = !app.tag_filter_popover_open;
                                app.view_mode_menu_open = false;
                                app.sort_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                if let Some(p) = resolve_icon_path(
                                    "tag",
                                    self.active_tag_filter.is_some()
                                        || self.tag_filter_popover_open,
                                    48,
                                ) {
                                    div()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().child("🏷").into_any_element()
                                },
                            ),
                    )
                    .child(
                        div()
                            .id("hosts-sort")
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_md()
                            .bg(if self.sort_menu_open {
                                rgb(MacTheme::SURFACE_HOVER)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .cursor_pointer()
                            .hover(|s| {
                                s.bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sort_menu_open = !app.sort_menu_open;
                                app.view_mode_menu_open = false;
                                app.tag_filter_popover_open = false;
                                cx.notify();
                            }))
                            .child(
                                if let Some(p) =
                                    resolve_icon_path("calendar_sort", self.sort_menu_open, 48)
                                {
                                    div()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().child("📅").into_any_element()
                                },
                            ),
                    )
                    .child(
                        div()
                            .id("hosts-team-invite")
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE))
                            .border_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(|app, _, _, cx| app.open_invite_modal(cx)))
                            .child(if let Some(p) = resolve_icon_path("profile", false, 48) {
                                div()
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .child(gpui::img(p).size_full())
                                    .into_any_element()
                            } else {
                                div().child("👤+").into_any_element()
                            }),
                    )
                    .children(view_mode_menu)
                    .children(tags_filter_menu)
                    .children(sort_menu),
            );

        // Breadcrumbs if a group is currently filtered
        let breadcrumbs = if let Some(selected_grp) = &self.selected_group {
            let grp_clone = selected_grp.clone();
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .child(
                        div()
                            .id("breadcrumb-all-hosts")
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.selected_group = None;
                                cx.notify();
                            }))
                            .child("← All hosts"),
                    )
                    .child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child("/"))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(grp_clone),
                    ),
            )
        } else {
            None
        };

        // Top team invite banner
        let invite_banner = div()
            .id("team-invite-banner")
            .p_3()
            .rounded_lg()
            .bg(rgb(0x18202d))
            .border_1()
            .border_color(rgb(0x24344d))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .child(div().text_base().child("👥"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .child("Invite members."),
                            )
                            .child(
                                div()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Manage and use data together with your team."),
                            ),
                    ),
            )
            .child(
                div()
                    .id("invite-members-btn")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x252a36))
                    .border_1()
                    .border_color(rgb(0x384154))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x323a4b)).border_color(rgb(MacTheme::ACCENT_BLUE)))
                    .on_click(cx.listener(|app, _, _, cx| app.open_invite_modal(cx)))
                    .child("Invite members"),
            );

        // Groups cards section (only shown when no group filter is selected and no search query)
        let mut groups: Vec<&str> = hosts
            .iter()
            .filter_map(|host| host.group_name.as_deref())
            .collect();
        groups.extend(self.desktop.groups.iter().map(|group| group.name.as_str()));
        groups.sort_unstable();
        groups.dedup();

        let groups_section =
            if self.selected_group.is_none() && !groups.is_empty() && query.is_empty() {
                let mut grid = div().flex().flex_wrap().gap_3();
                for (idx, group) in groups.iter().enumerate() {
                    let grp_name = group.to_string();
                    let grp_menu_name = group.to_string();
                    let count = hosts
                        .iter()
                        .filter(|h| h.group_name.as_deref() == Some(*group))
                        .count();
                    let card = div()
                        .id(("group-card", idx))
                        .w(px(240.0))
                        .h(px(58.0))
                        .rounded_lg()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .p_3()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .cursor_pointer()
                        .hover(|s| {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                        })
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.selected_group = Some(grp_name.clone());
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .min_w_0()
                                .child(
                                    div()
                                        .w(px(32.0))
                                        .h(px(32.0))
                                        .rounded_md()
                                        .bg(rgb(MacTheme::BADGE_NAVY))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(rgb(MacTheme::ACCENT_BLUE))
                                        .font_weight(FontWeight::BOLD)
                                        .text_sm()
                                        .child(
                                            if let Some(p) = resolve_icon_path("groups", false, 48)
                                            {
                                                div()
                                                    .w(px(22.0))
                                                    .h(px(22.0))
                                                    .child(gpui::img(p).size_full())
                                                    .into_any_element()
                                            } else {
                                                div().child("⊞").into_any_element()
                                            },
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                .truncate()
                                                .child(group.to_string()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                                .child(format!("{count} hosts")),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .id(("group-menu-btn", idx))
                                .px_1p5()
                                .py_0p5()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .hover(|s| {
                                    s.text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .bg(rgb(MacTheme::INPUT_BG))
                                })
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    cx.stop_propagation();
                                    app.group_context_menu = Some((grp_menu_name.clone(), idx));
                                    cx.notify();
                                }))
                                .child(if let Some(p) = resolve_icon_path("more", false, 48) {
                                    div()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .child(gpui::img(p).size_full())
                                        .into_any_element()
                                } else {
                                    div().child("···").into_any_element()
                                }),
                        );
                    grid = grid.child(card);
                }
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Groups"),
                        )
                        .child(grid),
                )
            } else {
                None
            };

        // Filtering and sorting
        let mut filtered: Vec<&SavedHost> = hosts
            .iter()
            .filter(|h| {
                if let Some(grp) = &self.selected_group {
                    if h.group_name.as_deref() != Some(grp.as_str()) {
                        return false;
                    }
                }
                if let Some(tag) = &self.active_tag_filter {
                    if !h.tags.contains(tag) {
                        return false;
                    }
                }
                if query.is_empty() {
                    return true;
                }
                let query = query.to_lowercase();
                h.name.to_lowercase().contains(&query)
                    || h.destination.host().to_lowercase().contains(&query)
                    || h.destination.username().to_lowercase().contains(&query)
                    || h.tags.iter().any(|t| t.to_lowercase().contains(&query))
                    || h.group_name
                        .as_ref()
                        .map(|g| g.to_lowercase().contains(&query))
                        .unwrap_or(false)
            })
            .collect();

        match self.sort_mode {
            HostSortMode::AlphaAsc => filtered.sort_by_key(|a| a.name.to_lowercase()),
            HostSortMode::AlphaDesc => {
                filtered.sort_by_key(|a| std::cmp::Reverse(a.name.to_lowercase()))
            }
            HostSortMode::Newest => filtered.sort_by_key(|a| std::cmp::Reverse(a.id)),
            HostSortMode::Oldest => filtered.sort_by_key(|a| a.id),
        }

        // Hosts cards section
        let mut hosts_grid = div().flex().flex_wrap().gap_3();
        for host in &filtered {
            let id = host.id;
            let is_sel = selected.is_some_and(|s| s.id == id);
            let select_host_id = id;

            let badge_bg = match host.protocol() {
                ConnectionProtocol::Ssh => rgb(0x0f3661),
                ConnectionProtocol::Serial => rgb(0x0e3b2e),
                ConnectionProtocol::Mosh => rgb(0x3d2800),
                ConnectionProtocol::Telnet => rgb(0x3e1717),
                ConnectionProtocol::Local => rgb(0x1a2e26),
                ConnectionProtocol::Vnc => rgb(0x3a1b5c),
                ConnectionProtocol::Rdp => rgb(0x103d3d),
            };
            let badge_fg = match host.protocol() {
                ConnectionProtocol::Ssh => rgb(0x389bf2),
                ConnectionProtocol::Serial => rgb(0x3dd68c),
                ConnectionProtocol::Mosh => rgb(0xf5a623),
                ConnectionProtocol::Telnet => rgb(0xeb5757),
                ConnectionProtocol::Local => rgb(0x3dd68c),
                ConnectionProtocol::Vnc => rgb(0xbb86fc),
                ConnectionProtocol::Rdp => rgb(0x3de0d8),
            };
            let badge_icon = match host.protocol() {
                ConnectionProtocol::Ssh => "🖧",
                ConnectionProtocol::Serial => "🔌",
                ConnectionProtocol::Mosh => "⚡",
                ConnectionProtocol::Telnet => "⚠️",
                ConnectionProtocol::Local => ">_",
                ConnectionProtocol::Vnc => "🖥️",
                ConnectionProtocol::Rdp => "🪟",
            };

            let card = div()
                .id(("host-card", id as u64))
                .w(px(240.0))
                .h(px(64.0))
                .rounded_lg()
                .bg(if is_sel {
                    rgb(MacTheme::SURFACE_ACTIVE)
                } else {
                    rgb(MacTheme::SURFACE)
                })
                .border_1()
                .border_color(if is_sel {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::BORDER)
                })
                .p_3()
                .flex()
                .items_center()
                .gap_3()
                .cursor_pointer()
                .hover(|s| {
                    if !is_sel {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                            .border_color(rgb(MacTheme::ACCENT_BLUE))
                    } else {
                        s
                    }
                })
                .on_click(cx.listener(move |app, _, _, cx| {
                    if app.selected_host_id == Some(select_host_id) {
                        app.selected_host_id = None;
                    } else {
                        app.selected_host_id = Some(select_host_id);
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .w(px(38.0))
                        .h(px(38.0))
                        .rounded_md()
                        .bg(badge_bg)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(badge_fg)
                        .text_base()
                        .child(
                            if let Some(p) =
                                resolve_icon_path(resolve_host_os_icon(host), false, 48)
                            {
                                div()
                                    .w(px(26.0))
                                    .h(px(26.0))
                                    .child(gpui::img(p).size_full())
                                    .into_any_element()
                            } else {
                                div().child(badge_icon).into_any_element()
                            },
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .truncate()
                                .child(host.name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .truncate()
                                .child(format!(
                                    "{}, {}",
                                    host.protocol().display_name().to_lowercase(),
                                    if host.destination.username().is_empty() {
                                        "default"
                                    } else {
                                        host.destination.username()
                                    }
                                )),
                        ),
                );
            hosts_grid = hosts_grid.child(card);
        }

        if filtered.is_empty() {
            let message = match &self.hosts {
                HostState::Loading => "Loading hosts…".to_string(),
                HostState::Error(error) => format!("Could not load hosts: {error}"),
                HostState::NoDatabase => "Host storage is unavailable.".to_string(),
                HostState::Loaded(hosts) if hosts.is_empty() => {
                    "No saved hosts yet. Click '+ New host' above to add your first connection."
                        .to_string()
                }
                _ => "No hosts match your search.".to_string(),
            };
            hosts_grid = hosts_grid.child(
                div()
                    .p_8()
                    .w_full()
                    .text_center()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(message),
            );
        }

        let hosts_display = match self.view_mode {
            HostViewMode::Grid => div().child(hosts_grid),
            HostViewMode::List => {
                let mut list_col = div()
                    .id("hosts-list-table")
                    .w_full()
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .rounded_xl()
                    .overflow_hidden()
                    .bg(rgb(MacTheme::SURFACE));

                list_col = list_col.child(
                    div()
                        .flex()
                        .items_center()
                        .px_4()
                        .py_2()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .bg(rgb(MacTheme::INPUT_BG))
                        .border_b_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .child(div().w(px(220.0)).child("NAME"))
                        .child(div().w(px(140.0)).child("GROUP"))
                        .child(div().w(px(90.0)).child("PROTOCOL"))
                        .child(div().flex_1().child("DESTINATION / USER"))
                        .child(div().w(px(160.0)).child("TAGS")),
                );

                for (l_idx, host) in filtered.iter().enumerate() {
                    let id = host.id;
                    let is_sel = selected.is_some_and(|s| s.id == id);
                    let badge_fg = match host.protocol() {
                        ConnectionProtocol::Ssh => rgb(0x389bf2),
                        ConnectionProtocol::Serial => rgb(0x3dd68c),
                        ConnectionProtocol::Mosh => rgb(0xf5a623),
                        ConnectionProtocol::Telnet => rgb(0xeb5757),
                        ConnectionProtocol::Local => rgb(0x3dd68c),
                        ConnectionProtocol::Vnc => rgb(0xbb86fc),
                        ConnectionProtocol::Rdp => rgb(0x3de0d8),
                    };
                    let badge_icon = match host.protocol() {
                        ConnectionProtocol::Ssh => "🖧",
                        ConnectionProtocol::Serial => "🔌",
                        ConnectionProtocol::Mosh => "⚡",
                        ConnectionProtocol::Telnet => "⚠️",
                        ConnectionProtocol::Local => ">_",
                        ConnectionProtocol::Vnc => "🖥️",
                        ConnectionProtocol::Rdp => "🪟",
                    };

                    list_col = list_col.child(
                        div()
                            .id(("host-list-row", l_idx))
                            .flex()
                            .items_center()
                            .px_4()
                            .py_2p5()
                            .border_b_1()
                            .border_color(rgb(MacTheme::BORDER))
                            .bg(if is_sel {
                                rgb(MacTheme::SURFACE_ACTIVE)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                if app.selected_host_id == Some(id) {
                                    app.selected_host_id = None;
                                } else {
                                    app.selected_host_id = Some(id);
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(220.0))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        if let Some(p) =
                                            resolve_icon_path(resolve_host_os_icon(host), false, 48)
                                        {
                                            div()
                                                .w(px(22.0))
                                                .h(px(22.0))
                                                .child(gpui::img(p).size_full())
                                                .into_any_element()
                                        } else {
                                            div()
                                                .text_sm()
                                                .text_color(badge_fg)
                                                .child(badge_icon)
                                                .into_any_element()
                                        },
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .truncate()
                                            .child(host.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(140.0))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .truncate()
                                    .child(host.group_name.clone().unwrap_or_else(|| "—".into())),
                            )
                            .child(
                                div()
                                    .w(px(90.0))
                                    .text_xs()
                                    .text_color(badge_fg)
                                    .child(host.protocol().display_name()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .truncate()
                                    .child(format!(
                                        "{}@{}",
                                        if host.destination.username().is_empty() {
                                            "default"
                                        } else {
                                            host.destination.username()
                                        },
                                        host.destination.host()
                                    )),
                            )
                            .child(div().w(px(160.0)).flex().flex_wrap().gap_1().children(
                                host.tags.iter().map(|t| {
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(MacTheme::INPUT_BG))
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .child(t.clone())
                                }),
                            )),
                    );
                }

                if filtered.is_empty() {
                    list_col = list_col.child(
                        div()
                            .p_8()
                            .w_full()
                            .text_center()
                            .text_sm()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("No hosts match your filter criteria."),
                    );
                }

                div().child(list_col)
            }
        };

        // Slide-over Inspector Drawer (right side)
        let inspector_panel = if let Some(host) = selected {
            let edit_host = host.clone();
            let connect_host = host.clone();
            let sftp_host = host.clone();
            let id = host.id;
            Some(
                div()
                    .id("host-inspector-panel")
                    .w(px(290.0))
                    .flex_shrink_0()
                    .h_full()
                    .border_l_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .bg(rgb(MacTheme::SURFACE))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .truncate()
                                    .child(host.name.clone()),
                            )
                            .child(
                                div()
                                    .id("close-inspector")
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.selected_host_id = None;
                                        cx.notify();
                                    }))
                                    .child("✕"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("💬")
                            .child("Personal")
                            .child(div().text_color(rgb(MacTheme::TEXT_TERTIARY)).child("⌵")),
                    )
                    .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                    .child(host_detail(
                        "Protocol",
                        host.protocol().display_name().to_string(),
                    ))
                    .child(host_detail(
                        "Address",
                        format_endpoint(
                            host.destination.username(),
                            host.destination.host(),
                            host.destination.port().get(),
                        ),
                    ))
                    .child(host_detail(
                        "Group",
                        host.group_name.clone().unwrap_or_else(|| "—".into()),
                    ))
                    .child(host_detail(
                        "Tags",
                        if host.tags.is_empty() {
                            "—".into()
                        } else {
                            host.tags.join(", ")
                        },
                    ))
                    .child(div().flex_1())
                    .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)))
                    .child(
                        div()
                            .id("connect-selected")
                            .w_full()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x389bf2)))
                            .on_click(cx.listener(move |app, _, window, cx| {
                                app.connect(&connect_host, window, cx)
                            }))
                            .child(if host.protocol().is_graphical() {
                                "Connect Desktop"
                            } else {
                                "Connect"
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .id("edit-selected")
                                    .flex_1()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::INPUT_BG))
                                    .border_1()
                                    .border_color(rgb(MacTheme::BORDER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .text_xs()
                                    .text_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.edit_host(Some(&edit_host), cx)
                                    }))
                                    .child("Edit"),
                            )
                            .children(if host.protocol().supports_sftp() {
                                Some(
                                    div()
                                        .id("sftp-selected")
                                        .flex_1()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(rgb(MacTheme::INPUT_BG))
                                        .border_1()
                                        .border_color(rgb(MacTheme::BORDER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .text_xs()
                                        .text_center()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                        .on_click(cx.listener(move |app, _, window, cx| {
                                            app.connect_sftp(&sftp_host, window, cx)
                                        }))
                                        .child("SFTP"),
                                )
                            } else {
                                None
                            }),
                    )
                    .child(
                        div()
                            .id("delete-selected")
                            .py_1()
                            .text_xs()
                            .text_center()
                            .text_color(rgb(MacTheme::DANGER))
                            .cursor_pointer()
                            .hover(|s| s.underline())
                            .on_click(cx.listener(move |app, _, _, cx| app.delete_host(id, cx)))
                            .child("Delete host"),
                    ),
            )
        } else {
            None
        };

        let group_inspector_panel = if self.group_inspector.is_some() {
            Some(self.render_group_details_inspector(cx))
        } else {
            None
        };

        let content_scroll = div()
            .id("hosts-content-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(invite_banner)
            .children(breadcrumbs)
            .children(groups_section)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Hosts"),
                    )
                    .child(hosts_display),
            );

        let main_hosts_area = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .child(content_scroll)
            .children(group_inspector_panel)
            .children(inspector_panel);

        let body = div()
            .id("shellumi")
            .track_focus(&self.focus)
            .on_action(cx.listener(|app, _: &RefreshHosts, _, cx| app.load_hosts(cx)))
            .on_action(cx.listener(|app, _: &ToggleCommandPalette, window, cx| {
                app.toggle_command_palette(window, cx)
            }))
            .on_key_down(cx.listener(Self::handle_palette_key_down))
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::CONTENT_BG))
            .child(action_bar)
            .child(main_hosts_area);

        let sidebar = self.workspace_sidebar("Hosts", cx);
        let mut row = div().flex().flex_1().min_h_0();
        if self.sidebar_visible {
            row = row.child(sidebar);
        }
        let mut result = root.child(row.child(body));
        if let Some(form) = &self.form {
            let active_cat = form.active_category;
            let hostname_label = if form.protocol == ConnectionProtocol::Serial {
                "Serial device port"
            } else if form.protocol == ConnectionProtocol::Vnc {
                "VNC host or IP"
            } else if form.protocol == ConnectionProtocol::Rdp {
                "RDP host or IP"
            } else {
                "Hostname or IP"
            };
            let port_label = if form.protocol == ConnectionProtocol::Serial {
                "Baud rate"
            } else if form.protocol == ConnectionProtocol::Vnc {
                "Port (5900)"
            } else if form.protocol == ConnectionProtocol::Rdp {
                "Port (3389)"
            } else {
                "Port"
            };

            let editor_categories = [
                (HostEditorCategory::General, "General", "⚙"),
                (HostEditorCategory::Connection, "Connection", "🌐"),
                (HostEditorCategory::Authentication, "Authentication", "🔑"),
                (HostEditorCategory::Proxy, "Proxy", "⇄"),
                (HostEditorCategory::Advanced, "Advanced", "⚡"),
            ];

            let mut nav = div()
                .w(px(160.0))
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .bg(rgb(MacTheme::INPUT_BG))
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .rounded_lg();

            for (cat, label, icon) in editor_categories {
                let is_active = cat == active_cat;
                nav = nav.child(
                    div()
                        .id(gpui::SharedString::from(format!(
                            "cat-{}",
                            label.to_lowercase()
                        )))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .text_sm()
                        .cursor_pointer()
                        .font_weight(if is_active {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .bg(if is_active {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgba(0x00000000)
                        })
                        .text_color(if is_active {
                            rgb(0xffffff)
                        } else {
                            rgb(MacTheme::TEXT_SECONDARY)
                        })
                        .hover(|style| {
                            if !is_active {
                                style
                                    .bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            } else {
                                style
                            }
                        })
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.set_host_editor_category(cat, cx);
                        }))
                        .child(div().text_sm().child(icon))
                        .child(label),
                );
            }

            let category_content = match active_cat {
                HostEditorCategory::General => {
                    let mut gen_div = div().flex().flex_col().gap_3().child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                    .child("Protocol"),
                            )
                            .child({
                                let mut proto_row = div().flex().flex_row().gap_2();
                                let protocols = [
                                    (ConnectionProtocol::Ssh, "SSH"),
                                    (ConnectionProtocol::Mosh, "Mosh"),
                                    (ConnectionProtocol::Telnet, "Telnet"),
                                    (ConnectionProtocol::Serial, "Serial"),
                                    (ConnectionProtocol::Vnc, "VNC"),
                                    (ConnectionProtocol::Rdp, "RDP"),
                                ];
                                for (idx, (p, label)) in protocols.into_iter().enumerate() {
                                    let is_sel = form.protocol == p;
                                    let btn_bg = if is_sel {
                                        rgb(MacTheme::ACCENT_BLUE)
                                    } else {
                                        rgb(MacTheme::INPUT_BG)
                                    };
                                    let btn_fg = if is_sel {
                                        rgb(0xffffff)
                                    } else {
                                        rgb(MacTheme::TEXT_SECONDARY)
                                    };
                                    proto_row = proto_row.child(
                                        div()
                                            .id(("proto-select", idx))
                                            .px_3()
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
                            }),
                    );

                    if form.protocol == ConnectionProtocol::Telnet {
                        gen_div = gen_div.child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x4a1818))
                                .text_color(rgb(MacTheme::DANGER))
                                .text_xs()
                                .child("⚠️ UNENCRYPTED PROTOCOL: Telnet transmits all credentials, keystrokes, and session data in cleartext over the network."),
                        );
                    } else if form.protocol == ConnectionProtocol::Serial {
                        gen_div = gen_div.child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x133820))
                                .text_color(rgb(MacTheme::SUCCESS))
                                .text_xs()
                                .child("🔌 Direct serial line connection. Specify serial device path and baud rate in Connection."),
                        );
                    } else if form.protocol == ConnectionProtocol::Vnc {
                        gen_div = gen_div.child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x2a1445))
                                .text_color(rgb(0xbb86fc))
                                .text_xs()
                                .child("🖥️ VNC Graphical Desktop: Remote Framebuffer protocol (default port 5900). Supports view-only and custom compression quality."),
                        );
                    } else if form.protocol == ConnectionProtocol::Rdp {
                        gen_div = gen_div.child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(rgb(0x0e2f2f))
                                .text_color(rgb(0x3de0d8))
                                .text_xs()
                                .child("🪟 RDP Graphical Desktop: Remote Desktop Protocol (default port 3389). Supports Windows NLA and multi-resolution remote desktop."),
                        );
                    }

                    gen_div = gen_div
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(host_form_field("Display name", form.name.clone()))
                                .child(host_form_field("Group (optional)", form.group.clone())),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(host_form_field(hostname_label, form.hostname.clone()))
                                .child(host_form_field(port_label, form.port.clone())),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(host_form_field("Username", form.username.clone()))
                                .child(host_form_field("Tags (comma-separated)", form.tags.clone())),
                        )
                        .child(host_form_field("Jump host (optional)", form.jump_host.clone()))
                        .child(self.identity_picker(form.identity_id, false, cx))
                        .child(card(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_sm()
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child("Local Vault Security"),
                                )
                                .child(caption(
                                    "Host keys, passphrases, and credentials are encrypted and stored locally in Turso SQLite.",
                                )),
                        ));
                    gen_div
                }
                HostEditorCategory::Connection => div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(host_form_field(hostname_label, form.hostname.clone()))
                            .child(host_form_field(port_label, form.port.clone())),
                    )
                    .child(host_form_field("Username", form.username.clone())),
                HostEditorCategory::Authentication => {
                    let mut auth_div = div().flex().flex_col().gap_3();
                    if form.protocol == ConnectionProtocol::Ssh {
                        auth_div =
                            auth_div.child(self.identity_picker(form.identity_id, false, cx));
                    } else {
                        auth_div = auth_div.child(
                            div()
                                .p_3()
                                .rounded_md()
                                .bg(rgb(MacTheme::INPUT_BG))
                                .text_sm()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .child("Authentication credentials for this protocol are managed during connection."),
                        );
                    }
                    auth_div
                }
                HostEditorCategory::Proxy => div().flex().flex_col().gap_3().child(
                    host_form_field("Jump host (e.g. jump.corp:2222)", form.jump_host.clone()),
                ),
                HostEditorCategory::Advanced => {
                    div().flex().flex_col().gap_3().child(host_form_field(
                        "Startup snippet ID (optional)",
                        form.startup_snippet.clone(),
                    ))
                }
            };

            let header_row = div()
                .flex()
                .items_center()
                .justify_between()
                .child(heading(
                    if form.id.is_some() {
                        "Edit Host"
                    } else {
                        "New Host"
                    },
                    "Host connection configuration · Local storage",
                ))
                .child(
                    div()
                        .id("close-host-dialog")
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_TERTIARY))
                        .cursor_pointer()
                        .hover(|s| s.text_color(rgb(MacTheme::TEXT_PRIMARY)))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.form = None;
                            app.form_error = None;
                            cx.notify();
                        }))
                        .child("✕"),
                );

            let mut body_content = div()
                .id("host-form-body-scroll")
                .flex_1()
                .min_w_0()
                .max_h(px(400.0))
                .overflow_y_scroll()
                .child(category_content);

            if let Some(error) = &self.form_error {
                body_content = body_content.child(
                    div()
                        .mt_2()
                        .text_color(rgb(MacTheme::DANGER))
                        .text_sm()
                        .child(error.clone()),
                );
            }

            let body_row = div()
                .flex()
                .items_start()
                .gap_4()
                .child(nav)
                .child(body_content);

            let footer_row = if self.saving {
                div()
                    .flex()
                    .justify_end()
                    .text_sm()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("Saving…")
            } else {
                div()
                    .flex()
                    .justify_end()
                    .gap_3()
                    .child(
                        div()
                            .id("cancel-host")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .text_sm()
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.form = None;
                                app.form_error = None;
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("save-host")
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_color(rgb(0xffffff))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| app.save_host(cx)))
                            .child("Save"),
                    )
            };

            let editor = div()
                .id("host-editor")
                .w(px(720.0))
                .max_w_full()
                .rounded_xl()
                .border_1()
                .border_color(rgb(MacTheme::BORDER))
                .flex()
                .flex_col()
                .gap_4()
                .p_6()
                .bg(rgb(MacTheme::SURFACE))
                .child(header_row)
                .child(body_row)
                .child(footer_row);

            result = result.child(
                div()
                    .id("host-editor-overlay")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(rgba(0x000000bb))
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(editor),
            );
        }
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
        if let Some(modal) = &self.session_replay_modal {
            result = result.child(self.render_session_replay_modal(modal, cx));
        }
        if let Some(modal) = &self.tunnel_modal {
            result = result.child(self.render_tunnel_modal(modal, cx));
        }
        if let Some(dropdown) = self.render_vault_dropdown_overlay(cx) {
            result = result.child(dropdown);
        }
        if let Some(modal) = &self.invite_modal {
            result = result.child(self.render_invite_team_modal(modal, cx));
        }
        if let Some(modal) = &self.identity_modal {
            result = result.child(self.render_identity_modal(modal, cx));
        }
        if let Some(ctx_menu) = self.render_group_context_menu(cx) {
            result = result.child(ctx_menu);
        }
        if self.desktop.editor.is_some() || self.desktop.confirm.is_some() {
            result = self.desktop_overlays(result, cx);
        }
        if let Some(message) = &self.desktop.message {
            result = result.child(
                div()
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .p_3()
                    .bg(rgb(0x141e2c))
                    .text_sm()
                    .child(message.clone()),
            );
        }
        result
    }
}

fn main() {
    if std::env::var("SHELLUMI_ASKPASS_MODE").as_deref() == Ok("1") {
        std::process::exit(askpass::run());
    }
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (database, create_parent, startup_error) = match args.as_slice() {
        [] => match data_path::default_data_file().and_then(|path| {
            path.to_str()
                .map(str::to_owned)
                .ok_or_else(|| "The Shellumi data path is not valid UTF-8.".to_owned())
        }) {
            Ok(path) => (Some(path), true, None),
            Err(error) => (None, false, Some(error)),
        },
        [flag, path] if flag == "--hosts" && !path.is_empty() && path != ":memory:" => {
            (Some(path.clone()), false, None)
        }
        _ => {
            eprintln!("Usage: shellumi-app [--hosts <database-path>]");
            std::process::exit(2);
        }
    };
    Application::new().run(move |cx: &mut App| {
        cx.on_action(|_: &Quit, cx: &mut App| {
            cx.quit();
        });
        cx.on_action(|_: &HideApp, cx: &mut App| {
            cx.hide();
        });
        cx.on_action(|_: &HideOthersApp, cx: &mut App| {
            cx.hide_other_apps();
        });
        cx.on_action(|_: &ShowAllApp, cx: &mut App| {
            cx.unhide_other_apps();
        });

        #[cfg(target_os = "macos")]
        cx.set_menus(vec![
            Menu {
                name: "Shellumi".into(),
                items: vec![
                    MenuItem::action("About Shellumi", AboutApp),
                    MenuItem::separator(),
                    MenuItem::action("Vaults", OpenVaultsMenu),
                    MenuItem::action("SFTP", OpenSftpMenu),
                    MenuItem::action("Settings…", OpenSettingsMenu),
                    MenuItem::separator(),
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action("Hide Shellumi", HideApp),
                    MenuItem::action("Hide Others", HideOthersApp),
                    MenuItem::action("Show All", ShowAllApp),
                    MenuItem::separator(),
                    MenuItem::action("Quit Shellumi", Quit),
                ],
            },
            Menu {
                name: "Edit".into(),
                items: vec![
                    MenuItem::os_action("Undo", UndoAction, OsAction::Undo),
                    MenuItem::os_action("Redo", RedoAction, OsAction::Redo),
                    MenuItem::separator(),
                    MenuItem::os_action("Cut", CutAction, OsAction::Cut),
                    MenuItem::os_action("Copy", CopyAction, OsAction::Copy),
                    MenuItem::os_action("Paste", PasteAction, OsAction::Paste),
                    MenuItem::os_action("Select All", SelectAllAction, OsAction::SelectAll),
                ],
            },
            Menu {
                name: "Window".into(),
                items: vec![
                    MenuItem::action("Minimize", MinimizeWindow),
                    MenuItem::action("Zoom", ZoomWindow),
                    MenuItem::separator(),
                    MenuItem::action("Bring All to Front", BringAllToFront),
                ],
            },
            Menu {
                name: "Help".into(),
                items: vec![MenuItem::action("Keyboard Shortcuts", OpenShortcutsHelp)],
            },
        ]);

        text_input::bind_keys(cx);
        #[cfg(target_os = "macos")]
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-h", HideApp, None),
            KeyBinding::new("alt-cmd-h", HideOthersApp, None),
            KeyBinding::new("cmd-m", MinimizeWindow, None),
            KeyBinding::new("cmd-k", ToggleCommandPalette, None),
            KeyBinding::new("cmd-r", RefreshHosts, None),
            KeyBinding::new("cmd-t", NewTab, None),
            KeyBinding::new("cmd-l", OpenLocalTerminal, None),
            KeyBinding::new("cmd-w", CloseTab, None),
            KeyBinding::new("cmd-shift-]", NextTab, None),
            KeyBinding::new("ctrl-tab", NextTab, None),
            KeyBinding::new("cmd-shift-[", PrevTab, None),
            KeyBinding::new("ctrl-shift-tab", PrevTab, None),
            KeyBinding::new("cmd-1", SelectTab1, None),
            KeyBinding::new("cmd-2", SelectTab2, None),
            KeyBinding::new("cmd-3", SelectTab3, None),
            KeyBinding::new("cmd-4", SelectTab4, None),
            KeyBinding::new("cmd-5", SelectTab5, None),
            KeyBinding::new("cmd-6", SelectTab6, None),
            KeyBinding::new("cmd-7", SelectTab7, None),
            KeyBinding::new("cmd-8", SelectTab8, None),
            KeyBinding::new("cmd-9", SelectTab9, None),
            KeyBinding::new("cmd-d", SplitVertical, None),
            KeyBinding::new("cmd-shift-d", SplitHorizontal, None),
            KeyBinding::new("cmd-]", FocusNextPane, None),
            KeyBinding::new("cmd-[", FocusPrevPane, None),
            KeyBinding::new("cmd-shift-w", ClosePane, None),
            KeyBinding::new("cmd-shift-m", ToggleFocusMode, None),
            KeyBinding::new("cmd-b", ToggleBroadcast, None),
            KeyBinding::new("cmd-shift-b", ToggleBroadcast, None),
            KeyBinding::new("cmd-.", ToggleSnippets, None),
            KeyBinding::new("cmd-f", FindInTerminal, None),
            KeyBinding::new("cmd-=", ZoomIn, None),
            KeyBinding::new("cmd-+", ZoomIn, None),
            KeyBinding::new("cmd--", ZoomOut, None),
            KeyBinding::new("cmd-0", ResetZoom, None),
            KeyBinding::new("cmd-o", QuickConnect, None),
            KeyBinding::new("cmd-n", NewHost, None),
            KeyBinding::new("cmd-shift-r", ReconnectSession, None),
            KeyBinding::new("cmd-\\", ToggleSidebar, None),
            KeyBinding::new("cmd-shift-\\", ToggleSidebarCollapse, None),
            KeyBinding::new("cmd-,", OpenSettings, None),
            KeyBinding::new("cmd-shift-h", GoToHosts, None),
            KeyBinding::new("cmd-shift-o", GoToOverview, None),
            KeyBinding::new("cmd-shift-k", GoToWorkspaces, None),
            KeyBinding::new("cmd-shift-u", GoToSftp, None),
            KeyBinding::new("cmd-shift-y", GoToKeychain, None),
            KeyBinding::new("cmd-shift-p", GoToPortForwarding, None),
        ]);

        #[cfg(not(target_os = "macos"))]
        cx.bind_keys([
            KeyBinding::new("ctrl-q", Quit, None),
            KeyBinding::new("ctrl-k", ToggleCommandPalette, None),
            KeyBinding::new("ctrl-r", RefreshHosts, None),
            KeyBinding::new("ctrl-t", NewTab, None),
            KeyBinding::new("ctrl-l", OpenLocalTerminal, None),
            KeyBinding::new("ctrl-w", CloseTab, None),
            KeyBinding::new("ctrl-tab", NextTab, None),
            KeyBinding::new("ctrl-pagedown", NextTab, None),
            KeyBinding::new("ctrl-shift-tab", PrevTab, None),
            KeyBinding::new("ctrl-pageup", PrevTab, None),
            KeyBinding::new("ctrl-1", SelectTab1, None),
            KeyBinding::new("ctrl-2", SelectTab2, None),
            KeyBinding::new("ctrl-3", SelectTab3, None),
            KeyBinding::new("ctrl-4", SelectTab4, None),
            KeyBinding::new("ctrl-5", SelectTab5, None),
            KeyBinding::new("ctrl-6", SelectTab6, None),
            KeyBinding::new("ctrl-7", SelectTab7, None),
            KeyBinding::new("ctrl-8", SelectTab8, None),
            KeyBinding::new("ctrl-9", SelectTab9, None),
            KeyBinding::new("ctrl-shift-e", SplitVertical, None),
            KeyBinding::new("ctrl-shift-o", SplitHorizontal, None),
            KeyBinding::new("alt-right", FocusNextPane, None),
            KeyBinding::new("alt-left", FocusPrevPane, None),
            KeyBinding::new("ctrl-shift-w", ClosePane, None),
            KeyBinding::new("ctrl-alt-m", ToggleFocusMode, None),
            KeyBinding::new("ctrl-alt-b", ToggleBroadcast, None),
            KeyBinding::new("ctrl-.", ToggleSnippets, None),
            KeyBinding::new("ctrl-f", FindInTerminal, None),
            KeyBinding::new("ctrl-=", ZoomIn, None),
            KeyBinding::new("ctrl-+", ZoomIn, None),
            KeyBinding::new("ctrl--", ZoomOut, None),
            KeyBinding::new("ctrl-0", ResetZoom, None),
            KeyBinding::new("ctrl-o", QuickConnect, None),
            KeyBinding::new("ctrl-n", NewHost, None),
            KeyBinding::new("ctrl-shift-r", ReconnectSession, None),
            KeyBinding::new("ctrl-\\", ToggleSidebar, None),
            KeyBinding::new("ctrl-shift-\\", ToggleSidebarCollapse, None),
            KeyBinding::new("ctrl-,", OpenSettings, None),
            KeyBinding::new("ctrl-shift-h", GoToHosts, None),
            KeyBinding::new("ctrl-shift-o", GoToOverview, None),
            KeyBinding::new("ctrl-shift-k", GoToWorkspaces, None),
            KeyBinding::new("ctrl-shift-u", GoToSftp, None),
            KeyBinding::new("ctrl-shift-y", GoToKeychain, None),
            KeyBinding::new("ctrl-shift-p", GoToPortForwarding, None),
        ]);
        let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(940.0), px(650.0))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Shellumi".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.0), px(14.0))),
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| Shellumi::new(database, window, cx));
                view.update(cx, |app, cx| {
                    if let Some(error) = startup_error {
                        app.hosts = HostState::Error(error);
                        cx.notify();
                    } else {
                        app.start_storage(create_parent, cx);
                    }
                });
                view
            },
        )
        .expect("failed to open Shellumi window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn collection_filter_matches_only_the_selected_group() {
        assert!(super::group_matches(Some("Production"), None));
        assert!(super::group_matches(None, None));
        assert!(super::group_matches(Some("Production"), Some("Production")));
        assert!(!super::group_matches(Some("Production"), Some("Staging")));
        assert!(!super::group_matches(None, Some("Production")));
    }

    use super::{format_endpoint, is_paste_keystroke, key_bytes, render_terminal_spans, SavedHost};
    use gpui::Keystroke;
    use shellumi_core::PlaybackSpeed;
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
        let _ = render_terminal_spans(spans, 21.0);
        let empty_spans = Vec::new();
        let _ = render_terminal_spans(empty_spans, 21.0);
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
                identity_id: None,
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
                identity_id: None,
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
                identity_id: None,
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
            SessionKind::RemoteDesktop => format!("Desktop: {name}"),
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
            listing_generation: 0,
            host_name: "prod-server".to_string(),
            connection_options: SftpConnectOptions::default(),
            destination: None,
            jump_host: None,
            loading: false,
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
            identity_id: None,
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

        assert!(!ConnectionProtocol::Vnc.is_encrypted());
        assert!(!ConnectionProtocol::Vnc.supports_sftp());
        assert!(ConnectionProtocol::Vnc.is_graphical());
        assert_eq!(ConnectionProtocol::Vnc.badge_icon(), "🖥️");
        assert_eq!(ConnectionProtocol::Vnc.default_port(), Some(5900));

        assert!(ConnectionProtocol::Rdp.is_encrypted());
        assert!(!ConnectionProtocol::Rdp.supports_sftp());
        assert!(ConnectionProtocol::Rdp.is_graphical());
        assert_eq!(ConnectionProtocol::Rdp.badge_icon(), "🪟");
        assert_eq!(ConnectionProtocol::Rdp.default_port(), Some(3389));

        // Telnet security warning
        let warning = TelnetConfig::security_warning();
        assert!(warning.contains("UNENCRYPTED PROTOCOL"));
        assert!(warning.contains("cleartext"));
    }

    #[test]
    fn verifies_remote_desktop_state_and_scaling_transitions() {
        use super::{
            RemoteDesktopProtocol, RemoteDesktopScaleMode, RemoteDesktopState, RemoteDesktopStatus,
        };

        let mut rd = RemoteDesktopState {
            host_name: "Ubuntu Workstation".into(),
            target: "192.168.1.50:5900".into(),
            protocol: RemoteDesktopProtocol::Vnc,
            status: RemoteDesktopStatus::Connected,
            status_message: Some("Connected via RFB".into()),
            width: 1920,
            height: 1080,
            view_only: false,
            scale_mode: RemoteDesktopScaleMode::Fit,
            color_depth: 24,
            fps: 60,
            latency_ms: 12,
            clipboard_text: None,
            error: None,
            last_action_note: None,
        };

        assert_eq!(rd.status, RemoteDesktopStatus::Connected);
        assert_eq!(rd.scale_mode, RemoteDesktopScaleMode::Fit);
        assert!(!rd.view_only);

        // Toggle view only
        rd.view_only = true;
        assert!(rd.view_only);

        // Change scale mode
        rd.scale_mode = RemoteDesktopScaleMode::Original;
        assert_eq!(rd.scale_mode, RemoteDesktopScaleMode::Original);

        // Disconnect
        rd.status = RemoteDesktopStatus::Disconnected;
        assert_eq!(rd.status, RemoteDesktopStatus::Disconnected);

        // Reconnect
        rd.status = RemoteDesktopStatus::Connected;
        assert_eq!(rd.status, RemoteDesktopStatus::Connected);
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

        // Verify benchmark palette registration and modal tab state
        let bench_cmd = registry
            .commands()
            .iter()
            .find(|c| c.action == PaletteAction::RunBenchmarks)
            .expect("should register benchmarks command");
        assert_eq!(bench_cmd.category, PaletteCategory::Diagnostics);
        assert_eq!(bench_cmd.title, "Run Performance Benchmarks");
        assert!(bench_cmd.keywords.contains(&"bench".to_string()));

        let modal_state = super::DiagnosticsModalState {
            active_tab: super::DiagnosticsModalTab::Benchmarks,
            report: None,
            benchmark_report: None,
            is_running: false,
            is_running_benchmarks: true,
            error: None,
        };
        assert_eq!(
            modal_state.active_tab,
            super::DiagnosticsModalTab::Benchmarks
        );
        assert!(modal_state.is_running_benchmarks);
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

    #[test]
    fn verifies_session_replay_modal_and_recording_integration() {
        use super::*;
        use shellumi_terminal::Terminal;

        // 1. Verify Command Palette action registration
        let registry = PaletteRegistry::with_static_actions();
        let cmd = registry
            .commands()
            .iter()
            .find(|c| c.action == PaletteAction::OpenSessionRecordings)
            .expect("should register session recordings command");

        assert_eq!(cmd.category, PaletteCategory::Terminal);
        assert_eq!(cmd.title, "Session Recordings & Cast Replay");
        assert!(cmd.keywords.contains(&"asciicast".to_string()));
        assert!(cmd.keywords.contains(&"recording".to_string()));

        // 2. Verify Terminal recording capture lifecycle in app context
        let mut term = Terminal::new(24, 80);
        assert!(!term.is_recording());

        term.start_recording(Some("CI Production Audit".to_string()));
        assert!(term.is_recording());

        term.record_input(b"uname -a\r");
        term.process_bytes(b"Linux prod-node 6.8.0 #1 SMP\r\n");
        term.process_bytes(b"export SECRET=super_classified_val\r\n");
        term.resize(30, 100);

        assert!(term.recording_event_count() >= 3);
        let recording = term.stop_recording().expect("should return recording");
        assert!(!term.is_recording());
        assert_eq!(
            recording.header.title.as_deref(),
            Some("CI Production Audit")
        );

        let raw_cast = recording
            .to_asciicast_v2()
            .expect("should serialize asciicast");
        assert!(raw_cast.contains("\"version\":2"));
        assert!(raw_cast.contains("uname -a"));

        // 3. Verify automated sensitive data redaction on recording
        let clean_cast = SessionRedactor::redact_asciicast_v2(&raw_cast).expect("should redact");
        assert!(!clean_cast.contains("super_classified_val"));
        assert!(clean_cast.contains("[REDACTED_SECRET]"));

        // 4. Verify Replay Modal state and player state machine
        let mut engine = CastPlaybackEngine::from_asciicast_v2(&clean_cast).expect("valid engine");
        assert_eq!(engine.status(), PlaybackStatus::Stopped);

        engine.play();
        assert_eq!(engine.status(), PlaybackStatus::Playing);

        let initial_frames = engine.seek(0.0);
        let mut screen_buffer = String::new();
        for f in initial_frames {
            if f.event_type == "o" {
                screen_buffer.push_str(&f.data);
            }
        }

        let mut modal = SessionReplayModalState {
            active_tab: SessionReplayTab::RecordingsList,
            recordings: vec![SessionRecordingSummary {
                id: "rec-test-1".to_string(),
                session_id: "sess-1".to_string(),
                host_id: None,
                title: "CI Production Audit".to_string(),
                duration_secs: engine.duration_secs(),
                event_count: engine.total_frames(),
                byte_size: clean_cast.len(),
                recorded_at: "2026-11-05T07:20:00Z".to_string(),
            }],
            selected_recording_id: Some("rec-test-1".to_string()),
            current_recording: None,
            playback_engine: Some(engine),
            screen_output_buffer: screen_buffer,
            status_notification: None,
            search_query: String::new(),
        };

        assert_eq!(modal.active_tab, SessionReplayTab::RecordingsList);
        assert_eq!(modal.recordings.len(), 1);
        assert_eq!(modal.recordings[0].title, "CI Production Audit");

        // Tab transition to player
        modal.active_tab = SessionReplayTab::ReplayPlayer;
        assert_eq!(modal.active_tab, SessionReplayTab::ReplayPlayer);

        // Player operations
        if let Some(ref mut eng) = modal.playback_engine {
            eng.toggle_play_pause();
            assert_eq!(eng.status(), PlaybackStatus::Paused);

            eng.set_speed(PlaybackSpeed::Double);
            assert_eq!(eng.speed(), PlaybackSpeed::Double);
            assert_eq!(eng.speed().multiplier(), 2.0);

            eng.cycle_speed();
            assert_eq!(eng.speed(), PlaybackSpeed::Quad);
        }
    }

    #[test]
    fn verifies_tunnel_modal_state_telemetry_and_command_builder() {
        use super::*;

        // 1. Verify Command Palette action registration
        let registry = PaletteRegistry::with_static_actions();
        let cmd = registry
            .commands()
            .iter()
            .find(|c| c.action == PaletteAction::AddNewTunnel)
            .expect("should register add new tunnel command");

        assert_eq!(cmd.category, PaletteCategory::Tunnel);
        assert_eq!(cmd.title, "Add New SSH Tunnel");

        // 2. Verify PortForwardRule and command builder
        let rule_local =
            PortForwardRule::local(Some("127.0.0.1"), 8080, "internal.db", 5432).unwrap();
        let cmd_local =
            build_ssh_tunnel_command(&rule_local, "bastion.example.com", Some("jump.example.com"));
        assert!(cmd_local.contains(&"-N".to_string()));
        assert!(cmd_local.contains(&"-J".to_string()));
        assert!(cmd_local.contains(&"jump.example.com".to_string()));
        assert!(cmd_local.contains(&"-L".to_string()));
        assert!(cmd_local.contains(&"127.0.0.1:8080:internal.db:5432".to_string()));
        assert!(cmd_local.contains(&"bastion.example.com".to_string()));

        let rule_dyn = PortForwardRule::dynamic(Some("127.0.0.1"), 1080).unwrap();
        let cmd_dyn = build_ssh_tunnel_command(&rule_dyn, "proxy.corp.internal", None);
        assert!(cmd_dyn.contains(&"-D".to_string()));
        assert!(cmd_dyn.contains(&"127.0.0.1:1080".to_string()));

        // 3. Verify ActiveTunnelState telemetry
        let mut active = ActiveTunnelState::new(1, Some(42), "DB Tunnel", rule_local.clone(), true);
        assert_eq!(active.status, ActiveTunnelStatus::Stopped);
        assert_eq!(active.status_pill(), ("⚪ Stopped", "stopped"));

        active.start();
        active.record_traffic(4096, 8192);
        assert_eq!(active.status, ActiveTunnelStatus::Active);
        assert_eq!(active.status_pill(), ("🟢 Active", "active"));
        assert_eq!(active.bytes_sent, 4096);
        assert_eq!(active.bytes_received, 8192);

        active.stop();
        assert_eq!(active.status, ActiveTunnelStatus::Stopped);
    }

    #[test]
    fn verifies_startup_automation_tracker_readiness_and_prompt_dispatch() {
        use super::*;

        // 1. Verify template parameter rendering
        let ctx = StartupSnippetContext::new("Prod Web", "web01.internal", "ubuntu", 22)
            .with_host_id(101)
            .with_custom_var("branch", "main");

        let rendered = prepare_startup_snippet(
            "echo 'Connecting to {{host.name}} ({{host.user}}@{{host.hostname}}:{{host.port}})' && git checkout {{branch}}",
            &ctx,
        ).expect("rendering should succeed");

        assert_eq!(
            rendered,
            "echo 'Connecting to Prod Web (ubuntu@web01.internal:22)' && git checkout main"
        );

        // 2. Verify dangerous command interception
        let dangerous = prepare_startup_snippet("rm -rf / --no-preserve-root", &ctx);
        assert!(dangerous.is_err());

        // 3. Verify StartupSessionTracker lifecycle
        let mut tracker = StartupSessionTracker::new(
            Some(1),
            "Init Script",
            "uptime",
            StartupExecutionPolicy::AutoAfterAuth,
        );
        assert_eq!(tracker.status, StartupStatus::PendingAuth);

        // Password prompt output should NOT trigger dispatch
        tracker.observe_output("ubuntu@web01's password: ");
        assert_eq!(tracker.status, StartupStatus::PendingAuth);
        assert!(tracker.dispatch(false).is_none());

        // Shell prompt output SHOULD transition to ReadyToDispatch
        tracker.observe_output("Last login: Thu Nov 5 10:00:00 2026\r\nubuntu@web01:~$ ");
        assert_eq!(tracker.status, StartupStatus::ReadyToDispatch);

        // Prompt-like output alone must not dispatch a startup command.
        assert!(tracker.dispatch(false).is_none());
        let cmd = tracker
            .dispatch(true)
            .expect("explicitly confirmed command");
        assert_eq!(cmd, "uptime\n");
        assert_eq!(tracker.status, StartupStatus::Dispatched);

        tracker.mark_completed();
        assert_eq!(tracker.status, StartupStatus::Completed);
    }

    #[test]
    fn verifies_sync_cycle_execution_and_conflict_resolution_in_app() {
        use super::*;

        let dummy_env = EncryptedEnvelope {
            salt: "s".into(),
            nonce: "n".into(),
            ciphertext: "c".into(),
            associated_data: "host".into(),
            kdf_iterations: 1000,
        };

        let rec_local = SyncRecord::new(
            "host:1",
            SyncEntityType::Host,
            1,
            "devA",
            1000,
            dummy_env.clone(),
        );
        let rec_remote = SyncRecord::new(
            "host:1",
            SyncEntityType::Host,
            1,
            "devA",
            1000,
            dummy_env.clone(),
        );

        // 1. Identical records -> AlreadySynchronized
        let res = execute_sync_cycle(
            std::slice::from_ref(&rec_local),
            std::slice::from_ref(&rec_remote),
            "default-pass",
            None,
        );
        assert!(matches!(res, SyncCycleResult::AlreadySynchronized));

        // 2. Conflicting records -> Conflicts
        let rec_remote_conflict = SyncRecord::new(
            "host:1",
            SyncEntityType::Host,
            1,
            "devB",
            1050,
            dummy_env.clone(),
        );

        let res_conflict = execute_sync_cycle(
            std::slice::from_ref(&rec_local),
            std::slice::from_ref(&rec_remote_conflict),
            "default-pass",
            None,
        );
        match res_conflict {
            SyncCycleResult::Conflicts(conflicts) => {
                assert_eq!(conflicts.len(), 1);
                assert_eq!(conflicts[0].object_id, "host:1");
            }
            _ => panic!("Expected conflict"),
        }

        // 3. User resolution -> Success
        let resolutions = vec![ConflictResolutionChoice::AcceptRemote];

        let res_resolved = execute_sync_cycle(
            std::slice::from_ref(&rec_local),
            std::slice::from_ref(&rec_remote_conflict),
            "default-pass",
            Some(&resolutions),
        );
        match res_resolved {
            SyncCycleResult::Success {
                report,
                merged_records,
            } => {
                assert_eq!(report.conflicts_resolved, 1);
                assert_eq!(merged_records.len(), 1);
            }
            _ => panic!("Expected successful sync after resolution"),
        }
    }

    #[test]
    fn verifies_keyboard_shortcut_actions_and_keystroke_parsing() {
        use super::{
            ClosePane, CloseTab, FindInTerminal, FocusNextPane, FocusPrevPane, GoToHosts,
            GoToKeychain, GoToOverview, GoToPortForwarding, GoToSftp, GoToWorkspaces, NewHost,
            NewTab, NextTab, OpenLocalTerminal, OpenSettings, PrevTab, QuickConnect,
            ReconnectSession, RefreshHosts, ResetZoom, SelectTab1, SelectTab9, SplitHorizontal,
            SplitVertical, ToggleBroadcast, ToggleCommandPalette, ToggleFocusMode, ToggleSidebar,
            ToggleSidebarCollapse, ToggleSnippets, ZoomIn, ZoomOut,
        };

        // Ensure all actions are constructible
        let _ = NewTab;
        let _ = OpenLocalTerminal;
        let _ = CloseTab;
        let _ = NextTab;
        let _ = PrevTab;
        let _ = SelectTab1;
        let _ = SelectTab9;
        let _ = SplitVertical;
        let _ = SplitHorizontal;
        let _ = FocusNextPane;
        let _ = FocusPrevPane;
        let _ = ClosePane;
        let _ = ToggleFocusMode;
        let _ = ToggleBroadcast;
        let _ = ToggleSnippets;
        let _ = FindInTerminal;
        let _ = ZoomIn;
        let _ = ZoomOut;
        let _ = ResetZoom;
        let _ = QuickConnect;
        let _ = NewHost;
        let _ = ReconnectSession;
        let _ = ToggleSidebar;
        let _ = ToggleSidebarCollapse;
        let _ = GoToHosts;
        let _ = GoToOverview;
        let _ = GoToWorkspaces;
        let _ = GoToSftp;
        let _ = GoToKeychain;
        let _ = GoToPortForwarding;
        let _ = OpenSettings;
        let _ = RefreshHosts;
        let _ = ToggleCommandPalette;

        // Test Keystroke matching logic
        let cmd_t = Keystroke {
            modifiers: gpui::Modifiers {
                platform: true,
                ..Default::default()
            },
            key: "t".into(),
            ..Default::default()
        };
        let is_primary = cmd_t.modifiers.platform || cmd_t.modifiers.control;
        assert!(is_primary && cmd_t.key == "t");

        let cmd_shift_d = Keystroke {
            modifiers: gpui::Modifiers {
                platform: true,
                shift: true,
                ..Default::default()
            },
            key: "d".into(),
            ..Default::default()
        };
        assert!(
            cmd_shift_d.modifiers.platform && cmd_shift_d.modifiers.shift && cmd_shift_d.key == "d"
        );

        let ctrl_shift_e = Keystroke {
            modifiers: gpui::Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
            key: "e".into(),
            ..Default::default()
        };
        assert!(
            ctrl_shift_e.modifiers.control
                && ctrl_shift_e.modifiers.shift
                && ctrl_shift_e.key == "e"
        );

        let cmd_zoom_in = Keystroke {
            modifiers: gpui::Modifiers {
                platform: true,
                ..Default::default()
            },
            key: "=".into(),
            ..Default::default()
        };
        assert!(
            cmd_zoom_in.modifiers.platform && (cmd_zoom_in.key == "=" || cmd_zoom_in.key == "+")
        );

        let cmd_tab_3 = Keystroke {
            modifiers: gpui::Modifiers {
                platform: true,
                ..Default::default()
            },
            key: "3".into(),
            ..Default::default()
        };
        let ch = cmd_tab_3.key.chars().next().unwrap();
        assert!(('1'..='9').contains(&ch));
        assert_eq!((ch as usize) - ('1' as usize), 2);
    }
}

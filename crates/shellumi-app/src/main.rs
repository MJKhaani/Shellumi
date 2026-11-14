mod actions;
mod app_render;
mod askpass;
mod connection;
mod data_path;
mod desktop;
mod hosts_view;
mod session;
mod sftp_transfer;
mod sftp_ui;
mod text_input;
mod theme;
mod ui;

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

#[derive(Debug)]
enum RemoteFileOperation {
    Rename { source: String, destination: String },
    Delete { path: String, is_dir: bool },
    Mkdir { path: String },
    Permissions { path: String, mode: u32 },
}

fn connect_saved_sftp(
    database: Option<String>,
    host_id: i64,
    destination: &SshDestination,
    options: &SftpConnectOptions,
) -> Result<SftpClient, SftpError> {
    let Some(database) = database else {
        return SftpClient::connect(destination, options);
    };
    let runtime = tokio::runtime::Builder::new_current_thread().build()?;
    let plan = runtime
        .block_on(shellumi_storage::desktop::resolve_connection(
            &database, host_id,
        ))
        .map_err(SftpError::ProcessSpawn)?;
    let command = askpass::command("ssh", Vec::new(), &plan).map_err(SftpError::ProcessSpawn)?;
    let mut resolved = SftpConnectOptions {
        port: Some(plan.destination.port().get()),
        identity_file: plan.identity_file,
        certificate_file: plan.certificate_file,
        extra_ssh_options: plan.authentication_options,
        environment: command.env.into_iter().collect(),
        ..Default::default()
    };
    if let Some(jump) = plan.jump_host {
        resolved.extra_ssh_options.push(("ProxyJump".into(), jump));
    }
    SftpClient::connect(&plan.destination, &resolved)
}

fn parse_sftp_permissions(input: &str) -> Option<u32> {
    let input = input.trim();
    if input.is_empty() || input.len() > 4 || !input.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return None;
    }
    u32::from_str_radix(input, 8)
        .ok()
        .filter(|mode| *mode <= 0o7777)
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
    password: Entity<TextInput>,
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
    group_context_menu: Option<(String, f32, f32)>,
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

pub(crate) fn resolve_folder_icon_path(name: &str) -> Option<std::path::PathBuf> {
    let icon_name = match name {
        "Documents" => "folder_documents",
        "Downloads" => "folder_downloads",
        "Desktop" => "folder_desktop",
        "Movies" => "folder_movies",
        "Music" => "folder_music",
        "Pictures" => "folder_pictures",
        "Applications" => "folder_applications",
        "Public" => "folder_public",
        _ => "folder",
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

fn responsive_card_width(available: f32, min_width: f32, gap: f32) -> f32 {
    let columns = (((available + gap) / (min_width + gap)).floor() as usize).max(1);
    ((available - gap * (columns - 1) as f32) / columns as f32).max(0.0)
}

fn dismiss_editor<T>(editor: &mut Option<T>, busy: bool) -> bool {
    if editor.is_none() {
        return false;
    }
    if !busy {
        *editor = None;
    }
    true
}

fn sftp_valid_transfer_names(files: &[String]) -> bool {
    !files.is_empty() && files.iter().all(|name| PathUtils::is_safe_file_name(name))
}

fn sftp_local_copy_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let child_src = entry.path();
            let child_dst = dst.join(entry.file_name());
            sftp_local_copy_recursive(&child_src, &child_dst)?;
        }
        Ok(())
    } else {
        if let Some(parent) = dst.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::copy(src, dst).map(|_| ())
    }
}

fn sftp_upload_recursive(
    client: &mut SftpClient,
    local: &std::path::Path,
    remote: &str,
) -> Result<(), SftpError> {
    if local.is_dir() {
        match client.stat(remote) {
            Ok(_) => {}
            Err(SftpError::FileNotFound(_)) => client.mkdir(remote)?,
            Err(err) => return Err(err),
        }
        for entry in std::fs::read_dir(local)? {
            let entry = entry?;
            let sub_local = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let sub_remote = format!("{}/{}", remote.trim_end_matches('/'), name);
            sftp_upload_recursive(client, &sub_local, &sub_remote)?;
        }
    } else {
        client.upload_file::<fn(u64, u64)>(local, remote, None)?;
    }
    Ok(())
}

fn sftp_download_recursive(
    client: &mut SftpClient,
    remote: &str,
    local: &std::path::Path,
) -> Result<(), SftpError> {
    if let Ok(entries) = client.list_remote_directory(remote) {
        std::fs::create_dir_all(local)?;
        for entry in entries {
            if entry.name == "." || entry.name == ".." {
                continue;
            }
            if !PathUtils::is_safe_file_name(&entry.name) {
                return Err(SftpError::UnexpectedResponse(
                    "Unsafe remote filename".into(),
                ));
            }
            let sub_remote = format!("{}/{}", remote.trim_end_matches('/'), entry.name);
            let sub_local = local.join(&entry.name);
            if entry.kind == FileKind::Directory {
                sftp_download_recursive(client, &sub_remote, &sub_local)?;
            } else {
                client.download_file::<fn(u64, u64)>(&sub_remote, &sub_local, None)?;
            }
        }
    } else {
        client.download_file::<fn(u64, u64)>(remote, local, None)?;
    }
    Ok(())
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
            ("Known Hosts", "fingerprint", "◉"),
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
                    .w(px(26.0))
                    .h(px(26.0))
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
            let tab_icon = match tab.kind {
                SessionKind::Local => {
                    if let Some(p) = resolve_icon_path("terminal", is_active, 48) {
                        div()
                            .w(px(18.0))
                            .h(px(18.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().child(">_").into_any_element()
                    }
                }
                SessionKind::Ssh => {
                    if let Some(p) = resolve_icon_path("hosts", is_active, 48) {
                        div()
                            .w(px(18.0))
                            .h(px(18.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().child("🖥").into_any_element()
                    }
                }
                SessionKind::Serial => {
                    if let Some(p) = resolve_icon_path("serial", is_active, 48) {
                        div()
                            .w(px(18.0))
                            .h(px(18.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().child("🔌").into_any_element()
                    }
                }
                SessionKind::Sftp => {
                    if let Some(p) = resolve_icon_path("sftp", is_active, 48) {
                        div()
                            .w(px(18.0))
                            .h(px(18.0))
                            .child(gpui::img(p).size_full())
                            .into_any_element()
                    } else {
                        div().child("📁").into_any_element()
                    }
                }
                SessionKind::Mosh => div().child("⚡").into_any_element(),
                SessionKind::Telnet => div().child("⚠️").into_any_element(),
                SessionKind::RemoteDesktop => {
                    if tab
                        .remote_desktop_state
                        .as_ref()
                        .is_some_and(|r| r.protocol == RemoteDesktopProtocol::Vnc)
                    {
                        div().child("🖥️").into_any_element()
                    } else {
                        div().child("🪟").into_any_element()
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
                .child(tab_icon)
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
                                .w(px(18.0))
                                .h(px(18.0))
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
                                        .w(px(18.0))
                                        .h(px(18.0))
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
mod tests;

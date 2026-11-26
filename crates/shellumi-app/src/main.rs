#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod actions;

pub(crate) struct CreatedInvite {
    pub(crate) scope: InviteScope,
    pub(crate) member_id: String,
    pub(crate) invitation_id: String,
    pub(crate) token: Option<shellumi_core::SecretString>,
    pub(crate) issuing_session: shellumi_core::SecretString,
    pub(crate) expires_at: i64,
    pub(crate) inspector_generation: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct InviteScope {
    binding: shellumi_storage::server_sync::VerifiedRemoteVaultBinding,
    account: uuid::Uuid,
}

mod app_render;
mod askpass;

mod connection;
mod data_path;
mod desktop;
mod embedded_icons;
mod hosts_view;
mod session;
mod sftp_transfer;
mod sftp_ui;
mod text_input;
mod theme;
mod ui;
mod vault_rotation;

pub use theme::{caption, card, glyph_badge, heading, item, pill, subheader, MacTheme};

use gpui::{
    actions, div, prelude::*, px, rgb, rgba, size, App, Application, Bounds, Context, Entity,
    FocusHandle, Focusable, FontWeight, KeyBinding, KeyDownEvent, Window,
};
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem, OsAction, SystemMenuType};
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
    SyncCycleResult, SystemHttpTransport, TeamMember, TeamRole, TelnetConfig, TransferDirection,
    TransferQueue, TransferStatus, VirtualAccessoryBar, VirtualKey, VirtualKeyModifier, VncConfig,
    WorkspaceLayout, WorkspaceSession, WorkspaceSessionKind,
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
pub(crate) const MONOSPACE_FONT: &str = "Menlo";
#[cfg(target_os = "windows")]
pub(crate) const MONOSPACE_FONT: &str = "Consolas";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) const MONOSPACE_FONT: &str = "DejaVu Sans Mono";

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
        OpenVaultInNewWindow,
        RunSetupWizard,
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
            .child(crate::ui::icon::Icon::new(crate::ui::icon::IconName::Snippets).size(16.0))
            .child(self.label.clone())
    }
}

enum HostState {
    NoDatabase,
    Loading,
    Loaded(Vec<SavedHost>),
    MigrationRequired(String),
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConflictModalState {
    vault_id: String,
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
pub(crate) enum CreateVaultModalTab {
    RemoteServer,
    Local,
}

pub(crate) struct CreateVaultModalState {
    pub(crate) active_tab: CreateVaultModalTab,
    pub(crate) server_url_input: Entity<TextInput>,
    pub(crate) username_input: Entity<TextInput>,
    pub(crate) password_input: Entity<TextInput>,
    pub(crate) device_name_input: Entity<TextInput>,
    pub(crate) vault_name_input: Entity<TextInput>,
    pub(crate) vault_passphrase_input: Entity<TextInput>,
    pub(crate) local_vault_desc_input: Entity<TextInput>,
    pub(crate) local_vault_password_input: Entity<TextInput>,
    pub(crate) local_vault_password_confirm_input: Entity<TextInput>,
    pub(crate) remember_in_keychain: bool,
    pub(crate) generated_recovery_code: Option<String>,
    pub(crate) recovery_code_confirmed: bool,
    pub(crate) error: Option<String>,
    pub(crate) status_message: Option<String>,
    pub(crate) is_testing: bool,
    pub(crate) is_submitting: bool,
}

pub(crate) struct UnlockVaultModalState {
    pub(crate) vault_id: String,
    pub(crate) vault_name: String,
    pub(crate) passphrase_input: Entity<TextInput>,
    pub(crate) remember_in_keychain: bool,
    pub(crate) error: Option<String>,
    pub(crate) is_submitting: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdditionalVaultsTab {
    CreateLocal,
    RestoreLocal,
    ConnectServer,
}

pub(crate) struct SetupWizardModalState {
    pub(crate) state: shellumi_core::setup::SetupState,
    pub(crate) installation_status: shellumi_core::setup::InstallationStatus,
    pub(crate) base_dir: std::path::PathBuf,
    // Step 2: Default Vault inputs
    pub(crate) default_password_input: Entity<TextInput>,
    pub(crate) default_password_confirm_input: Entity<TextInput>,
    pub(crate) default_recovery_code: Option<String>,
    pub(crate) recovery_code_confirmed: bool,
    pub(crate) remember_in_keychain: bool,
    pub(crate) legacy_password_input: Entity<TextInput>,
    pub(crate) existing_unlock_input: Entity<TextInput>,
    pub(crate) unlock_with_recovery_code: bool,
    pub(crate) restore_with_google_drive: bool,
    pub(crate) interrupted_recovery_code: bool,
    // Step 3: Google Drive inputs
    pub(crate) google_client_id_input: Entity<TextInput>,
    pub(crate) is_connecting_google: bool,
    pub(crate) google_auth_url: Option<String>,
    pub(crate) cloud_snapshots: Vec<shellumi_core::SyncVault>,
    pub(crate) cloud_vaults: Vec<shellumi_core::CloudVaultSummary>,
    pub(crate) cloud_vault_password_input: Entity<TextInput>,
    pub(crate) is_fetching_snapshots: bool,
    pub(crate) sync_default_opt_in: bool,
    // Step 4: Additional Vaults inputs
    pub(crate) additional_tab: AdditionalVaultsTab,
    pub(crate) sec_vault_name_input: Entity<TextInput>,
    pub(crate) sec_vault_password_input: Entity<TextInput>,
    pub(crate) sec_vault_password_confirm_input: Entity<TextInput>,
    pub(crate) sec_vault_keychain: bool,
    pub(crate) sec_vault_drive_opt_in: bool,
    pub(crate) sec_recovery_code: Option<String>,
    pub(crate) sec_recovery_confirmed: bool,
    // Step 4: Server inputs
    pub(crate) server_url_input: Entity<TextInput>,
    pub(crate) server_user_input: Entity<TextInput>,
    pub(crate) server_pass_input: Entity<TextInput>,
    pub(crate) server_device_name_input: Entity<TextInput>,
    pub(crate) is_testing_server: bool,
    // Step 5: Verification Report
    pub(crate) verification_report: Option<shellumi_core::setup::SetupVerificationReport>,
    // Common
    pub(crate) is_busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) status_message: Option<String>,
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

#[allow(dead_code)]
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

#[allow(dead_code)]
pub(crate) struct SnippetFormState {
    pub(crate) id: Option<i64>,
    pub(crate) name: Entity<TextInput>,
    pub(crate) description: Entity<TextInput>,
    pub(crate) command: Entity<TextInput>,
    pub(crate) tags: Entity<TextInput>,
    pub(crate) error: Option<String>,
    pub(crate) saving: bool,
}

#[allow(dead_code)]
pub(crate) struct TunnelFormState {
    pub(crate) id: Option<i64>,
    pub(crate) name: Entity<TextInput>,
    pub(crate) forwarding_type: ForwardingType,
    pub(crate) bind_port: Entity<TextInput>,
    pub(crate) target_host: Entity<TextInput>,
    pub(crate) target_port: Entity<TextInput>,
    pub(crate) host_id: Option<i64>,
    pub(crate) auto_start: bool,
    pub(crate) error: Option<String>,
    pub(crate) saving: bool,
}

#[allow(dead_code)]
pub(crate) struct WorkspaceFormState {
    pub(crate) id: Option<i64>,
    pub(crate) name: Entity<TextInput>,
    pub(crate) layout: String,
    pub(crate) description: Entity<TextInput>,
    pub(crate) error: Option<String>,
    pub(crate) saving: bool,
}

#[allow(dead_code)]
pub(crate) struct VaultFormState {
    pub(crate) vault_id: Option<String>,
    pub(crate) name: Entity<TextInput>,
    pub(crate) is_remote: bool,
    pub(crate) server_url: Entity<TextInput>,
    pub(crate) current_passphrase: Entity<TextInput>,
    pub(crate) passphrase: Entity<TextInput>,
    pub(crate) confirm_passphrase: Entity<TextInput>,
    pub(crate) generated_recovery_code: Option<String>,
    pub(crate) recovery_code_confirmed: bool,
    pub(crate) error: Option<String>,
    pub(crate) saving: bool,
}

#[allow(dead_code)]
pub(crate) struct ProviderFormState {
    pub(crate) id: Option<String>,
    pub(crate) name: Entity<TextInput>,
    pub(crate) provider_type: String,
    pub(crate) api_key: Entity<TextInput>,
    pub(crate) secret_key: Entity<TextInput>,
    pub(crate) region: Entity<TextInput>,
    pub(crate) error: Option<String>,
    pub(crate) saving: bool,
}

#[allow(dead_code)]
pub(crate) struct TeamFormState {
    pub(crate) member_id: Option<String>,
    pub(crate) email: Entity<TextInput>,
    pub(crate) role: String,
    pub(crate) public_key: Entity<TextInput>,
    pub(crate) error: Option<String>,
    pub(crate) sending: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DeleteTarget {
    Host(i64, String),
    Group(String),
    Identity(i64, String),
    Snippet(i64, String),
    Tunnel(i64, String),
    Workspace(i64, String),
    Vault(String, String),
    Provider(String, String),
    TeamMember(String, String),
}

struct Shellumi {
    sftp_jobs: std::collections::HashMap<(u64, u64), sftp_transfer::TransferJob>,
    pending_connections: std::collections::HashMap<i64, u64>,
    next_connection_op_id: u64,
    rd_credential_edit_gen: std::collections::HashMap<u64, u64>,
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
    pub(crate) workspaces: Vec<SavedWorkspace>,
    pub(crate) identities: Vec<SavedIdentity>,
    pub(crate) tunnels: Vec<SavedTunnel>,
    pub(crate) cloud_providers: Vec<shellumi_storage::CloudProviderRecord>,
    pub(crate) team_members: Vec<shellumi_core::TeamMember>,
    pub(crate) enrolled_devices: Vec<shellumi_core::EnrolledDevice>,
    pub(crate) has_pending_device_replacements: bool,
    pub(crate) incomplete_enrollments: Vec<shellumi_storage::EnrollmentOperationRecord>,
    pub(crate) team_invite_token_input: Option<Entity<TextInput>>,
    pub(crate) team_enrollment_busy: bool,
    pub(crate) team_enrollment_error: Option<String>,
    pub(crate) pending_device_revoke: Option<(String, String, String)>,
    pub(crate) pending_device_replace: Option<(String, String, String)>,
    pub(crate) invite_scope: Option<InviteScope>,
    pub(crate) pending_invitation_inspector: Option<u64>,
    pub(crate) invitation_generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub(crate) last_created_invite: Option<CreatedInvite>,
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
    create_vault_modal: Option<CreateVaultModalState>,
    unlock_vault_modal: Option<UnlockVaultModalState>,
    pub(crate) setup_wizard_modal: Option<SetupWizardModalState>,
    sidebar_visible: bool,
    sidebar_collapsed: bool,
    vault_dropdown_open: bool,
    active_vault: String,
    active_vault_id: String,
    pub(crate) active_vault_keys: std::collections::HashMap<String, shellumi_core::VaultKey>,
    pub(crate) is_server_sync_in_progress: bool,
    server_ws_status: Option<shellumi_core::server::WsConnectionStatus>,
    active_ws_client: Option<shellumi_core::server::ShellumiWsClient>,
    available_vaults: Vec<actions::VaultContextSummary>,
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
    pub(crate) delete_confirm: Option<DeleteTarget>,
    #[allow(dead_code)]
    pub(crate) inspector_coordinator: ui::inspector::InspectorCoordinator,
    pub(crate) current_inspector_width: Option<f32>,
    pub(crate) measured_workspace_width: Option<f32>,
    pub(crate) layout_epoch: u64,
    pub(crate) vault_load_generation: u64,
    pub(crate) tab_overflow_menu_open: bool,
    pub(crate) tab_overflow_x: f32,
    pub(crate) rd_display_menu_open: bool,
    pub(crate) rd_keyboard_menu_open: bool,
    pub(crate) rd_more_menu_open: bool,
    pub(crate) snippet_form: Option<SnippetFormState>,
    pub(crate) tunnel_form: Option<TunnelFormState>,
    pub(crate) workspace_form: Option<WorkspaceFormState>,
    pub(crate) vault_form: Option<VaultFormState>,
    pub(crate) provider_form: Option<ProviderFormState>,
    pub(crate) team_form: Option<TeamFormState>,
    pub(crate) pending_window_action: Option<PendingWindowAction>,
    pub(crate) approved_close_once: bool,
    pub(crate) rd_canvas_measurements: std::collections::HashMap<u64, CanvasMeasurement>,
    pub(crate) rd_pan_state: std::collections::HashMap<u64, shellumi_remote_desktop::PanState>,
    pub(crate) rd_resize_state: std::collections::HashMap<u64, RdAutoResizeState>,
    pub(crate) vault_trigger_bounds: Option<gpui::Bounds<gpui::Pixels>>,
    pub(crate) tab_overflow_trigger_bounds: Option<gpui::Bounds<gpui::Pixels>>,
    pub(crate) pending_rotation_recoveries: Vec<vault_rotation::PendingRotationRecovery>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConnectionOperationContext {
    pub window_id: gpui::WindowId,
    pub vault_id: String,
    pub vault_load_generation: u64,
    pub host_id: i64,
    pub operation_id: u64,
    pub tab_id: Option<u64>,
    pub connection_generation: Option<u64>,
    pub credential_edit_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct RemoteSessionKey {
    pub tab_id: u64,
    pub connection_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CanvasMeasurement {
    pub tab_id: u64,
    pub layout_epoch: u64,
    pub viewport: gpui::Bounds<gpui::Pixels>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RdAutoResizeState {
    pub layout_generation: u64,
    pub connection_generation: u64,
    pub desired: Option<(u32, u32)>,
    pub in_flight: Option<(u32, u32)>,
    pub in_flight_request_id: Option<u64>,
    pub next_request_id: u64,
    pub last_acknowledged: Option<(u32, u32)>,
    pub last_rejected: Option<(u32, u32)>,
    pub debounce_generation: u64,
    pub retry_count: u8,
    pub last_error: Option<String>,
}

impl RdAutoResizeState {
    pub fn reset_for_connection(&mut self, connection_generation: u64) {
        self.connection_generation = connection_generation;
        self.in_flight = None;
        self.in_flight_request_id = None;
        self.last_acknowledged = None;
        self.last_rejected = None;
        self.debounce_generation = self.debounce_generation.wrapping_add(1);
        self.retry_count = 0;
        self.last_error = None;
    }

    pub fn record_desired(&mut self, target: (u32, u32), layout_generation: u64) -> bool {
        if self.desired == Some(target)
            && (self.in_flight == Some(target) || self.last_acknowledged == Some(target))
        {
            return false;
        }
        self.layout_generation = layout_generation;
        self.desired = Some(target);
        self.debounce_generation = self.debounce_generation.wrapping_add(1);
        true
    }

    pub fn begin_dispatch(&mut self, target: (u32, u32)) -> u64 {
        self.next_request_id = self.next_request_id.wrapping_add(1);
        let req_id = self.next_request_id;
        self.in_flight = Some(target);
        self.in_flight_request_id = Some(req_id);
        req_id
    }

    pub fn fail_dispatch(&mut self, request_id: u64, err: String) {
        if self.in_flight_request_id == Some(request_id) {
            self.in_flight = None;
            self.in_flight_request_id = None;
            self.last_error = Some(err);
        }
    }

    pub fn timeout_request(&mut self, request_id: u64, target: (u32, u32)) -> Option<(u32, u32)> {
        if self.in_flight_request_id != Some(request_id) {
            return None;
        }
        self.in_flight = None;
        self.in_flight_request_id = None;
        if let Some(latest) = self.desired {
            if latest != target {
                self.retry_count = 0;
                return Some(latest);
            }
            if self.retry_count < 2 {
                self.retry_count += 1;
                return Some(target);
            }
            self.last_error = Some("Display resize acknowledgement timed out".into());
        }
        None
    }

    pub fn acknowledge(&mut self, size: (u32, u32)) -> Option<(u32, u32)> {
        self.last_acknowledged = Some(size);
        if self.in_flight == Some(size) {
            self.in_flight = None;
            self.in_flight_request_id = None;
            self.retry_count = 0;
            self.last_error = None;
        }
        match self.desired {
            Some(next) if next != size && self.in_flight.is_none() => Some(next),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn reject(&mut self, size: (u32, u32), reason: String) {
        self.last_rejected = Some(size);
        if self.in_flight == Some(size) {
            self.in_flight = None;
            self.in_flight_request_id = None;
        }
        self.last_error = Some(reason);
    }
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
    NotConnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32, max_attempts: u32 },
    Disconnected,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemoteDesktopScaleMode {
    Fit,
    Original,
    Fill,
}

#[derive(Clone, Debug)]
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
    fps: Option<u32>,
    latency_ms: Option<u32>,
    clipboard_text: Option<String>,
    error: Option<String>,
    last_action_note: Option<String>,
    framebuffer: Option<shellumi_remote_desktop::FramebufferSnapshot>,
    render_image: Option<std::sync::Arc<gpui::RenderImage>>,
    ui_handle: Option<shellumi_remote_desktop::SessionUiHandle>,
    metrics: shellumi_remote_desktop::SessionMetrics,
    username: Option<String>,
    password: Option<String>,
    domain: Option<String>,
    tls_strict: bool,
    show_diagnostics: bool,
    username_input: Option<Entity<TextInput>>,
    domain_input: Option<Entity<TextInput>>,
    password_input: Option<Entity<TextInput>>,
    enable_audio: bool,
    audio_muted: bool,
    audio_volume: u16,
    audio_active: bool,
    audio_format: Option<String>,
    audio_sample_rate: u32,
    audio_channels: u16,
    audio_bytes_received: u64,
    enable_microphone: bool,
    mic_muted: bool,
    mic_volume: u16,
    mic_active: bool,
    mic_format: Option<String>,
    mic_sample_rate: u32,
    mic_channels: u16,
    mic_bytes_sent: u64,
    mic_packets_sent: u64,
    display_control_ready: bool,
    max_monitor_area: Option<u64>,
    connection_generation: u64,
    dynamic_resize_enabled: bool,
    shared_folder: Option<std::path::PathBuf>,
    drive_name: Option<String>,
    drive_mounted: bool,
    drive_bytes_read: u64,
    drive_bytes_written: u64,
    drive_operations_count: u64,
}

impl Drop for RemoteDesktopState {
    fn drop(&mut self) {
        if let Some(pwd) = &mut self.password {
            use zeroize::Zeroize;
            pwd.zeroize();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitDirection {
    Grid,
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum PendingWindowAction {
    CloseTab(u64),
    CloseWindow,
    QuitApp,
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

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct TabCollabState {
    pub(crate) session_id: uuid::Uuid,
    pub(crate) key: zeroize::Zeroizing<[u8; 32]>,
    pub(crate) outbound_seq: u64,
    pub(crate) last_polled_seq: u64,
    pub(crate) server_url: String,
    pub(crate) token: String,
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
    collab_state: Option<TabCollabState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostEditorCategory {
    General,
    Connection,
    Authentication,
    Proxy,
    Advanced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostAuthSourceKind {
    AskOnConnect,
    ExistingIdentity,
    EnterCredentials,
    InheritGroup,
    NoAuthentication,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostPersistenceChoice {
    SessionOnly,
    SaveForThisHost,
    SaveAsIdentity,
}

/// Non-secret authentication metadata stored in `protocol_config` (never contains plaintext secrets).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct HostAuthMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_secret_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_passphrase_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_mode: Option<String>,
}

/// Typed persisted authentication state restored when opening an existing host in the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PersistedHostAuth {
    pub auth_source_kind: HostAuthSourceKind,
    pub credential_kind: shellumi_core::CredentialKind,
    pub persistence_choice: HostPersistenceChoice,
    pub host_secret_ref: Option<String>,
    pub host_key_path: Option<String>,
    pub host_passphrase_ref: Option<String>,
    pub has_host_secret: bool,
    pub has_host_passphrase: bool,
}

impl PersistedHostAuth {
    pub fn from_host(
        protocol: ConnectionProtocol,
        identity_id: Option<i64>,
        protocol_config: Option<&str>,
    ) -> Self {
        let caps = shellumi_core::ProtocolCapabilities::for_protocol(protocol);
        let auth_meta = protocol_config
            .and_then(|c| serde_json::from_str::<HostAuthMetadata>(c).ok())
            .unwrap_or_default();
        let host_secret_ref = auth_meta.host_secret_ref;
        let host_key_path = auth_meta.host_key_path;
        let host_passphrase_ref = auth_meta.host_passphrase_ref;
        let has_host_passphrase = host_passphrase_ref.is_some();
        let has_host_credentials =
            host_secret_ref.is_some() || host_key_path.is_some() || has_host_passphrase;

        let auth_source_kind = if !caps.allows_saved_identity && !caps.allows_ask_on_connect {
            if caps.allows_no_auth {
                HostAuthSourceKind::NoAuthentication
            } else {
                HostAuthSourceKind::AskOnConnect
            }
        } else if identity_id.is_some() {
            HostAuthSourceKind::ExistingIdentity
        } else if has_host_credentials {
            HostAuthSourceKind::EnterCredentials
        } else if auth_meta.auth_mode.as_deref() == Some("inherit_group") {
            HostAuthSourceKind::InheritGroup
        } else if auth_meta.auth_mode.as_deref() == Some("no_auth") && caps.allows_no_auth {
            HostAuthSourceKind::NoAuthentication
        } else {
            HostAuthSourceKind::AskOnConnect
        };

        let credential_kind = if (host_key_path.is_some() || has_host_passphrase)
            && caps
                .supported_credentials
                .contains(&shellumi_core::CredentialKind::SshKey)
        {
            shellumi_core::CredentialKind::SshKey
        } else {
            caps.supported_credentials
                .first()
                .copied()
                .unwrap_or(shellumi_core::CredentialKind::None)
        };

        let persistence_choice = if has_host_credentials {
            HostPersistenceChoice::SaveForThisHost
        } else {
            HostPersistenceChoice::SessionOnly
        };

        Self {
            auth_source_kind,
            credential_kind,
            persistence_choice,
            host_secret_ref,
            host_key_path,
            host_passphrase_ref,
            has_host_secret: has_host_credentials,
            has_host_passphrase,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum HostEditEvent {
    SetProtocol(ConnectionProtocol),
    SelectIdentity(Option<i64>),
    SetAuthSource(HostAuthSourceKind),
    SetCredentialKind(shellumi_core::CredentialKind),
    SetPersistence(HostPersistenceChoice),
    SetSecretUpdateAction(shellumi_core::SecretUpdateAction),
    SetPassphraseUpdateAction(shellumi_core::SecretUpdateAction),
    SetRdColorDepth(u8),
    ToggleRdViewOnly,
    ToggleRdTlsStrict,
    ToggleRdAudio,
    ToggleRdMicrophone,
    SetSerialDataBits(u8),
    SetSerialParity(shellumi_core::SerialParity),
    SetSerialStopBits(shellumi_core::SerialStopBits),
    SetSerialFlowControl(shellumi_core::SerialFlowControl),
}

struct HostForm {
    draft_generation: u64,
    active_category: HostEditorCategory,
    identity_id: Option<i64>,
    id: Option<i64>,
    name: Entity<TextInput>,
    hostname: Entity<TextInput>,
    username: Entity<TextInput>,
    port: Entity<TextInput>,
    port_is_manual: bool,
    group: Entity<TextInput>,
    tags: Entity<TextInput>,
    jump_host: Entity<TextInput>,
    startup_snippet: Entity<TextInput>,
    protocol: ConnectionProtocol,
    protocol_config: Option<String>,
    auth_source_kind: HostAuthSourceKind,
    credential_kind: shellumi_core::CredentialKind,
    persistence_choice: HostPersistenceChoice,
    auth_password: Entity<TextInput>,
    auth_key_path: Entity<TextInput>,
    auth_passphrase: Entity<TextInput>,
    new_identity_name: Entity<TextInput>,
    new_identity_tags: Entity<TextInput>,
    secret_update_action: shellumi_core::SecretUpdateAction,
    passphrase_update_action: shellumi_core::SecretUpdateAction,
    host_secret_ref: Option<String>,
    host_passphrase_ref: Option<String>,
    has_host_passphrase: bool,
    rd_domain: Entity<TextInput>,
    rd_width: Entity<TextInput>,
    rd_height: Entity<TextInput>,
    rd_color_depth: u8,
    rd_view_only: bool,
    rd_tls_strict: bool,
    rd_enable_audio: bool,
    rd_enable_microphone: bool,
    serial_baud: Entity<TextInput>,
    serial_data_bits: u8,
    serial_parity: shellumi_core::SerialParity,
    serial_stop_bits: shellumi_core::SerialStopBits,
    serial_flow_control: shellumi_core::SerialFlowControl,
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
        Self::new_with_vault(
            database,
            shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string(),
            "Personal".to_string(),
            window,
            cx,
        )
    }

    pub(crate) fn new_with_vault(
        database: Option<String>,
        vault_id: String,
        vault_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.on_release(|app, _| app.invalidate_invitation_scope())
            .detach();
        cx.on_app_quit(|app, _| {
            app.invalidate_invitation_scope();
            app.inspector_coordinator.invalidate_scope();
            std::future::ready(())
        })
        .detach();
        let entity = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let Some(entity) = entity.upgrade() else {
                return true;
            };
            entity.update(cx, |app, cx| app.handle_window_close_request(window, cx))
        });
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
            pending_connections: std::collections::HashMap::new(),
            next_connection_op_id: 1,
            rd_credential_edit_gen: std::collections::HashMap::new(),
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
            tunnels: Vec::new(),
            cloud_providers: Vec::new(),
            team_members: Vec::new(),
            enrolled_devices: Vec::new(),
            incomplete_enrollments: Vec::new(),
            has_pending_device_replacements: false,
            team_invite_token_input: None,
            team_enrollment_busy: false,
            team_enrollment_error: None,
            pending_device_revoke: None,
            pending_device_replace: None,
            invite_scope: None,
            pending_invitation_inspector: None,
            invitation_generation: Default::default(),
            last_created_invite: None,
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
            create_vault_modal: None,
            unlock_vault_modal: None,
            setup_wizard_modal: None,
            sidebar_visible: true,
            sidebar_collapsed: false,
            vault_dropdown_open: false,
            active_vault: vault_name.clone(),
            active_vault_id: vault_id.clone(),
            active_vault_keys: std::collections::HashMap::new(),
            is_server_sync_in_progress: false,
            server_ws_status: None,
            active_ws_client: None,
            available_vaults: vec![actions::VaultContextSummary {
                id: vault_id,
                name: vault_name,
                is_remote: false,
                server_url: None,
                server_vault_id: None,
                is_locked: false,
            }],
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
            delete_confirm: None,
            inspector_coordinator: ui::inspector::InspectorCoordinator::new(),
            current_inspector_width: Some(crate::ui::layout::INSPECTOR_PREFERRED_WIDTH),
            measured_workspace_width: None,
            layout_epoch: 0,
            vault_load_generation: 0,
            tab_overflow_menu_open: false,
            tab_overflow_x: 220.0,
            snippet_form: None,
            tunnel_form: None,
            workspace_form: None,
            vault_form: None,
            provider_form: None,
            team_form: None,
            pending_window_action: None,
            approved_close_once: false,
            rd_canvas_measurements: std::collections::HashMap::new(),
            rd_pan_state: std::collections::HashMap::new(),
            rd_resize_state: std::collections::HashMap::new(),
            vault_trigger_bounds: None,
            tab_overflow_trigger_bounds: None,
            pending_rotation_recoveries: Vec::new(),
            rd_display_menu_open: false,
            rd_keyboard_menu_open: false,
            rd_more_menu_open: false,
        }
    }

    /// Opens an independent application window bound to the specified vault context.
    pub(crate) fn open_vault_in_new_window(
        &mut self,
        vault_id: String,
        vault_name: String,
        cx: &mut Context<Self>,
    ) {
        let database = self.database.clone();
        let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
        let server_url = self
            .available_vaults
            .iter()
            .find(|v| v.id == vault_id)
            .and_then(|v| v.server_url.as_deref());
        let title = if let Some(url) = server_url {
            format!("Shellumi — {vault_name} ({url})")
        } else {
            format!("Shellumi — {vault_name}")
        };
        let _ = cx.open_window(
            ui::window_chrome::shellumi_window_options(title, bounds),
            |window, cx| {
                let view = cx
                    .new(|cx| Shellumi::new_with_vault(database, vault_id, vault_name, window, cx));
                view.update(cx, |app, cx| {
                    app.start_storage(false, cx);
                });
                view
            },
        );
        cx.activate(true);
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

#[allow(dead_code)]
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
    crate::ui::icon::resolve_icon_path(name, is_active, size_px)
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
    } else if combined.contains("debian") {
        "os_debian"
    } else if combined.contains("arch") {
        "os_arch"
    } else if combined.contains("fedora") {
        "os_fedora"
    } else if combined.contains("centos") {
        "os_centos"
    } else if combined.contains("rhel") || combined.contains("redhat") {
        "os_rhel"
    } else if combined.contains("rocky") {
        "os_rocky"
    } else if combined.contains("almalinux") || combined.contains("alma") {
        "os_almalinux"
    } else if combined.contains("alpine") {
        "os_alpine"
    } else if combined.contains("opensuse") || combined.contains("suse") {
        "os_opensuse"
    } else if combined.contains("kali") {
        "os_kali"
    } else if combined.contains("windows server") || combined.contains("winserver") {
        "os_windows_server"
    } else if combined.contains("windows") || combined.contains("win") {
        "os_windows"
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
    } else if combined.contains("android") {
        "os_android"
    } else if combined.contains("ios") {
        "os_ios"
    } else if combined.contains("linux") {
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
            ("Known Hosts", "known_hosts", "◉"),
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
                    .overflow_hidden()
                    .child(
                        crate::ui::icon::Icon::new(crate::ui::icon::IconName::ShellumiLogo)
                            .size(32.0),
                    )
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
                            app.navigate_page(page, Some(window), cx);
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
                            app.navigate_page("Settings", Some(window), cx)
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
                                crate::ui::icon::Icon::new(crate::ui::icon::IconName::Settings)
                                    .size(22.0)
                                    .active(self.page == Some("Settings"))
                                    .into_any_element()
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
                .overflow_hidden()
                .child(
                    crate::ui::icon::Icon::new(crate::ui::icon::IconName::ShellumiLogo).size(28.0),
                )
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
                        app.navigate_page(page, Some(window), cx);
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
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.navigate_page("Settings", Some(window), cx)
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
                            crate::ui::icon::Icon::new(crate::ui::icon::IconName::Settings)
                                .size(18.0)
                                .active(self.page == Some("Settings"))
                                .into_any_element()
                        },
                    )
                    .child("Settings"),
            )
    }
    /// Evaluate whether the current window may close, routing dirty drafts through the
    /// inspector navigation guard (`NavigationRequest::CloseWindow`).
    pub(crate) fn handle_window_close_request(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.approved_close_once {
            self.approved_close_once = false;
            return true;
        }
        if !self
            .inspector_coordinator
            .request_navigation(ui::inspector::NavigationRequest::CloseWindow)
        {
            tracing::debug!(
                vault_id = %self.active_vault_id,
                "Window close vetoed by dirty inspector guard"
            );
            cx.notify();
            return false;
        }
        true
    }

    fn render_unified_top_bar(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let is_hosts_view = self.page.is_none() && self.active_tab_idx.is_none();
        let chrome = ui::window_chrome::ChromeMetrics::from_window(window);

        let traffic_spacer = div()
            .w(px(chrome.left_reserved_width))
            .h_full()
            .flex_shrink_0();

        let vault_bounds_sensor = {
            let view = cx.entity().downgrade();
            gpui::canvas(
                move |bounds, _window, cx| {
                    cx.defer(move |cx| {
                        let _ = view.update(cx, |app, _cx| {
                            app.vault_trigger_bounds = Some(bounds);
                        });
                    });
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0()
        };

        // 1. Split Vault Button: Clicking name returns to main view without menu; clicking arrow toggles menu
        let vault_btn = div()
            .id("topbar-vault-btn-container")
            .relative()
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
            .child(vault_bounds_sensor)
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
                        app.navigate_page("Hosts", Some(window), cx);
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
                            crate::ui::icon::Icon::new(crate::ui::icon::IconName::Vaults)
                                .size(22.0)
                                .active(self.vault_dropdown_open)
                                .into_any_element()
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
                app.navigate_page("SFTP Files", Some(window), cx);
            }))
            .child(
                if let Some(icon_path) = resolve_icon_path("sftp", is_sftp_active, 48) {
                    div()
                        .w(px(22.0))
                        .h(px(22.0))
                        .child(gpui::img(icon_path).size_full())
                        .into_any_element()
                } else {
                    crate::ui::icon::Icon::new(crate::ui::icon::IconName::Sftp)
                        .size(20.0)
                        .into_any_element()
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
                            app.navigate_page("Hosts", Some(window), cx);
                            cx.stop_propagation();
                        }))
                        .child("✕"),
                )
            } else {
                None
            });

        // Remote Vault Sync Status & "Sync Now" controls
        let sync_badge = if self.is_active_vault_remote() {
            let (status_text, status_color, is_syncing) = match &self.sync_status {
                AppSyncStatus::Syncing => ("Syncing…", rgb(MacTheme::ACCENT_BLUE), true),
                AppSyncStatus::UpToDate { last_sync_time } => {
                    if *last_sync_time > 0 {
                        ("Synced", rgb(MacTheme::TEXT_SECONDARY), false)
                    } else {
                        ("Connected", rgb(MacTheme::TEXT_SECONDARY), false)
                    }
                }
                AppSyncStatus::Error(_) => ("Sync Error", rgb(MacTheme::DANGER), false),
                _ => ("Remote", rgb(MacTheme::TEXT_TERTIARY), false),
            };

            let server_host = self
                .available_vaults
                .iter()
                .find(|v| v.id == self.active_vault_id)
                .and_then(|v| v.server_url.as_deref())
                .map(|url| {
                    if let Some(stripped) = url.strip_prefix("https://") {
                        stripped.trim_end_matches('/')
                    } else if let Some(stripped) = url.strip_prefix("http://") {
                        stripped.trim_end_matches('/')
                    } else {
                        url
                    }
                });

            let mut sync_row = div()
                .id("topbar-sync-controls")
                .flex()
                .items_center()
                .gap_1();

            if let Some(host_str) = server_host {
                sync_row = sync_row.child(
                    div()
                        .id("topbar-server-host-badge")
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child(
                            crate::ui::icon::Icon::new(crate::ui::icon::IconName::KnownHosts)
                                .size(13.0),
                        )
                        .child(host_str.to_string()),
                );
            }

            sync_row = sync_row
                .child(
                    div()
                        .id("topbar-sync-status-badge")
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(rgba(0x2563eb18))
                        .text_xs()
                        .text_color(status_color)
                        .child(
                            crate::ui::icon::Icon::new(if is_syncing {
                                crate::ui::icon::IconName::ServerSync
                            } else {
                                crate::ui::icon::IconName::CloudSync
                            })
                            .size(13.0),
                        )
                        .child(status_text),
                )
                .child(
                    div()
                        .id("topbar-sync-now-btn")
                        .px_1p5()
                        .py_1()
                        .rounded_md()
                        .text_xs()
                        .cursor_pointer()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .hover(|s| {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        })
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.sync_active_vault(cx);
                        }))
                        .child("↻"),
                );

            Some(sync_row)
        } else {
            None
        };

        // 3. Tab bar containing Vault, SFTP and open terminal sessions
        let mut tabs_row = div().flex().items_center().gap_1p5().child(vault_btn);

        if let Some(sync_ctrls) = sync_badge {
            tabs_row = tabs_row.child(sync_ctrls);
        }

        tabs_row = tabs_row.child(sftp_tab);

        let window_w = window.bounds().size.width.to_f64() as f32;
        let total_tabs = self.tabs.len();
        let topbar_geom = ui::layout::calculate_topbar_geometry_with_chrome(
            window_w,
            total_tabs,
            is_hosts_view,
            chrome.total_reserved_width(),
        );
        let max_visible_tabs = topbar_geom.max_visible_tabs.clamp(1, 10);
        let active_idx = self.active_tab_idx;

        // When many tabs exist, compute visible slice ensuring active tab is always visible
        let (visible_indices, overflow_count) = if total_tabs > max_visible_tabs {
            let active = active_idx.unwrap_or(0);
            let start = if active >= max_visible_tabs {
                active + 1 - max_visible_tabs
            } else {
                0
            };
            let end = (start + max_visible_tabs).min(total_tabs);
            (
                (start..end).collect::<Vec<_>>(),
                total_tabs.saturating_sub(max_visible_tabs),
            )
        } else {
            ((0..total_tabs).collect::<Vec<_>>(), 0)
        };

        for &idx in &visible_indices {
            let Some(tab) = self.tabs.get(idx) else {
                continue;
            };
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
                        crate::ui::icon::Icon::new(crate::ui::icon::IconName::Ssh)
                            .size(18.0)
                            .active(is_active)
                            .into_any_element()
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
                        crate::ui::icon::Icon::new(crate::ui::icon::IconName::Serial)
                            .size(18.0)
                            .active(is_active)
                            .into_any_element()
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
                        crate::ui::icon::Icon::new(crate::ui::icon::IconName::Sftp)
                            .size(18.0)
                            .active(is_active)
                            .into_any_element()
                    }
                }
                SessionKind::Mosh => crate::ui::icon::Icon::new(crate::ui::icon::IconName::Mosh)
                    .size(18.0)
                    .active(is_active)
                    .into_any_element(),
                SessionKind::Telnet => {
                    crate::ui::icon::Icon::new(crate::ui::icon::IconName::Terminal)
                        .size(18.0)
                        .active(is_active)
                        .into_any_element()
                }
                SessionKind::RemoteDesktop => {
                    if tab
                        .remote_desktop_state
                        .as_ref()
                        .is_some_and(|r| r.protocol == RemoteDesktopProtocol::Vnc)
                    {
                        crate::ui::icon::Icon::new(crate::ui::icon::IconName::Vnc)
                            .size(18.0)
                            .active(is_active)
                            .into_any_element()
                    } else {
                        crate::ui::icon::Icon::new(crate::ui::icon::IconName::Rdp)
                            .size(18.0)
                            .active(is_active)
                            .into_any_element()
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

        if overflow_count > 0 {
            let overflow_btn_x = 220.0 + (visible_indices.len() as f32 * 140.0);
            let overflow_sensor = {
                let view = cx.entity().downgrade();
                gpui::canvas(
                    move |bounds, _window, cx| {
                        cx.defer(move |cx| {
                            let _ = view.update(cx, |app, _cx| {
                                app.tab_overflow_trigger_bounds = Some(bounds);
                            });
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0()
            };
            tabs_row = tabs_row.child(
                div()
                    .id("topbar-tabs-overflow-btn")
                    .relative()
                    .px_2()
                    .py_1()
                    .h(px(26.0))
                    .rounded_md()
                    .bg(if self.tab_overflow_menu_open {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        rgb(MacTheme::SURFACE)
                    })
                    .border_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if self.tab_overflow_menu_open {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .hover(|s| {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.tab_overflow_x = overflow_btn_x;
                        app.tab_overflow_menu_open = !app.tab_overflow_menu_open;
                        cx.notify();
                    }))
                    .child(overflow_sensor)
                    .child(format!("+{} more", overflow_count)),
            );
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

        // 4. Quick Connect Box: ONLY visible on Hosts view! Responsive width prevents squeezing tabs
        let quick_connect_box = if is_hosts_view {
            Some(
                div()
                    .id("topbar-quick-connect-container")
                    .flex()
                    .items_center()
                    .gap_2()
                    .w(px(340.0))
                    .min_w(px(180.0))
                    .max_w(px(460.0))
                    .flex_1()
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

        let right_actions = div().flex().items_center().gap_2().child(
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
                .on_click(cx.listener(|app, _, window, cx| app.open_command_palette(window, cx)))
                .child(if cfg!(target_os = "macos") {
                    "⌘K"
                } else {
                    "Ctrl+K"
                }),
        );

        let client_controls = ui::window_chrome::render_client_window_controls(window, cx);

        let bar = div()
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
                    .flex_1()
                    .min_w_0()
                    .overflow_x_hidden()
                    .child(traffic_spacer)
                    .child(tabs_row),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .flex_shrink_0()
                    .children(quick_connect_box)
                    .child(right_actions)
                    .children(client_controls),
            );
        ui::window_chrome::configure_topbar_drag(bar, chrome.mode)
    }

    fn render_vault_dropdown_overlay(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        if !self.vault_dropdown_open {
            return None;
        }
        let trigger_bounds = self.vault_trigger_bounds?;
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
                .child({
                    let local_vaults: Vec<_> = self
                        .available_vaults
                        .iter()
                        .filter(|v| !v.is_remote)
                        .cloned()
                        .collect();
                    let remote_vaults: Vec<_> = self
                        .available_vaults
                        .iter()
                        .filter(|v| v.is_remote)
                        .cloned()
                        .collect();

                    let win_size = window.bounds().size;
                    let placement = crate::ui::components::popover::place_popover_from_trigger(
                        trigger_bounds,
                        240.0,
                        320.0,
                        win_size.width.to_f64() as f32,
                        win_size.height.to_f64() as f32,
                        crate::ui::components::PopoverAnchor::BottomLeft,
                    );
                    let mut menu = div()
                        .id("vault-dropdown-menu")
                        .absolute()
                        .top(px(placement.top))
                        .left(px(placement.left))
                        .w(px(240.0))
                        .max_h(px(placement.max_height))
                        .overflow_y_scroll()
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
                        );

                    for v in local_vaults {
                        let is_active = v.id == self.active_vault_id;
                        let is_locked = v.is_locked;
                        let v_id = v.id.clone();
                        let v_name = v.name.clone();
                        let v_id_win = v.id.clone();
                        let v_name_win = v.name.clone();
                        menu = menu.child(
                            div()
                                .id(gpui::SharedString::from(format!("vault-opt-{}", v.id)))
                                .flex()
                                .items_center()
                                .justify_between()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .bg(if is_active {
                                    rgb(MacTheme::SURFACE_ACTIVE)
                                } else {
                                    rgba(0x00000000)
                                })
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .on_click(cx.listener(move |app, _, window, cx| {
                                    app.switch_vault(
                                        v_id.clone(),
                                        v_name.clone(),
                                        Some(window),
                                        cx,
                                    );
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .child(if is_locked { "🔒" } else { "💻" })
                                        .child(v.name)
                                        .children(if is_locked {
                                            Some(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                                    .child("(Locked)"),
                                            )
                                        } else {
                                            None
                                        }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            div()
                                                .id(gpui::SharedString::from(format!(
                                                    "vault-win-{}",
                                                    v.id
                                                )))
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded_md()
                                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                                .hover(|s| {
                                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                })
                                                .on_click(cx.listener(move |app, _, _, cx| {
                                                    app.vault_dropdown_open = false;
                                                    app.open_vault_in_new_window(
                                                        v_id_win.clone(),
                                                        v_name_win.clone(),
                                                        cx,
                                                    );
                                                    cx.stop_propagation();
                                                }))
                                                .child("🗗"),
                                        )
                                        .child(if is_active { "✓" } else { "" }),
                                ),
                        );
                    }

                    if !remote_vaults.is_empty() {
                        menu = menu
                            .child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)).my_1())
                            .child(
                                div()
                                    .px_2p5()
                                    .py_1()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .font_weight(FontWeight::BOLD)
                                    .child("REMOTE SERVER VAULTS"),
                            );

                        for v in remote_vaults {
                            let is_active = v.id == self.active_vault_id;
                            let is_locked = v.is_locked;
                            let v_id = v.id.clone();
                            let v_name = v.name.clone();
                            let v_id_win = v.id.clone();
                            let v_name_win = v.name.clone();
                            let url_hint = v.server_url.clone().unwrap_or_default();
                            menu = menu.child(
                                div()
                                    .id(gpui::SharedString::from(format!("vault-opt-{}", v.id)))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .px_2p5()
                                    .py_1p5()
                                    .rounded_md()
                                    .text_xs()
                                    .cursor_pointer()
                                    .bg(if is_active {
                                        rgb(MacTheme::SURFACE_ACTIVE)
                                    } else {
                                        rgba(0x00000000)
                                    })
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                    .on_click(cx.listener(move |app, _, window, cx| {
                                        app.switch_vault(
                                            v_id.clone(),
                                            v_name.clone(),
                                            Some(window),
                                            cx,
                                        );
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child(if is_locked {
                                                        "🔒"
                                                    } else {
                                                        "☁️"
                                                    })
                                                    .child(v.name)
                                                    .children(if is_locked {
                                                        Some(
                                                            div()
                                                                .text_xs()
                                                                .text_color(rgb(
                                                                    MacTheme::TEXT_TERTIARY,
                                                                ))
                                                                .child("(Locked)"),
                                                        )
                                                    } else {
                                                        None
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                                    .child(url_hint),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .id(gpui::SharedString::from(format!(
                                                        "vault-win-{}",
                                                        v.id
                                                    )))
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                                    .hover(|s| {
                                                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                    })
                                                    .on_click(cx.listener(move |app, _, _, cx| {
                                                        app.vault_dropdown_open = false;
                                                        app.open_vault_in_new_window(
                                                            v_id_win.clone(),
                                                            v_name_win.clone(),
                                                            cx,
                                                        );
                                                        cx.stop_propagation();
                                                    }))
                                                    .child("🗗"),
                                            )
                                            .child(if is_active { "✓" } else { "" }),
                                    ),
                            );
                        }
                    }

                    menu.child(div().h(px(1.0)).bg(rgb(MacTheme::SEPARATOR)).my_1())
                        .child(
                            div()
                                .id("vault-opt-new-window")
                                .px_2p5()
                                .py_2()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.vault_dropdown_open = false;
                                    app.open_vault_in_new_window(
                                        app.active_vault_id.clone(),
                                        app.active_vault.clone(),
                                        cx,
                                    );
                                }))
                                .child("🗗 Open in New Window (Cmd+Shift+N)"),
                        )
                        .child(
                            div()
                                .id("vault-opt-lock")
                                .px_2p5()
                                .py_2()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.vault_dropdown_open = false;
                                    app.lock_active_vault(cx);
                                }))
                                .child("🔒 Lock Active Vault"),
                        )
                        .child(
                            div()
                                .id("vault-opt-create")
                                .px_2p5()
                                .py_2()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.open_create_vault_modal(cx);
                                }))
                                .child("+ Connect Remote Server Vault…"),
                        )
                        .child(
                            div()
                                .id("vault-opt-manage")
                                .px_2p5()
                                .py_2()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.vault_dropdown_open = false;
                                    app.open_team_inspector_mode(
                                        None,
                                        crate::ui::inspector::InspectorMode::View,
                                        cx,
                                    );
                                }))
                                .child("👥 Manage Team Vaults…"),
                        )
                }),
        )
    }

    pub(crate) fn render_tab_overflow_popover(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        if !self.tab_overflow_menu_open {
            return None;
        }

        let trigger_bounds = self.tab_overflow_trigger_bounds?;
        let win_size = window.bounds().size;
        let placement = crate::ui::components::popover::place_popover_from_trigger(
            trigger_bounds,
            280.0,
            320.0,
            win_size.width.to_f64() as f32,
            win_size.height.to_f64() as f32,
            crate::ui::components::PopoverAnchor::BottomRight,
        );

        let mut list = div()
            .id("tab-overflow-popover")
            .absolute()
            .top(px(placement.top))
            .left(px(placement.left))
            .w(px(280.0))
            .max_h(px(placement.max_height.min(320.0)))
            .overflow_y_scroll()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .rounded_md()
            .shadow_lg()
            .p_1p5()
            .flex()
            .flex_col()
            .gap_1()
            .on_click(cx.listener(|_, _, _, cx| {
                cx.stop_propagation();
            }));

        list = list.child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child("Active Sessions"),
        );

        for (idx, tab) in self.tabs.iter().enumerate() {
            let is_active = self.active_tab_idx == Some(idx) && self.page.is_none();
            let tab_id = tab.id;
            let tab_name = tab.name.clone();

            let item = div()
                .id(("overflow-tab-item", idx))
                .px_2()
                .py_1p5()
                .rounded_sm()
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .bg(if is_active {
                    rgb(MacTheme::SURFACE_ACTIVE)
                } else {
                    rgb(MacTheme::SURFACE)
                })
                .text_color(if is_active {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::TEXT_PRIMARY)
                })
                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.page = None;
                    app.select_tab(Some(idx), window, cx);
                    app.tab_overflow_menu_open = false;
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .flex_1()
                        .child(div().text_xs().child(if is_active { "●" } else { "○" }))
                        .child(div().text_xs().truncate().child(tab_name)),
                )
                .child(
                    div()
                        .id(("overflow-tab-close", tab_id as usize))
                        .px_1p5()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .hover(|s| s.text_color(rgb(MacTheme::DANGER)))
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.close_tab(tab_id, window, cx);
                            cx.stop_propagation();
                        }))
                        .child("✕"),
                );

            list = list.child(item);
        }

        Some(
            div()
                .id("tab-overflow-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(rgba(0x00000033))
                .on_click(cx.listener(|app, _, _, cx| {
                    app.tab_overflow_menu_open = false;
                    cx.notify();
                }))
                .child(list),
        )
    }
}

fn main() {
    let askpass_mode = std::env::var("SHELLUMI_ASKPASS_MODE").as_deref() == Ok("1");
    let mut logging = shellumi_core::logging::LoggingConfig::for_app();
    if askpass_mode {
        // OpenSSH treats the helper's streams as a credential protocol. Keep
        // structured diagnostics in the native log even with verbose filters.
        logging.enable_stderr = false;
        logging.category = "askpass".into();
    }
    if let Err(e) = shellumi_core::logging::init_logging(&logging) {
        if askpass_mode {
            std::process::exit(1);
        }
        eprintln!("Warning: Failed to initialize application logging: {e}");
    }

    if askpass_mode {
        tracing::debug!("Running OpenSSH credential helper");
        std::process::exit(askpass::run());
    }

    shellumi_rdp::ensure_crypto_provider_installed();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        "Shellumi desktop application started"
    );

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut force_wizard = false;
    let mut custom_hosts = None;
    let mut arg_error = false;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--setup-wizard" | "--setup" => {
                force_wizard = true;
            }
            "--hosts" => {
                if let Some(path) = iter.next() {
                    if !path.is_empty() && path != ":memory:" {
                        custom_hosts = Some(path);
                    } else {
                        arg_error = true;
                    }
                } else {
                    arg_error = true;
                }
            }
            _ => {
                arg_error = true;
            }
        }
    }

    if arg_error {
        tracing::error!("Invalid command-line arguments");
        eprintln!("Usage: shellumi-app [--setup-wizard] [--hosts <database-path>]");
        std::process::exit(2);
    }

    let (mut database, create_parent, mut startup_error) = if let Some(path) = custom_hosts {
        (Some(path), false, None)
    } else {
        match data_path::default_data_file().and_then(|path| {
            path.to_str()
                .map(str::to_owned)
                .ok_or_else(|| "The Shellumi data path is not valid UTF-8.".to_owned())
        }) {
            Ok(path) => (Some(path), true, None),
            Err(error) => (None, false, Some(error)),
        }
    };

    let mut pending_rotation_recoveries = Vec::new();
    if let Some(path) = database.clone() {
        let cred_store = shellumi_core::default_credential_store();
        let rotation_res = tokio::runtime::Builder::new_current_thread()
            .build()
            .map_err(|e| e.to_string())
            .and_then(|rt| {
                rt.block_on(vault_rotation::reconcile_pending_rotations_at_startup(
                    std::path::Path::new(&path),
                    cred_store.as_ref(),
                ))
            });
        match rotation_res {
            Ok(recoveries) => {
                if let Some(default_rec) = recoveries.iter().find(|r| {
                    r.vault_id == shellumi_storage::LEGACY_LOCAL_VAULT_ID && r.is_corrupted
                }) {
                    startup_error = Some(default_rec.reason.clone());
                    database = None;
                }
                pending_rotation_recoveries = recoveries;
            }
            Err(error) => {
                startup_error = Some(error);
                database = None;
            }
        }
        if database.is_some() {
            if let Err(error) =
                data_path::detect_and_recover_interrupted_quarantine(std::path::Path::new(&path))
            {
                startup_error = Some(error);
                database = None;
            }
        }
    }
    let should_run_wizard = force_wizard || {
        if let Some(ref db_path) = database {
            let base_path = std::path::Path::new(db_path);
            let base_dir = if base_path.is_dir() {
                base_path
            } else if let Some(parent) = base_path.parent() {
                if parent.file_name() == Some(std::ffi::OsStr::new("vaults")) {
                    parent.parent().unwrap_or(parent)
                } else {
                    parent
                }
            } else {
                std::path::Path::new(".")
            };

            !shellumi_core::setup::SetupState::is_setup_completed(base_dir)
        } else {
            false
        }
    };
    if let Some(ref err) = startup_error {
        tracing::error!(error = %err, "Data path initialization failed");
    }
    Application::new()
        .with_assets(crate::ui::icon::ShellumiAssetSource::new())
        .run(move |cx: &mut App| {
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
                        MenuItem::action("Setup Wizard…", RunSetupWizard),
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
                    items: vec![
                        MenuItem::action("Keyboard Shortcuts", OpenShortcutsHelp),
                        MenuItem::separator(),
                        MenuItem::action("Run Setup Wizard…", RunSetupWizard),
                    ],
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
                KeyBinding::new("cmd-shift-n", OpenVaultInNewWindow, None),
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
                KeyBinding::new("ctrl-shift-n", OpenVaultInNewWindow, None),
            ]);
            let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
            cx.open_window(
                ui::window_chrome::shellumi_window_options("Shellumi — Personal", bounds),
                |window, cx| {
                    let view = cx.new(|cx| Shellumi::new(database, window, cx));
                    view.update(cx, |app, cx| {
                        app.pending_rotation_recoveries = pending_rotation_recoveries.clone();
                        if let Some(error) = startup_error {
                            app.hosts = HostState::Error(error);
                            cx.notify();
                        } else if should_run_wizard {
                            app.open_setup_wizard(cx);
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

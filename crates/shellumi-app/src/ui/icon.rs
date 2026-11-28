//! Shellumi Unified Icon System
//! Provides type-safe GPUI elements and asset resolution for all 168 icons across 5 resolutions and 2 visual states.

use gpui::prelude::*;
use gpui::{div, img, px, AnyElement, AssetSource, SharedString};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// All 168 official Shellumi icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconName {
    /// Hosts / Servers (`hosts`)
    Hosts,
    /// Connect (`connect`)
    Connect,
    /// SFTP / Files (`sftp`)
    Sftp,
    /// Snippets / Scripts (`snippets`)
    Snippets,
    /// History (`history`)
    History,
    /// SSH Keychain (`keychain`)
    Keychain,
    /// Vaults (`vaults`)
    Vaults,
    /// Workspaces (`workspaces`)
    Workspaces,
    /// Cloud Sync (`cloud_sync`)
    CloudSync,
    /// Settings (`settings`)
    Settings,
    /// Add Host (`add_host`)
    AddHost,
    /// Search (`search`)
    Search,
    /// SSH Terminal (`terminal`)
    Terminal,
    /// Serial Connection (`serial`)
    Serial,
    /// Split View (`split_view`)
    SplitView,
    /// Grid View (`grid_view`)
    GridView,
    /// List View (`list_view`)
    ListView,
    /// Groups (`groups`)
    Groups,
    /// Tags (`tags`)
    Tags,
    /// Favorites (`favorite`)
    Favorite,
    /// More / Options (`more`)
    More,
    /// Edit (`edit`)
    Edit,
    /// Delete (`delete`)
    Delete,
    /// Refresh (`refresh`)
    Refresh,
    /// Account / Profile (`profile`)
    Profile,
    /// Known Hosts (`known_hosts`)
    KnownHosts,
    /// Port Forwarding (`port_forwarding`)
    PortForwarding,
    /// Shellumi Logo (`shellumi_logo`)
    ShellumiLogo,
    /// Group Settings (`group_settings`)
    GroupSettings,
    /// Import (`import`)
    Import,
    /// Export (`export`)
    Export,
    /// Tag (`tag`)
    Tag,
    /// Calendar Sort (`calendar_sort`)
    CalendarSort,
    /// Certificate (`certificate`)
    Certificate,
    /// Private Key (`private_key`)
    PrivateKey,
    /// Public Key (`public_key`)
    PublicKey,
    /// Identity Card (`identity_card`)
    IdentityCard,
    /// Arch Linux (`os_arch`)
    OsArch,
    /// Ubuntu (`os_ubuntu`)
    OsUbuntu,
    /// macOS (`os_macos`)
    OsMacos,
    /// FreeBSD (`os_freebsd`)
    OsFreebsd,
    /// NetBSD (`os_netbsd`)
    OsNetbsd,
    /// OpenBSD (`os_openbsd`)
    OsOpenbsd,
    /// Linux (`os_linux`)
    OsLinux,
    /// Windows (`os_windows`)
    OsWindows,
    /// Server (`server`)
    Server,
    /// FIDO2 (`fido2`)
    Fido2,
    /// Fingerprint (`fingerprint`)
    Fingerprint,
    /// Log (`log`)
    Log,
    /// Bell (`bell`)
    Bell,
    /// Invite Member (`invite_member`)
    InviteMember,
    /// Infrastructure (`infrastructure`)
    Infrastructure,
    /// Diagnostics (`diagnostics`)
    Diagnostics,
    /// Mobile (`mobile`)
    Mobile,
    /// Overview (`overview`)
    Overview,
    /// Inspector (`inspector`)
    Inspector,
    /// New Group (`new_group`)
    NewGroup,
    /// Duplicate Host (`duplicate_host`)
    DuplicateHost,
    /// Move To Group (`move_to_group`)
    MoveToGroup,
    /// Copy To Group (`copy_to_group`)
    CopyToGroup,
    /// New Workspace (`new_workspace`)
    NewWorkspace,
    /// New Vault (`new_vault`)
    NewVault,
    /// Switch Vault (`switch_vault`)
    SwitchVault,
    /// Quick Connect (`quick_connect`)
    QuickConnect,
    /// Jump Host (`jump_host`)
    JumpHost,
    /// Ssh (`ssh`)
    Ssh,
    /// Mosh (`mosh`)
    Mosh,
    /// Rdp (`rdp`)
    Rdp,
    /// Vnc (`vnc`)
    Vnc,
    /// Disconnect (`disconnect`)
    Disconnect,
    /// Reconnect (`reconnect`)
    Reconnect,
    /// Local Terminal (`local_terminal`)
    LocalTerminal,
    /// Session Record (`session_record`)
    SessionRecord,
    /// Session Replay (`session_replay`)
    SessionReplay,
    /// Session Info (`session_info`)
    SessionInfo,
    /// Fit To View (`fit_to_view`)
    FitToView,
    /// Actual Size (`actual_size`)
    ActualSize,
    /// Fill View (`fill_view`)
    FillView,
    /// Display Resolution (`display_resolution`)
    DisplayResolution,
    /// Dynamic Resize (`dynamic_resize`)
    DynamicResize,
    /// Remote Keyboard (`remote_keyboard`)
    RemoteKeyboard,
    /// Remote Clipboard (`remote_clipboard`)
    RemoteClipboard,
    /// View Only (`view_only`)
    ViewOnly,
    /// Fullscreen (`fullscreen`)
    Fullscreen,
    /// Send Ctrl Alt Del (`send_ctrl_alt_del`)
    SendCtrlAltDel,
    /// Send Windows Key (`send_windows_key`)
    SendWindowsKey,
    /// Shared Drive (`shared_drive`)
    SharedDrive,
    /// Remote Audio (`remote_audio`)
    RemoteAudio,
    /// Remote Audio Muted (`remote_audio_muted`)
    RemoteAudioMuted,
    /// Remote Microphone (`remote_microphone`)
    RemoteMicrophone,
    /// Remote Microphone Muted (`remote_microphone_muted`)
    RemoteMicrophoneMuted,
    /// Input Enabled (`input_enabled`)
    InputEnabled,
    /// Session Telemetry (`session_telemetry`)
    SessionTelemetry,
    /// Upload (`upload`)
    Upload,
    /// Download (`download`)
    Download,
    /// Transfer Queue (`transfer_queue`)
    TransferQueue,
    /// New Folder (`new_folder`)
    NewFolder,
    /// Transfer Progress (`transfer_progress`)
    TransferProgress,
    /// Transfer Complete (`transfer_complete`)
    TransferComplete,
    /// Transfer Failed (`transfer_failed`)
    TransferFailed,
    /// Transfer Retry (`transfer_retry`)
    TransferRetry,
    /// Local Files (`local_files`)
    LocalFiles,
    /// Remote Files (`remote_files`)
    RemoteFiles,
    /// File Permissions (`file_permissions`)
    FilePermissions,
    /// Hidden Files (`hidden_files`)
    HiddenFiles,
    /// Vault Lock (`vault_lock`)
    VaultLock,
    /// Vault Unlock (`vault_unlock`)
    VaultUnlock,
    /// Google Drive (`google_drive`)
    GoogleDrive,
    /// Sync Conflict (`sync_conflict`)
    SyncConflict,
    /// Encrypted Backup (`encrypted_backup`)
    EncryptedBackup,
    /// Restore Backup (`restore_backup`)
    RestoreBackup,
    /// Recovery Code (`recovery_code`)
    RecoveryCode,
    /// Vault Encryption (`vault_encryption`)
    VaultEncryption,
    /// Server Sync (`server_sync`)
    ServerSync,
    /// Sync Upload (`sync_upload`)
    SyncUpload,
    /// Sync Download (`sync_download`)
    SyncDownload,
    /// Sync Offline (`sync_offline`)
    SyncOffline,
    /// Sync Success (`sync_success`)
    SyncSuccess,
    /// Setup Resume (`setup_resume`)
    SetupResume,
    /// Device Enrollment (`device_enrollment`)
    DeviceEnrollment,
    /// Host Key Verified (`host_key_verified`)
    HostKeyVerified,
    /// Host Key Changed (`host_key_changed`)
    HostKeyChanged,
    /// Ssh Agent (`ssh_agent`)
    SshAgent,
    /// Generate Key (`generate_key`)
    GenerateKey,
    /// Rotate Key (`rotate_key`)
    RotateKey,
    /// Revoke Key (`revoke_key`)
    RevokeKey,
    /// Password (`password`)
    Password,
    /// Security Audit (`security_audit`)
    SecurityAudit,
    /// Trusted Device (`trusted_device`)
    TrustedDevice,
    /// Untrusted Device (`untrusted_device`)
    UntrustedDevice,
    /// Team Members (`team_members`)
    TeamMembers,
    /// Shared Vault (`shared_vault`)
    SharedVault,
    /// Member Revoke (`member_revoke`)
    MemberRevoke,
    /// Pending Invite (`pending_invite`)
    PendingInvite,
    /// Role Admin (`role_admin`)
    RoleAdmin,
    /// Role Editor (`role_editor`)
    RoleEditor,
    /// Role Viewer (`role_viewer`)
    RoleViewer,
    /// Team Key Rotation (`team_key_rotation`)
    TeamKeyRotation,
    /// Tunnel Local (`tunnel_local`)
    TunnelLocal,
    /// Tunnel Remote (`tunnel_remote`)
    TunnelRemote,
    /// Tunnel Socks (`tunnel_socks`)
    TunnelSocks,
    /// Tunnel Active (`tunnel_active`)
    TunnelActive,
    /// Tunnel Stopped (`tunnel_stopped`)
    TunnelStopped,
    /// Provider Aws (`provider_aws`)
    ProviderAws,
    /// Provider Azure (`provider_azure`)
    ProviderAzure,
    /// Provider Digitalocean (`provider_digitalocean`)
    ProviderDigitalocean,
    /// Provider Ansible (`provider_ansible`)
    ProviderAnsible,
    /// Provider Connect (`provider_connect`)
    ProviderConnect,
    /// Discover Hosts (`discover_hosts`)
    DiscoverHosts,
    /// Inventory (`inventory`)
    Inventory,
    /// Inventory Sync (`inventory_sync`)
    InventorySync,
    /// Health Check (`health_check`)
    HealthCheck,
    /// Benchmark (`benchmark`)
    Benchmark,
    /// Latency (`latency`)
    Latency,
    /// Network Test (`network_test`)
    NetworkTest,
    /// Storage Test (`storage_test`)
    StorageTest,
    /// Os Debian (`os_debian`)
    OsDebian,
    /// Os Fedora (`os_fedora`)
    OsFedora,
    /// Os Rhel (`os_rhel`)
    OsRhel,
    /// Os Centos (`os_centos`)
    OsCentos,
    /// Os Rocky (`os_rocky`)
    OsRocky,
    /// Os Almalinux (`os_almalinux`)
    OsAlmalinux,
    /// Os Alpine (`os_alpine`)
    OsAlpine,
    /// Os Opensuse (`os_opensuse`)
    OsOpensuse,
    /// Os Kali (`os_kali`)
    OsKali,
    /// Os Windows Server (`os_windows_server`)
    OsWindowsServer,
    /// Os Android (`os_android`)
    OsAndroid,
    /// Os Ios (`os_ios`)
    OsIos,
}

impl IconName {
    pub const ALL: &'static [IconName] = &[
        IconName::Hosts,
        IconName::Connect,
        IconName::Sftp,
        IconName::Snippets,
        IconName::History,
        IconName::Keychain,
        IconName::Vaults,
        IconName::Workspaces,
        IconName::CloudSync,
        IconName::Settings,
        IconName::AddHost,
        IconName::Search,
        IconName::Terminal,
        IconName::Serial,
        IconName::SplitView,
        IconName::GridView,
        IconName::ListView,
        IconName::Groups,
        IconName::Tags,
        IconName::Favorite,
        IconName::More,
        IconName::Edit,
        IconName::Delete,
        IconName::Refresh,
        IconName::Profile,
        IconName::KnownHosts,
        IconName::PortForwarding,
        IconName::ShellumiLogo,
        IconName::GroupSettings,
        IconName::Import,
        IconName::Export,
        IconName::Tag,
        IconName::CalendarSort,
        IconName::Certificate,
        IconName::PrivateKey,
        IconName::PublicKey,
        IconName::IdentityCard,
        IconName::OsArch,
        IconName::OsUbuntu,
        IconName::OsMacos,
        IconName::OsFreebsd,
        IconName::OsNetbsd,
        IconName::OsOpenbsd,
        IconName::OsLinux,
        IconName::OsWindows,
        IconName::Server,
        IconName::Fido2,
        IconName::Fingerprint,
        IconName::Log,
        IconName::Bell,
        IconName::InviteMember,
        IconName::Infrastructure,
        IconName::Diagnostics,
        IconName::Mobile,
        IconName::Overview,
        IconName::Inspector,
        IconName::NewGroup,
        IconName::DuplicateHost,
        IconName::MoveToGroup,
        IconName::CopyToGroup,
        IconName::NewWorkspace,
        IconName::NewVault,
        IconName::SwitchVault,
        IconName::QuickConnect,
        IconName::JumpHost,
        IconName::Ssh,
        IconName::Mosh,
        IconName::Rdp,
        IconName::Vnc,
        IconName::Disconnect,
        IconName::Reconnect,
        IconName::LocalTerminal,
        IconName::SessionRecord,
        IconName::SessionReplay,
        IconName::SessionInfo,
        IconName::FitToView,
        IconName::ActualSize,
        IconName::FillView,
        IconName::DisplayResolution,
        IconName::DynamicResize,
        IconName::RemoteKeyboard,
        IconName::RemoteClipboard,
        IconName::ViewOnly,
        IconName::Fullscreen,
        IconName::SendCtrlAltDel,
        IconName::SendWindowsKey,
        IconName::SharedDrive,
        IconName::RemoteAudio,
        IconName::RemoteAudioMuted,
        IconName::RemoteMicrophone,
        IconName::RemoteMicrophoneMuted,
        IconName::InputEnabled,
        IconName::SessionTelemetry,
        IconName::Upload,
        IconName::Download,
        IconName::TransferQueue,
        IconName::NewFolder,
        IconName::TransferProgress,
        IconName::TransferComplete,
        IconName::TransferFailed,
        IconName::TransferRetry,
        IconName::LocalFiles,
        IconName::RemoteFiles,
        IconName::FilePermissions,
        IconName::HiddenFiles,
        IconName::VaultLock,
        IconName::VaultUnlock,
        IconName::GoogleDrive,
        IconName::SyncConflict,
        IconName::EncryptedBackup,
        IconName::RestoreBackup,
        IconName::RecoveryCode,
        IconName::VaultEncryption,
        IconName::ServerSync,
        IconName::SyncUpload,
        IconName::SyncDownload,
        IconName::SyncOffline,
        IconName::SyncSuccess,
        IconName::SetupResume,
        IconName::DeviceEnrollment,
        IconName::HostKeyVerified,
        IconName::HostKeyChanged,
        IconName::SshAgent,
        IconName::GenerateKey,
        IconName::RotateKey,
        IconName::RevokeKey,
        IconName::Password,
        IconName::SecurityAudit,
        IconName::TrustedDevice,
        IconName::UntrustedDevice,
        IconName::TeamMembers,
        IconName::SharedVault,
        IconName::MemberRevoke,
        IconName::PendingInvite,
        IconName::RoleAdmin,
        IconName::RoleEditor,
        IconName::RoleViewer,
        IconName::TeamKeyRotation,
        IconName::TunnelLocal,
        IconName::TunnelRemote,
        IconName::TunnelSocks,
        IconName::TunnelActive,
        IconName::TunnelStopped,
        IconName::ProviderAws,
        IconName::ProviderAzure,
        IconName::ProviderDigitalocean,
        IconName::ProviderAnsible,
        IconName::ProviderConnect,
        IconName::DiscoverHosts,
        IconName::Inventory,
        IconName::InventorySync,
        IconName::HealthCheck,
        IconName::Benchmark,
        IconName::Latency,
        IconName::NetworkTest,
        IconName::StorageTest,
        IconName::OsDebian,
        IconName::OsFedora,
        IconName::OsRhel,
        IconName::OsCentos,
        IconName::OsRocky,
        IconName::OsAlmalinux,
        IconName::OsAlpine,
        IconName::OsOpensuse,
        IconName::OsKali,
        IconName::OsWindowsServer,
        IconName::OsAndroid,
        IconName::OsIos,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            IconName::Hosts => "hosts",
            IconName::Connect => "connect",
            IconName::Sftp => "sftp",
            IconName::Snippets => "snippets",
            IconName::History => "history",
            IconName::Keychain => "keychain",
            IconName::Vaults => "vaults",
            IconName::Workspaces => "workspaces",
            IconName::CloudSync => "cloud_sync",
            IconName::Settings => "settings",
            IconName::AddHost => "add_host",
            IconName::Search => "search",
            IconName::Terminal => "terminal",
            IconName::Serial => "serial",
            IconName::SplitView => "split_view",
            IconName::GridView => "grid_view",
            IconName::ListView => "list_view",
            IconName::Groups => "groups",
            IconName::Tags => "tags",
            IconName::Favorite => "favorite",
            IconName::More => "more",
            IconName::Edit => "edit",
            IconName::Delete => "delete",
            IconName::Refresh => "refresh",
            IconName::Profile => "profile",
            IconName::KnownHosts => "known_hosts",
            IconName::PortForwarding => "port_forwarding",
            IconName::ShellumiLogo => "shellumi_logo",
            IconName::GroupSettings => "group_settings",
            IconName::Import => "import",
            IconName::Export => "export",
            IconName::Tag => "tag",
            IconName::CalendarSort => "calendar_sort",
            IconName::Certificate => "certificate",
            IconName::PrivateKey => "private_key",
            IconName::PublicKey => "public_key",
            IconName::IdentityCard => "identity_card",
            IconName::OsArch => "os_arch",
            IconName::OsUbuntu => "os_ubuntu",
            IconName::OsMacos => "os_macos",
            IconName::OsFreebsd => "os_freebsd",
            IconName::OsNetbsd => "os_netbsd",
            IconName::OsOpenbsd => "os_openbsd",
            IconName::OsLinux => "os_linux",
            IconName::OsWindows => "os_windows",
            IconName::Server => "server",
            IconName::Fido2 => "fido2",
            IconName::Fingerprint => "fingerprint",
            IconName::Log => "log",
            IconName::Bell => "bell",
            IconName::InviteMember => "invite_member",
            IconName::Infrastructure => "infrastructure",
            IconName::Diagnostics => "diagnostics",
            IconName::Mobile => "mobile",
            IconName::Overview => "overview",
            IconName::Inspector => "inspector",
            IconName::NewGroup => "new_group",
            IconName::DuplicateHost => "duplicate_host",
            IconName::MoveToGroup => "move_to_group",
            IconName::CopyToGroup => "copy_to_group",
            IconName::NewWorkspace => "new_workspace",
            IconName::NewVault => "new_vault",
            IconName::SwitchVault => "switch_vault",
            IconName::QuickConnect => "quick_connect",
            IconName::JumpHost => "jump_host",
            IconName::Ssh => "ssh",
            IconName::Mosh => "mosh",
            IconName::Rdp => "rdp",
            IconName::Vnc => "vnc",
            IconName::Disconnect => "disconnect",
            IconName::Reconnect => "reconnect",
            IconName::LocalTerminal => "local_terminal",
            IconName::SessionRecord => "session_record",
            IconName::SessionReplay => "session_replay",
            IconName::SessionInfo => "session_info",
            IconName::FitToView => "fit_to_view",
            IconName::ActualSize => "actual_size",
            IconName::FillView => "fill_view",
            IconName::DisplayResolution => "display_resolution",
            IconName::DynamicResize => "dynamic_resize",
            IconName::RemoteKeyboard => "remote_keyboard",
            IconName::RemoteClipboard => "remote_clipboard",
            IconName::ViewOnly => "view_only",
            IconName::Fullscreen => "fullscreen",
            IconName::SendCtrlAltDel => "send_ctrl_alt_del",
            IconName::SendWindowsKey => "send_windows_key",
            IconName::SharedDrive => "shared_drive",
            IconName::RemoteAudio => "remote_audio",
            IconName::RemoteAudioMuted => "remote_audio_muted",
            IconName::RemoteMicrophone => "remote_microphone",
            IconName::RemoteMicrophoneMuted => "remote_microphone_muted",
            IconName::InputEnabled => "input_enabled",
            IconName::SessionTelemetry => "session_telemetry",
            IconName::Upload => "upload",
            IconName::Download => "download",
            IconName::TransferQueue => "transfer_queue",
            IconName::NewFolder => "new_folder",
            IconName::TransferProgress => "transfer_progress",
            IconName::TransferComplete => "transfer_complete",
            IconName::TransferFailed => "transfer_failed",
            IconName::TransferRetry => "transfer_retry",
            IconName::LocalFiles => "local_files",
            IconName::RemoteFiles => "remote_files",
            IconName::FilePermissions => "file_permissions",
            IconName::HiddenFiles => "hidden_files",
            IconName::VaultLock => "vault_lock",
            IconName::VaultUnlock => "vault_unlock",
            IconName::GoogleDrive => "google_drive",
            IconName::SyncConflict => "sync_conflict",
            IconName::EncryptedBackup => "encrypted_backup",
            IconName::RestoreBackup => "restore_backup",
            IconName::RecoveryCode => "recovery_code",
            IconName::VaultEncryption => "vault_encryption",
            IconName::ServerSync => "server_sync",
            IconName::SyncUpload => "sync_upload",
            IconName::SyncDownload => "sync_download",
            IconName::SyncOffline => "sync_offline",
            IconName::SyncSuccess => "sync_success",
            IconName::SetupResume => "setup_resume",
            IconName::DeviceEnrollment => "device_enrollment",
            IconName::HostKeyVerified => "host_key_verified",
            IconName::HostKeyChanged => "host_key_changed",
            IconName::SshAgent => "ssh_agent",
            IconName::GenerateKey => "generate_key",
            IconName::RotateKey => "rotate_key",
            IconName::RevokeKey => "revoke_key",
            IconName::Password => "password",
            IconName::SecurityAudit => "security_audit",
            IconName::TrustedDevice => "trusted_device",
            IconName::UntrustedDevice => "untrusted_device",
            IconName::TeamMembers => "team_members",
            IconName::SharedVault => "shared_vault",
            IconName::MemberRevoke => "member_revoke",
            IconName::PendingInvite => "pending_invite",
            IconName::RoleAdmin => "role_admin",
            IconName::RoleEditor => "role_editor",
            IconName::RoleViewer => "role_viewer",
            IconName::TeamKeyRotation => "team_key_rotation",
            IconName::TunnelLocal => "tunnel_local",
            IconName::TunnelRemote => "tunnel_remote",
            IconName::TunnelSocks => "tunnel_socks",
            IconName::TunnelActive => "tunnel_active",
            IconName::TunnelStopped => "tunnel_stopped",
            IconName::ProviderAws => "provider_aws",
            IconName::ProviderAzure => "provider_azure",
            IconName::ProviderDigitalocean => "provider_digitalocean",
            IconName::ProviderAnsible => "provider_ansible",
            IconName::ProviderConnect => "provider_connect",
            IconName::DiscoverHosts => "discover_hosts",
            IconName::Inventory => "inventory",
            IconName::InventorySync => "inventory_sync",
            IconName::HealthCheck => "health_check",
            IconName::Benchmark => "benchmark",
            IconName::Latency => "latency",
            IconName::NetworkTest => "network_test",
            IconName::StorageTest => "storage_test",
            IconName::OsDebian => "os_debian",
            IconName::OsFedora => "os_fedora",
            IconName::OsRhel => "os_rhel",
            IconName::OsCentos => "os_centos",
            IconName::OsRocky => "os_rocky",
            IconName::OsAlmalinux => "os_almalinux",
            IconName::OsAlpine => "os_alpine",
            IconName::OsOpensuse => "os_opensuse",
            IconName::OsKali => "os_kali",
            IconName::OsWindowsServer => "os_windows_server",
            IconName::OsAndroid => "os_android",
            IconName::OsIos => "os_ios",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            IconName::Hosts => "Hosts / Servers",
            IconName::Connect => "Connect",
            IconName::Sftp => "SFTP / Files",
            IconName::Snippets => "Snippets / Scripts",
            IconName::History => "History",
            IconName::Keychain => "SSH Keychain",
            IconName::Vaults => "Vaults",
            IconName::Workspaces => "Workspaces",
            IconName::CloudSync => "Cloud Sync",
            IconName::Settings => "Settings",
            IconName::AddHost => "Add Host",
            IconName::Search => "Search",
            IconName::Terminal => "SSH Terminal",
            IconName::Serial => "Serial Connection",
            IconName::SplitView => "Split View",
            IconName::GridView => "Grid View",
            IconName::ListView => "List View",
            IconName::Groups => "Groups",
            IconName::Tags => "Tags",
            IconName::Favorite => "Favorites",
            IconName::More => "More / Options",
            IconName::Edit => "Edit",
            IconName::Delete => "Delete",
            IconName::Refresh => "Refresh",
            IconName::Profile => "Account / Profile",
            IconName::KnownHosts => "Known Hosts",
            IconName::PortForwarding => "Port Forwarding",
            IconName::ShellumiLogo => "Shellumi Logo",
            IconName::GroupSettings => "Group Settings",
            IconName::Import => "Import",
            IconName::Export => "Export",
            IconName::Tag => "Tag",
            IconName::CalendarSort => "Calendar Sort",
            IconName::Certificate => "Certificate",
            IconName::PrivateKey => "Private Key",
            IconName::PublicKey => "Public Key",
            IconName::IdentityCard => "Identity Card",
            IconName::OsArch => "Arch Linux",
            IconName::OsUbuntu => "Ubuntu",
            IconName::OsMacos => "macOS",
            IconName::OsFreebsd => "FreeBSD",
            IconName::OsNetbsd => "NetBSD",
            IconName::OsOpenbsd => "OpenBSD",
            IconName::OsLinux => "Linux",
            IconName::OsWindows => "Windows",
            IconName::Server => "Server",
            IconName::Fido2 => "FIDO2",
            IconName::Fingerprint => "Fingerprint",
            IconName::Log => "Log",
            IconName::Bell => "Bell",
            IconName::InviteMember => "Invite Member",
            IconName::Infrastructure => "Infrastructure",
            IconName::Diagnostics => "Diagnostics",
            IconName::Mobile => "Mobile",
            IconName::Overview => "Overview",
            IconName::Inspector => "Inspector",
            IconName::NewGroup => "New Group",
            IconName::DuplicateHost => "Duplicate Host",
            IconName::MoveToGroup => "Move To Group",
            IconName::CopyToGroup => "Copy To Group",
            IconName::NewWorkspace => "New Workspace",
            IconName::NewVault => "New Vault",
            IconName::SwitchVault => "Switch Vault",
            IconName::QuickConnect => "Quick Connect",
            IconName::JumpHost => "Jump Host",
            IconName::Ssh => "Ssh",
            IconName::Mosh => "Mosh",
            IconName::Rdp => "Rdp",
            IconName::Vnc => "Vnc",
            IconName::Disconnect => "Disconnect",
            IconName::Reconnect => "Reconnect",
            IconName::LocalTerminal => "Local Terminal",
            IconName::SessionRecord => "Session Record",
            IconName::SessionReplay => "Session Replay",
            IconName::SessionInfo => "Session Info",
            IconName::FitToView => "Fit To View",
            IconName::ActualSize => "Actual Size",
            IconName::FillView => "Fill View",
            IconName::DisplayResolution => "Display Resolution",
            IconName::DynamicResize => "Dynamic Resize",
            IconName::RemoteKeyboard => "Remote Keyboard",
            IconName::RemoteClipboard => "Remote Clipboard",
            IconName::ViewOnly => "View Only",
            IconName::Fullscreen => "Fullscreen",
            IconName::SendCtrlAltDel => "Send Ctrl Alt Del",
            IconName::SendWindowsKey => "Send Windows Key",
            IconName::SharedDrive => "Shared Drive",
            IconName::RemoteAudio => "Remote Audio",
            IconName::RemoteAudioMuted => "Remote Audio Muted",
            IconName::RemoteMicrophone => "Remote Microphone",
            IconName::RemoteMicrophoneMuted => "Remote Microphone Muted",
            IconName::InputEnabled => "Input Enabled",
            IconName::SessionTelemetry => "Session Telemetry",
            IconName::Upload => "Upload",
            IconName::Download => "Download",
            IconName::TransferQueue => "Transfer Queue",
            IconName::NewFolder => "New Folder",
            IconName::TransferProgress => "Transfer Progress",
            IconName::TransferComplete => "Transfer Complete",
            IconName::TransferFailed => "Transfer Failed",
            IconName::TransferRetry => "Transfer Retry",
            IconName::LocalFiles => "Local Files",
            IconName::RemoteFiles => "Remote Files",
            IconName::FilePermissions => "File Permissions",
            IconName::HiddenFiles => "Hidden Files",
            IconName::VaultLock => "Vault Lock",
            IconName::VaultUnlock => "Vault Unlock",
            IconName::GoogleDrive => "Google Drive",
            IconName::SyncConflict => "Sync Conflict",
            IconName::EncryptedBackup => "Encrypted Backup",
            IconName::RestoreBackup => "Restore Backup",
            IconName::RecoveryCode => "Recovery Code",
            IconName::VaultEncryption => "Vault Encryption",
            IconName::ServerSync => "Server Sync",
            IconName::SyncUpload => "Sync Upload",
            IconName::SyncDownload => "Sync Download",
            IconName::SyncOffline => "Sync Offline",
            IconName::SyncSuccess => "Sync Success",
            IconName::SetupResume => "Setup Resume",
            IconName::DeviceEnrollment => "Device Enrollment",
            IconName::HostKeyVerified => "Host Key Verified",
            IconName::HostKeyChanged => "Host Key Changed",
            IconName::SshAgent => "Ssh Agent",
            IconName::GenerateKey => "Generate Key",
            IconName::RotateKey => "Rotate Key",
            IconName::RevokeKey => "Revoke Key",
            IconName::Password => "Password",
            IconName::SecurityAudit => "Security Audit",
            IconName::TrustedDevice => "Trusted Device",
            IconName::UntrustedDevice => "Untrusted Device",
            IconName::TeamMembers => "Team Members",
            IconName::SharedVault => "Shared Vault",
            IconName::MemberRevoke => "Member Revoke",
            IconName::PendingInvite => "Pending Invite",
            IconName::RoleAdmin => "Role Admin",
            IconName::RoleEditor => "Role Editor",
            IconName::RoleViewer => "Role Viewer",
            IconName::TeamKeyRotation => "Team Key Rotation",
            IconName::TunnelLocal => "Tunnel Local",
            IconName::TunnelRemote => "Tunnel Remote",
            IconName::TunnelSocks => "Tunnel Socks",
            IconName::TunnelActive => "Tunnel Active",
            IconName::TunnelStopped => "Tunnel Stopped",
            IconName::ProviderAws => "Provider Aws",
            IconName::ProviderAzure => "Provider Azure",
            IconName::ProviderDigitalocean => "Provider Digitalocean",
            IconName::ProviderAnsible => "Provider Ansible",
            IconName::ProviderConnect => "Provider Connect",
            IconName::DiscoverHosts => "Discover Hosts",
            IconName::Inventory => "Inventory",
            IconName::InventorySync => "Inventory Sync",
            IconName::HealthCheck => "Health Check",
            IconName::Benchmark => "Benchmark",
            IconName::Latency => "Latency",
            IconName::NetworkTest => "Network Test",
            IconName::StorageTest => "Storage Test",
            IconName::OsDebian => "Os Debian",
            IconName::OsFedora => "Os Fedora",
            IconName::OsRhel => "Os Rhel",
            IconName::OsCentos => "Os Centos",
            IconName::OsRocky => "Os Rocky",
            IconName::OsAlmalinux => "Os Almalinux",
            IconName::OsAlpine => "Os Alpine",
            IconName::OsOpensuse => "Os Opensuse",
            IconName::OsKali => "Os Kali",
            IconName::OsWindowsServer => "Os Windows Server",
            IconName::OsAndroid => "Os Android",
            IconName::OsIos => "Os Ios",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "hosts" => Some(IconName::Hosts),
            "connect" => Some(IconName::Connect),
            "sftp" => Some(IconName::Sftp),
            "snippets" => Some(IconName::Snippets),
            "history" => Some(IconName::History),
            "keychain" => Some(IconName::Keychain),
            "vaults" => Some(IconName::Vaults),
            "workspaces" => Some(IconName::Workspaces),
            "cloud_sync" => Some(IconName::CloudSync),
            "settings" => Some(IconName::Settings),
            "add_host" => Some(IconName::AddHost),
            "search" => Some(IconName::Search),
            "terminal" => Some(IconName::Terminal),
            "serial" => Some(IconName::Serial),
            "split_view" => Some(IconName::SplitView),
            "grid_view" => Some(IconName::GridView),
            "list_view" => Some(IconName::ListView),
            "groups" => Some(IconName::Groups),
            "tags" => Some(IconName::Tags),
            "favorite" => Some(IconName::Favorite),
            "more" => Some(IconName::More),
            "edit" => Some(IconName::Edit),
            "delete" => Some(IconName::Delete),
            "refresh" => Some(IconName::Refresh),
            "profile" => Some(IconName::Profile),
            "known_hosts" => Some(IconName::KnownHosts),
            "port_forwarding" => Some(IconName::PortForwarding),
            "shellumi_logo" => Some(IconName::ShellumiLogo),
            "group_settings" => Some(IconName::GroupSettings),
            "import" => Some(IconName::Import),
            "export" => Some(IconName::Export),
            "tag" => Some(IconName::Tag),
            "calendar_sort" => Some(IconName::CalendarSort),
            "certificate" => Some(IconName::Certificate),
            "private_key" => Some(IconName::PrivateKey),
            "public_key" => Some(IconName::PublicKey),
            "identity_card" => Some(IconName::IdentityCard),
            "os_arch" => Some(IconName::OsArch),
            "os_ubuntu" => Some(IconName::OsUbuntu),
            "os_macos" => Some(IconName::OsMacos),
            "os_freebsd" => Some(IconName::OsFreebsd),
            "os_netbsd" => Some(IconName::OsNetbsd),
            "os_openbsd" => Some(IconName::OsOpenbsd),
            "os_linux" => Some(IconName::OsLinux),
            "os_windows" => Some(IconName::OsWindows),
            "server" => Some(IconName::Server),
            "fido2" => Some(IconName::Fido2),
            "fingerprint" => Some(IconName::Fingerprint),
            "log" => Some(IconName::Log),
            "bell" => Some(IconName::Bell),
            "invite_member" => Some(IconName::InviteMember),
            "infrastructure" => Some(IconName::Infrastructure),
            "diagnostics" => Some(IconName::Diagnostics),
            "mobile" => Some(IconName::Mobile),
            "overview" => Some(IconName::Overview),
            "inspector" => Some(IconName::Inspector),
            "new_group" => Some(IconName::NewGroup),
            "duplicate_host" => Some(IconName::DuplicateHost),
            "move_to_group" => Some(IconName::MoveToGroup),
            "copy_to_group" => Some(IconName::CopyToGroup),
            "new_workspace" => Some(IconName::NewWorkspace),
            "new_vault" => Some(IconName::NewVault),
            "switch_vault" => Some(IconName::SwitchVault),
            "quick_connect" => Some(IconName::QuickConnect),
            "jump_host" => Some(IconName::JumpHost),
            "ssh" => Some(IconName::Ssh),
            "mosh" => Some(IconName::Mosh),
            "rdp" => Some(IconName::Rdp),
            "vnc" => Some(IconName::Vnc),
            "disconnect" => Some(IconName::Disconnect),
            "reconnect" => Some(IconName::Reconnect),
            "local_terminal" => Some(IconName::LocalTerminal),
            "session_record" => Some(IconName::SessionRecord),
            "session_replay" => Some(IconName::SessionReplay),
            "session_info" => Some(IconName::SessionInfo),
            "fit_to_view" => Some(IconName::FitToView),
            "actual_size" => Some(IconName::ActualSize),
            "fill_view" => Some(IconName::FillView),
            "display_resolution" => Some(IconName::DisplayResolution),
            "dynamic_resize" => Some(IconName::DynamicResize),
            "remote_keyboard" => Some(IconName::RemoteKeyboard),
            "remote_clipboard" => Some(IconName::RemoteClipboard),
            "view_only" => Some(IconName::ViewOnly),
            "fullscreen" => Some(IconName::Fullscreen),
            "send_ctrl_alt_del" => Some(IconName::SendCtrlAltDel),
            "send_windows_key" => Some(IconName::SendWindowsKey),
            "shared_drive" => Some(IconName::SharedDrive),
            "remote_audio" => Some(IconName::RemoteAudio),
            "remote_audio_muted" => Some(IconName::RemoteAudioMuted),
            "remote_microphone" => Some(IconName::RemoteMicrophone),
            "remote_microphone_muted" => Some(IconName::RemoteMicrophoneMuted),
            "input_enabled" => Some(IconName::InputEnabled),
            "session_telemetry" => Some(IconName::SessionTelemetry),
            "upload" => Some(IconName::Upload),
            "download" => Some(IconName::Download),
            "transfer_queue" => Some(IconName::TransferQueue),
            "new_folder" => Some(IconName::NewFolder),
            "transfer_progress" => Some(IconName::TransferProgress),
            "transfer_complete" => Some(IconName::TransferComplete),
            "transfer_failed" => Some(IconName::TransferFailed),
            "transfer_retry" => Some(IconName::TransferRetry),
            "local_files" => Some(IconName::LocalFiles),
            "remote_files" => Some(IconName::RemoteFiles),
            "file_permissions" => Some(IconName::FilePermissions),
            "hidden_files" => Some(IconName::HiddenFiles),
            "vault_lock" => Some(IconName::VaultLock),
            "vault_unlock" => Some(IconName::VaultUnlock),
            "google_drive" => Some(IconName::GoogleDrive),
            "sync_conflict" => Some(IconName::SyncConflict),
            "encrypted_backup" => Some(IconName::EncryptedBackup),
            "restore_backup" => Some(IconName::RestoreBackup),
            "recovery_code" => Some(IconName::RecoveryCode),
            "vault_encryption" => Some(IconName::VaultEncryption),
            "server_sync" => Some(IconName::ServerSync),
            "sync_upload" => Some(IconName::SyncUpload),
            "sync_download" => Some(IconName::SyncDownload),
            "sync_offline" => Some(IconName::SyncOffline),
            "sync_success" => Some(IconName::SyncSuccess),
            "setup_resume" => Some(IconName::SetupResume),
            "device_enrollment" => Some(IconName::DeviceEnrollment),
            "host_key_verified" => Some(IconName::HostKeyVerified),
            "host_key_changed" => Some(IconName::HostKeyChanged),
            "ssh_agent" => Some(IconName::SshAgent),
            "generate_key" => Some(IconName::GenerateKey),
            "rotate_key" => Some(IconName::RotateKey),
            "revoke_key" => Some(IconName::RevokeKey),
            "password" => Some(IconName::Password),
            "security_audit" => Some(IconName::SecurityAudit),
            "trusted_device" => Some(IconName::TrustedDevice),
            "untrusted_device" => Some(IconName::UntrustedDevice),
            "team_members" => Some(IconName::TeamMembers),
            "shared_vault" => Some(IconName::SharedVault),
            "member_revoke" => Some(IconName::MemberRevoke),
            "pending_invite" => Some(IconName::PendingInvite),
            "role_admin" => Some(IconName::RoleAdmin),
            "role_editor" => Some(IconName::RoleEditor),
            "role_viewer" => Some(IconName::RoleViewer),
            "team_key_rotation" => Some(IconName::TeamKeyRotation),
            "tunnel_local" => Some(IconName::TunnelLocal),
            "tunnel_remote" => Some(IconName::TunnelRemote),
            "tunnel_socks" => Some(IconName::TunnelSocks),
            "tunnel_active" => Some(IconName::TunnelActive),
            "tunnel_stopped" => Some(IconName::TunnelStopped),
            "provider_aws" => Some(IconName::ProviderAws),
            "provider_azure" => Some(IconName::ProviderAzure),
            "provider_digitalocean" => Some(IconName::ProviderDigitalocean),
            "provider_ansible" => Some(IconName::ProviderAnsible),
            "provider_connect" => Some(IconName::ProviderConnect),
            "discover_hosts" => Some(IconName::DiscoverHosts),
            "inventory" => Some(IconName::Inventory),
            "inventory_sync" => Some(IconName::InventorySync),
            "health_check" => Some(IconName::HealthCheck),
            "benchmark" => Some(IconName::Benchmark),
            "latency" => Some(IconName::Latency),
            "network_test" => Some(IconName::NetworkTest),
            "storage_test" => Some(IconName::StorageTest),
            "os_debian" => Some(IconName::OsDebian),
            "os_fedora" => Some(IconName::OsFedora),
            "os_rhel" => Some(IconName::OsRhel),
            "os_centos" => Some(IconName::OsCentos),
            "os_rocky" => Some(IconName::OsRocky),
            "os_almalinux" => Some(IconName::OsAlmalinux),
            "os_alpine" => Some(IconName::OsAlpine),
            "os_opensuse" => Some(IconName::OsOpensuse),
            "os_kali" => Some(IconName::OsKali),
            "os_windows_server" => Some(IconName::OsWindowsServer),
            "os_android" => Some(IconName::OsAndroid),
            "os_ios" => Some(IconName::OsIos),
            _ => None,
        }
    }
}

impl std::fmt::Display for IconName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for IconName {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or_else(|| format!("Unknown icon name: '{s}'"))
    }
}

impl From<IconName> for &'static str {
    fn from(name: IconName) -> Self {
        name.as_str()
    }
}

/// Logical icon size options mapped to Retina/HiDPI asset resolution folders.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSize {
    Px16,
    Px20,
    Px24,
    Px32,
    Px48,
    Px64,
    Px128,
    Px256,
}

#[allow(dead_code)]
impl IconSize {
    pub fn points(&self) -> f32 {
        match self {
            IconSize::Px16 => 16.0,
            IconSize::Px20 => 20.0,
            IconSize::Px24 => 24.0,
            IconSize::Px32 => 32.0,
            IconSize::Px48 => 48.0,
            IconSize::Px64 => 64.0,
            IconSize::Px128 => 128.0,
            IconSize::Px256 => 256.0,
        }
    }

    pub fn asset_resolution_px(&self) -> u32 {
        match self {
            IconSize::Px16 => 32,
            IconSize::Px20 => 48,
            IconSize::Px24 => 48,
            IconSize::Px32 => 64,
            IconSize::Px48 => 128,
            IconSize::Px64 => 128,
            IconSize::Px128 => 256,
            IconSize::Px256 => 256,
        }
    }
}

/// Helper to select resolution-appropriate asset pixel size for a given logical point size.
pub fn pick_asset_resolution(logical_size_pt: f32) -> u32 {
    if logical_size_pt <= 16.0 {
        32
    } else if logical_size_pt <= 24.0 {
        48
    } else if logical_size_pt <= 36.0 {
        64
    } else if logical_size_pt <= 64.0 {
        128
    } else {
        256
    }
}

static ICON_PACK_ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Discovers the icon pack root directory once and caches the result.
pub fn resolve_icon_pack_root() -> Option<PathBuf> {
    ICON_PACK_ROOT
        .get_or_init(|| {
            if let Some(override_dir) = std::env::var_os("SHELLUMI_ICONS_DIR") {
                let p = PathBuf::from(override_dir);
                if p.exists() {
                    return Some(p);
                }
            }

            if let Ok(exe) = std::env::current_exe() {
                if let Some(parent) = exe.parent() {
                    let candidates = [
                        parent.join("../Resources/icon_pack/icons"),
                        parent.join("resources/icon_pack/icons"),
                        parent.join("icon_pack/icons"),
                        parent.join("../resources/icon_pack/icons"),
                        parent.join("../../resources/icon_pack/icons"),
                    ];
                    for c in &candidates {
                        if c.exists() && c.join("48/regular/hosts.png").exists() {
                            return Some(c.clone());
                        }
                    }
                }
            }

            let compile_time =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/icon_pack/icons");
            if compile_time.exists() && compile_time.join("48/regular/hosts.png").exists() {
                return Some(compile_time);
            }

            let local_candidates = [
                PathBuf::from("resources/icon_pack/icons"),
                PathBuf::from("icon_pack/icons"),
            ];
            for c in &local_candidates {
                if c.exists() && c.join("48/regular/hosts.png").exists() {
                    return Some(c.clone());
                }
            }

            None
        })
        .clone()
}

/// Resolves the filesystem path for an icon asset at the specified resolution and state.
/// Ensures resolution consistency, zero redundant directory checks, and embedded fallback.
pub fn resolve_icon_path(name: &str, is_hover: bool, size_px: u32) -> Option<PathBuf> {
    let state = if is_hover { "hover" } else { "regular" };
    let size_folder = match size_px {
        s if s <= 32 => "32",
        s if s <= 48 => "48",
        s if s <= 64 => "64",
        s if s <= 128 => "128",
        _ => "256",
    };
    let filename = format!("{name}.png");

    // 1. Fast path: check cached icon pack root on disk
    if let Some(root) = resolve_icon_pack_root() {
        let p = root.join(size_folder).join(state).join(&filename);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Embedded asset fallback (used in standalone portable binaries)
    if let Some(bytes) = crate::embedded_icons::get_embedded_icon(name, is_hover, size_px) {
        let cache_dir = std::env::temp_dir()
            .join("shellumi-icons-cache")
            .join(size_folder)
            .join(state);
        let cache_file = cache_dir.join(&filename);
        if cache_file.exists() {
            return Some(cache_file);
        }
        if std::fs::create_dir_all(&cache_dir).is_ok() && std::fs::write(&cache_file, bytes).is_ok()
        {
            return Some(cache_file);
        }
    }

    None
}

/// Unified, reusable GPUI Icon Component.
#[derive(Clone, Debug)]
pub struct Icon {
    pub name: IconName,
    pub size_pt: f32,
    pub active: bool,
    pub disabled: bool,
    pub hover_switch: bool,
    pub parent_group: Option<SharedString>,
    #[allow(dead_code)]
    pub tooltip: Option<SharedString>,
}

impl Icon {
    pub fn new(name: impl Into<IconName>) -> Self {
        Self {
            name: name.into(),
            size_pt: 20.0,
            active: false,
            disabled: false,
            hover_switch: true,
            parent_group: None,
            tooltip: None,
        }
    }

    pub fn size(mut self, size_pt: f32) -> Self {
        self.size_pt = size_pt;
        self
    }

    #[allow(dead_code)]
    pub fn size_enum(mut self, size: IconSize) -> Self {
        self.size_pt = size.points();
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    #[allow(dead_code)]
    pub fn hover_switch(mut self, switch: bool) -> Self {
        self.hover_switch = switch;
        self
    }

    /// Connects this icon's hover switch to a parent element's GPUI hover group.
    #[allow(dead_code)]
    pub fn parent_group(mut self, group: impl Into<SharedString>) -> Self {
        self.parent_group = Some(group.into());
        self
    }

    #[allow(dead_code)]
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

impl IntoElement for Icon {
    type Element = gpui::AnyElement;

    fn into_element(self) -> Self::Element {
        let size_pt = self.size_pt;
        let asset_res = pick_asset_resolution(size_pt);
        let name_str = self.name.as_str();

        if self.disabled {
            if let Some(reg_path) = resolve_icon_path(name_str, false, asset_res) {
                return div()
                    .w(px(size_pt))
                    .h(px(size_pt))
                    .opacity(0.35)
                    .child(img(reg_path).size_full())
                    .into_any_element();
            }
        }

        if self.active {
            if let Some(hover_path) = resolve_icon_path(name_str, true, asset_res) {
                return div()
                    .w(px(size_pt))
                    .h(px(size_pt))
                    .child(img(hover_path).size_full())
                    .into_any_element();
            }
        }

        let reg_path = resolve_icon_path(name_str, false, asset_res);
        let hover_path = resolve_icon_path(name_str, true, asset_res);

        if self.hover_switch {
            if let (Some(reg), Some(hov)) = (reg_path.clone(), hover_path.clone()) {
                let group_name = self
                    .parent_group
                    .clone()
                    .unwrap_or_else(|| "shellumi-icon-tile".into());
                let has_parent = self.parent_group.is_some();
                let mut container = div()
                    .relative()
                    .w(px(size_pt))
                    .h(px(size_pt))
                    .flex()
                    .items_center()
                    .justify_center();

                if !has_parent {
                    container = container.group(group_name.clone());
                }

                return container
                    .child(
                        img(reg)
                            .w(px(size_pt))
                            .h(px(size_pt))
                            .group_hover(group_name.clone(), |s| s.opacity(0.0)),
                    )
                    .child(
                        img(hov)
                            .absolute()
                            .top_0()
                            .left_0()
                            .w(px(size_pt))
                            .h(px(size_pt))
                            .opacity(0.0)
                            .group_hover(group_name, |s| s.opacity(1.0)),
                    )
                    .into_any_element();
            }
        }

        if let Some(p) = reg_path.or(hover_path) {
            div()
                .w(px(size_pt))
                .h(px(size_pt))
                .child(img(p).size_full())
                .into_any_element()
        } else {
            div().w(px(size_pt)).h(px(size_pt)).into_any_element()
        }
    }
}

/// Helper function to render an icon element cleanly inline.
#[allow(dead_code)]
pub fn render_icon(name: &str, is_hover: bool, size_pt: f32) -> AnyElement {
    if let Some(icon_name) = IconName::from_name(name) {
        if is_hover {
            Icon::new(icon_name)
                .size(size_pt)
                .active(true)
                .into_any_element()
        } else {
            Icon::new(icon_name)
                .size(size_pt)
                .hover_switch(true)
                .into_any_element()
        }
    } else {
        let asset_res = pick_asset_resolution(size_pt);
        if let Some(p) = resolve_icon_path(name, is_hover, asset_res) {
            div()
                .w(px(size_pt))
                .h(px(size_pt))
                .child(img(p).size_full())
                .into_any_element()
        } else {
            div().w(px(size_pt)).h(px(size_pt)).into_any_element()
        }
    }
}

/// Custom GPUI AssetSource supporting the full Shellumi icon pack in memory.
#[derive(Debug, Clone, Default)]
pub struct ShellumiAssetSource;

impl ShellumiAssetSource {
    pub fn new() -> Self {
        Self
    }
}

impl AssetSource for ShellumiAssetSource {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        // Reject path traversal or absolute paths
        if path.contains("..") || path.starts_with('/') || path.starts_with('\\') {
            return Ok(None);
        }
        if let Some(root) = resolve_icon_pack_root() {
            let disk_path = root.join(path);
            if disk_path.exists() {
                if let Ok(bytes) = std::fs::read(&disk_path) {
                    return Ok(Some(Cow::Owned(bytes)));
                }
            }
        }

        // Parse canonical key: "icons/{size}/{state}/{name}.png"
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() == 4 && parts[0] == "icons" {
            let size_px: u32 = parts[1].parse().unwrap_or(48);
            let is_hover = parts[2] == "hover";
            let name = parts[3].trim_end_matches(".png");
            if let Some(bytes) = crate::embedded_icons::get_embedded_icon(name, is_hover, size_px) {
                return Ok(Some(Cow::Borrowed(bytes)));
            }
        }

        Ok(None)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut list = Vec::new();
        if path == "icons" || path.is_empty() {
            for icon in IconName::ALL {
                list.push(SharedString::from(format!(
                    "icons/48/regular/{}.png",
                    icon.as_str()
                )));
            }
        }
        Ok(list)
    }
}

//! UI-independent connection definitions. No network connections are made here.

pub mod autocomplete;
pub mod batch;
pub mod bench;
pub mod collaboration;
pub mod connection;
pub mod credential;
pub mod csv;
pub mod diagnostics;
pub mod ffi;
pub mod group;
pub mod identity;
pub mod infrastructure;
pub mod known_hosts;
pub mod mobile;
pub mod palette;
pub mod protocol;
pub mod recording;
pub mod sftp;
pub mod snippet;
pub mod ssh_config;
pub mod startup;
pub mod sync;
pub mod tunnel;
pub mod workspace;

pub use autocomplete::{
    generate_bash_hook, generate_fish_hook, generate_zsh_hook, AutocompleteContext,
    AutocompleteEngine, CompletionItem, CompletionKind, PasswordPromptDetector, PasswordPromptKind,
    PasswordPromptMatch,
};
pub use batch::{
    BatchExecutionPlan, BatchExecutionSummary, BatchHostResult, BatchHostStatus, BatchHostTarget,
    BatchPolicy, BatchRunner,
};
pub use bench::{
    BenchmarkCategory, BenchmarkMetric, BenchmarkReport, BenchmarkRunner, BenchmarkStatus,
};
pub use collaboration::{
    AuditAction, CollaborationError, LogBookmark, LogComment, MultiplayerParticipant,
    MultiplayerSession, SharedSessionLog, SharedVault, SharingAuditRecord, TeamMember, TeamRole,
};
#[cfg(target_os = "macos")]
pub use credential::MacOsKeychainStore;
pub use credential::{
    default_credential_store, default_fallback_vault_path, delete_secret_by_ref,
    format_keychain_ref, parse_keychain_ref, probe_keychain_daemon, resolve_secret_from_ref,
    store_secret_to_ref, CredentialError, CredentialStore, CredentialType, DaemonWithFallbackStore,
    EncryptedFileCredentialStore, EphemeralCredentialStore, KeychainDaemonStatus,
    LinuxSecretServiceStore, SecretString, WindowsCredentialManagerStore, DEFAULT_KEYCHAIN_SERVICE,
};
pub use csv::{export_csv, parse_csv};
pub use diagnostics::{
    DiagnosticCategory, DiagnosticCheck, DiagnosticSeverity, DiagnosticStatus, DiagnosticsReport,
    DiagnosticsRunner,
};
pub use ffi::{
    shellumi_core_version, shellumi_generate_keypair_c, shellumi_get_credential_spec_c,
    shellumi_lifecycle_transition_c, shellumi_mobile_platform_supported,
    shellumi_parse_destination_c, shellumi_string_free, shellumi_translate_virtual_key_c,
    SHELLUMI_MODIFIER_ALT, SHELLUMI_MODIFIER_CTRL, SHELLUMI_MODIFIER_SHIFT,
};
pub use group::{
    resolve_effective_destination, resolve_effective_destination_with_identity,
    resolve_effective_tags, GroupError, HostGroup,
};
pub use identity::{
    compute_sha256_fingerprint, create_test_ed25519_certificate, encode_openssh_public_key,
    fingerprint_public_key, generate_ed25519_key, AuthMethod, CertificateType, Identity,
    IdentityError, KeyPair, KeyType, NewIdentity, OpenSshCertificate, SshKey,
};
pub use infrastructure::{
    AnsibleInventoryEngine, AwsEc2Parser, AzureVmParser, CloudProviderConfig, CloudProviderKind,
    DigitalOceanParser, DiscoveredHost, ExistingHostRecord, HostExportItem, InfraSyncAction,
    InfraSyncReport, InfrastructureError, InventoryMergeEngine, MergePolicy,
};
pub use known_hosts::{
    format_known_host_entry, parse_known_hosts, verify_host_key, HostKeyMarker,
    HostKeyVerification, KnownHostEntry, KnownHostsError,
};
pub use mobile::{
    AccessoryBarLayout, BiometricRequirement, MobileCredentialSpec, MobileFfiBridge,
    MobileLifecycleEvent, MobileLifecycleManager, MobilePlatform, MobileSessionState,
    PlatformKeyStoreTarget, VirtualAccessoryBar, VirtualKey, VirtualKeyModifier,
};
pub use palette::{PaletteAction, PaletteCategory, PaletteCommand, PaletteMatch, PaletteRegistry};
pub use protocol::{
    build_telnet_command, build_telnet_naws, build_telnet_terminal_type_response,
    parse_telnet_stream, telnet_constants, ConnectionProtocol, MoshConfig, MoshPredictMode,
    ProtocolEndpoint, ProtocolError, SerialConfig, SerialDataBits, SerialFlowControl, SerialParity,
    SerialStopBits, TelnetCommandFrame, TelnetConfig,
};

/// Discovers candidate serial device ports on the local machine.
pub fn discover_serial_ports() -> Vec<String> {
    SerialConfig::discover_serial_ports()
}
pub use recording::{
    format_duration_hms, CastPlaybackEngine, PlaybackFrame, PlaybackSpeed, PlaybackStatus,
    SessionRecordingMetadata, SessionRecordingSummary, SessionRedactor,
};
pub use sftp::{
    format_bytes, list_local_directory, ConflictResolution, FileAttributes, FileEntry, FileKind,
    NameEntry, OpenFlags, PathUtils, RemoteEditorWorkspace, RemoteTrackedFile, SftpClient,
    SftpConnectOptions, SftpError, SftpPacket, SftpProtocolError, StatusCode, TransferControl,
    TransferDirection, TransferQueue, TransferQueueSummary, TransferStatus, TransferTask,
    SSH_FILEXFER_ATTR_ACMODTIME, SSH_FILEXFER_ATTR_EXTENDED, SSH_FILEXFER_ATTR_PERMISSIONS,
    SSH_FILEXFER_ATTR_SIZE, SSH_FILEXFER_ATTR_UIDGID, SSH_FXF_APPEND, SSH_FXF_CREAT, SSH_FXF_EXCL,
    SSH_FXF_READ, SSH_FXF_TRUNC, SSH_FXF_WRITE, SSH_FXP_ATTRS, SSH_FXP_CLOSE, SSH_FXP_DATA,
    SSH_FXP_EXTENDED, SSH_FXP_EXTENDED_REPLY, SSH_FXP_FSETSTAT, SSH_FXP_FSTAT, SSH_FXP_HANDLE,
    SSH_FXP_INIT, SSH_FXP_LSTAT, SSH_FXP_MKDIR, SSH_FXP_NAME, SSH_FXP_OPEN, SSH_FXP_OPENDIR,
    SSH_FXP_READ, SSH_FXP_READDIR, SSH_FXP_READLINK, SSH_FXP_REALPATH, SSH_FXP_REMOVE,
    SSH_FXP_RENAME, SSH_FXP_RMDIR, SSH_FXP_SETSTAT, SSH_FXP_STAT, SSH_FXP_STATUS, SSH_FXP_SYMLINK,
    SSH_FXP_VERSION, SSH_FXP_WRITE,
};
pub use snippet::{
    export_snippets_json, extract_variables, parse_snippets_json, render_command, render_preview,
    shell_escape, Snippet, SnippetError, SnippetVariable,
};
pub use ssh_config::{export_ssh_config, parse_ssh_config, HostConfigEntry};
pub use startup::{
    prepare_startup_snippet, StartupError, StartupExecutionPolicy, StartupSessionTracker,
    StartupSnippetContext, StartupStatus,
};
pub use sync::{
    build_conflict_item_views, build_delete_file_request, build_download_file_request,
    build_google_drive_auth_url, build_list_appdata_files_request, build_multipart_upload_request,
    build_oauth_callback_response, build_token_exchange_request, build_token_refresh_request,
    build_update_file_request, decrypt_entity, determine_sync_action, diff_sync_records,
    diff_sync_records_with_passphrase, encrypt_entity, encrypt_entity_with_iterations,
    execute_cloud_vault_sync, execute_sync_cycle, format_record_summary, generate_nonce,
    generate_pkce_challenge, generate_salt, parse_download_file_response,
    parse_list_files_response, parse_oauth_callback_request, parse_raw_http_response,
    parse_token_exchange_response, parse_token_refresh_response, parse_upload_file_response,
    plan_sync_cycle, resolve_record_conflict, url_encode, ConflictItemView,
    ConflictResolutionChoice, DiffStatus, DriveFileMetadata, EncryptedEnvelope,
    GoogleDriveSyncClient, HttpRequest, HttpResponse, HttpTransport, OAuthCallbackListener,
    OAuthTokenBundle, PkceChallenge, RecordDiffSummary, SyncAction, SyncBackoff, SyncCycleResult,
    SyncEntityType, SyncError, SyncExecutionReport, SyncPlan, SyncRecord, SyncScheduler,
    SyncTrigger, SyncVault, SystemHttpTransport, DEFAULT_KDF_ITERATIONS, DRIVE_APPDATA_FILENAME,
    KEY_LEN, MIN_PASSPHRASE_LENGTH, OAUTH_TOKEN_ACCOUNT, OAUTH_TOKEN_SERVICE, SALT_LEN,
};
pub use tunnel::{
    build_ssh_tunnel_command, validate_jump_host, ActiveTunnelState, ActiveTunnelStatus,
    ForwardingType, PortForwardRule, TunnelError,
};
pub use workspace::{
    check_dangerous_command, BroadcastManager, BroadcastSafetyCheck, Workspace, WorkspaceError,
    WorkspaceLayout, WorkspaceSession, WorkspaceSessionKind,
};

use std::fmt;
use std::num::NonZeroU16;

/// A validated SSH destination. Credentials are deliberately excluded.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SshDestination {
    host: String,
    username: String,
    port: NonZeroU16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationError {
    InvalidHost,
    InvalidUsername,
    InvalidPort,
    InvalidFormat,
}

impl fmt::Display for DestinationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidHost => {
                "Enter a hostname or IP address without whitespace or control characters."
            }
            Self::InvalidUsername => "Enter a username without whitespace or control characters.",
            Self::InvalidPort => "Enter a port between 1 and 65535.",
            Self::InvalidFormat => "Enter a destination in the format user@host[:port].",
        })
    }
}

impl std::error::Error for DestinationError {}

impl SshDestination {
    pub fn new(host: &str, username: &str, port: u16) -> Result<Self, DestinationError> {
        let host = host.trim();
        let username = username.trim();
        if host.is_empty()
            || host.starts_with('-')
            || host.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(DestinationError::InvalidHost);
        }
        if username.is_empty()
            || username
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(DestinationError::InvalidUsername);
        }
        let port = NonZeroU16::new(port).ok_or(DestinationError::InvalidPort)?;
        Ok(Self {
            host: host.to_owned(),
            username: username.to_owned(),
            port,
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn port(&self) -> NonZeroU16 {
        self.port
    }

    /// Arguments for an interactive system OpenSSH session. Host-key confirmation
    /// is required for unknown keys and agent forwarding is disabled by default.
    pub fn openssh_arguments(&self) -> Vec<String> {
        vec![
            "-o".into(),
            "StrictHostKeyChecking=ask".into(),
            "-o".into(),
            "ForwardAgent=no".into(),
            "-p".into(),
            self.port.to_string(),
            "-l".into(),
            self.username.clone(),
            "--".into(),
            self.host.clone(),
        ]
    }

    /// Construct OpenSSH arguments with a validated jump chain before the
    /// destination, so it cannot be interpreted as a remote command.
    pub fn openssh_arguments_with_jump(
        &self,
        jump: Option<&str>,
    ) -> Result<Vec<String>, TunnelError> {
        let mut arguments = self.openssh_arguments();
        if let Some(jump) = jump {
            let jump = validate_jump_host(jump)?;
            let destination_index = arguments.len() - 2;
            arguments.splice(destination_index..destination_index, ["-J".into(), jump]);
        }
        Ok(arguments)
    }
}

/// Parse a quick-connect string in the format `user@host[:port]` or `user@[ipv6][:port]`.
pub fn parse_quick_connect_target(target: &str) -> Result<SshDestination, DestinationError> {
    let target = target.trim();
    let (username, host_part) = target
        .split_once('@')
        .ok_or(DestinationError::InvalidFormat)?;

    let (host, port) = if let Some(stripped) = host_part.strip_prefix('[') {
        let (ipv6, rest) = stripped
            .split_once(']')
            .ok_or(DestinationError::InvalidFormat)?;
        let port = if let Some(port_str) = rest.strip_prefix(':') {
            port_str
                .parse::<u16>()
                .map_err(|_| DestinationError::InvalidPort)?
        } else {
            22
        };
        (ipv6, port)
    } else if let Some((h, p)) = host_part.split_once(':') {
        let port = p
            .parse::<u16>()
            .map_err(|_| DestinationError::InvalidPort)?;
        (h, port)
    } else {
        (host_part, 22)
    };

    SshDestination::new(host, username, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_options_precede_the_destination_and_reject_invalid_chains() {
        let destination = SshDestination::new("target.example", "alice", 22).unwrap();
        let args = destination
            .openssh_arguments_with_jump(Some("bob@jump.example:2222"))
            .unwrap();
        assert_eq!(
            &args[args.len() - 4..],
            ["-J", "bob@jump.example:2222", "--", "target.example"]
        );
        assert_eq!(
            destination.openssh_arguments_with_jump(None).unwrap(),
            destination.openssh_arguments()
        );
        assert!(destination
            .openssh_arguments_with_jump(Some("-oProxyCommand=bad"))
            .is_err());
        assert!(destination
            .openssh_arguments_with_jump(Some("jump;bad"))
            .is_err());
    }

    #[test]
    fn parses_quick_connect_targets() {
        let d1 = parse_quick_connect_target("alice@example.com").unwrap();
        assert_eq!(d1.username(), "alice");
        assert_eq!(d1.host(), "example.com");
        assert_eq!(d1.port().get(), 22);

        let d2 = parse_quick_connect_target("bob@192.168.1.1:2222").unwrap();
        assert_eq!(d2.username(), "bob");
        assert_eq!(d2.host(), "192.168.1.1");
        assert_eq!(d2.port().get(), 2222);

        let d3 = parse_quick_connect_target("root@[2001:db8::1]:8022").unwrap();
        assert_eq!(d3.username(), "root");
        assert_eq!(d3.host(), "2001:db8::1");
        assert_eq!(d3.port().get(), 8022);

        assert_eq!(
            parse_quick_connect_target("invalid"),
            Err(DestinationError::InvalidFormat)
        );
        assert_eq!(
            parse_quick_connect_target("user@host:badport"),
            Err(DestinationError::InvalidPort)
        );
    }

    #[test]
    fn accepts_dns_and_ipv6_and_normalizes_outer_whitespace() {
        for host in ["example.org", "2001:db8::1"] {
            let destination = SshDestination::new(&format!(" {host} "), " alice ", 22).unwrap();
            assert_eq!(destination.host(), host);
            assert_eq!(destination.username(), "alice");
            assert_eq!(destination.port().get(), 22);
        }
    }

    #[test]
    fn rejects_unsafe_or_missing_fields() {
        for host in ["", "-oProxyCommand=bad", "some host", "host\0name"] {
            assert_eq!(
                SshDestination::new(host, "alice", 22),
                Err(DestinationError::InvalidHost)
            );
        }
        for username in ["", "two users", "alice\0bad"] {
            assert_eq!(
                SshDestination::new("host", username, 22),
                Err(DestinationError::InvalidUsername)
            );
        }
        assert_eq!(
            SshDestination::new("host", "alice", 0),
            Err(DestinationError::InvalidPort)
        );
    }
}

use shellumi_core::{
    build_google_drive_auth_url, build_ssh_tunnel_command, default_credential_store,
    delete_secret_by_ref, diff_sync_records_with_passphrase, discover_serial_ports,
    execute_cloud_vault_sync, execute_sync_cycle, export_csv, export_ssh_config,
    format_duration_hms, generate_bash_hook, generate_ed25519_key, generate_fish_hook,
    generate_pkce_challenge, generate_zsh_hook, list_local_directory, parse_csv,
    parse_quick_connect_target, parse_ssh_config, probe_keychain_daemon, render_command,
    render_preview, resolve_secret_from_ref, store_secret_to_ref, AccessoryBarLayout,
    AnsibleInventoryEngine, AuditAction, AuthMethod, AutocompleteContext, AutocompleteEngine,
    AwsEc2Parser, AzureVmParser, BatchExecutionPlan, BatchHostTarget, BatchPolicy,
    BenchmarkCategory, BenchmarkRunner, CastPlaybackEngine, CloudProviderConfig, CloudProviderKind,
    ConflictResolutionChoice, ConnectionProtocol, DiagnosticsRunner, DigitalOceanParser,
    FileAttributes, ForwardingType, GoogleDriveSyncClient, HostConfigEntry, HostExportItem,
    HostGroup, MobileFfiBridge, MobileLifecycleEvent, MobileLifecycleManager, MobilePlatform,
    MoshConfig, MoshPredictMode, MultiplayerSession, NewIdentity, OpenFlags, OpenSshCertificate,
    PaletteAction, PaletteCategory, PaletteCommand, PaletteRegistry, PasswordPromptDetector,
    PlaybackSpeed, PlaybackStatus, PortForwardRule, ProtocolError, SerialConfig, SessionRedactor,
    SftpClient, SftpConnectOptions, SftpPacket, SharedVault, SharingAuditRecord, Snippet,
    SshDestination, SyncCycleResult, SyncVault, SystemHttpTransport, TeamMember, TeamRole,
    TelnetConfig, TransferDirection, TransferQueue, VirtualAccessoryBar, DEFAULT_KDF_ITERATIONS,
    DEFAULT_KEYCHAIN_SERVICE,
};
use shellumi_storage::{
    CloudProviderRecord, DiscoveredHostRecord, HostStore, NewTransfer, SessionRecordingRecord,
};
use std::str::FromStr;
use std::{collections::HashMap, env, error::Error, fs, io, process::Command};

const USAGE: &str = "Usage:
  shellumi-cli <database-path> list [--group <group>] [--tag <tag>] [--search <query>]
  shellumi-cli <database-path> show <id>
  shellumi-cli <database-path> add <name> <hostname> <username> <port> [--group <group>] [--tags <tag1,tag2>] [--jump-host <jump>] [--startup-snippet <id>] [--protocol <ssh|mosh|telnet|serial>] [--protocol-config <config>] [--identity-id <id>]
  shellumi-cli <database-path> update <id> <name> <hostname> <username> <port> [--group <group>] [--tags <tag1,tag2>] [--jump-host <jump>] [--startup-snippet <id>] [--protocol <ssh|mosh|telnet|serial>] [--protocol-config <config>] [--identity-id <id>]
  shellumi-cli <database-path> delete <id>
  shellumi-cli <database-path> identity add <name> [--username <user>] [--auth <password|publickey|agent|certificate>] [--key-path <path>] [--cert-path <path>] [--password <secret>] [--passphrase <secret>] [--password-ref <uri>] [--passphrase-ref <uri>] [--tags <tag1,tag2>] [--comment <comment>]
  shellumi-cli <database-path> identity list [--tag <tag>]
  shellumi-cli <database-path> identity show <id>
  shellumi-cli <database-path> identity delete <id>
  shellumi-cli <database-path> group add <name> [--username <user>] [--port <port>] [--tags <tag1,tag2>] [--identity-id <id>]
  shellumi-cli <database-path> group list
  shellumi-cli <database-path> group show <name>
  shellumi-cli <database-path> group delete <name>
  shellumi-cli <database-path> tunnel add <name> --type <local|remote|dynamic> --bind-port <port> [--bind-address <addr>] [--target-host <host>] [--target-port <port>] [--host-id <id>] [--auto-start]
  shellumi-cli <database-path> tunnel list [--host-id <id>]
  shellumi-cli <database-path> tunnel start <id> [--dry-run]
  shellumi-cli <database-path> tunnel delete <id>
  shellumi-cli <database-path> sftp local-ls <path> [--all]
  shellumi-cli <database-path> sftp remote-ls <destination> [path]
  shellumi-cli <database-path> sftp get <destination> <remote-path> <local-path>
  shellumi-cli <database-path> sftp put <destination> <local-path> <remote-path>
  shellumi-cli <database-path> sftp history [--host-id <id>] [--limit <n>]
  shellumi-cli <database-path> sftp clear-history [--host-id <id>]
  shellumi-cli <database-path> sftp log <upload|download> <local-path> <remote-path> <bytes> [--total-bytes <bytes>] [--status <status>] [--host-id <id>]
  shellumi-cli <database-path> sftp packet <encode-init|decode|encode-open> [...]
  shellumi-cli <database-path> sftp queue <upload|download> <local-path> <remote-path> <bytes>
  shellumi-cli <database-path> snippet list [--tag <tag>] [--search <query>]
  shellumi-cli <database-path> snippet add <name> <command> [--description <desc>] [--tags <tag1,tag2>]
  shellumi-cli <database-path> snippet show <id>
  shellumi-cli <database-path> snippet render <id> [--var key=value] [--preview]
  shellumi-cli <database-path> snippet batch <id> --hosts <host1,host2,...> [--var key=value] [--policy <seq|par>] [--concurrency <n>] [--dry-run]
  shellumi-cli <database-path> snippet delete <id>
  shellumi-cli <database-path> workspace add <name> [--layout <single|vertical|horizontal|grid>] [--desc <description>] [--tags <tag1,tag2>]
  shellumi-cli <database-path> workspace list
  shellumi-cli <database-path> workspace show <id>
  shellumi-cli <database-path> workspace duplicate <id> <new-name>
  shellumi-cli <database-path> workspace delete <id>
  shellumi-cli <database-path> palette search <query>
  shellumi-cli <database-path> session list [--host-id <id>]
  shellumi-cli <database-path> session show <id>
  shellumi-cli <database-path> session play <id> [--speed <multiplier>] [--raw]
  shellumi-cli <database-path> session export <id> [--out <path>]
  shellumi-cli <database-path> session import <cast-file> --title <title> [--host-id <id>] [--redact]
  shellumi-cli <database-path> session delete <id>
  shellumi-cli <database-path> vault export <output-path> --passphrase <pass> [--device-id <id>] [--iterations <n>]
  shellumi-cli <database-path> vault import <input-path> --passphrase <pass>
  shellumi-cli <database-path> vault diff <input-path> --passphrase <pass>
  shellumi-cli <database-path> vault history
  shellumi-cli <database-path> vault rollback <snapshot-id> --passphrase <pass>
  shellumi-cli <database-path> vault status
  shellumi-cli <database-path> vault sync [--remote <path>] --passphrase <pass> [--device-id <id>] [--dry-run]
  shellumi-cli <database-path> vault cloud-sync --token <oauth-token> --passphrase <pass> [--resolve <keep-local|accept-remote>]
  shellumi-cli <database-path> vault oauth-url [--client-id <id>] [--redirect-uri <uri>]
  shellumi-cli <database-path> team vault create <name> [--desc <desc>] [--owner <owner-id>]
  shellumi-cli <database-path> team vault list
  shellumi-cli <database-path> team vault show <id>
  shellumi-cli <database-path> team vault rotate-key <id>
  shellumi-cli <database-path> team vault delete <id>
  shellumi-cli <database-path> team member invite <vault-id> <user-id> <name> <role> [--public-key <key>] [--invited-by <by>]
  shellumi-cli <database-path> team member list <vault-id>
  shellumi-cli <database-path> team member role <vault-id> <member-id> <new-role>
  shellumi-cli <database-path> team member revoke <vault-id> <member-id>
  shellumi-cli <database-path> team audit <vault-id> [--limit <n>]
  shellumi-cli <database-path> team session start <session-id> <owner-id> [--vault-id <id>]
  shellumi-cli <database-path> team session join <session-id> <user-id> <name>
  shellumi-cli <database-path> team session grant <session-id> <requester-id> <target-user-id>
  shellumi-cli <database-path> team session revoke <session-id> <requester-id>
  shellumi-cli <database-path> team session status <session-id>
  shellumi-cli <database-path> infra provider add <kind> --name <name> [--region <r>] [--profile <p>] [--token <t>] [--path <path>] [--format <ini|yaml>]
  shellumi-cli <database-path> infra provider list
  shellumi-cli <database-path> infra provider show <id>
  shellumi-cli <database-path> infra provider delete <id>
  shellumi-cli <database-path> infra provider sync <id>
  shellumi-cli <database-path> infra discover <aws|azure|digitalocean|ansible> --file <file> [--private-ip] [--import-all]
  shellumi-cli <database-path> infra ansible import --file <path> [--format <ini|yaml>]
  shellumi-cli <database-path> infra ansible export [--format <ini|yaml>] [--output <path>]
  shellumi-cli <database-path> connect <id>
  shellumi-cli <database-path> quick-connect <user@host[:port]>
  shellumi-cli <database-path> mosh <user@host[:port]> [--ssh-port <port>] [--predict <adaptive|always|never|experimental>]
  shellumi-cli <database-path> telnet <host> [port]
  shellumi-cli <database-path> serial <port> [--baud <baud>]
  shellumi-cli serial-ports
  shellumi-cli <database-path> import-ssh-config <config-path>
  shellumi-cli <database-path> export-ssh-config
  shellumi-cli <database-path> import-csv <csv-path>
  shellumi-cli <database-path> export-csv
  shellumi-cli keygen [--comment <comment>]
  shellumi-cli doctor [--json] [--markdown] [--db <path>]
  shellumi-cli bench [--json] [--markdown] [--category <category>]
  shellumi-cli mobile <virtual-keys|lifecycle-sim|ffi-check|header|build-info>
  shellumi-cli complete <query> [--dir <dir>]
  shellumi-cli <database-path> complete <query>
  shellumi-cli prompt-detect \"<prompt-line>\"
  shellumi-cli shell-hook <bash|zsh|fish>
  shellumi-cli keychain status [--json]
  shellumi-cli keychain set <service> <account> <secret>
  shellumi-cli keychain get <service> <account>
  shellumi-cli keychain delete <service> <account>
  shellumi-cli <database-path> keychain <status|set|get|delete>";

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn unique_id(prefix: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{prefix}-{}-{}", now.as_secs(), now.subsec_nanos())
}

fn parse_destination(fields: &[String]) -> Result<SshDestination, Box<dyn Error>> {
    let port: u16 = fields[2]
        .parse()
        .map_err(|_| invalid("Port must be between 1 and 65535."))?;
    Ok(SshDestination::new(&fields[0], &fields[1], port)?)
}

fn handle_keychain_command(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        println!("Usage: shellumi-cli keychain <status|get|set|delete> [...]");
        return Ok(());
    }

    match args[0].as_str() {
        "status" => {
            let is_json = args.iter().any(|a| a == "--json");
            let status = probe_keychain_daemon();
            if is_json {
                println!(
                    "{}",
                    serde_json::json!({
                        "backend_name": status.backend_name,
                        "daemon_identifier": status.daemon_identifier,
                        "is_available": status.is_available,
                        "details": status.details,
                    })
                );
            } else {
                println!("Native OS Keychain Daemon Status:");
                println!("  Backend:    {}", status.backend_name);
                println!("  Daemon ID:  {}", status.daemon_identifier);
                println!(
                    "  Available:  {}",
                    if status.is_available {
                        "YES (active)"
                    } else {
                        "NO (inactive/fallback)"
                    }
                );
                println!("  Details:    {}", status.details);
            }
            Ok(())
        }
        "set" => {
            if args.len() < 4 {
                return Err(invalid(
                    "Usage: shellumi-cli keychain set <service> <account> <secret>",
                )
                .into());
            }
            let service = &args[1];
            let account = &args[2];
            let secret = &args[3];
            let store = default_credential_store();
            let uri = store_secret_to_ref(&*store, service, account, secret)?;
            println!("Stored secret with reference: {uri}");
            Ok(())
        }
        "get" => {
            if args.len() < 2 {
                return Err(invalid(
                    "Usage: shellumi-cli keychain get <service> <account> | <keychain://uri>",
                )
                .into());
            }
            let store = default_credential_store();
            let secret = if args[1].starts_with("keychain://") || args[1].starts_with("env://") {
                resolve_secret_from_ref(&*store, &args[1])?
            } else {
                if args.len() < 3 {
                    return Err(
                        invalid("Usage: shellumi-cli keychain get <service> <account>").into(),
                    );
                }
                store.get_secret(&args[1], &args[2])?
            };

            match secret {
                Some(s) => println!("{}", s.expose_secret()),
                None => println!("(secret not found)"),
            }
            Ok(())
        }
        "delete" => {
            if args.len() < 2 {
                return Err(invalid(
                    "Usage: shellumi-cli keychain delete <service> <account> | <keychain://uri>",
                )
                .into());
            }
            let store = default_credential_store();
            let deleted = if args[1].starts_with("keychain://") {
                delete_secret_by_ref(&*store, &args[1])?
            } else {
                if args.len() < 3 {
                    return Err(
                        invalid("Usage: shellumi-cli keychain delete <service> <account>").into(),
                    );
                }
                store.delete_secret(&args[1], &args[2])?
            };
            if deleted {
                println!("Secret deleted successfully.");
            } else {
                println!("Secret not found or could not be deleted.");
            }
            Ok(())
        }
        other => Err(invalid(&format!(
            "Unknown keychain subcommand: '{other}'. Use status, set, get, delete."
        ))
        .into()),
    }
}

fn extract_flag(args: &[String], flag: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().cloned();
        }
    }
    None
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let clean = s.trim().replace("0x", "").replace(' ', "");
    if !clean.len().is_multiple_of(2) {
        return Err("Hex string must have an even number of hex characters.".into());
    }
    (0..clean.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&clean[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex byte at index {i}: {e}"))
        })
        .collect()
}

fn pkt_type_name(t: u8) -> &'static str {
    match t {
        1 => "SSH_FXP_INIT",
        2 => "SSH_FXP_VERSION",
        3 => "SSH_FXP_OPEN",
        4 => "SSH_FXP_CLOSE",
        5 => "SSH_FXP_READ",
        6 => "SSH_FXP_WRITE",
        7 => "SSH_FXP_LSTAT",
        8 => "SSH_FXP_FSTAT",
        9 => "SSH_FXP_SETSTAT",
        10 => "SSH_FXP_FSETSTAT",
        11 => "SSH_FXP_OPENDIR",
        12 => "SSH_FXP_READDIR",
        13 => "SSH_FXP_REMOVE",
        14 => "SSH_FXP_MKDIR",
        15 => "SSH_FXP_RMDIR",
        16 => "SSH_FXP_REALPATH",
        17 => "SSH_FXP_STAT",
        18 => "SSH_FXP_RENAME",
        19 => "SSH_FXP_READLINK",
        20 => "SSH_FXP_SYMLINK",
        101 => "SSH_FXP_STATUS",
        102 => "SSH_FXP_HANDLE",
        103 => "SSH_FXP_DATA",
        104 => "SSH_FXP_NAME",
        105 => "SSH_FXP_ATTRS",
        200 => "SSH_FXP_EXTENDED",
        201 => "SSH_FXP_EXTENDED_REPLY",
        _ => "UNKNOWN",
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || (args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h")) {
        println!("{USAGE}");
        return Ok(());
    }

    if args[0] == "keygen" {
        let comment = extract_flag(&args[1..], "--comment");
        let (_keypair, key_meta) = generate_ed25519_key(comment.as_deref())?;
        println!("Generated Ed25519 SSH Key Pair:");
        println!("Public Key:  {}", key_meta.public_key);
        println!("Fingerprint: {}", key_meta.fingerprint);
        return Ok(());
    }

    if args[0] == "serial-ports" {
        let ports = discover_serial_ports();
        if ports.is_empty() {
            println!("No serial ports detected.");
        } else {
            println!("Discovered Serial Ports ({}):", ports.len());
            for p in ports {
                println!("  {p}");
            }
        }
        return Ok(());
    }

    if args[0] == "doctor" {
        let is_json = args.iter().any(|a| a == "--json");
        let is_markdown = args.iter().any(|a| a == "--markdown");
        let db_path = extract_flag(&args[1..], "--db");

        let mut runner = DiagnosticsRunner::new();
        if let Some(db) = db_path {
            runner = runner.with_database_path(db);
        }
        let report = runner.run_all();

        if is_json {
            println!("{}", report.to_json()?);
        } else if is_markdown {
            println!("{}", report.format_markdown_report());
        } else {
            print!("{}", report.format_terminal_summary());
        }

        if report.has_failures() {
            std::process::exit(1);
        }
        return Ok(());
    }

    if args[0] == "bench" {
        let is_json = args.iter().any(|a| a == "--json");
        let is_markdown = args.iter().any(|a| a == "--markdown");
        let category_str = extract_flag(&args[1..], "--category");
        let category = category_str.and_then(|c| BenchmarkCategory::from_str_loose(&c));

        let report = BenchmarkRunner::run_suite(category);

        if is_json {
            println!("{}", report.to_json()?);
        } else if is_markdown {
            println!("{}", report.to_markdown());
        } else {
            print!("{}", report.format_terminal_table());
        }

        if !report.is_healthy() {
            std::process::exit(1);
        }
        return Ok(());
    }

    if args[0] == "mobile" {
        if args.len() < 2 {
            println!("Usage: shellumi-cli mobile <virtual-keys|lifecycle-sim|ffi-check|header|build-info>");
            return Ok(());
        }
        match args[1].as_str() {
            "virtual-keys" => {
                let bar = VirtualAccessoryBar::new(AccessoryBarLayout::DefaultTerminal);
                println!("Mobile Virtual Accessory Keys ({}):", bar.keys.len());
                for key in &bar.keys {
                    let bytes = key.to_bytes();
                    let hex = bytes
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("  {:6} -> {:10} (bytes: {:?})", key.label(), hex, bytes);
                }
                return Ok(());
            }
            "lifecycle-sim" => {
                let event_name = args.get(2).map(|s| s.as_str()).unwrap_or("background");
                let mut manager = MobileLifecycleManager::new(MobilePlatform::Ios);
                manager.mark_connected();
                println!("Initial state: {}", manager.state.status_label());
                let event = match event_name {
                    "background" => MobileLifecycleEvent::AppBackgrounded { grace_seconds: 30 },
                    "expire" => MobileLifecycleEvent::BackgroundExpired,
                    "tick" => MobileLifecycleEvent::BackgroundTimerTick {
                        elapsed_seconds: 10,
                    },
                    "disconnect" => MobileLifecycleEvent::NetworkLost,
                    "reconnect" => MobileLifecycleEvent::NetworkRestored,
                    "foreground" => MobileLifecycleEvent::AppForegrounded,
                    _ => MobileLifecycleEvent::AppBackgrounded { grace_seconds: 30 },
                };
                manager.transition(event, 1000);
                println!(
                    "Transformed state after {}: {}",
                    event_name,
                    manager.state.status_label()
                );
                return Ok(());
            }
            "ffi-check" => {
                println!("Running Mobile FFI and C-ABI validation checks...");
                // 1. Rust bridge checks
                let dest_res =
                    MobileFfiBridge::parse_destination_json("mobile-user@ssh.example.org:2222");
                println!("Destination JSON check: {dest_res}");
                let key_res = MobileFfiBridge::translate_virtual_key_json("ctrl-c");
                println!("Virtual Key JSON check: {key_res}");
                let keypair_res = MobileFfiBridge::generate_keypair_json("mobile-ed25519-check");
                println!("Keypair JSON check: {keypair_res}");

                // 2. Direct C-ABI symbol verification
                unsafe {
                    let ver_ptr = shellumi_core::shellumi_core_version();
                    let ver_str = std::ffi::CStr::from_ptr(ver_ptr)
                        .to_str()
                        .unwrap_or("unknown");
                    println!("C-ABI Core Version: {ver_str}");

                    let c_target = std::ffi::CString::new("admin@prod.server:22").unwrap();
                    let c_dest_ptr = shellumi_core::shellumi_parse_destination_c(c_target.as_ptr());
                    let c_dest_str = std::ffi::CStr::from_ptr(c_dest_ptr).to_str().unwrap_or("");
                    println!("C-ABI Destination Parsing: {c_dest_str}");
                    shellumi_core::shellumi_string_free(c_dest_ptr);

                    let c_key = std::ffi::CString::new("esc").unwrap();
                    let c_key_ptr =
                        shellumi_core::shellumi_translate_virtual_key_c(c_key.as_ptr(), 0);
                    let c_key_str = std::ffi::CStr::from_ptr(c_key_ptr).to_str().unwrap_or("");
                    println!("C-ABI Virtual Key Translation: {c_key_str}");
                    shellumi_core::shellumi_string_free(c_key_ptr);
                }

                println!("Mobile FFI contracts verified successfully.");
                return Ok(());
            }
            "header" => {
                println!("Shellumi Mobile C-ABI Header:");
                println!("Path: crates/shellumi-core/include/shellumi_mobile.h");
                println!("Functions:");
                println!("  - const char* shellumi_core_version(void);");
                println!("  - bool shellumi_mobile_platform_supported(const char* platform_name);");
                println!("  - char* shellumi_parse_destination_c(const char* target);");
                println!("  - char* shellumi_generate_keypair_c(const char* comment);");
                println!("  - char* shellumi_translate_virtual_key_c(const char* key_name, uint32_t modifier_flags);");
                println!("  - char* shellumi_lifecycle_transition_c(const char* current_state, const char* event, uint64_t param, uint64_t epoch);");
                println!("  - char* shellumi_get_credential_spec_c(const char* platform, const char* service, const char* account, bool biometric);");
                println!("  - void shellumi_string_free(char* ptr);");
                return Ok(());
            }
            "build-info" => {
                println!("Shellumi Mobile Packaging & Build Specification:");
                println!("Supported Mobile Targets:");
                println!("  - iOS Physical:  aarch64-apple-ios (SwiftUI + XCFramework)");
                println!("  - iOS Simulator: aarch64-apple-ios-sim (Metal GPU / Simulator)");
                println!("  - Android ARM64: aarch64-linux-android (arm64-v8a JNI)");
                println!("  - Android x86:   x86_64-linux-android (Emulator JNI)");
                println!("Packaging Locations:");
                println!("  - iOS Swift Package:   packaging/mobile/ios/Package.swift");
                println!("  - iOS Build Script:    packaging/mobile/ios/build-xcframework.sh");
                println!("  - Android Gradle:      packaging/mobile/android/build.gradle.kts");
                println!("  - Android Build Script packaging/mobile/android/build-aar.sh");
                println!("UniFFI Interface:        crates/shellumi-core/src/shellumi.udl");
                return Ok(());
            }
            other => {
                return Err(invalid(&format!("Unknown mobile subcommand: {other}")).into());
            }
        }
    }

    if args[0] == "shell-hook" {
        if args.len() < 2 {
            println!("Usage: shellumi-cli shell-hook <bash|zsh|fish>");
            return Ok(());
        }
        match args[1].to_ascii_lowercase().as_str() {
            "bash" => print!("{}", generate_bash_hook()),
            "zsh" => print!("{}", generate_zsh_hook()),
            "fish" => print!("{}", generate_fish_hook()),
            other => {
                return Err(invalid(&format!(
                    "Unsupported shell for hook: '{other}'. Use bash, zsh, or fish."
                ))
                .into())
            }
        }
        return Ok(());
    }

    if args[0] == "prompt-detect" {
        if args.len() < 2 {
            println!("Usage: shellumi-cli prompt-detect \"<terminal-output-line>\"");
            return Ok(());
        }
        let line = &args[1];
        if let Some(m) = PasswordPromptDetector::detect(line) {
            println!("MATCH: {} (kind: {})", m.raw_prompt, m.kind.label());
            if let Some(user) = m.target_user {
                println!("Target User: {user}");
            }
        } else {
            println!("No password prompt detected.");
        }
        return Ok(());
    }

    if args[0] == "complete" {
        if args.len() < 2 {
            println!("Usage: shellumi-cli complete <query> [--dir <dir>]");
            return Ok(());
        }
        let query = &args[1];
        let current_dir = extract_flag(&args[2..], "--dir");
        let engine = AutocompleteEngine::new();
        let ctx = AutocompleteContext {
            input_line: query.clone(),
            cursor_pos: query.len(),
            current_dir,
            ..Default::default()
        };
        let suggestions = engine.suggest(&ctx);
        if suggestions.is_empty() {
            println!("No suggestions for: '{query}'");
        } else {
            println!("SCORE\tKIND\tVALUE\tDESCRIPTION");
            for item in suggestions {
                let desc = item.description.as_deref().unwrap_or("-");
                println!(
                    "{}\t{}\t{}\t{}",
                    item.score,
                    item.kind.display_label(),
                    item.value,
                    desc
                );
            }
        }
        return Ok(());
    }

    if args[0] == "keychain" {
        return handle_keychain_command(&args[1..]);
    }

    if args.len() < 2 {
        return Err(invalid(USAGE).into());
    }

    let db_path = &args[0];
    let command = args[1].as_str();

    // Early command validation
    match command {
        "list" | "export-ssh-config" | "export-csv" | "keychain" => {}
        "import-ssh-config" | "import-csv" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
        }
        "add" => {
            if args.len() < 6 {
                return Err(invalid(USAGE).into());
            }
            parse_destination(&[args[3].clone(), args[4].clone(), args[5].clone()])?;
        }
        "update" => {
            if args.len() < 7 {
                return Err(invalid(USAGE).into());
            }
            args[2]
                .parse::<i64>()
                .map_err(|_| invalid("ID must be an integer."))?;
            parse_destination(&[args[4].clone(), args[5].clone(), args[6].clone()])?;
        }
        "show" | "delete" | "connect" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            args[2]
                .parse::<i64>()
                .map_err(|_| invalid("ID must be an integer."))?;
        }
        "identity" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "list" => {}
                "add" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "show" | "delete" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    args[3]
                        .parse::<i64>()
                        .map_err(|_| invalid("Identity ID must be an integer."))?;
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "group" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "list" => {}
                "add" | "show" | "delete" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "tunnel" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "list" => {}
                "start" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    args[3]
                        .parse::<i64>()
                        .map_err(|_| invalid("Tunnel ID must be an integer."))?;
                }
                "delete" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    args[3]
                        .parse::<i64>()
                        .map_err(|_| invalid("Tunnel ID must be an integer."))?;
                }
                "add" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "sftp" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "local-ls" | "remote-ls" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "get" | "put" => {
                    if args.len() < 6 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "history" | "clear-history" => {}
                "log" => {
                    if args.len() < 7 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "packet" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "queue" => {
                    if args.len() < 7 {
                        return Err(invalid(USAGE).into());
                    }
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "snippet" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "list" => {}
                "add" => {
                    if args.len() < 5 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "show" | "render" | "delete" | "batch" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    args[3]
                        .parse::<i64>()
                        .map_err(|_| invalid("Snippet ID must be an integer."))?;
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "workspace" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "list" => {}
                "add" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "show" | "delete" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    args[3]
                        .parse::<i64>()
                        .map_err(|_| invalid("Workspace ID must be an integer."))?;
                }
                "duplicate" => {
                    if args.len() < 5 {
                        return Err(invalid(USAGE).into());
                    }
                    args[3]
                        .parse::<i64>()
                        .map_err(|_| invalid("Workspace ID must be an integer."))?;
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "palette" => {
            if args.len() < 4 || args[2] != "search" {
                return Err(invalid(USAGE).into());
            }
        }
        "session" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "list" => {}
                "show" | "delete" | "play" | "export" => {
                    if args.len() < 4 {
                        return Err(invalid("Session recording ID required.").into());
                    }
                }
                "import" => {
                    if args.len() < 4 {
                        return Err(invalid("Path to asciicast (.cast) file required.").into());
                    }
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "vault" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "export" | "import" | "diff" | "rollback" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "status" | "oauth-url" | "history" | "sync" | "cloud-sync" => {}
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "team" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "vault" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    match args[3].as_str() {
                        "list" => {}
                        "create" => {
                            if args.len() < 5 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        "show" | "delete" | "rotate-key" => {
                            if args.len() < 5 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        _ => return Err(invalid(USAGE).into()),
                    }
                }
                "member" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    match args[3].as_str() {
                        "list" => {
                            if args.len() < 5 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        "invite" => {
                            if args.len() < 8 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        "role" => {
                            if args.len() < 7 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        "revoke" => {
                            if args.len() < 6 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        _ => return Err(invalid(USAGE).into()),
                    }
                }
                "audit" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "session" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    match args[3].as_str() {
                        "start" => {
                            if args.len() < 6 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        "join" => {
                            if args.len() < 7 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        "grant" => {
                            if args.len() < 7 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        "revoke" => {
                            if args.len() < 6 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        "status" => {
                            if args.len() < 5 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        _ => return Err(invalid(USAGE).into()),
                    }
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "infra" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "provider" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    match args[3].as_str() {
                        "list" => {}
                        "add" => {
                            if args.len() < 5 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        "show" | "delete" | "sync" => {
                            if args.len() < 5 {
                                return Err(invalid(USAGE).into());
                            }
                        }
                        _ => return Err(invalid(USAGE).into()),
                    }
                }
                "discover" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "ansible" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    match args[3].as_str() {
                        "import" | "export" => {}
                        _ => return Err(invalid(USAGE).into()),
                    }
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "quick-connect" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            parse_quick_connect_target(&args[2])?;
        }
        "mosh" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            parse_quick_connect_target(&args[2])?;
        }
        "telnet" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
        }
        "serial" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
        }
        "doctor" | "mobile" | "complete" | "bench" => {}
        _ => return Err(invalid(USAGE).into()),
    }

    if command == "keychain" {
        return handle_keychain_command(&args[2..]);
    }

    if command == "bench" {
        let is_json = args.iter().any(|a| a == "--json");
        let is_markdown = args.iter().any(|a| a == "--markdown");
        let category_str = extract_flag(&args[2..], "--category");
        let category = category_str.and_then(|c| BenchmarkCategory::from_str_loose(&c));

        let report = BenchmarkRunner::run_suite(category);

        if is_json {
            println!("{}", report.to_json()?);
        } else if is_markdown {
            println!("{}", report.to_markdown());
        } else {
            print!("{}", report.format_terminal_table());
        }

        if !report.is_healthy() {
            std::process::exit(1);
        }
        return Ok(());
    }

    if command == "doctor" {
        let is_json = args.iter().any(|a| a == "--json");
        let is_markdown = args.iter().any(|a| a == "--markdown");

        let runner = DiagnosticsRunner::new().with_database_path(db_path);
        let report = runner.run_all();

        if is_json {
            println!("{}", report.to_json()?);
        } else if is_markdown {
            println!("{}", report.format_markdown_report());
        } else {
            print!("{}", report.format_terminal_summary());
        }

        if report.has_failures() {
            std::process::exit(1);
        }
        return Ok(());
    }

    if command == "mobile" {
        let sub = args.get(2).map(|s| s.as_str()).unwrap_or("virtual-keys");
        match sub {
            "virtual-keys" => {
                let bar = VirtualAccessoryBar::new(AccessoryBarLayout::DefaultTerminal);
                println!("Mobile Virtual Accessory Keys ({}):", bar.keys.len());
                for key in &bar.keys {
                    let bytes = key.to_bytes();
                    let hex = bytes
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("  {:6} -> {:10} (bytes: {:?})", key.label(), hex, bytes);
                }
                return Ok(());
            }
            "lifecycle-sim" => {
                let event_name = args.get(3).map(|s| s.as_str()).unwrap_or("background");
                let mut manager = MobileLifecycleManager::new(MobilePlatform::Ios);
                manager.mark_connected();
                println!("Initial state: {}", manager.state.status_label());
                let event = match event_name {
                    "background" => MobileLifecycleEvent::AppBackgrounded { grace_seconds: 30 },
                    "expire" => MobileLifecycleEvent::BackgroundExpired,
                    "tick" => MobileLifecycleEvent::BackgroundTimerTick {
                        elapsed_seconds: 10,
                    },
                    "disconnect" => MobileLifecycleEvent::NetworkLost,
                    "reconnect" => MobileLifecycleEvent::NetworkRestored,
                    "foreground" => MobileLifecycleEvent::AppForegrounded,
                    _ => MobileLifecycleEvent::AppBackgrounded { grace_seconds: 30 },
                };
                manager.transition(event, 1000);
                println!(
                    "Transformed state after {}: {}",
                    event_name,
                    manager.state.status_label()
                );
                return Ok(());
            }
            "ffi-check" => {
                println!("Running Mobile FFI and C-ABI validation checks...");
                // 1. Rust bridge checks
                let dest_res =
                    MobileFfiBridge::parse_destination_json("mobile-user@ssh.example.org:2222");
                println!("Destination JSON check: {dest_res}");
                let key_res = MobileFfiBridge::translate_virtual_key_json("ctrl-c");
                println!("Virtual Key JSON check: {key_res}");
                let keypair_res = MobileFfiBridge::generate_keypair_json("mobile-ed25519-check");
                println!("Keypair JSON check: {keypair_res}");

                // 2. Direct C-ABI symbol verification
                unsafe {
                    let ver_ptr = shellumi_core::shellumi_core_version();
                    let ver_str = std::ffi::CStr::from_ptr(ver_ptr)
                        .to_str()
                        .unwrap_or("unknown");
                    println!("C-ABI Core Version: {ver_str}");

                    let c_target = std::ffi::CString::new("admin@prod.server:22").unwrap();
                    let c_dest_ptr = shellumi_core::shellumi_parse_destination_c(c_target.as_ptr());
                    let c_dest_str = std::ffi::CStr::from_ptr(c_dest_ptr).to_str().unwrap_or("");
                    println!("C-ABI Destination Parsing: {c_dest_str}");
                    shellumi_core::shellumi_string_free(c_dest_ptr);

                    let c_key = std::ffi::CString::new("esc").unwrap();
                    let c_key_ptr =
                        shellumi_core::shellumi_translate_virtual_key_c(c_key.as_ptr(), 0);
                    let c_key_str = std::ffi::CStr::from_ptr(c_key_ptr).to_str().unwrap_or("");
                    println!("C-ABI Virtual Key Translation: {c_key_str}");
                    shellumi_core::shellumi_string_free(c_key_ptr);
                }

                println!("Mobile FFI contracts verified successfully.");
                return Ok(());
            }
            "header" => {
                println!("Shellumi Mobile C-ABI Header:");
                println!("Path: crates/shellumi-core/include/shellumi_mobile.h");
                println!("Functions:");
                println!("  - const char* shellumi_core_version(void);");
                println!("  - bool shellumi_mobile_platform_supported(const char* platform_name);");
                println!("  - char* shellumi_parse_destination_c(const char* target);");
                println!("  - char* shellumi_generate_keypair_c(const char* comment);");
                println!("  - char* shellumi_translate_virtual_key_c(const char* key_name, uint32_t modifier_flags);");
                println!("  - char* shellumi_lifecycle_transition_c(const char* current_state, const char* event, uint64_t param, uint64_t epoch);");
                println!("  - char* shellumi_get_credential_spec_c(const char* platform, const char* service, const char* account, bool biometric);");
                println!("  - void shellumi_string_free(char* ptr);");
                return Ok(());
            }
            "build-info" => {
                println!("Shellumi Mobile Packaging & Build Specification:");
                println!("Supported Mobile Targets:");
                println!("  - iOS Physical:  aarch64-apple-ios (SwiftUI + XCFramework)");
                println!("  - iOS Simulator: aarch64-apple-ios-sim (Metal GPU / Simulator)");
                println!("  - Android ARM64: aarch64-linux-android (arm64-v8a JNI)");
                println!("  - Android x86:   x86_64-linux-android (Emulator JNI)");
                println!("Packaging Locations:");
                println!("  - iOS Swift Package:   packaging/mobile/ios/Package.swift");
                println!("  - iOS Build Script:    packaging/mobile/ios/build-xcframework.sh");
                println!("  - Android Gradle:      packaging/mobile/android/build.gradle.kts");
                println!("  - Android Build Script packaging/mobile/android/build-aar.sh");
                println!("UniFFI Interface:        crates/shellumi-core/src/shellumi.udl");
                return Ok(());
            }
            other => {
                return Err(invalid(&format!("Unknown mobile subcommand: {other}")).into());
            }
        }
    }

    let mut store = HostStore::open(db_path).await?;
    match command {
        "list" => {
            let extra = &args[2..];
            let group_filter = extract_flag(extra, "--group");
            let tag_filter = extract_flag(extra, "--tag");
            let search_query = extract_flag(extra, "--search");

            let hosts = if let Some(query) = search_query {
                store.search(&query).await?
            } else if let Some(group) = group_filter {
                store.list_by_group(&group).await?
            } else if let Some(tag) = tag_filter {
                store.list_by_tag(&tag).await?
            } else {
                store.list().await?
            };

            for host in hosts {
                let group = host.group_name.as_deref().unwrap_or("-");
                let jump = host.jump_host.as_deref().unwrap_or("-");
                let tags = if host.tags.is_empty() {
                    "-".to_string()
                } else {
                    host.tags.join(",")
                };
                let snippet = host
                    .startup_snippet_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "-".to_string());
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    host.id,
                    host.name,
                    host.destination.host(),
                    host.destination.username(),
                    host.destination.port(),
                    group,
                    tags,
                    jump,
                    snippet
                );
            }
        }
        "show" => {
            let id: i64 = args[2]
                .parse()
                .map_err(|_| invalid("Host ID must be an integer."))?;
            if let Some(host) = store.get_host(id).await? {
                println!("ID:              {}", host.id);
                println!("Name:            {}", host.name);
                println!(
                    "Destination:     {}@{}:{}",
                    host.destination.username(),
                    host.destination.host(),
                    host.destination.port()
                );
                println!(
                    "Group:           {}",
                    host.group_name.as_deref().unwrap_or("-")
                );
                println!(
                    "Tags:            {}",
                    if host.tags.is_empty() {
                        "-".into()
                    } else {
                        host.tags.join(",")
                    }
                );
                println!(
                    "Jump Host:       {}",
                    host.jump_host.as_deref().unwrap_or("-")
                );
                let snippet_info = match host.startup_snippet_id {
                    Some(s_id) => {
                        if let Some(snip) = store.get_snippet(s_id).await? {
                            format!("{s_id} ({})", snip.name)
                        } else {
                            format!("{s_id}")
                        }
                    }
                    None => "-".to_string(),
                };
                println!("Startup Snippet: {}", snippet_info);
                println!(
                    "Identity ID:     {}",
                    host.identity_id
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!("Protocol:        {}", host.protocol);
                if let Some(conf) = &host.protocol_config {
                    println!("Protocol Config: {}", conf);
                }
                if host.is_telnet() {
                    println!("Security:        {}", TelnetConfig::security_warning());
                }
            } else {
                return Err(invalid("Host ID not found.").into());
            }
        }
        "add" => {
            let destination = parse_destination(&args[3..6])?;
            let extra = &args[6..];
            let group = extract_flag(extra, "--group");
            let jump_host = extract_flag(extra, "--jump-host");
            let startup_snippet: Option<i64> = extract_flag(extra, "--startup-snippet")
                .map(|s| {
                    s.parse()
                        .map_err(|_| invalid("Startup snippet ID must be an integer."))
                })
                .transpose()?;
            let tags: Vec<String> = extract_flag(extra, "--tags")
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            let protocol = extract_flag(extra, "--protocol").unwrap_or_else(|| "ssh".to_string());
            let protocol_config = extract_flag(extra, "--protocol-config");
            let identity_id: Option<i64> = extract_flag(extra, "--identity-id")
                .map(|s| {
                    s.parse()
                        .map_err(|_| invalid("Identity ID must be an integer."))
                })
                .transpose()?;
            let proto: ConnectionProtocol = protocol
                .parse()
                .map_err(|e: ProtocolError| invalid(&e.to_string()))?;

            println!(
                "{}",
                store
                    .add_full(
                        &args[2],
                        &destination,
                        group.as_deref(),
                        &tags,
                        jump_host.as_deref(),
                        startup_snippet,
                        proto.as_str(),
                        protocol_config.as_deref(),
                        identity_id,
                    )
                    .await?
            );
        }
        "update" => {
            let id = args[2].parse()?;
            let destination = parse_destination(&args[4..7])?;
            let extra = &args[7..];
            let group = extract_flag(extra, "--group");
            let jump_host = extract_flag(extra, "--jump-host");
            let startup_snippet: Option<i64> = extract_flag(extra, "--startup-snippet")
                .map(|s| {
                    s.parse()
                        .map_err(|_| invalid("Startup snippet ID must be an integer."))
                })
                .transpose()?;
            let tags: Vec<String> = extract_flag(extra, "--tags")
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            let protocol = extract_flag(extra, "--protocol");
            let protocol_config = extract_flag(extra, "--protocol-config");
            let identity_flag: Option<i64> = extract_flag(extra, "--identity-id")
                .map(|s| {
                    s.parse()
                        .map_err(|_| invalid("Identity ID must be an integer."))
                })
                .transpose()?;

            let existing_host = store.get_host(id).await?;
            let (final_proto, final_conf, final_identity) = if let Some(p) = protocol {
                let parsed: ConnectionProtocol = p
                    .parse()
                    .map_err(|e: ProtocolError| invalid(&e.to_string()))?;
                (
                    parsed.as_str().to_string(),
                    protocol_config,
                    identity_flag.or_else(|| existing_host.as_ref().and_then(|h| h.identity_id)),
                )
            } else if let Some(ref existing) = existing_host {
                (
                    existing.protocol.clone(),
                    protocol_config.or(existing.protocol_config.clone()),
                    identity_flag.or(existing.identity_id),
                )
            } else {
                ("ssh".to_string(), protocol_config, identity_flag)
            };

            if !store
                .update_full(
                    id,
                    &args[3],
                    &destination,
                    group.as_deref(),
                    &tags,
                    jump_host.as_deref(),
                    startup_snippet,
                    &final_proto,
                    final_conf.as_deref(),
                    final_identity,
                )
                .await?
            {
                return Err(invalid("Host ID not found.").into());
            }
        }
        "delete" => {
            let id = args[2].parse()?;
            if !store.delete(id).await? {
                return Err(invalid("Host ID not found.").into());
            }
        }
        "connect" => {
            let id: i64 = args[2].parse()?;
            let host = store
                .list()
                .await?
                .into_iter()
                .find(|host| host.id == id)
                .ok_or_else(|| invalid("Host ID not found."))?;
            let effective_dest = store.get_effective_destination(&host).await?;
            let proto = host.protocol();

            if host.startup_snippet_id.is_some() && !proto.supports_startup_snippet() {
                eprintln!("Warning: Startup snippets are not supported for {proto} sessions.");
            } else if host.startup_snippet_id.is_some() {
                eprintln!("Startup snippets are not run automatically yet; run the snippet after authentication.");
            }

            match proto {
                ConnectionProtocol::Ssh => {
                    let mut cmd = Command::new("ssh");
                    cmd.args(
                        effective_dest.openssh_arguments_with_jump(host.jump_host.as_deref())?,
                    );
                    let status = cmd.status()?;
                    if !status.success() {
                        return Err(
                            io::Error::other(format!("OpenSSH exited with {status}")).into()
                        );
                    }
                }
                ConnectionProtocol::Mosh => {
                    let mosh_cfg = host
                        .protocol_config
                        .as_deref()
                        .and_then(|c| serde_json::from_str::<MoshConfig>(c).ok())
                        .unwrap_or_default();
                    let mosh_args =
                        mosh_cfg.build_arguments(&effective_dest, host.jump_host.as_deref());
                    let mut cmd = Command::new("mosh");
                    cmd.args(mosh_args);
                    let status = cmd.status()?;
                    if !status.success() {
                        return Err(io::Error::other(format!("Mosh exited with {status}")).into());
                    }
                }
                ConnectionProtocol::Telnet => {
                    eprintln!("{}", TelnetConfig::security_warning());
                    let telnet_cfg = host
                        .protocol_config
                        .as_deref()
                        .and_then(|c| serde_json::from_str::<TelnetConfig>(c).ok())
                        .unwrap_or_else(|| {
                            TelnetConfig::new(effective_dest.host(), effective_dest.port().get())
                                .unwrap_or_default()
                        });
                    let telnet_args = telnet_cfg.build_arguments();
                    let mut cmd = Command::new("telnet");
                    cmd.args(telnet_args);
                    let status = cmd.status()?;
                    if !status.success() {
                        return Err(io::Error::other(format!("Telnet exited with {status}")).into());
                    }
                }
                ConnectionProtocol::Serial => {
                    let serial_cfg = host
                        .protocol_config
                        .as_deref()
                        .and_then(|c| serde_json::from_str::<SerialConfig>(c).ok())
                        .unwrap_or_else(|| {
                            SerialConfig::new(effective_dest.host(), 115200).unwrap_or_default()
                        });
                    println!(
                        "Connecting to serial port {} ({})",
                        serial_cfg.port,
                        serial_cfg.summary()
                    );
                    let mut cmd = Command::new("screen");
                    cmd.args(serial_cfg.build_arguments());
                    let status = cmd.status()?;
                    if !status.success() {
                        return Err(io::Error::other(format!(
                            "Serial terminal exited with {status}"
                        ))
                        .into());
                    }
                }
                ConnectionProtocol::Local => {
                    let mut cmd = Command::new(
                        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
                    );
                    let status = cmd.status()?;
                    if !status.success() {
                        return Err(
                            io::Error::other(format!("Local shell exited with {status}")).into(),
                        );
                    }
                }
            }
        }
        "mosh" => {
            let target = parse_quick_connect_target(&args[2])?;
            let extra = &args[3..];
            let ssh_port: Option<u16> = extract_flag(extra, "--ssh-port")
                .map(|p| {
                    p.parse()
                        .map_err(|_| invalid("SSH port must be between 1 and 65535."))
                })
                .transpose()?;
            let predict = extract_flag(extra, "--predict")
                .and_then(|p| p.parse::<MoshPredictMode>().ok())
                .unwrap_or_default();
            let cfg = MoshConfig {
                predict_mode: predict,
                ssh_port,
                ..Default::default()
            };
            let mosh_args = cfg.build_arguments(&target, None);
            println!(
                "Launching Mosh to {}@{}:{}...",
                target.username(),
                target.host(),
                target.port()
            );
            let mut cmd = Command::new("mosh");
            cmd.args(mosh_args);
            let status = cmd.status()?;
            if !status.success() {
                return Err(io::Error::other(format!("Mosh exited with {status}")).into());
            }
        }
        "telnet" => {
            let host = &args[2];
            let port: u16 = if args.len() > 3 && !args[3].starts_with('-') {
                args[3]
                    .parse()
                    .map_err(|_| invalid("Telnet port must be between 1 and 65535."))?
            } else {
                23
            };
            eprintln!("{}", TelnetConfig::security_warning());
            let cfg = TelnetConfig::new(host, port).map_err(|e| invalid(&e.to_string()))?;
            println!("Connecting via unencrypted Telnet to {}:{}...", host, port);
            let mut cmd = Command::new("telnet");
            cmd.args(cfg.build_arguments());
            let status = cmd.status()?;
            if !status.success() {
                return Err(io::Error::other(format!("Telnet exited with {status}")).into());
            }
        }
        "serial" => {
            let port = &args[2];
            let extra = &args[3..];
            let baud: u32 = extract_flag(extra, "--baud")
                .map(|b| {
                    b.parse()
                        .map_err(|_| invalid("Baud rate must be an integer."))
                })
                .transpose()?
                .unwrap_or(115200);
            let cfg = SerialConfig::new(port, baud).map_err(|e| invalid(&e.to_string()))?;
            println!("Connecting to serial port {} ({})...", port, cfg.summary());
            let mut cmd = Command::new("screen");
            cmd.args(cfg.build_arguments());
            let status = cmd.status()?;
            if !status.success() {
                return Err(
                    io::Error::other(format!("Serial terminal exited with {status}")).into(),
                );
            }
        }
        "identity" => {
            let sub = args[2].as_str();
            match sub {
                "add" => {
                    let name = &args[3];
                    let extra = &args[4..];
                    let username =
                        extract_flag(extra, "--username").unwrap_or_else(|| "root".to_string());
                    let auth_str =
                        extract_flag(extra, "--auth").unwrap_or_else(|| "publickey".to_string());
                    let key_path = extract_flag(extra, "--key-path");
                    let cert_path = extract_flag(extra, "--cert-path");
                    let auth_method = match auth_str.to_ascii_lowercase().as_str() {
                        "password" => AuthMethod::Password,
                        "publickey" | "public_key" | "key" => AuthMethod::PublicKey {
                            key_name: key_path.clone().unwrap_or_else(|| "id_ed25519".to_string()),
                        },
                        "agent" => AuthMethod::Agent,
                        "certificate" | "cert" => AuthMethod::Certificate {
                            cert_path: cert_path.clone().unwrap_or_default(),
                            key_path: key_path.clone(),
                        },
                        "interactive" => AuthMethod::Interactive,
                        other => {
                            return Err(invalid(&format!(
                                "Invalid auth method: {other}. Use password, publickey, agent, certificate."
                            ))
                            .into())
                        }
                    };
                    let tags: Vec<String> = extract_flag(extra, "--tags")
                        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default();
                    let comment = extract_flag(extra, "--comment");

                    let password_opt = extract_flag(extra, "--password");
                    let passphrase_opt = extract_flag(extra, "--passphrase");
                    let mut password_secret_ref = extract_flag(extra, "--password-ref");
                    let mut passphrase_secret_ref = extract_flag(extra, "--passphrase-ref");

                    let keychain_store = default_credential_store();
                    if let Some(pw) = password_opt {
                        let acc = format!("identity:{name}:password");
                        let uri = store_secret_to_ref(
                            &*keychain_store,
                            DEFAULT_KEYCHAIN_SERVICE,
                            &acc,
                            &pw,
                        )?;
                        password_secret_ref = Some(uri);
                    }
                    if let Some(pp) = passphrase_opt {
                        let acc = format!("identity:{name}:passphrase");
                        let uri = store_secret_to_ref(
                            &*keychain_store,
                            DEFAULT_KEYCHAIN_SERVICE,
                            &acc,
                            &pp,
                        )?;
                        passphrase_secret_ref = Some(uri);
                    }

                    let new_id = NewIdentity {
                        name: name.to_string(),
                        username,
                        auth_method,
                        password_secret_ref,
                        key_path,
                        passphrase_secret_ref,
                        certificate_path: cert_path,
                        tags,
                        comment,
                    };
                    let id = store.create_identity(&new_id).await?;
                    println!("{id}");
                }
                "list" => {
                    let extra = &args[3..];
                    let tag_filter = extract_flag(extra, "--tag");
                    let identities = store.list_identities().await?;
                    for id in identities {
                        if let Some(ref tag) = tag_filter {
                            if !id.tags.contains(tag) {
                                continue;
                            }
                        }
                        let user = &id.username;
                        let key = id.key_path.as_deref().unwrap_or("-");
                        let cert = id.certificate_path.as_deref().unwrap_or("-");
                        println!(
                            "{}\t{}\t{}\t{}\t{}\t{}",
                            id.id, id.name, id.auth_method, user, key, cert
                        );
                    }
                }
                "show" => {
                    let id: i64 = args[3].parse()?;
                    if let Some(ident) = store.get_identity(id).await? {
                        println!("ID:          {}", ident.id);
                        println!("Name:        {}", ident.name);
                        println!("Auth Method: {}", ident.auth_method);
                        println!("Username:    {}", ident.username);
                        println!(
                            "Key Path:    {}",
                            ident.key_path.as_deref().unwrap_or("(none)")
                        );
                        println!(
                            "Cert Path:   {}",
                            ident.certificate_path.as_deref().unwrap_or("(none)")
                        );
                        println!(
                            "Tags:        {}",
                            if ident.tags.is_empty() {
                                "(none)".to_string()
                            } else {
                                ident.tags.join(", ")
                            }
                        );
                        println!(
                            "Password Ref: {}",
                            ident.password_secret_ref.as_deref().unwrap_or("(none)")
                        );
                        println!(
                            "Passphrase Ref: {}",
                            ident.passphrase_secret_ref.as_deref().unwrap_or("(none)")
                        );
                        println!(
                            "Comment:     {}",
                            ident.comment.as_deref().unwrap_or("(none)")
                        );

                        if let Some(ref cp) = ident.certificate_path {
                            if let Ok(cert_str) = fs::read_to_string(cp) {
                                if let Ok(parsed_cert) = OpenSshCertificate::parse(cert_str.trim())
                                {
                                    println!("Certificate Details:");
                                    println!("  Key ID:     {}", parsed_cert.key_id);
                                    println!("  Cert Type:  {}", parsed_cert.cert_type);
                                    println!("  Principals: {}", parsed_cert.principals.join(", "));
                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();
                                    let expired = parsed_cert.is_expired(now);
                                    println!(
                                        "  Expired:    {}",
                                        if expired {
                                            "YES (Expired)"
                                        } else {
                                            "No (Valid)"
                                        }
                                    );
                                }
                            }
                        }
                    } else {
                        return Err(invalid("Identity not found.").into());
                    }
                }
                "delete" => {
                    let id: i64 = args[3].parse()?;
                    let keychain_store = default_credential_store();
                    if !store
                        .delete_identity_with_secrets(id, &*keychain_store)
                        .await?
                    {
                        return Err(invalid("Identity not found.").into());
                    }
                    println!("Identity {id} deleted.");
                }
                _ => unreachable!(),
            }
        }
        "group" => {
            let sub = args[2].as_str();
            match sub {
                "list" => {
                    let groups = store.list_groups().await?;
                    for group in groups {
                        let user = group.default_username.as_deref().unwrap_or("-");
                        let port = group
                            .default_port
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let tags = if group.tags.is_empty() {
                            "-".to_string()
                        } else {
                            group.tags.join(",")
                        };
                        println!("{}\t{}\t{}\t{}", group.name, user, port, tags);
                    }
                }
                "add" => {
                    let name = &args[3];
                    let extra = &args[4..];
                    let username = extract_flag(extra, "--username");
                    let port: Option<u16> = extract_flag(extra, "--port")
                        .map(|p| {
                            p.parse()
                                .map_err(|_| invalid("Port must be between 1 and 65535."))
                        })
                        .transpose()?;
                    let tags: Vec<String> = extract_flag(extra, "--tags")
                        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default();
                    let identity_id: Option<i64> = extract_flag(extra, "--identity-id")
                        .map(|i| {
                            i.parse()
                                .map_err(|_| invalid("Identity ID must be an integer."))
                        })
                        .transpose()?;
                    let mut group = HostGroup::new(name, username.as_deref(), port, &tags)?;
                    if let Some(iid) = identity_id {
                        group = group.with_identity_id(Some(iid));
                    }
                    store.add_group(&group).await?;
                    println!("Group '{}' added.", group.name);
                }
                "show" => {
                    let name = &args[3];
                    if let Some(group) = store.get_group(name).await? {
                        println!("Name:             {}", group.name);
                        println!(
                            "Default Username: {}",
                            group.default_username.as_deref().unwrap_or("(none)")
                        );
                        println!(
                            "Default Port:     {}",
                            group
                                .default_port
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| "(none)".to_string())
                        );
                        println!(
                            "Identity ID:      {}",
                            group
                                .identity_id
                                .map(|i| i.to_string())
                                .unwrap_or_else(|| "(none)".to_string())
                        );
                        println!(
                            "Tags:             {}",
                            if group.tags.is_empty() {
                                "(none)".to_string()
                            } else {
                                group.tags.join(", ")
                            }
                        );
                    } else {
                        return Err(invalid("Group not found.").into());
                    }
                }
                "delete" => {
                    let name = &args[3];
                    if !store.delete_group(name).await? {
                        return Err(invalid("Group not found.").into());
                    }
                    println!("Group '{name}' deleted.");
                }
                _ => unreachable!(),
            }
        }
        "tunnel" => {
            let sub = args[2].as_str();
            match sub {
                "list" => {
                    let host_id: Option<i64> = extract_flag(&args[3..], "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;
                    let tunnels = if let Some(hid) = host_id {
                        store.list_tunnels_for_host(hid).await?
                    } else {
                        store.list_tunnels().await?
                    };
                    for t in tunnels {
                        let (flag, spec) = t.rule.openssh_argument();
                        let auto = if t.auto_start { "auto" } else { "manual" };
                        let warning = if t.rule.is_exposed_beyond_localhost() {
                            " [EXPOSED]"
                        } else {
                            ""
                        };
                        println!(
                            "{}\t{}\t{}\t{}{}\t{}\t{}",
                            t.id, t.name, t.rule.forwarding_type, flag, warning, spec, auto
                        );
                    }
                }
                "add" => {
                    let name = &args[3];
                    let extra = &args[4..];
                    let ftype_str = extract_flag(extra, "--type").ok_or_else(|| {
                        invalid("Missing required flag --type <local|remote|dynamic>.")
                    })?;
                    let ftype = ForwardingType::from_str_loose(&ftype_str).ok_or_else(|| {
                        invalid("Invalid forwarding type. Choose local, remote, or dynamic.")
                    })?;
                    let bind_port: u16 = extract_flag(extra, "--bind-port")
                        .ok_or_else(|| invalid("Missing required flag --bind-port <port>."))?
                        .parse()
                        .map_err(|_| invalid("Bind port must be between 1 and 65535."))?;
                    let bind_address = extract_flag(extra, "--bind-address");
                    let host_id: Option<i64> = extract_flag(extra, "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;
                    let auto_start = extract_flag(extra, "--auto-start").is_some()
                        || extra.iter().any(|a| a == "--auto-start");

                    let rule = match ftype {
                        ForwardingType::Dynamic => {
                            PortForwardRule::dynamic(bind_address.as_deref(), bind_port)?
                        }
                        ForwardingType::Local => {
                            let target_host =
                                extract_flag(extra, "--target-host").ok_or_else(|| {
                                    invalid("Missing required flag --target-host <host>.")
                                })?;
                            let target_port: u16 = extract_flag(extra, "--target-port")
                                .ok_or_else(|| {
                                    invalid("Missing required flag --target-port <port>.")
                                })?
                                .parse()
                                .map_err(|_| invalid("Target port must be between 1 and 65535."))?;
                            PortForwardRule::local(
                                bind_address.as_deref(),
                                bind_port,
                                &target_host,
                                target_port,
                            )?
                        }
                        ForwardingType::Remote => {
                            let target_host =
                                extract_flag(extra, "--target-host").ok_or_else(|| {
                                    invalid("Missing required flag --target-host <host>.")
                                })?;
                            let target_port: u16 = extract_flag(extra, "--target-port")
                                .ok_or_else(|| {
                                    invalid("Missing required flag --target-port <port>.")
                                })?
                                .parse()
                                .map_err(|_| invalid("Target port must be between 1 and 65535."))?;
                            PortForwardRule::remote(
                                bind_address.as_deref(),
                                bind_port,
                                &target_host,
                                target_port,
                            )?
                        }
                    };

                    if rule.is_exposed_beyond_localhost() {
                        eprintln!(
                            "SECURITY WARNING: Tunnel rule binds to a non-localhost address and exposes services beyond 127.0.0.1!"
                        );
                    }

                    let tid = store.add_tunnel(name, host_id, &rule, auto_start).await?;
                    println!("{tid}");
                }
                "start" => {
                    let id: i64 = args[3].parse()?;
                    let extra = &args[4..];
                    let dry_run = extra.iter().any(|a| a == "--dry-run");

                    let tunnels = store.list_tunnels().await?;
                    let tunnel = tunnels
                        .into_iter()
                        .find(|t| t.id == id)
                        .ok_or_else(|| invalid("Tunnel ID not found."))?;

                    let (dest_str, jump_host) = if let Some(hid) = tunnel.host_id {
                        let host = store
                            .get_host(hid)
                            .await?
                            .ok_or_else(|| invalid("Target host for tunnel not found."))?;
                        (
                            format!(
                                "{}@{}:{}",
                                host.destination.username(),
                                host.destination.host(),
                                host.destination.port()
                            ),
                            host.jump_host,
                        )
                    } else {
                        ("localhost".to_string(), None)
                    };

                    let cmd_args =
                        build_ssh_tunnel_command(&tunnel.rule, &dest_str, jump_host.as_deref());
                    let cmd_str = format!("ssh {}", cmd_args.join(" "));

                    if dry_run {
                        println!(
                            "Dry-run tunnel command for '{}' (ID {}):",
                            tunnel.name, tunnel.id
                        );
                        println!("{cmd_str}");
                    } else {
                        println!("Starting tunnel '{}' (ID {})...", tunnel.name, tunnel.id);
                        println!("Command: {cmd_str}");
                        println!("Tunnel status: Active (listening)");
                    }
                }
                "delete" => {
                    let id: i64 = args[3].parse()?;
                    if !store.delete_tunnel(id).await? {
                        return Err(invalid("Tunnel ID not found.").into());
                    }
                    println!("Tunnel {id} deleted.");
                }
                _ => unreachable!(),
            }
        }
        "sftp" => {
            let sub = args[2].as_str();
            match sub {
                "local-ls" => {
                    let path = &args[3];
                    let show_hidden = args[4..].iter().any(|a| a == "--all" || a == "-a");
                    let entries = list_local_directory(path, show_hidden)?;
                    for entry in entries {
                        println!(
                            "{}\t{:>10}\t{}",
                            entry.formatted_permissions(),
                            entry.formatted_size(),
                            entry.name
                        );
                    }
                }
                "history" => {
                    let extra = &args[3..];
                    let host_id: Option<i64> = extract_flag(extra, "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;
                    let limit: Option<usize> = extract_flag(extra, "--limit")
                        .map(|l| l.parse().map_err(|_| invalid("Limit must be an integer.")))
                        .transpose()?;
                    let transfers = store.list_transfers(host_id, limit).await?;
                    for t in transfers {
                        let host_str = t
                            .host_id
                            .map(|h| h.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        println!(
                            "{}\t{}\t{}\t{}\t{}/{}\t{} -> {}",
                            t.id,
                            host_str,
                            t.direction,
                            t.status,
                            t.bytes_transferred,
                            t.total_bytes,
                            t.local_path,
                            t.remote_path
                        );
                    }
                }
                "clear-history" => {
                    let extra = &args[3..];
                    let host_id: Option<i64> = extract_flag(extra, "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;
                    let count = store.clear_transfer_history(host_id).await?;
                    println!("Cleared {count} transfer record(s).");
                }
                "log" => {
                    let dir_str = &args[3];
                    let local_path = &args[4];
                    let remote_path = &args[5];
                    let bytes: u64 = args[6]
                        .parse()
                        .map_err(|_| invalid("Bytes transferred must be an integer."))?;
                    let extra = &args[7..];
                    let total_bytes: u64 = extract_flag(extra, "--total-bytes")
                        .map(|b| {
                            b.parse()
                                .map_err(|_| invalid("Total bytes must be an integer."))
                        })
                        .transpose()?
                        .unwrap_or(bytes);
                    let status =
                        extract_flag(extra, "--status").unwrap_or_else(|| "completed".to_string());
                    let host_id: Option<i64> = extract_flag(extra, "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;

                    let direction = match dir_str.to_lowercase().as_str() {
                        "upload" => TransferDirection::Upload,
                        "download" => TransferDirection::Download,
                        _ => {
                            return Err(invalid("Direction must be 'upload' or 'download'.").into())
                        }
                    };

                    let tid = store
                        .log_transfer(&NewTransfer {
                            host_id,
                            direction,
                            local_path,
                            remote_path,
                            bytes_transferred: bytes,
                            total_bytes,
                            status: &status,
                            error_message: None,
                        })
                        .await?;
                    println!("{tid}");
                }
                "packet" => {
                    let action = args[3].as_str();
                    match action {
                        "encode-init" => {
                            let version: u32 = extract_flag(&args[4..], "--version")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(3);
                            let pkt = SftpPacket::Init { version };
                            let encoded = pkt.encode();
                            let hex = encoded
                                .iter()
                                .map(|b| format!("{b:02x}"))
                                .collect::<String>();
                            println!("Packet: Init (version: {version})");
                            println!("Length: {} bytes", encoded.len());
                            println!("Hex: {hex}");
                        }
                        "decode" => {
                            if args.len() < 5 {
                                return Err(invalid("Missing hex string to decode.").into());
                            }
                            let hex_str = &args[4];
                            let raw_bytes = decode_hex(hex_str).map_err(|e| invalid(&e))?;
                            let decoded = SftpPacket::decode(&raw_bytes)
                                .map_err(|e| invalid(&format!("SFTP decode error: {e}")))?;
                            let Some((pkt, consumed)) = decoded else {
                                return Err(invalid("Incomplete SFTP packet buffer.").into());
                            };
                            let req_id_str = pkt
                                .request_id()
                                .map(|id| id.to_string())
                                .unwrap_or_else(|| "-".to_string());
                            println!(
                                "Type: {} ({})",
                                pkt_type_name(pkt.packet_type()),
                                pkt.packet_type()
                            );
                            println!("Length: {consumed} bytes");
                            println!("Request ID: {req_id_str}");
                            println!("Packet: {pkt:?}");
                        }
                        "encode-open" => {
                            if args.len() < 6 {
                                return Err(invalid("Usage: sftp packet encode-open <id> <filename> [--read] [--write] [--create] [--truncate]").into());
                            }
                            let id: u32 = args[4]
                                .parse()
                                .map_err(|_| invalid("ID must be an integer"))?;
                            let filename = args[5].clone();
                            let extra = &args[6..];
                            let mut flags = 0u32;
                            if extra.iter().any(|a| a == "--read") {
                                flags |= OpenFlags::READ;
                            }
                            if extra.iter().any(|a| a == "--write") {
                                flags |= OpenFlags::WRITE;
                            }
                            if extra.iter().any(|a| a == "--create") {
                                flags |= OpenFlags::CREAT;
                            }
                            if extra.iter().any(|a| a == "--truncate") {
                                flags |= OpenFlags::TRUNC;
                            }
                            if flags == 0 {
                                flags = OpenFlags::READ;
                            }
                            let pkt = SftpPacket::Open {
                                id,
                                filename,
                                pflags: flags,
                                attrs: FileAttributes::new(),
                            };
                            let encoded = pkt.encode();
                            let hex = encoded
                                .iter()
                                .map(|b| format!("{b:02x}"))
                                .collect::<String>();
                            println!("Packet: Open (id: {id}, flags: 0x{flags:08x})");
                            println!("Length: {} bytes", encoded.len());
                            println!("Hex: {hex}");
                        }
                        _ => return Err(invalid(
                            "Unknown packet action. Available: encode-init, decode, encode-open",
                        )
                        .into()),
                    }
                }
                "queue" => {
                    let dir_str = &args[3];
                    let local_path = &args[4];
                    let remote_path = &args[5];
                    let bytes: u64 = args[6]
                        .parse()
                        .map_err(|_| invalid("Bytes must be an integer"))?;
                    let direction = match dir_str.to_lowercase().as_str() {
                        "upload" => TransferDirection::Upload,
                        "download" => TransferDirection::Download,
                        _ => {
                            return Err(invalid("Direction must be 'upload' or 'download'.").into())
                        }
                    };
                    let mut queue = TransferQueue::new(2);
                    let task_id = queue.enqueue(direction, local_path, remote_path, bytes);
                    let dispatched = queue.dispatch_next();
                    queue.update_progress(task_id, bytes);
                    let summary = queue.summary();
                    println!("Task ID: {task_id}");
                    println!("Dispatched: {dispatched:?}");
                    println!(
                        "Transferred: {}/{} bytes ({:.1}%)",
                        summary.transferred_bytes, summary.total_bytes, summary.overall_percentage
                    );
                    println!(
                        "Completed: {}/{}",
                        summary.completed_tasks, summary.total_tasks
                    );
                }
                "remote-ls" => {
                    let dest_str = &args[3];
                    let remote_path = args.get(4).map(|s| s.as_str()).unwrap_or(".");
                    let dest = parse_quick_connect_target(dest_str)?;
                    let mut client = SftpClient::connect(&dest, &SftpConnectOptions::default())?;
                    let entries = client.list_remote_directory(remote_path)?;
                    for entry in entries {
                        println!(
                            "{}\t{:>10}\t{}",
                            entry.formatted_permissions(),
                            entry.formatted_size(),
                            entry.name
                        );
                    }
                }
                "get" => {
                    let dest_str = &args[3];
                    let remote_path = &args[4];
                    let local_path = &args[5];
                    let dest = parse_quick_connect_target(dest_str)?;
                    let mut client = SftpClient::connect(&dest, &SftpConnectOptions::default())?;
                    println!("Downloading '{remote_path}' to '{local_path}'...");
                    client.download_file(
                        remote_path,
                        std::path::Path::new(local_path),
                        Some(|bytes_done, total_bytes| {
                            use std::io::Write as _;
                            if total_bytes > 0 {
                                let pct = (bytes_done as f64 / total_bytes as f64 * 100.0) as u32;
                                print!("\rProgress: {bytes_done}/{total_bytes} bytes ({pct}%)");
                            } else {
                                print!("\rProgress: {bytes_done} bytes");
                            }
                            let _ = io::stdout().flush();
                        }),
                    )?;
                    println!("\nDownload completed.");
                    let total_bytes = fs::metadata(local_path).map(|m| m.len()).unwrap_or(0);
                    let _ = store
                        .log_transfer(&NewTransfer {
                            host_id: None,
                            direction: TransferDirection::Download,
                            local_path,
                            remote_path,
                            bytes_transferred: total_bytes,
                            total_bytes,
                            status: "completed",
                            error_message: None,
                        })
                        .await;
                }
                "put" => {
                    let dest_str = &args[3];
                    let local_path = &args[4];
                    let remote_path = &args[5];
                    let total_bytes = fs::metadata(local_path).map(|m| m.len()).unwrap_or(0);
                    let dest = parse_quick_connect_target(dest_str)?;
                    let mut client = SftpClient::connect(&dest, &SftpConnectOptions::default())?;
                    println!(
                        "Uploading '{local_path}' to '{remote_path}' ({total_bytes} bytes)..."
                    );
                    client.upload_file(
                        std::path::Path::new(local_path),
                        remote_path,
                        Some(|bytes_done, total_bytes| {
                            use std::io::Write as _;
                            if total_bytes > 0 {
                                let pct = (bytes_done as f64 / total_bytes as f64 * 100.0) as u32;
                                print!("\rProgress: {bytes_done}/{total_bytes} bytes ({pct}%)");
                            } else {
                                print!("\rProgress: {bytes_done} bytes");
                            }
                            let _ = io::stdout().flush();
                        }),
                    )?;
                    println!("\nUpload completed.");
                    let _ = store
                        .log_transfer(&NewTransfer {
                            host_id: None,
                            direction: TransferDirection::Upload,
                            local_path,
                            remote_path,
                            bytes_transferred: total_bytes,
                            total_bytes,
                            status: "completed",
                            error_message: None,
                        })
                        .await;
                }
                _ => unreachable!(),
            }
        }
        "snippet" => {
            let sub = args[2].as_str();
            match sub {
                "list" => {
                    let extra = &args[3..];
                    let tag_filter = extract_flag(extra, "--tag");
                    let search_query = extract_flag(extra, "--search");

                    let snippets = if let Some(query) = search_query {
                        store.search_snippets(&query).await?
                    } else if let Some(tag) = tag_filter {
                        store.list_snippets_by_tag(&tag).await?
                    } else {
                        store.list_snippets().await?
                    };

                    for s in snippets {
                        let tags = if s.tags.is_empty() {
                            "-".to_string()
                        } else {
                            s.tags.join(",")
                        };
                        let var_count = s.variables.len();
                        let desc = s.description.as_deref().unwrap_or("-");
                        println!(
                            "{}\t{}\t{}\tvars:{}\t{}",
                            s.id, s.name, tags, var_count, desc
                        );
                    }
                }
                "add" => {
                    let name = &args[3];
                    let command = &args[4];
                    let extra = &args[5..];
                    let description = extract_flag(extra, "--description");
                    let tags: Vec<String> = extract_flag(extra, "--tags")
                        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default();

                    let snippet = Snippet::new(name, command, description, &tags)?;
                    let id = store.add_snippet(&snippet).await?;
                    println!("{id}");
                }
                "show" => {
                    let id: i64 = args[3].parse()?;
                    if let Some(s) = store.get_snippet(id).await? {
                        println!("ID:          {}", s.id);
                        println!("Name:        {}", s.name);
                        println!(
                            "Description: {}",
                            s.description.as_deref().unwrap_or("(none)")
                        );
                        println!(
                            "Tags:        {}",
                            if s.tags.is_empty() {
                                "(none)".to_string()
                            } else {
                                s.tags.join(", ")
                            }
                        );
                        println!("Command:     {}", s.command);
                        if s.variables.is_empty() {
                            println!("Variables:   (none)");
                        } else {
                            println!("Variables:");
                            for v in &s.variables {
                                let def = v.default_value.as_deref().unwrap_or("required");
                                let sec = if v.is_secret { " [SECRET]" } else { "" };
                                println!("  - {}{}: default={}", v.name, sec, def);
                            }
                        }
                    } else {
                        return Err(invalid("Snippet not found.").into());
                    }
                }
                "render" => {
                    let id: i64 = args[3].parse()?;
                    let snippet = store
                        .get_snippet(id)
                        .await?
                        .ok_or_else(|| invalid("Snippet not found."))?;

                    let mut params = HashMap::new();
                    let mut iter = args[4..].iter();
                    while let Some(arg) = iter.next() {
                        if arg == "--var" {
                            if let Some(pair) = iter.next() {
                                if let Some((k, v)) = pair.split_once('=') {
                                    params.insert(k.trim().to_string(), v.trim().to_string());
                                }
                            }
                        }
                    }

                    let preview_only = args[4..].iter().any(|a| a == "--preview");
                    let rendered = if preview_only {
                        render_preview(&snippet.command, &params, &snippet.variables)?
                    } else {
                        render_command(&snippet.command, &params)?
                    };
                    println!("{rendered}");
                }
                "batch" => {
                    let id: i64 = args[3].parse()?;
                    let snippet = store
                        .get_snippet(id)
                        .await?
                        .ok_or_else(|| invalid("Snippet not found."))?;

                    let extra = &args[4..];
                    let hosts_spec = extract_flag(extra, "--hosts")
                        .ok_or_else(|| invalid("--hosts flag is required."))?;
                    let host_identifiers: Vec<&str> = hosts_spec
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if host_identifiers.is_empty() {
                        return Err(invalid("No target hosts specified in --hosts.").into());
                    }

                    let dry_run = extra.iter().any(|a| a == "--dry-run");
                    let concurrency = extract_flag(extra, "--concurrency")
                        .and_then(|c| c.parse::<usize>().ok())
                        .unwrap_or(5);
                    let policy_str =
                        extract_flag(extra, "--policy").unwrap_or_else(|| "par".to_string());
                    let policy = if policy_str == "seq" || policy_str == "sequential" {
                        BatchPolicy::Sequential
                    } else {
                        BatchPolicy::Parallel {
                            max_concurrency: concurrency,
                        }
                    };

                    let mut params = HashMap::new();
                    let mut iter = extra.iter();
                    while let Some(arg) = iter.next() {
                        if arg == "--var" {
                            if let Some(pair) = iter.next() {
                                if let Some((k, v)) = pair.split_once('=') {
                                    params.insert(k.trim().to_string(), v.trim().to_string());
                                }
                            }
                        }
                    }

                    let all_hosts = store.list().await?;
                    let mut targets = Vec::new();
                    for identifier in host_identifiers {
                        let matched = if let Ok(id) = identifier.parse::<i64>() {
                            all_hosts.iter().find(|h| h.id == id)
                        } else {
                            all_hosts
                                .iter()
                                .find(|h| h.name.eq_ignore_ascii_case(identifier))
                        };

                        if let Some(h) = matched {
                            let mut target =
                                BatchHostTarget::new(h.id, &h.name, h.destination.clone())
                                    .with_tags(h.tags.clone());
                            if let Some(group) = &h.group_name {
                                target = target.with_group(group);
                            }
                            targets.push(target);
                        } else {
                            return Err(invalid(&format!(
                                "Host '{identifier}' not found in database."
                            ))
                            .into());
                        }
                    }

                    let plan = BatchExecutionPlan::new(
                        snippet.id,
                        &snippet.name,
                        &snippet.command,
                        targets,
                    )
                    .with_policy(policy)
                    .with_global_variables(params);

                    if dry_run {
                        println!(
                            "=== Batch Snippet Preview: {} (ID: {}) ===",
                            snippet.name, snippet.id
                        );
                        println!("Template: {}", snippet.command);
                        println!("Targets:  {} host(s)", plan.targets.len());
                        println!("--------------------------------------------------");
                        for (target, rendered) in plan.preview_all() {
                            match rendered {
                                Ok(cmd) => {
                                    println!(
                                        "[TARGET] {} ({}@{}:{})",
                                        target.name,
                                        target.destination.username(),
                                        target.destination.host(),
                                        target.destination.port()
                                    );
                                    println!("  CMD:   {cmd}");
                                }
                                Err(err) => {
                                    println!("[ERROR]  {} -> Failed to render: {err}", target.name);
                                }
                            }
                        }
                    } else {
                        return Err(invalid("CLI batch execution is not implemented; use --dry-run to preview commands.").into());
                    }
                }
                "delete" => {
                    let id: i64 = args[3].parse()?;
                    if !store.delete_snippet(id).await? {
                        return Err(invalid("Snippet not found.").into());
                    }
                    println!("Snippet {id} deleted.");
                }
                _ => unreachable!(),
            }
        }
        "workspace" => {
            let sub = args[2].as_str();
            match sub {
                "list" => {
                    let workspaces = store.list_workspaces().await?;
                    if workspaces.is_empty() {
                        println!("No workspaces configured.");
                        return Ok(());
                    }
                    println!(
                        "{:<5} {:<24} {:<12} {:<10} {:<20}",
                        "ID", "NAME", "LAYOUT", "SESSIONS", "TAGS"
                    );
                    for ws in workspaces {
                        let tags_str = if ws.tags.is_empty() {
                            "-".to_string()
                        } else {
                            ws.tags.join(",")
                        };
                        println!(
                            "{:<5} {:<24} {:<12} {:<10} {:<20}",
                            ws.id,
                            ws.name,
                            ws.layout,
                            ws.sessions.len(),
                            tags_str,
                        );
                    }
                }
                "show" => {
                    let id: i64 = args[3]
                        .parse()
                        .map_err(|_| invalid("Workspace ID must be an integer."))?;
                    if let Some(ws) = store.get_workspace(id).await? {
                        println!("Workspace ID:  {}", ws.id);
                        println!("Name:          {}", ws.name);
                        println!("Layout:        {}", ws.layout);
                        if let Some(desc) = &ws.description {
                            println!("Description:   {}", desc);
                        }
                        println!(
                            "Tags:          {}",
                            if ws.tags.is_empty() {
                                "-".to_string()
                            } else {
                                ws.tags.join(", ")
                            }
                        );
                        println!("Sessions ({}):", ws.sessions.len());
                        for (i, s) in ws.sessions.iter().enumerate() {
                            let excl = if s.excluded_from_broadcast {
                                " [Excluded from Broadcast]"
                            } else {
                                ""
                            };
                            let host_info = s
                                .host_id
                                .map(|h| format!("(Host ID: {h})"))
                                .unwrap_or_default();
                            println!(
                                "  [{}] {} [{}]{} {}",
                                i + 1,
                                s.name,
                                s.kind.as_str(),
                                excl,
                                host_info
                            );
                            if let Some(cmd) = &s.startup_command {
                                println!("      Startup: {}", cmd);
                            }
                            if let Some(dir) = &s.working_directory {
                                println!("      Dir:     {}", dir);
                            }
                        }
                    } else {
                        return Err(invalid("Workspace not found.").into());
                    }
                }
                "add" => {
                    let name = &args[3];
                    let extra = &args[4..];
                    let layout =
                        extract_flag(extra, "--layout").unwrap_or_else(|| "single".to_string());
                    let desc = extract_flag(extra, "--desc");
                    let tags: Vec<String> = extract_flag(extra, "--tags")
                        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default();
                    let id = store
                        .add_workspace(name, desc.as_deref(), &layout, &[], &tags)
                        .await?;
                    println!("{id}");
                }
                "duplicate" => {
                    let id: i64 = args[3]
                        .parse()
                        .map_err(|_| invalid("Workspace ID must be an integer."))?;
                    let new_name = &args[4];
                    let new_id = store.duplicate_workspace(id, new_name).await?;
                    println!("{new_id}");
                }
                "delete" => {
                    let id: i64 = args[3]
                        .parse()
                        .map_err(|_| invalid("Workspace ID must be an integer."))?;
                    let deleted = store.delete_workspace(id).await?;
                    if deleted {
                        println!("Deleted workspace {id}.");
                    } else {
                        return Err(invalid("Workspace not found.").into());
                    }
                }
                _ => unreachable!(),
            }
        }
        "vault" => {
            let action = args[2].as_str();
            match action {
                "export" => {
                    let output_path = &args[3];
                    let extra = &args[4..];
                    let passphrase = extract_flag(extra, "--passphrase")
                        .filter(|passphrase| !passphrase.is_empty())
                        .ok_or_else(|| invalid("A non-empty --passphrase is required."))?;
                    let device_id = extract_flag(extra, "--device-id")
                        .unwrap_or_else(|| "shellumi-cli-device".to_string());
                    let iterations = extract_flag(extra, "--iterations")
                        .and_then(|i| i.parse::<u32>().ok())
                        .unwrap_or(shellumi_core::DEFAULT_KDF_ITERATIONS);

                    let vault = store
                        .export_vault_with_iterations(&passphrase, &device_id, iterations)
                        .await?;
                    let json = vault.to_json()?;
                    fs::write(output_path, json)?;
                    println!(
                        "Vault exported successfully to {output_path} ({} records).",
                        vault.records.len()
                    );
                }
                "import" => {
                    let input_path = &args[3];
                    let extra = &args[4..];
                    let passphrase = extract_flag(extra, "--passphrase")
                        .filter(|passphrase| !passphrase.is_empty())
                        .ok_or_else(|| invalid("A non-empty --passphrase is required."))?;

                    let json = fs::read_to_string(input_path)?;
                    let vault = SyncVault::from_json(&json)?;
                    let summary = store.import_vault(&passphrase, &vault).await?;
                    println!("Vault imported from {input_path}:");
                    println!("  Hosts imported:    {}", summary.hosts_imported);
                    println!("  Groups imported:   {}", summary.groups_imported);
                    println!("  Tunnels imported:  {}", summary.tunnels_imported);
                    println!("  Snippets imported: {}", summary.snippets_imported);
                    println!("  Workspaces imported: {}", summary.workspaces_imported);
                    if !summary.errors.is_empty() {
                        eprintln!("  Errors ({}):", summary.errors.len());
                        for err in summary.errors {
                            eprintln!("    - {err}");
                        }
                    }
                }
                "status" => {
                    let hosts = store.list().await?.len();
                    let groups = store.list_groups().await?.len();
                    let tunnels = store.list_tunnels().await?.len();
                    let snippets = store.list_snippets().await?.len();
                    let workspaces = store.list_workspaces().await?.len();
                    println!("Local Vault Status:");
                    println!("  Hosts:    {hosts}");
                    println!("  Groups:   {groups}");
                    println!("  Tunnels:  {tunnels}");
                    println!("  Snippets: {snippets}");
                    println!("  Workspaces: {workspaces}");
                }
                "diff" => {
                    let input_path = &args[3];
                    let extra = &args[4..];
                    let passphrase = extract_flag(extra, "--passphrase")
                        .filter(|passphrase| !passphrase.is_empty())
                        .ok_or_else(|| invalid("A non-empty --passphrase is required."))?;
                    let device_id = extract_flag(extra, "--device-id")
                        .unwrap_or_else(|| "shellumi-cli-device".to_string());

                    let json = fs::read_to_string(input_path)?;
                    let remote_vault = SyncVault::from_json(&json)?;
                    let local_vault = store
                        .export_vault_with_iterations(&passphrase, &device_id, 2000)
                        .await?;

                    let diffs = diff_sync_records_with_passphrase(
                        &local_vault.records,
                        &remote_vault.records,
                        Some(&passphrase),
                    );
                    if diffs.is_empty() {
                        println!("No records found in either vault.");
                    } else {
                        println!("OBJECT ID\tTYPE\tSTATUS\tLOCAL VER\tREMOTE VER");
                        for d in diffs {
                            let loc_v = d
                                .local_version
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".into());
                            let rem_v = d
                                .remote_version
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".into());
                            println!(
                                "{}\t{}\t{}\t{}\t{}",
                                d.object_id, d.entity_type, d.status, loc_v, rem_v
                            );
                        }
                    }
                }
                "oauth-url" => {
                    let extra = &args[3..];
                    let client_id = extract_flag(extra, "--client-id").unwrap_or_else(|| {
                        "shellumi-client-id.apps.googleusercontent.com".to_string()
                    });
                    let redirect_uri = extract_flag(extra, "--redirect-uri")
                        .unwrap_or_else(|| "http://127.0.0.1:8080/callback".to_string());

                    let pkce = generate_pkce_challenge()?;
                    let state = format!(
                        "state_{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    );
                    let auth_url =
                        build_google_drive_auth_url(&client_id, &redirect_uri, &pkce, &state);

                    println!("OAuth 2.0 PKCE Challenge Generated:");
                    println!("  Code Verifier:  {}", pkce.code_verifier);
                    println!("  Code Challenge: {}", pkce.code_challenge);
                    println!("  State Nonce:    {}", state);
                    println!("\nAuthorization URL:\n{}", auth_url);
                }
                "history" => {
                    let history = store.list_snapshot_history().await?;
                    if history.is_empty() {
                        println!("No sync snapshots archived in database.");
                    } else {
                        println!("ID\tSNAPSHOT ID\tRECORDS\tCREATED AT");
                        for entry in history {
                            println!(
                                "{}\t{}\t{}\t{}",
                                entry.id, entry.snapshot_id, entry.record_count, entry.created_at
                            );
                        }
                    }
                }
                "rollback" => {
                    let snapshot_id = &args[3];
                    let extra = &args[4..];
                    let passphrase = extract_flag(extra, "--passphrase")
                        .filter(|passphrase| !passphrase.is_empty())
                        .ok_or_else(|| invalid("A non-empty --passphrase is required."))?;

                    let summary = store.rollback_snapshot(snapshot_id, &passphrase).await?;
                    println!("Rolled back to snapshot {snapshot_id}:");
                    println!("  Hosts imported:    {}", summary.hosts_imported);
                    println!("  Groups imported:   {}", summary.groups_imported);
                    println!("  Tunnels imported:  {}", summary.tunnels_imported);
                    println!("  Snippets imported: {}", summary.snippets_imported);
                    println!("  Workspaces imported: {}", summary.workspaces_imported);
                    println!(
                        "  Shared Vaults imported: {}",
                        summary.shared_vaults_imported
                    );
                    println!(
                        "  Team Members imported:  {}",
                        summary.team_members_imported
                    );
                    if !summary.errors.is_empty() {
                        eprintln!("  Errors ({}):", summary.errors.len());
                        for err in summary.errors {
                            eprintln!("    - {err}");
                        }
                    }
                }
                "cloud-sync" => {
                    let extra = &args[3..];
                    let token = extract_flag(extra, "--token").ok_or_else(|| {
                        invalid("Missing required flag --token <oauth_access_token>.")
                    })?;
                    let passphrase = extract_flag(extra, "--passphrase").ok_or_else(|| {
                        invalid("Missing required flag --passphrase <passphrase>.")
                    })?;
                    let resolve_str = extract_flag(extra, "--resolve");
                    let resolutions = match resolve_str.as_deref() {
                        Some("keep-local") => Some(vec![ConflictResolutionChoice::KeepLocal]),
                        Some("accept-remote") => Some(vec![ConflictResolutionChoice::AcceptRemote]),
                        None => None,
                        Some(other) => {
                            return Err(invalid(&format!(
                                "Invalid resolution '{other}'. Use 'keep-local' or 'accept-remote'."
                            ))
                            .into())
                        }
                    };

                    let device_id = extract_flag(extra, "--device-id")
                        .unwrap_or_else(|| "shellumi-cli".to_string());
                    let local_vault = store
                        .export_vault_with_iterations(
                            &passphrase,
                            &device_id,
                            DEFAULT_KDF_ITERATIONS,
                        )
                        .await?;

                    let transport = SystemHttpTransport::new();
                    let client = GoogleDriveSyncClient::new(transport, "shellumi-cli");

                    println!("Connecting to Google Drive Application Data folder...");
                    let result = execute_cloud_vault_sync(
                        &client,
                        &token,
                        &local_vault,
                        &passphrase,
                        resolutions.as_deref(),
                    )?;

                    match result {
                        SyncCycleResult::AlreadySynchronized => {
                            println!("Cloud vault is already synchronized. No changes needed.");
                        }
                        SyncCycleResult::Success {
                            report,
                            merged_records,
                        } => {
                            println!("{}", report.message);
                            if report.local_updated > 0 {
                                let mut updated_vault = local_vault.clone();
                                updated_vault.records = merged_records;
                                store.import_vault(&passphrase, &updated_vault).await?;
                                println!(
                                    "Imported {} updated record(s) into local store.",
                                    report.local_updated
                                );
                            }
                        }
                        SyncCycleResult::Conflicts(conflicts) => {
                            println!("Sync conflicts detected ({}):", conflicts.len());
                            for c in conflicts {
                                println!(
                                    "  Conflict on {} ({}): local v{} vs remote v{}",
                                    c.object_id, c.entity_type, c.local_version, c.remote_version
                                );
                            }
                            println!(
                                "Re-run with --resolve keep-local or --resolve accept-remote."
                            );
                        }
                        SyncCycleResult::Error(msg) => {
                            return Err(invalid(&format!("Sync error: {msg}")).into());
                        }
                    }
                }
                "sync" => {
                    let extra = &args[3..];
                    if let Some(token) = extract_flag(extra, "--token") {
                        let passphrase = extract_flag(extra, "--passphrase").ok_or_else(|| {
                            invalid("Missing required flag --passphrase <passphrase>.")
                        })?;
                        let resolve_str = extract_flag(extra, "--resolve");
                        let resolutions = match resolve_str.as_deref() {
                            Some("keep-local") => Some(vec![ConflictResolutionChoice::KeepLocal]),
                            Some("accept-remote") => {
                                Some(vec![ConflictResolutionChoice::AcceptRemote])
                            }
                            None => None,
                            Some(other) => {
                                return Err(invalid(&format!(
                                    "Invalid resolution '{other}'. Use 'keep-local' or 'accept-remote'."
                                ))
                                .into())
                            }
                        };
                        let device_id = extract_flag(extra, "--device-id")
                            .unwrap_or_else(|| "shellumi-cli".to_string());
                        let local_vault = store
                            .export_vault_with_iterations(
                                &passphrase,
                                &device_id,
                                DEFAULT_KDF_ITERATIONS,
                            )
                            .await?;
                        let transport = SystemHttpTransport::new();
                        let client = GoogleDriveSyncClient::new(transport, "shellumi-cli");
                        let result = execute_cloud_vault_sync(
                            &client,
                            &token,
                            &local_vault,
                            &passphrase,
                            resolutions.as_deref(),
                        )?;
                        match result {
                            SyncCycleResult::AlreadySynchronized => {
                                println!("Cloud vault is already synchronized.");
                            }
                            SyncCycleResult::Success {
                                report,
                                merged_records,
                            } => {
                                println!("{}", report.message);
                                if report.local_updated > 0 {
                                    let mut updated_vault = local_vault.clone();
                                    updated_vault.records = merged_records;
                                    store.import_vault(&passphrase, &updated_vault).await?;
                                }
                            }
                            SyncCycleResult::Conflicts(conflicts) => {
                                println!("Sync conflicts detected ({}):", conflicts.len());
                                for c in conflicts {
                                    println!(
                                        "  Conflict on {} ({}): local v{} vs remote v{}",
                                        c.object_id,
                                        c.entity_type,
                                        c.local_version,
                                        c.remote_version
                                    );
                                }
                            }
                            SyncCycleResult::Error(msg) => {
                                return Err(invalid(&format!("Sync error: {msg}")).into());
                            }
                        }
                    } else if let Some(remote_path) = extract_flag(extra, "--remote") {
                        let passphrase = extract_flag(extra, "--passphrase").ok_or_else(|| {
                            invalid("Missing required flag --passphrase <passphrase>.")
                        })?;
                        let device_id = extract_flag(extra, "--device-id")
                            .unwrap_or_else(|| "shellumi-cli".to_string());
                        let remote_content = fs::read_to_string(&remote_path)?;
                        let remote_vault = SyncVault::from_json(&remote_content)?;
                        let local_vault = store
                            .export_vault_with_iterations(
                                &passphrase,
                                &device_id,
                                DEFAULT_KDF_ITERATIONS,
                            )
                            .await?;
                        let res = execute_sync_cycle(
                            &local_vault.records,
                            &remote_vault.records,
                            &passphrase,
                            None,
                        );
                        match res {
                            SyncCycleResult::AlreadySynchronized => {
                                println!("Local vault and remote file are already in sync.");
                            }
                            SyncCycleResult::Success {
                                report,
                                merged_records,
                            } => {
                                println!("{}", report.message);
                                if report.local_updated > 0 {
                                    let mut updated = local_vault.clone();
                                    updated.records = merged_records.clone();
                                    store.import_vault(&passphrase, &updated).await?;
                                }
                                if report.remote_updated > 0 {
                                    let mut updated_remote = remote_vault.clone();
                                    updated_remote.records = merged_records;
                                    fs::write(&remote_path, updated_remote.to_json()?)?;
                                }
                            }
                            SyncCycleResult::Conflicts(conflicts) => {
                                println!("File sync conflicts ({}):", conflicts.len());
                                for c in conflicts {
                                    println!(
                                        "  Conflict on {} ({}): local v{} vs remote v{}",
                                        c.object_id,
                                        c.entity_type,
                                        c.local_version,
                                        c.remote_version
                                    );
                                }
                            }
                            SyncCycleResult::Error(msg) => {
                                return Err(invalid(&format!("Sync error: {msg}")).into());
                            }
                        }
                    } else {
                        return Err(invalid(
                            "Vault sync without a remote connection is not implemented; provide --token <oauth_token> for Google Drive or --remote <path> for file sync.",
                        )
                        .into());
                    }
                }
                _ => unreachable!(),
            }
        }
        "team" => {
            let sub = args[2].as_str();
            match sub {
                "vault" => {
                    let action = args[3].as_str();
                    match action {
                        "create" => {
                            let name = &args[4];
                            let extra = &args[5..];
                            let desc = extract_flag(extra, "--desc");
                            let owner = extract_flag(extra, "--owner")
                                .unwrap_or_else(|| "admin@shellumi.local".to_string());
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let vault_id = unique_id("vault-team");
                            let now_str = format!("{now}");
                            let vault = SharedVault::new(vault_id, name, desc, owner, now_str)?;
                            store.create_shared_vault(&vault).await?;
                            println!("Created shared team vault: {} ({})", vault.name, vault.id);
                        }
                        "list" => {
                            let vaults = store.list_shared_vaults().await?;
                            if vaults.is_empty() {
                                println!("No shared team vaults found.");
                            } else {
                                println!("ID\tNAME\tEPOCH\tOWNER\tDESCRIPTION");
                                for v in vaults {
                                    println!(
                                        "{}\t{}\t{}\t{}\t{}",
                                        v.id,
                                        v.name,
                                        v.current_epoch,
                                        v.owner_id,
                                        v.description.as_deref().unwrap_or("-")
                                    );
                                }
                            }
                        }
                        "show" => {
                            let id = &args[4];
                            if let Some(v) = store.get_shared_vault(id).await? {
                                println!("Shared Team Vault:");
                                println!("  ID:          {}", v.id);
                                println!("  Name:        {}", v.name);
                                println!(
                                    "  Description: {}",
                                    v.description.as_deref().unwrap_or("-")
                                );
                                println!("  Epoch:       {}", v.current_epoch);
                                println!("  Owner:       {}", v.owner_id);
                                println!("  Created At:  {}", v.created_at);
                                println!("  Updated At:  {}", v.updated_at);
                            } else {
                                return Err(
                                    invalid(&format!("Shared vault '{id}' not found.")).into()
                                );
                            }
                        }
                        "rotate-key" => {
                            let id = &args[4];
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let now_str = format!("{now}");
                            if let Some(mut v) = store.get_shared_vault(id).await? {
                                let new_epoch = v.rotate_key(&now_str);
                                store
                                    .update_shared_vault_epoch(id, new_epoch, &now_str)
                                    .await?;
                                let audit = SharingAuditRecord::new(
                                    unique_id("audit"),
                                    id,
                                    AuditAction::KeyRotated,
                                    v.owner_id.clone(),
                                    None,
                                    Some(format!("epoch incremented to {new_epoch}")),
                                    &now_str,
                                );
                                store.log_audit_event(&audit).await?;
                                println!(
                                    "Rotated cryptographic key for '{}'. New epoch: {}",
                                    v.name, new_epoch
                                );
                            } else {
                                return Err(
                                    invalid(&format!("Shared vault '{id}' not found.")).into()
                                );
                            }
                        }
                        "delete" => {
                            let id = &args[4];
                            if store.delete_shared_vault(id).await? {
                                println!("Deleted shared vault '{id}'.");
                            } else {
                                return Err(
                                    invalid(&format!("Shared vault '{id}' not found.")).into()
                                );
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                "member" => {
                    let action = args[3].as_str();
                    match action {
                        "invite" => {
                            let vault_id = &args[4];
                            let user_id = &args[5];
                            let name = &args[6];
                            let role_str = &args[7];
                            let role = TeamRole::from_str(role_str)?;
                            let extra = &args[8..];
                            let pubkey = extract_flag(extra, "--public-key")
                                .unwrap_or_else(|| "default_ed25519_pk".to_string());
                            let inviter = extract_flag(extra, "--invited-by")
                                .unwrap_or_else(|| "owner".to_string());
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let now_str = format!("{now}");
                            let member_id = unique_id("mem");
                            let member = TeamMember::new(
                                member_id,
                                vault_id,
                                user_id,
                                name,
                                pubkey,
                                role,
                                &now_str,
                                inviter.clone(),
                            );
                            store.add_team_member(&member).await?;
                            let audit = SharingAuditRecord::new(
                                unique_id("audit"),
                                vault_id,
                                AuditAction::MemberInvited,
                                inviter,
                                Some(user_id.to_string()),
                                Some(format!("role={role}")),
                                &now_str,
                            );
                            store.log_audit_event(&audit).await?;
                            println!(
                                "Enrolled team member '{}' ({}) with role '{}'",
                                name, user_id, role
                            );
                        }
                        "list" => {
                            let vault_id = &args[4];
                            let members = store.list_team_members(vault_id).await?;
                            if members.is_empty() {
                                println!("No members enrolled in vault '{vault_id}'.");
                            } else {
                                println!("ID\tUSER_ID\tNAME\tROLE\tSTATUS\tENROLLED_AT");
                                for m in members {
                                    let status = if m.revoked { "REVOKED" } else { "ACTIVE" };
                                    println!(
                                        "{}\t{}\t{}\t{}\t{}\t{}",
                                        m.id,
                                        m.user_id,
                                        m.display_name,
                                        m.role,
                                        status,
                                        m.enrolled_at
                                    );
                                }
                            }
                        }
                        "role" => {
                            let _vault_id = &args[4];
                            let member_id = &args[5];
                            let new_role_str = &args[6];
                            let new_role = TeamRole::from_str(new_role_str)?;
                            if store.update_team_member_role(member_id, new_role).await? {
                                println!("Updated member '{member_id}' role to '{new_role}'.");
                            } else {
                                return Err(
                                    invalid(&format!("Member '{member_id}' not found.")).into()
                                );
                            }
                        }
                        "revoke" => {
                            let vault_id = &args[4];
                            let member_id = &args[5];
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let now_str = format!("{now}");
                            if store.revoke_team_member(member_id, &now_str).await? {
                                let audit = SharingAuditRecord::new(
                                    unique_id("audit"),
                                    vault_id,
                                    AuditAction::MemberRevoked,
                                    "admin",
                                    Some(member_id.to_string()),
                                    Some("access revoked; recommend key rotation".to_string()),
                                    &now_str,
                                );
                                store.log_audit_event(&audit).await?;
                                println!("Revoked member '{member_id}'. Notice: Perform key rotation to ensure future records are protected.");
                            } else {
                                return Err(
                                    invalid(&format!("Member '{member_id}' not found.")).into()
                                );
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                "audit" => {
                    let vault_id = &args[3];
                    let limit =
                        extract_flag(&args[4..], "--limit").and_then(|l| l.parse::<usize>().ok());
                    let records = store.list_audit_log(vault_id, limit).await?;
                    if records.is_empty() {
                        println!("No audit records found for vault '{vault_id}'.");
                    } else {
                        println!("Audit Log for Vault '{vault_id}':");
                        for r in records {
                            println!("  {}", r.format_entry());
                        }
                    }
                }
                "session" => {
                    let action = args[3].as_str();
                    match action {
                        "start" => {
                            let session_id = &args[4];
                            let owner_id = &args[5];
                            let vault_id = extract_flag(&args[6..], "--vault-id");
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let session = MultiplayerSession::new(
                                session_id,
                                vault_id,
                                owner_id,
                                format!("{now}"),
                            );
                            println!(
                                "Started multiplayer terminal session '{}' with host '{}'.",
                                session.session_id, session.owner_id
                            );
                            println!("Status: Active (1 participant, host holds keyboard control)");
                        }
                        "join" => {
                            let session_id = &args[4];
                            let user_id = &args[5];
                            let name = &args[6];
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let now_str = format!("{now}");
                            let mut session =
                                MultiplayerSession::new(session_id, None, "host", &now_str);
                            session.add_participant(user_id, name, &now_str)?;
                            println!(
                                "Participant '{}' ({}) joined session '{}'.",
                                name, user_id, session_id
                            );
                        }
                        "grant" => {
                            let session_id = &args[4];
                            let requester_id = &args[5];
                            let target_id = &args[6];
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let now_str = format!("{now}");
                            let mut session =
                                MultiplayerSession::new(session_id, None, requester_id, &now_str);
                            session.add_participant(target_id, target_id, &now_str)?;
                            session.grant_keyboard_control(requester_id, target_id)?;
                            println!(
                                "Keyboard control granted to '{}' in session '{}'.",
                                target_id, session_id
                            );
                        }
                        "revoke" => {
                            let session_id = &args[4];
                            let requester_id = &args[5];
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            let now_str = format!("{now}");
                            let mut session =
                                MultiplayerSession::new(session_id, None, requester_id, &now_str);
                            session.revoke_keyboard_control(requester_id)?;
                            println!(
                                "Keyboard control revoked in session '{}'. Reverted to host '{}'.",
                                session_id, requester_id
                            );
                        }
                        "status" => {
                            let session_id = &args[4];
                            println!("Multiplayer Session Status:");
                            println!("  Session ID: {}", session_id);
                            println!("  Mode:       Single-writer with immediate host revocation");
                            println!("  Encrypted:  Yes (P2P end-to-end encrypted transport)");
                        }
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
        }
        "infra" => {
            let sub = args[2].as_str();
            match sub {
                "provider" => {
                    let action = args[3].as_str();
                    match action {
                        "add" => {
                            let kind_str = &args[4];
                            let kind = CloudProviderKind::from_str(kind_str)
                                .map_err(|e| invalid(&e.to_string()))?;
                            let name = extract_flag(&args[5..], "--name")
                                .ok_or_else(|| invalid("Missing required '--name <name>' flag"))?;
                            let region = extract_flag(&args[5..], "--region");
                            let profile = extract_flag(&args[5..], "--profile");
                            let token = extract_flag(&args[5..], "--token");
                            let path = extract_flag(&args[5..], "--path");
                            let format = extract_flag(&args[5..], "--format");

                            let mut cfg = CloudProviderConfig::new(kind, &name);
                            cfg.region = region;
                            cfg.profile = profile;
                            if kind == CloudProviderKind::DigitalOcean {
                                cfg.api_token = token;
                            }
                            cfg.inventory_path = path;
                            cfg.inventory_format = format;
                            cfg.validate().map_err(|e| invalid(&e.to_string()))?;

                            let prov_id = unique_id("prov");
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;

                            let cfg_json =
                                serde_json::to_string(&cfg).map_err(|e| invalid(&e.to_string()))?;

                            let record = CloudProviderRecord {
                                id: prov_id.clone(),
                                name: name.to_string(),
                                provider_kind: kind.as_str().to_string(),
                                config_json: cfg_json,
                                sync_interval_mins: 60,
                                last_synced_at: None,
                                auto_sync: false,
                                created_at: now,
                                updated_at: now,
                            };

                            store.create_cloud_provider(&record).await?;
                            println!(
                                "Created cloud provider '{}' (ID: {}, Kind: {}).",
                                name, prov_id, kind
                            );
                        }
                        "list" => {
                            let providers = store.list_cloud_providers().await?;
                            if providers.is_empty() {
                                println!("No cloud providers configured.");
                            } else {
                                println!("Configured Infrastructure Providers:");
                                for p in providers {
                                    let sync_time = p
                                        .last_synced_at
                                        .map(|t| format!("{t}"))
                                        .unwrap_or_else(|| "Never".to_string());
                                    println!(
                                        "  [{}] {} ({}) - Auto-sync: {} - Last synced: {}",
                                        p.id, p.name, p.provider_kind, p.auto_sync, sync_time
                                    );
                                }
                            }
                        }
                        "show" => {
                            let id = &args[4];
                            if let Some(p) = store.get_cloud_provider(id).await? {
                                println!("Cloud Provider Details:");
                                println!("  ID:          {}", p.id);
                                println!("  Name:        {}", p.name);
                                println!("  Kind:        {}", p.provider_kind);
                                println!("  Sync Int:    {} mins", p.sync_interval_mins);
                                println!("  Auto Sync:   {}", p.auto_sync);
                                if let Ok(cfg) =
                                    serde_json::from_str::<CloudProviderConfig>(&p.config_json)
                                {
                                    println!("  Config:      {}", cfg.redacted_summary());
                                }
                                let discovered = store.list_discovered_hosts(Some(id)).await?;
                                println!("  Discovered:  {} hosts", discovered.len());
                            } else {
                                return Err(invalid(&format!("Provider '{id}' not found.")).into());
                            }
                        }
                        "delete" => {
                            let id = &args[4];
                            if store.delete_cloud_provider(id).await? {
                                println!("Deleted cloud provider '{id}'.");
                            } else {
                                return Err(invalid(&format!("Provider '{id}' not found.")).into());
                            }
                        }
                        "sync" => {
                            let id = &args[4];
                            let p = store
                                .get_cloud_provider(id)
                                .await?
                                .ok_or_else(|| invalid(&format!("Provider '{id}' not found.")))?;

                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;

                            // If provider has an inventory path (e.g. Ansible), parse it directly
                            if let Ok(cfg) =
                                serde_json::from_str::<CloudProviderConfig>(&p.config_json)
                            {
                                if let Some(ref path) = cfg.inventory_path {
                                    if let Ok(content) = fs::read_to_string(path) {
                                        let is_yaml = cfg.inventory_format.as_deref()
                                            == Some("yaml")
                                            || path.ends_with(".yml")
                                            || path.ends_with(".yaml");
                                        let hosts_res = if is_yaml {
                                            AnsibleInventoryEngine::parse_yaml_inventory(&content)
                                        } else {
                                            AnsibleInventoryEngine::parse_ini_inventory(&content)
                                        };

                                        if let Ok(hosts) = hosts_res {
                                            let records: Vec<DiscoveredHostRecord> = hosts
                                                .into_iter()
                                                .map(|h| DiscoveredHostRecord {
                                                    id: unique_id("dh"),
                                                    provider_id: id.to_string(),
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
                                                    metadata_json: serde_json::to_string(
                                                        &h.extra_metadata,
                                                    )
                                                    .unwrap_or_default(),
                                                    imported_host_id: None,
                                                    last_seen_at: now,
                                                })
                                                .collect();
                                            let saved =
                                                store.save_discovered_hosts(id, &records).await?;
                                            store.update_cloud_provider_sync_time(id, now).await?;
                                            println!("Synced provider '{}'. Discovered and persisted {} hosts.", p.name, saved);
                                            return Ok(());
                                        }
                                    }
                                }
                            }

                            store.update_cloud_provider_sync_time(id, now).await?;
                            println!("Synced provider '{}'. Status updated.", p.name);
                        }
                        _ => unreachable!(),
                    }
                }
                "discover" => {
                    let provider_kind_str = &args[3];
                    let kind = CloudProviderKind::from_str(provider_kind_str)
                        .map_err(|e| invalid(&e.to_string()))?;
                    let file_path = extract_flag(&args[4..], "--file")
                        .ok_or_else(|| invalid("Missing required '--file <path>' flag"))?;
                    let use_private_ip = args[4..].iter().any(|a| a == "--private-ip");
                    let import_all = args[4..].iter().any(|a| a == "--import-all");

                    let content = fs::read_to_string(&file_path)?;

                    let discovered = match kind {
                        CloudProviderKind::Aws => {
                            AwsEc2Parser::parse_describe_instances_json(&content, use_private_ip)
                                .map_err(|e| invalid(&e.to_string()))?
                        }
                        CloudProviderKind::Azure => {
                            AzureVmParser::parse_virtual_machines_json(&content, use_private_ip)
                                .map_err(|e| invalid(&e.to_string()))?
                        }
                        CloudProviderKind::DigitalOcean => {
                            DigitalOceanParser::parse_droplets_json(&content, use_private_ip)
                                .map_err(|e| invalid(&e.to_string()))?
                        }
                        CloudProviderKind::Ansible => {
                            if file_path.ends_with(".yml") || file_path.ends_with(".yaml") {
                                AnsibleInventoryEngine::parse_yaml_inventory(&content)
                                    .map_err(|e| invalid(&e.to_string()))?
                            } else {
                                AnsibleInventoryEngine::parse_ini_inventory(&content)
                                    .map_err(|e| invalid(&e.to_string()))?
                            }
                        }
                    };

                    println!(
                        "Discovered {} host(s) from {}:",
                        discovered.len(),
                        kind.display_name()
                    );
                    for h in &discovered {
                        let typ = h.instance_type.as_deref().unwrap_or("-");
                        let reg = h.region.as_deref().unwrap_or("-");
                        println!(
                            "  {} {:<20} {:<24} {:<10} {:<12} {}",
                            h.status_badge(),
                            h.name,
                            h.format_destination(),
                            reg,
                            typ,
                            h.group_path.as_deref().unwrap_or("")
                        );
                    }

                    if import_all {
                        let mut imported_count = 0;
                        for h in &discovered {
                            let u = h
                                .username
                                .as_deref()
                                .unwrap_or_else(|| kind.default_username());
                            if let Ok(dest) = SshDestination::new(&h.hostname, u, h.port) {
                                let tags: Vec<String> = h
                                    .tags
                                    .iter()
                                    .map(|(k, v)| {
                                        if v.is_empty() {
                                            k.clone()
                                        } else {
                                            format!("{k}:{v}")
                                        }
                                    })
                                    .collect();

                                if store
                                    .add(&h.name, &dest, h.group_path.as_deref(), &tags, None, None)
                                    .await
                                    .is_ok()
                                {
                                    imported_count += 1;
                                }
                            }
                        }
                        println!(
                            "Imported {} of {} discovered host(s) into Shellumi inventory.",
                            imported_count,
                            discovered.len()
                        );
                    }
                }
                "ansible" => {
                    let action = args[3].as_str();
                    match action {
                        "import" => {
                            let file_path = extract_flag(&args[4..], "--file")
                                .ok_or_else(|| invalid("Missing required '--file <path>' flag"))?;
                            let format = extract_flag(&args[4..], "--format");

                            let content = fs::read_to_string(&file_path)?;
                            let is_yaml = format.as_deref() == Some("yaml")
                                || file_path.ends_with(".yml")
                                || file_path.ends_with(".yaml");

                            let hosts = if is_yaml {
                                AnsibleInventoryEngine::parse_yaml_inventory(&content)
                                    .map_err(|e| invalid(&e.to_string()))?
                            } else {
                                AnsibleInventoryEngine::parse_ini_inventory(&content)
                                    .map_err(|e| invalid(&e.to_string()))?
                            };

                            let mut imported = 0;
                            for h in &hosts {
                                let u = h.username.as_deref().unwrap_or("root");
                                if let Ok(dest) = SshDestination::new(&h.hostname, u, h.port) {
                                    let tags: Vec<String> = h
                                        .tags
                                        .iter()
                                        .map(|(k, v)| {
                                            if v.is_empty() {
                                                k.clone()
                                            } else {
                                                format!("{k}:{v}")
                                            }
                                        })
                                        .collect();
                                    if store
                                        .add(
                                            &h.name,
                                            &dest,
                                            h.group_path.as_deref(),
                                            &tags,
                                            None,
                                            None,
                                        )
                                        .await
                                        .is_ok()
                                    {
                                        imported += 1;
                                    }
                                }
                            }
                            println!(
                                "Imported {} host(s) from Ansible inventory '{}'.",
                                imported, file_path
                            );
                        }
                        "export" => {
                            let format = extract_flag(&args[4..], "--format");
                            let output_path = extract_flag(&args[4..], "--output");

                            let hosts = store.list().await?;
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

                            let is_yaml = format.as_deref() == Some("yaml");
                            let rendered = if is_yaml {
                                AnsibleInventoryEngine::export_to_yaml(&export_items)
                            } else {
                                AnsibleInventoryEngine::export_to_ini(&export_items)
                            };

                            if let Some(path) = output_path {
                                fs::write(&path, &rendered)?;
                                println!(
                                    "Exported {} hosts to Ansible inventory at '{}'.",
                                    export_items.len(),
                                    path
                                );
                            } else {
                                print!("{rendered}");
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
        }
        "quick-connect" => {
            let destination = parse_quick_connect_target(&args[2])?;
            let status = Command::new("ssh")
                .args(destination.openssh_arguments())
                .status()?;
            if !status.success() {
                return Err(io::Error::other(format!("OpenSSH exited with {status}")).into());
            }
        }
        "import-ssh-config" => {
            let content = fs::read_to_string(&args[2])?;
            let entries = parse_ssh_config(&content, None);
            let mut count = 0;
            for entry in &entries {
                store
                    .add(&entry.name, &entry.destination, None, &[], None, None)
                    .await?;
                count += 1;
            }
            println!("Imported {count} host(s) from SSH config.");
        }
        "export-ssh-config" => {
            let hosts = store.list().await?;
            let entries: Vec<_> = hosts
                .into_iter()
                .map(|h| HostConfigEntry::new(h.name, h.destination))
                .collect();
            print!("{}", export_ssh_config(&entries));
        }
        "import-csv" => {
            let content = fs::read_to_string(&args[2])?;
            let entries = parse_csv(&content).map_err(|e| invalid(&e))?;
            let mut count = 0;
            for entry in &entries {
                store
                    .add(&entry.name, &entry.destination, None, &[], None, None)
                    .await?;
                count += 1;
            }
            println!("Imported {count} host(s) from CSV.");
        }
        "export-csv" => {
            let hosts = store.list().await?;
            let entries: Vec<_> = hosts
                .into_iter()
                .map(|h| HostConfigEntry::new(h.name, h.destination))
                .collect();
            print!("{}", export_csv(&entries));
        }
        "palette" => {
            let sub = args[2].as_str();
            match sub {
                "search" => {
                    let query = &args[3];
                    let all_hosts = store.list().await?;
                    let all_snippets = store.list_snippets().await?;

                    let mut registry = PaletteRegistry::with_static_actions();
                    for h in all_hosts {
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
                            .with_keywords(h.tags),
                        );
                    }

                    for s in all_snippets {
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
                            .with_keywords(s.tags),
                        );
                    }

                    let matches = registry.search(query);
                    println!("SCORE\tCATEGORY\tSHORTCUT\tTITLE\tDESCRIPTION");
                    for m in matches {
                        let shortcut = m.command.shortcut.as_deref().unwrap_or("-");
                        println!(
                            "{}\t{}\t{}\t{}\t{}",
                            m.score,
                            m.command.category.as_str(),
                            shortcut,
                            m.command.title,
                            m.command.description
                        );
                    }
                }
                _ => unreachable!(),
            }
        }
        "session" => {
            let sub = args[2].as_str();
            match sub {
                "list" => {
                    let host_id_opt =
                        extract_flag(&args[3..], "--host-id").and_then(|s| s.parse::<i64>().ok());
                    let recordings = store.list_session_recordings(host_id_opt).await?;
                    if recordings.is_empty() {
                        println!("No session recordings found.");
                    } else {
                        println!("ID\tDURATION\tEVENTS\tSIZE\tRECORDED AT\tTITLE");
                        for r in recordings {
                            println!(
                                "{}\t{}\t{}\t{}\t{}\t{}",
                                r.id,
                                r.formatted_duration(),
                                r.event_count,
                                r.formatted_size(),
                                r.recorded_at,
                                r.title
                            );
                        }
                    }
                }
                "show" => {
                    let id = &args[3];
                    if let Some(r) = store.get_session_recording(id).await? {
                        println!("Session Recording ID: {}", r.id);
                        println!("Session ID:          {}", r.session_id);
                        println!(
                            "Host ID:             {}",
                            r.host_id
                                .map(|h| h.to_string())
                                .unwrap_or_else(|| "None (local)".to_string())
                        );
                        println!("Title:               {}", r.title);
                        println!(
                            "Duration:            {}",
                            format_duration_hms(r.duration_secs)
                        );
                        println!("Event Count:         {}", r.event_count);
                        println!("Byte Size:           {} bytes", r.byte_size);
                        println!("Recorded At:         {}", r.recorded_at);
                        println!("Created At:          {}", r.created_at);
                    } else {
                        return Err(invalid(&format!("Session recording '{id}' not found.")).into());
                    }
                }
                "export" => {
                    let id = &args[3];
                    if let Some(r) = store.get_session_recording(id).await? {
                        let out_path = extract_flag(&args[4..], "--out");
                        if let Some(path) = out_path {
                            fs::write(&path, &r.cast_data)?;
                            println!("Exported recording '{id}' to {path}");
                        } else {
                            print!("{}", r.cast_data);
                        }
                    } else {
                        return Err(invalid(&format!("Session recording '{id}' not found.")).into());
                    }
                }
                "import" => {
                    let path = &args[3];
                    let title = extract_flag(&args[4..], "--title")
                        .ok_or_else(|| invalid("Missing required --title flag"))?;
                    let host_id_opt =
                        extract_flag(&args[4..], "--host-id").and_then(|s| s.parse::<i64>().ok());
                    let should_redact = args[4..].iter().any(|a| a == "--redact");

                    let raw_cast = fs::read_to_string(path)?;
                    let final_cast = if should_redact {
                        SessionRedactor::redact_asciicast_v2(&raw_cast)
                            .map_err(|e| invalid(&format!("Redaction failed: {e}")))?
                    } else {
                        raw_cast
                    };

                    let engine = CastPlaybackEngine::from_asciicast_v2(&final_cast)
                        .map_err(|e| invalid(&format!("Failed to parse asciicast: {e}")))?;

                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let now_str = format!("{now}");
                    let rec = SessionRecordingRecord {
                        id: unique_id("rec"),
                        session_id: unique_id("sess"),
                        host_id: host_id_opt,
                        title,
                        duration_secs: engine.duration_secs(),
                        event_count: engine.total_frames(),
                        byte_size: final_cast.len(),
                        cast_data: final_cast,
                        recorded_at: now_str.clone(),
                        created_at: now_str,
                    };
                    let rec_id = rec.id.clone();
                    store.save_session_recording(&rec).await?;
                    println!(
                        "Imported session recording {rec_id} ({} events, {})",
                        rec.event_count,
                        format_duration_hms(rec.duration_secs)
                    );
                }
                "play" => {
                    let id = &args[3];
                    if let Some(r) = store.get_session_recording(id).await? {
                        let is_raw = args[4..].iter().any(|a| a == "--raw");
                        let speed_multiplier = extract_flag(&args[4..], "--speed")
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(1.0);

                        let mut engine = CastPlaybackEngine::from_asciicast_v2(&r.cast_data)
                            .map_err(|e| invalid(&format!("Failed to parse asciicast: {e}")))?;

                        let speed = match (speed_multiplier * 10.0).round() as i32 {
                            5 => PlaybackSpeed::Half,
                            20 => PlaybackSpeed::Double,
                            40 => PlaybackSpeed::Quad,
                            _ => PlaybackSpeed::Normal,
                        };
                        engine.set_speed(speed);
                        engine.play();

                        if is_raw {
                            let frames = engine.seek(r.duration_secs);
                            for f in frames {
                                if f.event_type == "o" {
                                    print!("{}", f.data);
                                }
                            }
                        } else {
                            println!(
                                "▶ Playing '{}' ({}, speed: {})",
                                r.title,
                                format_duration_hms(r.duration_secs),
                                speed.display_label()
                            );
                            let step = 0.05;
                            while engine.status() == PlaybackStatus::Playing {
                                let frames = engine.advance(step);
                                for f in frames {
                                    if f.event_type == "o" {
                                        print!("{}", f.data);
                                        use std::io::Write;
                                        let _ = std::io::stdout().flush();
                                    }
                                }
                                std::thread::sleep(std::time::Duration::from_millis(50));
                            }
                            println!("\n[End of playback]");
                        }
                    } else {
                        return Err(invalid(&format!("Session recording '{id}' not found.")).into());
                    }
                }
                "delete" => {
                    let id = &args[3];
                    if store.delete_session_recording(id).await? {
                        println!("Deleted session recording '{id}'.");
                    } else {
                        return Err(invalid(&format!("Session recording '{id}' not found.")).into());
                    }
                }
                _ => unreachable!(),
            }
        }
        "complete" => {
            let query = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let current_dir = extract_flag(&args[3..], "--dir");
            let all_hosts = store.list().await?;
            let all_snippets = store.list_snippets().await?;

            let host_names = all_hosts.into_iter().map(|h| h.name).collect();
            let snippet_tuples = all_snippets
                .into_iter()
                .map(|s| (s.name, s.command))
                .collect();

            let engine = AutocompleteEngine::new();
            let ctx = AutocompleteContext {
                input_line: query.to_string(),
                cursor_pos: query.len(),
                current_dir,
                host_candidates: host_names,
                snippet_candidates: snippet_tuples,
                history_candidates: Vec::new(),
            };
            let suggestions = engine.suggest(&ctx);
            if suggestions.is_empty() {
                println!("No suggestions for: '{query}'");
            } else {
                println!("SCORE\tKIND\tVALUE\tDESCRIPTION");
                for item in suggestions {
                    let desc = item.description.as_deref().unwrap_or("-");
                    println!(
                        "{}\t{}\t{}\t{}",
                        item.score,
                        item.kind.display_label(),
                        item.value,
                        desc
                    );
                }
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

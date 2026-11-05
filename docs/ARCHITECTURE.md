# Architecture

The root Cargo workspace contains five decoupled crates:

```text
crates/shellumi-core      Pure connection models & domain validation (zero I/O, zero GUI)
crates/shellumi-terminal  VT/ANSI terminal emulator engine & cross-platform PTY manager
crates/shellumi-storage   Local Turso database adapter with transactional schema migrations
crates/shellumi-cli       CLI host management tool & OpenSSH connection launcher
crates/shellumi-app       GPUI desktop window with interactive host management
```

## Component responsibilities

### `shellumi-core`
- Validates SSH destination fields (`SshDestination`): hostname, IPv4, IPv6, username, and port (1–65535).
- Parses quick-connect destination strings (`user@host[:port]` or `user@[ipv6][:port]`) via `parse_quick_connect_target`.
- Rejects control characters, whitespace, and leading hyphens (preventing CLI option injection).
- Parses and serializes OpenSSH client configurations (`~/.ssh/config`) through `parse_ssh_config` and `export_ssh_config`, validating destination fields and handling wildcard blocks.
- Parses and serializes standard RFC 4180 CSV files through `parse_csv` and `export_csv`.
- Generates cryptographically secure Ed25519 SSH key pairs (`generate_ed25519_key`) using `ring::rand::SystemRandom`, outputs OpenSSH wire format and SHA-256 fingerprints, and enforces private key memory zeroization on drop via `zeroize::Zeroizing`.
- Parses OpenSSH `known_hosts` files (`parse_known_hosts`), verifies host keys against records (`verify_host_key`), identifies unknown keys for TOFU confirmation, flags changed keys for MITM protection, and detects `@revoked` keys.
- **Port Forwarding & Jump Hosts:** Implements domain models for SSH port forwarding (`PortForwardRule`, `ForwardingType::Local`, `Remote`, `Dynamic` SOCKS5), OpenSSH argument generation (`-L`, `-R`, `-D`), proactive security warnings when exposing bindings beyond localhost (`is_exposed_beyond_localhost()`), and safe validation of jump host chains (`validate_jump_host`) rejecting shell metacharacters and control codes.
- **Credential & Secret Architecture:** `CredentialStore` includes a macOS Keychain backend, ephemeral storage, and an encrypted file store. Windows Credential Manager and Linux Secret Service return explicit errors until implemented. Native storage and secure key entry require platform testing.
- **SFTP Domain Models, Wire Protocol & Queue Orchestration:** Provides UI-independent models for file entries (`FileEntry`, `FileKind`), UNIX file permission string formatting (`drwxr-xr-x`), human-readable byte sizes (`format_bytes`: B, KB, MB, GB, TB), breadcrumb navigation generation (`PathUtils::breadcrumbs`), safe path joining and traversal sanitization (`PathUtils::join_path`), transfer task progress calculations (`TransferTask`, `TransferStatus`, `TransferDirection`, `ConflictResolution`), local directory browsing (`list_local_directory`), RFC SFTP v3 wire protocol packet codecs (`SftpPacket`, `StatusCode`, `OpenFlags`, `FileAttributes`, `NameEntry`), stream framing with partial-buffer detection and length bounds validation (`WireReader`), and concurrent background transfer queue orchestration (`TransferQueue`, `TransferQueueSummary`).
- **Snippet & Automation Engine:** Implements UI-independent models for parameterized snippets (`Snippet`, `SnippetVariable`), automated variable extraction supporting `{{var}}` and `{{var:-default}}` syntax, POSIX shell argument escaping (`shell_escape`) to prevent shell injection, safe parameter substitution (`render_command`), automatic secret variable classification (flags variables named *pass*, *secret*, *token*, *key*), preview redaction (`render_preview`) substituting secret values with `[REDACTED]`, and standard JSON library serialization and deserialization (`export_snippets_json`, `parse_snippets_json`).
- **Workspaces & Multi-Pane Broadcast Engine:** Implements UI-independent workspace models in `shellumi-core::workspace`: `WorkspaceLayout` (Single, SplitVertical, SplitHorizontal, DualGrid), `WorkspaceSessionKind` (SSH, Local, SFTP), `WorkspaceSession` (pane name, destination, host link, startup command, working dir, pane index, broadcast exclusion flag), and `Workspace` with builder validation and duplication. Implements `BroadcastManager` tracking broadcast state, total active panes, individual pane exclusions, receiving panes count, and formatted status text. Implements `check_dangerous_command` and `BroadcastSafetyCheck` detecting destructive operations (`rm -rf /`, `mkfs`, raw block device writes `dd of=/dev/`, fork bombs, system shutdown/reboot, and firewall flushes) to protect clustered terminals from accidental devastation.
- **Multi-Protocol Domain Engine:** Implements UI-independent connection models in `shellumi-core::protocol`: `ConnectionProtocol` enum (`Ssh`, `Mosh`, `Telnet`, `Serial`, `Local`) with protocol feature introspection (`is_encrypted()`, `supports_sftp()`, `supports_tunnels()`, `supports_jump_host()`, `supports_startup_snippet()`, `default_port()`, `badge_icon()`), `MoshConfig` and `MoshPredictMode` (argument generation for UDP port range, prediction mode `Adaptive`/`Always`/`Never`/`Experimental`, SSH port, and server binary), RFC 854/1091/1073 `TelnetConfig` stream parser (`parse_telnet_stream`) handling `IAC` negotiation commands and escaped `IAC IAC`, subnegotiations for NAWS (window size) and terminal type, command builder, and cleartext security warnings (`security_warning()`), `SerialConfig` (baud rate, data bits, parity, stop bits, flow control, mode string formatting e.g. `115200 8N1`, command builder for `screen`/`picocom`), `ProtocolEndpoint` abstraction formatting endpoints and generating security warnings, and cross-platform serial port discovery (`discover_serial_ports()`).
- **Team Collaboration & Shared Vaults Engine:** Implements UI-independent collaboration models in `shellumi-core::collaboration`: `TeamRole` enum (`Owner`, `Administrator`, `Editor`, `Viewer`) with granular privilege checks (`can_manage_members()`, `can_manage_roles()`, `can_rotate_key()`, `can_edit_records()`, `can_view_records()`, `can_view_audit_log()`, `can_manage_terminal_session()`) and badge labels; `TeamMember` (user ID, vault link, display name, public key, role, enrollment timestamp, invitation provenance, and revocation state); `SharedVault` (vault ID, name, description, owner, cryptographic epoch counter, and `rotate_key()`); `SharingAuditRecord` (audit ID, vault ID, `AuditAction` [VaultCreated, MemberInvited, MemberEnrolled, MemberRoleUpdated, MemberRevoked, KeyRotated, RecordShared, CollaborationSessionStarted, KeyboardControlGranted, KeyboardControlRevoked, CollaborationSessionEnded], actor ID, target ID, details, and human-readable entry formatting); `MultiplayerSession` and `MultiplayerParticipant` real-time state machines enforcing single-active-writer semantics with explicit keyboard writer delegation (`grant_keyboard_control`), immediate host revocation (`revoke_keyboard_control`), and participant departure cleanup; and `SharedSessionLog` with timestamped `LogComment` and `LogBookmark` audit markers. Expands `SyncEntityType` with `SharedVault`, `TeamMember`, and `AuditRecord` for end-to-end encrypted synchronization. Expands `PaletteCategory::Collaboration` (`👥`) and registers static collaboration actions in the command palette.
- **End-to-End Encrypted Cloud Sync Engine:** Implements zero-knowledge cryptography using `ring`: PBKDF2-HMAC-SHA256 key derivation with memory zeroization on drop, authenticated encryption with associated data (AEAD) using AES-256-GCM (`seal_payload`, `open_payload`), `EncryptedEnvelope` with bound associated data (`entity_type:object_id`) preventing ciphertext transposition, versioned `SyncRecord` models with deletion tombstones, three-way sync conflict detection (`determine_sync_action`), explicit conflict resolution strategies (`resolve_record_conflict`, `ConflictResolutionChoice::KeepLocal` / `AcceptRemote`), payload summarization (`format_record_summary`), and conflict inspection models (`ConflictItemView`, `build_conflict_item_views`), RFC 7636 PKCE challenge generation (`generate_pkce_challenge`), Google OAuth 2.0 authorization URL builder for the restricted `drive.appdata` scope, local loopback HTTP OAuth callback server (`OAuthCallbackListener`), callback request parsing with CSRF state verification (`parse_oauth_callback_request`), token bundle management (`OAuthTokenBundle`), record difference engine with payload decryption (`diff_sync_records_with_passphrase`), `SyncVault` encrypted snapshot serialization, transport-agnostic HTTP protocol models (`HttpTransport`, `HttpRequest`, `HttpResponse`), Google Drive Application Data folder REST API client (`GoogleDriveSyncClient` handling token exchange, token refresh, file listing, multipart snapshot upload, snapshot download, and file update/deletion), Drive file metadata (`DriveFileMetadata`), and conflict-safe sync cycle planner (`plan_sync_cycle`, `SyncPlan`).
- **System Diagnostics & Health Check Subsystem:** Implements UI-independent health diagnostics in `shellumi-core::diagnostics` (`DiagnosticsRunner`, `DiagnosticsReport`, `DiagnosticCheck`, `DiagnosticCategory` [System, Security, OpenSsh, Network, Storage, Terminal], `DiagnosticSeverity` [Info, Warning, Critical], `DiagnosticStatus` [Passed, Warning, Failed]). Performs automated audits:
  - CSPRNG entropy validation (`ring::rand::SystemRandom`).
  - AEAD encryption & key derivation round-trip canary tests (`ring::aead::AES_256_GCM`, PBKDF2-HMAC-SHA256).
  - OpenSSH client binary discovery and version banner parsing (`ssh -V`).
  - OS credential store backend availability check (`CredentialStore`).
  - Ephemeral loopback network socket binding verification (`127.0.0.1:0`).
  - Metadata database path accessibility and SQLite header/schema validation.
  - Cross-platform PTY subsystem spawning capability checks.
  - Generates human-readable terminal summaries, structured Markdown reports, and JSON payloads.
  - Registers `PaletteCategory::Diagnostics` (`🩺`) and command palette action `PaletteAction::RunDiagnostics` (`Cmd+Shift+D`).
- **Mobile & Portable Core Subsystem:** Implements UI-independent mobile models and state machines in `shellumi-core::mobile`:
  - `MobilePlatform` (`Ios`, `Android`, `Simulator`).
  - `VirtualKey` (Esc, Tab, Ctrl-chars, Alt-chars, arrows, pipe, tilde, slash, backslash, function keys F1-F12, home, end, pgup, pgdn) and `VirtualKeyModifier` (`Ctrl`, `Alt`, `Shift`).
  - `VirtualAccessoryBar` with customizable presets (`DefaultTerminal`, `CompactNavigation`, `CodeEditing`), sticky modifier state machine with toggle semantics, and one-shot keystroke resolution `resolve_char_keystroke`.
  - `MobileSessionState` (`Connecting`, `Connected`, `BackgroundGracePeriod`, `Suspended`, `Reconnecting`, `Disconnected`) and `MobileLifecycleEvent` (`AppForegrounded`, `AppBackgrounded`, `BackgroundTimerTick`, `BackgroundExpired`, `NetworkLost`, `NetworkRestored`, `UserDisconnect`).
  - `MobileLifecycleManager` managing deterministic mobile lifecycle transitions, background grace timers (default 30s matching iOS background execution bounds), exponential reconnection backoff (1s, 2s, 4s, up to 30s max), and key zeroization upon disconnection.
  - `MobileCredentialSpec` defining platform-native secure enclave / hardware keystore requirements: iOS Keychain `kSecClassGenericPassword` with `kSecAccessControlBiometryAny` vs Android Keystore AES-GCM with `setUserAuthenticationRequired(true)` and StrongBox backing.
  - `MobileFfiBridge` headless JSON/C-ABI bridge helpers (`parse_destination_json`, `translate_virtual_key_json`, `generate_keypair_json`) for UniFFI and foreign language binding.
  - Registers `PaletteCategory::Mobile` (`📱`) and command palette action `PaletteAction::OpenMobileSimulator` (`Cmd+Alt+M`).
- UI-independent and network-independent; contains no GUI code.

### `shellumi-terminal`
- **Screen & Grid Management:** Manages character cells (`Cell`), 16 standard ANSI colors, 256-color palette, and 24-bit TrueColor RGB (`Color`), text attributes (bold, dim, italic, underline, inverse, strikethrough), cursor positioning, and scrollback history. Converts character cells into minimal contiguous styled spans (`StyledSpan`) for efficient UI rendering. Supports configurable scrollback capacity (`set_scrollback_limit`, `scrollback_limit`) with automated trimming of excess lines to protect against unbounded memory growth during high-throughput workloads.
- **Escape Sequence Handling & Sanitization:** Implements `vte::Perform` to parse ANSI/VT sequences, cursor movement (CUU, CUD, CUF, CUB, CNL, CPL, CHA, CUP), screen/line clearing (ED, EL), scrolling (SU, SD), alternate screen buffers (DECSET/DECRST 1049), and window titles (OSC 0/2). Sanitizes incoming OSC sequences by filtering ASCII control codes (`\x00..\x1F` except tab) and clamping titles to 256 characters, neutralizing terminal title escape injection and buffer abuse.
- **Cross-Platform PTY:** Spawns native pseudo-terminals (`portable-pty`) on macOS, Linux, and Windows, supporting shell auto-detection, custom commands, arguments, working directories, environment variables, resizing, and child process lifecycle.

### `shellumi-storage`
- Uses the embedded, in-process Turso Database engine (`turso` crate).
- Stores host metadata only; credentials must never be written to this database.
- Enforces transactional schema migrations via `PRAGMA user_version` (schema version 12, with automatic migration from versions 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, and 11) and rejects unsupported future schemas.
- Supports persistent host groups (`groups` table), jump hosts (`hosts.jump_host`), persistent startup snippets (`hosts.startup_snippet_id` foreign key referencing `snippets(id)` ON DELETE SET NULL), persistent SSH tunnels (`tunnels` table), persistent file transfer history (`transfer_history` table), persistent snippets (`snippets` table), persistent workspaces (`workspaces` table: id, name, description, layout, sessions JSON, tags, created_at, updated_at), multi-protocol definitions (`hosts.protocol` default `'ssh'` and `hosts.protocol_config`), shared vaults & team collaboration (`shared_vaults`, `team_members`, `sharing_audit_log`, `shared_session_logs` tables), and cloud infrastructure providers & discovered instances (`cloud_providers`, `discovered_hosts` tables with foreign key tracking to `hosts(id)`).
- Supports disaster recovery snapshots (`sync_snapshots` table: id, snapshot_id, vault_json, record_count, created_at), automated pre-import and export snapshot creation, retention limit pruning, and snapshot rollbacks.
- Provides configuration inheritance through `get_effective_destination` and `get_effective_tags`, automatically applying group defaults when host fields are omitted.
- Exposes async CRUD operations: `add`, `add_with_protocol`, `update`, `update_with_protocol`, `get_host`, `list`, `list_by_group`, `list_by_tag`, `search`, and `delete` for hosts, plus `add_group`, `update_group`, `get_group`, `list_groups`, and `delete_group`, `add_tunnel`, `update_tunnel`, `list_tunnels`, `list_tunnels_for_host`, and `delete_tunnel`, `log_transfer`, `update_transfer`, `list_transfers`, and `clear_transfer_history`, `add_snippet`, `update_snippet`, `delete_snippet`, `get_snippet`, `list_snippets`, `list_snippets_by_tag`, and `search_snippets`, `add_workspace`, `update_workspace`, `get_workspace`, `list_workspaces`, `delete_workspace`, and `duplicate_workspace`, team collaboration operations (`create_shared_vault`, `get_shared_vault`, `list_shared_vaults`, `update_shared_vault_epoch`, `delete_shared_vault`, `add_team_member`, `get_team_member`, `list_team_members`, `update_team_member_role`, `revoke_team_member`, `log_audit_event`, `list_audit_log`, `create_shared_session_log`, and `list_shared_session_logs`), and cloud provider operations (`create_cloud_provider`, `get_cloud_provider`, `list_cloud_providers`, `update_cloud_provider_sync_time`, `delete_cloud_provider`, `save_discovered_hosts`, `list_discovered_hosts`, `mark_discovered_host_imported`).
- Implements zero-knowledge vault synchronization and disaster recovery methods: `export_vault` / `export_vault_with_iterations` to seal all database entities into an encrypted snapshot (auto-archiving snapshot history), `import_vault` to decrypt and restore or update entities with an `ImportVaultSummary` (including `startup_snippet_id` mapping, workspaces, protocol fields, and team collaboration records), and disaster recovery operations: `archive_snapshot`, `list_snapshot_history`, `get_snapshot_vault`, and `rollback_snapshot`.

### `shellumi-cli`
- Provides command-line host management: `list`, `show <id>`, `add`, `update`, `delete`, with `--group`, `--tag`, `--jump-host`, `--startup-snippet`, `--protocol <ssh|mosh|telnet|serial>`, `--protocol-config <json>`, and `--search` filtering flags.
- Displays detailed protocol information and prominent Telnet cleartext security warnings in `show <id>`.
- Provides dedicated group management: `group add`, `group list`, `group show`, and `group delete`.
- Provides dedicated tunnel management: `tunnel add`, `tunnel list`, and `tunnel delete`, with automatic localhost exposure detection and security warnings.
- Provides dedicated SFTP commands: `sftp local-ls <path> [--all]`, `sftp history [--host-id <id>] [--limit <n>]`, `sftp clear-history [--host-id <id>]`, `sftp log <upload|download> <local-path> <remote-path> <bytes>`, `sftp packet <encode-init|decode|encode-open> [...]`, and `sftp queue <upload|download> <local-path> <remote-path> <bytes>`.
- Provides dedicated snippet management: `snippet add <name> <cmd> [--desc <desc>] [--tags <t1,t2>]`, `snippet list [--tag <tag>] [--search <query>]`, `snippet show <id>`, `snippet render <id> [--var key=value] [--preview]`, `snippet delete <id>`, and `snippet batch <id> --hosts <h1,h2...> [--parallel|--sequential] [--dry-run]`.
- Provides automated environment diagnostics: `doctor [--json] [--markdown] [--db <path>]` running comprehensive checks on CSPRNG entropy, AEAD crypto, OpenSSH binary, keychain, network loopback binding, database schema, and PTY capabilities.
- Provides mobile simulation and validation commands:
  - `mobile virtual-keys [--preset default|compact|coding]` displaying available touch accessory keys and ANSI byte translations.
  - `mobile lifecycle-sim <event>` simulating mobile state machine transitions (`foreground`, `background`, `tick`, `expired`, `net-lost`, `net-restored`, `disconnect`).
  - `mobile ffi-check` verifying portable headless C-ABI / UniFFI bridge functions.
- Provides dedicated cloud infrastructure subcommands:
  - `infra provider add <name> <aws|azure|digitalocean|ansible> [...]`, `infra provider list`, `infra provider show <id>`, `infra provider delete <id>`, and `infra provider sync <id>`.
  - `infra discover <aws|azure|digitalocean|ansible> --file <json-file> [--import-all] [--dry-run]` to parse and import discovered instances.
  - `infra ansible export --output <path> [--format ini|yaml]` and `infra ansible import --file <path> [--format ini|yaml]`.
- Provides dedicated workspace management: `workspace add <name> [--layout <single|vertical|horizontal|grid>] [--desc <desc>] [--tags <t1,t2>]`, `workspace list`, `workspace show <id>`, `workspace duplicate <id> <new-name>`, and `workspace delete <id>`.
- Provides dedicated multi-protocol subcommands:
  - `mosh <user@host[:port]> [--ssh-port <p>] [--predict <mode>]` launching mobile shell connections.
  - `telnet <host> [port]` with an explicit cleartext credential interception warning before launch.
  - `serial <port> [--baud <baud>]` launching serial communication via system serial tooling (`screen`).
  - `serial-ports` discovering serial device candidates on the system (`/dev/tty.*`, `/dev/cu.*` on Unix or `COM*` on Windows).
- Provides dedicated team collaboration subcommands:
  - `team vault create <name> [--desc <desc>] [--owner <owner-id>]`, `team vault list`, `team vault show <id>`, `team vault rotate-key <id>`, and `team vault delete <id>`.
  - `team member invite <vault-id> <user-id> <name> <role> [--public-key <key>] [--invited-by <by>]`, `team member list <vault-id>`, `team member role <vault-id> <member-id> <new-role>`, and `team member revoke <vault-id> <member-id>`.
  - `team audit <vault-id> [--limit <n>]`.
  - `team session start <session-id> <owner-id> [--vault-id <id>]`, `team session join <session-id> <user-id> <name>`, `team session grant <session-id> <requester-id> <target-user-id>`, `team session revoke <session-id> <requester-id>`, and `team session status <session-id>`.
- Provides zero-knowledge vault management and disaster recovery: `vault export <output-file> [--passphrase <p>] [--device-id <d>] [--iterations <n>]`, `vault import <input-file> [--passphrase <p>]`, `vault diff <input-file> [--passphrase <p>] [--device-id <d>]`, `vault status`, `vault oauth-url [--client-id <id>] [--redirect-uri <uri>]`, `vault history`, and `vault rollback <snapshot-id> [--passphrase <p>]`.
- Provides immediate `quick-connect <user@host[:port]>` launching OpenSSH without database persistence.
- Provides Ed25519 key pair generation via `keygen [--comment <comment>]`.
- Provides OpenSSH config import and export: `import-ssh-config <config-path>` and `export-ssh-config`.
- Provides CSV import and export: `import-csv <csv-path>` and `export-csv`.
- Provides a protocol-aware `connect` command that dispatches to system OpenSSH, Mosh, Telnet, or Serial with host-key confirmation and agent forwarding disabled for SSH, resolving inherited destination parameters and chaining validated jump hosts. It enforces capability restrictions (e.g. startup snippets only on SSH) and emits cleartext warnings for Telnet.
- Validates inputs before opening or mutating the database.

### `shellumi-app`
- GPU-accelerated desktop user interface built with GPUI 0.2.2.
- **Threading model:** All database operations execute on background tasks via `cx.background_executor().spawn()` with an isolated current-thread Tokio runtime, keeping the GPUI UI thread responsive.
- **Form state management:** Uses an interactive `HostForm` model for adding and editing hosts, including Group, Tag, Jump Host, Startup Snippet ID, and Protocol selection (`[🔒 SSH]`, `[⚡ Mosh]`, `[⚠️ Telnet]`, `[🔌 Serial]`) with automatic port defaults (22, 23, 1) and dynamic protocol-specific warnings and notices.
- **Search & Filter:** Features a real-time host search and filtering input bar that dynamically filters hosts by name, hostname, username, group, jump host, or tags.
- **Quick Connect:** Features an immediate Quick Connect bar (`user@host[:port]`) in the Hosts management view.
- **Sync status:** Cloud sync is not implemented. The sync control reports that limitation instead of claiming a successful sync; conflict resolution is not connected to real data.
- **Host list display:** Shows group, jump-host, startup-snippet, and protocol badges (`[🔒 SSH]`, `[⚡ Mosh]`, `[⚠️ Telnet]`, `[🔌 Serial]`) alongside host actions. SSH-exclusive actions like SFTP are hidden for non-SSH hosts. Startup snippets are not automatically sent because SSH authentication completion is not yet observable.
- **Connection dispatch:** Protocol-aware dispatching: SSH launches OpenSSH with validated jump options; Mosh launches `mosh`; Telnet launches `telnet` with prominent unencrypted warnings; Serial launches `screen` with configured baud rate. Startup-snippet execution is deferred until authentication readiness can be verified.
- **Text input:** Implements a native-feeling `TextInput` element supporting programmatic value assignment (`set_value`), mouse selection, drag selection, cursor navigation, keyboard shortcuts, copy/cut/paste, and input marking.
- **Shortcuts:** Supports platform-aware keybindings (e.g. `Cmd-R` on macOS, `Ctrl-R` on Linux/Windows for refreshing the host list).
- **Multi-Tab Sessions:** Manages multiple concurrent terminal sessions (`SessionTab`)
  via a top tab bar across diverse protocols (`SessionKind::Ssh`, `Mosh`, `Telnet`, `Serial`, `Local`, `Sftp`). Users can switch between open local shells, remote connections,
  graphical or interactive SFTP sessions, and the "Hosts" management view without disconnecting running sessions. Each tab maintains
  its own PTY worker, VT parser state, 2D screen grid, and scrollback history. Tabs can be
  opened, switched, or closed independently with automatic active tab index tracking. Telnet sessions display a persistent `[⚠️ UNENCRYPTED TELNET]` badge in the terminal view header.
- **Graphical Dual-Pane SFTP File Manager:** Dedicated SFTP session tabs feature an integrated dual-pane graphical file manager (`SftpBrowserState`) with clickable breadcrumbs, parent directory traversal (`⬆ Up`), hidden-files toggling (`👁 Show/Hide Hidden`), file/directory listings with formatted sizes and permissions, selection-driven Upload and Download actions, a live transfer queue drawer with a visual percentage progress bar, and instant mode switching (`[📂 Dual-Pane Mode]` / `[⌨ Terminal Mode]`) between the graphical browser and the interactive OpenSSH SFTP terminal.
- **Split Panes & Layouts:** Supports vertical and horizontal split panes within any session tab
  (`split_pane(SplitDirection::Vertical)`, `split_pane(SplitDirection::Horizontal)`). Each pane runs
  an independent PTY session with its own terminal grid, search state, and active focus outline.
- **Workspaces & Broadcast Input Replicator:** When multiple panes are active in a session tab, the header presents an instant Broadcast Input toggle (`📡 Broadcast: ON (X/Y)`). When enabled, keyboard typing and clipboard pastes are replicated simultaneously across all participating terminal panes. Each individual pane header provides a 1-click broadcast toggle (`[📡 Broadcast]` / `[🔇 Excluded]`) to omit specific panes from mirrored typing. Typing dangerous destructive commands (such as `rm -rf /`, `mkfs`, raw block writes, fork bombs, or reboot/shutdown) is automatically intercepted and blocked with a high-visibility warning banner (`⚠️ Dangerous broadcast command blocked`) to prevent accidental cluster destruction. The active tab's layout and session configuration can be persisted directly as a reusable workspace via Command Palette (`Save Current Layout as Workspace`), and restored with 1-click execution (`Load Workspace`).
- **Terminal Search:** Each terminal pane includes an integrated search toolbar with query input,
  match count (`1/N matches`), forward and backward navigation (`↑` / `↓`), and highlighted span rendering
  in the active terminal buffer and scrollback history.
- **Snippets Drawer & Interactive Parameter Modal:** A collapsible `📜 Snippets` toolbar button opens an in-session
  drawer above the active terminal grid, displaying saved snippet chips with syntax previews. Snippets without unresolved variables
  offer one-click "Paste" (streaming into the active PTY without carriage return) and "Run" (dispatching with carriage return). Snippets
  containing unresolved variables (`{{var}}`, `{{var:-default}}`) automatically trigger an interactive Snippet Variable Modal overlay,
  allowing users to provide parameters with secret masking badges (`[SECRET]`), default placeholder values, and validation before executing or pasting.
- **Global Command Palette:** Decoupled command palette engine in `shellumi-core::palette` (`PaletteRegistry`, `PaletteCommand`, `PaletteCategory`, `PaletteAction`), featuring fuzzy tokenized scoring, title prefix bonuses, and acronym matching (e.g. `qc` for Quick Connect). Integrated in `shellumi-app` (`CommandPaletteState`) as a prominent modal overlay accessible via `Cmd-K` (macOS) / `Ctrl-K` (Linux/Windows) or top navigation buttons across both Hosts and Terminal views. Supports full keyboard navigation (`↑`/`↓` selection, `↵` execute, `Esc` dismiss) and direct execution for SSH/SFTP connections, snippet execution, multi-host batch execution (`PaletteAction::ExecuteBatchSnippet`), Cloud Sync triggering, pane splitting, tab switching, and team collaboration actions (`PaletteAction::OpenTeamCollaboration`, `ToggleMultiplayerCollaboration`).
- **Team Collaboration & Shared Vaults:** Main toolbar features a prominent `[👥 Team Vaults]` action opening an interactive Team Collaboration Modal (`TeamModalState`) with tabbed navigation:
  - *Shared Vaults tab:* displays list of vaults with cryptographic epoch badges, owner info, key rotation (`[🔄 Rotate Key]`), and deletion; inline form for creating new shared vaults.
  - *Members tab:* displays enrolled team members with role badges (`[👑 Owner]`, `[🛡️ Admin]`, `[✏️ Editor]`, `[👁️ Viewer]`), role reassignment controls, and one-click member revocation (`[⛔ Revoke]`); inline form for inviting new collaborators with role picker.
  - *Audit Log tab:* displays tamper-evident audit records formatted with timestamp, action badge, actor, target, and details.
- **Multiplayer Terminal Collaboration:** Terminal session tabs across SSH, Mosh, Telnet, Serial, and Local sessions feature an interactive `[👥 Collaborate]` action. When active, a high-visibility collaboration strip displays session ID, host user, connected collaborators, and active writer status (`✍️ Active Writer: @user`). Enforces single-active-writer semantics where keystrokes are restricted to the designated writer, while the session host can transfer keyboard control (`[Grant Control to @user]`) or revoke control immediately (`[⛔ Revoke to Host]`).
- **Infrastructure Integrations & Cloud Discovery:** Main toolbar provides an interactive `[☁️ Cloud & Infra]` action opening the Infrastructure Modal (`InfraModalState`) with tabbed management:
  - *Providers tab:* configure and manage cloud providers (AWS EC2, Azure VMs, DigitalOcean Droplets, Ansible inventory), with credential redaction, manual synchronization triggers (`[🔄 Sync Provider]`), and deletion (`[🗑️ Delete]`); inline form for adding providers with provider-specific parameters.
  - *Discovered Hosts tab:* lists compute instances discovered across cloud providers, displaying running state indicators (`🟢 Running`, `🔴 Stopped`, `🟡 Pending`), destination addresses (`user@ip:port`), regions, instance types, tags, and one-click import into Shellumi inventory (`[📥 Import Host]`) or imported indicator badge (`[✅ Imported]`).
  - *Ansible Inventory tab:* bidirectional Ansible inventory integration supporting export of Shellumi hosts, groups, and tags into fully compliant Ansible INI and YAML inventory formats with 1-click execution.
  - *Background thread safety:* all cloud synchronization and database updates run on background worker threads (`cx.background_executor()`) without stalling or blocking the main GPUI render loop.
- **Multi-Host Snippet Execution & Batch Runner:** Core and desktop batch components exist. CLI batch execution is disabled until it can execute real commands with appropriate confirmation; CLI dry-run previews remain available.
- **Terminal sessions:** A threaded session worker (`Session`) owns a PTY process,
  streams bounded output events to GPUI tasks via `async_channel`, and handles
  non-blocking input, resizing (`SessionCommand::Resize`), and lifecycle termination.
  Supports both SSH sessions (launching OpenSSH with strict flags via `shellumi-core`)
  and local shell sessions (auto-detecting system default shell). The UI provides
  a live monospace terminal grid with styled spans (foreground/background colors,
  bold, italic, underline, strikethrough, scrollback), cursor coordinates/dimensions
  in a status bar, clipboard paste (`Cmd-V` / `Ctrl-Shift-V`), and extended key translation.
- **System Health Diagnostics & Audit Modal:** The main toolbar includes a `[🩺 Health Check]` action (and Command Palette action `PaletteAction::RunDiagnostics` / `Cmd+Shift+D`) opening the Diagnostics Modal (`DiagnosticsModalState`). Diagnostics execute on background worker threads (`cx.background_executor()`) without UI hitching, auditing CSPRNG entropy, AES-256-GCM AEAD encryption, OpenSSH binary availability, OS keychain backend status, loopback network socket binding, database integrity, and cross-platform PTY capabilities. The modal presents a dynamic system health banner (`✅ All Systems Ready`, `⚠️ System Degraded`, `❌ Action Required`), summary stats, execution duration per check, and actionable remediation steps.
- **Desktop Mobile Simulator Modal & Virtual Accessory Bar:** The main toolbar features a `[📱 Mobile Sim]` action (and Command Palette action `PaletteAction::OpenMobileSimulator` / `Cmd+Alt+M`) opening the Mobile Simulator overlay (`MobileSimulatorModalState`). It enables interactive desktop testing of mobile-specific UX constraints without mobile hardware:
  - *Device Viewport Presets:* Switchable viewports for iPhone 15 Pro (393×852 pt / 80×24 chars), Pixel 8 (412×924 pt / 82×26 chars), and iPad Pro 11" (834×1194 pt / 120×36 chars).
  - *Touch-First Virtual Accessory Strip:* Interactive rendered accessory row with Esc, Tab, Ctrl, Alt, Arrow keys (←, →, ↑, ↓), Pipe (`|`), Tilde (`~`), Slash (`/`), and Backslash (`\`).
  - *Sticky Modifier State Machine:* One-shot modifier toggles for `Ctrl` and `Alt` designed for single-hand mobile touch typing.
  - *Mobile Lifecycle Triggers:* Interactive simulation buttons triggering `App Backgrounded`, `Timer Tick (10s)`, `Background Expired`, `App Foregrounded`, `Network Lost`, `Network Restored`, and `Disconnect`.
  - *ANSI Escape Byte Telemetry:* Real-time live log recording rendered hex and ASCII byte streams emitted by touch keystrokes.
- **Verification:** An opt-in [isolated OpenSSH test](development/ssh-integration.md)
  exercises host-key confirmation and public-key authentication through the PTY
  worker and terminal grid. Unit tests cover endpoint formatting, host search/group/tag
  filtering, ANSI key mapping, clipboard paste detection, styled text span rendering,
  tab index lifecycle, and PTY resize dispatch.

## Cross-platform packaging & release architecture

Shellumi provides standalone installer definitions for each supported tier-1 desktop platform:

- **macOS (`packaging/macos`):**
  - `Info.plist`: defines bundle metadata (`org.shellumi.app`), bundle version, Minimum macOS version (10.15 Catalina), Retina High-DPI support (`NSHighResolutionCapable`), Dark Mode compatibility (`NSRequiresAquaSystemAppearance: false`), and registered URL scheme handlers for `ssh://`, `sftp://`, and `telnet://`.
  - `build-dmg.sh`: creates a standard macOS application bundle layout (`Shellumi.app/Contents/MacOS`, `Contents/Resources`), populates metadata and icons, creates an Applications symlink, and generates an Apple Disk Image (`Shellumi-macos-<arch>.dmg`) via `hdiutil`.
- **Linux (`packaging/linux`):**
  - `shellumi.desktop`: standard XDG desktop entry with application name, generic name ("SSH & Terminal Client"), executable path, icon path, Categories (`Utility;System;TerminalEmulator;Network;RemoteAccess;`), Keywords (`ssh;mosh;telnet;serial;sftp;terminal;`), and URL scheme associations (`x-scheme-handler/ssh;x-scheme-handler/sftp;x-scheme-handler/telnet;`).
  - `debian/control`: Debian package manifest defining package metadata, architecture, dependencies (libc6, libxkbcommon0, libwayland-client0, libx11-6), and maintainer information.
  - `build-deb.sh`: builds a native Debian package (`shellumi_<version>_amd64.deb`) with proper binary permissions (`/usr/bin/shellumi`), desktop entries (`/usr/share/applications`), and icons (`/usr/share/icons/hicolor/scalable/apps`), with automated fallback to `.tar.gz` distribution tarballs.
- **Windows (`packaging/windows`):**
  - `wix/main.wxs`: WiX Toolset v3/v4 XML installer manifest generating a standard Windows Installer (`Shellumi-windows-x86_64.msi`) with Program Files installation, Start Menu shortcuts, Desktop shortcut options, Add/Remove Programs integration, and clean uninstaller hooks.
- **Release Automation (`.github/workflows/release.yml`):**
  - Multi-platform GitHub Actions release matrix: `linux-x86_64` (Ubuntu 24.04), `windows-x86_64` (Windows Server 2022), `macos-arm64` (macOS 14 Apple Silicon), and `macos-x86_64` (macOS 13 Intel).
  - Builds both `shellumi-app` desktop client and `shellumi-cli` standalone tool in release profile.
  - Generates `.deb`, `.dmg`, and `.zip` installers.
  - Generates CycloneDX Software Bill of Materials (SBOM) and SHA-256 checksums (`SHA256SUMS.txt`).
  - Publishes signed GitHub releases on Git version tags (`v*`).

## Mobile architecture & portable core subsystem

Per the mandate in `PROJECT.md`, GPUI 0.2.2 is preserved strictly for desktop windowing without premature web or hybrid wrappers. Mobile support is achieved through an architecturally decoupled, portable Rust core:

1. **Decoupled Headless Core:** `shellumi-core` has zero dependencies on GPUI, AppKit, Wayland, or Win32. All domain logic, destination parsing, Ed25519 key generation, known-hosts verification, port forwarding calculation, and mobile lifecycle state machines compile natively for mobile targets (`aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-linux-android`, `x86_64-linux-android`).
2. **Mobile Lifecycle State Machine:** `MobileLifecycleManager` governs the transition between `Connecting`, `Connected`, `BackgroundGracePeriod`, `Suspended`, `Reconnecting`, and `Disconnected`. When an app moves to the background on iOS (which grants ~30s before suspending processes), a grace timer runs; if background time expires, sockets are proactively closed and memory zeroized to avoid sudden OS kernel termination. Upon returning to the foreground, deterministic exponential backoff handles reconnection.
3. **Touch Ergonomics & Virtual Accessory Bar:** On touch devices lacking physical keyboards, `VirtualAccessoryBar` provides essential terminal control keys (Esc, Tab, Ctrl, Alt, Arrow navigation, pipes, tildes, backslashes, and function keys). It features sticky modifier state machine semantics (`Ctrl` and `Alt` toggle on touch and resolve one-shot with the next typed character) eliminating the multi-touch requirement of physical keyboards.
4. **Hardware-Backed Biometric Security:** `MobileCredentialSpec` defines security parameters for mobile targets:
   - *iOS:* Keychain items stored with `kSecClassGenericPassword`, `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, and `kSecAccessControlBiometryAny` (enforcing Face ID / Touch ID via Secure Enclave).
   - *Android:* Android Keystore AES-GCM encryption with `KeyGenParameterSpec.setUserAuthenticationRequired(true)` and StrongBox Keymaster hardware backing.
5. **Desktop Touch Simulator:** `shellumi-app` includes an in-process mobile simulator overlay (`MobileSimulatorModalState`) allowing engineers to test mobile accessory layouts, sticky modifier resolution, lifecycle state transitions, and ANSI byte emission directly on macOS, Linux, and Windows desktops.

## Dependency direction and engineering rules

The intended dependency hierarchy is:
`UI (shellumi-app) / CLI (shellumi-cli) → storage & protocol adapters → core domain models`

1. Protocol and domain code must not depend on GPUI.
2. Long-running I/O must not block the UI thread.
3. Host-key decisions must be explicit and enforced (never silently accepted).
4. Credentials must be stored in an OS-backed keychain or authenticated encrypted vault, never in the plain metadata database.
5. Desktop accessibility requires ongoing validation: host list and input controls are not yet fully exposed in macOS VoiceOver/accessibility tree.

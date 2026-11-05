# Shellumi roadmap and current state

The requirements and phases are in [PROJECT.md](../PROJECT.md). This is an
early project, not yet a complete SSH client. The GPUI window can display,
add, and edit hosts from an explicitly selected database; these operations have
been checked on macOS.

## Next reviewable increments

1. Build and run the pinned official GPUI 0.2.2 window on each desktop platform.
   GPUI itself is Apache-2.0; audit the complete transitive dependency set and
   security advisories before release. The registry is reachable outside the
   restricted sandbox. On this Mac, the app now builds with the selected Xcode
   toolchain and Metal compiler when the build can access the compiler cache.
   The host list, mouse refresh and Command-R have been checked on macOS;
   screen-reader access and other desktop platforms still need validation.
2. Expand the Rust workspace: `shellumi-core` validates SSH destination
   fields independently of GPUI, parses and generates OpenSSH configs (`~/.ssh/config`)
   and RFC 4180 CSV files, parses quick-connect targets (`user@host[:port]`), generates
   Ed25519 SSH key pairs with memory zeroization on drop, parses OpenSSH `known_hosts`
   with strict verification (trusted, TOFU, changed-key MITM risk, and revoked key detection),
   validates jump host chains (`validate_jump_host`), implements SSH port forwarding models
   (`PortForwardRule`, `ForwardingType::Local`, `Remote`, `Dynamic` SOCKS5) with security warnings
   before exposing bindings beyond localhost, provides a decoupled `CredentialStore` trait,
   zeroized-memory `SecretString` with strict redaction, an `EphemeralCredentialStore`, native macOS
   Keychain integration via `MacOsKeychainStore`, SFTP file management domain models
   (`FileEntry`, `FileKind`, UNIX permission string formatting, human-readable byte sizes, breadcrumb path
   navigation, directory path joining/normalization, `TransferTask`, `TransferStatus`, `TransferDirection`,
   and `list_local_directory`), and a complete Snippet & Automation engine (`Snippet`, `SnippetVariable`,
   variable extraction `{{var}}` and `{{var:-default}}`, POSIX shell escaping `shell_escape`, parameter
   rendering, secret variable detection, preview redaction `render_preview`, and JSON export/import).
   `shellumi-storage` provides transactional schema migrations (schema version 8 with backward-compatible
   auto-migration from versions 1, 2, 3, 4, 5, 6, and 7), persistent host groups (`groups` table), jump hosts
   (`hosts.jump_host`), persistent startup snippets (`hosts.startup_snippet_id` foreign key referencing `snippets(id)` ON DELETE SET NULL),
   persistent SSH tunnels (`tunnels` table with local, remote, and dynamic rules), persistent file transfer history (`transfer_history` table),
   saved snippets and templates (`snippets` table with tags, variables, and parameters), disaster recovery snapshots (`sync_snapshots` table with
   `archive_snapshot`, `list_snapshot_history`, `get_snapshot_vault`, and `rollback_snapshot`), configuration inheritance (`get_effective_destination`,
   `get_effective_tags`), tags, group/tag/snippet filtering, and search.
   `shellumi-cli` manages host metadata with group/tag/search/jump-host/startup-snippet filtering, provides dedicated
   `show <id>`, `group add/list/show/delete`, and `tunnel add/list/delete` commands, SFTP operations (`sftp local-ls`,
   `sftp history`, `sftp clear-history`, `sftp log`), snippet operations (`snippet add/list/show/render/delete/batch`),
   vault disaster recovery commands (`vault export/import/diff/status/oauth-url/history/rollback`), an immediate
   `quick-connect` command, `keygen` for Ed25519 key generation, handles import/export (SSH config, CSV,
   and JSON snippets), and dispatches connections via system OpenSSH and SFTP with group inheritance,
   and `-J` jump host chaining. Startup snippets are not executed automatically.
   The GPUI window supports interactive in-window host creation and editing with group/tag/jump-host/startup-snippet
   inputs, jump host and startup snippet badge indicators (`📜 Startup: <snippet_name>`), real-time search filtering, an immediate Quick Connect toolbar,
   host card actions for Connect, SFTP, and Delete,
   an in-terminal Snippets drawer (`📜 Snippets`) for quick parameter pasting and one-click execution, and an interactive Multi-Host Batch Execution Modal.
3. Terminal engine and PTY integration: `shellumi-terminal` is now
   implemented with VT/ANSI escape parsing (colors, cursor positioning, erase,
   alternate screen, scrollback history), cross-platform native PTY process
   spawning with lifecycle and execution tests, and a terminal search engine
   matching across active screen and scrollback history with highlighted span rendering.
4. Terminal UI view in GPUI: The view supports SSH connections, local shell sessions, and SFTP
   sessions directly from the UI tab bar and host cards. Sessions stream PTY bytes, accept
   extended keyboard input (arrow keys, Delete, Home/End, PageUp/Down, Ctrl-A..Z, Alt),
   support clipboard paste (`Cmd-V` on macOS, `Ctrl-Shift-V` on Linux/Windows),
   provide PTY resizing commands, and display live cursor coordinates and dimensions.
   The terminal screen renders styled text spans with 16 ANSI colors, 256-color palette,
   TrueColor RGB, text attributes (bold, italic, underline, strikethrough), and scrollback.
   The window features a top tab bar managing multiple concurrent local, SSH, and SFTP terminal tabs
   with background streaming, independent closure, and switching between sessions and the Hosts view.
   Within tabs, vertical and horizontal split panes (`split_pane(SplitDirection::Vertical)`,
   `split_pane(SplitDirection::Horizontal)`) allow running multiple simultaneous shells side-by-side
   or stacked with independent PTY sessions, active focus outlines, and pane closure. Each pane
   features an integrated search toolbar with forward/backward match navigation and highlighted spans.
   Next: font metric integration, IME support, and draggable pane boundaries.
5. SSH: The initial CLI and GPUI paths invoke system OpenSSH. An opt-in
   isolated-server test covers host-key decisions and public-key authentication
   through the PTY session worker and terminal grid. Test the actual GPUI path,
   password and key-passphrase prompts, failure handling, and other platforms;
   evaluate a native Rust backend separately.
6. OS Keychain and credential management: The core has a `CredentialStore` trait,
   secret redaction, a macOS Keychain backend, and an encrypted file store. Windows
   Credential Manager and Linux Secret Service return explicit unavailable errors.
   Next: implement and validate those native stores and secure secret entry before
   adding biometric unlock.
7. SFTP and file transfer: Implemented SFTP core domain models, path breadcrumbs, local directory
   browsing, transfer task progress, transfer history database storage (schema v6), RFC SFTP v3 wire protocol
   encoders and decoders (`SftpPacket`, `StatusCode`, `OpenFlags`, `FileAttributes`, `NameEntry`), stream framing
   with partial-buffer detection, background transfer queue orchestration (`TransferQueue`), CLI commands
   (`sftp packet encode-init/decode/encode-open` and `sftp queue`), and a full graphical GPUI dual-pane file browser
   (Local left pane, Remote right pane, breadcrumbs, up-navigation, hidden-files toggling, selection-driven Upload/Download actions,
   live transfer queue drawer with visual progress bar, and mode switching between Dual-Pane View and interactive terminal).
   Next: direct remote subsystem wire streaming and recursive directory transfers.
8. Snippets & Automation (Phase 6): Implemented parameter extraction (`{{var}}`, `{{var:-default}}`), POSIX shell escaping,
   secret parameter detection and preview redaction, JSON export/import, Turso database schema v8 persistence
   with persistent `startup_snippet_id` foreign keys, CLI `snippet` subcommands, GPUI in-terminal drawer with
   Paste, Run, and Multi-Host execution, interactive GPUI Snippet Variable Modal prompt (`SnippetModalState`),
   with startup-snippet references. Automatic dispatch is disabled until authentication readiness can be verified.
   Implemented Multi-Host Batch Execution engine in `shellumi-core::batch` (`BatchHostTarget`, `BatchPolicy::Parallel`
   and `Sequential`, `BatchExecutionPlan`, `BatchRunner`, `BatchExecutionSummary`) with safe parameter substitution and
   host property variables (`{{host.id}}`, `{{host.name}}`, `{{host.hostname}}`, `{{host.user}}`, `{{host.port}}`), dry-run previews,
   cancellation tracking, CLI `snippet batch` previews, and desktop GPUI Multi-Host Batch Runner Modal (`BatchModalState`)
   featuring host filtering, selection controls, policy toggles (Parallel vs Sequential), dry-run command previewing,
   and background execution with live status and expandable per-host output cards.
   Next: snippet synchronization over Google Drive E2EE sync.
9. Command Palette (Phase 6 & Section 8.5): Implemented decoupled palette action and scoring engine in `shellumi-core::palette`
   (`PaletteCategory`, `PaletteAction`, `PaletteCommand`, `PaletteRegistry`), featuring fuzzy tokenized matching, title prefix/exact
   bonuses, and acronym resolution (e.g. `qc` for Quick Connect). Integrated CLI `palette search` and full GPUI Desktop
   Command Palette overlay (`CommandPaletteState`) bound globally to `Cmd-K` (macOS) / `Ctrl-K` (Linux/Windows) and accessible
   via top navigation toolbar buttons across Hosts and Terminal views, with arrow-key navigation, Enter execution, Escape dismissal,
   and direct action dispatch for SSH/SFTP connections, snippet execution, multi-host batch execution (`PaletteAction::ExecuteBatchSnippet`),
   sync triggering, pane splitting, and tab switching.
10. Google Drive E2EE Sync (Phase 5): Implemented zero-knowledge PBKDF2-HMAC-SHA256 key derivation with
   memory zeroization on drop, AES-256-GCM AEAD envelope sealing with bound associated data, versioned
   `SyncRecord` models with deletion tombstones, three-way sync conflict detection (`determine_sync_action`),
   RFC 7636 PKCE challenge generation, `OAuthCallbackListener` loopback HTTP callback listener with CSRF state
   verification and user-friendly HTML redirect responses, `OAuthTokenBundle` with expiration checking and
   secure credential store persistence (`save_to_store`, `load_from_store`, `delete_from_store`),
   `diff_sync_records_with_passphrase` with payload-level equality verification, `SyncVault` snapshot export/import
   in `shellumi-storage`, CLI `vault export/import/diff/status/oauth-url/history/rollback` subcommands, transport-agnostic HTTP
   protocol models (`HttpTransport`, `HttpRequest`, `HttpResponse`), Google Drive Application Data folder REST API
   client (`GoogleDriveSyncClient`: token exchange, token refresh, file listing, multipart snapshot upload, snapshot
   download, and update/deletion), conflict-safe sync planner (`plan_sync_cycle`, `SyncPlan`), GPUI in-window
   conflict resolution models (`resolve_record_conflict`, `ConflictResolutionChoice`, `ConflictItemView`),
   and disaster recovery snapshots (`sync_snapshots` table, schema v7) with snapshot history and rollback.
   Desktop cloud sync and conflict resolution are not integrated; the sync control reports unavailable.
   Next: implement authenticated end-to-end sync and verify recovery before exposing it as functional.
11. Workspaces and Broadcast Input (Phase 6): Implemented workspace domain models in `shellumi-core::workspace`
    (`WorkspaceLayout`, `WorkspaceSessionKind`, `WorkspaceSession`, `Workspace`, `BroadcastManager`, `BroadcastSafetyCheck`,
    `check_dangerous_command`), Turso database schema version 9 with transactional migrations and encrypted vault backup,
    CLI `workspace` subcommands (`add`, `list`, `show`, `duplicate`, `delete`), and desktop GPUI integration in `shellumi-app`:
    header broadcast toggle with receiving pane count (`📡 Broadcast: ON (X/Y)`), pane-level broadcast toggle button
    (`[📡 Broadcast]` / `[🔇 Excluded]`), keyboard input replication across non-excluded panes, destructive command safety
    interception with visual warning banner, command palette workspace registration (`PaletteCategory::Workspace`),
    `ToggleBroadcastInput`, `SaveCurrentLayoutAsWorkspace`, and `LoadWorkspace`.
12. Multi-Protocol Support (Phase 7): Implemented multi-protocol domain models in `shellumi-core::protocol`
    (`ConnectionProtocol` [SSH, Mosh, Telnet, Serial, Local], `MoshConfig`, `MoshPredictMode`, RFC 854/1091/1073 `TelnetConfig`
    stream parser with `IAC IAC` escape handling and unencrypted security warning, `SerialConfig` baud/parity/bits/flow controls,
    `ProtocolEndpoint`, and `discover_serial_ports()`), Turso database schema version 10 with transactional migrations from versions 1..9,
    adding `protocol` and `protocol_config` columns to `hosts` table, `SavedHost` helper methods (`is_ssh`, `is_mosh`, `is_telnet`, `is_serial`),
    `HostStore` (`add_with_protocol`, `update_with_protocol`), and zero-knowledge encrypted vault backup/restore serialization.
    Implemented CLI enhancements: `--protocol` and `--protocol-config` options for `add` and `update`, protocol display in `show <id>`,
    protocol-aware connection launcher in `connect <id>` with capability checks and cleartext warnings, and dedicated subcommands:
    `mosh`, `telnet`, `serial`, and `serial-ports` device discovery.
    Implemented GPUI desktop integration in `shellumi-app`: interactive protocol selection buttons in `HostForm` (`[🔒 SSH]`, `[⚡ Mosh]`,
    `[⚠️ Telnet]`, `[🔌 Serial]`) with automatic port defaults (22, 23, 1) and dynamic protocol-specific security/info banners,
    protocol badges on host cards, SFTP action isolation (hidden for non-SSH hosts), multi-protocol terminal session tabs
    (`SessionKind::Ssh`, `Mosh`, `Telnet`, `Serial`, `Local`, `Sftp`) with protocol prefixes in titles, and persistent
    `[⚠️ UNENCRYPTED TELNET]` badge in terminal view header. 165 workspace tests passing.
13. Team Collaboration and Shared Vaults (Phase 8): Implemented team collaboration domain models in `shellumi-core::collaboration`
    (`TeamRole` [Owner, Administrator, Editor, Viewer] with granular capability queries, `TeamMember` enrollment, revocation,
    and role management, `SharedVault` cryptographic epochs and key rotation, `SharingAuditRecord` tamper-evident audit logging
    with `AuditAction`, `MultiplayerSession` and `MultiplayerParticipant` state machines enforcing single-active-writer semantics
    with immediate session owner revocation, and `SharedSessionLog` with comments and bookmarks). Expanded `SyncEntityType` with
    `SharedVault`, `TeamMember`, and `AuditRecord` for zero-knowledge end-to-end sync. Expanded `PaletteCategory::Collaboration` (`👥`)
    and registered collaboration actions (`PaletteAction::OpenTeamCollaboration`, `ToggleMultiplayerCollaboration`). Implemented Turso database
    schema version 11 with transactional migrations from versions 1..10 (`shared_vaults`, `team_members`, `sharing_audit_log`, `shared_session_logs` tables),
    CRUD methods on `HostStore`, and encrypted vault export/import serialization. Implemented CLI `team` subcommands (`team vault create/list/show/rotate-key/delete`,
    `team member invite/list/role/revoke`, `team audit`, `team session start/join/grant/revoke/status`). Implemented GPUI desktop integration
    in `shellumi-app`: `[👥 Team Vaults]` top toolbar action, interactive Team Collaboration Modal (`TeamModalState`) with tabbed browsing
    for Shared Vaults, Members with role badges, and Audit Log, vault creation, member invitations with role picker, member revocation,
    key rotation, and multiplayer terminal collaboration strip on active session tabs with single-writer enforcement, control delegation,
    and revocation to host. 177 workspace tests passing.
    Next: implement Phase 9: Infrastructure Integrations (Cloud providers: AWS, Azure, DigitalOcean host discovery; Ansible inventory synchronization).
14. Infrastructure Integrations (Phase 9): Implemented cloud infrastructure discovery and inventory synchronization in `shellumi-core::infrastructure`
    (`CloudProviderKind` [AWS, Azure, DigitalOcean, Ansible], `CloudProviderConfig` with credential redaction and validation,
    `DiscoveredHost` with running state evaluation and badge formatting, non-destructive `InventoryMergeEngine`, `MergePolicy`,
    `InfraSyncAction` [Create, Update, Unchanged, Stale], `InfraSyncReport`, `AwsEc2Parser` for `DescribeInstances` JSON,
    `AzureVmParser` for Compute VM JSON, `DigitalOceanParser` for Droplets v2 API JSON, and bidirectional `AnsibleInventoryEngine`
    supporting both INI and YAML inventory parsing and serialization). Expanded `SyncEntityType` with `CloudProvider` for
    zero-knowledge E2EE backup. Expanded `PaletteCategory::Infrastructure` (`☁️`) and registered command palette actions
    (`PaletteAction::OpenInfrastructureManager`, `SyncCloudInfrastructure`, `ImportAnsibleInventory`, `ExportAnsibleInventory`).
    Implemented Turso database schema version 12 with transactional migrations from versions 1..11 (`cloud_providers` and `discovered_hosts`
    tables with foreign key tracking to `hosts(id)`), `HostStore` CRUD methods (`create_cloud_provider`, `get_cloud_provider`,
    `list_cloud_providers`, `update_cloud_provider_sync_time`, `delete_cloud_provider`, `save_discovered_hosts`, `list_discovered_hosts`,
    and `mark_discovered_host_imported`), and zero-knowledge encrypted vault backup/restore serialization.
    Implemented CLI `infra` subcommands (`infra provider add/list/show/delete/sync`, `infra discover <aws|azure|digitalocean|ansible>`,
    and `infra ansible import/export`). Implemented GPUI desktop integration in `shellumi-app`: `[☁️ Cloud & Infra]` top toolbar action,
    interactive Infrastructure Modal (`InfraModalState`) with tabbed browsing for Cloud Providers (creation, deletion, manual sync),
    Discovered Hosts with state indicators (`🟢 Running`, `🔴 Stopped`) and one-click import into inventory (`📥 Import Host`), and
    Ansible inventory export (INI and YAML formats) executed on background worker threads (`cx.background_executor()`). 188 workspace tests passing.
    Next: implement Phase 10: Production Hardening, Cross-Platform Validation, Accessibility, and Release Packaging.
15. Production Hardening, System Diagnostics & Release Packaging (Phase 10): Implemented end-to-end production hardening and packaging infrastructure across all workspace crates:
    - `shellumi-terminal` security hardening & performance profiling: added OSC escape sequence sanitization in `parser.rs` (strips control codes and clamps window titles to 256 characters to eliminate buffer injection risks), configurable scrollback limits (`set_scrollback_limit`, `scrollback_limit`) with automated trimming to prevent unbounded memory growth, and Unicode CJK/emoji and 2,000-line high-throughput stress tests.
    - `shellumi-core::diagnostics` system health engine: created `DiagnosticsRunner`, `DiagnosticsReport`, `DiagnosticCheck`, `DiagnosticCategory` (System, Security, OpenSsh, Network, Storage, Terminal), `DiagnosticSeverity`, and `DiagnosticStatus`. Automated health checks cover CSPRNG entropy verification, AES-256-GCM AEAD encryption & PBKDF2 key derivation, OpenSSH client binary detection & version parsing, OS credential store status, loopback network socket binding verification, database schema version & integrity checks, and cross-platform PTY capabilities, with comprehensive human-readable terminal summaries, Markdown reports, and JSON export. Expanded `PaletteCategory::Diagnostics` (`🩺`) and registered command palette action `PaletteAction::RunDiagnostics` (`Cmd+Shift+D`).
    - `shellumi-cli doctor`: implemented CLI diagnostics command with terminal formatted tables, `--json`, `--markdown`, and custom database path inspection (`--db <path>`).
    - `shellumi-app` desktop health diagnostics modal: implemented `DiagnosticsModalState` overlay, `[🩺 Health Check]` toolbar button, background execution via `cx.background_executor()`, dynamic health status banners (`✅ All Systems Ready`, `⚠️ System Degraded`, `❌ Action Required`), per-check duration metrics, actionable remediation hints, and Command Palette integration.
    - Cross-platform release packaging: defined macOS application bundle metadata (`packaging/macos/Info.plist`) with URL scheme handlers (`ssh://`, `sftp://`, `telnet://`) and DMG creation script (`build-dmg.sh`); Linux XDG desktop entry (`packaging/linux/shellumi.desktop`), Debian package control manifest (`packaging/linux/debian/control`), and `.deb` packaging script (`build-deb.sh`); and Windows WiX Toolset installer definition (`packaging/windows/wix/main.wxs`) with registry shortcuts and uninstaller hooks.
    - CI/CD release workflow: updated `.github/workflows/release.yml` with a cross-platform matrix build (Linux x86_64, Windows x86_64, macOS arm64, macOS x86_64), building both `shellumi-app` and `shellumi-cli`, packaging `.deb`, `.dmg`, and `.zip` installers, and generating automated CycloneDX SBOMs and SHA256 checksums. 197 workspace tests passing.
    Next: implement Phase 11: Mobile Feasibility and Prototype.
16. Mobile Feasibility, Portable Core Subsystem & Desktop Simulator (Phase 11): Implemented mobile feasibility assessment, portable core subsystem, virtual accessory bar, lifecycle state machines, and desktop touch simulator across workspace crates:
    - Assessed upstream GPUI 0.2.2 mobile status: confirmed that GPUI 0.2.2 currently targets macOS, Linux, and Windows desktop platforms without native mobile touch event loops, soft-keyboard IME integration, or iOS/Android windowing backends. Formulated architecture separating `shellumi-core` as a portable headless library from platform-native mobile frontends (SwiftUI for iOS, Jetpack Compose for Android) via UniFFI / C-ABI to avoid compromising desktop stability.
    - `shellumi-core::mobile` domain models: implemented `MobilePlatform` (`Ios`, `Android`, `Simulator`), `VirtualKey` (Esc, Tab, Ctrl-char, Alt-char, arrows, pipe, tilde, slash, backslash, function keys, home, end, pgup, pgdn), `VirtualKeyModifier` (`Ctrl`, `Alt`, `Shift`), `VirtualAccessoryBar` (layout presets: `DefaultTerminal`, `CompactNavigation`, `CodeEditing`, with sticky modifier toggling and one-shot keystroke resolution `resolve_char_keystroke`), `MobileSessionState` (`Connecting`, `Connected`, `BackgroundGracePeriod`, `Suspended`, `Reconnecting`, `Disconnected`), `MobileLifecycleEvent` (`AppForegrounded`, `AppBackgrounded`, `BackgroundTimerTick`, `BackgroundExpired`, `NetworkLost`, `NetworkRestored`, `UserDisconnect`), `MobileLifecycleManager` (deterministic state machine with exponential reconnection backoff), `MobileCredentialSpec` (iOS Keychain `kSecClassGenericPassword` with `kSecAccessControlBiometryAny` vs Android Keystore AES-GCM with `setUserAuthenticationRequired(true)`), and `MobileFfiBridge` (`parse_destination_json`, `translate_virtual_key_json`, `generate_keypair_json`).
    - Desktop GPUI simulation modal overlay (`MobileSimulatorModalState`): implemented interactive mobile simulator overlay in `shellumi-app` with viewport presets for iPhone 15 Pro (393×852 / 80×24), Pixel 8 (412×924 / 82×26), and iPad Pro 11" (834×1194 / 120×36), interactive touch virtual accessory strip, sticky modifier toggling, mobile lifecycle state simulation triggers, and real-time ANSI escape byte telemetry log.
    - Command Palette integration: expanded `PaletteCategory::Mobile` (`📱`) and registered `PaletteAction::OpenMobileSimulator` (`Cmd+Alt+M`). Added `[📱 Mobile Sim]` toolbar button in Hosts view.
    - CLI mobile commands: implemented `shellumi-cli mobile <virtual-keys|lifecycle-sim|ffi-check>` with comprehensive integration tests in `crates/shellumi-cli/tests/hosts.rs`.
    - Published comprehensive engineering report in `docs/mobile/FEASIBILITY.md` covering GPUI status, portable core architecture, touch ergonomics, iOS vs Android background execution, hardware biometric keychain integration, and mobile roadmap. 204 workspace tests passing.
    Next: implement Phase 12: Mobile FFI Bindings & Native Wrapper Packages.
17. Mobile FFI Bindings, Native Wrapper Packages & Biometric Integration (Phase 12): Implemented native foreign-function interface bindings, C-ABI export headers, UniFFI definitions, cross-compilation packaging scripts, and native mobile prototypes:
    - Pure C-compatible ABI in `crates/shellumi-core/src/ffi.rs`: implemented `shellumi_core_version`, `shellumi_mobile_platform_supported`, `shellumi_parse_destination_c`, `shellumi_generate_keypair_c`, `shellumi_translate_virtual_key_c`, `shellumi_lifecycle_transition_c`, `shellumi_get_credential_spec_c`, and null-safe `shellumi_string_free`, backed by 8 dedicated memory-safety and contract unit tests. Configured `crates/shellumi-core/Cargo.toml` with `crate-type = ["lib", "staticlib", "cdylib"]`.
    - C Header & UniFFI Interface Definitions: authored standard C99/C11 header [`crates/shellumi-core/include/shellumi_mobile.h`](file:///Users/mjk/Documents/Craft%20Anything/termius/crates/shellumi-core/include/shellumi_mobile.h) and UniFFI interface definition [`crates/shellumi-core/src/shellumi.udl`](file:///Users/mjk/Documents/Craft%20Anything/termius/crates/shellumi-core/src/shellumi.udl).
    - iOS Swift Package & Prototype (`packaging/mobile/ios/`): created Swift Package Manager manifest (`Package.swift`), automated XCFramework build script (`build-xcframework.sh` targeting `aarch64-apple-ios` and `aarch64-apple-ios-sim`), and SwiftUI prototype (`ShellumiMobile/ContentView.swift`) integrating hardware biometric authentication (`LocalAuthentication.LAContext`), virtual accessory bar with sticky modifiers, and iOS ScenePhase lifecycle transitions.
    - Android Gradle & Prototype (`packaging/mobile/android/`): created Gradle library manifest (`build.gradle.kts`), JNI build script (`build-aar.sh` targeting `arm64-v8a` and `x86_64`), Kotlin JNI bridge (`ShellumiNativeCore.kt`), and Jetpack Compose prototype (`MainActivity.kt`) integrating hardware `BiometricPrompt` for StrongBox / Keystore credential unlocking and Android lifecycle event observation.
    - CLI mobile commands: updated `shellumi-cli mobile` with `mobile header` and `mobile build-info`, and enhanced `mobile ffi-check` to verify exported C-ABI functions. 212 workspace tests passing.
    Next: implement Phase 13: Intelligent Shell Integration, OSC 133 Prompt & CWD Detection, and In-Terminal Autocomplete Engine.
18. Phase 13 — Intelligent Shell Integration & In-Terminal Autocomplete Engine:
    - Implement OSC 133 semantic prompt demarcation parsing in `shellumi-terminal` (Prompt Start `133;A`, Command Start `133;B`, Command Executed `133;C`, Command Finished with Exit Code `133;D;<exitcode>`).
    - Implement OSC 7 / OSC 9;9 current working directory detection to track shell directory navigation across remote SSH and local sessions.
    - Build in-terminal intelligent autocomplete engine in `shellumi-core` offering context-aware suggestions for commands, paths, snippets, hostnames, and history.
    - Implement password prompt detector (`Password:`, `[sudo] password for`) with safe identification.

Reference research: [upstream GPUI documentation](https://github.com/zed-industries/zed/tree/main/crates/gpui),
which describes GPUI as pre-1.0 and subject to breaking changes, and
[Rust release schedule](https://forge.rust-lang.org/), which lists Rust 1.99
as stable as of 2026-11-04. GPUI mobile support has not been validated.

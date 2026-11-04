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
- **Credential & Secret Architecture:** Provides a decoupled `CredentialStore` trait for secure credential management, a `SecretString` type that zeroizes heap buffers on drop and redacts secret values in `Debug` and `Display` formatting to prevent accidental log leakage, an in-memory `EphemeralCredentialStore` with zeroization for tests, native OS platform keychains (`MacOsKeychainStore` leveraging Apple's Security Framework Keychain Services API, `WindowsCredentialManagerStore` targeting the Windows Credential Manager API, and `LinuxSecretServiceStore` targeting the FreeDesktop.org Secret Service D-Bus API), a portable `EncryptedFileCredentialStore` utilizing AES-256-GCM AEAD encryption and PBKDF2-HMAC-SHA256 key derivation with atomic file updates and memory zeroization, an automated `default_credential_store()` factory providing platform-aware routing with safe fallbacks, and persistent OAuth token management (`OAuthTokenBundle::save_to_store`, `load_from_store`, `delete_from_store`) securing Google Drive synchronization credentials.
- **SFTP Domain Models & Navigation:** Provides UI-independent models for file entries (`FileEntry`, `FileKind`), UNIX file permission string formatting (`drwxr-xr-x`), human-readable byte sizes (`format_bytes`: B, KB, MB, GB, TB), breadcrumb navigation generation (`PathUtils::breadcrumbs`), safe path joining and traversal sanitization (`PathUtils::join_path`), transfer task progress calculations (`TransferTask`, `TransferStatus`, `TransferDirection`, `ConflictResolution`), and local directory browsing (`list_local_directory`).
- **Snippet & Automation Engine:** Implements UI-independent models for parameterized snippets (`Snippet`, `SnippetVariable`), automated variable extraction supporting `{{var}}` and `{{var:-default}}` syntax, POSIX shell argument escaping (`shell_escape`) to prevent shell injection, safe parameter substitution (`render_command`), automatic secret variable classification (flags variables named *pass*, *secret*, *token*, *key*), preview redaction (`render_preview`) substituting secret values with `[REDACTED]`, and standard JSON library serialization and deserialization (`export_snippets_json`, `parse_snippets_json`).
- **End-to-End Encrypted Cloud Sync Engine:** Implements zero-knowledge cryptography using `ring`: PBKDF2-HMAC-SHA256 key derivation with memory zeroization on drop, authenticated encryption with associated data (AEAD) using AES-256-GCM (`seal_payload`, `open_payload`), `EncryptedEnvelope` with bound associated data (`entity_type:object_id`) preventing ciphertext transposition, versioned `SyncRecord` models with deletion tombstones, three-way sync conflict detection (`determine_sync_action`), RFC 7636 PKCE challenge generation (`generate_pkce_challenge`), Google OAuth 2.0 authorization URL builder for the restricted `drive.appdata` scope, local loopback HTTP OAuth callback server (`OAuthCallbackListener`), callback request parsing with CSRF state verification (`parse_oauth_callback_request`), token bundle management (`OAuthTokenBundle`), record difference engine with payload decryption (`diff_sync_records_with_passphrase`), `SyncVault` encrypted snapshot serialization, transport-agnostic HTTP protocol models (`HttpTransport`, `HttpRequest`, `HttpResponse`), Google Drive Application Data folder REST API client (`GoogleDriveSyncClient` handling token exchange, token refresh, file listing, multipart snapshot upload, snapshot download, and file update/deletion), Drive file metadata (`DriveFileMetadata`), and conflict-safe sync cycle planner (`plan_sync_cycle`, `SyncPlan`).
- UI-independent and network-independent; contains no GUI code.

### `shellumi-terminal`
- **Screen & Grid Management:** Manages character cells (`Cell`), 16 standard ANSI colors, 256-color palette, and 24-bit TrueColor RGB (`Color`), text attributes (bold, dim, italic, underline, inverse, strikethrough), cursor positioning, and scrollback history. Converts character cells into minimal contiguous styled spans (`StyledSpan`) for efficient UI rendering.
- **Escape Sequence Handling:** Implements `vte::Perform` to parse ANSI/VT sequences, cursor movement (CUU, CUD, CUF, CUB, CNL, CPL, CHA, CUP), screen/line clearing (ED, EL), scrolling (SU, SD), alternate screen buffers (DECSET/DECRST 1049), and window titles (OSC 0/2).
- **Cross-Platform PTY:** Spawns native pseudo-terminals (`portable-pty`) on macOS, Linux, and Windows, supporting shell auto-detection, custom commands, arguments, working directories, environment variables, resizing, and child process lifecycle.

### `shellumi-storage`
- Uses the embedded, in-process Turso Database engine (`turso` crate).
- Stores host metadata only; credentials must never be written to this database.
- Enforces transactional schema migrations via `PRAGMA user_version` (schema version 6, with automatic migration from versions 1, 2, 3, 4, and 5) and rejects unsupported future schemas.
- Supports persistent host groups (`groups` table), jump hosts (`hosts.jump_host`), persistent SSH tunnels (`tunnels` table), persistent file transfer history (`transfer_history` table), and persistent snippets (`snippets` table: id, name, command, description, tags, variables, created_at, updated_at).
- Provides configuration inheritance through `get_effective_destination` and `get_effective_tags`, automatically applying group defaults when host fields are omitted.
- Exposes async CRUD operations: `add`, `update`, `list`, `list_by_group`, `list_by_tag`, `search`, and `delete` for hosts, plus `add_group`, `update_group`, `get_group`, `list_groups`, and `delete_group`, `add_tunnel`, `update_tunnel`, `list_tunnels`, `list_tunnels_for_host`, and `delete_tunnel`, `log_transfer`, `update_transfer`, `list_transfers`, and `clear_transfer_history`, as well as `add_snippet`, `update_snippet`, `delete_snippet`, `get_snippet`, `list_snippets`, `list_snippets_by_tag`, and `search_snippets`.
- Implements zero-knowledge vault synchronization methods: `export_vault` / `export_vault_with_iterations` to seal all database entities into an encrypted snapshot, and `import_vault` to decrypt and restore or update entities with an `ImportVaultSummary`.

### `shellumi-cli`
- Provides command-line host management: `list`, `add`, `update`, `delete`, with `--group`, `--tag`, `--jump-host`, and `--search` filtering flags.
- Provides dedicated group management: `group add`, `group list`, `group show`, and `group delete`.
- Provides dedicated tunnel management: `tunnel add`, `tunnel list`, and `tunnel delete`, with automatic localhost exposure detection and security warnings.
- Provides dedicated SFTP commands: `sftp local-ls <path> [--all]`, `sftp history [--host-id <id>] [--limit <n>]`, `sftp clear-history [--host-id <id>]`, and `sftp log <upload|download> <local-path> <remote-path> <bytes>`.
- Provides dedicated snippet management: `snippet add <name> <cmd> [--desc <desc>] [--tags <t1,t2>]`, `snippet list [--tag <tag>] [--search <query>]`, `snippet show <id>`, `snippet render <id> [--var key=value] [--preview]`, and `snippet delete <id>`.
- Provides zero-knowledge vault management: `vault export <output-file> [--passphrase <p>] [--device-id <d>] [--iterations <n>]`, `vault import <input-file> [--passphrase <p>]`, `vault diff <input-file> [--passphrase <p>] [--device-id <d>]`, `vault status`, and `vault oauth-url [--client-id <id>] [--redirect-uri <uri>]`.
- Provides immediate `quick-connect <user@host[:port]>` launching OpenSSH without database persistence.
- Provides Ed25519 key pair generation via `keygen [--comment <comment>]`.
- Provides OpenSSH config import and export: `import-ssh-config <config-path>` and `export-ssh-config`.
- Provides CSV import and export: `import-csv <csv-path>` and `export-csv`.
- Provides an initial `connect` command that safely dispatches to system OpenSSH with strict flags (`-o StrictHostKeyChecking=ask`, `-o ForwardAgent=no`, `-p <port>`, `-l <username>`), resolving effective destination parameters from parent groups and chaining jump hosts via `-J <jump>`.
- Validates inputs before opening or mutating the database.

### `shellumi-app`
- GPU-accelerated desktop user interface built with GPUI 0.2.2.
- **Threading model:** All database operations execute on background tasks via `cx.background_executor().spawn()` with an isolated current-thread Tokio runtime, keeping the GPUI UI thread responsive.
- **Form state management:** Uses an interactive `HostForm` model for adding and editing hosts, including Group, Tag, and Jump Host fields.
- **Search & Filter:** Features a real-time host search and filtering input bar that dynamically filters hosts by name, hostname, username, group, jump host, or tags.
- **Quick Connect:** Features an immediate Quick Connect bar (`user@host[:port]`) in the Hosts management view.
- **Sync Status & Trigger:** Displays an active Cloud Sync status pill (`☁ Sync: Local only`, `☁ Sync: Up to date`, `☁ Syncing…`, `⚠ Sync conflict`) in the Hosts view with one-click manual synchronization execution dispatched to background tasks without UI blocking.
- **Host list display:** Renders group badges, jump host indicators (`via <jump>`), host endpoints, tag pills, and Edit, Connect (SSH), and SFTP action buttons for each saved host.
- **Connection dispatch:** Connects to hosts with automatic `-J` jump host chaining and group parameter inheritance for SSH and SFTP.
- **Text input:** Implements a native-feeling `TextInput` element supporting mouse selection, drag selection, cursor navigation, keyboard shortcuts, copy/cut/paste, and input marking.
- **Shortcuts:** Supports platform-aware keybindings (e.g. `Cmd-R` on macOS, `Ctrl-R` on Linux/Windows for refreshing the host list).
- **Multi-Tab Sessions:** Manages multiple concurrent terminal sessions (`SessionTab`)
  via a top tab bar. Users can switch between open local shells, remote SSH connections,
  interactive SFTP sessions, and the "Hosts" management view without disconnecting running sessions. Each tab maintains
  its own PTY worker, VT parser state, 2D screen grid, and scrollback history. Tabs can be
  opened, switched, or closed independently with automatic active tab index tracking.
- **Split Panes & Layouts:** Supports vertical and horizontal split panes within any session tab
  (`split_pane(SplitDirection::Vertical)`, `split_pane(SplitDirection::Horizontal)`). Each pane runs
  an independent PTY session with its own terminal grid, search state, and active focus outline.
- **Terminal Search:** Each terminal pane includes an integrated search toolbar with query input,
  match count (`1/N matches`), forward and backward navigation (`↑` / `↓`), and highlighted span rendering
  in the active terminal buffer and scrollback history.
- **Snippets Drawer & Quick Automation:** A collapsible `📜 Snippets` toolbar button opens an in-session
  drawer above the active terminal grid, displaying saved snippet chips with syntax previews. Each snippet
  offers one-click "Paste" (streaming into the active PTY without carriage return) and "Run" (dispatching
  with carriage return).
- **Terminal sessions:** A threaded session worker (`Session`) owns a PTY process,
  streams bounded output events to GPUI tasks via `async_channel`, and handles
  non-blocking input, resizing (`SessionCommand::Resize`), and lifecycle termination.
  Supports both SSH sessions (launching OpenSSH with strict flags via `shellumi-core`)
  and local shell sessions (auto-detecting system default shell). The UI provides
  a live monospace terminal grid with styled spans (foreground/background colors,
  bold, italic, underline, strikethrough, scrollback), cursor coordinates/dimensions
  in a status bar, clipboard paste (`Cmd-V` / `Ctrl-Shift-V`), and extended key translation.
- **Verification:** An opt-in [isolated OpenSSH test](development/ssh-integration.md)
  exercises host-key confirmation and public-key authentication through the PTY
  worker and terminal grid. Unit tests cover endpoint formatting, host search/group/tag
  filtering, ANSI key mapping, clipboard paste detection, styled text span rendering,
  tab index lifecycle, and PTY resize dispatch.

## Dependency direction and engineering rules

The intended dependency hierarchy is:
`UI (shellumi-app) / CLI (shellumi-cli) → storage & protocol adapters → core domain models`

1. Protocol and domain code must not depend on GPUI.
2. Long-running I/O must not block the UI thread.
3. Host-key decisions must be explicit and enforced (never silently accepted).
4. Credentials must be stored in an OS-backed keychain or authenticated encrypted vault, never in the plain metadata database.
5. Desktop accessibility requires ongoing validation: host list and input controls are not yet fully exposed in macOS VoiceOver/accessibility tree.

# Shellumi

![Shellumi logo](resources/logo.png)

Shellumi is an early-stage, open-source desktop SSH client project written in
Rust. Its goal is to provide a fast, local-first tool for administering remote
infrastructure, with a native GPU-accelerated interface built using GPUI.
Shellumi is intended to remain free to use and does not require an account for
local operation.

**Status: experimental.** The GPUI window manages host metadata and has an
initial SSH terminal backed by the system OpenSSH client. Its PTY session path
has been tested with an isolated OpenSSH server, but the full window interaction
has not. Shellumi is not ready for everyday or production administration.

## Current progress

| Area | Current state |
| --- | --- |
| Desktop interface | The GPUI host manager supports saved hosts with multi-protocol support (SSH, Mosh, Telnet, Serial), protocol badges on host cards, an interactive protocol selector with automatic port defaults (22, 23, 1) and dynamic security notices, action gating (SFTP restricted to SSH), and terminal tabs across protocols (`SessionKind::Ssh`, `Mosh`, `Telnet`, `Serial`, `Local`, `Sftp`) with unencrypted Telnet warning badges. The top toolbar provides a `[👥 Team Vaults]` action opening an interactive Team Collaboration Modal with tabbed management for Shared Vaults, Members with role badges, and tamper-evident Audit Logs. Active terminal sessions feature a Multiplayer Collaboration strip (`👥 Collaborate`) enforcing single-active-writer control with one-click delegation and immediate host revocation. Startup snippets are not automatically executed; cloud sync is unavailable. Screen-reader access and cross-platform behavior need validation. |
| Connection & SFTP model | The core provides destination validation, multi-protocol models (`ConnectionProtocol`, `MoshConfig`, RFC 854/1091/1073 `TelnetConfig` stream parser with `IAC IAC` escape handling and unencrypted warning, `SerialConfig`, `ProtocolEndpoint`, and `discover_serial_ports`), Team Collaboration domain models (`TeamRole` [Owner, Admin, Editor, Viewer], `TeamMember`, `SharedVault` with cryptographic epochs, `SharingAuditRecord` with `AuditAction`, `MultiplayerSession` and `MultiplayerParticipant` state machines, and `SharedSessionLog`), import/export models, known-host parsing, terminal and SFTP building blocks, and credential abstractions. macOS Keychain is implemented but environment-dependent; Windows Credential Manager and Linux Secret Service return an explicit unavailable error until implemented. |
| Terminal & PTY | The GPUI window supports running multiple concurrent SSH sessions, local shell sessions, and SFTP sessions in a tabbed workspace with vertical and horizontal split panes (`split_pane(SplitDirection::Vertical/Horizontal)`), independent PTY sessions per pane, in-pane search toolbar with match navigation (`↑`/`↓`) and highlighted spans, multi-pane Broadcast Input (`📡 Broadcast: ON (X/Y)`) with keyboard replication, pane-level exclusions (`[📡 Broadcast]` / `[🔇 Excluded]`), destructive command safety interception (`rm -rf /`, `mkfs`, raw block writes, fork bombs, reboot/shutdown) with warning banners, real-time multiplayer terminal collaboration strips with writer status badges (`✍️ Active Writer: @user`) and revocation controls, resizing commands, clipboard paste (`Cmd-V` / `Ctrl-Shift-V`), extended ANSI navigation keys, styled text span rendering (16/256/TrueColor RGB and text attributes), scrollback buffer, a live cursor coordinate/dimension status bar, and a quick-action Snippets drawer with Paste, Run, and Multi-Host batch execution into active terminal panes. |
| Local storage | An async Turso Database crate supports transactional schema migrations (schema version 11 with auto-migrations from versions 1, 2, 3, 4, 5, 6, 7, 8, 9, and 10), persistent host groups (`groups` table), jump hosts (`hosts.jump_host`), persistent startup snippets (`hosts.startup_snippet_id` foreign key referencing `snippets(id)` ON DELETE SET NULL), persistent SSH tunnels (`tunnels` table), persistent file transfer audit history (`transfer_history` table), saved snippets and templates (`snippets` table with tags, parameters, and variable schema), persistent workspaces (`workspaces` table with layouts, sessions, and broadcast settings), protocol persistence (`hosts.protocol` and `hosts.protocol_config`), shared vaults and team collaboration (`shared_vaults`, `team_members`, `sharing_audit_log`, `shared_session_logs` tables), disaster recovery snapshots with history pruning (`sync_snapshots` table, `archive_snapshot`, `list_snapshot_history`, `get_snapshot_vault`, `rollback_snapshot`), configuration inheritance (`get_effective_destination`, `get_effective_tags`), tags, group/tag filtering, search, CRUD operations, and zero-knowledge encrypted vault backup export and restore (`export_vault`, `import_vault`). Credentials are never written to this database. |
| Host & SFTP CLI | The CLI manages hosts, groups, tunnels, snippets, workspaces (`workspace add/list/show/duplicate/delete`), and team collaboration (`team vault create/list/show/rotate-key/delete`, `team member invite/list/role/revoke`, `team audit`, `team session start/join/grant/revoke/status`), supports multi-protocol options (`--protocol <ssh|mosh|telnet|serial>`, `--protocol-config <json>`), provides dedicated subcommands (`mosh`, `telnet`, `serial`, `serial-ports`), and dispatches connections via system clients with cleartext warnings for Telnet. Batch snippets support preview only; live CLI batch execution is not implemented. |
| Tests | 177 unit and integration tests cover core, storage, terminal, CLI, and app behavior. A real-server SSH test is opt-in. Native keychains and all desktop platforms require separate verification. |
| Releases | A tag-triggered GitHub Actions workflow is configured to build binaries and publish a release; platform builds have not yet been verified. |

Turso runs locally and stores host metadata only; credentials and tokens are
not stored there. A macOS Keychain backend and an authenticated encrypted file
store exist. Windows and Linux native credential stores are not yet implemented.
Turso Database is pre-1.0; its maintainers recommend independent backups.
See the [architecture](docs/ARCHITECTURE.md) and [threat model](docs/security/threat-model.md)
for design decisions and known gaps.

## Planned capabilities

The project aims to support secure SSH connections with explicit host-key
verification, an interactive terminal, organized hosts and identities, SFTP,
SSH tunnels, workspaces, snippets, and optional encrypted Google Drive sync.
Additional protocols, collaboration, infrastructure integrations, and mobile
clients are longer-term goals. None of these user-facing features is complete.

See [PROJECT.md](PROJECT.md) for the full requirements, [the roadmap](docs/ROADMAP.md)
for the next increments, and [the feature matrix](docs/FEATURE_MATRIX.md) for
current status. A feature is not considered complete until it works, is
integrated, and has appropriate security and test coverage.

## Repository structure

```text
crates/shellumi-app/       GPUI application window
crates/shellumi-core/      UI-independent connection model
crates/shellumi-terminal/  VT/ANSI terminal emulator & cross-platform PTY manager
crates/shellumi-storage/   Local Turso host metadata storage
crates/shellumi-cli/       Command-line host metadata management
docs/                      Architecture, roadmap, feature status, threat model
.github/workflows/         Tag-triggered release builds
```

The UI, core, terminal, and storage are separate workspace crates. The SSH
connection uses system OpenSSH; the terminal view is an initial implementation.
Credential abstractions include a macOS Keychain backend and an encrypted file store; Windows and Linux native credential stores are not implemented.
Google Drive synchronization is not yet integrated end-to-end.

## Build from source

Install a current stable [Rust toolchain](https://www.rust-lang.org/tools/install)
and the native dependencies required by [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).
The project uses a locked Cargo dependency graph and pins GPUI 0.2.2.

- **macOS:** Install Xcode, select its developer directory, and ensure
  `xcrun --find metal` succeeds. Command Line Tools alone may not provide the
  Metal compiler.
- **Linux:** Install a C/C++ build toolchain, `pkg-config`, and GPUI's font,
  Wayland, X11, and XKB development libraries. The release workflow lists the
  packages used on Ubuntu 24.04.
- **Windows:** Install the Rust MSVC toolchain and the Visual Studio C++ build
  tools required for native Rust builds.

From the repository root:

```sh
cargo run --locked -p shellumi-app
```

To display a host database, use:

```sh
cargo run --locked -p shellumi-app -- --hosts ./hosts.db
```

The database is created if it does not exist, and schema migrations are applied
when opening it. Hosts can be created and edited directly in the window, or
managed using the CLI tool below. A refresh shortcut is available (Command-R on
macOS, Control-R elsewhere); Command-R has been verified on macOS, while
Control-R has not yet been checked on Windows or Linux.
The current host list, forms, and refresh controls are not yet exposed in the
macOS accessibility tree observed during testing. The window provides a top tab bar
for managing multiple concurrent local shells, remote SSH connections, and SFTP file transfers
alongside the Hosts manager. The toolbar provides a "Local terminal" button to open a shell
session at any time. When a database is loaded, "Connect" starts an experimental
OpenSSH session with strict host-key verification, while "SFTP" opens a dedicated file transfer
session featuring both a full graphical dual-pane browser (Local left pane, Remote right pane,
clickable breadcrumbs, directory traversal, hidden-files toggling, selection-driven Upload/Download actions,
and a live transfer queue drawer with visual progress bar) and an interactive raw SFTP terminal mode.
The window provides a global Command Palette accessible via `Cmd-K` (`Ctrl-K` on Linux/Windows) or top toolbar buttons,
supporting instant fuzzy search across hosts (direct SSH/SFTP connection), saved snippets, navigation, and cloud sync actions.
The terminal view provides styled text spans (16/256/TrueColor RGB and text attributes), scrollback history,
clipboard paste, extended ANSI navigation keys, PTY resize support, an in-pane search toolbar, and a live cursor coordinate status bar.
The PTY worker has been tested with an isolated SSH server; full window interaction
on Linux and Windows has not yet been established.
macOS, Windows, and Linux are target platforms; successful builds
or runtime behavior on all of them have not yet been established.

## Manage hosts from the command line

The CLI takes a database path on every command (or standalone flags for key generation)
and creates the database if it does not exist. It stores host metadata only; do not put
passwords or keys in these fields. For example:

```sh
cargo run --locked -p shellumi-cli -- ./hosts.db add "Web server" example.org alice 22 --group Production --tags web,primary
cargo run --locked -p shellumi-cli -- ./hosts.db list
cargo run --locked -p shellumi-cli -- ./hosts.db list --group Production
cargo run --locked -p shellumi-cli -- ./hosts.db list --tag web
cargo run --locked -p shellumi-cli -- ./hosts.db list --search example.org
cargo run --locked -p shellumi-cli -- ./hosts.db update 1 "Web server" example.org alice 2222 --group Staging --tags web,backup
cargo run --locked -p shellumi-cli -- ./hosts.db delete 1
cargo run --locked -p shellumi-cli -- ./hosts.db connect 1
cargo run --locked -p shellumi-cli -- ./hosts.db quick-connect alice@example.org:2222
cargo run --locked -p shellumi-cli -- keygen --comment "alice@shellumi"
cargo run --locked -p shellumi-cli -- ./hosts.db import-ssh-config ~/.ssh/config
cargo run --locked -p shellumi-cli -- ./hosts.db export-ssh-config
cargo run --locked -p shellumi-cli -- ./hosts.db import-csv ./hosts.csv
cargo run --locked -p shellumi-cli -- ./hosts.db export-csv
cargo run --locked -p shellumi-cli -- ./hosts.db vault export ./vault.json --passphrase "my-passphrase"
cargo run --locked -p shellumi-cli -- ./hosts.db vault diff ./vault.json --passphrase "my-passphrase"
cargo run --locked -p shellumi-cli -- ./hosts.db vault import ./vault.json --passphrase "my-passphrase"
cargo run --locked -p shellumi-cli -- ./hosts.db vault status
cargo run --locked -p shellumi-cli -- ./hosts.db vault oauth-url
cargo run --locked -p shellumi-cli -- ./hosts.db sftp local-ls ./
cargo run --locked -p shellumi-cli -- ./hosts.db sftp packet encode-init --version 3
cargo run --locked -p shellumi-cli -- ./hosts.db sftp packet decode 000000050100000003
cargo run --locked -p shellumi-cli -- ./hosts.db sftp queue upload /local/file /remote/file 1024
cargo run --locked -p shellumi-cli -- ./hosts.db snippet batch 1 --hosts web-01,web-02 --var env=prod --dry-run
cargo run --locked -p shellumi-cli -- ./hosts.db palette search "web-01"
cargo run --locked -p shellumi-cli -- ./hosts.db mosh alice@example.org --ssh-port 2222
cargo run --locked -p shellumi-cli -- ./hosts.db telnet router.local 23
cargo run --locked -p shellumi-cli -- ./hosts.db serial /dev/tty.usbserial-10 --baud 115200
cargo run --locked -p shellumi-cli -- ./hosts.db serial-ports
cargo run --locked -p shellumi-cli -- ./hosts.db team vault create "DevOps Team" --desc "Shared infrastructure"
cargo run --locked -p shellumi-cli -- ./hosts.db team vault list
cargo run --locked -p shellumi-cli -- ./hosts.db team vault rotate-key vlt-1
cargo run --locked -p shellumi-cli -- ./hosts.db team member invite vlt-1 bob@example.org "Bob Developer" editor
cargo run --locked -p shellumi-cli -- ./hosts.db team member list vlt-1
cargo run --locked -p shellumi-cli -- ./hosts.db team member role vlt-1 mem-1 admin
cargo run --locked -p shellumi-cli -- ./hosts.db team member revoke vlt-1 mem-1
cargo run --locked -p shellumi-cli -- ./hosts.db team audit vlt-1
cargo run --locked -p shellumi-cli -- ./hosts.db team session start sess-1 host-user
cargo run --locked -p shellumi-cli -- ./hosts.db team session join sess-1 guest-user "Guest Teammate"
cargo run --locked -p shellumi-cli -- ./hosts.db team session grant sess-1 host-user guest-user
cargo run --locked -p shellumi-cli -- ./hosts.db team session revoke sess-1 host-user
cargo run --locked -p shellumi-cli -- ./hosts.db team session status sess-1
```

`add` prints the new numeric ID. `list` prints tab-separated ID, name, hostname,
username, port, group, and tags. Supports filtering with `--group <name>`, `--tag <tag>`,
or `--search <query>`. `add` and `update` accept `--protocol <ssh|mosh|telnet|serial>` and `--protocol-config <json>`.
`quick-connect` launches an immediate OpenSSH session without
saving a permanent record. `keygen` generates a standard Ed25519 keypair and outputs its
OpenSSH public key and SHA-256 fingerprint, wiping private key material on drop.
`import-ssh-config` and `export-ssh-config` migrate hosts to and from standard OpenSSH
configurations (`~/.ssh/config`). `import-csv` and `export-csv` support standard
comma-separated host tables. `vault export`, `vault import`, `vault diff`, `vault status`,
`vault oauth-url`, `vault history`, and `vault rollback` handle zero-knowledge encrypted envelope backups, diff comparisons,
RFC 7636 PKCE authorization parameters, and automated disaster recovery rollback. `sftp local-ls`,
`sftp history`, `sftp log`, `sftp packet`, and `sftp queue` provide local filesystem browsing, transfer history auditing,
RFC-compliant SFTP v3 wire packet inspection/encoding, and concurrent transfer queue orchestration.
`snippet batch --dry-run` previews commands; CLI batch execution is not implemented. Vault commands require an explicit passphrase; command-line arguments may be visible to other local processes.
`palette search` queries the global Command Palette across static actions, hosts, snippets, and navigation entries with tokenized and acronym scoring.
`mosh`, `telnet`, `serial`, and `serial-ports` handle dedicated protocol connections and local serial port discovery; `telnet` emits an explicit cleartext credential warning before launch.
`connect` executes the appropriate client for the host protocol (`ssh`, `mosh`, `telnet`, `screen`) with strict SSH flags and cleartext warnings for Telnet. Run with `--help` to display full
command syntax.

## Tests and development checks

```sh
cargo fmt --all -- --check
cargo test --locked -p shellumi-core -p shellumi-storage -p shellumi-cli
cargo clippy --locked -p shellumi-core -p shellumi-storage -p shellumi-cli --all-targets -- -D warnings
```

The GPUI app also needs the platform dependencies above. For a full workspace
check, use `cargo test --locked --workspace` and
`cargo clippy --locked --workspace --all-targets -- -D warnings` on a configured
machine. Tests of the individual core and storage crates do not validate the
desktop window or an SSH server connection. See the [isolated SSH test guide](docs/development/ssh-integration.md)
for the opt-in real-server test and its prerequisites.

## Releases

Pushing a Git tag triggers the [release workflow](.github/workflows/release.yml).
It targets Linux x86-64, Windows x86-64, and macOS Apple Silicon and Intel. If
all builds succeed, it publishes the standalone binaries and SHA-256 checksums
to a GitHub Release. These are unsigned development binaries, not installers;
the workflow and platform builds still need end-to-end validation.

## Security and contributions

Shellumi's security architecture is still being designed. Do not use the
current application to store secrets or rely on it for remote access. The
[threat model](docs/security/threat-model.md) tracks planned controls and gaps.
Contributions are welcome: discuss substantial changes first, keep protocol
and storage logic independent of GPUI, and include appropriate tests and
documentation. Report security issues privately to the repository maintainers.

## License

Shellumi is licensed under the [MIT License](LICENSE). Dependencies retain their
own licenses.

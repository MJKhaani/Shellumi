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
| Desktop interface | A GPUI host manager loads from a selected database, featuring in-window Add/Edit forms with host groups, tags, and jump hosts, real-time search filtering, jump host badges, a Quick Connect toolbar, an in-window Cloud Sync status pill (`☁ Sync: Local only` / `☁ Sync: Up to date` / `☁ Syncing…` / `⚠ Sync conflict`) with one-click manual synchronization trigger, and an interactive Sync Conflict Resolution modal presenting side-by-side local vs. remote comparisons with one-click Keep Local and Accept Remote resolution choices. Each host card provides quick actions for "Edit", "Connect" (SSH), "SFTP" (interactive Secure File Transfer session), and "Delete" (with automated SyncScheduler mutation debouncing). The top tab bar supports concurrent SSH, Local, and SFTP sessions. A collapsible Snippets drawer (`📜 Snippets`) in the terminal header allows one-click command pasting and execution into active terminal panes. Persisted asynchronously via background executor. Loading, adding, editing, mouse refresh, and Command-R have been checked on macOS; other platforms and screen-reader access need validation. |
| Connection & SFTP model | The core library validates SSH destination fields, parses quick-connect targets (`user@host[:port]`), parses/generates OpenSSH configs and RFC 4180 CSVs, parses OpenSSH `known_hosts` with TOFU and changed-key detection, generates Ed25519 SSH keypairs with memory zeroization on drop, validates jump host chains, models SSH port forwarding (`PortForwardRule`: Local, Remote, Dynamic SOCKS5) with exposure warnings, provides a decoupled credential manager (`CredentialStore`, `SecretString` with redaction, native OS keychain backends `MacOsKeychainStore`, `WindowsCredentialManagerStore`, and `LinuxSecretServiceStore`, portable `EncryptedFileCredentialStore` with AES-256-GCM AEAD and PBKDF2-HMAC-SHA256, atomic file commits, in-memory zeroization, and `default_credential_store()` routing), implements persistent OAuth token storage (`OAuthTokenBundle::save_to_store`, `load_from_store`, `delete_from_store`), implements SFTP file management domain models (`FileEntry`, `FileKind`, UNIX permission formatting, human-readable byte formatting, breadcrumb path generation, directory joining/normalization, `TransferTask`, and local filesystem listing `list_local_directory`), provides a Snippet & Automation engine (`Snippet`, `SnippetVariable`, variable extraction `{{var}}` / `{{var:-default}}`, POSIX shell escaping `shell_escape`, parameter rendering, secret variable detection, preview redaction `render_preview`, and JSON library export/import), and provides an end-to-end zero-knowledge synchronization and encryption engine (`derive_key` PBKDF2-HMAC-SHA256, AES-256-GCM AEAD envelope sealing `seal_payload` and opening `open_payload`, `EncryptedEnvelope` with bound associated data preventing transposition attacks, `SyncRecord` versioning with tombstones, `determine_sync_action` three-way conflict detection, `resolve_record_conflict` resolution strategies, `build_conflict_item_views`, `format_record_summary`, RFC 7636 PKCE challenge generation `generate_pkce_challenge`, `OAuthCallbackListener` loopback server, OAuth callback response generation, token expiration checks `OAuthTokenBundle`, record comparison `diff_sync_records_with_passphrase`, `SyncVault` encrypted snapshot model, Google Drive AppData REST API client `GoogleDriveSyncClient`, transport-agnostic HTTP protocol abstractions `HttpTransport`/`HttpRequest`/`HttpResponse`, Google Drive file metadata `DriveFileMetadata`, conflict-safe sync planner `plan_sync_cycle`, and `SyncScheduler` tracking debouncing windows and exponential backoffs). |
| Terminal & PTY | The GPUI window supports running multiple concurrent SSH sessions, local shell sessions, and SFTP sessions in a tabbed workspace with vertical and horizontal split panes (`split_pane(SplitDirection::Vertical/Horizontal)`), independent PTY sessions per pane, in-pane search toolbar with match navigation (`↑`/`↓`) and highlighted spans, resizing commands, clipboard paste (`Cmd-V` / `Ctrl-Shift-V`), extended ANSI navigation keys, styled text span rendering (16/256/TrueColor RGB and text attributes), scrollback buffer, a live cursor coordinate/dimension status bar, and a quick-action Snippets drawer for inserting and running parameterized scripts. |
| Local storage | An async Turso Database crate supports transactional schema migrations (schema version 7 with auto-migrations from versions 1, 2, 3, 4, 5, and 6), persistent host groups (`groups` table), jump hosts (`hosts.jump_host`), persistent SSH tunnels (`tunnels` table), persistent file transfer audit history (`transfer_history` table), saved snippets and templates (`snippets` table with tags, parameters, and variable schema), disaster recovery snapshots with history pruning (`sync_snapshots` table, `archive_snapshot`, `list_snapshot_history`, `get_snapshot_vault`, `rollback_snapshot`), configuration inheritance (`get_effective_destination`, `get_effective_tags`), tags, group/tag filtering, search, CRUD operations, and zero-knowledge encrypted vault backup export and restore (`export_vault`, `import_vault`). Credentials are never written to this database. |
| Host & SFTP CLI | A command-line tool manages host metadata, filters by `--group`, `--tag`, `--jump-host`, and `--search`, provides dedicated `group add/list/show/delete`, `tunnel add/list/delete`, `snippet add/list/show/render/delete`, and `vault export/import/diff/status/oauth-url/history/rollback` commands, SFTP subcommands (`sftp local-ls`, `sftp history`, `sftp clear-history`, `sftp log`), `quick-connect <user@host[:port]>`, generates Ed25519 keys via `keygen`, handles OpenSSH config and CSV import/export, and dispatches connections via system OpenSSH and SFTP with group configuration inheritance and `-J` jump host chaining. |
| Tests | 120 unit and integration tests across 5 crates cover destination validation, quick-connect parsing, storage migrations (v1-v6 to v7), disaster recovery snapshot history and rollback, host groups/tags/search, group inheritance, jump hosts, port forwarding rules, tunnel CRUD, SFTP local directory listing, transfer task progress, transfer history lifecycle, snippet variable extraction and safe rendering, snippet preview redaction, snippet storage CRUD, zero-knowledge AES-256-GCM envelope sealing/opening, PBKDF2 key derivation and passphrase enforcement, sync conflict resolution choices (`resolve_record_conflict`, `build_conflict_item_views`, `format_record_summary`), PKCE challenge generation, loopback OAuth callback listener, token bundle expiration, record diffing with payload decryption, Google Drive appdata file listing/download/multipart upload/update, sync cycle planning, mock HTTP transport roundtrips, sync status UI formatting, interactive conflict resolution modal state, sync scheduler mutation debouncing, encrypted vault export/import and disaster recovery in storage and CLI (`vault export`, `vault import`, `vault diff`, `vault status`, `vault oauth-url`, `vault history`, `vault rollback`), key generation, secret zeroization, multi-platform credential stores (macOS Keychain, Windows Credential Manager, Linux Secret Service, encrypted file store), OAuth token bundle persistence, known-hosts verification, terminal VT/ANSI engine, search highlight matching, split pane state, PTY input/output, and key mapping. An opt-in isolated OpenSSH test covers host-key decisions and public-key authentication through the PTY worker. |
| Releases | A tag-triggered GitHub Actions workflow is configured to build binaries and publish a release; platform builds have not yet been verified. |

Turso runs locally and stores host metadata only; credentials and tokens are
never stored in Turso. Secrets are handled by OS native keychains (macOS Keychain,
Windows Credential Manager, Linux Secret Service) or authenticated encrypted file
stores. Turso Database is pre-1.0; its maintainers recommend independent backups.
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
Credential management supports native OS keychains and encrypted file stores,
and Google Drive sync is integrated end-to-end with zero-knowledge encryption.

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
for managing multiple concurrent local shells and remote SSH connections alongside
the Hosts manager. The toolbar provides a "Local terminal" button to open a shell
session at any time. When a database is loaded, "Connect" starts an experimental
OpenSSH session with strict host-key verification. The terminal view provides styled
text spans (16/256/TrueColor RGB and text attributes), scrollback history, clipboard paste,
extended ANSI navigation keys, PTY resize support, and a live cursor coordinate status bar.
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
```

`add` prints the new numeric ID. `list` prints tab-separated ID, name, hostname,
username, port, group, and tags. Supports filtering with `--group <name>`, `--tag <tag>`,
or `--search <query>`. `quick-connect` launches an immediate OpenSSH session without
saving a permanent record. `keygen` generates a standard Ed25519 keypair and outputs its
OpenSSH public key and SHA-256 fingerprint, wiping private key material on drop.
`import-ssh-config` and `export-ssh-config` migrate hosts to and from standard OpenSSH
configurations (`~/.ssh/config`). `import-csv` and `export-csv` support standard
comma-separated host tables. `vault export`, `vault import`, `vault diff`, `vault status`,
`vault oauth-url`, `vault history`, and `vault rollback` handle zero-knowledge encrypted envelope backups, diff comparisons,
RFC 7636 PKCE authorization parameters, and automated disaster recovery rollback. `connect` executes system OpenSSH with
`-o StrictHostKeyChecking=ask` and `-o ForwardAgent=no`. Run with `--help` to display full
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

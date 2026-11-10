# Desktop GUI implementation — v0.0.6-alpha

The desktop is Rust/GPUI. The supplied UI design pack is a visual reference;
its sample data and prototype QA are not evidence of working application features.

## Connected workflows

| Screen | GUI behavior |
| --- | --- |
| Overview | Live local host, session, workspace and snippet counts; connect to saved hosts; create host/snippet; open local shell and workspaces. |
| Hosts | Search and group filtering, selection and inspector, host create/edit/delete, group default editing, clipboard OpenSSH config import/export, SSH and SFTP dispatch. Auto-discovers detected OpenSSH keys from `~/.ssh/` and presents them directly in the Host form identity selector. Full keyboard Tab/Shift+Tab focus navigation across all inputs. View mode switcher (▦ Grid vs ≡ List), 🏷 Tag filter popover, 📅 Sort dropdown (A-z, Z-a, Newest, Oldest), 👥 Team Invite banner, slide-over Group Details Inspector, and group card context menu. |
| Workspaces | Create, edit/rename, duplicate, delete with confirmation, choose local and saved SSH sessions, select single/vertical/horizontal/grid layout, reopen saved sessions. Opening creates new sessions; it does not restore remote processes. |
| Terminal | Real PTY sessions, measured grid dimensions and PTY resize, independent split panes, wheel scrollback with a visible position indicator, follow output when at the bottom, search, paste, recordings, snippets and broadcast controls. |
| Remote Desktop | Embedded graphical VNC (RFB v3.8) and RDP (MS-RDPBCGR/TPKT) session client with live metrics (latency, FPS, resolution, bpp), SAS key shortcuts (Ctrl+Alt+Del, Win, Alt+Tab), scaling switcher (Fit/100%/Fill), view-only mode, and clipboard sync. Dispatches seamlessly alongside terminal and SFTP tabs. |
| SFTP Files | Real local directory browsing, parent/path navigation, hidden files, copy local file paths, graphical dual-pane SFTP browser with live remote directory listings, multi-file checkbox selection with Select All toggle, drag-and-drop file transfers between panes, upload/download queue with two-worker scheduling, live byte progress and Cancel actions, private staged publication, and live system SFTP terminal toggle. |
| Snippets | Package/tag filtering, live search, selected-script preview, create/edit multi-line commands with Tab navigation between fields, template variables, delete confirmation, clipboard copy and JSON package import/export, explicit parameter/run confirmation, paste into an existing terminal. |
| Keychain | Native credential-store status card (macOS `securityd`, Linux Secret Service via `secret-tool`, Windows Credential Manager `PasswordVault`) with active/fallback indicators; reusable database identities with OpenSSH certificate validation, secret reference badges (`🔐 Keychain`), and credential metadata; read public `.ssh/*.pub` metadata and fingerprints; auto-discovers and imports system keys into saved identities with `🔑` indicators; copy public keys; start interactive system `ssh-keygen`; load a selected identity with `ssh-add`; in-place Identity settings modification modal (Name, Username, Key Path, Passphrase, Tags). |
| Port Forwarding | Persisted tunnel rules, host selection, local/remote/dynamic forwarding, rule editing and deletion, real OpenSSH tunnel session launch. Network readiness is determined by the SSH session rather than fabricated traffic counters. |
| Known Hosts | Read actual OpenSSH trust entries; inspect public metadata and fingerprints; copy public keys; explicitly revoke an unmarked key. Revocation checks the original record before rewriting and refuses symlinks. |
| Session Logs | Existing recording library, selected recording playback, transport controls, export and deletion through the recording manager. |
| Vaults | Existing local vault and membership-record manager; real collaborator public keys are required. These are local records, not network invitations. |
| Collaboration | Explicit unavailable state for network session sharing. Local membership administration is available in Vaults. |
| Integrations | Existing provider-record, discovered-inventory and Ansible tools embedded in the navigation page. Provider records do not establish authenticated cloud API connections. |
| Google Drive Sync | Initial Google Drive vault synchronization via SystemHttpTransport (system curl), AES-256-GCM encrypted diffing, interactive conflict resolution modal, and working encrypted local backup export and restore dialogs. Export refuses existing output files. Restore uses a transaction and rolls back decryption/import errors. |
| Settings | Category rail; persistent interface and terminal font sizes, scrollback, default startup screen, local hidden-file preference; storage location, SSH security behavior, shell integration tools, shortcut reference and diagnostics. |

Preferences are stored beside the selected database as `*.preferences.json`.
They contain display/navigation options only. Backup passphrases are masked in
the GUI, wrapped in the core's redacted secret type for background work, and
never written to preferences. Encrypted backup contents use the existing vault
cryptography and are not uploaded anywhere.

## Verification

The macOS GUI was exercised with an isolated database: snippet creation and
preview, workspace creation and shell launch, changing a terminal preference,
long output (`seq 1 200`), scrolling back inside the pane, and split-pane sizing.
Automated coverage includes persistent snippet/workspace/group editing, host
import, preference validation/reopen, public-key fingerprint validation,
known-host revocation with stale-record rejection, encrypted backup round-trip
and wrong-passphrase rollback, viewport sizing, and primary-button construction.

## Remaining backend and platform work

Authenticated cloud inventory discovery (AWS/GCP/DigitalOcean API synchronization)
and network collaboration (live multi-user terminal session transport) are not connected.
The GUI must not show simulated cloud inventory or fake successful remote terminal sharing.

Isolated macOS identity/SFTP transport acceptance passed with an encrypted key.
GUI interaction, real password/MFA, Drive-account acceptance and native credential
access tests on Windows/Linux remain. Drive account setup is not integrated.

Full theme/language switching, launch-at-login integration, all prototype
subdialogs, screen-reader accessibility and non-macOS visual verification are
not complete. The design reference contains capabilities beyond the currently
implemented application; those controls are not presented as functioning toggles.

## Files in this GUI increment

- `crates/shellumi-app/src/theme.rs`: macOS HIG Dark theme tokens with navy surface palette (`WINDOW_BG: 0x191a24`, `CONTENT_BG: 0x1e202d`, `SIDEBAR_BG: 0x1b1c28`, `SURFACE: 0x242636`, `ACCENT_BLUE: 0x1e80ff`).
- `crates/shellumi-app/src/desktop.rs`: in-place page navigation without creating tabs, refined Termius card grids for Keychain and Known Hosts, daemon status badge, clean section layouts without redundant header banners, modal bleed elimination across page switches.
- `crates/shellumi-app/src/main.rs`: unified macOS titlebar with traffic light offset, native macOS menu bar (`Shellumi`, `Edit`, `Window`, `Help`), `Cmd+Q` application quit action, root-level vault selector dropdown overlay, redesigned Termius-matched dual-pane SFTP file manager (Local/Remote independent selection on both sides, table layout, breadcrumbs, action menus), enlarged quick connect container (48px bar / 36px input) preventing overflow, collapsible sidebar rail with Retina icon pack assets (`resources/icon_pack`), interactive New Host modal with scrollable body and sticky footer.
- `packaging/macos/Info.plist` & `packaging/macos/build-dmg.sh`: updated bundle configuration, icon pack copying into bundle resources, and disk image generation script for `dist/Shellumi.app` and `dist/Shellumi-0.0.4-alpha-macos-x86_64.dmg`.
  confirmation dialogs, preferences, local files and backup/public-key UI harmonized with macOS dark tokens.
- `crates/shellumi-app/src/main.rs`: page routing, host/group/import controls,
  embedded managers, real tunnel sessions, measured terminal sizing/scrolling,
  workspace restoration, modals, and native macOS dark styling.
- `crates/shellumi-app/src/text_input.rs`: masked passphrase and search input styling with macOS dark tokens.
- `crates/shellumi-app/src/connection.rs` and `askpass.rs`: identity-aware launch
  coordination and separate credential helper; `tests/askpass.rs` tests helper processes.
- `crates/shellumi-core/src/connection.rs`: shared launch policy, strict prompt
  matching, validated arguments and real OpenSSH configuration-parser coverage.
- `crates/shellumi-storage/src/desktop.rs` and `src/lib.rs`: background desktop
  operations, preferences, atomic imports/restores, encrypted backups and tests.
- `crates/shellumi-core/src/identity.rs` and `src/lib.rs`: public-key fingerprint
  validation and tests.
- `README.md`, `docs/ROADMAP.md`, `docs/GUI_STATUS.md`: implementation status.

Verification commands: `cargo test --locked --offline --workspace` (321 passed,
3 opt-in tests ignored on macOS), app unit tests (47 passed, 1 ignored),
`cargo clippy --locked --offline --workspace --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, and the native macOS release build. Workspace
integration tests requiring a loopback listener were run with loopback access.


## README screenshot

`resources/screenshots/shellumi-macos.png` is a capture of the running macOS desktop
application. It reflects the modern unified interface with groups, hosts,
vault switcher, and action toolbars.


## Credential and synchronization configuration

Native credential storage is used by default. An encrypted fallback is enabled
only with an explicit `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` of at least 12
characters. Its default file is under the platform's persistent application data
directory; `SHELLUMI_VAULT_PATH` can select another absolute location. No shared
fallback key or silent ephemeral fallback is used for default credential storage.
Drive sync requires an OAuth token and an explicit `SHELLUMI_VAULT_PASSPHRASE`;
missing configuration produces an error. Authorization headers and UTF-8 request
bodies travel through curl stdin; binary request bodies are not supported by
this transport. Graphical batch SFTP requires a previously trusted host key.

## Identity connection increment

Host identity selection overrides the group identity; selected identity username
overrides host username. Clearing a host binding restores group inheritance or
system OpenSSH defaults. Key/certificate files, authentication preferences and
jump hosts are propagated consistently. Missing hosts/identities and malformed
metadata fail before a session is started; workspace commands are all resolved
before any panes launch. Legacy workspace destinations without host IDs retain
OpenSSH defaults. Quick Connect does not select an identity.

The app executable doubles as a minimal OpenSSH askpass helper. Only references
are passed in its environment. It resolves secrets in the helper process and
writes them solely to OpenSSH's helper pipe. Password and encrypted-key prompts
must match the planned user/host or absolute key path. Trust confirmations and
MFA prompts are rejected, and secret-assisted connections force strict host-key
checking. Use an interactive connection to verify and trust a new server first.
Graphical SFTP without a saved credential uses noninteractive OpenSSH/agent access.

New automated checks exercise the actual helper subprocess, refusal of trust/MFA
and wrong-target prompts, database reopen and host/group precedence, missing
bindings, IPv6 SFTP formatting, and OpenSSH certificate paths with spaces/quotes.
These checks do not replace live native-keychain authentication or visual testing
of the new selectors. Existing README screenshots predate this increment.

On the local macOS test fixture, real OpenSSH authenticated with an encrypted
private key through the app helper (including a path with spaces), and refused
an unknown host with the same secret-assisted launch settings. The test used
isolated keys and known-hosts files; the loopback server was stopped afterward.
The automated CLI credential CRUD test explicitly configures a temporary,
user-keyed encrypted fallback, rather than relying on a shared fallback key or
claiming native keychain acceptance.

## SFTP completion and real-server acceptance increment

The opt-in `crates/shellumi-app/tests/ssh_sftp_acceptance.rs` now creates and
cleans up its own temporary macOS OpenSSH server, keys, Turso identity/group
binding, known-hosts file and transfer files. The real app askpass helper unlocks
an encrypted key. Listing, multi-packet binary and empty-file round-trips,
reconnect, missing-directory errors, changed/unknown host-key refusal, and
wrong-passphrase refusal passed. This exercises the same core transport used by
the graphical SFTP page; it does not test GPUI clicks or layout.

`crates/shellumi-core/src/sftp.rs` now checks the server's final CLOSE status and
request ID for uploads, downloads and directory listings. Failure, disconnect or
a mismatched response propagates as an error instead of a successful task.
Three automated regression tests cover those cases. The test guide and roadmap
record remaining password/MFA, native credential and transfer safety work.

## Staged transfer and GUI cancellation increment

`crates/shellumi-app/src/sftp_transfer.rs` owns queued/running job snapshots,
background transfer launch, bounded progress delivery and cancellation outcomes.
Queued cancellation starts no connection. Running cancellation is requested in
the UI and acknowledged only by the worker; a transfer already publishing cannot
be relabelled cancelled. Closing a tab cancels its pending/running work. Cleanup
failures remain visible as failed tasks with the temporary remote path.

The dispatcher honors the actual queue limit of two workers per tab and starts
waiting work after a worker finishes. Identical queued/running destinations are
rejected. Transfer callbacks use stable tab IDs; remote listing generations also
reject obsolete navigation responses. Local metadata refresh after a download
runs in the background. Progress no longer implies completion, and the progress
bar/path rows fit the available width.

Core transfers now use private staging and publish by rename after verification.
Tests cover destination preservation on failed CLOSE, premature EOF and cancel,
no delete-target fallback, race exclusion between cancellation and publication,
symlink safety and retained cleanup warnings. The real-server test checks both
directions of cancellation and staging cleanup. These transport/helper tests do
not establish visual, keyboard or screen-reader acceptance of the new Cancel UI.

Cancellation now interrupts live SSH pipe waits, including connection setup;
packet reads/writes have bounded deadlines. Filesystem IO and final publication
are separate: a commit already in progress cannot be relabelled cancelled.
Original permissions/timestamps are not preserved (new files use private modes).
Abrupt shutdown/network loss can leave staging files; these cases still need
abandoned-staging recovery work.

## v0.0.2-alpha bounded connection cancellation

The transfer worker calls `SftpClient::connect_with_control`, propagating its
one-shot cancellation token through SFTP initialization and packet IO. GUI Cancel
and tab close can interrupt a nonresponsive SSH connection rather than waiting
indefinitely for a pipe read. The UI still acknowledges the worker outcome;
cleanup warnings remain visible if interruption leaves remote staging behind.

Core pipe IO uses bounded channels and dedicated blocking-handle workers, with
cancellation checks while waiting and one deadline per packet. Read/write timeout
or interruption terminates the SSH child, avoiding reuse of a partial frame.
Normal transfer/cancellation and stalled-connect tests passed against actual
OpenSSH. No new visual/keyboard acceptance is inferred from these tests.

## Release packaging follow-up

The `v0.0.2-alpha` source compiled on Linux, Windows and both macOS targets. The
initial Debian package step failed before publication; main now repairs the
prerelease conversion and supports retries without changing the source tag.
Three packaging-helper assembly tests bring main to 321 workspace tests. No GUI
implementation changed in this repair. Release publication status and provenance
are tracked in [the release guide](development/releases.md).

The repaired [release run](https://github.com/MJKhaani/Shellumi/actions/runs/42269432654) passed every native build/package job and
published all 15 assets. Checksums and source/helper provenance were verified.
This establishes distribution availability, not additional GUI/runtime acceptance.

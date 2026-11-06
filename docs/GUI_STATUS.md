# Desktop GUI implementation — v0.0.1-alpha

The desktop is Rust/GPUI. The supplied UI design pack is a visual reference;
its sample data and prototype QA are not evidence of working application features.

## Connected workflows

| Screen | GUI behavior |
| --- | --- |
| Overview | Live local host, session, workspace and snippet counts; connect to saved hosts; create host/snippet; open local shell and workspaces. |
| Hosts | Search and group filtering, selection and inspector, host create/edit/delete, group default editing, clipboard OpenSSH config import/export, SSH and SFTP dispatch. Import ignores wildcard config blocks and imports destination metadata only. |
| Workspaces | Create, edit/rename, duplicate, delete with confirmation, choose local and saved SSH sessions, select single/vertical/horizontal/grid layout, reopen saved sessions. Opening creates new sessions; it does not restore remote processes. |
| Terminal | Real PTY sessions, measured grid dimensions and PTY resize, independent split panes, wheel scrollback with a visible position indicator, follow output when at the bottom, search, paste, recordings, snippets and broadcast controls. |
| SFTP Files | Real local directory browsing, parent/path navigation, hidden files, copy local file paths, choose a saved SSH host and open its live system SFTP terminal. |
| Snippets | Package/tag filtering, live search, selected-script preview, create/edit multi-line commands, template variables, delete confirmation, clipboard copy and JSON package import/export, explicit parameter/run confirmation, paste into an existing terminal. |
| Keychain | Read public `.ssh/*.pub` metadata and fingerprints; copy public keys; start interactive system `ssh-keygen`; load a selected identity with `ssh-add`. Private key files are not read by the GUI. |
| Port Forwarding | Persisted tunnel rules, host selection, local/remote/dynamic forwarding, rule editing and deletion, real OpenSSH tunnel session launch. Network readiness is determined by the SSH session rather than fabricated traffic counters. |
| Known Hosts | Read actual OpenSSH trust entries; inspect public metadata and fingerprints; copy public keys; explicitly revoke an unmarked key. Revocation checks the original record before rewriting and refuses symlinks. |
| Session Logs | Existing recording library, selected recording playback, transport controls, export and deletion through the recording manager. |
| Vaults | Existing local vault and membership-record manager; real collaborator public keys are required. These are local records, not network invitations. |
| Collaboration | Explicit unavailable state for network session sharing. Local membership administration is available in Vaults. |
| Integrations | Existing provider-record, discovered-inventory and Ansible tools embedded in the navigation page. Provider records do not establish authenticated cloud API connections. |
| Google Drive Sync | Explicit unavailable state for Drive transport; working encrypted local backup export and restore dialogs. Export refuses existing output files. Restore uses a transaction and rolls back decryption/import errors. |
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

Graphical remote SFTP listing/transfer, authenticated cloud inventory discovery,
network collaboration and Google Drive transport are not connected. The GUI
must not show sample remote files, simulated tunnel traffic or fake successful
transfers. The system SFTP terminal is available for actual file operations.

Full theme/language switching, launch-at-login integration, all prototype
subdialogs, screen-reader accessibility and non-macOS visual verification are
not complete. The design reference contains capabilities beyond the currently
implemented application; those controls are not presented as functioning toggles.

## Files in this GUI increment

- `crates/shellumi-app/src/desktop.rs`: navigation pages, library editors,
  confirmation dialogs, preferences, local files and backup/public-key UI.
- `crates/shellumi-app/src/main.rs`: page routing, host/group/import controls,
  embedded managers, real tunnel sessions, measured terminal sizing/scrolling,
  workspace restoration, and removal of simulated SFTP output.
- `crates/shellumi-app/src/text_input.rs`: masked passphrase entry.
- `crates/shellumi-storage/src/desktop.rs` and `src/lib.rs`: background desktop
  operations, preferences, atomic imports/restores, encrypted backups and tests.
- `crates/shellumi-core/src/identity.rs` and `src/lib.rs`: public-key fingerprint
  validation and tests.
- `README.md`, `docs/ROADMAP.md`, `docs/GUI_STATUS.md`: implementation status.

Verification commands: `cargo test --locked --offline --workspace` (270 passed,
1 opt-in test ignored), final app tests (44 passed, 1 ignored),
`cargo clippy --locked --offline --workspace --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, and the native macOS release build. Workspace
integration tests requiring a loopback listener were run with loopback access.


## README screenshots

`resources/screenshots/snippets-macos.jpg` and `settings-macos.jpg` are captures
of the running macOS development app with isolated test data. They are not design
prototype renders, do not establish cross-platform behavior, and may differ in
minor details from subsequent builds.

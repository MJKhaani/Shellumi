# Shellumi roadmap

The current milestone is **v0.0.7-alpha**. Requirements remain in
[PROJECT.md](../PROJECT.md); the [feature matrix](FEATURE_MATRIX.md) and
[GUI status](GUI_STATUS.md) describe implementation evidence and limitations.

## Operating modes and integration progress

Each vault will be either local, with optional direct Google Drive personal sync, or remote, using a self-hosted [Shellumi Server](https://github.com/MJKhaani/Shellumi-Server) for sync, vault sharing and collaboration. Local use and Drive do not depend on the server. The app currently has initial Drive integration and local collaboration models; a server client and end-to-end interoperability have not been implemented or verified.

Users must be able to create local and remote vaults, connect remote vaults to multiple independent servers, and open concurrent windows bound to different vaults within one app instance. Vault data, credentials, server sessions and sync state must remain isolated. Core has validated vault and server connection identifiers and a vault context distinguishing local from server-bound remote vaults. Schema v15 adds a persistent context registry and a default local registry entry while preserving existing data. Schema v16 assigns existing hosts to the default local vault and introduces a restricted vault-scoped host store. Schema v17 assigns identity records to vaults and a restricted local-vault API checks new keychain references against the vault. Identity names are still globally unique, and app credential resolution and existing secrets are not yet migrated to the wrapper. Other records remain unscoped; this does not establish complete vault isolation or implement server or window integration.

The default local vault is the user's normal vault and the synchronized directory of other vaults. The directory contains discovery and connection metadata, never the contents of those other vaults or device-bound secrets. Other local vaults need their own content synchronization; remote vaults have separate encrypted offline caches and reconcile on reconnect. This design is not yet implemented: the current database is not encrypted, the registry is not in Drive sync, and remote caches are unavailable.

Security foundation: core now provides random vault keys and passphrase-wrapped, vault-bound recovery envelopes. This primitive is not wired to app storage, the OS keychain, or sync. Next, generate and provision a random key for each vault, protect it with the OS credential store for local auto-unlock, and provide a user-chosen recovery passphrase wrapping that key for cross-device recovery. Use a versioned KDF and authenticated encryption; synchronize ciphertext and wrapped keys only. Address unavailable keychains, recovery-key loss, offline revocation, crash-safe migration and explicit opt-in before release.

The server compatibility matrix is a protocol target rather than proof that this app supports it. Complete the following increments before presenting online mode as available:

1. Define vault and server-connection identity and isolation; migrate existing data into a local vault without uploading it. Scope storage, credentials, queries, sessions, backup/restore, and background work to a vault, and test overlapping IDs and explicit cross-vault actions.
2. Implement independent vault-bound windows in one instance, visible vault/server context, window lifecycle, and isolation tests with simultaneous company vaults.
3. Define and test the v1 wire and cryptographic contract, including envelope format, associated data, supported suites and migration from existing local/Drive data. Preserve a recoverable local backup and keep providers independent.
4. Add opt-in server configuration, capability negotiation, authentication and refresh, secure credential storage, and device key registration and revocation.
5. Implement personal vault sync with stable identifiers, signed conditional commits, idempotency, paginated changes, persisted cursors, tombstones, offline changes, conflict handling and recovery.
6. Add team vault membership, invitations, role enforcement, device-wrapped keys and key rotation, with explicit authorization and revocation.
7. Integrate encrypted terminal collaboration with enrollment, streaming or polling, writer fencing, immediate revocation and termination.
8. Verify against at least two real self-hosted servers and two clients, including concurrent windows and credential isolation: enrollment, convergence, concurrent edits, deletion, reconnect, migration and recovery, sharing/revocation, and collaboration. Validate security and cross-platform behavior before calling the mode complete.

## Alpha milestone

The following work is included in the alpha source:

- Uniform pinned 56px action bar and scrolling content architecture across all application views (`Hosts`, `Keychain`, `Port Forwarding`, `Snippets`, `Known Hosts`, `Session Logs`, `Settings`, and `Workspaces`), eliminating visual jitter and vertical margin discrepancies.
- Fully functional 4-scenario SFTP file manager (Local-Local, Local-Remote, Remote-Local, Remote-Remote) with recursive directory transfers, platform folder MIME icons, dedicated selection status bar with instant "Deselect" action, empty space click-to-deselect, and hover-isolated context menus.
- Upgraded Command Palette (`Cmd+K`) and view mode menus with high-resolution vector SVG iconography.
- Native Rust/GPUI desktop with native macOS Dark visual direction and design tokens,
  navigation, host inventory/inspector, centered editors and local-first startup.
- Group Details Inspector slide-over drawer (Group icon, name, parent group, cloud sync toggle, default username/port/identity, and save/remove actions).
- Group card context menu (`⚡ Quick Connect`, `⚡ Connect >`, `✏️ Edit Group Details`, `👥 Collaborate`, `📄 Move to >`, `📄 Copy to >`, `🗑️ Remove`).
- In-place Reusable Identity Settings Modal to edit/create identity name, username, key path, passphrase, and tags.
- Advanced Host view controls: Grid (▦) vs List (≡) mode switcher, 🏷 Tag filter popover, and 📅 Sort mode dropdown (A-z, Z-a, Newest to oldest, Oldest to newest).
- Team collaboration invite modal dialog with multiple email recipients, and top host invite banner.
- Host destination username strictly takes precedence over identity username, allowing custom users like `root` with user keys.
- Automatic platform data paths, Turso database creation and migration,
  asynchronous storage, and persistent preferences.
- Host/group editing, filters, jump hosts, OpenSSH config import/export and
  explicit SSH/SFTP session dispatch.
- Local/remote PTYs, terminal tabs and split/grid layouts, viewport-based sizing,
  wheel scrollback, search, paste, ANSI styling and opt-in recordings.
- Functional snippet and workspace editors with storage persistence, template
  variables, import/export and confirmation for destructive library actions.
- Settings that apply and persist, public SSH-key/trust inspection, agent/key
  tools, saved forwarding rules and actual OpenSSH tunnel sessions.
- Reusable identity management with OpenSSH certificate parsing/validation,
  database persistence (schema v14), host/group GUI identity selection and inheritance,
  SSH/SFTP/workspace/tunnel launch policy, reference-only askpass, and Keychain display.
- Redesigned Termius-matched 4-scenario SFTP file manager (Local-Local, Local-Remote, Remote-Local, Remote-Remote) with interactive breadcrumb path navigation (< and >), detailed columns (Name, permissions, date modified, size, kind), file/folder multi-selection with batch transfers, full context menu (Copy to target directory, Rename, Delete, Refresh, New Folder, Edit Permissions), and complete icon pack integration.
- Protocol coverage expansion with domain models and initial negotiation helpers: VNC (RFB v3.8 protocol codec and packet models) and RDP (TPKT/X.224 negotiation request and MS-RDPBCGR connection codecs).
- Graphical Remote Desktop client session tab UI layout with toolbar controls, scaling switcher (Fit/100%/Fill), view-only toggle, SAS shortcuts (Ctrl+Alt+Del, Win, Alt+Tab), and clipboard synchronization UI. Complete native pure-Rust RFB VNC client engine (`shellumi-vnc`) with DES challenge-response VncAuth, RFB 3.3/3.7/3.8 negotiation, Raw/CopyRect/DesktopSize decoders, Hextile tile decoding, persistent ZRLE with decompression bomb guards, pure-Rust Tight JPEG/zlib decoding, and bidirectional clipboard synchronization with loop suppression (see [Remote Desktop Implementation Plan](REMOTE_DESKTOP_IMPLEMENTATION.md)). Embedded IronRDP client integration is next up in Phase 4.
- Live graphical remote SFTP transport engine over SSH subsystem (`ssh -s sftp`)
  with RFC SFTP v3 wire codec, dual-pane remote browsing, file upload/download, and transfer queue.
- Initial Google Drive HTTP transport (`SystemHttpTransport`) for zero-knowledge
  cloud vault sync, encrypted diffing, conflict resolution modal, and desktop sync panel.
- Encrypted local backup dialogs with overwrite refusal, transactional restore
  and wrong-passphrase rollback.
- Local inventory, vault membership and recording managers, with unfinished
  network transports identified rather than represented as successful actions.
- macOS development screenshots in the README and a tag-triggered prerelease
  workflow for Linux x86_64, Windows x86_64, macOS arm64 and macOS x86_64.

## Verification recorded for alpha

- 323 automated workspace tests passed on macOS; three real-server tests are
  opt-in. The self-contained SSH/SFTP acceptance test passed separately.
- Local OpenSSH encrypted-key authentication through the app helper succeeded;
  the same setup refused an untrusted host. Self-contained real-server SFTP
  listing, binary/empty transfers, reconnect, wrong-passphrase and changed/unknown
  host-key rejection also passed. Password/MFA/native-store and GUI interaction
  acceptance remain pending.
- Formatting and workspace Clippy with `-D warnings` passed with 0 warnings.
- Optimized Intel macOS app and CLI builds passed locally for `0.0.2-alpha`.
- macOS GUI checks covered saving snippets/workspaces, launching a workspace
  shell, changing a terminal preference, long output, scrolling and split sizing.
- Linux, Windows and Apple Silicon runtime behavior is not established by these
  local checks. Tag-triggered CI build results must be reviewed separately.

## Next engineering increments

1. **SSH/terminal acceptance:** test password, encrypted-key and MFA prompts,
   changed/revoked keys, disconnect/reconnect failures, resize under full-screen
   terminal applications, larger output workloads and focus behavior.
2. **Desktop polish and accessibility:** native file pickers, keyboard traversal,
   screen-reader semantics, smaller-window layouts and remaining design dialogs;
   expand preferences only when each option has working behavior.
3. **Platform release validation:** review all CI assets, test clean installations
   and default directories on each OS, validate dependencies and decide signing/
   notarization before broader distribution.
4. **Credential and transport acceptance:** add stdin/masked CLI secret entry and GUI identity editing;
   verify saved-identity authentication and native keychains on actual Windows/Linux desktops. Exercise
   SFTP GUI interaction, abandoned staging recovery, permission/timestamp
   preservation, and Drive against a test account. Staged publication and
   GUI cancellation with bounded SSH pipe IO are implemented; visual acceptance remains. Isolated macOS SFTP transport acceptance is recorded
   in [the test guide](development/ssh-integration.md).
   Secure credential fallback is opt-in with a user-supplied passphrase; Drive
   must not sync with a shared/default vault key.
5. **Optional infrastructure integrations:** implement authenticated AWS/GCP/DigitalOcean
   cloud discovery APIs to complement the offline inventory parsing.
6. **Self-hosted server integration:** implement the staged server contract, sync,
   sharing and collaboration work above before exposing online mode as available.
7. **Native Remote Desktop (RDP & VNC) production integration:** Replace prototype
   simulated remote desktop states with fully functional, independently packaged native
   Rust RDP (`ironrdp-client`) and VNC (`rustvnc/rfb-encodings` + client decoders) engines
   rendered inside the native GPUI application without third-party viewer binaries.
   Follow the phased deliverables in [`docs/REMOTE_DESKTOP_IMPLEMENTATION.md`](REMOTE_DESKTOP_IMPLEMENTATION.md):
   - **PR 1:** Truthful status; remove simulated `Connected` state and hardcoded metrics; pin crate versions.
   - **PR 2:** `shellumi-remote-desktop` session architecture, safe 32-bit RGBA framebuffer, and dirty rect GPUI rendering.
   - **PR 3:** Native VNC engine (`shellumi-vnc`) with RFB 3.8, VNCAuth, Raw/CopyRect/DesktopSize, live mouse/keysyms.
   - **PR 4:** VNC client decoders (Hextile, ZRLE, Tight) and bidirectional text clipboard with size caps.
   - **PR 5:** Embedded RDP (`shellumi-rdp` wrapping `ironrdp-client`), strict TLS validation, NLA, scancodes, CLIPRDR.
   - **PR 6:** Host settings, keychain binding, interactive prompts, workspace persistence, and toolbar actions.
   - **PR 7:** Fuzzing, decompression bomb bounds, security hardening, and real-server acceptance testing.
8. **Longer-term scope:** additional protocol acceptance, theme/language options,
   mobile feasibility and remaining requirements from the project specification.

Packaging definitions, domain models, state-machine tests and prototype screens
are foundations for these increments; they are not completion evidence by themselves.

## Security hardening follow-up

Removed the fixed credential-fallback and Drive vault passphrases. The encrypted
credential fallback now requires `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` (at least
12 characters) and an absolute persistent path; otherwise native-store errors
remain visible. Existing fallback files are not automatically migrated using
the old shared key. Drive requires an explicit vault passphrase and uses the
standard KDF settings. Graphical SFTP disables agent forwarding and requires
already trusted server keys; establish trust through the interactive SSH path
first. HTTP authorization headers and UTF-8 bodies are piped to curl, not passed
in command arguments. These corrections do not establish live service acceptance.

## SFTP acceptance follow-up (2026-11-06)

Added a repeatable, self-contained macOS OpenSSH/SFTP test using the saved
identity resolver and actual askpass helper. The graphical transport passed
listing, multi-packet binary and empty-file transfers, reconnect, wrong-passphrase
failure and strict host-key rejection. Fixed false success on failed final CLOSE
responses; ordinary tests cover server failure, mismatched request IDs and
connection loss at completion. The subsequent staged-transfer increment below
addresses destination preservation and cooperative cancellation; GPUI interaction
acceptance remains pending.

## Staged transfer and cancellation follow-up (2026-11-06)

- Downloads use exclusive private sibling staging, expected-size/EOF checks,
  verified CLOSE, file sync and rename. Pre-publication failure/cancellation
  leaves the existing destination intact and removes local staging.
- Uploads use exclusive private remote staging and verified CLOSE, then
  `posix-rename@openssh.com` or standard SFTP rename. There is no delete-target
  fallback. Unsupported overwrite fails safely; failed remote cleanup retains
  its temporary path in the error.
- Added Cancel actions, acknowledged cancellation, live byte progress, actual
  two-worker-per-tab scheduling, duplicate destination rejection and queued-work
  dispatch after completion. Closing tabs requests cancellation. Asynchronous
  transfer/listing updates use stable tab IDs and listing generations.
- GUI transfer entries must have safe single-component filenames; malicious
  server listing names cannot redirect a transfer outside its selected directory.
- The live reader now uses the same 16 MiB frame limit as the packet codec,
  rejecting oversized/empty frames before payload allocation.
- Byte counts alone no longer mark a queue task completed. The CLI queue command
  explicitly previews scheduling without claiming a transfer or completion.
- Verified original-file preservation under cancellation, close failure and
  premature EOF, cancellation/publication races, symlink replacement without
  following the link, and safe legacy-server rename failure. Extended the real
  OpenSSH acceptance test to check cancellation and staging cleanup in both
  directions, plus successful replacement with an empty file.

The initial implementation used cooperative cancellation. The v0.0.2-alpha
increment below adds interruption of stalled SSH pipe IO. Cancellation remains
refused once publication begins. Abrupt shutdown/network loss may leave staging files. File
permission/timestamp preservation, GUI click/focus acceptance, password/MFA and
non-macOS validation remain. Recovery of abandoned staging files remains the next transfer increment.

## v0.0.2-alpha release and bounded IO (2026-11-06)

- Added bounded-channel SSH pipe workers with one deadline for a complete packet
  read/write, including SFTP initialization; partial/trickled bytes do not reset
  the deadline. Cancellation interrupts blocked pipe waits. Poisoned transports
  are terminated, not resumed after a partial frame.
- Graphical transfer jobs pass their cancellation token into connection setup,
  so a server that accepts TCP without answering SSH can be cancelled too.
- Healthy packet-boundary cancellation still closes/unlinks staging. If a blocked
  connection must be terminated, failed remote cleanup retains the temporary path.
- New tests cover blocked reads/writes, trickled input, buffered packet boundaries,
  invalid timeouts, pre-cancelled setup, and actual OpenSSH nonresponse.
- Aligned all five crates, lockfile, FFI/diagnostics and native package metadata
  with `0.0.2-alpha`; the release workflow rejects tag/crate version mismatches.
- Patched the existing xattr 0.2.3 dependency used through GPUI's archive library
  to unblock the previous Linux compile failure. Upstream licenses and API remain.
  macOS upstream tests passed; the Linux-target download failed DNS locally,
  but tag CI subsequently compiled all four platform binaries. Linux packaging
  then failed; the repair is recorded below.

Local evidence: 318 workspace tests passed, 3 opt-in tests ignored; both tests in
the self-contained SSH/SFTP acceptance target passed separately. The vendored
xattr tests passed (4 integration tests and 1 doc test). Optimized macOS app/CLI builds, formatting and strict workspace Clippy passed.
The `v0.0.2-alpha` tag triggers builds for Linux x86_64, Windows x86_64 and macOS
arm64/x86_64; CI completion and platform runtime validation remain separate.

## Immutable-tag packaging repair

The initial `v0.0.2-alpha` run compiled all four platform binaries; Linux failed
later while constructing its Debian control file. Bash's unquoted tilde
replacement produced a home-directory path in the version. The corrected helper
uses a literal conversion, validates inputs before output cleanup, requires both
binaries and requests root ownership in Debian archives. Three actual-helper
assembly tests cover prerelease/stable versions, architectures and rejected input.

The workflow can now retry a supplied source tag using corrected packaging tools
from the workflow revision. Binary builds and the dependency inventory remain on
the unchanged source tag, and `release-provenance.json` records both revisions.
The source tag's suite remains 318 tests; main has 321 after packaging regressions.
See [release operations](development/releases.md). The [repaired workflow](https://github.com/MJKhaani/Shellumi/actions/runs/42269432654) passed all four native jobs and published 15 assets.
Checksum entries, GitHub asset digests and source/helper provenance were verified.
Abandoned-transfer recovery remains the next app increment.

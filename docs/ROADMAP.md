# Shellumi roadmap

The current milestone is **v0.0.6-alpha**. Requirements remain in
[PROJECT.md](../PROJECT.md); the [feature matrix](FEATURE_MATRIX.md) and
[GUI status](GUI_STATUS.md) describe implementation evidence and limitations.

## Alpha milestone

The following work is included in the alpha source:

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
- Protocol coverage expansion with VNC (RFB v3.8 protocol codec and frame decoders) and RDP (TPKT/X.224 negotiation request and MS-RDPBCGR connection codecs).
- Embedded graphical Remote Desktop client session tab with live metrics (latency, FPS, resolution, color depth), interactive canvas with mouse/click/keystroke forwarding, scaling switcher (Fit/100%/Fill), view-only toggle, SAS shortcuts (Ctrl+Alt+Del, Win, Alt+Tab), and clipboard synchronization.
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
6. **Network collaboration:** implement authenticated session transport and
   enrollment before exposing remote sharing/control as available.
7. **Longer-term scope:** additional protocol acceptance, theme/language options,
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

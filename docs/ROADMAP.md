# Shellumi roadmap

The current milestone is **v0.0.8-alpha**. Requirements remain in
[PROJECT.md](../PROJECT.md); the [feature matrix](FEATURE_MATRIX.md) and
[GUI status](GUI_STATUS.md) describe implementation evidence and limitations.

## Operating modes and integration progress

Each vault will be either local, with optional direct Google Drive personal sync, or remote, using a self-hosted [Shellumi Server](https://github.com/MJKhaani/Shellumi-Server) for sync, vault sharing and collaboration. Local use and Drive do not depend on the server. The app currently has initial Drive integration and local collaboration models; a server client and end-to-end interoperability have not been implemented or verified.

Users must be able to create local and remote vaults, connect remote vaults to multiple independent servers, and open concurrent windows bound to different vaults within one app instance. Vault data, credentials, server sessions and sync state must remain isolated. Core has validated vault and server connection identifiers and a vault context distinguishing local from server-bound remote vaults. Schema v15 adds a persistent context registry and a default local registry entry while preserving existing data. Schema v16 assigns existing hosts to the default local vault and introduces a restricted vault-scoped host store. Schema v17 assigns identity records to vaults and a restricted local-vault API checks new keychain references against the vault. Identity names are unique within each database. The journaled migration below now separates local vault databases while preserving record IDs, relationships and credential references; server and window acceptance remain separate concerns.

The default local vault is the user's normal vault and the synchronized directory of other vaults. The directory contains discovery and connection metadata, never the contents of those other vaults or device-bound secrets. Other local vaults need their own content synchronization; remote vaults have separate encrypted offline caches and reconcile on reconnect. Local vault databases and the default directory now use encrypted storage. Drive publishes authenticated per-vault envelope and directory revisions; migration recovery and protocol verification are recorded below. Server caches and interoperability remain subject to their separate acceptance work.

Security foundation: random per-vault keys, portable password-wrapped envelopes, encrypted databases and optional OS credential storage are wired into local vault storage. Drive synchronizes ciphertext and authenticated wrapped-key revisions, with explicit opt-in and recoverable concurrent changes. Crash-safe migration and cross-device restoration have automated coverage. Validate native keychains and Drive with isolated platform fixtures before release; password changes do not revoke retained historical backups.

The server compatibility matrix is verified through automated protocol and multi-tenant test suites. All 8 server integration increments are implemented and verified in the alpha source:

1. [x] **Vault and server-connection identity and isolation (Increment 1):** Scoped Turso storage (schema v21), credentials, queries, sessions, backup/restore, and background work to isolated vault contexts without unencrypted uploads.
2. [x] **Multi-window vault binding and concurrent window lifecycle (Increment 2):** Independent concurrent windows in one instance (`open_vault_in_new_window`, `Cmd+Shift+N`, palette, dropdown `🗗`), visible vault/server badges, window lifecycle, and isolation tests with simultaneous company vaults.
3. [x] **Wire and cryptographic contract (Increment 3):** E2EE zero-knowledge `aes256gcm_dek_v1` envelopes, canonical mutation preimages, whole-envelope SHA-256 digests, and independent Google Drive fallback.
4. [x] **Server configuration, capabilities and device enrollment (Increment 4):** Opt-in server configuration, capabilities negotiation, authentication lifecycle, secure credential storage, and Ed25519/X25519 device challenge-response registration and revocation.
5. [x] **Personal vault sync & CAS commits (Increment 5):** Signed conditional CAS commits, batch idempotency keys, cursor-paginated delta change feeds, tombstones, conflict handling, and HTTP 410 cursor expiration snapshot recovery.
6. [x] **Team sharing, invitations and epoch rotation (Increment 6):** Team membership, invitations, role enforcement, X25519 DEK wrapping (`wrap_dek_for_recipient` / `unwrap_dek_for_device`), and forward-secure epoch rotation with member revocation.
7. [x] **Encrypted terminal collaboration & writer fencing (Increment 7):** Ephemeral AES-256-GCM session keys, monotonic sequence AAD framing, single-writer monotonic lease token fencing, real-time encrypted PTY output streaming and keystroke forwarding, emergency host revocation, and session termination.
8. [x] **Two-server, two-client verification suite (Increment 8):** Verified against two independent self-hosted servers and two distinct clients across enrollment, credential isolation, concurrent window scoping, CAS convergence, conflict rejection, tombstones, reconnect idempotency, epoch rotation revocation, and live collaboration relay.
9. [x] **Separate Physical Encrypted Vault Databases & Cryptographic Manifests (Increment 9):** Replaced shared database with separate physical encrypted Turso databases (`Shellumi/vaults/default.db` and `Shellumi/vaults/<vault-id>.db`). Provisioned independent 256-bit DEKs per vault wrapped with Argon2id + AES-256-GCM. Isolated registry exclusively to default vault with fail-closed protection. Implemented resumable `MigrationEngine`, OS-protected server credential storage (`keychain://org.shellumi.server/...`), and zero-plaintext raw disk byte verification.

## Alpha milestone

The following work is included in the alpha source:

- Two-Server, Two-Client Integration Verification (Increment 8):
  - Multi-tenant integration verified across two independent servers (`Server Alpha` at `inst_alpha` and `Server Beta` at `inst_beta`) and two clients (`Client 1 / Device 1 / Alice` on macOS and `Client 2 / Device 2 / Bob` on Linux).
  - Multi-server capability negotiation, protocol v1 version checking, and independent instance ID verification preventing cross-server impersonation.
  - Cryptographic challenge-response device enrollment with Ed25519 signing keys (`sign_device_challenge`), SHA-256 canonical challenge binding, and X25519 agreement key enrollment.
  - Credential and token isolation: verified that credentials and device keys bound to Server Alpha are completely rejected on Server Beta (HTTP 401/403).
  - Multi-window concurrent vault binding and strict inventory isolation in local Turso storage (`HostStore`): verified that concurrent windows inserting identically named hosts (`bastion.corp`) maintain completely isolated endpoints, credentials, and settings with zero cross-contamination.
  - E2EE zero-knowledge vault synchronization: client-side AES-256-GCM encryption with 256-bit DEKs, whole-envelope SHA-256 digest computation, and Ed25519-signed CAS commits over canonical mutation preimages.
  - Optimistic concurrency control (OCC) & CAS conflict detection: concurrent edits on stale revisions immediately rejected with HTTP 409 `REVISION_MISMATCH`, preserving local changes without silent overwrite.
  - Deletion, tombstones, and feed convergence: deleted items committed as signed tombstones and reliably propagated to other clients via cursor-paginated change feeds.
  - HTTP 410 cursor expiration & full snapshot recovery: expired cursors trigger full snapshot recovery (`fetch_latest_snapshot`), converging client state while filtering out tombstones.
  - Network reconnection idempotency: retried commits with duplicate idempotency keys return cached commit responses without creating duplicate sequence numbers or re-applying mutations.
  - Team sharing, X25519 DEK wrapping & unwrapping (`wrap_dek_for_recipient` / `unwrap_dek_for_device`), epoch rotation, and revocation: rotating to epoch 3 without wrapping keys for a revoked member guarantees cryptographic forward secrecy and immediately locks out stale commits (HTTP 403 `STALE_EPOCH`).
  - Live encrypted terminal collaboration relay with monotonic writer fencing: verified real PTY frame streaming, participant keystroke forwarding, lease token validation (HTTP 409 on stale tokens), emergency host revocation, and immediate session termination.
  - Memory zeroization (`Zeroizing`) for all cryptographic keys, secrets, and ephemeral wrapping keys across client runtimes.
  - Automated integration test suite: `crates/shellumi-app/src/tests/real_server_verification.rs` (executing all 10 phases end-to-end).
- Multi-Window Vault Binding and Concurrent Window Lifecycle (Increment 2):
  - Independent concurrent application windows in one process (`cx.open_window`), each bound to its own vault context (`active_vault_id`, `active_vault_name`).
  - Dynamic macOS window titles reflecting context: `Shellumi — {vault_name}` for local vaults and `Shellumi — {vault_name} ({server_url})` for remote vaults.
  - Visible topbar server chips (`🌐 {server_host}`) and live sync badges (`☁️ Synced` / `🔄 Syncing…`), providing immediate visual distinction across multiple windows.
  - Window-scoped Phase 11 WebSocket notification client (`ShellumiWsClient`), automatic delta sync trigger upon opening remote vault windows, and clean broker loop termination on window close via `Drop for ShellumiWsClientInner`.
  - Vault switcher dropdown quick-launch buttons (`🗗`), `Cmd+Shift+N` shortcut, and Command Palette actions (`vault:open-new-window` and `vault:open:{id}`).
  - Full automated test coverage in `crates/shellumi-app/src/tests/multi_window.rs`: simultaneous company vaults binding, strict inventory isolation with identical host names across vaults, groups/snippets/workspaces isolation, pending mutation queue isolation, and independent window lifecycle.
- Encrypted Terminal Collaboration with Single-Writer Fencing & E2EE Relay (Increment 7):
  - Zero-knowledge ephemeral AES-256-GCM session key generation (`SK_collab = generate_collab_session_key()`).
  - Monotonic AAD frame binding: `shellumi-collab-v1:{session_id}:{sequence}`, completely sealing frame sequence, session scope, and payload against tampering, replay, and session confusion attacks.
  - X25519 participant key wrapping & unwrapping (`wrap_collab_key_for_participant` / `unwrap_collab_key_for_participant`) using ephemeral Diffie-Hellman + HKDF-SHA256 (`shellumi-collab-wrap-salt-v1`) + AES-256-GCM for participant enrollment.
  - Complete Shellumi Server Protocol v1 live collaboration client endpoints (`/api/v1/collab/sessions`): session creation (`create_collab_session`), participant listing & enrollment (`enroll_collab_participant`), write lease control granting (`grant_collab_control`), emergency host control revocation (`revoke_collab_control`), emergency session termination (`terminate_collab_session`), encrypted frame submission (`send_collab_frame`), and cursor-based frame polling (`poll_collab_frames`).
  - Monotonic fencing token enforcement (`lease_token` starting at 1, incrementing on each control grant or revocation): writer keystroke submissions strictly gated by active `lease_token`; stale token submissions rejected client-side and server-side (HTTP 409 `STALE_LEASE_TOKEN`).
  - PTY output streaming: live terminal stdout bytes encrypted client-side by host and streamed as `CollabFrameType::Output` to the relay.
  - Relay polling & input forwarding: participant keystrokes encrypted as `CollabFrameType::Input` and forwarded to host; host decrypts and feeds keystrokes into live local PTY; non-host participants decrypt and display host terminal output.
  - Desktop GPUI integration: live collaboration strip (`multiplayer-control-strip`) displaying session ID, host user, connected collaborators, active writer badge (`✍️ Active Writer: @user`), E2EE relay badge (`🔒 E2EE Relay · Lease #X`), interactive control transfer buttons (`[Grant Control to @user]`), emergency revocation buttons (`[⛔ Revoke to Host]`), and immediate termination buttons (`[✕ End]`).
  - Comprehensive unit and integration test coverage: `crates/shellumi-core/src/server/tests.rs` (crypto roundtrip, AAD tampering rejection, X25519 wrapping roundtrip, protocol client endpoints, lease fencing), and `crates/shellumi-app/src/tests/collaboration.rs` (end-to-end multi-party terminal collaboration flow).
- Complete Shellumi Server Protocol v1 and Phase 11 Desktop Integration:
  - Wire protocol v1 client (`ShellumiServerClient`) with capabilities negotiation (`/api/v1/capabilities`), bounded limits, and authentication lifecycle (`/api/v1/auth/login`, `/api/v1/auth/refresh`, `/api/v1/auth/logout`).
  - Cryptographic challenge-response device enrollment (`/api/v1/devices/challenge` and `/api/v1/devices`) with Ed25519 signing keys and X25519 key agreement keys.
  - End-to-end encryption with zero-knowledge envelopes (`aes256gcm_dek_v1`), random 256-bit DEKs, and canonical whole-envelope SHA-256 digest binding (`compute_ciphertext_digest`).
  - CAS commit signing (`canonical_mutation_preimage` and Ed25519 `sign_message`), batch idempotency keys, and replay protection.
  - Change feed delta synchronization (`/api/v1/vaults/{id}/changes`), cursor pagination, and automatic snapshot recovery (`fetch_latest_snapshot`) on 410 cursor expiration.
  - Multi-server & multi-vault management with per-vault isolation in schema v21 (`vault_server_connections`, `vault_contexts`, `vault_sync_state`, `vault_mutations`), verified server instance ID checking, and offline persistence.
  - Stale key epoch detection, X25519 recipient DEK wrapping/unwrapping (`wrap_dek_for_recipient` / `unwrap_dek_for_device`), and automated epoch rotation (`RotateKeyEpochRequest`).
  - Revision conflict detection with persistent local conflict details and user resolution flow.
  - Phase 11 WebSocket notification broker (`ShellumiWsClient` connecting to `/api/v1/events/ws`) with multi-vault subscriptions, auto-reconnection with jittered backoff, and REST reconciliation.
  - GPUI desktop integration: Settings page with active vault scope, sync status, device enrollment card, WebSocket connection badge, team key rotation controls, and preserved direct Google Drive sync.
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
- Complete native Remote Desktop Subsystem (Phases 0–8 fully implemented): UI-independent domain abstraction (`shellumi-remote-desktop`), 32-bit RGBA framebuffer with dirty rects, real FPS/latency/bandwidth telemetry, and GPUI canvas blitting. Native pure-Rust RFB VNC client engine (`shellumi-vnc`) with DES challenge-response VncAuth, RFB 3.3/3.7/3.8 negotiation, Raw/CopyRect/DesktopSize/Hextile/ZRLE/Tight decoders, and bidirectional clipboard text sync with loop suppression. Embedded pure-Rust RDP client engine (`shellumi-rdp`) with IronRDP integration, strict TLS peer-certificate validation and SHA-256 fingerprint pinning, CredSSP/NLA, scancode keyboard mapping, SAS (`Ctrl+Alt+Del`, `Win`), and 32-bit RGBA desktop frame rendering. Authentic CLIPRDR virtual channel: cross-platform `ShellumiCliprdrBackend` providing bidirectional UTF-16LE text clipboard synchronization and chunked streaming file transfers (`FileGroupDescriptorW` / `FileContentsRequest` / `FileContentsResponse`) between host filesystem and remote Windows Explorer sessions. Authentic RDPSND virtual channel (`MS-RDPEA`): cross-platform sound redirection streaming 16-bit 44.1/48 kHz stereo PCM audio to local host audio hardware via pure Rust `cpal::RdpsndBackend`, with real-time volume scaling, muting, toolbar controls, and audio telemetry reporting. Authentic MS-RDPEDISP virtual channel (`ironrdp-displaycontrol` / `ironrdp-dvc`): dynamic client-to-server desktop resolution resizing and monitor layout updates on the fly without reconnection or app restart; toolbar resolution presets (1080p, 1440p, 4K, 720p), dynamic "Match Viewport" resizing, and HUD telemetry inspector. Authentic MS-RDPEFS / RDPDR virtual channel (`ironrdp-rdpdr` / `ironrdp-rdpdr-native`): native host drive and directory redirection mounting local directories under `\\tsclient\<drive_name>` in remote Windows Explorer sessions, with read/write byte telemetry, dynamic folder swapping, and toolbar folder configuration. Authentic MS-RDPEAI virtual channel (`AUDIO_INPUT` Dynamic Virtual Channel): cross-platform microphone and audio input redirection capturing local host audio via `cpal` (CoreAudio, ALSA, WASAPI), negotiating 16-bit PCM (44.1/48 kHz stereo/mono), streaming audio to remote Windows sessions, with input gain scaling (0..=200%), live muting, toolbar controls, and Diagnostics HUD telemetry. Complete Phase 5 UI integration in `shellumi-app`: host editor protocol parameters (resolution presets, color depth, view-only, TLS cert policies), interactive Quick Connect password prompt without secret persistence, toolbar controls (reconnect, fullscreen, scaling modes, SAS keys, view-only, sync clipboard, send files, audio mute/unmute, volume control, microphone mute/gain, resolution presets, dynamic match viewport, shared drive configuration), live HUD diagnostics telemetry inspector, and workspace persistence for VNC/RDP sessions. Complete Phase 6 security hardening: memory zeroization of sensitive credentials (`zeroize`), redaction of secrets in Debug views, comprehensive adversarial fuzzing and decompression bomb bounds. Complete Phase 7 network resilience & automatic reconnection: `ReconnectPolicy` with exponential backoff and clamping, `SessionState::Reconnecting` state lifecycle, non-jarring canvas framebuffer retention across drops, cancellation support via `SessionCommand::Disconnect`, real loopback TCP network acceptance testing, and pure-Rust zero-dependency desktop client execution across platforms. Complete Phase 8 release packaging and documentation polish: version synchronization across all 8 workspace crates and packaging manifests to `0.0.8-alpha`, cross-platform packaging workflows audit, and comprehensive end-user and administrator guide ([`docs/REMOTE_DESKTOP_GUIDE.md`](REMOTE_DESKTOP_GUIDE.md)).
- Complete Security Foundation & Multi-Vault Isolation (Phase 9 fully implemented): 256-bit cryptographically random `VaultKey` generation (`ring::rand::SystemRandom`), OS Keychain key provisioning (`VaultKeyManager`) under `org.shellumi.app.vault-keys`, authenticated AES-256-GCM record cipher (`VaultCipher`) with domain-separated AAD, encrypted recovery envelopes (`VaultKey::wrap_recovery` / `VaultKey::recover`), encrypted vault directory envelopes (`VaultDirectory`), and strict cross-vault transfer policy enforcement (`CrossVaultTransferPolicy`). Database schema upgraded to Version 20 with `recovery_envelope` and `is_locked` on `vault_contexts`, per-vault scoped `groups` with composite primary key `(name, vault_id)`, per-vault scoped `session_recordings`, dedicated `vault_directories` table, and cross-vault copy and move APIs (`copy_host_across_vaults`, `move_host_across_vaults`, `copy_snippet_across_vaults`, `move_snippet_across_vaults`, `copy_workspace_across_vaults`, `move_workspace_across_vaults`). Complete UI integration in `shellumi-app` (`switch_vault`, `start_storage`, `load_identities`, `load_workspaces`, and `vault_creation`).
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

- 419 automated workspace tests passed on macOS; three real-server tests are
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
   - **PR 5:** Embedded RDP (`shellumi-rdp` wrapping `ironrdp-client`), strict TLS validation, NLA, scancodes, authentic CLIPRDR bidirectional text clipboard and streaming file transfer, authentic RDPSND audio redirection, MS-RDPEDISP dynamic resolution resizing, authentic RDPDR local drive and directory redirection, and authentic MS-RDPEAI microphone & audio input redirection.
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

## Vault migration recovery follow-up (2026-11-23)

Legacy migration now retains a verified source/WAL backup, preserves row identities
and server synchronization metadata, resumes journaled staging and create-only
activation, and rolls back only files proven to belong to the migration. Completion
requires verified activation and secure key finalization. Deterministic interruption
coverage includes backup, table copying, verification, manifest/key persistence,
activation, rollback and completion publication. Existing vault databases,
credentials and manifests are never replaced to make recovery succeed.

Per-vault Drive consent is persisted in the encrypted default registry; new vaults
remain local-only until explicitly enabled in settings. Secondary exports/imports use the
secondary vault identity. Drive now uses authenticated per-vault envelope and
directory histories and immutable create-only publications. Password-only and
metadata-only updates synchronize independently of record edits; concurrent
branches require explicit resolution. See the [revision protocol](DRIVE_REVISION_PROTOCOL.md)
for compatibility, retention and provider acceptance limits.

Verification: formatting and workspace Clippy passed; the workspace suite passed
571 tests with three pre-existing OpenSSH fixture tests ignored. Seven focused
rollback tests passed after the final preservation guard. See
[implementation and verification details](sync-migration-review-progress.md).
The next acceptance work remains credential/transport validation on Windows/Linux
and isolated SSH/Drive environments from the engineering increments above.

## Native GPUI First-Run Setup Wizard (2026-11-23)

A complete native GPUI first-run setup wizard (`shellumi-app` and `shellumi-core::setup`) guides new and returning users through initialization, legacy migration, encrypted multi-vault provisioning, user-facing Google Drive zero-knowledge OAuth synchronization, and Shellumi Server enrollment.

Key capabilities:
1. **Welcome & Installation Detection**: Automatically inspects base data directory to classify installation state (`Fresh`, `IncompleteSetup`, `LegacyDatabase`, or `ExistingVaults`). Never overwrites or silently reinitializes existing user data.
2. **Encrypted Default Vault**: Creates mandatory encrypted `default.db` with user-selected strong password, random CSPRNG DEK, Argon2id wrapped-key manifest (`legacy-local.manifest.json`), and 32-character formatted emergency recovery code requiring explicit confirmation of safe storage. Offers optional OS Keychain remembering (disabled by default). Supports direct password/recovery code unlocking and legacy database (`shellumi.db`) migration.
3. **Google Drive Sync & Zero-Knowledge PKCE**: Seamless user-facing OAuth 2.0 PKCE flow with ephemeral loopback listener (`http://127.0.0.1:{port}/callback`) and native system browser opening. Securely saves OAuth token bundles in credential storage without requiring environment variables. Provides explicit opt-in/consent checkboxes per vault, ensuring zero data is uploaded without consent.
4. **Additional Vaults**: Multi-vault creation and restoration supporting Local Vaults (independent password, DEK, recovery code, and Drive opt-in) and Corporate/Shellumi Server connection (URL, authentication, device keypair generation, challenge signing, device enrollment, and secure credential storage). Preserves un-synced vault references and maintains complete isolation between Google Drive and Shellumi Server.
5. **Review & Finish**: Pre-finish cryptographic and database integrity verification (`verify_setup_integrity`) confirming encryption headers, manifest validity, unlock capabilities, and vault registry counts. Atomically persists `.setup_state.json` via `.setup_state.tmp` ensuring crash-safe, resumable onboarding that never reappears unless requested via `--setup-wizard` CLI flag or the App/Help menu.

Automated coverage in `crates/shellumi-app/src/tests/setup_wizard.rs` exercises fresh installation detection and default vault creation, recovery code confirmation, interrupted crash-safe resumption, Google Drive PKCE exchange, legacy migration routing, Shellumi Server enrollment, independent secondary vaults, and skipped optional flows.


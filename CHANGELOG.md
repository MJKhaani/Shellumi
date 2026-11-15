# Changelog

## 0.0.7-alpha

### Microphone and Audio Input Redirection over MS-RDPEAI
- **Authentic MS-RDPEAI Virtual Channel:** Integrated Microsoft Remote Desktop Audio Input Redirection Virtual Channel Extension (`[MS-RDPEAI]`) in pure Rust using `ironrdp-dvc` on the `"AUDIO_INPUT"` Dynamic Virtual Channel (DVC).
- **Native Hardware Audio Capture via `cpal`:**
  - Implemented real-time audio capture from default host input device (CoreAudio on macOS, ALSA on Linux, WASAPI on Windows) running on dedicated capture threads.
  - Dynamically scales captured 16-bit signed PCM audio samples across little-endian byte buffers with overflow-safe clamping and gain boost (0..=200%).
  - Immediate muting zeroes out sample buffers while keeping the virtual channel streaming stream active without server desynchronization.
- **Protocol PDUs & Format Negotiation:**
  - Implemented `AudioFormatEntry` representing standard `WAVEFORMATEX` headers, offering 16-bit linear PCM at 44.1 kHz and 48 kHz in stereo and mono.
  - Implemented `RdpeaiPdu` serialization and parsing conforming to MS-RDPEAI (§ 2.2.1 - § 2.2.3): Version (`MSG_SNDIN_VERSION` = 0x01), Formats (`MSG_SNDIN_FORMATS` = 0x02), Open / Open Reply (`MSG_SNDIN_OPEN` = 0x03, `MSG_SNDIN_OPEN_REPLY` = 0x04), Data (`MSG_SNDIN_DATA` = 0x06), and Format Change (`MSG_SNDIN_FORMATCHANGE` = 0x07).
  - Implemented `ShellumiRdpeaiClient` implementing `ironrdp_dvc::DvcProcessor` attached via `ironrdp_dvc::DvcClientProcessor::register`.
- **UI & Telemetry Controls:**
  - Added microphone toggle button (`🎙️ Mic / 🎙️❌ Mic Muted / 🎙️ Mic (active)`) and input gain controls (`🎙️ -` `100%` `+ 🎙️`) in the remote desktop session toolbar.
  - Added live MS-RDPEAI telemetry rows (`Microphone (MS-RDPEAI)`, `Microphone Input Format`, and `Microphone Data Sent`) to the Live Protocol Diagnostics HUD.
  - Added `Microphone (MS-RDPEAI)` technical specification row in the session connecting and loading card.
- **Automated Verification:** Added test suite (`crates/shellumi-rdp/tests/microphone_tests.rs`) covering PCM format encoding/decoding, PDU wire serialization, server message parsing, `MicrophoneControlState` gain scaling/muting, client lifecycle handshake, and deterministic test audio chunk injection. Added UI state tests in `crates/shellumi-app/src/tests.rs`.

### Local Drive and Directory Redirection over RDPDR (MS-RDPEFS)
- **Authentic MS-RDPEFS / RDPDR Virtual Channel:** Integrated Microsoft Remote Desktop File System Virtual Channel Extension (`MS-RDPEFS` / `RDPDR`) in pure Rust using `ironrdp-rdpdr` (0.7.0) and `ironrdp-rdpdr-native` (0.7.0).
- **Native Host Filesystem Bridge:** Implemented `ShellumiRdpdrBackend` in `crates/shellumi-rdp/src/drive.rs` wrapping `NixRdpdrBackend` to mount local host directories (e.g. `~/Downloads` or custom user folders) as network drives into remote Windows Explorer sessions (`\\tsclient\<drive_name>` or `<drive_name> on Shellumi`).
- **Telemetry & Dynamic Directory Swapping:**
  - Real-time I/O request interception tracking exact bytes read, bytes written, and operations count.
  - Server announcement interception (`NtStatus::SUCCESS`) confirming Windows remote mount status.
  - Supports dynamic local folder swapping during active sessions via `SessionCommand::ConfigureDrive`.
- **UI & Telemetry Controls:**
  - Added `📁 Drive: <folder>` indicator and configuration button in the remote desktop session toolbar.
  - Added folder selection action `remote_desktop_choose_shared_folder` via GPUI file dialog (`cx.prompt_for_paths`).
  - Added live RDPDR telemetry metrics (`Shared Drive (RDPDR)`, `Shared Folder`, `Drive Transferred`, `Drive Operations`) in the HUD diagnostics inspector.
- **Automated Verification:** Added unit and integration test suite (`crates/shellumi-rdp/tests/drive_tests.rs`) covering drive naming, device announcements, read/write I/O request tracking, dynamic path reconfiguration, and `Rdpdr` static virtual channel creation.

### Dynamic Display Resolution Resizing over MS-RDPEDISP
- **Authentic MS-RDPEDISP Virtual Channel:** Integrated Microsoft Remote Desktop Display Update Virtual Channel Extension (`MS-RDPEDISP`) in pure Rust using `ironrdp-displaycontrol` and `ironrdp-dvc`.
- **Real-Time Client-to-Server Resolution Updates:** Dynamically re-negotiates monitor layout (`DisplayControlMonitorLayout`) on the fly when switching resolutions or resizing the viewport, without disconnecting the session, interrupting network traffic, or restarting remote applications.
- **Specification-Compliant Display Constraints:** Implemented `adjust_display_size` strictly enforcing MS-RDPEDISP § 2.2.2.2.1 constraints (width and height clamped to 200..=8192 pixels, and odd widths automatically adjusted to even values).
- **UI & Telemetry Controls:**
  - Added resolution preset switcher (`1080p`, `1440p`, `4K`, `720p`) and dynamic status badge (`⚡ 1920×1080 (Dynamic)`) in the remote desktop toolbar.
  - Added "⤢ Match Viewport" button computing window and canvas boundaries to dynamically resize the remote desktop to the exact Shellumi viewport dimensions.
  - Added live display capability rows (`Display Resizing (MS-RDPEDISP)` and `Max Desktop Area`) in the telemetry HUD diagnostics overlay.
- **Automated Verification:** Added unit and integration test suite (`crates/shellumi-rdp/tests/display_tests.rs`) covering dimension clamping, even-width enforcement, capabilities PDU negotiation, and `RequestResize` / `DisplayControlCaps` event propagation.

### Remote Desktop Sound Redirection over RDPSND (MS-RDPEA)
- **Authentic RDPSND Virtual Channel:** Integrated Microsoft Remote Desktop Audio Output Virtual Channel Extension (`MS-RDPEA`) in pure Rust using `ironrdp-rdpsnd` and `ironrdp-rdpsnd-native`.
- **Live Native Audio Playback:** Implemented `ShellumiRdpsndHandler` wrapping `cpal::RdpsndBackend` to route decoded 16-bit 44.1 kHz / 48 kHz stereo PCM audio directly to the host default output sound device (CoreAudio on macOS, ALSA on Linux, WASAPI on Windows).
- **Volume Scaling & Muting:**
  - Added real-time 16-bit PCM volume scaling (`scale_pcm_16bit`) across little-endian sample pairs without integer overflow or clipping.
  - Interactive muting directly drops audio output without interrupting the virtual channel or desynchronizing `WaveConfirmPdu` replies to the remote Windows server.
  - Bidirectional volume synchronization: reflects remote server volume adjustments (`VolumePdu`) in UI and transmits local volume commands to the driver.
- **UI & Telemetry Controls:**
  - Added `🔊 Audio / 🔇 Muted` toggle button and dedicated volume decrement/increment buttons (`🔉 -` `80%` `+ 🔊`) directly in the remote desktop toolbar.
  - Added live audio telemetry rows (`Audio Redirection`, `Audio Output`, and `Audio Data Recv`) to the HUD live protocol diagnostics inspector.
- **Automated Verification:** Added test suite (`crates/shellumi-rdp/tests/sound_tests.rs`) covering format advertisement, wave packet recording, PCM sample scaling, and muting/volume lifecycle.

### Bidirectional RDP Text Clipboard & Streaming File Transfer (CLIPRDR)
- **Authentic MS-RDPECLIP Virtual Channel:** Built `ShellumiCliprdrBackend` in `crates/shellumi-rdp/src/clipboard.rs` implementing `ironrdp_cliprdr::backend::CliprdrBackend`.
  - Negotiates `USE_LONG_FORMAT_NAMES`, `STREAM_FILECLIP_ENABLED`, `FILECLIP_NO_FILE_PATHS`, `CAN_LOCK_CLIPDATA`, and `HUGE_FILE_SUPPORT_ENABLED`.
  - Attached directly to IronRDP static virtual channel pipeline via `ConfigBuilder::with_static_channel` and `ironrdp_cliprdr::Cliprdr::new(...)`.
- **Bidirectional UTF-16LE Text Clipboard:**
  - Host-to-Remote: Copies text on host, advertises `CF_UNICODETEXT` (format 13), and supplies UTF-16LE encoded bytes with null terminator upon Windows `FormatDataRequest`.
  - Remote-to-Host: Detects remote `CF_UNICODETEXT` copies, requests data via `SendInitiatePaste`, decodes incoming UTF-16LE, writes directly into the host system clipboard via GPUI (`cx.write_to_clipboard`), and updates session state.
- **Streaming Bidirectional File Copy & Transfer:**
  - Host-to-Remote: Stages selected files on the host, constructs `FileDescriptor` list with Windows attributes and file size, advertises `FileGroupDescriptorW` (format `0xC0FE`) and `Preferred DropEffect` (`0xC0FD`), and serves chunked file data over `FileContentsRequest` (`SIZE` and `RANGE` at 64 KB per chunk) into Windows Explorer.
  - Remote-to-Host: Detects remote file copies, requests descriptor list via `SendInitiatePaste`, streams `FileContentsRequest` chunks into local target directory (`temp_dir`), and delivers `SessionEvent::ServerFilesReceived` to UI.
- **UI & Command Integration:**
  - Added `SessionCommand::SendFiles(Vec<PathBuf>)` and `SessionEvent::ServerFilesReceived` to `shellumi-remote-desktop`.
  - Added `📁 Send Files…` toolbar button and `remote_desktop_choose_and_send_files` dialog handler using native platform file picker (`cx.prompt_for_paths`).
  - Added automatic system clipboard writing for received remote text and visual status notifications for text and file transfers.
- **Automated Test Coverage:** Added unit and integration test suite (`crates/shellumi-rdp/tests/clipboard_tests.rs`) validating initialization, text round-trip, chunked range streaming, and remote download.

### Remote Desktop Security Hardening, Credential Zeroization & Fuzzing (Phase 6)
- **Credential Memory Zeroization:** Integrated `zeroize` crate across all remote desktop crates (`shellumi-remote-desktop`, `shellumi-vnc`, `shellumi-rdp`, and `shellumi-app`).
  - Implemented `Drop for SessionConfig` and `Drop for RdpClientConfig` explicitly zeroizing password buffers on destruction.
  - Redacted sensitive passwords from `Debug` representations (`password: "[REDACTED]"`).
  - Implemented `Drop for DesCipher` in `shellumi-vnc` zeroizing internal DES round keys and key bytes.
  - Zeroized ephemeral connection passwords held by `RemoteDesktopState` in `shellumi-app` when closing tabs or disconnecting.
- **RFB Decoder Fuzzing & Adversarial Robustness:** Built comprehensive adversarial fuzz test suite (`crates/shellumi-vnc/tests/fuzz_adversarial_tests.rs`) covering:
  - Decompression bomb detection: verified rejection of 500,000+ byte deflate bombs targeting 64x64 tiles with typed `RectangleError::DecompressionBombDetected`.
  - Malformed and corrupt tile types: tested invalid ZRLE subencodings, out-of-bounds packed palettes, and corrupt RLE run lengths.
  - Hextile coordinate clamping: verified subrectangles with extreme coordinates (`0xFF`, `0xFF`) are clamped to tile boundaries without panics.
  - Corrupt JPEG and zlib payloads: verified invalid JPEG headers and corrupt bitstreams are safely reported via `RectangleError::DecompressionFailed`.
  - Rectangle bounds and overflow protection: verified zero dimensions, excessive dimensions (> `MAX_RECTANGLE_DIMENSION`), and `u16` coordinate overflow edge cases.
  - Pseudorandom byte stream fuzzing: confirmed panic-free error handling across hundreds of pseudo-random byte inputs across all decoders.
- **Documentation & Compatibility:** Updated threat model (`docs/security/threat-model.md`), architecture specifications (`docs/ARCHITECTURE.md`), implementation guide (`docs/REMOTE_DESKTOP_IMPLEMENTATION.md`), and feature matrix (`docs/FEATURE_MATRIX.md`) with complete server compatibility notes and security policies.

### Remote Desktop UI Polish, Quick Connect & Workspace Persistence (Phase 5)
- **Host Editor Upgrade:** Added protocol-specific parameters for VNC (resolution presets 1080p/1440p/4K/720p, custom width/height inputs, color depth 24/32-bit, view-only mode) and RDP (Windows Domain input, resolution presets/custom, color depth, view-only mode, and Strict CA vs TOFU pinned SHA-256 certificate policy).
- **Interactive Quick Connect:** Extended Quick Connect to parse graphical targets (`vnc://[user@]host[:port]` and `rdp://[user@]host[:port]`). Prompts interactively on-canvas for credentials on demand without persisting secrets in URLs, command-line arguments, or unencrypted storage.
- **Toolbar Polish & Controls:** Added Fullscreen toggle (`window.toggle_fullscreen()`), Scaling mode switcher (`Fit`, `100%`, `Fill`), SAS key buttons (`Ctrl+Alt+Del`, `Win`, `Alt+Tab`), view-only toggle, and reconnect button.
- **Live Measured Telemetry Inspector:** Built collapsible HUD overlay displaying real-time FPS, round-trip latency, network bytes received/sent, decoded frames, rolling decode duration, geometry, and security policy directly from live protocol events.
- **Workspace Persistence:** Added `WorkspaceSessionKind::Vnc` and `WorkspaceSessionKind::Rdp` to `shellumi-core` and workspace layouts. Persists remote desktop sessions across app restarts and restores graphical desktop tabs on workspace restore.

### Embedded RDP Protocol Engine with IronRDP (Phase 4)
- Implemented `crates/shellumi-rdp`: Embedded native Microsoft Remote Desktop Protocol (RDP) client integration powered by pure-Rust `ironrdp-client` (v0.1.0) and associated subcrates (`ironrdp-pdu`, `ironrdp-connector`, `ironrdp-session`, `ironrdp-graphics`).
- Strict TLS Peer-Certificate Verification by Default: Implemented `ShellumiCertificateVerifier` enforcing `CertificateValidationPolicy::Strict` against system root trust stores, SHA-256 fingerprint computation and pinning for self-signed certificates, and TOFU (Trust-On-First-Use) verification with protection against mismatched fingerprints.
- NLA & CredSSP Authentication: Configured CredSSP and Network Level Authentication (NLA) with automatic logon and domain routing.
- Input & Scancode Translation: Implemented bidirectional mouse pointer movement and button mask translations (`PointerFlags::MOVE`, `LEFT_BUTTON`, `RIGHT_BUTTON`, `MIDDLE_BUTTON`), vertical wheel delta mapping with negative direction flags, X11 keysym to hardware scancode mapping with Unicode fallback, and SAS (`Ctrl+Alt+Del`, `Win`) FastPath PDU sequence generation.
- Decoded 32-bit RGBA Bitmaps: Converted incoming `RdpOutputEvent::Image` buffers into 32-bit RGBA `Framebuffer` updates with dirty rectangle damage tracking and frame ready notifications.
- Integrated RDP background session task into `shellumi-app` actions and wired toolbar SAS button.

### Native Remote Desktop Engine & VNC Advanced Decoders (Phase 0, 1, 2, 3)
- Implemented `crates/shellumi-remote-desktop`: 32-bit RGBA `Framebuffer` with row-major memory, dirty rectangle tracking, `MAX_FRAMEBUFFER_DIMENSION = 8192` bounds enforcement, bounded async channels (`SessionCommand` and `SessionEvent`), and true FPS/latency/byte counters.
- Built native GPUI canvas rendering component (`remote_desktop_canvas.rs`) converting thread-safe `FramebufferSnapshot` into `gpui::RenderImage` with aspect-ratio preserving `Fit`, `Original` (100%), and `Fill` (stretch) scaling modes.
- Implemented `crates/shellumi-vnc`: Native pure-Rust RFB client protocol engine supporting RFB 3.3, 3.7, and 3.8 version negotiation, standard FIPS 46-3 DES challenge-response VncAuth (RFC 6143 Section 7.2.2), and ServerInit screen geometry parsing.
- Added advanced rectangle decoders in `crates/shellumi-vnc/src/decoder/`:
  - `Hextile` (Encoding 5): 16x16 tile decoder handling Raw tiles, BackgroundSpecified, ForegroundSpecified, AnySubrects, and SubrectsColoured with bounds clamping.
  - `ZRLE` (Encoding 16): Persistent zlib decompressor across RFB updates, 64x64 tiles, Raw, Solid, Packed Palette (1/2/4 bpp), Plain RLE, Palette RLE, and strict decompression bomb limits.
  - `Tight` (Encoding 7): Pure-Rust JPEG decompression using `image` (no external `turbojpeg` binaries), Fill compression, and Basic zlib compression with 4 persistent streams, palette filter, and gradient prediction.
  - `Raw` (Encoding 0), `CopyRect` (Encoding 1), and `DesktopSize` (Encoding -223).
- Implemented bidirectional clipboard text synchronization (`ServerCutText` and `ClientCutText`) with `MAX_CLIPBOARD_BYTES` safety limits and loop suppression to eliminate echo loops.
- Replaced mock Ubuntu/Windows drawings and hardcoded 60 FPS/14 ms metrics with truthful protocol state indicators and real measured telemetry.

### Dual-Pane Graphical SFTP File Manager
- Implemented full 4-scenario SFTP file management: Local-to-Local, Local-to-Remote, Remote-to-Local, and Remote-to-Remote.
- Native macOS folder MIME icons (`Documents`, `Downloads`, `Desktop`, `Movies`, `Music`, `Pictures`, `Applications`, `Public`, and generic `folder`) integrated into both table file rows and macOS breadcrumb path bar.
- Interactive macOS breadcrumb path bar with history back/forward navigation (`<` and `>`), detailed columns (Name, Permissions, Date Modified, Size, Kind), and natural parent navigation (`..`).
- Recursive file and folder transfer engines (`sftp_local_copy_recursive`, `sftp_upload_recursive`, `sftp_download_recursive`) supporting arbitrary directory hierarchies without OS errors.
- Dedicated per-pane status bar (`h(px(26.0))`) displaying real-time selection counts and instant "Deselect" action.
- Empty space click-to-deselect with event stop propagation on row clicks to prevent selection conflicts.
- Isolated context menu overlay with transparent click-outside backdrop (`inset_0()`) and hover isolation preventing underlying row highlighting.

### Uniform Action Bar & Navigation Architecture
- Fixed top padding inconsistencies across all pages (`Hosts`, `Keychain`, `Port Forwarding`, `Snippets`, `Known Hosts`, `Session Logs`, `Settings`, and `Workspaces`).
- Every page now features a pinned 56px action bar flush with the window top divider (`border_b_1().border_color(rgb(MacTheme::BORDER))`) and uniform scrollable content area (`p_6()`).
- Eliminates vertical jitter and alignment jumps when navigating between sidebar items.

### Command Palette & Navigation Iconography Overhaul
- Upgraded Command Palette (`Cmd+K`) icons with high-resolution vector assets (`edit`, `invite member`, `Hosts`, `Infrastructure`, `Diagnostics`, `Terminal`, `Mobile`).
- Upgraded filtering, sorting, grid/list view mode icons and popover menus.
- Connected group settings inspector and reusable identity settings modal dialogs.
- Fixed identity selection precedence so host destination usernames (e.g. `root`) take precedence over local user defaults when selecting an existing SSH key.

## 0.0.6-alpha

### Modern Desktop Navigation & Group Details Inspector
- Added slide-over Group Details Inspector panel with parent group hierarchies, cloud sync toggles, and protocol defaults.
- Group card context menu with Quick Connect, Connect, Edit Group Details, Collaborate, Move To, Copy To, and Remove actions.
- Reusable Identity settings modal dialog for managing keys, passphrases, and usernames.
- Team member invite modal dialog supporting multi-recipient team collaboration invitations.
- Multi-protocol support foundations (Telnet, Serial, Mosh, Local, SFTP, Remote Desktop VNC/RDP).

## 0.0.4-alpha

### macOS Dark Gallery UI Overhaul
- Updated the entire desktop GUI design across all screens, modals, cards, badges, and controls to match the native macOS Dark theme and tokens from `gpui_macos_dark_gallery`.
- Replaced host list item chrome with rounded 30x30 navy glyph badges (`⌘`), secondary caption typography, and interactive hover states.
- Restructured host editor modal into a two-column layout with left category navigation rail (`General`, `Connection`, `Authentication`, `Proxy`, `Advanced`), local vault security card, and right-aligned actions.
- Styled terminal canvas with deep black contrast background (`0x0c0f13`), 1px subtle split divider borders (`0x46464a`), and protocol-specific tab glyphs (`⌘`, `⚡`, `⚠`, `🔌`, `📁`).
- Redesigned cards across Port Forwarding (status dots & `⇄` badge), Keychain (`⚿` blue badge), Snippets (line-numbered code block with comment highlights), Workspaces (deep blue badges), and Overview metrics.

### Keyboard Shortcuts Implementation & Action Architecture
- Created comprehensive keyboard shortcuts reference in `docs/SHORTCUTS.md` covering application, tab, pane, terminal, and navigation shortcuts across macOS, Linux, and Windows.
- Declared 37 discrete GPUI action structs with corresponding platform keybindings (`cmd-...` on macOS, `ctrl-...` on Linux/Windows).
- Mounted top-level focus tracking (`.track_focus(&self.focus)`) and centralized `.on_action(...)` handlers on `shellumi-root` to ensure shortcuts function consistently across all desktop pages, tabs, and modals.
- Implemented dual-layer keyboard shortcut interceptor (`handle_app_shortcut`) in `terminal_input` to prevent active PTY streams from swallowing application hotkeys.
- Added sidebar visibility toggle (`Cmd+\` / `Ctrl+\`) across all workspace view modes.
- Added automated test suite `verifies_keyboard_shortcut_actions_and_keystroke_parsing`.

## 0.0.3-alpha

### macOS HIG Dark Theme & Visual Design Overhaul
- Centralized macOS Human Interface Guidelines (HIG) dark mode tokens in `shellumi-app::theme` (`MacTheme`).
- Harmonized all application UI surfaces: window canvas (`0x1c1c1e`), workspace content (`0x18181a`), sidebar (`0x252528`), surfaces/toolbars (`0x2c2c2e`), borders (`0x46464a`), and inputs (`0x303034`).
- Updated command palette (`Cmd+K`), tab bar chrome, terminal active canvas, and split-pane dividers.
- Harmonized all overlay modals: dual-pane SFTP transfer drawer, port forwarding manager, session replay player, shell integration inspector, mobile simulator, multi-host batch runner, team vaults, and system diagnostics.
- Refactored button interaction hierarchy to eliminate duplicate GPUI hover style registrations.

### Brand & Multi-Platform HIG App Icons
- Redesigned and replaced application logo and platform icons following Apple macOS Human Interface Guidelines (squircle geometry with continuous curvature, subtle lighting, border, drop shadow).
- Generated complete platform icon assets: macOS `.icns`, Windows `.ico`, Linux PNG icon themes (`16x16` up to `1024x1024`), Favicon, and Web Touch icons.
- Updated documentation and README assets.

### System Keychain Daemons & Benchmark SLA Hardening
- Integrated native OS credential daemons status detection for macOS `securityd`, Linux Secret Service (`secret-tool`), and Windows Credential Manager (`PasswordVault`).
- Calibrated PBKDF2-HMAC-SHA256 benchmark SLA criteria to accommodate unoptimized crypto iterations during debug test runs (`cfg!(debug_assertions)`).
- Enforced strict `.gitignore` patterns for local GUI screenshot galleries and attachment caches.

## 0.0.2-alpha

### Post-tag packaging repair

- Verified and published the unchanged `v0.0.2-alpha` tag using the repaired
  workflow: all four native build/package jobs passed, with 15 assets and
  validated checksums/source provenance.

- Repair Debian prerelease version conversion, validate package inputs before
  output cleanup, require both binaries, and generate root-owned Debian archives.
- Add three actual-helper packaging regression tests (321 workspace tests on main).
- Support manual release retries from unchanged source tags with corrected
  packaging tools and explicit source/helper provenance.

### Application changes

- Interrupt blocked SSH pipe reads/writes on cancellation and enforce one bounded
  deadline per complete packet (30 seconds by default), including SFTP handshake.
- Terminate the SSH child after pipe failure/interruption; keep cleanup warnings
  with remote staging paths when a connection cannot be reused safely.
- Add blocked/trickled-pipe and nonresponsive SSH connection acceptance tests.
- Align crate, FFI/diagnostic and package versions; require release tags to match crate versions.
- Patch the existing xattr 0.2.3 Linux errno compatibility failure while retaining
  upstream licenses and native behavior; native Linux app build remains for CI.


- Reject unsafe listing filenames before joining GUI transfer destinations.
- Align live SFTP reads with the codec’s 16 MiB frame limit before allocation.
- Stage SFTP downloads/uploads privately and publish only after validated transfer
  completion; preserve existing destinations on pre-publication failure/cancel.
- Add graphical queued/running Cancel actions, live byte progress, real worker
  limits and stable tab/listing updates. Cancellation interrupts SSH pipe waits; failed
  remote cleanup retains a visible temporary-file path.
- Prevent byte progress and CLI queue previews from claiming completed transfers.
- Extend OpenSSH acceptance with destination preservation and cancellation in
  both directions; cover EOF/close failures, symlink safety and commit races.

- Add self-contained macOS OpenSSH/SFTP acceptance covering saved group identity
  resolution, encrypted-key askpass, listing, binary/empty transfers, reconnect,
  wrong-passphrase and changed/unknown server-key rejection.
- Require a matching successful final SFTP CLOSE response before reporting upload,
  download or directory-listing success; propagate close errors and disconnects.
- Add optional explicit OpenSSH client configuration for isolated SFTP acceptance.

- Add host/group GUI identity selection with preserved bindings and refresh.
- Apply saved identities to SSH, graphical/terminal SFTP, workspace restores and
  tunnels using a shared background resolver and validated launch policy.
- Support password/key-passphrase references through a separate OpenSSH askpass
  helper; require existing host trust, reject MFA/trust prompts, and keep secrets
  out of command arguments. Key/certificate paths support spaces and quotes.

- Reconcile progress documents with the actual SFTP, identity, keychain and Drive
  integration status; record missing live acceptance.
- Remove shared credential-fallback and Drive vault encryption passphrases;
  require explicit keys and standard Drive KDF settings.
- Disable SFTP agent forwarding and require previously trusted host keys.
- Pipe HTTP headers and UTF-8 bodies to curl through stdin rather than argv;
  reject malformed header input and ignore user curl configuration.
- Regression coverage: 318 workspace tests passed on macOS, three opt-in tests ignored;
  both self-contained SSH/SFTP acceptance tests passed separately. Native xattr
  verification passed 4 integration tests and 1 doc test.

## 0.0.1-alpha

First alpha milestone for Shellumi's local-first Rust/GPUI desktop.

### Added and connected

- Automatic platform data paths and database initialization.
- Navy/teal desktop navigation, host inventory and inspector, and host editing.
- Functional snippet/workspace editors, saved settings, host import/export and
  group defaults connected to local storage.
- Real SSH/local PTY sessions, measured terminal sizing, scrolling and split panes.
- Public SSH-key and trust-store tools, real tunnel sessions and local file browsing.
- Masked encrypted-backup export/restore with transactional error rollback.
- README app screenshots, current feature status and engineering roadmap.

### Corrected

- Preserve terminal text attributes across separate input chunks.
- Stop automatic dispatch of startup commands before authentication readiness is known.
- Remove simulated remote SFTP listings, transfer completion and tunnel traffic.
- Report unfinished cloud synchronization and network collaboration explicitly.
- Update release packaging for prerelease versions and exclude staging directories
  from uploaded assets and checksum generation.

### Verification and limits

The implementation passed 270 workspace tests, with one additional opt-in SSH
server test exercised separately. Formatting, Clippy and a local Intel macOS
release build passed. GUI checks covered local editing, shell launch, preferences,
long output, scrolling and split sizing.

This alpha is unsigned, not production-ready and not validated on every target
platform. Graphical remote SFTP, Drive transport, authenticated cloud discovery
and network collaboration remain unfinished. See `docs/GUI_STATUS.md`.

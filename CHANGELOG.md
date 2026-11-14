# Changelog

## 0.0.7-alpha

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

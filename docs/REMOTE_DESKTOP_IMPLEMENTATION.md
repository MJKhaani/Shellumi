# Shellumi — Production RDP & VNC Implementation Specification

**Status:** Active Implementation Specification & Architecture Reference  
**Last Updated:** 2026-11-14  
**Primary References:**
- **Shellumi Repository:** [https://github.com/MJKhaani/Shellumi](https://github.com/MJKhaani/Shellumi)
- **VNC Encoding Library:** [`rustvnc/rfb-encodings`](https://github.com/rustvnc/rfb-encodings) (Version 0.1.7, Apache-2.0, MSRV 1.90)
- **RDP Engine:** [`Devolutions/IronRDP`](https://github.com/Devolutions/IronRDP) (`ironrdp-client` 0.17.0, Rust toolchain 1.94.1)
- **RFB Standard:** RFC 6143 and vendor extensions
- **RDP Specifications:** Microsoft Open Specifications, MS-RDPBCGR, MS-RDPEDISP, MS-RDPECLIP, and related protocols

---

## 1. Mission and High-Level Goals

Replace Shellumi's prototype and simulated remote desktop behaviors with fully functional, independently packaged, native Rust RDP and VNC clients rendered inside its existing GPUI desktop application.

### Key Objectives
1. **Executable Independence:** Neither protocol may invoke or require locally installed RDP/VNC viewer binaries (such as `mstsc`, `xfreerdp`, `wlfreerdp`, `rdesktop`, `vncviewer`, `open`, `xdg-open`, or OS URI scheme handlers) or a separate background daemon. All connection, handshake, security negotiation, decompression, and framebuffer rendering logic runs embedded within Shellumi.
2. **Truthful Telemetry & State:** All visible session states (latency, FPS, resolution, color depth, clipboard status, network traffic) must derive strictly from real measured and observed protocol events. No simulated `Connected` state, hardcoded `60 FPS` / `14 ms`, or placeholder desktop frames are permitted.
3. **Native GPUI Rendering:** Framebuffers must render efficiently inside the GPUI window hierarchy with dirty rectangle updates, HiDPI scaling, and responsive mouse/keyboard event dispatch.
4. **Preserve Security & Vault Boundaries:** Credential references must resolve through the Shellumi Keychain without exposing plaintext passwords in SQLite databases, logs, process arguments, or crash dumps. Strict TLS certificate verification must be enforced by default for RDP; clear warnings must be presented for unencrypted legacy VNC connections.

---

## 2. Verified Upstream Facts and Technical Constraints

| Component | Upstream Fact / Verified Constraint | Implementation Impact |
|---|---|---|
| **`rfb-encodings` (VNC)** | Version 0.1.7, Apache-2.0, Rust 2021, MSRV 1.90. The public production `Encoding` trait exposes `encode(...)`, **not** a general-purpose streaming `decode(...)` interface. The repository's `tests/decoders.rs` contains minimal test decoders only. Default features are empty. Optional `turbojpeg` requires an external native library. | We must develop or integrate a production client-side decoder module (`shellumi-vnc::decoder`) using `rfb-encodings`' public pixel format and compression utilities. Do not copy incomplete test decoders. Keep `turbojpeg` disabled unless packaged and audited. |
| **`IronRDP` (RDP)** | `ironrdp-client` library (v0.17.0) exposes reusable `RdpClient`, `ConfigBuilder`, output events, and session driver independent of its GUI viewer. Toolchain requires Rust >= 1.94.1. Documentation explicitly notes that default TLS peer-certificate validation is non-strict. | Must explicitly configure `CertificateValidation::Strict` and implement a dedicated fingerprint/pinning exception workflow. Evaluate `ironrdp-client` with `rustls` and `client-clipboard` features for a lean build graph. |
| **Existing Shellumi Codebase** | Currently has `ConnectionProtocol::{Vnc,Rdp}`, `VncConfig`, `RdpConfig`, initial wire helpers in `crates/shellumi-core`, and UI session layout in `crates/shellumi-app`. In `crates/shellumi-app/src/actions/remote_desktop.rs`, `launch_remote_desktop` currently simulates connection and metrics without transmitting network packets. | Must replace mock state machines with real Tokio worker tasks communicating over bounded crossbeam/tokio mpsc channels. |
| **GPUI Environment** | Shellumi is built on Rust 2021 and GPUI 0.2.2. SSH/SFTP/Terminal workflows must remain untouched. | Remote desktop tabs must behave like first-class GPUI tabs, managing texture memory safely without blocking the UI thread or leaking Tokio tasks on tab closure. |

---

## 3. Current vs. Target Feature Matrix

| Capability | Current State | Target State (Production Plan) | Target Phase |
|---|---|---|---|
| **VNC Host Configuration** | Working (host, port, credentials in `VncConfig`) | Working (retained, with encoding and view-only options) | Phase 4 |
| **VNC Wire Negotiation** | Partial (packet helpers in `shellumi-core`) | Full RFC 6143 version handshake (3.3/3.7/3.8), security negotiation | Phase 2 |
| **VNC Authentication** | Unimplemented (simulated in UI) | VNCAuth (DES challenge-response), None (opt-in with warning), VeNCrypt/TLS | Phase 2 |
| **VNC Decoders** | Unimplemented | Raw, CopyRect, DesktopSize, Hextile, ZRLE, Tight | Phase 2, 3 |
| **VNC Live Interaction** | Simulated (local-only clicks/keys) | Real X11 keysyms, modifier tracking, pointer masks, wheel events | Phase 2 |
| **RDP Host Configuration** | Working (host, port, credentials in `RdpConfig`) | Working (domain, screen mode, cert policy, NLA toggles) | Phase 4 |
| **RDP Wire Negotiation** | Partial (TPKT / X.224 negotiation request helpers) | Complete MS-RDPBCGR connection sequence via `ironrdp-client` | Phase 3 |
| **RDP Authentication** | Unimplemented (simulated in UI) | CredSSP / NLA with Kerberos/NTLM, strict TLS validation | Phase 3 |
| **RDP Bitmap Pipeline** | Unimplemented (mock canvas) | Decoded 32-bit RGBA bitmaps, dirty rectangle updates | Phase 3 |
| **RDP Live Interaction** | Simulated (local-only keys/clicks) | Scancode / Unicode mapping, mouse events, SAS (Ctrl+Alt+Del) | Phase 3 |
| **GPUI Framebuffer Rendering**| Simulated (mock gradient drawing) | Texture/Image-backed dirty region renderer with aspect scaling | Phase 1 |
| **Clipboard Sync** | Simulated (local text flag) | RFC / CLIPRDR text clipboard with size caps and loop prevention | Phase 2, 3 |
| **Telemetry & Metrics** | Simulated (hardcoded 60 FPS, 14 ms) | Real measured frame rate, decode time, bytes received, round-trip ping | Phase 1 |

---

## 4. System Architecture and Component Structure

```text
crates/
  shellumi-remote-desktop/        # Shared session abstractions, commands, events, framebuffer
    src/
      lib.rs
      session.rs                  # SessionState, RemoteProtocol, lifecycle state machine
      framebuffer.rs              # 32-bit RGBA buffer, dirty rect accumulation, snapshotting
      input.rs                    # Mouse pointer, wheel, keyboard modifiers, X11/scancode mapping
      events.rs                   # SessionCommand (UI -> Task) and SessionEvent (Task -> UI)
      security.rs                 # Certificate verification policy, TLS pinning, VNC auth warnings
      metrics.rs                  # Measured FPS, throughput, decode latency, dropped frames
  shellumi-vnc/                   # RFB transport, negotiation, security, and decoding
    src/
      lib.rs
      client.rs                   # Async VNC connection loop and event driver
      handshake.rs                # RFB 3.3/3.7/3.8 and security type negotiation
      security.rs                 # VNCAuth DES challenge-response, TLS/VeNCrypt
      wire.rs                     # FramebufferUpdateRequest, SetPixelFormat, SetEncodings
      decoder.rs                  # Production client decoders using rfb-encodings utilities
      rectangles.rs               # Raw, CopyRect, Hextile, ZRLE, Tight rectangle processing
      clipboard.rs                # ClientCutText / ServerCutText bidirectional text sync
  shellumi-rdp/                   # Embedded IronRDP client integration
    src/
      lib.rs
      client.rs                   # Async RdpClient event loop driver
      config.rs                   # RdpConfig -> IronRDP ConfigBuilder mapper
      events.rs                   # RdpOutputEvent to SessionEvent translator
      keyboard.rs                 # Host keystrokes to RDP scancodes / Unicode events
      clipboard.rs                # CLIPRDR virtual channel text clipboard integration
  shellumi-app/
    src/
      actions/remote_desktop.rs   # Session lifecycle dispatch (connect, disconnect, commands)
      ui/remote_desktop_session.rs# GPUI tab container, real metrics toolbar, status banner
      ui/remote_desktop_canvas.rs # Native GPUI framebuffer renderer with pointer mapping
```

### Dataflow Diagram

```text
SavedHost + Keychain Identity
             │
             ▼
Shellumi GPUI Remote Desktop Tab
             │  (SessionCommand channel: Pointer, Key, Resize, Disconnect)
             ▼
   Tokio Background Task
   ┌───────────────────────────────────────────────┐
   │ Protocol Client Engine                        │
   │  ├── VNC: RFB stream + rfb-encodings decoder  │
   │  └── RDP: ironrdp-client + NLA / TLS engine   │
   └───────────────────────────────────────────────┘
             │  (SessionEvent channel: Status, FrameUpdate, RemoteSize, Error)
             ▼
   GPUI Framebuffer Renderer
   ┌───────────────────────────────────────────────┐
   │ - Thread-safe 32-bit RGBA buffer              │
   │ - Dirty rectangle blitting                    │
   │ - Aspect-preserving scaling (Fit / 100% / Fill)│
   │ - Measured FPS & real decode latency metrics  │
   └───────────────────────────────────────────────┘
```

---

## 5. Phased Implementation Roadmap

### Phase 0 — Audit, Design, and Truthful Status (PR 1)
- [ ] Inspect existing protocol types, remote desktop actions, and UI canvas components.
- [ ] Remove fake `Connected` state, hardcoded `60 FPS` / `14 ms`, artificial desktop drawing, and simulated clipboard success messages.
- [ ] Replace with honest `Not connected` / `Connecting...` / `Feature in development` UI states.
- [ ] Pin exact dependencies in `Cargo.lock` (`rfb-encodings = "0.1.7"`, `ironrdp-client = "0.17.0"`).
- [ ] Verify toolchain requirements (Rust 1.94.1 compatibility across the workspace).
- **Gate:** Workspace builds with zero warnings; status tests prove no impossible `Connected` states.

### Phase 1 — Real Session Manager and Framebuffer Architecture (PR 2)
- [ ] Create `crates/shellumi-remote-desktop` domain crate.
- [ ] Implement `Framebuffer` with safe 32-bit RGBA storage, dimension bounds, and dirty rectangle tracking.
- [ ] Implement `SessionCommand` and `SessionEvent` bounded mpsc channel plumbing.
- [ ] Build concrete GPUI canvas rendering directly from the shared framebuffer buffer (handling HiDPI, letterboxing, and coordinate transformations).
- [ ] Implement true telemetry collectors: measured render FPS, received byte counters, and decode timing.
- **Gate:** Deterministic test frames render through the exact same GPUI canvas pipeline as network frames; coordinate mapping tests pass for all scaling modes.

### Phase 2 — Native VNC Client Engine (PR 3)
- [ ] Implement async TCP stream with deadlines, DNS resolution, and interruptible I/O.
- [ ] Implement RFB 3.3/3.7/3.8 handshake, client/server initialization, and desktop name/size parsing.
- [ ] Implement VNCAuth (DES challenge-response) with password resolution from Keychain references.
- [ ] Implement `SetPixelFormat`, `SetEncodings`, and incremental `FramebufferUpdateRequest` scheduling.
- [ ] Implement `Raw`, `CopyRect`, and `DesktopSize` rectangle processing into the framebuffer.
- [ ] Build live pointer and keyboard transmission using X11 keysyms and button masks.
- **Gate:** Authenticates with real TigerVNC/TightVNC server, renders live remote screen updates, and transmits bidirectional input.

### Phase 3 — VNC Advanced Decoders and Clipboard (PR 4)
- [ ] Implement production client decoders in `shellumi-vnc::decoder` utilizing `rfb-encodings`:
  - Hextile tile decoding.
  - ZRLE with persistent zlib decompression state.
  - Tight encoding (without requiring external native `turbojpeg` binaries).
- [ ] Enforce decompression bomb limits, maximum rectangle bounds, and memory quotas.
- [ ] Implement bidirectional `ClientCutText` / `ServerCutText` with loop suppression and size caps.
- **Gate:** Verified interoperability against independent VNC server implementations across multiple encodings.

### Phase 4 — Embedded RDP Client with IronRDP (PR 5)
- [ ] Create `crates/shellumi-rdp` wrapping `ironrdp-client`.
- [ ] Map `RdpConfig` and Keychain credentials into `ironrdp-client::ConfigBuilder`.
- [ ] Enforce `CertificateValidation::Strict` by default; implement interactive certificate inspection and fingerprint pinning for self-signed certificates.
- [ ] Wire CredSSP / NLA authentication.
- [ ] Translate `ironrdp-client` output events (`Connected`, `DesktopUpdate`, `Image`, `ConnectionFailure`) into `SessionEvent`s.
- [ ] Map host keyboard keystrokes to RDP scancodes and mouse inputs to pointer messages.
- [ ] Implement CLIPRDR text clipboard virtual channel.
- **Gate:** Authenticates against live Windows 10/11 Pro and Windows Server machines with NLA enabled; renders live desktop frames and handles input without any external viewer executable.

### Phase 5 — UI Polish, Persistence, and Workspace Integration (PR 6)
- [ ] Upgrade host editor with protocol-specific parameters (resolution, cert policy, encoding preferences, view-only mode).
- [ ] Wire interactive Quick Connect password prompt without persisting secrets in URLs or shell arguments.
- [ ] Wire toolbar actions: Reconnect, Fullscreen, Scaling Mode (Fit/100%/Fill), SAS key buttons (Ctrl+Alt+Del, Win, Alt+Tab), view-only toggle.
- [ ] Workspace persistence: save non-secret preferences and reconnect on restore.
- **Gate:** Full UI acceptance test covering tab navigation, workspace save/restore, and credential isolation.

### Phase 6 — Security Hardening, Fuzzing, and Release (PR 7)
- [ ] Implement fuzz targets for RFB rectangle parsers and decompression bounds.
- [ ] Zeroize sensitive credentials in memory using `zeroize`.
- [ ] Validate clean VM execution on Windows, macOS, and Linux without external viewer packages.
- [ ] Document all server compatibility notes, known limitations, and security policies.
- **Gate:** All workspace tests pass, clippy passes with `-D warnings`, zero native viewer prerequisites.

---

## 6. Real-Server Acceptance Test Matrix

| Protocol | Server Target | OS Environment | Required Test Scenarios |
|---|---|---|---|
| **VNC** | TigerVNC | Linux (Ubuntu / Fedora) | RFB 3.8, VNCAuth, Raw/Hextile/ZRLE decoders, mouse/keyboard, dynamic resize, clipboard |
| **VNC** | TightVNC / UltraVNC | Windows 10 / 11 | Legacy auth warning, 16/24/32-bit pixel formats, encoding fallback |
| **VNC** | macOS Screen Sharing | macOS 14 / 15 | Compatible auth modes, graceful failure reporting on unsupported vendor auth |
| **RDP** | Windows 10/11 Pro | Windows | NLA, strict TLS cert trust, live desktop updates, scancode keyboard input, reconnect |
| **RDP** | Windows Server | Windows Server 2022 | NLA, lock/unlock screen, dynamic resize, multi-session isolation |
| **RDP** | xrdp | Linux | Compatibility negotiation, X11 session desktop updates, clipboard exchange |

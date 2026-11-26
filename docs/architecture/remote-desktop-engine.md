# Remote Desktop Engine (RDP & VNC)

[Docs Home](../README.md) / [Architecture](README.md) / Remote Desktop Engine

---

Shellumi's remote desktop subsystem is composed of three UI-independent protocol crates (`shellumi-remote-desktop`, `shellumi-vnc`, `shellumi-rdp`) driven by `shellumi-app` (`actions/remote_desktop.rs`, `ui/remote_desktop_canvas.rs`, `ui/remote_desktop_session.rs`).

## Protocol Backends & Worker Lifecycle

```mermaid
flowchart LR
    UI["GPUI Canvas & Session Toolbar"] -->|SessionCommand (async_channel)| Worker["Tokio Protocol Worker (RDP / VNC)"]
    Worker -->|SessionEvent (RemoteResize, FrameReady, ResizeFailed, DisplayControlCaps)| UI
    Worker -->|32-bit RGBA FramebufferSnapshot| Canvas["GPUI RenderImage Blit"]
```

- **`shellumi-remote-desktop`:** Defines `SessionCommand`, `SessionEvent`, `FramebufferSnapshot`, `PanState`, `ReconnectPolicy`, and `SessionMetricsSnapshot`.
- **`shellumi-vnc`:** Pure-Rust RFB 3.3/3.7/3.8 client with VncAuth DES challenge-response, Raw/CopyRect/DesktopSize/Hextile/ZRLE/Tight decoders, decompression-bomb bounds, and clipboard sync.
- **`shellumi-rdp`:** Embedded `IronRDP` client with strict TLS/SHA-256 TOFU pinning, CredSSP/NLA, `MS-RDPEDISP` dynamic display control, `CLIPRDR` clipboard/file streaming, `RDPSND` (`cpal`) audio output, `RDPDR` local drive redirection, and `MS-RDPEAI` (`cpal`) microphone input.

---

## Measured Canvas Geometry & Input Mapping

- **Canvas Sensor:** `ui/remote_desktop_canvas.rs` wraps the canvas in a `gpui::canvas` bounds sensor tagged with `app.layout_epoch`, storing `CanvasMeasurement { tab_id, layout_epoch, viewport }` only when bounds change.
- **Coordinate Mapping (`CanvasTransform`):** Subtracts measured canvas origin, excludes letterbox/pillarbox margins in `Fit` mode, applies `PanState` offsets in `100% Actual Size` mode, and scales into remote framebuffer coordinates.

---

## Single Resize Coordinator, Generations & Limitations

`RdAutoResizeState` (`crates/shellumi-app/src/main.rs`) coordinates all `ResizeOrigin::{Automatic, Manual}` requests per `RemoteSessionKey { tab_id, connection_generation }`:

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Debouncing: Automatic layout change (captures layout_gen + auto_cancel_gen)
    Debouncing --> InFlight: take_ready_to_dispatch() [layout_gen == layout_epoch]
    Idle --> InFlight: Manual Match Viewport / Preset
    InFlight --> Idle: SessionEvent::RemoteResize confirms dimensions
    InFlight --> Idle: SessionEvent::ResizeFailed / DisplayControlUnavailable / Timeout
```

1. **Single In-Flight Negotiation:** At most one `PendingResizeRequest` may be `in_flight` at a time; manual requests arriving mid-flight queue in `desired` and never overwrite or run concurrently.
2. **Layout-Generation Validation:** `Shellumi::invalidate_layout()` advances `layout_epoch`, clears `rd_canvas_measurements`, bumps `debounce_generation`, drops obsolete automatic requests, and flags `pending_manual_recalc` for manual Match Viewport recalculation once fresh geometry arrives. Every debounce, promotion, dispatch, retry, and timeout validates `req.layout_generation == self.layout_epoch`.
3. **Automatic Cancellation Tokens:** Every automatic `PendingResizeRequest` captures `auto_cancel_generation`. Disabling automatic resizing increments `auto_cancel_generation` so canceled requests cannot retry or reactivate even if dynamic resizing is re-enabled before an old timeout or ack arrives.
4. **Protocol Confirmation & Limitations:**
   - `MS-RDPEDISP` / `ironrdp_displaycontrol` does **not** expose per-request resize rejection PDUs or request-ID correlation.
   - `shellumi-rdp` emits `SessionEvent::ResizeFailed` when worker submission (`input_sender.send`) fails, and `apply_rd_session_event` treats only `SessionEvent::RemoteResize` (never arbitrary `FrameReady` ticks) as confirmation.
   - Because the server does not return a request ID on `RemoteResize`, a timed-out request cannot be proven canceled remotely; recovery uses bounded retries (max 2) and conservative dimension matching.
   - Live end-to-end resize acceptance against an external Windows RDP server remains `NOT TESTED`.

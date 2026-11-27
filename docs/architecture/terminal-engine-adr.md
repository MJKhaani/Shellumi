# <img src="../../resources/icon_pack/icons/48/regular/terminal.png" width="26" height="26" alt="Terminal Engine ADR"> ADR: Terminal Emulator Engine Selection (`TERM-ARCH-01`)

[Docs Home](../README.md) / [Architecture](README.md) / Terminal Engine ADR

- **Status:** Accepted (2026-11-27)
- **Requirement:** `TERM-ARCH-01`
- **Scope:** `crates/shellumi-terminal`, `crates/shellumi-app`, `crates/shellumi-cli`

---

## 1. Context

Shellumi requires a cross-platform (macOS, Windows, Linux), MIT-compatible, GPUI-integrated terminal emulator engine supporting full VT100/VT220/xterm-style semantics, Unicode/combining/wide-character grids, scrollback reflow, typed terminal events (bell, title, cwd, OSC 133 prompt markers, clipboard/protocol replies), and deterministic session recording/replay.

We evaluated two options:
1. **Option A — Adopt `alacritty_terminal`:** Replace `shellumi-terminal`'s parser/grid with `alacritty_terminal` (Apache-2.0).
2. **Option B — Retain & Harden `shellumi-terminal`:** Keep `shellumi-terminal`'s `vte`-based parser and `portable-pty` transport, and upgrade its grid, cell model, VT state machine, and typed-event architecture to full compatibility.

---

## 2. Evaluation

| Criterion | Option A (`alacritty_terminal`) | Option B (`shellumi-terminal` + `vte`) |
| --- | --- | --- |
| **Licensing** | Apache-2.0 (compatible with MIT) | MIT (`shellumi-terminal`), MIT/Apache-2.0 (`vte 0.11.1`, `portable-pty 0.8.1`, `unicode-width`, `unicode-segmentation`) |
| **Offline / Locked Build** | Not present in `Cargo.lock` or local Cargo cache; introduces a large transitive graph (`parking_lot`, `polling`, `rustix`, `signal-hook`, `vte` 0.13+, `windows-sys`) | All dependencies (`vte 0.11.1`, `grid 0.18.0`, `portable-pty 0.8.1`, `unicode-width`, `unicode-segmentation`, `regex`) are already pinned in `Cargo.lock` and compile `--locked --offline` |
| **GPUI Integration** | Requires bridging `alacritty_terminal::Term<EventProxy>` + `FairMutex` + custom `EventLoop` to GPUI's `async_channel` and `BackgroundExecutor`, duplicating `portable-pty` | Direct ownership inside GPUI/session controller with zero lock contention on the UI thread, dirty-row tracking, and native `StyledSpan`/measured-cell rendering |
| **Recording & Replay** | External to `Term`; requires wrapping PTY I/O and replaying into a separate `Term` | Integrated `SessionRecorder` + isolated replay `Terminal` instance with identical parser/grid semantics |
| **Security & Sandboxing** | Must disable/override default OSC 52 / title / response hooks | Explicit typed `TerminalEvent` queue; parser callbacks cannot block UI, mutate clipboard, or execute local commands; bounded OSC/response payloads |

---

## 3. Decision

**Select Option B: Retain and harden `shellumi-terminal`** on `vte 0.11.1` and `portable-pty 0.8.1`, using `unicode-width` and `unicode-segmentation` (already in `Cargo.lock`) for Unicode cell widths and grapheme boundaries, and `regex` (already in `Cargo.lock`) for bounded terminal search.

### Why Not `alacritty_terminal`
- `alacritty_terminal` is not in the pinned `Cargo.lock` / offline cache, and adopting its `EventLoop`/`Pty` model would conflict with Shellumi's `portable-pty` + OpenSSH/Mosh/Telnet/Serial transport abstraction and GPUI's single-owner entity model.
- Retaining `shellumi-terminal` preserves public APIs (`Terminal`, `TerminalGrid`, `PtySession`, `SessionRecorder`, `ShellIntegrationState`) while allowing us to enforce strict security boundaries (no direct parser-to-OS side effects) and typed event routing by stable session/pane ID and generation.

---

## 4. Architecture, Performance, Security & Test Strategy

1. **Typed Events & Controller (`TERM-ARCH-02`):**
   - `Terminal` emits bounded `TerminalEvent` values (`Bell`, `TitleChanged`, `CwdChanged`, `PromptMark`, `ClipboardWriteRequest`, `HyperlinkHovered`, `ProtocolReply(Vec<u8>)`, `ModeChanged`) instead of mutating OS state from parser callbacks.
   - `ProtocolReply` bytes are routed back to the **originating** session generation only, never the currently focused pane.
2. **VT & Unicode Compatibility (`TERM-VT-01..03`):**
   - Full C0/C1, DECSET/DECRST (including 1/7/25/47/1000/1002/1003/1004/1006/1047/1048/1049/2004), scroll margins (`DECSTBM`), tab stops, delayed autowrap, `DECOM`, `ICH`/`DCH`/`IL`/`DL`, `SU`/`SD`, `DA`/`DSR`/`CPR`, `RIS`/`DECSTR`.
   - Cell model supports width-0 combining marks, width-2 wide characters + continuation cells, resize reflow on primary screen, and separate alternate screen.
   - Honest `TERM` capability (`xterm-256color` only for verified sequences, documented in compatibility matrix).
3. **Performance (`TERM-UI-05`, `TERM-PREF-02`):**
   - Dirty-row bitset/generation tracking in `TerminalGrid`.
   - Viewport-virtualized GPUI rendering (visible rows + bounded overscan only; zero scrollback DOM construction).
   - Bounded I/O queues with backpressure and frame-coalesced UI notifications.
4. **Verification:**
   - Table-driven VT fixtures chunked across 1-byte and random boundaries.
   - `vttest`-equivalent compatibility fixtures, Unicode/reflow snapshot tests, and malformed-stream fuzzing.
   - Real local PTY/ConPTY, isolated `sshd`, and native GPUI event dispatch/render tests.

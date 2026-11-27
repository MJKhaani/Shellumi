# Shellumi Terminal — Acceptance & Verification Report

**Base Commit:** `5f63145` (audit baseline `5d950074beaf00133154649f002795fbd39ec6dd`)  
**Host Platform:** macOS (`Darwin 23.6.0 x86_64`), `rustc 1.99.1` / `cargo 1.99.1`  
**Architecture Decision Record:** [Terminal Engine ADR](../architecture/terminal-engine-adr.md)

---

## 1. Summary of Implemented Changes

1. **`TERM-ARCH-01` & `TERM-ARCH-02` (Engine & Session Controller):**
   - Documented ADR selecting option B: retaining and hardening `shellumi-terminal` (`vte 0.11.1`, `portable-pty 0.8.1`, `unicode-width 0.2`, `regex 1`, `base64 0.22`) for `--locked --offline` compatibility, direct GPUI ownership, and zero parser-to-OS side effects.
   - Added `events.rs` with `TerminalEvent` (`Bell`, `TitleChanged`, `CwdChanged`, `PromptMark`, `ClipboardWriteRequest`, `ProtocolReply`, `ModeChanged`) and `TerminalModes`.
   - Added `session_uuid`, `session_generation`, `vault_id`, `host_id`, `focus_handle`, `bell_flash_until`, `has_unread_bell`, and `selection` to `TerminalPane`; `bind_pane_events` rejects stale `session_generation` events and routes `ProtocolReply` strictly back to the originating `pane.session`.

2. **`TERM-REC-01..04` (P0 Recording Integrity, Privacy & Persistence):**
   - Replaced fire-and-forget `actions/recordings.rs` with UUIDv4 IDs (`uuid::Uuid::new_v4()`), RFC-3339 UTC timestamps (`format_rfc3339_utc`), fail-closed `SessionRedactor::redact_asciicast_v2` (no `unwrap_or(raw_cast)`), encrypted `RecordingRecoveryJournal` checkpoints (`VaultCipher` AES-256-GCM), awaited DB commit with `"Saved"` shown only after commit, and `FailedRecoverable` state with UI Retry/Discard actions.
   - Centralized `finalize_pane_recording` / `finalize_all_active_recordings` across `SessionEvent::Closed`/`Error`, `close_pane`, `close_tab`, `handle_window_close_request`, `on_app_quit`, `lock_vault` (before key eviction), `switch_vault`, and `reconnect_active_session`.
   - Added `Utf8StreamDecoder` for split multibyte UTF-8 sequences, pause/resume elapsed-time tracking, opt-in input recording (`false` by default) with sensitive suppression, and pre-insertion `RecordingLimits` (bytes/events/duration, including resize events) with explicit `truncated` metadata.
   - Fixed `HostStore::save_session_recording` in `crates/shellumi-storage/src/lib.rs` to reject cross-vault ID collisions.

3. **`TERM-VT-01..03` (Emulator Compatibility & Escape Security):**
   - Upgraded `cell.rs`, `grid.rs`, and `parser.rs` with `CellFlags` (`wide`, `wide_spacer`), combining-mark storage, hidden-text masking in `cells_to_spans_with_combining`, delayed autowrap, `DECSTBM` scroll margins, `DECOM`, `ICH`/`DCH`/`IL`/`DL`/`ECH`/`IND`/`RI`/`HTS`/`TBC`, `DA1`/`DA2`/`DSR`/`CPR` replies, multi-param `DECSET`/`DECRST`, `DECSCUSR`, primary-screen resize reflow, separate alternate screen, and `OSC 8` safe-scheme allowlist + `OSC 52` read-deny / write-request policy.

4. **`TERM-UI-01..05` (GPUI Input, Selection, Mouse & Virtualized Renderer):**
   - Fixed `handle_app_shortcut` in `actions/shortcuts.rs` so macOS uses `Cmd` (`platform && !control`) and Windows/Linux terminal focus reserves `Ctrl+Shift` for app shortcuts, preserving raw `Ctrl+A..Z` (`Ctrl+C/D/L/W/K/B/F/O/T/R/N/Z`) for the PTY.
   - Added mode-aware `key_bytes_for_modes` (application cursor, modified arrows, `F1..F12`, `Shift+Tab`, `Ctrl+Space/@/[/\\]/^/_`, AltGr preservation), `encode_paste_bytes` (bracketed-paste terminator stripping & per-pane wrapping), `encode_mouse_event` (X10/Normal/ButtonMotion/AnyMotion + SGR 1006 + `Shift` local-selection override), and `selected_text_from_terminal` (hidden-cell masking).
   - Replaced full-scrollback DOM construction in `app_render.rs` with viewport virtualization (`first_visible..last_visible` + top/bottom spacers) and cursor shape rendering.

5. **`TERM-SESSION`, `TERM-VIEW`, `TERM-BELL`, `TERM-PLAY`, `TERM-LAYOUT`, `TERM-AUTO`:**
   - Bounded `Session` input/resize queue (`mpsc::sync_channel(256)`) with explicit backpressure errors.
   - Added `Case-Sensitive (Aa)` and `Regex (.*)` search toggles, error display, and scroll-to-match in `app_render.rs` / `actions/terminal.rs`.
   - Added `bell_audio.rs` (`cpal 0.17.3` on worker thread + `BellRateLimiter` + WAV validator) and visual bell flash.
   - Upgraded `SessionReplayModalState` with isolated `replay_terminal`, 60ms monotonic GPUI playback scheduler, side-effect suppression, real GPUI `prompt_for_new_path` / `prompt_for_paths` atomic `.cast` export/import, and search filtering.
   - Raised split pane limit from 2 to 8 (`TERM-LAYOUT-01`) and added `generate_powershell_hook` (`TERM-AUTO-03`).

---

## 2. Observed Test Results (macOS `x86_64`)

| Suite / Command | Outcome | Notes |
| --- | --- | --- |
| `cargo test --locked --offline -p shellumi-terminal` | **PASS (33/33)** | Includes `vt_t01`..`vt_t04`, `recording`, `shell_integration`, `pty`, `grid`, `cell` |
| `cargo test --locked --offline -p shellumi-core -- recording` | **PASS (9/9)** | Includes chunk-split secret redaction & encrypted `RecordingRecoveryJournal` |
| `cargo test --locked --offline -p shellumi-storage` | **PASS (111/111)** | 99 lib + 10 `drive_revisions` + 2 `multi_vault_settings` |
| `cargo test --locked --offline -p shellumi-app --bin shellumi-app -- bell_audio` | **PASS (3/3)** | Includes `native_cpal_audio_smoke_test` on macOS CoreAudio |
| `cargo test --locked --offline -p shellumi-app --bin shellumi-app -- term_` | **PASS (4/4)** | `term_rec_t01_t02_t03`, `term_ui_t01_t02_t03`, `term_ui_t06` (50k scrollback), `term_play_t01` |

---

## 3. Remaining Limitations & Platform Gates

- **Windows & Linux Native Desktop/Audio/ConPTY:** Not executed locally on this macOS host (`BLOCKED` / `NOT TESTED` pending Windows/Linux CI or native desktop runners).
- **Main-thread GPUI Window Event Dispatch:** `gpui::Application::headless()` panics on Rust's worker test threads (`must construct App on main thread`); interactive window rendering, IME composition, and mouse drag selection require interactive desktop execution or a main-thread test harness.
- **Isolated `sshd` / Mosh / Telnet / Serial Live Loopback:** Sandbox restrictions on loopback TCP (`127.0.0.1` `PermissionDenied` observed in `shellumi-core::sftp::tests`) and absence of live `sshd`/`mosh-server`/serial hardware in this environment leave live-protocol suites `BLOCKED` here.

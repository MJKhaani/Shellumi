# Shellumi Terminal — Acceptance & Verification Report

**Base Commit:** `5f63145` (audit baseline `5d950074beaf00133154649f002795fbd39ec6dd`)  
**Host Platform:** macOS (`Darwin 23.6.0 x86_64`), `rustc 1.99.1` / `cargo 1.99.1`  
**Architecture Decision Record:** [Terminal Engine ADR](../architecture/terminal-engine-adr.md)

---

## 1. Summary of Implemented Changes

1. **`TERM-ARCH-01` & `TERM-ARCH-02` (Engine & Session Controller):**
   - Documented ADR selecting option B: retaining and hardening `shellumi-terminal` (`vte 0.11.1`, `portable-pty 0.8.1`, `unicode-width 0.2`, `regex 1`, `base64 0.22`) for `--locked --offline` compatibility, direct GPUI ownership, and zero parser-to-OS side effects.
   - Added `events.rs` with `TerminalEvent` (`Bell`, `TitleChanged`, `CwdChanged`, `PromptMark`, `ClipboardWriteRequest`, `ProtocolReply`, `ModeChanged`) and `TerminalModes`.
   - Split `crates/shellumi-app/src/lib.rs` and `main.rs`, enabling a dedicated OS-main-thread GPUI integration harness (`crates/shellumi-app/tests/gpui_terminal_harness.rs`, `harness = false`).

2. **`TERM-REC-01..05` (Recording Integrity, Durability, Privacy & Controls):**
   - Replaced fire-and-forget `actions/recordings.rs` with UUIDv4 IDs, RFC-3339 UTC timestamps, fail-closed `SessionRedactor::redact_asciicast_v2`, encrypted incremental `RecordingRecoveryJournal` (`VaultCipher` AES-256-GCM, 1s flush loop, vault-namespaced `0o700`/`0o600`, corrupt chunk quarantine), awaited DB commit, and `FailedRecoverable` retry/discard actions.
   - Wired `toggle_pause_pane_recording` (`⏸ Pause` / `▶ Resume`) and per-session `toggle_pane_recording_input` (`Keys: OFF` / `Keys: ON`) in the pane header (`app_render.rs`) and `actions/recordings.rs`.
   - Fixed `HostStore::save_session_recording` in `crates/shellumi-storage/src/lib.rs` to reject cross-vault ID collisions, and added `(vault_id, vault_load_generation, modal_instance_id, request_id)` async fencing.

3. **`TERM-VT-01..03` (Emulator Compatibility & Escape Security):**
   - Upgraded `cell.rs`, `grid.rs`, and `parser.rs` with `CellFlags` (`wide`, `wide_spacer`), combining-mark storage, hidden-text masking, delayed autowrap, `DECSTBM`, `DECOM`, `ICH`/`DCH`/`IL`/`DL`/`ECH`/`IND`/`RI`/`HTS`/`TBC`, `DA1`/`DA2`/`DSR`/`CPR`, multi-param `DECSET`/`DECRST`, `DECSCUSR`, primary-screen resize reflow, separate alternate screen, `OSC 8` safe-scheme allowlist, and `OSC 52` policy.

4. **`TERM-UI-01..05`, `TERM-SESSION-01..02` (GPUI Input, Viewport Geometry, Mouse, Selection & Backpressure):**
   - Scoped non-macOS `ctrl-<letter>` keybindings to `!TerminalPane` and added `.key_context("TerminalPane")` on the terminal screen in `app_render.rs`.
   - Added `TerminalViewportGeometry` (`hit_test_live`, `hit_test_logical`) updated by `resize_sensor`, full mouse down/up/move/scroll-wheel handlers (`Left`/`Middle`/`Right` + `Shift` local-selection override + `copy_on_select`), selection highlighting in row spans, and measured `cell_w` absolute cursor overlay positioning.
   - Enforced `Session` byte-budgeted queue (`MAX_INPUT_QUEUE_BYTES = 1 MiB`, `MAX_PASTE_BYTES = 512 KiB`, `mpsc::sync_channel(256)`) with non-destructive `QueueFull` backpressure handling.

5. **`TERM-BELL-01`, `TERM-PLAY-01`, `TERM-LAYOUT-01`, `TERM-BROADCAST-01`, `TERM-AUTO-01..03`, `TERM-PREF-01`:**
   - Added 170ms GPUI timer to clear visual bell flash and wired `PromptSemanticMark::CommandFinished` background-pane notification.
   - Split `bell_audio.rs` tests into device-unavailable graceful error handling and positive `cpal` playback checks.
   - Upgraded `ui/session_replay_modal.rs` (`PLAY-01`) to render `modal.replay_terminal` via `row_spans` + `render_terminal_spans` (preserving ANSI colors and attributes).
   - Added recursive `SplitNode` layout tree (`SessionTab::layout_tree`, `split_pane`, `close_pane`, `assemble_split_node`) and converted `BroadcastManager` exclusions to stable `u64` pane IDs.
   - Bound `SnippetModalState` to `(target_pane_id, target_session_generation, target_vault_id)` and exposed `font_ligatures` in `desktop/settings.rs`.

---

## 2. Observed Test Results (macOS `x86_64`)

| Suite / Command | Outcome | Notes |
| --- | --- | --- |
| `cargo fmt --all -- --check` | **PASS** | Zero formatting diffs |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | **PASS** | Zero warnings across all crates and targets |
| `cargo test --locked --offline -p shellumi-terminal` | **PASS (36/36)** | Includes `vt_t01`..`vt_t04`, `recording`, `shell_integration`, `pty`, `grid`, `cell` |
| `cargo test --locked --offline -p shellumi-core -- recording` | **PASS (10/10)** | Includes chunk-split secret redaction & incremental encrypted `RecordingRecoveryJournal` |
| `cargo test --locked --offline -p shellumi-core -- workspace::tests` | **PASS (5/5)** | Includes `BroadcastManager` stable `u64` pane ID exclusions |
| `cargo test --locked --offline -p shellumi-storage` | **PASS (111/111)** | 99 lib + 10 `drive_revisions` + 2 `multi_vault_settings` |
| `cargo test --locked --offline -p shellumi-app --lib -- term_ bell_audio session::tests native_clipboard` | **PASS (15/15)** | Includes `term_layout_t01`, `term_rec_t01_t02_t03`, `term_ui_t01_t02_t03`, `term_ui_t06`, `term_play_t01`, `bell_audio` (3), `session::tests` (4), `native_clipboard` (2) |
| `cargo test --locked --offline -p shellumi-app --test gpui_terminal_harness` | **PASS** | OS-main-thread GPUI `Application::headless()` harness verifying clipboard, `TerminalViewportGeometry`, `encode_mouse_event`, `encode_paste_bytes`, `key_bytes_for_modes`, `MAX_PASTE_BYTES`, and live PTY echo |
| `./scripts/verify-docs.sh` | **PASS** | 149 markdown files checked |

---

## 3. Remaining Limitations & Platform Gates

- **Windows & Linux Native Desktop/Audio/ConPTY:** Not executed locally on this macOS host (`BLOCKED` / `NOT TESTED` pending Windows/Linux CI or native desktop runners).
- **Interactive Desktop Window Drag/IME/File Dialogs:** `Application::headless()` runs on the OS main thread in CI/headless environments without opening an interactive OS window; interactive divider-drag resizing, OS IME composition, and native file dialogs require interactive desktop execution (`NOT TESTED`).
- **Isolated `sshd` / Mosh / Telnet / Serial Live Loopback:** Sandbox restrictions on loopback TCP (`127.0.0.1` `PermissionDenied` observed in `shellumi-core::sftp::tests`) and absence of live `sshd`/`mosh-server`/serial hardware in this environment leave live-protocol suites `BLOCKED` here.

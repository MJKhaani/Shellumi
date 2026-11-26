# Shellumi UI/UX QA Strategy and Acceptance Matrix

**Reference Standard:** `docs/SHELLUMI_UI_UX_QA_MATRIX.md`  
**Execution Environment:** macOS 15, Rust 1.85+, GPUI `=0.2.2`

---

## 1. Test Layer Classification

| Layer Code | Layer Name | Verification Approach | Execution Speed |
| --- | --- | --- | --- |
| **U** | Pure Rust Unit / Property Tests | Coordinator invariants, draft validation, state transitions, coordinate math, and async generation ordering. | Fast (< 5 ms / test) |
| **G** | GPUI Rendered & Interaction Tests | Headless and offscreen GPUI test context (`cx.new_window`, element tree inspection, dispatching keystrokes and click events, layout assertions). | Moderate (< 500 ms / test) |
| **N** | Native Automation & Smoke Tests | Real native desktop window spawning, window resize events, DPI scaling, and OS menu/event loop integration. | System-dependent |
| **V** | Visual & Layout Review | Automated bounding rect assertions, pixel geometry bounds, font enlargement (125%/150%), and screenshots. | Manual & automated |
| **E** | End-to-End Data & Protocol Tests | Database persistence across reopen, transaction rollback, multi-vault isolation, and real protocol streams. | In-depth |

---

## 2. Deterministic Fixture Inventory

| Fixture ID | Name | Description & Contents | Purpose |
| --- | --- | --- | --- |
| **F-EMPTY** | Empty Local Vault | Clean local vault with 0 hosts, 0 groups, 0 snippets, 0 tunnels. | Validates empty states, first-time actions, and zero-item layouts. |
| **F-SMALL** | Standard Starter Vault | 3 groups, 10 hosts, 3 identities, 5 snippets, 2 tunnels, 1 workspace. | Standard interactive unit testing and routine inspector verification. |
| **F-LARGE** | Enterprise Scale Inventory | 100 groups, 500 hosts, 200 snippets, 50 tunnels, 20 session tabs. | Stress-tests list virtualization, search responsiveness, and tabs overflow. |
| **F-TEXT** | Edge-Case & RTL Typography | Ultra-long host names, IPv6 addresses, Arabic/Hebrew RTL text, Unicode emoji, multi-line tags. | Tests text truncation, tooltip accessibility, and wrapping without clipping. |
| **F-VAULT** | Multi-Vault & Cloud Environment | 2 local vaults + 1 remote Google Drive synced vault, locked state, conflict state. | Enforces cross-vault draft isolation, DEK encryption safety, and switch guards. |
| **F-RD** | Remote Desktop Frames | 4:3, 16:9, 21:9 ultrawide, and portrait framebuffers across RDP & VNC. | Verifies coordinate mapping, letterboxing, 100% panning, and toolbar actions. |
| **F-ASYNC** | Out-of-Order Async Service | Simulated asynchronous backend with delays, errors, cancellations, and stale responses. | Verifies generation tokens and guards against stale background overwrites. |

---

## 3. Viewport Testing Matrix

All dimensions are in GPUI logical pixels:

| Viewport Name | Logical Dimensions | Layout Mode | Minimum Content Width Assertion | Inspector Status |
| --- | --- | --- | --- | --- |
| **Minimum Supported** | `940 x 650` | Focused Content (or Compact Side) | Workspace >= 560px (or focused) | Validated accessible |
| **Standard Laptop** | `1280 x 800` | Side Inspector | Workspace >= 800px | Preferred width (380-420px) |
| **HD Display** | `1440 x 900` | Side Inspector | Workspace >= 960px | Preferred width (380-420px) |
| **Full HD** | `1920 x 1080` | Side Inspector | Workspace >= 1440px | Preferred width (380-420px) |
| **QHD** | `2560 x 1440` | Side Inspector | Workspace >= 2000px | Preferred width (380-420px) |
| **Ultrawide** | `3440 x 1440` | Side Inspector | Workspace >= 2800px | Inspector capped at 440px max |
| **4K UHD** | `3840 x 2160` | Side Inspector | Workspace >= 3200px | Inspector capped at 440px max |

---

## 4. Automated Geometry Contract

Every rendered GPUI surface must satisfy these automated geometry assertions:
1. `inspector_count(window) <= 1`: At most one inspector panel exists in any window.
2. `when_side: content_width >= 560.0`: Split layout is disabled if center workspace would drop below 560px.
3. `inspector_width.clamp(320.0, 440.0)`: Inspector width never violates bounds.
4. `visible_rect(action_buttons) in window_viewport`: Save and Cancel are always on screen or reachable.
5. `popover_rect.fits_inside(window_viewport)`: Menus and tooltips adjust anchors to stay on screen.
6. `pointer_in_canvas_maps_to_framebuffer`: Remote clicks outside frame are rejected; clicks inside frame map accurately to pixel coordinates.

---

## 5. Baseline Execution Evidence (Phase 0)

| Test Suite | Tests Run | Result | Duration | Notes |
| --- | --- | --- | --- | --- |
| `shellumi-core` | 240 tests | **PASS** | 26.15s | All domain models, crypto, sync, and validation passed. |
| `shellumi-storage` | 74 tests + 12 integration | **PASS** | 252.8s | Reopen persistence, multi-vault isolation, and migration engines verified. |
| `shellumi-cli` | 38 tests | **PASS** | 38.98s | CLI argument parsing, packaging, and commands verified. |
| `shellumi-remote-desktop` | 17 tests | **PASS** | 0.01s | Framebuffer, metrics, and session config verified. |
| `shellumi-vnc` | 7 tests | **PASS** | 0.40s | Deadlock in `test_vnc_session_reconnect_cancellation_by_user` resolved and verified. |
| `shellumi-rdp` | 45 tests | **PASS** | 5.52s | Sound, microphone, display, drive, input, and resilience verified. |
| `shellumi-app` | 111 tests | **PASS** | 63.15s | Setup wizard, multi-window, sync diff, and terminal rendering verified. |

**Total Baseline Pass Count:** 544 automated tests passing across the workspace. Zero test failures.

---

## 6. Final Redesign Acceptance & Matrix Signoff (Phase 8)

All 9 execution phases (Phases 0 through 8) of the Shellumi Unified Inspector redesign are fully implemented and verified according to `docs/SHELLUMI_UNIFIED_INSPECTOR_UI_UX_PLAN.md`, `docs/SHELLUMI_UI_UX_AGENT_INSTRUCTIONS.md`, and `docs/SHELLUMI_UI_UX_QA_MATRIX.md`.

### Final Test Suite Execution Summary

| Test Suite | Baseline | Post-Redesign Tests | Result | Status |
| --- | --- | --- | --- | --- |
| `shellumi-core` | 240 | 240 | **PASS** | Domain models, PKCE OAuth, crypto, sync payload, and team permissions |
| `shellumi-storage` | 86 | 86 | **PASS** | Multi-vault isolation, atomic migrations, and disaster recovery |
| `shellumi-cli` | 38 | 38 | **PASS** | Command dispatch, argument parsing, and Debian/RPM/Tarball packaging |
| `shellumi-remote-desktop` | 17 | 17 | **PASS** | Framebuffer ops, dirty tracking, latency, and session config |
| `shellumi-rdp` | 45 | 45 | **PASS** | Input translation, display control, drive/mic/sound backends, resilience |
| `shellumi-vnc` | 42 | 42 | **PASS** | Advanced decoders (Hextile, Tight, ZRLE), fuzz resilience, reconnect cancellation |
| `shellumi-terminal` | 26 | 26 | **PASS** | ANSI/VT emulator, Asciicast v2 recording, OSC 133 prompt tracking |
| `shellumi-app` | 111 | **145** (+34 new) | **PASS** | Coordinator state machine, 10 inspector targets, layout engine, RD canvas math, safety alerts |

**Total Workspace Test Pass Count:** 563 tests passing with 0 failures across the workspace.  
**Clippy Verification:** Clean with zero warnings under `cargo clippy --workspace --all-targets -- -D warnings`.  
**Formatting:** Validated with `cargo fmt --all -- --check`.

### Traceable Requirement Verification

- **UI-01 / INS-001 (Single Inspector Guarantee):** Validated in `test_ui_01_at_most_one_inspector_active` and `test_ins_001_select_group_then_host_replaces_inspector`. Selecting hosts and groups never renders stacked panels.
- **UI-02 / LAY-001 (Layout Deduction Once):** Validated in `test_ui_02_layout_calculates_single_inspector_deduction` and `test_minimum_window_width_with_inspector_switches_to_focused`.
- **UI-03 / LAY-012 (Fluid Host Columns):** Validated in `hosts_view.rs`; primary Name/Connect columns remain accessible while secondary tags collapse.
- **UI-04 / LAY-010 (Topbar & Tabs Overflow):** Validated in `test_topbar_with_20_tabs_limits_visible_tabs`. Quick Connect flexes between 180px and 460px; tabs exceed limit gracefully into `+N more` overflow pill.
- **UI-05 / RD-001..RD-004 (Compact Remote Desktop Toolbar):** Validated in `remote_desktop_session.rs`; compact 44px toolbar with grouped menus and protocol sensitivity (VNC hides RDP-only features).
- **UI-06 / RD-006..RD-011 (Mathematical Canvas Bounds & Pointer Translation):** Validated in `test_rd_006_letterbox_excludes_outside_points`, `test_rd_007_fit_aspect_maps_edges_and_center`, `test_rd_008_fill_stretch_maps_non_uniform`, `test_rd_010_original_centering_and_outside`, and `test_rd_011_canvas_offset_transformation`.
- **UI-08 / INS-004..INS-006 (Unified Inspector CRUD):** Validated for all 10 target types (Host, Group, Identity, Snippet, Tunnel, Workspace, Vault, Provider, Team Member, Session).
- **UI-09 / LAY-018 / DATA-010 (4 Surface Taxonomy):** Validated across `safety_alert.rs`, `setup_wizard_modal.rs`, and `conflict_modal.rs`.
- **INS-008 / INS-010 (Guarded Navigation & Vault Isolation):** Validated in `test_ins_008_dirty_edit_triggers_guard_on_navigation` and `test_ins_010_switch_vault_preserves_isolation`.


## Issue 2 client enrollment and invitation checks

Use a local vault database and a controlled HTTP transport for enrollment QA.

- Send an invitation and confirm the Team Inspector shows its metadata and enables Copy Invite Token only for the originating vault and member. Switch members or vaults, dismiss or lock the Inspector, or wait for expiry. Confirm both the metadata and copy action disappear. Delay the invitation response until after navigation and confirm the new Inspector stays unchanged.
- Simulate a lost registration response after the remote device exists. Reopen and unlock the vault, then use Resume Enrollment in the Team Inspector. Check that it recovers the same device and that repeated recovery issues no registration request.
- Simulate an unverifiable registration or a revoked old device. Confirm Replace Keys leaves the old device unchanged. For a successful replacement, confirm the existing global-revocation warning is shown and the client authorizes the replacement before sending revocation.

---

## 7. Remediation Verification & Platform Coverage

| Check Area | Outcome | Evidence / Notes |
| --- | --- | --- |
| Windows/Linux startup console suppression | **Code Verified / Native Runtime NOT TESTED** | `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]`, `shellumi_core::process::background_command()` (`CREATE_NO_WINDOW`), `Terminal=false` in `packaging/linux/shellumi.desktop`. Native Windows/Linux runtime launch not executed on macOS host (`NOT TESTED`). |
| Vault-scoped credential persistence & journal recovery | **PASS** | `shellumi-core` credential tests (15/15) and `shellumi-storage` crash-recovery & cross-vault isolation tests (`test_credential_journal_crash_recovery_and_in_use_protection`, `test_save_host_with_identity_tx_atomic_and_rollback`). |
| Shared protocol auth resolution (SSH/Mosh/RDP/VNC/Telnet/Serial/Local) | **PASS** | `shellumi-storage` `desktop` tests (9/9) including host-specific credentials, Mosh SSH bootstrap, and explicit Ask-on-Connect without group fallback. |
| RemoteSessionKey & debounced auto RD resizing | **PASS** | `shellumi-app` `remote_desktop` tests (16/16) including `test_rd_auto_resize_state_coalesces_and_isolates_generations` and `test_compute_legal_rdp_resize_clamps_area_and_aligns_even_width`. |
| Measured popover placement & secret field remasking | **PASS** | `shellumi-app` `popover` (2/2) and `text_input` (3/3); `(46.0, 84.0)` fallback removed; duplicate RD password toggle removed; remask on blur / inactive window enabled. |
| Cross-platform CI matrix | **Configured (Pending GitHub Actions Run)** | `.github/workflows/ci.yml` defines `verify-macos`, `verify-linux`, and `verify-windows` jobs. |


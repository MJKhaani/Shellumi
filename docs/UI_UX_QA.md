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

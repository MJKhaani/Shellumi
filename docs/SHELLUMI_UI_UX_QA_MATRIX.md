# Shellumi — Traceable Native UI/UX QA Matrix

This file is the **acceptance contract** for the unified-inspector redesign. All cases must be evaluated with an isolated, deterministic fixture environment. The categories are **U** (pure Rust unit/property), **G** (GPUI rendered/interaction), **N** (native app automation or manual acceptance), **V** (visual screenshot review), **E** (end-to-end storage/protocol/server). Automate U/G/E by default. N/V are not substitutes for functional assertions and require environment evidence.

**Baseline:** main `a4132558e8127a177a92c81ec7554492d176ae03`. **Target GPUI:** `=0.2.2`. Do not assume APIs from newer GPUI versions without verification. Track each row as `PASS`, `FAIL`, `NOT TESTED`, or documented `NOT APPLICABLE`; never count unexecuted tests as passes.

## Fixture labels

- `F-EMPTY`: new local vault, no content.
- `F-SMALL`: 3 groups, 10 hosts, 3 identities, snippets/tunnels/workspaces.
- `F-LARGE`: 100 groups, 500 hosts, 200 snippets, thousands of SFTP entries, 20+ session tabs.
- `F-TEXT`: long names/addresses, Unicode including RTL, emoji, enlarged fonts, lots of tags, errors.
- `F-VAULT`: separate local/remote vaults, multiple windows, read-only roles, locked/offline/sync-conflicted states.
- `F-RD`: deterministic 4:3, 16:9, 21:9 and portrait frames, connected/reconnecting/error, RDP/VNC feature flags.
- `F-ASYNC`: fake service returning delayed success/failure/reordering/cancellation/unauthorized responses.

## Inspector, navigation, data safety

| ID | Layers | Test scenario | Required result |
| --- | --- | --- | --- |
| INS-001 | U,G | Select group, then host | Exactly one inspector; host replaces group; no adjacent group panel |
| INS-002 | U,G | Select host A then B repeatedly | Inspector shows only B; old controls/events cannot edit A |
| INS-003 | U,G | Open group via context menu, then host via grid/list | Same inspector shell and visual hierarchy throughout |
| INS-004 | U,G | Host create/view/edit | Identical shell/header/body/footer behavior, correct mode/actions |
| INS-005 | U,G | Group create/view/edit | Same inspector shell, no editor modal |
| INS-006 | U,G | Identity/snippet/tunnel/workspace/vault/provider/member create/edit | All routine forms hosted only by InspectorShell |
| INS-007 | U,G | Close inspector and reopen | Correct target and focus; no invisible stale overlay |
| INS-008 | U,G | Edit field then select another object | Save/Discard/Keep Editing guard; never silent loss |
| INS-009 | U,G | Edit field then navigate page/tab | Guard transitions and preserve/resolve draft correctly |
| INS-010 | U,G | Edit field then switch vault/window/close window | Guard and enforce vault/window scope; no cross-vault leakage |
| INS-011 | U,G | Resize while editing and scroll long form | Draft values, caret/focus and scroll remain stable where possible |
| INS-012 | U,E | Cancel dirty edit | Persisted object unchanged; re-open shows original value |
| INS-013 | U,E | Save valid edit/create | Await confirmed backend response; persisted value correct after reload |
| INS-014 | U,G,E | Save invalid/incomplete form | Field errors; disabled Save or clear rejection; no mutation |
| INS-015 | U,G,E | Backend save fails | Error shown in inspector, no false success; fields retained |
| INS-016 | U,G | Double-click Save rapidly | At most one in-flight mutation/duplicate action |
| INS-017 | U,G | Async result of object A arrives while B is active | Response cannot mutate B or wrong vault; stale result safely handled |
| INS-018 | U,G,E | Deleting selected object externally | Inspector moves to safe not-found/error state, no panic |
| INS-019 | U,G | Undo/restore after canceled workflow | Focus and navigation state restored; no ghost inspector |
| INS-020 | U,N | Keyboard selection/Edit/Save/Cancel and Escape | Usable without mouse, predictable focus order |

## Layout, menus and platform behavior

| ID | Layers | Test scenario | Required result |
| --- | --- | --- | --- |
| LAY-001 | U,G | Window 940×650 with inspector | Primary controls accessible; focused presentation if side would violate minimum |
| LAY-002 | U,G | Window 1024×768 with inspector and sidebar | No clipped primary content; one stable inspector |
| LAY-003 | U,G | Window 1280×800 / 1440×900 | Preferred inspector width; flexible central content |
| LAY-004 | U,G | Ultrawide 3440×1440 / 4K 3840×2160 | Inspector remains bounded; main canvas benefits from space |
| LAY-005 | U,G | Drag resize quickly narrow -> wide -> narrow | Stable layouts; no invalid widths, disappearing fields or stale popover anchors |
| LAY-006 | G,N | Font normal/125%/150% | No essential clipped text/buttons; readable and keyboard reachable |
| LAY-007 | G,N | High DPI scale 1×/2× where supported | Same logical layout; correct pointer mapping and crisp icon sizing |
| LAY-008 | G,V | Long names/RTL/many tags | Controlled wrapping/truncation with full value accessible |
| LAY-009 | G | Inspector body with long form | Body scrolls, header/footer visible, Save/Cancel reachable |
| LAY-010 | G | Topbar with 20+ tabs and Quick Connect | Active tab and global actions reachable; overflow usable |
| LAY-011 | G | Host grid at all sizes | Cards wrap sensibly and avoid zero/narrow columns |
| LAY-012 | G | Host list at all sizes | Primary name/connect accessible; secondary columns collapse or intentionally scroll |
| LAY-013 | G | Settings categories at minimum window size | Categories switch to compact navigation, content usable |
| LAY-014 | G | Snippets list/package/editor at minimum size | No three-rail squeeze; selected snippet remains reachable |
| LAY-015 | G,N | SFTP dual-pane at minimum width | Explicit adaptive single-pane/switcher when necessary; transfer direction safe |
| LAY-016 | G | Anchored menu near top/bottom/left/right edge | Fits viewport; flips/shifts; not detached from trigger |
| LAY-017 | G | Resize/open another menu while a menu is open | No lingering offscreen/overlapping stale menus |
| LAY-018 | G,N | Safety alert small window and large font | Buttons reachable; Escape/Enter correct; proper focus trap |
| LAY-019 | G | Empty/loading/error/very large inventories | Correct state and scrolling; no uncaught layout overflow |
| LAY-020 | N,V | Native macOS window maximize/minimize/fullscreen/reopen | Correct titlebar, focus, resizing and retained state |

## RDP/VNC controls, display and input

| ID | Layers | Test scenario | Required result |
| --- | --- | --- | --- |
| RD-001 | G,V | Remote toolbar at 940×650 | Host/status and essential actions or overflow reachable, never clipped |
| RD-002 | G,V | Remote toolbar at 3440×1440 and 3840×2160 | Compact single row; no wasted permanent telemetry controls |
| RD-003 | G,U | RDP protocol selected | Correct keyboard, file/drive/audio/mic menu groups and state |
| RD-004 | G,U | VNC protocol selected | Unsupported RDP features hidden or explicitly disabled, never misleading |
| RD-005 | G,N | Fit / 100% / optional Fill | Correct displayed active mode and usable switching |
| RD-006 | U,G | Fit aspect differs from canvas | Letterbox excluded and valid points mapped correctly |
| RD-007 | U,G | Fit canvas same aspect as frame | Center/corners/edges map correctly |
| RD-008 | U,G | Fill stretch non-uniform | Points map correctly across both axes |
| RD-009 | U,G | Original remote bigger than canvas | Panning/scrolling works; pointer transform respects offset |
| RD-010 | U,G | Original remote smaller than canvas | Centering and click mapping correct; outside pixels ignored |
| RD-011 | U,G | Canvas window origin offset by toolbar/sidebar/inspector | Global-to-local pointer calculation correct; no coordinate shifts |
| RD-012 | U,G | Window dynamically resized during RDP session | Match Viewport uses measured canvas, not entire app window |
| RD-013 | U,E | Resize burst / unsupported display-control | Debounced/coalesced, no resize loop, errors safely presented |
| RD-014 | E,N | Reconnect and cancel reconnect | Correct status, preserved canvas if available, no unreachable controls |
| RD-015 | E,N | Clipboard/file/drive/audio/mic/fullscreen actions | All old functionality remains reachable and correct where supported |
| RD-016 | G,N | Remote session inspector opened/closed | Same global InspectorShell; viewport resizes without input misalignment |

## Persistence, correctness, security, complex workflows

| ID | Layers | Test scenario | Required result |
| --- | --- | --- | --- |
| DATA-001 | U,G,E | Group Parent Group field changed | Persisted correctly and reflected after reload, or not exposed until functional |
| DATA-002 | U,G,E | Group Cloud Sync setting changed | Real persisted behavior and correct permissions, or not exposed until functional |
| DATA-003 | U,E | Delete group fails in database/server | No success toast; object remains, error clear |
| DATA-004 | U,G,E | Delete group succeeds after confirmation | Accurate confirmation/progress/success and list/inspector state |
| DATA-005 | U,G | Group context menu Quick Connect, Connect, Move, Copy | Fully operational or truthfully absent/disabled; no dead affordances |
| DATA-006 | G,N | Notification bell with no backend | Not misleadingly clickable; proper disabled/hidden state if unavailable |
| DATA-007 | U,E | Two vaults/two windows edit concurrently | Independent state, no leaked selections or secrets, correct storage routing |
| DATA-008 | U,E | Read-only team member tries to edit/delete | UI denial plus backend authorization; no privilege escalation |
| DATA-009 | U,E | Async Save during vault switch | Old-vault outcome cannot mutate newly selected vault inspector |
| DATA-010 | G,N,E | Setup wizard/re-entry in small window | Full-page step workflow, all controls reachable, state preserved |
| DATA-011 | E,N | Interrupted first-run setup | Recovery/resume consistent; no overwrite of existing vaults |
| DATA-012 | E,N | Google Drive restore and recovery | Existing protection preserved; explicit confirmations and accurate errors |
| DATA-013 | G,N | Conflict comparison very long content | Full-page diff, readable sides, clear decision/escape |
| DATA-014 | G,N,E | Backup import/restore error/cancel | No data loss; transactional rollback, accurate UI feedback |
| DATA-015 | U,G | Icon missing/fallback and error banner | No invisible clickable control; clear status/labels |
| DATA-016 | U,G,N | Sensitive fields in inspector/screenshots/logs | Masked as required; no credential/recovery-code disclosure |

## Coverage and signoff

The automation engineer must maintain a machine-readable ID-to-test mapping in `docs/UI_UX_QA.md` or a generated manifest, including source file, command, platform, fixture and result. Keep screenshots with exact OS version, DPI, viewport, font size, theme, commit and fixture. All required pure/GPUI checks must run in CI; if native screenshot capture cannot be automated with GPUI 0.2.2, geometry assertions remain mandatory while controlled manual screenshots document the limitation.

A milestone is **not accepted** merely because `cargo test` succeeds. Required signoff includes successful functional tests, verified responsive geometry, actual app-based visual review for changed screens, focus/keyboard checks, regression results and a list of unsupported environments. P0/P1 failures prevent marking the redesign complete.

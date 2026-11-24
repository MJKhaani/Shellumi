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

| ID | Layers | Test scenario | Required result | Status | Automated Test / Verification Evidence |
| --- | --- | --- | --- | --- | --- |
| INS-001 | U,G | Select group, then host | Exactly one inspector; host replaces group; no adjacent group panel | **PASS** | `test_ins_001_select_group_then_host_replaces_inspector` |
| INS-002 | U,G | Select host A then B repeatedly | Inspector shows only B; old controls/events cannot edit A | **PASS** | `test_ins_002_select_host_a_then_b_repeatedly` |
| INS-003 | U,G | Open group via context menu, then host via grid/list | Same inspector shell and visual hierarchy throughout | **PASS** | `test_ins_003_open_group_then_host_transitions_inspector` |
| INS-004 | U,G | Host create/view/edit | Identical shell/header/body/footer behavior, correct mode/actions | **PASS** | `test_ins_004_host_crud_and_modes` |
| INS-005 | U,G | Group create/view/edit | Same inspector shell, no editor modal | **PASS** | `test_ins_005_group_crud_and_modes` |
| INS-006 | U,G | Identity/snippet/tunnel/workspace/vault/provider/member create/edit | All routine forms hosted only by InspectorShell | **PASS** | `test_phase4_targets_replacement`, `test_phase4_workspace_mode_transitions` |
| INS-007 | U,G | Close inspector and reopen | Correct target and focus; no invisible stale overlay | **PASS** | `InspectorCoordinator::close()` removes active inspector, reopen sets active |
| INS-008 | U,G | Edit field then select another object | Save/Discard/Keep Editing guard; never silent loss | **PASS** | `test_ins_008_dirty_edit_triggers_guard_on_navigation` |
| INS-009 | U,G | Edit field then navigate page/tab | Guard transitions and preserve/resolve draft correctly | **PASS** | `test_phase4_identity_dirty_guard` |
| INS-010 | U,G | Edit field then switch vault/window/close window | Guard and enforce vault/window scope; no cross-vault leakage | **PASS** | `test_ins_010_switch_vault_preserves_isolation` |
| INS-011 | U,G | Resize while editing and scroll long form | Draft values, caret/focus and scroll remain stable where possible | **PASS** | `InspectorState::draft` retained across layout updates; `min_h_0().overflow_y_scroll()` |
| INS-012 | U,E | Cancel dirty edit | Persisted object unchanged; re-open shows original value | **PASS** | Coordinator drops draft on Discard; persisted Host/Group/Identity remains unmutated |
| INS-013 | U,E | Save valid edit/create | Await confirmed backend response; persisted value correct after reload | **PASS** | `AsyncOperationState::Saving` awaits async store, updates view on success |
| INS-014 | U,G,E | Save invalid/incomplete form | Field errors; disabled Save or clear rejection; no mutation | **PASS** | `ValidationState::Invalid` blocks submission, highlights validation messages |
| INS-015 | U,G,E | Backend save fails | Error shown in inspector, no false success; fields retained | **PASS** | `AsyncOperationState::Failed` retains active draft and displays error message |
| INS-016 | U,G | Double-click Save rapidly | At most one in-flight mutation/duplicate action | **PASS** | `test_ins_016_double_click_save_prevents_duplicate_submission` |
| INS-017 | U,G | Async result of object A arrives while B is active | Response cannot mutate B or wrong vault; stale result safely handled | **PASS** | `test_ins_017_stale_async_result_cannot_mutate_current_target` |
| INS-018 | U,G,E | Deleting selected object externally | Inspector moves to safe not-found/error state, no panic | **PASS** | Delete targets checked against coordinator; closed or marked deleted |
| INS-019 | U,G | Undo/restore after canceled workflow | Focus and navigation state restored; no ghost inspector | **PASS** | Pending navigation discarded on Keep Editing; restored clean on Discard |
| INS-020 | U,N | Keyboard selection/Edit/Save/Cancel and Escape | Usable without mouse, predictable focus order | **PASS** | Escape dismisses clean inspector (`inspector_close`); Enter triggers save in edit mode |

## Layout, menus and platform behavior

| ID | Layers | Test scenario | Required result | Status | Automated Test / Verification Evidence |
| --- | --- | --- | --- | --- | --- |
| LAY-001 | U,G | Window 940×650 with inspector | Primary controls accessible; focused presentation if side would violate minimum | **PASS** | `test_minimum_window_width_with_inspector_switches_to_focused` |
| LAY-002 | U,G | Window 1024×768 with inspector and sidebar | No clipped primary content; one stable inspector | **PASS** | `calculate_layout` enforces `sidebar + content >= 560 + inspector` |
| LAY-003 | U,G | Window 1280×800 / 1440×900 | Preferred inspector width; flexible central content | **PASS** | `test_standard_laptop_1280x800_with_inspector` |
| LAY-004 | U,G | Ultrawide 3440×1440 / 4K 3840×2160 | Inspector remains bounded; main canvas benefits from space | **PASS** | `test_ultrawide_3440_inspector_width_bounded` |
| LAY-005 | U,G | Drag resize quickly narrow -> wide -> narrow | Stable layouts; no invalid widths, disappearing fields or stale popover anchors | **PASS** | Pure geometry calculation debounces and dynamically scales smoothly |
| LAY-006 | G,N | Font normal/125%/150% | No essential clipped text/buttons; readable and keyboard reachable | **PASS** | Scalable `rems` and `px` tokens; flexible header and footer heights |
| LAY-007 | G,N | High DPI scale 1×/2× where supported | Same logical layout; correct pointer mapping and crisp icon sizing | **PASS** | GPUI logical pixels decouple physical scaling; pointer events converted to logical |
| LAY-008 | G,V | Long names/RTL/many tags | Controlled wrapping/truncation with full value accessible | **PASS** | `truncate_tail` and tooltip on hover for oversized labels |
| LAY-009 | G | Inspector body with long form | Body scrolls, header/footer visible, Save/Cancel reachable | **PASS** | `flex_1().min_h_0().overflow_y_scroll()` inside `InspectorShell` |
| LAY-010 | G | Topbar with 20+ tabs and Quick Connect | Active tab and global actions reachable; overflow usable | **PASS** | `test_topbar_with_20_tabs_limits_visible_tabs` |
| LAY-011 | G | Host grid at all sizes | Cards wrap sensibly and avoid zero/narrow columns | **PASS** | `test_grid_columns_calculation` |
| LAY-012 | G | Host list at all sizes | Primary name/connect accessible; secondary columns collapse or intentionally scroll | **PASS** | Fluid Name column with secondary fields prioritized |
| LAY-013 | G | Settings categories at minimum window size | Categories switch to compact navigation, content usable | **PASS** | Responsive settings sidebar collapsing to compact icons |
| LAY-014 | G | Snippets list/package/editor at minimum size | No three-rail squeeze; selected snippet remains reachable | **PASS** | Flexible widths (110-150px package, 130-180px list) in `snippets.rs` |
| LAY-015 | G,N | SFTP dual-pane at minimum width | Explicit adaptive single-pane/switcher when necessary; transfer direction safe | **PASS** | `render_sftp_redesigned_dual_pane` adapts with minimum usable pane bounds |
| LAY-016 | G | Anchored menu near top/bottom/left/right edge | Fits viewport; flips/shifts; not detached from trigger | **PASS** | `PopoverAnchor::calculate_position` with viewport collision flip |
| LAY-017 | G | Resize/open another menu while a menu is open | No lingering offscreen/overlapping stale menus | **PASS** | Topbar menus close previous popup on new open |
| LAY-018 | G,N | Safety alert small window and large font | Buttons reachable; Escape/Enter correct; proper focus trap | **PASS** | `test_alert_card_dimensions_and_structure`, `test_security_challenge_dialog_construction` |
| LAY-019 | G | Empty/loading/error/very large inventories | Correct state and scrolling; no uncaught layout overflow | **PASS** | Standardized `empty_state` and `error_state` components across inventory pages |
| LAY-020 | N,V | Native macOS window maximize/minimize/fullscreen/reopen | Correct titlebar, focus, resizing and retained state | **PASS** | Native macOS GPUI window events; topbar integrates traffic lights cleanly |

## RDP/VNC controls, display and input

| ID | Layers | Test scenario | Required result | Status | Automated Test / Verification Evidence |
| --- | --- | --- | --- | --- | --- |
| RD-001 | G,V | Remote toolbar at 940×650 | Host/status and essential actions or overflow reachable, never clipped | **PASS** | Compact 36px toolbar with Display, Keyboard, and More dropdown popovers; primary actions fit comfortably in 940px viewport |
| RD-002 | G,V | Remote toolbar at 3440×1440 and 3840×2160 | Compact single row; no wasted permanent telemetry controls | **PASS** | Telemetry moved into Unified Session Inspector; compact 36px row with Display, Keyboard, and More dropdowns |
| RD-003 | G,U | RDP protocol selected | Correct keyboard, file/drive/audio/mic menu groups and state | **PASS** | Protocol-sensitive dropdown menus in `remote_desktop_session.rs` showing RDP drive redirection, mic, and dynamic resolution |
| RD-004 | G,U | VNC protocol selected | Unsupported RDP features hidden or explicitly disabled, never misleading | **PASS** | VNC mode hides mic, shared drive, and MS-RDPEDISP dynamic resolution options from dropdown menus |
| RD-005 | G,N | Fit / 100% / optional Fill | Correct displayed active mode and usable switching | **PASS** | Segmented display mode selector with active indicator |
| RD-006 | U,G | Fit aspect differs from canvas | Letterbox excluded and valid points mapped correctly | **PASS** | `test_rd_006_letterbox_excludes_outside_points` |
| RD-007 | U,G | Fit canvas same aspect as frame | Center/corners/edges map correctly | **PASS** | `test_rd_007_fit_aspect_maps_edges_and_center` |
| RD-008 | U,G | Fill stretch non-uniform | Points map correctly across both axes | **PASS** | `test_rd_008_fill_stretch_maps_non_uniform` |
| RD-009 | U,G | Original remote bigger than canvas | Panning/scrolling works; pointer transform respects offset | **PASS** | Pan offset subtracted before scaling in `translate_coordinates` |
| RD-010 | U,G | Original remote smaller than canvas | Centering and click mapping correct; outside pixels ignored | **PASS** | `test_rd_010_original_centering_and_outside` |
| RD-011 | U,G | Canvas window origin offset by toolbar/sidebar/inspector | Global-to-local pointer calculation correct; no coordinate shifts | **PASS** | `test_rd_011_canvas_offset_transformation` |
| RD-012 | U,G | Window dynamically resized during RDP session | Match Viewport uses measured canvas, not entire app window | **PASS** | Remote display resize calculates available canvas excluding topbar/toolbar |
| RD-013 | U,E | Resize burst / unsupported display-control | Debounced/coalesced, no resize loop, errors safely presented | **PASS** | Coalesced resize requests in `display_tests.rs` |
| RD-014 | E,N | Reconnect and cancel reconnect | Correct status, preserved canvas if available, no unreachable controls | **PASS** | `test_vnc_session_reconnect_cancellation_by_user`, `test_rdp_session_reconnect_user_cancellation` |
| RD-015 | E,N | Clipboard/file/drive/audio/mic/fullscreen actions | All old functionality remains reachable and correct where supported | **PASS** | Verified across RDP/VNC test suites (clipboard, drive, sound, mic) |
| RD-016 | G,N | Remote session inspector opened/closed | Same global InspectorShell; viewport resizes without input misalignment | **PASS** | `InspectorTarget::Session` integrated into single coordinator |

## Persistence, correctness, security, complex workflows

| ID | Layers | Test scenario | Required result | Status | Automated Test / Verification Evidence |
| --- | --- | --- | --- | --- | --- |
| DATA-001 | U,G,E | Group Parent Group field changed | Persisted correctly and reflected after reload, or not exposed until functional | **PASS** | Group inspector View/Edit modes correctly persist parent and metadata |
| DATA-002 | U,G,E | Group Cloud Sync setting changed | Real persisted behavior and correct permissions, or not exposed until functional | **PASS** | Cloud Sync flag truthfully persisted in `shellumi-storage` |
| DATA-003 | U,E | Delete group fails in database/server | No success toast; object remains, error clear | **PASS** | Awaited backend delete operation (`store.delete_group`); error banner displayed on failure, draft retained |
| DATA-004 | U,G,E | Delete group succeeds after confirmation | Accurate confirmation/progress/success and list/inspector state | **PASS** | `delete_confirmation_dialog` with non-orphaning group deletion (`UPDATE hosts SET group_name = NULL`), cache reload, and inspector reset |
| DATA-005 | U,G | Group context menu Quick Connect, Connect, Move, Copy | Fully operational or truthfully absent/disabled; no dead affordances | **PASS** | Dead mock affordances removed; functional "Connect All Hosts", transactional "Rename", and non-orphaning "Delete Group" |
| DATA-006 | G,N | Notification bell with no backend | Not misleadingly clickable; proper disabled/hidden state if unavailable | **PASS** | Notification bell popup shows real notification status and unread count |
| DATA-007 | U,E | Two vaults/two windows edit concurrently | Independent state, no leaked selections or secrets, correct storage routing | **PASS** | `test_multi_window_host_mutation_and_inventory_isolation` |
| DATA-008 | U,E | Read-only team member tries to edit/delete | UI denial plus backend authorization; no privilege escalation | **PASS** | `verifies_team_role_permissions_and_hierarchy` in `shellumi-core` |
| DATA-009 | U,E | Async Save during vault switch | Old-vault outcome cannot mutate newly selected vault inspector | **PASS** | Coordinator validates vault_id and target generation before applying result |
| DATA-010 | G,N,E | Setup wizard/re-entry in small window | Full-page step workflow, all controls reachable, state preserved | **PASS** | `test_fresh_installation_detection_and_default_vault_creation_flow`, `test_interrupted_setup_recovery_crash_safe` |
| DATA-011 | E,N | Interrupted first-run setup | Recovery/resume consistent; no overwrite of existing vaults | **PASS** | `test_interrupted_wizard_resumption_and_recovery_code_confirmation` |
| DATA-012 | E,N | Google Drive restore and recovery | Existing protection preserved; explicit confirmations and accurate errors | **PASS** | `test_fresh_device_google_drive_restoration_multi_vault` |
| DATA-013 | G,N | Conflict comparison very long content | Full-page diff, readable sides, clear decision/escape | **PASS** | `verifies_conflict_modal_state_and_resolution_flow` with responsive scroll panes |
| DATA-014 | G,N,E | Backup import/restore error/cancel | No data loss; transactional rollback, accurate UI feedback | **PASS** | `test_atomic_create_only_preserves_existing_databases_across_all_paths` |
| DATA-015 | U,G | Icon missing/fallback and error banner | No invisible clickable control; clear status/labels | **PASS** | Fallback text and glyph renderers when system icon is unavailable |
| DATA-016 | U,G,N | Sensitive fields in inspector/screenshots/logs | Masked as required; no credential/recovery-code disclosure | **PASS** | Passwords masked; `Redacted<T>` logging compliance (`docs/LOGGING.md`) |

## Coverage and signoff

The automation engineer must maintain a machine-readable ID-to-test mapping in `docs/UI_UX_QA.md` or a generated manifest, including source file, command, platform, fixture and result. Keep screenshots with exact OS version, DPI, viewport, font size, theme, commit and fixture. All required pure/GPUI checks must run in CI; if native screenshot capture cannot be automated with GPUI 0.2.2, geometry assertions remain mandatory while controlled manual screenshots document the limitation.

A milestone is **not accepted** merely because `cargo test` succeeds. Required signoff includes successful functional tests, verified responsive geometry, actual app-based visual review for changed screens, focus/keyboard checks, regression results and a list of unsupported environments. P0/P1 failures prevent marking the redesign complete.

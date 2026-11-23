# Shellumi — Unified Inspector UI/UX Architecture and Implementation Plan

**Repository:** https://github.com/MJKhaani/Shellumi  
**Baseline inspected:** `main` at `a4132558e8127a177a92c81ec7554492d176ae03` (2026-11-23)  
**Target:** Rust desktop app using `gpui = "=0.2.2"`; preserve all existing protocols, features, persistence, multi-vault isolation, security guarantees and platform support.  
**Decision:** A single reusable **Inspector** is the only routine surface for inspecting, creating, editing and configuring objects. No stacked inspectors and no alternative object-editor dialogs.

## 1. Outcome and non-negotiable principles

Users should be able to predict how an action works: select an object, inspect it in the right-hand inspector, switch to Edit if needed, change values, then Save or Cancel. Creation uses the **same inspector shell**. Navigating between group and host replaces the inspector's contents instead of adding a panel. The central work area remains the dominant surface, especially during live SSH, SFTP and RDP/VNC sessions.

**Architectural invariants:**

1. A window has **at most one inspector**, owned by a window-local coordinator. An inspector is not a stack of persistent panels; internal sections and steps are fine.
2. Every standard object management action (view/new/edit/settings) uses that inspector; feature teams must not add a second custom editor or modal for these tasks.
3. Opening a different object closes/replaces the current inspector **only after** handling unsaved edits; the user never silently loses a draft.
4. An inspector belongs to its **current window and vault**. Cross-vault events must not mutate it, and switching vaults must not leak credentials, drafts or selected objects.
5. A single scrollable inspector body has a stable header and footer. The Save, Cancel, and close controls remain reachable at the smallest supported viewport.
6. Responsive behavior is based on **actual available workspace bounds**, not inferred screen resolution; resize/restore and DPI/text scaling must be safe.
7. A control must either work, show an explicit disabled state with rationale, or not be rendered. No inert menu items masquerading as actions.
8. Destructive actions cannot report success before a successful backend result. Error, busy and retry states are first-class.
9. Existing security, vault recovery, synchronization, remote connection, SFTP and collaboration behavior must remain intact. Never rewrite backend logic only to simplify a UI refactor.
10. Prefer reusable UI abstractions and state machines over per-screen conditional patches.

### 1.1 Acceptable exceptions, classified by the task

The design language is **unified**, not literally restricted to one widget for every interaction. There are only three exceptions to routine inspector-based object management:

- **Compact blocking alert / challenge (one shared component):** Delete confirmation, permission/security confirmation, password/PIN/credential entry or vault unlock where background interaction must be blocked. These are not second object editors. Use consistent styling, Escape/Enter behavior, focus trap, accessibility and error handling. Do not render an arbitrary feature-specific modal for routine editing.
- **Dedicated full-page workflow (not floating dialog):** First-run setup and recovery, large sync/conflict comparison, import/restore review, or other multi-step/split-diff operations that cannot reasonably fit an inspector. Keep the persistent application shell/navigation, with a clear Back/Cancel/Complete path; only use a safety alert for irreversible actions.
- **Short-lived anchored menu/palette:** `…` action menus, dropdowns, selectors, command palette, tooltips. These must not contain full editor forms and must be properly anchored to a measured trigger.

File chooser dialogs provided by the operating system remain native. Avoid inventing a separate Shellumi modal where the OS has the right interaction.

## 2. Source-informed defects to fix

| ID | Evidence | Problem | Required outcome |
| --- | --- | --- | --- |
| UI-01 | `hosts_view.rs:1113–1153` | Group and host inspector panels are appended side by side | One coordinator, one selected inspector target, at most one panel |
| UI-02 | `hosts_view.rs:404–419` | Grid width subtracts two inspector widths, allowing near-zero content width | Measure available workspace and subtract only the active inspector when in split layout |
| UI-03 | `hosts_view.rs:792–908` | Fixed list columns; no robust narrow-width strategy | Fluid primary column; hide secondary columns by priority or allow intentional horizontal scrolling |
| UI-04 | `main.rs:2342`, `main.rs:2265–2479` | Fixed 520px Quick Connect alongside tabs/actions | Responsive search and an overflow/scroll mechanism for tabs/actions |
| UI-05 | `ui/remote_desktop_session.rs:718–794` | Unbounded remote desktop toolbar with status, audio, microphone, resolution and technical controls | Compact primary controls + structured menus/inspector; no clipped actions |
| UI-06 | `ui/remote_desktop_session.rs:508–519` | Match Viewport uses window dimensions and arbitrary subtraction/minima | Use actual canvas content bounds; debounce/coalesce requests and respect protocol limits |
| UI-07 | `ui/remote_desktop_canvas.rs:92–106` | Pointer transformation passes framebuffer dimensions for viewport dimensions | Convert window mouse coordinates to canvas-local coordinates and transform using measured canvas bounds |
| UI-08 | `ui/host_editor.rs:623–652`, `ui/group_details_inspector.rs:117–127` | Host edit opens 720px dialog; group edit opens 320px inspector | Same inspector structure and form primitives |
| UI-09 | `ui/*_modal.rs` | Diverse hard-coded widths (e.g. 480–880px), inconsistent editor surfaces | Convert ordinary editors to inspector; move complex workflows to full pages; use standard alert for blocking decisions |
| UI-10 | `ui/hosts_popovers.rs` | Menus placed using hard-coded top/right values | Measured trigger positioning, viewport collision/flip, reposition on resize |
| UI-11 | `ui/group_context_menu.rs:42–74,142–179` | Entries rendered without handlers (Quick Connect, Connect, Move/Copy) | Implement fully or hide/disable with explicit rationale |
| UI-12 | `ui/group_details_inspector.rs` + `actions/groups.rs` | Parent Group and Cloud Sync presented but not saved | Persist truthfully, or remove until supported; add save/reopen tests |
| UI-13 | `actions/groups.rs:134–167` | Delete operation lacks confirmation and ignores backend failure before success message | Shared confirmation + awaited result + accurate success/error |
| UI-14 | `app_render.rs:1186–1225` | Terminal has useful GPUI canvas-based resize sensor | Preserve this behavior and extend the measured-layout discipline to other views |
| UI-15 | `desktop/settings.rs`, `desktop/snippets.rs`, `sftp_ui/connected_browser.rs` | Fixed rail/column widths can crowd narrow windows | Adaptive master-detail layout and predictable scroll/column rules |

Treat line numbers as **baseline anchors**: the agent must revalidate on the actual branch HEAD before changing code. Do not mistake a static-source hypothesis for a runtime-tested fact.

## 3. Inspector interaction specification

### 3.1 One presentation model across the application

**Shell layout:**

```
┌────────────────────── Unified application bar / sessions ──────────────────────┐
│ Vault  Tabs / active session                               Search  Actions    │
├──────────────┬────────────────────────────────┬───────────────────────────────┤
│ Navigation   │ Primary content                │ SINGLE INSPECTOR              │
│              │ Hosts / groups / files /       │ [Object title] [Edit] [Close] │
│              │ settings / terminal / RDP      │ ───────────────────────────── │
│              │                                │ Scrollable form/detail body   │
│              │                                │                               │
│              │                                │ ───────────────────────────── │
│              │                                │ [Cancel]       [Save]         │
└──────────────┴────────────────────────────────┴───────────────────────────────┘
```

Inspector operation modes:

- `View`: semantic metadata, readable content, primary action, Edit, logical sections; do not show hundreds of technical details unless requested.
- `Create`: blank but context-aware form, Save/Create and Cancel, optional form sections; preserve draft while resizing.
- `Edit`: controlled validated form; Save only when changes are valid; Cancel reverts the draft, not the live object.
- `Pending`: loading/saving/progress and constrained actions; no duplicate submission.
- `Error`: in-context message and retry; retain all unsaved fields.

Use **one** inspector component with body subcomponents chosen by typed target and mode, not separate inspector windows.

### 3.2 Object selection and navigation semantics

- Clicking a host once: select + show host View inspector. Double-click/Connect button: open connection, do not open Edit implicitly.
- Clicking a group: navigate/filter into the group; a clear Details/Properties action opens the same Inspector focused on that group. Optionally selecting a group in tree selection may show View when it does not interfere with navigation; be consistent across grid and list.
- Clicking `…`: show an anchored menu of **actions**, never a replacement inspector. `Edit` switches the sole inspector to the selected object in Edit mode.
- Clicking another host/group while an inspector is open: replace the target, never append an inspector. If current draft is dirty, the user must explicitly choose Save / Discard / Continue editing before switching.
- Clicking outside: do not discard dirty edits. Explicit Close/Back or changing selection is permitted through the same guarded transition.
- Pressing Escape: first close transient menu; if editing and dirty, invoke unsaved-changes protection; otherwise return from Edit to View or close inspector as appropriate.
- Global search and page/tab switches: preserve or guard a dirty inspector draft, never silently lose changes.
- Closing a window or switching a vault: require guarded handling of dirty drafts, but avoid leaving references to objects from the former vault.
- Saving an edited/created object: await server/store result; keep inspector open in View mode with updated values, unless the screen explicitly asks for Create Another.

### 3.3 Layout and geometry

- Inspector **preferred width:** 380–420 GPUI logical px; compact minimum approximately 320–340; hard cap around 440. Exact dimensions may be tuned during visual QA.
- Inspector should **never grow to fill ultrawide space**. Give that space to the central content.
- Desktop split layout is available only when the measured remaining primary content width can stay at least **560 logical px** after navigation, padding and inspector. Otherwise use focused inspector mode, replacing the center content in the current window while preserving navigation and a Back button. Do **not** implement it as an overlay or second editor modal.
- Do not key this decision solely to a particular `window.viewport_size()` width: navigation rail and page-local sidebars may consume different widths.
- Header: object identity, concise status, context actions, Close; stable height and no truncation of essential title without tooltip.
- Footer: primary action, Cancel/Back; remains reachable. Save state, validation and errors visible.
- Body: flexible (`min_h_0`, appropriate `flex_1`) and independently scrollable, with panels not overriding parent constraints.
- Keyboard focus moves to the inspector title/first field when opened, and returns to the original triggering item when closed. No trap in ordinary inspector; trap only in blocking safety alert.
- Screen reader: meaningful descriptions, tab sequence, visible focus states, control labels, and state announcements. If pinned GPUI accessibility support is incomplete, document exact limitations and implement what is possible without fabricating coverage.

### 3.4 Typed state and coordinator (illustrative pseudocode, not a drop-in compilation claim)

```rust
// Design sketch: adapt to actual GPUI 0.2.2 and Shellumi types.
enum InspectorTarget {
    Host(HostId), Group(GroupId), Identity(IdentityId),
    Snippet(SnippetId), Tunnel(TunnelId), Workspace(WorkspaceId),
    Vault(VaultId), Provider(ProviderId), Session(SessionId),
}
enum InspectorMode { View, Create, Edit }
enum InspectorPresentation { Side, FocusedContent }
enum InspectorDraft {
    Host(HostDraft), Group(GroupDraft), Identity(IdentityDraft),
    Snippet(SnippetDraft), Tunnel(TunnelDraft),
    Workspace(WorkspaceDraft), Vault(VaultDraft), Provider(ProviderDraft),
    Session(SessionDraft),
}
struct InspectorState {
    vault_id: VaultId,
    target: InspectorTarget,
    mode: InspectorMode,
    draft: Option<InspectorDraft>,
    dirty: bool,
    validation: ValidationState,
    operation: AsyncOperationState,
    origin_focus: Option<FocusHandle>,
}
struct InspectorCoordinator {
    active: Option<InspectorState>, // the only inspector in this window
    pending_navigation: Option<NavigationRequest>,
}
```

Use stable object keys (never list indices) and a per-window coordinator; derive selection highlight from the coordinator or keep a single synchronized source of truth. If using an ID for a newly created object, switch to its persisted ID once save succeeds. When a backend result arrives, verify vault, target, generation/request token and current mode before applying it, preventing stale async completion from overwriting another object's inspector.

### 3.5 Common inspector components

Introduce reusable design-system elements or equivalents with native GPUI:

- `InspectorShell`, `InspectorHeader`, `InspectorSection`, `InspectorFooter`, `InspectorActionBar`.
- `Field`, `TextField`, `MultilineField`, `NumberField`, `SecureField`, `Select`, `Checkbox`, `Toggle`, `TokenPicker`, `FilePathPicker`, `ValidationMessage`.
- `ActionMenu`, `AnchoredPopover`, `OverflowMenu`, `Tooltip`, `InlineNotice`, `BusyIndicator`, `Toast`.
- `ConfirmationAlert`/`SecurityChallenge` as **one standardized exception**, never a collection of ad hoc management dialogs.
- `AppButton`, `IconButton`, `SegmentedControl`, `StatusIndicator`, `EmptyState`, `ErrorState`.

Build theme tokens for spacing, typography, control height, border, radius, states, focus, disabled, success, warning and danger. Prefer existing icon pack; ensure every referenced icon exists or use a stylistically compatible visible fallback. Avoid emoji mixed with product iconography, except when deliberately part of real user content.

### 3.6 Unsaved drafts and dangerous actions

The draft is the only mutable form state until Save. Cancel discards the draft after an appropriate guard; it must never modify underlying persisted data. A `dirty` flag must represent semantically changed values, not focus changes. Fields with inherited defaults should show origin clearly (`Inherited from group`, `Host override`, `Vault default`).

**Deletion** is not an editing mode: action from View -> standardized confirmation alert (explains affected dependencies and irreversible behavior) -> awaited backend operation -> only on success close/replace inspector and update list. On failure, retain context and report the real error. For group rename/deletion, verify linked host relationships, database operations, and server sync semantics before implementing.

Sensitive fields: avoid logging/debug-printing credentials, recovery codes and tokens, avoid screenshot fixtures with secrets, zeroize ephemeral secret buffers when feasible, and maintain existing keychain/vault policy. An inspector may show whether a secret is configured, not reveal it by default. Server-backed operations must enforce permissions in the backend; UI disabled states are not authorization.

## 4. Component migration matrix

| Feature/old surface | New default location | Key behaviors |
| --- | --- | --- |
| Host details + `host_editor` | `InspectorTarget::Host`, View/Create/Edit | All protocols, identities, group/tags, jump/port/remote desktop settings, validation, inherited settings |
| `group_details_inspector` + group menu | `InspectorTarget::Group`, View/Create/Edit | Never next to host inspector; save all real fields; nested group semantics; delete confirm |
| Identity editor (`identity_modal`) | Identity inspector | Public/private key metadata, secure secret references, fingerprint, key paths, tags |
| Snippet editor (`snippet_modal` when editing; `desktop/snippets.rs`) | Snippet inspector | Multiline code editor, variables, tags, preview, run action outside editor |
| Tunnel editor (`tunnel_modal`) | Tunnel inspector | Direction, local/remote endpoints, chosen host, validation, live status |
| Workspace editor (`desktop/workspaces.rs`) | Workspace inspector | Name, layout, members, duplication, launch, empty/large workspace handling |
| Vault creation and routine vault properties | Vault inspector | Vault type/server, encryption labels, opt-in sync; preserve immutable identifiers/security |
| Infrastructure/provider records (`infra_modal`) | Provider inspector + inventory page | Credentials/regions/accounts, editable properties; full-page for very large inventory review |
| Team/membership (`team_modal`, invite modal) | Member/invitation inspector + team page | Role changes, invitations, confirm revocation, permissions |
| Settings | Native settings page with standard form controls; item-level subsettings in inspector | Do not convert global navigation into a floating dialog |
| RDP/VNC session controls | Session inspector opened from compact toolbar | Display/audio/microphone/redirection/diagnostics advanced settings |
| Diagnostics/replay/shell integration | Dedicated pages with optional session/item inspector | These are workflows/tools, not ordinary object editors |
| Batch operations | Dedicated workflow page or compact bulk-actions inspector, depending on form complexity | Keep bulk progress and partial failures clear |
| Setup wizard | Dedicated step-based page | Never a 740px fixed modal; crash-safe recovery/re-entry and vault safety preserved |
| Sync conflict comparison | Full-page split diff and resolution workflow | Both sides visible, explicit decisions and safe cancel |
| Backup import/restore | Full-page preview/progress workflow with optional security alert | No accidental overwrite; dry-run preview where feasible; rollback preserved |
| Command palette | One transient command palette | Search and dispatch only, not editor |
| Delete/password/security challenges | Shared compact blocking alert | Short, consistent, and justified exception |
| SFTP file operations | File browser with selected-file inspector or inline rename; OS file picker | Avoid nested custom editor dialogs; batch operations remain appropriately full-page |

**Do not mechanically convert every legacy `*_modal` function into an inspector.** First categorize the user task: object management vs full-page workflow vs safety challenge vs short-lived picker. Rename files and extract modules by responsibility as part of the migration.

## 5. Global layout and navigation redesign

### 5.1 Unified top bar

- Retain vault identity and session tabs, but avoid forcing fixed 520px Quick Connect alongside ever-growing tabs. Allow search to shrink/collapse into an action entry point.
- Make the active tab always reachable. Support tab horizontal scroll, overflow list, pinned/active priority or equivalent. Do not delete the open tab if its visual tab is hidden in overflow.
- Use tooltips and predictable action grouping; secondary status belongs in a menu/inspector, not a permanent full sentence in a toolbar.
- Preserve existing per-vault, per-window session behaviors and tab shortcuts.

### 5.2 Hosts and groups

- Use one grid/list selection source of truth with coherent single-/double-click, keyboard navigation and context-menu behavior.
- The main Hosts page is navigational. Only one inspector may be displayed. A selected group does not remain open as a separate inspector after a host is selected.
- Preserve group breadcrumbs and filters, and support `Back`/breadcrumb navigation from focused inspector mode.
- Use responsive cards based on actual content region. Never calculate a nonzero but unusably narrow card or table; use fewer columns or a compact list.
- On list mode, keep Name/Connect primary. Secondary columns collapse by priority or scroll horizontally in an intentional bounded viewport.
- Correct group hierarchy: either persist and implement nested groups with sensible inheritance and recursive filtering, or suppress fields not actually supported. Make search/filter behavior match displayed grouping.

### 5.3 Other pages

- **SFTP:** adaptive dual-pane split; when one pane cannot meet minimum usable width, switch to a documented single-pane/switcher mode. Preserve transfer source/destination and queue state; do not silently swap them.
- **Snippets, Settings, Keychain, Workspaces, Tunnel pages:** collapse secondary navigation/list rails when available width is insufficient. The inspector is reserved for selected object properties. Never stack two different property editors.
- **Session view:** reduce explanatory banner clutter; user should see session canvas and a concise status/action affordance.
- **Search/menus:** follow trigger when window resized, flip when near edge, close on focus loss/action, and avoid obstructing security confirmation.

## 6. RDP/VNC: compact toolbar and robust canvas

### 6.1 Toolbar information architecture

Primary row should target approximately 40–48 logical px height and retain:

1. Host/session name (truncate safely) and a small connection indicator.
2. One `Display` button (Fit / 100% / optional Fill) showing current mode.
3. One `Keyboard` menu for Ctrl+Alt+Del, Win, Alt+Tab, with protocol-sensitive disabled/hidden options.
4. Fullscreen.
5. One overflow `More` button.
6. Clear Disconnect/Reconnect, protected from accidental action where relevant.

Under `More`, organize sections for clipboard/file transfer, shared drive, audio and microphone (RDP only), view-only, resolutions/monitors, technical diagnostics, and session preferences. Never render unavailable VNC features as if operational. Keep status compact; reveal FPS, RTT, color depth, dimensions and throughput in the session inspector/diagnostics. Show error/reconnecting state by compact contextual status or temporary banner, not an always-growing toolbar label.

Overflow must remain usable at 940px window width and smaller canvas region with sidebar/inspector. If space is tight, collapse labels to icons and move secondary controls to overflow. Do not allow toolbar contents to draw beyond the right edge.

### 6.2 Canvas measurement and input mapping

- Determine the actual **content rectangle** used to display the framebuffer via measured GPUI bounds. Convert window/global pointer coordinates into canvas-local coordinates. Keep logical-pixel vs device-pixel conversions explicit.
- Feed `translate_coordinates` the actual canvas width/height, framebuffer width/height and scale mode. For Fit, account for letterboxing; for Original, account for clipping and pan/scroll offsets; for Fill, calculate nonuniform scaling correctly.
- Ignore or reject events outside the active rendered frame. Test corners, center, edges, letterbox, aspect changes, zoom, and window resize.
- Implement pan/scroll for Original 100% when remote size exceeds viewport; clipping with no navigation is unacceptable.
- For display-control `Match Viewport`, measure the *remote canvas* (excluding app topbar/inspector), respect server bounds/minima, round dimensions only as required by protocol, coalesce rapid resize events, avoid feedback loops, and handle unsupported resize gracefully.
- Test dynamically resizing the Shellumi window while connected; make it unnecessary to reconnect to recover layout.

## 7. Detailed execution phases

Each phase is a separate, reviewable change set with tests, screenshots where appropriate, docs, and a short regression note. **Do not combine unrelated backend rewrites with UX migration.**

### Phase 0 — Baseline, inventory and acceptance specification

**Actions:** inspect HEAD and existing implementation; list every view, modal, menu, form, interaction, and state owner; map to one of the four surface types defined above. Capture baseline application behavior, screenshots if a real macOS runner is available, and failures. Create `docs/UI_UX_ARCHITECTURE.md`, `docs/UI_UX_MIGRATION.md`, `docs/UI_UX_QA.md`. Record a clear coverage matrix, including out-of-scope items. Build deterministic fixture data separate from production vaults.

**Exit:** all existing editor surfaces classified, no missing feature owner, baseline tests run and failures documented. Do not claim screenshots tested if the app was not launched.

### Phase 1 — Design tokens, layout rules, and shared primitives

**Actions:** centralize spacing/typography/control/button/focus tokens and standardized status behavior; create common buttons, fields, tooltips, anchored menus, alerts and inspector shell. Establish measured responsive layout policy and testable geometry helpers. Preserve existing theme until all components migrate, then remove dead variants.

**Exit:** demo/test fixture can render inspector with stable header/body/footer at all target window sizes and font scales; keyboard focus works; bounds checks pass.

### Phase 2 — Single inspector coordinator and state machine

**Actions:** implement typed target/mode/draft, a single window-local coordinator, guarded navigation and async generation tokens; integrate with application router. Remove separate persistent inspector flags after migration. Implement Save/Cancel, dirty confirmation, error retry, and focus restore.

**Exit:** switching host -> group -> host never shows two panels; dirty fields survive resize and are not silently discarded; async result for previous selection cannot overwrite current selection; switching vault/window preserves isolation.

### Phase 3 — Hosts and groups end to end

**Actions:** move host and group create/edit to inspector, eliminate `render_host_editor` modal, unify selection semantics in grid/list, fix menu affordances, verify group inheritance/parent/sync settings, fix deletion confirmation/error. Resolve fixed host grid/list sizing and focus on the strongest existing host flows.

**Exit:** full CRUD, re-open persistence, keyboard navigation, group/host switch, error handling, minimum-size geometry and database integration tests pass.

### Phase 4 — Other editable objects

**Actions:** migrate identities, snippets, tunnels, workspaces, vault properties, integrations/providers, team membership and invitations to shared inspector shell. Rework large inventory/analytics pages as page workflows, not custom editor dialogs. Preserve storage and permission enforcement.

**Exit:** no routine object editor exists outside inspector; feature parity verified against baseline; each object has create/edit/save/cancel/error/permissions tests.

### Phase 5 — Unified navigation and responsive content pages

**Actions:** topbar and tabs overflow; responsive settings/snippets/keychain pages; adaptive SFTP dual-pane; anchor-based popovers; consistent empty/loading/error states; keep active main content meaningful on ultrawides and small windows.

**Exit:** all supported window sizes pass bounds and access tests; no inaccessible control under zoom/text scaling; tab navigation deterministic.

### Phase 6 — RDP/VNC toolbar, session inspector and canvas correctness

**Actions:** implement compact toolbar and session inspector, protocol-sensitive overflow, fix viewport-relative pointer and dynamic resolution, add pan for Original zoom, test no toolbar clipping.

**Exit:** remote desktop fixture and live loopback tests pass for Fit/Fill/100%, resizing, reconnect, errors, and essential keyboard/file/audio actions. No regression to RDP/VNC protocol functionality.

### Phase 7 — Complex workflows and standard safety alerts

**Actions:** convert wizard and substantial restore/conflict/replay flows to dedicated pages where appropriate; consolidate security/password/delete confirmation into one standard blocking-alert component. Preserve crash recovery, cloud restoration, vault protection and import/export semantics.

**Exit:** no routine editor dialog remains; complex workflows work at minimum viewport with accessible Back/Cancel; existing vaults remain untouched unless an explicitly authorized operation succeeds.

### Phase 8 — Full native QA, accessibility, performance

**Actions:** run full test matrix, visual review, keyboard-only pass, screen reader evaluation where supported, font scale/high DPI, error/empty/long-content fixtures, all supported OSes, and performance profiles. Fix regressions before signoff. Document inaccessible platform capabilities precisely.

**Exit:** no open P0/P1 UI defects, no silent data loss, no inaccessible primary controls, all automated gates green with actual evidence, and manual acceptance recorded.

## 8. Best-practice QA strategy for GPUI 0.2.2

**Avoid a screenshot-only or browser-oriented test strategy.** Use four complementary layers:

### Layer A — Deterministic pure Rust unit/property tests (fast, every PR)

Test inspector state transitions, draft diff/dirty calculation, pending-navigation guards, validation rules, responsive layout decisions, menu placement arithmetic, column collapse, resize/debounce, and viewport-to-framebuffer coordinate translation. Include boundary tests, invalid inputs, long strings and property-based random scenarios where valuable. A pure function is preferable to UI-event-based tests when it expresses the same invariant.

### Layer B — GPUI render/interaction tests (every PR, macOS primary)

Use the pinned `gpui = 0.2.2` test APIs **as confirmed against source**: `#[gpui::test]`, `TestAppContext` / `VisualTestContext`, simulated resize and keystrokes and `debug_bounds`/measured bounding boxes when supported. The reference docs for that version show `simulate_resize`, `simulate_event`, `simulate_keystrokes` and `debug_bounds`; the agent must verify compile-time signatures in the locked dependency and add minimal test-only selectors/probes where required.

Assert not just state but **actual rendered geometry**: inspector within viewport, center >= defined minimum when split, primary buttons visible, no unintended overlapping panels, popovers inside window, action bar contents within bounds, focus returns, and all critical interactions reachable. Use unique stable selector/test IDs. Do not claim that existing GPUI ID fields alone supply a full semantic accessibility tree.

### Layer C — Controlled screenshot/visual regression (nightly + targeted PRs)

Where the pinned GPUI version and macOS rendering backend demonstrably support deterministic capture, save reviewed visual baselines by OS/render setup, scale factor, theme, and font. Otherwise use a controlled real-window capture harness with documented permissions and known caveats. Keep screenshots as evidence, not the only assertion; compare only meaningful sections with tolerances that account for font/antialias differences. Never silently regenerate baselines to make tests pass. Review every intentional baseline update.

Do not assume screenshot APIs from `gpui-pre 0.3.x` or forks exist in pinned `0.2.2`. If screenshot capture cannot be automated in this pin, mark it as an explicit gap, keep geometry tests gating and run manual/native captures until a separately justified upgrade.

### Layer D — Live application acceptance + manual expert review (milestone/release)

Run Shellumi on a supported macOS host (real GPUI/Metal), exercising mouse, scroll, keyboard and platform window resize; test Linux/Windows where available. Review task completion and accessibility, not just appearance. Use synthetic credentials and local protocol fixtures or isolated test servers—never production vaults, live user secrets or sensitive screenshots. Record environment details and failures.

### Test fixtures

- Empty vault; one host; 500 hosts/100 groups; deep hierarchy; long host names; Unicode, RTL text, emoji, narrow/long tags.
- Local + remote vaults in two different windows; offline, connecting, locked, sync-conflict and read-only cases; simultaneous window operations.
- Host protocols SSH/SFTP/Mosh/Telnet/Serial/RDP/VNC (when built), advanced inherited fields, identities, no identity, invalid and partial forms.
- RDP/VNC 4:3, 16:9, 21:9, portrait, huge remote frame, smaller-than-canvas frame, reconnecting, failure, 4K remote, varying FPS/latency and absent optional capabilities.
- Empty, huge, permission denied and interrupted SFTP file lists/queues; many tabs, split terminal panes, large scrollback.
- Unsaved form with changed values; backend delayed success/error; operation canceled; object deleted externally mid-edit; vault switched while request pending.

### Viewport matrix

All measurements are **logical GPUI pixels** unless marked otherwise. Minimum is currently 940×650 in `main.rs` window options; validate other main-window configurations. Run at least: `940×650`, `1024×768`, `1280×800`, `1440×900`, `1920×1080`, `2560×1440`, `3440×1440`, `3840×2160`. Test 1× and 2× scale factors where supported, UI font normal/large/extra-large, long-label localization fixtures, window maximized/restored, and side inspector collapsed/open.

### Automated geometry contract

For each rendered fixture, verify as applicable:

- `visible_rect(control)` is within the window content viewport and inside the appropriate scrolling viewport, or the control is reachable by a tested scroll/action.
- `inspector_count(window) <= 1`; no group+host side-by-side; active inspector matches current vault/target.
- When `Side`, `primary_content_width >= min_primary_width` and `inspector_width` within configured range; when `FocusedContent`, main workspace is replaced intentionally, not rendered under an overlapping editor.
- Modal alerts, selectors, tooltips and menus fit in the usable viewport and do not cover the only escape/confirm button.
- Essential toolbar actions remain accessible after narrow resize, font increase and session-status changes (via visible buttons or overflow menu).
- Screen point → canvas-local point → remote framebuffer mapping preserves valid coordinates and rejects letterbox points correctly.
- No duplicate form submission or success notification on failed backend operations.

### CI policy

Existing `.github/workflows/ci.yml` runs `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace` on `macos-15`. Extend it incrementally:

1. Keep existing checks mandatory; separate fast pure tests from layout/integration tests only when it improves diagnostics, not to reduce coverage.
2. Add GPUI component/layout regression job on macOS using deterministic fixtures; fail PR if geometry/interaction assertions fail.
3. Add optional controlled screenshot job when supported, uploading screenshots/diffs and a manifest with viewport/theme/DPI/commit SHA. Make acceptance criteria explicit before turning it into a required gate.
4. Add platform compilation and functional smoke jobs for Linux/Windows where dependencies and display environment support them. Clearly mark unsupported runtime scenarios as NOT TESTED, not green.
5. Build test summary with passed/failed/skipped counts, fixture ID and reproduction instructions. Required tests must not use `|| true`, ignored assertions, blanket `#[ignore]` or unreviewed snapshot updates.
6. Add documented manual approval checklist before release. CI passing alone does not prove UI quality.

## 9. Accessibility and usability requirements

- Keyboard-only workflows for inventory selection, inspector open/edit/save/cancel, navigation, popovers and command palette.
- Predictable Tab/Shift+Tab order. Visible focus indicators, accessible text labels and tooltips for icon-only controls, Escape/Enter semantics appropriate to context.
- No feature requires hover-only discovery; menu entries support keyboard and screen readers to the extent offered by pinned GPUI. Document supported/failing platform accessibility instead of assuming completion.
- Distinguish disabled from inactive using affordance and explanatory text. No status conveyed by color alone.
- Align platform shortcuts (macOS `Cmd`, Windows/Linux `Ctrl`) and handle focused terminal input safely so shortcuts do not steal remote keystrokes unexpectedly.
- Use compact but legible typography and adequate interactive hit targets. Test normal and enlarged fonts; text truncation must preserve full values via tooltip/inspector body.
- Feedback should be precise: `Saving…`, `Saved`, `Could not save — retry`, `Connected`, `Reconnecting`, `Disconnected`; avoid showing completion before confirmation.

## 10. Documentation and handoff expectations

For each phase, require the agent to update:

- `docs/UI_UX_ARCHITECTURE.md`: finalized interaction surface taxonomy and single-inspector state diagram.
- `docs/UI_UX_MIGRATION.md`: exact old->new file/component mapping, removals, current status, blockers and known limitations.
- `docs/UI_UX_QA.md`: commands, environment, fixture scenarios, test coverage, screenshots/diffs and pass/fail evidence.
- `docs/GUI_STATUS.md` and `README.md` if UI behavior/screenshots differ.
- New tests placed near state machines/components plus integration tests for end-to-end flows.

**Agent completion proof:** commit/PR references; scope and changed files; tests and CI runs with links; before/after screenshots from a real app where available; a coverage matrix mapping requirements IDs to tests; open issues and explicit unsupported environments. Do not claim visual approval based solely on source inspection or unit tests.

## 11. Definition of Done (all mandatory)

- One inspector per window; no routine object-management dialog and no stacked editors.
- Every ordinary create/view/edit flow uses the standard inspector and retains feature parity.
- Dirty edits are safeguarded on selection/page/vault/window changes; no loss on resize.
- All async save/delete/restore feedback reflects confirmed outcomes; destructive operations are guarded.
- Responsive shell, nav, tabs, list, settings, snippets, SFTP and remote desktop toolbar usable at the configured minimum and target sizes.
- RDP/VNC viewport mapping, dynamic resize, 100% panning and protocol-specific menus are correct.
- All visible actions functional or explicitly disabled/hidden with truthful UX.
- Common tokens/fields/buttons/menus/alerts are used; dead code and duplicate modal paths removed.
- Keyboard/focus and supported accessibility behavior verified with documented limitations.
- Automated pure Rust + GPUI interaction/geometry tests are in CI; visual/manual acceptance done and evidenced.
- Existing vault isolation, sync safety, backup recovery, SSH/SFTP, RDP/VNC and server integration regression suites pass.
- No unresolved P0 or P1 defect; unresolved P2/P3 work documented with owners and reproducible steps.

## 12. Implementation discipline

Begin at Phase 0; execute subsequent phases in order, but fix critical security/data-loss issues as early as practicable. Do not ask for further user design choices unless an actual product/business decision cannot be inferred from this specification. Use clear incremental commits, update docs after each, and finish the entire requested scope only after verification; otherwise report what remains and do not mark incomplete work as complete. Never weaken tests or security to make CI green.

### References

- Shellumi repository: https://github.com/MJKhaani/Shellumi
- Pinned GPUI 0.2.2 testing API: https://docs.rs/crate/gpui/0.2.2/source/src/test.rs
- Pinned GPUI 0.2.2 test context: https://docs.rs/crate/gpui/0.2.2/source/src/app/test_context.rs
- Pinned GPUI 0.2.2 layout API: https://docs.rs/crate/gpui/0.2.2/source/src/window.rs

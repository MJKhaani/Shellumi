> [!NOTE]
> **Historical Archive Record**
> - **Original Purpose:** Agent implementation brief for the Unified Inspector UI/UX redesign.
> - **Status:** Historical implementation plan; work completed in `v0.0.8-alpha`.
> - **Authoritative Replacement:** [UI/UX Guidelines](../../design/ui-ux-guidelines.md) & [Acceptance Matrix](../../qa/acceptance-matrix.md).

# Agent Implementation Instructions — Shellumi Unified Inspector and Full UI/UX QA

**Scope:** https://github.com/MJKhaani/Shellumi  
**Authoritative design specification:** `SHELLUMI_UNIFIED_INSPECTOR_UI_UX_PLAN.md` (supplied alongside this prompt)  
**Required verification inventory:** `SHELLUMI_UI_UX_QA_MATRIX.md` (supplied alongside this prompt)

## Mission

Implement a complete, consistent, responsive and accessible native GPUI desktop UI/UX across Shellumi. **Use one shared right-hand Inspector for all routine viewing, creation, editing and configuration**; do not mix object-editor dialogs and inspectors, and never stack group and host inspectors. Keep one inspector per window/vault. Make every view adapt to window resizing, including the smallest supported window, and redesign the RDP/VNC toolbar to be compact with advanced controls behind menus and the unified session inspector. Preserve all existing features, protocols, security and data behavior.

Read `AGENTS.md`, `AGENT.md`, the project docs, current implementation, the attached plan and test matrix before writing. Recheck current branch HEAD; the original source review used commit `a4132558e8127a177a92c81ec7554492d176ae03`. Follow repo conventions and do not overwrite unrelated changes.

## Mandatory decisions (do not improvise alternative UX)

1. **One InspectorCoordinator per window** with a single active typed target/mode/draft, scoped to the active vault. Group -> host selection **replaces** inspector content, not append another pane.
2. **Routine CRUD and settings = Inspector only.** All hosts, groups, identities, snippets, tunnels, workspaces, vault details, integrations/provider records, team membership/invitations and remote session properties use common Inspector shell/components.
3. **No generic floating edit dialogs.** Only standardized short blocking alerts for dangerous/security decisions; native file choosers; transient anchored menus/command palette; complex setup, restore, conflict resolution and other substantial workflows become dedicated full-page experiences.
4. **Responsive measured layout.** Preferred inspector width ~380–420 logical px; switch to *focused in-app content mode* if a side inspector would leave under ~560 logical px of primary workspace. Use actual available bounds. Sticky inspector header/footer, scrollable body, accessible close/back controls.
5. **Guard unsaved edits.** Save/Discard/Keep Editing on attempted selection, page, tab, vault or window change; async saves/errors must preserve drafts and respect request generation/target/vault. Don't falsely confirm success or bypass backend permission checks.
6. **Compact remote toolbar.** Host name/status, Display menu, Keyboard menu, Fullscreen, More and Disconnect/Reconnect. Move FPS/latency/technical info, audio/mic gain, drives, clipboard, file operations, resolutions and diagnostics to categorized menus/session inspector. Hide unsupported protocol actions; maintain feature parity.
7. **Fix remote canvas geometry.** Use measured canvas bounds, not framebuffer dimensions, for pointer coordinates; use actual canvas size for dynamic resolution requests. Add pan/scroll for Original/100%; test Fit/Fill/Original and letterboxing at different aspect ratios and sizes.
8. **No false affordances.** All visible controls (including group context actions and notification bell) must function, be truthfully disabled with explanation or be removed.
9. **Single design system.** Shared typography, spacing, controls, menus, fields, buttons, icons, notifications, focus states and alerts; minimize raw duplicated GPUI `div()` interactive widgets.
10. **Do not degrade security or backend behavior.** Preserve multi-vault/window isolation, sync/restore transaction safety, encrypted credentials, server permissions and the SSH/SFTP/RDP/VNC behavior.

## Execution process

**Phase 0: Audit and baseline.** Inventory all `crates/shellumi-app/src/{main.rs,app_render.rs,hosts_view.rs,desktop.rs,ui/*,desktop/*,sftp_ui/*,actions/*}` editor surfaces, all navigation and layout decisions; classify each editor as Inspector, full-page flow, safety alert or transient menu. Verify baseline behavior and existing tests. Document old -> new route. Start `docs/UI_UX_ARCHITECTURE.md`, `docs/UI_UX_MIGRATION.md`, and `docs/UI_UX_QA.md`.

**Phase 1: Shared design/layout primitives.** Build consistent controls, InspectorShell, anchored menu, safety alert and responsive geometry utilities. Verify layout behavior and focus before migration.

**Phase 2: Inspector state machine.** Implement coordinator, draft models, asynchronous operation states, guarded navigation, window/vault isolation and focused presentation on narrow windows. Test everything before migrating the remaining editors.

**Phase 3: Hosts and groups.** Refactor group/host selection and editors to single Inspector; remove double-panel rendering and fixed-size assumptions; fix inert context actions, group field persistence and destructive action correctness. Verify all protocol-specific host fields.

**Phase 4: Remaining object management.** Convert identities, snippets, tunnels, workspaces, vaults, providers/integrations, members/invites and session settings to the same inspector. Do not implement a bespoke editor per feature.

**Phase 5: Remaining responsive pages and navigation.** Fix top-bar Quick Connect/tabs overflow, host list columns, settings/snippets/keychain, SFTP split, menu anchors, text scaling, error/empty/loading states and keyboard navigation.

**Phase 6: Remote desktop UX.** Compact protocol-aware toolbar, unified session inspector, actual canvas measurement, correct pointer translation, dynamic resize logic, 100% panning. Regression-test remote I/O and session persistence.

**Phase 7: Complex workflows.** Route first-run wizard, recovery, restore, large conflict diff and similar complex tasks to dedicated pages; consolidate safety/password prompts into one standard alert; verify crash recovery/restore protections.

**Phase 8: Native QA and polish.** Run all QA scenarios and viewport matrix with real GPUI render/interaction tests, target-platform smoke tests and actual visual review. Fix remaining P0/P1, remove dead old editor routes and update docs/README/screenshots.

Perform work in reviewable incremental commits/PRs. Each phase must have code, accompanying tests, docs and a pass/fail report before proceeding. Do not skip ahead to a cosmetic toolbar change while the foundational inspector/layout problem remains.

## Mandatory test strategy

- Pure Rust unit/property tests for coordinator invariants, selection/draft transitions, validation, menu positioning, responsive width, async out-of-order results and remote coordinate transforms.
- Native GPUI `0.2.2` tests using **verified APIs** (`#[gpui::test]`, `TestAppContext`/`VisualTestContext`, simulate resize/keyboard/input and rendered bounds where supported). Do not copy GPUI 0.3.x/fork-only APIs without confirming they exist in the pinned version.
- Controlled visual regression with reviewed baselines/screenshots where supported; if deterministic capture is unavailable, record the limitation and use real macOS screenshots plus mandatory bounds assertions and manual review.
- Real native macOS UI acceptance and Linux/Windows build/smoke when feasible; never equate passing `cargo test` with visual acceptance.
- Test `940×650`, `1024×768`, `1280×800`, `1440×900`, `1920×1080`, `2560×1440`, `3440×1440`, `3840×2160`, different text sizes and scale factors; test all supported pages and inspector states.
- Use synthetic test vaults, fake secrets, offline/local protocol fixtures; no production databases, accounts, recovery codes or sensitive screenshots.
- Add required checks to `.github/workflows/ci.yml` without weakening existing `fmt`, Clippy, workspace tests and release build safety. Document unsupported native runtime test platforms separately.

## Quality gates and finish condition

Every requirement in the plan and every critical/major case in the QA matrix must map to a passing automated assertion or an explicitly recorded, human-reviewed native acceptance result. There must be **zero stacked inspectors**, **zero routine editor dialogs**, **zero inaccessible primary controls at minimum size**, **zero silent draft loss**, **zero false success after backend failure**, and **zero clipping of the remote toolbar's essential controls**. The actual canvas-to-remote input mapping must be verified. Preserve full existing feature parity and passing protocol/storage/server suites.

Update project docs continuously, and end with: implemented-phase checklist, changed paths, commit hashes, exact test commands/results, CI links, screenshots/evidence when captured, unresolved limitations and reproducible known issues. **Do not mark the project done if any P0/P1 UI defect or required test remains unverified.** Do not ask me to choose between different edit/inspector/dialog patterns: the decision is fixed here.

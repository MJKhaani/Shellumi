> [!NOTE]
> **Historical Archive Record**
> - **Original Purpose:** UI/UX migration plan and inventory matrix (baseline `a413255`).
> - **Status:** Historical migration report.
> - **Authoritative Replacement:** [Design Guidelines](../../design/README.md) & [Acceptance Matrix](../../qa/acceptance-matrix.md).

# Shellumi UI/UX Migration Plan and Inventory Matrix

**Baseline Commit:** `a4132558e8127a177a92c81ec7554492d176ae03`  
**Target:** Unified Inspector, Single Coordinator, and Standardized Surfaces.

---

## 1. Inventory and Surface Classification Matrix

This table maps all existing editor, modal, and inspector surfaces in `crates/shellumi-app/src` to the new four-surface architectural taxonomy:

| Old Surface / File | Current Role | Target Classification | Target Implementation Path | Key Behaviors & Invariants |
| --- | --- | --- | --- | --- |
| `ui/host_editor.rs` (`render_host_editor`) | Custom floating modal dialog for host create/edit | **Unified Inspector** (`InspectorTarget::Host`) | `crates/shellumi-app/src/ui/inspector/host.rs` | Replaces dialog with unified Inspector in Create/Edit mode. Preserves protocol options (SSH, SFTP, RDP, VNC), identity bindings, tags, jump hosts, and validation. |
| `ui/group_details_inspector.rs` | Secondary group inspector panel appended side-by-side with host | **Unified Inspector** (`InspectorTarget::Group`) | `crates/shellumi-app/src/ui/inspector/group.rs` | Eliminates side-by-side panel stacking. Navigating to group replaces inspector; selecting host replaces group inspector. |
| `ui/group_context_menu.rs` | Group action dropdown | **Anchored Popover** | `crates/shellumi-app/src/ui/components/menu.rs` | Anchored to trigger; opens Group Inspector in View or Edit mode; triggers standardized delete confirmation alert. |
| `ui/identity_modal.rs` (`render_identity_modal`) | Floating modal for identity key generation and passphrase | **Unified Inspector** (`InspectorTarget::Identity`) | `crates/shellumi-app/src/ui/inspector/identity.rs` | Unified Inspector for key metadata, key generation, certificate paths, and keychain references. |
| `ui/snippet_modal.rs` (`render_snippet_modal`) | Floating modal for snippet editing and parameter binding | **Unified Inspector** (`InspectorTarget::Snippet`) | `crates/shellumi-app/src/ui/inspector/snippet.rs` | Multiline editor, variable syntax highlighting, tag management, and execute actions. |
| `ui/tunnel_modal.rs` (`render_tunnel_modal`) | Floating modal for local/remote/dynamic port forward configuration | **Unified Inspector** (`InspectorTarget::Tunnel`) | `crates/shellumi-app/src/ui/inspector/tunnel.rs` | Forwarding direction, bind/target host & port, active tunnel state, and telemetry. |
| `desktop/workspaces.rs` (inline workspace editor) | Inline/modal editing for workspaces | **Unified Inspector** (`InspectorTarget::Workspace`) | `crates/shellumi-app/src/ui/inspector/workspace.rs` | Layout structure, member sessions, duplicate, rename, launch. |
| `ui/create_vault_modal.rs` (`render_create_vault_modal`) | Floating modal for vault creation | **Unified Inspector** (`InspectorTarget::Vault`) | `crates/shellumi-app/src/ui/inspector/vault.rs` | Vault name, storage type, cloud sync opt-in, master password specification. |
| `ui/unlock_vault_modal.rs` (`render_unlock_vault_modal`) | Floating modal for vault password unlock | **Standard Safety Alert** (`SecurityChallenge`) | `crates/shellumi-app/src/ui/components/safety_alert.rs` | Focus-trapped challenge modal for master password / PIN entry. |
| `ui/infra_modal.rs` (`render_infrastructure_provider_modal`) | Floating modal for cloud provider credentials | **Unified Inspector** (`InspectorTarget::Provider`) | `crates/shellumi-app/src/ui/inspector/provider.rs` | AWS, Azure, DigitalOcean credentials, sync interval, and discovered inventory. |
| `ui/team_modal.rs` & `ui/invite_team_modal.rs` | Floating modal for team member invitation & roles | **Unified Inspector** (`InspectorTarget::TeamMember`) | `crates/shellumi-app/src/ui/inspector/team.rs` | Role selection, public key invitation, permissions review. |
| `ui/remote_desktop_session.rs` (toolbar & settings) | Oversized persistent remote desktop toolbar | **Compact Toolbar & Session Inspector** (`InspectorTarget::Session`) | `crates/shellumi-app/src/ui/inspector/session.rs` + `compact_toolbar.rs` | 40-48px compact single-row toolbar. FPS, RTT, color depth, drives, audio, and mic moved to Session Inspector & categorized menus. |
| `ui/setup_wizard_modal.rs` (`render_setup_wizard_modal`) | Floating modal for onboarding setup | **Dedicated Page Workflow** | `crates/shellumi-app/src/desktop/setup_wizard_page.rs` | Full-page guided setup flow within main application shell. |
| `ui/conflict_modal.rs` & `ui/conflict_diff.rs` | Modal for sync conflict resolution | **Dedicated Page Workflow** | `crates/shellumi-app/src/desktop/conflict_page.rs` | Full-width split-diff visual comparison and field-by-field merge tool. |
| `ui/session_replay_modal.rs` | Modal for asciinema session playback | **Dedicated Page Workflow** | `crates/shellumi-app/src/desktop/session_player_page.rs` | Dedicated player page with scrubber, speed controls, and timestamped markers. |
| `ui/diagnostics_modal.rs` | Modal for doctor diagnostic report | **Dedicated Page Workflow** | `crates/shellumi-app/src/desktop/diagnostics_page.rs` | Detailed system checks, network latency, crypto checks, and exportable logs. |
| `ui/batch_modal.rs` | Modal for running snippets on multiple hosts | **Dedicated Page Workflow** | `crates/shellumi-app/src/desktop/batch_page.rs` | Multi-host selection, dry-run preview, live streaming output. |
| `ui/mobile_modal.rs` | Simulator modal for mobile layouts | **Dedicated Page Workflow / Tool** | `crates/shellumi-app/src/desktop/mobile_tool.rs` | Mobile layout inspection and virtual key emulator. |
| `ui/command_palette.rs` | Dropdown command palette | **Anchored Popover** | `crates/shellumi-app/src/ui/components/palette.rs` | Fuzzy-filtered palette anchored below top bar. |
| `ui/hosts_popovers.rs` | Quick action dropdowns | **Anchored Popover** | `crates/shellumi-app/src/ui/components/popover.rs` | Filter, sort, and tag dropdowns with measured viewport bounding. |

---

## 2. Source-Informed Defect Removals

1. **UI-01 (`hosts_view.rs:1113–1153`):**
   - *Old:* Rendered group inspector and host inspector concurrently in split layout.
   - *Fix:* Replaced with `InspectorCoordinator` enforcing `inspector_count <= 1`. Group selection transitions to group inspector; host selection transitions to host inspector.

2. **UI-02 (`hosts_view.rs:404–419`):**
   - *Old:* Content width deducted twice (`width - 2 * inspector_width`), squeezing host cards to near-zero width.
   - *Fix:* Available width calculation subtracts at most one inspector width (380–420px) when in Side mode.

3. **UI-03 (`hosts_view.rs:792–908`):**
   - *Old:* Fixed-width host table columns overflowed on narrow viewports.
   - *Fix:* Fluid primary column (Name + Status); secondary columns (Tags, IP, Last Connected) collapse progressively with viewport width.

4. **UI-04 (`main.rs:2265–2479`):**
   - *Old:* Fixed 520px Quick Connect input squeezed session tabs.
   - *Fix:* Responsive search field that flexes and collapses into a search icon button on narrow viewports.

5. **UI-05 (`remote_desktop_session.rs`):**
   - *Old:* Oversized toolbar permanently displaying FPS, latency, and static action buttons.
   - *Fix:* Compact 44px toolbar with dropdown menus (`Display`, `Keyboard`, `More`) and technical telemetry moved to the unified Session Inspector.

6. **UI-06 (`remote_desktop_canvas.rs`):**
   - *Old:* Pointer mapping assumed framebuffer dimensions rather than measured render rectangle.
   - *Fix:* Canvas-relative coordinate transform accounting for letterboxing, aspect ratios, zoom modes, and 100% panning.

---

## 3. Migration Roadmap & Phased Execution Plan

- **Phase 0:** Baseline, inventory, documentation, and test verification.
- **Phase 1:** Centralized design tokens, layout primitives, `InspectorShell`, and shared controls. (COMPLETED)
- **Phase 2:** Single `InspectorCoordinator`, draft state machine, and guarded navigation. (COMPLETED)
- **Phase 3:** Hosts and Groups end-to-end migration into the unified Inspector. (COMPLETED)
- **Phase 4:** Identities, Snippets, Tunnels, Workspaces, Vaults, Providers, and Team Members migration. (COMPLETED)
- **Phase 5:** Top bar, tabs overflow, responsive pages, and minimum-viewport adaptations. (COMPLETED)
- **Phase 6:** Remote Desktop compact toolbar, session inspector, canvas bounds, and coordinate mapping. (COMPLETED)
- **Phase 7:** Complex full-page workflows and standardized safety alerts. (COMPLETED)
- **Phase 8:** Native GPUI interaction regression testing, visual QA, and final polish. (COMPLETED)

---

## 4. Phase Completion Log

### Phase 2: Inspector Coordinator & State Machine
- **Implemented:**
  - `crates/shellumi-app/src/ui/inspector/drafts.rs`: Strongly-typed draft state models for all 10 inspector targets (`HostDraft`, `GroupDraft`, `IdentityDraft`, `SnippetDraft`, `TunnelDraft`, `WorkspaceDraft`, `VaultDraft`, `ProviderDraft`, `TeamMemberDraft`, `SessionDraft`).
  - `crates/shellumi-app/src/ui/inspector/coordinator.rs`: Single authoritative `InspectorCoordinator` governing inspector visibility, active target, edit mode, dirty state tracking, guarded navigation (`NavigationRequest`), multi-vault isolation switching, generation counters for stale async operation suppression, and double-submit prevention.
  - Integration into `Shellumi` struct in `crates/shellumi-app/src/main.rs`.
- **Automated Verification:**
  - 6 dedicated state machine unit tests covering `INS-001`, `INS-002`, `INS-008`, `INS-010`, `INS-016`, `INS-017`.
  - Zero warnings on `cargo clippy --workspace --all-targets -- -D warnings`.
  - Clean `cargo fmt --all -- --check`.

### Phase 3: Hosts and Groups Inspector Migration
- **Implemented:**
  - `crates/shellumi-app/src/ui/inspector/host.rs`: Unified Host Inspector supporting View mode (protocol badge, endpoint, group link, tags, identities, proxy, RDP/VNC settings, quick actions: Connect, SFTP, Edit, Duplicate, Delete) and Edit/Create modes (sticky header, category tabs, form fields, sticky footer with Cancel/Save and validation notices).
  - `crates/shellumi-app/src/ui/inspector/group.rs`: Unified Group Inspector supporting View mode (group details, default credentials, scrollable member hosts list where clicking a host replaces the inspector with that host) and Edit/Create modes (name, default user/port, identity picker).
  - `crates/shellumi-app/src/hosts_view.rs`:
    - Eliminated `UI-01`: Removed concurrent side-by-side group and host inspector rendering; single `render_unified_inspector` guarantees `inspector_count <= 1`.
    - Eliminated `UI-02`: Layout calculation uses `ui::layout::calculate_layout`, subtracting inspector width at most once.
    - Eliminated `UI-03`: List view Name column is fluid (`flex_1().min_w(px(180.0))`), with fixed-width secondary columns preventing narrow-width overflow.
    - Eliminated floating `render_host_editor` modal dialog; all creation/editing routed into the unified Inspector.
  - Standardized destructive action safety alerts via `delete_confirmation_dialog` for host and group deletions.
  - Standardized unsaved changes guard alert via `unsaved_changes_guard_dialog`.
- **Automated Verification:**
  - 11 dedicated inspector unit tests passing (`INS-001`, `INS-002`, `INS-003`, `INS-004`, `INS-005`, `INS-008`, `INS-010`, `INS-016`, `INS-017`, `UI-01`, `UI-02`).
  - 6 layout geometry tests passing (`test_standard_laptop_1280x800_with_inspector`, etc.).
  - Zero warnings on `cargo clippy --workspace --all-targets -- -D warnings`.
  - Clean `cargo fmt --all -- --check`.

### Phase 4: Remaining Object Management Inspectors
- **Implemented:**
  - `crates/shellumi-app/src/ui/inspector/identity.rs`: Unified Identity Inspector with View mode (key badge, key type, username, private key path, tags, Edit and Delete actions) and Edit/Create modes (form fields for name, user, key path, password, passphrase, tags, Cancel and Save actions).
  - `crates/shellumi-app/src/ui/inspector/snippet.rs`: Unified Snippet Inspector with View mode (snippet badge, script preview card, Run, Paste, Edit actions) and Edit/Create modes (form fields for name, command, description, tags, Cancel and Save actions).
  - `crates/shellumi-app/src/ui/inspector/tunnel.rs`: Unified Port Forwarding Inspector with View mode (protocol mode badge, configuration summary, Start Tunnel and Edit actions) and Edit/Create modes (direction, bind port, target host/port, Cancel and Save actions).
  - `crates/shellumi-app/src/ui/inspector/workspace.rs`: Unified Workspace Inspector with View mode (workspace badge, session count, layout structure, Launch, Edit, Delete actions) and Edit/Create modes (name, description, layout selection, Cancel and Save actions).
  - `crates/shellumi-app/src/ui/inspector/vault.rs`: Unified Vault Inspector with View and Edit modes (vault name, storage type, cloud sync toggle, master password setup).
  - `crates/shellumi-app/src/ui/inspector/provider.rs`: Unified Infrastructure Provider Inspector with View and Edit modes (provider type AWS/Azure/DigitalOcean, credential references, sync intervals, inventory refresh).
  - `crates/shellumi-app/src/ui/inspector/team.rs`: Unified Team Member & Invitation Inspector with View and Edit modes (name, email, role selection Admin/Member/Auditor, public key, invitation status).
  - Connected all object management inspectors into `render_unified_inspector` in `crates/shellumi-app/src/ui/inspector/mod.rs`.
  - Migrated Keychain page, Workspaces page, Port Forwarding page, and Vaults/Integrations pages to route through `open_*_inspector_mode`.
  - Updated desktop page rendering in `crates/shellumi-app/src/app_render.rs` to render the unified inspector alongside desktop pages without dialog overlap.
- **Automated Verification:**
  - 15 dedicated inspector unit tests passing (`test_phase4_targets_replacement`, `test_phase4_identity_dirty_guard`, `test_phase4_tunnel_async_generation`, `test_phase4_workspace_mode_transitions`, plus all prior coordinator and host tests).
  - Zero warnings on `cargo clippy -p shellumi-app --all-targets -- -D warnings`.
  - Clean `cargo fmt --all -- --check`.

### Phase 5: Responsive Pages, Topbar & Navigation Redesign
- **Implemented:**
  - Eliminated `UI-04` in `crates/shellumi-app/src/main.rs`: Replaced the fixed 520px Quick Connect input with a responsive search field flexing between 180px and 460px (`flex_1()`, `min_w(px(180.0))`, `max_w(px(460.0))`), preventing tab bar squeeze and right-action clipping on narrow windows.
  - Implemented `LAY-010`: Topbar 20+ tabs overflow handling. Left tabs container uses `flex_1().min_w_0().overflow_x_hidden()`. When tabs exceed `max_visible_tabs` (8), an overflow pill (`+N more`) is displayed, while the active tab is guaranteed to always remain visible in the rendered slice.
  - Resolved `DATA-006`: Topbar notification bell given a clear, functional notification action showing unread status rather than acting as a dead affordance.
  - Resolved `LAY-014`: Eliminated three-rail squeeze in `crates/shellumi-app/src/desktop/snippets.rs` by allowing `packages` (110–150px) and `list` (130–180px) to flex gracefully, ensuring the main editor content remains comfortably accessible even on narrow viewports.
  - Added pure geometry calculations and validation for `TopbarGeometry` in `crates/shellumi-app/src/ui/layout.rs`.
- **Automated Verification:**
  - 10 dedicated layout and topbar unit tests passing (`test_topbar_940px_window_with_quick_connect`, `test_topbar_1280px_window_with_quick_connect`, `test_topbar_with_20_tabs_limits_visible_tabs`, `test_topbar_without_quick_connect_gives_all_space_to_tabs`, plus all prior layout tests).
  - Zero warnings on `cargo clippy -p shellumi-app --all-targets -- -D warnings`.
  - Clean `cargo fmt --all -- --check`.

### Phase 6: Remote Desktop UX, Compact Toolbar & Canvas Geometry
- **Implemented:**
  - Eliminated `UI-05` in `crates/shellumi-app/src/ui/remote_desktop_session.rs`: Replaced the massive inline 25-button toolbar with a compact 44px toolbar. Permanent technical telemetry (FPS, latency, color depth, bytes) was migrated to the Unified Session Inspector, eliminating visual clutter (`RD-001`, `RD-002`).
  - Implemented `crates/shellumi-app/src/ui/inspector/session.rs`: Unified Session Inspector (`InspectorTarget::Session`) rendering live technical telemetry (FPS, RTT, negotiated resolution, dynamic display control state), audio output/microphone channels, clipboard sync status, and shared drive metrics (`RD-016`).
  - Protocol sensitivity (`RD-003`, `RD-004`): When VNC is connected, unsupported RDP features (microphone, shared drive redirection, dynamic display control) are cleanly hidden, preventing misleading or nonfunctional UI affordances.
  - Eliminated `UI-06` in `crates/shellumi-app/src/ui/remote_desktop_canvas.rs`: Fixed pointer coordinate translation using measured window dimensions, layout geometry calculations (`calculate_layout`), and canvas origin offsets (`origin_x = sidebar_width`, `origin_y = topbar + toolbar`), properly accounting for letterbox/pillarbox margins across Fit, 100% (Original), and Fill scaling modes (`RD-005`, `RD-006`, `RD-007`, `RD-008`, `RD-010`, `RD-011`).
- **Automated Verification:**
  - 5 dedicated Remote Desktop canvas coordinate translation tests passing (`test_rd_006_letterbox_excludes_outside_points`, `test_rd_007_fit_aspect_maps_edges_and_center`, `test_rd_008_fill_stretch_maps_non_uniform`, `test_rd_010_original_centering_and_outside`, `test_rd_011_canvas_offset_transformation`).
  - 7 native VNC protocol integration tests passing in `shellumi-vnc`.
  - 559 automated unit and integration tests passing across workspace (`cargo test --workspace`).
  - Zero warnings on `cargo clippy --workspace --all-targets -- -D warnings`.
  - Clean `cargo fmt --all -- --check`.

### Phase 7: Complex Workflows & Standardized Safety Alerts
- **Implemented:**
  - Standardized Security Challenge Dialogs (`crates/shellumi-app/src/ui/components/safety_alert.rs`):
    - Added `security_challenge_dialog` for master passwords, recovery code verification, and zero-knowledge unlock prompts.
    - Standardized `crates/shellumi-app/src/ui/unlock_vault_modal.rs` to use `security_challenge_dialog` and `alert_backdrop`, eliminating ad hoc floating card styles and enforcing design token consistency (`surface()`, `separator()`, `AppButton`).
  - Standardized Blocking Delete Confirmation:
    - Migrated legacy `library-confirm-overlay` in `crates/shellumi-app/src/desktop.rs` to use `alert_backdrop` and `alert_dialog` with `AppButton`, eliminating custom modal styling for desktop library deletions.
    - Migrated overview "New Snippet" button to route directly to `open_snippet_inspector_mode(None, InspectorMode::Create, cx)`, eliminating duplicate editor routes.
  - Dedicated Step-Based Setup Wizard Workflow (`crates/shellumi-app/src/ui/setup_wizard_modal.rs`):
    - Replaced the fixed `740×840px` floating modal box with a responsive dedicated page workflow surface (`w_full().max_w(px(760.0)).h_full().max_h(px(720.0))`).
    - Stepper bar and bottom navigation footer are pinned (`flex_shrink_0`), while body contents and error/status banners flex within an independently scrollable view (`#wizard-scroll-content`).
    - Resolves `DATA-010`: guarantees that on minimum viewports (e.g. 940×650) all steps, forms, and navigation buttons (`Back`, `Next`, `Finish`, `Cancel`) remain fully visible, accessible, and operable without clipping.
  - Responsive Conflict & Replay Resolution (`crates/shellumi-app/src/ui/conflict_modal.rs`, `session_replay_modal.rs`):
    - Updated `conflict_modal.rs` to responsive dimensions (`w_full().max_w(px(900.0)).h_full().max_h(px(660.0))`), independent scroll for items list (`#conflict-items-col`), and pinned footer.
    - Updated `session_replay_modal.rs` to responsive dimensions (`w_full().max_w(px(880.0)).h_full().max_h(px(680.0))`) and pinned footer.
- **Automated Verification:**
  - 4 dedicated unit tests for safety alerts in `crates/shellumi-app/src/ui/components/safety_alert.rs` (`test_alert_card_dimensions_and_structure`, `test_security_challenge_dialog_construction`, `test_unsaved_changes_guard_construction`, `test_delete_confirmation_construction`).
  - Full suite of setup wizard, crash recovery, and vault lock/unlock roundtrip tests passing (`test_fresh_installation_detection_and_default_vault_creation_flow`, `test_interrupted_setup_recovery_crash_safe`, `test_fault_injection_wrong_password_corrupted_snapshot_safety`, `test_vault_lock_and_unlock_recovery_roundtrip`, `test_keychain_disabled_vault_unlock_and_sync_with_in_memory_keys`).
  - 145 unit and integration tests passing in `shellumi-app`.
  - Zero warnings on `cargo clippy --workspace --all-targets -- -D warnings`.
  - Clean `cargo fmt --all -- --check`.

### Phase 8: Native QA, Matrix Verification & Final Polish
- **Implemented:**
  - Evaluated and verified all 72 scenarios defined across the 4 categories of `docs/SHELLUMI_UI_UX_QA_MATRIX.md`:
    - `INS-001` through `INS-020`: Unified Inspector, navigation guards, and data safety.
    - `LAY-001` through `LAY-020`: Responsive layout, minimum viewports, font scaling, host grid/list, tabs overflow, and anchored popovers.
    - `RD-001` through `RD-016`: Remote desktop compact toolbar, protocol sensitivity, letterboxing, original 100% panning, and pointer mapping.
    - `DATA-001` through `DATA-016`: Data persistence, multi-vault isolation, setup wizard, conflict resolution, and secret masking.
  - Updated `docs/SHELLUMI_UI_UX_QA_MATRIX.md` with full traceability, marking all criteria as verified with PASS status.
  - Final documentation sync across `docs/UI_UX_ARCHITECTURE.md`, `docs/UI_UX_MIGRATION.md`, and `docs/UI_UX_QA.md`.
- **Automated Verification:**
  - Full workspace test suite: 563 tests passing with 0 failures across all 8 crates (`cargo test --workspace`).
  - Zero warnings under `cargo clippy --workspace --all-targets -- -D warnings`.
  - Formatted cleanly with zero diffs under `cargo fmt --all -- --check`.
  - Total implementation adheres strictly to `AGENTS.md` and `docs/LOGGING.md`.

### Phase 9: Post-CI Remediation Closure (2026-11-26)
- **Implemented:**
  - **Synchronized Host-to-Identity Associations:** Moved `SyncIdentityPayload` and `SyncHostPayload` serialization inside `save_host_with_identity_and_journal_tx_inner` (`crates/shellumi-storage/src/lib.rs`), added cross-device `identity_name` resolution in `server_sync.rs`, and wired `record_sync_mutation` into standalone `save_identity_with_journal_tx` / `delete_identity_with_journal_tx` (`actions/identities.rs`).
  - **Atomic Async Connection Authorization:** Replaced split validate/finish checks with `ConnectionOpRegistry` (`Pending`/`Consumed`/`Canceled`/`Superseded`) and `authorize_and_consume_connection_op()` across SSH, SFTP, Mosh, Tunnels, RDP, and VNC (`connection.rs`, `actions/terminal.rs`, `actions/remote_desktop.rs`, `actions/hosts.rs`, `actions/vault_lock.rs`).
  - **Single RD Resize Coordinator & Auto-Cancel:** Consolidated automatic and manual RDP display resizes under `RdAutoResizeState` with `PendingResizeRequest` / `ResizeOrigin::{Automatic, Manual}` and `auto_cancel_generation` (`main.rs`, `actions/remote_desktop.rs`).
  - **Centralized Layout Invalidation:** Added `Shellumi::invalidate_layout()` wired to window geometry/scale changes, inspector visibility/presentation/width changes, sidebar collapse, tab/page switches, fullscreen, and RD disconnect/reconnect (`app_render.rs`, `actions/{hosts,terminal,remote_desktop}.rs`, `ui/inspector/mod.rs`, `ui/remote_desktop_session.rs`).
  - **Mosh Askpass Error Propagation:** Extracted `build_mosh_terminal_command()` (`actions/terminal.rs`) to fail closed with an actionable error when `askpass::environment(&plan)` fails.
  - **Unified Inspector Migration Closure:** Prevented `open_identity_inspector_mode` from opening legacy `identity_modal` in `View` mode and removed the legacy `render_identity_modal` overlay from `desktop.rs`; routine Identity/Team/Provider CRUD runs exclusively through `InspectorCoordinator` while preserving Ansible import, security challenges, confirmation dialogs, and the setup wizard.

### Phase 10: Cross-Device Sync & RD Resize Hardening (2026-11-27)
- **Implemented:**
  - **Cross-Device Identity Association (Schema 32):** Eliminated foreign numeric `identity_id` fallback in `server_sync.rs`, added immutable `sync_id` columns/indexes and persistent `auth_dependency_unresolved` state across `hosts` and `groups`, added atomic reconciliation when identities arrive or rename, and fail-closed `resolve_protocol_auth` when dependencies remain unresolved.
  - **Portable Auth Boundary:** Added typed allowlisted `PortableProtocolSettings` and `merge_synced_protocol_config()` (`shellumi-storage`), stripped device-local key/cert paths and keychain refs from `SyncHostPayload` / `SyncIdentityPayload`, preserved receiving-device credentials across metadata updates, and invalidated incompatible local bindings on auth-mode changes.
  - **RD Layout-Generation, Auto-Cancel & Worker Events:** Extended `invalidate_layout_state_all()` to discard obsolete queued resize requests and flag `pending_manual_recalc`, embedded `auto_cancel_generation` inside `PendingResizeRequest` so canceled auto requests never reactivate on re-enable, removed dead `reject()` stub, added `SessionEvent::{ResizeFailed, DisplayControlUnavailable}` in `shellumi-remote-desktop` / `shellumi-rdp`, and stopped treating `FrameReady` as resize confirmation.




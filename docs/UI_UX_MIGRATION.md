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
- **Phase 1:** Centralized design tokens, layout primitives, `InspectorShell`, and shared controls.
- **Phase 2:** Single `InspectorCoordinator`, draft state machine, and guarded navigation.
- **Phase 3:** Hosts and Groups end-to-end migration into the unified Inspector.
- **Phase 4:** Identities, Snippets, Tunnels, Workspaces, Vaults, Providers, and Team Members migration.
- **Phase 5:** Top bar, tabs overflow, responsive pages, and minimum-viewport adaptations.
- **Phase 6:** Remote Desktop compact toolbar, session inspector, canvas bounds, and coordinate mapping.
- **Phase 7:** Complex full-page workflows and standardized safety alerts.
- **Phase 8:** Native GPUI interaction regression testing, visual QA, and final polish.

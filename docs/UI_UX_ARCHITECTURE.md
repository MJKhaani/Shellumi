# Shellumi UI/UX Architecture Specification

**Status:** Authoritative  
**Target Platform:** Native macOS / Linux / Windows desktop with GPUI (`gpui = "=0.2.2"`)  
**Design Paradigm:** Unified Right-Hand Inspector, Standardized Safety Alerts, Dedicated Page Workflows, and Compact Protocol Toolbars.

---

## 1. Architectural Invariants

1. **Single Inspector Coordinator Per Window:**
   Each application window contains at most **one** active Inspector owned by a window-local `InspectorCoordinator`. An inspector is a single cohesive surface with a sticky header, independently scrollable body, and fixed action footer. Navigating between objects (e.g. from a group to a host) **replaces** the inspector content; multiple inspectors are never stacked or appended side-by-side.

2. **Routine CRUD & Configuration Surface:**
   All routine viewing, creation, editing, and configuration of core objects (Hosts, Groups, Identities, Snippets, Tunnels, Workspaces, Vault details, Cloud Providers, Team Members, and Remote Session Settings) use the shared `InspectorShell`. Bespoke floating editor dialogs for routine CRUD are eliminated.

3. **Guarded Draft Transitions (Zero Data Loss):**
   Unsaved form changes are strictly guarded. Attempting to select another object, switch navigation tabs, change active vaults, or close the window triggers a standard `Save / Discard / Keep Editing` confirmation guard. Draft states are preserved during window resizes and transient errors.

4. **Multi-Vault & Window Isolation:**
   The active inspector belongs exclusively to its window and active `VaultId`. Switching vaults cancels or guards pending drafts without leaking data across vaults. Asynchronous backend operations use generation tokens and target IDs to prevent delayed responses from mutating different objects or wrong vaults.

5. **Responsive Measured Layout Engine:**
   Layout transitions are based on actual measured GPUI content bounds, not screen resolution.
   - **Preferred Inspector Width:** 380–420 logical px (minimum 320 px, maximum 440 px).
   - **Side Presentation Mode:** Enabled only when the remaining primary content width is **>= 560 logical px**.
   - **Focused Content Presentation Mode:** When remaining primary content would drop below 560 logical px, the inspector smoothly transitions to full center content presentation, providing an accessible Back/Close control while preserving the main navigation rail.

6. **Authentic Controls & No False Affordances:**
   Every visible UI element must either function, display an explicit disabled state with explanation, or be omitted. Unsupported protocol actions (e.g. RDP audio redirection on VNC sessions) are hidden or clearly marked unavailable.

7. **Compact Protocol-Aware Remote Toolbar:**
   Remote desktop (RDP/VNC) sessions utilize a compact single-row toolbar (40–48 logical px high). Telemetry (FPS, latency, throughput), diagnostic details, and peripheral redirection settings (audio, microphone, drives, clipboard) are organized into categorized dropdown menus and a unified Session Inspector.

8. **Mathematical Canvas Bounds & Pointer Translation:**
   Remote canvas interaction uses measured display bounds to map window pointer events to framebuffer coordinates, properly handling letterboxing, centering, clipping, panning in 100% zoom, and stretch modes.

---

## 2. Interaction Surface Taxonomy

All user interfaces across Shellumi fall strictly into four categorized surfaces:

```mermaid
graph TD
    UI[Shellumi UI Surfaces] --> INS[1. Unified Inspector]
    UI --> ALT[2. Standardized Safety Alert]
    UI --> DED[3. Dedicated Page Workflow]
    UI --> POP[4. Anchored Menu / Popover]

    INS --> INS_VIEW[View Mode: Semantic metadata & actions]
    INS --> INS_CREATE[Create Mode: Blank context-aware form]
    INS --> INS_EDIT[Edit Mode: Controlled validated draft]

    ALT --> ALT_DEL[Delete Confirmation]
    ALT --> ALT_GUARD[Unsaved Changes Guard]
    ALT --> ALT_AUTH[Vault Unlock & Master Password]
    ALT --> ALT_TOFU[Host Key Fingerprint Trust]

    DED --> DED_WIZ[Setup & Onboarding Wizard]
    DED --> DED_DIFF[Sync Conflict Split-Diff]
    DED --> DED_REC[Disaster Recovery & Restore]
    DED --> DED_DOC[System Diagnostics Doctor]
    DED --> DED_PLAY[Session Recording Player]

    POP --> POP_ACT[Object Action Menu ...]
    POP --> POP_PAL[Command Palette & Quick Connect]
    POP --> POP_DISP[Display & Keyboard Dropdowns]
```

### 2.1 The Unified Inspector (`InspectorShell`)
- **Structure:**
  - `InspectorHeader`: Object title, icon/badge, status indicator, action menu, and close/back button.
  - `InspectorBody`: Scrollable region (`flex_1`, `min_h_0`, `overflow_y_scroll`) containing structured `InspectorSection`s.
  - `InspectorFooter`: Sticky action bar containing primary (Save/Create/Connect) and secondary (Cancel/Back) actions, with validation messages and busy indicators.
- **Modes:** `View`, `Create`, `Edit`, `Pending`, `Error`.

### 2.2 Standardized Safety Alerts (`ConfirmationAlert` / `SecurityChallenge`)
Short-lived modal dialogs that trap focus and block background interaction for irreversible or cryptographic decisions:
- Deleting hosts, groups, vaults, or credentials.
- Guarding unsaved changes (`Save`, `Discard`, `Keep Editing`).
- Master passphrase entry and biometric unlock.
- SSH host-key verification and TOFU acceptance.

### 2.3 Dedicated Page Workflows
Complex, multi-step, or full-width tasks that require extensive real estate:
- Setup onboarding wizard (`/setup`)
- Sync conflict split diff review (`/sync/conflict`)
- Session replay player (`/sessions/replay`)
- Batch execution monitor (`/batch`)
- System diagnostics doctor (`/settings/diagnostics`)

### 2.4 Transient Anchored Popovers & Menus
Transient popups positioned relative to measured trigger elements:
- Context menus (`…`)
- Command palette (Cmd+K)
- Display settings dropdown (Fit / 100% / Fill)
- Virtual keyboard menus (Ctrl+Alt+Del, Win, Alt+Tab)

---

## 3. Single Inspector Coordinator State Machine

```mermaid
stateDiagram-v2
    [*] --> Closed
    Closed --> Opening: Select Object or New Action
    Opening --> ViewMode: Existing Object
    Opening --> CreateMode: New Object

    state ViewMode {
        [*] --> Idle
        Idle --> Connecting: Connect Clicked
        Connecting --> Idle: Connected or Failed
    }

    ViewMode --> EditMode: Click Edit
    EditMode --> DirtyEdit: User Modifies Field
    DirtyEdit --> DirtyEdit: Additional Edits

    DirtyEdit --> GuardPrompt: Navigate Away / Close / Switch Target
    GuardPrompt --> Saving: User selects 'Save'
    GuardPrompt --> Reverting: User selects 'Discard'
    GuardPrompt --> DirtyEdit: User selects 'Keep Editing'

    EditMode --> Saving: Click Save (Valid)
    Saving --> ViewMode: Save Confirmed by Backend
    Saving --> EditMode: Backend Error (Retain Draft)

    EditMode --> ViewMode: Click Cancel (Clean)
    Reverting --> ViewMode: Discard Draft
    Reverting --> Closed: Closed with Discard

    ViewMode --> Closed: Click Close / Escape
```

---

## 4. Responsive Layout Architecture

```text
+----------------------------------------------------------------------------------+
| Top Bar (Vault Selector, Quick Connect / Search, Session Tabs, Overflow Menu)    |
+----------+----------------------------------------------+------------------------+
| Nav Rail | Primary Workspace                            | Unified Inspector      |
|          | (Hosts Grid/List, SFTP Browser, Terminal,    | Preferred: 380-420px   |
|          |  or Remote Desktop Canvas)                   | Sticky Header          |
|          |                                              | Scrollable Body        |
|          | Width >= 560px required for Side mode        | Sticky Footer          |
+----------+----------------------------------------------+------------------------+
```

When total window width or available primary content width drops below the minimum constraint:
- Side mode is replaced with **Focused Content mode**.
- The Inspector occupies the center workspace, maintaining the global top bar and navigation rail.
- An accessible Back button in the Inspector header returns the user to the previous inventory view.

---

## 5. Remote Desktop Coordinate Mapping

Coordinate mapping converts measured window coordinates to native remote framebuffer coordinates:

```text
Pointer Event (Global Screen / Window)
         │
         ▼
[GPUI Content Bounds Subtraction]
         │
         ▼
Canvas-Local Coordinates (cx, cy)
         │
         ▼
[Letterbox Exclusion & Aspect Check]
         │
         ▼  (Outside render area? -> Reject)
[Scale & Offset Transform: Fit, Fill, 100% Pan]
         │
         ▼
Framebuffer Coordinates (fx, fy) -> Dispatched to Protocol Worker
```

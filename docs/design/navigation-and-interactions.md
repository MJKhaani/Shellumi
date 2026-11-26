# Navigation & Interaction Surfaces

[Docs Home](../README.md) / [Design](README.md) / Navigation & Interactions

---

## Surface Classification

| Surface Type | When to Use | Examples |
| :--- | :--- | :--- |
| **Unified Inspector** | Routine Create, View, Edit, and telemetry inspection | Host, Group, Identity, Snippet, Tunnel, Workspace, Vault, Provider, TeamMember, RD Session |
| **Dedicated Page** | Full-screen browsing, multi-pane workflows, or large diffs | Hosts, SFTP Dual-Pane Browser, Terminal/RD Tabs, Session Logs, Settings, Conflict Resolution, Setup Wizard |
| **Anchored Popover** | Lightweight contextual filters, pickers, or overflow lists | Vault Switcher, Tag Filter, Sort Dropdown, Tab `+N more` Overflow, RD Display/Keyboard/More menus |
| **Context Menu** | Right-click object actions positioned at cursor | Group Card actions, SFTP File/Folder actions |
| **Confirmation Dialog (`alert_dialog`)** | Destructive confirmations and unsaved-draft guards | Delete Host/Group/Identity/Vault, Unsaved Changes (`Save` / `Discard` / `Keep Editing`) |
| **Security Challenge (`security_challenge_dialog`)** | Blocking cryptographic or trust decisions | Vault Unlock, Master Password / Recovery Code, SSH/RDP/VNC TOFU Trust |
| **Native File Chooser** | Selecting local filesystem paths | SSH key/certificate paths, Backup export/import, RDP Shared Drive folder |

> [!IMPORTANT]
> The first-run **Setup Wizard** (`ui/setup_wizard_modal.rs`, `actions/setup_wizard.rs`) is a preserved dedicated responsive step workflow (`max_w(760px)`, `max_h(720px)` with pinned stepper/footer and scrollable body) and must not be redesigned into a generic Inspector.

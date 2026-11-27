# <img src="../../resources/icon_pack/icons/48/regular/shellumi_logo.png" width="24" height="24" alt="Shellumi"> Application Interface

[Docs Home](../README.md) / [User Guide](README.md) / Application Interface

---

Shellumi is a native GPUI (`0.2.2`) desktop application organized into four coordinated surfaces:

1. **Top Bar & Window Chrome:** Displays the active **Vault Badge** (and remote server chip for Shellumi Server vaults), sync status indicator, quick-open new vault window (`🗗` / `Cmd+Shift+N`), Quick Connect, and platform window controls (transparent titlebar + traffic lights on macOS; server-side window decorations on Windows/Linux).
2. **Left Navigation Rail (`nav_items`):**
   - Primary rail items (`main.rs:2244-2251`): **Hosts**, **Keychain**, **Port Forwarding**, **Snippets**, **Known Hosts**, **Session Logs**, plus **Settings** at the rail footer.
   - Additional registered workspace/tool routes (`app_render.rs`): **Overview**, **Workspaces**, **SFTP Files**, **Vaults**, **Integrations**.
3. **Primary Content Canvas:** Renders the active inventory list/grid, terminal tabs & split panes, dual-pane SFTP manager, or Remote Desktop canvas.
4. **Unified Right-Hand Inspector (`InspectorCoordinator`):** At most **one** Inspector is active per window. Selecting any object replaces the Inspector smoothly:
   - **Side Mode:** Shown alongside content when remaining primary content width is `>= 560` logical px.
   - **Focused Content Mode:** Expands into the center content area on narrower windows (`< 560` logical px) with an accessible Back/Close control.

---

## Navigation, Draft Guards & Dialogs

- **Unsaved-Draft Guard:** Navigating to another host, switching tabs/pages, changing vaults, or closing a window with unsaved Inspector edits triggers a **Save / Discard / Keep Editing** confirmation dialog (`INS-008`..`010`).
- **Multi-Window Vault Binding:** Each window binds independently to its own `VaultId`; closing one window closes only that window without terminating unrelated windows or sessions.
- **Command Palette:** Open with `Cmd+K` / `Ctrl+K` / `Cmd+Shift+P` (see [Keyboard Shortcuts](../reference/keyboard-shortcuts.md)) to search hosts, launch connections, switch vaults/pages, and discover shortcuts.

## Accessibility & Platform Notes

- Keyboard navigation, focus indicators, high-contrast themes, and Escape/Enter Inspector controls are supported (`INS-020`).
- **Limitation:** GPUI `0.2.2` has limited native OS screen-reader tree exposure ([Accessibility](../design/accessibility.md)). Windows and Linux native window chrome and interactive DPI/menu behavior remain `NOT TESTED` (`PLAT-001`, `PLAT-002`).

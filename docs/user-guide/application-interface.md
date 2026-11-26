# Application Interface

[Docs Home](../README.md) / [User Guide](README.md) / Application Interface

---

## Layout Regions

1. **Top Bar:** Contains the Vault Switcher, SFTP tab shortcut, active session tabs (with `+N more` overflow menu), Quick Connect bar (`Cmd+O` / `Ctrl+O`), and Command Palette trigger (`Cmd+K` / `Ctrl+K`).
2. **Navigation Rail:** Collapsible sidebar (`Cmd+\` / `Ctrl+\`) switching between Overview, Hosts, Keychain, Port Forwarding, Snippets, Known Hosts, Session Logs, Workspaces, Vaults, Integrations, and Settings.
3. **Primary Workspace:** Displays the active inventory page, SFTP dual-pane browser, terminal split layout, or Remote Desktop canvas.
4. **Unified Inspector:** Slides open on the right (`Side` mode on wide windows, `FocusedContent` with a Back bar on narrow windows) to view, create, or edit hosts, groups, identities, tunnels, snippets, workspaces, vaults, providers, team members, or live remote desktop telemetry.

> [!NOTE]
> Unsaved edits in the Inspector are protected by a dirty-state navigation guard (`Save`, `Discard`, or `Keep Editing`) so switching tabs, vaults, or closing the window never silently discards changes.

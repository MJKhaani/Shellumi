# <img src="../../resources/icon_pack/icons/48/regular/workspaces.png" width="24" height="24" alt="Workspaces"> Workspaces & Layouts

[Docs Home](../README.md) / [User Guide](README.md) / Workspaces

---

Workspaces save multi-session arrangements—associated hosts, split layouts (`single`, `vertical`, `horizontal`, `grid`), tab names, descriptions, and tags—so you can reopen an entire operational environment in one action inside the active vault.

## Prerequisites & Access

- An unlocked local or remote vault.
- Open **Workspaces** via the Command Palette or `Cmd+Shift+K` (macOS) / `Ctrl+Shift+K` (Linux/Windows).
- Click **New Workspace** or select an existing workspace to inspect, edit, open, duplicate, or delete it in the right-hand **Inspector**.

---

## Step-by-Step: Creating & Opening a Workspace

1. Open **Workspaces** and click **New Workspace**.
2. Enter **Name** (e.g., `incident-triage`), **Layout** (`single`, `vertical`, `horizontal`, or `grid`), optional **Description**, and comma-separated **Tags**.
3. Associate saved hosts/panes from the current vault and click **Save**.
4. To launch the workspace, select it and click **Open** (or use **Duplicate** to clone its layout under a new name, or **Delete** to remove it).

### Saved Layout vs. Live Remote Processes

- Reopening a workspace **launches fresh SSH/terminal sessions** for the saved host definitions and layout.
- Shellumi does **not** restore live remote OS processes (such as an already-running `htop` or `vim` process on a remote server) across application restarts (`REQ-TERM-006`).

---

## Broadcast Input & Split Panes

- Inside an active multi-pane session, toggle **Broadcast Input** explicitly to mirror keystrokes across selected panes.
- Excluded panes are visually distinct, and broadcast mode can be disabled immediately from the toolbar.

---

## Vault Isolation, Errors & CLI

- **Vault Isolation:** Workspaces are strictly scoped to their owning vault (`SavedWorkspace`). Hosts from another vault cannot be mixed silently.
- **Missing or Failed Hosts:** If a host referenced by a workspace was deleted or fails authentication, the affected pane displays a connection error without crashing unrelated panes.
- **CLI Management:**
  ```bash
  shellumi-cli ~/vault.db workspace add incident-triage --layout grid --tags ops,prod
  shellumi-cli ~/vault.db workspace list
  shellumi-cli ~/vault.db workspace show <id>
  shellumi-cli ~/vault.db workspace duplicate <id> incident-triage-eu
  shellumi-cli ~/vault.db workspace delete <id>
  ```

## Related Documentation

- [Terminal](terminal.md) · [Vaults](vaults.md) · [Keyboard Shortcuts](../reference/keyboard-shortcuts.md)

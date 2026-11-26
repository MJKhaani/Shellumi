# <img src="../../resources/icon_pack/icons/48/regular/sftp.png" width="24" height="24" alt="SFTP"> SFTP File Manager

[Docs Home](../README.md) / [User Guide](README.md) / SFTP File Manager

---

## Dual-Pane Browser

Open **SFTP** from the top bar (`Cmd+Shift+U` / `Ctrl+Shift+U`) to browse two panes side-by-side:
- **Supported transfer modes:** Local-to-Local, Local-to-Remote, Remote-to-Local, and Remote-to-Remote.
- **Navigation:** Click breadcrumb segments, use `<` / `>` history buttons, or double-click folders.
- **File operations:** Drag-and-drop between panes or use the right-click context menu (**Copy to target**, **Rename**, **Delete**, **New Folder**, **Edit Permissions**, **Show/Hide Hidden Files**, **Refresh**).
- **Remote Editor:** Open remote text files in the built-in editor with SHA-256 dirty tracking and atomic save.

> [!NOTE]
> Remote SFTP transfers stage to a temporary file and publish by rename only after verified `CLOSE` acknowledgment, so existing destination files are preserved if a transfer fails or is canceled mid-stream.

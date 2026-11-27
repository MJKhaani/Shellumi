# <img src="../../resources/icon_pack/icons/48/regular/sftp.png" width="24" height="24" alt="SFTP"> SFTP File Manager

[Docs Home](../README.md) / [User Guide](README.md) / SFTP

---

Shellumi includes a dual-pane SFTP file manager (`shellumi-core::sftp` and `shellumi-app/src/sftp_ui`) for browsing, transferring, and editing files over SSH without leaving the workspace.

## Prerequisites & Opening SFTP

- An SSH host configured with valid credentials (password, key, or SSH agent) and `sftp-server` enabled on the remote host.
- Open **SFTP** from the left navigation rail or launch SFTP directly from a selected host or active SSH session.

---

## Dual-Pane Navigation & File Operations

- **Local & Remote Panes:** Left/local pane browses your workstation; right/remote pane browses the connected SSH server.
- **Navigation Controls:** Breadcrumb bar, editable path/address bar, Back/Forward history, Refresh, and **Show Hidden Files** toggle.
- **Supported Operations:**
  - Upload (`local → remote`) and Download (`remote → local`).
  - Create files and directories, rename, move, delete, and update permissions (`chmod`).
  - Inspect file metadata (size, permissions, modified timestamp, symlink targets).

> **Server Extension Note:** Not all SFTP servers implement identical SFTP v3+ extensions, checksums, or byte-range resume. Shellumi probes capabilities safely rather than assuming non-standard extensions exist.

---

## Remote Text Editing

1. Double-click or choose **Edit** on a supported remote text file.
2. Shellumi downloads the file into an isolated temporary workspace.
3. Edit the file and save (`Cmd+S` / `Ctrl+S`): dirty tracking detects local changes and uploads them back to the server.
4. Remote modification checks help prevent silently overwriting changes made concurrently on the server, and temporary files are cleaned up when the editor closes.

---

## Transfer Queue, Atomic Staged Uploads & Recovery

- **Transfer Queue:** Concurrent uploads and downloads display progress, bytes transferred, throughput, and status.
- **Cancellation & Retry:** Cancel active transfers or retry failed transfers from the queue.
- **Atomic Staged Uploads & Overwrite Protection:** Staged upload handling and conflict choices (`Overwrite`, `Skip`, `Rename`) protect destination files from partial corruption on interrupted links.
- **Transfer History:** Completed and failed transfers are recorded in vault-scoped history (`shellumi-cli sftp history` / `clear-history`).

## Troubleshooting

- **`sftp-server` Subsystem Error:** Ensure `Subsystem sftp` is enabled in `/etc/ssh/sshd_config` on the remote host.
- **Permission Denied:** Check remote directory ownership/permissions or `chmod` settings.
- **CLI Verification:** Test connectivity directly with `shellumi-cli ~/vault.db sftp remote-ls user@host /tmp`.

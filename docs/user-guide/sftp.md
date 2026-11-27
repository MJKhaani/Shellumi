# <img src="../../resources/icon_pack/icons/48/regular/sftp.png" width="24" height="24" alt="SFTP"> SFTP File Manager

[Docs Home](../README.md) / [User Guide](README.md) / SFTP

---

Shellumi includes a dual-pane SFTP file manager (`shellumi-core::sftp` and `shellumi-app/src/sftp_ui`) for browsing, transferring, and editing files over SSH without leaving the workspace.

## Prerequisites & Opening SFTP

- An SSH host configured with valid credentials (password, key, or SSH agent) and `sftp-server` enabled on the remote host.
- Open **SFTP** from the left navigation rail or launch SFTP directly from a selected host or active SSH session.

---

## Dual-Pane Navigation & File Operations

- **Local & Remote Panes:** Either pane can browse the local filesystem or an authorized remote SSH host; remote home directories are discovered via `REALPATH(".")`.
- **Navigation Controls:** Breadcrumb bar, editable path/address bar, Back/Forward history, Refresh, column sorting (Name, Date Modified, Size, Kind), and **Show Hidden Files** toggle.
- **Supported Operations:**
  - All four transfer directions: `Local → Local` (staged atomic copy), `Local → Remote` (pipelined upload), `Remote → Local` (pipelined download), and `Remote → Remote` (bounded streaming relay).
  - Create files and directories (exclusive creation), rename, move, recursive delete (post-order without following symlinks), update permissions (`chmod`), inspect properties & SHA-256 checksums.
  - Inspect file metadata (size, permissions including `s/S`/`t/T` special bits, modified timestamp, symlink targets, local disk capacity).

> **Server Extension Note:** Not all SFTP servers implement identical SFTP v3+ extensions (`posix-rename@openssh.com`, `fsync@openssh.com`, `statvfs@openssh.com`, `copy-data`, `limits@openssh.com`). Shellumi negotiates capabilities at `SSH_FXP_VERSION` and falls back safely without deleting existing destinations.

---

## Integrated Administrative Text/Code Editor

1. Double-click a regular text file or choose **Edit in Editor** from the context menu.
2. Shellumi detects the language mode across 15 admin/config/script formats (JSON, JSONC, YAML, TOML, INI/systemd, dotenv, SSH config, nginx/Apache, Shell, Python, PowerShell, Dockerfile, XML, Properties, Plain Text) without executing file contents.
3. **Formatting:** Explicit **Format** supports strict non-lossy JSON (rejecting duplicate keys and preserving key order), conservative YAML (preserving block scalars and comments), and conservative TOML; script/config modes where full-document formatting could alter semantics remain safely disabled.
4. **Safe Save & Conflict Detection:** Saving verifies the target SHA-256 against checkout baseline, surfaces conflicts before overwriting, and keeps newer edits dirty if you continue typing while a save is in flight.
5. **Vault Isolation:** Locking or switching vaults cancels unauthorized SFTP sessions/transfers and clears remote editor state.

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

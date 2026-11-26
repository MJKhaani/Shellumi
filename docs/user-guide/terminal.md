# <img src="../../resources/icon_pack/icons/48/regular/terminal.png" width="24" height="24" alt="Terminal"> Terminal & Console Protocols

[Docs Home](../README.md) / [User Guide](README.md) / Terminal

---

## Terminal Features

- **Tabs & Local Shells:** Open a local shell (`Cmd+L` / `Ctrl+L`), connect to SSH/Mosh/Telnet/Serial hosts, or switch tabs (`Cmd+1`–`9` / `Ctrl+1`–`9`).
- **Split Panes:** Split vertically (`Cmd+D` / `Ctrl+Shift+E`) or horizontally (`Cmd+Shift+D` / `Ctrl+Shift+O`), cycle pane focus, or toggle focus mode (`Cmd+Shift+M` / `Ctrl+Alt+M`).
- **Broadcast Input:** Toggle broadcast (`Cmd+B` / `Ctrl+Alt+B`) to send keystrokes to all visible panes in the active tab simultaneously.
- **Search & Zoom:** Find in scrollback (`Cmd+F` / `Ctrl+F`) and zoom text (`Cmd+=` / `Cmd+-` / `Cmd+0` or `Ctrl+=` / `Ctrl+-` / `Ctrl+0`).
- **Snippets Panel:** Toggle the snippets drawer (`Cmd+.` / `Ctrl+.`) to paste or run saved commands.

## Mosh, Telnet, and Serial

- **Mosh:** Uses `mosh` with `--ssh=...` bootstrap and fail-closed `SSH_ASKPASS` propagation.
- **Telnet:** Displays an explicit cleartext warning because Telnet traffic and credentials are unencrypted.
- **Serial:** Connects to local serial ports via `screen` with configurable baud rate, data bits, stop bits, parity, and flow control.

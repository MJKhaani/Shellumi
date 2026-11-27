# <img src="../../resources/icon_pack/icons/48/regular/terminal.png" width="24" height="24" alt="Terminal"> Terminal & Connection Sessions

[Docs Home](../README.md) / [User Guide](README.md) / Terminal

---

Shellumi's GPU-accelerated terminal (`shellumi-terminal` + GPUI) handles interactive SSH sessions, local OS shells, and serial/telnet/Mosh sessions.

## Connection Types & Runtime Status

| Session Type | Implementation Status | Runtime Notes & Dependencies |
| --- | --- | --- |
| **SSHv2** | Implemented | Native Rust SSH client with TOFU `known_hosts` verification, jump hosts, and port forwarding. |
| **Local Terminal** | Implemented | Native PTY on macOS/Linux (`$SHELL` / `/bin/sh`) and ConPTY on Windows (`%COMSPEC%` / `cmd.exe`; Windows interactive `NOT TESTED`). |
| **Serial** | Implemented (core + CLI) | Device discovery (`shellumi-cli serial-ports`) and configurable baud rate. |
| **Telnet** | Partial | Option negotiation & unencrypted-traffic warning; live telnet server `NOT TESTED`. |
| **Mosh** | Partial | Capability config, prediction modes (`adaptive`/`always`/`never`/`experimental`), and askpass error propagation (`build_mosh_terminal_command`); requires `mosh-client`/`mosh-server`, live Mosh server `NOT TESTED`. |

---

## Multi-Terminal Workflow: Tabs, Splits & Broadcast Input

- **Session Tabs:** Open multiple hosts or local shells in parallel tabs; rename, switch, or close tabs (closing a dirty/active session respects confirmation guards).
- **Split Panes:** Split the active terminal vertically or horizontally to view logs alongside interactive commands.
- **Broadcast Input:** Toggle Broadcast Mode explicitly to send keystrokes to selected panes simultaneously. Active broadcast targets are highlighted visually, individual panes can be excluded, and broadcast can be disabled immediately.
- **Scrollback & Search:** Search terminal scrollback with case-sensitive or regex matching and jump between next/previous matches.

---

## Appearance & Keyboard Shortcuts

- Configure font family, font size, cursor shape/blink, color theme (Dark / Light / High Contrast / System), and scrollback limits in **Settings → Terminal** and **Settings → Appearance**.
- See [Keyboard Shortcuts](../reference/keyboard-shortcuts.md) for all verified registered bindings (note: macOS `Cmd+C`/`Cmd+V` copy/paste inside terminal buffers uses context/selection actions rather than overriding raw terminal `Ctrl+C` `SIGINT`).

---

## Host-Key Verification, Cancellation & Reconnection

- **TOFU Host-Key Verification:** On first connection to an SSH host, Shellumi displays the server's SHA-256 fingerprint for explicit acceptance. If a known host's key changes, the connection is **blocked immediately** with a mismatch alert—host-key verification is never silently disabled.
- **Connection Cancellation:** Long-running connection handshakes can be canceled via `ConnectionOpRegistry` without leaving stale tabs or orphaned auth prompts.
- **Reconnection:** Disconnected tabs display clear status and allow one-click reconnect without re-entering saved credentials.

---

## Shell Integration, Autocomplete & Session Recording (Implemented vs. Planned)

- **Implemented:**
  - Reversible shell hooks via `shellumi-cli shell-hook <bash|zsh|fish>`.
  - `AutocompleteEngine` (`shellumi-cli complete`) and `PasswordPromptDetector` (`shellumi-cli prompt-detect`).
  - Session recording/playback/redaction (`.cast` format) in [Session Recording](session-recording.md).
- **Planned:** Inline PowerShell shell hook, full GUI autocomplete popup integration, and optional AI command assistant (`REQ-AI-001`..`003`).

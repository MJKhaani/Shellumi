# Protocol Capabilities Reference

[Docs Home](../README.md) / [Reference](README.md) / Protocol Capabilities

---

Derived from `ProtocolCapabilities::for_protocol` (`crates/shellumi-core/src/protocol.rs`).

| Protocol | Default Port | Network Host/Port | Username / Domain | Saved Identity | Group Inheritance | Host-Private Secret | Ask on Connect | No Auth | Runtime Binary |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| <img src="../../resources/icon_pack/icons/32/regular/ssh.png" width="16" height="16" alt="SSH"> **SSH** | `22` | Required | User: Yes / Domain: No | Yes | Yes | Password / Key / Passphrase | Yes | No | System `ssh` |
| <img src="../../resources/icon_pack/icons/32/regular/sftp.png" width="16" height="16" alt="SFTP"> **SFTP** | `22` | Required | User: Yes / Domain: No | Yes | Yes | Password / Key / Passphrase | Yes | No | System `ssh -s sftp` |
| <img src="../../resources/icon_pack/icons/32/regular/mosh.png" width="16" height="16" alt="Mosh"> **Mosh** | `22` | Required | User: Yes / Domain: No | Yes | Yes | Password / Key / Passphrase | Yes | No | System `mosh` + `ssh` |
| <img src="../../resources/icon_pack/icons/32/regular/rdp.png" width="16" height="16" alt="RDP"> **RDP** | `3389` | Required | User: Yes / Domain: Yes | Yes (Password) | Yes | Password | Yes | No | Embedded `IronRDP` |
| <img src="../../resources/icon_pack/icons/32/regular/vnc.png" width="16" height="16" alt="VNC"> **VNC** | `5900` | Required | User: Optional / Domain: No | Yes (Password) | Yes | Password | Yes | Yes | Embedded `RFB` |
| <img src="../../resources/icon_pack/icons/32/regular/port_forwarding.png" width="16" height="16" alt="Telnet"> **Telnet** | `23` | Required | User: Optional / Domain: No | No | No | No | Yes | Yes | System `telnet` |
| <img src="../../resources/icon_pack/icons/32/regular/serial.png" width="16" height="16" alt="Serial"> **Serial** | `—` | Device path | No | No | No | No | No | Yes | System `screen` (Unix) |
| <img src="../../resources/icon_pack/icons/32/regular/terminal.png" width="16" height="16" alt="Local"> **Local** | `—` | None | No | No | No | No | No | Yes | `$SHELL` / `COMSPEC` |

> [!WARNING]
> **Telnet Cleartext Warning:** Telnet transmits credentials and session data unencrypted and displays an explicit warning badge in both CLI and desktop UI.

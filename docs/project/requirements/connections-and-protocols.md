[Documentation Home](../../README.md) > [Requirements](README.md) > **Connections & Protocols**

# Connections, Protocols, Tunnels & Remote Desktop Requirements

Recovered from §3.1, §3.2, and §3.6 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## SSH Client (`REQ-SSH-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-SSH-001` | SSHv2 connection via hostname, IPv4, IPv6, custom port, DNS resolution, timeouts, keepalive, quick-connect, and multi-session tabs. | **Implemented** | `SshDestination`, `session.rs`, `ssh_sftp_acceptance.rs`. |
| `REQ-SSH-002` | Automatic reconnection with exponential backoff, connectivity monitoring, diagnostics, and graceful disconnect. | **Partial** | Diagnostics and reconnect policy implemented; full network-change monitor partial. |
| `REQ-SSH-003` | Authentication: password, public-key (Ed25519, ECDSA, RSA-SHA2), keyboard-interactive, passphrase-protected keys, multi-identity per host. | **Implemented** | `AuthMethod`, `identity` store. |
| `REQ-SSH-004` | SSH certificate authentication and CA compatibility; multi-factor authentication. | **Partial** | `OpenSshCertificate` implemented; interactive MFA flows partial. |
| `REQ-SSH-005` | Post-quantum / hybrid key exchange when standards-compliant in upstream SSH libraries. | **Planned** | Contingent on upstream SSH crate support. |
| `REQ-SSH-006` | SSH agent integration (Unix `ssh-agent`, Windows SSH agent) with explicit, never-silent agent forwarding and security warnings. | **Partial** | Unix agent implemented; Windows agent interactive `NOT TESTED`. |
| `REQ-SSH-007` | Host-key verification: OpenSSH `known_hosts`, SHA-256 fingerprints, TOFU confirmation, pinned/revoked keys; never silently disabled. | **Implemented** | `known_hosts` enforcement in `shellumi-core`. |

## Additional Protocols (`REQ-PROTO-*`, `REQ-VNC-*`, `REQ-RDP-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-PROTO-001` | Mosh connections with roaming session preservation and server prerequisite detection. | **Partial** | `MoshConfig`, `MoshPredictMode`, CLI `mosh`; live `mosh-server` `NOT TESTED`. |
| `REQ-PROTO-002` | Telnet connections with option negotiation and prominent unencrypted-traffic warning. | **Partial** | `TelnetConfig`, CLI `telnet`; live telnet server `NOT TESTED`. |
| `REQ-PROTO-003` | Local terminal launching default/custom shells (Bash, Zsh, Fish, PowerShell, CMD) via native PTY/ConPTY. | **Implemented** | `shellumi-terminal/src/pty.rs`; Windows ConPTY interactive `NOT TESTED`. |
| `REQ-PROTO-004` | Serial terminal discovery, baud/data/parity/stop/flow-control config, reconnect, and logging. | **Partial** | `SerialConfig`, `discover_serial_ports`, CLI `serial`. |
| `REQ-VNC-001` | Native RFB 3.3/3.7/3.8 client with Raw, CopyRect, DesktopSize, Hextile, ZRLE, Tight decoders and GPUI framebuffer blitting. | **Implemented** (deterministic); **Partial** (live) | `shellumi-vnc`; mock RFB tested, live VNC `NOT TESTED`. |
| `REQ-VNC-002` | VncAuth DES challenge-response, unencrypted transport warnings, VeNCrypt/TLS where available. | **Implemented** (VncAuth); **Partial** (VeNCrypt) | `shellumi-vnc`. |
| `REQ-VNC-003` | Bidirectional VNC text clipboard (`ClientCutText` / `ServerCutText`) with size limits and loop suppression. | **Implemented** | `shellumi-vnc`. |
| `REQ-RDP-001` | Embedded IronRDP client (`MS-RDPBCGR`, `MS-RDPEDISP`, `MS-RDPECLIP`), CredSSP/NLA, strict TLS TOFU pinning. | **Implemented** (deterministic); **Partial** (live) | `shellumi-rdp`; live Windows/xrdp server `NOT TESTED`. |
| `REQ-RDP-002` | Dynamic desktop resizing (`MS-RDPEDISP`) and manual Match Viewport / fixed presets. | **Partial** | Single coordinator implemented; `RD-001` timeout overlap open. |
| `REQ-RDP-003` | Keyboard scancode/Unicode mapping, mouse events, and `Ctrl+Alt+Del` SAS forwarding. | **Implemented** | `shellumi-rdp`, `remote_desktop_canvas.rs`. |
| `REQ-RDP-004` | RDP peripheral redirection: clipboard/file streaming (`CLIPRDR`), audio (`RDPSND`), drives (`RDPDR`), microphone (`MS-RDPEAI`). | **Partial** | Implemented in `shellumi-rdp`; live server acceptance `NOT TESTED`. |

## Jump Hosts, Proxies & Tunnels (`REQ-TUNNEL-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-TUNNEL-001` | Single and chained jump hosts (`ProxyJump`-compatible), independent per-hop auth, hop failure diagnostics, and `ProxyCommand` investigation. | **Implemented** (jump hosts); **Planned** (`ProxyCommand`) | `jump_host` support in hosts & SSH engine. |
| `REQ-TUNNEL-002` | SOCKS5 and HTTP CONNECT proxy support with auth, DNS options, and group inheritance. | **Partial** | Proxy configuration fields present; full transport coverage partial. |
| `REQ-TUNNEL-003` | Local, remote, and dynamic (SOCKS) SSH port forwarding with saved profiles, auto-start, port conflict detection, and non-localhost binding warnings. | **Implemented** | `PortForwardRule`, `build_ssh_tunnel_command`, CLI `tunnel`. |

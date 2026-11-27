[Documentation Home](../../README.md) > [Requirements](README.md) > **Connections & Protocols**

# Connections, Protocols, Tunnels & Remote Desktop Requirements

Recovered from §3.1, §3.2, and §3.6 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## SSH Client (`REQ-SSH-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-SSH-001` | SSHv2 connection via hostname, IPv4, IPv6, custom port, DNS, timeouts, keepalive, quick-connect, multi-session tabs. | **Implemented** | Unit tested; Live protocol tested (opt-in macOS `sshd`) | `shellumi-core::SshDestination`, `actions/terminal.rs`, `session.rs` | `ssh_sftp_acceptance.rs` + `SshDestination` validation tests pass; Win/Linux `NOT TESTED`. |
| `REQ-SSH-002` | Automatic reconnection with backoff, connectivity monitoring, diagnostics, graceful disconnect. | **Partial** | Unit tested (`ReconnectPolicy`) | `shellumi-core::diagnostics`, `shellumi-remote-desktop::ReconnectPolicy` | Reconnect policy unit-tested; automatic OS network-change monitor **Planned**. |
| `REQ-SSH-003a` | Password, public-key (Ed25519, ECDSA, RSA-SHA2), and passphrase-protected SSH keys. | **Implemented** | Unit tested; Integration tested (`askpass.rs`) | `shellumi-core::AuthMethod`, `askpass.rs` | `askpass.rs` & `ssh_sftp_acceptance.rs` verify password/key/passphrase dispatch. |
| `REQ-SSH-003b` | Keyboard-interactive and multi-factor (MFA) SSH authentication. | **Partial** | Source-inspected; `NOT TESTED` live | `actions/terminal.rs` (system `ssh` TTY/askpass) | Interactive TTY prompt passes through to `ssh`; dedicated GUI MFA flow **Planned**. |
| `REQ-SSH-003c` | Multiple authentication identities bound to a single host. | **Planned** | Source-inspected | `hosts.identity_id` (`Option<i64>`) in `shellumi-storage/src/lib.rs` | Currently 1 `identity_id` per host (+ host-specific fallback); multi-identity list **Planned**. |
| `REQ-SSH-004` | OpenSSH certificate authentication and CA compatibility. | **Partial** | Unit tested (`OpenSshCertificate`) | `shellumi-core::OpenSshCertificate`, `NewIdentity` | Certificate parsing/association unit-tested; live CA/cert auth `NOT TESTED`. |
| `REQ-SSH-005` | Post-quantum / hybrid key exchange when standards-compliant in upstream SSH. | **Planned** | — | — | Contingent on upstream SSH transport/OpenSSH support. |
| `REQ-SSH-006` | SSH agent integration (`ssh-agent` / Windows SSH agent) with explicit, never-silent forwarding. | **Partial** | Source-inspected; `NOT TESTED` (Windows) | `AuthMethod::Agent`, `actions/terminal.rs` | Unix agent supported via system `ssh`; Windows interactive agent `NOT TESTED`. |
| `REQ-SSH-007` | Host-key verification: OpenSSH `known_hosts`, SHA-256 fingerprints, TOFU confirmation, pinned/revoked keys; never silently disabled. | **Implemented** | Integration tested (`session.rs`) | `shellumi-app/src/session.rs` | `session.rs` verifies TOFU prompt acceptance/rejection and blocks mismatches. |

## Additional Protocols (`REQ-PROTO-*`, `REQ-VNC-*`, `REQ-RDP-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-PROTO-001` | Mosh connections with roaming preservation and server prerequisite detection. | **Partial** | Unit tested (`build_mosh_terminal_command`) | `shellumi-core::MoshConfig`, `actions/terminal.rs` | Askpass error propagation & escaping unit-tested; live `mosh-server` `NOT TESTED`. |
| `REQ-PROTO-002` | Telnet connections with option negotiation and unencrypted-traffic warning. | **Experimental** | Unit tested (`TelnetConfig`) | `shellumi-core::TelnetConfig`, `actions/terminal.rs` | Config validation tested; live Telnet server `NOT TESTED`. |
| `REQ-PROTO-003` | Local terminal launching default/custom shells via native PTY/ConPTY. | **Implemented** | Unit tested; Native GUI verified (macOS) | `shellumi-terminal/src/pty.rs` | macOS PTY verified; Windows ConPTY interactive `NOT TESTED` (`PLAT-001`). |
| `REQ-PROTO-004` | Serial terminal discovery, baud/data/parity/stop/flow-control config, logging. | **Experimental** | Unit tested (`SerialConfig`) | `shellumi-core::SerialConfig`, `discover_serial_ports` | Config & port discovery tested; live serial hardware `NOT TESTED`. |
| `REQ-VNC-001` | Native RFB 3.3/3.7/3.8 client (Raw/CopyRect/DesktopSize/Hextile/ZRLE/Tight) + GPUI framebuffer. | **Implemented** (engine); **Partial** (live) | Unit + Integration tested (mock RFB + fuzz) | `shellumi-vnc` | Mock RFB handshake/decoder/fuzz tests pass; live VNC server `NOT TESTED`. |
| `REQ-VNC-002` | VncAuth DES challenge-response, unencrypted warnings, VeNCrypt/TLS. | **Partial** | Unit tested (`shellumi-vnc`) | `shellumi-vnc` | VncAuth tested; live VeNCrypt/TLS server `NOT TESTED`. |
| `REQ-VNC-003` | Bidirectional VNC text clipboard (`ClientCutText` / `ServerCutText`) with size limits. | **Implemented** | Unit tested (`shellumi-vnc`) | `shellumi-vnc` | Clipboard framing/loop-suppression tested; live VNC server `NOT TESTED`. |
| `REQ-RDP-001` | Embedded IronRDP client (`MS-RDPBCGR`, `MS-RDPEDISP`, `MS-RDPECLIP`), CredSSP/NLA, strict TLS TOFU pinning. | **Implemented** (engine); **Partial** (live) | Unit + Integration tested | `shellumi-rdp` | Deterministic IronRDP & TOFU tests pass; live Windows/`xrdp` server `NOT TESTED`. |
| `REQ-RDP-002` | Dynamic desktop resizing (`MS-RDPEDISP`) and manual Match Viewport / fixed presets. | **Partial** | Unit tested (`RdAutoResizeState`) | `main.rs`, `actions/remote_desktop.rs` | Single local in-flight coordinator tested; [`RD-001`](../known-issues.md#rd-001--ambiguous-rdp-resize-timeouts-can-permit-overlapping-remote-negotiations) open, live resize `NOT TESTED`. |
| `REQ-RDP-003` | Keyboard scancode/Unicode mapping, mouse events, `Ctrl+Alt+Del` SAS forwarding. | **Implemented** | Unit tested | `shellumi-rdp`, `remote_desktop_canvas.rs` | Pointer coordinate & SAS dispatch tested; live RDP input `NOT TESTED`. |
| `REQ-RDP-004` | RDP peripheral redirection: clipboard/files (`CLIPRDR`), audio (`RDPSND`), drives (`RDPDR`), mic (`MS-RDPEAI`). | **Partial** | Unit tested | `shellumi-rdp/src/{clipboard,audio,drive}.rs` | Path traversal & unit tests pass; live RDP peripherals `NOT TESTED`. |

## Jump Hosts, Proxies & Tunnels (`REQ-TUNNEL-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-TUNNEL-001a` | Single and chained jump hosts (`ProxyJump`-compatible). | **Implemented** | Unit tested | `validate_jump_host`, `actions/terminal.rs` | Jump host validation & SSH `-J` command construction tested. |
| `REQ-TUNNEL-001b` | `ProxyCommand` support with strict security controls. | **Planned** | — | — | Not yet implemented. |
| `REQ-TUNNEL-002` | SOCKS5 and HTTP CONNECT proxy support with auth and group inheritance. | **Planned** | — | — | Not yet implemented as first-class host/group proxy fields. |
| `REQ-TUNNEL-003` | Local, remote, and dynamic (SOCKS) SSH port forwarding with saved profiles, auto-start, port conflict detection, and non-localhost warnings. | **Implemented** (profiles/SSH); **Partial** (telemetry) | Unit tested (`PortForwardRule`) | `build_ssh_tunnel_command`, `actions/tunnels.rs` | Tunnel validation & CLI `tunnel start --dry-run` tested; no live traffic telemetry. |

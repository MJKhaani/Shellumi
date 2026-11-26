# Canonical Feature Matrix (`v0.0.8-alpha`)

[Docs Home](../README.md) / [Reference](README.md) / Feature Matrix

---

Implementation completeness and verification evidence are tracked **independently**:

- **Implementation Status:** `Implemented`, `Partial`, `Experimental`, `Planned`, `Unsupported`, `Deprecated`
- **Verification Indicators:** `Unit tested`, `Integration tested`, `CI verified`, `Native GUI verified`, `Live protocol verified`, `Not tested`

| Feature | Status | Platforms | Modes | Source | Verification | Known Limitations | Docs |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Desktop GUI (GPUI 0.2.2)** | Implemented | macOS, Linux, Windows | All | `shellumi-app` | CI verified; Native GUI verified (macOS); Not tested (Win/Linux GUI) | Win/Linux interactive GUI not run on native hosts | [App Interface](../user-guide/application-interface.md) |
| **Setup Wizard** | Implemented | macOS, Linux, Windows | All | `ui/setup_wizard_modal.rs` | Integration tested; CI verified | Live Google Drive restore needs real account | [First Launch](../getting-started/first-launch.md) |
| **Hosts, Groups & Unified Inspector** | Implemented | macOS, Linux, Windows | All | `ui/inspector/`, `actions/hosts.rs` | Unit tested; Integration tested; CI verified | Bulk edit & rich import preview minimal | [Hosts & Groups](../user-guide/hosts-and-groups.md) |
| **Identities & OS Keychain** | Implemented | macOS, Linux, Windows | All | `shellumi-core::credential`, `actions/identities.rs` | Unit tested; Integration tested; CI verified | Win/Linux native daemon runtime `Not tested` | [Identities](../user-guide/identities-and-keychain.md) |
| **SSH & Askpass** | Implemented | macOS, Linux, Windows | All | `actions/terminal.rs`, `askpass.rs` | Unit tested; Live protocol verified (macOS `sshd` opt-in) | Depends on system `ssh`; Win/Linux `Not tested` | [Terminal](../user-guide/terminal.md) |
| **Terminal Emulator & Splits** | Implemented | macOS, Linux, Windows | All | `shellumi-terminal` | Unit tested; Native GUI verified (macOS) | Windows ConPTY interactive runtime `Not tested` | [Terminal](../user-guide/terminal.md) |
| **SFTP Dual-Pane Browser** | Implemented | macOS, Linux, Windows | All | `shellumi-core::sftp`, `sftp_ui/` | Unit tested (31/31); Live protocol verified (macOS `sshd` opt-in) | Non-OpenSSH without `posix-rename` may refuse overwrite | [SFTP](../user-guide/sftp.md) |
| **Mosh** | Partial | macOS, Linux, Windows | All | `actions/terminal.rs`, `shellumi-core` | Unit tested (askpass error + escaping) | Requires external `mosh`; live server `Not tested` | [Terminal](../user-guide/terminal.md) |
| **Telnet & Serial** | Experimental | macOS, Linux, Windows | All | `actions/terminal.rs`, `shellumi-core` | Unit tested (config/validation) | Telnet unencrypted; Serial uses `screen`; live hw `Not tested` | [Terminal](../user-guide/terminal.md) |
| **Port Forwarding Tunnels** | Implemented | macOS, Linux, Windows | All | `actions/tunnels.rs`, `ui/inspector/tunnel.rs` | Unit tested; CI verified | Relies on system `ssh`; no live traffic telemetry | [Port Forwarding](../user-guide/port-forwarding.md) |
| **RDP (IronRDP)** | Implemented | macOS, Linux, Windows | All | `shellumi-rdp`, `actions/remote_desktop.rs` | Unit tested; Integration tested; CI verified | Live Windows RDP server resize `Not tested`; no request-ID ack | [Remote Desktop](../user-guide/remote-desktop.md) |
| **VNC (RFB)** | Implemented | macOS, Linux, Windows | All | `shellumi-vnc`, `actions/remote_desktop.rs` | Unit tested; Integration tested (mock RFB + fuzz); CI verified | Live external VNC server `Not tested` | [Remote Desktop](../user-guide/remote-desktop.md) |
| **Snippets & Workspaces** | Implemented | macOS, Linux, Windows | All | `actions/{snippets,workspaces}.rs` | Unit tested; CI verified | Workspaces launch new sessions, not remote processes | [Snippets](../user-guide/snippets.md) |
| **Session Recording** | Implemented | macOS, Linux, Windows | All | `shellumi-terminal`, `actions/recordings.rs` | Unit tested; CI verified | Redaction is best-effort | [Session Recording](../user-guide/session-recording.md) |
| **Encrypted Multi-Vaults & Rotation** | Implemented | macOS, Linux, Windows | All | `shellumi-storage`, `vault_rotation.rs` | Unit tested; Integration tested (crash-recovery); CI verified | — | [Vaults](../user-guide/vaults.md) |
| **Google Drive Sync** | Partial | macOS, Linux, Windows | Drive | `shellumi-core::sync`, `actions/sync.rs` | Integration tested (controlled HTTP DAG fixture) | Live Google Drive account `Not tested`; `"desktop-app"` fallback ID | [Google Drive](../synchronization/google-drive.md) |
| **Shellumi Server Sync & Collab** | Partial | macOS, Linux, Windows | Server | `shellumi-storage::server_sync`, `actions/server_sync.rs` | Integration tested (controlled server/sync fixtures) | Live deployed server E2E & team epoch rotation gated | [Shellumi Server](../synchronization/shellumi-server.md) |
| **Infrastructure & Ansible** | Partial | macOS, Linux, Windows | All | `shellumi-core::infrastructure` | Unit tested; CI verified | Live cloud API polling (AWS/Azure/DO) `Planned` | [Infrastructure](../user-guide/infrastructure.md) |
| **Mobile Apps** | Unsupported | iOS, Android | — | `shellumi-core` FFI models only | Unit tested (FFI header/layout models) | Prototype/feasibility only; no released mobile app | [Mobile Feasibility](../project/mobile-feasibility.md) |

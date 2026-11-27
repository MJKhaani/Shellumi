# <img src="../../resources/icon_pack/icons/48/regular/rdp.png" width="24" height="24" alt="Remote Desktop"> Remote Desktop (RDP & VNC)

[Docs Home](../README.md) / [User Guide](README.md) / Remote Desktop

---

Shellumi provides native Rust Remote Desktop engines—**IronRDP** (`shellumi-rdp`) for Microsoft RDP and **RFB 3.3/3.7/3.8** (`shellumi-vnc`) for VNC—rendering directly into the GPUI canvas without external viewers (`mstsc`, `xfreerdp`, or VNC binaries).

> [!NOTE]
> **Feature Parity & Live Testing Boundary:**
> RDP and VNC are distinct protocols. Audio (`RDPSND`), Microphone (`MS-RDPEAI`), Drive Redirection (`RDPDR`), File Clipboard (`CLIPRDR`), and Dynamic Display Resizing (`MS-RDPEDISP`) are **RDP-only** capabilities and are hidden/disabled on VNC sessions. Live end-to-end acceptance against external Windows RDP, `xrdp`, and live VNC servers remains `NOT TESTED` in CI ([Verification Status](../qa/verification-status.md)).

---

## 1. Microsoft RDP (`shellumi-rdp`)

### Prerequisites & Host Configuration
- Target Windows host with **Remote Desktop** enabled (TCP `3389`) or Linux host running `xrdp`.
- In **Hosts → New Host → Protocol: RDP**, configure:
  - **Hostname / IP** & **Port** (default `3389`).
  - **Username** and optional **Domain** (e.g., `CORP`).
  - **Password** (via Existing Identity, Save for This Host, or Ask on Connect).
  - **Initial Resolution** (e.g., `1920x1080`) & **Color Depth**.
  - **Strict TLS:** Enabled by default with SHA-256 TOFU certificate pinning.
  - **View-Only Mode:** Blocks outgoing mouse/keyboard input when monitoring.
  - **Peripheral Redirection (RDP-only):** Toggle **Audio (`RDPSND`)**, **Microphone (`MS-RDPEAI`)**, and **Shared Drives (`RDPDR`)**.

### RDP Session Toolbar & Controls
The compact 40–48 px session toolbar provides:
- **Display Mode:** **Fit** (aspect-preserving scale), **Fill**, or **Actual Size** (1:1 pixel mapping with smooth panning), plus **Fullscreen**.
- **Resolution & Dynamic Resizing (`MS-RDPEDISP`):** Toggle **Automatic Dynamic Resizing** (debounced on pane/window resize), trigger manual **Match Viewport**, or pick a **Fixed Resolution Preset**.
- **Special Keys:** Send `Ctrl+Alt+Del` (SAS) directly from the toolbar.
- **Clipboard (`CLIPRDR`):** Bidirectional UTF-8 text and streaming file copy/paste (with path-traversal sanitization).
- **Audio / Mic / Drives:** Mute/volume controls, active microphone indicator, and sandboxed local folder shares (`\\tsclient\<Share>` on Windows, `thinclient_drives` on `xrdp`).
- **Diagnostics Inspector:** Live measured FPS, RTT, throughput, and active channel status.

---

## 2. VNC (`shellumi-vnc`)

### Prerequisites & Host Configuration
- Target host running Apple Screen Sharing, TigerVNC, TightVNC, or `x11vnc` (TCP `5900` or `5900 + Display`).
- In **Hosts → New Host → Protocol: VNC**, configure:
  - **Hostname / IP** & **Port** (default `5900`).
  - **Username** (optional) & **Password** (`VncAuth` DES challenge-response or No-Auth where server permits).
  - **Security / TLS:** Supports `VeNCrypt`/TLS when offered by the server; displays a clear warning for unencrypted VNC transport.
  - **Initial Resolution**, **Color Depth**, and **View-Only Mode**.
- **Supported VNC Features:** Raw, CopyRect, DesktopSize, Hextile, ZRLE, and Tight encodings; bidirectional text clipboard (`ClientCutText` / `ServerCutText`); Fit / Fill / Actual Size panning.

---

## 3. Security: TLS & TOFU Certificate Pinning

On first connection over TLS (RDP CredSSP/TLS or VNC `VeNCrypt`), Shellumi displays the server's **SHA-256 certificate fingerprint** for explicit TOFU confirmation. If the certificate changes on a later connection, Shellumi **blocks the connection** with a **Certificate Fingerprint Mismatch** warning until you explicitly review and accept or reject it.

---

## 4. Troubleshooting

| Symptom | Likely Cause | Resolution |
| --- | --- | --- |
| **Black screen on connect** | Headless display sleeping, locked session, or missing virtual display driver | Wake/unlock the remote session or attach a virtual display dummy on headless Linux/macOS hosts. |
| **CredSSP / NLA auth failure** | Wrong username/password or missing Windows domain prefix | Verify credentials and set **Domain** (or `DOMAIN\user`). |
| **Certificate mismatch alert** | Server certificate rotated/rebuilt, or potential MitM | Verify the new SHA-256 fingerprint out-of-band before accepting. |
| **Incorrect pointer position** | Stale canvas bounds or zoom/pan offset | Toggle **Fit** / **Actual Size** or trigger **Match Viewport** to refresh `CanvasTransform`. |
| **Dynamic resizing unavailable** | Server does not advertise `MS-RDPEDISP` (e.g., older Windows Server or minimal `xrdp`) | Use **Fit** mode or select a **Fixed Resolution Preset**. |
| **Resize timeout / oscillation (`RD-001`)** | High-latency link overlapping bounded retries | Wait a moment for the remote desktop to settle or use a fixed preset ([`RD-001`](../project/known-issues.md#rd-001--ambiguous-rdp-resize-timeouts-can-permit-overlapping-remote-negotiations)). |
| **Clipboard not syncing** | `CLIPRDR` / `ClientCutText` disabled on server or session in **View-Only** | Disable **View-Only** and verify server clipboard policy. |
| **Audio / Mic / Shared Drives missing** | Session is VNC (RDP-only features), OS mic permission denied, or server GPO blocks redirection | Confirm protocol is **RDP**, grant OS microphone permission, and verify server redirection policies. |

# Remote Desktop User & Administrator Guide (VNC & RDP)

Shellumi provides a native, high-performance, GPU-accelerated Remote Desktop engine written entirely in Rust. It enables seamless remote administration and graphical desktop access over **Microsoft Remote Desktop Protocol (RDP)** and **Virtual Network Computing (RFB / VNC)** without relying on third-party webviews, Electron, or external client wrappers.

---

## Table of Contents

1. [Architecture & Design Principles](#1-architecture--design-principles)
2. [Connecting to Remote Desktops](#2-connecting-to-remote-desktops)
   - [Configuring an RDP Connection](#configuring-an-rdp-connection)
   - [Configuring a VNC Connection](#configuring-a-vnc-connection)
3. [Security, TLS, and TOFU Fingerprinting](#3-security-tls-and-tofu-fingerprinting)
   - [Trust-On-First-Use (TOFU) Verification](#trust-on-first-use-tofu-verification)
   - [Managing Certificate Fingerprints](#managing-certificate-fingerprints)
   - [Security Warnings and MitM Protection](#security-warnings-and-mitm-protection)
4. [Display Scaling & Dynamic Resizing](#4-display-scaling--dynamic-resizing)
   - [Display Modes & Fixed Presets](#display-modes--fixed-presets)
   - [Dynamic Display Resizing (MS-RDPEDISP)](#dynamic-display-resizing-ms-rdpedisp)
   - [High-DPI & Retina Scaling](#high-dpi--retina-scaling)
5. [Bidirectional Clipboard & File Transfer (CLIPRDR)](#5-bidirectional-clipboard--file-transfer-cliprdr)
   - [Text & UTF-8 Synchronization](#text--utf-8-synchronization)
   - [Streaming File Copy & Paste](#streaming-file-copy--paste)
   - [File Transfer Progress & Security Boundaries](#file-transfer-progress--security-boundaries)
6. [Sound Redirection (RDPSND)](#6-sound-redirection-rdpsnd)
   - [Audio Playback Architecture](#audio-playback-architecture)
   - [HUD Audio Controls & Volume Adjustment](#hud-audio-controls--volume-adjustment)
7. [Microphone & Audio Input Redirection (MS-RDPEAI)](#7-microphone--audio-input-redirection-ms-rdpeai)
   - [Voice Capture & Remote Streaming](#voice-capture--remote-streaming)
   - [Input Controls & Privacy](#input-controls--privacy)
8. [Local Drive & Directory Redirection (RDPDR)](#8-local-drive--directory-redirection-rdpdr)
   - [Mounting Local Folders into Remote Sessions](#mounting-local-folders-into-remote-sessions)
   - [Accessing Shared Folders on Windows & Linux](#accessing-shared-folders-on-windows--linux)
   - [Filesystem Security & Traversal Protection](#filesystem-security--traversal-protection)
9. [Network Resilience & Auto-Reconnection](#9-network-resilience--auto-reconnection)
   - [Transient Drop Detection](#transient-drop-detection)
   - [Exponential Backoff Retries](#exponential-backoff-retries)
   - [Session Recovery & State Rehydration](#session-recovery--state-rehydration)
10. [Diagnostics HUD & Performance Telemetry](#10-diagnostics-hud--performance-telemetry)
    - [Live HUD Metrics Overlay](#live-hud-metrics-overlay)
    - [Interpreting Session Telemetry](#interpreting-session-telemetry)
11. [Troubleshooting & FAQs](#11-troubleshooting--faqs)

---

## 1. Architecture & Design Principles

Shellumi's Remote Desktop subsystem is built from the ground up to uphold the core tenets of the application:
- **100% Pure Rust:** Powered by native protocol decoders (`IronRDP` for RDP and custom RFB decoders for VNC).
- **GPU-Accelerated Rendering:** The remote framebuffer is streamed directly into GPUI pixel buffers with minimal memory copies and ultra-low frame presentation latency.
- **Local-First & Zero Telemetry:** No external relay servers, cloud proxies, or telemetry tracking. All network traffic is point-to-point between your client and destination server.
- **Authentic Protocol State:** All telemetry indicators (FPS, RTT, bandwidth, packet loss) represent true measured network and protocol events rather than synthetic placeholders.

```
┌─────────────────────────────────────────────────────────────┐
│                       Shellumi Desktop (GPUI)               │
│  ┌─────────────────┐  ┌──────────────────┐  ┌────────────┐  │
│  │ Framebuffer Canvas │  │ Diagnostics HUD  │  │ Audio/Mic  │  │
│  └────────▲────────┘  └────────▲─────────┘  └─────▲──────┘  │
└───────────┼────────────────────┼──────────────────┼─────────┘
            │                    │                  │
┌───────────┴────────────────────┴──────────────────┴─────────┐
│              shellumi-remote-desktop (Shared Abstraction)    │
│  • Framebuffer Queue  • TOFU Storage    • Audio Buffers     │
│  • Input Dispatcher   • Cliprdr Bridge  • Resilient Session │
└───────────▲───────────────────────────────────────▲─────────┘
            │                                       │
┌───────────┴──────────────┐         ┌──────────────┴─────────┐
│     shellumi-rdp         │         │     shellumi-vnc       │
│  • IronRDP Engine        │         │  • RFB 3.3 / 3.8       │
│  • MS-RDPEDISP (Resize)  │         │  • Raw/CopyRect/ZRLE   │
│  • CLIPRDR (Files/Text)  │         │  • Tight / JPEG        │
│  • RDPSND / MS-RDPEAI    │         │  • VeNCrypt & TLS      │
│  • RDPDR (Drive Mount)   │         │  • DES Authentication  │
└───────────▲──────────────┘         └──────────────▲─────────┘
            │ TCP 3389                              │ TCP 5900
            ▼                                       ▼
┌──────────────────────────┐         ┌────────────────────────┐
│ Remote Windows / Linux   │         │ Remote VNC Server      │
│ (xrdp / Windows Desktop) │         │ (TigerVNC, x11vnc, etc)│
└──────────────────────────┘         └────────────────────────┘
```

---

## 2. Connecting to Remote Desktops

### Configuring an RDP Connection

To connect to a Windows workstation, Windows Server, or Linux host running `xrdp`:

1. Open Shellumi and click **New Host** or press `Ctrl+N` / `Cmd+N`.
2. Under **Protocol**, select **RDP**.
3. Fill in the connection settings:
   - **Host / IP Address:** Hostname or IPv4/IPv6 address (e.g. `192.168.1.100` or `desktop.corp.internal`).
   - **Port:** Default is `3389`.
   - **Username:** Remote user account (e.g. `Administrator` or `john`).
   - **Domain (Optional):** Active Directory or Windows domain if applicable.
   - **Password:** User password. Shellumi stores secrets encrypted locally via the platform keychain or encrypted local storage.
4. Under **Advanced Settings**, configure optional channels:
   - **Audio Redirection (RDPSND):** Enable to route remote sound to your local speakers.
   - **Microphone Redirection (RDPEAI):** Enable for remote audio recording and conferencing.
   - **Local Drive Sharing (RDPDR):** Choose local directory paths to mount into the remote session.
   - **Dynamic Resizing (MS-RDPEDISP):** Enable to allow the remote desktop to adjust resolution when resizing the Shellumi window.
5. Click **Connect** or **Save**.

### Configuring a VNC Connection

To connect to macOS Screen Sharing, TigerVNC, TightVNC, or x11vnc:

1. Click **New Host** and select **VNC** under Protocol.
2. Fill in the connection parameters:
   - **Host / IP Address:** Remote host IP or domain.
   - **Port:** Standard VNC port is `5900` (or `5900 + Display Number`).
   - **Password:** VNC server authentication password.
   - **Security Type:** Choose **Auto-Negotiate**, **VeNCrypt / TLS**, or **Standard VNC Authentication**.
   - **Encoding Preference:** Choose between **Auto**, **ZRLE**, **Tight (JPEG)**, or **Raw**.
3. Click **Connect**.

---

## 3. Security, TLS, and TOFU Fingerprinting

### Trust-On-First-Use (TOFU) Verification

Both RDP (over TLS / CredSSP) and VNC (over VeNCrypt / TLS) present server certificates to authenticate their identity. Shellumi uses a **Trust-On-First-Use (TOFU)** policy:

1. On the initial connection, Shellumi extracts the SHA-256 fingerprint of the server's certificate.
2. A verification modal prompts you to confirm the server's identity:
   - **Server Hostname:** e.g., `win-server-2022.local`
   - **Issuer & Subject:** Certificate details
   - **SHA-256 Fingerprint:** e.g., `A1:B2:C3:D4:...:E5:F6`
3. If you accept, the fingerprint is stored in your encrypted local database.
4. Subsequent connections to that host automatically verify the certificate fingerprint against the saved trust store.

### Security Warnings and MitM Protection

If the remote host's certificate changes (due to a server rebuild, certificate renewal, or a potential Man-in-the-Middle attack):
- Shellumi **blocks the connection immediately**.
- A prominent alert modal indicates a **Certificate Fingerprint Mismatch**.
- You are shown both the **Known Fingerprint** and the **New Fingerprint**.
- You must explicitly review and accept the change before any credentials or keystrokes are transmitted.

---

## 4. Display Scaling & Dynamic Resizing

Shellumi gives you complete control over how remote desktops render within your workspace.

### Display Modes & Fixed Presets

In the top session toolbar, click the **Display** menu to choose between:
- **Fit Window:** Automatically scales the remote desktop image to fit the current pane while preserving aspect ratio.
- **Original Size (1:1):** Displays the desktop at its native pixel resolution. If larger than the pane, smooth panning and scrollbars become active.
- **Fixed Presets:** Force specific resolutions (e.g., `1280x720`, `1920x1080`, `2560x1440`, `3840x2160`).

### Dynamic Display Resizing (MS-RDPEDISP)

For RDP connections, Shellumi supports the **MS-RDPEDISP** (Display Update Virtual Channel Extension):
- When enabled, resizing your Shellumi application window or splitting panes automatically signals the remote Windows/Linux OS to reconfigure its virtual monitor resolution.
- The remote desktop smoothly adapts to your window size in real time without disconnecting or terminating applications.
- Layout changes are automatically debounced (150ms) to ensure smooth transitions while dragging window borders.

### High-DPI & Retina Scaling

On high-density displays (such as macOS Retina screens or 4K monitors):
- Shellumi negotiates DPI scaling with RDP servers (100%, 125%, 150%, 200%).
- Keystrokes, mouse coordinates, and desktop elements remain sharp and proportional.

---

## 5. Bidirectional Clipboard & File Transfer (CLIPRDR)

Shellumi's RDP implementation features full bidirectional clipboard integration conforming to **MS-RDPECLIP (CLIPRDR)**.

### Text & UTF-8 Synchronization

- Copying text on your local host immediately updates the remote clipboard.
- Copying text inside the remote desktop immediately updates your local clipboard.
- Full UTF-8 support ensures characters, accents, and multi-language scripts transfer without corruption.

### Streaming File Copy & Paste

You can copy and paste files directly between your host and the remote machine:

1. **Copying from Host to Remote:**
   - In your local file manager (macOS Finder, Linux Files, Windows Explorer), select files and press `Ctrl+C` / `Cmd+C`.
   - Focus the Shellumi remote desktop window and press `Ctrl+V`.
   - Shellumi streams the file data over the virtual channel; the files appear in the remote destination folder.
2. **Copying from Remote to Host:**
   - In the remote desktop, select files and copy them.
   - Switch to your local machine and paste in any directory.

### File Transfer Progress & Security Boundaries

- Transfers display an overlay indicator with throughput speed (MB/s) and completion percentage.
- Path traversal sequences (such as `../` or `..\`) and absolute path exploits are strictly sanitized before writing to local or remote storage.

---

## 6. Sound Redirection (RDPSND)

Remote sound playback is handled over the **MS-RDPEA (RDPSND)** virtual channel.

### Audio Playback Architecture

- Shellumi negotiates standard uncompressed PCM audio (typically 44.1 kHz or 48 kHz, 16-bit stereo).
- Incoming audio frames are buffered through a low-latency ring buffer to prevent underruns or stuttering during minor network jitter.
- Audio is routed directly to your default system audio output.

### HUD Audio Controls & Volume Adjustment

The Remote Desktop session HUD provides immediate audio controls:
- **Speaker Icon:** Click to mute or unmute remote audio output.
- **Volume Slider:** Adjust output volume from 0% to 100% without altering your host operating system's master volume.

---

## 7. Microphone & Audio Input Redirection (MS-RDPEAI)

For teleconferencing, dictation, and voice applications inside the remote desktop, Shellumi supports upstream microphone redirection via **MS-RDPEAI**.

### Voice Capture & Remote Streaming

- Audio from your default local microphone is captured, encoded, and streamed upstream to the remote virtual audio device.
- Operates at low latency, enabling seamless use of Microsoft Teams, Zoom, or Slack inside the remote session.

### Input Controls & Privacy

- When microphone redirection is active, a glowing **Mic Indicator** appears in the top toolbar to alert you that audio input is being shared.
- Click the microphone icon or use the shortcut to instantly mute your microphone locally.

---

## 8. Local Drive & Directory Redirection (RDPDR)

The **MS-RDPEFS / RDPDR** extension allows you to share local folders as network drives within the remote session.

### Mounting Local Folders into Remote Sessions

1. In the Host configuration modal or the active session **Devices** menu, select **Shared Folders**.
2. Click **Add Folder** and select the local path you want to expose (e.g., `~/Documents/Projects`).
3. Assign a share name (e.g., `Projects`).
4. Connect to the session.

### Accessing Shared Folders on Windows & Linux

- **On Windows:** Open Windows Explorer and navigate to **This PC** or network location `\\tsclient\Projects`. You can also map it to a drive letter (e.g. `Z:`).
- **On Linux (xrdp):** Look under `thinclient_drives` in your home directory or `/media`.

### Filesystem Security & Traversal Protection

- Directory redirection is strictly sandboxed to the root path you selected.
- Symbolic link attacks and traversal attempts outside the shared tree are blocked by the Shellumi filesystem driver.

---

## 9. Network Resilience & Auto-Reconnection

Real-world network connections experience packet loss, Wi-Fi drops, and VPN resets. Shellumi includes a robust resilience layer to keep your workflow intact.

### Transient Drop Detection

- Shellumi actively monitors protocol heartbeat frames and TCP socket health.
- If the socket terminates abruptly or fails to respond within the heartbeat deadline, the session enters `Reconnecting` state rather than closing immediately.

### Exponential Backoff Retries

- Shellumi attempts automatic reconnection using jittered exponential backoff:
  - Attempt 1: 1 second delay
  - Attempt 2: 2 seconds delay
  - Attempt 3: 4 seconds delay
  - Subsequent: up to 10 seconds delay
- Up to 5 retries are attempted by default (configurable).
- A **Cancel** button allows you to abort reconnection at any time.

### Session Recovery & State Rehydration

- When reconnected, RDP and VNC sessions resume without requiring manual re-entry of passwords.
- Windows RDP sessions reattach to the existing remote desktop session, preserving your open windows and running processes.

---

## 10. Diagnostics HUD & Performance Telemetry

Press `Ctrl+Shift+D` / `Cmd+Shift+D` or click the **HUD** icon in the toolbar to display the live diagnostics overlay.

```
┌──────────────────────────────────────────────┐
│  DIAGNOSTICS & TELEMETRY                     │
│  Protocol: RDP (TLS 1.3 / CredSSP)           │
│  Resolution: 1920x1080 @ 60 Hz               │
│  Framerate: 58.4 FPS                         │
│  Round-Trip Time (RTT): 18.2 ms              │
│  Bandwidth In: 1.42 MB/s                     │
│  Bandwidth Out: 12.8 KB/s                    │
│  Packets Dropped: 0                          │
│  Active Channels: CLIPRDR, RDPSND, RDPDR     │
└──────────────────────────────────────────────┘
```

### Interpreting Session Telemetry

- **FPS (Frames Per Second):** Measured rendering rate of updated desktop regions.
- **RTT (Latency):** Measured round-trip network time to the remote host.
- **Bandwidth:** Live incoming display stream data and outgoing input/file data.
- **Drop Counter:** Number of network reconnections or dropped frames.

---

## 11. Troubleshooting & FAQs

### Common Issues & Solutions

| Issue | Likely Cause | Solution |
|---|---|---|
| **CredSSP / NLA Authentication Failed** | Server requires Network Level Authentication and credentials were incorrect or empty | Verify username and password. On domains, ensure you enter the domain prefix (e.g. `CORP\john`). |
| **Certificate Fingerprint Mismatch Warning** | The server's SSL certificate changed since your last connection | If the server was recently re-imaged or renewed its cert, verify the fingerprint out-of-band and accept the new certificate. If unexpected, investigate potential MitM. |
| **VNC Connection Refused (Port 5900)** | VNC server is not running or blocked by firewall | Ensure the VNC service is running on the host. Verify firewall rules allow TCP traffic on port 5900. |
| **Blank Screen on Connect** | Remote display server is sleeping or locked without virtual display dummy | Check remote display settings or enable dummy display dongle if running headless. |
| **Dynamic Resizing Not Responding** | Remote host does not support MS-RDPEDISP (e.g. older Windows Server or minimal xrdp) | Switch display mode to **Fit Window** or choose a fixed preset in the Display menu. |

---

## Need Further Help?

- Consult the Shellumi issue tracker: `https://github.com/shellumi/shellumi/issues`
- Inspect application logs in your system log directory or run `shellumi-cli --debug`.

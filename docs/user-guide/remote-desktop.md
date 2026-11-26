# <img src="../../resources/icon_pack/icons/48/regular/rdp.png" width="24" height="24" alt="Remote Desktop"> Remote Desktop (RDP & VNC)

[Docs Home](../README.md) / [User Guide](README.md) / Remote Desktop

---

Shellumi includes native pure-Rust **RDP** (`IronRDP`) and **VNC** (`RFB`) clients with GPU-accelerated canvas rendering.

## Connecting & Certificate Trust

1. Create or edit a host with protocol **RDP** (default port `3389`) or **VNC** (default port `5900`), or use Quick Connect.
2. Configure authentication (username/domain/password for RDP NLA/CredSSP; password or no-auth for VNC) and color depth / initial resolution.
3. On first connection to an untrusted TLS/VeNCrypt server, Shellumi displays an interactive **TOFU certificate fingerprint verification** dialog before completing the handshake.

## Compact Toolbar & Controls

| Control Group | Available Actions |
| :--- | :--- |
| **Display** | **Fit** (letterboxed aspect), **100% Actual Size** (with drag/scroll panning), **Fill**, **Fullscreen**, **Dynamic Resize** toggle (`MS-RDPEDISP`, RDP only), **Match Viewport**, and resolution presets (`720p`, `1080p`, `1440p`, `4K`) |
| **Keyboard** | Send **Ctrl+Alt+Del**, **Win**, **Alt+Tab**, or toggle **View-Only** mode |
| **More** | **Audio** mute/volume (`RDPSND`, RDP only), **Microphone** mute/gain (`MS-RDPEAI`, RDP only), **Shared Drive** folder (`RDPDR`, RDP only), **Sync Clipboard** (`CLIPRDR` / RFB), **Send Files** (RDP), **Reconnect**, and **Session Inspector** |

> [!NOTE]
> Controls unsupported by VNC (such as microphone redirection, shared drive mounting, and `MS-RDPEDISP` dynamic resizing) are automatically hidden in VNC sessions.

## Live Telemetry

Click the **Inspector** button in the session toolbar to open the **Session Inspector**, displaying live measured FPS, RTT latency, negotiated framebuffer dimensions, audio/microphone status, and shared drive byte counters.

See [Remote Desktop Engine](../architecture/remote-desktop-engine.md) for the underlying architecture and current protocol limitations.

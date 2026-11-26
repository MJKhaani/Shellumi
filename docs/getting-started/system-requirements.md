# System Requirements

[Docs Home](../README.md) / [Getting Started](README.md) / System Requirements

---

## Supported Operating Systems & Architectures

| Platform | Architectures | Build / CI Status | Native GUI Runtime Status |
| :--- | :--- | :--- | :--- |
| <img src="../../resources/icon_pack/icons/32/regular/os_macos.png" width="18" height="18" alt="macOS"> **macOS** | Apple Silicon (`aarch64`), Intel (`x86_64`) | CI verified (`macos-15`, `macos-15-intel`) | Exercised on macOS |
| <img src="../../resources/icon_pack/icons/32/regular/os_linux.png" width="18" height="18" alt="Linux"> **Linux** | `x86_64` | CI verified (`ubuntu-latest` / `ubuntu-24.04`) | **NOT TESTED** |
| <img src="../../resources/icon_pack/icons/32/regular/os_windows.png" width="18" height="18" alt="Windows"> **Windows** | `x86_64` | CI verified (`windows-latest` / `windows-2022`) | **NOT TESTED** |

---

## Runtime Dependencies

- **OpenSSH (`ssh`):** Required for SSH, SFTP (`ssh -s sftp`), and SSH Port Forwarding tunnels.
- **Mosh (`mosh`):** Optional; required only when launching Mosh hosts.
- **Serial (`screen`):** Optional on Unix for Serial console sessions.
- **Linux Desktop Keyring (`secret-tool`):** Recommended on Linux for `LinuxSecretServiceStore` (`org.freedesktop.secrets`).
- **Linux Audio/Display Libraries:** Wayland or X11 (`libxkbcommon`, `libxcb`, `libfontconfig`, `libasound2`) for GPUI rendering and RDP audio.
- **Native RDP & VNC:** Embedded pure-Rust engines (`shellumi-rdp` / `shellumi-vnc`) require no external RDP or VNC client binaries.

> [!NOTE]
> See [Platform Compatibility](../reference/platform-compatibility.md) for full details on credential stores, data paths, and platform-specific behavior.

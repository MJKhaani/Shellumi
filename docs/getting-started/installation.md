# Installation

[Docs Home](../README.md) / [Getting Started](README.md) / Installation

---

## Release Packages (Prebuilt)

Download the latest release from [GitHub Releases](https://github.com/MJKhaani/Shellumi/releases/tag/v0.0.8-alpha):

### <img src="../../resources/icon_pack/icons/32/regular/os_macos.png" width="18" height="18" alt="macOS"> macOS

- Download `Shellumi-<version>-macos-arm64.dmg` (Apple Silicon) or `Shellumi-<version>-macos-x86_64.dmg` (Intel).
- Open the `.dmg` and drag **Shellumi.app** into `/Applications`.
- Optional CLI: download `shellumi-cli-macos-arm64` or `shellumi-cli-macos-x86_64`, mark it executable (`chmod +x`), and place it on your `PATH`.

### <img src="../../resources/icon_pack/icons/32/regular/os_linux.png" width="18" height="18" alt="Linux"> Linux

- Download `shellumi_<version>_amd64.deb` and install:
  ```sh
  sudo apt install ./shellumi_<version>_amd64.deb
  ```
  The package installs `shellumi-app`, `shellumi-cli`, and the `shellumi.desktop` entry (`Terminal=false`).

### <img src="../../resources/icon_pack/icons/32/regular/os_windows.png" width="18" height="18" alt="Windows"> Windows

- Download `shellumi-<version>-windows-x86_64.zip` and extract it to a folder of your choice.
- Run `shellumi-app.exe` (`WINDOWS_GUI` subsystem) for the desktop app, or `shellumi-cli.exe` (`WINDOWS_CUI` subsystem) from PowerShell/Windows Terminal.

---

## Verify Release Checksums

Every release includes `SHA256SUMS.txt` covering all binaries and packages:

```sh
sha256sum -c SHA256SUMS.txt --ignore-missing
```

---

## Build from Source

See [Environment Setup](../development/environment-setup.md) and [Build and Run](../development/build-and-run.md).

# Installation

[Docs Home](../README.md) / [Getting Started](README.md) / Installation

---

## Release Packages (Prebuilt)

Download published pre-release binaries and packages from **[GitHub Releases](https://github.com/MJKhaani/Shellumi/releases)** (latest published pre-release: [`v0.0.8-alpha`](https://github.com/MJKhaani/Shellumi/releases/tag/v0.0.8-alpha), published November 27, 2026; note that pre-releases may not appear under GitHub's stable `/releases/latest` redirect).

### <img src="../../resources/icon_pack/icons/32/regular/os_macos.png" width="18" height="18" alt="macOS"> macOS

- Download `Shellumi-0.0.8-alpha-macos-arm64.dmg` (Apple Silicon) or `Shellumi-0.0.8-alpha-macos-x86_64.dmg` (Intel).
- Open the `.dmg` and drag **Shellumi.app** into `/Applications`.
- Optional standalone binaries: `shellumi-app-macos-arm64`, `shellumi-app-macos-x86_64`, `shellumi-cli-macos-arm64`, or `shellumi-cli-macos-x86_64` (`chmod +x` before running).

### <img src="../../resources/icon_pack/icons/32/regular/os_linux.png" width="18" height="18" alt="Linux"> Linux

- Download `shellumi_0.0.8-alpha_amd64.deb` and install:
  ```sh
  sudo apt install ./shellumi_0.0.8-alpha_amd64.deb
  ```
  The package installs `shellumi-app`, `shellumi-cli`, and the `shellumi.desktop` entry (`Terminal=false`).
- Optional standalone binaries: `shellumi-app-linux-x86_64` and `shellumi-cli-linux-x86_64`.

### <img src="../../resources/icon_pack/icons/32/regular/os_windows.png" width="18" height="18" alt="Windows"> Windows

- Download `shellumi-0.0.8-alpha-windows-x86_64.zip` and extract it to a folder of your choice.
- Run `shellumi-app.exe` (`WINDOWS_GUI` subsystem) for the desktop app, or `shellumi-cli.exe` (`WINDOWS_CUI` subsystem) from PowerShell/Windows Terminal.
- Optional standalone binaries: `shellumi-app-windows-x86_64.exe` and `shellumi-cli-windows-x86_64.exe`.

---

## Verify Release Checksums

Every published release includes `SHA256SUMS.txt` (plus `shellumi-sbom-metadata.json` and `release-provenance.json`). To verify an individual downloaded asset against `SHA256SUMS.txt`:

- **Linux (`sha256sum`):**
  ```sh
  grep 'shellumi_0.0.8-alpha_amd64.deb' SHA256SUMS.txt | sha256sum -c -
  ```
- **macOS (`shasum`):**
  ```sh
  grep 'Shellumi-0.0.8-alpha-macos-arm64.dmg' SHA256SUMS.txt | shasum -a 256 -c -
  ```
- **Windows (PowerShell — `NOT TESTED` interactively here):**
  ```powershell
  Get-FileHash .\shellumi-0.0.8-alpha-windows-x86_64.zip -Algorithm SHA256
  Select-String -Path .\SHA256SUMS.txt -Pattern 'shellumi-0.0.8-alpha-windows-x86_64.zip'
  ```

---

## Build from Source

See [Environment Setup](../development/environment-setup.md) and [Build and Run](../development/build-and-run.md).

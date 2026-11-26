# Packaging

[Docs Home](../README.md) / [Development](README.md) / Packaging

---

| Platform | Packaging Assets | Output |
| :--- | :--- | :--- |
| **macOS** | `packaging/macos/Info.plist`, `packaging/macos/build-dmg.sh` | `Shellumi-<version>-macos-{arm64,x86_64}.dmg` + standalone binaries |
| **Linux** | `packaging/linux/build-deb.sh`, `packaging/linux/shellumi.desktop`, `packaging/linux/debian/` | `shellumi_<version>_amd64.deb` (`root/root` ownership, `Terminal=false`) + standalone binaries |
| **Windows** | `packaging/windows/wix/` (WiX config) + release zip bundle in `release.yml` | `shellumi-<version>-windows-x86_64.zip` + standalone `.exe` binaries |

> [!NOTE]
> `packaging/mobile/` exists as an early prototype directory; no mobile release packages are built or published.

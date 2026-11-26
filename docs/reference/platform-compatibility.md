# Platform Compatibility

[Docs Home](../README.md) / [Reference](README.md) / Platform Compatibility

---

| Dimension | <img src="../../resources/icon_pack/icons/32/regular/os_macos.png" width="16" height="16" alt="macOS"> macOS | <img src="../../resources/icon_pack/icons/32/regular/os_linux.png" width="16" height="16" alt="Linux"> Linux | <img src="../../resources/icon_pack/icons/32/regular/os_windows.png" width="16" height="16" alt="Windows"> Windows |
| :--- | :--- | :--- | :--- |
| **Architectures** | `aarch64`, `x86_64` | `x86_64` | `x86_64` |
| **Release Artifacts** | `.dmg`, standalone `shellumi-app` & `shellumi-cli` | `.deb`, standalone `shellumi-app` & `shellumi-cli` | `.zip` bundle, `shellumi-app.exe`, `shellumi-cli.exe` |
| **Data Directory** | `~/Library/Application Support/Shellumi` | `$XDG_DATA_HOME/shellumi` or `~/.local/share/shellumi` | `%APPDATA%\Shellumi` |
| **Credential Store** | `MacOsKeychainStore` (`security-framework`) | `LinuxSecretServiceStore` (`secret-tool`) | `WindowsCredentialManagerStore` (`PasswordVault`) |
| **Window Chrome** | Transparent titlebar + traffic lights | `WindowDecorations::Server` (`Terminal=false` `.desktop`) | `WindowDecorations::Server` (PE `WINDOWS_GUI = 2`) |
| **Terminal Backend** | POSIX PTY (`$SHELL`) | POSIX PTY (`$SHELL`) | ConPTY (`%COMSPEC%` / PowerShell) |
| **Verification** | CI verified; macOS GUI exercised | CI verified; Interactive GUI `NOT TESTED` | CI verified; Interactive GUI `NOT TESTED` |

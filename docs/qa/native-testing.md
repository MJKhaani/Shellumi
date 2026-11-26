[Documentation Home](../README.md) > [QA](README.md) > **Native OS Testing**

# Native OS Testing

Automated CI verifies compilation and unit/integration test suites on macOS, Linux, and Windows, but **does not launch an interactive desktop session**. This checklist governs native OS verification.

## macOS (`aarch64-apple-darwin` / `x86_64-apple-darwin`)

- **Packaging:** `.app` bundle and `.dmg` release artifact launch cleanly without Gatekeeper or missing-framework errors.
- **Keychain:** `MacOsKeychainStore` reads, writes, and deletes vault/host secrets without repeated unexpected prompts.
- **Windowing & Menus:** Native macOS menu bar (`Menu`, `MenuItem`, `SystemMenuType`) and registered `cmd-*` shortcuts operate as documented in [Keyboard Shortcuts](../reference/keyboard-shortcuts.md).
- **PTY & Terminal:** Local shell (`zsh`/`bash`), interactive SSH, resize (`SIGWINCH`), and scrollback work under Retina scaling.
- **Status:** Core workspace, setup wizard, and visual screenshot baseline (`resources/screenshots/shellumi-macos.png`) verified on macOS; full release-matrix interactive sign-off is tracked per release in [Verification Status](verification-status.md).

## Windows (`x86_64-pc-windows-msvc`) — `NOT TESTED` interactively

The following native behaviors compile and pass headless unit/integration tests in CI, but require interactive Windows verification (`NOT TESTED` in current baseline):

- **Subsystem & Subprocesses:** `shellumi.exe` (`#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]`) launches from Explorer, Start Menu, and MSI/portable packages without spawning a stray console window, while background helpers apply `CREATE_NO_WINDOW` and ConPTY remains interactive.
- **Credential Manager:** `WindowsCredentialManagerStore` (`PasswordVault`) persists and retrieves secrets across reboots.
- **SSH Askpass:** Dedicated askpass/credential-helper piped communication works when launched from the GUI subsystem.
- **High-DPI & Clipboard:** Multi-monitor DPI scaling and clipboard copy/paste in terminal and remote desktop views.

## Linux (`x86_64-unknown-linux-gnu`) — `NOT TESTED` interactively

The following native behaviors compile and pass headless unit/integration tests in CI, but require interactive Linux desktop verification (`NOT TESTED` in current baseline):

- **Desktop Entry & Launchers:** `packaging/linux/shellumi.desktop` (`Terminal=false`) launches cleanly from GNOME and KDE application menus under X11 and Wayland without spawning an extra terminal emulator.
- **Secret Service:** `LinuxSecretServiceStore` (`secret-tool` / D-Bus Secret Service) unlocks and stores credentials, and gracefully falls back when no Secret Service daemon is running.
- **Packaging:** `.deb` / `.rpm` / AppImage or binary tarball installs icons, desktop entry, and `shellumi-cli` cleanly.

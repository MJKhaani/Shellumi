# Platform Integration

[Docs Home](../README.md) / [Architecture](README.md) / Platform Integration

---

- **Window Chrome (`crates/shellumi-app/src/ui/window_chrome.rs`):** macOS uses a unified transparent titlebar with traffic-light spacing (`14, 14`); Windows and Linux use `WindowDecorations::Server` (`appears_transparent: false`), rendering client fallback controls only if a compositor forces client decorations. Window close routes through `NavigationRequest::CloseWindow` (`window.remove_window()`).
- **No Startup Console Windows:**
  - Windows `shellumi-app.exe` compiles with `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]` (PE subsystem `WINDOWS_GUI = 2`), while `shellumi-cli.exe` retains `WINDOWS_CUI = 3`.
  - Non-interactive helper subprocesses use `shellumi_core::process::background_command()` (`CREATE_NO_WINDOW` on Windows) without applying it to PTY/ConPTY sessions.
  - Linux `packaging/linux/shellumi.desktop` sets `Terminal=false`.
- **Application Data Paths (`crates/shellumi-app/src/data_path.rs`):**
  - macOS: `~/Library/Application Support/Shellumi`
  - Linux: `$XDG_DATA_HOME/shellumi` or `~/.local/share/shellumi`
  - Windows: `%APPDATA%\Shellumi`
- **Icons (`crates/shellumi-app/src/ui/icon.rs`):** Loads 168 icons across 5 resolutions (`32`, `48`, `64`, `128`, `256`) and 2 states (`regular`, `hover`) from `resources/icon_pack` (overrideable via `SHELLUMI_ICONS_DIR`).

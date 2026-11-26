# Platform UI Guidelines

[Docs Home](../README.md) / [Design](README.md) / Platform Guidelines

---

| Aspect | macOS | Windows & Linux |
| :--- | :--- | :--- |
| **Window Chrome** | Unified transparent titlebar with traffic-light inset (`14, 14`) + native macOS menu bar | Native OS `WindowDecorations::Server` (`appears_transparent: false`); fallback controls only if compositor forces client decorations |
| **Primary Modifier** | `Cmd` (`cmd-*` bindings) | `Ctrl` (`ctrl-*` bindings) |
| **Window Close** | `window.on_window_should_close` routes through `NavigationRequest::CloseWindow`, closing only the target window | Same per-window dirty-guard routing (`window.remove_window()`) |
| **Startup Console** | `.app` bundle / binary launch | Windows `WINDOWS_GUI` (`2`) + `CREATE_NO_WINDOW` helpers; Linux `.desktop` `Terminal=false` |

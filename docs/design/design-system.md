# Design System

[Docs Home](../README.md) / [Design](README.md) / Design System

---

- **Theme Tokens (`crates/shellumi-app/src/theme.rs`):** macOS-inspired dark palette (`WINDOW_BG: 0x191a24`, `CONTENT_BG: 0x1e202d`, `SIDEBAR_BG: 0x1b1c28`, `SURFACE: 0x242636`, `ACCENT_BLUE: 0x1e80ff`) with semantic border, muted text, danger, and warning tokens.
- **Buttons (`ui/components/button.rs`):** All primary, secondary, ghost, and danger actions use `AppButton` and `ButtonVariant` with consistent hover and disabled states.
- **Text & Password Inputs (`crates/shellumi-app/src/text_input.rs`):**
  - Shared `TextInput` surface background (`SURFACE`), focus border highlighting, Tab/Shift+Tab navigation, and clipboard support.
  - Sensitive fields enforce masking (`masked: true`), explicit visibility toggle, and automatic re-masking on blur or inactive window state.
- **No Ad-Hoc Emojis:** Functional UI and documentation use the Shellumi icon pack (`resources/icon_pack/`) rather than decorative unicode emojis.

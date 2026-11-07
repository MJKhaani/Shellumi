#![allow(dead_code)]

use gpui::{rgb, Rgba};

/// Native macOS Dark visual design tokens adhering to Apple Human Interface Guidelines
/// and the neutral graphite + system blue color scheme from the visual design specification.
pub struct MacTheme;

impl MacTheme {
    // Window & layout canvases
    pub const WINDOW_BG: u32 = 0x1c1c1e;
    pub const CONTENT_BG: u32 = 0x18181a;
    pub const SIDEBAR_BG: u32 = 0x252528;
    pub const TOOLBAR_BG: u32 = 0x2c2c2e;

    // Surfaces, cards, and containers
    pub const SURFACE: u32 = 0x2c2c2e;
    pub const SURFACE_HOVER: u32 = 0x38383a;
    pub const SURFACE_ACTIVE: u32 = 0x46464a;
    pub const INPUT_BG: u32 = 0x303034;
    pub const SEPARATOR: u32 = 0x46464a;
    pub const SEPARATOR_SUBTLE: u32 = 0x3a3a3f;

    // Typography hierarchy
    pub const TEXT_PRIMARY: u32 = 0xf5f5f7;
    pub const TEXT_SECONDARY: u32 = 0xa1a1a6;
    pub const TEXT_TERTIARY: u32 = 0x767680;

    // Semantic roles & system accents
    pub const ACCENT_BLUE: u32 = 0x0a84ff;
    pub const ACCENT_BLUE_HOVER: u32 = 0x409cff;
    pub const ACCENT_BLUE_MUTED: u32 = 0x1963a8;
    pub const SUCCESS: u32 = 0x30d158;
    pub const WARNING: u32 = 0xffd60a;
    pub const DANGER: u32 = 0xff453a;

    // Terminal canvas
    pub const TERMINAL_BG: u32 = 0x18181a;
    pub const TERMINAL_FG: u32 = 0xf5f5f7;

    // Geometry
    pub const WINDOW_RADIUS: f32 = 12.0;
    pub const PANEL_RADIUS: f32 = 10.0;
    pub const CONTROL_RADIUS: f32 = 8.0;
    pub const CONTROL_HEIGHT: f32 = 32.0;
    pub const SIDEBAR_WIDTH: f32 = 228.0;
    pub const ROW_HEIGHT: f32 = 40.0;
}

// Convenient helper constructors
#[inline]
pub fn window_bg() -> Rgba {
    rgb(MacTheme::WINDOW_BG)
}

#[inline]
pub fn content_bg() -> Rgba {
    rgb(MacTheme::CONTENT_BG)
}

#[inline]
pub fn sidebar_bg() -> Rgba {
    rgb(MacTheme::SIDEBAR_BG)
}

#[inline]
pub fn toolbar_bg() -> Rgba {
    rgb(MacTheme::TOOLBAR_BG)
}

#[inline]
pub fn surface() -> Rgba {
    rgb(MacTheme::SURFACE)
}

#[inline]
pub fn surface_hover() -> Rgba {
    rgb(MacTheme::SURFACE_HOVER)
}

#[inline]
pub fn input_bg() -> Rgba {
    rgb(MacTheme::INPUT_BG)
}

#[inline]
pub fn separator() -> Rgba {
    rgb(MacTheme::SEPARATOR)
}

#[inline]
pub fn text_primary() -> Rgba {
    rgb(MacTheme::TEXT_PRIMARY)
}

#[inline]
pub fn text_secondary() -> Rgba {
    rgb(MacTheme::TEXT_SECONDARY)
}

#[inline]
pub fn text_tertiary() -> Rgba {
    rgb(MacTheme::TEXT_TERTIARY)
}

#[inline]
pub fn accent_blue() -> Rgba {
    rgb(MacTheme::ACCENT_BLUE)
}

#[inline]
pub fn accent_blue_hover() -> Rgba {
    rgb(MacTheme::ACCENT_BLUE_HOVER)
}

#[inline]
pub fn success() -> Rgba {
    rgb(MacTheme::SUCCESS)
}

#[inline]
pub fn warning() -> Rgba {
    rgb(MacTheme::WARNING)
}

#[inline]
pub fn danger() -> Rgba {
    rgb(MacTheme::DANGER)
}

//! Typed terminal events, terminal modes, and escape-sequence security policies.

use crate::shell_integration::PromptSemanticMark;
use serde::{Deserialize, Serialize};

/// Maximum length of a sanitized window/tab title.
pub const MAX_TITLE_LEN: usize = 256;

/// Maximum length of an OSC 8 hyperlink URI.
pub const MAX_HYPERLINK_URI_LEN: usize = 2048;

/// Maximum decoded payload size for an OSC 52 clipboard write request (64 KiB).
pub const MAX_OSC52_PAYLOAD_BYTES: usize = 64 * 1024;

/// Maximum number of queued terminal events per drain cycle.
pub const MAX_TERMINAL_EVENTS: usize = 1024;

/// Cursor rendering shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Beam,
}

/// Mouse tracking protocol mode negotiated by the remote/local application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseTrackingMode {
    #[default]
    None,
    /// DECSET 9: X10 compatibility mode (button press only).
    X10,
    /// DECSET 1000: Normal tracking mode (button press and release).
    Normal,
    /// DECSET 1002: Button-event tracking (press, release, and motion while pressed).
    ButtonMotion,
    /// DECSET 1003: Any-event tracking (all motion events).
    AnyMotion,
}

/// Terminal protocol modes controlled by DECSET/DECRST and ESC sequences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalModes {
    /// DECCKM (DECSET 1): Application cursor keys (`\x1bOA` vs `\x1b[A`).
    pub application_cursor: bool,
    /// DECKPAM / DECKPNM: Application keypad mode.
    pub application_keypad: bool,
    /// DECAWM (DECSET 7): Auto-wrap mode (default true).
    pub autowrap: bool,
    /// DECOM (DECSET 6): Origin mode (cursor relative to scroll margins).
    pub origin_mode: bool,
    /// IRM (CSI 4 h/l): Insert mode.
    pub insert_mode: bool,
    /// DECSET 2004: Bracketed paste mode.
    pub bracketed_paste: bool,
    /// DECSET 1004: Focus in/out reporting (`\x1b[I` / `\x1b[O`).
    pub focus_reporting: bool,
    /// Active mouse tracking mode.
    pub mouse_mode: MouseTrackingMode,
    /// DECSET 1006: SGR extended mouse coordinate encoding.
    pub sgr_mouse: bool,
    /// DECSCUSR: Cursor shape.
    pub cursor_shape: CursorShape,
    /// DECSET 12 / DECSCUSR: Cursor blinking state.
    pub cursor_blink: bool,
}

impl Default for TerminalModes {
    fn default() -> Self {
        Self {
            application_cursor: false,
            application_keypad: false,
            autowrap: true,
            origin_mode: false,
            insert_mode: false,
            bracketed_paste: false,
            focus_reporting: false,
            mouse_mode: MouseTrackingMode::None,
            sgr_mouse: false,
            cursor_shape: CursorShape::Block,
            cursor_blink: true,
        }
    }
}

/// Validated hyperlink target from OSC 8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HyperlinkTarget {
    pub id: Option<String>,
    pub uri: String,
}

/// Policy for OSC 52 clipboard write requests from terminal streams.
/// Note: OSC 52 clipboard *reads* (`?`) are always denied to prevent data exfiltration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Osc52Policy {
    #[default]
    Deny,
    Prompt,
    AllowWriteOnly,
}

/// Typed events emitted by `Terminal` for consumption by the session/UI controller.
/// Parser callbacks never mutate OS clipboard, audio, or other sessions directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Standalone BEL (0x07) — never emitted when BEL terminates an OSC sequence.
    Bell,
    /// Window/tab title updated via OSC 0 or OSC 2 (already sanitized and bounded).
    TitleChanged(Option<String>),
    /// Current working directory updated via OSC 7 or OSC 9;9.
    CwdChanged(String),
    /// Semantic prompt mark updated via OSC 133.
    PromptMark(PromptSemanticMark),
    /// OSC 52 clipboard write request (decoded UTF-8, bounded). UI must enforce `Osc52Policy`.
    ClipboardWriteRequest(String),
    /// Protocol reply bytes (e.g. DA1, DA2, DSR, CPR) that must be sent ONLY to the originating live session.
    ProtocolReply(Vec<u8>),
    /// Terminal mode flags changed (application cursor/keypad, bracketed paste, mouse, focus, etc.).
    ModeChanged,
}

/// Validates and sanitizes an OSC 8 hyperlink URI.
/// Only allows `http`, `https`, and `mailto` schemes; rejects control characters and oversized URIs.
pub fn sanitize_hyperlink_uri(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_HYPERLINK_URI_LEN {
        return None;
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Sanitizes an OSC 0/2 title string by stripping control characters and bounding length.
pub fn sanitize_title(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .take(MAX_TITLE_LEN)
        .collect()
}

//! Input event abstractions and viewport-to-framebuffer coordinate transformation.

use serde::{Deserialize, Serialize};

/// Display scaling modes for remote desktop viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScaleMode {
    /// Uniformly scale remote display to fit within the viewport, preserving aspect ratio.
    #[default]
    Fit,
    /// 1:1 pixel mapping at remote native resolution.
    Original,
    /// Stretch remote display non-uniformly to completely fill the viewport.
    Fill,
}

/// Standard mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Button4,
    Button5,
}

impl MouseButton {
    /// Return the RFB/domain bitmask bit for this button (0 for extended buttons not in baseline RFB).
    pub fn bit_mask(self) -> u8 {
        match self {
            Self::Left => pointer_mask::BUTTON_LEFT,
            Self::Middle => pointer_mask::BUTTON_MIDDLE,
            Self::Right => pointer_mask::BUTTON_RIGHT,
            Self::Button4 | Self::Button5 => 0,
        }
    }
}

/// Typed remote desktop shortcut sequences (never matched by UI label strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemoteShortcut {
    CtrlAltDel,
    AltTab,
    WinKey,
    CtrlEscape,
    CtrlC,
    CtrlV,
    CtrlX,
    CtrlA,
    CtrlZ,
}

impl RemoteShortcut {
    /// Human-readable label for status messages and diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Self::CtrlAltDel => "Ctrl+Alt+Del",
            Self::AltTab => "Alt+Tab",
            Self::WinKey => "Win/Super",
            Self::CtrlEscape => "Ctrl+Escape",
            Self::CtrlC => "Ctrl+C",
            Self::CtrlV => "Ctrl+V",
            Self::CtrlX => "Ctrl+X",
            Self::CtrlA => "Ctrl+A",
            Self::CtrlZ => "Ctrl+Z",
        }
    }
}

/// Keyboard mapping policy for macOS and cross-platform clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KeyboardMappingMode {
    /// Native physical modifier mapping (Command -> Super/Meta, Option -> Alt, Control -> Control).
    #[default]
    Native,
    /// Convenience mapping on macOS where Cmd+C/V/X/A/Z map to Ctrl+C/V/X/A/Z for Windows/Linux targets.
    MacToWindowsLinux,
}

/// Stable physical key identity captured at key-down so key-up releases the exact same key
/// even if the keyboard layout or modifier state changes before release.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteKey {
    /// X11 keysym (used by VNC/RFB and as fallback).
    pub keysym: u32,
    /// PS/2 Set 1 scancode (used by RDP).
    pub scancode: u16,
    /// Whether the RDP extended flag (0xE0) applies.
    pub extended: bool,
    /// True if this key is a modifier key (Shift, Ctrl, Alt, Super/Meta).
    pub is_modifier: bool,
    /// Canonical key name for diagnostics (never logged for secret text).
    pub key_name: String,
}

/// Semantic normalized remote input event.
///
/// Scroll units: `120` represents one standard wheel detent/notch (matching Windows `WHEEL_DELTA`).
/// Coordinate space: remote framebuffer logical pixels `(0..fb_width, 0..fb_height)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteInput {
    PointerMove {
        x: u32,
        y: u32,
    },
    PointerButton {
        button: MouseButton,
        down: bool,
        x: u32,
        y: u32,
    },
    Scroll {
        x: u32,
        y: u32,
        horizontal: i32,
        vertical: i32,
    },
    PhysicalKey {
        key: RemoteKey,
        down: bool,
        repeat: bool,
    },
    CommittedText(String),
    Shortcut(RemoteShortcut),
    ReleaseAll,
}

/// Bitmask constants for mouse buttons (compatible with RFB 3.8 pointer events).
pub mod pointer_mask {
    pub const BUTTON_LEFT: u8 = 1 << 0;
    pub const BUTTON_MIDDLE: u8 = 1 << 1;
    pub const BUTTON_RIGHT: u8 = 1 << 2;
    pub const BUTTON_WHEEL_UP: u8 = 1 << 3;
    pub const BUTTON_WHEEL_DOWN: u8 = 1 << 4;
}

/// Mouse pointer movement or button press/release event in remote framebuffer coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointerEvent {
    pub x: u32,
    pub y: u32,
    pub button_mask: u8,
    pub pressed: bool,
}

/// Scroll wheel event with directional deltas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WheelEvent {
    pub x: u32,
    pub y: u32,
    pub delta_x: i32,
    pub delta_y: i32,
}

/// Active keyboard modifier flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct KeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

/// Keyboard key press or release event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    /// X11 keysym (standard for RFB / VNC).
    pub keysym: u32,
    /// Hardware/USB scancode (standard for RDP).
    pub scancode: u16,
    /// True for key press / down; false for key release / up.
    pub pressed: bool,
    /// Active modifiers during the key event.
    pub modifiers: KeyModifiers,
    /// Printable text character if applicable.
    pub text: Option<String>,
}

/// Clipboard synchronization payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardEvent {
    pub text: String,
}

/// Per-session pan offset for 100% (`ScaleMode::Original`) mode.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PanState {
    pub x: f64,
    pub y: f64,
}

impl PanState {
    /// Clamp pan offsets so oversized axes stay within `[0, frame - viewport]`
    /// and axes that fit within the viewport reset to `0.0`.
    pub fn clamped(self, viewport_size: (f64, f64), frame_size: (u32, u32)) -> Self {
        let max_x = (frame_size.0 as f64 - viewport_size.0).max(0.0);
        let max_y = (frame_size.1 as f64 - viewport_size.1).max(0.0);
        Self {
            x: if self.x.is_finite() {
                self.x.clamp(0.0, max_x)
            } else {
                0.0
            },
            y: if self.y.is_finite() {
                self.y.clamp(0.0, max_y)
            } else {
                0.0
            },
        }
    }
}

/// Single authoritative transform used for both rendering the remote desktop image
/// and mapping window-local pointer events to framebuffer coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasTransform {
    pub viewport_origin: (f64, f64),
    pub viewport_size: (f64, f64),
    pub frame_size: (u32, u32),
    /// Image top-left origin relative to the viewport's top-left corner.
    pub image_origin: (f64, f64),
    pub image_size: (f64, f64),
    pub scale: (f64, f64),
}

impl CanvasTransform {
    /// Compute the exact image rectangle and pointer transform.
    /// Returns `None` if any dimension is zero or non-finite.
    pub fn new(
        viewport_origin: (f64, f64),
        viewport_size: (f64, f64),
        frame_size: (u32, u32),
        mode: ScaleMode,
        pan: (f64, f64),
    ) -> Option<Self> {
        let (ox, oy) = viewport_origin;
        let (w, h) = viewport_size;
        let (fw, fh) = frame_size;
        if !ox.is_finite()
            || !oy.is_finite()
            || !w.is_finite()
            || !h.is_finite()
            || w <= 0.0
            || h <= 0.0
            || fw == 0
            || fh == 0
        {
            return None;
        }
        let fw_f = fw as f64;
        let fh_f = fh as f64;

        let (scale, image_size, image_origin) = match mode {
            ScaleMode::Fit => {
                let s = (w / fw_f).min(h / fh_f);
                let iw = fw_f * s;
                let ih = fh_f * s;
                let ix = (w - iw) / 2.0;
                let iy = (h - ih) / 2.0;
                ((s, s), (iw, ih), (ix, iy))
            }
            ScaleMode::Fill => {
                let sx = w / fw_f;
                let sy = h / fh_f;
                ((sx, sy), (w, h), (0.0, 0.0))
            }
            ScaleMode::Original => {
                let clamped = PanState { x: pan.0, y: pan.1 }.clamped((w, h), (fw, fh));
                let ix = if fw_f <= w {
                    (w - fw_f) / 2.0
                } else {
                    -clamped.x
                };
                let iy = if fh_f <= h {
                    (h - fh_f) / 2.0
                } else {
                    -clamped.y
                };
                ((1.0, 1.0), (fw_f, fh_f), (ix, iy))
            }
        };

        Some(Self {
            viewport_origin,
            viewport_size,
            frame_size,
            image_origin,
            image_size,
            scale,
        })
    }

    /// Map a window-local logical pointer coordinate `(win_x, win_y)` to remote
    /// framebuffer pixel coordinates `(u32, u32)`.
    ///
    /// Rejects points outside the viewport (`[0, W) x [0, H)`) or outside the
    /// rendered image (`[Ix, Ix+Iw) x [Iy, Iy+Ih)`), never clamping letterbox clicks.
    pub fn map_window_point_to_remote(&self, win_x: f64, win_y: f64) -> Option<(u32, u32)> {
        if !win_x.is_finite() || !win_y.is_finite() {
            return None;
        }
        let local_x = win_x - self.viewport_origin.0;
        let local_y = win_y - self.viewport_origin.1;

        let (w, h) = self.viewport_size;
        if local_x < 0.0 || local_x >= w || local_y < 0.0 || local_y >= h {
            return None;
        }

        let (ix, iy) = self.image_origin;
        let (iw, ih) = self.image_size;
        if local_x < ix || local_x >= ix + iw || local_y < iy || local_y >= iy + ih {
            return None;
        }

        let rx = ((local_x - ix) / self.scale.0).floor() as i64;
        let ry = ((local_y - iy) / self.scale.1).floor() as i64;
        let max_x = self.frame_size.0.saturating_sub(1) as i64;
        let max_y = self.frame_size.1.saturating_sub(1) as i64;

        Some((rx.clamp(0, max_x) as u32, ry.clamp(0, max_y) as u32))
    }
}

/// Translate viewport window pixel coordinates `(screen_x, screen_y)` into remote framebuffer
/// coordinates `(u32, u32)` based on the selected scaling mode and container dimensions.
///
/// Returns `None` if the point is outside the active remote desktop image (e.g. within letterbox margins).
pub fn translate_coordinates(
    screen_x: f32,
    screen_y: f32,
    viewport_width: f32,
    viewport_height: f32,
    fb_width: u32,
    fb_height: u32,
    scale_mode: ScaleMode,
) -> Option<(u32, u32)> {
    let tx = CanvasTransform::new(
        (0.0, 0.0),
        (viewport_width as f64, viewport_height as f64),
        (fb_width, fb_height),
        scale_mode,
        (0.0, 0.0),
    )?;
    tx.map_window_point_to_remote(screen_x as f64, screen_y as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_fit_coordinate_translation_and_letterbox_rejection() {
        // 1920x1080 framebuffer inside a 1000x1000 square viewport
        // Scale should be 1000 / 1920 = 0.520833
        // Rendered height will be 1080 * (1000 / 1920) = 562.5
        // Vertical letterbox margins: top and bottom offset = (1000 - 562.5) / 2 = 218.75
        let fb_w = 1920;
        let fb_h = 1080;
        let vp_w = 1000.0;
        let vp_h = 1000.0;

        // Click in top letterbox margin (y = 100) must return None
        assert_eq!(
            translate_coordinates(500.0, 100.0, vp_w, vp_h, fb_w, fb_h, ScaleMode::Fit),
            None
        );

        // Click in bottom letterbox margin (y = 900) must return None
        assert_eq!(
            translate_coordinates(500.0, 900.0, vp_w, vp_h, fb_w, fb_h, ScaleMode::Fit),
            None
        );

        // Click at exact visual center of active area (500.0, 500.0)
        let center =
            translate_coordinates(500.0, 500.0, vp_w, vp_h, fb_w, fb_h, ScaleMode::Fit).unwrap();
        // Framebuffer center is (960, 540)
        assert_eq!(center, (960, 540));
    }

    #[test]
    fn verifies_original_and_fill_coordinate_translation() {
        let fb_w = 800;
        let fb_h = 600;
        let vp_w = 1200.0;
        let vp_h = 1000.0;

        // Original 1:1 centered: offset_x = (1200 - 800) / 2 = 200, offset_y = (1000 - 600) / 2 = 200
        assert_eq!(
            translate_coordinates(100.0, 100.0, vp_w, vp_h, fb_w, fb_h, ScaleMode::Original),
            None
        );
        let orig_origin =
            translate_coordinates(200.0, 200.0, vp_w, vp_h, fb_w, fb_h, ScaleMode::Original)
                .unwrap();
        assert_eq!(orig_origin, (0, 0));

        // Fill mode: non-uniform stretch across entire viewport
        let fill_bottom_right =
            translate_coordinates(1199.0, 999.0, vp_w, vp_h, fb_w, fb_h, ScaleMode::Fill).unwrap();
        assert_eq!(fill_bottom_right, (799, 599));
    }

    #[test]
    fn verifies_exact_canvas_transform_and_pan_fixtures() {
        // 1. Fit: viewport origin (100,60), 800x800, frame 1920x1080
        let tx_fit = CanvasTransform::new(
            (100.0, 60.0),
            (800.0, 800.0),
            (1920, 1080),
            ScaleMode::Fit,
            (0.0, 0.0),
        )
        .unwrap();
        assert_eq!(tx_fit.image_origin, (0.0, 175.0));
        assert_eq!(tx_fit.image_size, (800.0, 450.0));
        assert_eq!(
            tx_fit.map_window_point_to_remote(500.0, 460.0),
            Some((960, 540))
        );
        assert_eq!(tx_fit.map_window_point_to_remote(500.0, 100.0), None);
        assert_eq!(
            tx_fit.map_window_point_to_remote(100.0, 235.0),
            Some((0, 0))
        );
        // Exclusive right/bottom edges rejected
        assert_eq!(tx_fit.map_window_point_to_remote(900.0, 460.0), None);
        assert_eq!(tx_fit.map_window_point_to_remote(500.0, 685.0), None);

        // 2. Original: viewport 800x600, frame 1920x1080, pan (400,200)
        let tx_orig_pan = CanvasTransform::new(
            (0.0, 0.0),
            (800.0, 600.0),
            (1920, 1080),
            ScaleMode::Original,
            (400.0, 200.0),
        )
        .unwrap();
        assert_eq!(
            tx_orig_pan.map_window_point_to_remote(0.0, 0.0),
            Some((400, 200))
        );
        assert_eq!(
            tx_orig_pan.map_window_point_to_remote(799.0, 599.0),
            Some((1199, 799))
        );

        // 3. Oversized width only: pan clamped on X, centered on Y
        let tx_wide = CanvasTransform::new(
            (0.0, 0.0),
            (800.0, 900.0),
            (1200, 600),
            ScaleMode::Original,
            (999.0, 999.0),
        )
        .unwrap();
        assert_eq!(tx_wide.image_origin, (-400.0, 150.0));

        // 4. Zero / NaN dimensions rejected
        assert!(CanvasTransform::new(
            (0.0, 0.0),
            (0.0, 600.0),
            (800, 600),
            ScaleMode::Fit,
            (0.0, 0.0)
        )
        .is_none());
        assert!(CanvasTransform::new(
            (0.0, 0.0),
            (f64::NAN, 600.0),
            (800, 600),
            ScaleMode::Fit,
            (0.0, 0.0)
        )
        .is_none());
    }
}

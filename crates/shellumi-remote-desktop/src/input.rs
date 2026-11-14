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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Button4,
    Button5,
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
    if viewport_width <= 0.0 || viewport_height <= 0.0 || fb_width == 0 || fb_height == 0 {
        return None;
    }

    match scale_mode {
        ScaleMode::Fit => {
            let scale = (viewport_width / fb_width as f32).min(viewport_height / fb_height as f32);
            let rendered_w = fb_width as f32 * scale;
            let rendered_h = fb_height as f32 * scale;
            let offset_x = (viewport_width - rendered_w) / 2.0;
            let offset_y = (viewport_height - rendered_h) / 2.0;

            // Check if coordinates lie inside the letterbox or pillarbox margins
            if screen_x < offset_x
                || screen_x >= offset_x + rendered_w
                || screen_y < offset_y
                || screen_y >= offset_y + rendered_h
            {
                return None;
            }

            let local_x = ((screen_x - offset_x) / scale).floor() as u32;
            let local_y = ((screen_y - offset_y) / scale).floor() as u32;

            Some((
                local_x.min(fb_width.saturating_sub(1)),
                local_y.min(fb_height.saturating_sub(1)),
            ))
        }
        ScaleMode::Original => {
            let offset_x = ((viewport_width - fb_width as f32) / 2.0).max(0.0);
            let offset_y = ((viewport_height - fb_height as f32) / 2.0).max(0.0);

            if screen_x < offset_x
                || screen_x >= offset_x + fb_width as f32
                || screen_y < offset_y
                || screen_y >= offset_y + fb_height as f32
            {
                return None;
            }

            let local_x = (screen_x - offset_x).floor() as u32;
            let local_y = (screen_y - offset_y).floor() as u32;

            Some((
                local_x.min(fb_width.saturating_sub(1)),
                local_y.min(fb_height.saturating_sub(1)),
            ))
        }
        ScaleMode::Fill => {
            if screen_x < 0.0
                || screen_x >= viewport_width
                || screen_y < 0.0
                || screen_y >= viewport_height
            {
                return None;
            }

            let scale_x = viewport_width / fb_width as f32;
            let scale_y = viewport_height / fb_height as f32;

            let local_x = ((screen_x / scale_x).floor() as u32).min(fb_width.saturating_sub(1));
            let local_y = ((screen_y / scale_y).floor() as u32).min(fb_height.saturating_sub(1));

            Some((local_x, local_y))
        }
    }
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
}

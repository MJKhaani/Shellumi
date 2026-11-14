//! Resource bounds, decompression limits, and security constraints for RFB decoders.

use crate::wire::RectangleHeader;
use shellumi_remote_desktop::{Framebuffer, MAX_FRAMEBUFFER_DIMENSION};

/// Maximum permitted rectangle width or height (RFC 6143).
pub const MAX_RECTANGLE_DIMENSION: u32 = MAX_FRAMEBUFFER_DIMENSION;

/// Maximum compressed rectangle payload accepted from untrusted network stream (32 MiB).
pub const MAX_COMPRESSED_PAYLOAD_SIZE: usize = 32 * 1024 * 1024;

/// Maximum clipboard text size accepted from server or sent to server (2 MiB).
pub const MAX_CLIPBOARD_BYTES: usize = 2 * 1024 * 1024;

/// Validate rectangle coordinates against framebuffer dimensions and security limits.
pub fn validate_rectangle_bounds(
    header: &RectangleHeader,
    fb: &Framebuffer,
) -> Result<(), crate::rectangles::RectangleError> {
    let w = header.width as u32;
    let h = header.height as u32;
    let x = header.x as u32;
    let y = header.y as u32;

    if w == 0 || h == 0 {
        return Err(crate::rectangles::RectangleError::InvalidDimensions {
            width: w,
            height: h,
        });
    }

    if w > MAX_RECTANGLE_DIMENSION || h > MAX_RECTANGLE_DIMENSION {
        return Err(crate::rectangles::RectangleError::InvalidDimensions {
            width: w,
            height: h,
        });
    }

    if x + w > fb.width() || y + h > fb.height() {
        return Err(crate::rectangles::RectangleError::OutOfBounds {
            x,
            y,
            width: w,
            height: h,
            fb_width: fb.width(),
            fb_height: fb.height(),
        });
    }

    Ok(())
}

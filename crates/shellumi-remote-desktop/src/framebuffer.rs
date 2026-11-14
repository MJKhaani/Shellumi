//! Thread-safe 32-bit RGBA framebuffer with dirty rectangle tracking and bounds validation.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Maximum allowed width or height in pixels to guard against memory exhaustion DoS attacks.
pub const MAX_FRAMEBUFFER_DIMENSION: u32 = 8192;

/// Framebuffer manipulation errors.
#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum FramebufferError {
    #[error("Framebuffer dimensions must be greater than zero (requested {width}x{height})")]
    ZeroDimensions { width: u32, height: u32 },

    #[error("Framebuffer dimensions {width}x{height} exceed maximum allowed {max}x{max}")]
    ExceedsMaxDimensions { width: u32, height: u32, max: u32 },

    #[error("Rectangle {rect:?} is out of framebuffer bounds ({width}x{height})")]
    OutOfBounds { rect: Rect, width: u32, height: u32 },

    #[error("Buffer data length mismatch: expected {expected} bytes, received {actual} bytes")]
    DataSizeMismatch { expected: usize, actual: usize },
}

/// A 2D integer rectangle in framebuffer pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Create a new rectangle.
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Return true if the rectangle has zero area.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Right edge coordinate (`x + width`).
    pub const fn right(&self) -> u32 {
        self.x.saturating_add(self.width)
    }

    /// Bottom edge coordinate (`y + height`).
    pub const fn bottom(&self) -> u32 {
        self.y.saturating_add(self.height)
    }

    /// Check if this rectangle intersects another rectangle.
    pub fn intersects(&self, other: &Rect) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Compute the intersection of two rectangles, if any.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        if !self.intersects(other) {
            return None;
        }

        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right > x && bottom > y {
            Some(Rect {
                x,
                y,
                width: right - x,
                height: bottom - y,
            })
        } else {
            None
        }
    }

    /// Compute the minimal bounding rectangle covering both rectangles.
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }

        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());

        Rect {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        }
    }

    /// Clamp this rectangle to the given viewport bounds.
    pub fn clamp_to(&self, max_w: u32, max_h: u32) -> Option<Rect> {
        let viewport = Rect::new(0, 0, max_w, max_h);
        self.intersection(&viewport)
    }
}

/// Accumulator for damaged / modified regions of the framebuffer.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DirtyRegion {
    rects: Vec<Rect>,
}

impl DirtyRegion {
    /// Create an empty dirty region.
    pub fn new() -> Self {
        Self { rects: Vec::new() }
    }

    /// Record a damaged rectangle.
    pub fn push(&mut self, rect: Rect) {
        if !rect.is_empty() {
            self.rects.push(rect);
        }
    }

    /// Check if there are any pending dirty rectangles.
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// Drain all accumulated dirty rectangles.
    pub fn drain(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.rects)
    }

    /// Clear all pending dirty rectangles.
    pub fn clear(&mut self) {
        self.rects.clear();
    }

    /// Compute the aggregate bounding box of all dirty rectangles.
    pub fn bounding_box(&self) -> Option<Rect> {
        self.rects.iter().copied().reduce(|acc, r| acc.union(&r))
    }
}

/// Immutable, thread-safe snapshot of the framebuffer for UI rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramebufferSnapshot {
    pub width: u32,
    pub height: u32,
    pub sequence: u64,
    pub pixels: Arc<[u8]>,
}

impl FramebufferSnapshot {
    /// Total byte length of the RGBA buffer.
    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }

    /// Look up RGBA components at `(x, y)` if within bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = ((y * self.width + x) * 4) as usize;
        if offset + 4 <= self.pixels.len() {
            Some([
                self.pixels[offset],
                self.pixels[offset + 1],
                self.pixels[offset + 2],
                self.pixels[offset + 3],
            ])
        } else {
            None
        }
    }
}

/// 32-bit RGBA framebuffer.
///
/// Pixels are stored row-major in RGBA8888 format (4 bytes per pixel).
#[derive(Debug, Clone)]
pub struct Framebuffer {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    sequence: u64,
    dirty: DirtyRegion,
}

impl Framebuffer {
    /// Initialize a black framebuffer of the given dimensions.
    pub fn new(width: u32, height: u32) -> Result<Self, FramebufferError> {
        validate_dimensions(width, height)?;

        let byte_len = ((width as usize) * (height as usize))
            .checked_mul(4)
            .ok_or(FramebufferError::ExceedsMaxDimensions {
                width,
                height,
                max: MAX_FRAMEBUFFER_DIMENSION,
            })?;

        // Initialize to fully opaque black
        let mut pixels = vec![0u8; byte_len];
        for i in (3..byte_len).step_by(4) {
            pixels[i] = 255;
        }

        let mut fb = Self {
            width,
            height,
            pixels,
            sequence: 1,
            dirty: DirtyRegion::new(),
        };
        fb.dirty.push(Rect::new(0, 0, width, height));
        Ok(fb)
    }

    /// Current width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Current height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Monotonically increasing sequence number incremented on every buffer modification.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Borrow the underlying raw RGBA byte slice.
    pub fn as_slice(&self) -> &[u8] {
        &self.pixels
    }

    /// Look up RGBA components at `(x, y)` if within bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = ((y * self.width + x) * 4) as usize;
        if offset + 4 <= self.pixels.len() {
            Some([
                self.pixels[offset],
                self.pixels[offset + 1],
                self.pixels[offset + 2],
                self.pixels[offset + 3],
            ])
        } else {
            None
        }
    }

    /// Take and clear accumulated dirty rectangles.
    pub fn take_dirty(&mut self) -> Vec<Rect> {
        self.dirty.drain()
    }

    /// Create an immutable snapshot for concurrent rendering.
    pub fn snapshot(&self) -> FramebufferSnapshot {
        FramebufferSnapshot {
            width: self.width,
            height: self.height,
            sequence: self.sequence,
            pixels: Arc::from(self.pixels.as_slice()),
        }
    }

    /// Resize the framebuffer. Existing overlapping pixels are preserved; new regions are filled with black.
    pub fn resize(&mut self, new_width: u32, new_height: u32) -> Result<(), FramebufferError> {
        validate_dimensions(new_width, new_height)?;

        if self.width == new_width && self.height == new_height {
            return Ok(());
        }

        let byte_len = ((new_width as usize) * (new_height as usize))
            .checked_mul(4)
            .ok_or(FramebufferError::ExceedsMaxDimensions {
                width: new_width,
                height: new_height,
                max: MAX_FRAMEBUFFER_DIMENSION,
            })?;

        let mut new_pixels = vec![0u8; byte_len];
        for i in (3..byte_len).step_by(4) {
            new_pixels[i] = 255;
        }

        let copy_w = self.width.min(new_width);
        let copy_h = self.height.min(new_height);

        for row in 0..copy_h {
            let src_start = ((row * self.width) * 4) as usize;
            let src_end = src_start + (copy_w * 4) as usize;
            let dst_start = ((row * new_width) * 4) as usize;
            let dst_end = dst_start + (copy_w * 4) as usize;

            new_pixels[dst_start..dst_end].copy_from_slice(&self.pixels[src_start..src_end]);
        }

        self.width = new_width;
        self.height = new_height;
        self.pixels = new_pixels;
        self.sequence = self.sequence.wrapping_add(1);
        self.dirty.clear();
        self.dirty.push(Rect::new(0, 0, new_width, new_height));

        Ok(())
    }

    /// Update a sub-rectangle with raw RGBA8888 pixel data.
    pub fn update_rect(&mut self, rect: Rect, rgba_data: &[u8]) -> Result<(), FramebufferError> {
        if rect.is_empty() {
            return Ok(());
        }

        if rect.right() > self.width || rect.bottom() > self.height {
            return Err(FramebufferError::OutOfBounds {
                rect,
                width: self.width,
                height: self.height,
            });
        }

        let expected_bytes = (rect.width as usize)
            .checked_mul(rect.height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(FramebufferError::DataSizeMismatch {
                expected: usize::MAX,
                actual: rgba_data.len(),
            })?;

        if rgba_data.len() != expected_bytes {
            return Err(FramebufferError::DataSizeMismatch {
                expected: expected_bytes,
                actual: rgba_data.len(),
            });
        }

        let row_bytes = (rect.width * 4) as usize;
        for r in 0..rect.height {
            let src_start = (r as usize) * row_bytes;
            let src_end = src_start + row_bytes;

            let dst_y = rect.y + r;
            let dst_start = ((dst_y * self.width + rect.x) * 4) as usize;
            let dst_end = dst_start + row_bytes;

            self.pixels[dst_start..dst_end].copy_from_slice(&rgba_data[src_start..src_end]);
        }

        self.sequence = self.sequence.wrapping_add(1);
        self.dirty.push(rect);
        Ok(())
    }

    /// Copy a rectangle of pixels from `(src_x, src_y)` to `dst_rect` (VNC CopyRect encoding).
    pub fn copy_rect(
        &mut self,
        src_x: u32,
        src_y: u32,
        dst_rect: Rect,
    ) -> Result<(), FramebufferError> {
        if dst_rect.is_empty() {
            return Ok(());
        }

        let src_rect = Rect::new(src_x, src_y, dst_rect.width, dst_rect.height);
        if src_rect.right() > self.width || src_rect.bottom() > self.height {
            return Err(FramebufferError::OutOfBounds {
                rect: src_rect,
                width: self.width,
                height: self.height,
            });
        }

        if dst_rect.right() > self.width || dst_rect.bottom() > self.height {
            return Err(FramebufferError::OutOfBounds {
                rect: dst_rect,
                width: self.width,
                height: self.height,
            });
        }

        let row_bytes = (dst_rect.width * 4) as usize;
        let mut row_buffer = vec![0u8; row_bytes];

        // Handle vertical overlap safely by ordering row copies
        if dst_rect.y > src_y {
            // Copy bottom to top
            for r in (0..dst_rect.height).rev() {
                let sy = src_y + r;
                let dy = dst_rect.y + r;
                let src_start = ((sy * self.width + src_x) * 4) as usize;
                let src_end = src_start + row_bytes;
                let dst_start = ((dy * self.width + dst_rect.x) * 4) as usize;
                let dst_end = dst_start + row_bytes;

                row_buffer.copy_from_slice(&self.pixels[src_start..src_end]);
                self.pixels[dst_start..dst_end].copy_from_slice(&row_buffer);
            }
        } else {
            // Copy top to bottom
            for r in 0..dst_rect.height {
                let sy = src_y + r;
                let dy = dst_rect.y + r;
                let src_start = ((sy * self.width + src_x) * 4) as usize;
                let src_end = src_start + row_bytes;
                let dst_start = ((dy * self.width + dst_rect.x) * 4) as usize;
                let dst_end = dst_start + row_bytes;

                row_buffer.copy_from_slice(&self.pixels[src_start..src_end]);
                self.pixels[dst_start..dst_end].copy_from_slice(&row_buffer);
            }
        }

        self.sequence = self.sequence.wrapping_add(1);
        self.dirty.push(dst_rect);
        Ok(())
    }

    /// Fill a rectangle with a solid RGBA color.
    pub fn fill_rect(
        &mut self,
        rect: Rect,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) -> Result<(), FramebufferError> {
        if rect.is_empty() {
            return Ok(());
        }

        if rect.right() > self.width || rect.bottom() > self.height {
            return Err(FramebufferError::OutOfBounds {
                rect,
                width: self.width,
                height: self.height,
            });
        }

        let pixel_bytes = [r, g, b, a];
        let row_pixels = rect.width as usize;
        let mut row_buffer = Vec::with_capacity(row_pixels * 4);
        for _ in 0..row_pixels {
            row_buffer.extend_from_slice(&pixel_bytes);
        }

        let row_bytes = row_pixels * 4;
        for r in 0..rect.height {
            let dy = rect.y + r;
            let dst_start = ((dy * self.width + rect.x) * 4) as usize;
            let dst_end = dst_start + row_bytes;
            self.pixels[dst_start..dst_end].copy_from_slice(&row_buffer);
        }

        self.sequence = self.sequence.wrapping_add(1);
        self.dirty.push(rect);
        Ok(())
    }
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), FramebufferError> {
    if width == 0 || height == 0 {
        return Err(FramebufferError::ZeroDimensions { width, height });
    }
    if width > MAX_FRAMEBUFFER_DIMENSION || height > MAX_FRAMEBUFFER_DIMENSION {
        return Err(FramebufferError::ExceedsMaxDimensions {
            width,
            height,
            max: MAX_FRAMEBUFFER_DIMENSION,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_framebuffer_allocation_and_dimension_validation() {
        assert!(matches!(
            Framebuffer::new(0, 100),
            Err(FramebufferError::ZeroDimensions { .. })
        ));
        assert!(matches!(
            Framebuffer::new(100, 0),
            Err(FramebufferError::ZeroDimensions { .. })
        ));
        assert!(matches!(
            Framebuffer::new(10000, 100),
            Err(FramebufferError::ExceedsMaxDimensions { .. })
        ));

        let fb = Framebuffer::new(800, 600).unwrap();
        assert_eq!(fb.width(), 800);
        assert_eq!(fb.height(), 600);
        assert_eq!(fb.as_slice().len(), 800 * 600 * 4);
        // Default color is opaque black
        assert_eq!(fb.pixel(0, 0), Some([0, 0, 0, 255]));
        assert_eq!(fb.pixel(799, 599), Some([0, 0, 0, 255]));
        assert_eq!(fb.pixel(800, 600), None);
    }

    #[test]
    fn verifies_update_rect_and_clipping() {
        let mut fb = Framebuffer::new(100, 100).unwrap();

        // 2x2 red rectangle
        let red_pixels = [
            255, 0, 0, 255, // (0, 0)
            255, 0, 0, 255, // (1, 0)
            255, 0, 0, 255, // (0, 1)
            255, 0, 0, 255, // (1, 1)
        ];

        let rect = Rect::new(10, 20, 2, 2);
        fb.update_rect(rect, &red_pixels).unwrap();

        assert_eq!(fb.pixel(10, 20), Some([255, 0, 0, 255]));
        assert_eq!(fb.pixel(11, 20), Some([255, 0, 0, 255]));
        assert_eq!(fb.pixel(10, 21), Some([255, 0, 0, 255]));
        assert_eq!(fb.pixel(11, 21), Some([255, 0, 0, 255]));
        // Neighboring pixels remain black
        assert_eq!(fb.pixel(9, 20), Some([0, 0, 0, 255]));
        assert_eq!(fb.pixel(12, 20), Some([0, 0, 0, 255]));

        // Out of bounds update fails cleanly
        let bad_rect = Rect::new(99, 99, 10, 10);
        assert!(matches!(
            fb.update_rect(bad_rect, &red_pixels),
            Err(FramebufferError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn verifies_copy_rect_and_fill_rect() {
        let mut fb = Framebuffer::new(50, 50).unwrap();

        // Fill a 10x10 green box at (5, 5)
        let green_rect = Rect::new(5, 5, 10, 10);
        fb.fill_rect(green_rect, 0, 255, 0, 255).unwrap();

        assert_eq!(fb.pixel(5, 5), Some([0, 255, 0, 255]));
        assert_eq!(fb.pixel(14, 14), Some([0, 255, 0, 255]));

        // Copy from (5, 5) to (20, 20)
        let dst_rect = Rect::new(20, 20, 10, 10);
        fb.copy_rect(5, 5, dst_rect).unwrap();

        assert_eq!(fb.pixel(20, 20), Some([0, 255, 0, 255]));
        assert_eq!(fb.pixel(29, 29), Some([0, 255, 0, 255]));
    }

    #[test]
    fn verifies_dirty_tracking_and_snapshot() {
        let mut fb = Framebuffer::new(200, 200).unwrap();
        // Clear initial dirty rect from creation
        let _ = fb.take_dirty();

        let r1 = Rect::new(10, 10, 20, 20);
        let r2 = Rect::new(50, 50, 30, 30);
        fb.fill_rect(r1, 10, 20, 30, 255).unwrap();
        fb.fill_rect(r2, 40, 50, 60, 255).unwrap();

        let dirty = fb.take_dirty();
        assert_eq!(dirty.len(), 2);
        assert_eq!(dirty[0], r1);
        assert_eq!(dirty[1], r2);

        // Snapshot is immutable and safe across threads
        let snap = fb.snapshot();
        assert_eq!(snap.width, 200);
        assert_eq!(snap.height, 200);
        assert_eq!(snap.pixel(10, 10), Some([10, 20, 30, 255]));
        assert_eq!(snap.pixel(50, 50), Some([40, 50, 60, 255]));
    }

    #[test]
    fn verifies_rect_operations() {
        let r1 = Rect::new(10, 10, 20, 20);
        let r2 = Rect::new(20, 20, 20, 20);
        assert!(r1.intersects(&r2));

        let inter = r1.intersection(&r2).unwrap();
        assert_eq!(inter, Rect::new(20, 20, 10, 10));

        let union = r1.union(&r2);
        assert_eq!(union, Rect::new(10, 10, 30, 30));

        let non_overlap = Rect::new(100, 100, 10, 10);
        assert!(!r1.intersects(&non_overlap));
        assert_eq!(r1.intersection(&non_overlap), None);
    }
}

//! RFB rectangle processing for Raw, CopyRect, and DesktopSize encodings.

use crate::wire::{PixelFormat, RectangleHeader};
use byteorder::{BigEndian, ByteOrder};
use shellumi_remote_desktop::{Framebuffer, Rect};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};

/// Standard RFB encoding constants (RFC 6143 Section 7.7).
pub const ENCODING_RAW: i32 = 0;
pub const ENCODING_COPYRECT: i32 = 1;
pub const ENCODING_RRE: i32 = 2;
pub const ENCODING_HEXTILE: i32 = 5;
pub const ENCODING_ZRLE: i32 = 16;
pub const ENCODING_DESKTOP_SIZE: i32 = -223;
pub const ENCODING_CURSOR: i32 = -239;

/// Rectangle decoding errors.
#[derive(Debug, Error)]
pub enum RectangleError {
    #[error("Unsupported encoding: {0}")]
    UnsupportedEncoding(i32),

    #[error("Framebuffer update failed: {0}")]
    FramebufferError(#[from] shellumi_remote_desktop::FramebufferError),

    #[error("I/O error reading rectangle payload: {0}")]
    Io(#[from] std::io::Error),
}

/// Decode a Raw pixel rectangle directly into the shared framebuffer.
#[allow(clippy::chunks_exact_to_as_chunks, clippy::manual_checked_ops)]
pub async fn decode_raw_rectangle<R: AsyncRead + Unpin>(
    stream: &mut R,
    header: &RectangleHeader,
    server_pf: &PixelFormat,
    fb: &mut Framebuffer,
) -> Result<(), RectangleError> {
    let pixel_count = (header.width as usize) * (header.height as usize);
    let bytes_per_pixel = (server_pf.bits_per_pixel as usize) / 8;
    let total_wire_bytes = pixel_count * bytes_per_pixel;

    let mut raw_bytes = vec![0u8; total_wire_bytes];
    stream.read_exact(&mut raw_bytes).await?;

    let mut rgba_buf = vec![0u8; pixel_count * 4];

    if server_pf.bits_per_pixel == 32 {
        let is_be = server_pf.big_endian_flag != 0;
        let r_shift = server_pf.red_shift;
        let g_shift = server_pf.green_shift;
        let b_shift = server_pf.blue_shift;
        let r_max = server_pf.red_max as u32;
        let g_max = server_pf.green_max as u32;
        let b_max = server_pf.blue_max as u32;

        for (i, chunk) in raw_bytes.chunks_exact(4).enumerate() {
            let pixel_val = if is_be {
                BigEndian::read_u32(chunk)
            } else {
                byteorder::LittleEndian::read_u32(chunk)
            };

            let r = if r_max > 0 {
                (((pixel_val >> r_shift) & r_max) * 255 / r_max) as u8
            } else {
                0
            };
            let g = if g_max > 0 {
                (((pixel_val >> g_shift) & g_max) * 255 / g_max) as u8
            } else {
                0
            };
            let b = if b_max > 0 {
                (((pixel_val >> b_shift) & b_max) * 255 / b_max) as u8
            } else {
                0
            };

            let dst_offset = i * 4;
            rgba_buf[dst_offset] = r;
            rgba_buf[dst_offset + 1] = g;
            rgba_buf[dst_offset + 2] = b;
            rgba_buf[dst_offset + 3] = 255;
        }
    } else if server_pf.bits_per_pixel == 16 {
        for (i, chunk) in raw_bytes.chunks_exact(2).enumerate() {
            let pixel_val = byteorder::LittleEndian::read_u16(chunk) as u32;
            // Standard RGB565 unpacking
            let r = (((pixel_val >> 11) & 0x1F) * 255 / 31) as u8;
            let g = (((pixel_val >> 5) & 0x3F) * 255 / 63) as u8;
            let b = ((pixel_val & 0x1F) * 255 / 31) as u8;

            let dst_offset = i * 4;
            rgba_buf[dst_offset] = r;
            rgba_buf[dst_offset + 1] = g;
            rgba_buf[dst_offset + 2] = b;
            rgba_buf[dst_offset + 3] = 255;
        }
    } else {
        // Fallback: fill with opaque black/gray
        for chunk in rgba_buf.chunks_exact_mut(4) {
            chunk[3] = 255;
        }
    }

    let rect = Rect::new(
        header.x as u32,
        header.y as u32,
        header.width as u32,
        header.height as u32,
    );
    fb.update_rect(rect, &rgba_buf)?;
    Ok(())
}

/// Decode a CopyRect rectangle (RFC 6143 Section 7.7.2).
pub async fn decode_copy_rect<R: AsyncRead + Unpin>(
    stream: &mut R,
    header: &RectangleHeader,
    fb: &mut Framebuffer,
) -> Result<(), RectangleError> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await?;

    let src_x = BigEndian::read_u16(&buf[0..2]) as u32;
    let src_y = BigEndian::read_u16(&buf[2..4]) as u32;

    let dst_rect = Rect::new(
        header.x as u32,
        header.y as u32,
        header.width as u32,
        header.height as u32,
    );

    fb.copy_rect(src_x, src_y, dst_rect)?;
    Ok(())
}

/// Handle DesktopSize pseudo-encoding (RFC 6143 Section 7.7.5).
pub fn handle_desktop_size(
    header: &RectangleHeader,
    fb: &mut Framebuffer,
) -> Result<(), RectangleError> {
    fb.resize(header.width as u32, header.height as u32)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verifies_decode_raw_rectangle_into_framebuffer() {
        let mut fb = Framebuffer::new(10, 10).unwrap();
        let pf = PixelFormat::rgba_32();

        // 2x2 red rectangle in little-endian RGBA32 wire format
        let raw_wire_data = [
            255, 0, 0, 0, // (0, 0)
            255, 0, 0, 0, // (1, 0)
            255, 0, 0, 0, // (0, 1)
            255, 0, 0, 0, // (1, 1)
        ];

        let header = RectangleHeader {
            x: 2,
            y: 3,
            width: 2,
            height: 2,
            encoding_type: ENCODING_RAW,
        };

        let mut stream = &raw_wire_data[..];
        decode_raw_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        assert_eq!(fb.pixel(2, 3), Some([255, 0, 0, 255]));
        assert_eq!(fb.pixel(3, 3), Some([255, 0, 0, 255]));
        assert_eq!(fb.pixel(2, 4), Some([255, 0, 0, 255]));
        assert_eq!(fb.pixel(3, 4), Some([255, 0, 0, 255]));
        // Neighboring pixels remain black
        assert_eq!(fb.pixel(1, 3), Some([0, 0, 0, 255]));
    }

    #[tokio::test]
    async fn verifies_decode_copy_rect_into_framebuffer() {
        let mut fb = Framebuffer::new(20, 20).unwrap();
        // Fill 4x4 blue box at (2, 2)
        fb.fill_rect(Rect::new(2, 2, 4, 4), 0, 0, 255, 255).unwrap();

        let copy_wire_data = [
            0x00, 0x02, // src_x = 2
            0x00, 0x02, // src_y = 2
        ];

        let header = RectangleHeader {
            x: 10,
            y: 10,
            width: 4,
            height: 4,
            encoding_type: ENCODING_COPYRECT,
        };

        let mut stream = &copy_wire_data[..];
        decode_copy_rect(&mut stream, &header, &mut fb)
            .await
            .unwrap();

        assert_eq!(fb.pixel(10, 10), Some([0, 0, 255, 255]));
        assert_eq!(fb.pixel(13, 13), Some([0, 0, 255, 255]));
    }
}

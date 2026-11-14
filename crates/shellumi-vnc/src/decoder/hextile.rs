//! RFB Hextile encoding decoder (RFC 6143 Section 7.7.3).

use crate::decoder::limits::validate_rectangle_bounds;
use crate::rectangles::RectangleError;
use crate::wire::{PixelFormat, RectangleHeader};
use shellumi_remote_desktop::{Framebuffer, Rect};
use tokio::io::{AsyncRead, AsyncReadExt};

pub const HEXTILE_RAW: u8 = 0x01;
pub const HEXTILE_BACKGROUND_SPECIFIED: u8 = 0x02;
pub const HEXTILE_FOREGROUND_SPECIFIED: u8 = 0x04;
pub const HEXTILE_ANY_SUBRECTS: u8 = 0x08;
pub const HEXTILE_SUBRECTS_COLOURED: u8 = 0x10;

/// Decode a Hextile rectangle from an asynchronous stream into the framebuffer.
pub async fn decode_hextile_rectangle<R: AsyncRead + Unpin>(
    stream: &mut R,
    header: &RectangleHeader,
    pf: &PixelFormat,
    fb: &mut Framebuffer,
) -> Result<(), RectangleError> {
    validate_rectangle_bounds(header, fb)?;

    let rect_x = header.x as u32;
    let rect_y = header.y as u32;
    let rect_w = header.width as u32;
    let rect_h = header.height as u32;
    let bpp = pf.bytes_per_pixel();

    let mut last_bg = [0u8, 0, 0, 255];
    let mut last_fg = [255u8, 255, 255, 255];

    let tiles_x = rect_w.div_ceil(16);
    let tiles_y = rect_h.div_ceil(16);

    let mut pixel_buf = vec![0u8; bpp.max(4)];

    for ty in 0..tiles_y {
        let tile_y = ty * 16;
        let tile_h = (rect_h - tile_y).min(16);

        for tx in 0..tiles_x {
            let tile_x = tx * 16;
            let tile_w = (rect_w - tile_x).min(16);

            let subencoding = stream.read_u8().await?;

            if (subencoding & HEXTILE_RAW) != 0 {
                let raw_size = (tile_w as usize) * (tile_h as usize) * bpp;
                let mut raw_bytes = vec![0u8; raw_size];
                stream.read_exact(&mut raw_bytes).await?;

                let pixel_count = (tile_w as usize) * (tile_h as usize);
                let mut rgba_buf = vec![0u8; pixel_count * 4];
                for (i, chunk) in raw_bytes.chunks_exact(bpp).enumerate() {
                    let rgba = pf.unpack_rgba(chunk);
                    rgba_buf[i * 4..i * 4 + 4].copy_from_slice(&rgba);
                }

                let tile_rect = Rect::new(rect_x + tile_x, rect_y + tile_y, tile_w, tile_h);
                fb.update_rect(tile_rect, &rgba_buf)?;
                continue;
            }

            if (subencoding & HEXTILE_BACKGROUND_SPECIFIED) != 0 {
                stream.read_exact(&mut pixel_buf[..bpp]).await?;
                last_bg = pf.unpack_rgba(&pixel_buf[..bpp]);
            }

            if (subencoding & HEXTILE_FOREGROUND_SPECIFIED) != 0 {
                stream.read_exact(&mut pixel_buf[..bpp]).await?;
                last_fg = pf.unpack_rgba(&pixel_buf[..bpp]);
            }

            let tile_rect = Rect::new(rect_x + tile_x, rect_y + tile_y, tile_w, tile_h);
            fb.fill_rect(tile_rect, last_bg[0], last_bg[1], last_bg[2], last_bg[3])?;

            if (subencoding & HEXTILE_ANY_SUBRECTS) != 0 {
                let n_subrects = stream.read_u8().await? as usize;
                let subrects_coloured = (subencoding & HEXTILE_SUBRECTS_COLOURED) != 0;

                for _ in 0..n_subrects {
                    let color = if subrects_coloured {
                        stream.read_exact(&mut pixel_buf[..bpp]).await?;
                        pf.unpack_rgba(&pixel_buf[..bpp])
                    } else {
                        last_fg
                    };

                    let xy = stream.read_u8().await?;
                    let wh = stream.read_u8().await?;

                    let sx = ((xy >> 4) & 0x0F) as u32;
                    let sy = (xy & 0x0F) as u32;
                    let sw = (((wh >> 4) & 0x0F) + 1) as u32;
                    let sh = ((wh & 0x0F) + 1) as u32;

                    if sx < tile_w && sy < tile_h {
                        let actual_w = sw.min(tile_w - sx);
                        let actual_h = sh.min(tile_h - sy);
                        let sub_rect = Rect::new(
                            rect_x + tile_x + sx,
                            rect_y + tile_y + sy,
                            actual_w,
                            actual_h,
                        );
                        fb.fill_rect(sub_rect, color[0], color[1], color[2], color[3])?;
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verifies_hextile_raw_tile() {
        let mut fb = Framebuffer::new(32, 32).unwrap();
        let pf = PixelFormat::rgba_32();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            encoding_type: 5,
        };

        // Subencoding = RAW (0x01)
        let mut wire_data = vec![HEXTILE_RAW];
        // 2x2 pixels = 4 pixels, 4 bytes each = 16 bytes (Blue pixels)
        for _ in 0..4 {
            wire_data.extend_from_slice(&[0, 0, 255, 0]);
        }

        let mut stream = &wire_data[..];
        decode_hextile_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        assert_eq!(fb.pixel(0, 0), Some([0, 0, 255, 255]));
        assert_eq!(fb.pixel(1, 1), Some([0, 0, 255, 255]));
        assert_eq!(fb.pixel(2, 2), Some([0, 0, 0, 255])); // untouched
    }

    #[tokio::test]
    async fn verifies_hextile_bg_fg_and_subrects() {
        let mut fb = Framebuffer::new(32, 32).unwrap();
        let pf = PixelFormat::rgba_32();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
            encoding_type: 5,
        };

        let mut wire_data = vec![
            HEXTILE_BACKGROUND_SPECIFIED | HEXTILE_FOREGROUND_SPECIFIED | HEXTILE_ANY_SUBRECTS,
        ];
        // BG: Red [255, 0, 0, 0]
        wire_data.extend_from_slice(&[255, 0, 0, 0]);
        // FG: Green [0, 255, 0, 0]
        wire_data.extend_from_slice(&[0, 255, 0, 0]);
        // 1 subrectangle
        wire_data.push(1);
        // xy: x=1, y=1 -> (1 << 4) | 1 = 0x11
        wire_data.push(0x11);
        // wh: w=2 (2 - 1 = 1), h=2 (2 - 1 = 1) -> 0x11
        wire_data.push(0x11);

        let mut stream = &wire_data[..];
        decode_hextile_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        // (0, 0) should be background (Red)
        assert_eq!(fb.pixel(0, 0), Some([255, 0, 0, 255]));
        // (1, 1) and (2, 2) should be foreground (Green)
        assert_eq!(fb.pixel(1, 1), Some([0, 255, 0, 255]));
        assert_eq!(fb.pixel(2, 2), Some([0, 255, 0, 255]));
        // (3, 3) should be background (Red)
        assert_eq!(fb.pixel(3, 3), Some([255, 0, 0, 255]));
    }

    #[tokio::test]
    async fn verifies_hextile_subrects_coloured() {
        let mut fb = Framebuffer::new(16, 16).unwrap();
        let pf = PixelFormat::rgba_32();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            encoding_type: 5,
        };

        let mut wire_data =
            vec![HEXTILE_BACKGROUND_SPECIFIED | HEXTILE_ANY_SUBRECTS | HEXTILE_SUBRECTS_COLOURED];
        // BG: Black [0, 0, 0, 0]
        wire_data.extend_from_slice(&[0, 0, 0, 0]);
        // 1 subrectangle
        wire_data.push(1);
        // Subrect Color: Yellow [255, 255, 0, 0]
        wire_data.extend_from_slice(&[255, 255, 0, 0]);
        // xy: x=0, y=0 -> 0x00
        wire_data.push(0x00);
        // wh: w=1, h=1 -> (0 << 4) | 0 = 0x00
        wire_data.push(0x00);

        let mut stream = &wire_data[..];
        decode_hextile_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        assert_eq!(fb.pixel(0, 0), Some([255, 255, 0, 255]));
        assert_eq!(fb.pixel(1, 1), Some([0, 0, 0, 255]));
    }

    #[tokio::test]
    async fn verifies_hextile_bounds_rejection() {
        let mut fb = Framebuffer::new(10, 10).unwrap();
        let pf = PixelFormat::rgba_32();

        let header = RectangleHeader {
            x: 8,
            y: 8,
            width: 5, // 8 + 5 = 13 > 10
            height: 5,
            encoding_type: 5,
        };

        let wire_data = [HEXTILE_RAW];
        let mut stream = &wire_data[..];
        let res = decode_hextile_rectangle(&mut stream, &header, &pf, &mut fb).await;
        assert!(matches!(res, Err(RectangleError::OutOfBounds { .. })));
    }
}

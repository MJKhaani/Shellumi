//! RFB Tight encoding decoder with pure Rust JPEG and zlib decompression (no turbojpeg binary required).

use crate::decoder::limits::{validate_rectangle_bounds, MAX_COMPRESSED_PAYLOAD_SIZE};
use crate::rectangles::RectangleError;
use crate::wire::{PixelFormat, RectangleHeader};
use flate2::{Decompress, FlushDecompress, Status};
use shellumi_remote_desktop::{Framebuffer, Rect};
use tokio::io::{AsyncRead, AsyncReadExt};

pub const TIGHT_SUBENCODING_BASIC: u8 = 0x00;
pub const TIGHT_SUBENCODING_FILL: u8 = 0x08;
pub const TIGHT_SUBENCODING_JPEG: u8 = 0x09;

pub const TIGHT_FILTER_COPY: u8 = 0x00;
pub const TIGHT_FILTER_PALETTE: u8 = 0x01;
pub const TIGHT_FILTER_GRADIENT: u8 = 0x02;

/// Persistent Tight decoder holding 4 independent zlib streams across the RFB session.
pub struct TightDecoder {
    zlib_streams: [Decompress; 4],
}

impl Default for TightDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl TightDecoder {
    pub fn new() -> Self {
        Self {
            zlib_streams: [
                Decompress::new(true),
                Decompress::new(true),
                Decompress::new(true),
                Decompress::new(true),
            ],
        }
    }

    /// Reset all 4 zlib streams (e.g. on session reconnect).
    pub fn reset_all(&mut self) {
        for s in &mut self.zlib_streams {
            s.reset(true);
        }
    }

    /// Decode a Tight rectangle from an asynchronous stream into the framebuffer.
    pub async fn decode_rectangle<R: AsyncRead + Unpin>(
        &mut self,
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
        let cpixel_len = pf.bytes_per_cpixel();

        let comp_ctrl = stream.read_u8().await?;

        // Reset streams if requested by high nibble
        for (i, s) in self.zlib_streams.iter_mut().enumerate() {
            if (comp_ctrl & (1 << (4 + i))) != 0 {
                s.reset(true);
            }
        }

        let subencoding = comp_ctrl & 0x0F;

        match subencoding {
            TIGHT_SUBENCODING_FILL => {
                let mut pixel_buf = vec![0u8; cpixel_len];
                stream.read_exact(&mut pixel_buf).await?;
                let color = pf.unpack_rgba(&pixel_buf);
                fb.fill_rect(
                    Rect::new(rect_x, rect_y, rect_w, rect_h),
                    color[0],
                    color[1],
                    color[2],
                    color[3],
                )?;
            }
            TIGHT_SUBENCODING_JPEG => {
                let jpeg_len = read_compact_len(stream).await?;
                if jpeg_len > MAX_COMPRESSED_PAYLOAD_SIZE {
                    return Err(RectangleError::PayloadTooLarge {
                        size: jpeg_len,
                        limit: MAX_COMPRESSED_PAYLOAD_SIZE,
                    });
                }

                let mut jpeg_buf = vec![0u8; jpeg_len];
                stream.read_exact(&mut jpeg_buf).await?;

                // Pure Rust JPEG decoding with image crate (using zune-jpeg backend)
                let img = image::load_from_memory_with_format(&jpeg_buf, image::ImageFormat::Jpeg)
                    .map_err(|e| {
                        RectangleError::DecompressionFailed(format!("JPEG decode failed: {e}"))
                    })?;
                let rgba = img.to_rgba8();

                // Blit into framebuffer
                let actual_w = (rect_w).min(rgba.width());
                let actual_h = (rect_h).min(rgba.height());

                if actual_w == rect_w
                    && actual_h == rect_h
                    && rgba.width() == rect_w
                    && rgba.height() == rect_h
                {
                    fb.update_rect(Rect::new(rect_x, rect_y, rect_w, rect_h), rgba.as_raw())?;
                } else {
                    // Extract matching sub-rectangle
                    let mut tile_rgba = vec![0u8; (rect_w as usize) * (rect_h as usize) * 4];
                    for row in 0..actual_h {
                        let src_start = (row * rgba.width() * 4) as usize;
                        let src_end = src_start + (actual_w * 4) as usize;
                        let dst_start = (row * rect_w * 4) as usize;
                        let dst_end = dst_start + (actual_w * 4) as usize;
                        tile_rgba[dst_start..dst_end]
                            .copy_from_slice(&rgba.as_raw()[src_start..src_end]);
                    }
                    fb.update_rect(Rect::new(rect_x, rect_y, rect_w, rect_h), &tile_rgba)?;
                }
            }
            TIGHT_SUBENCODING_BASIC => {
                let stream_id = ((comp_ctrl >> 4) & 0x03) as usize;
                let has_filter = (comp_ctrl & 0x40) != 0;

                let mut filter_id = TIGHT_FILTER_COPY;
                let mut palette = Vec::new();

                if has_filter {
                    filter_id = stream.read_u8().await?;
                    if filter_id == TIGHT_FILTER_PALETTE {
                        let palette_size_minus_1 = stream.read_u8().await? as usize;
                        let palette_size = palette_size_minus_1 + 1;
                        let mut p_bytes = vec![0u8; palette_size * cpixel_len];
                        stream.read_exact(&mut p_bytes).await?;
                        for chunk in p_bytes.chunks_exact(cpixel_len) {
                            palette.push(pf.unpack_rgba(chunk));
                        }
                    }
                }

                let pixel_count = (rect_w as usize) * (rect_h as usize);
                let uncompressed_len = match filter_id {
                    TIGHT_FILTER_PALETTE => {
                        if palette.len() <= 2 {
                            pixel_count.div_ceil(8)
                        } else {
                            pixel_count
                        }
                    }
                    _ => pixel_count * cpixel_len,
                };

                let raw_data = if uncompressed_len < 12 {
                    let mut data = vec![0u8; uncompressed_len];
                    stream.read_exact(&mut data).await?;
                    data
                } else {
                    let comp_len = read_compact_len(stream).await?;
                    if comp_len > MAX_COMPRESSED_PAYLOAD_SIZE {
                        return Err(RectangleError::PayloadTooLarge {
                            size: comp_len,
                            limit: MAX_COMPRESSED_PAYLOAD_SIZE,
                        });
                    }
                    let mut comp_data = vec![0u8; comp_len];
                    stream.read_exact(&mut comp_data).await?;

                    let mut decompressed = vec![0u8; uncompressed_len];
                    let stream_dec = &mut self.zlib_streams[stream_id];

                    let _before_in = stream_dec.total_in();
                    let before_out = stream_dec.total_out();

                    let status = stream_dec
                        .decompress(&comp_data, &mut decompressed, FlushDecompress::Sync)
                        .map_err(|e| {
                            RectangleError::DecompressionFailed(format!(
                                "Tight zlib decode failed: {e}"
                            ))
                        })?;

                    let produced = (stream_dec.total_out() - before_out) as usize;
                    if produced < uncompressed_len
                        && status != Status::StreamEnd
                        && status != Status::Ok
                    {
                        return Err(RectangleError::DecompressionFailed(
                            "Tight zlib produced truncated output".into(),
                        ));
                    }
                    decompressed
                };

                let mut rgba_buf = vec![0u8; pixel_count * 4];

                match filter_id {
                    TIGHT_FILTER_PALETTE => {
                        if palette.len() <= 2 {
                            // 1 bit per pixel
                            for i in 0..pixel_count {
                                let byte_idx = i / 8;
                                let bit_shift = 7 - (i % 8);
                                let pal_idx = ((raw_data[byte_idx] >> bit_shift) & 1) as usize;
                                let color = if pal_idx < palette.len() {
                                    palette[pal_idx]
                                } else {
                                    [0, 0, 0, 255]
                                };
                                rgba_buf[i * 4..i * 4 + 4].copy_from_slice(&color);
                            }
                        } else {
                            // 8 bits per pixel palette index
                            for (i, &idx) in raw_data.iter().take(pixel_count).enumerate() {
                                let pal_idx = idx as usize;
                                let color = if pal_idx < palette.len() {
                                    palette[pal_idx]
                                } else {
                                    [0, 0, 0, 255]
                                };
                                rgba_buf[i * 4..i * 4 + 4].copy_from_slice(&color);
                            }
                        }
                    }
                    #[allow(clippy::needless_range_loop)]
                    TIGHT_FILTER_GRADIENT => {
                        // 2D Gradient filter: pixel(x, y) = raw(x, y) + (left + top - top_left)
                        let mut prev_row = vec![[0i32; 3]; rect_w as usize];
                        for y in 0..rect_h as usize {
                            let mut prev_pixel = [0i32; 3];
                            let mut prev_diag = [0i32; 3];
                            for x in 0..rect_w as usize {
                                let idx = (y * (rect_w as usize) + x) * cpixel_len;
                                if idx + cpixel_len <= raw_data.len() {
                                    let raw_color =
                                        pf.unpack_rgba(&raw_data[idx..idx + cpixel_len]);
                                    let top = prev_row[x];
                                    let left = prev_pixel;
                                    let diag = prev_diag;

                                    let mut reconstructed = [0u8; 4];
                                    for c in 0..3 {
                                        let pred = left[c] + top[c] - diag[c];
                                        let clamped_pred = pred.clamp(0, 255);
                                        let final_val = (raw_color[c] as i32 + clamped_pred) & 0xFF;
                                        reconstructed[c] = final_val as u8;
                                    }
                                    reconstructed[3] = 255;

                                    prev_diag = top;
                                    prev_pixel = [
                                        reconstructed[0] as i32,
                                        reconstructed[1] as i32,
                                        reconstructed[2] as i32,
                                    ];
                                    prev_row[x] = prev_pixel;

                                    let out_idx = (y * (rect_w as usize) + x) * 4;
                                    rgba_buf[out_idx..out_idx + 4].copy_from_slice(&reconstructed);
                                }
                            }
                        }
                    }
                    _ => {
                        // FILTER_COPY or no filter: unpack raw CPIXELs
                        for (i, chunk) in raw_data
                            .chunks_exact(cpixel_len)
                            .take(pixel_count)
                            .enumerate()
                        {
                            let rgba = pf.unpack_rgba(chunk);
                            rgba_buf[i * 4..i * 4 + 4].copy_from_slice(&rgba);
                        }
                    }
                }

                fb.update_rect(Rect::new(rect_x, rect_y, rect_w, rect_h), &rgba_buf)?;
            }
            other => {
                return Err(RectangleError::InvalidSubencoding(other));
            }
        }

        Ok(())
    }
}

async fn read_compact_len<R: AsyncRead + Unpin>(stream: &mut R) -> Result<usize, std::io::Error> {
    let mut len = stream.read_u8().await? as usize;
    if (len & 0x80) != 0 {
        len &= 0x7F;
        let b1 = stream.read_u8().await? as usize;
        len |= (b1 & 0x7F) << 7;
        if (b1 & 0x80) != 0 {
            let b2 = stream.read_u8().await? as usize;
            len |= b2 << 14;
        }
    }
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn write_compact_len(buf: &mut Vec<u8>, mut len: usize) {
        let b0 = (len & 0x7F) as u8;
        len >>= 7;
        if len == 0 {
            buf.push(b0);
        } else {
            buf.push(b0 | 0x80);
            let b1 = (len & 0x7F) as u8;
            len >>= 7;
            if len == 0 {
                buf.push(b1);
            } else {
                buf.push(b1 | 0x80);
                buf.push((len & 0xFF) as u8);
            }
        }
    }

    #[tokio::test]
    async fn verifies_tight_fill_compression() {
        let mut fb = Framebuffer::new(20, 20).unwrap();
        let pf = PixelFormat::rgba_32();
        let mut decoder = TightDecoder::new();

        let header = RectangleHeader {
            x: 2,
            y: 2,
            width: 4,
            height: 4,
            encoding_type: 7,
        };

        // comp_ctrl = 0x08 (FILL)
        // 3-byte CPIXEL = [150, 75, 200]
        let wire_data = [TIGHT_SUBENCODING_FILL, 150, 75, 200];

        let mut stream = &wire_data[..];
        decoder
            .decode_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        assert_eq!(fb.pixel(2, 2), Some([150, 75, 200, 255]));
        assert_eq!(fb.pixel(5, 5), Some([150, 75, 200, 255]));
        assert_eq!(fb.pixel(0, 0), Some([0, 0, 0, 255])); // untouched
    }

    #[tokio::test]
    async fn verifies_tight_jpeg_decompression() {
        let mut fb = Framebuffer::new(20, 20).unwrap();
        let pf = PixelFormat::rgba_32();
        let mut decoder = TightDecoder::new();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
            encoding_type: 7,
        };

        // Create a 8x8 solid image and encode as JPEG in memory
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb([200, 100, 50]));
        let mut jpeg_bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut jpeg_bytes),
            image::ImageFormat::Jpeg,
        )
        .unwrap();

        let mut wire_data = vec![TIGHT_SUBENCODING_JPEG];
        write_compact_len(&mut wire_data, jpeg_bytes.len());
        wire_data.extend_from_slice(&jpeg_bytes);

        let mut stream = &wire_data[..];
        decoder
            .decode_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        // Due to lossy JPEG DCT, colors will be within ~5 of original
        let px = fb.pixel(0, 0).unwrap();
        assert!((px[0] as i32 - 200).abs() < 10);
        assert!((px[1] as i32 - 100).abs() < 10);
        assert!((px[2] as i32 - 50).abs() < 10);
        assert_eq!(px[3], 255);
    }

    #[tokio::test]
    async fn verifies_tight_basic_palette_zlib() {
        let mut fb = Framebuffer::new(32, 32).unwrap();
        let pf = PixelFormat::rgba_32();
        let mut decoder = TightDecoder::new();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 16,
            height: 8, // 128 pixels -> 16 bytes at 1 bpp (>= 12 bytes, so zlib compressed)
            encoding_type: 7,
        };

        // comp_ctrl = 0x40 (explicit filter) | 0x00 (BASIC)
        let comp_ctrl = 0x40 | TIGHT_SUBENCODING_BASIC;
        // filter = 0x01 (PALETTE), palette_size_minus_1 = 1 (2 colors)
        let filter_id = TIGHT_FILTER_PALETTE;
        let pal_size_minus_1 = 1u8;
        // 2 CPIXELs: Red [255, 0, 0], Green [0, 255, 0]
        let pal_colors = [255, 0, 0, 0, 255, 0];

        // 128 pixels at 1 bit/pixel = 16 bytes: alternating 0b1010_1010
        let raw_bits = vec![0b1010_1010u8; 16];

        // Compress raw_bits with zlib
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw_bits).unwrap();
        let comp_data = encoder.finish().unwrap();

        let mut wire_data = vec![comp_ctrl, filter_id, pal_size_minus_1];
        wire_data.extend_from_slice(&pal_colors);
        write_compact_len(&mut wire_data, comp_data.len());
        wire_data.extend_from_slice(&comp_data);

        let mut stream = &wire_data[..];
        decoder
            .decode_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        // 0b10101010: bit 7 (MSB) is 1 -> Green, bit 6 is 0 -> Red
        assert_eq!(fb.pixel(0, 0), Some([0, 255, 0, 255]));
        assert_eq!(fb.pixel(1, 0), Some([255, 0, 0, 255]));
        assert_eq!(fb.pixel(2, 0), Some([0, 255, 0, 255]));
        assert_eq!(fb.pixel(3, 0), Some([255, 0, 0, 255]));
    }

    #[tokio::test]
    async fn verifies_tight_basic_uncompressed() {
        let mut fb = Framebuffer::new(10, 10).unwrap();
        let pf = PixelFormat::rgba_32();
        let mut decoder = TightDecoder::new();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 1,
            height: 1, // 1 pixel = 3 bytes (< 12 bytes -> sent uncompressed)
            encoding_type: 7,
        };

        // comp_ctrl = 0x00 (BASIC, copy filter, no explicit filter)
        // 3-byte CPIXEL = [11, 22, 33]
        let wire_data = [TIGHT_SUBENCODING_BASIC, 11, 22, 33];

        let mut stream = &wire_data[..];
        decoder
            .decode_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        assert_eq!(fb.pixel(0, 0), Some([11, 22, 33, 255]));
    }
}

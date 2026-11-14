//! RFB ZRLE (Zlib Run-Length Encoding) decoder (RFC 6143 Section 7.7.4).

use crate::decoder::limits::{validate_rectangle_bounds, MAX_COMPRESSED_PAYLOAD_SIZE};
use crate::rectangles::RectangleError;
use crate::wire::{PixelFormat, RectangleHeader};
use byteorder::{BigEndian, ByteOrder};
use flate2::{Decompress, FlushDecompress, Status};
use shellumi_remote_desktop::{Framebuffer, Rect};
use tokio::io::{AsyncRead, AsyncReadExt};

/// Persistent ZRLE decoder maintaining the zlib decompression stream across the session.
pub struct ZrleDecoder {
    decompressor: Decompress,
}

impl Default for ZrleDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ZrleDecoder {
    pub fn new() -> Self {
        Self {
            decompressor: Decompress::new(true),
        }
    }

    /// Reset internal zlib stream state if requested or on session reconnect.
    pub fn reset(&mut self) {
        self.decompressor.reset(true);
    }

    /// Decode a ZRLE rectangle from an asynchronous stream into the framebuffer.
    pub async fn decode_rectangle<R: AsyncRead + Unpin>(
        &mut self,
        stream: &mut R,
        header: &RectangleHeader,
        pf: &PixelFormat,
        fb: &mut Framebuffer,
    ) -> Result<(), RectangleError> {
        validate_rectangle_bounds(header, fb)?;

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let compressed_len = BigEndian::read_u32(&len_buf) as usize;

        if compressed_len > MAX_COMPRESSED_PAYLOAD_SIZE {
            return Err(RectangleError::PayloadTooLarge {
                size: compressed_len,
                limit: MAX_COMPRESSED_PAYLOAD_SIZE,
            });
        }

        let mut compressed_data = vec![0u8; compressed_len];
        stream.read_exact(&mut compressed_data).await?;

        let rect_x = header.x as u32;
        let rect_y = header.y as u32;
        let rect_w = header.width as u32;
        let rect_h = header.height as u32;

        let tiles_x = rect_w.div_ceil(64);
        let tiles_y = rect_h.div_ceil(64);
        let num_tiles = (tiles_x as usize) * (tiles_y as usize);

        // Security bound against decompression bombs
        let max_decompressed_bytes = num_tiles * (1 + 64 * 64 * 4) + 65536;

        let mut decompressed =
            Vec::with_capacity(compressed_len.saturating_mul(2).min(1024 * 1024));
        let mut chunk = [0u8; 16384];
        let mut in_pos = 0;

        while in_pos < compressed_data.len() {
            let before_in = self.decompressor.total_in();
            let before_out = self.decompressor.total_out();

            let status = self
                .decompressor
                .decompress(
                    &compressed_data[in_pos..],
                    &mut chunk,
                    FlushDecompress::Sync,
                )
                .map_err(|e| RectangleError::DecompressionFailed(e.to_string()))?;

            let in_consumed = (self.decompressor.total_in() - before_in) as usize;
            let out_produced = (self.decompressor.total_out() - before_out) as usize;

            in_pos += in_consumed;
            if out_produced > 0 {
                if decompressed.len() + out_produced > max_decompressed_bytes {
                    return Err(RectangleError::DecompressionBombDetected {
                        produced: decompressed.len() + out_produced,
                        limit: max_decompressed_bytes,
                    });
                }
                decompressed.extend_from_slice(&chunk[..out_produced]);
            }

            if status == Status::StreamEnd || (in_consumed == 0 && out_produced == 0) {
                break;
            }
        }

        // Parse tiles from decompressed buffer
        let mut pos = 0;
        let cpixel_len = pf.bytes_per_cpixel();

        for ty in 0..tiles_y {
            let tile_y = ty * 64;
            let tile_h = (rect_h - tile_y).min(64);

            for tx in 0..tiles_x {
                let tile_x = tx * 64;
                let tile_w = (rect_w - tile_x).min(64);
                let pixel_count = (tile_w as usize) * (tile_h as usize);

                let subencoding = read_byte(&decompressed, &mut pos)?;

                match subencoding {
                    0 => {
                        // Raw tile
                        let needed = pixel_count * cpixel_len;
                        let raw = read_bytes(&decompressed, &mut pos, needed)?;
                        let mut tile_rgba = vec![0u8; pixel_count * 4];
                        for (i, chunk) in raw.chunks_exact(cpixel_len).enumerate() {
                            let rgba = pf.unpack_rgba(chunk);
                            tile_rgba[i * 4..i * 4 + 4].copy_from_slice(&rgba);
                        }
                        fb.update_rect(
                            Rect::new(rect_x + tile_x, rect_y + tile_y, tile_w, tile_h),
                            &tile_rgba,
                        )?;
                    }
                    1 => {
                        // Solid color
                        let pixel_data = read_bytes(&decompressed, &mut pos, cpixel_len)?;
                        let color = pf.unpack_rgba(pixel_data);
                        fb.fill_rect(
                            Rect::new(rect_x + tile_x, rect_y + tile_y, tile_w, tile_h),
                            color[0],
                            color[1],
                            color[2],
                            color[3],
                        )?;
                    }
                    2..=16 => {
                        // Packed palette
                        let p_size = subencoding as usize;
                        let mut palette = Vec::with_capacity(p_size);
                        for _ in 0..p_size {
                            let p_data = read_bytes(&decompressed, &mut pos, cpixel_len)?;
                            palette.push(pf.unpack_rgba(p_data));
                        }

                        let m = if p_size == 2 {
                            1
                        } else if p_size <= 4 {
                            2
                        } else {
                            4
                        };

                        let packed_len = (pixel_count * m).div_ceil(8);
                        let packed_data = read_bytes(&decompressed, &mut pos, packed_len)?;

                        let mut tile_rgba = vec![0u8; pixel_count * 4];
                        let mask = (1 << m) - 1;
                        for i in 0..pixel_count {
                            let byte_idx = (i * m) / 8;
                            let bit_shift = 8 - m - ((i * m) % 8);
                            let pal_idx = ((packed_data[byte_idx] >> bit_shift) & mask) as usize;
                            let color = if pal_idx < palette.len() {
                                palette[pal_idx]
                            } else {
                                [0, 0, 0, 255]
                            };
                            tile_rgba[i * 4..i * 4 + 4].copy_from_slice(&color);
                        }

                        fb.update_rect(
                            Rect::new(rect_x + tile_x, rect_y + tile_y, tile_w, tile_h),
                            &tile_rgba,
                        )?;
                    }
                    128 => {
                        // Plain RLE
                        let mut tile_rgba = vec![0u8; pixel_count * 4];
                        let mut written = 0;
                        while written < pixel_count {
                            let p_data = read_bytes(&decompressed, &mut pos, cpixel_len)?;
                            let color = pf.unpack_rgba(p_data);
                            let mut run_len = 1;
                            loop {
                                let b = read_byte(&decompressed, &mut pos)?;
                                run_len += b as usize;
                                if b != 255 {
                                    break;
                                }
                            }
                            let count = run_len.min(pixel_count - written);
                            for j in 0..count {
                                let idx = (written + j) * 4;
                                tile_rgba[idx..idx + 4].copy_from_slice(&color);
                            }
                            written += count;
                        }

                        fb.update_rect(
                            Rect::new(rect_x + tile_x, rect_y + tile_y, tile_w, tile_h),
                            &tile_rgba,
                        )?;
                    }
                    130..=255 => {
                        // Palette RLE
                        let p_size = (subencoding - 128) as usize;
                        let mut palette = Vec::with_capacity(p_size);
                        for _ in 0..p_size {
                            let p_data = read_bytes(&decompressed, &mut pos, cpixel_len)?;
                            palette.push(pf.unpack_rgba(p_data));
                        }

                        let mut tile_rgba = vec![0u8; pixel_count * 4];
                        let mut written = 0;
                        while written < pixel_count {
                            let b = read_byte(&decompressed, &mut pos)?;
                            let (pal_idx, run_len) = if (b & 0x80) == 0 {
                                (b as usize, 1)
                            } else {
                                let idx = (b & 0x7F) as usize;
                                let mut r = 2;
                                loop {
                                    let lb = read_byte(&decompressed, &mut pos)?;
                                    r += lb as usize;
                                    if lb != 255 {
                                        break;
                                    }
                                }
                                (idx, r)
                            };

                            let color = if pal_idx < palette.len() {
                                palette[pal_idx]
                            } else {
                                [0, 0, 0, 255]
                            };
                            let count = run_len.min(pixel_count - written);
                            for j in 0..count {
                                let offset = (written + j) * 4;
                                tile_rgba[offset..offset + 4].copy_from_slice(&color);
                            }
                            written += count;
                        }

                        fb.update_rect(
                            Rect::new(rect_x + tile_x, rect_y + tile_y, tile_w, tile_h),
                            &tile_rgba,
                        )?;
                    }
                    other => {
                        return Err(RectangleError::InvalidSubencoding(other));
                    }
                }
            }
        }

        Ok(())
    }
}

fn read_byte(buf: &[u8], pos: &mut usize) -> Result<u8, RectangleError> {
    if *pos >= buf.len() {
        return Err(RectangleError::TruncatedPayload);
    }
    let b = buf[*pos];
    *pos += 1;
    Ok(b)
}

fn read_bytes<'a>(buf: &'a [u8], pos: &mut usize, len: usize) -> Result<&'a [u8], RectangleError> {
    if *pos + len > buf.len() {
        return Err(RectangleError::TruncatedPayload);
    }
    let slice = &buf[*pos..*pos + len];
    *pos += len;
    Ok(slice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn zlib_compress(data: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    #[tokio::test]
    async fn verifies_zrle_solid_tile() {
        let mut fb = Framebuffer::new(64, 64).unwrap();
        let pf = PixelFormat::rgba_32();
        let mut decoder = ZrleDecoder::new();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
            encoding_type: 16,
        };

        // Subencoding 1 = Solid color, 3-byte CPIXEL = [200, 100, 50]
        let uncompressed = vec![1, 200, 100, 50];
        let compressed = zlib_compress(&uncompressed);

        let mut wire_data = Vec::new();
        let mut len_buf = [0u8; 4];
        BigEndian::write_u32(&mut len_buf, compressed.len() as u32);
        wire_data.extend_from_slice(&len_buf);
        wire_data.extend_from_slice(&compressed);

        let mut stream = &wire_data[..];
        decoder
            .decode_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        assert_eq!(fb.pixel(0, 0), Some([200, 100, 50, 255]));
        assert_eq!(fb.pixel(3, 3), Some([200, 100, 50, 255]));
    }

    #[tokio::test]
    async fn verifies_zrle_packed_palette_tile() {
        let mut fb = Framebuffer::new(64, 64).unwrap();
        let pf = PixelFormat::rgba_32();
        let mut decoder = ZrleDecoder::new();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 4,
            height: 2, // 8 pixels total
            encoding_type: 16,
        };

        // Subencoding 2 = Palette size 2
        // Palette 0: Red [255, 0, 0]
        // Palette 1: Green [0, 255, 0]
        // 8 pixels at 1 bit/pixel = 1 byte: 0b10101010 (alternating)
        let mut uncompressed = vec![2];
        uncompressed.extend_from_slice(&[255, 0, 0]); // pal 0
        uncompressed.extend_from_slice(&[0, 255, 0]); // pal 1
        uncompressed.push(0b1010_1010); // 1, 0, 1, 0, 1, 0, 1, 0

        let compressed = zlib_compress(&uncompressed);

        let mut wire_data = Vec::new();
        let mut len_buf = [0u8; 4];
        BigEndian::write_u32(&mut len_buf, compressed.len() as u32);
        wire_data.extend_from_slice(&len_buf);
        wire_data.extend_from_slice(&compressed);

        let mut stream = &wire_data[..];
        decoder
            .decode_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        // 0b10101010: pixel 0 is 1 (Green), pixel 1 is 0 (Red), etc.
        assert_eq!(fb.pixel(0, 0), Some([0, 255, 0, 255]));
        assert_eq!(fb.pixel(1, 0), Some([255, 0, 0, 255]));
        assert_eq!(fb.pixel(2, 0), Some([0, 255, 0, 255]));
        assert_eq!(fb.pixel(3, 0), Some([255, 0, 0, 255]));
    }

    #[tokio::test]
    async fn verifies_zrle_plain_rle_tile() {
        let mut fb = Framebuffer::new(64, 64).unwrap();
        let pf = PixelFormat::rgba_32();
        let mut decoder = ZrleDecoder::new();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 2,
            height: 2, // 4 pixels
            encoding_type: 16,
        };

        // Subencoding 128 = Plain RLE
        // Pixel: Blue [0, 0, 255]
        // Run length: 4 (encoded as length minus one = 3)
        let uncompressed = vec![128, 0, 0, 255, 3];
        let compressed = zlib_compress(&uncompressed);

        let mut wire_data = Vec::new();
        let mut len_buf = [0u8; 4];
        BigEndian::write_u32(&mut len_buf, compressed.len() as u32);
        wire_data.extend_from_slice(&len_buf);
        wire_data.extend_from_slice(&compressed);

        let mut stream = &wire_data[..];
        decoder
            .decode_rectangle(&mut stream, &header, &pf, &mut fb)
            .await
            .unwrap();

        assert_eq!(fb.pixel(0, 0), Some([0, 0, 255, 255]));
        assert_eq!(fb.pixel(1, 1), Some([0, 0, 255, 255]));
    }

    #[tokio::test]
    async fn verifies_zrle_decompression_bomb_rejection() {
        let mut fb = Framebuffer::new(64, 64).unwrap();
        let pf = PixelFormat::rgba_32();
        let mut decoder = ZrleDecoder::new();

        let header = RectangleHeader {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            encoding_type: 16,
        };

        // For a 2x2 rect, 1 tile: max_decompressed_bytes = 1 * (1 + 64*64*4) + 65536 ~ 81921 bytes
        // Create 200,000 bytes of zeros which compress to very few bytes
        let huge_uncompressed = vec![0u8; 200_000];
        let compressed = zlib_compress(&huge_uncompressed);

        let mut wire_data = Vec::new();
        let mut len_buf = [0u8; 4];
        BigEndian::write_u32(&mut len_buf, compressed.len() as u32);
        wire_data.extend_from_slice(&len_buf);
        wire_data.extend_from_slice(&compressed);

        let mut stream = &wire_data[..];
        let res = decoder
            .decode_rectangle(&mut stream, &header, &pf, &mut fb)
            .await;

        assert!(matches!(
            res,
            Err(RectangleError::DecompressionBombDetected { .. })
        ));
    }
}

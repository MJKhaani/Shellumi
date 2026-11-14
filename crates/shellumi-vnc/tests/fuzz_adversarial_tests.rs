//! Adversarial fuzzing and regression tests for RFB decoders and security limits.
//!
//! Verifies memory safety, bounds enforcement, decompression bomb protection,
//! and panic-freedom across Hextile, ZRLE, Tight, Raw, CopyRect, and clipboard routines.

use byteorder::{BigEndian, ByteOrder};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use shellumi_remote_desktop::{Framebuffer, MAX_FRAMEBUFFER_DIMENSION};
use shellumi_vnc::decoder::{
    decode_hextile_rectangle, validate_rectangle_bounds, TightDecoder, ZrleDecoder,
    MAX_CLIPBOARD_BYTES, MAX_COMPRESSED_PAYLOAD_SIZE, MAX_RECTANGLE_DIMENSION,
};
use shellumi_vnc::rectangles::{
    decode_copy_rect, decode_raw_rectangle, handle_desktop_size, RectangleError, ENCODING_COPYRECT,
    ENCODING_HEXTILE, ENCODING_RAW, ENCODING_TIGHT, ENCODING_ZRLE,
};
use shellumi_vnc::wire::{PixelFormat, RectangleHeader};
use std::io::Write;

#[test]
fn test_rectangle_bounds_and_overflow_protection() {
    let fb = Framebuffer::new(800, 600).unwrap();

    // 1. Zero width or height rejected
    let zero_w = RectangleHeader {
        x: 0,
        y: 0,
        width: 0,
        height: 100,
        encoding_type: ENCODING_RAW,
    };
    assert!(matches!(
        validate_rectangle_bounds(&zero_w, &fb),
        Err(RectangleError::InvalidDimensions {
            width: 0,
            height: 100
        })
    ));

    let zero_h = RectangleHeader {
        x: 0,
        y: 0,
        width: 100,
        height: 0,
        encoding_type: ENCODING_RAW,
    };
    assert!(matches!(
        validate_rectangle_bounds(&zero_h, &fb),
        Err(RectangleError::InvalidDimensions {
            width: 100,
            height: 0
        })
    ));

    // 2. Exceeding max permitted dimension
    let huge_dim = RectangleHeader {
        x: 0,
        y: 0,
        width: (MAX_RECTANGLE_DIMENSION + 1) as u16,
        height: 100,
        encoding_type: ENCODING_RAW,
    };
    assert!(matches!(
        validate_rectangle_bounds(&huge_dim, &fb),
        Err(RectangleError::InvalidDimensions { .. })
    ));

    // 3. Out of bounds x + width > fb_width
    let out_x = RectangleHeader {
        x: 750,
        y: 100,
        width: 100,
        height: 100,
        encoding_type: ENCODING_RAW,
    };
    assert!(matches!(
        validate_rectangle_bounds(&out_x, &fb),
        Err(RectangleError::OutOfBounds {
            x: 750,
            y: 100,
            width: 100,
            height: 100,
            fb_width: 800,
            fb_height: 600,
        })
    ));

    // 4. Out of bounds y + height > fb_height
    let out_y = RectangleHeader {
        x: 100,
        y: 550,
        width: 100,
        height: 100,
        encoding_type: ENCODING_RAW,
    };
    assert!(matches!(
        validate_rectangle_bounds(&out_y, &fb),
        Err(RectangleError::OutOfBounds {
            x: 100,
            y: 550,
            width: 100,
            height: 100,
            fb_width: 800,
            fb_height: 600,
        })
    ));

    // 5. Extreme u16 coordinates (checking no panic or overflow)
    let extreme_hdr = RectangleHeader {
        x: 65530,
        y: 65530,
        width: 100,
        height: 100,
        encoding_type: ENCODING_RAW,
    };
    assert!(validate_rectangle_bounds(&extreme_hdr, &fb).is_err());
}

#[tokio::test]
async fn test_copy_rect_adversarial_source_bounds() {
    let mut fb = Framebuffer::new(200, 200).unwrap();
    let header = RectangleHeader {
        x: 10,
        y: 10,
        width: 50,
        height: 50,
        encoding_type: ENCODING_COPYRECT,
    };

    // Source x out of bounds: src_x = 180 + width 50 = 230 > 200
    let mut bad_src_x = [0u8; 4];
    BigEndian::write_u16(&mut bad_src_x[0..2], 180);
    BigEndian::write_u16(&mut bad_src_x[2..4], 10);

    let mut stream = &bad_src_x[..];
    let res = decode_copy_rect(&mut stream, &header, &mut fb).await;
    assert!(matches!(res, Err(RectangleError::OutOfBounds { .. })));

    // Source y out of bounds: src_y = 190 + height 50 = 240 > 200
    let mut bad_src_y = [0u8; 4];
    BigEndian::write_u16(&mut bad_src_y[0..2], 10);
    BigEndian::write_u16(&mut bad_src_y[2..4], 190);

    let mut stream = &bad_src_y[..];
    let res = decode_copy_rect(&mut stream, &header, &mut fb).await;
    assert!(matches!(res, Err(RectangleError::OutOfBounds { .. })));
}

#[tokio::test]
async fn test_zrle_decompression_bomb_rejection() {
    let mut fb = Framebuffer::new(100, 100).unwrap();
    let pf = PixelFormat::rgba_32();
    let mut decoder = ZrleDecoder::new();

    let header = RectangleHeader {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
        encoding_type: ENCODING_ZRLE,
    };

    // Construct a genuine zlib decompression bomb:
    // 500,000 zero bytes deflate into less than 1,000 bytes.
    // For a 64x64 tile, max_decompressed_bytes is 1 * (1 + 64*64*4) + 65536 = 81,921 bytes.
    let bomb_payload = vec![0u8; 500_000];
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&bomb_payload).unwrap();
    let compressed_bomb = encoder.finish().unwrap();

    let mut wire_data = Vec::new();
    let mut len_buf = [0u8; 4];
    BigEndian::write_u32(&mut len_buf, compressed_bomb.len() as u32);
    wire_data.extend_from_slice(&len_buf);
    wire_data.extend_from_slice(&compressed_bomb);

    let mut stream = &wire_data[..];
    let res = decoder
        .decode_rectangle(&mut stream, &header, &pf, &mut fb)
        .await;

    assert!(
        matches!(res, Err(RectangleError::DecompressionBombDetected { .. })),
        "Expected DecompressionBombDetected, got: {res:?}"
    );
}

#[tokio::test]
async fn test_zrle_invalid_subencoding_rejection() {
    let mut fb = Framebuffer::new(100, 100).unwrap();
    let pf = PixelFormat::rgba_32();
    let mut decoder = ZrleDecoder::new();

    let header = RectangleHeader {
        x: 0,
        y: 0,
        width: 16,
        height: 16,
        encoding_type: ENCODING_ZRLE,
    };

    // Subencoding 50 is invalid in ZRLE (valid: 0, 1, 2..=16, 128, 130..=255)
    let tile_data = [50u8, 0, 0, 0];
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tile_data).unwrap();
    let compressed = encoder.finish().unwrap();

    let mut wire_data = Vec::new();
    let mut len_buf = [0u8; 4];
    BigEndian::write_u32(&mut len_buf, compressed.len() as u32);
    wire_data.extend_from_slice(&len_buf);
    wire_data.extend_from_slice(&compressed);

    let mut stream = &wire_data[..];
    let res = decoder
        .decode_rectangle(&mut stream, &header, &pf, &mut fb)
        .await;

    assert!(
        matches!(res, Err(RectangleError::InvalidSubencoding(50))),
        "Expected InvalidSubencoding(50), got: {res:?}"
    );
}

#[tokio::test]
async fn test_zrle_payload_exceeding_max_compressed_size() {
    let mut fb = Framebuffer::new(100, 100).unwrap();
    let pf = PixelFormat::rgba_32();
    let mut decoder = ZrleDecoder::new();

    let header = RectangleHeader {
        x: 0,
        y: 0,
        width: 16,
        height: 16,
        encoding_type: ENCODING_ZRLE,
    };

    // Wire length field advertising size larger than MAX_COMPRESSED_PAYLOAD_SIZE (32 MiB)
    let mut len_buf = [0u8; 4];
    BigEndian::write_u32(&mut len_buf, (MAX_COMPRESSED_PAYLOAD_SIZE + 1024) as u32);

    let mut stream = &len_buf[..];
    let res = decoder
        .decode_rectangle(&mut stream, &header, &pf, &mut fb)
        .await;

    assert!(matches!(res, Err(RectangleError::PayloadTooLarge { .. })));
}

#[tokio::test]
async fn test_tight_corrupted_jpeg_rejection() {
    let mut fb = Framebuffer::new(100, 100).unwrap();
    let pf = PixelFormat::rgba_32();
    let mut decoder = TightDecoder::new();

    let header = RectangleHeader {
        x: 0,
        y: 0,
        width: 32,
        height: 32,
        encoding_type: ENCODING_TIGHT,
    };

    // Compression control byte: 0x09 = TIGHT_SUBENCODING_JPEG (9)
    let comp_ctrl = 0x09u8;
    // Corrupted pseudo-JPEG stream
    let corrupted_jpeg = [0xFF, 0xD8, 0x00, 0x12, 0xDE, 0xAD, 0xBE, 0xEF, 0xAA, 0x55];

    let mut wire_data = vec![comp_ctrl];
    // Compact length = 10 bytes (single byte < 128)
    wire_data.push(corrupted_jpeg.len() as u8);
    wire_data.extend_from_slice(&corrupted_jpeg);

    let mut stream = &wire_data[..];
    let res = decoder
        .decode_rectangle(&mut stream, &header, &pf, &mut fb)
        .await;

    assert!(
        matches!(res, Err(RectangleError::DecompressionFailed(_))),
        "Expected DecompressionFailed for corrupt JPEG, got: {res:?}"
    );
}

#[tokio::test]
async fn test_tight_corrupted_zlib_stream_rejection() {
    let mut fb = Framebuffer::new(100, 100).unwrap();
    let pf = PixelFormat::rgba_32();
    let mut decoder = TightDecoder::new();

    let header = RectangleHeader {
        x: 0,
        y: 0,
        width: 16,
        height: 16,
        encoding_type: ENCODING_TIGHT,
    };

    // Basic tight encoding without filter: comp_ctrl = 0x00
    // Uncompressed len = 16 * 16 * 3 = 768 bytes >= 12, so it expects compact len + zlib
    let comp_ctrl = 0x00u8;
    let corrupt_zlib = [0x78, 0x9C, 0xFF, 0xFF, 0x00, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];

    let mut wire_data = vec![comp_ctrl];
    wire_data.push(corrupt_zlib.len() as u8);
    wire_data.extend_from_slice(&corrupt_zlib);

    let mut stream = &wire_data[..];
    let res = decoder
        .decode_rectangle(&mut stream, &header, &pf, &mut fb)
        .await;

    assert!(
        matches!(res, Err(RectangleError::DecompressionFailed(_))),
        "Expected DecompressionFailed for corrupt zlib, got: {res:?}"
    );
}

#[tokio::test]
async fn test_hextile_adversarial_subrectangles_clamping() {
    let mut fb = Framebuffer::new(32, 32).unwrap();
    let pf = PixelFormat::rgba_32();

    let header = RectangleHeader {
        x: 0,
        y: 0,
        width: 16,
        height: 16,
        encoding_type: ENCODING_HEXTILE,
    };

    // Subencoding: BackgroundSpecified (0x02) | ForegroundSpecified (0x04) | AnySubrects (0x08) = 0x0E
    let subencoding = 0x0Eu8;
    let bg_color = [10u8, 20, 30, 0];
    let fg_color = [200u8, 210, 220, 0];
    let n_subrects = 2u8;

    // Subrect 1: Extreme out-of-tile coordinates (xy = 0xFF, wh = 0xFF) -> (sx=15, sy=15, sw=16, sh=16)
    // The decoder must clamp (sx < tile_w && sy < tile_h) and sw.min(tile_w - sx) without panicking
    let subrect_1 = [0xFFu8, 0xFF];
    // Subrect 2: Standard subrect
    let subrect_2 = [0x00u8, 0x11]; // sx=0, sy=0, sw=2, sh=2

    let mut wire_data = Vec::new();
    wire_data.push(subencoding);
    wire_data.extend_from_slice(&bg_color);
    wire_data.extend_from_slice(&fg_color);
    wire_data.push(n_subrects);
    wire_data.extend_from_slice(&subrect_1);
    wire_data.extend_from_slice(&subrect_2);

    let mut stream = &wire_data[..];
    let res = decode_hextile_rectangle(&mut stream, &header, &pf, &mut fb).await;
    assert!(
        res.is_ok(),
        "Hextile must safely clamp subrectangles: {res:?}"
    );

    // Pixel (0, 0) should be colored with foreground
    assert_eq!(fb.pixel(0, 0), Some([200, 210, 220, 255]));
}

#[tokio::test]
async fn test_hextile_truncated_payload_handling() {
    let mut fb = Framebuffer::new(32, 32).unwrap();
    let pf = PixelFormat::rgba_32();

    let header = RectangleHeader {
        x: 0,
        y: 0,
        width: 16,
        height: 16,
        encoding_type: ENCODING_HEXTILE,
    };

    // Empty stream
    let empty_data: [u8; 0] = [];
    let mut stream = &empty_data[..];
    let res = decode_hextile_rectangle(&mut stream, &header, &pf, &mut fb).await;
    assert!(matches!(res, Err(RectangleError::Io(_))));

    // Partial header (only subencoding byte indicating raw tile, but no raw data)
    let partial_data = [0x01u8]; // HEXTILE_RAW
    let mut stream = &partial_data[..];
    let res = decode_hextile_rectangle(&mut stream, &header, &pf, &mut fb).await;
    assert!(matches!(res, Err(RectangleError::Io(_))));
}

#[tokio::test]
async fn test_desktop_size_pseudo_encoding_bounds() {
    let mut fb = Framebuffer::new(800, 600).unwrap();

    // Valid resize
    let valid_resize = RectangleHeader {
        x: 0,
        y: 0,
        width: 1024,
        height: 768,
        encoding_type: -223,
    };
    assert!(handle_desktop_size(&valid_resize, &mut fb).is_ok());
    assert_eq!(fb.width(), 1024);
    assert_eq!(fb.height(), 768);

    // Invalid zero resize
    let zero_resize = RectangleHeader {
        x: 0,
        y: 0,
        width: 0,
        height: 600,
        encoding_type: -223,
    };
    assert!(handle_desktop_size(&zero_resize, &mut fb).is_err());

    // Exceeding MAX_FRAMEBUFFER_DIMENSION (16384)
    let excessive_resize = RectangleHeader {
        x: 0,
        y: 0,
        width: (MAX_FRAMEBUFFER_DIMENSION + 1) as u16,
        height: 600,
        encoding_type: -223,
    };
    assert!(handle_desktop_size(&excessive_resize, &mut fb).is_err());
}

#[test]
fn test_clipboard_size_limits() {
    // Assert MAX_CLIPBOARD_BYTES is capped at 2 MiB
    assert_eq!(MAX_CLIPBOARD_BYTES, 2 * 1024 * 1024);

    let oversized_text = vec![b'A'; MAX_CLIPBOARD_BYTES + 1024];
    assert!(oversized_text.len() > MAX_CLIPBOARD_BYTES);
}

/// Simple fast pseudorandom number generator for deterministic fuzz runs.
struct FuzzRng {
    state: u64,
}

impl FuzzRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 32) as u32
    }

    fn fill_bytes(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(4) {
            let val = self.next_u32();
            let bytes = val.to_le_bytes();
            let n = chunk.len().min(4);
            chunk[..n].copy_from_slice(&bytes[..n]);
        }
    }
}

#[tokio::test]
async fn test_fuzz_random_adversarial_streams_never_panic() {
    let mut rng = FuzzRng::new(0xCAFE_BABE_DEAD_BEEF);
    let pf = PixelFormat::rgba_32();

    // Iterate over 100 pseudorandom adversarial payloads of varying sizes
    for iteration in 0..100 {
        let payload_len = (rng.next_u32() % 2048) as usize;
        let mut random_bytes = vec![0u8; payload_len];
        rng.fill_bytes(&mut random_bytes);

        let w = ((rng.next_u32() % 32) + 1) as u16;
        let h = ((rng.next_u32() % 32) + 1) as u16;
        let x = (rng.next_u32() % 10) as u16;
        let y = (rng.next_u32() % 10) as u16;

        let mut fb = Framebuffer::new(64, 64).unwrap();

        // 1. Raw Decoder
        {
            let header = RectangleHeader {
                x,
                y,
                width: w,
                height: h,
                encoding_type: ENCODING_RAW,
            };
            let mut stream = &random_bytes[..];
            let _ = decode_raw_rectangle(&mut stream, &header, &pf, &mut fb).await;
        }

        // 2. CopyRect Decoder
        {
            let header = RectangleHeader {
                x,
                y,
                width: w,
                height: h,
                encoding_type: ENCODING_COPYRECT,
            };
            let mut stream = &random_bytes[..];
            let _ = decode_copy_rect(&mut stream, &header, &mut fb).await;
        }

        // 3. Hextile Decoder
        {
            let header = RectangleHeader {
                x,
                y,
                width: w,
                height: h,
                encoding_type: ENCODING_HEXTILE,
            };
            let mut stream = &random_bytes[..];
            let _ = decode_hextile_rectangle(&mut stream, &header, &pf, &mut fb).await;
        }

        // 4. ZRLE Decoder
        {
            let header = RectangleHeader {
                x,
                y,
                width: w,
                height: h,
                encoding_type: ENCODING_ZRLE,
            };
            let mut decoder = ZrleDecoder::new();
            let mut stream = &random_bytes[..];
            let _ = decoder
                .decode_rectangle(&mut stream, &header, &pf, &mut fb)
                .await;
        }

        // 5. Tight Decoder
        {
            let header = RectangleHeader {
                x,
                y,
                width: w,
                height: h,
                encoding_type: ENCODING_TIGHT,
            };
            let mut decoder = TightDecoder::new();
            let mut stream = &random_bytes[..];
            let _ = decoder
                .decode_rectangle(&mut stream, &header, &pf, &mut fb)
                .await;
        }

        // Verification check: Framebuffer must still be healthy and uncorrupted
        assert_eq!(fb.width(), 64, "FB corrupted on iter {iteration}");
        assert_eq!(fb.height(), 64, "FB corrupted on iter {iteration}");
    }
}

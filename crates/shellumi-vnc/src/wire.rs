//! RFB wire protocol message serialization, deserialization, and pixel format structures.

use byteorder::{BigEndian, ByteOrder};

/// 16-byte RFB pixel format descriptor (RFC 6143 Section 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelFormat {
    pub bits_per_pixel: u8,
    pub depth: u8,
    pub big_endian_flag: u8,
    pub true_colour_flag: u8,
    pub red_max: u16,
    pub green_max: u16,
    pub blue_max: u16,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
    pub pad: [u8; 3],
}

impl PixelFormat {
    /// Standard 32-bit RGBA (little-endian byte order: R=0, G=8, B=16, A=24).
    pub fn rgba_32() -> Self {
        Self {
            bits_per_pixel: 32,
            depth: 24,
            big_endian_flag: 0,
            true_colour_flag: 1,
            red_max: 255,
            green_max: 255,
            blue_max: 255,
            red_shift: 0,
            green_shift: 8,
            blue_shift: 16,
            pad: [0; 3],
        }
    }

    /// Serialize into 16-byte big-endian RFB wire structure.
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0] = self.bits_per_pixel;
        buf[1] = self.depth;
        buf[2] = self.big_endian_flag;
        buf[3] = self.true_colour_flag;
        BigEndian::write_u16(&mut buf[4..6], self.red_max);
        BigEndian::write_u16(&mut buf[6..8], self.green_max);
        BigEndian::write_u16(&mut buf[8..10], self.blue_max);
        buf[10] = self.red_shift;
        buf[11] = self.green_shift;
        buf[12] = self.blue_shift;
        buf[13..16].copy_from_slice(&self.pad);
        buf
    }

    /// Parse from 16-byte big-endian RFB wire structure.
    pub fn from_bytes(buf: &[u8; 16]) -> Self {
        Self {
            bits_per_pixel: buf[0],
            depth: buf[1],
            big_endian_flag: buf[2],
            true_colour_flag: buf[3],
            red_max: BigEndian::read_u16(&buf[4..6]),
            green_max: BigEndian::read_u16(&buf[6..8]),
            blue_max: BigEndian::read_u16(&buf[8..10]),
            red_shift: buf[10],
            green_shift: buf[11],
            blue_shift: buf[12],
            pad: [buf[13], buf[14], buf[15]],
        }
    }
}

/// Client message IDs (RFC 6143 Section 7.5).
pub mod client_msg {
    pub const SET_PIXEL_FORMAT: u8 = 0;
    pub const SET_ENCODINGS: u8 = 2;
    pub const FRAMEBUFFER_UPDATE_REQUEST: u8 = 3;
    pub const KEY_EVENT: u8 = 4;
    pub const POINTER_EVENT: u8 = 5;
    pub const CLIENT_CUT_TEXT: u8 = 6;
}

/// Server message IDs (RFC 6143 Section 7.6).
pub mod server_msg {
    pub const FRAMEBUFFER_UPDATE: u8 = 0;
    pub const SET_COLOUR_MAP_ENTRIES: u8 = 1;
    pub const BELL: u8 = 2;
    pub const SERVER_CUT_TEXT: u8 = 3;
}

/// Build a 20-byte `SetPixelFormat` packet.
pub fn build_set_pixel_format(pf: &PixelFormat) -> [u8; 20] {
    let mut buf = [0u8; 20];
    buf[0] = client_msg::SET_PIXEL_FORMAT;
    // buf[1..4] padding
    buf[4..20].copy_from_slice(&pf.to_bytes());
    buf
}

/// Build a `SetEncodings` packet for the given list of encoding IDs.
pub fn build_set_encodings(encodings: &[i32]) -> Vec<u8> {
    let mut buf = vec![0u8; 4 + encodings.len() * 4];
    buf[0] = client_msg::SET_ENCODINGS;
    // buf[1] padding
    BigEndian::write_u16(&mut buf[2..4], encodings.len() as u16);
    for (i, &enc) in encodings.iter().enumerate() {
        BigEndian::write_i32(&mut buf[4 + i * 4..8 + i * 4], enc);
    }
    buf
}

/// Build a 10-byte `FramebufferUpdateRequest` packet.
pub fn build_framebuffer_update_request(
    incremental: bool,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> [u8; 10] {
    let mut buf = [0u8; 10];
    buf[0] = client_msg::FRAMEBUFFER_UPDATE_REQUEST;
    buf[1] = if incremental { 1 } else { 0 };
    BigEndian::write_u16(&mut buf[2..4], x);
    BigEndian::write_u16(&mut buf[4..6], y);
    BigEndian::write_u16(&mut buf[6..8], width);
    BigEndian::write_u16(&mut buf[8..10], height);
    buf
}

/// Build an 8-byte `KeyEvent` packet.
pub fn build_key_event(down: bool, keysym: u32) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0] = client_msg::KEY_EVENT;
    buf[1] = if down { 1 } else { 0 };
    // buf[2..4] padding
    BigEndian::write_u32(&mut buf[4..8], keysym);
    buf
}

/// Build a 6-byte `PointerEvent` packet.
pub fn build_pointer_event(button_mask: u8, x: u16, y: u16) -> [u8; 6] {
    let mut buf = [0u8; 6];
    buf[0] = client_msg::POINTER_EVENT;
    buf[1] = button_mask;
    BigEndian::write_u16(&mut buf[2..4], x);
    BigEndian::write_u16(&mut buf[4..6], y);
    buf
}

/// Build a `ClientCutText` packet.
pub fn build_client_cut_text(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut buf = vec![0u8; 8 + bytes.len()];
    buf[0] = client_msg::CLIENT_CUT_TEXT;
    // buf[1..4] padding
    BigEndian::write_u32(&mut buf[4..8], bytes.len() as u32);
    buf[8..].copy_from_slice(bytes);
    buf
}

/// Rectangle header inside a `FramebufferUpdate` message (12 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectangleHeader {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub encoding_type: i32,
}

impl RectangleHeader {
    pub fn from_bytes(buf: &[u8; 12]) -> Self {
        Self {
            x: BigEndian::read_u16(&buf[0..2]),
            y: BigEndian::read_u16(&buf[2..4]),
            width: BigEndian::read_u16(&buf[4..6]),
            height: BigEndian::read_u16(&buf[6..8]),
            encoding_type: BigEndian::read_i32(&buf[8..12]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_pixel_format_roundtrip() {
        let pf = PixelFormat::rgba_32();
        let bytes = pf.to_bytes();
        let parsed = PixelFormat::from_bytes(&bytes);
        assert_eq!(pf, parsed);
        assert_eq!(parsed.bits_per_pixel, 32);
        assert_eq!(parsed.red_max, 255);
    }

    #[test]
    fn verifies_packet_builders() {
        let pf = PixelFormat::rgba_32();
        let set_pf = build_set_pixel_format(&pf);
        assert_eq!(set_pf[0], client_msg::SET_PIXEL_FORMAT);
        assert_eq!(set_pf.len(), 20);

        let encs = [0, 1, -223];
        let set_enc = build_set_encodings(&encs);
        assert_eq!(set_enc[0], client_msg::SET_ENCODINGS);
        assert_eq!(BigEndian::read_u16(&set_enc[2..4]), 3);

        let fbur = build_framebuffer_update_request(true, 10, 20, 800, 600);
        assert_eq!(fbur[0], client_msg::FRAMEBUFFER_UPDATE_REQUEST);
        assert_eq!(fbur[1], 1); // incremental
        assert_eq!(BigEndian::read_u16(&fbur[2..4]), 10);
        assert_eq!(BigEndian::read_u16(&fbur[4..6]), 20);

        let key = build_key_event(true, 0xFF0D); // Return key
        assert_eq!(key[0], client_msg::KEY_EVENT);
        assert_eq!(key[1], 1); // down
        assert_eq!(BigEndian::read_u32(&key[4..8]), 0xFF0D);

        let ptr = build_pointer_event(1, 300, 400);
        assert_eq!(ptr[0], client_msg::POINTER_EVENT);
        assert_eq!(ptr[1], 1);
        assert_eq!(BigEndian::read_u16(&ptr[2..4]), 300);
        assert_eq!(BigEndian::read_u16(&ptr[4..6]), 400);
    }
}

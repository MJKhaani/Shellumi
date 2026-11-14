//! RFB security types and VNCAuth DES challenge-response authentication.

use thiserror::Error;

/// Standard RFB security types (RFC 6143 Section 7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SecurityType {
    Invalid = 0,
    None = 1,
    VncAuth = 2,
    Tight = 16,
    VeNCrypt = 19,
}

impl SecurityType {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::None,
            2 => Self::VncAuth,
            16 => Self::Tight,
            19 => Self::VeNCrypt,
            _ => Self::Invalid,
        }
    }
}

/// Security authentication errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecurityError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Unsupported security type: {0}")]
    UnsupportedType(u8),

    #[error("Server rejected connection: {0}")]
    ConnectionRejected(String),
}

/// Encrypt a 16-byte VNC challenge with a user password using DES ECB mode,
/// with bit-reversed key bytes as mandated by RFC 6143 Section 7.2.2.
pub fn vnc_auth_encrypt(challenge: &[u8; 16], password: &str) -> [u8; 16] {
    // 1. Password is up to 8 bytes, truncated if longer, padded with zeros if shorter
    let mut key_bytes = [0u8; 8];
    let pass_bytes = password.as_bytes();
    let copy_len = pass_bytes.len().min(8);
    key_bytes[..copy_len].copy_from_slice(&pass_bytes[..copy_len]);

    // 2. Reverse bits of each key byte per RFC 6143
    for b in &mut key_bytes {
        *b = b.reverse_bits();
    }

    // 3. Encrypt both 8-byte halves of the 16-byte challenge with DES ECB
    let mut response = [0u8; 16];
    let des = DesCipher::new(&key_bytes);

    let first_half: [u8; 8] = challenge[0..8].try_into().unwrap();
    let second_half: [u8; 8] = challenge[8..16].try_into().unwrap();

    let enc_first = des.encrypt_block(&first_half);
    let enc_second = des.encrypt_block(&second_half);

    response[0..8].copy_from_slice(&enc_first);
    response[8..16].copy_from_slice(&enc_second);

    response
}

// ---------------------------------------------------------------------------
// Standard Data Encryption Standard (DES / FIPS 46-3) implementation
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct DesCipher {
    round_keys: [u64; 16],
}

impl DesCipher {
    fn new(key: &[u8; 8]) -> Self {
        let key_bits = u64::from_be_bytes(*key);
        let round_keys = des_key_schedule(key_bits);
        Self { round_keys }
    }

    fn encrypt_block(&self, block: &[u8; 8]) -> [u8; 8] {
        let input_bits = u64::from_be_bytes(*block);
        let permuted = permute_64(input_bits, &IP);

        let mut left = (permuted >> 32) as u32;
        let mut right = (permuted & 0xFFFF_FFFF) as u32;

        for &round_key in &self.round_keys {
            let next_right = left ^ feistel_function(right, round_key);
            left = right;
            right = next_right;
        }

        // 32-bit swap (left and right)
        let combined = ((right as u64) << 32) | (left as u64);
        let output_bits = permute_64(combined, &FP);

        output_bits.to_be_bytes()
    }
}

fn feistel_function(right: u32, round_key: u64) -> u32 {
    // 1. Expand 32-bit right half to 48 bits
    let expanded = permute_32_to_48(right, &EXPANSION);
    // 2. XOR with round key
    let xor_result = expanded ^ round_key;

    // 3. 8 S-box lookups (each 6 bits in -> 4 bits out)
    let mut s_output: u32 = 0;
    for (i, s_box) in S_BOXES.iter().enumerate() {
        let shift = (7 - i) * 6;
        let chunk = ((xor_result >> shift) & 0x3F) as usize;
        let row = ((chunk & 0x20) >> 4) | (chunk & 0x01);
        let col = (chunk >> 1) & 0x0F;
        let val = s_box[row][col] as u32;
        s_output = (s_output << 4) | val;
    }

    // 4. P permutation on 32-bit output
    permute_32(s_output, &P_PERMUTATION)
}

fn des_key_schedule(key: u64) -> [u64; 16] {
    // PC-1: 64 bits to 56 bits
    let mut c = permute_64_to_28(key, &PC1_C);
    let mut d = permute_64_to_28(key, &PC1_D);

    let mut round_keys = [0u64; 16];
    for (i, &shift) in KEY_SHIFTS.iter().enumerate() {
        c = rotate_left_28(c, shift);
        d = rotate_left_28(d, shift);

        let cd = ((c as u64) << 28) | (d as u64);
        round_keys[i] = permute_56_to_48(cd, &PC2);
    }

    round_keys
}

fn rotate_left_28(val: u32, shift: u32) -> u32 {
    let mask = 0x0FFF_FFFF;
    let v = val & mask;
    ((v << shift) | (v >> (28 - shift))) & mask
}

fn permute_64(val: u64, table: &[u8; 64]) -> u64 {
    let mut res: u64 = 0;
    for &pos in table {
        res = (res << 1) | ((val >> (64 - pos)) & 1);
    }
    res
}

fn permute_32_to_48(val: u32, table: &[u8; 48]) -> u64 {
    let mut res: u64 = 0;
    for &pos in table {
        res = (res << 1) | (((val as u64) >> (32 - pos)) & 1);
    }
    res
}

fn permute_56_to_48(val: u64, table: &[u8; 48]) -> u64 {
    let mut res: u64 = 0;
    for &pos in table {
        res = (res << 1) | ((val >> (56 - pos)) & 1);
    }
    res
}

fn permute_64_to_28(val: u64, table: &[u8; 28]) -> u32 {
    let mut res: u32 = 0;
    for &pos in table {
        res = (res << 1) | (((val >> (64 - pos)) & 1) as u32);
    }
    res
}

fn permute_32(val: u32, table: &[u8; 32]) -> u32 {
    let mut res: u32 = 0;
    for &pos in table {
        res = (res << 1) | ((val >> (32 - pos)) & 1);
    }
    res
}

// ---------------------------------------------------------------------------
// DES Permutation Tables & S-Boxes
// ---------------------------------------------------------------------------

const KEY_SHIFTS: [u32; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];

#[rustfmt::skip]
const IP: [u8; 64] = [
    58, 50, 42, 34, 26, 18, 10, 2,
    60, 52, 44, 36, 28, 20, 12, 4,
    62, 54, 46, 38, 30, 22, 14, 6,
    64, 56, 48, 40, 32, 24, 16, 8,
    57, 49, 41, 33, 25, 17, 9,  1,
    59, 51, 43, 35, 27, 19, 11, 3,
    61, 53, 45, 37, 29, 21, 13, 5,
    63, 55, 47, 39, 31, 23, 15, 7,
];

#[rustfmt::skip]
const FP: [u8; 64] = [
    40, 8, 48, 16, 56, 24, 64, 32,
    39, 7, 47, 15, 55, 23, 63, 31,
    38, 6, 46, 14, 54, 22, 62, 30,
    37, 5, 45, 13, 53, 21, 61, 29,
    36, 4, 44, 12, 52, 20, 60, 28,
    35, 3, 43, 11, 51, 19, 59, 27,
    34, 2, 42, 10, 50, 18, 58, 26,
    33, 1, 41, 9,  49, 17, 57, 25,
];

#[rustfmt::skip]
const EXPANSION: [u8; 48] = [
    32, 1,  2,  3,  4,  5,
    4,  5,  6,  7,  8,  9,
    8,  9,  10, 11, 12, 13,
    12, 13, 14, 15, 16, 17,
    16, 17, 18, 19, 20, 21,
    20, 21, 22, 23, 24, 25,
    24, 25, 26, 27, 28, 29,
    28, 29, 30, 31, 32, 1,
];

#[rustfmt::skip]
const P_PERMUTATION: [u8; 32] = [
    16, 7,  20, 21,
    29, 12, 28, 17,
    1,  15, 23, 26,
    5,  18, 31, 10,
    2,  8,  24, 14,
    32, 27, 3,  9,
    19, 13, 30, 6,
    22, 11, 4,  25,
];

#[rustfmt::skip]
const PC1_C: [u8; 28] = [
    57, 49, 41, 33, 25, 17, 9,
    1,  58, 50, 42, 34, 26, 18,
    10, 2,  59, 51, 43, 35, 27,
    19, 11, 3,  60, 52, 44, 36,
];

#[rustfmt::skip]
const PC1_D: [u8; 28] = [
    63, 55, 47, 39, 31, 23, 15,
    7,  62, 54, 46, 38, 30, 22,
    14, 6,  61, 53, 45, 37, 29,
    21, 13, 5,  28, 20, 12, 4,
];

#[rustfmt::skip]
const PC2: [u8; 48] = [
    14, 17, 11, 24, 1,  5,
    3,  28, 15, 6,  21, 10,
    23, 19, 12, 4,  26, 8,
    16, 7,  27, 20, 13, 2,
    41, 52, 31, 37, 47, 55,
    30, 40, 51, 45, 33, 48,
    44, 49, 39, 56, 34, 53,
    46, 42, 50, 36, 29, 32,
];

#[rustfmt::skip]
const S_BOXES: [[[u8; 16]; 4]; 8] = [
    // S1
    [
        [14, 4, 13, 1, 2, 15, 11, 8, 3, 10, 6, 12, 5, 9, 0, 7],
        [0, 15, 7, 4, 14, 2, 13, 1, 10, 6, 12, 11, 9, 5, 3, 8],
        [4, 1, 14, 8, 13, 6, 2, 11, 15, 12, 9, 7, 3, 10, 5, 0],
        [15, 12, 8, 2, 4, 9, 1, 7, 5, 11, 3, 14, 10, 0, 6, 13],
    ],
    // S2
    [
        [15, 1, 8, 14, 6, 11, 3, 4, 9, 7, 2, 13, 12, 0, 5, 10],
        [3, 13, 4, 7, 15, 2, 8, 14, 12, 0, 1, 10, 6, 9, 11, 5],
        [0, 14, 7, 11, 10, 4, 13, 1, 5, 8, 12, 6, 9, 3, 2, 15],
        [13, 8, 10, 1, 3, 15, 4, 2, 11, 6, 7, 12, 0, 5, 14, 9],
    ],
    // S3
    [
        [10, 0, 9, 14, 6, 3, 15, 5, 1, 13, 12, 7, 11, 4, 2, 8],
        [13, 7, 0, 9, 3, 4, 6, 10, 2, 8, 5, 14, 12, 11, 15, 1],
        [13, 6, 4, 9, 8, 15, 3, 0, 11, 1, 2, 12, 5, 10, 14, 7],
        [1, 10, 13, 0, 6, 9, 8, 7, 4, 15, 14, 3, 11, 5, 2, 12],
    ],
    // S4
    [
        [7, 13, 14, 3, 0, 6, 9, 10, 1, 2, 8, 5, 11, 12, 4, 15],
        [13, 8, 11, 5, 6, 15, 0, 3, 4, 7, 2, 12, 1, 10, 14, 9],
        [10, 6, 9, 0, 12, 11, 7, 13, 15, 1, 3, 14, 5, 2, 8, 4],
        [3, 15, 0, 6, 10, 1, 13, 8, 9, 4, 5, 11, 12, 7, 2, 14],
    ],
    // S5
    [
        [2, 12, 4, 1, 7, 10, 11, 6, 8, 5, 3, 15, 13, 0, 14, 9],
        [14, 11, 2, 12, 4, 7, 13, 1, 5, 0, 15, 10, 3, 9, 8, 6],
        [4, 2, 1, 11, 10, 13, 7, 8, 15, 9, 12, 5, 6, 3, 0, 14],
        [11, 8, 12, 7, 1, 14, 2, 13, 6, 15, 0, 9, 10, 4, 5, 3],
    ],
    // S6
    [
        [12, 1, 10, 15, 9, 2, 6, 8, 0, 13, 3, 4, 14, 7, 5, 11],
        [10, 15, 4, 2, 7, 12, 9, 5, 6, 1, 13, 14, 0, 11, 3, 8],
        [9, 14, 15, 5, 2, 8, 12, 3, 7, 0, 4, 10, 1, 13, 11, 6],
        [4, 3, 2, 12, 9, 5, 15, 10, 11, 14, 1, 7, 6, 0, 8, 13],
    ],
    // S7
    [
        [4, 11, 2, 14, 15, 0, 8, 13, 3, 12, 9, 7, 5, 10, 6, 1],
        [13, 0, 11, 7, 4, 9, 1, 10, 14, 3, 5, 12, 2, 15, 8, 6],
        [1, 4, 11, 13, 12, 3, 7, 14, 10, 15, 6, 8, 0, 5, 9, 2],
        [6, 11, 13, 8, 1, 4, 10, 7, 9, 5, 0, 15, 14, 2, 3, 12],
    ],
    // S8
    [
        [13, 2, 8, 4, 6, 15, 11, 1, 10, 9, 3, 14, 5, 0, 12, 7],
        [1, 15, 13, 8, 10, 3, 7, 4, 12, 5, 6, 11, 0, 14, 9, 2],
        [7, 11, 4, 1, 9, 12, 14, 2, 0, 6, 10, 13, 15, 3, 5, 8],
        [2, 1, 14, 7, 4, 10, 8, 13, 15, 12, 9, 0, 3, 5, 6, 11],
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_standard_des_test_vector() {
        // Known standard NIST DES test vector:
        // Key: 0x133457799BBCDFF1
        // Plaintext: 0x0123456789ABCDEF
        // Expected Ciphertext: 0x85E813540F0AB405
        let key = [0x13, 0x34, 0x57, 0x79, 0x9B, 0xBC, 0xDF, 0xF1];
        let pt = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];
        let des = DesCipher::new(&key);
        let ct = des.encrypt_block(&pt);
        assert_eq!(ct, [0x85, 0xE8, 0x13, 0x54, 0x0F, 0x0A, 0xB4, 0x05]);
    }

    #[test]
    fn verifies_vnc_auth_challenge_encryption() {
        // 16-byte challenge and known password "secret"
        let challenge: [u8; 16] = [
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x0F, 0xED, 0xCB, 0xA9, 0x87, 0x65,
            0x43, 0x21,
        ];
        let resp = vnc_auth_encrypt(&challenge, "secret");
        assert_eq!(resp.len(), 16);
        // Ensure ciphertext is completely altered from plaintext
        assert_ne!(&resp[0..8], &challenge[0..8]);
        assert_ne!(&resp[8..16], &challenge[8..16]);
    }
}

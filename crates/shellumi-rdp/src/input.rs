//! Input mapping and scancode translation from Shellumi domain events to RDP FastPath PDUs.

use ironrdp_pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use ironrdp_pdu::input::mouse::PointerFlags;
use ironrdp_pdu::input::MousePdu;
use shellumi_remote_desktop::input::{KeyEvent, PointerEvent, WheelEvent};
use smallvec::{smallvec, SmallVec};

/// Translate a Shellumi `PointerEvent` into an RDP `FastPathInputEvent::MouseEvent`.
pub fn translate_pointer_event(p: &PointerEvent) -> FastPathInputEvent {
    let mut flags = PointerFlags::empty();

    if p.button_mask == 0 {
        flags |= PointerFlags::MOVE;
    } else {
        if (p.button_mask & 1) != 0 {
            flags |= PointerFlags::LEFT_BUTTON | PointerFlags::DOWN;
        }
        if (p.button_mask & 2) != 0 {
            flags |= PointerFlags::MIDDLE_BUTTON_OR_WHEEL | PointerFlags::DOWN;
        }
        if (p.button_mask & 4) != 0 {
            flags |= PointerFlags::RIGHT_BUTTON | PointerFlags::DOWN;
        }
    }

    FastPathInputEvent::MouseEvent(MousePdu {
        flags,
        number_of_wheel_rotation_units: 0,
        x_position: p.x.clamp(0, u16::MAX as u32) as u16,
        y_position: p.y.clamp(0, u16::MAX as u32) as u16,
    })
}

/// Translate a Shellumi `WheelEvent` into an RDP `FastPathInputEvent::MouseEvent`.
pub fn translate_wheel_event(w: &WheelEvent) -> FastPathInputEvent {
    let mut flags = PointerFlags::VERTICAL_WHEEL;
    let delta = w.delta_y.clamp(-120, 120) as i16;

    if delta < 0 {
        flags |= PointerFlags::WHEEL_NEGATIVE;
    }

    FastPathInputEvent::MouseEvent(MousePdu {
        flags,
        number_of_wheel_rotation_units: delta.abs(),
        x_position: w.x.clamp(0, u16::MAX as u32) as u16,
        y_position: w.y.clamp(0, u16::MAX as u32) as u16,
    })
}

/// Translate a Shellumi `KeyEvent` into an RDP `FastPathInputEvent`.
pub fn translate_key_event(k: &KeyEvent) -> FastPathInputEvent {
    let mut flags = KeyboardFlags::empty();
    if !k.pressed {
        flags |= KeyboardFlags::RELEASE;
    }

    if let Some((scancode, extended)) = keysym_to_scancode(k.keysym) {
        if extended {
            flags |= KeyboardFlags::EXTENDED;
        }
        FastPathInputEvent::KeyboardEvent(flags, scancode)
    } else if let Some(ch) = k.text.as_deref().and_then(|t| t.chars().next()) {
        FastPathInputEvent::UnicodeKeyboardEvent(flags, ch as u16)
    } else {
        FastPathInputEvent::KeyboardEvent(flags, 0)
    }
}

/// Generate a Ctrl+Alt+Del hardware scancode sequence chunked into FastPath pairs.
pub fn build_ctrl_alt_del_sequence() -> Vec<SmallVec<[FastPathInputEvent; 2]>> {
    vec![
        // 1. Ctrl Down (0x1D), Alt Down (0x38)
        smallvec![
            FastPathInputEvent::KeyboardEvent(KeyboardFlags::empty(), 0x1D),
            FastPathInputEvent::KeyboardEvent(KeyboardFlags::empty(), 0x38),
        ],
        // 2. Del Down (0x53, Extended), Del Up (0x53, Extended | Release)
        smallvec![
            FastPathInputEvent::KeyboardEvent(KeyboardFlags::EXTENDED, 0x53),
            FastPathInputEvent::KeyboardEvent(
                KeyboardFlags::EXTENDED | KeyboardFlags::RELEASE,
                0x53
            ),
        ],
        // 3. Alt Up (0x38, Release), Ctrl Up (0x1D, Release)
        smallvec![
            FastPathInputEvent::KeyboardEvent(KeyboardFlags::RELEASE, 0x38),
            FastPathInputEvent::KeyboardEvent(KeyboardFlags::RELEASE, 0x1D),
        ],
    ]
}

/// Generate a Windows Key (Super) keypress and release sequence.
pub fn build_win_key_sequence() -> SmallVec<[FastPathInputEvent; 2]> {
    let mut events = SmallVec::new();
    events.push(FastPathInputEvent::KeyboardEvent(
        KeyboardFlags::EXTENDED,
        0x5B,
    ));
    events.push(FastPathInputEvent::KeyboardEvent(
        KeyboardFlags::EXTENDED | KeyboardFlags::RELEASE,
        0x5B,
    ));
    events
}

/// Generate an Alt+Tab task switcher keypress and release sequence.
pub fn build_alt_tab_sequence() -> SmallVec<[FastPathInputEvent; 4]> {
    let mut events = SmallVec::new();
    events.push(FastPathInputEvent::KeyboardEvent(
        KeyboardFlags::empty(),
        0x38,
    ));
    events.push(FastPathInputEvent::KeyboardEvent(
        KeyboardFlags::empty(),
        0x0F,
    ));
    events.push(FastPathInputEvent::KeyboardEvent(
        KeyboardFlags::RELEASE,
        0x0F,
    ));
    events.push(FastPathInputEvent::KeyboardEvent(
        KeyboardFlags::RELEASE,
        0x38,
    ));
    events
}

/// Map X11 keysym to PC AT / PS/2 Set 1 scancode `(scancode, is_extended)`.
fn keysym_to_scancode(keysym: u32) -> Option<(u8, bool)> {
    match keysym {
        0xFF1B => Some((0x01, false)), // Escape
        0x0031..=0x0039 => Some(((keysym - 0x0031 + 0x02) as u8, false)), // 1..9
        0x0030 => Some((0x0B, false)), // 0
        0x002D => Some((0x0C, false)), // -
        0x003D => Some((0x0D, false)), // =
        0xFF08 => Some((0x0E, false)), // Backspace
        0xFF09 => Some((0x0F, false)), // Tab

        0x0071 | 0x0051 => Some((0x10, false)), // Q
        0x0077 | 0x0057 => Some((0x11, false)), // W
        0x0065 | 0x0045 => Some((0x12, false)), // E
        0x0072 | 0x0052 => Some((0x13, false)), // R
        0x0074 | 0x0054 => Some((0x14, false)), // T
        0x0079 | 0x0059 => Some((0x15, false)), // Y
        0x0075 | 0x0055 => Some((0x16, false)), // U
        0x0069 | 0x0049 => Some((0x17, false)), // I
        0x006F | 0x004F => Some((0x18, false)), // O
        0x0070 | 0x0050 => Some((0x19, false)), // P
        0x005B => Some((0x1A, false)),          // [
        0x005D => Some((0x1B, false)),          // ]
        0xFF0D => Some((0x1C, false)),          // Enter / Return

        0xFFE3 => Some((0x1D, false)), // Left Control
        0xFFE4 => Some((0x1D, true)),  // Right Control

        0x0061 | 0x0041 => Some((0x1E, false)), // A
        0x0073 | 0x0053 => Some((0x1F, false)), // S
        0x0064 | 0x0044 => Some((0x20, false)), // D
        0x0066 | 0x0046 => Some((0x21, false)), // F
        0x0067 | 0x0047 => Some((0x22, false)), // G
        0x0068 | 0x0048 => Some((0x23, false)), // H
        0x006A | 0x004A => Some((0x24, false)), // J
        0x006B | 0x004B => Some((0x25, false)), // K
        0x006C | 0x004C => Some((0x26, false)), // L

        0xFFE1 => Some((0x2A, false)), // Left Shift
        0xFFE2 => Some((0x36, false)), // Right Shift

        0x007A | 0x005A => Some((0x2C, false)), // Z
        0x0078 | 0x0058 => Some((0x2D, false)), // X
        0x0063 | 0x0043 => Some((0x2E, false)), // C
        0x0076 | 0x0056 => Some((0x2F, false)), // V
        0x0062 | 0x0042 => Some((0x30, false)), // B
        0x006E | 0x004E => Some((0x31, false)), // N
        0x006D | 0x004D => Some((0x32, false)), // M

        0x0020 => Some((0x39, false)), // Space

        0xFFE9 => Some((0x38, false)), // Left Alt
        0xFFEA => Some((0x38, true)),  // Right Alt (AltGr)

        0xFFBE..=0xFFC7 => Some(((keysym - 0xFFBE + 0x3B) as u8, false)), // F1..F10
        0xFFC8 => Some((0x57, false)),                                    // F11
        0xFFC9 => Some((0x58, false)),                                    // F12

        0xFF50 => Some((0x47, true)), // Home
        0xFF52 => Some((0x48, true)), // Up Arrow
        0xFF55 => Some((0x49, true)), // Page Up
        0xFF51 => Some((0x4B, true)), // Left Arrow
        0xFF53 => Some((0x4D, true)), // Right Arrow
        0xFF57 => Some((0x4F, true)), // End
        0xFF54 => Some((0x50, true)), // Down Arrow
        0xFF56 => Some((0x51, true)), // Page Down
        0xFF63 => Some((0x52, true)), // Insert
        0xFFFF => Some((0x53, true)), // Delete

        0xFFEB => Some((0x5B, true)), // Left Super / Windows
        0xFFEC => Some((0x5C, true)), // Right Super / Windows

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_pointer_translation() {
        let p = PointerEvent {
            button_mask: 1, // Left click
            x: 100,
            y: 200,
            pressed: true,
        };
        let event = translate_pointer_event(&p);
        match event {
            FastPathInputEvent::MouseEvent(pdu) => {
                assert!(pdu.flags.contains(PointerFlags::LEFT_BUTTON));
                assert!(pdu.flags.contains(PointerFlags::DOWN));
                assert_eq!(pdu.x_position, 100);
                assert_eq!(pdu.y_position, 200);
            }
            _ => panic!("Expected MouseEvent"),
        }
    }

    #[test]
    fn verifies_ctrl_alt_del_sequence() {
        let seq = build_ctrl_alt_del_sequence();
        assert_eq!(seq.len(), 3);
        // First is Ctrl Down (0x1D)
        match &seq[0][0] {
            FastPathInputEvent::KeyboardEvent(flags, code) => {
                assert_eq!(*code, 0x1D);
                assert!(!flags.contains(KeyboardFlags::RELEASE));
            }
            _ => panic!("Expected KeyboardEvent"),
        }
    }

    #[test]
    fn verifies_key_translation() {
        let k = KeyEvent {
            pressed: true,
            keysym: 0xFF0D, // Return
            scancode: 0,
            text: Some("\n".into()),
            modifiers: shellumi_remote_desktop::input::KeyModifiers::default(),
        };
        let event = translate_key_event(&k);
        match event {
            FastPathInputEvent::KeyboardEvent(flags, code) => {
                assert_eq!(code, 0x1C); // Return scancode
                assert!(!flags.contains(KeyboardFlags::RELEASE));
            }
            _ => panic!("Expected KeyboardEvent"),
        }
    }
}

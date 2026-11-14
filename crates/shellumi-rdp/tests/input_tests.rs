use ironrdp_pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use ironrdp_pdu::input::mouse::PointerFlags;
use shellumi_rdp::input::{
    build_ctrl_alt_del_sequence, build_win_key_sequence, translate_key_event,
    translate_pointer_event, translate_wheel_event,
};
use shellumi_remote_desktop::input::{
    pointer_mask, KeyEvent, KeyModifiers, PointerEvent, WheelEvent,
};

const KEYSYM_RETURN: u32 = 0xFF0D;

#[test]
fn test_translate_pointer_motion() {
    let p = PointerEvent {
        x: 640,
        y: 480,
        button_mask: 0,
        pressed: false,
    };
    let event = translate_pointer_event(&p);
    match event {
        FastPathInputEvent::MouseEvent(m) => {
            assert!(m.flags.contains(PointerFlags::MOVE));
            assert_eq!(m.x_position, 640);
            assert_eq!(m.y_position, 480);
        }
        other => panic!("Expected MouseEvent, got {other:?}"),
    }
}

#[test]
fn test_translate_pointer_buttons() {
    // Left button
    let p_left = PointerEvent {
        x: 100,
        y: 200,
        button_mask: pointer_mask::BUTTON_LEFT,
        pressed: true,
    };
    if let FastPathInputEvent::MouseEvent(m) = translate_pointer_event(&p_left) {
        assert!(m.flags.contains(PointerFlags::LEFT_BUTTON));
        assert!(m.flags.contains(PointerFlags::DOWN));
    } else {
        panic!("Expected MouseEvent");
    }

    // Right button
    let p_right = PointerEvent {
        x: 100,
        y: 200,
        button_mask: pointer_mask::BUTTON_RIGHT,
        pressed: true,
    };
    if let FastPathInputEvent::MouseEvent(m) = translate_pointer_event(&p_right) {
        assert!(m.flags.contains(PointerFlags::RIGHT_BUTTON));
        assert!(m.flags.contains(PointerFlags::DOWN));
    } else {
        panic!("Expected MouseEvent");
    }
}

#[test]
fn test_translate_wheel_vertical() {
    // Positive scroll (wheel up)
    let w_up = WheelEvent {
        x: 300,
        y: 400,
        delta_x: 0,
        delta_y: 120,
    };
    if let FastPathInputEvent::MouseEvent(m) = translate_wheel_event(&w_up) {
        assert!(m.flags.contains(PointerFlags::VERTICAL_WHEEL));
        assert!(!m.flags.contains(PointerFlags::WHEEL_NEGATIVE));
        assert_eq!(m.number_of_wheel_rotation_units, 120);
    } else {
        panic!("Expected MouseEvent");
    }

    // Negative scroll (wheel down)
    let w_down = WheelEvent {
        x: 300,
        y: 400,
        delta_x: 0,
        delta_y: -60,
    };
    if let FastPathInputEvent::MouseEvent(m) = translate_wheel_event(&w_down) {
        assert!(m.flags.contains(PointerFlags::VERTICAL_WHEEL));
        assert!(m.flags.contains(PointerFlags::WHEEL_NEGATIVE));
        assert_eq!(m.number_of_wheel_rotation_units, 60);
    } else {
        panic!("Expected MouseEvent");
    }
}

#[test]
fn test_translate_key_events() {
    // Return key press
    let k_return = KeyEvent {
        keysym: KEYSYM_RETURN,
        scancode: 0,
        pressed: true,
        modifiers: KeyModifiers::default(),
        text: None,
    };
    if let FastPathInputEvent::KeyboardEvent(flags, scancode) = translate_key_event(&k_return) {
        assert!(!flags.contains(KeyboardFlags::RELEASE));
        assert_eq!(scancode, 0x1C);
    } else {
        panic!("Expected KeyboardEvent");
    }

    // Return key release
    let k_return_up = KeyEvent {
        keysym: KEYSYM_RETURN,
        scancode: 0,
        pressed: false,
        modifiers: KeyModifiers::default(),
        text: None,
    };
    if let FastPathInputEvent::KeyboardEvent(flags, scancode) = translate_key_event(&k_return_up) {
        assert!(flags.contains(KeyboardFlags::RELEASE));
        assert_eq!(scancode, 0x1C);
    } else {
        panic!("Expected KeyboardEvent");
    }

    // Unicode fallback
    let k_unicode = KeyEvent {
        keysym: 0,
        scancode: 0,
        pressed: true,
        modifiers: KeyModifiers::default(),
        text: Some("€".into()),
    };
    if let FastPathInputEvent::UnicodeKeyboardEvent(flags, code) = translate_key_event(&k_unicode) {
        assert!(!flags.contains(KeyboardFlags::RELEASE));
        assert_eq!(code, '€' as u16);
    } else {
        panic!("Expected UnicodeKeyboardEvent");
    }
}

#[test]
fn test_ctrl_alt_del_sequence() {
    let chunks = build_ctrl_alt_del_sequence();
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].len(), 2); // Ctrl down, Alt down
    assert_eq!(chunks[1].len(), 2); // Del down, Del up
    assert_eq!(chunks[2].len(), 2); // Alt up, Ctrl up

    // Verify first event is Ctrl down (0x1D)
    if let FastPathInputEvent::KeyboardEvent(flags, code) = &chunks[0][0] {
        assert!(!flags.contains(KeyboardFlags::RELEASE));
        assert_eq!(*code, 0x1D);
    } else {
        panic!("Expected KeyboardEvent");
    }

    // Verify Del down has EXTENDED flag
    if let FastPathInputEvent::KeyboardEvent(flags, code) = &chunks[1][0] {
        assert!(flags.contains(KeyboardFlags::EXTENDED));
        assert!(!flags.contains(KeyboardFlags::RELEASE));
        assert_eq!(*code, 0x53);
    } else {
        panic!("Expected KeyboardEvent");
    }
}

#[test]
fn test_win_key_sequence() {
    let seq = build_win_key_sequence();
    assert_eq!(seq.len(), 2);
    // Win key down
    if let FastPathInputEvent::KeyboardEvent(flags, code) = &seq[0] {
        assert!(flags.contains(KeyboardFlags::EXTENDED));
        assert!(!flags.contains(KeyboardFlags::RELEASE));
        assert_eq!(*code, 0x5B);
    }
    // Win key up
    if let FastPathInputEvent::KeyboardEvent(flags, code) = &seq[1] {
        assert!(flags.contains(KeyboardFlags::EXTENDED));
        assert!(flags.contains(KeyboardFlags::RELEASE));
        assert_eq!(*code, 0x5B);
    }
}

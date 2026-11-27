use shellumi_rdp::{adjust_display_size, create_display_control_client, DisplayControlState};
use shellumi_remote_desktop::{SessionCommand, SessionEvent};

#[test]
fn test_display_adjust_size_bounds_and_even_width() {
    // Exact standard resolutions
    assert_eq!(adjust_display_size(1920, 1080), (1920, 1080));
    assert_eq!(adjust_display_size(2560, 1440), (2560, 1440));
    assert_eq!(adjust_display_size(3840, 2160), (3840, 2160));
    assert_eq!(adjust_display_size(1280, 720), (1280, 720));

    // Odd width adjustment per MS-RDPEDISP § 2.2.2.2.1
    let (w, h) = adjust_display_size(1921, 1080);
    assert_eq!(w % 2, 0);
    assert_eq!((w, h), (1920, 1080));

    let (w2, h2) = adjust_display_size(1365, 768);
    assert_eq!(w2 % 2, 0);
    assert_eq!((w2, h2), (1364, 768));

    // Minimum boundary clamp (200x200)
    assert_eq!(adjust_display_size(50, 50), (200, 200));
    assert_eq!(adjust_display_size(199, 199), (200, 200));

    // Maximum boundary clamp (8192x8192)
    assert_eq!(adjust_display_size(9000, 9000), (8192, 8192));
}

#[test]
fn test_display_control_state_and_event_channel() {
    let state = DisplayControlState::new();
    assert!(!state.is_ready());
    assert_eq!(state.max_monitors(), 1);
    assert_eq!(state.max_area(), 0);

    let (event_tx, _event_rx) = async_channel::bounded(16);
    let _client = create_display_control_client(state.clone(), event_tx);

    state.update_caps(4, 3840 * 2160 * 4);
    assert!(state.is_ready());
    assert_eq!(state.max_monitors(), 4);
    assert_eq!(state.max_area(), 3840 * 2160 * 4);

    // Verify SessionCommand::RequestResize can be constructed and passed
    let cmd = SessionCommand::RequestResize {
        identity: shellumi_remote_desktop::SessionIdentity::new(1, 1, 1),
        width: 1920,
        height: 1080,
    };
    if let SessionCommand::RequestResize { width, height, .. } = cmd {
        let (adj_w, adj_h) = adjust_display_size(width, height);
        assert_eq!((adj_w, adj_h), (1920, 1080));
    } else {
        panic!("Expected RequestResize command");
    }

    // Verify SessionEvent::DisplayControlCaps event channel receiving
    let event = SessionEvent::DisplayControlCaps {
        max_monitors: 4,
        max_area: 33177600,
    };
    let (tx, rx) = async_channel::unbounded();
    assert!(tx.try_send(event).is_ok());
    let received = rx.try_recv().expect("event received");
    match received {
        SessionEvent::DisplayControlCaps {
            max_monitors,
            max_area,
        } => {
            assert_eq!(max_monitors, 4);
            assert_eq!(max_area, 33177600);
        }
        _ => panic!("Expected DisplayControlCaps event"),
    }
}

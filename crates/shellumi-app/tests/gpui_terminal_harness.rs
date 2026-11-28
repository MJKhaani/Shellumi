use gpui::{Keystroke, Modifiers};
use shellumi_app::{
    encode_mouse_event, encode_paste_bytes, key_bytes_for_modes, selected_text_from_terminal,
    session::{Session, SessionEvent, SessionInputError, MAX_INPUT_QUEUE_BYTES, MAX_PASTE_BYTES},
    TerminalSelection, TerminalViewportGeometry,
};
use shellumi_terminal::{Terminal, TerminalCommand, TerminalModes};
use std::collections::HashMap;
use std::time::Duration;

fn collect_screen_text(term: &Terminal) -> String {
    (0..term.grid().rows())
        .filter_map(|r| term.row_text(r))
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() {
    gpui::Application::headless().run(|cx| {
        // 1. Verify GPUI App context, focus handle, and clipboard on OS main thread.
        let focus = cx.focus_handle();
        let _ = focus;
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
            "shellumi-gpui-clipboard".to_string(),
        ));
        let clip_text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        assert_eq!(clip_text, "shellumi-gpui-clipboard");

        // 2. Verify viewport geometry hit-testing, mouse reporting, and local selection extraction.
        let geom = TerminalViewportGeometry {
            origin_x: 100.0,
            origin_y: 50.0,
            width: 824.0,
            height: 504.0,
            padding: 12.0,
            cell_width: 10.0,
            line_height: 20.0,
            rows: 24,
            cols: 80,
        };
        assert_eq!(geom.hit_test_live(105.0, 55.0), None);
        assert_eq!(geom.hit_test_live(112.0, 62.0), Some((1, 1)));
        assert_eq!(geom.hit_test_live(137.0, 102.0), Some((3, 3)));

        let mut term = Terminal::new(24, 80);
        term.process_bytes(b"alpha beta\r\ngamma delta\r\n");
        let (l0, c0) = geom.hit_test_logical(112.0, 62.0, 0.0, 24);
        let (l1, c1) = geom.hit_test_logical(152.0, 62.0, 0.0, 24);
        let sel = TerminalSelection {
            start_line: l0,
            start_col: c0,
            end_line: l1,
            end_col: c1,
            rectangular: false,
        };
        assert_eq!(selected_text_from_terminal(&term, sel), "alpha");

        // 3. Verify key encoding, paste sanitization, and mouse reporting modes.
        let modes = TerminalModes {
            sgr_mouse: true,
            mouse_mode: shellumi_terminal::MouseTrackingMode::Normal,
            ..Default::default()
        };
        let mouse_bytes = encode_mouse_event(0, 3, 3, true, false, &Modifiers::default(), &modes)
            .expect("SGR mouse event must encode");
        assert_eq!(mouse_bytes, b"\x1b[<0;3;3M");
        assert_eq!(
            encode_mouse_event(
                0,
                3,
                3,
                true,
                false,
                &Modifiers {
                    shift: true,
                    ..Default::default()
                },
                &modes
            ),
            None,
            "Shift must override remote mouse tracking for local selection"
        );

        let sanitized_paste = encode_paste_bytes("echo hi\x1b[201~evil\n", true);
        assert_eq!(sanitized_paste, b"\x1b[200~echo hievil\r\x1b[201~");

        let ctrl_c = Keystroke {
            modifiers: Modifiers {
                control: true,
                ..Default::default()
            },
            key: "c".to_string(),
            key_char: None,
        };
        assert_eq!(key_bytes_for_modes(&ctrl_c, &modes), Some(vec![0x03]));

        // 4. Verify real PTY session + input byte-budget protection inside GPUI headless main thread.
        #[cfg(windows)]
        let cmd = TerminalCommand {
            program: "cmd".to_string(),
            args: vec!["/C".to_string(), "echo gpui-pty-ok".to_string()],
            cwd: None,
            env: HashMap::new(),
        };
        #[cfg(not(windows))]
        let cmd = TerminalCommand {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "echo gpui-pty-ok".to_string()],
            cwd: None,
            env: HashMap::new(),
        };
        let session = Session::start(cmd);
        let oversized = vec![b'x'; MAX_PASTE_BYTES + 1];
        assert_eq!(
            session.input(oversized),
            Err(SessionInputError::PayloadTooLarge)
        );
        const { assert!(MAX_INPUT_QUEUE_BYTES >= MAX_PASTE_BYTES) };

        let mut pty_term = Terminal::new(24, 80);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            match session.events.try_recv() {
                Ok(SessionEvent::Output(bytes)) => pty_term.process_bytes(&bytes),
                Ok(SessionEvent::Closed) | Ok(SessionEvent::Error(_)) => break,
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
            if collect_screen_text(&pty_term).contains("gpui-pty-ok") {
                break;
            }
        }
        let screen = collect_screen_text(&pty_term);
        assert!(
            screen.contains("gpui-pty-ok"),
            "Expected PTY output in GPUI harness, got: {screen}"
        );

        cx.quit();
    });
    println!("gpui_terminal_harness: PASS");
}

use gpui::{AppContext, EntityInputHandler};
use shellumi_app::{
    byte_to_utf16, range_from_utf16, range_to_utf16, utf16_to_byte, CodeEditorView,
};
use shellumi_core::{EditorBuffer, EditorLanguage, LineEnding};

fn main() {
    gpui::Application::headless().run(|cx| {
        // 1. UTF-8 <-> UTF-16 round-trips for multi-script & surrogate-pair emoji text.
        let sample = "Aé中🙂سلام\r\nPermitRootLogin no\r\n";
        assert_eq!(byte_to_utf16(sample, 0), 0);
        let after_emoji_b = "Aé中🙂".len();
        let after_emoji_u16 = byte_to_utf16(sample, after_emoji_b);
        assert_eq!(utf16_to_byte(sample, after_emoji_u16), after_emoji_b);
        // Mid-surrogate UTF-16 offset must clamp cleanly to char boundary without panic.
        let mid_surrogate = utf16_to_byte(sample, after_emoji_u16 - 1);
        assert!(sample.is_char_boundary(mid_surrogate));
        let r_u16 = range_to_utf16(sample, 1..after_emoji_b);
        assert_eq!(range_from_utf16(sample, &r_u16), 1..after_emoji_b);

        // 2. Real GPUI Entity<CodeEditorView> with 1,500 lines: goto_line, scroll, selection, CRLF.
        let mut big_doc = String::new();
        for i in 1..=1500 {
            big_doc.push_str(&format!("line {i:04}: PermitRootLogin no\r\n"));
        }
        let buf = EditorBuffer::from_bytes(big_doc.as_bytes()).unwrap();
        assert_eq!(buf.line_ending(), LineEnding::Crlf);

        let window_handle = cx
            .open_window(gpui::WindowOptions::default(), |_, cx| {
                cx.new(|cx| CodeEditorView::new(buf, EditorLanguage::SshConfig, false, cx))
            })
            .unwrap();

        window_handle
            .update(cx, |view, window, cx| {
                view.goto_line(1400, cx);
                assert_eq!(view.buffer.line_col().0, 1400);
                assert!(
                    view.scroll_line > 1000,
                    "scroll_line must advance past line 120"
                );

                // Select & test EntityInputHandler UTF-16 contract + IME deduplication
                let cur = view.buffer.cursor();
                view.selection_anchor = Some(cur);
                view.buffer.set_cursor(cur + 9);
                assert_eq!(view.selected_text(), "line 1400");

                let sel = view.selected_text_range(false, window, cx).unwrap();
                assert_eq!(sel.range.end - sel.range.start, 9);

                // Replace selection via IME mark -> unmark -> duplicate commit guard
                view.replace_and_mark_text_in_range(None, "Aé🙂", Some(4..4), window, cx);
                assert!(view.marked_range.is_some());
                view.unmark_text(window, cx);
                assert!(view.marked_range.is_none());
                // Duplicate commit of exact unmarked text must be ignored
                let before_dup = view.buffer.text().len();
                view.replace_text_in_range(None, "Aé🙂", window, cx);
                assert_eq!(view.buffer.text().len(), before_dup);
            })
            .unwrap();

        cx.quit();
    });
    println!("gpui_sftp_harness: PASS");
}

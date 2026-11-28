//! Shellumi Terminal Emulator engine and PTY manager.
//!
//! Provides VT/ANSI sequence parsing, screen grid state management,
//! alternate screens, colors, cursor tracking, and cross-platform PTY execution.

pub mod cell;
pub mod color;
pub mod events;
pub mod grid;
pub mod parser;
pub mod pty;
pub mod recording;
pub mod shell_integration;

pub use cell::{
    cells_to_spans, cells_to_spans_with_combining, Cell, CellAttributes, CellFlags, StyledSpan,
};
pub use color::{Color, NamedColor};
pub use events::{
    sanitize_hyperlink_uri, sanitize_title, CursorShape, HyperlinkTarget, MouseTrackingMode,
    Osc52Policy, TerminalEvent, TerminalModes,
};
pub use grid::{Cursor, SearchBuffer, SearchMatch, SearchOptions, TerminalGrid};
pub use pty::{PtySession, TerminalCommand};
pub use recording::{AsciicastHeader, CastEvent, CastEventType, SessionRecorder, SessionRecording};
pub use shell_integration::{PromptSemanticMark, ShellIntegrationState};

use parser::TerminalHandler;
use vte::Parser;

/// A high-level terminal emulator instance managing grid state, escape sequence parsing, and typed events.
pub struct Terminal {
    grid: TerminalGrid,
    parser: Parser,
    title: Option<String>,
    shell_integration: ShellIntegrationState,
    recorder: Option<SessionRecorder>,
    events: Vec<TerminalEvent>,
    foreground: Color,
    background: Color,
    attributes: CellAttributes,
}

impl Terminal {
    /// Creates a new terminal emulator with the given dimensions.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            grid: TerminalGrid::new(rows, cols),
            parser: Parser::new(),
            title: None,
            shell_integration: ShellIntegrationState::new(),
            recorder: None,
            events: Vec::new(),
            foreground: Color::Default,
            background: Color::Default,
            attributes: CellAttributes::default(),
        }
    }

    /// Feeds incoming raw bytes (from PTY or SSH stream) into the VT parser.
    pub fn process_bytes(&mut self, bytes: &[u8]) {
        if let Some(ref mut rec) = self.recorder {
            rec.record_output(bytes);
        }
        let mut handler = TerminalHandler::new(&mut self.grid, &mut self.title, &mut self.events)
            .with_shell_integration(&mut self.shell_integration);
        handler.fg = self.foreground;
        handler.bg = self.background;
        handler.attrs = self.attributes;
        for &byte in bytes {
            self.parser.advance(&mut handler, byte);
        }
        self.foreground = handler.fg;
        self.background = handler.bg;
        self.attributes = handler.attrs;
    }

    /// Drains all typed terminal events emitted since the last drain.
    pub fn drain_events(&mut self) -> Vec<TerminalEvent> {
        std::mem::take(&mut self.events)
    }

    /// Returns active terminal protocol modes.
    pub fn modes(&self) -> &TerminalModes {
        self.grid.modes()
    }

    /// Returns the OSC 8 hyperlink target at the given active screen coordinates, if any.
    pub fn hyperlink_at(&self, row: usize, col: usize) -> Option<&HyperlinkTarget> {
        self.grid.hyperlink_at(row, col)
    }

    /// Searches across scrollback and active screen with literal/case/regex options.
    pub fn search_with_options(&self, opts: &SearchOptions) -> Result<Vec<SearchMatch>, String> {
        self.grid.search_with_options(opts)
    }

    /// Feeds an incoming UTF-8 string into the VT parser.
    pub fn process_str(&mut self, s: &str) {
        self.process_bytes(s.as_bytes());
    }

    /// Accesses semantic shell integration state (OSC 133, OSC 7, etc.).
    pub fn shell_integration(&self) -> &ShellIntegrationState {
        &self.shell_integration
    }

    /// Mutably accesses semantic shell integration state.
    pub fn shell_integration_mut(&mut self) -> &mut ShellIntegrationState {
        &mut self.shell_integration
    }

    /// Returns the current working directory if reported by the shell via OSC 7 or OSC 9;9.
    pub fn current_working_directory(&self) -> Option<&str> {
        self.shell_integration.current_dir()
    }

    /// Returns the exit code of the last completed command if reported via OSC 133;D.
    pub fn last_exit_code(&self) -> Option<i32> {
        self.shell_integration.last_exit_code()
    }

    /// Returns the current semantic prompt mark (e.g. PromptStart, CommandExecuted).
    pub fn prompt_mark(&self) -> Option<&PromptSemanticMark> {
        self.shell_integration.current_mark.as_ref()
    }

    /// Returns command history captured via semantic shell integration.
    pub fn command_history(&self) -> &[String] {
        &self.shell_integration.command_history
    }

    /// Accesses the underlying terminal screen grid.
    pub fn grid(&self) -> &TerminalGrid {
        &self.grid
    }

    /// Mutably accesses the underlying terminal screen grid.
    pub fn grid_mut(&mut self) -> &mut TerminalGrid {
        &mut self.grid
    }

    /// Returns the window/tab title if set via OSC 0 or OSC 2.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Resizes the terminal screen grid.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        if let Some(ref mut rec) = self.recorder {
            rec.record_resize(cols, rows);
        }
        self.grid.resize(rows, cols);
    }

    /// Starts recording session input/output in Asciicast v2 format.
    pub fn start_recording(&mut self, title: Option<String>) {
        let mut recorder = SessionRecorder::new(self.grid.cols(), self.grid.rows(), title);
        recorder.start();
        self.recorder = Some(recorder);
    }

    /// Stops session recording and returns the recorded session if active.
    pub fn stop_recording(&mut self) -> Option<SessionRecording> {
        self.recorder.take().map(|mut r| r.stop())
    }

    /// Returns true if session recording is active.
    pub fn is_recording(&self) -> bool {
        self.recorder.as_ref().is_some_and(|r| r.is_active())
    }

    pub fn pause_recording(&mut self) {
        if let Some(ref mut rec) = self.recorder {
            rec.pause();
        }
    }

    pub fn resume_recording(&mut self) {
        if let Some(ref mut rec) = self.recorder {
            rec.resume();
        }
    }

    pub fn is_recording_paused(&self) -> bool {
        self.recorder.as_ref().is_some_and(|r| r.is_paused())
    }

    pub fn is_recording_truncated(&self) -> bool {
        self.recorder.as_ref().is_some_and(|r| r.is_truncated())
    }

    pub fn recording_header(&self) -> Option<AsciicastHeader> {
        self.recorder.as_ref().map(|r| r.header().clone())
    }

    pub fn recording_byte_count(&self) -> usize {
        self.recorder.as_ref().map_or(0, |r| r.byte_count())
    }

    pub fn recording_unflushed_bytes(&self) -> usize {
        self.recorder.as_ref().map_or(0, |r| r.unflushed_bytes())
    }

    pub fn peek_unflushed_recording_events(&self) -> (usize, Vec<CastEvent>) {
        self.recorder
            .as_ref()
            .map_or((0, Vec::new()), |r| r.peek_unflushed_events())
    }

    pub fn mark_recording_events_flushed(&mut self, count: usize) {
        if let Some(ref mut rec) = self.recorder {
            rec.mark_events_flushed(count);
        }
    }

    /// Records user keystroke input sent to the terminal PTY.
    pub fn record_input(&mut self, bytes: &[u8]) {
        if let Some(ref mut rec) = self.recorder {
            rec.record_input(bytes);
        }
    }

    /// Records input only when input recording is enabled and `sensitive` is false.
    pub fn record_input_with_policy(&mut self, bytes: &[u8], sensitive: bool) {
        if let Some(ref mut rec) = self.recorder {
            rec.record_input_with_policy(bytes, sensitive);
        }
    }

    /// Enables or disables input capture for the active recorder.
    pub fn set_recording_input_enabled(&mut self, enabled: bool) {
        if let Some(ref mut rec) = self.recorder {
            rec.set_input_recording_enabled(enabled);
        }
    }

    /// Returns whether input capture is enabled for the active recorder.
    pub fn is_recording_input_enabled(&self) -> bool {
        self.recorder
            .as_ref()
            .is_some_and(|r| r.input_recording_enabled())
    }

    /// Returns the number of events recorded in the active session.
    pub fn recording_event_count(&self) -> usize {
        self.recorder.as_ref().map_or(0, |r| r.event_count())
    }

    /// Returns elapsed duration since session recording started.
    pub fn recording_duration(&self) -> std::time::Duration {
        self.recorder
            .as_ref()
            .map_or(std::time::Duration::ZERO, |r| r.elapsed_duration())
    }

    /// Sets the maximum scrollback capacity.
    pub fn set_scrollback_limit(&mut self, limit: usize) {
        self.grid.set_scrollback_limit(limit);
    }

    /// Converts visible screen content to a formatted string.
    pub fn screen_to_string(&self) -> String {
        self.grid.screen_to_string()
    }

    /// Returns the styled text spans for a given row in the active screen.
    pub fn row_spans(&self, row: usize) -> Option<Vec<StyledSpan>> {
        self.grid.row_spans(row)
    }

    /// Returns plain text for an active screen row, with trailing whitespace trimmed.
    pub fn row_text(&self, row: usize) -> Option<String> {
        self.grid.row_text(row)
    }

    /// Returns the number of lines stored in scrollback history.
    pub fn scrollback_count(&self) -> usize {
        self.grid.scrollback_count()
    }

    /// Returns the styled text spans for a given row in the scrollback history.
    pub fn scrollback_row_spans(&self, idx: usize) -> Option<Vec<StyledSpan>> {
        self.grid.scrollback_row_spans(idx)
    }

    /// Search for text occurrences across scrollback history and active screen.
    pub fn search(&self, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
        self.grid.search(query, case_sensitive)
    }

    /// Returns styled text spans for an active screen row, highlighting matching columns.
    pub fn row_spans_highlighted(
        &self,
        row: usize,
        highlight_cols: &[std::ops::Range<usize>],
    ) -> Option<Vec<StyledSpan>> {
        self.grid.row_spans_highlighted(row, highlight_cols)
    }

    /// Returns styled text spans for a scrollback row, highlighting matching columns.
    pub fn scrollback_row_spans_highlighted(
        &self,
        idx: usize,
        highlight_cols: &[std::ops::Range<usize>],
    ) -> Option<Vec<StyledSpan>> {
        self.grid
            .scrollback_row_spans_highlighted(idx, highlight_cols)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn retains_text_attributes_across_input_chunks_until_reset() {
        let mut terminal = Terminal::new(4, 40);
        terminal.process_bytes(b"\x1b[1;31mfirst");
        terminal.process_bytes(b" second");
        terminal.process_bytes(b"\x1b[0m plain");
        let first = terminal.grid().get_cell(0, 0).unwrap();
        let second = terminal.grid().get_cell(0, 6).unwrap();
        let plain = terminal.grid().get_cell(0, 13).unwrap();
        assert_eq!(first.fg, Color::Named(NamedColor::Red));
        assert!(first.attrs.bold);
        assert_eq!(second.fg, Color::Named(NamedColor::Red));
        assert!(second.attrs.bold);
        assert_eq!(plain.fg, Color::Default);
        assert!(!plain.attrs.bold);
    }

    #[test]
    fn parses_plain_text_and_newlines() {
        let mut term = Terminal::new(10, 40);
        term.process_str("Hello World\r\nSecond Line");
        assert_eq!(term.grid().line_to_string(0).unwrap(), "Hello World");
        assert_eq!(term.grid().line_to_string(1).unwrap(), "Second Line");
        assert_eq!(term.grid().cursor().row, 1);
        assert_eq!(term.grid().cursor().col, 11);
    }

    #[test]
    fn handles_carriage_return_and_backspace() {
        let mut term = Terminal::new(5, 20);
        term.process_str("Abc\rX");
        assert_eq!(term.grid().line_to_string(0).unwrap(), "Xbc");

        // Cursor is at col 1. Write 'Y' (cursor at col 2), backspace (cursor at col 1), then 'Z'
        term.process_str("Y\x08Z");
        assert_eq!(term.grid().line_to_string(0).unwrap(), "XZc");
    }

    #[test]
    fn handles_colors_and_text_attributes() {
        let mut term = Terminal::new(5, 20);
        // Red bold "Hi" followed by reset "!"
        term.process_str("\x1b[1;31mHi\x1b[0m!");

        let cell0 = term.grid().get_cell(0, 0).unwrap();
        assert_eq!(cell0.c, 'H');
        assert_eq!(cell0.fg, Color::Named(NamedColor::Red));
        assert!(cell0.attrs.bold);

        let cell1 = term.grid().get_cell(0, 1).unwrap();
        assert_eq!(cell1.c, 'i');
        assert_eq!(cell1.fg, Color::Named(NamedColor::Red));
        assert!(cell1.attrs.bold);

        let cell2 = term.grid().get_cell(0, 2).unwrap();
        assert_eq!(cell2.c, '!');
        assert_eq!(cell2.fg, Color::Default);
        assert!(!cell2.attrs.bold);
    }

    #[test]
    fn handles_cursor_positioning_and_clearing() {
        let mut term = Terminal::new(10, 40);
        // Position at row 3, col 5 (1-based in ANSI: 3;5)
        term.process_str("\x1b[3;5HPoint");
        assert_eq!(term.grid().cursor().row, 2);
        assert_eq!(term.grid().cursor().col, 9);
        assert_eq!(term.grid().line_to_string(2).unwrap(), "    Point");

        // Clear line from cursor back
        term.process_str("\x1b[1K");
        // Row should now be blank up to col 9
        assert_eq!(term.grid().line_to_string(2).unwrap(), "");
    }

    #[test]
    fn handles_scrolling_and_scrollback_history() {
        let mut term = Terminal::new(3, 10);
        term.process_str("Line 1\r\nLine 2\r\nLine 3\r\nLine 4");
        // Line 1 should have scrolled into scrollback history
        assert_eq!(term.grid().scrollback_count(), 1);
        let scrolled_spans = term.grid().scrollback_row_spans(0).unwrap();
        let scrolled_str: String = scrolled_spans.into_iter().map(|s| s.text).collect();
        assert_eq!(scrolled_str.trim_end(), "Line 1");

        // Visible rows should now be Line 2, Line 3, Line 4
        assert_eq!(term.grid().line_to_string(0).unwrap(), "Line 2");
        assert_eq!(term.grid().line_to_string(1).unwrap(), "Line 3");
        assert_eq!(term.grid().line_to_string(2).unwrap(), "Line 4");
    }

    #[test]
    fn handles_alternate_screen_buffer() {
        let mut term = Terminal::new(5, 20);
        term.process_str("Primary Content");
        assert_eq!(term.grid().line_to_string(0).unwrap(), "Primary Content");
        assert!(!term.grid().is_alt_screen());

        // Switch to alt screen (DECSET 1049)
        term.process_str("\x1b[?1049h");
        assert!(term.grid().is_alt_screen());
        term.process_str("Alt Content");
        assert_eq!(term.grid().line_to_string(0).unwrap(), "Alt Content");

        // Switch back to primary screen (DECRST 1049)
        term.process_str("\x1b[?1049l");
        assert!(!term.grid().is_alt_screen());
        assert_eq!(term.grid().line_to_string(0).unwrap(), "Primary Content");
    }

    #[test]
    fn handles_window_title_osc() {
        let mut term = Terminal::new(5, 20);
        term.process_str("\x1b]0;Shellumi Session\x07");
        assert_eq!(term.title(), Some("Shellumi Session"));
    }

    #[test]
    fn sanitizes_window_title_osc_sequences() {
        let mut term = Terminal::new(5, 20);
        // Title with non-printable control characters (BEL, ESC) and long content
        let long_title = "A".repeat(300);
        term.process_str(&format!("\x1b]0;Valid\x01\x02Title {long_title}\x07"));
        let title = term.title().expect("should have title");
        assert!(title.starts_with("ValidTitle "));
        assert_eq!(title.len(), 256); // Capped to 256
        assert!(!title.contains('\x01'));
        assert!(!title.contains('\x02'));
    }

    #[test]
    fn handles_unicode_cjk_and_emoji_output() {
        let mut term = Terminal::new(5, 40);
        term.process_str("Shell: 你好世界 🚀 [OK]\r\n");
        term.process_str("Box: ┌─┬─┐ │ │\r\n");
        let line0 = term.grid().line_to_string(0).unwrap();
        assert!(line0.contains("你好世界"));
        assert!(line0.contains("🚀"));
        assert!(line0.contains("[OK]"));

        let line1 = term.grid().line_to_string(1).unwrap();
        assert!(line1.contains("┌─┬─┐"));
    }

    #[test]
    fn handles_large_output_workload_and_scrollback_capacity() {
        let mut term = Terminal::new(24, 80);
        term.set_scrollback_limit(500);
        // Simulate massive 2,000 line command stream
        for i in 1..=2000 {
            term.process_str(&format!(
                "Log stream row {i}: status=OK payload_bytes=1024\r\n"
            ));
        }
        // Verify scrollback limit is strictly respected
        assert_eq!(term.grid().scrollback_count(), 500);
        // Verify cursor bounds
        assert!(term.grid().cursor().row < 24);
        assert!(term.grid().cursor().col < 80);
        // Verify latest visible lines
        let last_line = term.grid().line_to_string(23).unwrap();
        assert!(last_line.contains("Log stream row 2000") || last_line.is_empty());
    }

    #[test]
    fn spawns_local_pty_and_executes_echo() {
        #[cfg(windows)]
        let cmd = TerminalCommand::new("cmd").args(["/C", "echo", "hello from pty"]);
        #[cfg(not(windows))]
        let cmd = TerminalCommand::new("/bin/echo").arg("hello from pty");
        let session = PtySession::spawn(&cmd, 24, 80).expect("failed to spawn echo in PTY");
        let mut reader = session.try_clone_reader().expect("failed to clone reader");
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });

        let mut term = Terminal::new(24, 80);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(chunk) => {
                    term.process_bytes(&chunk);
                    if term.screen_to_string().contains("hello from pty") {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(session);
        assert!(term.screen_to_string().contains("hello from pty"));
    }

    #[test]
    fn handles_osc_133_and_cwd_in_terminal_stream() {
        let mut term = Terminal::new(24, 80);

        // Shell prompt start (OSC 133;A) and CWD (OSC 7)
        term.process_str("\x1b]133;A\x07");
        term.process_str("\x1b]7;file://ubuntu-server/var/log\x07");
        assert_eq!(term.prompt_mark(), Some(&PromptSemanticMark::PromptStart));
        assert_eq!(term.current_working_directory(), Some("/var/log"));

        // Prompt end / command start (OSC 133;B) and command line property (OSC 133;P;cl=ls -la)
        term.process_str("\x1b]133;B\x07");
        term.process_str("\x1b]133;P;cl=ls -la\x07");
        assert_eq!(term.prompt_mark(), Some(&PromptSemanticMark::CommandStart));

        // Command executed (OSC 133;C)
        term.process_str("\x1b]133;C\x07");
        assert_eq!(
            term.prompt_mark(),
            Some(&PromptSemanticMark::CommandExecuted)
        );
        assert_eq!(term.command_history(), &["ls -la".to_string()]);

        // Command finished with exit code 0 (OSC 133;D;0)
        term.process_str("\x1b]133;D;0\x07");
        assert_eq!(term.last_exit_code(), Some(0));

        // Next prompt start (OSC 133;A) with changed directory via OSC 9;9
        term.process_str("\x1b]9;9;\"/home/dev/projects\"\x07");
        assert_eq!(term.current_working_directory(), Some("/home/dev/projects"));
    }

    #[test]
    fn tracks_and_records_terminal_session_to_asciicast() {
        let mut term = Terminal::new(24, 80);
        assert!(!term.is_recording());

        term.start_recording(Some("CI Recording".to_string()));
        assert!(term.is_recording());

        // Input recording is disabled by default; opt-in enables input capture
        term.record_input(b"ignored_before_opt_in\r");
        assert_eq!(term.recording_event_count(), 0);
        term.set_recording_input_enabled(true);
        term.record_input(b"uptime\r");
        term.process_str(" 10:00:00 up 42 days, 1 user\r\n");
        term.resize(30, 100);

        assert_eq!(term.recording_event_count(), 3);

        let recording = term.stop_recording().expect("recording should exist");
        assert!(!term.is_recording());
        assert_eq!(recording.events.len(), 3);
        assert_eq!(recording.header.title.as_deref(), Some("CI Recording"));

        let cast_output = recording.to_asciicast_v2().expect("should serialize cast");
        assert!(cast_output.contains("\"version\":2"));
        assert!(cast_output.contains("uptime\\r"));
        assert!(cast_output.contains("10:00:00"));
        assert!(cast_output.contains("100x30"));
    }

    #[test]
    fn vt_t01_delayed_autowrap_scroll_margins_and_editing_ops() {
        let mut term = Terminal::new(5, 10);
        // Delayed autowrap: writing 10 chars leaves cursor at col 9 with pending_wrap=true
        term.process_str("0123456789");
        assert_eq!(term.grid().cursor().row, 0);
        assert_eq!(term.grid().cursor().col, 9);
        assert!(term.grid().pending_wrap());
        // Writing 11th char wraps to row 1 col 1
        term.process_str("A");
        assert_eq!(term.grid().cursor().row, 1);
        assert_eq!(term.grid().cursor().col, 1);
        assert_eq!(term.grid().line_to_string(0).unwrap(), "0123456789");
        assert_eq!(term.grid().line_to_string(1).unwrap(), "A");

        // Scroll margins (DECSTBM 2..4) and origin mode (DECOM)
        term.process_str("\x1b[2J\x1b[H");
        term.process_str("\x1b[2;4r");
        term.process_str("\x1b[?6h");
        term.process_str("\x1b[1;1HRow2\r\nRow3\r\nRow4\r\nRow5");
        assert_eq!(term.grid().line_to_string(0).unwrap(), "");
        assert_eq!(term.grid().line_to_string(1).unwrap(), "Row3");
        assert_eq!(term.grid().line_to_string(2).unwrap(), "Row4");
        assert_eq!(term.grid().line_to_string(3).unwrap(), "Row5");
        assert_eq!(term.grid().line_to_string(4).unwrap(), "");

        // ICH / DCH / ECH
        term.process_str("\x1b[?6l\x1b[r\x1b[2J\x1b[1;1Habcdef");
        term.process_str("\x1b[1;3H\x1b[2@XY"); // Insert 2 blanks at col 3, write XY
        assert_eq!(term.grid().line_to_string(0).unwrap(), "abXYcdef");
        term.process_str("\x1b[1;3H\x1b[2P"); // Delete 2 chars at col 3
        assert_eq!(term.grid().line_to_string(0).unwrap(), "abcdef");
        term.process_str("\x1b[1;2H\x1b[2X"); // Erase 2 chars at col 2
        assert_eq!(term.grid().line_to_string(0).unwrap(), "a  def");
    }

    #[test]
    fn vt_t01_device_queries_and_byte_by_byte_chunking() {
        let seq = b"\x1b[?1;2004;1006h\x1b[5n\x1b[3;7H\x1b[6n\x1b[c\x1b[>c";
        let mut whole = Terminal::new(24, 80);
        whole.process_bytes(seq);
        let whole_events = whole.drain_events();

        let mut chunked = Terminal::new(24, 80);
        for b in seq {
            chunked.process_bytes(&[*b]);
        }
        let chunked_events = chunked.drain_events();
        assert_eq!(whole_events, chunked_events);
        assert!(chunked.modes().application_cursor);
        assert!(chunked.modes().bracketed_paste);
        assert!(chunked.modes().sgr_mouse);

        let replies: Vec<Vec<u8>> = chunked_events
            .into_iter()
            .filter_map(|e| match e {
                TerminalEvent::ProtocolReply(b) => Some(b),
                _ => None,
            })
            .collect();
        assert_eq!(
            replies,
            vec![
                b"\x1b[0n".to_vec(),
                b"\x1b[3;7R".to_vec(),
                b"\x1b[?62;22c".to_vec(),
                b"\x1b[>1;10;0c".to_vec(),
            ]
        );
    }

    #[test]
    fn vt_t02_unicode_combining_wide_overwrite_and_reflow() {
        let mut term = Terminal::new(4, 10);
        // Combining mark + wide CJK + Persian text
        term.process_str("e\u{0301}中سلام");
        let spans = term.row_spans(0).unwrap();
        let text: String = spans.into_iter().map(|s| s.text).collect();
        assert_eq!(text, "e\u{0301}中سلام");

        // Overwrite second half of wide char '中' (col 2) clears leading cell (col 1)
        term.process_str("\x1b[1;3HX");
        let after = term.grid().line_to_string(0).unwrap();
        assert!(after.starts_with("e\u{0301} X"));

        // Reflow test: wrap long line across 10 cols, then grow to 20 cols
        let mut reflow = Terminal::new(4, 10);
        reflow.process_str("1234567890ABCDE");
        assert_eq!(reflow.grid().line_to_string(0).unwrap(), "1234567890");
        assert_eq!(reflow.grid().line_to_string(1).unwrap(), "ABCDE");
        reflow.resize(4, 20);
        assert_eq!(reflow.grid().line_to_string(0).unwrap(), "1234567890ABCDE");
        assert_eq!(reflow.grid().line_to_string(1).unwrap(), "");
    }

    #[test]
    fn vt_t02_fuzz_malformed_escape_streams_and_resizes_never_panic() {
        let mut term = Terminal::new(10, 20);
        let mut seed: u64 = 0x5eed_cafe_1234_5678;
        let mut next_u8 = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 56) as u8
        };
        let mut buf = [0u8; 256];
        for step in 0..40 {
            for b in &mut buf {
                *b = if next_u8() % 4 == 0 { 0x1b } else { next_u8() };
            }
            term.process_bytes(&buf);
            let r = ((next_u8() as usize) % 30).max(1);
            let c = ((next_u8() as usize) % 60).max(1);
            if step % 5 == 0 {
                term.resize(r, c);
            }
            assert!(term.grid().cursor().row < term.grid().rows());
            assert!(term.grid().cursor().col < term.grid().cols());
            let _ = term.drain_events();
        }
    }

    #[test]
    fn vt_t04_osc_security_bell_vs_osc_terminator_and_hyperlinks() {
        let mut term = Terminal::new(5, 40);
        // OSC 0 terminated by BEL must NOT emit Bell event
        term.process_str("\x1b]0;Safe Title\x07");
        let events = term.drain_events();
        assert!(!events.contains(&TerminalEvent::Bell));
        assert!(events.contains(&TerminalEvent::TitleChanged(Some("Safe Title".into()))));

        // Standalone BEL emits Bell event
        term.process_bytes(b"\x07");
        assert_eq!(term.drain_events(), vec![TerminalEvent::Bell]);

        // OSC 8 unsafe URI schemes are rejected; safe https is accepted
        term.process_str("\x1b]8;;javascript:alert(1)\x07bad\x1b]8;;\x07");
        assert!(term.hyperlink_at(0, 0).is_none());
        term.process_str("\x1b]8;id=doc;https://example.com/docs\x07link\x1b]8;;\x07");
        let link = term
            .hyperlink_at(0, 4)
            .expect("safe hyperlink should attach");
        assert_eq!(link.uri, "https://example.com/docs");
        assert_eq!(link.id.as_deref(), Some("doc"));

        // OSC 52 read ("?") must emit nothing; write emits ClipboardWriteRequest only
        term.process_str("\x1b]52;c;?\x07");
        assert!(term.drain_events().is_empty());
        term.process_str("\x1b]52;c;aGVsbG8=\x07");
        assert_eq!(
            term.drain_events(),
            vec![TerminalEvent::ClipboardWriteRequest("hello".into())]
        );
    }
}

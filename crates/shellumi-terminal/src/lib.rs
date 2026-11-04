//! Shellumi Terminal Emulator engine and PTY manager.
//!
//! Provides VT/ANSI sequence parsing, screen grid state management,
//! alternate screens, colors, cursor tracking, and cross-platform PTY execution.

pub mod cell;
pub mod color;
pub mod grid;
pub mod parser;
pub mod pty;

pub use cell::{cells_to_spans, Cell, CellAttributes, StyledSpan};
pub use color::{Color, NamedColor};
pub use grid::{Cursor, SearchBuffer, SearchMatch, TerminalGrid};
pub use pty::{PtySession, TerminalCommand};

use parser::TerminalHandler;
use vte::Parser;

/// A high-level terminal emulator instance managing grid state and escape sequence parsing.
pub struct Terminal {
    grid: TerminalGrid,
    parser: Parser,
    title: Option<String>,
}

impl Terminal {
    /// Creates a new terminal emulator with the given dimensions.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            grid: TerminalGrid::new(rows, cols),
            parser: Parser::new(),
            title: None,
        }
    }

    /// Feeds incoming raw bytes (from PTY or SSH stream) into the VT parser.
    pub fn process_bytes(&mut self, bytes: &[u8]) {
        let mut handler = TerminalHandler::new(&mut self.grid, &mut self.title);
        for &byte in bytes {
            self.parser.advance(&mut handler, byte);
        }
    }

    /// Feeds an incoming UTF-8 string into the VT parser.
    pub fn process_str(&mut self, s: &str) {
        self.process_bytes(s.as_bytes());
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
        self.grid.resize(rows, cols);
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
        assert_eq!(term.grid().scrollback().len(), 1);
        let scrolled = &term.grid().scrollback()[0];
        let scrolled_str: String = scrolled.iter().map(|c| c.c).collect();
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
        assert_eq!(term.grid().scrollback().len(), 500);
        // Verify cursor bounds
        assert!(term.grid().cursor().row < 24);
        assert!(term.grid().cursor().col < 80);
        // Verify latest visible lines
        let last_line = term.grid().line_to_string(23).unwrap();
        assert!(last_line.contains("Log stream row 2000") || last_line.is_empty());
    }

    #[test]
    fn spawns_local_pty_and_executes_echo() {
        let cmd = TerminalCommand::new("/bin/echo").arg("hello from pty");
        let session = PtySession::spawn(&cmd, 24, 80).expect("failed to spawn echo in PTY");
        let mut reader = session.try_clone_reader().expect("failed to clone reader");

        let mut output = Vec::new();
        let mut buf = [0u8; 1024];
        // Read until EOF from echo
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            output.extend_from_slice(&buf[..n]);
        }

        let mut term = Terminal::new(24, 80);
        term.process_bytes(&output);
        assert!(term.screen_to_string().contains("hello from pty"));
    }
}

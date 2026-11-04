//! Terminal grid and screen buffer management.

use crate::cell::{cells_to_spans, Cell, StyledSpan};
use grid::Grid;
use std::collections::VecDeque;

const DEFAULT_SCROLLBACK_CAPACITY: usize = 10_000;

/// Cursor position and state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub visible: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
        }
    }
}

/// The terminal grid containing the active screen cells and scrollback history.
#[derive(Clone, Debug)]
pub struct TerminalGrid {
    rows: usize,
    cols: usize,
    primary_screen: Grid<Cell>,
    alt_screen: Option<Grid<Cell>>,
    is_alt_screen: bool,
    cursor: Cursor,
    saved_cursor: Cursor,
    scrollback: VecDeque<Vec<Cell>>,
    scrollback_limit: usize,
}

impl TerminalGrid {
    pub fn new(rows: usize, cols: usize) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        Self {
            rows,
            cols,
            primary_screen: Grid::init(rows, cols, Cell::default()),
            alt_screen: None,
            is_alt_screen: false,
            cursor: Cursor::default(),
            saved_cursor: Cursor::default(),
            scrollback: VecDeque::new(),
            scrollback_limit: DEFAULT_SCROLLBACK_CAPACITY,
        }
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursor
    }

    pub fn is_alt_screen(&self) -> bool {
        self.is_alt_screen
    }

    pub fn scrollback(&self) -> &VecDeque<Vec<Cell>> {
        &self.scrollback
    }

    /// Updates the maximum scrollback capacity and trims excess history if necessary.
    pub fn set_scrollback_limit(&mut self, limit: usize) {
        self.scrollback_limit = limit;
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.pop_front();
        }
    }

    /// Returns the current maximum scrollback line limit.
    pub fn scrollback_limit(&self) -> usize {
        self.scrollback_limit
    }

    fn active_screen(&self) -> &Grid<Cell> {
        if self.is_alt_screen {
            self.alt_screen.as_ref().unwrap_or(&self.primary_screen)
        } else {
            &self.primary_screen
        }
    }

    fn active_screen_mut(&mut self) -> &mut Grid<Cell> {
        if self.is_alt_screen {
            self.alt_screen.as_mut().unwrap_or(&mut self.primary_screen)
        } else {
            &mut self.primary_screen
        }
    }

    pub fn get_cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.active_screen().get(row, col)
    }

    pub fn get_cell_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        self.active_screen_mut().get_mut(row, col)
    }

    pub fn set_cell(&mut self, row: usize, col: usize, cell: Cell) {
        if let Some(target) = self.get_cell_mut(row, col) {
            *target = cell;
        }
    }

    pub fn write_char_at_cursor(&mut self, cell: Cell) {
        let r = self.cursor.row;
        let c = self.cursor.col;
        if r < self.rows && c < self.cols {
            self.set_cell(r, c, cell);
            self.cursor.col += 1;
            if self.cursor.col >= self.cols {
                self.cursor.col = 0;
                self.cursor.row += 1;
                if self.cursor.row >= self.rows {
                    self.scroll_up();
                    self.cursor.row = self.rows - 1;
                }
            }
        }
    }

    pub fn scroll_up(&mut self) {
        if !self.is_alt_screen {
            // Push top line to scrollback
            let mut top_line = Vec::with_capacity(self.cols);
            for c in 0..self.cols {
                top_line.push(*self.primary_screen.get(0, c).unwrap_or(&Cell::default()));
            }
            if self.scrollback.len() >= self.scrollback_limit {
                self.scrollback.pop_front();
            }
            self.scrollback.push_back(top_line);
        }

        // Shift rows up
        for r in 0..(self.rows - 1) {
            for c in 0..self.cols {
                let next_cell = *self
                    .active_screen()
                    .get(r + 1, c)
                    .unwrap_or(&Cell::default());
                self.set_cell(r, c, next_cell);
            }
        }

        // Clear last row
        let last_row = self.rows - 1;
        for c in 0..self.cols {
            self.set_cell(last_row, c, Cell::default());
        }
    }

    pub fn scroll_down(&mut self) {
        for r in (1..self.rows).rev() {
            for c in 0..self.cols {
                let prev_cell = *self
                    .active_screen()
                    .get(r - 1, c)
                    .unwrap_or(&Cell::default());
                self.set_cell(r, c, prev_cell);
            }
        }
        for c in 0..self.cols {
            self.set_cell(0, c, Cell::default());
        }
    }

    pub fn clear_all(&mut self) {
        let rows = self.rows;
        let cols = self.cols;
        for r in 0..rows {
            for c in 0..cols {
                self.set_cell(r, c, Cell::default());
            }
        }
    }

    pub fn clear_from_cursor_down(&mut self) {
        let cur_r = self.cursor.row;
        let cur_c = self.cursor.col;
        for c in cur_c..self.cols {
            self.set_cell(cur_r, c, Cell::default());
        }
        for r in (cur_r + 1)..self.rows {
            for c in 0..self.cols {
                self.set_cell(r, c, Cell::default());
            }
        }
    }

    pub fn clear_from_cursor_up(&mut self) {
        let cur_r = self.cursor.row;
        let cur_c = self.cursor.col;
        for r in 0..cur_r {
            for c in 0..self.cols {
                self.set_cell(r, c, Cell::default());
            }
        }
        for c in 0..=cur_c.min(self.cols - 1) {
            self.set_cell(cur_r, c, Cell::default());
        }
    }

    pub fn clear_line(&mut self, mode: u8) {
        let r = self.cursor.row;
        match mode {
            0 => {
                // Clear from cursor to end of line
                for c in self.cursor.col..self.cols {
                    self.set_cell(r, c, Cell::default());
                }
            }
            1 => {
                // Clear from start of line to cursor
                for c in 0..=self.cursor.col.min(self.cols - 1) {
                    self.set_cell(r, c, Cell::default());
                }
            }
            2 => {
                // Clear entire line
                for c in 0..self.cols {
                    self.set_cell(r, c, Cell::default());
                }
            }
            _ => {}
        }
    }

    pub fn enter_alt_screen(&mut self) {
        if !self.is_alt_screen {
            self.alt_screen = Some(Grid::init(self.rows, self.cols, Cell::default()));
            self.is_alt_screen = true;
            self.saved_cursor = self.cursor;
            self.cursor = Cursor::default();
        }
    }

    pub fn exit_alt_screen(&mut self) {
        if self.is_alt_screen {
            self.alt_screen = None;
            self.is_alt_screen = false;
            self.cursor = self.saved_cursor;
        }
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor = self.cursor;
    }

    pub fn restore_cursor(&mut self) {
        self.cursor = self.saved_cursor;
        self.cursor.row = self.cursor.row.min(self.rows.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(self.cols.saturating_sub(1));
    }

    pub fn resize(&mut self, new_rows: usize, new_cols: usize) {
        let new_rows = new_rows.max(1);
        let new_cols = new_cols.max(1);
        if new_rows == self.rows && new_cols == self.cols {
            return;
        }

        let mut new_primary = Grid::init(new_rows, new_cols, Cell::default());
        let copy_rows = self.rows.min(new_rows);
        let copy_cols = self.cols.min(new_cols);

        for r in 0..copy_rows {
            for c in 0..copy_cols {
                if let Some(cell) = self.primary_screen.get(r, c) {
                    new_primary[(r, c)] = *cell;
                }
            }
        }
        self.primary_screen = new_primary;

        if let Some(ref alt) = self.alt_screen {
            let mut new_alt = Grid::init(new_rows, new_cols, Cell::default());
            for r in 0..copy_rows {
                for c in 0..copy_cols {
                    if let Some(cell) = alt.get(r, c) {
                        new_alt[(r, c)] = *cell;
                    }
                }
            }
            self.alt_screen = Some(new_alt);
        }

        self.rows = new_rows;
        self.cols = new_cols;
        self.cursor.row = self.cursor.row.min(new_rows - 1);
        self.cursor.col = self.cursor.col.min(new_cols - 1);
    }

    pub fn line_to_string(&self, row: usize) -> Option<String> {
        if row >= self.rows {
            return None;
        }
        let mut s = String::with_capacity(self.cols);
        for c in 0..self.cols {
            let cell = self.get_cell(row, c).copied().unwrap_or_default();
            s.push(cell.c);
        }
        Some(s.trim_end().to_string())
    }

    pub fn screen_to_string(&self) -> String {
        let mut lines = Vec::with_capacity(self.rows);
        for r in 0..self.rows {
            lines.push(self.line_to_string(r).unwrap_or_default());
        }
        // Remove trailing empty lines
        while lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }

    /// Returns the styled text spans for a given row in the active screen.
    pub fn row_spans(&self, row: usize) -> Option<Vec<StyledSpan>> {
        if row >= self.rows {
            return None;
        }
        let mut cells = Vec::with_capacity(self.cols);
        for c in 0..self.cols {
            cells.push(self.get_cell(row, c).copied().unwrap_or_default());
        }
        Some(cells_to_spans(&cells))
    }

    /// Returns the total number of lines stored in scrollback history.
    pub fn scrollback_count(&self) -> usize {
        self.scrollback.len()
    }

    /// Returns the styled text spans for a given row in the scrollback buffer.
    pub fn scrollback_row_spans(&self, idx: usize) -> Option<Vec<StyledSpan>> {
        self.scrollback.get(idx).map(|cells| cells_to_spans(cells))
    }

    /// Search for text occurrences across the scrollback history and active screen.
    pub fn search(&self, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
        let trimmed_query = query.trim();
        if trimmed_query.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();
        let target_query = if case_sensitive {
            trimmed_query.to_string()
        } else {
            trimmed_query.to_lowercase()
        };

        // 1. Search scrollback buffer
        for (idx, line_cells) in self.scrollback.iter().enumerate() {
            let mut line_str = String::with_capacity(line_cells.len());
            for cell in line_cells {
                line_str.push(cell.c);
            }
            let trimmed_line = line_str.trim_end().to_string();
            let hay = if case_sensitive {
                trimmed_line.clone()
            } else {
                trimmed_line.to_lowercase()
            };

            let mut start_idx = 0;
            while let Some(pos) = hay[start_idx..].find(&target_query) {
                let col_start = start_idx + pos;
                let col_end = col_start + target_query.len();
                matches.push(SearchMatch {
                    buffer: SearchBuffer::Scrollback(idx),
                    col_start,
                    col_end,
                    line_text: trimmed_line.clone(),
                });
                start_idx = col_end;
            }
        }

        // 2. Search active screen rows
        for r in 0..self.rows {
            if let Some(line) = self.line_to_string(r) {
                if line.is_empty() {
                    continue;
                }
                let hay = if case_sensitive {
                    line.clone()
                } else {
                    line.to_lowercase()
                };

                let mut start_idx = 0;
                while let Some(pos) = hay[start_idx..].find(&target_query) {
                    let col_start = start_idx + pos;
                    let col_end = col_start + target_query.len();
                    matches.push(SearchMatch {
                        buffer: SearchBuffer::Screen(r),
                        col_start,
                        col_end,
                        line_text: line.clone(),
                    });
                    start_idx = col_end;
                }
            }
        }

        matches
    }

    /// Returns styled text spans for an active screen row, highlighting matching columns in bright yellow.
    pub fn row_spans_highlighted(
        &self,
        row: usize,
        highlight_cols: &[std::ops::Range<usize>],
    ) -> Option<Vec<StyledSpan>> {
        if row >= self.rows {
            return None;
        }
        let mut cells = Vec::with_capacity(self.cols);
        for c in 0..self.cols {
            let mut cell = self.get_cell(row, c).copied().unwrap_or_default();
            let is_highlighted = highlight_cols.iter().any(|range| range.contains(&c));
            if is_highlighted {
                cell.bg = crate::color::Color::Rgb(250, 204, 21); // yellow highlight
                cell.fg = crate::color::Color::Rgb(0, 0, 0); // black text
            }
            cells.push(cell);
        }
        Some(cells_to_spans(&cells))
    }

    /// Returns styled text spans for a scrollback row, highlighting matching columns in bright yellow.
    pub fn scrollback_row_spans_highlighted(
        &self,
        idx: usize,
        highlight_cols: &[std::ops::Range<usize>],
    ) -> Option<Vec<StyledSpan>> {
        self.scrollback.get(idx).map(|cells| {
            let mut modified = cells.clone();
            for (c, item) in modified.iter_mut().enumerate() {
                let is_highlighted = highlight_cols.iter().any(|range| range.contains(&c));
                if is_highlighted {
                    item.bg = crate::color::Color::Rgb(250, 204, 21);
                    item.fg = crate::color::Color::Rgb(0, 0, 0);
                }
            }
            cells_to_spans(&modified)
        })
    }
}

/// Identifies whether a match is located in scrollback history or on the active screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchBuffer {
    Scrollback(usize),
    Screen(usize),
}

/// A search match located in the terminal screen or scrollback buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    pub buffer: SearchBuffer,
    pub col_start: usize,
    pub col_end: usize,
    pub line_text: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::CellAttributes;
    use crate::color::{Color, NamedColor};

    #[test]
    fn row_spans_returns_spans_for_active_screen() {
        let mut grid = TerminalGrid::new(2, 10);
        grid.write_char_at_cursor(Cell::new(
            'A',
            Color::Named(NamedColor::Red),
            Color::Default,
            CellAttributes::default(),
        ));
        grid.write_char_at_cursor(Cell::new(
            'B',
            Color::Named(NamedColor::Red),
            Color::Default,
            CellAttributes::default(),
        ));

        let spans = grid.row_spans(0).unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "AB");
        assert_eq!(spans[0].fg, Color::Named(NamedColor::Red));

        assert!(grid.row_spans(1).unwrap().is_empty());
        assert!(grid.row_spans(5).is_none());
    }

    #[test]
    fn searches_active_screen_and_scrollback_correctly() {
        let mut grid = TerminalGrid::new(2, 20);
        // Write line 1
        for ch in "hello world".chars() {
            grid.write_char_at_cursor(Cell::new(
                ch,
                Color::Default,
                Color::Default,
                CellAttributes::default(),
            ));
        }
        grid.scroll_up(); // pushes "hello world" to scrollback

        // Write line 2 on active screen
        grid.cursor.row = 0;
        grid.cursor.col = 0;
        for ch in "HELLO AGAIN".chars() {
            grid.write_char_at_cursor(Cell::new(
                ch,
                Color::Default,
                Color::Default,
                CellAttributes::default(),
            ));
        }

        // Case-insensitive search finds both
        let matches = grid.search("hello", false);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].buffer, SearchBuffer::Scrollback(0));
        assert_eq!(matches[0].col_start, 0);
        assert_eq!(matches[0].col_end, 5);
        assert_eq!(matches[1].buffer, SearchBuffer::Screen(0));
        assert_eq!(matches[1].col_start, 0);
        assert_eq!(matches[1].col_end, 5);

        // Case-sensitive search finds only one
        let case_matches = grid.search("HELLO", true);
        assert_eq!(case_matches.len(), 1);
        assert_eq!(case_matches[0].buffer, SearchBuffer::Screen(0));

        // Highlight spans
        let ranges = [0..5, 10..15];
        let highlighted = grid.row_spans_highlighted(0, &ranges).unwrap();
        assert_eq!(highlighted[0].text, "HELLO");
        assert_eq!(highlighted[0].bg, Color::Rgb(250, 204, 21));
        assert_eq!(highlighted[0].fg, Color::Rgb(0, 0, 0));
    }
}

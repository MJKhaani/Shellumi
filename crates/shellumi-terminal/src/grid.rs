//! Terminal grid, screen buffer, reflow, and search management.

use crate::cell::{cells_to_spans_with_combining, Cell, CellAttributes, CellFlags, StyledSpan};
use crate::color::Color;
use crate::events::{HyperlinkTarget, TerminalModes};
use regex::RegexBuilder;
use std::collections::{HashMap, VecDeque};
use unicode_width::UnicodeWidthChar;

const DEFAULT_SCROLLBACK_CAPACITY: usize = 10_000;
const MAX_SEARCH_MATCHES: usize = 10_000;
const MAX_COMBINING_PER_CELL: usize = 8;

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

#[derive(Clone, Copy, Debug, Default)]
struct SavedCursorState {
    cursor: Cursor,
    fg: Color,
    bg: Color,
    attrs: CellAttributes,
    origin_mode: bool,
    autowrap: bool,
}

#[derive(Clone, Debug, Default)]
struct GridRow {
    cells: Vec<Cell>,
    combining: HashMap<usize, String>,
    wrapped: bool,
}

impl GridRow {
    fn new(cols: usize) -> Self {
        Self {
            cells: vec![Cell::default(); cols],
            combining: HashMap::new(),
            wrapped: false,
        }
    }

    fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
        self.combining.clear();
        self.wrapped = false;
    }

    fn to_trimmed_string(&self) -> String {
        let mut s = String::with_capacity(self.cells.len());
        for (idx, cell) in self.cells.iter().enumerate() {
            if cell.flags.wide_spacer {
                continue;
            }
            s.push(cell.c);
            if let Some(comb) = self.combining.get(&idx) {
                s.push_str(comb);
            }
        }
        s.trim_end().to_string()
    }
}

/// Search options for terminal buffer search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchOptions {
    pub query: String,
    pub case_sensitive: bool,
    pub regex: bool,
    pub max_matches: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            regex: false,
            max_matches: MAX_SEARCH_MATCHES,
        }
    }
}

/// The terminal grid containing active screen cells, alternate screen, modes, and scrollback history.
#[derive(Clone, Debug)]
pub struct TerminalGrid {
    rows: usize,
    cols: usize,
    primary_screen: Vec<GridRow>,
    alt_screen: Option<Vec<GridRow>>,
    is_alt_screen: bool,
    cursor: Cursor,
    pending_wrap: bool,
    saved_primary: SavedCursorState,
    saved_alt: SavedCursorState,
    scrollback: VecDeque<GridRow>,
    scrollback_limit: usize,
    scroll_top: usize,
    scroll_bottom: usize,
    tab_stops: Vec<bool>,
    modes: TerminalModes,
    dirty_rows: Vec<bool>,
    dirty_generation: u64,
    hyperlinks: HashMap<u32, HyperlinkTarget>,
    next_hyperlink_id: u32,
    active_hyperlink_id: u32,
}

impl TerminalGrid {
    pub fn new(rows: usize, cols: usize) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        Self {
            rows,
            cols,
            primary_screen: (0..rows).map(|_| GridRow::new(cols)).collect(),
            alt_screen: None,
            is_alt_screen: false,
            cursor: Cursor::default(),
            pending_wrap: false,
            saved_primary: SavedCursorState::default(),
            saved_alt: SavedCursorState::default(),
            scrollback: VecDeque::new(),
            scrollback_limit: DEFAULT_SCROLLBACK_CAPACITY,
            scroll_top: 0,
            scroll_bottom: rows - 1,
            tab_stops: default_tab_stops(cols),
            modes: TerminalModes::default(),
            dirty_rows: vec![true; rows],
            dirty_generation: 1,
            hyperlinks: HashMap::new(),
            next_hyperlink_id: 1,
            active_hyperlink_id: 0,
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
        self.pending_wrap = false;
        &mut self.cursor
    }

    pub fn pending_wrap(&self) -> bool {
        self.pending_wrap
    }

    pub fn clear_pending_wrap(&mut self) {
        self.pending_wrap = false;
    }

    pub fn modes(&self) -> &TerminalModes {
        &self.modes
    }

    pub fn modes_mut(&mut self) -> &mut TerminalModes {
        &mut self.modes
    }

    pub fn scroll_margins(&self) -> (usize, usize) {
        (self.scroll_top, self.scroll_bottom)
    }

    pub fn set_scroll_margins(&mut self, top: Option<usize>, bottom: Option<usize>) {
        let t = top.unwrap_or(1).saturating_sub(1);
        let b = bottom
            .map(|v| v.saturating_sub(1))
            .unwrap_or(self.rows - 1)
            .min(self.rows - 1);
        if t < b {
            self.scroll_top = t;
            self.scroll_bottom = b;
        } else {
            self.scroll_top = 0;
            self.scroll_bottom = self.rows - 1;
        }
        self.move_cursor_home();
    }

    pub fn move_cursor_home(&mut self) {
        self.pending_wrap = false;
        self.cursor.col = 0;
        self.cursor.row = if self.modes.origin_mode {
            self.scroll_top
        } else {
            0
        };
    }

    pub fn set_cursor_pos(&mut self, row_1based: usize, col_1based: usize) {
        self.pending_wrap = false;
        let max_col = self.cols.saturating_sub(1);
        let col = col_1based.saturating_sub(1).min(max_col);
        let row = if self.modes.origin_mode {
            let target = self.scroll_top + row_1based.saturating_sub(1);
            target.min(self.scroll_bottom)
        } else {
            row_1based
                .saturating_sub(1)
                .min(self.rows.saturating_sub(1))
        };
        self.cursor.row = row;
        self.cursor.col = col;
    }

    pub fn is_alt_screen(&self) -> bool {
        self.is_alt_screen
    }

    pub fn dirty_generation(&self) -> u64 {
        self.dirty_generation
    }

    pub fn is_row_dirty(&self, row: usize) -> bool {
        self.dirty_rows.get(row).copied().unwrap_or(false)
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_rows.fill(false);
    }

    fn mark_row_dirty(&mut self, row: usize) {
        if let Some(flag) = self.dirty_rows.get_mut(row) {
            *flag = true;
        }
        self.dirty_generation = self.dirty_generation.wrapping_add(1);
    }

    fn mark_all_dirty(&mut self) {
        self.dirty_rows.fill(true);
        self.dirty_generation = self.dirty_generation.wrapping_add(1);
    }

    /// Sets or clears the active OSC 8 hyperlink for newly written cells.
    pub fn set_active_hyperlink(&mut self, target: Option<HyperlinkTarget>) {
        match target {
            Some(t) => {
                let id = self.next_hyperlink_id;
                self.next_hyperlink_id = self.next_hyperlink_id.wrapping_add(1).max(1);
                if self.hyperlinks.len() >= 4096 {
                    // Evict oldest entries if pathological stream creates thousands of links
                    self.hyperlinks.clear();
                }
                self.hyperlinks.insert(id, t);
                self.active_hyperlink_id = id;
            }
            None => {
                self.active_hyperlink_id = 0;
            }
        }
    }

    pub fn active_hyperlink_id(&self) -> u32 {
        self.active_hyperlink_id
    }

    pub fn hyperlink_at(&self, row: usize, col: usize) -> Option<&HyperlinkTarget> {
        let cell = self.get_cell(row, col)?;
        if cell.hyperlink_id == 0 {
            None
        } else {
            self.hyperlinks.get(&cell.hyperlink_id)
        }
    }

    /// Updates the maximum scrollback capacity and trims excess history if necessary.
    pub fn set_scrollback_limit(&mut self, limit: usize) {
        self.scrollback_limit = limit;
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.pop_front();
        }
    }

    pub fn scrollback_limit(&self) -> usize {
        self.scrollback_limit
    }

    fn active_rows(&self) -> &[GridRow] {
        if self.is_alt_screen {
            self.alt_screen.as_deref().unwrap_or(&self.primary_screen)
        } else {
            &self.primary_screen
        }
    }

    fn active_rows_mut(&mut self) -> &mut Vec<GridRow> {
        if self.is_alt_screen {
            self.alt_screen.as_mut().unwrap_or(&mut self.primary_screen)
        } else {
            &mut self.primary_screen
        }
    }

    pub fn get_cell(&self, row: usize, col: usize) -> Option<&Cell> {
        self.active_rows().get(row)?.cells.get(col)
    }

    pub fn get_cell_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        self.mark_row_dirty(row);
        self.active_rows_mut().get_mut(row)?.cells.get_mut(col)
    }

    pub fn set_cell(&mut self, row: usize, col: usize, cell: Cell) {
        if let Some(target_row) = self.active_rows_mut().get_mut(row) {
            if let Some(target) = target_row.cells.get_mut(col) {
                *target = cell;
                target_row.combining.remove(&col);
            }
        }
        self.mark_row_dirty(row);
    }

    fn clear_wide_pair_at(&mut self, row: usize, col: usize) {
        let Some(r) = self.active_rows_mut().get_mut(row) else {
            return;
        };
        if col >= r.cells.len() {
            return;
        }
        if r.cells[col].flags.wide && col + 1 < r.cells.len() {
            r.cells[col + 1] = Cell::default();
            r.combining.remove(&(col + 1));
        } else if r.cells[col].flags.wide_spacer && col > 0 {
            r.cells[col - 1] = Cell::default();
            r.combining.remove(&(col - 1));
        }
    }

    pub fn write_char_at_cursor(&mut self, mut cell: Cell) {
        let width = UnicodeWidthChar::width(cell.c).unwrap_or(0);
        cell.hyperlink_id = self.active_hyperlink_id;

        if width == 0 {
            // Combining mark: attach to previous cell
            let (target_row, mut target_col) = if self.pending_wrap {
                (self.cursor.row, self.cols.saturating_sub(1))
            } else if self.cursor.col > 0 {
                (self.cursor.row, self.cursor.col - 1)
            } else {
                return;
            };
            if let Some(r) = self.active_rows_mut().get_mut(target_row) {
                if r.cells.get(target_col).is_some_and(|c| c.flags.wide_spacer) && target_col > 0 {
                    target_col -= 1;
                }
                let entry = r.combining.entry(target_col).or_default();
                if entry.chars().count() < MAX_COMBINING_PER_CELL {
                    entry.push(cell.c);
                }
            }
            self.mark_row_dirty(target_row);
            return;
        }

        if self.pending_wrap {
            if self.modes.autowrap {
                let cur_row = self.cursor.row;
                if let Some(r) = self.active_rows_mut().get_mut(cur_row) {
                    r.wrapped = true;
                }
                self.line_feed();
                self.cursor.col = 0;
            } else {
                self.cursor.col = self.cols.saturating_sub(1);
            }
            self.pending_wrap = false;
        }

        if width == 2 && self.cursor.col + 1 >= self.cols {
            if self.cols < 2 {
                return;
            }
            if self.modes.autowrap {
                let r = self.cursor.row;
                self.clear_wide_pair_at(r, self.cursor.col);
                self.set_cell(r, self.cursor.col, Cell::default());
                if let Some(row_obj) = self.active_rows_mut().get_mut(r) {
                    row_obj.wrapped = true;
                }
                self.line_feed();
                self.cursor.col = 0;
            } else {
                return;
            }
        }

        let r = self.cursor.row;
        let c = self.cursor.col;
        if r >= self.rows || c >= self.cols {
            return;
        }

        if self.modes.insert_mode {
            self.insert_chars(width);
        }

        self.clear_wide_pair_at(r, c);
        if width == 2 && c + 1 < self.cols {
            self.clear_wide_pair_at(r, c + 1);
            cell.flags = CellFlags {
                wide: true,
                wide_spacer: false,
            };
            self.set_cell(r, c, cell);
            let spacer = Cell {
                c: ' ',
                fg: cell.fg,
                bg: cell.bg,
                attrs: cell.attrs,
                flags: CellFlags {
                    wide: false,
                    wide_spacer: true,
                },
                hyperlink_id: cell.hyperlink_id,
            };
            self.set_cell(r, c + 1, spacer);
        } else {
            cell.flags = CellFlags::default();
            self.set_cell(r, c, cell);
        }

        if c + width >= self.cols {
            self.cursor.col = self.cols - 1;
            self.pending_wrap = true;
        } else {
            self.cursor.col = c + width;
        }
    }

    pub fn line_feed(&mut self) {
        self.pending_wrap = false;
        if self.cursor.row == self.scroll_bottom {
            self.scroll_up_in_region(1);
        } else if self.cursor.row + 1 < self.rows {
            self.cursor.row += 1;
        }
    }

    pub fn reverse_index(&mut self) {
        self.pending_wrap = false;
        if self.cursor.row == self.scroll_top {
            self.scroll_down_in_region(1);
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
        }
    }

    pub fn set_tab_stop(&mut self) {
        if let Some(stop) = self.tab_stops.get_mut(self.cursor.col) {
            *stop = true;
        }
    }

    pub fn clear_tab_stop(&mut self, mode: u16) {
        match mode {
            0 => {
                if let Some(stop) = self.tab_stops.get_mut(self.cursor.col) {
                    *stop = false;
                }
            }
            3 => {
                self.tab_stops.fill(false);
            }
            _ => {}
        }
    }

    pub fn tab_forward(&mut self, count: usize) {
        self.pending_wrap = false;
        let max_col = self.cols.saturating_sub(1);
        let mut col = self.cursor.col;
        for _ in 0..count.max(1) {
            let mut next = col + 1;
            while next < self.cols && !self.tab_stops.get(next).copied().unwrap_or(false) {
                next += 1;
            }
            col = next.min(max_col);
        }
        self.cursor.col = col;
    }

    pub fn tab_backward(&mut self, count: usize) {
        self.pending_wrap = false;
        let mut col = self.cursor.col;
        for _ in 0..count.max(1) {
            if col == 0 {
                break;
            }
            let mut prev = col - 1;
            while prev > 0 && !self.tab_stops.get(prev).copied().unwrap_or(false) {
                prev -= 1;
            }
            col = prev;
        }
        self.cursor.col = col;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_up_in_region(1);
    }

    pub fn scroll_up_in_region(&mut self, count: usize) {
        let top = self.scroll_top.min(self.rows.saturating_sub(1));
        let bottom = self.scroll_bottom.min(self.rows.saturating_sub(1));
        if top > bottom {
            return;
        }
        let cols = self.cols;
        let is_full_primary = !self.is_alt_screen && top == 0 && bottom + 1 == self.rows;
        for _ in 0..count.max(1) {
            let removed = self.active_rows_mut().remove(top);
            if is_full_primary && self.scrollback_limit > 0 {
                if self.scrollback.len() >= self.scrollback_limit {
                    self.scrollback.pop_front();
                }
                self.scrollback.push_back(removed);
            }
            self.active_rows_mut().insert(bottom, GridRow::new(cols));
        }
        for r in top..=bottom {
            self.mark_row_dirty(r);
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll_down_in_region(1);
    }

    pub fn scroll_down_in_region(&mut self, count: usize) {
        let top = self.scroll_top.min(self.rows.saturating_sub(1));
        let bottom = self.scroll_bottom.min(self.rows.saturating_sub(1));
        if top > bottom {
            return;
        }
        let cols = self.cols;
        for _ in 0..count.max(1) {
            self.active_rows_mut().remove(bottom);
            self.active_rows_mut().insert(top, GridRow::new(cols));
        }
        for r in top..=bottom {
            self.mark_row_dirty(r);
        }
    }

    pub fn insert_lines(&mut self, count: usize) {
        let r = self.cursor.row;
        if r < self.scroll_top || r > self.scroll_bottom {
            return;
        }
        let old_top = self.scroll_top;
        self.scroll_top = r;
        self.scroll_down_in_region(count);
        self.scroll_top = old_top;
        self.cursor.col = 0;
        self.pending_wrap = false;
    }

    pub fn delete_lines(&mut self, count: usize) {
        let r = self.cursor.row;
        if r < self.scroll_top || r > self.scroll_bottom {
            return;
        }
        let old_top = self.scroll_top;
        self.scroll_top = r;
        let cols = self.cols;
        let bottom = self.scroll_bottom.min(self.rows.saturating_sub(1));
        for _ in 0..count.max(1) {
            if r <= bottom {
                self.active_rows_mut().remove(r);
                self.active_rows_mut().insert(bottom, GridRow::new(cols));
            }
        }
        self.scroll_top = old_top;
        for row in r..=bottom {
            self.mark_row_dirty(row);
        }
        self.cursor.col = 0;
        self.pending_wrap = false;
    }

    pub fn insert_chars(&mut self, count: usize) {
        let r = self.cursor.row;
        let c = self.cursor.col;
        let cols = self.cols;
        if r >= self.rows || c >= cols {
            return;
        }
        self.clear_wide_pair_at(r, c);
        let n = count.max(1).min(cols - c);
        if let Some(row_obj) = self.active_rows_mut().get_mut(r) {
            for _ in 0..n {
                row_obj.cells.pop();
                row_obj.cells.insert(c, Cell::default());
            }
            row_obj.combining.retain(|&col_idx, _| col_idx < c);
        }
        self.mark_row_dirty(r);
    }

    pub fn delete_chars(&mut self, count: usize) {
        let r = self.cursor.row;
        let c = self.cursor.col;
        let cols = self.cols;
        if r >= self.rows || c >= cols {
            return;
        }
        self.clear_wide_pair_at(r, c);
        let n = count.max(1).min(cols - c);
        if let Some(row_obj) = self.active_rows_mut().get_mut(r) {
            for _ in 0..n {
                row_obj.cells.remove(c);
                row_obj.cells.push(Cell::default());
            }
            row_obj.combining.retain(|&col_idx, _| col_idx < c);
        }
        self.mark_row_dirty(r);
    }

    pub fn erase_chars(&mut self, count: usize) {
        let r = self.cursor.row;
        let c = self.cursor.col;
        let end = (c + count.max(1)).min(self.cols);
        for col in c..end {
            self.clear_wide_pair_at(r, col);
            self.set_cell(r, col, Cell::default());
        }
    }

    pub fn clear_all(&mut self) {
        for r in self.active_rows_mut().iter_mut() {
            r.clear();
        }
        self.mark_all_dirty();
    }

    pub fn clear_scrollback(&mut self) {
        self.scrollback.clear();
    }

    pub fn clear_from_cursor_down(&mut self) {
        let cur_r = self.cursor.row;
        let cur_c = self.cursor.col;
        for c in cur_c..self.cols {
            self.clear_wide_pair_at(cur_r, c);
            self.set_cell(cur_r, c, Cell::default());
        }
        for r in (cur_r + 1)..self.rows {
            if let Some(row_obj) = self.active_rows_mut().get_mut(r) {
                row_obj.clear();
            }
            self.mark_row_dirty(r);
        }
    }

    pub fn clear_from_cursor_up(&mut self) {
        let cur_r = self.cursor.row;
        let cur_c = self.cursor.col;
        for r in 0..cur_r {
            if let Some(row_obj) = self.active_rows_mut().get_mut(r) {
                row_obj.clear();
            }
            self.mark_row_dirty(r);
        }
        for c in 0..=cur_c.min(self.cols - 1) {
            self.clear_wide_pair_at(cur_r, c);
            self.set_cell(cur_r, c, Cell::default());
        }
    }

    pub fn clear_line(&mut self, mode: u8) {
        let r = self.cursor.row;
        match mode {
            0 => {
                for c in self.cursor.col..self.cols {
                    self.clear_wide_pair_at(r, c);
                    self.set_cell(r, c, Cell::default());
                }
            }
            1 => {
                for c in 0..=self.cursor.col.min(self.cols - 1) {
                    self.clear_wide_pair_at(r, c);
                    self.set_cell(r, c, Cell::default());
                }
            }
            2 => {
                if let Some(row_obj) = self.active_rows_mut().get_mut(r) {
                    row_obj.clear();
                }
                self.mark_row_dirty(r);
            }
            _ => {}
        }
    }

    pub fn enter_alt_screen(&mut self) {
        self.enter_alt_screen_with_options(true, true);
    }

    pub fn enter_alt_screen_with_options(&mut self, save_cursor: bool, clear: bool) {
        if save_cursor {
            self.saved_primary.cursor = self.cursor;
        }
        if !self.is_alt_screen {
            let cols = self.cols;
            let rows = self.rows;
            if clear || self.alt_screen.is_none() {
                self.alt_screen = Some((0..rows).map(|_| GridRow::new(cols)).collect());
            }
            self.is_alt_screen = true;
            if clear {
                self.cursor = Cursor {
                    row: 0,
                    col: 0,
                    visible: self.cursor.visible,
                };
            }
            self.pending_wrap = false;
            self.scroll_top = 0;
            self.scroll_bottom = rows.saturating_sub(1);
            self.mark_all_dirty();
        }
    }

    pub fn exit_alt_screen(&mut self) {
        self.exit_alt_screen_with_options(true);
    }

    pub fn exit_alt_screen_with_options(&mut self, restore_cursor: bool) {
        if self.is_alt_screen {
            self.alt_screen = None;
            self.is_alt_screen = false;
            if restore_cursor {
                self.cursor = self.saved_primary.cursor;
                self.cursor.row = self.cursor.row.min(self.rows.saturating_sub(1));
                self.cursor.col = self.cursor.col.min(self.cols.saturating_sub(1));
            }
            self.pending_wrap = false;
            self.scroll_top = 0;
            self.scroll_bottom = self.rows.saturating_sub(1);
            self.mark_all_dirty();
        }
    }

    pub fn save_cursor(&mut self) {
        let state = SavedCursorState {
            cursor: self.cursor,
            fg: Color::Default,
            bg: Color::Default,
            attrs: CellAttributes::default(),
            origin_mode: self.modes.origin_mode,
            autowrap: self.modes.autowrap,
        };
        if self.is_alt_screen {
            self.saved_alt = state;
        } else {
            self.saved_primary = state;
        }
    }

    pub fn save_cursor_with_sgr(&mut self, fg: Color, bg: Color, attrs: CellAttributes) {
        let state = SavedCursorState {
            cursor: self.cursor,
            fg,
            bg,
            attrs,
            origin_mode: self.modes.origin_mode,
            autowrap: self.modes.autowrap,
        };
        if self.is_alt_screen {
            self.saved_alt = state;
        } else {
            self.saved_primary = state;
        }
    }

    pub fn restore_cursor(&mut self) {
        let _ = self.restore_cursor_with_sgr();
    }

    pub fn restore_cursor_with_sgr(&mut self) -> (Color, Color, CellAttributes) {
        let saved = if self.is_alt_screen {
            self.saved_alt
        } else {
            self.saved_primary
        };
        self.cursor = saved.cursor;
        self.cursor.row = self.cursor.row.min(self.rows.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(self.cols.saturating_sub(1));
        self.modes.origin_mode = saved.origin_mode;
        self.modes.autowrap = saved.autowrap;
        self.pending_wrap = false;
        (saved.fg, saved.bg, saved.attrs)
    }

    pub fn reset(&mut self, hard: bool) {
        self.modes = TerminalModes::default();
        self.cursor = Cursor::default();
        self.pending_wrap = false;
        self.scroll_top = 0;
        self.scroll_bottom = self.rows.saturating_sub(1);
        self.tab_stops = default_tab_stops(self.cols);
        self.active_hyperlink_id = 0;
        if hard {
            self.is_alt_screen = false;
            self.alt_screen = None;
            self.clear_all();
            self.scrollback.clear();
            self.hyperlinks.clear();
        }
        self.mark_all_dirty();
    }

    pub fn resize(&mut self, new_rows: usize, new_cols: usize) {
        let new_rows = new_rows.max(1);
        let new_cols = new_cols.max(1);
        if new_rows == self.rows && new_cols == self.cols {
            return;
        }

        // Reflow primary screen + scrollback into logical lines when columns change
        // or rows shrink/grow, preserving wrapped line continuity.
        let mut all_rows: Vec<GridRow> = self.scrollback.drain(..).collect();
        let primary_start_idx = all_rows.len();
        all_rows.append(&mut self.primary_screen);

        let cursor_abs_row = primary_start_idx + self.cursor.row;
        let cursor_col = self.cursor.col;

        // Group physical rows into logical lines
        struct LogicalLine {
            cells: Vec<(Cell, Option<String>)>,
            cursor_offset: Option<usize>,
        }

        let mut logical_lines: Vec<LogicalLine> = Vec::new();
        let mut current_cells: Vec<(Cell, Option<String>)> = Vec::new();
        let mut current_cursor: Option<usize> = None;

        for (r_idx, row) in all_rows.into_iter().enumerate() {
            let is_cursor_row = !self.is_alt_screen && r_idx == cursor_abs_row;
            if is_cursor_row {
                current_cursor = Some(current_cells.len() + cursor_col);
            }
            for (c_idx, cell) in row.cells.into_iter().enumerate() {
                let comb = row.combining.get(&c_idx).cloned();
                current_cells.push((cell, comb));
            }
            if !row.wrapped {
                // Trim trailing default spaces unless cursor is past content
                let keep_len = current_cells
                    .iter()
                    .rposition(|(cell, _)| {
                        cell.c != ' '
                            || cell.bg != Color::Default
                            || cell.flags.wide
                            || cell.flags.wide_spacer
                            || cell.attrs.inverse
                            || cell.attrs.underline
                    })
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let limit = current_cursor
                    .map(|c| (c + 1).max(keep_len))
                    .unwrap_or(keep_len)
                    .min(current_cells.len());
                current_cells.truncate(limit);
                logical_lines.push(LogicalLine {
                    cells: std::mem::take(&mut current_cells),
                    cursor_offset: current_cursor.take(),
                });
            }
        }
        if !current_cells.is_empty() || current_cursor.is_some() {
            logical_lines.push(LogicalLine {
                cells: current_cells,
                cursor_offset: current_cursor,
            });
        }

        // Re-wrap logical lines to new_cols
        let mut reflowed_rows: Vec<GridRow> = Vec::new();
        let mut new_cursor_abs_row = 0usize;
        let mut new_cursor_col = 0usize;
        let mut found_cursor = false;

        for line in logical_lines {
            if line.cells.is_empty() {
                if let Some(off) = line.cursor_offset {
                    new_cursor_abs_row = reflowed_rows.len();
                    new_cursor_col = off.min(new_cols - 1);
                    found_cursor = true;
                }
                reflowed_rows.push(GridRow::new(new_cols));
                continue;
            }

            let mut row = GridRow::new(new_cols);
            let mut col = 0usize;
            let mut idx = 0usize;
            while idx < line.cells.len() {
                let (cell, comb) = &line.cells[idx];
                if cell.flags.wide_spacer {
                    idx += 1;
                    continue;
                }
                let w = if cell.flags.wide { 2 } else { 1 };
                if w == 2 && col + 1 >= new_cols && new_cols >= 2 {
                    row.wrapped = true;
                    reflowed_rows.push(row);
                    row = GridRow::new(new_cols);
                    col = 0;
                }
                if line.cursor_offset == Some(idx) {
                    new_cursor_abs_row = reflowed_rows.len();
                    new_cursor_col = col.min(new_cols - 1);
                    found_cursor = true;
                }
                if col < new_cols {
                    row.cells[col] = *cell;
                    if let Some(c_str) = comb {
                        row.combining.insert(col, c_str.clone());
                    }
                    if w == 2 && col + 1 < new_cols {
                        row.cells[col + 1] = Cell {
                            c: ' ',
                            fg: cell.fg,
                            bg: cell.bg,
                            attrs: cell.attrs,
                            flags: CellFlags {
                                wide: false,
                                wide_spacer: true,
                            },
                            hyperlink_id: cell.hyperlink_id,
                        };
                    }
                }
                col += w;
                if col >= new_cols && idx + 1 < line.cells.len() {
                    row.wrapped = true;
                    reflowed_rows.push(row);
                    row = GridRow::new(new_cols);
                    col = 0;
                }
                idx += 1;
            }
            if !found_cursor {
                if let Some(off) = line.cursor_offset {
                    if off >= line.cells.len() {
                        new_cursor_abs_row = reflowed_rows.len();
                        new_cursor_col = col.min(new_cols - 1);
                        found_cursor = true;
                    }
                }
            }
            reflowed_rows.push(row);
        }

        // Remove trailing blank rows beyond new_rows unless cursor is there
        while reflowed_rows.len() > new_rows {
            let last_idx = reflowed_rows.len() - 1;
            if found_cursor && new_cursor_abs_row >= last_idx {
                break;
            }
            let is_blank = reflowed_rows[last_idx].cells.iter().all(|c| {
                c.c == ' ' && c.bg == Color::Default && !c.attrs.inverse && !c.attrs.underline
            });
            if is_blank {
                reflowed_rows.pop();
            } else {
                break;
            }
        }

        while reflowed_rows.len() < new_rows {
            reflowed_rows.push(GridRow::new(new_cols));
        }

        let split_point = reflowed_rows.len().saturating_sub(new_rows);
        for row in reflowed_rows.drain(..split_point) {
            if self.scrollback.len() >= self.scrollback_limit && self.scrollback_limit > 0 {
                self.scrollback.pop_front();
            }
            if self.scrollback_limit > 0 {
                self.scrollback.push_back(row);
            }
        }
        self.primary_screen = reflowed_rows;

        // Resize alternate screen independently (no reflow of primary scrollback)
        if let Some(ref mut alt) = self.alt_screen {
            let mut new_alt: Vec<GridRow> = (0..new_rows).map(|_| GridRow::new(new_cols)).collect();
            let copy_rows = alt.len().min(new_rows);
            for r in 0..copy_rows {
                let copy_cols = alt[r].cells.len().min(new_cols);
                new_alt[r].cells[..copy_cols].copy_from_slice(&alt[r].cells[..copy_cols]);
                for (c, comb) in &alt[r].combining {
                    if *c < copy_cols {
                        new_alt[r].combining.insert(*c, comb.clone());
                    }
                }
            }
            *alt = new_alt;
        }

        self.rows = new_rows;
        self.cols = new_cols;
        self.scroll_top = 0;
        self.scroll_bottom = new_rows - 1;
        self.tab_stops = default_tab_stops(new_cols);
        self.dirty_rows = vec![true; new_rows];
        self.dirty_generation = self.dirty_generation.wrapping_add(1);
        self.pending_wrap = false;

        if !self.is_alt_screen && found_cursor {
            self.cursor.row = new_cursor_abs_row
                .saturating_sub(split_point)
                .min(new_rows - 1);
            self.cursor.col = new_cursor_col.min(new_cols - 1);
        } else {
            self.cursor.row = self.cursor.row.min(new_rows - 1);
            self.cursor.col = self.cursor.col.min(new_cols - 1);
        }

        self.saved_primary.cursor.row = self.saved_primary.cursor.row.min(new_rows - 1);
        self.saved_primary.cursor.col = self.saved_primary.cursor.col.min(new_cols - 1);
        self.saved_alt.cursor.row = self.saved_alt.cursor.row.min(new_rows - 1);
        self.saved_alt.cursor.col = self.saved_alt.cursor.col.min(new_cols - 1);
    }

    pub fn line_to_string(&self, row: usize) -> Option<String> {
        self.active_rows().get(row).map(|r| r.to_trimmed_string())
    }

    pub fn screen_to_string(&self) -> String {
        let mut lines = Vec::with_capacity(self.rows);
        for r in 0..self.rows {
            lines.push(self.line_to_string(r).unwrap_or_default());
        }
        while lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }

    pub fn row_spans(&self, row: usize) -> Option<Vec<StyledSpan>> {
        self.active_rows()
            .get(row)
            .map(|r| cells_to_spans_with_combining(&r.cells, Some(&r.combining)))
    }

    pub fn row_text(&self, row: usize) -> Option<String> {
        self.row_spans(row).map(|spans| {
            let mut s = String::new();
            for span in spans {
                s.push_str(&span.text);
            }
            s.trim_end().to_string()
        })
    }

    pub fn is_row_wrapped(&self, row: usize) -> bool {
        self.active_rows()
            .get(row)
            .map(|r| r.wrapped)
            .unwrap_or(false)
    }

    pub fn is_scrollback_row_wrapped(&self, idx: usize) -> bool {
        self.scrollback.get(idx).map(|r| r.wrapped).unwrap_or(false)
    }

    pub fn scrollback_count(&self) -> usize {
        self.scrollback.len()
    }

    pub fn scrollback_row_spans(&self, idx: usize) -> Option<Vec<StyledSpan>> {
        self.scrollback
            .get(idx)
            .map(|r| cells_to_spans_with_combining(&r.cells, Some(&r.combining)))
    }

    /// Search for text occurrences across the scrollback history and active screen.
    pub fn search(&self, query: &str, case_sensitive: bool) -> Vec<SearchMatch> {
        self.search_with_options(&SearchOptions {
            query: query.to_string(),
            case_sensitive,
            regex: false,
            max_matches: MAX_SEARCH_MATCHES,
        })
        .unwrap_or_default()
    }

    /// Search with literal or regex options and bounded match count.
    pub fn search_with_options(&self, opts: &SearchOptions) -> Result<Vec<SearchMatch>, String> {
        let trimmed_query = opts.query.trim();
        if trimmed_query.is_empty() {
            return Ok(Vec::new());
        }

        let max_matches = opts.max_matches.clamp(1, MAX_SEARCH_MATCHES);
        let regex_matcher = if opts.regex {
            let re = RegexBuilder::new(trimmed_query)
                .case_insensitive(!opts.case_sensitive)
                .size_limit(1 << 20)
                .build()
                .map_err(|e| format!("Invalid regex: {e}"))?;
            Some(re)
        } else {
            None
        };

        let mut matches = Vec::new();
        let target_query = if opts.case_sensitive {
            trimmed_query.to_string()
        } else {
            trimmed_query.to_lowercase()
        };

        let search_row = |buffer: SearchBuffer, row: &GridRow, matches: &mut Vec<SearchMatch>| {
            if matches.len() >= max_matches {
                return;
            }
            let line = row.to_trimmed_string();
            if line.is_empty() {
                return;
            }
            if let Some(ref re) = regex_matcher {
                for m in re.find_iter(&line) {
                    if matches.len() >= max_matches {
                        break;
                    }
                    let col_start = line[..m.start()].chars().count();
                    let col_end = line[..m.end()].chars().count();
                    matches.push(SearchMatch {
                        buffer,
                        col_start,
                        col_end: col_end.max(col_start + 1),
                        line_text: line.clone(),
                    });
                }
            } else {
                let hay = if opts.case_sensitive {
                    line.clone()
                } else {
                    line.to_lowercase()
                };
                let mut byte_idx = 0;
                while let Some(pos) = hay[byte_idx..].find(&target_query) {
                    if matches.len() >= max_matches {
                        break;
                    }
                    let start_byte = byte_idx + pos;
                    let end_byte = start_byte + target_query.len();
                    let col_start = hay[..start_byte].chars().count();
                    let col_end = hay[..end_byte].chars().count();
                    matches.push(SearchMatch {
                        buffer,
                        col_start,
                        col_end: col_end.max(col_start + 1),
                        line_text: line.clone(),
                    });
                    byte_idx = end_byte;
                }
            }
        };

        for (idx, row) in self.scrollback.iter().enumerate() {
            search_row(SearchBuffer::Scrollback(idx), row, &mut matches);
        }
        for (r, row) in self.active_rows().iter().enumerate() {
            search_row(SearchBuffer::Screen(r), row, &mut matches);
        }

        Ok(matches)
    }

    pub fn row_spans_highlighted(
        &self,
        row: usize,
        highlight_cols: &[std::ops::Range<usize>],
    ) -> Option<Vec<StyledSpan>> {
        self.active_rows()
            .get(row)
            .map(|r| highlight_cells(&r.cells, Some(&r.combining), highlight_cols))
    }

    pub fn scrollback_row_spans_highlighted(
        &self,
        idx: usize,
        highlight_cols: &[std::ops::Range<usize>],
    ) -> Option<Vec<StyledSpan>> {
        self.scrollback
            .get(idx)
            .map(|r| highlight_cells(&r.cells, Some(&r.combining), highlight_cols))
    }
}

fn default_tab_stops(cols: usize) -> Vec<bool> {
    (0..cols).map(|c| c > 0 && c % 8 == 0).collect()
}

/// Identifies which buffer a search match resides in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchBuffer {
    Scrollback(usize),
    Screen(usize),
}

/// A search match within the terminal grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    pub buffer: SearchBuffer,
    pub col_start: usize,
    pub col_end: usize,
    pub line_text: String,
}

fn highlight_cells(
    cells: &[Cell],
    combining: Option<&HashMap<usize, String>>,
    highlight_cols: &[std::ops::Range<usize>],
) -> Vec<StyledSpan> {
    let mut modified_cells = cells.to_vec();
    for (col_idx, cell) in modified_cells.iter_mut().enumerate() {
        if highlight_cols.iter().any(|r| r.contains(&col_idx)) {
            cell.fg = Color::Named(crate::color::NamedColor::Black);
            cell.bg = Color::Named(crate::color::NamedColor::BrightYellow);
            cell.attrs.inverse = false;
        }
    }
    cells_to_spans_with_combining(&modified_cells, combining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_spans_returns_spans_for_active_screen() {
        let mut grid = TerminalGrid::new(5, 20);
        for (i, ch) in "Hello".chars().enumerate() {
            grid.set_cell(
                0,
                i,
                Cell::new(ch, Color::Default, Color::Default, Default::default()),
            );
        }
        let spans = grid.row_spans(0).unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "Hello");
    }

    #[test]
    fn searches_active_screen_and_scrollback_correctly() {
        let mut grid = TerminalGrid::new(3, 20);
        for (i, ch) in "alpha".chars().enumerate() {
            grid.set_cell(
                0,
                i,
                Cell::new(ch, Color::Default, Color::Default, Default::default()),
            );
        }
        grid.scroll_up();
        for (i, ch) in "Alpha beta".chars().enumerate() {
            grid.set_cell(
                2,
                i,
                Cell::new(ch, Color::Default, Color::Default, Default::default()),
            );
        }
        let insensitive = grid.search("alpha", false);
        assert_eq!(insensitive.len(), 2);
        let sensitive = grid.search("Alpha", true);
        assert_eq!(sensitive.len(), 1);
        assert_eq!(sensitive[0].buffer, SearchBuffer::Screen(2));
    }

    #[test]
    fn regex_search_handles_invalid_patterns_safely() {
        let grid = TerminalGrid::new(3, 20);
        let err = grid.search_with_options(&SearchOptions {
            query: "[unclosed".into(),
            case_sensitive: false,
            regex: true,
            max_matches: 100,
        });
        assert!(err.is_err());
    }
}

//! VT/ANSI escape sequence processing updating the terminal grid.

use crate::cell::{Cell, CellAttributes};
use crate::color::{Color, NamedColor};
use crate::grid::TerminalGrid;
use crate::shell_integration::ShellIntegrationState;
use vte::{Params, Perform};

/// Handles VTE parser actions and applies them to the terminal grid.
pub struct TerminalHandler<'a> {
    pub grid: &'a mut TerminalGrid,
    pub title: &'a mut Option<String>,
    pub shell_integration: Option<&'a mut ShellIntegrationState>,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttributes,
}

impl<'a> TerminalHandler<'a> {
    pub fn new(grid: &'a mut TerminalGrid, title: &'a mut Option<String>) -> Self {
        Self {
            grid,
            title,
            shell_integration: None,
            fg: Color::Default,
            bg: Color::Default,
            attrs: CellAttributes::default(),
        }
    }

    pub fn with_shell_integration(mut self, integration: &'a mut ShellIntegrationState) -> Self {
        self.shell_integration = Some(integration);
        self
    }

    fn current_cell(&self, c: char) -> Cell {
        Cell::new(c, self.fg, self.bg, self.attrs)
    }

    fn parse_sgr(&mut self, params: &Params) {
        let mut iter = params.iter();
        if params.is_empty() {
            self.reset_sgr();
            return;
        }

        while let Some(param) = iter.next() {
            let code = param.first().copied().unwrap_or(0);
            match code {
                0 => self.reset_sgr(),
                1 => self.attrs.bold = true,
                2 => self.attrs.dim = true,
                3 => self.attrs.italic = true,
                4 => self.attrs.underline = true,
                7 => self.attrs.inverse = true,
                8 => self.attrs.hidden = true,
                9 => self.attrs.strikethrough = true,
                22 => {
                    self.attrs.bold = false;
                    self.attrs.dim = false;
                }
                23 => self.attrs.italic = false,
                24 => self.attrs.underline = false,
                27 => self.attrs.inverse = false,
                28 => self.attrs.hidden = false,
                29 => self.attrs.strikethrough = false,
                30..=37 => self.fg = Color::Named(ansi_named_color(code - 30, false)),
                38 => {
                    if let Some(color) = parse_extended_color(&mut iter) {
                        self.fg = color;
                    }
                }
                39 => self.fg = Color::Default,
                40..=47 => self.bg = Color::Named(ansi_named_color(code - 40, false)),
                48 => {
                    if let Some(color) = parse_extended_color(&mut iter) {
                        self.bg = color;
                    }
                }
                49 => self.bg = Color::Default,
                90..=97 => self.fg = Color::Named(ansi_named_color(code - 90, true)),
                100..=107 => self.bg = Color::Named(ansi_named_color(code - 100, true)),
                _ => {}
            }
        }
    }

    fn reset_sgr(&mut self) {
        self.fg = Color::Default;
        self.bg = Color::Default;
        self.attrs = CellAttributes::default();
    }
}

fn ansi_named_color(offset: u16, bright: bool) -> NamedColor {
    if bright {
        match offset {
            0 => NamedColor::BrightBlack,
            1 => NamedColor::BrightRed,
            2 => NamedColor::BrightGreen,
            3 => NamedColor::BrightYellow,
            4 => NamedColor::BrightBlue,
            5 => NamedColor::BrightMagenta,
            6 => NamedColor::BrightCyan,
            _ => NamedColor::BrightWhite,
        }
    } else {
        match offset {
            0 => NamedColor::Black,
            1 => NamedColor::Red,
            2 => NamedColor::Green,
            3 => NamedColor::Yellow,
            4 => NamedColor::Blue,
            5 => NamedColor::Magenta,
            6 => NamedColor::Cyan,
            _ => NamedColor::White,
        }
    }
}

fn parse_extended_color<'b, I>(iter: &mut I) -> Option<Color>
where
    I: Iterator<Item = &'b [u16]>,
{
    match iter.next()?.first().copied()? {
        5 => {
            // 256-color palette
            let index = *iter.next()?.first()? as u8;
            Some(Color::Indexed(index))
        }
        2 => {
            // 24-bit TrueColor RGB
            let r = *iter.next()?.first()? as u8;
            let g = *iter.next()?.first()? as u8;
            let b = *iter.next()?.first()? as u8;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

impl<'a> Perform for TerminalHandler<'a> {
    fn print(&mut self, c: char) {
        let cell = self.current_cell(c);
        self.grid.write_char_at_cursor(cell);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0b | 0x0c => {
                let cur_row = self.grid.cursor().row;
                if cur_row + 1 >= self.grid.rows() {
                    self.grid.scroll_up();
                } else {
                    self.grid.cursor_mut().row += 1;
                }
            }
            b'\r' => {
                self.grid.cursor_mut().col = 0;
            }
            0x08 => {
                // Backspace
                let cur_col = self.grid.cursor().col;
                if cur_col > 0 {
                    self.grid.cursor_mut().col = cur_col - 1;
                }
            }
            b'\t' => {
                // Tab (advance to next multiple of 8)
                let cur_col = self.grid.cursor().col;
                let next_col = ((cur_col / 8) + 1) * 8;
                self.grid.cursor_mut().col = next_col.min(self.grid.cols().saturating_sub(1));
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let is_private = intermediates.first() == Some(&b'?');
        let first_param = params
            .iter()
            .next()
            .and_then(|p| p.first())
            .copied()
            .unwrap_or(0);
        let second_param = params
            .iter()
            .nth(1)
            .and_then(|p| p.first())
            .copied()
            .unwrap_or(0);

        match action {
            'A' => {
                // Cursor Up (CUU)
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().row;
                self.grid.cursor_mut().row = cur.saturating_sub(count);
            }
            'B' => {
                // Cursor Down (CUD)
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().row;
                let max_row = self.grid.rows().saturating_sub(1);
                self.grid.cursor_mut().row = (cur + count).min(max_row);
            }
            'C' => {
                // Cursor Forward (CUF)
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().col;
                let max_col = self.grid.cols().saturating_sub(1);
                self.grid.cursor_mut().col = (cur + count).min(max_col);
            }
            'D' => {
                // Cursor Back (CUB)
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().col;
                self.grid.cursor_mut().col = cur.saturating_sub(count);
            }
            'E' => {
                // Cursor Next Line (CNL)
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().row;
                let max_row = self.grid.rows().saturating_sub(1);
                self.grid.cursor_mut().row = (cur + count).min(max_row);
                self.grid.cursor_mut().col = 0;
            }
            'F' => {
                // Cursor Previous Line (CPL)
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().row;
                self.grid.cursor_mut().row = cur.saturating_sub(count);
                self.grid.cursor_mut().col = 0;
            }
            'G' => {
                // Cursor Horizontal Absolute (CHA)
                let col = (first_param as usize).saturating_sub(1);
                let max_col = self.grid.cols().saturating_sub(1);
                self.grid.cursor_mut().col = col.min(max_col);
            }
            'H' | 'f' => {
                // Cursor Position (CUP / HVP)
                let row = (first_param as usize).saturating_sub(1);
                let col = (second_param as usize).saturating_sub(1);
                let max_row = self.grid.rows().saturating_sub(1);
                let max_col = self.grid.cols().saturating_sub(1);
                self.grid.cursor_mut().row = row.min(max_row);
                self.grid.cursor_mut().col = col.min(max_col);
            }
            'J' => {
                // Erase in Display (ED)
                match first_param {
                    0 => self.grid.clear_from_cursor_down(),
                    1 => self.grid.clear_from_cursor_up(),
                    2 | 3 => self.grid.clear_all(),
                    _ => {}
                }
            }
            'K' => {
                // Erase in Line (EL)
                self.grid.clear_line(first_param as u8);
            }
            'S' => {
                // Scroll Up (SU)
                let count = (first_param as usize).max(1);
                for _ in 0..count {
                    self.grid.scroll_up();
                }
            }
            'T' => {
                // Scroll Down (SD)
                let count = (first_param as usize).max(1);
                for _ in 0..count {
                    self.grid.scroll_down();
                }
            }
            'm' => {
                // Select Graphic Rendition (SGR)
                self.parse_sgr(params);
            }
            'h' if is_private => {
                // DECSET private modes
                match first_param {
                    25 => self.grid.cursor_mut().visible = true,
                    47 | 1047 | 1049 => self.grid.enter_alt_screen(),
                    _ => {}
                }
            }
            'l' if is_private => {
                // DECRST private modes
                match first_param {
                    25 => self.grid.cursor_mut().visible = false,
                    47 | 1047 | 1049 => self.grid.exit_alt_screen(),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'7' => self.grid.save_cursor(),
            b'8' => self.grid.restore_cursor(),
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() >= 2 {
            let command = params[0];
            if command == b"0" || command == b"2" {
                if let Ok(title) = std::str::from_utf8(params[1]) {
                    // Sanitize non-printable control characters and cap title length to 256 characters
                    let sanitized: String = title
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(256)
                        .collect();
                    *self.title = Some(sanitized);
                }
            } else if command == b"7" {
                // OSC 7: Current Working Directory (file://[hostname]/path)
                if let Some(ref mut integration) = self.shell_integration {
                    integration.handle_osc_7(params[1]);
                }
            } else if command == b"133" {
                // OSC 133: FinalTerm / iTerm2 Semantic Prompt Protocol
                if let Some(ref mut integration) = self.shell_integration {
                    integration.handle_osc_133(&params[1..]);
                }
            } else if command == b"9" && params.len() >= 3 && params[1] == b"9" {
                // OSC 9;9: ConEmu CWD ("path")
                if let Some(ref mut integration) = self.shell_integration {
                    integration.handle_osc_9_9(params[2]);
                }
            }
        }
    }
}

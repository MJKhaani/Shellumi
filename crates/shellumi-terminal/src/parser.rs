//! VT/ANSI escape sequence processing updating the terminal grid and emitting typed events.

use crate::cell::{Cell, CellAttributes};
use crate::color::{Color, NamedColor};
use crate::events::{
    sanitize_hyperlink_uri, sanitize_title, CursorShape, HyperlinkTarget, MouseTrackingMode,
    TerminalEvent, MAX_OSC52_PAYLOAD_BYTES, MAX_TERMINAL_EVENTS,
};
use crate::grid::TerminalGrid;
use crate::shell_integration::ShellIntegrationState;
use base64::Engine as _;
use vte::{Params, Perform};

/// Handles VTE parser actions, updates the terminal grid, and records typed events.
pub struct TerminalHandler<'a> {
    pub grid: &'a mut TerminalGrid,
    pub title: &'a mut Option<String>,
    pub shell_integration: Option<&'a mut ShellIntegrationState>,
    pub events: &'a mut Vec<TerminalEvent>,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttributes,
}

impl<'a> TerminalHandler<'a> {
    pub fn new(
        grid: &'a mut TerminalGrid,
        title: &'a mut Option<String>,
        events: &'a mut Vec<TerminalEvent>,
    ) -> Self {
        Self {
            grid,
            title,
            shell_integration: None,
            events,
            fg: Color::Default,
            bg: Color::Default,
            attrs: CellAttributes::default(),
        }
    }

    pub fn with_shell_integration(mut self, integration: &'a mut ShellIntegrationState) -> Self {
        self.shell_integration = Some(integration);
        self
    }

    fn emit(&mut self, event: TerminalEvent) {
        if self.events.len() < MAX_TERMINAL_EVENTS {
            self.events.push(event);
        }
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

    fn set_private_mode(&mut self, mode: u16, enabled: bool) {
        let mut mode_changed = true;
        match mode {
            1 => self.grid.modes_mut().application_cursor = enabled,
            6 => {
                self.grid.modes_mut().origin_mode = enabled;
                self.grid.move_cursor_home();
            }
            7 => self.grid.modes_mut().autowrap = enabled,
            9 => {
                self.grid.modes_mut().mouse_mode = if enabled {
                    MouseTrackingMode::X10
                } else {
                    MouseTrackingMode::None
                };
            }
            12 => self.grid.modes_mut().cursor_blink = enabled,
            25 => self.grid.cursor_mut().visible = enabled,
            47 => {
                if enabled {
                    self.grid.enter_alt_screen_with_options(false, false);
                } else {
                    self.grid.exit_alt_screen_with_options(false);
                }
            }
            1000 => {
                self.grid.modes_mut().mouse_mode = if enabled {
                    MouseTrackingMode::Normal
                } else {
                    MouseTrackingMode::None
                };
            }
            1002 => {
                self.grid.modes_mut().mouse_mode = if enabled {
                    MouseTrackingMode::ButtonMotion
                } else {
                    MouseTrackingMode::None
                };
            }
            1003 => {
                self.grid.modes_mut().mouse_mode = if enabled {
                    MouseTrackingMode::AnyMotion
                } else {
                    MouseTrackingMode::None
                };
            }
            1004 => self.grid.modes_mut().focus_reporting = enabled,
            1006 => self.grid.modes_mut().sgr_mouse = enabled,
            1047 => {
                if enabled {
                    self.grid.enter_alt_screen_with_options(false, true);
                } else {
                    self.grid.exit_alt_screen_with_options(false);
                }
            }
            1048 => {
                if enabled {
                    self.grid.save_cursor_with_sgr(self.fg, self.bg, self.attrs);
                } else {
                    let (fg, bg, attrs) = self.grid.restore_cursor_with_sgr();
                    self.fg = fg;
                    self.bg = bg;
                    self.attrs = attrs;
                }
            }
            1049 => {
                if enabled {
                    self.grid.save_cursor_with_sgr(self.fg, self.bg, self.attrs);
                    self.grid.enter_alt_screen_with_options(false, true);
                } else {
                    self.grid.exit_alt_screen_with_options(false);
                    let (fg, bg, attrs) = self.grid.restore_cursor_with_sgr();
                    self.fg = fg;
                    self.bg = bg;
                    self.attrs = attrs;
                }
            }
            2004 => self.grid.modes_mut().bracketed_paste = enabled,
            _ => {
                mode_changed = false;
            }
        }
        if mode_changed {
            self.emit(TerminalEvent::ModeChanged);
        }
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
            let index = *iter.next()?.first()? as u8;
            Some(Color::Indexed(index))
        }
        2 => {
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
            0x07 => {
                // Standalone BEL (vte consumes BEL inside OSC terminators without calling execute)
                self.emit(TerminalEvent::Bell);
            }
            0x08 => {
                // Backspace (BS)
                self.grid.clear_pending_wrap();
                let cur_col = self.grid.cursor().col;
                if cur_col > 0 {
                    self.grid.cursor_mut().col = cur_col - 1;
                }
            }
            b'\t' => {
                // Horizontal Tab (HT)
                self.grid.tab_forward(1);
            }
            b'\n' | 0x0b | 0x0c => {
                // LF / VT / FF
                self.grid.line_feed();
            }
            b'\r' => {
                // Carriage Return (CR)
                self.grid.clear_pending_wrap();
                self.grid.cursor_mut().col = 0;
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let is_private = intermediates.first() == Some(&b'?');
        let is_gt = intermediates.first() == Some(&b'>');
        let is_space = intermediates.first() == Some(&b' ');
        let is_bang = intermediates.first() == Some(&b'!');

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
                // CUU
                let count = (first_param as usize).max(1);
                let min_row = if self.grid.modes().origin_mode {
                    self.grid.scroll_margins().0
                } else {
                    0
                };
                let cur = self.grid.cursor().row;
                self.grid.cursor_mut().row = cur.saturating_sub(count).max(min_row);
            }
            'B' | 'e' => {
                // CUD / VPR
                let count = (first_param as usize).max(1);
                let max_row = if self.grid.modes().origin_mode {
                    self.grid.scroll_margins().1
                } else {
                    self.grid.rows().saturating_sub(1)
                };
                let cur = self.grid.cursor().row;
                self.grid.cursor_mut().row = (cur + count).min(max_row);
            }
            'C' | 'a' => {
                // CUF / HPR
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().col;
                let max_col = self.grid.cols().saturating_sub(1);
                self.grid.cursor_mut().col = (cur + count).min(max_col);
            }
            'D' => {
                // CUB
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().col;
                self.grid.cursor_mut().col = cur.saturating_sub(count);
            }
            'E' => {
                // CNL
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().row;
                let max_row = self.grid.rows().saturating_sub(1);
                self.grid.cursor_mut().row = (cur + count).min(max_row);
                self.grid.cursor_mut().col = 0;
            }
            'F' => {
                // CPL
                let count = (first_param as usize).max(1);
                let cur = self.grid.cursor().row;
                self.grid.cursor_mut().row = cur.saturating_sub(count);
                self.grid.cursor_mut().col = 0;
            }
            'G' | '`' => {
                // CHA / HPA
                let col = (first_param as usize).saturating_sub(1);
                let max_col = self.grid.cols().saturating_sub(1);
                self.grid.cursor_mut().col = col.min(max_col);
            }
            'd' => {
                // VPA
                let row = (first_param as usize).max(1);
                let col_1 = self.grid.cursor().col + 1;
                self.grid.set_cursor_pos(row, col_1);
            }
            'H' | 'f' => {
                // CUP / HVP
                let row = (first_param as usize).max(1);
                let col = (second_param as usize).max(1);
                self.grid.set_cursor_pos(row, col);
            }
            'I' => {
                // CHT
                self.grid.tab_forward((first_param as usize).max(1));
            }
            'Z' => {
                // CBT
                self.grid.tab_backward((first_param as usize).max(1));
            }
            'g' => {
                // TBC
                self.grid.clear_tab_stop(first_param);
            }
            'J' => {
                // ED
                self.grid.clear_pending_wrap();
                match first_param {
                    0 => self.grid.clear_from_cursor_down(),
                    1 => self.grid.clear_from_cursor_up(),
                    2 => self.grid.clear_all(),
                    3 => {
                        self.grid.clear_all();
                        self.grid.clear_scrollback();
                    }
                    _ => {}
                }
            }
            'K' => {
                // EL
                self.grid.clear_pending_wrap();
                self.grid.clear_line(first_param as u8);
            }
            'X' => {
                // ECH
                self.grid.clear_pending_wrap();
                self.grid.erase_chars((first_param as usize).max(1));
            }
            '@' => {
                // ICH
                self.grid.clear_pending_wrap();
                self.grid.insert_chars((first_param as usize).max(1));
            }
            'P' => {
                // DCH
                self.grid.clear_pending_wrap();
                self.grid.delete_chars((first_param as usize).max(1));
            }
            'L' => {
                // IL
                self.grid.clear_pending_wrap();
                self.grid.insert_lines((first_param as usize).max(1));
            }
            'M' => {
                // DL
                self.grid.clear_pending_wrap();
                self.grid.delete_lines((first_param as usize).max(1));
            }
            'S' => {
                // SU
                self.grid.scroll_up_in_region((first_param as usize).max(1));
            }
            'T' => {
                // SD
                self.grid
                    .scroll_down_in_region((first_param as usize).max(1));
            }
            'r' if !is_private => {
                // DECSTBM
                let top = if first_param == 0 {
                    None
                } else {
                    Some(first_param as usize)
                };
                let bottom = if second_param == 0 {
                    None
                } else {
                    Some(second_param as usize)
                };
                self.grid.set_scroll_margins(top, bottom);
            }
            's' if !is_private => {
                self.grid.save_cursor_with_sgr(self.fg, self.bg, self.attrs);
            }
            'u' if !is_private => {
                let (fg, bg, attrs) = self.grid.restore_cursor_with_sgr();
                self.fg = fg;
                self.bg = bg;
                self.attrs = attrs;
            }
            'c' if is_gt => {
                // Secondary Device Attributes (DA2)
                self.emit(TerminalEvent::ProtocolReply(b"\x1b[>1;10;0c".to_vec()));
            }
            'c' if !is_private => {
                // Primary Device Attributes (DA1)
                self.emit(TerminalEvent::ProtocolReply(b"\x1b[?62;22c".to_vec()));
            }
            'n' if !is_private => {
                // Device Status Report (DSR) & Cursor Position Report (CPR)
                match first_param {
                    5 => {
                        self.emit(TerminalEvent::ProtocolReply(b"\x1b[0n".to_vec()));
                    }
                    6 => {
                        let row = if self.grid.modes().origin_mode {
                            self.grid
                                .cursor()
                                .row
                                .saturating_sub(self.grid.scroll_margins().0)
                                + 1
                        } else {
                            self.grid.cursor().row + 1
                        };
                        let col = self.grid.cursor().col + 1;
                        self.emit(TerminalEvent::ProtocolReply(
                            format!("\x1b[{row};{col}R").into_bytes(),
                        ));
                    }
                    _ => {}
                }
            }
            'm' if !is_private && !is_gt => {
                self.parse_sgr(params);
            }
            'h' if is_private => {
                for param in params.iter() {
                    if let Some(&mode) = param.first() {
                        self.set_private_mode(mode, true);
                    }
                }
            }
            'l' if is_private => {
                for param in params.iter() {
                    if let Some(&mode) = param.first() {
                        self.set_private_mode(mode, false);
                    }
                }
            }
            'h' if !is_private => {
                for param in params.iter() {
                    if param.first().copied() == Some(4) {
                        self.grid.modes_mut().insert_mode = true;
                        self.emit(TerminalEvent::ModeChanged);
                    }
                }
            }
            'l' if !is_private => {
                for param in params.iter() {
                    if param.first().copied() == Some(4) {
                        self.grid.modes_mut().insert_mode = false;
                        self.emit(TerminalEvent::ModeChanged);
                    }
                }
            }
            'q' if is_space => {
                // DECSCUSR
                let (shape, blink) = match first_param {
                    0 | 1 => (CursorShape::Block, true),
                    2 => (CursorShape::Block, false),
                    3 => (CursorShape::Underline, true),
                    4 => (CursorShape::Underline, false),
                    5 => (CursorShape::Beam, true),
                    6 => (CursorShape::Beam, false),
                    _ => return,
                };
                self.grid.modes_mut().cursor_shape = shape;
                self.grid.modes_mut().cursor_blink = blink;
                self.emit(TerminalEvent::ModeChanged);
            }
            'p' if is_bang => {
                // DECSTR (Soft terminal reset)
                self.reset_sgr();
                self.grid.reset(false);
                self.emit(TerminalEvent::ModeChanged);
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'7' => self.grid.save_cursor_with_sgr(self.fg, self.bg, self.attrs),
            b'8' => {
                let (fg, bg, attrs) = self.grid.restore_cursor_with_sgr();
                self.fg = fg;
                self.bg = bg;
                self.attrs = attrs;
            }
            b'D' => self.grid.line_feed(),
            b'E' => {
                self.grid.cursor_mut().col = 0;
                self.grid.line_feed();
            }
            b'H' => self.grid.set_tab_stop(),
            b'M' => self.grid.reverse_index(),
            b'c' => {
                self.reset_sgr();
                self.grid.reset(true);
                self.emit(TerminalEvent::ModeChanged);
            }
            b'=' => {
                self.grid.modes_mut().application_keypad = true;
                self.emit(TerminalEvent::ModeChanged);
            }
            b'>' => {
                self.grid.modes_mut().application_keypad = false;
                self.emit(TerminalEvent::ModeChanged);
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.len() < 2 {
            return;
        }
        let command = params[0];
        if command == b"0" || command == b"2" {
            if let Ok(title) = std::str::from_utf8(params[1]) {
                let sanitized = sanitize_title(title);
                *self.title = Some(sanitized.clone());
                self.emit(TerminalEvent::TitleChanged(Some(sanitized)));
            }
        } else if command == b"7" {
            let mut dir_opt = None;
            if let Some(ref mut integration) = self.shell_integration {
                integration.handle_osc_7(params[1]);
                dir_opt = integration.current_dir().map(|s| s.to_string());
            }
            if let Some(dir) = dir_opt {
                self.emit(TerminalEvent::CwdChanged(dir));
            }
        } else if command == b"8" {
            // OSC 8 ; params ; URI ST
            if params.len() >= 3 {
                let raw_uri = std::str::from_utf8(params[2]).unwrap_or("");
                if raw_uri.trim().is_empty() {
                    self.grid.set_active_hyperlink(None);
                } else if let Some(uri) = sanitize_hyperlink_uri(raw_uri) {
                    let id = std::str::from_utf8(params[1]).ok().and_then(|p| {
                        p.split(':')
                            .find_map(|kv| kv.strip_prefix("id="))
                            .filter(|s| !s.is_empty() && s.len() <= 128)
                            .map(|s| s.to_string())
                    });
                    self.grid
                        .set_active_hyperlink(Some(HyperlinkTarget { id, uri }));
                } else {
                    // Unsafe or malformed URI: close active hyperlink instead of applying
                    self.grid.set_active_hyperlink(None);
                }
            }
        } else if command == b"52" {
            // OSC 52 ; selection ; base64_data ST
            // Never read clipboard ("?"); bound decoded size; emit request for policy check.
            if params.len() >= 3 {
                let payload = params[2];
                if payload == b"?" || payload.is_empty() {
                    return;
                }
                // Cap encoded length before decoding (~4/3 of MAX_OSC52_PAYLOAD_BYTES)
                if payload.len() > MAX_OSC52_PAYLOAD_BYTES * 2 {
                    return;
                }
                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(payload) {
                    if decoded.len() <= MAX_OSC52_PAYLOAD_BYTES {
                        if let Ok(text) = std::str::from_utf8(&decoded) {
                            self.emit(TerminalEvent::ClipboardWriteRequest(text.to_string()));
                        }
                    }
                }
            }
        } else if command == b"133" {
            if let Some(ref mut integration) = self.shell_integration {
                integration.handle_osc_133(&params[1..]);
                if let Some(mark) = integration.current_mark.clone() {
                    self.emit(TerminalEvent::PromptMark(mark));
                }
            }
        } else if command == b"9" && params.len() >= 3 && params[1] == b"9" {
            let mut dir_opt = None;
            if let Some(ref mut integration) = self.shell_integration {
                integration.handle_osc_9_9(params[2]);
                dir_opt = integration.current_dir().map(|s| s.to_string());
            }
            if let Some(dir) = dir_opt {
                self.emit(TerminalEvent::CwdChanged(dir));
            }
        }
    }
}

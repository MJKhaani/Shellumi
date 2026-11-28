//! Native GPUI multiline administrative text/code editor for local and remote SFTP files.
//! Uses `shellumi_core::EditorBuffer` as the sole authoritative document model (no `TextInput`
//! single-line copy, no 40-line preview) and fails closed on remote/local verification errors.

use super::*;
use gpui::{
    fill, point, px, relative, size, Bounds, ClipboardItem, CursorStyle, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId,
    KeyDownEvent, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, ScrollWheelEvent, ShapedLine, Style, TextRun, UTF16Selection, Window,
};
use shellumi_core::{
    detect_language, format_document, highlight_line, EditorBuffer, EditorId, EditorLanguage,
    EditorSessionMeta, EndpointKind, FormatOutcome, LineEnding, SftpConnectOptions, SftpError,
    SyntaxTokenKind, VaultScope, DEFAULT_EDITABLE_LIMIT_BYTES,
};
use std::ops::Range;

/// Clamp `byte_idx` to a valid UTF-8 character boundary `<= text.len()`.
pub(crate) fn clamp_char_boundary(text: &str, byte_idx: usize) -> usize {
    let mut idx = byte_idx.min(text.len());
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Convert a UTF-8 byte offset in `text` to a UTF-16 code-unit offset.
pub fn byte_to_utf16(text: &str, byte_offset: usize) -> usize {
    let clamped = clamp_char_boundary(text, byte_offset);
    text[..clamped].encode_utf16().count()
}

/// Convert a UTF-16 code-unit offset in `text` to a valid UTF-8 byte boundary.
pub fn utf16_to_byte(text: &str, utf16_offset: usize) -> usize {
    let mut u16_count = 0usize;
    for (byte_idx, ch) in text.char_indices() {
        let ch_u16 = ch.len_utf16();
        if u16_count + ch_u16 > utf16_offset {
            return byte_idx;
        }
        u16_count += ch_u16;
    }
    text.len()
}

pub fn range_to_utf16(text: &str, range: Range<usize>) -> Range<usize> {
    byte_to_utf16(text, range.start)..byte_to_utf16(text, range.end)
}

pub fn range_from_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    let start = utf16_to_byte(text, range.start);
    let end = utf16_to_byte(text, range.end).max(start);
    start..end
}

#[derive(Clone, Debug)]
pub(crate) struct EditorConflictState {
    pub message: String,
    pub remote_sha256: String,
}

#[derive(Clone)]
struct RenderedLineLayout {
    byte_start: usize,
    byte_len: usize,
    shaped: ShapedLine,
}

/// Dedicated GPUI multiline code editor entity backed by a single authoritative `EditorBuffer`.
pub struct CodeEditorView {
    pub buffer: EditorBuffer,
    pub language: EditorLanguage,
    pub read_only: bool,
    pub word_wrap: bool,
    pub focus_handle: FocusHandle,
    pub scroll_line: usize,
    pub viewport_lines: usize,
    pub selection_anchor: Option<usize>,
    pub marked_range: Option<Range<usize>>,
    last_unmarked: Option<(Range<usize>, String)>,
    is_selecting: bool,
    last_bounds: Option<Bounds<Pixels>>,
    last_line_height: Pixels,
    last_gutter_width: Pixels,
    last_layouts: Vec<RenderedLineLayout>,
}

impl CodeEditorView {
    pub fn new(
        buffer: EditorBuffer,
        language: EditorLanguage,
        read_only: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            buffer,
            language,
            read_only,
            word_wrap: false,
            focus_handle: cx.focus_handle(),
            scroll_line: 0,
            viewport_lines: 36,
            selection_anchor: None,
            marked_range: None,
            last_unmarked: None,
            is_selecting: false,
            last_bounds: None,
            last_line_height: px(18.0),
            last_gutter_width: px(46.0),
            last_layouts: Vec::new(),
        }
    }

    pub fn selected_byte_range(&self) -> Range<usize> {
        let head = clamp_char_boundary(self.buffer.text(), self.buffer.cursor());
        match self.selection_anchor {
            Some(anchor) => {
                let a = clamp_char_boundary(self.buffer.text(), anchor);
                if a <= head {
                    a..head
                } else {
                    head..a
                }
            }
            None => head..head,
        }
    }

    pub fn selected_text(&self) -> String {
        let r = self.selected_byte_range();
        if r.is_empty() {
            String::new()
        } else {
            self.buffer.text()[r].to_string()
        }
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    fn delete_selection_if_any(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        let r = self.selected_byte_range();
        if r.is_empty() {
            return false;
        }
        self.buffer.apply_edit(r, "");
        self.selection_anchor = None;
        true
    }

    pub fn ensure_cursor_visible(&mut self) {
        let (cur_line_1based, _) = self.buffer.line_col();
        let cur_line = cur_line_1based.saturating_sub(1);
        let vis = self.viewport_lines.max(1);
        if cur_line < self.scroll_line {
            self.scroll_line = cur_line;
        } else if cur_line >= self.scroll_line + vis {
            self.scroll_line = cur_line.saturating_sub(vis.saturating_sub(1));
        }
    }

    pub fn goto_line(&mut self, one_based_line: usize, cx: &mut Context<Self>) {
        let text = self.buffer.text();
        let target_idx = one_based_line.saturating_sub(1);
        let mut offset = 0usize;
        for (line_idx, line) in text.split('\n').enumerate() {
            if line_idx == target_idx {
                break;
            }
            offset += line.len() + 1;
        }
        let clamped = clamp_char_boundary(text, offset);
        self.clear_selection();
        self.buffer.set_cursor(clamped);
        self.scroll_line = target_idx.saturating_sub(self.viewport_lines / 3);
        cx.notify();
    }

    fn insert_str(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.delete_selection_if_any();
        let pos = self.buffer.cursor();
        self.buffer.apply_edit(pos..pos, text);
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.delete_selection_if_any() {
            self.ensure_cursor_visible();
            cx.notify();
            return;
        }
        let pos = self.buffer.cursor();
        if pos == 0 {
            return;
        }
        let text = self.buffer.text();
        let prev = text[..pos]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        // Never split CRLF on backspace
        let prev =
            if pos >= 2 && text.as_bytes()[pos - 1] == b'\n' && text.as_bytes()[pos - 2] == b'\r' {
                pos - 2
            } else {
                prev
            };
        self.buffer.apply_edit(prev..pos, "");
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.delete_selection_if_any() {
            self.ensure_cursor_visible();
            cx.notify();
            return;
        }
        let pos = self.buffer.cursor();
        let text = self.buffer.text();
        if pos >= text.len() {
            return;
        }
        let next = if pos + 1 < text.len()
            && text.as_bytes()[pos] == b'\r'
            && text.as_bytes()[pos + 1] == b'\n'
        {
            pos + 2
        } else {
            text[pos..]
                .chars()
                .next()
                .map(|c| pos + c.len_utf8())
                .unwrap_or(text.len())
        };
        self.buffer.apply_edit(pos..next, "");
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn set_cursor_with_shift(&mut self, new_pos: usize, shift: bool, cx: &mut Context<Self>) {
        let clamped = clamp_char_boundary(self.buffer.text(), new_pos);
        if shift {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.buffer.cursor());
            }
        } else {
            self.selection_anchor = None;
        }
        self.buffer.set_cursor(clamped);
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn move_left(&mut self, shift: bool, cx: &mut Context<Self>) {
        let pos = self.buffer.cursor();
        if pos > 0 {
            let text = self.buffer.text();
            let prev = if pos >= 2
                && text.as_bytes()[pos - 1] == b'\n'
                && text.as_bytes()[pos - 2] == b'\r'
            {
                pos - 2
            } else {
                text[..pos]
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            };
            self.set_cursor_with_shift(prev, shift, cx);
        } else if !shift {
            self.clear_selection();
            cx.notify();
        }
    }

    fn move_right(&mut self, shift: bool, cx: &mut Context<Self>) {
        let pos = self.buffer.cursor();
        let text = self.buffer.text();
        if pos < text.len() {
            let next = if pos + 1 < text.len()
                && text.as_bytes()[pos] == b'\r'
                && text.as_bytes()[pos + 1] == b'\n'
            {
                pos + 2
            } else {
                text[pos..]
                    .chars()
                    .next()
                    .map(|c| pos + c.len_utf8())
                    .unwrap_or(text.len())
            };
            self.set_cursor_with_shift(next, shift, cx);
        } else if !shift {
            self.clear_selection();
            cx.notify();
        }
    }

    fn move_vertical(&mut self, delta: isize, shift: bool, cx: &mut Context<Self>) {
        let (cur_line, cur_col) = self.buffer.line_col();
        let lines: Vec<&str> = self.buffer.text().split('\n').collect();
        let target_line = (cur_line as isize - 1 + delta)
            .clamp(0, lines.len().saturating_sub(1) as isize) as usize;
        let mut offset = 0usize;
        for line in lines.iter().take(target_line) {
            offset += line.len() + 1;
        }
        let line_str = lines
            .get(target_line)
            .copied()
            .unwrap_or("")
            .strip_suffix('\r')
            .unwrap_or(lines.get(target_line).copied().unwrap_or(""));
        let target_char_idx = cur_col.saturating_sub(1);
        let byte_in_line = line_str
            .char_indices()
            .nth(target_char_idx)
            .map(|(i, _)| i)
            .unwrap_or(line_str.len());
        self.set_cursor_with_shift(offset + byte_in_line, shift, cx);
    }

    fn move_home(&mut self, shift: bool, cx: &mut Context<Self>) {
        let pos = self.buffer.cursor();
        let start = self.buffer.text()[..pos]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        self.set_cursor_with_shift(start, shift, cx);
    }

    fn move_end(&mut self, shift: bool, cx: &mut Context<Self>) {
        let pos = self.buffer.cursor();
        let text = self.buffer.text();
        let mut end = text[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(text.len());
        if end > 0 && text.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        self.set_cursor_with_shift(end, shift, cx);
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;
        let primary = ks.modifiers.platform || ks.modifiers.control;
        let shift = ks.modifiers.shift;
        if primary {
            match ks.key.as_str() {
                "a" => {
                    let len = self.buffer.text().len();
                    self.selection_anchor = Some(0);
                    self.buffer.set_cursor(len);
                    cx.notify();
                    return;
                }
                "z" if shift => {
                    if !self.read_only && self.buffer.redo() {
                        self.clear_selection();
                        self.ensure_cursor_visible();
                        cx.notify();
                    }
                    return;
                }
                "z" => {
                    if !self.read_only && self.buffer.undo() {
                        self.clear_selection();
                        self.ensure_cursor_visible();
                        cx.notify();
                    }
                    return;
                }
                "y" => {
                    if !self.read_only && self.buffer.redo() {
                        self.clear_selection();
                        self.ensure_cursor_visible();
                        cx.notify();
                    }
                    return;
                }
                "c" => {
                    let sel = self.selected_text();
                    if !sel.is_empty() {
                        cx.write_to_clipboard(ClipboardItem::new_string(sel));
                    }
                    return;
                }
                "x" => {
                    let sel = self.selected_text();
                    if !sel.is_empty() {
                        cx.write_to_clipboard(ClipboardItem::new_string(sel));
                        if !self.read_only {
                            self.delete_selection_if_any();
                            self.ensure_cursor_visible();
                            cx.notify();
                        }
                    }
                    return;
                }
                "v" => {
                    if !self.read_only {
                        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                            self.insert_str(&text, cx);
                        }
                    }
                    return;
                }
                "home" => {
                    self.set_cursor_with_shift(0, shift, cx);
                    return;
                }
                "end" => {
                    let len = self.buffer.text().len();
                    self.set_cursor_with_shift(len, shift, cx);
                    return;
                }
                _ => {}
            }
        }

        match ks.key.as_str() {
            "backspace" => self.backspace(cx),
            "delete" => self.delete_forward(cx),
            "enter" | "return" => {
                let nl = match self.buffer.line_ending() {
                    LineEnding::Lf => "\n",
                    LineEnding::Crlf => "\r\n",
                };
                self.insert_str(nl, cx);
            }
            "tab" => {
                self.insert_str("  ", cx);
            }
            "left" => self.move_left(shift, cx),
            "right" => self.move_right(shift, cx),
            "up" => self.move_vertical(-1, shift, cx),
            "down" => self.move_vertical(1, shift, cx),
            "pageup" => {
                let step = self.viewport_lines.max(1) as isize;
                self.move_vertical(-step, shift, cx);
            }
            "pagedown" => {
                let step = self.viewport_lines.max(1) as isize;
                self.move_vertical(step, shift, cx);
            }
            "home" => self.move_home(shift, cx),
            "end" => self.move_end(shift, cx),
            _ => {
                // Printable text is handled by `EntityInputHandler::replace_text_in_range`
                // when registered via `window.handle_input` to avoid double-insertion with IME.
            }
        }
    }

    fn point_to_byte_offset(&self, point: Point<Pixels>) -> Option<usize> {
        let bounds = self.last_bounds?;
        let text_left = bounds.left() + self.last_gutter_width + px(8.0);
        let rel_y = (point.y - bounds.top() - px(8.0)).max(px(0.0));
        let row_idx = (f32::from(rel_y) / f32::from(self.last_line_height).max(1.0)) as usize;
        if self.last_layouts.is_empty() {
            return Some(0);
        }
        let layout = self
            .last_layouts
            .get(row_idx.min(self.last_layouts.len() - 1))?;
        let rel_x = (point.x - text_left).max(px(0.0));
        let col_byte = layout
            .shaped
            .closest_index_for_x(rel_x)
            .min(layout.byte_len);
        Some(clamp_char_boundary(
            self.buffer.text(),
            layout.byte_start + col_byte,
        ))
    }
}

impl Focusable for CodeEditorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for CodeEditorView {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.buffer.text();
        let byte_range = range_from_utf16(text, &range_utf16);
        *actual_range = Some(range_to_utf16(text, byte_range.clone()));
        Some(text[byte_range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.buffer.text();
        let sel = self.selected_byte_range();
        let head = clamp_char_boundary(text, self.buffer.cursor());
        let reversed = self
            .selection_anchor
            .is_some_and(|a| clamp_char_boundary(text, a) > head);
        Some(UTF16Selection {
            range: range_to_utf16(text, sel),
            reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let text = self.buffer.text();
        self.marked_range
            .as_ref()
            .map(|r| range_to_utf16(text, r.clone()))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(range) = self.marked_range.take() {
            let text = self.buffer.text();
            let clamped =
                clamp_char_boundary(text, range.start)..clamp_char_boundary(text, range.end);
            self.last_unmarked = Some((clamped.clone(), text[clamped].to_string()));
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let text = self.buffer.text();
        let byte_range = match range_utf16 {
            Some(r) => range_from_utf16(text, &r),
            None => self
                .marked_range
                .clone()
                .unwrap_or_else(|| self.selected_byte_range()),
        };
        if let Some((last_range, last_text)) = self.last_unmarked.take() {
            if byte_range == (last_range.end..last_range.end) && new_text == last_text {
                return;
            }
        }
        self.buffer.apply_edit(byte_range, new_text);
        self.marked_range = None;
        self.selection_anchor = None;
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let text = self.buffer.text();
        let byte_range = match range_utf16 {
            Some(r) => range_from_utf16(text, &r),
            None => self
                .marked_range
                .clone()
                .unwrap_or_else(|| self.selected_byte_range()),
        };
        let start_byte = byte_range.start;
        self.buffer.apply_edit(byte_range, new_text);
        self.marked_range = if new_text.is_empty() {
            None
        } else {
            Some(start_byte..start_byte + new_text.len())
        };
        if let Some(sel_u16) = new_selected_range_utf16 {
            let rel_start = utf16_to_byte(new_text, sel_u16.start);
            let rel_end = utf16_to_byte(new_text, sel_u16.end);
            self.selection_anchor = if rel_start != rel_end {
                Some(start_byte + rel_start)
            } else {
                None
            };
            self.buffer.set_cursor(start_byte + rel_end);
        } else {
            self.selection_anchor = None;
        }
        self.ensure_cursor_visible();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        let text = self.buffer.text();
        let byte_range = range_from_utf16(text, &range_utf16);
        let text_left = bounds.left() + self.last_gutter_width + px(8.0);
        for (row, layout) in self.last_layouts.iter().enumerate() {
            let line_end = layout.byte_start + layout.byte_len;
            if byte_range.start >= layout.byte_start && byte_range.start <= line_end {
                let local_start = (byte_range.start - layout.byte_start).min(layout.byte_len);
                let local_end =
                    (byte_range.end.min(line_end) - layout.byte_start).min(layout.byte_len);
                let x0 = text_left + layout.shaped.x_for_index(local_start);
                let x1 = text_left + layout.shaped.x_for_index(local_end);
                let top = bounds.top() + px(8.0) + self.last_line_height * (row as f32);
                return Some(Bounds::from_corners(
                    point(x0, top),
                    point(x1.max(x0 + px(2.0)), top + self.last_line_height),
                ));
            }
        }
        None
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<gpui::Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let byte_offset = self.point_to_byte_offset(point)?;
        Some(byte_to_utf16(self.buffer.text(), byte_offset))
    }
}

fn token_color(kind: SyntaxTokenKind) -> gpui::Rgba {
    match kind {
        SyntaxTokenKind::Comment => rgb(0x6b7280),
        SyntaxTokenKind::Keyword | SyntaxTokenKind::Directive => rgb(0x60a5fa),
        SyntaxTokenKind::String => rgb(0x34d399),
        SyntaxTokenKind::Key | SyntaxTokenKind::Section => rgb(0xfbbf24),
        SyntaxTokenKind::Variable => rgb(0xc084fc),
        SyntaxTokenKind::Number | SyntaxTokenKind::Boolean => rgb(0xf472b6),
        SyntaxTokenKind::Punctuation => rgb(0x94a3b8),
        SyntaxTokenKind::Plain => rgb(MacTheme::TEXT_PRIMARY),
    }
}

struct CodeEditorElement {
    editor: Entity<CodeEditorView>,
}

struct CodeEditorPrepaint {
    line_layouts: Vec<RenderedLineLayout>,
    gutter_layouts: Vec<ShapedLine>,
    line_height: Pixels,
    gutter_width: Pixels,
    active_row: Option<usize>,
    selection_quads: Vec<PaintQuad>,
    caret_quad: Option<PaintQuad>,
}

impl IntoElement for CodeEditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for CodeEditorElement {
    type RequestLayoutState = ();
    type PrepaintState = CodeEditorPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let line_height = px(18.0);
        let font_size = px(12.0);
        let gutter_width = px(48.0);
        let viewport_lines = ((f32::from(bounds.size.height - px(16.0)) / f32::from(line_height))
            .floor()
            .max(1.0)) as usize;

        let (text, language, scroll_line, cursor_byte, sel_range, is_focused) = {
            let view = self.editor.read(cx);
            (
                view.buffer.text().to_string(),
                view.language,
                view.scroll_line,
                clamp_char_boundary(view.buffer.text(), view.buffer.cursor()),
                view.selected_byte_range(),
                view.focus_handle.is_focused(window),
            )
        };

        let mut line_starts = Vec::new();
        let mut offset = 0usize;
        for raw_line in text.split('\n') {
            let clean = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            line_starts.push((offset, clean.len()));
            offset += raw_line.len() + 1;
        }
        let total_lines = line_starts.len().max(1);
        let start = scroll_line.min(total_lines.saturating_sub(1));
        let end = (start + viewport_lines + 1).min(total_lines);

        let base_style = window.text_style();
        let mut line_layouts = Vec::with_capacity(end.saturating_sub(start));
        let mut gutter_layouts = Vec::with_capacity(end.saturating_sub(start));
        let mut selection_quads = Vec::new();
        let mut caret_quad = None;
        let mut active_row = None;

        let text_left = bounds.left() + gutter_width + px(8.0);

        for (row, line_idx) in (start..end).enumerate() {
            let (byte_start, byte_len) = line_starts[line_idx];
            let line_slice = &text[byte_start..byte_start + byte_len];
            let line_num = line_idx + 1;
            let row_top = bounds.top() + px(8.0) + line_height * (row as f32);

            let is_cur = cursor_byte >= byte_start && cursor_byte <= byte_start + byte_len;
            if is_cur {
                active_row = Some(row);
            }

            let gutter_str = format!("{:>4}", line_num);
            let mut g_run = base_style.to_run(gutter_str.len());
            g_run.color = if is_cur {
                rgb(MacTheme::TEXT_PRIMARY).into()
            } else {
                rgb(MacTheme::TEXT_TERTIARY).into()
            };
            gutter_layouts.push(window.text_system().shape_line(
                gutter_str.into(),
                font_size,
                &[g_run],
                None,
            ));

            let spans = highlight_line(language, line_slice);
            let mut runs: Vec<TextRun> = Vec::new();
            let mut cur = 0usize;
            for span in spans {
                if span.range.start > cur && line_slice.is_char_boundary(span.range.start) {
                    let mut r = base_style.to_run(span.range.start - cur);
                    r.color = rgb(MacTheme::TEXT_PRIMARY).into();
                    runs.push(r);
                    cur = span.range.start;
                }
                if line_slice.is_char_boundary(span.range.start)
                    && line_slice.is_char_boundary(span.range.end)
                    && span.range.end > cur
                {
                    let mut r = base_style.to_run(span.range.end - cur);
                    r.color = token_color(span.kind).into();
                    runs.push(r);
                    cur = span.range.end;
                }
            }
            if cur < line_slice.len() {
                let mut r = base_style.to_run(line_slice.len() - cur);
                r.color = rgb(MacTheme::TEXT_PRIMARY).into();
                runs.push(r);
            }
            let shaped = if line_slice.is_empty() {
                window
                    .text_system()
                    .shape_line(" ".into(), font_size, &[], None)
            } else {
                window.text_system().shape_line(
                    line_slice.to_string().into(),
                    font_size,
                    &runs,
                    None,
                )
            };

            // Selection quad on this line
            let line_end = byte_start + byte_len;
            if !sel_range.is_empty() && sel_range.end > byte_start && sel_range.start < line_end + 1
            {
                let s_local = sel_range.start.saturating_sub(byte_start).min(byte_len);
                let e_local = (sel_range.end - byte_start).min(byte_len);
                let x0 = text_left + shaped.x_for_index(s_local);
                let x1 = text_left
                    + if sel_range.end > line_end {
                        shaped.x_for_index(byte_len) + px(6.0)
                    } else {
                        shaped.x_for_index(e_local)
                    };
                selection_quads.push(fill(
                    Bounds::from_corners(
                        point(x0, row_top),
                        point(x1.max(x0 + px(2.0)), row_top + line_height),
                    ),
                    gpui::rgba(0x3b82f655),
                ));
            }

            if is_cur && is_focused {
                let col_byte = (cursor_byte - byte_start).min(byte_len);
                let caret_x = text_left + shaped.x_for_index(col_byte);
                caret_quad = Some(fill(
                    Bounds::new(point(caret_x, row_top), size(px(2.0), line_height)),
                    rgb(MacTheme::ACCENT_BLUE),
                ));
            }

            line_layouts.push(RenderedLineLayout {
                byte_start,
                byte_len,
                shaped,
            });
        }

        CodeEditorPrepaint {
            line_layouts,
            gutter_layouts,
            line_height,
            gutter_width,
            active_row,
            selection_quads,
            caret_quad,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.editor.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        let viewport_lines = ((f32::from(bounds.size.height - px(16.0))
            / f32::from(prepaint.line_height))
        .floor()
        .max(1.0)) as usize;
        self.editor.update(cx, |view, _| {
            view.last_bounds = Some(bounds);
            view.last_line_height = prepaint.line_height;
            view.last_gutter_width = prepaint.gutter_width;
            view.last_layouts = prepaint.line_layouts.clone();
            view.viewport_lines = viewport_lines;
        });

        // Paint active line background
        if let Some(row) = prepaint.active_row {
            let row_top = bounds.top() + px(8.0) + prepaint.line_height * (row as f32);
            window.paint_quad(fill(
                Bounds::new(
                    point(bounds.left(), row_top),
                    size(bounds.size.width, prepaint.line_height),
                ),
                rgb(MacTheme::SURFACE_ACTIVE),
            ));
        }

        // Paint selection quads
        for quad in prepaint.selection_quads.drain(..) {
            window.paint_quad(quad);
        }

        // Paint gutter and shaped code lines
        let text_left = bounds.left() + prepaint.gutter_width + px(8.0);
        for (row, (g_line, l_layout)) in prepaint
            .gutter_layouts
            .iter()
            .zip(prepaint.line_layouts.iter())
            .enumerate()
        {
            let row_top = bounds.top() + px(8.0) + prepaint.line_height * (row as f32);
            let _ = g_line.paint(
                point(bounds.left() + px(6.0), row_top),
                prepaint.line_height,
                window,
                cx,
            );
            let _ =
                l_layout
                    .shaped
                    .paint(point(text_left, row_top), prepaint.line_height, window, cx);
        }

        // Paint caret
        if let Some(caret) = prepaint.caret_quad.take() {
            window.paint_quad(caret);
        }
    }
}

impl Render for CodeEditorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sftp-code-editor-surface")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .flex_1()
            .min_h_0()
            .bg(rgb(MacTheme::SURFACE))
            .font_family(MONOSPACE_FONT)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus_handle);
                    if let Some(byte_off) = this.point_to_byte_offset(event.position) {
                        if event.click_count == 2 {
                            let text = this.buffer.text();
                            let mut s = byte_off;
                            let mut e = byte_off;
                            while s > 0 {
                                let prev = text[..s].char_indices().next_back().unwrap_or((0, ' '));
                                if prev.1.is_alphanumeric() || prev.1 == '_' {
                                    s = prev.0;
                                } else {
                                    break;
                                }
                            }
                            while e < text.len() {
                                if let Some(ch) = text[e..].chars().next() {
                                    if ch.is_alphanumeric() || ch == '_' {
                                        e += ch.len_utf8();
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                            this.selection_anchor = Some(s);
                            this.buffer.set_cursor(e);
                        } else {
                            this.set_cursor_with_shift(byte_off, event.modifiers.shift, cx);
                        }
                    }
                    this.is_selecting = true;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if this.is_selecting {
                    if let Some(byte_off) = this.point_to_byte_offset(event.position) {
                        this.set_cursor_with_shift(byte_off, true, cx);
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                    this.is_selecting = false;
                    cx.notify();
                }),
            )
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
                let delta_px = f32::from(event.delta.pixel_delta(px(18.0)).y);
                let lines_delta = (-delta_px / 18.0).round() as isize;
                if lines_delta != 0 {
                    let total_lines = this.buffer.text().split('\n').count().max(1);
                    let max_scroll = total_lines.saturating_sub(1);
                    this.scroll_line = ((this.scroll_line as isize) + lines_delta)
                        .clamp(0, max_scroll as isize)
                        as usize;
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(CodeEditorElement {
                editor: cx.entity().clone(),
            })
    }
}

pub(crate) struct ExternalEditorCheckout {
    pub _temp_dir: Option<tempfile::TempDir>,
    pub checkout_path: std::path::PathBuf,
    pub last_sha256: String,
}

fn next_editor_language(lang: EditorLanguage) -> EditorLanguage {
    match lang {
        EditorLanguage::PlainText => EditorLanguage::Json,
        EditorLanguage::Json => EditorLanguage::Jsonc,
        EditorLanguage::Jsonc => EditorLanguage::Yaml,
        EditorLanguage::Yaml => EditorLanguage::Toml,
        EditorLanguage::Toml => EditorLanguage::IniSystemd,
        EditorLanguage::IniSystemd => EditorLanguage::Dotenv,
        EditorLanguage::Dotenv => EditorLanguage::SshConfig,
        EditorLanguage::SshConfig => EditorLanguage::NginxApache,
        EditorLanguage::NginxApache => EditorLanguage::Shell,
        EditorLanguage::Shell => EditorLanguage::Python,
        EditorLanguage::Python => EditorLanguage::PowerShell,
        EditorLanguage::PowerShell => EditorLanguage::Dockerfile,
        EditorLanguage::Dockerfile => EditorLanguage::Xml,
        EditorLanguage::Xml => EditorLanguage::Properties,
        EditorLanguage::Properties => EditorLanguage::PlainText,
    }
}

pub(crate) struct SftpEditorTabState {
    pub meta: EditorSessionMeta,
    pub editor: Entity<CodeEditorView>,
    pub read_only_reason: Option<String>,
    pub find_open: bool,
    pub find_input: Entity<TextInput>,
    pub replace_input: Entity<TextInput>,
    pub goto_line_open: bool,
    pub goto_line_input: Entity<TextInput>,
    pub external_checkout: Option<ExternalEditorCheckout>,
    pub is_saving: bool,
    pub dirty_close_confirm: bool,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub conflict: Option<EditorConflictState>,
    pub connect_options: Option<SftpConnectOptions>,
    pub database_path: Option<String>,
    pub host_id: Option<i64>,
}

impl SftpEditorTabState {
    pub fn title(&self, cx: &App) -> String {
        let base = std::path::Path::new(&self.meta.path_display)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.meta.path_display);
        if self.editor.read(cx).buffer.is_dirty() {
            format!("● {base}")
        } else {
            base.to_string()
        }
    }

    pub fn is_same_target(&self, scope: &VaultScope, endpoint: &EndpointKind, path: &str) -> bool {
        self.meta.vault_scope.is_authorized_under(scope)
            && self.meta.endpoint.same_underlying_target(endpoint)
            && self.meta.path_display == path
    }
}

impl Shellumi {
    /// Open a local or remote file in the integrated GPUI multiline editor, deduplicating by
    /// `(VaultScope, EndpointKind, path)` and registering a vault-scoped operation.
    pub(crate) fn sftp_open_in_editor(
        &mut self,
        is_left: bool,
        entry_path: String,
        cx: &mut Context<Self>,
    ) {
        let pane = if is_left {
            &self.sftp_left_pane
        } else {
            &self.sftp_right_pane
        };
        let Some(host) = pane.host.clone() else {
            return;
        };
        let scope = self.current_vault_scope();
        let endpoint = match &host {
            SftpPaneHost::Local => EndpointKind::local(scope.clone()),
            SftpPaneHost::Remote(h) => EndpointKind::Remote(shellumi_core::RemoteEndpoint {
                endpoint_id: pane.endpoint_id.unwrap_or_default(),
                vault_scope: scope.clone(),
                host_id: h.id,
                host_uuid: None,
                display_name: h.name.clone(),
                destination: h.destination.clone(),
                connection_generation: pane.listing_generation,
                database_path: self.database.clone(),
            }),
        };

        if let Some(idx) = self
            .sftp_editors
            .iter()
            .position(|ed| ed.is_same_target(&scope, &endpoint, &entry_path))
        {
            self.active_sftp_editor_idx = Some(idx);
            cx.notify();
            return;
        }

        let controller = self.sftp_controller.clone();
        let Ok((op_id, ctrl)) = controller.register_op(&scope) else {
            self.desktop.message = Some("Cannot open editor: vault is locked.".into());
            cx.notify();
            return;
        };
        let _database = self.database.clone();
        let _active_vault_id = self.active_vault_id.clone();
        let _active_vault_key = self.active_vault_key();
        let entry_path_clone = entry_path.clone();
        let ep_for_worker = endpoint.clone();

        let load_task = cx.background_executor().spawn(async move {
            if !controller.is_op_authorized(op_id, &scope) {
                return Err(SftpError::Cancelled);
            }
            let res = match &host {
                SftpPaneHost::Local => {
                    let p = std::path::Path::new(&entry_path_clone);
                    let meta = std::fs::symlink_metadata(p)?;
                    if !meta.file_type().is_file() {
                        return Err(SftpError::UnexpectedResponse(
                            "Only regular files can be opened in the editor.".into(),
                        ));
                    }
                    if meta.len() > shellumi_core::DEFAULT_PREVIEW_LIMIT_BYTES {
                        return Err(SftpError::UnexpectedResponse(format!(
                            "File is {} bytes (exceeds {} MiB preview limit)",
                            meta.len(),
                            shellumi_core::DEFAULT_PREVIEW_LIMIT_BYTES / (1024 * 1024)
                        )));
                    }
                    let bytes = std::fs::read(p)?;
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs());
                    #[cfg(unix)]
                    let mode = {
                        use std::os::unix::fs::MetadataExt;
                        Some(meta.mode())
                    };
                    #[cfg(not(unix))]
                    let mode = None;
                    Ok((bytes, meta.len(), mtime, mode, None, None))
                }
                SftpPaneHost::Remote(h) => {
                    let mut opts = SftpConnectOptions {
                        port: Some(h.destination.port().get()),
                        ..Default::default()
                    };
                    if let Some(jump) = &h.jump_host {
                        opts.extra_ssh_options
                            .push(("ProxyJump".to_string(), jump.clone()));
                    }
                    let mut client = controller.open_worker_client(
                        &scope,
                        &ep_for_worker,
                        Some(&opts),
                        &ctrl,
                    )?;
                    let attrs = client.lstat(&entry_path_clone)?;
                    if let Some(sz) = attrs.size {
                        if sz > shellumi_core::DEFAULT_PREVIEW_LIMIT_BYTES {
                            return Err(SftpError::UnexpectedResponse(format!(
                                "Remote file is {sz} bytes (exceeds {} MiB preview limit)",
                                shellumi_core::DEFAULT_PREVIEW_LIMIT_BYTES / (1024 * 1024)
                            )));
                        }
                    }
                    let tmp = tempfile::NamedTempFile::new()?;
                    let downloaded = client.download_file_with_control(
                        &entry_path_clone,
                        tmp.path(),
                        &ctrl,
                        None::<fn(u64, u64)>,
                    )?;
                    let bytes = std::fs::read(tmp.path())?;
                    Ok((
                        bytes,
                        downloaded,
                        attrs.mtime.map(|m| m as u64),
                        attrs.permissions,
                        Some(opts),
                        Some(h.id),
                    ))
                }
            };
            controller.complete_op(op_id);
            res
        });

        let controller_ui = self.sftp_controller.clone();
        let scope_ui = self.current_vault_scope();
        cx.spawn(async move |app, cx| {
            let res = load_task.await;
            let _ = app.update(cx, |app, cx| {
                // Reject delayed completion after vault lock/switch (even errors).
                if !controller_ui.auth_gate.is_authorized(&scope_ui) {
                    return;
                }
                match res {
                    Ok((bytes, size, mtime, mode, connect_options, host_id)) => {
                        let buffer = match EditorBuffer::from_bytes(&bytes) {
                            Ok(b) => b,
                            Err(reason) => {
                                app.desktop.message =
                                    Some(format!("Cannot open in editor: {reason}"));
                                cx.notify();
                                return;
                            }
                        };
                        let read_only_reason = if size > DEFAULT_EDITABLE_LIMIT_BYTES {
                            Some(format!(
                                "Read-only preview (file exceeds {} MiB editable limit)",
                                DEFAULT_EDITABLE_LIMIT_BYTES / (1024 * 1024)
                            ))
                        } else {
                            None
                        };
                        let language = detect_language(&entry_path, buffer.text());
                        let sha = buffer.sha256_hex();
                        let read_only = read_only_reason.is_some();
                        let editor =
                            cx.new(|cx| CodeEditorView::new(buffer, language, read_only, cx));
                        let find_input = cx.new(|cx| TextInput::new(cx, "", "Find…"));
                        let replace_input = cx.new(|cx| TextInput::new(cx, "", "Replace…"));
                        let goto_line_input = cx.new(|cx| TextInput::new(cx, "", "Line #"));

                        let state = SftpEditorTabState {
                            meta: EditorSessionMeta {
                                editor_id: EditorId::new(),
                                vault_scope: scope_ui,
                                endpoint,
                                path_display: entry_path.clone(),
                                language,
                                original_sha256: sha,
                                original_size: size,
                                original_mtime: mtime,
                                original_mode: mode,
                                read_only,
                            },
                            editor,
                            read_only_reason,
                            find_open: false,
                            find_input,
                            replace_input,
                            goto_line_open: false,
                            goto_line_input,
                            external_checkout: None,
                            is_saving: false,
                            dirty_close_confirm: false,
                            status_message: None,
                            error_message: None,
                            conflict: None,
                            connect_options,
                            database_path: app.database.clone(),
                            host_id,
                        };
                        app.sftp_editors.push(state);
                        app.active_sftp_editor_idx = Some(app.sftp_editors.len() - 1);
                        cx.notify();
                    }
                    Err(err) => {
                        app.desktop.message = Some(format!("Failed to open file: {err}"));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn sftp_editor_format_active(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_sftp_editor_idx else {
            return;
        };
        let Some(ed) = self.sftp_editors.get_mut(idx) else {
            return;
        };
        if ed.meta.read_only {
            ed.error_message = Some("Cannot format a read-only preview.".into());
            cx.notify();
            return;
        }
        let current = ed.editor.read(cx).buffer.text().to_string();
        match format_document(ed.meta.language, &current, 2) {
            FormatOutcome::Formatted { text, changed } => {
                if changed {
                    ed.editor.update(cx, |view, cx| {
                        let len = view.buffer.text().len();
                        view.buffer.apply_edit(0..len, &text);
                        cx.notify();
                    });
                    ed.status_message = Some(format!(
                        "Formatted {} document.",
                        ed.meta.language.display_name()
                    ));
                } else {
                    ed.status_message = Some("Document already formatted.".into());
                }
                ed.error_message = None;
            }
            FormatOutcome::Unsupported(msg) => {
                ed.error_message = Some(msg);
            }
            FormatOutcome::SyntaxError {
                line,
                column,
                message,
            } => {
                ed.error_message = Some(format!(
                    "Format aborted at line {line}, col {column}: {message}"
                ));
            }
        }
        cx.notify();
    }

    /// Save the active editor document using staged atomic publication and **fail-closed**
    /// remote/local fingerprint verification.
    pub(crate) fn sftp_editor_save_active(
        &mut self,
        force_overwrite: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(idx) = self.active_sftp_editor_idx else {
            return;
        };
        let Some(ed) = self.sftp_editors.get_mut(idx) else {
            return;
        };
        if ed.meta.read_only || ed.is_saving {
            return;
        }

        let (snapshot_bytes, new_sha, save_rev) = {
            let view = ed.editor.read(cx);
            (
                view.buffer.to_bytes(),
                view.buffer.sha256_hex(),
                view.buffer.revision(),
            )
        };
        let editor_id = ed.meta.editor_id;
        let scope = ed.meta.vault_scope.clone();
        let orig_sha = ed.meta.original_sha256.clone();
        let orig_mode = ed.meta.original_mode;
        let endpoint = ed.meta.endpoint.clone();
        let path = ed.meta.path_display.clone();
        let opts = ed.connect_options.clone().unwrap_or_default();
        let database = ed.database_path.clone();
        let host_id = ed.host_id;

        let controller = self.sftp_controller.clone();
        let Ok(lease) = controller.register_lease(&scope) else {
            ed.error_message = Some("Cannot save: vault scope is locked or inactive.".into());
            cx.notify();
            return;
        };

        ed.is_saving = true;
        ed.error_message = None;
        ed.status_message = Some("Saving…".into());
        let active_vault_id = scope.vault_id.clone();
        let active_vault_key = self.active_vault_keys.get(&active_vault_id).cloned();
        cx.notify();

        let save_task = cx.background_executor().spawn(async move {
            let _lease = lease;
            _lease.check()?;
            let ctrl = &_lease.control;
            let res = match &endpoint {
                EndpointKind::Local { .. } => {
                    let p = std::path::Path::new(&path);
                    let (expected_len, expected_mtime) = match std::fs::symlink_metadata(p) {
                        Ok(meta) => {
                            if meta.file_type().is_symlink() || !meta.file_type().is_file() {
                                return Err(SftpError::UnexpectedResponse(format!(
                                    "Refusing to save over non-regular/symlink path '{}'",
                                    p.display()
                                )));
                            }
                            if !force_overwrite {
                                let disk_sha = shellumi_core::compute_local_sha256(p, ctrl)?;
                                if disk_sha != orig_sha {
                                    return Ok(Err(EditorConflictState {
                                        message: "File changed on disk since checkout.".into(),
                                        remote_sha256: disk_sha,
                                    }));
                                }
                            }
                            (
                                Some(meta.len()),
                                meta.modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs()),
                            )
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            if !force_overwrite {
                                return Ok(Err(EditorConflictState {
                                    message: "Target file was deleted on disk since checkout."
                                        .into(),
                                    remote_sha256: "<missing>".into(),
                                }));
                            }
                            (None, None)
                        }
                        Err(e) => return Err(SftpError::Io(e)),
                    };
                    let tmp = tempfile::NamedTempFile::new_in(
                        p.parent().unwrap_or_else(|| std::path::Path::new(".")),
                    )?;
                    std::fs::write(tmp.path(), &snapshot_bytes)?;
                    #[cfg(unix)]
                    if let Some(mode) = orig_mode {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = std::fs::set_permissions(
                            tmp.path(),
                            std::fs::Permissions::from_mode(mode),
                        );
                    }
                    let pub_policy = shellumi_core::PublicationPolicy::ReplaceConfirmed {
                        expected_len: if force_overwrite { None } else { expected_len },
                        expected_mtime: if force_overwrite {
                            None
                        } else {
                            expected_mtime
                        },
                    };
                    match shellumi_core::staged_local_copy_file_with_policy(
                        tmp.path(),
                        p,
                        &pub_policy,
                        ctrl,
                        None,
                    ) {
                        Ok(_) => Ok::<Result<String, EditorConflictState>, SftpError>(Ok(new_sha)),
                        Err(SftpError::ConflictRequired(details)) => Ok(Err(EditorConflictState {
                            message: format!("File changed before commit ({:?}).", details.reason),
                            remote_sha256: "<changed>".into(),
                        })),
                        Err(err) => Err(err),
                    }
                }
                EndpointKind::Remote(r) => {
                    let mut client = controller.open_worker_client(
                        &_lease.scope,
                        &endpoint,
                        Some(&opts),
                        ctrl,
                    )?;
                    let _ = (database, active_vault_id, active_vault_key, host_id, r);
                    let attrs = match client.lstat(&path) {
                        Ok(a) => {
                            if a.is_symlink() || !a.is_file() {
                                return Err(SftpError::UnexpectedResponse(format!(
                                    "Refusing to save over remote non-regular/symlink path '{path}'"
                                )));
                            }
                            Some(a)
                        }
                        Err(SftpError::FileNotFound(_)) => {
                            if !force_overwrite {
                                return Ok(Err(EditorConflictState {
                                    message: "Remote file was deleted since checkout.".into(),
                                    remote_sha256: "<missing>".into(),
                                }));
                            }
                            None
                        }
                        Err(err) => return Err(err),
                    };
                    if !force_overwrite {
                        // Fail closed: any verification error (other than FileNotFound) aborts save!
                        match shellumi_core::compute_remote_sha256(&mut client, &path, ctrl) {
                            Ok(remote_sha) if remote_sha != orig_sha => {
                                return Ok(Err(EditorConflictState {
                                    message: "Remote file changed since checkout.".into(),
                                    remote_sha256: remote_sha,
                                }));
                            }
                            Ok(_) => {}
                            Err(SftpError::FileNotFound(_)) => {
                                return Ok(Err(EditorConflictState {
                                    message: "Remote file was deleted since checkout.".into(),
                                    remote_sha256: "<missing>".into(),
                                }));
                            }
                            Err(err) => return Err(err),
                        }
                    }
                    let tmp = tempfile::NamedTempFile::new()?;
                    std::fs::write(tmp.path(), &snapshot_bytes)?;
                    let pub_policy = shellumi_core::PublicationPolicy::ReplaceConfirmed {
                        expected_len: if force_overwrite {
                            None
                        } else {
                            attrs.as_ref().and_then(|a| a.size)
                        },
                        expected_mtime: if force_overwrite {
                            None
                        } else {
                            attrs.as_ref().and_then(|a| a.mtime.map(u64::from))
                        },
                    };
                    match client.upload_file_with_policy(
                        tmp.path(),
                        &path,
                        ctrl,
                        &pub_policy,
                        None::<fn(u64, u64)>,
                    ) {
                        Ok(_) => {
                            if let Some(mode) = orig_mode {
                                let _ = client.setstat(
                                    &path,
                                    shellumi_core::FileAttributes::new().with_permissions(mode),
                                );
                            }
                            Ok::<Result<String, EditorConflictState>, SftpError>(Ok(new_sha))
                        }
                        Err(SftpError::ConflictRequired(details)) => Ok(Err(EditorConflictState {
                            message: format!(
                                "Remote file changed before commit ({:?}).",
                                details.reason
                            ),
                            remote_sha256: "<changed>".into(),
                        })),
                        Err(err) => Err(err),
                    }
                }
            };
            res
        });

        let controller_ui = self.sftp_controller.clone();
        cx.spawn(async move |app, cx| {
            let res = save_task.await;
            let _ = app.update(cx, |app, cx| {
                if !controller_ui.auth_gate.is_authorized(&scope) {
                    return;
                }
                let Some(ed) = app
                    .sftp_editors
                    .iter_mut()
                    .find(|e| e.meta.editor_id == editor_id)
                else {
                    return;
                };
                ed.is_saving = false;
                match res {
                    Ok(Ok(new_sha)) => {
                        ed.meta.original_sha256 = new_sha;
                        let still_dirty = ed.editor.update(cx, |view, cx| {
                            view.buffer.mark_saved_revision(save_rev);
                            cx.notify();
                            view.buffer.is_dirty()
                        });
                        ed.conflict = None;
                        ed.status_message = Some(if still_dirty {
                            "Saved prior revision; newer edits remain unsaved.".into()
                        } else {
                            "Saved.".into()
                        });
                        if ed.dirty_close_confirm && !still_dirty {
                            ed.dirty_close_confirm = false;
                            app.sftp_close_editor_by_id(editor_id, true, cx);
                            return;
                        }
                    }
                    Ok(Err(conflict)) => {
                        ed.conflict = Some(conflict);
                        ed.status_message = None;
                    }
                    Err(err) => {
                        ed.error_message = Some(format!("Save failed: {err}"));
                        ed.status_message = None;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn sftp_close_editor_by_id(
        &mut self,
        editor_id: EditorId,
        force_discard: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(idx) = self
            .sftp_editors
            .iter()
            .position(|e| e.meta.editor_id == editor_id)
        else {
            return;
        };
        let is_dirty = self.sftp_editors[idx].editor.read(cx).buffer.is_dirty();
        if is_dirty && !force_discard {
            self.sftp_editors[idx].dirty_close_confirm = true;
            cx.notify();
            return;
        }
        self.sftp_editors.remove(idx);
        if self.sftp_editors.is_empty() {
            self.active_sftp_editor_idx = None;
        } else if let Some(active) = self.active_sftp_editor_idx {
            if active >= self.sftp_editors.len() {
                self.active_sftp_editor_idx = Some(self.sftp_editors.len() - 1);
            }
        }
        cx.notify();
    }

    /// Open the active editor file in the OS default external editor (`F15`), or
    /// ingest and upload changes from an active remote external-editor checkout.
    pub(crate) fn sftp_editor_open_externally(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.active_sftp_editor_idx else {
            return;
        };
        let scope = self.current_vault_scope();
        let auth_ok = self.sftp_controller.auth_gate.is_authorized(&scope);
        let Some(ed) = self.sftp_editors.get_mut(idx) else {
            return;
        };
        if !auth_ok || !ed.meta.vault_scope.is_authorized_under(&scope) {
            ed.error_message = Some("Vault is locked or inactive.".into());
            cx.notify();
            return;
        }
        if ed.meta.read_only {
            ed.error_message = Some("Cannot edit read-only file externally.".into());
            cx.notify();
            return;
        }

        if let Some(checkout) = ed.external_checkout.take() {
            let Ok(meta) = std::fs::symlink_metadata(&checkout.checkout_path) else {
                ed.error_message = Some("External checkout file is no longer accessible.".into());
                cx.notify();
                return;
            };
            if meta.file_type().is_symlink() || !meta.file_type().is_file() {
                ed.error_message =
                    Some("Refusing to read external checkout: not a regular file.".into());
                cx.notify();
                return;
            }
            let Ok(new_bytes) = std::fs::read(&checkout.checkout_path) else {
                ed.error_message = Some("Failed to read external checkout file.".into());
                cx.notify();
                return;
            };
            let Ok(new_sha) = shellumi_core::compute_local_sha256(
                &checkout.checkout_path,
                &shellumi_core::TransferControl::default(),
            ) else {
                ed.error_message = Some("Failed to hash external checkout file.".into());
                cx.notify();
                return;
            };
            if new_sha == checkout.last_sha256 {
                ed.status_message = Some("No external edits detected; checkout closed.".into());
                cx.notify();
                return;
            }
            let new_text = match String::from_utf8(new_bytes) {
                Ok(s) => s,
                Err(_) => {
                    ed.error_message =
                        Some("External editor wrote invalid UTF-8; upload aborted.".into());
                    cx.notify();
                    return;
                }
            };
            let integrated_dirty = ed.editor.read(cx).buffer.is_dirty();
            if integrated_dirty {
                ed.external_checkout = Some(checkout);
                ed.error_message = Some(
                    "Both integrated and external editors have unsaved changes; save or discard integrated edits first."
                        .into(),
                );
                cx.notify();
                return;
            }
            ed.editor.update(cx, |v, cx| {
                let len = v.buffer.text().len();
                v.buffer.apply_edit(0..len, &new_text);
                cx.notify();
            });
            drop(checkout);
            self.sftp_editor_save_active(false, cx);
            return;
        }

        let target_path = match &ed.meta.endpoint {
            EndpointKind::Local { .. } => std::path::PathBuf::from(&ed.meta.path_display),
            EndpointKind::Remote(_) => {
                let Ok(dir) = tempfile::Builder::new().prefix("shellumi-ext-").tempdir() else {
                    ed.error_message = Some("Failed to create private external checkout.".into());
                    cx.notify();
                    return;
                };
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(
                        dir.path(),
                        std::fs::Permissions::from_mode(0o700),
                    );
                }
                let fname = std::path::Path::new(&ed.meta.path_display)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .filter(|s| !s.is_empty() && !s.contains('/') && !s.contains('\\'))
                    .unwrap_or("remote-file.txt");
                let checkout_path = dir.path().join(fname);
                let bytes = ed.editor.read(cx).buffer.to_bytes();
                if std::fs::write(&checkout_path, &bytes).is_err() {
                    ed.error_message = Some("Failed to write external checkout file.".into());
                    cx.notify();
                    return;
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(
                        &checkout_path,
                        std::fs::Permissions::from_mode(0o600),
                    );
                }
                let sha = ed.editor.read(cx).buffer.sha256_hex();
                ed.external_checkout = Some(ExternalEditorCheckout {
                    _temp_dir: Some(dir),
                    checkout_path: checkout_path.clone(),
                    last_sha256: sha,
                });
                checkout_path
            }
        };

        let launch_res = {
            #[cfg(target_os = "macos")]
            {
                std::process::Command::new("open").arg(&target_path).spawn()
            }
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("xdg-open")
                    .arg(&target_path)
                    .spawn()
            }
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("cmd")
                    .args(["/C", "start", ""])
                    .arg(&target_path)
                    .spawn()
            }
        };
        match launch_res {
            Ok(_) => {
                ed.status_message = Some(
                    "Opened in external editor. Click 'Upload Ext' when done; private checkout is cleaned up on close/lock."
                        .into(),
                );
            }
            Err(err) => {
                ed.external_checkout = None;
                ed.error_message = Some(format!("Failed to launch external editor: {err}"));
            }
        }
        cx.notify();
    }

    pub(crate) fn render_sftp_editor_panel(
        &self,
        ed: &SftpEditorTabState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let editor_id = ed.meta.editor_id;
        let view = ed.editor.read(cx);
        let (line, col) = view.buffer.line_col();
        let is_dirty = view.buffer.is_dirty();
        let word_wrap = view.word_wrap;
        let line_ending_str = match view.buffer.line_ending() {
            LineEnding::Lf => "LF",
            LineEnding::Crlf => "CRLF",
        };

        let mut panel = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(MacTheme::CONTENT_BG));

        // Toolbar
        panel = panel.child(
            div()
                .h(px(40.0))
                .px_3()
                .border_b_1()
                .border_color(rgb(MacTheme::BORDER))
                .bg(rgb(MacTheme::SURFACE))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .child(ed.title(cx)),
                        )
                        .child(
                            div()
                                .id("sftp-editor-lang-btn")
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(rgb(MacTheme::SURFACE_ACTIVE))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    if let Some(ed) = app
                                        .active_sftp_editor_idx
                                        .and_then(|i| app.sftp_editors.get_mut(i))
                                    {
                                        let next = next_editor_language(ed.meta.language);
                                        ed.meta.language = next;
                                        ed.editor.update(cx, |v, cx| {
                                            v.language = next;
                                            cx.notify();
                                        });
                                        cx.notify();
                                    }
                                }))
                                .child(ed.meta.language.display_name()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("sftp-editor-goto-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    if let Some(ed) = app
                                        .active_sftp_editor_idx
                                        .and_then(|i| app.sftp_editors.get_mut(i))
                                    {
                                        ed.goto_line_open = !ed.goto_line_open;
                                        cx.notify();
                                    }
                                }))
                                .child("Go to Line"),
                        )
                        .child(
                            div()
                                .id("sftp-editor-ext-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_editor_open_externally(cx);
                                }))
                                .child(if ed.external_checkout.is_some() {
                                    "Upload Ext"
                                } else {
                                    "Open Ext"
                                }),
                        )
                        .child(
                            div()
                                .id("sftp-editor-find-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    if let Some(ed) = app
                                        .active_sftp_editor_idx
                                        .and_then(|i| app.sftp_editors.get_mut(i))
                                    {
                                        ed.find_open = !ed.find_open;
                                        cx.notify();
                                    }
                                }))
                                .child("Find"),
                        )
                        .child(
                            div()
                                .id("sftp-editor-wrap-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    if let Some(ed) = app
                                        .active_sftp_editor_idx
                                        .and_then(|i| app.sftp_editors.get_mut(i))
                                    {
                                        ed.editor.update(cx, |v, cx| {
                                            v.word_wrap = !v.word_wrap;
                                            cx.notify();
                                        });
                                        cx.notify();
                                    }
                                }))
                                .child(if word_wrap { "Wrap: On" } else { "Wrap: Off" }),
                        )
                        .child(
                            div()
                                .id("sftp-editor-format-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(if ed.meta.language.supports_safe_format() {
                                    rgb(MacTheme::TEXT_PRIMARY)
                                } else {
                                    rgb(MacTheme::TEXT_TERTIARY)
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_editor_format_active(cx);
                                }))
                                .child("Format"),
                        )
                        .child(
                            div()
                                .id("sftp-editor-save-btn")
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.sftp_editor_save_active(false, cx);
                                }))
                                .child(if ed.is_saving { "Saving…" } else { "Save" }),
                        )
                        .child(
                            div()
                                .id("sftp-editor-close-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(MacTheme::SURFACE_HOVER))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.sftp_close_editor_by_id(editor_id, false, cx);
                                }))
                                .child("Close"),
                        ),
                ),
        );

        if ed.dirty_close_confirm {
            panel = panel.child(
                div()
                    .px_3()
                    .py_2()
                    .bg(rgb(0x4a2a12))
                    .border_b_1()
                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Unsaved changes will be lost if you close this tab."),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("sftp-editor-close-save")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.sftp_editor_save_active(false, cx);
                                    }))
                                    .child("Save & Close"),
                            )
                            .child(
                                div()
                                    .id("sftp-editor-close-discard")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::DANGER))
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.sftp_close_editor_by_id(editor_id, true, cx);
                                    }))
                                    .child("Discard"),
                            )
                            .child(
                                div()
                                    .id("sftp-editor-close-cancel")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(MacTheme::SURFACE_HOVER))
                                    .text_xs()
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        if let Some(e) = app
                                            .sftp_editors
                                            .iter_mut()
                                            .find(|e| e.meta.editor_id == editor_id)
                                        {
                                            e.dirty_close_confirm = false;
                                        }
                                        cx.notify();
                                    }))
                                    .child("Cancel"),
                            ),
                    ),
            );
        }

        if let Some(reason) = &ed.read_only_reason {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1p5()
                    .bg(rgb(MacTheme::SURFACE))
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child(reason.clone()),
            );
        }

        if ed.goto_line_open {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1p5()
                    .bg(rgb(MacTheme::SURFACE))
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(120.0)).child(ed.goto_line_input.clone()))
                    .child(
                        div()
                            .id("sftp-editor-goto-submit")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::ACCENT_BLUE))
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(ed) = app
                                    .active_sftp_editor_idx
                                    .and_then(|i| app.sftp_editors.get_mut(i))
                                {
                                    let raw =
                                        ed.goto_line_input.read(cx).value().trim().to_string();
                                    if let Ok(line_num) = raw.parse::<usize>() {
                                        if line_num >= 1 {
                                            ed.editor.update(cx, |v, cx| v.goto_line(line_num, cx));
                                            ed.goto_line_open = false;
                                            ed.error_message = None;
                                        } else {
                                            ed.error_message =
                                                Some("Line number must be >= 1.".into());
                                        }
                                    } else {
                                        ed.error_message =
                                            Some("Enter a valid 1-based line number.".into());
                                    }
                                    cx.notify();
                                }
                            }))
                            .child("Jump"),
                    ),
            );
        }

        if ed.find_open {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1p5()
                    .bg(rgb(MacTheme::SURFACE))
                    .border_b_1()
                    .border_color(rgb(MacTheme::BORDER))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(180.0)).child(ed.find_input.clone()))
                    .child(div().w(px(180.0)).child(ed.replace_input.clone()))
                    .child(
                        div()
                            .id("sftp-editor-find-next")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_xs()
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(ed) = app
                                    .active_sftp_editor_idx
                                    .and_then(|i| app.sftp_editors.get_mut(i))
                                {
                                    let q = ed.find_input.read(cx).value().to_string();
                                    if !q.is_empty() {
                                        let found = ed.editor.update(cx, |v, cx| {
                                            let text = v.buffer.text();
                                            let cur = v.buffer.cursor().min(text.len());
                                            let pos = text[cur..]
                                                .find(&q)
                                                .map(|rel| cur + rel)
                                                .or_else(|| text[..cur].find(&q));
                                            if let Some(start) = pos {
                                                v.selection_anchor = Some(start);
                                                v.buffer.set_cursor(start + q.len());
                                                v.ensure_cursor_visible();
                                                cx.notify();
                                                true
                                            } else {
                                                false
                                            }
                                        });
                                        ed.status_message = Some(if found {
                                            "Match selected.".into()
                                        } else {
                                            "No match found.".into()
                                        });
                                        cx.notify();
                                    }
                                }
                            }))
                            .child("Find Next"),
                    )
                    .child(
                        div()
                            .id("sftp-editor-replace-all")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE_HOVER))
                            .text_xs()
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(ed) = app
                                    .active_sftp_editor_idx
                                    .and_then(|i| app.sftp_editors.get_mut(i))
                                {
                                    let q = ed.find_input.read(cx).value().to_string();
                                    let r = ed.replace_input.read(cx).value().to_string();
                                    let count = ed.editor.update(cx, |v, cx| {
                                        let c = v.buffer.replace_all(&q, &r, true);
                                        cx.notify();
                                        c
                                    });
                                    ed.status_message =
                                        Some(format!("Replaced {count} match(es)."));
                                    cx.notify();
                                }
                            }))
                            .child("Replace All"),
                    ),
            );
        }

        if let Some(conflict) = &ed.conflict {
            panel = panel.child(
                div()
                    .px_3()
                    .py_2()
                    .bg(rgb(0x4a1818))
                    .border_b_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::DANGER))
                            .child(format!(
                                "Conflict: {} (remote SHA-256: {}…)",
                                conflict.message,
                                &conflict.remote_sha256[..conflict.remote_sha256.len().min(12)]
                            )),
                    )
                    .child(
                        div()
                            .id("sftp-editor-force-overwrite")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(MacTheme::DANGER))
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.sftp_editor_save_active(true, cx);
                            }))
                            .child("Overwrite Anyway"),
                    ),
            );
        }

        if let Some(err) = &ed.error_message {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1p5()
                    .bg(rgb(0x4a1818))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(err.clone()),
            );
        } else if let Some(msg) = &ed.status_message {
            panel = panel.child(
                div()
                    .px_3()
                    .py_1p5()
                    .bg(rgb(0x133820))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(msg.clone()),
            );
        }

        // Native multiline GPUI CodeEditorView surface
        panel = panel.child(ed.editor.clone());

        // Status bar
        panel.child(
            div()
                .h(px(26.0))
                .px_3()
                .border_t_1()
                .border_color(rgb(MacTheme::BORDER))
                .bg(rgb(MacTheme::SURFACE))
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .child(format!("Ln {line}, Col {col}"))
                .child(format!(
                    "{} · UTF-8 · {} · {}",
                    ed.meta.language.display_name(),
                    line_ending_str,
                    if ed.meta.read_only {
                        "Read-Only"
                    } else if is_dirty {
                        "Modified"
                    } else {
                        "Clean"
                    }
                )),
        )
    }
}

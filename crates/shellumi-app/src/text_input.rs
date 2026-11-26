// Adapted from GPUI 0.2.2 examples/input.rs, Copyright 2022–2025 Zed Industries, Inc.
// Licensed under Apache-2.0; see resources/licenses/gpui-APACHE.txt.
use std::ops::Range;

use gpui::{
    actions, div, fill, point, prelude::*, px, relative, rgb, rgba, size, App, Bounds,
    ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window,
};

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        TabFocusNext,
        TabFocusPrev,
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextInputVariant {
    /// Standard full-width framed field with theme-derived background, border, focus ring, and optional secret toggle.
    Framed,
    /// Borderless/backgroundless editor embedded inside a parent container (e.g. Quick Connect).
    Embedded,
}

pub(crate) struct TextInput {
    focus_handle: Option<FocusHandle>,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_unmarked: Option<(Range<usize>, String)>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    /// Display masking state (true = show `*`, false = reveal plaintext).
    masked: bool,
    /// Secret classification (true = password/passphrase/token; blocks copy/cut even when revealed).
    is_secret: bool,
    variant: TextInputVariant,
    disabled: bool,
    read_only: bool,
    has_error: bool,
    was_focused: bool,
}

impl Drop for TextInput {
    fn drop(&mut self) {
        if self.is_secret {
            // Note: GPUI's `SharedString` is an immutable `Arc<str>`/`&'static str`, so we cannot
            // zeroize its backing allocation in place; we drop our reference and scrub mutable buffers.
            self.content = SharedString::default();
            if let Some((_, ref mut s)) = self.last_unmarked {
                zeroize::Zeroize::zeroize(s);
            }
            self.last_unmarked = None;
        }
    }
}

#[allow(dead_code)]
impl TextInput {
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn tab_next(&mut self, _: &TabFocusNext, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_next();
        cx.notify();
    }

    fn tab_prev(&mut self, _: &TabFocusPrev, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_prev();
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace("\n", " "), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_secret || self.masked {
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.read_only {
            return;
        }
        if self.is_secret || self.masked {
            self.replace_text_in_range(None, "", window, cx);
            return;
        }
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        let display_idx = line.closest_index_for_x(position.x - bounds.left());
        self.display_byte_to_content_byte(display_idx)
    }

    /// Map a UTF-8 byte offset in `self.content` to a byte offset in `masked_display_text()`.
    pub(crate) fn content_byte_to_display_byte(&self, byte_idx: usize) -> usize {
        let clamped = self.clamp_to_char_boundary(byte_idx.min(self.content.len()));
        if !self.masked || self.content.is_empty() {
            return clamped;
        }
        // Each character in `content` maps to a single 1-byte `*` in `masked_display_text()`.
        self.content[..clamped].chars().count()
    }

    /// Map a byte offset in `masked_display_text()` back to a UTF-8 char-boundary byte offset in `self.content`.
    pub(crate) fn display_byte_to_content_byte(&self, display_idx: usize) -> usize {
        if !self.masked || self.content.is_empty() {
            return self.clamp_to_char_boundary(display_idx.min(self.content.len()));
        }
        self.content
            .char_indices()
            .nth(display_idx)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or(self.content.len())
    }

    fn clamp_to_char_boundary(&self, mut index: usize) -> usize {
        index = index.min(self.content.len());
        while !self.content.is_char_boundary(index) {
            index = index.saturating_sub(1);
        }
        index
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .char_indices()
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .char_indices()
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    pub(crate) fn handle_unmark_text(&mut self) {
        if let Some(range) = self.marked_range.take() {
            if range.start <= range.end && range.end <= self.content.len() {
                self.last_unmarked = Some((range.clone(), self.content[range].to_string()));
            }
        }
    }

    pub(crate) fn handle_replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
    ) -> bool {
        if range_utf16.is_none() && self.marked_range.is_none() {
            if let Some((unmarked_range, ref unmarked_text)) = self.last_unmarked.take() {
                // On macOS AppKit with NSTextInputClient, committing an IME or autocorrect composition
                // may invoke `unmarkText` followed immediately by `insertText:replacementRange:` with the
                // exact string that was already written to `self.content` during `setMarkedText`.
                // If so, the text is already committed at the cursor position; do not insert it twice.
                if unmarked_text == new_text && unmarked_range.end == self.selected_range.start {
                    return false;
                }
            }
        } else {
            self.last_unmarked = None;
        }

        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        self.last_unmarked = None;
        true
    }

    pub(crate) fn handle_replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
    ) {
        self.last_unmarked = None;
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
    }

    pub(crate) fn masked_display_text(&self) -> String {
        if self.content.is_empty() {
            self.placeholder.to_string()
        } else if self.masked {
            "*".repeat(self.content.chars().count())
        } else {
            self.content.to_string()
        }
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.handle_unmark_text();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.read_only {
            return;
        }
        let _ = self.handle_replace_text_in_range(range_utf16, new_text);
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
        if self.disabled || self.read_only {
            return;
        }
        self.handle_replace_and_mark_text_in_range(range_utf16, new_text, new_selected_range_utf16);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        let display_idx = last_layout.index_for_x(point.x - line_point.x)?;
        let utf8_index = self.display_byte_to_content_byte(display_idx);
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

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
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
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
        let input = self.input.read(cx);
        let content = input.content.clone();
        let display_selected_start = input.content_byte_to_display_byte(input.selected_range.start);
        let display_selected_end = input.content_byte_to_display_byte(input.selected_range.end);
        let selected_range = display_selected_start..display_selected_end;
        let cursor = input.content_byte_to_display_byte(input.cursor_offset());
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (
                input.placeholder.clone(),
                crate::theme::text_tertiary().into(),
            )
        } else {
            (SharedString::from(input.masked_display_text()), style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            let m_start = input
                .content_byte_to_display_byte(marked_range.start)
                .min(display_text.len());
            let m_end = input
                .content_byte_to_display_byte(marked_range.end)
                .min(display_text.len())
                .max(m_start);
            vec![
                TextRun {
                    len: m_start,
                    ..run.clone()
                },
                TextRun {
                    len: m_end - m_start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len().saturating_sub(m_end),
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    gpui::blue(),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    rgba(0x3311ff30),
                )),
                None,
            )
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
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
        let focus_handle = self
            .input
            .read(cx)
            .focus_handle
            .clone()
            .expect("FocusHandle must be present");
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        line.paint(bounds.origin, window.line_height(), window, cx)
            .unwrap();

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle(cx);
        let is_focused = focus_handle.is_focused(window);
        if self.is_secret
            && !self.masked
            && ((self.was_focused && !is_focused) || !window.is_window_active())
        {
            self.masked = true;
        }
        self.was_focused = is_focused;

        let base = div()
            .flex()
            .items_center()
            .key_context("TextInput")
            .track_focus(&focus_handle)
            .cursor(if self.disabled {
                CursorStyle::Arrow
            } else {
                CursorStyle::IBeam
            })
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::tab_next))
            .on_action(cx.listener(Self::tab_prev))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move));

        if self.variant == TextInputVariant::Embedded {
            base.text_color(crate::theme::text_primary())
                .line_height(px(22.))
                .text_size(px(13.))
                .w_full()
                .child(div().w_full().child(TextElement { input: cx.entity() }))
        } else {
            let bg_color = if self.disabled {
                crate::theme::surface()
            } else {
                crate::theme::input_bg()
            };
            let border_color = if self.has_error {
                crate::theme::danger()
            } else if is_focused {
                crate::theme::accent_blue()
            } else {
                crate::theme::separator()
            };
            let text_color = if self.disabled {
                crate::theme::text_secondary()
            } else {
                crate::theme::text_primary()
            };

            let mut row = base
                .w_full()
                .min_h(px(34.))
                .px(px(8.))
                .py(px(4.))
                .gap_1p5()
                .bg(bg_color)
                .text_color(text_color)
                .rounded_md()
                .border_1()
                .border_color(border_color)
                .line_height(px(22.))
                .text_size(px(13.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(TextElement { input: cx.entity() }),
                );

            if self.is_secret && !self.disabled {
                let is_masked = self.masked;
                let toggle_icon = if is_masked {
                    crate::ui::icon::IconName::RoleViewer
                } else {
                    crate::ui::icon::IconName::ViewOnly
                };
                row = row.child(
                    div()
                        .id("secret-visibility-toggle")
                        .flex_shrink_0()
                        .w(px(24.))
                        .h(px(24.))
                        .rounded_sm()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(crate::theme::MacTheme::SURFACE_HOVER)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .on_click(cx.listener(|input, _, window, cx| {
                            if let Some(fh) = &input.focus_handle {
                                window.focus(fh);
                            }
                            input.toggle_masked(cx);
                        }))
                        .child(
                            crate::ui::icon::Icon::new(toggle_icon)
                                .size(16.0)
                                .active(!is_masked),
                        ),
                );
            }

            row
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle
            .clone()
            .expect("FocusHandle must be present")
    }
}

#[allow(dead_code)]
impl TextInput {
    pub(crate) fn new(cx: &mut Context<Self>, value: &str, placeholder: &str) -> Self {
        Self {
            focus_handle: Some(cx.focus_handle().tab_stop(true)),
            content: value.to_owned().into(),
            placeholder: placeholder.to_owned().into(),
            selected_range: value.len()..value.len(),
            selection_reversed: false,
            marked_range: None,
            last_unmarked: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            masked: false,
            is_secret: false,
            variant: TextInputVariant::Framed,
            disabled: false,
            read_only: false,
            has_error: false,
            was_focused: false,
        }
    }
    #[cfg(test)]
    pub(crate) fn for_test(value: &str) -> Self {
        Self {
            focus_handle: None,
            content: value.to_owned().into(),
            placeholder: "".into(),
            selected_range: value.len()..value.len(),
            selection_reversed: false,
            marked_range: None,
            last_unmarked: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            masked: false,
            is_secret: false,
            variant: TextInputVariant::Framed,
            disabled: false,
            read_only: false,
            has_error: false,
            was_focused: false,
        }
    }
    pub(crate) fn bare(mut self) -> Self {
        self.variant = TextInputVariant::Embedded;
        self
    }
    pub(crate) fn embedded(mut self) -> Self {
        self.variant = TextInputVariant::Embedded;
        self
    }
    pub(crate) fn masked(mut self) -> Self {
        self.masked = true;
        self.is_secret = true;
        self
    }
    pub(crate) fn is_secret(&self) -> bool {
        self.is_secret
    }
    pub(crate) fn set_error(&mut self, has_error: bool, cx: &mut Context<Self>) {
        if self.has_error != has_error {
            self.has_error = has_error;
            cx.notify();
        }
    }
    pub(crate) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            cx.notify();
        }
    }
    pub(crate) fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        if self.read_only != read_only {
            self.read_only = read_only;
            cx.notify();
        }
    }
    pub(crate) fn reset_reveal(&mut self, cx: &mut Context<Self>) {
        if self.is_secret && !self.masked {
            self.masked = true;
            cx.notify();
        }
    }
    pub(crate) fn clear_secret(&mut self, cx: &mut Context<Self>) {
        self.content = SharedString::default();
        self.selected_range = 0..0;
        self.marked_range = None;
        if let Some((_, ref mut s)) = self.last_unmarked {
            zeroize::Zeroize::zeroize(s);
        }
        self.last_unmarked = None;
        if self.is_secret {
            self.masked = true;
        }
        cx.notify();
    }
    pub(crate) fn toggle_masked(&mut self, cx: &mut Context<Self>) {
        self.masked = !self.masked;
        cx.notify();
    }
    pub(crate) fn is_masked(&self) -> bool {
        self.masked
    }
    pub(crate) fn value(&self) -> &str {
        &self.content
    }
    pub(crate) fn set_value(&mut self, value: &str, cx: &mut Context<Self>) {
        self.content = value.to_owned().into();
        self.selected_range = value.len()..value.len();
        self.marked_range = None;
        self.last_unmarked = None;
        cx.notify();
    }
}

pub(crate) fn bind_keys(cx: &mut App) {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", Left, Some("TextInput")),
        KeyBinding::new("right", Right, Some("TextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new(&format!("{modifier}-a"), SelectAll, Some("TextInput")),
        KeyBinding::new(&format!("{modifier}-v"), Paste, Some("TextInput")),
        KeyBinding::new(&format!("{modifier}-c"), Copy, Some("TextInput")),
        KeyBinding::new(&format!("{modifier}-x"), Cut, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("tab", TabFocusNext, Some("TextInput")),
        KeyBinding::new("shift-tab", TabFocusPrev, Some("TextInput")),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ime_unmark_and_insert_deduplication() {
        let mut input = TextInput::for_test("");

        // 1. User types "a", macOS IME marks "a"
        input.handle_replace_and_mark_text_in_range(None, "a", Some(1..1));
        assert_eq!(input.value(), "a");
        assert_eq!(input.marked_range, Some(0..1));
        assert_eq!(input.selected_range, 1..1);

        // 2. macOS IME unmarks "a" (commits it)
        input.handle_unmark_text();
        assert_eq!(input.value(), "a");
        assert_eq!(input.marked_range, None);
        assert_eq!(input.last_unmarked, Some((0..1, "a".to_string())));

        // 3. macOS AppKit calls insertText with "a"
        let inserted = input.handle_replace_text_in_range(None, "a");
        assert!(!inserted, "duplicate commit of 'a' must be ignored");
        assert_eq!(
            input.value(),
            "a",
            "content must not be duplicated into 'aa'"
        );
        assert_eq!(input.last_unmarked, None);

        // 4. Next keypress "b" normal typing
        let inserted_b = input.handle_replace_text_in_range(None, "b");
        assert!(inserted_b);
        assert_eq!(input.value(), "ab");
    }

    #[test]
    fn test_masked_display_chars_count_and_toggle() {
        let mut input = TextInput::for_test("").masked();
        assert_eq!(input.masked_display_text(), "");

        // ASCII password
        input.handle_replace_text_in_range(None, "secret");
        assert_eq!(input.masked_display_text(), "******");
        assert_eq!(input.masked_display_text().len(), 6);

        // Multi-byte UTF-8 characters (e.g. Persian/Arabic: 2 bytes each in UTF-8)
        let mut input_fa = TextInput::for_test("").masked();
        input_fa.handle_replace_text_in_range(None, "سلام"); // 4 Persian characters, 8 bytes in UTF-8
        assert_eq!(input_fa.content.len(), 8);
        assert_eq!(
            input_fa.masked_display_text(),
            "****",
            "Must produce 4 stars for 4 characters, not 8 stars"
        );

        // Toggle mask
        input_fa.masked = false;
        assert_eq!(input_fa.masked_display_text(), "سلام");
    }

    #[test]
    fn test_masked_unicode_caret_and_display_index_mapping() {
        let mut input = TextInput::for_test("").masked();
        // " aé🚀 " -> leading/trailing spaces + ASCII + 2-byte UTF-8 + 4-byte emoji
        input.handle_replace_text_in_range(None, " aé🚀 ");
        assert_eq!(input.value(), " aé🚀 ");
        assert_eq!(input.masked_display_text(), "*****");
        // Byte offsets in content: ' '(0..1), 'a'(1..2), 'é'(2..4), '🚀'(4..8), ' '(8..9)
        assert_eq!(input.content_byte_to_display_byte(0), 0);
        assert_eq!(input.content_byte_to_display_byte(1), 1);
        assert_eq!(input.content_byte_to_display_byte(2), 2);
        assert_eq!(input.content_byte_to_display_byte(4), 3);
        assert_eq!(input.content_byte_to_display_byte(8), 4);
        assert_eq!(input.content_byte_to_display_byte(9), 5);

        // Reverse mapping from display '*' index back to UTF-8 content byte offsets
        assert_eq!(input.display_byte_to_content_byte(0), 0);
        assert_eq!(input.display_byte_to_content_byte(1), 1);
        assert_eq!(input.display_byte_to_content_byte(2), 2);
        assert_eq!(input.display_byte_to_content_byte(3), 4);
        assert_eq!(input.display_byte_to_content_byte(4), 8);
        assert_eq!(input.display_byte_to_content_byte(5), 9);

        // Secret classification remains true even when unmasked
        assert!(input.is_secret());
        input.masked = false;
        assert!(input.is_secret());
    }
}

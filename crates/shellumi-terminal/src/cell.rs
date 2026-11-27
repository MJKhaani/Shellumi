//! Terminal character cell representation.

use crate::color::Color;
use std::collections::HashMap;

/// Text rendering attributes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellAttributes {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub hidden: bool,
    pub strikethrough: bool,
}

/// Structural flags for wide characters and continuation cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellFlags {
    /// Leading cell of a 2-column wide character (CJK / emoji).
    pub wide: bool,
    /// Trailing continuation spacer cell of a 2-column wide character.
    pub wide_spacer: bool,
}

/// A single character cell on the terminal grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttributes,
    pub flags: CellFlags,
    pub hyperlink_id: u32,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Default,
            bg: Color::Default,
            attrs: CellAttributes::default(),
            flags: CellFlags::default(),
            hyperlink_id: 0,
        }
    }
}

impl Cell {
    pub fn new(c: char, fg: Color, bg: Color, attrs: CellAttributes) -> Self {
        Self {
            c,
            fg,
            bg,
            attrs,
            flags: CellFlags::default(),
            hyperlink_id: 0,
        }
    }

    pub fn with_flags(mut self, flags: CellFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn with_hyperlink(mut self, hyperlink_id: u32) -> Self {
        self.hyperlink_id = hyperlink_id;
        self
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// A styled contiguous text run with uniform colors and attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyledSpan {
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttributes,
}

/// Converts a slice of cells into minimal contiguous styled spans,
/// trimming unstyled trailing spaces.
pub fn cells_to_spans(cells: &[Cell]) -> Vec<StyledSpan> {
    cells_to_spans_with_combining(cells, None)
}

/// Converts a slice of cells and optional per-column combining marks into
/// minimal contiguous styled spans, skipping wide-char continuation spacers
/// and masking hidden text.
pub fn cells_to_spans_with_combining(
    cells: &[Cell],
    combining: Option<&HashMap<usize, String>>,
) -> Vec<StyledSpan> {
    let last_content_idx = cells.iter().rposition(|c| {
        (!c.flags.wide_spacer && c.c != ' ')
            || c.bg != Color::Default
            || c.attrs.underline
            || c.attrs.strikethrough
            || c.attrs.inverse
    });

    let Some(last_idx) = last_content_idx else {
        return Vec::new();
    };

    let relevant_cells = &cells[..=last_idx];
    let mut spans = Vec::new();
    let mut current_span: Option<StyledSpan> = None;

    for (col_idx, cell) in relevant_cells.iter().enumerate() {
        if cell.flags.wide_spacer {
            continue;
        }

        let (fg, bg) = if cell.attrs.inverse {
            (cell.bg, cell.fg)
        } else {
            (cell.fg, cell.bg)
        };

        // Never display hidden text through normal visual rendering.
        let ch = if cell.attrs.hidden { ' ' } else { cell.c };

        if let Some(ref mut span) = current_span {
            if span.fg == fg && span.bg == bg && span.attrs == cell.attrs {
                span.text.push(ch);
                if !cell.attrs.hidden {
                    if let Some(comb) = combining.and_then(|m| m.get(&col_idx)) {
                        span.text.push_str(comb);
                    }
                }
                continue;
            }
            spans.push(current_span.take().unwrap());
        }

        let mut text = String::new();
        text.push(ch);
        if !cell.attrs.hidden {
            if let Some(comb) = combining.and_then(|m| m.get(&col_idx)) {
                text.push_str(comb);
            }
        }
        current_span = Some(StyledSpan {
            text,
            fg,
            bg,
            attrs: cell.attrs,
        });
    }

    if let Some(span) = current_span {
        spans.push(span);
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::NamedColor;

    #[test]
    fn groups_adjacent_identical_cells_into_spans() {
        let cells = vec![
            Cell::new(
                'H',
                Color::Named(NamedColor::Green),
                Color::Default,
                CellAttributes::default(),
            ),
            Cell::new(
                'i',
                Color::Named(NamedColor::Green),
                Color::Default,
                CellAttributes::default(),
            ),
            Cell::new(
                ' ',
                Color::Default,
                Color::Default,
                CellAttributes::default(),
            ),
            Cell::new(
                '!',
                Color::Named(NamedColor::Red),
                Color::Default,
                CellAttributes {
                    bold: true,
                    ..Default::default()
                },
            ),
            Cell::new(
                ' ',
                Color::Default,
                Color::Default,
                CellAttributes::default(),
            ),
            Cell::new(
                ' ',
                Color::Default,
                Color::Default,
                CellAttributes::default(),
            ),
        ];

        let spans = cells_to_spans(&cells);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].text, "Hi");
        assert_eq!(spans[0].fg, Color::Named(NamedColor::Green));
        assert_eq!(spans[1].text, " ");
        assert_eq!(spans[2].text, "!");
        assert_eq!(spans[2].fg, Color::Named(NamedColor::Red));
        assert!(spans[2].attrs.bold);
    }

    #[test]
    fn inverts_colors_when_inverse_attribute_is_set() {
        let cells = vec![Cell::new(
            'X',
            Color::Named(NamedColor::White),
            Color::Named(NamedColor::Blue),
            CellAttributes {
                inverse: true,
                ..Default::default()
            },
        )];
        let spans = cells_to_spans(&cells);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].fg, Color::Named(NamedColor::Blue));
        assert_eq!(spans[0].bg, Color::Named(NamedColor::White));
    }

    #[test]
    fn masks_hidden_cells_and_attaches_combining_marks() {
        let cells = vec![
            Cell::new(
                'e',
                Color::Default,
                Color::Default,
                CellAttributes::default(),
            ),
            Cell::new(
                'S',
                Color::Default,
                Color::Default,
                CellAttributes {
                    hidden: true,
                    ..Default::default()
                },
            ),
        ];
        let mut combining = HashMap::new();
        combining.insert(0, "\u{0301}".to_string());
        combining.insert(1, "\u{0301}".to_string());
        let spans = cells_to_spans_with_combining(&cells, Some(&combining));
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "e\u{0301}");
        assert_eq!(spans[1].text, " ");
    }
}

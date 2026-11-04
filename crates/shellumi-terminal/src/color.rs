//! Terminal colors and styling.

/// Standard 16 ANSI colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

/// A terminal color: default/reset, standard 16 named color, 256-color index, or 24-bit TrueColor RGB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    #[default]
    Default,
    Named(NamedColor),
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl NamedColor {
    /// Returns the standard 24-bit RGB values for this ANSI color.
    pub fn to_rgb(self) -> (u8, u8, u8) {
        match self {
            NamedColor::Black => (0, 0, 0),
            NamedColor::Red => (205, 49, 49),
            NamedColor::Green => (13, 188, 121),
            NamedColor::Yellow => (229, 229, 16),
            NamedColor::Blue => (36, 114, 200),
            NamedColor::Magenta => (188, 63, 188),
            NamedColor::Cyan => (17, 168, 205),
            NamedColor::White => (229, 229, 229),
            NamedColor::BrightBlack => (102, 102, 102),
            NamedColor::BrightRed => (241, 76, 76),
            NamedColor::BrightGreen => (35, 209, 139),
            NamedColor::BrightYellow => (245, 245, 67),
            NamedColor::BrightBlue => (59, 142, 234),
            NamedColor::BrightMagenta => (214, 112, 214),
            NamedColor::BrightCyan => (41, 184, 219),
            NamedColor::BrightWhite => (255, 255, 255),
        }
    }
}

impl Color {
    /// Resolves the color to RGB, using `default_rgb` when the color is `Color::Default`.
    pub fn to_rgb(self, default_rgb: (u8, u8, u8)) -> (u8, u8, u8) {
        match self {
            Color::Default => default_rgb,
            Color::Named(named) => named.to_rgb(),
            Color::Indexed(idx) => match idx {
                0 => NamedColor::Black.to_rgb(),
                1 => NamedColor::Red.to_rgb(),
                2 => NamedColor::Green.to_rgb(),
                3 => NamedColor::Yellow.to_rgb(),
                4 => NamedColor::Blue.to_rgb(),
                5 => NamedColor::Magenta.to_rgb(),
                6 => NamedColor::Cyan.to_rgb(),
                7 => NamedColor::White.to_rgb(),
                8 => NamedColor::BrightBlack.to_rgb(),
                9 => NamedColor::BrightRed.to_rgb(),
                10 => NamedColor::BrightGreen.to_rgb(),
                11 => NamedColor::BrightYellow.to_rgb(),
                12 => NamedColor::BrightBlue.to_rgb(),
                13 => NamedColor::BrightMagenta.to_rgb(),
                14 => NamedColor::BrightCyan.to_rgb(),
                15 => NamedColor::BrightWhite.to_rgb(),
                16..=231 => {
                    let idx = idx - 16;
                    let r = (idx / 36) % 6;
                    let g = (idx / 6) % 6;
                    let b = idx % 6;
                    let cv = |val: u8| if val == 0 { 0 } else { 55 + val * 40 };
                    (cv(r), cv(g), cv(b))
                }
                232..=255 => {
                    let gray = (idx - 232) * 10 + 8;
                    (gray, gray, gray)
                }
            },
            Color::Rgb(r, g, b) => (r, g, b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_named_and_indexed_colors_to_rgb() {
        assert_eq!(NamedColor::Red.to_rgb(), (205, 49, 49));
        assert_eq!(
            Color::Named(NamedColor::Green).to_rgb((0, 0, 0)),
            (13, 188, 121)
        );
        assert_eq!(Color::Default.to_rgb((12, 34, 56)), (12, 34, 56));
        assert_eq!(Color::Rgb(1, 2, 3).to_rgb((0, 0, 0)), (1, 2, 3));
        assert_eq!(Color::Indexed(1).to_rgb((0, 0, 0)), (205, 49, 49)); // Red
        assert_eq!(Color::Indexed(16).to_rgb((0, 0, 0)), (0, 0, 0)); // Cube start
        assert_eq!(Color::Indexed(231).to_rgb((0, 0, 0)), (255, 255, 255)); // Cube end
        assert_eq!(Color::Indexed(232).to_rgb((0, 0, 0)), (8, 8, 8)); // Gray start
        assert_eq!(Color::Indexed(255).to_rgb((0, 0, 0)), (238, 238, 238)); // Gray end
    }
}

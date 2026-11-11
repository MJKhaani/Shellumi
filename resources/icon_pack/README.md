# Shellumi macOS Dark GPUI Icon Pack — v7 (v5 style)

**Source:** complete icon pack v5, retaining all its assets. `edit`, `hosts`, and `terminal` were restored byte-for-byte from v5 because those already follow the exact requested style.

Four added names: `invite_member`, `infrastructure`, `diagnostics`, `mobile`. All four have regular and hover PNG assets following v5 styling and dimensions.

**State convention:**

- `icons/<size>/regular/`: standalone metallic-blue/silver glyph, transparent background, no border/tile.
- `icons/<size>/hover/`: same motif on a dark graphite rounded-square tile with border.

**Sizes:** 32, 48, 64, 128, 256 pixels.

**GPUI:** use full-color PNG assets through `gpui::img()` with a file path or registered asset source. `gpui_icon.rs` provides a basic path-based example. State swapping belongs to the button's hover behavior.

**Notes:** The four new glyphs were generated to follow the v5 visual vocabulary. They are additional artwork, not extracted originals. The hover tiles are matched stylistically to v5, not derived from pixel-exact v5 hover surfaces.

# Shellumi — macOS Dark GPUI Icon Pack

28 matching icons in two states, exported from the image sheets in this conversation and extended with three additional icons in the same style.

- **regular** = the first sheet style (floating glossy icon on transparent background).
- **hover** = the second sheet style (icon presented on a dark rounded-square hover tile).
- **28 icons × 2 states = 56 unique artworks**, in **5 raster resolutions** = 280 PNG files.
- All PNGs are RGBA with transparency, and each regular/hover pair has exactly the same pixel dimensions and placement canvas.
- Original source sheets and a preview HTML gallery are included.

## Included additional icons

- `known_hosts`
- `port_forwarding`
- `shellumi_logo`

## File naming

`icons/256/regular/hosts.png` and `icons/256/hover/hosts.png` form a pair.

Available sizes: `256`, `128`, `64`, `48`, `32` (square pixels). For a 24-point icon on a Retina macOS screen, the `48` folder is the nominal 2× size; you can also use the `256` master for high-quality downscaling.

## Rust / GPUI

GPUI uses `img()` for full-color PNGs. It is intentionally not `svg()` because that treats the artwork as a single-color mask and would lose the multicolor styling. GPUI's `img()` accepts a `PathBuf` pointing to an actual PNG on disk. If passing a string path, it is interpreted as an embedded asset key and requires a registered `AssetSource`.

See `gpui_icon.rs` for a simple helper.

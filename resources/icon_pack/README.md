# Shellumi — GPUI macOS Dark Icon Pack · v4 (state-corrected)

**Corrected asset states:**

- `regular/` — **no rounded tile/border**. Standalone metallic-blue/silver artwork on a transparent canvas.
- `hover/` — **dark rounded-square tile with border** around the icon.

**33 named icons × 2 states × 5 resolutions = 330 individual RGBA PNGs**.

This version fixes 22 reversed icon pairs from the earlier archive. It preserves all artwork and only changes the folders where needed. Includes `known_hosts`, `port_forwarding`, `shellumi_logo`, `group_settings`, `import`, `export`, `tag`, `calendar_sort`, `connect`, `grid_view`, and `list_view`.

## File organization

```text
shellumi_gpui_icon_pack/
├── icons/
│   ├── 32/
│   ├── 48/
│   ├── 64/
│   ├── 128/
│   └── 256/
│       ├── regular/hosts.png    # borderless
│       └── hover/hosts.png      # dark bordered tile
├── gpui_icon.rs
├── manifest.json
├── manifest.csv
├── preview.html
├── contact_sheet.jpg
└── source_sheets/
    ├── regular.png              # borderless sheet
    └── hover.png                # bordered sheet
```

Use GPUI's `img()` for full-color PNGs, as shown in `gpui_icon.rs`. Images are stored as transparent RGBA canvases at exactly 32, 48, 64, 128, and 256 square pixels.

## QA

Every size and state is present for all 33 names. Both states use identically sized canvases. Asset changes are folder moves; there is no re-rendering of old icons in this correction.

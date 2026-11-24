# Shellumi GPUI Icon Pack v10 — MAPPING AND CROP REPAIR

## Why v8 was incorrect
The generated icon source sheets were **misnamed/assigned to the wrong groups** during export. All eight image-to-name mappings were wrong. The nine-image OS sheet is a **3×6** grid (3×3 regular + 3×3 hover), not a 5×6 grid. This produced incorrect images under valid filenames.

## Repair method
- Re-extracted all **114 newly generated** icons from the verified, visually identified source sheets.
- Used the correct **5×6** grids for seven sheets, and **3×6** for the last OS sheet.
- Preserved all 54 previously existing icon names from v7, correcting clearly off-center standalone artwork for 20 icons (including `delete`).
- Regular = standalone icon on transparent background, **no tile**.
- Hover = same icon on a dark rounded-square tile.
- Generated transparent RGBA PNG assets at 32, 48, 64, 128, 256 pixels.
- Supplied an explicit **source_mapping.csv**, a full manifest, and an interactive searchable gallery.

## Contents
- `icons/<size>/regular/<name>.png` and `icons/<size>/hover/<name>.png`
- `manifest.json` and `manifest.csv`
- `source_mapping.csv`
- `gpui_icon.rs`
- `preview.html` — searchable gallery
- `contact_sheet_new_regular.jpg` / `contact_sheet_new_hover.jpg`
- `source_sheets/` — original prior v7 references
- `recovered_sheets/` — exact v8 generated source images, with original filenames
- `agent_catalog/` — the original agent report from the ZIP
- `qa_report.json`

## QC disclaimer
Generated art is not guaranteed to match brand identity perfectly or remain legible at the smallest sizes. This repair validates the **source-to-icon names and extraction geometry** against the supplied catalog. Branding and icon semantics should still receive a human review before shipping.

## Additional v10 correction: grid overflow
The generated source images were not perfectly aligned to the grid: parts of one icon occasionally crossed a cell boundary. Previous fixed-cell slicing lost part of those icons or captured pieces of adjacent icons. v10 identifies connected illustrated objects over the **whole sheet** and assigns each object to the closest intended icon center; antialiased edge pixels belong to the closest illustrated object. This recovers content that straddled cell lines, while removing neighbor spill.

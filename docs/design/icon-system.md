# <img src="../../resources/icon_pack/icons/48/regular/hosts.png" width="24" height="24" alt="Icon System"> Icon System

[Docs Home](../README.md) / [Design](README.md) / Icon System

---

Shellumi embeds a custom raster icon pack under `resources/icon_pack/` (`manifest.json`), rendered in GPUI via `crates/shellumi-app/src/ui/icon.rs` and `gpui::AssetSource`.

- **Inventory:** 168 icons across 5 resolutions (`32`, `48`, `64`, `128`, `256` px) and 2 states (`regular`, `hover`).
- **Verification:** `test_icon_pack_all_168_resolutions_and_hover` and `test_all_1680_icon_assets_png_decode_dimensions_and_artwork` decode all 1,680 PNG assets and verify exact dimensions and non-empty artwork.
- **Documentation Usage Rules:**
  - Always use `regular` variants (`resources/icon_pack/icons/48/regular/` or `32/regular/`) in static documentation unless illustrating hover behavior.
  - Never duplicate the icon pack inside `docs/` or modify application filenames/mappings.
  - Always pair icons with descriptive `alt` text and visible text labels.

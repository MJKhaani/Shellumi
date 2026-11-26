# Responsive Layout & Geometry

[Docs Home](../README.md) / [Design](README.md) / Responsive Layout

---

- **Minimum Usable Window:** `940×650` logical pixels (default launch `1440×900`).
- **Single Layout Calculation:** `calculate_layout` (`crates/shellumi-app/src/ui/layout.rs`) deducts sidebar and inspector widths once from the measured workspace container (`gpui::canvas` bounds sensor in `app_render.rs`), never via hardcoded window subtractions.
- **Top Bar & Tab Overflow:** Quick Connect flexes between `180px` and `460px`; excess open tabs collapse gracefully into a `+N more` anchored overflow popover.
- **Fluid Host Inventory:** Host List/Grid columns keep Name and Connect reachable at `940px` while collapsing secondary tag columns when narrow.
- **Centralized Layout Invalidation:** `Shellumi::invalidate_layout()` advances `layout_epoch` and clears stale `rd_canvas_measurements` only when window bounds, scale/DPI, sidebar collapse, inspector open/close/width/presentation, tab/page selection, or fullscreen state actually change.

//! Native GPUI remote desktop canvas renderer directly consuming shared framebuffer frames.

use super::*;
use gpui::{ClickEvent, ObjectFit, RenderImage};
use shellumi_remote_desktop::{CanvasTransform, FramebufferSnapshot, PanState, ScaleMode};
use std::sync::Arc;

/// Convert a thread-safe `FramebufferSnapshot` into a GPUI `RenderImage`.
#[allow(dead_code)]
pub fn create_render_image_from_snapshot(
    snapshot: &FramebufferSnapshot,
) -> Option<Arc<RenderImage>> {
    if snapshot.width == 0 || snapshot.height == 0 {
        return None;
    }

    let raw_bytes = snapshot.pixels.to_vec();
    let buffer = image::RgbaImage::from_raw(snapshot.width, snapshot.height, raw_bytes)?;
    let frame = image::Frame::new(buffer);
    let render_image = RenderImage::new(smallvec::smallvec![frame]);
    Some(Arc::new(render_image))
}

impl Shellumi {
    /// Render the remote desktop canvas element for an active tab.
    pub(crate) fn render_remote_desktop_canvas(
        &self,
        tab_idx: usize,
        state: &RemoteDesktopState,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let tab_id = self
            .tabs
            .get(tab_idx)
            .map(|t| t.id)
            .unwrap_or(tab_idx as u64);
        let layout_epoch = self.layout_epoch;

        let view = cx.entity().downgrade();
        let bounds_sensor = gpui::canvas(
            move |bounds, _window, cx| {
                let w = f32::from(bounds.size.width);
                let h = f32::from(bounds.size.height);
                if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
                    return;
                }
                cx.defer(move |cx| {
                    let _ = view.update(cx, |app, cx| {
                        if app.layout_epoch != layout_epoch {
                            return;
                        }
                        let is_active_tab = app
                            .active_tab_idx
                            .and_then(|idx| app.tabs.get(idx))
                            .is_some_and(|t| t.id == tab_id);
                        if !is_active_tab {
                            return;
                        }
                        let new_m = crate::CanvasMeasurement {
                            tab_id,
                            layout_epoch,
                            viewport: bounds,
                        };
                        let changed = app
                            .rd_canvas_measurements
                            .get(&tab_id)
                            .is_none_or(|prev| *prev != new_m);
                        if changed {
                            app.rd_canvas_measurements.insert(tab_id, new_m);
                            if app
                                .rd_resize_state
                                .get(&tab_id)
                                .is_some_and(|s| s.pending_manual_recalc)
                            {
                                app.remote_desktop_match_viewport(tab_id, cx);
                            } else {
                                app.schedule_rd_auto_resize(tab_id, cx);
                            }
                            cx.notify();
                        }
                    });
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();

        if let Some(render_image) = &state.render_image {
            let image_source: gpui::ImageSource = render_image.clone().into();

            // Use actual displayed framebuffer dimensions, not requested state.width/height.
            let img_size = render_image.size(0);
            let fb_width = (img_size.width.0.max(0) as u32).max(1);
            let fb_height = (img_size.height.0.max(0) as u32).max(1);
            let scale_mode = match state.scale_mode {
                RemoteDesktopScaleMode::Fit => ScaleMode::Fit,
                RemoteDesktopScaleMode::Original => ScaleMode::Original,
                RemoteDesktopScaleMode::Fill => ScaleMode::Fill,
            };
            let pan = self.rd_pan_state.get(&tab_id).copied().unwrap_or_default();

            let measurement = self
                .rd_canvas_measurements
                .get(&tab_id)
                .filter(|m| m.tab_id == tab_id && m.layout_epoch == layout_epoch);

            let transform = measurement.and_then(|m| {
                CanvasTransform::new(
                    (
                        f64::from(f32::from(m.viewport.origin.x)),
                        f64::from(f32::from(m.viewport.origin.y)),
                    ),
                    (
                        f64::from(f32::from(m.viewport.size.width)),
                        f64::from(f32::from(m.viewport.size.height)),
                    ),
                    (fb_width, fb_height),
                    scale_mode,
                    (pan.x, pan.y),
                )
            });

            let reconnect_overlay = if let RemoteDesktopStatus::Reconnecting {
                attempt,
                max_attempts,
            } = state.status
            {
                Some(
                    div()
                        .absolute()
                        .top(px(16.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_4()
                        .py_2()
                        .rounded_lg()
                        .bg(rgba(0x1a1200dd))
                        .border_1()
                        .border_color(rgb(0xf2a900))
                        .shadow_lg()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(0xf2a900))
                                .child(format!(
                                    "⚡ Connection interrupted. Reconnecting (attempt {attempt}/{max_attempts})…"
                                )),
                        ),
                )
            } else {
                None
            };

            // Pan controls in Original mode when frame exceeds viewport on either axis
            let pan_overlay = if scale_mode == ScaleMode::Original {
                if let Some(tx) = transform {
                    let overflow_x = (fb_width as f64 - tx.viewport_size.0).max(0.0);
                    let overflow_y = (fb_height as f64 - tx.viewport_size.1).max(0.0);
                    if overflow_x > 1.0 || overflow_y > 1.0 {
                        let vp_size = tx.viewport_size;
                        Some(
                            div()
                                .id(("rd-pan-bar", tab_id))
                                .absolute()
                                .bottom(px(12.0))
                                .right(px(12.0))
                                .flex()
                                .items_center()
                                .gap_1()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgba(0x18181add))
                                .border_1()
                                .border_color(rgb(MacTheme::BORDER))
                                .on_click(cx.listener(|_, _, _, cx| cx.stop_propagation()))
                                .child(
                                    div()
                                        .id(("rd-pan-left", tab_id))
                                        .px_1p5()
                                        .py_0p5()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            let cur = app
                                                .rd_pan_state
                                                .get(&tab_id)
                                                .copied()
                                                .unwrap_or_default();
                                            let next = PanState {
                                                x: cur.x - 120.0,
                                                y: cur.y,
                                            }
                                            .clamped(vp_size, (fb_width, fb_height));
                                            app.rd_pan_state.insert(tab_id, next);
                                            cx.notify();
                                        }))
                                        .child("←"),
                                )
                                .child(
                                    div()
                                        .id(("rd-pan-up", tab_id))
                                        .px_1p5()
                                        .py_0p5()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            let cur = app
                                                .rd_pan_state
                                                .get(&tab_id)
                                                .copied()
                                                .unwrap_or_default();
                                            let next = PanState {
                                                x: cur.x,
                                                y: cur.y - 120.0,
                                            }
                                            .clamped(vp_size, (fb_width, fb_height));
                                            app.rd_pan_state.insert(tab_id, next);
                                            cx.notify();
                                        }))
                                        .child("↑"),
                                )
                                .child(
                                    div()
                                        .id(("rd-pan-down", tab_id))
                                        .px_1p5()
                                        .py_0p5()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            let cur = app
                                                .rd_pan_state
                                                .get(&tab_id)
                                                .copied()
                                                .unwrap_or_default();
                                            let next = PanState {
                                                x: cur.x,
                                                y: cur.y + 120.0,
                                            }
                                            .clamped(vp_size, (fb_width, fb_height));
                                            app.rd_pan_state.insert(tab_id, next);
                                            cx.notify();
                                        }))
                                        .child("↓"),
                                )
                                .child(
                                    div()
                                        .id(("rd-pan-right", tab_id))
                                        .px_1p5()
                                        .py_0p5()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            let cur = app
                                                .rd_pan_state
                                                .get(&tab_id)
                                                .copied()
                                                .unwrap_or_default();
                                            let next = PanState {
                                                x: cur.x + 120.0,
                                                y: cur.y,
                                            }
                                            .clamped(vp_size, (fb_width, fb_height));
                                            app.rd_pan_state.insert(tab_id, next);
                                            cx.notify();
                                        }))
                                        .child("→"),
                                ),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let mut viewport_container = div()
                .id(("rd-canvas-viewport", tab_id))
                .relative()
                .w_full()
                .h_full()
                .overflow_hidden()
                .bg(rgb(0x0c0f14))
                .cursor_pointer()
                .on_click(cx.listener(move |app, event: &ClickEvent, _window, cx| {
                    let Some(pos) = event.mouse_position() else {
                        return;
                    };
                    let Some(cur_tab_idx) = app.tabs.iter().position(|t| t.id == tab_id) else {
                        return;
                    };
                    let Some(cur_rd) = app.tabs[cur_tab_idx].remote_desktop_state.as_ref() else {
                        return;
                    };
                    if cur_rd.view_only {
                        return;
                    }
                    let Some(m) = app
                        .rd_canvas_measurements
                        .get(&tab_id)
                        .filter(|m| m.tab_id == tab_id && m.layout_epoch == app.layout_epoch)
                    else {
                        // Never fall back to estimated geometry when measurement is missing or stale.
                        return;
                    };
                    let Some(img) = cur_rd.render_image.as_ref() else {
                        return;
                    };
                    let sz = img.size(0);
                    let cur_fw = sz.width.0.max(0) as u32;
                    let cur_fh = sz.height.0.max(0) as u32;
                    let cur_mode = match cur_rd.scale_mode {
                        RemoteDesktopScaleMode::Fit => ScaleMode::Fit,
                        RemoteDesktopScaleMode::Original => ScaleMode::Original,
                        RemoteDesktopScaleMode::Fill => ScaleMode::Fill,
                    };
                    let cur_pan = app.rd_pan_state.get(&tab_id).copied().unwrap_or_default();
                    let Some(tx) = CanvasTransform::new(
                        (
                            f64::from(f32::from(m.viewport.origin.x)),
                            f64::from(f32::from(m.viewport.origin.y)),
                        ),
                        (
                            f64::from(f32::from(m.viewport.size.width)),
                            f64::from(f32::from(m.viewport.size.height)),
                        ),
                        (cur_fw, cur_fh),
                        cur_mode,
                        (cur_pan.x, cur_pan.y),
                    ) else {
                        return;
                    };

                    if let Some((remote_x, remote_y)) =
                        tx.map_window_point_to_remote(pos.x.to_f64(), pos.y.to_f64())
                    {
                        app.remote_desktop_click_canvas(cur_tab_idx, remote_x, remote_y, cx);
                    }
                }))
                .child(bounds_sensor);

            if let Some(tx) = transform {
                viewport_container = viewport_container.child(
                    gpui::img(image_source)
                        .absolute()
                        .left(px(tx.image_origin.0 as f32))
                        .top(px(tx.image_origin.1 as f32))
                        .w(px(tx.image_size.0 as f32))
                        .h(px(tx.image_size.1 as f32))
                        .object_fit(ObjectFit::Fill),
                );
            }

            viewport_container = viewport_container
                .children(reconnect_overlay)
                .children(pan_overlay);

            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(viewport_container)
        } else {
            // Awaiting genuine framebuffer frames from background protocol task;
            // still mount the bounds sensor so Match Viewport / auto-resize can measure.
            let placeholder_container = div()
                .id(("rd-canvas-placeholder", tab_id))
                .relative()
                .w_full()
                .h_full()
                .bg(rgb(0x0c0f14))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .child(bounds_sensor)
                .child(div().text_2xl().child("📡"))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                        .child("Connected to Remote Desktop Server"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Awaiting native framebuffer stream from protocol worker."),
                );

            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(placeholder_container)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shellumi_remote_desktop::{translate_coordinates, Framebuffer, Rect};

    #[test]
    fn verifies_deterministic_test_frame_converts_to_render_image() {
        let mut fb = Framebuffer::new(64, 64).unwrap();
        // Draw test pattern: top-left 32x32 blue, bottom-right 32x32 red
        fb.fill_rect(Rect::new(0, 0, 32, 32), 0, 0, 255, 255)
            .unwrap();
        fb.fill_rect(Rect::new(32, 32, 32, 32), 255, 0, 0, 255)
            .unwrap();

        let snapshot = fb.snapshot();
        assert_eq!(snapshot.width, 64);
        assert_eq!(snapshot.height, 64);

        let render_image = create_render_image_from_snapshot(&snapshot);
        assert!(render_image.is_some());
        let img = render_image.unwrap();
        assert_eq!(img.frame_count(), 1);
        let bytes = img.as_bytes(0).expect("frame bytes should exist");
        assert_eq!(bytes.len(), 64 * 64 * 4);
        // Verify pixel color in raw bytes
        // Top-left pixel (0, 0) should be blue (0, 0, 255, 255)
        assert_eq!(&bytes[0..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn test_rd_006_letterbox_excludes_outside_points() {
        // Framebuffer 1920x1080 (16:9) placed in an 800x800 square container
        // Rendered size in Fit mode is 800 x 450.
        // Letterbox offset_y = (800 - 450) / 2 = 175.0.
        let viewport_w = 800.0;
        let viewport_h = 800.0;
        let fb_w = 1920;
        let fb_h = 1080;

        // Click in top letterbox margin (y < 175.0) -> None
        assert_eq!(
            translate_coordinates(
                400.0,
                50.0,
                viewport_w,
                viewport_h,
                fb_w,
                fb_h,
                ScaleMode::Fit
            ),
            None
        );

        // Click in bottom letterbox margin (y >= 625.0) -> None
        assert_eq!(
            translate_coordinates(
                400.0,
                700.0,
                viewport_w,
                viewport_h,
                fb_w,
                fb_h,
                ScaleMode::Fit
            ),
            None
        );

        // Click dead center (400, 400) -> remote center (960, 540)
        let (rx, ry) = translate_coordinates(
            400.0,
            400.0,
            viewport_w,
            viewport_h,
            fb_w,
            fb_h,
            ScaleMode::Fit,
        )
        .expect("center click should hit remote image");
        assert_eq!(rx, 960);
        assert_eq!(ry, 540);
    }

    #[test]
    fn test_rd_007_fit_aspect_maps_edges_and_center() {
        // 1920x1080 inside 960x540 (exact 2:1 scale, zero letterbox)
        let (rx, ry) = translate_coordinates(0.0, 0.0, 960.0, 540.0, 1920, 1080, ScaleMode::Fit)
            .expect("top-left should hit");
        assert_eq!((rx, ry), (0, 0));

        let (rx, ry) =
            translate_coordinates(480.0, 270.0, 960.0, 540.0, 1920, 1080, ScaleMode::Fit)
                .expect("center should hit");
        assert_eq!((rx, ry), (960, 540));
    }

    #[test]
    fn test_rd_008_fill_stretch_maps_non_uniform() {
        // Fill stretches horizontally by 0.5 (960/1920) and vertically by 0.25 (270/1080)
        let (rx, ry) =
            translate_coordinates(480.0, 135.0, 960.0, 270.0, 1920, 1080, ScaleMode::Fill)
                .expect("fill should hit");
        assert_eq!((rx, ry), (960, 540));
    }

    #[test]
    fn test_rd_010_original_centering_and_outside() {
        // 800x600 remote inside 1200x900 viewport in 100% (Original) mode
        // Centering offset_x = (1200 - 800) / 2 = 200.
        // Centering offset_y = (900 - 600) / 2 = 150.
        // Click outside left -> None
        assert_eq!(
            translate_coordinates(100.0, 450.0, 1200.0, 900.0, 800, 600, ScaleMode::Original),
            None
        );
        // Click at offset_x + 10, offset_y + 10 -> (10, 10)
        let (rx, ry) =
            translate_coordinates(210.0, 160.0, 1200.0, 900.0, 800, 600, ScaleMode::Original)
                .expect("inside should hit");
        assert_eq!((rx, ry), (10, 10));
    }

    #[test]
    fn test_rd_011_canvas_offset_transformation() {
        let sidebar_w: f32 = 184.0;
        let top_offset: f32 = 48.0 + 36.0; // topbar + compact remote desktop toolbar (<40px)
        let global_cursor_x: f32 = 184.0 + 400.0;
        let global_cursor_y: f32 = 84.0 + 400.0;

        let canvas_local_x = (global_cursor_x - sidebar_w).max(0.0);
        let canvas_local_y = (global_cursor_y - top_offset).max(0.0);

        assert_eq!(canvas_local_x, 400.0);
        assert_eq!(canvas_local_y, 400.0);
    }
}

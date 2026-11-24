//! Native GPUI remote desktop canvas renderer directly consuming shared framebuffer frames.

use super::*;
use gpui::{ClickEvent, ObjectFit, RenderImage};
use shellumi_remote_desktop::{translate_coordinates, FramebufferSnapshot, ScaleMode};
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
        if let Some(render_image) = &state.render_image {
            let image_source: gpui::ImageSource = render_image.clone().into();
            let object_fit = match state.scale_mode {
                RemoteDesktopScaleMode::Fit => ObjectFit::Contain,
                RemoteDesktopScaleMode::Original => ObjectFit::None,
                RemoteDesktopScaleMode::Fill => ObjectFit::Fill,
            };

            let fb_width = state.width;
            let fb_height = state.height;
            let scale_mode = match state.scale_mode {
                RemoteDesktopScaleMode::Fit => ScaleMode::Fit,
                RemoteDesktopScaleMode::Original => ScaleMode::Original,
                RemoteDesktopScaleMode::Fill => ScaleMode::Fill,
            };

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

            let viewport_container = div()
                .id(("rd-canvas-viewport", tab_idx))
                .relative()
                .w_full()
                .h_full()
                .overflow_hidden()
                .bg(rgb(0x0c0f14))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(cx.listener(move |app, event: &ClickEvent, window, cx| {
                    if let Some(pos) = event.mouse_position() {
                        let vp = window.viewport_size();
                        let total_w = vp.width.to_f64() as f32;
                        let total_h = vp.height.to_f64() as f32;

                        let sidebar_w = if app.sidebar_visible {
                            if app.sidebar_collapsed {
                                56.0
                            } else {
                                crate::theme::MacTheme::SIDEBAR_WIDTH
                            }
                        } else {
                            0.0
                        };
                        let inspector_open = app.inspector_coordinator.is_open();
                        let geom = crate::ui::layout::calculate_layout(
                            total_w,
                            sidebar_w,
                            inspector_open,
                            None,
                        );
                        let topbar_h: f32 = 48.0;
                        let toolbar_h: f32 = 36.0;

                        let canvas_w = geom.primary_content_width.max(100.0);
                        let canvas_h = (total_h - topbar_h - toolbar_h).max(100.0);

                        let origin_x = sidebar_w;
                        let origin_y = topbar_h + toolbar_h;

                        let screen_x = (pos.x.to_f64() as f32 - origin_x).max(0.0);
                        let screen_y = (pos.y.to_f64() as f32 - origin_y).max(0.0);

                        if let Some((remote_x, remote_y)) = translate_coordinates(
                            screen_x, screen_y, canvas_w, canvas_h, fb_width, fb_height, scale_mode,
                        ) {
                            app.remote_desktop_click_canvas(tab_idx, remote_x, remote_y, cx);
                        }
                    }
                }))
                .child(
                    div()
                        .w_full()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            gpui::img(image_source)
                                .w_full()
                                .h_full()
                                .object_fit(object_fit),
                        ),
                )
                .children(reconnect_overlay);

            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(viewport_container)
        } else {
            // Awaiting genuine framebuffer frames from background protocol task
            let placeholder_container = div()
                .id(("rd-canvas-placeholder", tab_idx))
                .w_full()
                .h_full()
                .bg(rgb(0x0c0f14))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
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
    use shellumi_remote_desktop::{Framebuffer, Rect};

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

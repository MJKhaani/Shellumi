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
                .on_click(cx.listener(move |app, event: &ClickEvent, _window, cx| {
                    if let Some(pos) = event.mouse_position() {
                        // Viewport width and height approximate container or 1920x1080 display
                        let screen_x: f32 = pos.x.into();
                        let screen_y: f32 = pos.y.into();

                        if let Some((remote_x, remote_y)) = translate_coordinates(
                            screen_x,
                            screen_y,
                            fb_width as f32,
                            fb_height as f32,
                            fb_width,
                            fb_height,
                            scale_mode,
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
                );

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
}

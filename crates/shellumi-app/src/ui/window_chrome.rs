//! Cross-platform window chrome, titlebar options, and client fallback controls.
//!
//! macOS uses a transparent titlebar with native traffic lights.
//! Windows and Linux prefer native server-side decorations (`WindowDecorations::Server`),
//! with a minimal client-side control strip rendered *only* when a compositor (e.g. Wayland)
//! forces `Decorations::Client`.

#[cfg(any(target_os = "macos", test))]
use gpui::point;
use gpui::{
    div, prelude::*, px, rgb, size, Bounds, Context, FontWeight, MouseButton, ParentElement,
    Pixels, SharedString, Styled, TitlebarOptions, Window, WindowBounds, WindowControlArea,
    WindowOptions,
};
#[cfg(not(target_os = "macos"))]
use gpui::{Decorations, WindowDecorations};

use crate::theme::MacTheme;
use crate::ui::layout::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH};
use crate::Shellumi;

/// Active window decoration mode for the current window.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeMode {
    /// macOS transparent titlebar with native traffic lights.
    MacTransparent,
    /// Native OS-decorated window (Windows / Linux with server-side decorations).
    NativeDecorated,
    /// Client-side fallback controls (only when compositor forces `Decorations::Client`).
    ClientFallback,
}

/// Measured chrome space consumed inside the top bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeMetrics {
    pub mode: ChromeMode,
    /// Logical px reserved on the left edge of the top bar (e.g. macOS traffic lights).
    pub left_reserved_width: f32,
    /// Logical px reserved on the right edge of the top bar (e.g. CSD window buttons).
    pub right_reserved_width: f32,
    pub is_fullscreen: bool,
}

impl ChromeMetrics {
    /// Compute chrome metrics from the live GPUI `Window`.
    pub fn from_window(window: &Window) -> Self {
        let is_fullscreen = window.is_fullscreen();
        #[cfg(target_os = "macos")]
        {
            // In macOS fullscreen the traffic lights live in the auto-hiding menu bar,
            // so reserving 80px in the top bar would leave a phantom gap.
            Self {
                mode: ChromeMode::MacTransparent,
                left_reserved_width: if is_fullscreen { 12.0 } else { 80.0 },
                right_reserved_width: 0.0,
                is_fullscreen,
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if is_fullscreen {
                return Self {
                    mode: ChromeMode::NativeDecorated,
                    left_reserved_width: 12.0,
                    right_reserved_width: 0.0,
                    is_fullscreen,
                };
            }
            match window.window_decorations() {
                Decorations::Server => Self {
                    mode: ChromeMode::NativeDecorated,
                    left_reserved_width: 12.0,
                    right_reserved_width: 0.0,
                    is_fullscreen,
                },
                Decorations::Client { .. } => {
                    let controls = window.window_controls();
                    let mut btn_count = 1usize; // close is always present
                    if controls.minimize {
                        btn_count += 1;
                    }
                    if controls.maximize {
                        btn_count += 1;
                    }
                    Self {
                        mode: ChromeMode::ClientFallback,
                        left_reserved_width: 12.0,
                        right_reserved_width: (btn_count as f32) * 32.0 + 8.0,
                        is_fullscreen,
                    }
                }
            }
        }
    }

    /// Total horizontal chrome width consumed inside the top bar.
    pub fn total_reserved_width(&self) -> f32 {
        self.left_reserved_width + self.right_reserved_width
    }
}

/// Build platform-appropriate `TitlebarOptions`.
pub fn shellumi_titlebar(title: SharedString) -> TitlebarOptions {
    #[cfg(target_os = "macos")]
    {
        TitlebarOptions {
            title: Some(title),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.0), px(14.0))),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        TitlebarOptions {
            title: Some(title),
            appears_transparent: false,
            traffic_light_position: None,
        }
    }
}

/// Unified `WindowOptions` factory for all Shellumi application windows.
pub fn shellumi_window_options(
    title: impl Into<SharedString>,
    bounds: Bounds<Pixels>,
) -> WindowOptions {
    #[cfg(target_os = "macos")]
    let window_decorations = None;
    #[cfg(not(target_os = "macos"))]
    let window_decorations = Some(WindowDecorations::Server);

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(MIN_WINDOW_WIDTH), px(MIN_WINDOW_HEIGHT))),
        titlebar: Some(shellumi_titlebar(title.into())),
        window_decorations,
        app_id: Some("dev.shellumi.app".to_string()),
        ..Default::default()
    }
}

/// Render client-side window control buttons *only* when `ChromeMode::ClientFallback` is active.
pub(crate) fn render_client_window_controls(
    window: &Window,
    cx: &mut Context<Shellumi>,
) -> Option<gpui::Div> {
    let metrics = ChromeMetrics::from_window(window);
    if metrics.mode != ChromeMode::ClientFallback {
        return None;
    }

    let controls = window.window_controls();
    let is_maximized = window.is_maximized();
    let mut row = div().flex().items_center().gap_1().ml_2().flex_shrink_0();

    if controls.minimize {
        row = row.child(
            div()
                .id("chrome-minimize-btn")
                .window_control_area(WindowControlArea::Min)
                .w(px(28.0))
                .h(px(26.0))
                .rounded_md()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .cursor_pointer()
                .hover(|s| {
                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                })
                .on_click(|_, window, _| {
                    window.minimize_window();
                })
                .child("—"),
        );
    }

    if controls.maximize {
        row = row.child(
            div()
                .id("chrome-maximize-btn")
                .window_control_area(WindowControlArea::Max)
                .w(px(28.0))
                .h(px(26.0))
                .rounded_md()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_SECONDARY))
                .cursor_pointer()
                .hover(|s| {
                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                })
                .on_click(|_, window, _| {
                    window.zoom_window();
                })
                .child(if is_maximized { "❐" } else { "□" }),
        );
    }

    row = row.child(
        div()
            .id("chrome-close-btn")
            .window_control_area(WindowControlArea::Close)
            .w(px(28.0))
            .h(px(26.0))
            .rounded_md()
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(MacTheme::TEXT_SECONDARY))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::DANGER)).text_color(rgb(0xffffff)))
            .on_click(cx.listener(|app, _, window, cx| {
                if app.handle_window_close_request(window, cx) {
                    app.approved_close_once = true;
                    window.remove_window();
                }
            }))
            .child("✕"),
    );

    Some(row)
}

/// Attach drag/double-click behavior to the topbar background when using transparent or CSD chrome.
pub(crate) fn configure_topbar_drag(
    bar: gpui::Stateful<gpui::Div>,
    mode: ChromeMode,
) -> gpui::Stateful<gpui::Div> {
    match mode {
        ChromeMode::MacTransparent | ChromeMode::ClientFallback => bar
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, |event, window, _| {
                if event.click_count >= 2 {
                    window.zoom_window();
                } else if !cfg!(target_os = "macos") {
                    window.start_window_move();
                }
            }),
        ChromeMode::NativeDecorated => bar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_options_enforces_minimum_size_and_platform_titlebar() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(1440.0), px(900.0)));
        let opts = shellumi_window_options("Shellumi — Personal", bounds);
        assert_eq!(
            opts.window_min_size,
            Some(size(px(MIN_WINDOW_WIDTH), px(MIN_WINDOW_HEIGHT)))
        );
        let titlebar = opts.titlebar.expect("titlebar options must be set");
        assert_eq!(
            titlebar.title.as_ref().map(|s| s.as_ref()),
            Some("Shellumi — Personal")
        );
        #[cfg(target_os = "macos")]
        {
            assert!(titlebar.appears_transparent);
            assert!(titlebar.traffic_light_position.is_some());
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert!(!titlebar.appears_transparent);
            assert!(titlebar.traffic_light_position.is_none());
            assert_eq!(opts.window_decorations, Some(WindowDecorations::Server));
        }
    }
}

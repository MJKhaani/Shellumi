#![allow(dead_code, clippy::new_ret_no_self)]
//! Reusable InspectorShell container providing stable header, scrollable body, and sticky footer.

use crate::theme::{sidebar_bg, text_primary, text_secondary, MacTheme};
use gpui::{
    div, prelude::*, px, rgb, AnyElement, Div, ElementId, IntoElement, ParentElement, SharedString,
};

/// The primary shell container for the Unified Inspector.
pub struct InspectorShell;

impl InspectorShell {
    /// Create an inspector container.
    ///
    /// # Arguments
    /// * `header` - The sticky top header element.
    /// * `body` - The flexible, scrollable content region.
    /// * `footer` - The sticky bottom action bar.
    /// * `width` - Optional explicit width in logical pixels (defaults to full width of parent container).
    pub fn new(
        header: impl IntoElement,
        body: impl IntoElement,
        footer: Option<impl IntoElement>,
        width: Option<f32>,
    ) -> Div {
        let mut shell = div()
            .flex()
            .flex_col()
            .h_full()
            .bg(sidebar_bg())
            .border_l_1()
            .border_color(rgb(MacTheme::SEPARATOR));

        if let Some(w) = width {
            shell = shell.w(px(w)).min_w(px(w)).max_w(px(w));
        } else {
            shell = shell.w_full();
        }

        shell = shell.child(header);

        // Body is flexible and independently scrollable (requires element id for scroll state)
        let scrollable_body = div()
            .id("inspector-body-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_4()
            .child(body);

        shell = shell.child(scrollable_body);

        if let Some(f) = footer {
            shell = shell.child(f);
        }

        shell
    }

    /// Build a standard inspector header with title, subtitle/badge, actions, and close button.
    pub fn header(
        title: impl Into<SharedString>,
        subtitle: Option<impl Into<SharedString>>,
        badge: Option<AnyElement>,
        actions: Option<AnyElement>,
        close_id: impl Into<ElementId>,
        is_back: bool,
    ) -> Div {
        let close_icon_glyph = if is_back { "←" } else { "✕" };

        let close_btn = div()
            .id(close_id)
            .w(px(28.0))
            .h(px(28.0))
            .rounded(px(MacTheme::CONTROL_RADIUS))
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(text_secondary())
            .cursor_pointer()
            .hover(|s| {
                s.bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_color(text_primary())
            })
            .child(close_icon_glyph);

        let mut left_content = div().flex().items_center().gap_2().min_w_0();

        if let Some(b) = badge {
            left_content = left_content.child(b);
        }

        let mut title_column = div().flex().flex_col().min_w_0();
        title_column = title_column.child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(text_primary())
                .overflow_hidden()
                .text_ellipsis()
                .child(title.into()),
        );

        if let Some(sub) = subtitle {
            title_column = title_column.child(
                div()
                    .text_xs()
                    .text_color(text_secondary())
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(sub.into()),
            );
        }

        left_content = left_content.child(title_column);

        let mut right_content = div().flex().items_center().gap_1();
        if let Some(a) = actions {
            right_content = right_content.child(a);
        }
        right_content = right_content.child(close_btn);

        div()
            .h(px(52.0))
            .px_4()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .bg(rgb(0x202023))
            .child(left_content)
            .child(right_content)
    }

    /// Build a standard inspector footer with primary and secondary action buttons.
    pub fn footer(
        primary_action: impl IntoElement,
        secondary_action: Option<impl IntoElement>,
        extra_left: Option<AnyElement>,
    ) -> Div {
        let mut left = div().flex().items_center().gap_2();
        if let Some(el) = extra_left {
            left = left.child(el);
        }

        let mut right = div().flex().items_center().gap_2();
        if let Some(sec) = secondary_action {
            right = right.child(sec);
        }
        right = right.child(primary_action);

        div()
            .h(px(56.0))
            .px_4()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .flex()
            .items_center()
            .justify_between()
            .bg(rgb(0x202023))
            .child(left)
            .child(right)
    }
}

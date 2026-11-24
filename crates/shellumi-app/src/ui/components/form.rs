#![allow(dead_code)]
//! Standard form controls and structured field containers.

use crate::theme::{danger, input_bg, text_primary, text_secondary, warning, MacTheme};
use gpui::{
    div, prelude::*, px, rgb, AnyElement, Div, ElementId, IntoElement, ParentElement, SharedString,
    Stateful,
};

/// A logical form section inside an inspector or full-page editor.
pub fn form_section(
    title: impl Into<SharedString>,
    description: Option<impl Into<SharedString>>,
    content: impl IntoElement,
) -> Div {
    let mut s = div().flex().flex_col().gap_2().child(
        div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(text_primary())
            .child(title.into()),
    );

    if let Some(desc) = description {
        s = s.child(
            div()
                .text_xs()
                .text_color(text_secondary())
                .child(desc.into()),
        );
    }

    s.child(content)
}

/// A simple form section with just a title and content.
pub fn simple_section(title: impl Into<SharedString>, content: impl IntoElement) -> Div {
    form_section(title, None::<SharedString>, content)
}

/// A simple form field with just a label and input widget.
pub fn simple_field(label: impl Into<SharedString>, input_widget: impl IntoElement) -> Div {
    form_field(
        label,
        false,
        input_widget,
        None::<SharedString>,
        None::<SharedString>,
    )
}

/// A structured form field container with label, required indicator, input widget, and optional error.
pub fn form_field(
    label: impl Into<SharedString>,
    required: bool,
    input_widget: impl IntoElement,
    error: Option<impl Into<SharedString>>,
    helper: Option<impl Into<SharedString>>,
) -> Div {
    let mut field = div().flex().flex_col().gap_1();

    let mut label_row = div()
        .flex()
        .items_center()
        .gap_1()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(text_secondary())
        .child(label.into());

    if required {
        label_row = label_row.child(div().text_color(danger()).child("*"));
    }

    field = field.child(label_row);
    field = field.child(input_widget);

    if let Some(err) = error {
        field = field.child(validation_message(err));
    } else if let Some(help) = helper {
        field = field.child(
            div()
                .text_xs()
                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                .child(help.into()),
        );
    }

    field
}

/// An inline validation error message.
pub fn validation_message(message: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .gap_1()
        .text_xs()
        .text_color(danger())
        .child(message.into())
}

/// A standard toggle switch element.
pub fn toggle_switch(
    id: impl Into<ElementId>,
    enabled: bool,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    let bg_color = if enabled {
        crate::theme::accent_blue()
    } else {
        rgb(MacTheme::SEPARATOR)
    };

    let circle_margin = if enabled { px(18.0) } else { px(2.0) };

    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .py_1()
        .cursor_pointer()
        .child(
            div()
                .text_sm()
                .text_color(text_primary())
                .child(label.into()),
        )
        .child(
            div()
                .w(px(38.0))
                .h(px(22.0))
                .rounded_full()
                .bg(bg_color)
                .flex()
                .items_center()
                .child(
                    div()
                        .w(px(18.0))
                        .h(px(18.0))
                        .rounded_full()
                        .bg(rgb(0xffffff))
                        .ml(circle_margin),
                ),
        )
}

/// An inline error notice with error styling.
pub fn error_notice(message: impl Into<SharedString>) -> Div {
    inline_notice(None, message, false)
}

/// An inline notice or warning card within an inspector or form.
pub fn inline_notice(
    icon: Option<AnyElement>,
    message: impl Into<SharedString>,
    is_warning: bool,
) -> Div {
    let (bg, border, fg) = if is_warning {
        (rgb(0x332800), warning(), warning())
    } else {
        (input_bg(), rgb(MacTheme::SEPARATOR), text_secondary())
    };

    let mut el = div()
        .flex()
        .items_center()
        .gap_2()
        .p_2p5()
        .rounded(px(MacTheme::CONTROL_RADIUS))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_xs()
        .text_color(fg);

    if let Some(i) = icon {
        el = el.child(i);
    }

    el.child(message.into())
}

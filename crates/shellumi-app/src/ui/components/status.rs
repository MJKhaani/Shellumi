#![allow(dead_code)]
//! Status indicators, empty states, and error presentations.

use crate::theme::{danger, success, text_primary, text_secondary, warning, MacTheme};
use gpui::{div, px, rgb, AnyElement, Div, IntoElement, ParentElement, SharedString, Styled};

/// Categorized status state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Online,
    Connecting,
    Reconnecting,
    Offline,
    Warning,
    Error,
}

/// A compact status dot with optional label.
pub fn status_indicator(kind: StatusKind, label: Option<impl Into<SharedString>>) -> Div {
    let dot_color = match kind {
        StatusKind::Online => success(),
        StatusKind::Connecting | StatusKind::Reconnecting => warning(),
        StatusKind::Offline => rgb(MacTheme::TEXT_TERTIARY),
        StatusKind::Warning => warning(),
        StatusKind::Error => danger(),
    };

    let mut row = div().flex().items_center().gap_1p5();

    row = row.child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(dot_color));

    if let Some(l) = label {
        row = row.child(div().text_xs().text_color(text_secondary()).child(l.into()));
    }

    row
}

/// A friendly empty state message when a list, section, or search has no items.
pub fn empty_state(
    icon: Option<impl Into<AnyElement>>,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    action: Option<impl IntoElement>,
) -> Div {
    let mut container = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .py_12()
        .px_4()
        .gap_2()
        .text_center();

    if let Some(i) = icon {
        container = container.child(
            div()
                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                .mb_2()
                .child(i.into()),
        );
    }

    container = container.child(
        div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(text_primary())
            .child(title.into()),
    );

    container = container.child(
        div()
            .text_xs()
            .text_color(text_secondary())
            .max_w(px(280.0))
            .child(description.into()),
    );

    if let Some(act) = action {
        container = container.child(div().mt_3().child(act));
    }

    container
}

/// A structured error state presentation with retry action.
pub fn error_state(
    error_message: impl Into<SharedString>,
    retry_action: Option<impl IntoElement>,
) -> Div {
    let mut container = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .p_6()
        .gap_3()
        .rounded(px(MacTheme::PANEL_RADIUS))
        .bg(rgb(0x281919))
        .border_1()
        .border_color(danger())
        .text_center();

    container = container.child(
        div()
            .text_sm()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(danger())
            .child("Something went wrong"),
    );

    container = container.child(
        div()
            .text_xs()
            .text_color(text_secondary())
            .child(error_message.into()),
    );

    if let Some(retry) = retry_action {
        container = container.child(retry);
    }

    container
}

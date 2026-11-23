#![allow(dead_code, clippy::new_ret_no_self)]
//! Standard button primitives adhering to macOS Dark theme tokens.

use crate::theme::{accent_blue, accent_blue_hover, danger, surface, surface_hover, MacTheme};
use gpui::{
    div, prelude::*, px, rgb, AnyElement, Context, Div, ElementId, IntoElement, ParentElement,
    Render, SharedString, Stateful, Window,
};

/// A reusable tooltip component for icon buttons and actions.
pub struct AppTooltip(pub SharedString);

impl Render for AppTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(MacTheme::SURFACE_HOVER))
            .text_xs()
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .child(self.0.clone())
    }
}

/// Visual styling variants for buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// High-contrast primary action button (System Blue).
    Primary,
    /// Standard secondary action button (Surface Gray).
    Secondary,
    /// Dangerous destructive action button (Danger Red).
    Danger,
    /// Borderless subtle button for inline toolbars.
    Ghost,
}

/// Sizing scales for buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Sm,
    Md,
    Lg,
}

/// A standard design-system button.
pub struct AppButton;

impl AppButton {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        variant: ButtonVariant,
    ) -> Stateful<Div> {
        Self::sized(id, label, variant, ButtonSize::Md)
    }

    pub fn sized(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        variant: ButtonVariant,
        size: ButtonSize,
    ) -> Stateful<Div> {
        let (py, px_val, font_size, height) = match size {
            ButtonSize::Sm => (1.0, 2.0, gpui::rems(0.75), 26.0),
            ButtonSize::Md => (1.5, 3.0, gpui::rems(0.875), 32.0),
            ButtonSize::Lg => (2.0, 4.0, gpui::rems(1.0), 38.0),
        };

        let mut el = div()
            .id(id)
            .h(px(height))
            .py(px(py * 4.0))
            .px(px(px_val * 4.0))
            .rounded(px(MacTheme::CONTROL_RADIUS))
            .text_size(font_size)
            .font_weight(gpui::FontWeight::MEDIUM)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(label.into());

        match variant {
            ButtonVariant::Primary => {
                el = el
                    .bg(accent_blue())
                    .text_color(rgb(0xffffff))
                    .border_1()
                    .border_color(accent_blue())
                    .hover(|s| s.bg(accent_blue_hover()));
            }
            ButtonVariant::Secondary => {
                el = el
                    .bg(surface())
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .hover(|s| s.bg(surface_hover()));
            }
            ButtonVariant::Danger => {
                el = el
                    .bg(danger())
                    .text_color(rgb(0xffffff))
                    .border_1()
                    .border_color(danger())
                    .hover(|s| s.bg(rgb(0xff6961)));
            }
            ButtonVariant::Ghost => {
                el = el
                    .bg(gpui::transparent_black())
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .hover(|s| {
                        s.bg(surface_hover())
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    });
            }
        }

        el
    }

    /// Render a disabled representation of an action button.
    pub fn disabled(label: impl Into<SharedString>, size: ButtonSize) -> Div {
        let (py, px_val, font_size, height) = match size {
            ButtonSize::Sm => (1.0, 2.0, gpui::rems(0.75), 26.0),
            ButtonSize::Md => (1.5, 3.0, gpui::rems(0.875), 32.0),
            ButtonSize::Lg => (2.0, 4.0, gpui::rems(1.0), 38.0),
        };

        div()
            .h(px(height))
            .py(px(py * 4.0))
            .px(px(px_val * 4.0))
            .rounded(px(MacTheme::CONTROL_RADIUS))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR_SUBTLE))
            .bg(rgb(0x242426))
            .text_color(rgb(MacTheme::TEXT_TERTIARY))
            .text_size(font_size)
            .flex()
            .items_center()
            .justify_center()
            .cursor_not_allowed()
            .child(label.into())
    }
}

/// A compact icon-only action button with accessible title/tooltip.
pub struct IconButton;

impl IconButton {
    pub fn new(
        id: impl Into<ElementId>,
        icon: impl Into<AnyElement>,
        tooltip: impl Into<SharedString>,
    ) -> Stateful<Div> {
        let tt: SharedString = tooltip.into();
        div()
            .id(id)
            .w(px(32.0))
            .h(px(32.0))
            .rounded(px(MacTheme::CONTROL_RADIUS))
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(MacTheme::TEXT_SECONDARY))
            .cursor_pointer()
            .hover(|s| {
                s.bg(surface_hover())
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
            })
            .tooltip(move |_, cx| cx.new(|_| AppTooltip(tt.clone())).into())
            .child(icon.into())
    }
}

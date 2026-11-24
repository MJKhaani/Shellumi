#![allow(dead_code)]
//! Anchored popovers, action menus, and dropdown primitives.

use crate::theme::{danger, separator, surface, text_primary, text_secondary, MacTheme};
use gpui::{div, prelude::*, px, rgb, AnyElement, Div, ElementId, ParentElement, SharedString};

/// Alignment anchor for popover positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopoverAnchor {
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}

/// An action menu item descriptor.
pub struct MenuItem {
    pub id: ElementId,
    pub label: SharedString,
    pub shortcut: Option<SharedString>,
    pub icon: Option<AnyElement>,
    pub is_danger: bool,
    pub is_disabled: bool,
}

impl MenuItem {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            icon: None,
            is_danger: false,
            is_disabled: false,
        }
    }

    pub fn shortcut(mut self, sc: impl Into<SharedString>) -> Self {
        self.shortcut = Some(sc.into());
        self
    }

    pub fn danger(mut self, is_danger: bool) -> Self {
        self.is_danger = is_danger;
        self
    }

    pub fn disabled(mut self, is_disabled: bool) -> Self {
        self.is_disabled = is_disabled;
        self
    }

    pub fn icon(mut self, icon: impl Into<AnyElement>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Render a standard action menu dropdown.
pub fn action_menu(items: Vec<MenuItem>, min_width: Option<f32>) -> Div {
    let mut menu = div()
        .bg(surface())
        .border_1()
        .border_color(separator())
        .rounded(px(MacTheme::PANEL_RADIUS))
        .shadow_md()
        .p_1()
        .flex()
        .flex_col()
        .gap_0p5();

    if let Some(w) = min_width {
        menu = menu.min_w(px(w));
    } else {
        menu = menu.min_w(px(160.0));
    }

    for item in items {
        if item.is_disabled {
            menu = menu.child(
                div()
                    .px_3()
                    .py_1p5()
                    .rounded(px(MacTheme::CONTROL_RADIUS))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_not_allowed()
                    .child(item.label),
            );
            continue;
        }

        let fg_color = if item.is_danger {
            danger()
        } else {
            text_primary()
        };

        let mut row = div()
            .id(item.id)
            .px_3()
            .py_1p5()
            .rounded(px(MacTheme::CONTROL_RADIUS))
            .text_xs()
            .text_color(fg_color)
            .flex()
            .items_center()
            .justify_between();

        let mut left = div().flex().items_center().gap_2();
        if let Some(ic) = item.icon {
            left = left.child(ic);
        }
        left = left.child(item.label);
        row = row.child(left);

        if let Some(sc) = item.shortcut {
            row = row.child(div().text_xs().text_color(text_secondary()).child(sc));
        }

        menu = menu.child(row);
    }

    menu
}

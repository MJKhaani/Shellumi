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

/// Computed screen-space placement for a measured popover or context menu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopoverPlacement {
    pub top: f32,
    pub left: f32,
    pub max_height: f32,
    pub flipped_vertical: bool,
}

/// Place a popover anchored to a measured trigger `Bounds<Pixels>`, clamping
/// horizontally within the window and flipping vertically if it would clip the bottom edge.
pub fn place_popover_from_trigger(
    trigger: gpui::Bounds<gpui::Pixels>,
    popover_width: f32,
    estimated_height: f32,
    window_width: f32,
    window_height: f32,
    preferred: PopoverAnchor,
) -> PopoverPlacement {
    let margin = 8.0;
    let gap = 4.0;
    let tx = f32::from(trigger.origin.x);
    let ty = f32::from(trigger.origin.y);
    let tw = f32::from(trigger.size.width);
    let th = f32::from(trigger.size.height);

    let max_left = (window_width - popover_width - margin).max(margin);
    let raw_left = match preferred {
        PopoverAnchor::BottomLeft | PopoverAnchor::TopLeft => tx,
        PopoverAnchor::BottomRight | PopoverAnchor::TopRight => tx + tw - popover_width,
    };
    let left = raw_left.clamp(margin, max_left);

    let space_below = (window_height - (ty + th + gap) - margin).max(0.0);
    let space_above = (ty - gap - margin).max(0.0);
    let prefer_top = matches!(preferred, PopoverAnchor::TopLeft | PopoverAnchor::TopRight);

    let (top, max_height, flipped_vertical) = if prefer_top {
        if space_above >= estimated_height || space_above >= space_below {
            let h = estimated_height.min(space_above).max(120.0);
            ((ty - gap - h).max(margin), space_above.max(120.0), false)
        } else {
            (ty + th + gap, space_below.max(120.0), true)
        }
    } else if space_below >= estimated_height || space_below >= space_above {
        (ty + th + gap, space_below.max(120.0), false)
    } else {
        let h = estimated_height.min(space_above).max(120.0);
        ((ty - gap - h).max(margin), space_above.max(120.0), true)
    };

    PopoverPlacement {
        top,
        left,
        max_height,
        flipped_vertical,
    }
}

/// Place a cursor-anchored context menu within window bounds, flipping vertically if needed.
pub fn place_context_menu(
    cursor_x: f32,
    cursor_y: f32,
    menu_width: f32,
    estimated_height: f32,
    window_width: f32,
    window_height: f32,
) -> PopoverPlacement {
    let margin = 8.0;
    let left = cursor_x.clamp(margin, (window_width - menu_width - margin).max(margin));
    let space_below = (window_height - cursor_y - margin).max(0.0);
    let space_above = (cursor_y - margin).max(0.0);
    if space_below >= estimated_height || space_below >= space_above {
        PopoverPlacement {
            top: cursor_y.max(margin),
            left,
            max_height: space_below.max(120.0),
            flipped_vertical: false,
        }
    } else {
        let h = estimated_height.min(space_above).max(120.0);
        PopoverPlacement {
            top: (cursor_y - h).max(margin),
            left,
            max_height: space_above.max(120.0),
            flipped_vertical: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{bounds, point, px, size};

    #[test]
    fn test_popover_clamps_right_edge_and_flips_on_bottom_overflow() {
        let trigger = bounds(point(px(1200.0), px(650.0)), size(px(120.0), px(32.0)));
        let placement = place_popover_from_trigger(
            trigger,
            240.0,
            280.0,
            1280.0,
            720.0,
            PopoverAnchor::BottomLeft,
        );
        assert!(placement.flipped_vertical);
        assert!(placement.left <= 1280.0 - 240.0 - 8.0);
        assert!(placement.top < 650.0);
    }

    #[test]
    fn test_context_menu_clamps_and_flips_near_bottom_right() {
        let placement = place_context_menu(1240.0, 680.0, 220.0, 240.0, 1280.0, 720.0);
        assert!(placement.flipped_vertical);
        assert!(placement.left <= 1280.0 - 220.0 - 8.0);
        assert!(placement.top < 680.0);
    }
}

#![allow(dead_code)]
//! Responsive layout calculation and geometry rules.
//!
//! Provides pure, testable geometry algorithms for calculating available workspace,
//! inspector presentation mode (`Side` vs `FocusedContent`), width constraints, and
//! minimum primary content guarantees across arbitrary window sizes.

/// Preferred width for the Unified Inspector panel in logical GPUI pixels.
pub const INSPECTOR_PREFERRED_WIDTH: f32 = 400.0;

/// Minimum allowable width for the side inspector before compressing content.
pub const INSPECTOR_MIN_WIDTH: f32 = 320.0;

/// Maximum width for the side inspector on ultrawide displays.
pub const INSPECTOR_MAX_WIDTH: f32 = 440.0;

/// Minimum allowable width for the primary content region (hosts grid, terminal, remote canvas)
/// to remain in side-by-side split layout.
pub const MIN_PRIMARY_CONTENT_WIDTH: f32 = 560.0;

/// Width of the application navigation rail / sidebar.
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 184.0;

/// Minimum supported application window dimensions.
pub const MIN_WINDOW_WIDTH: f32 = 940.0;
pub const MIN_WINDOW_HEIGHT: f32 = 650.0;

/// Presentation mode of the unified inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorPresentation {
    /// Inspector is docked to the right of the primary content in a split layout.
    Side,
    /// Viewport is too narrow for side-by-side display without compressing content;
    /// Inspector replaces the central workspace with an accessible Back control.
    FocusedContent,
}

/// Geometry calculation result for the main workspace.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutGeometry {
    /// Active inspector presentation mode.
    pub presentation: InspectorPresentation,
    /// Calculated width of the inspector in logical pixels (0 if closed).
    pub inspector_width: f32,
    /// Calculated width of the primary content area in logical pixels.
    pub primary_content_width: f32,
    /// Available workspace width (excluding navigation rail).
    pub available_workspace_width: f32,
}

/// Calculate the layout geometry for a window given its available bounds.
///
/// # Arguments
/// * `total_window_width` - The measured total width of the GPUI window in logical pixels.
/// * `sidebar_width` - The width consumed by the navigation sidebar / rail.
/// * `inspector_open` - Whether the inspector is currently requested to be open.
/// * `preferred_inspector_width` - Optional custom inspector width within [320.0, 440.0].
pub fn calculate_layout(
    total_window_width: f32,
    sidebar_width: f32,
    inspector_open: bool,
    preferred_inspector_width: Option<f32>,
) -> LayoutGeometry {
    let available_workspace = (total_window_width - sidebar_width).max(0.0);

    if !inspector_open {
        return LayoutGeometry {
            presentation: InspectorPresentation::Side,
            inspector_width: 0.0,
            primary_content_width: available_workspace,
            available_workspace_width: available_workspace,
        };
    }

    let target_inspector_width = preferred_inspector_width
        .unwrap_or(INSPECTOR_PREFERRED_WIDTH)
        .clamp(INSPECTOR_MIN_WIDTH, INSPECTOR_MAX_WIDTH);

    let remaining_primary = available_workspace - target_inspector_width;

    if remaining_primary >= MIN_PRIMARY_CONTENT_WIDTH {
        // We have enough room for side-by-side presentation!
        LayoutGeometry {
            presentation: InspectorPresentation::Side,
            inspector_width: target_inspector_width,
            primary_content_width: remaining_primary,
            available_workspace_width: available_workspace,
        }
    } else {
        // Check if compressing the inspector to its absolute minimum (320px) leaves >= 560px
        let compressed_remaining = available_workspace - INSPECTOR_MIN_WIDTH;
        if compressed_remaining >= MIN_PRIMARY_CONTENT_WIDTH {
            LayoutGeometry {
                presentation: InspectorPresentation::Side,
                inspector_width: INSPECTOR_MIN_WIDTH,
                primary_content_width: compressed_remaining,
                available_workspace_width: available_workspace,
            }
        } else {
            // Cannot satisfy the 560px primary content guarantee side-by-side;
            // switch to focused presentation mode!
            LayoutGeometry {
                presentation: InspectorPresentation::FocusedContent,
                inspector_width: available_workspace,
                primary_content_width: 0.0,
                available_workspace_width: available_workspace,
            }
        }
    }
}

/// Calculate optimal column count for the hosts card grid given the available primary content width.
pub fn calculate_grid_columns(content_width: f32) -> usize {
    const MIN_CARD_WIDTH: f32 = 280.0;
    const CARD_GAP: f32 = 16.0;

    if content_width <= 0.0 {
        return 1;
    }

    let cols = ((content_width + CARD_GAP) / (MIN_CARD_WIDTH + CARD_GAP)).floor() as usize;
    cols.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimum_window_width_without_inspector() {
        let geom = calculate_layout(MIN_WINDOW_WIDTH, DEFAULT_SIDEBAR_WIDTH, false, None);
        assert_eq!(geom.presentation, InspectorPresentation::Side);
        assert_eq!(geom.inspector_width, 0.0);
        assert_eq!(geom.primary_content_width, 940.0 - 184.0);
    }

    #[test]
    fn test_minimum_window_width_with_inspector_switches_to_focused() {
        // At 940px total: 940 - 184 = 756px workspace.
        // If inspector is 320px (min), 756 - 320 = 436px < 560px min primary.
        // Therefore, it MUST switch to FocusedContent!
        let geom = calculate_layout(MIN_WINDOW_WIDTH, DEFAULT_SIDEBAR_WIDTH, true, None);
        assert_eq!(geom.presentation, InspectorPresentation::FocusedContent);
        assert_eq!(geom.primary_content_width, 0.0);
        assert_eq!(geom.inspector_width, 756.0);
    }

    #[test]
    fn test_standard_laptop_1280x800_with_inspector() {
        // 1280 - 184 = 1096px workspace.
        // 1096 - 400 = 696px primary >= 560px!
        let geom = calculate_layout(1280.0, DEFAULT_SIDEBAR_WIDTH, true, None);
        assert_eq!(geom.presentation, InspectorPresentation::Side);
        assert_eq!(geom.inspector_width, 400.0);
        assert_eq!(geom.primary_content_width, 696.0);
    }

    #[test]
    fn test_full_hd_1920x1080_with_inspector() {
        // 1920 - 184 = 1736px workspace.
        // 1736 - 400 = 1336px primary.
        let geom = calculate_layout(1920.0, DEFAULT_SIDEBAR_WIDTH, true, None);
        assert_eq!(geom.presentation, InspectorPresentation::Side);
        assert_eq!(geom.inspector_width, 400.0);
        assert_eq!(geom.primary_content_width, 1336.0);
    }

    #[test]
    fn test_ultrawide_3440_inspector_width_bounded() {
        // Inspector width must never blow up on ultrawide monitors
        let geom = calculate_layout(3440.0, DEFAULT_SIDEBAR_WIDTH, true, Some(600.0));
        assert_eq!(geom.presentation, InspectorPresentation::Side);
        assert_eq!(geom.inspector_width, INSPECTOR_MAX_WIDTH); // clamped to 440px
        assert_eq!(geom.primary_content_width, 3440.0 - 184.0 - 440.0);
    }

    #[test]
    fn test_grid_columns_calculation() {
        assert_eq!(calculate_grid_columns(0.0), 1);
        assert_eq!(calculate_grid_columns(300.0), 1);
        assert_eq!(calculate_grid_columns(580.0), 2);
        assert_eq!(calculate_grid_columns(900.0), 3);
        assert_eq!(calculate_grid_columns(1200.0), 4);
    }
}

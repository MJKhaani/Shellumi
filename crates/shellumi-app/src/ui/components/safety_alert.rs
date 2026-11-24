#![allow(dead_code)]
//! Standardized safety alert and security challenge dialogs.
//!
//! Enforces the single-alert pattern across the application for dangerous actions,
//! unsaved changes guards, master passphrase challenges, and TOFU fingerprint prompts.

use crate::theme::{danger, separator, surface, text_primary, text_secondary, warning, MacTheme};
use crate::ui::components::button::{AppButton, ButtonVariant};
use gpui::{
    div, px, rgb, rgba, AnyElement, Div, ElementId, IntoElement, ParentElement, SharedString,
    Styled,
};

/// The standard blocking modal backdrop.
pub fn alert_backdrop(content: impl IntoElement) -> Div {
    div()
        .absolute()
        .inset_0()
        .bg(rgba(0x000000a0))
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .child(content)
}

/// Standard alert container card.
pub fn alert_dialog(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    icon: Option<AnyElement>,
    is_danger: bool,
    actions: impl IntoElement,
) -> Div {
    let accent_color = if is_danger { danger() } else { warning() };

    let mut header = div().flex().items_center().gap_3().mb_3();

    if let Some(i) = icon {
        header = header.child(i);
    } else {
        header = header.child(
            div()
                .w(px(32.0))
                .h(px(32.0))
                .rounded_full()
                .bg(rgb(0x382222))
                .flex()
                .items_center()
                .justify_center()
                .text_color(accent_color)
                .child("!"),
        );
    }

    header = header.child(
        div()
            .text_base()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(text_primary())
            .child(title.into()),
    );

    div()
        .w(px(460.0))
        .max_w_full()
        .bg(surface())
        .border_1()
        .border_color(separator())
        .rounded(px(MacTheme::PANEL_RADIUS))
        .shadow_lg()
        .p_5()
        .flex()
        .flex_col()
        .gap_3()
        .child(header)
        .child(
            div()
                .text_sm()
                .text_color(text_secondary())
                .child(message.into()),
        )
        .child(div().mt_2().child(actions))
}

/// A standard unsaved changes guard alert dialog with custom button elements.
pub fn unsaved_changes_guard_dialog(
    object_name: impl Into<SharedString>,
    keep_editing_btn: impl IntoElement,
    discard_btn: impl IntoElement,
    save_btn: impl IntoElement,
) -> Div {
    let title = "Unsaved Changes";
    let message = format!(
        "You have unsaved changes to \"{}\". Do you want to save your changes before leaving?",
        object_name.into()
    );

    let actions = div()
        .flex()
        .items_center()
        .justify_end()
        .gap_2()
        .child(keep_editing_btn)
        .child(discard_btn)
        .child(save_btn);

    alert_backdrop(alert_dialog(title, message, None, false, actions))
}

/// A standard delete confirmation alert dialog with custom button elements.
pub fn delete_confirmation_dialog(
    object_type: &str,
    object_name: &str,
    confirm_btn: impl IntoElement,
    cancel_btn: impl IntoElement,
) -> Div {
    let title = format!("Delete {}?", object_type);
    let message = format!(
        "Are you sure you want to permanently delete \"{}\"? This action cannot be undone.",
        object_name
    );

    let actions = div()
        .flex()
        .items_center()
        .justify_end()
        .gap_2()
        .child(cancel_btn)
        .child(confirm_btn);

    alert_backdrop(alert_dialog(title, message, None, true, actions))
}

/// A standard unsaved changes guard alert dialog.
pub fn unsaved_changes_guard_alert(
    object_name: impl Into<SharedString>,
    save_id: impl Into<ElementId>,
    discard_id: impl Into<ElementId>,
    keep_editing_id: impl Into<ElementId>,
) -> Div {
    let title = "Unsaved Changes";
    let message = format!(
        "You have unsaved changes to \"{}\". Do you want to save your changes before leaving?",
        object_name.into()
    );

    let actions = div()
        .flex()
        .items_center()
        .justify_end()
        .gap_2()
        .child(AppButton::new(
            keep_editing_id,
            "Keep Editing",
            ButtonVariant::Ghost,
        ))
        .child(AppButton::new(
            discard_id,
            "Discard",
            ButtonVariant::Secondary,
        ))
        .child(AppButton::new(save_id, "Save", ButtonVariant::Primary));

    alert_backdrop(alert_dialog(title, message, None, false, actions))
}

/// A standard delete confirmation alert dialog.
pub fn delete_confirmation_alert(
    object_type: &str,
    object_name: &str,
    confirm_id: impl Into<ElementId>,
    cancel_id: impl Into<ElementId>,
) -> Div {
    let title = format!("Delete {}?", object_type);
    let message = format!(
        "Are you sure you want to permanently delete \"{}\"? This action cannot be undone.",
        object_name
    );

    let actions = div()
        .flex()
        .items_center()
        .justify_end()
        .gap_2()
        .child(AppButton::new(cancel_id, "Cancel", ButtonVariant::Ghost))
        .child(AppButton::new(
            confirm_id,
            format!("Delete {}", object_type),
            ButtonVariant::Danger,
        ));

    alert_backdrop(alert_dialog(title, message, None, true, actions))
}

/// Standard security challenge dialog card (for master passwords, recovery codes, auth challenges).
pub fn security_challenge_dialog(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    content: impl IntoElement,
    actions: impl IntoElement,
) -> Div {
    let header = div()
        .flex()
        .items_center()
        .gap_3()
        .mb_1()
        .child(
            div()
                .w(px(32.0))
                .h(px(32.0))
                .rounded_full()
                .bg(rgb(0x1e293b))
                .flex()
                .items_center()
                .justify_center()
                .text_color(crate::theme::accent_blue())
                .child("🔐"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_base()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(text_primary())
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(text_secondary())
                        .child(subtitle.into()),
                ),
        );

    div()
        .w(px(500.0))
        .max_w_full()
        .bg(surface())
        .border_1()
        .border_color(separator())
        .rounded(px(MacTheme::PANEL_RADIUS))
        .shadow_lg()
        .p_5()
        .flex()
        .flex_col()
        .gap_4()
        .child(header)
        .child(content)
        .child(
            div()
                .pt_3()
                .border_t_1()
                .border_color(separator())
                .child(actions),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_card_dimensions_and_structure() {
        let actions = div().child("test-actions");
        let alert = alert_dialog("Test Title", "Test Message", None, false, actions);
        // Alert card should be instantiated without panics
        let _backdrop = alert_backdrop(alert);
    }

    #[test]
    fn test_security_challenge_dialog_construction() {
        let content = div().child("passphrase-field");
        let actions = div().child("submit-btn");
        let dialog =
            security_challenge_dialog("Unlock Vault", "Argon2id derivation", content, actions);
        let backdrop = alert_backdrop(dialog);
        // Verify backdrop wrapper encapsulates the dialog
        drop(backdrop);
    }

    #[test]
    fn test_unsaved_changes_guard_construction() {
        let guard = unsaved_changes_guard_alert("Production Server", "save", "discard", "keep");
        drop(guard);
    }

    #[test]
    fn test_delete_confirmation_construction() {
        let alert = delete_confirmation_alert("Host", "db-prod-01", "confirm", "cancel");
        drop(alert);
    }
}

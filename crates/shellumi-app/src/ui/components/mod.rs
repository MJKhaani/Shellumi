#![allow(dead_code, unused_imports)]
//! Shared UI primitives, design tokens, and standard components.

pub mod button;
pub mod field;
pub mod form;
pub mod inspector_shell;
pub mod popover;
pub mod safety_alert;
pub mod status;

pub use button::{AppButton, ButtonSize, ButtonVariant, IconButton};
pub(crate) use field::{PasswordField, TextField};
pub use form::{
    error_notice, form_field, form_section, inline_notice, simple_field, simple_section,
    toggle_switch, validation_message,
};
pub use inspector_shell::InspectorShell;
pub use popover::{action_menu, MenuItem, PopoverAnchor};
pub use safety_alert::{
    alert_backdrop, alert_dialog, delete_confirmation_alert, delete_confirmation_dialog,
    security_challenge_dialog, unsaved_changes_guard_alert, unsaved_changes_guard_dialog,
};
pub use status::{empty_state, error_state, status_indicator, StatusKind};

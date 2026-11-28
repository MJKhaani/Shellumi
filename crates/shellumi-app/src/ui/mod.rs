//! Focused UI renderers for dialogs and session views.
use super::*;

pub(crate) mod components;
pub(crate) mod icon;
pub(crate) use icon::{Icon, IconName};
pub(crate) mod inspector;
pub(crate) mod layout;
pub(crate) mod window_chrome;

mod batch_modal;
mod command_palette;
pub(crate) mod conflict_diff;
mod conflict_modal;
mod create_vault_modal;
mod diagnostics_modal;
mod group_context_menu;
mod group_details_inspector;
mod host_editor;
mod hosts_popovers;
mod identity_modal;
mod infra_modal;
mod invite_team_modal;
mod mobile_modal;
pub(crate) mod remote_desktop_canvas;
mod remote_desktop_session;
mod session_replay_modal;
pub(crate) mod setup_wizard_modal;
pub(crate) mod setup_wizard_window;
mod shell_integration_modal;
mod snippet_modal;
mod team_modal;
mod tunnel_modal;
mod unlock_vault_modal;

//! Application actions, grouped by feature.
use super::*;

mod batch;
mod diagnostics;
mod groups;
mod hosts;
mod identities;
mod infrastructure;
mod invitations;
mod legacy_sftp;
mod mobile;
mod palette;
mod recordings;
mod remote_desktop;
mod server_sync;
mod shell_integration;
mod shortcuts;
mod snippets;
mod storage;
mod sync;
mod team;
mod terminal;
mod terminal_input;
mod tunnels;
mod vault_creation;
mod workspaces;

pub(crate) use server_sync::VaultContextSummary;

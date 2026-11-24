//! Generated embedded icon asset fallback for all 168 Shellumi icons across all resolutions.
//! Automatically generated from resources/icon_pack/manifest.json.

pub fn get_embedded_icon(name: &str, is_hover: bool, size_px: u32) -> Option<&'static [u8]> {
    let state = if is_hover { "hover" } else { "regular" };
    let size_folder = match size_px {
        s if s <= 32 => 32,
        s if s <= 48 => 48,
        s if s <= 64 => 64,
        s if s <= 128 => 128,
        _ => 256,
    };
    match (name, state, size_folder) {
        ("hosts", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/hosts.png"
        )),
        ("hosts", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/hosts.png"
        )),
        ("hosts", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/hosts.png"
        )),
        ("hosts", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/hosts.png"
        )),
        ("hosts", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/hosts.png"
        )),
        ("hosts", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/hosts.png"
        )),
        ("hosts", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/hosts.png"
        )),
        ("hosts", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/hosts.png"
        )),
        ("hosts", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/hosts.png"
        )),
        ("hosts", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/hosts.png"
        )),
        ("connect", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/connect.png"
        )),
        ("connect", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/connect.png"
        )),
        ("connect", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/connect.png"
        )),
        ("connect", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/connect.png"
        )),
        ("connect", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/connect.png"
        )),
        ("connect", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/connect.png"
        )),
        ("connect", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/connect.png"
        )),
        ("connect", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/connect.png"
        )),
        ("connect", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/connect.png"
        )),
        ("connect", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/connect.png"
        )),
        ("sftp", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/sftp.png"
        )),
        ("sftp", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/sftp.png"
        )),
        ("sftp", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/sftp.png"
        )),
        ("sftp", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/sftp.png"
        )),
        ("sftp", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/sftp.png"
        )),
        ("sftp", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/sftp.png"
        )),
        ("sftp", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/sftp.png"
        )),
        ("sftp", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/sftp.png"
        )),
        ("sftp", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/sftp.png"
        )),
        ("sftp", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/sftp.png"
        )),
        ("snippets", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/snippets.png"
        )),
        ("snippets", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/snippets.png"
        )),
        ("snippets", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/snippets.png"
        )),
        ("snippets", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/snippets.png"
        )),
        ("snippets", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/snippets.png"
        )),
        ("snippets", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/snippets.png"
        )),
        ("snippets", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/snippets.png"
        )),
        ("snippets", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/snippets.png"
        )),
        ("snippets", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/snippets.png"
        )),
        ("snippets", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/snippets.png"
        )),
        ("history", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/history.png"
        )),
        ("history", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/history.png"
        )),
        ("history", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/history.png"
        )),
        ("history", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/history.png"
        )),
        ("history", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/history.png"
        )),
        ("history", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/history.png"
        )),
        ("history", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/history.png"
        )),
        ("history", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/history.png"
        )),
        ("history", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/history.png"
        )),
        ("history", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/history.png"
        )),
        ("keychain", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/keychain.png"
        )),
        ("keychain", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/keychain.png"
        )),
        ("keychain", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/keychain.png"
        )),
        ("keychain", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/keychain.png"
        )),
        ("keychain", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/keychain.png"
        )),
        ("keychain", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/keychain.png"
        )),
        ("keychain", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/keychain.png"
        )),
        ("keychain", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/keychain.png"
        )),
        ("keychain", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/keychain.png"
        )),
        ("keychain", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/keychain.png"
        )),
        ("vaults", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/vaults.png"
        )),
        ("vaults", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/vaults.png"
        )),
        ("vaults", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/vaults.png"
        )),
        ("vaults", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/vaults.png"
        )),
        ("vaults", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/vaults.png"
        )),
        ("vaults", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/vaults.png"
        )),
        ("vaults", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/vaults.png"
        )),
        ("vaults", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/vaults.png"
        )),
        ("vaults", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/vaults.png"
        )),
        ("vaults", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/vaults.png"
        )),
        ("workspaces", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/workspaces.png"
        )),
        ("workspaces", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/workspaces.png"
        )),
        ("workspaces", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/workspaces.png"
        )),
        ("workspaces", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/workspaces.png"
        )),
        ("workspaces", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/workspaces.png"
        )),
        ("workspaces", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/workspaces.png"
        )),
        ("workspaces", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/workspaces.png"
        )),
        ("workspaces", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/workspaces.png"
        )),
        ("workspaces", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/workspaces.png"
        )),
        ("workspaces", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/workspaces.png"
        )),
        ("cloud_sync", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/cloud_sync.png"
        )),
        ("cloud_sync", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/cloud_sync.png"
        )),
        ("cloud_sync", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/cloud_sync.png"
        )),
        ("cloud_sync", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/cloud_sync.png"
        )),
        ("cloud_sync", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/cloud_sync.png"
        )),
        ("cloud_sync", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/cloud_sync.png"
        )),
        ("cloud_sync", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/cloud_sync.png"
        )),
        ("cloud_sync", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/cloud_sync.png"
        )),
        ("cloud_sync", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/cloud_sync.png"
        )),
        ("cloud_sync", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/cloud_sync.png"
        )),
        ("settings", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/settings.png"
        )),
        ("settings", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/settings.png"
        )),
        ("settings", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/settings.png"
        )),
        ("settings", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/settings.png"
        )),
        ("settings", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/settings.png"
        )),
        ("settings", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/settings.png"
        )),
        ("settings", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/settings.png"
        )),
        ("settings", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/settings.png"
        )),
        ("settings", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/settings.png"
        )),
        ("settings", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/settings.png"
        )),
        ("add_host", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/add_host.png"
        )),
        ("add_host", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/add_host.png"
        )),
        ("add_host", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/add_host.png"
        )),
        ("add_host", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/add_host.png"
        )),
        ("add_host", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/add_host.png"
        )),
        ("add_host", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/add_host.png"
        )),
        ("add_host", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/add_host.png"
        )),
        ("add_host", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/add_host.png"
        )),
        ("add_host", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/add_host.png"
        )),
        ("add_host", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/add_host.png"
        )),
        ("search", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/search.png"
        )),
        ("search", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/search.png"
        )),
        ("search", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/search.png"
        )),
        ("search", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/search.png"
        )),
        ("search", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/search.png"
        )),
        ("search", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/search.png"
        )),
        ("search", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/search.png"
        )),
        ("search", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/search.png"
        )),
        ("search", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/search.png"
        )),
        ("search", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/search.png"
        )),
        ("terminal", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/terminal.png"
        )),
        ("terminal", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/terminal.png"
        )),
        ("terminal", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/terminal.png"
        )),
        ("terminal", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/terminal.png"
        )),
        ("terminal", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/terminal.png"
        )),
        ("terminal", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/terminal.png"
        )),
        ("terminal", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/terminal.png"
        )),
        ("terminal", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/terminal.png"
        )),
        ("terminal", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/terminal.png"
        )),
        ("terminal", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/terminal.png"
        )),
        ("serial", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/serial.png"
        )),
        ("serial", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/serial.png"
        )),
        ("serial", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/serial.png"
        )),
        ("serial", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/serial.png"
        )),
        ("serial", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/serial.png"
        )),
        ("serial", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/serial.png"
        )),
        ("serial", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/serial.png"
        )),
        ("serial", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/serial.png"
        )),
        ("serial", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/serial.png"
        )),
        ("serial", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/serial.png"
        )),
        ("split_view", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/split_view.png"
        )),
        ("split_view", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/split_view.png"
        )),
        ("split_view", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/split_view.png"
        )),
        ("split_view", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/split_view.png"
        )),
        ("split_view", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/split_view.png"
        )),
        ("split_view", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/split_view.png"
        )),
        ("split_view", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/split_view.png"
        )),
        ("split_view", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/split_view.png"
        )),
        ("split_view", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/split_view.png"
        )),
        ("split_view", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/split_view.png"
        )),
        ("grid_view", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/grid_view.png"
        )),
        ("grid_view", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/grid_view.png"
        )),
        ("grid_view", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/grid_view.png"
        )),
        ("grid_view", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/grid_view.png"
        )),
        ("grid_view", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/grid_view.png"
        )),
        ("grid_view", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/grid_view.png"
        )),
        ("grid_view", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/grid_view.png"
        )),
        ("grid_view", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/grid_view.png"
        )),
        ("grid_view", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/grid_view.png"
        )),
        ("grid_view", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/grid_view.png"
        )),
        ("list_view", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/list_view.png"
        )),
        ("list_view", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/list_view.png"
        )),
        ("list_view", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/list_view.png"
        )),
        ("list_view", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/list_view.png"
        )),
        ("list_view", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/list_view.png"
        )),
        ("list_view", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/list_view.png"
        )),
        ("list_view", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/list_view.png"
        )),
        ("list_view", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/list_view.png"
        )),
        ("list_view", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/list_view.png"
        )),
        ("list_view", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/list_view.png"
        )),
        ("groups", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/groups.png"
        )),
        ("groups", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/groups.png"
        )),
        ("groups", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/groups.png"
        )),
        ("groups", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/groups.png"
        )),
        ("groups", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/groups.png"
        )),
        ("groups", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/groups.png"
        )),
        ("groups", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/groups.png"
        )),
        ("groups", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/groups.png"
        )),
        ("groups", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/groups.png"
        )),
        ("groups", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/groups.png"
        )),
        ("tags", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/tags.png"
        )),
        ("tags", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/tags.png"
        )),
        ("tags", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/tags.png"
        )),
        ("tags", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/tags.png"
        )),
        ("tags", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/tags.png"
        )),
        ("tags", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/tags.png"
        )),
        ("tags", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/tags.png"
        )),
        ("tags", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/tags.png"
        )),
        ("tags", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/tags.png"
        )),
        ("tags", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/tags.png"
        )),
        ("favorite", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/favorite.png"
        )),
        ("favorite", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/favorite.png"
        )),
        ("favorite", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/favorite.png"
        )),
        ("favorite", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/favorite.png"
        )),
        ("favorite", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/favorite.png"
        )),
        ("favorite", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/favorite.png"
        )),
        ("favorite", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/favorite.png"
        )),
        ("favorite", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/favorite.png"
        )),
        ("favorite", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/favorite.png"
        )),
        ("favorite", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/favorite.png"
        )),
        ("more", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/more.png"
        )),
        ("more", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/more.png"
        )),
        ("more", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/more.png"
        )),
        ("more", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/more.png"
        )),
        ("more", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/more.png"
        )),
        ("more", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/more.png"
        )),
        ("more", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/more.png"
        )),
        ("more", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/more.png"
        )),
        ("more", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/more.png"
        )),
        ("more", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/more.png"
        )),
        ("edit", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/edit.png"
        )),
        ("edit", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/edit.png"
        )),
        ("edit", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/edit.png"
        )),
        ("edit", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/edit.png"
        )),
        ("edit", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/edit.png"
        )),
        ("edit", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/edit.png"
        )),
        ("edit", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/edit.png"
        )),
        ("edit", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/edit.png"
        )),
        ("edit", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/edit.png"
        )),
        ("edit", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/edit.png"
        )),
        ("delete", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/delete.png"
        )),
        ("delete", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/delete.png"
        )),
        ("delete", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/delete.png"
        )),
        ("delete", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/delete.png"
        )),
        ("delete", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/delete.png"
        )),
        ("delete", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/delete.png"
        )),
        ("delete", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/delete.png"
        )),
        ("delete", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/delete.png"
        )),
        ("delete", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/delete.png"
        )),
        ("delete", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/delete.png"
        )),
        ("refresh", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/refresh.png"
        )),
        ("refresh", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/refresh.png"
        )),
        ("refresh", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/refresh.png"
        )),
        ("refresh", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/refresh.png"
        )),
        ("refresh", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/refresh.png"
        )),
        ("refresh", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/refresh.png"
        )),
        ("refresh", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/refresh.png"
        )),
        ("refresh", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/refresh.png"
        )),
        ("refresh", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/refresh.png"
        )),
        ("refresh", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/refresh.png"
        )),
        ("profile", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/profile.png"
        )),
        ("profile", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/profile.png"
        )),
        ("profile", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/profile.png"
        )),
        ("profile", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/profile.png"
        )),
        ("profile", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/profile.png"
        )),
        ("profile", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/profile.png"
        )),
        ("profile", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/profile.png"
        )),
        ("profile", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/profile.png"
        )),
        ("profile", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/profile.png"
        )),
        ("profile", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/profile.png"
        )),
        ("known_hosts", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/known_hosts.png"
        )),
        ("known_hosts", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/known_hosts.png"
        )),
        ("known_hosts", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/known_hosts.png"
        )),
        ("known_hosts", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/known_hosts.png"
        )),
        ("known_hosts", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/known_hosts.png"
        )),
        ("known_hosts", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/known_hosts.png"
        )),
        ("known_hosts", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/known_hosts.png"
        )),
        ("known_hosts", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/known_hosts.png"
        )),
        ("known_hosts", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/known_hosts.png"
        )),
        ("known_hosts", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/known_hosts.png"
        )),
        ("port_forwarding", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/port_forwarding.png"
        )),
        ("port_forwarding", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/port_forwarding.png"
        )),
        ("port_forwarding", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/port_forwarding.png"
        )),
        ("port_forwarding", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/port_forwarding.png"
        )),
        ("port_forwarding", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/port_forwarding.png"
        )),
        ("port_forwarding", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/port_forwarding.png"
        )),
        ("port_forwarding", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/port_forwarding.png"
        )),
        ("port_forwarding", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/port_forwarding.png"
        )),
        ("port_forwarding", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/port_forwarding.png"
        )),
        ("port_forwarding", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/port_forwarding.png"
        )),
        ("shellumi_logo", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/shellumi_logo.png"
        )),
        ("shellumi_logo", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/shellumi_logo.png"
        )),
        ("shellumi_logo", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/shellumi_logo.png"
        )),
        ("shellumi_logo", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/shellumi_logo.png"
        )),
        ("shellumi_logo", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/shellumi_logo.png"
        )),
        ("shellumi_logo", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/shellumi_logo.png"
        )),
        ("shellumi_logo", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/shellumi_logo.png"
        )),
        ("shellumi_logo", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/shellumi_logo.png"
        )),
        ("shellumi_logo", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/shellumi_logo.png"
        )),
        ("shellumi_logo", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/shellumi_logo.png"
        )),
        ("group_settings", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/group_settings.png"
        )),
        ("group_settings", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/group_settings.png"
        )),
        ("group_settings", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/group_settings.png"
        )),
        ("group_settings", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/group_settings.png"
        )),
        ("group_settings", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/group_settings.png"
        )),
        ("group_settings", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/group_settings.png"
        )),
        ("group_settings", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/group_settings.png"
        )),
        ("group_settings", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/group_settings.png"
        )),
        ("group_settings", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/group_settings.png"
        )),
        ("group_settings", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/group_settings.png"
        )),
        ("import", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/import.png"
        )),
        ("import", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/import.png"
        )),
        ("import", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/import.png"
        )),
        ("import", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/import.png"
        )),
        ("import", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/import.png"
        )),
        ("import", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/import.png"
        )),
        ("import", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/import.png"
        )),
        ("import", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/import.png"
        )),
        ("import", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/import.png"
        )),
        ("import", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/import.png"
        )),
        ("export", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/export.png"
        )),
        ("export", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/export.png"
        )),
        ("export", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/export.png"
        )),
        ("export", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/export.png"
        )),
        ("export", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/export.png"
        )),
        ("export", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/export.png"
        )),
        ("export", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/export.png"
        )),
        ("export", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/export.png"
        )),
        ("export", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/export.png"
        )),
        ("export", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/export.png"
        )),
        ("tag", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/tag.png"
        )),
        ("tag", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/tag.png"
        )),
        ("tag", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/tag.png"
        )),
        ("tag", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/tag.png"
        )),
        ("tag", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/tag.png"
        )),
        ("tag", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/tag.png"
        )),
        ("tag", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/tag.png"
        )),
        ("tag", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/tag.png"
        )),
        ("tag", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/tag.png"
        )),
        ("tag", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/tag.png"
        )),
        ("calendar_sort", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/calendar_sort.png"
        )),
        ("calendar_sort", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/calendar_sort.png"
        )),
        ("calendar_sort", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/calendar_sort.png"
        )),
        ("calendar_sort", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/calendar_sort.png"
        )),
        ("calendar_sort", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/calendar_sort.png"
        )),
        ("calendar_sort", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/calendar_sort.png"
        )),
        ("calendar_sort", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/calendar_sort.png"
        )),
        ("calendar_sort", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/calendar_sort.png"
        )),
        ("calendar_sort", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/calendar_sort.png"
        )),
        ("calendar_sort", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/calendar_sort.png"
        )),
        ("certificate", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/certificate.png"
        )),
        ("certificate", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/certificate.png"
        )),
        ("certificate", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/certificate.png"
        )),
        ("certificate", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/certificate.png"
        )),
        ("certificate", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/certificate.png"
        )),
        ("certificate", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/certificate.png"
        )),
        ("certificate", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/certificate.png"
        )),
        ("certificate", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/certificate.png"
        )),
        ("certificate", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/certificate.png"
        )),
        ("certificate", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/certificate.png"
        )),
        ("private_key", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/private_key.png"
        )),
        ("private_key", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/private_key.png"
        )),
        ("private_key", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/private_key.png"
        )),
        ("private_key", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/private_key.png"
        )),
        ("private_key", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/private_key.png"
        )),
        ("private_key", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/private_key.png"
        )),
        ("private_key", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/private_key.png"
        )),
        ("private_key", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/private_key.png"
        )),
        ("private_key", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/private_key.png"
        )),
        ("private_key", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/private_key.png"
        )),
        ("public_key", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/public_key.png"
        )),
        ("public_key", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/public_key.png"
        )),
        ("public_key", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/public_key.png"
        )),
        ("public_key", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/public_key.png"
        )),
        ("public_key", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/public_key.png"
        )),
        ("public_key", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/public_key.png"
        )),
        ("public_key", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/public_key.png"
        )),
        ("public_key", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/public_key.png"
        )),
        ("public_key", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/public_key.png"
        )),
        ("public_key", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/public_key.png"
        )),
        ("identity_card", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/identity_card.png"
        )),
        ("identity_card", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/identity_card.png"
        )),
        ("identity_card", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/identity_card.png"
        )),
        ("identity_card", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/identity_card.png"
        )),
        ("identity_card", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/identity_card.png"
        )),
        ("identity_card", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/identity_card.png"
        )),
        ("identity_card", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/identity_card.png"
        )),
        ("identity_card", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/identity_card.png"
        )),
        ("identity_card", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/identity_card.png"
        )),
        ("identity_card", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/identity_card.png"
        )),
        ("os_arch", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_arch.png"
        )),
        ("os_arch", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_arch.png"
        )),
        ("os_arch", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_arch.png"
        )),
        ("os_arch", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_arch.png"
        )),
        ("os_arch", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_arch.png"
        )),
        ("os_arch", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_arch.png"
        )),
        ("os_arch", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_arch.png"
        )),
        ("os_arch", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_arch.png"
        )),
        ("os_arch", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_arch.png"
        )),
        ("os_arch", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_arch.png"
        )),
        ("os_ubuntu", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_ubuntu.png"
        )),
        ("os_ubuntu", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_ubuntu.png"
        )),
        ("os_ubuntu", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_ubuntu.png"
        )),
        ("os_ubuntu", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_ubuntu.png"
        )),
        ("os_ubuntu", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_ubuntu.png"
        )),
        ("os_ubuntu", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_ubuntu.png"
        )),
        ("os_ubuntu", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_ubuntu.png"
        )),
        ("os_ubuntu", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_ubuntu.png"
        )),
        ("os_ubuntu", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_ubuntu.png"
        )),
        ("os_ubuntu", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_ubuntu.png"
        )),
        ("os_macos", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_macos.png"
        )),
        ("os_macos", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_macos.png"
        )),
        ("os_macos", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_macos.png"
        )),
        ("os_macos", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_macos.png"
        )),
        ("os_macos", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_macos.png"
        )),
        ("os_macos", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_macos.png"
        )),
        ("os_macos", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_macos.png"
        )),
        ("os_macos", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_macos.png"
        )),
        ("os_macos", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_macos.png"
        )),
        ("os_macos", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_macos.png"
        )),
        ("os_freebsd", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_freebsd.png"
        )),
        ("os_freebsd", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_freebsd.png"
        )),
        ("os_freebsd", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_freebsd.png"
        )),
        ("os_freebsd", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_freebsd.png"
        )),
        ("os_freebsd", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_freebsd.png"
        )),
        ("os_freebsd", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_freebsd.png"
        )),
        ("os_freebsd", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_freebsd.png"
        )),
        ("os_freebsd", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_freebsd.png"
        )),
        ("os_freebsd", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_freebsd.png"
        )),
        ("os_freebsd", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_freebsd.png"
        )),
        ("os_netbsd", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_netbsd.png"
        )),
        ("os_netbsd", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_netbsd.png"
        )),
        ("os_netbsd", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_netbsd.png"
        )),
        ("os_netbsd", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_netbsd.png"
        )),
        ("os_netbsd", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_netbsd.png"
        )),
        ("os_netbsd", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_netbsd.png"
        )),
        ("os_netbsd", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_netbsd.png"
        )),
        ("os_netbsd", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_netbsd.png"
        )),
        ("os_netbsd", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_netbsd.png"
        )),
        ("os_netbsd", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_netbsd.png"
        )),
        ("os_openbsd", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_openbsd.png"
        )),
        ("os_openbsd", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_openbsd.png"
        )),
        ("os_openbsd", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_openbsd.png"
        )),
        ("os_openbsd", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_openbsd.png"
        )),
        ("os_openbsd", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_openbsd.png"
        )),
        ("os_openbsd", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_openbsd.png"
        )),
        ("os_openbsd", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_openbsd.png"
        )),
        ("os_openbsd", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_openbsd.png"
        )),
        ("os_openbsd", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_openbsd.png"
        )),
        ("os_openbsd", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_openbsd.png"
        )),
        ("os_linux", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_linux.png"
        )),
        ("os_linux", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_linux.png"
        )),
        ("os_linux", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_linux.png"
        )),
        ("os_linux", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_linux.png"
        )),
        ("os_linux", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_linux.png"
        )),
        ("os_linux", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_linux.png"
        )),
        ("os_linux", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_linux.png"
        )),
        ("os_linux", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_linux.png"
        )),
        ("os_linux", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_linux.png"
        )),
        ("os_linux", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_linux.png"
        )),
        ("os_windows", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_windows.png"
        )),
        ("os_windows", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_windows.png"
        )),
        ("os_windows", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_windows.png"
        )),
        ("os_windows", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_windows.png"
        )),
        ("os_windows", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_windows.png"
        )),
        ("os_windows", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_windows.png"
        )),
        ("os_windows", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_windows.png"
        )),
        ("os_windows", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_windows.png"
        )),
        ("os_windows", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_windows.png"
        )),
        ("os_windows", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_windows.png"
        )),
        ("server", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/server.png"
        )),
        ("server", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/server.png"
        )),
        ("server", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/server.png"
        )),
        ("server", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/server.png"
        )),
        ("server", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/server.png"
        )),
        ("server", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/server.png"
        )),
        ("server", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/server.png"
        )),
        ("server", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/server.png"
        )),
        ("server", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/server.png"
        )),
        ("server", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/server.png"
        )),
        ("fido2", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/fido2.png"
        )),
        ("fido2", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/fido2.png"
        )),
        ("fido2", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/fido2.png"
        )),
        ("fido2", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/fido2.png"
        )),
        ("fido2", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/fido2.png"
        )),
        ("fido2", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/fido2.png"
        )),
        ("fido2", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/fido2.png"
        )),
        ("fido2", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/fido2.png"
        )),
        ("fido2", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/fido2.png"
        )),
        ("fido2", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/fido2.png"
        )),
        ("fingerprint", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/fingerprint.png"
        )),
        ("fingerprint", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/fingerprint.png"
        )),
        ("fingerprint", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/fingerprint.png"
        )),
        ("fingerprint", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/fingerprint.png"
        )),
        ("fingerprint", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/fingerprint.png"
        )),
        ("fingerprint", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/fingerprint.png"
        )),
        ("fingerprint", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/fingerprint.png"
        )),
        ("fingerprint", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/fingerprint.png"
        )),
        ("fingerprint", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/fingerprint.png"
        )),
        ("fingerprint", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/fingerprint.png"
        )),
        ("log", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/log.png"
        )),
        ("log", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/log.png"
        )),
        ("log", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/log.png"
        )),
        ("log", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/log.png"
        )),
        ("log", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/log.png"
        )),
        ("log", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/log.png"
        )),
        ("log", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/log.png"
        )),
        ("log", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/log.png"
        )),
        ("log", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/log.png"
        )),
        ("log", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/log.png"
        )),
        ("bell", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/bell.png"
        )),
        ("bell", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/bell.png"
        )),
        ("bell", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/bell.png"
        )),
        ("bell", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/bell.png"
        )),
        ("bell", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/bell.png"
        )),
        ("bell", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/bell.png"
        )),
        ("bell", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/bell.png"
        )),
        ("bell", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/bell.png"
        )),
        ("bell", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/bell.png"
        )),
        ("bell", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/bell.png"
        )),
        ("invite_member", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/invite_member.png"
        )),
        ("invite_member", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/invite_member.png"
        )),
        ("invite_member", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/invite_member.png"
        )),
        ("invite_member", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/invite_member.png"
        )),
        ("invite_member", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/invite_member.png"
        )),
        ("invite_member", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/invite_member.png"
        )),
        ("invite_member", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/invite_member.png"
        )),
        ("invite_member", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/invite_member.png"
        )),
        ("invite_member", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/invite_member.png"
        )),
        ("invite_member", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/invite_member.png"
        )),
        ("infrastructure", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/infrastructure.png"
        )),
        ("infrastructure", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/infrastructure.png"
        )),
        ("infrastructure", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/infrastructure.png"
        )),
        ("infrastructure", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/infrastructure.png"
        )),
        ("infrastructure", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/infrastructure.png"
        )),
        ("infrastructure", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/infrastructure.png"
        )),
        ("infrastructure", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/infrastructure.png"
        )),
        ("infrastructure", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/infrastructure.png"
        )),
        ("infrastructure", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/infrastructure.png"
        )),
        ("infrastructure", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/infrastructure.png"
        )),
        ("diagnostics", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/diagnostics.png"
        )),
        ("diagnostics", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/diagnostics.png"
        )),
        ("diagnostics", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/diagnostics.png"
        )),
        ("diagnostics", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/diagnostics.png"
        )),
        ("diagnostics", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/diagnostics.png"
        )),
        ("diagnostics", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/diagnostics.png"
        )),
        ("diagnostics", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/diagnostics.png"
        )),
        ("diagnostics", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/diagnostics.png"
        )),
        ("diagnostics", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/diagnostics.png"
        )),
        ("diagnostics", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/diagnostics.png"
        )),
        ("mobile", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/mobile.png"
        )),
        ("mobile", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/mobile.png"
        )),
        ("mobile", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/mobile.png"
        )),
        ("mobile", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/mobile.png"
        )),
        ("mobile", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/mobile.png"
        )),
        ("mobile", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/mobile.png"
        )),
        ("mobile", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/mobile.png"
        )),
        ("mobile", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/mobile.png"
        )),
        ("mobile", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/mobile.png"
        )),
        ("mobile", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/mobile.png"
        )),
        ("overview", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/overview.png"
        )),
        ("overview", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/overview.png"
        )),
        ("overview", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/overview.png"
        )),
        ("overview", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/overview.png"
        )),
        ("overview", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/overview.png"
        )),
        ("overview", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/overview.png"
        )),
        ("overview", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/overview.png"
        )),
        ("overview", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/overview.png"
        )),
        ("overview", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/overview.png"
        )),
        ("overview", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/overview.png"
        )),
        ("inspector", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/inspector.png"
        )),
        ("inspector", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/inspector.png"
        )),
        ("inspector", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/inspector.png"
        )),
        ("inspector", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/inspector.png"
        )),
        ("inspector", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/inspector.png"
        )),
        ("inspector", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/inspector.png"
        )),
        ("inspector", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/inspector.png"
        )),
        ("inspector", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/inspector.png"
        )),
        ("inspector", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/inspector.png"
        )),
        ("inspector", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/inspector.png"
        )),
        ("new_group", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/new_group.png"
        )),
        ("new_group", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/new_group.png"
        )),
        ("new_group", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/new_group.png"
        )),
        ("new_group", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/new_group.png"
        )),
        ("new_group", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/new_group.png"
        )),
        ("new_group", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/new_group.png"
        )),
        ("new_group", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/new_group.png"
        )),
        ("new_group", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/new_group.png"
        )),
        ("new_group", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/new_group.png"
        )),
        ("new_group", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/new_group.png"
        )),
        ("duplicate_host", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/duplicate_host.png"
        )),
        ("duplicate_host", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/duplicate_host.png"
        )),
        ("duplicate_host", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/duplicate_host.png"
        )),
        ("duplicate_host", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/duplicate_host.png"
        )),
        ("duplicate_host", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/duplicate_host.png"
        )),
        ("duplicate_host", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/duplicate_host.png"
        )),
        ("duplicate_host", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/duplicate_host.png"
        )),
        ("duplicate_host", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/duplicate_host.png"
        )),
        ("duplicate_host", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/duplicate_host.png"
        )),
        ("duplicate_host", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/duplicate_host.png"
        )),
        ("move_to_group", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/move_to_group.png"
        )),
        ("move_to_group", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/move_to_group.png"
        )),
        ("move_to_group", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/move_to_group.png"
        )),
        ("move_to_group", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/move_to_group.png"
        )),
        ("move_to_group", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/move_to_group.png"
        )),
        ("move_to_group", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/move_to_group.png"
        )),
        ("move_to_group", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/move_to_group.png"
        )),
        ("move_to_group", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/move_to_group.png"
        )),
        ("move_to_group", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/move_to_group.png"
        )),
        ("move_to_group", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/move_to_group.png"
        )),
        ("copy_to_group", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/copy_to_group.png"
        )),
        ("copy_to_group", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/copy_to_group.png"
        )),
        ("copy_to_group", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/copy_to_group.png"
        )),
        ("copy_to_group", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/copy_to_group.png"
        )),
        ("copy_to_group", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/copy_to_group.png"
        )),
        ("copy_to_group", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/copy_to_group.png"
        )),
        ("copy_to_group", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/copy_to_group.png"
        )),
        ("copy_to_group", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/copy_to_group.png"
        )),
        ("copy_to_group", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/copy_to_group.png"
        )),
        ("copy_to_group", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/copy_to_group.png"
        )),
        ("new_workspace", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/new_workspace.png"
        )),
        ("new_workspace", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/new_workspace.png"
        )),
        ("new_workspace", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/new_workspace.png"
        )),
        ("new_workspace", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/new_workspace.png"
        )),
        ("new_workspace", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/new_workspace.png"
        )),
        ("new_workspace", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/new_workspace.png"
        )),
        ("new_workspace", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/new_workspace.png"
        )),
        ("new_workspace", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/new_workspace.png"
        )),
        ("new_workspace", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/new_workspace.png"
        )),
        ("new_workspace", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/new_workspace.png"
        )),
        ("new_vault", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/new_vault.png"
        )),
        ("new_vault", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/new_vault.png"
        )),
        ("new_vault", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/new_vault.png"
        )),
        ("new_vault", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/new_vault.png"
        )),
        ("new_vault", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/new_vault.png"
        )),
        ("new_vault", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/new_vault.png"
        )),
        ("new_vault", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/new_vault.png"
        )),
        ("new_vault", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/new_vault.png"
        )),
        ("new_vault", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/new_vault.png"
        )),
        ("new_vault", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/new_vault.png"
        )),
        ("switch_vault", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/switch_vault.png"
        )),
        ("switch_vault", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/switch_vault.png"
        )),
        ("switch_vault", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/switch_vault.png"
        )),
        ("switch_vault", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/switch_vault.png"
        )),
        ("switch_vault", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/switch_vault.png"
        )),
        ("switch_vault", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/switch_vault.png"
        )),
        ("switch_vault", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/switch_vault.png"
        )),
        ("switch_vault", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/switch_vault.png"
        )),
        ("switch_vault", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/switch_vault.png"
        )),
        ("switch_vault", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/switch_vault.png"
        )),
        ("quick_connect", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/quick_connect.png"
        )),
        ("quick_connect", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/quick_connect.png"
        )),
        ("quick_connect", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/quick_connect.png"
        )),
        ("quick_connect", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/quick_connect.png"
        )),
        ("quick_connect", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/quick_connect.png"
        )),
        ("quick_connect", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/quick_connect.png"
        )),
        ("quick_connect", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/quick_connect.png"
        )),
        ("quick_connect", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/quick_connect.png"
        )),
        ("quick_connect", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/quick_connect.png"
        )),
        ("quick_connect", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/quick_connect.png"
        )),
        ("jump_host", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/jump_host.png"
        )),
        ("jump_host", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/jump_host.png"
        )),
        ("jump_host", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/jump_host.png"
        )),
        ("jump_host", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/jump_host.png"
        )),
        ("jump_host", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/jump_host.png"
        )),
        ("jump_host", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/jump_host.png"
        )),
        ("jump_host", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/jump_host.png"
        )),
        ("jump_host", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/jump_host.png"
        )),
        ("jump_host", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/jump_host.png"
        )),
        ("jump_host", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/jump_host.png"
        )),
        ("ssh", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/ssh.png"
        )),
        ("ssh", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/ssh.png"
        )),
        ("ssh", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/ssh.png"
        )),
        ("ssh", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/ssh.png"
        )),
        ("ssh", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/ssh.png"
        )),
        ("ssh", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/ssh.png"
        )),
        ("ssh", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/ssh.png"
        )),
        ("ssh", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/ssh.png"
        )),
        ("ssh", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/ssh.png"
        )),
        ("ssh", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/ssh.png"
        )),
        ("mosh", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/mosh.png"
        )),
        ("mosh", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/mosh.png"
        )),
        ("mosh", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/mosh.png"
        )),
        ("mosh", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/mosh.png"
        )),
        ("mosh", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/mosh.png"
        )),
        ("mosh", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/mosh.png"
        )),
        ("mosh", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/mosh.png"
        )),
        ("mosh", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/mosh.png"
        )),
        ("mosh", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/mosh.png"
        )),
        ("mosh", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/mosh.png"
        )),
        ("rdp", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/rdp.png"
        )),
        ("rdp", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/rdp.png"
        )),
        ("rdp", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/rdp.png"
        )),
        ("rdp", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/rdp.png"
        )),
        ("rdp", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/rdp.png"
        )),
        ("rdp", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/rdp.png"
        )),
        ("rdp", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/rdp.png"
        )),
        ("rdp", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/rdp.png"
        )),
        ("rdp", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/rdp.png"
        )),
        ("rdp", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/rdp.png"
        )),
        ("vnc", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/vnc.png"
        )),
        ("vnc", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/vnc.png"
        )),
        ("vnc", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/vnc.png"
        )),
        ("vnc", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/vnc.png"
        )),
        ("vnc", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/vnc.png"
        )),
        ("vnc", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/vnc.png"
        )),
        ("vnc", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/vnc.png"
        )),
        ("vnc", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/vnc.png"
        )),
        ("vnc", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/vnc.png"
        )),
        ("vnc", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/vnc.png"
        )),
        ("disconnect", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/disconnect.png"
        )),
        ("disconnect", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/disconnect.png"
        )),
        ("disconnect", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/disconnect.png"
        )),
        ("disconnect", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/disconnect.png"
        )),
        ("disconnect", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/disconnect.png"
        )),
        ("disconnect", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/disconnect.png"
        )),
        ("disconnect", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/disconnect.png"
        )),
        ("disconnect", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/disconnect.png"
        )),
        ("disconnect", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/disconnect.png"
        )),
        ("disconnect", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/disconnect.png"
        )),
        ("reconnect", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/reconnect.png"
        )),
        ("reconnect", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/reconnect.png"
        )),
        ("reconnect", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/reconnect.png"
        )),
        ("reconnect", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/reconnect.png"
        )),
        ("reconnect", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/reconnect.png"
        )),
        ("reconnect", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/reconnect.png"
        )),
        ("reconnect", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/reconnect.png"
        )),
        ("reconnect", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/reconnect.png"
        )),
        ("reconnect", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/reconnect.png"
        )),
        ("reconnect", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/reconnect.png"
        )),
        ("local_terminal", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/local_terminal.png"
        )),
        ("local_terminal", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/local_terminal.png"
        )),
        ("local_terminal", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/local_terminal.png"
        )),
        ("local_terminal", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/local_terminal.png"
        )),
        ("local_terminal", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/local_terminal.png"
        )),
        ("local_terminal", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/local_terminal.png"
        )),
        ("local_terminal", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/local_terminal.png"
        )),
        ("local_terminal", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/local_terminal.png"
        )),
        ("local_terminal", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/local_terminal.png"
        )),
        ("local_terminal", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/local_terminal.png"
        )),
        ("session_record", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/session_record.png"
        )),
        ("session_record", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/session_record.png"
        )),
        ("session_record", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/session_record.png"
        )),
        ("session_record", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/session_record.png"
        )),
        ("session_record", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/session_record.png"
        )),
        ("session_record", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/session_record.png"
        )),
        ("session_record", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/session_record.png"
        )),
        ("session_record", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/session_record.png"
        )),
        ("session_record", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/session_record.png"
        )),
        ("session_record", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/session_record.png"
        )),
        ("session_replay", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/session_replay.png"
        )),
        ("session_replay", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/session_replay.png"
        )),
        ("session_replay", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/session_replay.png"
        )),
        ("session_replay", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/session_replay.png"
        )),
        ("session_replay", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/session_replay.png"
        )),
        ("session_replay", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/session_replay.png"
        )),
        ("session_replay", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/session_replay.png"
        )),
        ("session_replay", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/session_replay.png"
        )),
        ("session_replay", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/session_replay.png"
        )),
        ("session_replay", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/session_replay.png"
        )),
        ("session_info", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/session_info.png"
        )),
        ("session_info", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/session_info.png"
        )),
        ("session_info", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/session_info.png"
        )),
        ("session_info", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/session_info.png"
        )),
        ("session_info", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/session_info.png"
        )),
        ("session_info", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/session_info.png"
        )),
        ("session_info", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/session_info.png"
        )),
        ("session_info", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/session_info.png"
        )),
        ("session_info", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/session_info.png"
        )),
        ("session_info", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/session_info.png"
        )),
        ("fit_to_view", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/fit_to_view.png"
        )),
        ("fit_to_view", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/fit_to_view.png"
        )),
        ("fit_to_view", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/fit_to_view.png"
        )),
        ("fit_to_view", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/fit_to_view.png"
        )),
        ("fit_to_view", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/fit_to_view.png"
        )),
        ("fit_to_view", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/fit_to_view.png"
        )),
        ("fit_to_view", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/fit_to_view.png"
        )),
        ("fit_to_view", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/fit_to_view.png"
        )),
        ("fit_to_view", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/fit_to_view.png"
        )),
        ("fit_to_view", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/fit_to_view.png"
        )),
        ("actual_size", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/actual_size.png"
        )),
        ("actual_size", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/actual_size.png"
        )),
        ("actual_size", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/actual_size.png"
        )),
        ("actual_size", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/actual_size.png"
        )),
        ("actual_size", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/actual_size.png"
        )),
        ("actual_size", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/actual_size.png"
        )),
        ("actual_size", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/actual_size.png"
        )),
        ("actual_size", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/actual_size.png"
        )),
        ("actual_size", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/actual_size.png"
        )),
        ("actual_size", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/actual_size.png"
        )),
        ("fill_view", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/fill_view.png"
        )),
        ("fill_view", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/fill_view.png"
        )),
        ("fill_view", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/fill_view.png"
        )),
        ("fill_view", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/fill_view.png"
        )),
        ("fill_view", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/fill_view.png"
        )),
        ("fill_view", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/fill_view.png"
        )),
        ("fill_view", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/fill_view.png"
        )),
        ("fill_view", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/fill_view.png"
        )),
        ("fill_view", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/fill_view.png"
        )),
        ("fill_view", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/fill_view.png"
        )),
        ("display_resolution", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/display_resolution.png"
        )),
        ("display_resolution", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/display_resolution.png"
        )),
        ("display_resolution", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/display_resolution.png"
        )),
        ("display_resolution", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/display_resolution.png"
        )),
        ("display_resolution", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/display_resolution.png"
        )),
        ("display_resolution", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/display_resolution.png"
        )),
        ("display_resolution", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/display_resolution.png"
        )),
        ("display_resolution", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/display_resolution.png"
        )),
        ("display_resolution", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/display_resolution.png"
        )),
        ("display_resolution", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/display_resolution.png"
        )),
        ("dynamic_resize", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/dynamic_resize.png"
        )),
        ("dynamic_resize", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/dynamic_resize.png"
        )),
        ("dynamic_resize", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/dynamic_resize.png"
        )),
        ("dynamic_resize", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/dynamic_resize.png"
        )),
        ("dynamic_resize", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/dynamic_resize.png"
        )),
        ("dynamic_resize", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/dynamic_resize.png"
        )),
        ("dynamic_resize", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/dynamic_resize.png"
        )),
        ("dynamic_resize", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/dynamic_resize.png"
        )),
        ("dynamic_resize", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/dynamic_resize.png"
        )),
        ("dynamic_resize", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/dynamic_resize.png"
        )),
        ("remote_keyboard", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/remote_keyboard.png"
        )),
        ("remote_keyboard", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/remote_keyboard.png"
        )),
        ("remote_keyboard", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/remote_keyboard.png"
        )),
        ("remote_keyboard", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/remote_keyboard.png"
        )),
        ("remote_keyboard", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/remote_keyboard.png"
        )),
        ("remote_keyboard", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/remote_keyboard.png"
        )),
        ("remote_keyboard", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/remote_keyboard.png"
        )),
        ("remote_keyboard", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/remote_keyboard.png"
        )),
        ("remote_keyboard", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/remote_keyboard.png"
        )),
        ("remote_keyboard", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/remote_keyboard.png"
        )),
        ("remote_clipboard", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/remote_clipboard.png"
        )),
        ("remote_clipboard", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/remote_clipboard.png"
        )),
        ("remote_clipboard", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/remote_clipboard.png"
        )),
        ("remote_clipboard", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/remote_clipboard.png"
        )),
        ("remote_clipboard", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/remote_clipboard.png"
        )),
        ("remote_clipboard", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/remote_clipboard.png"
        )),
        ("remote_clipboard", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/remote_clipboard.png"
        )),
        ("remote_clipboard", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/remote_clipboard.png"
        )),
        ("remote_clipboard", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/remote_clipboard.png"
        )),
        ("remote_clipboard", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/remote_clipboard.png"
        )),
        ("view_only", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/view_only.png"
        )),
        ("view_only", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/view_only.png"
        )),
        ("view_only", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/view_only.png"
        )),
        ("view_only", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/view_only.png"
        )),
        ("view_only", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/view_only.png"
        )),
        ("view_only", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/view_only.png"
        )),
        ("view_only", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/view_only.png"
        )),
        ("view_only", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/view_only.png"
        )),
        ("view_only", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/view_only.png"
        )),
        ("view_only", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/view_only.png"
        )),
        ("fullscreen", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/fullscreen.png"
        )),
        ("fullscreen", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/fullscreen.png"
        )),
        ("fullscreen", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/fullscreen.png"
        )),
        ("fullscreen", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/fullscreen.png"
        )),
        ("fullscreen", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/fullscreen.png"
        )),
        ("fullscreen", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/fullscreen.png"
        )),
        ("fullscreen", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/fullscreen.png"
        )),
        ("fullscreen", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/fullscreen.png"
        )),
        ("fullscreen", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/fullscreen.png"
        )),
        ("fullscreen", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/fullscreen.png"
        )),
        ("send_ctrl_alt_del", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/send_ctrl_alt_del.png"
        )),
        ("send_ctrl_alt_del", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/send_ctrl_alt_del.png"
        )),
        ("send_ctrl_alt_del", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/send_ctrl_alt_del.png"
        )),
        ("send_ctrl_alt_del", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/send_ctrl_alt_del.png"
        )),
        ("send_ctrl_alt_del", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/send_ctrl_alt_del.png"
        )),
        ("send_ctrl_alt_del", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/send_ctrl_alt_del.png"
        )),
        ("send_ctrl_alt_del", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/send_ctrl_alt_del.png"
        )),
        ("send_ctrl_alt_del", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/send_ctrl_alt_del.png"
        )),
        ("send_ctrl_alt_del", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/send_ctrl_alt_del.png"
        )),
        ("send_ctrl_alt_del", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/send_ctrl_alt_del.png"
        )),
        ("send_windows_key", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/send_windows_key.png"
        )),
        ("send_windows_key", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/send_windows_key.png"
        )),
        ("send_windows_key", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/send_windows_key.png"
        )),
        ("send_windows_key", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/send_windows_key.png"
        )),
        ("send_windows_key", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/send_windows_key.png"
        )),
        ("send_windows_key", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/send_windows_key.png"
        )),
        ("send_windows_key", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/send_windows_key.png"
        )),
        ("send_windows_key", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/send_windows_key.png"
        )),
        ("send_windows_key", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/send_windows_key.png"
        )),
        ("send_windows_key", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/send_windows_key.png"
        )),
        ("shared_drive", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/shared_drive.png"
        )),
        ("shared_drive", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/shared_drive.png"
        )),
        ("shared_drive", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/shared_drive.png"
        )),
        ("shared_drive", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/shared_drive.png"
        )),
        ("shared_drive", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/shared_drive.png"
        )),
        ("shared_drive", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/shared_drive.png"
        )),
        ("shared_drive", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/shared_drive.png"
        )),
        ("shared_drive", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/shared_drive.png"
        )),
        ("shared_drive", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/shared_drive.png"
        )),
        ("shared_drive", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/shared_drive.png"
        )),
        ("remote_audio", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/remote_audio.png"
        )),
        ("remote_audio", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/remote_audio.png"
        )),
        ("remote_audio", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/remote_audio.png"
        )),
        ("remote_audio", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/remote_audio.png"
        )),
        ("remote_audio", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/remote_audio.png"
        )),
        ("remote_audio", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/remote_audio.png"
        )),
        ("remote_audio", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/remote_audio.png"
        )),
        ("remote_audio", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/remote_audio.png"
        )),
        ("remote_audio", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/remote_audio.png"
        )),
        ("remote_audio", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/remote_audio.png"
        )),
        ("remote_audio_muted", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/remote_audio_muted.png"
        )),
        ("remote_audio_muted", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/remote_audio_muted.png"
        )),
        ("remote_audio_muted", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/remote_audio_muted.png"
        )),
        ("remote_audio_muted", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/remote_audio_muted.png"
        )),
        ("remote_audio_muted", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/remote_audio_muted.png"
        )),
        ("remote_audio_muted", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/remote_audio_muted.png"
        )),
        ("remote_audio_muted", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/remote_audio_muted.png"
        )),
        ("remote_audio_muted", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/remote_audio_muted.png"
        )),
        ("remote_audio_muted", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/remote_audio_muted.png"
        )),
        ("remote_audio_muted", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/remote_audio_muted.png"
        )),
        ("remote_microphone", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/remote_microphone.png"
        )),
        ("remote_microphone", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/remote_microphone.png"
        )),
        ("remote_microphone", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/remote_microphone.png"
        )),
        ("remote_microphone", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/remote_microphone.png"
        )),
        ("remote_microphone", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/remote_microphone.png"
        )),
        ("remote_microphone", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/remote_microphone.png"
        )),
        ("remote_microphone", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/remote_microphone.png"
        )),
        ("remote_microphone", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/remote_microphone.png"
        )),
        ("remote_microphone", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/remote_microphone.png"
        )),
        ("remote_microphone", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/remote_microphone.png"
        )),
        ("remote_microphone_muted", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/remote_microphone_muted.png"
        )),
        ("remote_microphone_muted", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/remote_microphone_muted.png"
        )),
        ("remote_microphone_muted", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/remote_microphone_muted.png"
        )),
        ("remote_microphone_muted", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/remote_microphone_muted.png"
        )),
        ("remote_microphone_muted", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/remote_microphone_muted.png"
        )),
        ("remote_microphone_muted", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/remote_microphone_muted.png"
        )),
        ("remote_microphone_muted", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/remote_microphone_muted.png"
        )),
        ("remote_microphone_muted", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/remote_microphone_muted.png"
        )),
        ("remote_microphone_muted", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/remote_microphone_muted.png"
        )),
        ("remote_microphone_muted", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/remote_microphone_muted.png"
        )),
        ("input_enabled", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/input_enabled.png"
        )),
        ("input_enabled", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/input_enabled.png"
        )),
        ("input_enabled", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/input_enabled.png"
        )),
        ("input_enabled", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/input_enabled.png"
        )),
        ("input_enabled", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/input_enabled.png"
        )),
        ("input_enabled", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/input_enabled.png"
        )),
        ("input_enabled", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/input_enabled.png"
        )),
        ("input_enabled", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/input_enabled.png"
        )),
        ("input_enabled", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/input_enabled.png"
        )),
        ("input_enabled", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/input_enabled.png"
        )),
        ("session_telemetry", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/session_telemetry.png"
        )),
        ("session_telemetry", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/session_telemetry.png"
        )),
        ("session_telemetry", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/session_telemetry.png"
        )),
        ("session_telemetry", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/session_telemetry.png"
        )),
        ("session_telemetry", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/session_telemetry.png"
        )),
        ("session_telemetry", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/session_telemetry.png"
        )),
        ("session_telemetry", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/session_telemetry.png"
        )),
        ("session_telemetry", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/session_telemetry.png"
        )),
        ("session_telemetry", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/session_telemetry.png"
        )),
        ("session_telemetry", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/session_telemetry.png"
        )),
        ("upload", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/upload.png"
        )),
        ("upload", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/upload.png"
        )),
        ("upload", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/upload.png"
        )),
        ("upload", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/upload.png"
        )),
        ("upload", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/upload.png"
        )),
        ("upload", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/upload.png"
        )),
        ("upload", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/upload.png"
        )),
        ("upload", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/upload.png"
        )),
        ("upload", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/upload.png"
        )),
        ("upload", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/upload.png"
        )),
        ("download", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/download.png"
        )),
        ("download", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/download.png"
        )),
        ("download", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/download.png"
        )),
        ("download", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/download.png"
        )),
        ("download", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/download.png"
        )),
        ("download", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/download.png"
        )),
        ("download", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/download.png"
        )),
        ("download", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/download.png"
        )),
        ("download", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/download.png"
        )),
        ("download", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/download.png"
        )),
        ("transfer_queue", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/transfer_queue.png"
        )),
        ("transfer_queue", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/transfer_queue.png"
        )),
        ("transfer_queue", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/transfer_queue.png"
        )),
        ("transfer_queue", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/transfer_queue.png"
        )),
        ("transfer_queue", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/transfer_queue.png"
        )),
        ("transfer_queue", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/transfer_queue.png"
        )),
        ("transfer_queue", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/transfer_queue.png"
        )),
        ("transfer_queue", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/transfer_queue.png"
        )),
        ("transfer_queue", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/transfer_queue.png"
        )),
        ("transfer_queue", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/transfer_queue.png"
        )),
        ("new_folder", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/new_folder.png"
        )),
        ("new_folder", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/new_folder.png"
        )),
        ("new_folder", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/new_folder.png"
        )),
        ("new_folder", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/new_folder.png"
        )),
        ("new_folder", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/new_folder.png"
        )),
        ("new_folder", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/new_folder.png"
        )),
        ("new_folder", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/new_folder.png"
        )),
        ("new_folder", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/new_folder.png"
        )),
        ("new_folder", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/new_folder.png"
        )),
        ("new_folder", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/new_folder.png"
        )),
        ("transfer_progress", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/transfer_progress.png"
        )),
        ("transfer_progress", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/transfer_progress.png"
        )),
        ("transfer_progress", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/transfer_progress.png"
        )),
        ("transfer_progress", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/transfer_progress.png"
        )),
        ("transfer_progress", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/transfer_progress.png"
        )),
        ("transfer_progress", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/transfer_progress.png"
        )),
        ("transfer_progress", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/transfer_progress.png"
        )),
        ("transfer_progress", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/transfer_progress.png"
        )),
        ("transfer_progress", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/transfer_progress.png"
        )),
        ("transfer_progress", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/transfer_progress.png"
        )),
        ("transfer_complete", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/transfer_complete.png"
        )),
        ("transfer_complete", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/transfer_complete.png"
        )),
        ("transfer_complete", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/transfer_complete.png"
        )),
        ("transfer_complete", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/transfer_complete.png"
        )),
        ("transfer_complete", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/transfer_complete.png"
        )),
        ("transfer_complete", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/transfer_complete.png"
        )),
        ("transfer_complete", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/transfer_complete.png"
        )),
        ("transfer_complete", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/transfer_complete.png"
        )),
        ("transfer_complete", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/transfer_complete.png"
        )),
        ("transfer_complete", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/transfer_complete.png"
        )),
        ("transfer_failed", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/transfer_failed.png"
        )),
        ("transfer_failed", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/transfer_failed.png"
        )),
        ("transfer_failed", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/transfer_failed.png"
        )),
        ("transfer_failed", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/transfer_failed.png"
        )),
        ("transfer_failed", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/transfer_failed.png"
        )),
        ("transfer_failed", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/transfer_failed.png"
        )),
        ("transfer_failed", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/transfer_failed.png"
        )),
        ("transfer_failed", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/transfer_failed.png"
        )),
        ("transfer_failed", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/transfer_failed.png"
        )),
        ("transfer_failed", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/transfer_failed.png"
        )),
        ("transfer_retry", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/transfer_retry.png"
        )),
        ("transfer_retry", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/transfer_retry.png"
        )),
        ("transfer_retry", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/transfer_retry.png"
        )),
        ("transfer_retry", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/transfer_retry.png"
        )),
        ("transfer_retry", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/transfer_retry.png"
        )),
        ("transfer_retry", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/transfer_retry.png"
        )),
        ("transfer_retry", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/transfer_retry.png"
        )),
        ("transfer_retry", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/transfer_retry.png"
        )),
        ("transfer_retry", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/transfer_retry.png"
        )),
        ("transfer_retry", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/transfer_retry.png"
        )),
        ("local_files", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/local_files.png"
        )),
        ("local_files", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/local_files.png"
        )),
        ("local_files", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/local_files.png"
        )),
        ("local_files", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/local_files.png"
        )),
        ("local_files", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/local_files.png"
        )),
        ("local_files", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/local_files.png"
        )),
        ("local_files", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/local_files.png"
        )),
        ("local_files", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/local_files.png"
        )),
        ("local_files", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/local_files.png"
        )),
        ("local_files", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/local_files.png"
        )),
        ("remote_files", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/remote_files.png"
        )),
        ("remote_files", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/remote_files.png"
        )),
        ("remote_files", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/remote_files.png"
        )),
        ("remote_files", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/remote_files.png"
        )),
        ("remote_files", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/remote_files.png"
        )),
        ("remote_files", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/remote_files.png"
        )),
        ("remote_files", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/remote_files.png"
        )),
        ("remote_files", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/remote_files.png"
        )),
        ("remote_files", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/remote_files.png"
        )),
        ("remote_files", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/remote_files.png"
        )),
        ("file_permissions", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/file_permissions.png"
        )),
        ("file_permissions", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/file_permissions.png"
        )),
        ("file_permissions", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/file_permissions.png"
        )),
        ("file_permissions", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/file_permissions.png"
        )),
        ("file_permissions", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/file_permissions.png"
        )),
        ("file_permissions", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/file_permissions.png"
        )),
        ("file_permissions", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/file_permissions.png"
        )),
        ("file_permissions", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/file_permissions.png"
        )),
        ("file_permissions", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/file_permissions.png"
        )),
        ("file_permissions", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/file_permissions.png"
        )),
        ("hidden_files", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/hidden_files.png"
        )),
        ("hidden_files", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/hidden_files.png"
        )),
        ("hidden_files", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/hidden_files.png"
        )),
        ("hidden_files", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/hidden_files.png"
        )),
        ("hidden_files", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/hidden_files.png"
        )),
        ("hidden_files", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/hidden_files.png"
        )),
        ("hidden_files", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/hidden_files.png"
        )),
        ("hidden_files", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/hidden_files.png"
        )),
        ("hidden_files", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/hidden_files.png"
        )),
        ("hidden_files", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/hidden_files.png"
        )),
        ("vault_lock", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/vault_lock.png"
        )),
        ("vault_lock", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/vault_lock.png"
        )),
        ("vault_lock", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/vault_lock.png"
        )),
        ("vault_lock", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/vault_lock.png"
        )),
        ("vault_lock", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/vault_lock.png"
        )),
        ("vault_lock", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/vault_lock.png"
        )),
        ("vault_lock", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/vault_lock.png"
        )),
        ("vault_lock", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/vault_lock.png"
        )),
        ("vault_lock", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/vault_lock.png"
        )),
        ("vault_lock", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/vault_lock.png"
        )),
        ("vault_unlock", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/vault_unlock.png"
        )),
        ("vault_unlock", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/vault_unlock.png"
        )),
        ("vault_unlock", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/vault_unlock.png"
        )),
        ("vault_unlock", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/vault_unlock.png"
        )),
        ("vault_unlock", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/vault_unlock.png"
        )),
        ("vault_unlock", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/vault_unlock.png"
        )),
        ("vault_unlock", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/vault_unlock.png"
        )),
        ("vault_unlock", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/vault_unlock.png"
        )),
        ("vault_unlock", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/vault_unlock.png"
        )),
        ("vault_unlock", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/vault_unlock.png"
        )),
        ("google_drive", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/google_drive.png"
        )),
        ("google_drive", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/google_drive.png"
        )),
        ("google_drive", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/google_drive.png"
        )),
        ("google_drive", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/google_drive.png"
        )),
        ("google_drive", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/google_drive.png"
        )),
        ("google_drive", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/google_drive.png"
        )),
        ("google_drive", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/google_drive.png"
        )),
        ("google_drive", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/google_drive.png"
        )),
        ("google_drive", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/google_drive.png"
        )),
        ("google_drive", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/google_drive.png"
        )),
        ("sync_conflict", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/sync_conflict.png"
        )),
        ("sync_conflict", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/sync_conflict.png"
        )),
        ("sync_conflict", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/sync_conflict.png"
        )),
        ("sync_conflict", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/sync_conflict.png"
        )),
        ("sync_conflict", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/sync_conflict.png"
        )),
        ("sync_conflict", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/sync_conflict.png"
        )),
        ("sync_conflict", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/sync_conflict.png"
        )),
        ("sync_conflict", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/sync_conflict.png"
        )),
        ("sync_conflict", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/sync_conflict.png"
        )),
        ("sync_conflict", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/sync_conflict.png"
        )),
        ("encrypted_backup", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/encrypted_backup.png"
        )),
        ("encrypted_backup", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/encrypted_backup.png"
        )),
        ("encrypted_backup", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/encrypted_backup.png"
        )),
        ("encrypted_backup", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/encrypted_backup.png"
        )),
        ("encrypted_backup", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/encrypted_backup.png"
        )),
        ("encrypted_backup", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/encrypted_backup.png"
        )),
        ("encrypted_backup", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/encrypted_backup.png"
        )),
        ("encrypted_backup", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/encrypted_backup.png"
        )),
        ("encrypted_backup", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/encrypted_backup.png"
        )),
        ("encrypted_backup", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/encrypted_backup.png"
        )),
        ("restore_backup", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/restore_backup.png"
        )),
        ("restore_backup", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/restore_backup.png"
        )),
        ("restore_backup", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/restore_backup.png"
        )),
        ("restore_backup", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/restore_backup.png"
        )),
        ("restore_backup", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/restore_backup.png"
        )),
        ("restore_backup", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/restore_backup.png"
        )),
        ("restore_backup", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/restore_backup.png"
        )),
        ("restore_backup", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/restore_backup.png"
        )),
        ("restore_backup", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/restore_backup.png"
        )),
        ("restore_backup", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/restore_backup.png"
        )),
        ("recovery_code", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/recovery_code.png"
        )),
        ("recovery_code", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/recovery_code.png"
        )),
        ("recovery_code", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/recovery_code.png"
        )),
        ("recovery_code", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/recovery_code.png"
        )),
        ("recovery_code", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/recovery_code.png"
        )),
        ("recovery_code", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/recovery_code.png"
        )),
        ("recovery_code", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/recovery_code.png"
        )),
        ("recovery_code", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/recovery_code.png"
        )),
        ("recovery_code", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/recovery_code.png"
        )),
        ("recovery_code", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/recovery_code.png"
        )),
        ("vault_encryption", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/vault_encryption.png"
        )),
        ("vault_encryption", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/vault_encryption.png"
        )),
        ("vault_encryption", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/vault_encryption.png"
        )),
        ("vault_encryption", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/vault_encryption.png"
        )),
        ("vault_encryption", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/vault_encryption.png"
        )),
        ("vault_encryption", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/vault_encryption.png"
        )),
        ("vault_encryption", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/vault_encryption.png"
        )),
        ("vault_encryption", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/vault_encryption.png"
        )),
        ("vault_encryption", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/vault_encryption.png"
        )),
        ("vault_encryption", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/vault_encryption.png"
        )),
        ("server_sync", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/server_sync.png"
        )),
        ("server_sync", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/server_sync.png"
        )),
        ("server_sync", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/server_sync.png"
        )),
        ("server_sync", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/server_sync.png"
        )),
        ("server_sync", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/server_sync.png"
        )),
        ("server_sync", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/server_sync.png"
        )),
        ("server_sync", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/server_sync.png"
        )),
        ("server_sync", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/server_sync.png"
        )),
        ("server_sync", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/server_sync.png"
        )),
        ("server_sync", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/server_sync.png"
        )),
        ("sync_upload", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/sync_upload.png"
        )),
        ("sync_upload", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/sync_upload.png"
        )),
        ("sync_upload", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/sync_upload.png"
        )),
        ("sync_upload", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/sync_upload.png"
        )),
        ("sync_upload", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/sync_upload.png"
        )),
        ("sync_upload", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/sync_upload.png"
        )),
        ("sync_upload", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/sync_upload.png"
        )),
        ("sync_upload", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/sync_upload.png"
        )),
        ("sync_upload", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/sync_upload.png"
        )),
        ("sync_upload", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/sync_upload.png"
        )),
        ("sync_download", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/sync_download.png"
        )),
        ("sync_download", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/sync_download.png"
        )),
        ("sync_download", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/sync_download.png"
        )),
        ("sync_download", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/sync_download.png"
        )),
        ("sync_download", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/sync_download.png"
        )),
        ("sync_download", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/sync_download.png"
        )),
        ("sync_download", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/sync_download.png"
        )),
        ("sync_download", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/sync_download.png"
        )),
        ("sync_download", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/sync_download.png"
        )),
        ("sync_download", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/sync_download.png"
        )),
        ("sync_offline", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/sync_offline.png"
        )),
        ("sync_offline", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/sync_offline.png"
        )),
        ("sync_offline", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/sync_offline.png"
        )),
        ("sync_offline", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/sync_offline.png"
        )),
        ("sync_offline", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/sync_offline.png"
        )),
        ("sync_offline", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/sync_offline.png"
        )),
        ("sync_offline", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/sync_offline.png"
        )),
        ("sync_offline", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/sync_offline.png"
        )),
        ("sync_offline", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/sync_offline.png"
        )),
        ("sync_offline", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/sync_offline.png"
        )),
        ("sync_success", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/sync_success.png"
        )),
        ("sync_success", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/sync_success.png"
        )),
        ("sync_success", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/sync_success.png"
        )),
        ("sync_success", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/sync_success.png"
        )),
        ("sync_success", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/sync_success.png"
        )),
        ("sync_success", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/sync_success.png"
        )),
        ("sync_success", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/sync_success.png"
        )),
        ("sync_success", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/sync_success.png"
        )),
        ("sync_success", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/sync_success.png"
        )),
        ("sync_success", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/sync_success.png"
        )),
        ("setup_resume", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/setup_resume.png"
        )),
        ("setup_resume", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/setup_resume.png"
        )),
        ("setup_resume", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/setup_resume.png"
        )),
        ("setup_resume", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/setup_resume.png"
        )),
        ("setup_resume", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/setup_resume.png"
        )),
        ("setup_resume", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/setup_resume.png"
        )),
        ("setup_resume", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/setup_resume.png"
        )),
        ("setup_resume", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/setup_resume.png"
        )),
        ("setup_resume", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/setup_resume.png"
        )),
        ("setup_resume", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/setup_resume.png"
        )),
        ("device_enrollment", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/device_enrollment.png"
        )),
        ("device_enrollment", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/device_enrollment.png"
        )),
        ("device_enrollment", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/device_enrollment.png"
        )),
        ("device_enrollment", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/device_enrollment.png"
        )),
        ("device_enrollment", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/device_enrollment.png"
        )),
        ("device_enrollment", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/device_enrollment.png"
        )),
        ("device_enrollment", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/device_enrollment.png"
        )),
        ("device_enrollment", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/device_enrollment.png"
        )),
        ("device_enrollment", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/device_enrollment.png"
        )),
        ("device_enrollment", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/device_enrollment.png"
        )),
        ("host_key_verified", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/host_key_verified.png"
        )),
        ("host_key_verified", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/host_key_verified.png"
        )),
        ("host_key_verified", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/host_key_verified.png"
        )),
        ("host_key_verified", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/host_key_verified.png"
        )),
        ("host_key_verified", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/host_key_verified.png"
        )),
        ("host_key_verified", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/host_key_verified.png"
        )),
        ("host_key_verified", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/host_key_verified.png"
        )),
        ("host_key_verified", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/host_key_verified.png"
        )),
        ("host_key_verified", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/host_key_verified.png"
        )),
        ("host_key_verified", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/host_key_verified.png"
        )),
        ("host_key_changed", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/host_key_changed.png"
        )),
        ("host_key_changed", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/host_key_changed.png"
        )),
        ("host_key_changed", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/host_key_changed.png"
        )),
        ("host_key_changed", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/host_key_changed.png"
        )),
        ("host_key_changed", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/host_key_changed.png"
        )),
        ("host_key_changed", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/host_key_changed.png"
        )),
        ("host_key_changed", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/host_key_changed.png"
        )),
        ("host_key_changed", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/host_key_changed.png"
        )),
        ("host_key_changed", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/host_key_changed.png"
        )),
        ("host_key_changed", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/host_key_changed.png"
        )),
        ("ssh_agent", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/ssh_agent.png"
        )),
        ("ssh_agent", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/ssh_agent.png"
        )),
        ("ssh_agent", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/ssh_agent.png"
        )),
        ("ssh_agent", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/ssh_agent.png"
        )),
        ("ssh_agent", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/ssh_agent.png"
        )),
        ("ssh_agent", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/ssh_agent.png"
        )),
        ("ssh_agent", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/ssh_agent.png"
        )),
        ("ssh_agent", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/ssh_agent.png"
        )),
        ("ssh_agent", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/ssh_agent.png"
        )),
        ("ssh_agent", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/ssh_agent.png"
        )),
        ("generate_key", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/generate_key.png"
        )),
        ("generate_key", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/generate_key.png"
        )),
        ("generate_key", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/generate_key.png"
        )),
        ("generate_key", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/generate_key.png"
        )),
        ("generate_key", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/generate_key.png"
        )),
        ("generate_key", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/generate_key.png"
        )),
        ("generate_key", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/generate_key.png"
        )),
        ("generate_key", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/generate_key.png"
        )),
        ("generate_key", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/generate_key.png"
        )),
        ("generate_key", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/generate_key.png"
        )),
        ("rotate_key", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/rotate_key.png"
        )),
        ("rotate_key", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/rotate_key.png"
        )),
        ("rotate_key", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/rotate_key.png"
        )),
        ("rotate_key", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/rotate_key.png"
        )),
        ("rotate_key", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/rotate_key.png"
        )),
        ("rotate_key", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/rotate_key.png"
        )),
        ("rotate_key", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/rotate_key.png"
        )),
        ("rotate_key", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/rotate_key.png"
        )),
        ("rotate_key", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/rotate_key.png"
        )),
        ("rotate_key", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/rotate_key.png"
        )),
        ("revoke_key", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/revoke_key.png"
        )),
        ("revoke_key", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/revoke_key.png"
        )),
        ("revoke_key", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/revoke_key.png"
        )),
        ("revoke_key", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/revoke_key.png"
        )),
        ("revoke_key", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/revoke_key.png"
        )),
        ("revoke_key", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/revoke_key.png"
        )),
        ("revoke_key", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/revoke_key.png"
        )),
        ("revoke_key", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/revoke_key.png"
        )),
        ("revoke_key", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/revoke_key.png"
        )),
        ("revoke_key", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/revoke_key.png"
        )),
        ("password", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/password.png"
        )),
        ("password", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/password.png"
        )),
        ("password", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/password.png"
        )),
        ("password", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/password.png"
        )),
        ("password", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/password.png"
        )),
        ("password", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/password.png"
        )),
        ("password", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/password.png"
        )),
        ("password", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/password.png"
        )),
        ("password", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/password.png"
        )),
        ("password", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/password.png"
        )),
        ("security_audit", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/security_audit.png"
        )),
        ("security_audit", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/security_audit.png"
        )),
        ("security_audit", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/security_audit.png"
        )),
        ("security_audit", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/security_audit.png"
        )),
        ("security_audit", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/security_audit.png"
        )),
        ("security_audit", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/security_audit.png"
        )),
        ("security_audit", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/security_audit.png"
        )),
        ("security_audit", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/security_audit.png"
        )),
        ("security_audit", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/security_audit.png"
        )),
        ("security_audit", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/security_audit.png"
        )),
        ("trusted_device", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/trusted_device.png"
        )),
        ("trusted_device", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/trusted_device.png"
        )),
        ("trusted_device", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/trusted_device.png"
        )),
        ("trusted_device", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/trusted_device.png"
        )),
        ("trusted_device", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/trusted_device.png"
        )),
        ("trusted_device", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/trusted_device.png"
        )),
        ("trusted_device", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/trusted_device.png"
        )),
        ("trusted_device", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/trusted_device.png"
        )),
        ("trusted_device", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/trusted_device.png"
        )),
        ("trusted_device", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/trusted_device.png"
        )),
        ("untrusted_device", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/untrusted_device.png"
        )),
        ("untrusted_device", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/untrusted_device.png"
        )),
        ("untrusted_device", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/untrusted_device.png"
        )),
        ("untrusted_device", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/untrusted_device.png"
        )),
        ("untrusted_device", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/untrusted_device.png"
        )),
        ("untrusted_device", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/untrusted_device.png"
        )),
        ("untrusted_device", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/untrusted_device.png"
        )),
        ("untrusted_device", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/untrusted_device.png"
        )),
        ("untrusted_device", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/untrusted_device.png"
        )),
        ("untrusted_device", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/untrusted_device.png"
        )),
        ("team_members", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/team_members.png"
        )),
        ("team_members", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/team_members.png"
        )),
        ("team_members", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/team_members.png"
        )),
        ("team_members", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/team_members.png"
        )),
        ("team_members", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/team_members.png"
        )),
        ("team_members", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/team_members.png"
        )),
        ("team_members", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/team_members.png"
        )),
        ("team_members", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/team_members.png"
        )),
        ("team_members", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/team_members.png"
        )),
        ("team_members", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/team_members.png"
        )),
        ("shared_vault", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/shared_vault.png"
        )),
        ("shared_vault", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/shared_vault.png"
        )),
        ("shared_vault", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/shared_vault.png"
        )),
        ("shared_vault", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/shared_vault.png"
        )),
        ("shared_vault", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/shared_vault.png"
        )),
        ("shared_vault", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/shared_vault.png"
        )),
        ("shared_vault", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/shared_vault.png"
        )),
        ("shared_vault", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/shared_vault.png"
        )),
        ("shared_vault", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/shared_vault.png"
        )),
        ("shared_vault", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/shared_vault.png"
        )),
        ("member_revoke", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/member_revoke.png"
        )),
        ("member_revoke", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/member_revoke.png"
        )),
        ("member_revoke", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/member_revoke.png"
        )),
        ("member_revoke", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/member_revoke.png"
        )),
        ("member_revoke", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/member_revoke.png"
        )),
        ("member_revoke", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/member_revoke.png"
        )),
        ("member_revoke", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/member_revoke.png"
        )),
        ("member_revoke", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/member_revoke.png"
        )),
        ("member_revoke", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/member_revoke.png"
        )),
        ("member_revoke", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/member_revoke.png"
        )),
        ("pending_invite", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/pending_invite.png"
        )),
        ("pending_invite", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/pending_invite.png"
        )),
        ("pending_invite", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/pending_invite.png"
        )),
        ("pending_invite", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/pending_invite.png"
        )),
        ("pending_invite", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/pending_invite.png"
        )),
        ("pending_invite", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/pending_invite.png"
        )),
        ("pending_invite", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/pending_invite.png"
        )),
        ("pending_invite", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/pending_invite.png"
        )),
        ("pending_invite", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/pending_invite.png"
        )),
        ("pending_invite", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/pending_invite.png"
        )),
        ("role_admin", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/role_admin.png"
        )),
        ("role_admin", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/role_admin.png"
        )),
        ("role_admin", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/role_admin.png"
        )),
        ("role_admin", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/role_admin.png"
        )),
        ("role_admin", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/role_admin.png"
        )),
        ("role_admin", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/role_admin.png"
        )),
        ("role_admin", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/role_admin.png"
        )),
        ("role_admin", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/role_admin.png"
        )),
        ("role_admin", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/role_admin.png"
        )),
        ("role_admin", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/role_admin.png"
        )),
        ("role_editor", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/role_editor.png"
        )),
        ("role_editor", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/role_editor.png"
        )),
        ("role_editor", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/role_editor.png"
        )),
        ("role_editor", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/role_editor.png"
        )),
        ("role_editor", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/role_editor.png"
        )),
        ("role_editor", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/role_editor.png"
        )),
        ("role_editor", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/role_editor.png"
        )),
        ("role_editor", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/role_editor.png"
        )),
        ("role_editor", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/role_editor.png"
        )),
        ("role_editor", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/role_editor.png"
        )),
        ("role_viewer", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/role_viewer.png"
        )),
        ("role_viewer", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/role_viewer.png"
        )),
        ("role_viewer", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/role_viewer.png"
        )),
        ("role_viewer", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/role_viewer.png"
        )),
        ("role_viewer", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/role_viewer.png"
        )),
        ("role_viewer", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/role_viewer.png"
        )),
        ("role_viewer", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/role_viewer.png"
        )),
        ("role_viewer", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/role_viewer.png"
        )),
        ("role_viewer", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/role_viewer.png"
        )),
        ("role_viewer", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/role_viewer.png"
        )),
        ("team_key_rotation", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/team_key_rotation.png"
        )),
        ("team_key_rotation", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/team_key_rotation.png"
        )),
        ("team_key_rotation", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/team_key_rotation.png"
        )),
        ("team_key_rotation", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/team_key_rotation.png"
        )),
        ("team_key_rotation", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/team_key_rotation.png"
        )),
        ("team_key_rotation", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/team_key_rotation.png"
        )),
        ("team_key_rotation", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/team_key_rotation.png"
        )),
        ("team_key_rotation", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/team_key_rotation.png"
        )),
        ("team_key_rotation", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/team_key_rotation.png"
        )),
        ("team_key_rotation", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/team_key_rotation.png"
        )),
        ("tunnel_local", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/tunnel_local.png"
        )),
        ("tunnel_local", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/tunnel_local.png"
        )),
        ("tunnel_local", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/tunnel_local.png"
        )),
        ("tunnel_local", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/tunnel_local.png"
        )),
        ("tunnel_local", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/tunnel_local.png"
        )),
        ("tunnel_local", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/tunnel_local.png"
        )),
        ("tunnel_local", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/tunnel_local.png"
        )),
        ("tunnel_local", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/tunnel_local.png"
        )),
        ("tunnel_local", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/tunnel_local.png"
        )),
        ("tunnel_local", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/tunnel_local.png"
        )),
        ("tunnel_remote", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/tunnel_remote.png"
        )),
        ("tunnel_remote", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/tunnel_remote.png"
        )),
        ("tunnel_remote", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/tunnel_remote.png"
        )),
        ("tunnel_remote", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/tunnel_remote.png"
        )),
        ("tunnel_remote", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/tunnel_remote.png"
        )),
        ("tunnel_remote", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/tunnel_remote.png"
        )),
        ("tunnel_remote", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/tunnel_remote.png"
        )),
        ("tunnel_remote", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/tunnel_remote.png"
        )),
        ("tunnel_remote", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/tunnel_remote.png"
        )),
        ("tunnel_remote", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/tunnel_remote.png"
        )),
        ("tunnel_socks", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/tunnel_socks.png"
        )),
        ("tunnel_socks", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/tunnel_socks.png"
        )),
        ("tunnel_socks", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/tunnel_socks.png"
        )),
        ("tunnel_socks", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/tunnel_socks.png"
        )),
        ("tunnel_socks", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/tunnel_socks.png"
        )),
        ("tunnel_socks", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/tunnel_socks.png"
        )),
        ("tunnel_socks", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/tunnel_socks.png"
        )),
        ("tunnel_socks", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/tunnel_socks.png"
        )),
        ("tunnel_socks", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/tunnel_socks.png"
        )),
        ("tunnel_socks", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/tunnel_socks.png"
        )),
        ("tunnel_active", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/tunnel_active.png"
        )),
        ("tunnel_active", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/tunnel_active.png"
        )),
        ("tunnel_active", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/tunnel_active.png"
        )),
        ("tunnel_active", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/tunnel_active.png"
        )),
        ("tunnel_active", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/tunnel_active.png"
        )),
        ("tunnel_active", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/tunnel_active.png"
        )),
        ("tunnel_active", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/tunnel_active.png"
        )),
        ("tunnel_active", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/tunnel_active.png"
        )),
        ("tunnel_active", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/tunnel_active.png"
        )),
        ("tunnel_active", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/tunnel_active.png"
        )),
        ("tunnel_stopped", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/tunnel_stopped.png"
        )),
        ("tunnel_stopped", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/tunnel_stopped.png"
        )),
        ("tunnel_stopped", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/tunnel_stopped.png"
        )),
        ("tunnel_stopped", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/tunnel_stopped.png"
        )),
        ("tunnel_stopped", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/tunnel_stopped.png"
        )),
        ("tunnel_stopped", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/tunnel_stopped.png"
        )),
        ("tunnel_stopped", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/tunnel_stopped.png"
        )),
        ("tunnel_stopped", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/tunnel_stopped.png"
        )),
        ("tunnel_stopped", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/tunnel_stopped.png"
        )),
        ("tunnel_stopped", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/tunnel_stopped.png"
        )),
        ("provider_aws", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/provider_aws.png"
        )),
        ("provider_aws", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/provider_aws.png"
        )),
        ("provider_aws", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/provider_aws.png"
        )),
        ("provider_aws", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/provider_aws.png"
        )),
        ("provider_aws", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/provider_aws.png"
        )),
        ("provider_aws", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/provider_aws.png"
        )),
        ("provider_aws", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/provider_aws.png"
        )),
        ("provider_aws", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/provider_aws.png"
        )),
        ("provider_aws", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/provider_aws.png"
        )),
        ("provider_aws", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/provider_aws.png"
        )),
        ("provider_azure", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/provider_azure.png"
        )),
        ("provider_azure", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/provider_azure.png"
        )),
        ("provider_azure", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/provider_azure.png"
        )),
        ("provider_azure", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/provider_azure.png"
        )),
        ("provider_azure", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/provider_azure.png"
        )),
        ("provider_azure", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/provider_azure.png"
        )),
        ("provider_azure", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/provider_azure.png"
        )),
        ("provider_azure", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/provider_azure.png"
        )),
        ("provider_azure", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/provider_azure.png"
        )),
        ("provider_azure", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/provider_azure.png"
        )),
        ("provider_digitalocean", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/provider_digitalocean.png"
        )),
        ("provider_digitalocean", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/provider_digitalocean.png"
        )),
        ("provider_digitalocean", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/provider_digitalocean.png"
        )),
        ("provider_digitalocean", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/provider_digitalocean.png"
        )),
        ("provider_digitalocean", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/provider_digitalocean.png"
        )),
        ("provider_digitalocean", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/provider_digitalocean.png"
        )),
        ("provider_digitalocean", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/provider_digitalocean.png"
        )),
        ("provider_digitalocean", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/provider_digitalocean.png"
        )),
        ("provider_digitalocean", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/provider_digitalocean.png"
        )),
        ("provider_digitalocean", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/provider_digitalocean.png"
        )),
        ("provider_ansible", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/provider_ansible.png"
        )),
        ("provider_ansible", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/provider_ansible.png"
        )),
        ("provider_ansible", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/provider_ansible.png"
        )),
        ("provider_ansible", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/provider_ansible.png"
        )),
        ("provider_ansible", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/provider_ansible.png"
        )),
        ("provider_ansible", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/provider_ansible.png"
        )),
        ("provider_ansible", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/provider_ansible.png"
        )),
        ("provider_ansible", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/provider_ansible.png"
        )),
        ("provider_ansible", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/provider_ansible.png"
        )),
        ("provider_ansible", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/provider_ansible.png"
        )),
        ("provider_connect", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/provider_connect.png"
        )),
        ("provider_connect", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/provider_connect.png"
        )),
        ("provider_connect", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/provider_connect.png"
        )),
        ("provider_connect", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/provider_connect.png"
        )),
        ("provider_connect", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/provider_connect.png"
        )),
        ("provider_connect", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/provider_connect.png"
        )),
        ("provider_connect", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/provider_connect.png"
        )),
        ("provider_connect", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/provider_connect.png"
        )),
        ("provider_connect", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/provider_connect.png"
        )),
        ("provider_connect", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/provider_connect.png"
        )),
        ("discover_hosts", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/discover_hosts.png"
        )),
        ("discover_hosts", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/discover_hosts.png"
        )),
        ("discover_hosts", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/discover_hosts.png"
        )),
        ("discover_hosts", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/discover_hosts.png"
        )),
        ("discover_hosts", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/discover_hosts.png"
        )),
        ("discover_hosts", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/discover_hosts.png"
        )),
        ("discover_hosts", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/discover_hosts.png"
        )),
        ("discover_hosts", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/discover_hosts.png"
        )),
        ("discover_hosts", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/discover_hosts.png"
        )),
        ("discover_hosts", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/discover_hosts.png"
        )),
        ("inventory", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/inventory.png"
        )),
        ("inventory", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/inventory.png"
        )),
        ("inventory", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/inventory.png"
        )),
        ("inventory", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/inventory.png"
        )),
        ("inventory", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/inventory.png"
        )),
        ("inventory", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/inventory.png"
        )),
        ("inventory", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/inventory.png"
        )),
        ("inventory", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/inventory.png"
        )),
        ("inventory", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/inventory.png"
        )),
        ("inventory", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/inventory.png"
        )),
        ("inventory_sync", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/inventory_sync.png"
        )),
        ("inventory_sync", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/inventory_sync.png"
        )),
        ("inventory_sync", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/inventory_sync.png"
        )),
        ("inventory_sync", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/inventory_sync.png"
        )),
        ("inventory_sync", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/inventory_sync.png"
        )),
        ("inventory_sync", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/inventory_sync.png"
        )),
        ("inventory_sync", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/inventory_sync.png"
        )),
        ("inventory_sync", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/inventory_sync.png"
        )),
        ("inventory_sync", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/inventory_sync.png"
        )),
        ("inventory_sync", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/inventory_sync.png"
        )),
        ("health_check", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/health_check.png"
        )),
        ("health_check", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/health_check.png"
        )),
        ("health_check", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/health_check.png"
        )),
        ("health_check", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/health_check.png"
        )),
        ("health_check", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/health_check.png"
        )),
        ("health_check", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/health_check.png"
        )),
        ("health_check", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/health_check.png"
        )),
        ("health_check", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/health_check.png"
        )),
        ("health_check", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/health_check.png"
        )),
        ("health_check", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/health_check.png"
        )),
        ("benchmark", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/benchmark.png"
        )),
        ("benchmark", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/benchmark.png"
        )),
        ("benchmark", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/benchmark.png"
        )),
        ("benchmark", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/benchmark.png"
        )),
        ("benchmark", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/benchmark.png"
        )),
        ("benchmark", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/benchmark.png"
        )),
        ("benchmark", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/benchmark.png"
        )),
        ("benchmark", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/benchmark.png"
        )),
        ("benchmark", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/benchmark.png"
        )),
        ("benchmark", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/benchmark.png"
        )),
        ("latency", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/latency.png"
        )),
        ("latency", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/latency.png"
        )),
        ("latency", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/latency.png"
        )),
        ("latency", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/latency.png"
        )),
        ("latency", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/latency.png"
        )),
        ("latency", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/latency.png"
        )),
        ("latency", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/latency.png"
        )),
        ("latency", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/latency.png"
        )),
        ("latency", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/latency.png"
        )),
        ("latency", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/latency.png"
        )),
        ("network_test", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/network_test.png"
        )),
        ("network_test", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/network_test.png"
        )),
        ("network_test", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/network_test.png"
        )),
        ("network_test", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/network_test.png"
        )),
        ("network_test", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/network_test.png"
        )),
        ("network_test", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/network_test.png"
        )),
        ("network_test", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/network_test.png"
        )),
        ("network_test", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/network_test.png"
        )),
        ("network_test", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/network_test.png"
        )),
        ("network_test", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/network_test.png"
        )),
        ("storage_test", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/storage_test.png"
        )),
        ("storage_test", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/storage_test.png"
        )),
        ("storage_test", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/storage_test.png"
        )),
        ("storage_test", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/storage_test.png"
        )),
        ("storage_test", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/storage_test.png"
        )),
        ("storage_test", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/storage_test.png"
        )),
        ("storage_test", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/storage_test.png"
        )),
        ("storage_test", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/storage_test.png"
        )),
        ("storage_test", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/storage_test.png"
        )),
        ("storage_test", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/storage_test.png"
        )),
        ("os_debian", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_debian.png"
        )),
        ("os_debian", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_debian.png"
        )),
        ("os_debian", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_debian.png"
        )),
        ("os_debian", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_debian.png"
        )),
        ("os_debian", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_debian.png"
        )),
        ("os_debian", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_debian.png"
        )),
        ("os_debian", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_debian.png"
        )),
        ("os_debian", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_debian.png"
        )),
        ("os_debian", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_debian.png"
        )),
        ("os_debian", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_debian.png"
        )),
        ("os_fedora", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_fedora.png"
        )),
        ("os_fedora", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_fedora.png"
        )),
        ("os_fedora", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_fedora.png"
        )),
        ("os_fedora", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_fedora.png"
        )),
        ("os_fedora", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_fedora.png"
        )),
        ("os_fedora", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_fedora.png"
        )),
        ("os_fedora", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_fedora.png"
        )),
        ("os_fedora", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_fedora.png"
        )),
        ("os_fedora", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_fedora.png"
        )),
        ("os_fedora", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_fedora.png"
        )),
        ("os_rhel", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_rhel.png"
        )),
        ("os_rhel", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_rhel.png"
        )),
        ("os_rhel", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_rhel.png"
        )),
        ("os_rhel", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_rhel.png"
        )),
        ("os_rhel", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_rhel.png"
        )),
        ("os_rhel", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_rhel.png"
        )),
        ("os_rhel", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_rhel.png"
        )),
        ("os_rhel", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_rhel.png"
        )),
        ("os_rhel", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_rhel.png"
        )),
        ("os_rhel", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_rhel.png"
        )),
        ("os_centos", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_centos.png"
        )),
        ("os_centos", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_centos.png"
        )),
        ("os_centos", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_centos.png"
        )),
        ("os_centos", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_centos.png"
        )),
        ("os_centos", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_centos.png"
        )),
        ("os_centos", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_centos.png"
        )),
        ("os_centos", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_centos.png"
        )),
        ("os_centos", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_centos.png"
        )),
        ("os_centos", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_centos.png"
        )),
        ("os_centos", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_centos.png"
        )),
        ("os_rocky", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_rocky.png"
        )),
        ("os_rocky", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_rocky.png"
        )),
        ("os_rocky", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_rocky.png"
        )),
        ("os_rocky", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_rocky.png"
        )),
        ("os_rocky", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_rocky.png"
        )),
        ("os_rocky", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_rocky.png"
        )),
        ("os_rocky", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_rocky.png"
        )),
        ("os_rocky", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_rocky.png"
        )),
        ("os_rocky", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_rocky.png"
        )),
        ("os_rocky", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_rocky.png"
        )),
        ("os_almalinux", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_almalinux.png"
        )),
        ("os_almalinux", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_almalinux.png"
        )),
        ("os_almalinux", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_almalinux.png"
        )),
        ("os_almalinux", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_almalinux.png"
        )),
        ("os_almalinux", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_almalinux.png"
        )),
        ("os_almalinux", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_almalinux.png"
        )),
        ("os_almalinux", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_almalinux.png"
        )),
        ("os_almalinux", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_almalinux.png"
        )),
        ("os_almalinux", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_almalinux.png"
        )),
        ("os_almalinux", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_almalinux.png"
        )),
        ("os_alpine", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_alpine.png"
        )),
        ("os_alpine", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_alpine.png"
        )),
        ("os_alpine", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_alpine.png"
        )),
        ("os_alpine", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_alpine.png"
        )),
        ("os_alpine", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_alpine.png"
        )),
        ("os_alpine", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_alpine.png"
        )),
        ("os_alpine", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_alpine.png"
        )),
        ("os_alpine", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_alpine.png"
        )),
        ("os_alpine", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_alpine.png"
        )),
        ("os_alpine", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_alpine.png"
        )),
        ("os_opensuse", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_opensuse.png"
        )),
        ("os_opensuse", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_opensuse.png"
        )),
        ("os_opensuse", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_opensuse.png"
        )),
        ("os_opensuse", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_opensuse.png"
        )),
        ("os_opensuse", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_opensuse.png"
        )),
        ("os_opensuse", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_opensuse.png"
        )),
        ("os_opensuse", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_opensuse.png"
        )),
        ("os_opensuse", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_opensuse.png"
        )),
        ("os_opensuse", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_opensuse.png"
        )),
        ("os_opensuse", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_opensuse.png"
        )),
        ("os_kali", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_kali.png"
        )),
        ("os_kali", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_kali.png"
        )),
        ("os_kali", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_kali.png"
        )),
        ("os_kali", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_kali.png"
        )),
        ("os_kali", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_kali.png"
        )),
        ("os_kali", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_kali.png"
        )),
        ("os_kali", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_kali.png"
        )),
        ("os_kali", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_kali.png"
        )),
        ("os_kali", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_kali.png"
        )),
        ("os_kali", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_kali.png"
        )),
        ("os_windows_server", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_windows_server.png"
        )),
        ("os_windows_server", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_windows_server.png"
        )),
        ("os_windows_server", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_windows_server.png"
        )),
        ("os_windows_server", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_windows_server.png"
        )),
        ("os_windows_server", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_windows_server.png"
        )),
        ("os_windows_server", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_windows_server.png"
        )),
        ("os_windows_server", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_windows_server.png"
        )),
        ("os_windows_server", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_windows_server.png"
        )),
        ("os_windows_server", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_windows_server.png"
        )),
        ("os_windows_server", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_windows_server.png"
        )),
        ("os_android", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_android.png"
        )),
        ("os_android", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_android.png"
        )),
        ("os_android", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_android.png"
        )),
        ("os_android", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_android.png"
        )),
        ("os_android", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_android.png"
        )),
        ("os_android", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_android.png"
        )),
        ("os_android", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_android.png"
        )),
        ("os_android", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_android.png"
        )),
        ("os_android", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_android.png"
        )),
        ("os_android", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_android.png"
        )),
        ("os_ios", "regular", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/regular/os_ios.png"
        )),
        ("os_ios", "regular", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_ios.png"
        )),
        ("os_ios", "regular", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/regular/os_ios.png"
        )),
        ("os_ios", "regular", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/regular/os_ios.png"
        )),
        ("os_ios", "regular", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/regular/os_ios.png"
        )),
        ("os_ios", "hover", 32) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/32/hover/os_ios.png"
        )),
        ("os_ios", "hover", 48) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_ios.png"
        )),
        ("os_ios", "hover", 64) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/64/hover/os_ios.png"
        )),
        ("os_ios", "hover", 128) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/128/hover/os_ios.png"
        )),
        ("os_ios", "hover", 256) => Some(include_bytes!(
            "../../../resources/icon_pack/icons/256/hover/os_ios.png"
        )),
        _ => None,
    }
}

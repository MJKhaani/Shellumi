//! Embedded icon asset fallback ensuring Shellumi always has high-fidelity icons on all platforms,
//! even when running portable standalone executables without external resources.

pub fn get_embedded_icon(name: &str, is_active: bool) -> Option<&'static [u8]> {
    let state = if is_active { "hover" } else { "regular" };
    match (name, state) {
        ("add_host", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/add_host.png"
        )),
        ("add_host", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/add_host.png"
        )),
        ("bell", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/bell.png"
        )),
        ("bell", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/bell.png"
        )),
        ("calendar_sort", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/calendar_sort.png"
        )),
        ("calendar_sort", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/calendar_sort.png"
        )),
        ("certificate", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/certificate.png"
        )),
        ("certificate", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/certificate.png"
        )),
        ("cloud_sync", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/cloud_sync.png"
        )),
        ("cloud_sync", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/cloud_sync.png"
        )),
        ("connect", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/connect.png"
        )),
        ("connect", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/connect.png"
        )),
        ("delete", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/delete.png"
        )),
        ("delete", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/delete.png"
        )),
        ("diagnostics", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/diagnostics.png"
        )),
        ("diagnostics", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/diagnostics.png"
        )),
        ("edit", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/edit.png"
        )),
        ("edit", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/edit.png"
        )),
        ("export", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/export.png"
        )),
        ("export", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/export.png"
        )),
        ("favorite", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/favorite.png"
        )),
        ("favorite", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/favorite.png"
        )),
        ("fido2", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/fido2.png"
        )),
        ("fido2", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/fido2.png"
        )),
        ("fingerprint", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/fingerprint.png"
        )),
        ("fingerprint", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/fingerprint.png"
        )),
        ("grid_view", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/grid_view.png"
        )),
        ("grid_view", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/grid_view.png"
        )),
        ("group_settings", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/group_settings.png"
        )),
        ("group_settings", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/group_settings.png"
        )),
        ("groups", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/groups.png"
        )),
        ("groups", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/groups.png"
        )),
        ("history", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/history.png"
        )),
        ("history", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/history.png"
        )),
        ("hosts", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/hosts.png"
        )),
        ("hosts", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/hosts.png"
        )),
        ("identity_card", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/identity_card.png"
        )),
        ("identity_card", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/identity_card.png"
        )),
        ("import", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/import.png"
        )),
        ("import", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/import.png"
        )),
        ("infrastructure", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/infrastructure.png"
        )),
        ("infrastructure", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/infrastructure.png"
        )),
        ("invite_member", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/invite_member.png"
        )),
        ("invite_member", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/invite_member.png"
        )),
        ("keychain", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/keychain.png"
        )),
        ("keychain", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/keychain.png"
        )),
        ("known_hosts", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/known_hosts.png"
        )),
        ("known_hosts", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/known_hosts.png"
        )),
        ("list_view", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/list_view.png"
        )),
        ("list_view", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/list_view.png"
        )),
        ("log", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/log.png"
        )),
        ("log", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/log.png"
        )),
        ("mobile", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/mobile.png"
        )),
        ("mobile", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/mobile.png"
        )),
        ("more", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/more.png"
        )),
        ("more", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/more.png"
        )),
        ("port_forwarding", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/port_forwarding.png"
        )),
        ("port_forwarding", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/port_forwarding.png"
        )),
        ("private_key", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/private_key.png"
        )),
        ("private_key", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/private_key.png"
        )),
        ("profile", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/profile.png"
        )),
        ("profile", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/profile.png"
        )),
        ("public_key", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/public_key.png"
        )),
        ("public_key", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/public_key.png"
        )),
        ("refresh", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/refresh.png"
        )),
        ("refresh", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/refresh.png"
        )),
        ("search", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/search.png"
        )),
        ("search", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/search.png"
        )),
        ("serial", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/serial.png"
        )),
        ("serial", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/serial.png"
        )),
        ("server", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/server.png"
        )),
        ("server", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/server.png"
        )),
        ("settings", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/settings.png"
        )),
        ("settings", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/settings.png"
        )),
        ("sftp", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/sftp.png"
        )),
        ("sftp", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/sftp.png"
        )),
        ("shellumi_logo", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/shellumi_logo.png"
        )),
        ("shellumi_logo", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/shellumi_logo.png"
        )),
        ("snippets", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/snippets.png"
        )),
        ("snippets", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/snippets.png"
        )),
        ("split_view", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/split_view.png"
        )),
        ("split_view", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/split_view.png"
        )),
        ("tag", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/tag.png"
        )),
        ("tag", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/tag.png"
        )),
        ("tags", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/tags.png"
        )),
        ("tags", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/tags.png"
        )),
        ("terminal", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/terminal.png"
        )),
        ("terminal", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/terminal.png"
        )),
        ("vaults", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/vaults.png"
        )),
        ("vaults", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/vaults.png"
        )),
        ("workspaces", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/workspaces.png"
        )),
        ("workspaces", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/workspaces.png"
        )),
        ("os_arch", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_arch.png"
        )),
        ("os_arch", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_arch.png"
        )),
        ("os_freebsd", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_freebsd.png"
        )),
        ("os_freebsd", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_freebsd.png"
        )),
        ("os_linux", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_linux.png"
        )),
        ("os_linux", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_linux.png"
        )),
        ("os_macos", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_macos.png"
        )),
        ("os_macos", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_macos.png"
        )),
        ("os_netbsd", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_netbsd.png"
        )),
        ("os_netbsd", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_netbsd.png"
        )),
        ("os_openbsd", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_openbsd.png"
        )),
        ("os_openbsd", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_openbsd.png"
        )),
        ("os_ubuntu", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_ubuntu.png"
        )),
        ("os_ubuntu", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_ubuntu.png"
        )),
        ("os_windows", "regular") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/regular/os_windows.png"
        )),
        ("os_windows", "hover") => Some(include_bytes!(
            "../../../resources/icon_pack/icons/48/hover/os_windows.png"
        )),
        _ => None,
    }
}

//! Shellumi GPUI raster icon assets. UI components select regular or hover.
//! Copy this file into your project and place the icon-pack folder under assets.
use gpui::{img, px, Img, Styled};
use std::path::{Path, PathBuf};

pub const ICON_NAMES: &[&str] = &[
    "hosts", "connect", "sftp", "snippets", "history",
    "keychain", "vaults", "workspaces", "cloud_sync", "settings",
    "add_host", "search", "terminal", "serial", "split_view",
    "grid_view", "list_view", "groups", "tags", "favorite",
    "more", "edit", "delete", "refresh", "profile",
    "known_hosts", "port_forwarding", "shellumi_logo", "group_settings", "import",
    "export", "tag", "calendar_sort", "certificate", "private_key",
    "public_key", "identity_card", "os_arch", "os_ubuntu", "os_macos",
    "os_freebsd", "os_netbsd", "os_openbsd", "os_linux", "os_windows",
    "server", "fido2", "fingerprint", "log", "bell",
    "invite_member", "infrastructure", "diagnostics", "mobile", "overview",
    "inspector", "new_group", "duplicate_host", "move_to_group", "copy_to_group",
    "new_workspace", "new_vault", "switch_vault", "quick_connect", "jump_host",
    "ssh", "mosh", "rdp", "vnc", "disconnect",
    "reconnect", "local_terminal", "session_record", "session_replay", "session_info",
    "fit_to_view", "actual_size", "fill_view", "display_resolution", "dynamic_resize",
    "remote_keyboard", "remote_clipboard", "view_only", "fullscreen", "send_ctrl_alt_del",
    "send_windows_key", "shared_drive", "remote_audio", "remote_audio_muted", "remote_microphone",
    "remote_microphone_muted", "input_enabled", "session_telemetry", "upload", "download",
    "transfer_queue", "new_folder", "transfer_progress", "transfer_complete", "transfer_failed",
    "transfer_retry", "local_files", "remote_files", "file_permissions", "hidden_files",
    "vault_lock", "vault_unlock", "google_drive", "sync_conflict", "encrypted_backup",
    "restore_backup", "recovery_code", "vault_encryption", "server_sync", "sync_upload",
    "sync_download", "sync_offline", "sync_success", "setup_resume", "device_enrollment",
    "host_key_verified", "host_key_changed", "ssh_agent", "generate_key", "rotate_key",
    "revoke_key", "password", "security_audit", "trusted_device", "untrusted_device",
    "team_members", "shared_vault", "member_revoke", "pending_invite", "role_admin",
    "role_editor", "role_viewer", "team_key_rotation", "tunnel_local", "tunnel_remote",
    "tunnel_socks", "tunnel_active", "tunnel_stopped", "provider_aws", "provider_azure",
    "provider_digitalocean", "provider_ansible", "provider_connect", "discover_hosts", "inventory",
    "inventory_sync", "health_check", "benchmark", "latency", "network_test",
    "storage_test", "os_debian", "os_fedora", "os_rhel", "os_centos",
    "os_rocky", "os_almalinux", "os_alpine", "os_opensuse", "os_kali",
    "os_windows_server", "os_android", "os_ios",
];

pub fn shellumi_icon(root: &Path, name: &str, hovered: bool, size_pt: f32) -> Img {
    let state = if hovered { "hover" } else { "regular" };
    let path: PathBuf = root.join("icons/256").join(state).join(format!("{name}.png"));
    img(path).w(px(size_pt)).h(px(size_pt))
}

pub fn bundled_icon(name: &str, hovered: bool, size_pt: f32) -> Img {
    shellumi_icon(&Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/shellumi_gpui_icon_pack"), name, hovered, size_pt)
}

//! Shellumi icon helper. Copy this file into your GPUI application.
//! Add the `icons` directory next to the app's Cargo.toml, or adjust root.
use gpui::{img, px, Img, Styled};
use std::path::{Path, PathBuf};

pub const ICON_NAMES: &[&str] = &[
    "hosts", "connect", "sftp", "snippets", "history",
    "keychain", "vaults", "workspaces", "cloud_sync", "settings",
    "add_host", "search", "terminal", "serial", "split_view",
    "grid_view", "list_view", "groups", "tags", "favorite",
    "more", "edit", "delete", "refresh", "profile",
    "known_hosts", "port_forwarding", "shellumi_logo",
];

/// `root` is the unpacked icon-pack folder (which contains `icons/`).
/// `hovered` should be the actual mouse-over state of the control.
pub fn shellumi_icon(root: &Path, name: &str, hovered: bool, size_pt: f32) -> Img {
    let state = if hovered { "hover" } else { "regular" };
    let path: PathBuf = root.join("icons/256").join(state).join(format!("{name}.png"));
    img(path).w(px(size_pt)).h(px(size_pt))
}

/// If icon assets are kept in your Cargo project directory:
pub fn bundled_icon(name: &str, hovered: bool, size_pt: f32) -> Img {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/shellumi_gpui_icon_pack");
    shellumi_icon(&root, name, hovered, size_pt)
}

// Example inside `Render::render`, where your view has `self.is_hovered`:
// div().child(bundled_icon("hosts", self.is_hovered, 24.0))
//
// Update `self.is_hovered` using GPUI mouse-enter/mouse-leave handling
// in the containing interactive element, then call `cx.notify()`.
// Alternatively use a GPUI Kit button's hover state and choose the
// corresponding `regular` or `hover` image path.

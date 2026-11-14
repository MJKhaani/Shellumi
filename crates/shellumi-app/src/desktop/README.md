# Desktop UI modules

`desktop.rs` owns desktop navigation, shared controls, editors, and overlays. Page-specific code lives here:

- `public_inventory.rs`: Keychain and Known Hosts, including inventory refresh.
- `files.rs`: local file browser and remote connection list.
- `backup.rs`: encrypted backup page.
- `snippets.rs`: snippets page and run action.
- `settings.rs`: settings page and preference controls.
- `workspaces.rs`: workspaces page.
- `overview.rs`: overview page.
- `sessions.rs`: terminal and SFTP session lists.

The SFTP pane composition is in `../sftp_ui/browser.rs`; host selection, connected browsing, and overlays have matching files in `../sftp_ui/`. Actions are in `../sftp_ui/actions.rs`, and the older session view is in `../sftp_ui/legacy.rs`. Transport helpers live in `../sftp_transfer.rs` and the core crate.

Dialog and session renderers are grouped in `../ui/`, with one descriptively named file per view. Feature actions are grouped in `../actions/`. The Hosts view is in `../hosts_view.rs`, with its editor and popovers in `../ui/host_editor.rs` and `../ui/hosts_popovers.rs`; application rendering in `../app_render.rs`, and application tests in `../tests.rs`. Shared application state and startup remain in `main.rs`; desktop navigation and common controls remain in `desktop.rs`.

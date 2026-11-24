# Shellumi Custom Icon Inventory — November 24, 2026

**Source audited:** `MJKhaani/Shellumi`, `main` at `a4132558e8127a177a92c81ec7554492d176ae03`; `resources/icon_pack/manifest.csv`, `resources/icon_pack/README.md`, and major Rust/GPUI screens.

**Already in repository:** 54 custom icon names with `regular` and `hover` PNGs at 32/48/64/128/256 px. This is the v7 pack retaining the v5 appearance.

**Proposed custom icons not in repository:** 114 distinct names (37 P0 essential, 65 P1 functional, 12 P2 optional OS badges).

The **missing** list is a design inventory for the unified inspector redesign and existing Shellumi capabilities, not a claim that all these names are called by the current code. Only introduce controls for features the app really supports. Reuse existing glyphs when semantically identical; do not create duplicates for synonyms.

## Exact design constraints

- Work from the **existing v5/v7 reference PNGs**, not a new icon style.
- **Regular:** standalone centered metallic-blue/silver glyph; fully transparent outside; NO tile, border, or solid background.
- **Hover:** same glyph, same shape/alignment, on a dark graphite rounded-square bordered tile. Do not swap which state has the tile.
- Preserve the existing restrained blue accent, silver highlights, geometry, shadowing, edge smoothness, and balanced internal margins. No captions, labels, text, emoji, watermarks, or large decorative scenery.
- Each icon needs two matched masters, regular and hover, and the existing five export sizes 32, 48, 64, 128, 256 PNG (RGBA). Use a high-resolution master and downsample with legibility checks, rather than generating each size independently.
- Required filename convention: `resources/icon_pack/icons/<size>/regular/<name>.png` and `resources/icon_pack/icons/<size>/hover/<name>.png`.
- Hover is a **visual state**, not the state of a connection or a security indication. Security and status glyphs remain legible in regular state.
- For actual provider and operating-system logos, use approved and accurate source marks; generative reconstruction can distort trademarks. Maintain consistent icon padding without altering the official mark itself.
- When the pack is integrated: update `manifest.csv/json`, `gpui_icon.rs`, `embedded_icons.rs`, and asset lookups. Test graceful handling of absent optional assets.

## Already available — do not regenerate

`hosts, connect, sftp, snippets, history, keychain, vaults, workspaces, cloud_sync, settings, add_host, search, terminal, serial, split_view, grid_view, list_view, groups, tags, favorite, more, edit, delete, refresh, profile, known_hosts, port_forwarding, shellumi_logo, group_settings, import, export, tag, calendar_sort, certificate, private_key, public_key, identity_card, os_arch, os_ubuntu, os_macos, os_freebsd, os_netbsd, os_openbsd, os_linux, os_windows, server, fido2, fingerprint, log, bell, invite_member, infrastructure, diagnostics, mobile`

The app separately contains SFTP file-type icons in `resources/file_icons/` (folder, code, documents, images, audio, video, and others).

## Missing custom icons — generation manifest

P0 = create first for basic UI functionality. P1 = create next for supported feature surfaces. P2 = optional and only if OS recognition exists. Every filename below is the basename without `.png`.

### Navigation and unified inspector

| Priority | Filename | Image-generation description |
|---|---|---|
| P0 | `overview.png` | A four-panel infrastructure dashboard, 2×2 miniature cards. |
| P0 | `inspector.png` | A main content rectangle beside a narrow right-hand details pane with two information lines. |
| P0 | `new_group.png` | A folder carrying a small plus badge, distinct from existing Groups. |
| P0 | `duplicate_host.png` | Two offset, overlapping server cards indicating an exact duplicate. |
| P0 | `move_to_group.png` | One server card and an arrow entering a folder (move; do not show a copied card). |
| P0 | `copy_to_group.png` | Two layered server cards with an arrow pointing toward a folder (copy). |
| P1 | `new_workspace.png` | Two neatly arranged application panes with a small plus badge. |
| P1 | `new_vault.png` | A compact secure safe door with a small plus badge. |
| P1 | `switch_vault.png` | Two safe-door silhouettes joined by a curved switch arrow. |
| P1 | `quick_connect.png` | A server endpoint and one fast connecting arrow, not a lightning emoji. |
| P1 | `jump_host.png` | Three small servers in line: a central bastion between local and destination. |

### Connection types and session actions

| Priority | Filename | Image-generation description |
|---|---|---|
| P0 | `ssh.png` | A terminal window with a tiny secure network link; distinct from the existing Terminal icon. |
| P0 | `mosh.png` | A terminal with two curved roaming/signal strokes that express resilient mobile connection. |
| P0 | `rdp.png` | A desktop monitor with an inset four-pane Windows-inspired motif; no letters. |
| P0 | `vnc.png` | A remote monitor with a dotted network link and pointer cursor; distinct from RDP. |
| P0 | `disconnect.png` | A split link/cable with two visibly separated endpoints. |
| P0 | `reconnect.png` | A link being restored by a clockwise circular arrow. |
| P1 | `local_terminal.png` | A terminal screen above a miniature local computer base. |
| P1 | `session_record.png` | A terminal outline with a prominent small circular recording dot. |
| P1 | `session_replay.png` | A terminal outline containing a play triangle and short timeline ticks. |
| P1 | `session_info.png` | A terminal identity card with a status light and two information lines. |

### RDP and VNC compact toolbar

| Priority | Filename | Image-generation description |
|---|---|---|
| P0 | `fit_to_view.png` | A monitor bounded by four inward corner arrows, indicating fit the whole desktop. |
| P0 | `actual_size.png` | A monitor with an inset 1:1 symbol, indicating native-pixel scaling. |
| P0 | `fill_view.png` | A monitor bounded by four outward corner arrows, indicating fill/crop. |
| P0 | `display_resolution.png` | A monitor with tiny horizontal and vertical measurement ticks. |
| P0 | `dynamic_resize.png` | A monitor inside an adjustable frame, with diagonal resize arrows. |
| P0 | `remote_keyboard.png` | A keyboard with an outward arrow toward a remote monitor. |
| P0 | `remote_clipboard.png` | Two overlapping clipboards connected by two short opposite arrows. |
| P0 | `view_only.png` | A monitor with a clearly recognizable eye, indicating viewing without control. |
| P0 | `fullscreen.png` | A monitor with four outward-pointing corner markers. |
| P1 | `send_ctrl_alt_del.png` | Three adjacent keycaps on a small keyboard strip; avoid unreadable text. |
| P1 | `send_windows_key.png` | One keycap containing the recognizable four-pane Windows shape. |
| P1 | `shared_drive.png` | A storage drive with a small shared-folder/network-link symbol. |
| P1 | `remote_audio.png` | A speaker with two restrained sound waves. |
| P1 | `remote_audio_muted.png` | The same speaker with one clean diagonal mute slash. |
| P1 | `remote_microphone.png` | A microphone with one compact signal curve. |
| P1 | `remote_microphone_muted.png` | The same microphone with one clean diagonal mute slash. |
| P1 | `input_enabled.png` | A monitor with a pointer cursor, indicating input control is permitted. |
| P1 | `session_telemetry.png` | A small performance line graph inset into a monitor outline. |

### SFTP and file transfers

| Priority | Filename | Image-generation description |
|---|---|---|
| P0 | `upload.png` | A file document with an upward arrow toward a small remote-server endpoint. |
| P0 | `download.png` | A remote-server endpoint with a downward arrow toward a file document. |
| P0 | `transfer_queue.png` | Three short stacked file rows beside a small progress arc. |
| P0 | `new_folder.png` | A familiar file-browser folder with a clearly visible plus badge. |
| P1 | `transfer_progress.png` | A file rectangle overlaid by a half-complete progress ring. |
| P1 | `transfer_complete.png` | A file with a tiny successful check badge. |
| P1 | `transfer_failed.png` | A file with a tiny failure/cross badge. |
| P1 | `transfer_retry.png` | A file with a small clockwise retry arrow. |
| P1 | `local_files.png` | A desktop/laptop device with a tiny folder in the foreground. |
| P1 | `remote_files.png` | A server-rack device with a tiny folder in the foreground. |
| P1 | `file_permissions.png` | A document with a lock and small checked permission marks, no text. |
| P1 | `hidden_files.png` | A document paired with a crossed-out eye, representing hidden entries. |

### Multi-vault security, backup and sync

| Priority | Filename | Image-generation description |
|---|---|---|
| P0 | `vault_lock.png` | A safe door combined with a closed padlock. |
| P0 | `vault_unlock.png` | The same safe door combined with an open padlock. |
| P0 | `google_drive.png` | An accurate simplified triangular Google Drive mark, without text. |
| P0 | `sync_conflict.png` | Two opposing sync arrows meeting at a warning diamond. |
| P0 | `encrypted_backup.png` | A backup/archive container with a distinct closed padlock. |
| P0 | `restore_backup.png` | A backup container with an arrow returning into a safe/vault. |
| P0 | `recovery_code.png` | A protective shield containing an abstract recovery-card/key motif; never actual digits. |
| P1 | `vault_encryption.png` | A safe door carrying a centered security shield and lock inset. |
| P1 | `server_sync.png` | Two server-rack silhouettes connected by circular sync arrows. |
| P1 | `sync_upload.png` | A cloud outline with one clear upward arrow. |
| P1 | `sync_download.png` | A cloud outline with one clear downward arrow. |
| P1 | `sync_offline.png` | A cloud outline crossed by a small disconnected-link slash. |
| P1 | `sync_success.png` | A cloud outline with a small checked success badge. |
| P1 | `setup_resume.png` | A five-step progress path with one highlighted unfinished step and a forward arrow. |
| P1 | `device_enrollment.png` | A laptop and phone linked by a verified connection symbol. |

### SSH identities, authentication, host-key trust

| Priority | Filename | Image-generation description |
|---|---|---|
| P0 | `host_key_verified.png` | A server endpoint with a shield and a check badge. |
| P0 | `host_key_changed.png` | A server endpoint with a shield and a distinct warning triangle. |
| P1 | `ssh_agent.png` | A metal key inside a small terminal agent/socket graphic. |
| P1 | `generate_key.png` | A metal key bearing a tiny new/plus spark. |
| P1 | `rotate_key.png` | A metal key surrounded by a short clockwise rotation arrow. |
| P1 | `revoke_key.png` | A key with a decisive diagonal cancellation slash. |
| P1 | `password.png` | A padlock above a row of four masked dots; no real characters. |
| P1 | `security_audit.png` | A shield beside a compact document checklist. |
| P1 | `trusted_device.png` | A desktop device with a checked shield badge. |
| P1 | `untrusted_device.png` | A desktop device with a warning shield badge. |

### Team, collaboration and permissions

| Priority | Filename | Image-generation description |
|---|---|---|
| P1 | `team_members.png` | Three small balanced human silhouettes as one team cluster. |
| P1 | `shared_vault.png` | A safe door placed between two small collaborating user silhouettes. |
| P1 | `member_revoke.png` | A user silhouette with a small blocked/cancellation badge. |
| P1 | `pending_invite.png` | An envelope carrying a small clock badge. |
| P1 | `role_admin.png` | A user silhouette with a small privileged shield-and-star badge. |
| P1 | `role_editor.png` | A user silhouette with a small pencil badge. |
| P1 | `role_viewer.png` | A user silhouette with a small eye badge. |
| P1 | `team_key_rotation.png` | A safe/vault key with a surrounding rotation arrow and shield accent. |

### SSH tunnels and port forwarding

| Priority | Filename | Image-generation description |
|---|---|---|
| P0 | `tunnel_local.png` | A local computer on the left and a server on the right connected by a forwarding tunnel arrow. |
| P0 | `tunnel_remote.png` | A server on the right connected back to a local computer by an opposite forwarding arrow. |
| P0 | `tunnel_socks.png` | A central tunnel hub with three small outward branches (dynamic SOCKS proxy). |
| P1 | `tunnel_active.png` | A bidirectional tunnel link with a checked active badge. |
| P1 | `tunnel_stopped.png` | A bidirectional tunnel link with a small square stop badge. |

### Infrastructure inventory and providers

| Priority | Filename | Image-generation description |
|---|---|---|
| P1 | `provider_aws.png` | Faithful official AWS brand symbol (not an invented cloud logo); preserve its proportions. |
| P1 | `provider_azure.png` | Faithful official Microsoft Azure symbol; preserve its proportions. |
| P1 | `provider_digitalocean.png` | Faithful official DigitalOcean circle/square brand symbol. |
| P1 | `provider_ansible.png` | Faithful official Ansible circular arrow/A symbol. |
| P1 | `provider_connect.png` | A cloud-provider node with a linked/connected badge. |
| P1 | `discover_hosts.png` | A server and small magnifying glass revealing two nearby network nodes. |
| P1 | `inventory.png` | A compact sheet with three inventory rows and miniature server indicators. |
| P1 | `inventory_sync.png` | Two structured inventory sheets connected by circular arrows. |

### Diagnostics and performance

| Priority | Filename | Image-generation description |
|---|---|---|
| P1 | `health_check.png` | A diagnostic heartbeat trace crossing a small checkmark shield. |
| P1 | `benchmark.png` | A compact speedometer/gauge with three restrained tick marks. |
| P1 | `latency.png` | Two small network nodes connected through a central stopwatch. |
| P1 | `network_test.png` | Two connected nodes with a test pulse traveling between them. |
| P1 | `storage_test.png` | A drive/storage unit with a small checked verification badge. |

### Additional operating-system badges — optional, only when supported by detection

| Priority | Filename | Image-generation description |
|---|---|---|
| P2 | `os_debian.png` | Faithful Debian spiral logo, clear at 32 px. |
| P2 | `os_fedora.png` | Faithful Fedora loop/f symbol, clear at 32 px. |
| P2 | `os_rhel.png` | Faithful Red Hat Enterprise Linux hat silhouette. |
| P2 | `os_centos.png` | Faithful CentOS compass/star geometric mark. |
| P2 | `os_rocky.png` | Faithful Rocky Linux angular mountain mark. |
| P2 | `os_almalinux.png` | Faithful AlmaLinux multicolor petal/star geometry. |
| P2 | `os_alpine.png` | Faithful Alpine Linux stylized mountain mark. |
| P2 | `os_opensuse.png` | Faithful openSUSE chameleon silhouette. |
| P2 | `os_kali.png` | Faithful Kali Linux dragon silhouette, simplified for legibility. |
| P2 | `os_windows_server.png` | Windows four-pane motif with a tiny server-rack accent. |
| P2 | `os_android.png` | Recognizable Android robot-head silhouette. |
| P2 | `os_ios.png` | An iOS mobile device silhouette with a restrained platform-identifying accent. |


## Native/vector controls — do NOT commission as custom illustrated PNGs
Use lightweight consistent native/vector line icons (or GPUI drawn shapes) for these tiny controls. They still belong in the design system but are **not missing custom icon-pack assets**:

`close`, `back`, `forward`, `chevron_up`, `chevron_down`, `chevron_left`, `chevron_right`, `plus`, `minus`, `check`, `warning`, `error`, `info`, `help`, `save`, `cancel`, `undo`, `redo`, `copy`, `cut`, `paste`, `rename`, `pin`, `unpin`, `filter`, `sort_ascending`, `sort_descending`, `open_external`, `new_tab`, `tab_close`, `menu`, `drag_handle`, `maximize`, `minimize`, `stop`, `play`, `pause`, `eye`, `eye_off`, `lock`, `unlock`, `volume_plus`, `volume_minus`, `scroll_up`, `scroll_down`, `home`, `up_one_level`, `breadcrumb_separator`.

Use one vector family and consistent stroke weight. Avoid replacing all of these with high-detail metallic PNGs, particularly in a dense toolbar or 32px UI.

## QA for exported icons

Check normal/hover parity, fully transparent regular background, dark-border tile only on hover, centered motif, 32px readability, preserved original 54 files, all requested sizes, and manifest/embedded-asset lookup coverage.

# <img src="../../resources/icon_pack/icons/48/regular/settings.png" width="24" height="24" alt="Settings"> Application Settings

[Docs Home](../README.md) / [User Guide](README.md) / Settings

---

Open **Settings** from the bottom of the left navigation rail (or `Cmd+,` on macOS) to inspect and adjust Shellumi's preferences, security status, and synchronization configuration.

## Persisted Desktop Preferences (`Preferences`)

The following preferences are stored per vault database in `database.with_extension("preferences.json")` (e.g., `default.preferences.json`, never containing secrets; see [Configuration Reference](../reference/configuration.md)):

| Setting | Default | Valid Range | Applies |
| --- | --- | --- | --- |
| **Terminal Font Size** | `14` px | `10..=24` | Immediate on open terminals |
| **UI Font Size** | `13` px | `11..=18` | Immediate across UI |
| **Scrollback Lines** | `5000` | `100..=50000` | New / active terminal buffers |
| **Show Hidden Files (SFTP)** | `false` | `true` / `false` | SFTP local/remote panes |
| **Startup Overview** | `false` | `true` / `false` | Next launch |
| **Bell Mode** | `"both"` | `"off"` / `"audible"` / `"visual"` / `"both"` | Immediate |
| **Bell Volume** | `70` | `0..=100` | Immediate |
| **Cursor Shape** | `"block"` | `"block"` / `"underline"` / `"beam"` | Immediate on open terminals |
| **Cursor Blink** | `true` | `true` / `false` | Immediate on open terminals |
| **Copy on Select** | `false` | `true` / `false` | Immediate |
| **Record Input (opt-in)** | `false` | `true` / `false` | New / active recordings |
| **OSC 52 Clipboard Policy** | `"deny"` | `"deny"` / `"prompt"` / `"allow_write"` | Immediate (reads always denied) |

## Settings Sections

- **Appearance & Terminal:** Adjust UI/terminal font sizes, scrollback depth, and theme presentation.
- **Connections & Security:** Review TOFU host-key policy (always enforced) and OS credential-store status.
- **Synchronization:** Opt in to personal [Google Drive Sync](../synchronization/google-drive.md) or manage [Shellumi Server](../synchronization/shellumi-server.md) vault connections.
- **Diagnostics:** Run `shellumi-cli doctor --markdown` to inspect active storage, credential backend, and environment variables.

## Platform & Environment Overrides

- Runtime environment variables (`SHELLUMI_VAULT_PATH`, `SHELLUMI_CREDENTIAL_STORE`, `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE`, `SHELLUMI_LOG`, `SHELLUMI_DEVICE_ID`) take effect at process startup and are documented in [Environment Variables](../reference/environment-variables.md).
- Shellumi never phones home, collects telemetry, or forces automatic updates (`REQ-ARCH-001`, `REQ-SET-002`).

# Configuration & Database Reference

[Docs Home](../README.md) / [Reference](README.md) / Configuration

---

## Persistent Files

| File | Location | Contents |
| :--- | :--- | :--- |
| **Vault Databases** | `<data_dir>/vaults/default.db`, `<data_dir>/vaults/<vault-id>.db` | AES-256-GCM encrypted Turso SQLite databases (`SCHEMA_VERSION = 32`) |
| **Vault Manifests** | `<data_dir>/vault_manifests/<vault-id>.manifest.json` | Format v2 Argon2id + AES-256-GCM wrapped primary & recovery DEK envelopes |
| **Preferences** | `<db_path>.preferences.json` | UI font size, terminal font size, scrollback lines, startup page, hidden-file toggle (no secrets) |

## Portable vs. Device-Local Host Config

- **`PortableProtocolSettings`:** Synchronized across devices (`auth_mode`, `local_credentials_required`, `required_credential_kind`, `domain`, `width`, `height`, `color_depth`, `view_only`, `strict_tls`, `audio`, `mic`, `dynamic_resize`, Mosh/Serial/Telnet options).
- **Device-local fields (never exported):** `host_secret_ref`, `host_passphrase_ref`, `host_key_path`.

## Database Schema Version (`32`)

- **Schemas 1–19:** Hosts, groups, identities, snippets, tunnels, workspaces, SFTP transfers, recordings, cloud providers, discovered hosts, `vault_contexts`, `vault_id` scoping.
- **Schemas 20–30:** Vault-scoped groups/recordings, `vault_server_connections`, `vault_mutations`, `vault_sync_state`, `enrolled_devices`, `device_replacements`, and enrollment operation journaling.
- **Schema 31:** `credential_operations` crash-safe credential journal.
- **Schema 32:** Immutable `sync_id` on `identities` & `hosts`, plus `unresolved_identity_sync_id`, `unresolved_identity_name`, and `auth_dependency_unresolved` on `hosts` & `groups`.

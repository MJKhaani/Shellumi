# Storage and Database Architecture

[Docs Home](../README.md) / [Architecture](README.md) / Storage and Database

---

Shellumi uses local async **Turso** (`turso`) SQLite databases with experimental AES-256-GCM page encryption enabled for every vault (`vaults/default.db` and `vaults/<vault-id>.db`).

## Current Schema Version: `32`

| Table(s) | Purpose |
| :--- | :--- |
| `hosts`, `groups`, `identities` | Vault-scoped inventory; Schema 32 adds `sync_id` and `unresolved_identity_sync_id` / `unresolved_identity_name` / `auth_dependency_unresolved` |
| `snippets`, `tunnels`, `workspaces` | Reusable scripts, SSH port-forwarding rules, and saved multi-session layouts |
| `sftp_transfers`, `session_recordings` | Transfer history and opt-in Asciicast v2 recording metadata |
| `cloud_providers`, `discovered_hosts` | Local infrastructure provider records and merged discovery inventory |
| `credential_operations` (Schema 31) | Crash-safe journal (`Planned` → `Staged` → `Committed` → `Completed`/`RolledBack`) for OS credential mutations |
| `vault_mutations`, `vault_sync_state`, `sync_snapshots` | Outbound sync outbox, cursor/revision state, and local historical encrypted snapshots |
| `vault_contexts`, `vault_server_connections`, `enrolled_devices`, `device_replacements` | Default-vault registry, server bindings, and Ed25519/X25519 enrollment state (Schemas 20–30) |

## Transactional & Migration Guarantees

- **Idempotent Migrations:** `HostStore::open_with_key` runs migrations inside `BEGIN IMMEDIATE` transactions, checks `PRAGMA table_info(...)` before `ALTER TABLE` in Schema 32, and rejects databases whose `PRAGMA user_version > SCHEMA_VERSION`.
- **Fail-Closed Encryption:** Opening an encrypted vault path without its 256-bit DEK is refused before any SQLite file creation or query executes.
- **Atomic Host+Identity+Sync Commits:** `save_host_with_identity_and_journal_tx_inner` creates/updates the identity and host, serializes sanitized sync payloads, updates `vault_mutations`, and marks `credential_operations` as `Committed` in a single SQLite transaction.

See [Database Migrations](../development/database-migrations.md) for contributor rules.

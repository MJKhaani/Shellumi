# Synchronization Engine

[Docs Home](../README.md) / [Architecture](README.md) / Synchronization Engine

---

Shellumi's synchronization layer spans `shellumi-core::{sync, server}` and `shellumi-storage::{lib, server_sync}`:

- **Local Outbox (`vault_mutations`):** `HostStore` writes `vault_mutations` records inside the same SQLite transaction for **host** and **identity** mutations (`save_host_with_identity_and_journal_tx_inner`, `delete_host_with_journal_tx`, `delete_identity_with_journal_tx`). Other entities (groups, snippets, tunnels, workspaces) do not all share this transactional `vault_mutations` outbox write path.
- **Google Drive Engine:** Uses `GoogleDriveSyncClient` over `SystemHttpTransport` (`curl` stdin transport) with immutable create-only snapshots (`files.generateIds` + read-back verification) and an HMAC-SHA-256 authenticated `cloud_state` DAG.
- **Shellumi Server Engine:** Uses `ServerSyncClient` with Ed25519-signed Compare-And-Swap (CAS) mutation commits, cursor-paginated delta feeds, HTTP 410 snapshot recovery, and per-vault WebSocket notifications (`/api/v1/events/ws`).
- **Portable Auth Boundary:** `SyncHostPayload` carries allowlisted `PortableProtocolSettings` and strips device-local credential references/paths (`host_secret_ref`, `host_passphrase_ref`, `host_key_path`) on incremental sync. See [Object Identifiers](../synchronization/object-identifiers.md).

## Transaction & Reconciliation Boundaries

1. **Atomic Local Host + Identity Creation:** `HostStore::save_host_with_identity_and_journal_tx_inner()` serializes `SyncIdentityPayload` and `SyncHostPayload` *inside* the SQLite transaction after local IDs are resolved, alongside the `vault_mutations` outbox record.
2. **Credential Operation Journal:** Local credential writes and deletions stage and commit entries in `credential_operations` (`Planned` → `Staged` → `Committed` → `Completed` / `RetryPending`) so interrupted OS Keychain updates can be reconciled on startup.
3. **Incoming Identity Persistence vs. Dependency Reconciliation:** In `server_sync::apply_change_item()`, persisting an incoming identity and reconciling dependent `hosts` (`auth_dependency_unresolved`) or `groups` execute through **separate database statements** rather than a single cross-entity atomic transaction.
4. **Outstanding Atomicity & Recovery Limitations:** See [Known Issues](../project/known-issues.md) for [`SYNC-001`](../project/known-issues.md#sync-001--schema-32-migration-generates-synchronization-identifiers-from-local-row-ids) through [`SYNC-006`](../project/known-issues.md#sync-006--google-drive-defaults-to-the-shared-installation-identifier-desktop-app), including `SYNC-005` (incoming credential invalidation bypassing `credential_operations` cleanup).

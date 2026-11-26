# Synchronization Engine

[Docs Home](../README.md) / [Architecture](README.md) / Synchronization Engine

---

Shellumi's synchronization layer spans `shellumi-core::{sync, server}` and `shellumi-storage::{lib, server_sync}`:

- **Local Outbox (`vault_mutations`):** Host, identity, group, snippet, tunnel, and workspace writes in remote vaults append mutations inside the same SQLite transaction as the local row update.
- **Google Drive Engine:** Uses `GoogleDriveSyncClient` over `SystemHttpTransport` (`curl` stdin transport) with immutable create-only snapshots (`files.generateIds` + read-back verification) and an HMAC-SHA-256 authenticated `cloud_state` DAG.
- **Shellumi Server Engine:** Uses `ServerSyncClient` with Ed25519-signed Compare-And-Swap (CAS) mutation commits, cursor-paginated delta feeds, HTTP 410 snapshot recovery, and per-vault WebSocket notifications (`/api/v1/events/ws`).
- **Portable Auth Boundary:** `SyncHostPayload` carries allowlisted `PortableProtocolSettings` and strips device-local credential references/paths (`host_secret_ref`, `host_passphrase_ref`, `host_key_path`). See [Object Identifiers](../synchronization/object-identifiers.md).

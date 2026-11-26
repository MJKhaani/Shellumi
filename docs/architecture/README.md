# <img src="../../resources/icon_pack/icons/48/regular/infrastructure.png" width="26" height="26" alt="Architecture"> Architecture Documentation

[Docs Home](../README.md) / Architecture

Technical architecture of Shellumi's 8-crate Rust workspace, storage, connection engine, remote desktop stack, and platform integration.

---

| Document | Scope |
| :--- | :--- |
| [**Workspace & Crates**](workspace-and-crates.md) | 8-crate Cargo workspace, dependency graph, GPUI `0.2.2`, Turso, and Tokio |
| [**Application Lifecycle**](application-lifecycle.md) | Startup sequence, vault unlock, multi-window isolation, and background tasks |
| [**Connection Engine**](connection-engine.md) | `ConnectionOpRegistry`, `resolve_protocol_auth`, OpenSSH, Askpass, SFTP, and Mosh |
| [**Storage & Database**](storage-and-database.md) | Encrypted Turso SQLite, Schema 32 tables, migrations, and transaction safety |
| [**Credential Management**](credential-management.md) | `VaultCredentialStore`, OS backends, encrypted fallback, and credential journal |
| [**Vault Architecture**](vault-architecture.md) | Physical vault partitioning, `VaultManifest`, DEK wrapping, and rotation |
| [**Synchronization Engine**](synchronization-engine.md) | Local outbox, Google Drive DAG transport, and Shellumi Server CAS/WS engine |
| [**Remote Desktop Engine**](remote-desktop-engine.md) | `shellumi-rdp`, `shellumi-vnc`, `CanvasTransform`, resize coordinator, and limits |
| [**Platform Integration**](platform-integration.md) | Window chrome, `CREATE_NO_WINDOW`, desktop entries, data paths, and icon assets |

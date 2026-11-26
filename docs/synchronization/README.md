# <img src="../../resources/icon_pack/icons/48/regular/server_sync.png" width="26" height="26" alt="Synchronization"> Synchronization & Operating Modes

[Docs Home](../README.md) / Synchronization

Shellumi supports three distinct operating modes. Standalone local use never requires a cloud account or server enrollment.

---

## Operating Mode Comparison

| Capability | Standalone (Local) | <img src="../../resources/icon_pack/icons/32/regular/google_drive.png" width="16" height="16" alt="Google Drive"> Google Drive | <img src="../../resources/icon_pack/icons/32/regular/server_sync.png" width="16" height="16" alt="Shellumi Server"> Shellumi Server |
| :--- | :--- | :--- | :--- |
| **Local vault** | Encrypted `vaults/*.db` with per-vault 256-bit DEK | Encrypted `vaults/*.db` with per-vault 256-bit DEK | Encrypted `vaults/*.db` with per-vault 256-bit DEK |
| **Offline operation** | 100% offline | Full offline use; syncs when online | Full offline cache; reconciles on reconnect |
| **Cloud synchronization** | None | Personal E2EE sync (`drive.appdata`) | E2EE CAS mutations, delta feed, WebSocket events |
| **Server enrollment** | Not required | Not required | Required (Ed25519 + X25519 device enrollment) |
| **Team collaboration** | Not applicable (local-only) | Not supported | Shared vaults, roles, invitations, E2EE terminal relay |
| **Device revocation** | Not applicable | Not applicable | Supported (local authorization & global server revocation) |
| **Backup & restore** | Encrypted local export/import + `sync_snapshots` | Immutable Drive snapshots + local export/import | Server snapshots + local export/import |

---

## Synchronization Guides

| Document | Topic |
| :--- | :--- |
| [**Standalone Mode**](standalone.md) | Offline-only local vaults with zero external network dependencies |
| [**Google Drive Sync**](google-drive.md) | OAuth PKCE, authenticated revision DAG, create-only concurrency, and limits |
| [**Shellumi Server**](shellumi-server.md) | Self-hosted server connection, trust boundaries, CAS commits, and offline sync |
| [**Backup & Restore**](backup-and-restore.md) | Encrypted vault export/import, historical snapshots, and overwrite protection |
| [**Conflict Resolution**](conflict-resolution.md) | Three-way merge, concurrent branch handling, and credential safety |
| [**Device Enrollment**](device-enrollment.md) | Ed25519/X25519 enrollment, invitations, key replacement, and revocation |
| [**Object Identifiers**](object-identifiers.md) | Device-local vs. portable identifiers, current issues, and target design |
| [**Migration & Recovery**](migration-and-recovery.md) | Resumable `MigrationEngine`, staging journals, and crash recovery |

# Backup and Restore

[Docs Home](../README.md) / [Synchronization](README.md) / Backup and Restore

---

- **Encrypted Local Export:** Settings → Backup (or `shellumi-cli vault export <path> --passphrase <pass>`) exports an AES-256-GCM encrypted `SyncVault` snapshot. Export refuses to overwrite existing files.
- **Transactional Import / Restore:** Decrypts and validates the snapshot inside a SQLite transaction, rolling back cleanly on wrong passphrase, tampered envelope, or vault mismatch.
- **Historical Snapshots (`sync_snapshots`):** Export and import operations archive historical snapshots locally (retaining the latest 3 by default), inspectable via `vault history` and restorable via `vault rollback <snapshot-id> --passphrase <pass>`.
- **Existing Vault Protection:** Second-device restoration refuses to overwrite existing `vaults/*.db` or `vault_manifests/*.manifest.json` files.

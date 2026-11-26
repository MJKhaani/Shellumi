# Migration and Recovery

[Docs Home](../README.md) / [Synchronization](README.md) / Migration and Recovery

---

## Resumable Legacy Migration (`MigrationEngine`)

When upgrading from legacy single-file `shellumi.db` installations:

1. **Detection & Backup:** Creates timestamped private backups (`shellumi.db.backup.<timestamp>`) of the database, WAL, and SHM files under `0700`/`0600` Unix permissions.
2. **Versioned Staging Journal:** Tracks progress in `vaults/.migration_staging/` under an OS file lock.
3. **Physical Partitioning:** Splits `default.db` and `vaults/<vault-id>.db` with independent 256-bit DEKs, moving plaintext server tokens/keys into `keychain://org.shellumi.server/...` references.
4. **Atomic Activation:** Publishes staged databases via create-only hard links (refusing to overwrite existing destinations) and verifies ciphertext digests before renaming `shellumi.db` to `shellumi.db.migrated`.

## Crash-Safe Credential & Rotation Journals

- **`credential_operations` (Schema 31):** Journals `Planned` → `Staged` → `Committed` → `Completed` / `RolledBack` transitions around OS credential-store mutations and reconciles orphaned staged or pending-delete secrets on vault load.
- **`vault_rotation`:** Journals password-rotation stages across manifest, target DB, registry DB, and OS credential store, recovering safely on restart if interrupted.

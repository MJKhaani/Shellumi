# Migration and Recovery

[Docs Home](../README.md) / [Synchronization](README.md) / Migration and Recovery

---

## Onboarding Inventory & Route Precedence

Shellumi separates observed filesystem/credential metadata (`InstallationInventory` in `shellumi-core::onboarding`) from workflow routing (`OnboardingRoute`) and runtime activity (`OperationState`):

1. **Active Discard Transaction (`InterruptedDiscard`):** Takes highest precedence. Resumes purge if `DeleteCommitted` was persisted, or safely rolls back to original paths if interrupted prior to commit.
2. **Interrupted Migration (`InterruptedMigration`):** Detected from `vaults/.migration_staging/migration_journal.json` whenever a migration has not reached verified `Phase::Complete`. Never falls through to generic password unlock when a manifest does not yet exist.
3. **Corrupt / Inconsistent Artifacts (`InconsistentInstallation`):** Surfaces diagnostics and backup options without auto-creating or overwriting any files.
4. **Verified Active Vault (`ExistingVault` / `ExplicitReentry`):** Reconciles stale `.setup_state.json` checkpoints from verified storage without triggering a duplicate migration when a legacy backup is retained.
5. **Legacy Database Detected (`LegacyMigration`):** Presents the explicit decision between **Migrate existing data (Recommended)** and **Start fresh — remove existing database**.

---

## Resumable Legacy Migration (`MigrationEngine`)

When upgrading from legacy single-file `shellumi.db` installations:

1. **Detection & Backup:** Creates verified private backups (`vaults/.migration_staging/backup/`) of `shellumi.db`, `-wal`, and `-shm` files under `0700`/`0600` Unix permissions.
2. **Versioned Staging Journal:** Tracks progress in `vaults/.migration_staging/migration_journal.json` under an exclusive `.migration.lock`.
3. **Separate Source & Destination Credentials:** Decrypts encrypted legacy sources with the source key (while unencrypted SQLite sources open without a key), and provisions fresh independent 256-bit DEKs, Argon2id `VaultManifest` files, and 8-word recovery codes for each newly created vault. Onboarding requires verified portable manifests (`require_portable_manifests = true`).
4. **Atomic Activation:** Publishes staged databases and manifests via create-only hard links (refusing to overwrite existing destinations), verifies ciphertext digests and reopen with the password-derived key, and records `vaults/.migration_complete`. The original `shellumi.db` is retained intact.

---

## Explicit Legacy Database Removal (`LegacyDiscardEngine`)

Users who choose **Start fresh — remove existing database** on the Legacy Decision page are routed to a dedicated confirmation screen requiring:
- Optional verified offline backup (`LegacyDiscardEngine::create_backup`) to a user-selected directory outside the Shellumi data root, or explicit acknowledgement to proceed without a backup.
- Unchecked acknowledgement checkbox (`I understand that my existing local database will be permanently removed.`).
- Exact typed confirmation token `DELETE`.

```text
Prepared (explicit confirmation + inventory sealed)
  -> Quarantining (move only scoped shellumi.db + -wal/-shm/-journal)
  -> QuarantinedVerified (source absent; owned copies verified by SHA-256)
  -> DeleteCommitted (irreversible boundary persisted)
  -> Purging (remove only transaction-owned quarantined files)
  -> Completed (postconditions verified; .setup_state.json reconciled to Fresh)
```

- **What is removed:** Only canonical `shellumi.db` and its verified local sidecars (`-wal`, `-shm`, `-journal`), after rolling back any uncommitted `vaults/.migration_staging` outputs.
- **What is preserved:** Existing modern vaults, portable manifests, SSH keys, session recordings, user-created backups, unrelated Keychain entries, and remote Google Drive / Shellumi Server snapshots.

---

## Recovering from an Earlier Interrupted Migration (`Default vault manifest not found`)

If an earlier build crashed mid-migration and displayed `Default vault manifest not found` on restart:
- **Do not** manually delete `.setup_state.json`, `vaults/`, `vault_manifests/`, or OS Keychain entries.
- Launch the updated Shellumi client: `InstallationInventory` and `MigrationEngine::inspect_state` automatically detect `vaults/.migration_staging/migration_journal.json` and route directly to **Interrupted Migration**, where you can either **Resume Migration** (preserving your original `shellumi.db` and completing portable manifest creation) or **Roll Back Safely** (returning to the Legacy Decision screen).

---

## Crash-Safe Credential & Rotation Journals

- **`credential_operations` (Schema 31):** Journals `Planned` → `Staged` → `Committed` → `Completed` / `RolledBack` transitions around OS credential-store mutations and reconciles orphaned staged or pending-delete secrets on vault load.
- **`vault_rotation`:** Journals password-rotation stages across manifest, target DB, registry DB, and OS credential store, recovering safely on restart if interrupted.

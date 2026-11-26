# Database Migrations

[Docs Home](../README.md) / [Development](README.md) / Database Migrations

---

Shellumi uses local Turso SQLite databases at **`SCHEMA_VERSION = 32`** (`crates/shellumi-storage/src/lib.rs`).

## Rules for Schema Changes

1. **Transactional & Idempotent:** Wrap every migration step in `BEGIN IMMEDIATE` / `COMMIT` and check `PRAGMA table_info(<table>)` before `ALTER TABLE ADD COLUMN`.
2. **Preserve Existing Data:** Never drop or overwrite user rows during migration.
3. **Reject Future Schemas:** Opening a database with `PRAGMA user_version > SCHEMA_VERSION` must fail closed (`UnsupportedSchemaVersion`).
4. **Required Regression Tests:** Add tests for:
   - Clean upgrade from previous schema version(s).
   - Rollback on injected failure leaving `PRAGMA user_version` and indexes intact.
   - Reopen persistence after migration.
   - Rejection of `SCHEMA_VERSION + 1`.

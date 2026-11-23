# Vault migration recovery and sync review

## Migration implementation

The migration now uses a versioned journal in `vaults/.migration_staging/` and an operating-system file lock. Startup validates the journal before using a recorded stage. A private, verified backup includes the legacy database and its WAL/SHM sidecars. Schema upgrades run on a working copy, so opening an older or unsupported schema cannot modify the original database. Encrypted legacy databases retain their original DEK in a migration-specific secure credential account for restart recovery.

Every known data table is covered. Hosts, identities, groups, snippets, tunnels, workspaces, recordings, transfer history, discovery data, collaboration data, snapshots, registry fields, server sync state, mutations and AUTOINCREMENT watermarks retain their original values and IDs. Host-owned auxiliary rows follow their host. Global metadata remains in the default vault; provider records needed by secondary discovery rows are included with those rows. Complete row comparisons and foreign-key checks run against staged databases. Unknown schema objects, unsafe/colliding vault filenames, or relationships that cannot be represented across separate databases produce actionable errors before activation.

Each verified vault is sealed by a ciphertext digest and retained on restart. An interrupted, unverified attempt can be rebuilt from the last verified source checkpoint; previously verified vaults are reused. Password envelopes are written before activation. Migration keys and moved server secrets use transaction-specific secure credential accounts. Existing credential accounts and manifests are preserved. Server identifiers, cursors, sequences, epochs, mutation IDs, idempotency keys and credential references remain compatible with Shellumi Server.

Activation uses atomic, create-only hard links on the same filesystem. Each link has a durable journal intent before publication. Existing destinations are never replaced, including apparently corrupt files. Staged files remain as ownership witnesses, allowing restart to distinguish an interrupted publication from an unrelated file. Rollback removes only unchanged links proven to reference migration-owned files and can itself resume after interruption. If an activated database was modified externally, rollback preserves it for inspection instead of risking newer records. Original databases, manifests, credentials, backups and encrypted staging files remain available.

The completion receipt is published only after all activated files, retained manifests, and final secure credential keys have been verified. A bare legacy `verified` marker cannot bypass journal recovery. Completed migrations allow normal subsequent database updates without comparing live databases against their historical hashes.

## Recovery operation and boundaries

Run migration during startup with other clients closed. If the source changes during backup or activation, migration stops or rolls back rather than silently dropping the newer records. Existing source data and all recovery artifacts remain available. Use `vaults/default.db` and each secondary database for subsequent CLI operations; `shellumi.db` is retained for recovery.

Retain `.migration_staging` and its journal together with the completion receipt. Backups of a plaintext legacy source contain the original legacy data and use private filesystem permissions (0700 directories and 0600 files on Unix); they are deliberately retained rather than automatically deleted. Keys are stored only in the secure credential store, never in the journal. On Unix, file and parent-directory changes are flushed. Other platforms use the available file flush and filesystem atomic-publication guarantees. Cross-filesystem publication is refused safely.

Old journals without sufficient ownership information are rejected with their contents intact. An existing valid destination, a corrupt verified artifact, an unsupported schema, or a changed source requires operator attention; migration never resolves those cases by overwriting user data. A rolled-back migration retains its recovery bundle for inspection. Resolve the reported conflict and archive that bundle before requesting a new migration attempt.

The small `same-file` 1.0.6 library, already present in the workspace dependency graph, is now a direct storage dependency for portable file-identity checks. Its MIT/Unlicense licensing, cross-platform implementation and existing offline availability were reviewed against the [upstream repository](https://github.com/BurntSushi/same-file). No new transport or database dependency was introduced.

## Related sync safeguards

The default registry now supplies each vault's Drive opt-in setting, including the default vault. Fresh vaults are disabled until explicit opt-in. A secondary sync uses its actual vault ID for export and import. Registry errors propagate to the desktop action. Explicit Enable/Disable controls persist consent in Settings → Synchronization. Persistent multi-vault integration tests cover independent settings, secondary export identity, default-vault restore, and multiple restored vaults sharing an existing server connection without replacing its credentials.

The follow-up now implements authenticated envelope and directory revisions, immutable create-only Drive publications, explicit concurrent-branch resolution, and cross-device password restoration. The encrypted local revision cache retains high-water marks across restarts. See [the protocol and compatibility boundaries](DRIVE_REVISION_PROTOCOL.md). The migration recovery protocol is unchanged.

## Verification

Deterministic fault injection exercises the initial journal, each backup file, key and manifest persistence, each copied table, staging commits, verification checkpoints, every activation intent/link/checkpoint, key finalization and completion publication. Separate cases interrupt rollback and test ownership collisions, corrupted verified files, changed sources, legacy markers, symlinks, encrypted sources, WAL recovery, future schemas and deleted-ID watermarks. Tests compare original bytes and manifests, preserve pre-existing credential values, and reopen migrated databases to validate records and relationships.

Migration verification before the Drive revision follow-up on macOS:

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --offline --workspace --all-targets -- -D warnings`: passed. Cargo reports existing future-incompatibility notices in the transitive `block` and `proc-macro-error2` packages.
- `cargo test --offline --workspace -- --test-threads=2`: passed, 571 tests passed, zero failures, three pre-existing isolated-OpenSSH tests ignored. The complete suite required approved loopback access outside the filesystem/network sandbox for its diagnostics integration tests.
- `cargo test --offline -p shellumi-storage roll`: passed, seven focused tests, including the final guard that retains externally modified activation files.
- Multi-vault registry integration tests: two passed, including shared-server restore without credential replacement.
- `git diff --check`: passed.

Native Windows/Linux migration acceptance and the ignored OpenSSH fixtures were not run here.

Verification also exposed two unrelated logging issues: the askpass helper now keeps verbose diagnostics off its credential streams, and logging-default tests use explicit environment inputs rather than depending on the caller's process environment.

## Files changed

- `Cargo.lock`
- `crates/shellumi-app/src/actions/sync.rs`
- `crates/shellumi-app/src/actions/vault_creation.rs`
- `crates/shellumi-app/src/askpass.rs`
- `crates/shellumi-app/src/desktop/settings.rs`
- `crates/shellumi-app/src/main.rs`
- `crates/shellumi-app/tests/askpass.rs`
- `crates/shellumi-core/src/logging.rs`
- `crates/shellumi-core/src/sync.rs`
- `crates/shellumi-core/src/sync/tests/drive.rs`
- `crates/shellumi-storage/Cargo.toml`
- `crates/shellumi-storage/src/lib.rs`
- `crates/shellumi-storage/src/migration.rs`
- `crates/shellumi-storage/src/migration/tests.rs`
- `crates/shellumi-storage/tests/multi_vault_settings.rs`
- `docs/ROADMAP.md`
- `docs/sync-migration-review-progress.md`

## Drive revision follow-up (2026-11-23)

Implemented independent authenticated component histories and full-snapshot seals;
create-only publications with exact read-back and retained ancestors; stable record
versions; portable manifest revisioning with stale-writer rejection; and consistent
desktop/CLI import of exact authenticated snapshots. Registry refresh is centralized
in manifest-backed storage exports; file sync seals complete reconciled snapshots.
Repeated imports update existing snippets/tunnels instead of duplicating them,
and all record ciphertexts are authenticated before row writes. Metadata conflicts are explicit
whole-directory choices, bound to the reviewed revisions in the desktop.

Integration coverage includes password-only and metadata-only updates, a fresh
second device with no original secrets, stale and concurrent password changes,
legacy upgrades, restart persistence, independent vault histories, incomplete or
missing cloud history, interrupted uploads and racing immutable publications.

Verification results on macOS:
- `cargo fmt --all -- --check`: passed (0 formatting errors).
- `cargo clippy --workspace --all-targets -- -D warnings`: passed (0 warnings).
- `cargo test -p shellumi-core`: passed (234 unit tests passed, 0 failed).
- `cargo test -p shellumi-storage`: passed (86 tests passed: 74 unit tests, 10 integration tests in `drive_revisions.rs`, 2 in `multi_vault_settings.rs`, 0 failed).
- `cargo test -p shellumi-cli`: passed (38 tests passed, 0 failed).
- `cargo test -p shellumi-terminal`: passed (26 unit tests passed, 0 failed).
- `cargo test -p shellumi-remote-desktop`: passed (17 tests passed, 0 failed).
- `cargo test -p shellumi-vnc`: passed (42 tests passed, 0 failed).
- `cargo test -p shellumi-rdp`: passed (47 tests passed, 0 failed).
- `cargo test -p shellumi-app`: passed (86 tests passed: 84 unit tests, 2 in `askpass.rs`, 3 pre-existing loopback socket OpenSSH fixtures ignored as expected, 0 failed).
Total: 576 workspace tests passed with zero failures.

Files for this follow-up: core `sync.rs`, `sync/cloud.rs`, `sync/revisions.rs`,
`vault_manifest.rs`, `lib.rs`, Drive tests and shared mock transport; storage
`lib.rs`, `drive_revisions.rs` and integration tests; desktop sync actions and test
fixtures; CLI sync actions; this document, the roadmap and the new protocol guide.

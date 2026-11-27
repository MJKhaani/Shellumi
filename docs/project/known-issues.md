[Documentation Home](../README.md) > [Project](README.md) > **Known Issues**

# Canonical Known Issues & Defect Register

This document is the **canonical defect register** for Shellumi (baseline `7244263`). Other documents reference these stable Issue IDs rather than maintaining duplicate defect lists.

> **Scope Distinction:** Synchronization defects (`SYNC-001`..`SYNC-006`) affect cross-device synchronization, full-vault export/import, or multi-device identity/host matching. They do **not** prevent standalone local vault operation, local SSH/SFTP/terminal testing, or single-device acceptance testing.

---

## Confirmed Defects

### `SYNC-001` — Schema-32 migration generates synchronization identifiers from local row IDs
- **Severity:** High
- **Current Status:** Open
- **Affected Operating Modes:** Shellumi Server Sync, Google Drive Sync (multi-device vaults with pre-Schema-32 records)
- **Affected Platforms:** macOS, Windows, Linux
- **Root Cause:** Migration 31→32 backfills `sync_id` using `vault_id || ':identity:' || id` and `vault_id || ':host:' || id`, which deterministically derives a supposedly cross-device identifier from device-local SQLite `AUTOINCREMENT` row IDs.
- **User Impact:** Two devices that independently created pre-Schema-32 hosts or identities with the same local row IDs (e.g., `id = 1`) receive colliding `sync_id` values during synchronization.
- **Relevant Source Files:** `crates/shellumi-storage/src/lib.rs` (migration 31→32), `crates/shellumi-storage/src/server_sync.rs`
- **Reproduction Scenario:** Create host #1 on Device A and a different host #1 on Device B before Schema 32, upgrade both, and sync into a shared/synced vault.
- **Existing Mitigation:** Newly created post-Schema-32 records generate random UUIDs; standalone local vaults are unaffected.
- **Recommended Correction:** Generate cryptographically random UUIDv4 identifiers during migration when no verified server provenance exists, or reconcile provenance deterministically before assigning `sync_id`.
- **Verification Requirements:** Two-device pre-Schema-32 upgrade and sync convergence test verifying zero `sync_id` collisions.

### `SYNC-002` — Incoming host synchronization can match unrelated hosts by name
- **Severity:** High
- **Current Status:** Open
- **Affected Operating Modes:** Shellumi Server Sync
- **Affected Platforms:** macOS, Windows, Linux
- **Root Cause:** `server_sync::apply_change_item()` falls back to matching an existing local host by display `name` when `sync_id` lookup fails.
- **User Impact:** An incoming host from another device with the same display name (e.g., `prod-bastion`) can overwrite an unrelated local host's endpoint, port, or authentication configuration.
- **Relevant Source Files:** `crates/shellumi-storage/src/server_sync.rs`
- **Reproduction Scenario:** Device A creates local host `web-1` (IP `10.0.0.1`); Device B creates a different host `web-1` (IP `10.0.0.2`) and syncs before Device A's `sync_id` is reconciled.
- **Existing Mitigation:** None automatic when `sync_id` differs and `name` collides.
- **Recommended Correction:** Remove unverified name-based overwrite fallback; surface a conflict or create a distinct host record with disambiguated metadata.
- **Verification Requirements:** Sync replay test with distinct `sync_id` values and identical `name` proving both hosts remain distinct.

### `SYNC-003` — Identity deletion can unintentionally change effective authentication
- **Severity:** High
- **Current Status:** Open
- **Affected Operating Modes:** Standalone Local, Google Drive Sync, Shellumi Server Sync
- **Affected Platforms:** macOS, Windows, Linux
- **Root Cause:** Deleting an identity triggers `ON DELETE SET NULL` on referencing `hosts.identity_id`; if a host falls back to group inheritance or `inherit_group` without an explicit blocked-auth marker, its effective authentication can silently shift.
- **User Impact:** Connecting to a host after deleting its identity may attempt an inherited group identity or unexpected auth method instead of failing closed or prompting.
- **Relevant Source Files:** `crates/shellumi-storage/src/lib.rs`, `crates/shellumi-storage/src/server_sync.rs`
- **Reproduction Scenario:** Assign Identity X to Host H inside Group G (which has Group Identity Y), delete Identity X, and inspect Host H's effective authentication.
- **Existing Mitigation:** `auth_dependency_unresolved` tracking exists for incoming sync dependencies, but local `ON DELETE SET NULL` still clears the foreign key.
- **Recommended Correction:** Mark affected hosts explicitly as `auth_dependency_unresolved` / blocked until the user selects a replacement credential.
- **Verification Requirements:** Storage + connection dispatch test proving identity deletion blocks silent auth fallback.

### `SYNC-004` — Full vault export/import does not consistently preserve stable object associations
- **Severity:** Medium
- **Current Status:** Open
- **Affected Operating Modes:** Encrypted Vault Export/Import, Google Drive Sync, Backup/Restore
- **Affected Platforms:** macOS, Windows, Linux
- **Root Cause:** `export_vault` / `import_vault` in `shellumi-storage/src/lib.rs` constructs `SyncHostPayload` / `SyncIdentityPayload` with `sync_id: None`, `identity_sync_id: None`, and `identity_name: None`, and includes `protocol_config` containing device-local credential references.
- **User Impact:** Restoring or importing a full-vault export can lose stable cross-device `sync_id` associations and carry device-local `keychain://` references that do not exist on another machine.
- **Relevant Source Files:** `crates/shellumi-storage/src/lib.rs`, `crates/shellumi-core/src/sync.rs`
- **Reproduction Scenario:** Export an encrypted vault with host-to-identity links, import it on a second device, and run incremental sync.
- **Existing Mitigation:** Encrypted payloads protect confidentiality at rest/in transit; local single-device backup/restore works when OS keychain items are still present.
- **Recommended Correction:** Export stable `sync_id` and `identity_sync_id`, and strip or mark device-local `keychain://` references explicitly during cross-device export/import.
- **Verification Requirements:** Round-trip export/import test across two simulated devices verifying preserved `sync_id` associations and safe handling of non-portable credential refs.

### `SYNC-005` — Incoming synchronization bypasses journaled cleanup of incompatible credentials
- **Severity:** Medium
- **Current Status:** Open
- **Affected Operating Modes:** Shellumi Server Sync, Google Drive Sync
- **Affected Platforms:** macOS, Windows, Linux
- **Root Cause:** When incoming host synchronization invalidates incompatible local credential references (`merge_synced_protocol_config`), it strips them from `protocol_config` without staging/committing an entry in the `credential_operations` journal.
- **User Impact:** Orphaned secrets can remain in the OS credential store after remote host updates change authentication modes.
- **Relevant Source Files:** `crates/shellumi-storage/src/server_sync.rs`, `crates/shellumi-storage/src/lib.rs`
- **Reproduction Scenario:** Device A saves a host with a host-specific password in OS Keychain; Device B updates the same host to public-key auth and syncs; Device A applies the update without cleaning the old Keychain entry.
- **Existing Mitigation:** Local edits in `save_host_with_identity_and_journal_tx()` use the `credential_operations` journal properly.
- **Recommended Correction:** Stage and commit a `credential_operations` cleanup entry whenever incoming sync invalidates a local credential reference.
- **Verification Requirements:** Sync-merge test verifying orphaned credential references are queued in `credential_operations` and cleaned post-commit.

### `SYNC-006` — Google Drive defaults to the shared installation identifier `desktop-app`
- **Severity:** Medium
- **Current Status:** Open
- **Affected Operating Modes:** Google Drive Sync
- **Affected Platforms:** macOS, Windows, Linux
- **Root Cause:** Desktop Google Drive sync (`crates/shellumi-app/src/actions/sync.rs`) falls back to the static string `"desktop-app"` whenever `SHELLUMI_DEVICE_ID` is unset.
- **User Impact:** Multiple desktop installations using Google Drive sync without `SHELLUMI_DEVICE_ID` set appear under the same device identifier in revision metadata.
- **Relevant Source Files:** `crates/shellumi-app/src/actions/sync.rs`
- **Reproduction Scenario:** Configure Google Drive sync on two desktop machines without setting `SHELLUMI_DEVICE_ID`.
- **Existing Mitigation:** Users can explicitly set `SHELLUMI_DEVICE_ID` in the environment as a workaround.
- **Recommended Correction:** Generate and persist a random UUIDv4 installation identifier per installation on first launch.
- **Verification Requirements:** Multi-device Drive sync test confirming distinct persisted installation IDs by default.

### `RD-001` — Ambiguous RDP resize timeouts can permit overlapping remote negotiations
- **Severity:** Medium
- **Current Status:** Open
- **Affected Operating Modes:** Remote Desktop (RDP)
- **Affected Platforms:** macOS, Windows, Linux
- **Root Cause:** `MS-RDPEDISP` / `ironrdp_displaycontrol` does not expose per-request acknowledgement or rejection PDUs with request IDs. `RdAutoResizeState` tracks one local in-flight request, but a local timeout does not prove remote cancellation, so bounded retries (up to 2) can overlap an unresolved server-side resize.
- **User Impact:** Under slow or congested RDP links, an earlier resize and a retry can race, causing brief resolution oscillation or ambiguous dimension matching.
- **Relevant Source Files:** `crates/shellumi-app/src/main.rs` (`RdAutoResizeState`), `crates/shellumi-rdp/src/lib.rs`
- **Reproduction Scenario:** Trigger dynamic resize or Match Viewport over a high-latency RDP connection where `RemoteResize` arrives just after the local timeout expires.
- **Existing Mitigation:** Single-in-flight coordinator, `layout_epoch` validation, `auto_cancel_generation` tokens, and conservative `settling_after_timeout` suppression (suppressing automatic retries after timeout until late confirmation, manual resize, or reconnect).
- **Recommended Correction:** Verify conservative post-timeout recovery against a live RDP server under artificial latency.
- **Verification Requirements:** Delayed-response RDP resize simulation (implemented) + live Windows/`xrdp` acceptance test (`NOT TESTED`).

---

## Platform, Security & Operational Gaps

| ID | Summary | Severity | Status | Affected Scope | Details |
| --- | --- | --- | --- | --- | --- |
| `PLAT-001` | Windows interactive GUI, `windows_subsystem = "windows"`, `CREATE_NO_WINDOW`, `PasswordVault`, and askpass pipe behavior | High | `NOT TESTED` | Windows desktop | Compiles & passes CI unit tests; requires interactive Windows verification ([Native Testing](../qa/native-testing.md)). |
| `PLAT-002` | Linux interactive GUI (GNOME/KDE, X11/Wayland) and `secret-tool` Secret Service behavior | High | `NOT TESTED` | Linux desktop | Compiles & `.deb` builds in CI; requires interactive Linux desktop verification ([Native Testing](../qa/native-testing.md)). |
| `SEC-001` | CLI `--passphrase` / `--password` / `--token` arguments visible in local process list (`ps`) | Medium | Open | `shellumi-cli` | Prefer `SHELLUMI_VAULT_PASSPHRASE` / `SHELLUMI_GDRIVE_TOKEN` env vars where supported; see [CLI Reference](../reference/cli.md). |
| `TEAM-001` | Live Shellumi Server multi-client epoch rotation & collaboration relay end-to-end verification | Medium | `NOT TESTED` | Shellumi Server Sync | Deterministic client & two-server tests pass locally; live external server QA pending ([Verification Status](../qa/verification-status.md)). |

---

## Proposed Research & Unverified Sync Concerns

The following topics are **not** confirmed implementation defects; they represent architectural hardening items under evaluation:

- **Tombstone Compaction Policy:** Evaluating safe epoch-based pruning of historical tombstones in long-lived vaults.
- **Three-Way Field Merge UI:** Exploring field-level merge UI for concurrent offline edits beyond current conflict-choice resolution.
- **Out-of-Order Group/Host Replay:** Evaluating deferred dependency queues when delta items arrive out of parent-child order.
- **Per-Entity Batch Commit Diagnostics:** Exposing granular per-entity rollback details on failed batch sync commits.

[Documentation Home](../../README.md) > [Requirements](README.md) > **Sync & Collaboration**

# Synchronization, Collaboration & Infrastructure Requirements

Recovered from §1, §4, §5, and §6 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## Vault Isolation & Multi-Window (`REQ-SYNC-001`..`004`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-SYNC-001` | Per-vault operating mode selection (local vs. remote) coexisting in one instance with isolated DBs (`default.db`, `<vault-id>.db`) and DEKs. | **Implemented** | Unit + Integration tested | `separate_databases.rs`, `VaultContext` | Split DB & raw-disk zero-plaintext tests pass. |
| `REQ-SYNC-002` | Default local vault holds encrypted discovery directory of other vaults without copying other vaults' contents or device-bound secrets. | **Implemented** | Integration tested | `SyncVault.encrypted_directory` | Directory isolation tests pass. |
| `REQ-SYNC-003` | Concurrent windows bound to independent vaults (`Cmd+Shift+N`, badges, scoped queries/sessions/background tasks). | **Implemented** | Integration + GPUI tested (`multi_window.rs`) | `multi_window.rs`, `open_vault_in_new_window` | Multi-window vault isolation & lifecycle tests pass (`INS-010`). |
| `REQ-SYNC-004` | Cross-vault copy/move/credential reuse requires explicit user confirmation and never mixes silently. | **Implemented** | Integration + GPUI tested | Vault-scoped `HostStore` & `VaultCredentialStore` | Cross-vault stale save rejection tests pass. |

## Google Drive Personal Sync (`REQ-SYNC-005`..`008`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-SYNC-005` | OAuth 2.0 PKCE (`drive.appdata`), system browser, loopback redirect, token storage, refresh/revoke/disconnect. | **Partial** | Integration tested (HTTP fixture); Live `NOT TESTED` | `build_google_drive_auth_url`, `GoogleDriveSyncClient` | Fixture OAuth/PKCE tests pass; live Google account `NOT TESTED`. |
| `REQ-SYNC-006` | Client-side AEAD (`AES-256-GCM`) + `Argon2id` key wrapping, authenticated `cloud_state` DAG, create-only snapshots (`files.generateIds`). | **Partial** | Integration tested (HTTP fixture); Live `NOT TESTED` | `docs/synchronization/google-drive.md` | DAG & HTTP 409 read-back fixture tests pass; [`SYNC-004`](../known-issues.md#sync-004--full-vault-exportimport-does-not-consistently-preserve-stable-object-associations) & [`SYNC-006`](../known-issues.md#sync-006--google-drive-defaults-to-the-shared-installation-identifier-desktop-app) open. |
| `REQ-SYNC-007` | Sync preferences, offline editing, conflict detection/resolution, encrypted backups. | **Partial** | Unit + CLI tested | `ConflictResolutionChoice`, `vault export/import/diff/rollback` | Backup/conflict tests pass; selective category sync toggles & [`SYNC-004`](../known-issues.md#sync-004--full-vault-exportimport-does-not-consistently-preserve-stable-object-associations) open. |
| `REQ-SYNC-008` | Dedicated sync status UI (account, status, last sync, errors, pending changes, conflict resolution, backup/restore). | **Implemented** (UI); **Partial** (live) | GPUI + Fixture tested | `actions/sync.rs` | Sync state UI wired; live Drive account acceptance `NOT TESTED`. |

## Team Collaboration & Shellumi Server (`REQ-TEAM-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-TEAM-001` | Self-hosted Shellumi Server sync: Ed25519/X25519 enrollment, signed CAS commits, delta feeds, tombstones, HTTP 410 recovery. | **Partial** | Integration tested (controlled server fixture); Live `NOT TESTED` | `server_sync.rs`, `real_server_verification.rs` | Controlled 2-server fixture passes; [`SYNC-001`](../known-issues.md#sync-001--schema-32-migration-generates-synchronization-identifiers-from-local-row-ids)..`005` & live server [`TEAM-001`](../known-issues.md#platform-security--operational-gaps) open. |
| `REQ-TEAM-002` | Shared vaults with Owner/Admin/Editor/Viewer roles, X25519 DEK wrapping, epoch rotation, and audit logs. | **Partial** | Integration + CLI tested; Live `NOT TESTED` | `SharedVault`, CLI `team` | Fixture & CLI tests pass; live multi-client epoch rotation `NOT TESTED` (`TEAM-001`). |
| `REQ-TEAM-003` | Real-time encrypted terminal collaboration relay with single-writer monotonic lease fencing and emergency revocation. | **Partial** | Integration tested (fixture); Live `NOT TESTED` | `MultiplayerSession`, `real_server_verification.rs` | Controlled relay fixture passes; live external server `NOT TESTED`. |
| `REQ-TEAM-004` | Enterprise extensibility: SAML/OIDC SSO, mandatory MFA policies, approved-domain policies, org provisioning, audit exports. | **Planned** | — | — | Requires optional self-hosted identity/server extensions. |

## Infrastructure Integrations (`REQ-INFRA-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-INFRA-001` | AWS EC2 discovery & inventory import without overwriting local customization. | **Partial** (file parser); **Planned** (live AWS API) | Unit + CLI tested | `AwsEc2Parser`, CLI `infra discover aws --file` | JSON parser & CLI tests pass; live AWS SDK polling **Planned**. |
| `REQ-INFRA-002` | Microsoft Azure VM discovery & subscription/resource-group organization. | **Partial** (file parser); **Planned** (live Azure API) | Unit + CLI tested | `AzureVmParser`, CLI `infra discover azure --file` | JSON parser & CLI tests pass; live Azure API polling **Planned**. |
| `REQ-INFRA-003` | DigitalOcean Droplet discovery & tag/project organization. | **Partial** (file parser); **Planned** (live DO API) | Unit + CLI tested | `DigitalOceanParser`, CLI `infra discover digitalocean --file` | JSON parser & CLI tests pass; live DO API polling **Planned**. |
| `REQ-INFRA-004` | Ansible inventory import/export (INI/YAML), local API/CLI bridge, external audit/Vanta exports. | **Partial** (Ansible/CLI); **Planned** (Vanta/audit connectors) | Unit + CLI tested | `AnsibleInventoryEngine`, CLI `infra ansible` | INI/YAML import/export tests pass; Vanta/audit connectors **Planned**. |

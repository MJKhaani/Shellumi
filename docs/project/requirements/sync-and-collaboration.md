[Documentation Home](../../README.md) > [Requirements](README.md) > **Sync & Collaboration**

# Synchronization, Collaboration & Infrastructure Requirements

Recovered from §1, §4, §5, and §6 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## Vault Isolation & Multi-Window (`REQ-SYNC-001`..`004`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-SYNC-001` | Per-vault operating mode selection (local vs. remote) coexisting in one app instance with isolated DBs (`default.db`, `<vault-id>.db`) and DEKs. | **Implemented** | `separate_databases.rs`, `VaultContext`. |
| `REQ-SYNC-002` | Default local vault holds encrypted discovery directory of other vaults without copying other vaults' contents or device-bound secrets. | **Implemented** | `SyncVault.encrypted_directory`. |
| `REQ-SYNC-003` | Concurrent windows bound to independent vaults (`Cmd+Shift+N`, badges, scoped queries/sessions/background tasks). | **Implemented** | `multi_window.rs`, `open_vault_in_new_window`. |
| `REQ-SYNC-004` | Cross-vault copy/move/credential reuse requires explicit user confirmation and never mixes silently. | **Implemented** | Vault-scoped queries & UI guards. |

## Google Drive Personal Sync (`REQ-SYNC-005`..`008`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-SYNC-005` | OAuth 2.0 PKCE (`drive.appdata`), system browser, loopback redirect, secure token storage, refresh/revoke/disconnect. | **Implemented** (fixture-tested); **Partial** (live Drive `NOT TESTED`) | `build_google_drive_auth_url`, `GoogleDriveSyncClient`. |
| `REQ-SYNC-006` | Client-side AEAD (`AES-256-GCM`) + `Argon2id` key wrapping, authenticated `cloud_state` DAG, create-only immutable snapshots (`files.generateIds`). | **Implemented** (fixture-tested) | `docs/synchronization/google-drive.md`; see `SYNC-004`, `SYNC-006` for device-local metadata & device ID. |
| `REQ-SYNC-007` | Sync preferences (per-vault opt-in, optional keys/passwords/logs), offline editing, conflict detection/resolution, encrypted backups. | **Implemented** (core); **Partial** (selective category toggles) | `ConflictResolutionChoice`, `vault export/import/diff/rollback`. |
| `REQ-SYNC-008` | Sync status UI (account, status, last sync, errors, pending changes, device info, conflict resolution, backup/restore). | **Implemented** | Sync settings/inspector UI. |

## Team Collaboration & Shellumi Server (`REQ-TEAM-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-TEAM-001` | Self-hosted Shellumi Server sync: Ed25519/X25519 device enrollment, signed CAS commits, delta feeds, tombstones, HTTP 410 snapshot recovery. | **Partial** | Client implemented & fixture-tested (`server_sync.rs`, `real_server_verification.rs`); live server `NOT TESTED`, `SYNC-001`..`005` open. |
| `REQ-TEAM-002` | Shared vaults with Owner/Admin/Editor/Viewer roles, X25519 DEK wrapping, forward-secure epoch rotation, and audit logs. | **Partial** | Client crypto & CLI `team` commands implemented; live multi-client rotation `NOT TESTED`. |
| `REQ-TEAM-003` | Real-time encrypted terminal collaboration relay with single-writer monotonic lease fencing and emergency revocation. | **Partial** | Client relay & `MultiplayerSession` implemented; live server `NOT TESTED`. |
| `REQ-TEAM-004` | Enterprise extensibility: SAML/OIDC SSO, mandatory MFA policies, approved-domain policies, org provisioning, audit exports. | **Planned** | Requires optional self-hosted identity/server extensions. |

## Infrastructure Integrations (`REQ-INFRA-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-INFRA-001` | AWS EC2 discovery & inventory import without overwriting local customization. | **Partial** | `AwsEc2Parser` & CLI `infra discover aws --file`; live AWS SDK polling planned. |
| `REQ-INFRA-002` | Microsoft Azure VM discovery & subscription/resource-group organization. | **Partial** | `AzureVmParser` & CLI `infra discover azure --file`; live Azure API polling planned. |
| `REQ-INFRA-003` | DigitalOcean Droplet discovery & tag/project organization. | **Partial** | `DigitalOceanParser` & CLI `infra discover digitalocean --file`; live DO API polling planned. |
| `REQ-INFRA-004` | Ansible inventory import/export (INI/YAML), local API/CLI bridge, external audit/Vanta exports. | **Partial** (Ansible/CLI); **Planned** (Vanta/external audit connectors) | `AnsibleInventoryEngine`, CLI `infra ansible`. |

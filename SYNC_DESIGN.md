# Google Drive Synchronization Design

This document details the architectural specification for Shellumi's encrypted synchronization engine using Google Drive.

---

## 1. Design Principles

1. **Fully Opt-in:** Shellumi must remain 100% functional locally without enabling Google Drive sync.
2. **Zero Knowledge / End-to-End Encryption:** Google Drive acts as dumb blob storage. Google never receives plaintext data, encryption keys, or vault passphrases.
3. **Least Privilege Scope:** Uses the restricted Google Drive Application Data folder:
   `https://www.googleapis.com/auth/drive.appdata`
   This isolates Shellumi data from general Drive files and prevents reading or modifying unrelated user files.
4. **Crash-Safe & Atomic:** Synchronization must never corrupt local state or overwrite newer remote data blindly.

---

## 2. Authentication Flow

- **Protocol:** OAuth 2.0 Authorization Code Flow with PKCE (Proof Key for Code Exchange, RFC 7636).
- **User Experience:** Launches default system browser; captures callback via a local HTTP loopback redirect listener (`http://127.0.0.1:<random_port>/callback`).
- **Token Security:** The intended design stores OAuth tokens in an OS-backed credential store. The macOS Keychain backend exists; Windows Credential Manager and Linux Secret Service are not yet implemented.
- **No Embedded Client Secrets:** Designed for public desktop clients without embedded privileged credentials.

---

## 3. Cryptographic Architecture

```text
User Passphrase
     │
     ▼ (Argon2id: Salt, memory-hard parameters)
Master Key Encryption Key (KEK)
     │
     ▼ (Decrypts envelope)
Data Encryption Key (DEK)
     │
     ▼ (XChaCha20-Poly1305 / AES-256-GCM AEAD)
Authenticated Ciphertext Envelope
```

- **AEAD Encryption:** Every synced record is sealed with an Authenticated Encryption with Associated Data (AEAD) cipher (XChaCha20-Poly1305 or AES-256-GCM).
- **Key Derivation:** Argon2id is parameterized with modern recommended memory and iteration costs.
- **Unique Nonces & Salts:** Nonces and salts are cryptographically random and unique per envelope.
- **Associated Data:** Metadata influencing decryption (e.g. schema version, entity type, object ID) is bound into the AEAD authentication tag to prevent record transposition attacks.

---

## 4. Sync Data Model & Conflict Resolution

### Record Structure
Every synchronized record contains:
- `object_id`: Stable UUIDv4 identifying the entity.
- `version`: Incrementing revision counter.
- `device_id`: Unique identifier of the originating device.
- `updated_at`: Timestamp (ISO 8601 UTC).
- `deleted`: Boolean tombstone marker for deletions.
- `ciphertext`: Encrypted payload containing entity fields.
- `signature` / `tag`: AEAD authentication tag.

### Conflict Resolution Strategy
- **Additive & Independent Changes:** Automatic three-way merge where property sets do not overlap.
- **Concurrent Modifications:** If two devices modify the same host record concurrently, Shellumi creates a backup snapshot and prompts the user with an interactive diff preview.
- **No Blind Last-Write-Wins (LWW):** Unconditional LWW is strictly forbidden for credentials, certificates, or SSH keys to avoid accidental credential loss.
- **Tombstones:** Deleted records are tracked as tombstones so deletions propagate across all active devices before being purged.

---

## 5. Network Engine & Recovery

- **Triggers:** Sync on startup, manual sync, and debounced sync-on-change (e.g., 3–5 second debounce window).
- **Offline First:** All modifications commit immediately to the local Turso database and queue an outbound sync event.
- **Network Resilience:** Exponential backoff with jitter on network disconnects or API rate limits.
- **Disaster Recovery:** Shellumi preserves the previous two sync snapshots locally so users can restore historical versions if needed.

---

## 6. Implementation Status

- **Cryptographic Engine (`shellumi-core::sync`):** Implemented AES-256-GCM authenticated envelope encryption (`seal_payload`, `open_payload`), PBKDF2-HMAC-SHA256 key derivation with memory zeroization on drop, minimum passphrase length enforcement, bound associated data (`entity_type:object_id`), versioned `SyncRecord` models with deletion tombstones, and three-way conflict detection (`determine_sync_action`).
- **OAuth 2.0 PKCE Generator & Loopback Listener (`shellumi-core::sync`):** Implemented RFC 7636 compliant 32-byte cryptographically secure random code verifier, SHA-256 code challenge generation, Google Drive authorization URL builder scoped to `https://www.googleapis.com/auth/drive.appdata`, `OAuthCallbackListener` loopback HTTP server with CSRF state validation, user-friendly HTML redirect response pages (`build_oauth_callback_response`), and token expiration tracking (`OAuthTokenBundle`).
- **Record Difference Engine (`shellumi-core::sync`):** Implemented `diff_sync_records_with_passphrase` providing record-by-record comparison across local storage and remote snapshots, incorporating payload-level decryption to verify true entity equivalence and eliminate false-positive conflicts across independent export runs.
- **Google Drive AppData REST API Client (`shellumi-core::sync`):** Implemented `GoogleDriveSyncClient` over transport-agnostic `HttpTransport`, supporting authorization code exchange, token refresh, file listing, multipart snapshot upload (`DRIVE_APPDATA_FILENAME = "shellumi_vault.json"`), snapshot download, and file update/deletion.
- **Conflict-Safe Sync Planner (`shellumi-core::sync`):** Implemented `plan_sync_cycle` generating `SyncPlan` (`UploadInitial`, `DownloadInitial`, `AlreadySynchronized`, `UploadLocalUpdates`, `ApplyRemoteUpdates`, `ConflictDetected`) to prevent blind overwriting of concurrent changes.
- **Database Vault Export & Import (`shellumi-storage`):** Implemented `HostStore::export_vault` and `export_vault_with_iterations` to serialize and seal all hosts (including protocol and protocol_config), groups, tunnels, snippets, and workspaces into an encrypted snapshot, and `HostStore::import_vault` to decrypt and restore or update entities with an `ImportVaultSummary` (schema version 10).
- **CLI Vault Commands (`shellumi-cli`):** Implemented `vault export <output-path>`, `vault import <input-path>`, `vault diff <input-path>`, `vault status`, and `vault oauth-url`.
- **Desktop UI status:** Cloud synchronization is not integrated. The sync control reports unavailable instead of claiming success.
- **Multi-Platform Credential Storage & OAuth Token Persistence (`shellumi-core::credential`):** Implemented native OS credential store backends for macOS (`MacOsKeychainStore`), Windows (`WindowsCredentialManagerStore`), and Linux (`LinuxSecretServiceStore`), alongside a cross-platform AES-256-GCM authenticated file credential store (`EncryptedFileCredentialStore`) with PBKDF2-HMAC-SHA256 and automated routing via `default_credential_store()`. Built secure token lifecycle persistence via `OAuthTokenBundle::save_to_store`, `OAuthTokenBundle::load_from_store`, and `OAuthTokenBundle::delete_from_store`.
- **Conflict resolution:** Domain models exist, but the desktop conflict resolver is not connected to real synchronization and cannot apply resolutions.
- **Disaster Recovery & Historical Snapshots (`shellumi-storage` & `shellumi-cli`):** Implemented `sync_snapshots` table (schema version 7, auto-migrating from versions 1..=6 to 7), automated snapshot archiving during export and import operations with configurable retention pruning (keeping the latest 3 snapshots by default), snapshot listing (`list_snapshot_history`), snapshot vault extraction (`get_snapshot_vault`), and historical snapshot rollback (`rollback_snapshot`). Exposed via CLI subcommands `vault history` and `vault rollback <snapshot-id> [--passphrase <p>]`.
- **Sync scheduler:** Scheduling models exist; they do not perform real cloud synchronization.
- **Next milestone:** Implement and test authenticated end-to-end cloud synchronization and recovery before enabling it.

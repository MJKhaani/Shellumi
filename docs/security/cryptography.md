[Documentation Home](../README.md) > [Security](README.md) > **Cryptography**

# Vault Security & Cryptographic Envelope Specification

This document specifies the cryptographic architecture, envelope format, Key Derivation Functions (KDF), manifest storage, and synchronization security for Shellumi vaults.

---

## 1. Security Architecture Overview

Shellumi provides authentic zero-knowledge isolation and encryption for all vault contents. Each vault is encrypted independently at rest, preventing unauthorized access across vault boundaries or cross-vault leakage.

```text
+-----------------------------------------------------------------------------------+
| User Password / Recovery Code                                                    |
+-----------------------------------------------------------------------------------+
                                         │
                                         ▼ Argon2id KDF (m=64 MiB, t=3, p=1, salt >= 16 bytes)
+-----------------------------------------------------------------------------------+
| 256-bit Key Encryption Key (KEK)                                                  |
+-----------------------------------------------------------------------------------+
                                         │
                                         ▼ AES-256-GCM Unwrap (AAD bound to VaultId & params)
+-----------------------------------------------------------------------------------+
| 256-bit Data Encryption Key (DEK)                                                 |
+-----------------------------------------------------------------------------------+
                                         │
                                         ▼ Turso Experimental Encryption (AES-256-GCM)
+-----------------------------------------------------------------------------------+
| Encrypted Vault Database (hosts, credentials, snippets, workspaces, tunnels)      |
+-----------------------------------------------------------------------------------+
```

---

## 2. Independent Random Data Encryption Keys (DEKs)

1. **Generation:** Every vault—including the default vault—generates an independent, cryptographically secure random 256-bit (32-byte) Data Encryption Key (DEK) using system entropy (`ring::rand::SystemRandom`).
2. **Decoupling:** The DEK is **never** derived directly from the user's password. This allows instant, atomic password changes and multiple independent unlock envelopes (e.g. primary password and recovery code) without re-encrypting database pages.
3. **Database Page Encryption:** The recovered DEK is supplied directly to the Turso storage engine via experimental AES-256-GCM page encryption:
   ```rust
   turso::Builder::new_local(path)
       .experimental_encryption(true)
       .with_encryption(turso::core::EncryptionOpts {
           cipher: "aes256gcm",
           hexkey: hex::encode(dek.expose_bytes()),
       })
   ```

---

## 3. Password Policies & KEK Derivation

### Password Complexity Requirements
Vault passwords must meet the following baseline requirements enforced by `validate_vault_password`:
- **Minimum length:** 8 characters (longer passphrases recommended).
- **Character classes:** At least one uppercase letter (`A-Z`), one lowercase letter (`a-z`), one numeric digit (`0-9`), and one symbol (e.g. `!@#$%^&*`).
- High-entropy recovery codes (32 random uppercase alphanumeric characters formatted as `XXXX-XXXX-...`) are exempt from lowercase/symbol requirements.

### Key Derivation Function (Argon2id)
Key Encryption Keys (KEKs) are derived from user passwords using **Argon2id** (`argon2` crate):
- **Algorithm Identifier:** `argon2id_aes256gcm_v2`
- **Memory Cost (`m_cost`):** 65,536 KiB (64 MiB)
- **Time Cost / Iterations (`t_cost`):** 3
- **Parallelism (`p_cost`):** 1 lane
- **Salt:** At least 16 cryptographically random bytes generated per envelope.
- **Output Key Length:** 256 bits (32 bytes).

---

## 4. Cryptographic Envelope & Associated Data Binding

The DEK is wrapped using AES-256-GCM:
- **Nonce:** Fresh 96-bit (12-byte) random nonce per wrap.
- **Tag:** 128-bit authentication tag appended to the ciphertext.
- **Authenticated Associated Data (AAD):** Prevents ciphertext transposition across vaults or parameter tampering:
  ```text
  shellumi:vault-envelope:v2:{vault_id}:argon2id_aes256gcm_v2:{m_cost}:{t_cost}:{p_cost}
  ```
  Any tampering with the vault ID, suite, or Argon2id parameters causes tag verification to fail immediately.

---

## 5. Portable Key Envelope Manifest (`VaultManifest`)

Because the database file is encrypted on disk with the DEK, the envelope cannot be stored solely inside the database before opening it. Shellumi persists a portable, versioned JSON manifest outside the database file:

### Manifest Location
```text
{database_dir}/vault_manifests/{vault_id}.manifest.json
```

### Format Schema (Version 2)
```json
{
  "format_version": 2,
  "vault_id": "vlt-1732200000-123456",
  "name": "Production Infrastructure",
  "primary_envelope": {
    "format_version": 2,
    "suite": "argon2id_aes256gcm_v2",
    "key_epoch": 0,
    "salt": "<base64-encoded-salt>",
    "nonce": "<base64-encoded-96bit-nonce>",
    "ciphertext": "<base64-encoded-wrapped-dek-and-tag>",
    "associated_data": "shellumi:vault-envelope:v2:vlt-1732200000-123456:argon2id_aes256gcm_v2:65536:3:1",
    "kdf_iterations": 3,
    "argon2_m_cost": 65536,
    "argon2_t_cost": 3,
    "argon2_p_cost": 1
  },
  "recovery_envelope": {
    "format_version": 2,
    "suite": "argon2id_aes256gcm_v2",
    "key_epoch": 0,
    "salt": "<base64-encoded-salt>",
    "nonce": "<base64-encoded-nonce>",
    "ciphertext": "<base64-encoded-ciphertext>",
    "associated_data": "...",
    "kdf_iterations": 3,
    "argon2_m_cost": 65536,
    "argon2_t_cost": 3,
    "argon2_p_cost": 1
  },
  "remember_in_keychain": true,
  "created_at": 1732200000,
  "updated_at": 1732200000
}
```

### Atomic Manifest Updates
Manifest writes use an atomic write-and-rename pattern:
1. Write JSON contents to a temporary file `{manifest_path}.tmp.{nanos}`.
2. Flush to disk (`sync_all`).
3. Atomically rename the temporary file over `{manifest_path}`.

---

## 6. Vault Unlocking & OS Credential Store (Keychain)

### Unlocking Sequence
1. Shellumi checks whether the active vault's DEK is cached in memory (`active_vault_keys`).
2. If absent and `remember_in_keychain` is enabled, Shellumi queries the native OS Credential Store (macOS Keychain, Windows Credential Manager, or Linux Secret Service via `keyring`).
3. If still locked, the interactive **Unlock Vault** modal prompts the user for their password or 32-character recovery code.
4. Shellumi reads `VaultManifest`, verifies format version and AAD, derives the KEK via Argon2id, and decrypts the DEK.
5. If the user checked "Remember on this device", the DEK is saved to the OS Credential Store.
6. The recovered DEK is passed to `HostStore::open_with_key`, unlocking the database.

### Keychain Security Guarantees
- **Passwords are never stored in the Keychain.** Only the random 256-bit DEK is stored.
- Keychain entries are bound exclusively to the local device and are **never synchronized** to cloud storage or remote servers.
- Unlocking the default vault **never** unlocks secondary or remote vaults.

---

## 7. Password Changes and Recovery

- **Password Rotation:** To change a vault's password, the user enters the current password and the new password. The DEK is unwrapped in memory, validated against password rules, re-wrapped with fresh salt and nonce using the new password, and atomically saved to `VaultManifest`. The underlying database pages remain unchanged.
- **Recovery Codes:** During vault creation, a 32-character high-entropy recovery code (`generate_recovery_code`) is generated and wraps a secondary envelope. The user can paste this recovery code into the unlock dialog to regain access if the primary password is lost.

---

## 8. Cloud & Google Drive Synchronization Security

- **Opted-in Vaults:** For secondary vaults opted into Google Drive synchronization, Shellumi exports encrypted records together with the password-wrapped DEK envelope (`SyncVault.key_envelope`). Another authorized device running Shellumi can download the snapshot, prompt for the vault password, unwrap the DEK, and initialize local storage.
- **Default Vault Isolation:** The default vault (`legacy-local`) synchronizes **only directory metadata** (`SyncVault.encrypted_directory`) describing vault identifiers and server connection coordinates. It **never** synchronizes local credentials, hosts, snippets, or other vaults' DEKs.
- **Shellumi Server Integration:** Remote vaults retain end-to-end encryption with ephemeral X25519 device key agreements and Ed25519 signatures, preserving zero-knowledge security across self-hosted and corporate server clusters.

---

## 9. Backwards Compatibility & Version Migration

- **Format Version 1 Deprecation:** Legacy envelopes utilizing format version 1 (`pbkdf2_hmac_sha256_aes256gcm`) are detected during `unwrap_with_password` and unwrapped using legacy PBKDF2 parameters.
- **Predictable Passphrase Elimination:** Legacy code that derived passphrases using predictable strings (such as `shellumi-vault-recovery-{vault_id}`) has been permanently eliminated. All new vaults strictly generate independent random DEKs and prompt for user-defined passwords.
- **Seamless Upgrade:** When a legacy vault's password is updated or re-saved, it is automatically re-wrapped and upgraded to Format Version 2 (Argon2id + AES-256-GCM).

---

## 10. Physical Encrypted Database Architecture

To provide total hardware- and disk-level isolation, Shellumi stores vaults in separate physical encrypted SQLite databases:
- **Default / Registry Database:** `Shellumi/vaults/default.db` (for default vault `legacy-local`). Encrypted with its own random 256-bit DEK. Houses the central vault registry (`vault_contexts`, `vault_server_connections`, `vault_sync_state`, `vault_directories`) and the user's default records.
- **Secondary Vault Databases:** `Shellumi/vaults/<vault-id>.db`. Each secondary vault database has its own independent random 256-bit DEK and contains exclusively that vault's records (hosts, identities, groups, snippets, workspaces, tunnels, session recordings).
- **Registry Privacy:** The registry in `default.db` stores metadata only (vault ID, display name, location type, remote server reconnect URLs). It **never** contains secondary vaults' credentials, hosts, private keys, DEKs, or encrypted contents.
- **Strict Decoupling:** Unlocking one vault never unlocks another. Opening a secondary database using the default DEK—or vice-versa—fails immediately.
- **Fail-Closed Protection:** `HostStore::open_with_key` rejects any unencrypted open request (`key.is_none()`) targeting protected vault paths before any file creation or query execution occurs.

---

## 11. Resumable Migration Engine (`MigrationEngine`)

Shellumi provides safe, resumable, zero-loss migration from legacy shared `shellumi.db` installations:
1. **Detection:** `MigrationEngine::is_migration_needed` triggers when `shellumi.db` exists and `vaults/default.db` has not yet been provisioned.
2. **Timestamped Backups:** Creates atomic backups of `shellumi.db`, WAL, and SHM files to `shellumi.db.backup.<timestamp>`.
3. **Registry and Default Records:** Re-initializes `vaults/default.db` with an encrypted backend and migrates contexts, directory models, and default records.
4. **Server Credential Hardening:** Extracts plaintext tokens (`auth_token`, `refresh_token`), signing private keys, and agreement keys from `vault_server_connections` columns into the OS-protected `CredentialStore`, replacing column values with opaque `keychain://org.shellumi.server/...` references.
5. **Physical Partitioning:** Migrates each secondary vault into `vaults/<vault-id>.db` with its own newly provisioned DEK and manifest.
6. **Atomic Activation:** Renames the legacy database to `shellumi.db.migrated`. Subsequent app launches detect `vaults/default.db` and bypass migration.

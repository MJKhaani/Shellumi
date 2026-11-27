# Credential Management

[Docs Home](../README.md) / [Architecture](README.md) / Credential Management

---

`crates/shellumi-core/src/credential.rs` defines the `CredentialStore` trait and `VaultCredentialStore` ownership wrapper.

## Credential Store Backends

| Backend | Platform | Mechanism |
| :--- | :--- | :--- |
| **`MacOsKeychainStore`** | macOS | Native `security-framework` Keychain Services (`kSecClassGenericPassword`) |
| **`WindowsCredentialManagerStore`** | Windows | Windows PowerShell helper invoking `Windows.Security.Credentials.PasswordVault` (`CREATE_NO_WINDOW`) |
| **`LinuxSecretServiceStore`** | Linux | `secret-tool` (`org.freedesktop.secrets` D-Bus Secret Service) |
| **`EncryptedFileCredentialStore`** | Cross-platform | Opt-in AES-256-GCM + PBKDF2-HMAC-SHA256 fallback file (`SHELLUMI_VAULT_PATH`, default `Shellumi/credentials.enc`) paired with the native backend (`DaemonWithFallbackStore`) when `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` (>= 12 chars) is set |
| **`EphemeralCredentialStore`** | Tests / `SHELLUMI_CREDENTIAL_STORE=ephemeral` | Non-durable in-memory store (also fallback on unsupported OSes) |

## Vault Ownership & Credential Journal

- Every stored secret uses a canonical vault-scoped URI (`keychain://...`) generated with cryptographically secure randomness (`VaultCredentialStore::generate_credential_id`) and validated by `VaultCredentialStore::validate_reference()`.
- Updates and deletions run through `credential_operations` (Schema 31):
  ```mermaid
  stateDiagram-v2
      [*] --> Planned: Record old_refs & planned_new_refs
      Planned --> Staged: Stage new secrets in OS store
      Staged --> Committed: Commit host/identity SQLite TX
      Committed --> Completed: Delete old_refs no longer in use
      Staged --> RolledBack: Abort on DB error -> delete staged refs
  ```
- On vault load, `HostStore::reconcile_credential_operations` cleans up any interrupted `Staged` or `Committed` operations without deleting secrets still referenced by other records.

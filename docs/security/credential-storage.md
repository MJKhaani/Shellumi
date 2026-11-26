# Credential Storage & Secret Hygiene

[Docs Home](../README.md) / [Security](README.md) / Credential Storage

---

- **OS Credential Stores:** Shellumi integrates with macOS Keychain (`security-framework`), Windows Credential Manager (`PasswordVault` PowerShell helper with `CREATE_NO_WINDOW`), and Linux Secret Service (`secret-tool`), plus an explicit opt-in `EncryptedFileCredentialStore` (AES-256-GCM + PBKDF2-HMAC-SHA256) when `SHELLUMI_CREDENTIAL_STORE=encrypted-file` and `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` (>= 12 chars) are configured.
- **Vault-Scoped Ownership:** Every secret reference (`keychain://...`) is validated by `VaultCredentialStore::validate_reference()` before reading, replacing, or deleting credentials.
- **No Plaintext Persistence or Logging:** Passwords, passphrases, and private keys are never stored in `*.preferences.json`, `SyncHostPayload`, `SyncIdentityPayload`, or logs (`Redacted<T>` / `redact_sensitive` per [Logging Guidelines](../development/logging.md)).
- **Memory Zeroization:** Sensitive secret buffers and invitation tokens implement `Zeroize` / `ZeroizeOnDrop` where supported by the Rust toolchain.
- **Crash-Safe Replacement & Cleanup:** Secret updates and deletions are journaled in `credential_operations` (Schema 31) and reconciled on vault load.

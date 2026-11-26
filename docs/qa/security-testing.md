[Documentation Home](../README.md) > [QA](README.md) > **Security Testing**

# Security Testing

Security properties in Shellumi are verified through automated unit/integration tests and static lint gates.

## Covered Automated Checks

1. **Key Derivation & Vault Encryption (`shellumi-core`, `shellumi-storage`):**
   - `Argon2id` (`argon2id_aes256gcm_v2`, `m=65536`, `t=3`, `p=1`) key derivation and AES-256-GCM authenticated encryption/decryption round-trips.
   - Wrong-passphrase rejection and tampered-ciphertext authentication failure.
2. **Vault Isolation & Cross-Scope Boundaries:**
   - Separate vault databases/namespaces prevent credential or host leakage across local, Google Drive, and Team vaults.
3. **Credential Operation Journal & Crash Recovery:**
   - Atomic SQLite/Turso transactions ensure `save_host_with_identity_and_journal_tx()` and related mutations either commit host, identity, sync mutation, and journal state together or roll back cleanly.
   - Startup recovery reconciles incomplete `Pending` journal entries without leaving orphaned credentials.
4. **SSH Host-Key Verification (TOFU):**
   - Host-key mismatch rejects connections rather than silently accepting changed fingerprints.
5. **Input & Command Injection Guards:**
   - Hostnames, usernames, ports, and jump-host specifications reject whitespace, control characters, and shell metacharacters.
6. **Log Sanitization & Redaction (`docs/development/logging.md`):**
   - `Redacted<T>` and `redact_sensitive` ensure passwords, private keys, passphrases, and tokens are never emitted in `tracing` output.

## Manual / Pending Security Verification

- **Native OS Keychain & Credential Manager Edge Cases:** Interactive Windows `PasswordVault` and Linux `secret-tool` failure/lockout behavior (`NOT TESTED` interactively).
- **Independent External Security Audit:** No third-party cryptographic audit has been completed yet.

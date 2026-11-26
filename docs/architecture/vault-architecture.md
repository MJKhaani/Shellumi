# Vault Architecture

[Docs Home](../README.md) / [Architecture](README.md) / Vault Architecture

---

Shellumi isolates each vault at the filesystem, cryptographic, and UI levels:

- **Physical Databases:**
  - Default vault (`legacy-local`): `Shellumi/vaults/default.db` (also hosts the vault registry `vault_contexts` and `vault_server_connections`).
  - Secondary/remote vaults: `Shellumi/vaults/<vault-id>.db`.
- **Per-Vault DEKs & Manifests:** Each vault generates a random 256-bit Data Encryption Key (DEK) via `ring::rand::SystemRandom` and persists primary + recovery wrapped envelopes in `vault_manifests/<vault-id>.manifest.json` (Format v2: Argon2id `m=65536, t=3, p=1` + AES-256-GCM with vault-bound AAD).
- **Atomic Password Rotation (`vault_rotation.rs`):** Re-wraps the existing DEK under the new password across manifest, target DB, registry DB, and OS credential store using exclusive file locks and a durable recovery journal.

See [Cryptography & Envelopes](../security/cryptography.md) and [Vault Isolation](../security/vault-isolation.md).

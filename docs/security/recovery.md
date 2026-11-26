# Recovery & Password Rotation Security

[Docs Home](../README.md) / [Security](README.md) / Recovery

---

- **Recovery Codes:** Each vault generates a 32-character high-entropy recovery code wrapping a secondary DEK envelope in `VaultManifest`.
- **Atomic Password Rotation (`vault_rotation`):** Re-wraps the existing DEK across `VaultManifest`, target DB, registry DB, and OS credential store under exclusive file locks and a durable recovery journal. Interrupted rotations recover deterministically on next startup.
- **Legacy Migration Recovery:** `MigrationEngine` keeps timestamped backups (`0700`/`0600` on Unix) and refuses to overwrite existing destinations.
- **Important Limit:** Password rotation re-wraps the DEK locally and in new Drive key histories, but cannot revoke historical encrypted snapshots already sealed under that DEK.

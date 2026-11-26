# Glossary

[Docs Home](../README.md) / [Reference](README.md) / Glossary

---

- **DEK (Data Encryption Key):** Random 256-bit symmetric key used for Turso AES-256-GCM database encryption.
- **KEK (Key Encryption Key):** 256-bit key derived from a user password or recovery code via Argon2id to wrap/unwrap a vault's DEK.
- **TOFU (Trust-On-First-Use):** Interactive verification of an unknown SSH host key or RDP/VNC TLS certificate fingerprint on first connection.
- **CAS (Compare-And-Swap):** Optimistic concurrency mechanism used by Shellumi Server commits (`REVISION_MISMATCH` on stale revisions).
- **`MS-RDPEDISP`:** RDP Dynamic Display Control virtual channel used for live desktop resolution resizing.
- **`sync_id`:** Immutable cross-device synchronization object identifier introduced in Schema 32.

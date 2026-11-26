# <img src="../../resources/icon_pack/icons/48/regular/security_audit.png" width="26" height="26" alt="Security"> Security Architecture & Documentation

[Docs Home](../README.md) / Security

Authoritative security guarantees, cryptographic specifications, trust boundaries, and known limitations for Shellumi.

---

| Document | Scope |
| :--- | :--- |
| [**Threat Model**](threat-model.md) | System threat model, trust boundaries, and attack mitigations |
| [**Cryptography & Envelopes**](cryptography.md) | Argon2id KDF, AES-256-GCM DEK wrapping, Turso encryption, and AAD binding |
| [**Credential Storage**](credential-storage.md) | OS keychain backends, `VaultCredentialStore` ownership, and memory zeroization |
| [**Vault Isolation**](vault-isolation.md) | Physical database isolation, multi-window isolation, and fail-closed opening |
| [**Device Trust & Transport**](device-trust.md) | SSH host-key TOFU, RDP/VNC TLS pinning, Ed25519/X25519 enrollment |
| [**Recovery & Rotation**](recovery.md) | Recovery codes, `vault_rotation`, quarantine, and `credential_operations` recovery |
| [**Known Limitations**](known-limitations.md) | Open security boundaries, unverified runtime checks, and sync caveats |

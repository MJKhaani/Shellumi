# Security Policy

## Reporting a Vulnerability

If you discover a potential security vulnerability in Shellumi, please report it **privately**:

- **Do not open public GitHub issues** for undisclosed security vulnerabilities.
- Contact the repository maintainers privately via GitHub Security Advisories (`https://github.com/MJKhaani/Shellumi/security/advisories/new`) or private maintainer contact.
- Include reproduction steps, affected versions/commits, and threat impact. We aim to acknowledge reports within 48 hours and coordinate a responsible disclosure timeline.

---

## Supported Versions

Shellumi is currently in active alpha development (`v0.0.8-alpha`). Security fixes are applied to the `main` branch and included in subsequent tagged alpha releases.

---

## Security Architecture Overview

Shellumi is built with a **local-first, client-side encrypted** security model:

- **Offline-First & No Telemetry:** Shellumi works completely offline without mandatory accounts or telemetry.
- **Per-Vault Database Encryption:** Each vault uses an independent random 256-bit Data Encryption Key (DEK) for Turso AES-256-GCM database encryption, wrapped in a `VaultManifest` using Argon2id + AES-256-GCM.
- **Explicit Host-Key & TLS Verification:** SSH connections enforce host-key verification via OpenSSH `known_hosts`; RDP/VNC connections verify TLS/VeNCrypt certificates via Trust-On-First-Use (TOFU) SHA-256 fingerprint pinning.
- **Secret Hygiene:** Passwords, passphrases, and private keys are never written to logs (enforced via `Redacted<T>` and `docs/development/logging.md`) or serialized into portable sync payloads.

For full technical details, see the authoritative security documentation:
- [Security Overview](docs/security/README.md)
- [Threat Model](docs/security/threat-model.md)
- [Cryptography & Envelopes](docs/security/cryptography.md)
- [Credential Storage](docs/security/credential-storage.md)
- [Known Security Limitations](docs/security/known-limitations.md)

---

## Important Current Limitations

- **OpenSSH Process Dispatch:** SSH/SFTP/Mosh/Tunnel connections invoke the system `ssh`/`mosh` binaries with explicit arguments and an askpass helper pipe rather than an embedded Rust SSH stack.
- **Native Runtime Acceptance:** Windows Credential Manager (`PasswordVault` PowerShell helper) and Linux Secret Service (`secret-tool`) backends are implemented in `shellumi-core`, but interactive Windows/Linux desktop acceptance testing has not yet been completed on native hosts (`NOT TESTED`).
- **Synchronization Identifiers & Team Epoch Keys:** See [Object Identifiers](docs/synchronization/object-identifiers.md) and [Known Security Limitations](docs/security/known-limitations.md) for active security boundaries and open remediation items.

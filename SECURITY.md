# Security Policy and Architecture

## 1. Security Philosophy

Shellumi is designed with a **security-first, privacy-first, local-first** architecture:
- **No mandatory cloud accounts:** The application operates completely offline without telemetry, tracking, or cloud dependencies.
- **Client-Side End-to-End Encryption (E2EE):** Any optional synchronization (e.g., Google Drive) sends authenticated ciphertext only. The remote storage provider never sees plaintext secrets or passphrases.
- **Explicit Host-Key Verification:** Host keys are never accepted silently. Unknown host keys require explicit confirmation (TOFU), and changed keys are flagged with prominent security warnings.
- **Zero Plaintext Secrets:** Private keys, passwords, and sensitive tokens are never stored in plain configuration files, logs, or unencrypted database tables.

---

## 2. Cryptography and Credential Management

- **AEAD Encryption:** The implemented vault envelope uses AES-256-GCM.
- **Key Derivation:** The current implementation uses PBKDF2-HMAC-SHA256 with random salts. Argon2id is not implemented.
- **Credential Storage:** A macOS Keychain backend and encrypted file store exist. Windows Credential Manager and Linux Secret Service are not yet implemented and return errors. End-to-end credential workflows need validation.
- **Vault passphrases:** CLI vault operations require an explicit passphrase. Arguments can be visible to local processes; secure interactive entry is not yet implemented. Exports made with the former hard-coded default should be replaced.
- **Buffer Hygiene:** Secret memory buffers should be zeroized upon release where supported by the Rust toolchain.
- **Mobile Hardware Security:** Mobile platforms enforce hardware-backed key storage (`MobileCredentialSpec`) leveraging the iOS Secure Enclave (`kSecAccessControlBiometryAny`) and Android StrongBox Keystore (`setUserAuthenticationRequired(true)`). Passwords and private keys are never stored in unencrypted mobile storage.

---

## 3. Threat Model Overview

| Area | Threat | Mitigation Strategy |
| :--- | :--- | :--- |
| **SSH Connections** | Man-in-the-Middle (MitM) & Host-key substitution | Enforce strict host-key verification against known-hosts; prohibit silent bypass. |
| **Terminal Emulator** | Malicious escape sequences / clipboard hijack | Defense-in-depth parsing; OSC sequence sanitization strips ASCII control characters and clamps titles to 256 characters; configurable scrollback capacity prevents memory exhaustion; clipboard writes require explicit user action. |
| **Process Execution** | Command injection via crafted arguments | Arguments are passed as discrete vectors without shell evaluation; delimiter `--` separates options from target hostnames. |
| **Cloud Sync** | Data compromise via stolen sync files or account breach | Client-side AEAD encryption envelopes derived via Argon2id; provider receives ciphertext only. |
| **Local Storage** | Unauthorized database access | Metadata database is stored in user app data; credentials are kept in OS keychain or encrypted vaults. |
| **Telnet Connections** | Cleartext credential and traffic sniffing | Prohibit silent usage; enforce prominent warning badges in CLI and desktop UI; default port 23; gate SSH-exclusive capabilities (SFTP, port forwarding) away from Telnet. |
| **Serial Connections** | Unauthorized local device access / device misconfiguration | Strict validation of port paths, baud rates, parity, stop bits; isolate from remote network tunnels and prevent automated command injection. |
| **Team Collaboration** | Unauthorized vault modification, privilege escalation, or stale member access | Role hierarchy (`Owner`, `Administrator`, `Editor`, `Viewer`) gating administrative and write capabilities; cryptographic epoch key rotation upon member revocation; append-only audit trail for all vault and membership events. |
| **Multiplayer Terminal** | Concurrent keystroke race conditions or rogue participant commands | Single-active-writer keystroke arbitration; non-controlling participants are strictly read-only; session host retains instantaneous override/revocation control. |
| **Cloud & Infrastructure Integrations** | Credential leakage in logs or diagnostics; unauthorized inventory overwrite | Sensitive provider credentials (AWS secret keys, Azure secrets, DigitalOcean tokens) are redacted in summaries and encrypted in storage; non-destructive `InventoryMergeEngine` preserves existing user tags and notes during dynamic discovery; foreign keys link discovered records to inventory hosts. |
| **System Health & Integrity** | Cryptographic degradation or compromised local environment | Automated runtime health diagnostics (`DiagnosticsRunner`, CLI `doctor`, desktop Diagnostics Modal) actively verify CSPRNG entropy, AES-256-GCM AEAD round-trip encryption, OpenSSH binary discovery, and loopback socket binding; CI/CD release workflow generates CycloneDX SBOMs and SHA-256 checksums. |
| **Mobile Security** | Device theft, background socket leakage, or touch keylogging | Enforce biometric authorization (Face ID / Touch ID / BiometricPrompt) backed by hardware keystores; proactively drop TCP sockets and zeroize sensitive session state upon background expiration; render accessory bar as native canvas controls bypassing third-party software keyboards. |

For detailed analysis, see [docs/security/threat-model.md](docs/security/threat-model.md).

---

## 4. Current Security Limitations

Shellumi is currently in active foundational development:
- The local Turso database stores **host metadata only**. Do not store credentials in this database.
- An OS-backed keychain integration and native SSH cryptographic engine are currently under development.
- CLI connection dispatching relies on system OpenSSH (`ssh`) with `-o StrictHostKeyChecking=ask` and `-o ForwardAgent=no`.

---

## 5. Reporting a Vulnerability

If you discover a potential security vulnerability in Shellumi, please report it privately:
- **Email:** Contact the maintainers privately (see repository author contact).
- **Do not open public GitHub issues for undisclosed security vulnerabilities.**
- Provide detailed reproduction steps and threat impact. We will acknowledge receipt within 48 hours and work with you on a responsible disclosure timeline.

# Security Policy and Architecture

## 1. Security Philosophy

Shellumi is designed with a **security-first, privacy-first, local-first** architecture:
- **No mandatory cloud accounts:** The application operates completely offline without telemetry, tracking, or cloud dependencies.
- **Client-Side End-to-End Encryption (E2EE):** Any optional synchronization (e.g., Google Drive) sends authenticated ciphertext only. The remote storage provider never sees plaintext secrets or passphrases.
- **Explicit Host-Key Verification:** Host keys are never accepted silently. Unknown host keys require explicit confirmation (TOFU), and changed keys are flagged with prominent security warnings.
- **Zero Plaintext Secrets:** Private keys, passwords, and sensitive tokens are never stored in plain configuration files, logs, or unencrypted database tables.

---

## 2. Cryptography and Credential Management

- **AEAD Encryption:** Client-side encryption utilizes standardized AEAD ciphers (XChaCha20-Poly1305 or AES-256-GCM).
- **Key Derivation:** Vault passphrases are derived using Argon2id with cryptographically secure random salts and memory-hard parameters.
- **Credential Storage:** Sensitive credentials (passwords, private keys, certificates) are held in platform keystores (macOS Keychain, Windows Credential Manager, Secret Service API on Linux) or encrypted vaults.
- **Buffer Hygiene:** Secret memory buffers should be zeroized upon release where supported by the Rust toolchain.

---

## 3. Threat Model Overview

| Area | Threat | Mitigation Strategy |
| :--- | :--- | :--- |
| **SSH Connections** | Man-in-the-Middle (MitM) & Host-key substitution | Enforce strict host-key verification against known-hosts; prohibit silent bypass. |
| **Terminal Emulator** | Malicious escape sequences / clipboard hijack | Defense-in-depth parsing; restrict OSC commands, hyperlink clicks, and clipboard write permissions. |
| **Process Execution** | Command injection via crafted arguments | Arguments are passed as discrete vectors without shell evaluation; delimiter `--` separates options from target hostnames. |
| **Cloud Sync** | Data compromise via stolen sync files or account breach | Client-side AEAD encryption envelopes derived via Argon2id; provider receives ciphertext only. |
| **Local Storage** | Unauthorized database access | Metadata database is stored in user app data; credentials are kept in OS keychain or encrypted vaults. |

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

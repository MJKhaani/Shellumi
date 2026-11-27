[Documentation Home](../../README.md) > [Requirements](README.md) > **Hosts & Credentials**

# Hosts, Organization, Identities & Certificates Requirements

Recovered from §3.4, §3.5, and §7 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## Host & Group Management (`REQ-HOST-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-HOST-001` | Host profiles with unique ID, display name, hostname/IP, description, tags, color/icon, protocol, custom port, username, identity/credential refs, jump hosts, proxy, SSH options, env vars, startup commands, cwd, SFTP/tunnel refs, notes. | **Implemented** | `HostStore`, `InspectorShell`. |
| `REQ-HOST-002` | Single host supporting multiple protocol-specific configurations without cross-protocol feature misrepresentation. | **Implemented** | `protocol_config`, `ProtocolCapabilities`. |
| `REQ-HOST-003` | Unlimited hosts, nested groups/subgroups, tags, favorites, recent hosts, search, filter, sort, duplicate, bulk actions, move across groups/vaults. | **Implemented** (core); **Partial** (bulk edit/cross-vault move UX) | `HostStore`, `InspectorShell`. |
| `REQ-HOST-004` | Group-level inheritance (username, identity, port, protocol, proxy, jump hosts, env, startup snippets, terminal settings) with clear inherited vs. overridden display. | **Implemented** | Group inheritance engine. |
| `REQ-HOST-005` | Import/export: OpenSSH `~/.ssh/config`, `known_hosts`, SSH keys, CSV, JSON, encrypted Shellumi backup, PuTTY sessions; never export plaintext secrets silently. | **Implemented** (OpenSSH/CSV/JSON/backup); **Planned** (PuTTY) | `parse_ssh_config`, `export_ssh_config`, `parse_csv`, `export_csv`. |

## Keychain & Identity Management (`REQ-AUTH-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-AUTH-001` | Credential types: SSH private/public keys, certificates, passwords, passphrases, usernames, reusable identities, hardware key / device-bound refs. | **Implemented** (keys/certs/passwords/identities); **Planned** (hardware key refs) | `NewIdentity`, `credential.rs`. |
| `REQ-AUTH-002` | Key generation & management: Ed25519, RSA, ECDSA, FIDO2-backed SSH keys, import/export, fingerprint view, passphrase protection, rotate, `authorized_keys` deploy. | **Implemented** (Ed25519 generation/import/fingerprint); **Planned** (FIDO2, RSA/ECDSA gen, `authorized_keys` deploy) | `generate_ed25519_key`, CLI `keygen`. |
| `REQ-AUTH-003` | Reusable identities shared across multiple hosts and groups without duplicating credentials, plus host-specific and Ask-on-Connect modes. | **Implemented** | `identities` table, `save_host_with_identity_and_journal_tx()`. |
| `REQ-AUTH-004` | Security: encrypted at rest, OS credential store integration, masked fields, memory zeroization, no plaintext logs/configs, optional local-only secrets. | **Implemented** (macOS/zeroize/journal); **Partial** (Win/Linux interactive `NOT TESTED`) | `credential.rs`, `Zeroizing`, `Redacted<T>`. |

## SSH Certificates & Hardware Security (`REQ-CERT-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-CERT-001` | Import OpenSSH certificates, associate with identities, display validity/expiration, authenticate with certificates, rotation workflows. | **Partial** | `OpenSshCertificate` parsing & identity association implemented; expiry alerts partial. |
| `REQ-CERT-002` | Hardware security: FIDO2 authentication, presence confirmation, device-bound credentials, platform biometric unlock (Touch ID / Windows Hello). | **Planned** | Preserved requirement; not yet wired to OS biometric/FIDO2 APIs. |
| `REQ-CERT-003` | Passkey-style SSH access / optional self-hosted short-lived SSH CA investigation. | **Planned** | Future self-hosted CA capability. |

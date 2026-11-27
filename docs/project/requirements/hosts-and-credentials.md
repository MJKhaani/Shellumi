[Documentation Home](../../README.md) > [Requirements](README.md) > **Hosts & Credentials**

# Hosts, Organization, Identities & Certificates Requirements

Recovered from §3.4, §3.5, and §7 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## Host & Group Management (`REQ-HOST-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-HOST-001a` | Core host profile: ID, display name, hostname/IP, protocol, port, username, tags, identity/credential refs, jump host, startup snippet. | **Implemented** | Unit + Integration + GPUI tested | `HostStore`, `InspectorShell`, `actions/hosts.rs` | `INS-004` & `save_host_with_identity_and_journal_tx` tests pass. |
| `REQ-HOST-001b` | Extended host metadata: description, color/icon, custom SSH options, env vars, remote cwd, SFTP/tunnel refs, notes. | **Partial** | Source-inspected | `hosts` table & `protocol_config` in `shellumi-storage` | Basic notes/tags/protocol config exist; dedicated per-host env/cwd/color/SFTP UI fields **Planned**. |
| `REQ-HOST-002a` | Single host storing a selected protocol and its `protocol_config`. | **Implemented** | Unit tested | `ProtocolCapabilities::for_protocol`, `hosts.protocol_config` | Protocol capability & default port switching tested. |
| `REQ-HOST-002b` | Single host storing multiple simultaneous protocol-specific configurations. | **Planned** | Source-inspected | `hosts` table has 1 `protocol` + 1 `protocol_config` | Multi-protocol profile array per host remains **Planned**. |
| `REQ-HOST-003` | Host organization: groups, tags, search, filter, sort, duplicate, move across groups/vaults, bulk actions. | **Partial** | Unit + GPUI tested (core CRUD/search) | `HostStore`, `hosts.rs` | CRUD/search/filter/duplicate tested; bulk multi-host edit UI **Planned**. |
| `REQ-HOST-004a` | Group inheritance for `default_username`, `default_port`, `tags`, and `identity_id`. | **Implemented** | Unit tested | `shellumi-core::HostGroup`, `resolve_effective_tags` | `group.rs` & storage inheritance tests pass (`SYNC-003` open on delete). |
| `REQ-HOST-004b` | Group inheritance for `protocol`, `proxy`, `jump_hosts`, `environment`, `startup_snippets`, `terminal_settings`. | **Planned** | Source-inspected (`group.rs:10-16`) | `HostGroup` struct only holds username/port/tags/identity_id | Additional inheritable group fields remain **Planned**. |
| `REQ-HOST-005a` | Import/export OpenSSH `~/.ssh/config`, `known_hosts`, SSH keys, CSV, JSON, encrypted backup. | **Implemented** (SSH config/CSV/backup); **Partial** (rich preview) | Unit + CLI tested | `parse_ssh_config`, `export_ssh_config`, `parse_csv`, `export_csv` | CLI import/export tests pass; [`SYNC-004`](../known-issues.md#sync-004--full-vault-exportimport-does-not-consistently-preserve-stable-object-associations) open. |
| `REQ-HOST-005b` | PuTTY session import. | **Planned** | — | — | Not yet implemented. |

## Keychain & Identity Management (`REQ-AUTH-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-AUTH-001` | Credential types: SSH keys, certs, passwords, passphrases, usernames, reusable identities. | **Implemented** (software); **Planned** (hardware refs) | Unit + Integration tested | `NewIdentity`, `credential.rs` | Identity CRUD & `credential_operations` journal tests pass. |
| `REQ-AUTH-002` | Key generation & management: Ed25519 generation, import, fingerprint, passphrase protection; RSA/ECDSA/FIDO2 gen & `authorized_keys` deploy. | **Partial** | Unit + CLI tested (`keygen`) | `generate_ed25519_key`, CLI `keygen` | Ed25519 `keygen` tested; RSA/ECDSA/FIDO2 generation & `authorized_keys` deploy **Planned**. |
| `REQ-AUTH-003` | Reusable identities shared across hosts/groups, host-specific credentials, and Ask-on-Connect. | **Implemented** | Integration + GPUI tested | `save_host_with_identity_and_journal_tx`, `InspectorShell` | Inline & standalone identity binding tests pass. |
| `REQ-AUTH-004` | Encrypted at rest, OS credential stores, masked fields, memory zeroization, no plaintext logs. | **Implemented** (macOS/fallback); **NOT TESTED** (Win/Linux interactive) | Unit + Integration tested | `credential.rs`, `Zeroizing`, `Redacted<T>` | macOS & `EncryptedFileCredentialStore` tested; [`PLAT-001`](../known-issues.md#platform-security--operational-gaps)/[`002`](../known-issues.md#platform-security--operational-gaps) `NOT TESTED`. |

## SSH Certificates & Hardware Security (`REQ-CERT-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-CERT-001` | Import OpenSSH certificates, associate with identities, display validity/expiration, authenticate. | **Partial** | Unit tested (`OpenSshCertificate`) | `shellumi-core` | Cert model & CLI `--cert-path` exist; expiry alerts & rotation workflows **Planned**. |
| `REQ-CERT-002` | Hardware security: FIDO2 authentication, presence confirmation, biometric unlock (Touch ID / Windows Hello). | **Planned** | — | — | Preserved requirement; not yet implemented. |
| `REQ-CERT-003` | Passkey-style SSH access / optional self-hosted short-lived SSH CA. | **Planned** | — | — | Preserved requirement; not yet implemented. |

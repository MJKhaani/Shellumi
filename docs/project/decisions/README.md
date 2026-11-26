[Documentation Home](../../README.md) > [Project](../README.md) > **Architecture Decisions**

# Architecture Decision Records (ADRs)

This directory stores immutable Architecture Decision Records (ADRs) documenting significant technical choices in Shellumi.

## Foundational Decisions (Pre-ADR Baseline)

Key architectural decisions established prior to adopting numbered ADRs are documented in their respective authoritative guides:

| Decision | Summary | Reference |
| --- | --- | --- |
| **GPUI Desktop Framework** | Native Rust UI with GPUI `0.2.2`; no Electron/webviews. | [Workspace & Crates](../../architecture/workspace-and-crates.md) |
| **Isolated Per-Vault Turso DBs** | Physical DB per vault (`default.db`, `<vault-id>.db`) with independent 256-bit DEKs. | [Cryptography](../../security/cryptography.md) |
| **Argon2id + AES-256-GCM** | `argon2id_aes256gcm_v2` (`m=64 MiB, t=3, p=1`) key wrapping + Turso page encryption. | [Credential Storage](../../security/credential-storage.md) |
| **Unified Single Inspector** | Window-local `InspectorCoordinator` replacing routine CRUD modals. | [Unified Inspector](../../design/unified-inspector.md) |
| **Decoupled Mobile Strategy** | Keep `shellumi-core` UI-free for future SwiftUI/Compose mobile apps. | [Mobile Feasibility](../mobile-feasibility.md) |

## Creating a New ADR

Use `NNNN-short-kebab-title.md` (e.g., `0001-tombstone-compaction.md`) with Context, Decision, and Consequences sections, and add it to the index table above.

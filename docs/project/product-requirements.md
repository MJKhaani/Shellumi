[Documentation Home](../README.md) > [Project](README.md) > **Product Requirements**

# Shellumi Product Requirements Specification

Shellumi is a free, open-source (MIT), local-first desktop SSH client and remote infrastructure manager built in Rust with GPUI (`gpui = "=0.2.2"`).

> **Traceability & Preservation:** Every substantive requirement from the original 2,176-line master specification ([`master-project-specification.md`](../archive/implementation-plans/master-project-specification.md), baseline `3123db2`) is preserved with stable `REQ-*` identifiers under [Requirements Inventory](requirements/README.md). Unimplemented or incomplete capabilities are explicitly tracked as **Planned**, **Partial**, or **Experimental**.

## Core Architectural & Product Mandates

1. **Local-First, Zero Mandatory Accounts (`REQ-ARCH-001`):** Full local SSH, SFTP, terminal, vault, and CLI workflows operate offline without any cloud account, paid subscription, or telemetry.
2. **Rust & GPUI Exclusively (`REQ-ARCH-002`):** Core business logic and UI are implemented in Rust using GPUI `0.2.2`; Electron, Tauri, webviews, and React wrappers are prohibited.
3. **Strict Component Decoupling (`REQ-ARCH-003`):** Protocol and storage crates never depend on GPUI; `shellumi-app` never embeds raw SQL migrations or protocol internals.
4. **Security by Default (`REQ-SEC-001`):** Encrypted per-vault Turso databases (`Argon2id` + `AES-256-GCM`), TOFU host-key verification (never silently disabled), zero plaintext secrets in logs or config files, and strict input validation.
5. **Authentic State & Verification Honesty (`REQ-QA-001`):** Never fabricate protocol states, FPS/latency metrics, or simulated desktops. Never claim `NOT TESTED` Windows/Linux interactive GUI or live server acceptance as verified.
6. **Multi-Vault & Multi-Window Isolation (`REQ-SYNC-001`):** Local vaults (with optional Google Drive E2EE sync) and remote vaults (with optional self-hosted Shellumi Server E2EE sync) coexist in one instance with isolated physical databases, DEKs, credentials, and concurrent windows.

## Requirement Domains

| Domain | Identifier Prefixes | Specification |
| --- | --- | --- |
| **Requirement Migration Inventory** | `REQ-*` | [Requirements Inventory](requirements/README.md) |
| **Connections, Protocols, Tunnels & Remote Desktop** | `REQ-SSH-*`, `REQ-PROTO-*`, `REQ-RDP-*`, `REQ-VNC-*`, `REQ-TUNNEL-*` | [Connections & Protocols](requirements/connections-and-protocols.md) |
| **Terminal, SFTP, Automation, AI & Session Logs** | `REQ-TERM-*`, `REQ-SFTP-*`, `REQ-AUTO-*`, `REQ-AI-*`, `REQ-LOG-*` | [Terminal & SFTP](requirements/terminal-and-sftp.md) |
| **Hosts, Organization, Identities & Certificates** | `REQ-HOST-*`, `REQ-AUTH-*`, `REQ-CERT-*` | [Hosts & Credentials](requirements/hosts-and-credentials.md) |
| **Sync, Collaboration, Teams & Infrastructure** | `REQ-SYNC-*`, `REQ-TEAM-*`, `REQ-INFRA-*` | [Sync & Collaboration](requirements/sync-and-collaboration.md) |
| **UI/UX, Settings, Architecture, Security, Platform & QA** | `REQ-UI-*`, `REQ-SET-*`, `REQ-ARCH-*`, `REQ-SEC-*`, `REQ-PLAT-*`, `REQ-PERF-*`, `REQ-QA-*` | [UI, Platform & Quality](requirements/ui-platform-and-quality.md) |

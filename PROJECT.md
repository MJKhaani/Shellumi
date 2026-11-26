# <img src="resources/icon_pack/icons/48/regular/shellumi_logo.png" width="32" height="32" alt="Shellumi"> Shellumi — Project Definition

**Shellumi** is a free, open-source (MIT), local-first desktop SSH client and remote infrastructure manager built in Rust with GPUI (`0.2.2`). Its mission is to provide a fast, secure, self-contained alternative to proprietary clients without forced cloud accounts, telemetry, or feature paywalls.

---

## Core Engineering Objectives

1. **Pure Rust & Native GPU UI:** Built with Rust (2021 edition) and GPUI — zero Electron, webviews, or JavaScript runtime overhead.
2. **Local-First, Offline-Capable Storage:** All hosts, identities, groups, snippets, workspaces, tunnels, and session logs live in per-vault AES-256-GCM encrypted Turso databases (`vaults/*.db`).
3. **Three Clear Operating Modes:**
   - **Standalone (Local):** 100% offline local vaults with encrypted backups.
   - **Google Drive Sync:** Opt-in client-side encrypted synchronization via `drive.appdata` with authenticated revision history.
   - **Shellumi Server:** Opt-in self-hosted team synchronization, device enrollment, role-based vault sharing, and encrypted terminal collaboration.
4. **Native Remote Desktop & Terminal Engines:** Built-in VT/ANSI PTY terminal emulator alongside native Rust RDP (`IronRDP`) and VNC (`RFB`) engines.
5. **Security by Default:** Strict SSH host-key verification, TLS certificate pinning, Argon2id + AES-256-GCM vault wrapping, OS credential-store integration, and structured redacted logging.

---

## Authoritative Documentation

| Area | Document |
| :--- | :--- |
| **Product Requirements** | [`docs/project/product-requirements.md`](docs/project/product-requirements.md) |
| **Roadmap & Milestones** | [`docs/project/roadmap.md`](docs/project/roadmap.md) |
| **Known Issues** | [`docs/project/known-issues.md`](docs/project/known-issues.md) |
| **Architecture** | [`docs/architecture/README.md`](docs/architecture/README.md) |
| **Security & Threat Model** | [`docs/security/README.md`](docs/security/README.md) |
| **Feature Matrix** | [`docs/reference/feature-matrix.md`](docs/reference/feature-matrix.md) |
| **QA & Acceptance** | [`docs/qa/README.md`](docs/qa/README.md) |
| **Mobile Feasibility** | [`docs/project/mobile-feasibility.md`](docs/project/mobile-feasibility.md) |

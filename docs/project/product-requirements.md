[Documentation Home](../README.md) > [Project](README.md) > **Product Requirements**

# Product Requirements

Shellumi is a free, open-source (MIT), local-first desktop SSH client and remote infrastructure manager built in Rust with GPUI.

## Core Non-Functional Requirements

1. **Local-First, Zero Mandatory Accounts:** Full SSH, SFTP, terminal, vault, and CLI functionality must work offline without an account or telemetry.
2. **Native Performance:** Built exclusively in Rust and GPUI (`0.2.2`) without Electron, webviews, or React wrappers.
3. **Security by Default:** Encrypted vault databases (`Argon2id` + `AES-256-GCM`), TOFU host-key verification, no plaintext secrets in logs or config files, and strict input validation.
4. **Authentic Protocol & UI State:** Never fabricate connection states, FPS/latency metrics, or simulated remote desktops.
5. **Strict Component Decoupling:** `shellumi-core` and protocol crates never depend on GPUI; `shellumi-app` never embeds raw SQL/migration logic or protocol internals.

## Functional Scope

- **Connections:** SSH (password, key, agent, jump hosts), SFTP dual-pane + remote editor, local PTY shells, RDP (`shellumi-rdp`), VNC (`shellumi-vnc`), and Mosh scaffolding.
- **Organization:** Isolated local and remote vaults, host groups, reusable identities, port forwarding tunnels, snippets, workspaces, and multi-window vault binding.
- **Synchronization:** Standalone local-only mode, opt-in personal Google Drive E2EE revision sync, and opt-in self-hosted Shellumi Server E2EE sync & collaboration.
- **CLI (`shellumi-cli`):** Scriptable host management and connection dispatch sharing `shellumi-core` and `shellumi-storage`.

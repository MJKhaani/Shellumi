[Documentation Home](../../README.md) > [Requirements](README.md) > **UI, Platform & Quality**

# UI/UX, Settings, Architecture, Security, Platform & QA Requirements

Recovered from §2, §7 (security config), §8, §9, §10, §11, §12, §13, §14, and §16 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## UI/UX & Settings (`REQ-UI-*`, `REQ-SET-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-UI-001` | Original Shellumi visual language with custom icon pack, light/dark/system themes, high-contrast, responsive panels, empty/error states. | **Implemented** | `resources/icon_pack/`, `design-system.md`. |
| `REQ-UI-002` | Unified Right-Hand Inspector (`InspectorCoordinator`) for routine CRUD with guarded dirty transitions and responsive side/focused modes. | **Implemented** | `acceptance-matrix.md` (`INS-001`..`020`, `LAY-001`..`004`). |
| `REQ-UI-003` | 14 primary navigation screens (Hosts, Workspaces, Terminal, SFTP, Snippets, Keychain, Port Forwarding, Known Hosts, Session Logs, Vaults, Team Collaboration, Integrations, Google Drive Sync, Settings). | **Implemented** | `shellumi-app` main navigation. |
| `REQ-UI-004` | Global Command Palette (`PaletteRegistry`) for hosts, connections, workspaces, snippets, SFTP, tunnels, actions, screens, shortcuts. | **Implemented** | `PaletteRegistry`, CLI `palette search`. |
| `REQ-UI-005` | Accessibility: keyboard-only navigation, focus indicators, scalable UI, clear errors, screen-reader support where GPUI `0.2.2` permits. | **Partial** | Keyboard navigation & contrast implemented; GPUI `0.2.2` screen-reader APIs limited (`accessibility.md`). |
| `REQ-SET-001` | Categorized preferences (General, Appearance, Terminal, Connections, Files, Security, Synchronization, Advanced). | **Implemented** | `settings.md`. |
| `REQ-SET-002` | Optional update checks without forced automatic updates; launch-at-login & localization. | **Planned** | Not yet implemented. |

## Architecture, Security & Performance (`REQ-ARCH-*`, `REQ-SEC-*`, `REQ-PERF-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-ARCH-001`..`004` | Local-first offline operation, Rust + GPUI `0.2.2` only, strict UI/protocol/storage decoupling, MIT license, zero telemetry. | **Implemented** | 8-crate Cargo workspace (consolidated from 25 suggested crates in `3123db2`). |
| `REQ-ARCH-005` | Embedded transactional SQLite/Turso database with schema versioning, atomic migrations, referential integrity, and crash-safe recovery. | **Implemented** | `shellumi-storage`, schema 32, `MigrationEngine`. |
| `REQ-SEC-001`..`004` | `Argon2id` + `AES-256-GCM` encryption, threat model, log redaction (`Redacted<T>`), idle lock, secure temp files, `cargo audit`. | **Implemented** (crypto/redaction/model); **Partial** (idle lock/biometrics) | `docs/security/README.md`, `threat-model.md`. |
| `REQ-PERF-001` | Low memory, low input latency, fast scrolling, minimal redraws, predictable resource usage under large output. | **Partial** | GPUI virtualized rendering + `bench` suite; external comparative benchmarks pending. |

## Cross-Platform, Packaging & QA (`REQ-PLAT-*`, `REQ-QA-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-PLAT-001` | macOS (Apple Silicon & Intel): `.app`/`.dmg`, native PTY, Keychain, native menu bar, code-signing/notarization docs. | **Implemented** (Apple Silicon dev); **Partial** (DMG CI retry added, signing/notarization manual) | `build-dmg.sh`, `MacOsKeychainStore`. |
| `REQ-PLAT-002` | Windows (10/11): ConPTY, Credential Manager (`PasswordVault`), system theme, high-DPI, no stray console window (`windows_subsystem = "windows"`). | **Partial** | Compiles & passes CI unit tests; interactive Windows runtime `NOT TESTED`. |
| `REQ-PLAT-003` | Linux (X11/Wayland): `.deb` / AppImage, Secret Service (`secret-tool`), `Terminal=false` desktop entry, GNOME/KDE compatibility. | **Partial** | Compiles & `.deb` builds in CI; interactive Linux desktop `NOT TESTED`. |
| `REQ-PLAT-004` | Mobile (iOS/Android): keep `shellumi-core` UI-independent and FFI-ready without compromising desktop GPUI. | **Implemented** (core isolation & FFI bridge); **Planned** (native mobile apps) | `shellumi-core/src/mobile.rs`, `ffi.rs`, `mobile-feasibility.md`. |
| `REQ-QA-001`..`004` | Layered unit/integration/security/CI tests across macOS/Linux/Windows, honest `NOT TESTED` tracking, no mock disguises. | **Implemented** (CI); **Partial** (live protocol & Win/Linux native) | `docs/qa/verification-status.md`. |

[Documentation Home](../../README.md) > [Requirements](README.md) > **UI, Platform & Quality**

# UI/UX, Settings, Architecture, Security, Platform & QA Requirements

Recovered from §2, §7 (security config), §8, §9, §10, §11, §12, §13, §14, and §16 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## UI/UX & Settings (`REQ-UI-*`, `REQ-SET-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-UI-001` | Original Shellumi visual language, custom icon pack, light/dark/system/high-contrast themes, responsive panels. | **Implemented** | GPUI + Visual (macOS) | `resources/icon_pack/`, `design-system.md` | macOS visual baseline verified; Win/Linux native chrome `NOT TESTED`. |
| `REQ-UI-002` | Unified Right-Hand Inspector (`InspectorCoordinator`) with guarded dirty transitions and Side/FocusedContent modes. | **Implemented** | Unit + GPUI tested | `acceptance-matrix.md` (`INS-001`..`020`, `LAY-001`..`004`) | All 20 Inspector & 4 Layout automated assertions pass. |
| `REQ-UI-003a` | Primary navigation rail screens (`Hosts`, `Keychain`, `Port Forwarding`, `Snippets`, `Known Hosts`, `Session Logs`, `Settings`). | **Implemented** | GPUI + Source-inspected | `nav_items` in `main.rs:2244-2251`, `app_render.rs` | Rail items & Settings route registered and reachable. |
| `REQ-UI-003b` | Secondary workspace/tool screens (`Overview`, `Workspaces`, `SFTP Files`, `Vaults`, `Integrations`). | **Implemented** | GPUI + Source-inspected | `app_render.rs:153-175`, `desktop.rs:1285-1306` | Registered `app.page` routes & keyboard shortcuts (`GoTo*`). |
| `REQ-UI-003c` | Team Collaboration and Google Drive Sync surfaces (within Vaults/Settings/Sync views rather than separate rail items). | **Partial** | Integration tested (fixtures) | `actions/{sync,server_sync}.rs` | Accessed via Vaults/Settings; live external server/Drive `NOT TESTED`. |
| `REQ-UI-004` | Global Command Palette (`PaletteRegistry`) for hosts, connections, workspaces, snippets, SFTP, tunnels, actions, screens, shortcuts. | **Implemented** | Unit + CLI tested | `PaletteRegistry`, CLI `palette search` | Palette search & action dispatch tested. |
| `REQ-UI-005` | Accessibility: keyboard navigation, focus indicators, scalable UI, screen-reader support where GPUI `0.2.2` permits. | **Partial** | Unit + Source-inspected | `accessibility.md` | Keyboard & contrast implemented; GPUI `0.2.2` lacks full OS screen-reader tree. |
| `REQ-SET-001` | Core persisted preferences (`terminal_font_size`, `ui_font_size`, `scrollback_lines`, `show_hidden_files`, `startup_overview`). | **Implemented** | Unit tested (`Preferences`) | `shellumi-storage/src/desktop.rs:9-45` | Validation & JSON persistence (`*.preferences.json`) tested. |
| `REQ-SET-002` | Localization, launch-at-login, and optional update checks without forced updates. | **Planned** | — | — | Preserved requirements; not yet implemented. |

## Architecture, Security & Performance (`REQ-ARCH-*`, `REQ-SEC-*`, `REQ-PERF-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-ARCH-001`..`004` | Local-first offline operation, Rust + GPUI `0.2.2` only, strict UI/protocol/storage decoupling, MIT license, zero telemetry. | **Implemented** | CI verified (macOS/Linux/Windows) | 8-crate Cargo workspace (`Cargo.toml`) | Workspace structure consolidated from 25 suggested crates to 8 (**Superseded** layout). |
| `REQ-ARCH-005` | Embedded transactional SQLite/Turso database with schema versioning, atomic migrations, and crash-safe recovery. | **Implemented** | Unit + Integration tested | `shellumi-storage`, `SCHEMA_VERSION = 32`, `MigrationEngine` | Migration, reopen, future-version rejection, and crash-recovery tests pass. |
| `REQ-SEC-001`..`004` | `Argon2id` + `AES-256-GCM` encryption, threat model, log redaction (`Redacted<T>`), secure temp files, `cargo audit`. | **Implemented** (crypto/redaction); **Partial** (idle lock/biometrics) | Unit + Integration tested | `docs/security/README.md`, `threat-model.md` | Encryption, wrong-key rejection, and redaction tests pass. |
| `REQ-PERF-001` | Low memory, low input latency, fast scrolling, minimal redraws under large output. | **Partial** | Unit + CLI `bench` | `shellumi-core::BenchmarkRunner`, CLI `bench` | Internal `bench` framework exists; comparative external benchmarks **Planned**. |

## Cross-Platform, Packaging & QA (`REQ-PLAT-*`, `REQ-QA-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-PLAT-001` | macOS (`arm64`, `x86_64`): `.app`/`.dmg`, native PTY, Keychain, native menus, signing/notarization docs. | **Implemented** (dev/CI); **Partial** (signing/notarization) | CI + Release `45914814451` | `packaging/macos/build-dmg.sh`, `MacOsKeychainStore` | `.dmg` & binaries published in `v0.0.8-alpha`; Apple notarization manual. |
| `REQ-PLAT-002` | Windows (`x86_64`): ConPTY, `PasswordVault`, `windows_subsystem = "windows"`, `CREATE_NO_WINDOW`. | **Implemented** (code); **NOT TESTED** (interactive) | CI + Release `45914814451` | `main.rs`, `credential.rs`, `askpass.rs` | Compiles & `.zip`/`.exe` published; interactive Windows runtime [`PLAT-001`](../known-issues.md#platform-security--operational-gaps) `NOT TESTED`. |
| `REQ-PLAT-003` | Linux (`x86_64`): `.deb`, `secret-tool`, `Terminal=false` desktop entry, X11/Wayland. | **Implemented** (code); **NOT TESTED** (interactive) | CI + Release `45914814451` | `packaging/linux/build-deb.sh`, `shellumi.desktop` | `.deb` & binaries published; interactive Linux desktop [`PLAT-002`](../known-issues.md#platform-security--operational-gaps) `NOT TESTED`. |
| `REQ-PLAT-004` | Mobile (iOS/Android): keep `shellumi-core` UI-independent and FFI-ready without compromising desktop GPUI. | **Implemented** (core isolation); **Planned** (mobile apps) | Unit tested (`ffi.rs`, `mobile.rs`) | `shellumi-core/src/{mobile,ffi}.rs` | FFI & mobile lifecycle models tested; native SwiftUI/Compose apps **Planned**. |
| `REQ-QA-001`..`004` | Layered unit/integration/security/CI tests across macOS/Linux/Windows, honest `NOT TESTED` tracking, no mock disguises. | **Implemented** (CI); **Partial** (live protocol & Win/Linux native) | CI `45917337863` + Docs `45917337930` | `docs/qa/verification-status.md` | 3-platform CI & release packaging verified; interactive/live gaps tracked honestly. |

[Documentation Home](../README.md) > [QA](README.md) > **Verification Status**

# Verification Status

This page separates Shellumi's ten independent verification categories and records exact GitHub Actions run conclusions without conflating compilation or CI unit tests with package publication, signing/notarization, or interactive native acceptance.

## Verification Categories

1. **Source compilation** (`cargo check` / `cargo test` build)
2. **Formatting** (`cargo fmt --all -- --check`)
3. **Clippy** (`cargo clippy --workspace --all-targets -- -D warnings`)
4. **Automated workspace tests** (`cargo test --workspace`)
5. **Release binary compilation** (`cargo build --locked --release -p shellumi-app -p shellumi-cli`)
6. **Package generation** (`.dmg`, `.deb`, `.zip`)
7. **Release artifact publication** (GitHub Release upload + `SHA256SUMS.txt`, `shellumi-sbom-metadata.json`, `release-provenance.json`)
8. **Native application launch** (GUI launches without console window or fatal error)
9. **Interactive native GUI acceptance** (OS keychain/Secret Service, DPI, clipboard, menus)
10. **Live protocol acceptance** (external `sshd`, `xrdp`/Windows RDP, TigerVNC, `mosh-server`, Google Drive API, Shellumi Server)

---

## Recorded GitHub Actions Runs (As of November 27, 2026)

| Workflow | Commit / Tag | Run ID & URL | Platforms | Conclusion | Notes |
| --- | --- | --- | --- | --- | --- |
| **CI** | `3123db229050a588e75cdc60147b7e39a5d0e13c` (`main`) | [`45903344785`](https://github.com/MJKhaani/Shellumi/actions/runs/45903344785) | macOS, Linux, Windows | **success** | Categories 1–4 passed across all 3 platforms (2026-11-26). |
| **Release binaries and packages** | `3123db229050a588e75cdc60147b7e39a5d0e13c` (`v0.0.8-alpha`) | [`45911871766`](https://github.com/MJKhaani/Shellumi/actions/runs/45911871766) | Linux, Windows, macOS (`arm64`, `x86_64`) | **failure** | Historical failure in `Package macOS assets` (`x86_64`) with `hdiutil: create failed - Resource busy`; superseded by `45914814451`. |
| **CI** | `19f02c8b148f40aae30107a461f63fda2e8f7b19` (`main`) | [`45914091300`](https://github.com/MJKhaani/Shellumi/actions/runs/45914091300) | macOS, Linux, Windows | **success** | Categories 1–4 passed (2026-11-26). |
| **Verify Documentation** | `19f02c8b148f40aae30107a461f63fda2e8f7b19` (`main`) | [`45914091298`](https://github.com/MJKhaani/Shellumi/actions/runs/45914091298) | Ubuntu | **success** | Docs link/asset validation passed (2026-11-26). |
| **CI** | `7244263968ff4e24bac9bfbc6704cadbc10f4fec` (`main`) | [`45914800286`](https://github.com/MJKhaani/Shellumi/actions/runs/45914800286) | macOS, Linux, Windows | **success** | Categories 1–4 passed after `build-dmg.sh` retry fix (2026-11-27). |
| **Release binaries and packages** | `7244263968ff4e24bac9bfbc6704cadbc10f4fec` (`v0.0.8-alpha`) | [`45914814451`](https://github.com/MJKhaani/Shellumi/actions/runs/45914814451) | Linux, Windows, macOS (`arm64`, `x86_64`) | **success** | Categories 5–7 passed across all 4 targets + publish job; published [`v0.0.8-alpha`](https://github.com/MJKhaani/Shellumi/releases/tag/v0.0.8-alpha) at `2026-11-27T00:32:37Z`. |
| **CI** | `89316fda83048f15e601652da9cf541e0f2c9545` (`main`) | [`45917337863`](https://github.com/MJKhaani/Shellumi/actions/runs/45917337863) | macOS, Linux, Windows | **success** | Categories 1–4 passed on docs remediation commit (2026-11-27). |
| **Verify Documentation** | `89316fda83048f15e601652da9cf541e0f2c9545` (`main`) | [`45917337930`](https://github.com/MJKhaani/Shellumi/actions/runs/45917337930) | Ubuntu | **success** | 11 validator regression tests + 139 markdown checks passed (2026-11-27). |

**Latest Published GitHub Pre-Release:** [`v0.0.8-alpha`](https://github.com/MJKhaani/Shellumi/releases/tag/v0.0.8-alpha) (source commit `7244263968ff4e24bac9bfbc6704cadbc10f4fec`, 15 uploaded assets including `SHA256SUMS.txt`, `shellumi-sbom-metadata.json`, and `release-provenance.json`). Release publication does **not** constitute Apple notarization, Windows Authenticode signing, native GUI acceptance, or live protocol verification.

---

## What CI & Release Publication Do NOT Prove (`NOT TESTED`)

| Area | Status | Issue ID / Notes |
| --- | --- | --- |
| Windows interactive desktop launch, `PasswordVault`, askpass | `NOT TESTED` | [`PLAT-001`](../project/known-issues.md#platform-security--operational-gaps) |
| Linux interactive desktop launch (GNOME/KDE, X11/Wayland), `secret-tool` | `NOT TESTED` | [`PLAT-002`](../project/known-issues.md#platform-security--operational-gaps) |
| Live RDP server (`xrdp`, Windows RDP) | `NOT TESTED` | Deterministic/unit tests pass; [`RD-001`](../project/known-issues.md#rd-001--ambiguous-rdp-resize-timeouts-can-permit-overlapping-remote-negotiations) open |
| Live VNC server (TigerVNC, TightVNC, Apple Screen Sharing) | `NOT TESTED` | Mock RFB tests pass |
| Live Mosh server (`mosh-server`) | `NOT TESTED` | Scaffolding & CLI dispatch present |
| Live Google Drive v3 OAuth & sync | `NOT TESTED` | Fixture tests pass; [`SYNC-006`](../project/known-issues.md#sync-006--google-drive-defaults-to-the-shared-installation-identifier-desktop-app) open |
| Live Shellumi Server multi-device sync & collaboration | `NOT TESTED` | [`TEAM-001`](../project/known-issues.md#platform-security--operational-gaps), [`SYNC-001`](../project/known-issues.md#sync-001--schema-32-migration-generates-synchronization-identifiers-from-local-row-ids)..`005` |

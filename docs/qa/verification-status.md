[Documentation Home](../README.md) > [QA](README.md) > **Verification Status**

# Verification Status

This page records the exact verified state of Shellumi as of the current documentation baseline.

## Baseline

| Field | Value |
| --- | --- |
| **Baseline Commit** | `3123db229050a588e75cdc60147b7e39a5d0e13c` |
| **CI Run** | GitHub Actions Run `45903344785` |
| **Release Tag Baseline** | `v0.0.8-alpha` (Release workflow run `45911871766`) |

## Automated CI Matrix (Run `45903344785`)

| Job | Runner | Result | Duration |
| --- | --- | --- | --- |
| `Verify (macOS)` | `macos-latest` | **PASSED** | `20m27s` |
| `Verify (Linux)` | `ubuntu-latest` | **PASSED** | `13m4s` |
| `Verify (Windows)` | `windows-latest` | **PASSED** | `27m27s` |

Each CI job executes `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.

## What CI Does NOT Prove (`NOT TESTED`)

Automated CI proves compilation, formatting, zero-warning Clippy compliance, and headless unit/integration test passage across all 3 platforms. It does **not** verify:

| Area | Status | Notes |
| --- | --- | --- |
| Windows interactive desktop launch, tray/console suppression, `PasswordVault`, askpass | `NOT TESTED` | Compiles & passes unit tests; requires interactive Windows session. |
| Linux interactive desktop launch (GNOME/KDE, X11/Wayland), `secret-tool` D-Bus prompt | `NOT TESTED` | Compiles & passes unit tests; requires interactive Linux desktop session. |
| Live RDP server connection (`xrdp`, Windows RDP) | `NOT TESTED` | Deterministic/unit tests pass; live server handshake not run in CI. |
| Live VNC server connection (TigerVNC, TightVNC, Apple Screen Sharing) | `NOT TESTED` | Mock RFB tests pass; live server session not run in CI. |
| Live Mosh server connection | `NOT TESTED` | Capability scaffolding present; live `mosh-server` not run in CI. |
| Live Google Drive v3 OAuth & sync | `NOT TESTED` | Protocol/state-machine tests pass; live Drive API not exercised in CI. |
| Live Shellumi Server multi-device sync | `NOT TESTED` | Local SQLite/Turso replay tests pass; live server round-trip not run in CI. |

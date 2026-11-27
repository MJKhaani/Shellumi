# Environment Variables Reference

[Docs Home](../README.md) / [Reference](README.md) / Environment Variables

---

This reference documents every environment variable read by Shellumi's Rust crates (`crates/`), verified directly against source code. Variables from old drafts that are not read in code are intentionally excluded.

## 1. User-Facing Configuration Variables

| Variable | Purpose | Accepted Values / Default | Scope & Platforms | Sensitivity | Source |
| --- | --- | --- | --- | --- | --- |
| `SHELLUMI_VAULT_PATH` | Override default vault/database directory | Directory path; unset uses OS app data dir | Runtime (macOS, Linux, Windows) | Path | `crates/shellumi-core/src/credential.rs` |
| `SHELLUMI_CREDENTIAL_STORE` | Override OS credential store backend | Backend selector (e.g., `file`, `keychain`, `memory`); unset auto-selects OS backend | Runtime (macOS, Linux, Windows) | Config | `crates/shellumi-core/src/credential.rs` |
| `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` | Passphrase for `EncryptedFileCredentialStore` fallback | UTF-8 string; unset prompts or fails closed | Runtime (macOS, Linux, Windows) | **Secret** | `crates/shellumi-core/src/credential.rs` |
| `SHELLUMI_VAULT_PASSPHRASE` | Vault passphrase for Google Drive / Server sync operations | UTF-8 string; unset prompts in UI | Runtime (macOS, Linux, Windows) | **Secret** | `crates/shellumi-app/src/actions/{sync,server_sync}.rs` |
| `SHELLUMI_GOOGLE_CLIENT_ID` | Google OAuth 2.0 installed-app Client ID | OAuth Client ID string; unset uses built-in/configured default if present | Runtime (macOS, Linux, Windows) | Public ID | `crates/shellumi-core/src/sync.rs` |
| `SHELLUMI_GDRIVE_TOKEN` | Google Drive OAuth bearer token (primary) | OAuth access token; falls back to `GOOGLE_DRIVE_TOKEN` | Runtime (macOS, Linux, Windows) | **Secret** | `crates/shellumi-core/src/sync.rs` |
| `GOOGLE_DRIVE_TOKEN` | Fallback Google Drive OAuth bearer token | OAuth access token; lower precedence than `SHELLUMI_GDRIVE_TOKEN` | Runtime (macOS, Linux, Windows) | **Secret** | `crates/shellumi-core/src/sync.rs` |
| `SHELLUMI_DEVICE_ID` | Installation identifier for Google Drive sync metadata | Non-empty string; **defaults to `"desktop-app"` when unset** (`SYNC-006`) | Runtime (macOS, Linux, Windows) | Metadata | `crates/shellumi-app/src/actions/sync.rs` |
| `SHELLUMI_LOG` | Logging filter directive (higher precedence than `RUST_LOG`) | `tracing_subscriber` filter (e.g. `info`, `shellumi_app=debug`) | Runtime (macOS, Linux, Windows) | Non-secret | `crates/shellumi-core/src/logging.rs` |
| `RUST_LOG` | Standard Rust tracing filter fallback when `SHELLUMI_LOG` is unset | `tracing_subscriber` filter | Runtime (macOS, Linux, Windows) | Non-secret | `crates/shellumi-core/src/logging.rs` |
| `SHELLUMI_ICONS_DIR` | Override icon pack asset directory | Directory path; unset uses bundled/manifest `resources/icon_pack/icons` | Runtime (macOS, Linux, Windows) | Path | `crates/shellumi-app/src/ui/icon.rs` |

> [!WARNING]
> **Google Drive `SHELLUMI_DEVICE_ID` Limitation ([`SYNC-006`](../project/known-issues.md#sync-006--google-drive-defaults-to-the-shared-installation-identifier-desktop-app)):**
> When `SHELLUMI_DEVICE_ID` is unset, desktop Google Drive sync falls back to the shared static string `"desktop-app"`. If syncing multiple desktop machines via Google Drive, set a distinct `SHELLUMI_DEVICE_ID` per machine until persisted installation UUIDs are implemented.

---

## 2. Internal Process-Control Flags

| Variable | Purpose | Accepted Values | Source |
| --- | --- | --- | --- |
| `SHELLUMI_ASKPASS_MODE` | Launches `shellumi-app` in SSH askpass helper mode (`1`) instead of opening a GUI window | `"1"` (unset = normal GUI) | `crates/shellumi-app/src/main.rs`, `crates/shellumi-app/src/askpass.rs` |

---

## 3. Build-Time & Cargo Variables

| Variable | Purpose | Source |
| --- | --- | --- |
| `CARGO_PKG_VERSION` | Embedded version string reported by diagnostics, FFI, and mobile info (`env!("CARGO_PKG_VERSION")`) | `crates/shellumi-app/src/main.rs`, `crates/shellumi-core/src/{diagnostics,ffi}.rs` |
| `CARGO_MANIFEST_DIR` | Locates `resources/shellumi.ico` and `resources/icon_pack/icons` in dev/packaging builds | `crates/shellumi-app/build.rs`, `crates/shellumi-app/src/ui/icon.rs` |
| `OUT_DIR` | Cargo build script output directory (`build.rs`) | `crates/shellumi-app/build.rs` |
| `CARGO_BIN_EXE_shellumi-app` / `CARGO_BIN_EXE_shellumi-cli` | Integration test binary paths | `crates/shellumi-app/tests/*`, `crates/shellumi-cli/tests/*` |

---

## 4. Test-Only Variables

| Variable | Purpose | Source |
| --- | --- | --- |
| `SHELLUMI_SSH_TEST_DIR`, `SHELLUMI_SSH_TEST_PORT` | Opt-in live `sshd` acceptance fixture directory and port | `crates/shellumi-app/src/session.rs` |
| `SHELLUMI_CRASH_DIR`, `SHELLUMI_CRASH_POINT` | Simulated crash-point injection for vault rotation recovery tests | `crates/shellumi-app/src/vault_rotation.rs` |
| `SHELLUMI_RECOVERY_RACE_DIR`, `SHELLUMI_EXPECT_LOCKED` | Concurrent recovery lock-race test fixture | `crates/shellumi-app/src/vault_rotation.rs` |
| `SHELLUMI_TEST_DEVICE_JOURNAL_DB`, `SHELLUMI_TEST_DEVICE_JOURNAL_EXPECT_BUSY` | Device journal SQLite busy/recovery test fixture | `crates/shellumi-storage/src/server_sync.rs` |
| `SHELLUMI_TEST_SPLIT_REGISTRY_DIR` | Separate-database split registry test fixture | `crates/shellumi-storage/src/server_sync/separate_databases.rs` |

---

## 5. Standard OS Environment Variables

| Variable | Purpose | Platforms | Source |
| --- | --- | --- | --- |
| `HOME`, `USER`, `SHELL` | Home directory, default username, and default POSIX shell (`/bin/sh` fallback) | macOS, Linux | `data_path.rs`, `pty.rs`, `actions/terminal.rs` |
| `XDG_DATA_HOME` | Base data directory (`$XDG_DATA_HOME/shellumi`, fallback `~/.local/share/shellumi`) | Linux | `data_path.rs`, `credential.rs` |
| `APPDATA`, `USERPROFILE`, `COMSPEC`, `SystemRoot`, `windir`, `ProgramFiles` | Windows data directory (`%APPDATA%\Shellumi`), ConPTY shell (`cmd.exe` fallback), and OpenSSH lookup | Windows | `data_path.rs`, `pty.rs`, `credential.rs` |

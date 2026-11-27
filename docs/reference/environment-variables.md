# Environment Variables Reference

[Docs Home](../README.md) / [Reference](README.md) / Environment Variables

---

This reference documents environment variables read or constructed by Shellumi's Rust crates (`crates/`), verified against the current source code. Dynamic environment-variable keys (if any external tool reads them) cannot be exhaustively enumerated by static `env::var` greps alone.

## 1. User-Facing Configuration Variables

| Variable | Purpose | Recognized Values / Unset Behavior | Scope & Platforms | Sensitivity | Source |
| --- | --- | --- | --- | --- | --- |
| `SHELLUMI_VAULT_PATH` | Override the **fallback encrypted credential file path** (not the app data dir or SQLite vault DB directory) | Absolute file path (e.g. `/path/to/credentials.enc`); unset defaults to `<base>/Shellumi/credentials.enc` (`%APPDATA%` on Windows, `$HOME/Library/Application Support` on macOS, absolute `$XDG_DATA_HOME` else `$HOME/.local/share` on Linux) | Runtime (macOS, Linux, Windows) | Path | `crates/shellumi-core/src/credential.rs` (`default_fallback_vault_path`) |
| `SHELLUMI_CREDENTIAL_STORE` | Override OS credential store backend | Only `"ephemeral"` is explicitly recognized, returning in-memory `EphemeralCredentialStore` (**not durable across restarts**). Unset or any other value selects the native OS backend (`MacOsKeychainStore`, `WindowsCredentialManagerStore`, `LinuxSecretServiceStore`, or `EphemeralCredentialStore` on unsupported OSes) and passes `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` to `configured_credential_store()`. | Runtime (macOS, Linux, Windows) | Config | `crates/shellumi-core/src/credential.rs` (`default_credential_store`) |
| `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` | Enable `EncryptedFileCredentialStore` fallback paired with the native OS backend (`DaemonWithFallbackStore`) | UTF-8 string of **at least 12 characters** paired with an **absolute** `SHELLUMI_VAULT_PATH` (or default path). Unset: uses native OS backend only (no file fallback, no headless prompt). Invalid (`< 12` chars or relative path): returns `UnavailableCredentialStore` (fails closed). | Runtime (macOS, Linux, Windows) | **Secret** | `crates/shellumi-core/src/credential.rs` (`configured_credential_store`) |
| `SHELLUMI_VAULT_PASSPHRASE` | Vault passphrase for Google Drive / Server sync operations | UTF-8 string; unset prompts in UI | Runtime (macOS, Linux, Windows) | **Secret** | `crates/shellumi-app/src/actions/{sync,server_sync}.rs` |
| `SHELLUMI_GOOGLE_CLIENT_ID` | Google OAuth 2.0 installed-app Client ID | OAuth Client ID string; unset uses configured default if present | Runtime (macOS, Linux, Windows) | Public ID | `crates/shellumi-core/src/sync.rs` |
| `SHELLUMI_GDRIVE_TOKEN` | Google Drive OAuth bearer token (primary) | OAuth access token; takes precedence over `GOOGLE_DRIVE_TOKEN` | Runtime (macOS, Linux, Windows) | **Secret** | `crates/shellumi-core/src/sync.rs` |
| `GOOGLE_DRIVE_TOKEN` | Fallback Google Drive OAuth bearer token | OAuth access token; lower precedence than `SHELLUMI_GDRIVE_TOKEN` | Runtime (macOS, Linux, Windows) | **Secret** | `crates/shellumi-core/src/sync.rs` |
| `SHELLUMI_DEVICE_ID` | Installation identifier for Google Drive sync metadata | Non-empty string; **defaults to `"desktop-app"` when unset** (`SYNC-006`) | Runtime (macOS, Linux, Windows) | Metadata | `crates/shellumi-app/src/actions/sync.rs` |
| `SHELLUMI_LOG` | Logging filter directive (higher precedence than `RUST_LOG`) | `tracing_subscriber` filter (e.g. `info`, `shellumi_app=debug`) | Runtime (macOS, Linux, Windows) | Non-secret | `crates/shellumi-core/src/logging.rs` |
| `RUST_LOG` | Standard Rust tracing filter fallback when `SHELLUMI_LOG` is unset | `tracing_subscriber` filter | Runtime (macOS, Linux, Windows) | Non-secret | `crates/shellumi-core/src/logging.rs` |
| `SHELLUMI_ICONS_DIR` | Override icon pack asset directory | Directory path; unset uses bundled/manifest `resources/icon_pack/icons` | Runtime (macOS, Linux, Windows) | Path | `crates/shellumi-app/src/ui/icon.rs` |

> [!WARNING]
> **Google Drive `SHELLUMI_DEVICE_ID` Limitation ([`SYNC-006`](../project/known-issues.md#sync-006--google-drive-defaults-to-the-shared-installation-identifier-desktop-app)):**
> When `SHELLUMI_DEVICE_ID` is unset, desktop Google Drive sync falls back to the shared static string `"desktop-app"`. Set a distinct `SHELLUMI_DEVICE_ID` per desktop machine until persisted installation UUIDs are implemented.

---

## 2. Internal Askpass & Subprocess Control Variables

These variables are **internal process-control signals** passed between `shellumi-app` and its OpenSSH askpass helper (`crates/shellumi-app/src/askpass.rs`) or child processes. Setting `SHELLUMI_ASKPASS_MODE=1` manually is **not** a normal application launch procedure.

| Variable | Direction | Purpose | Sensitivity | Source |
| --- | --- | --- | --- | --- |
| `SHELLUMI_ASKPASS_MODE` | Parent → `shellumi-app` helper | `"1"` launches `shellumi-app` in non-GUI OpenSSH askpass helper mode | Internal flag | `crates/shellumi-app/src/main.rs`, `askpass.rs` |
| `SHELLUMI_ASKPASS_TARGET` | Parent → helper | Target `"<username>@<host>"` validated against the OpenSSH prompt | Metadata | `crates/shellumi-app/src/askpass.rs` |
| `SHELLUMI_ASKPASS_KEY` | Parent → helper | Identity key file path (`plan.identity_file`) for prompt matching | Path / metadata | `crates/shellumi-app/src/askpass.rs` |
| `SHELLUMI_ASKPASS_PASSWORD_REF` | Parent → helper | Vault-scoped credential reference URI (`plan.password_ref`) | **Sensitive ref** | `crates/shellumi-app/src/askpass.rs` |
| `SHELLUMI_ASKPASS_PASSPHRASE_REF` | Parent → helper | Vault-scoped passphrase reference URI (`plan.passphrase_ref`) | **Sensitive ref** | `crates/shellumi-app/src/askpass.rs` |
| `SSH_ASKPASS`, `SSH_ASKPASS_REQUIRE`, `DISPLAY`, `SSH_ASKPASS_PROMPT` | Parent → OpenSSH → helper | Standard OpenSSH askpass plumbing (`SSH_ASKPASS_REQUIRE=force`, `DISPLAY=shellumi:0`; `SSH_ASKPASS_PROMPT` cleared by parent, populated by OpenSSH) | Subprocess control | `crates/shellumi-app/src/askpass.rs` |
| `TERM`, `COLORTERM` | Parent → PTY child | Set to `xterm-256color` and `truecolor` on spawned local PTY shells | Terminal env | `crates/shellumi-terminal/src/pty.rs` |

---

## 3. Build-Time & Cargo Variables

| Variable | Purpose | Source |
| --- | --- | --- |
| `CARGO_PKG_VERSION` | Embedded version string (`env!("CARGO_PKG_VERSION")`) | `crates/shellumi-app/src/main.rs`, `crates/shellumi-core/src/{diagnostics,ffi}.rs` |
| `CARGO_MANIFEST_DIR` | Locates `resources/shellumi.ico` and `resources/icon_pack/icons` in dev/packaging builds | `crates/shellumi-app/build.rs`, `crates/shellumi-app/src/ui/icon.rs` |
| `OUT_DIR` | Cargo build script output directory | `crates/shellumi-app/build.rs` |
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
| `SHELLUMI_TEST_SECRET` | Synthetic test secret in askpass integration test | `crates/shellumi-app/tests/askpass.rs` |
| `SHELLUMI_VERSION` | Packaging test / release workflow version string | `crates/shellumi-cli/tests/packaging.rs` |

---

## 5. Standard OS Environment Variables

| Variable | Purpose | Platforms | Source |
| --- | --- | --- | --- |
| `HOME`, `USER`, `SHELL` | Home directory, default username, and default POSIX shell (`/bin/sh` fallback) | macOS, Linux | `data_path.rs`, `pty.rs`, `actions/terminal.rs` |
| `XDG_DATA_HOME` | Base data directory (`$XDG_DATA_HOME/shellumi` or `$XDG_DATA_HOME/Shellumi/credentials.enc` when absolute; fallback `~/.local/share`) | Linux | `data_path.rs`, `credential.rs` |
| `APPDATA`, `USERPROFILE`, `COMSPEC`, `SystemRoot`, `windir`, `ProgramFiles` | Windows data/credential base (`%APPDATA%\Shellumi`), ConPTY shell (`cmd.exe` fallback), and OpenSSH lookup | Windows | `data_path.rs`, `pty.rs`, `credential.rs` |

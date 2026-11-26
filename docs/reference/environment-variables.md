# Environment Variables Reference

[Docs Home](../README.md) / [Reference](README.md) / Environment Variables

---

All environment variables read by Shellumi (`std::env::var`), ordered by category.

| Variable | Purpose | Type / Values | Default | Required | Sensitivity |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`SHELLUMI_LOG`** | Primary `tracing` log filter (`error`, `info`, `debug`, `trace`, crate filters) | `EnvFilter` string | `error` | Optional | Non-secret |
| **`RUST_LOG`** | Fallback `tracing` log filter when `SHELLUMI_LOG` is unset | `EnvFilter` string | `error` | Optional | Non-secret |
| **`SHELLUMI_CREDENTIAL_STORE`** | Override credential backend (`encrypted-file`) | String | Native OS store | Optional | Non-secret |
| **`SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE`** | Passphrase for `EncryptedFileCredentialStore` (>= 12 chars) | String | None | Only for `encrypted-file` | **Secret** |
| **`SHELLUMI_VAULT_PATH`** | Override encrypted credential file path | Absolute path | OS data dir | Optional | Non-secret |
| **`SHELLUMI_VAULT_PASSPHRASE`** | Vault passphrase for background Drive/Server sync | String | None | Only for headless sync | **Secret** |
| **`SHELLUMI_GOOGLE_CLIENT_ID`** | OAuth 2.0 Client ID for Google Drive PKCE flow | String | Built-in/none | Only for OAuth setup | Low |
| **`SHELLUMI_GDRIVE_TOKEN`** / **`GOOGLE_DRIVE_TOKEN`** | OAuth bearer token for Google Drive sync | Token string | None | Only for CLI/env Drive sync | **Secret** |
| **`SHELLUMI_DEVICE_ID`** | Override device ID for Google Drive sync | String | `"desktop-app"` | Optional | Non-secret |
| **`SHELLUMI_ICONS_DIR`** | Override `resources/icon_pack` root directory | Path | Embedded/bundled | Optional | Non-secret |
| **`SHELLUMI_ASKPASS_MODE`** | Internal flag (`1`) launching `shellumi-app` as OpenSSH askpass helper | `"1"` | Unset | Internal | Non-secret |
| **`SHELLUMI_SSH_TEST_DIR`**, **`SHELLUMI_SSH_TEST_PORT`** | Fixture directory & port for opt-in OpenSSH integration tests | Path / `u16` | Unset | Test-only | Non-secret |

**Precedence:** `SHELLUMI_LOG` overrides `RUST_LOG`; `SHELLUMI_GDRIVE_TOKEN` overrides `GOOGLE_DRIVE_TOKEN`; `SHELLUMI_CREDENTIAL_STORE=encrypted-file` overrides native OS keychain routing.

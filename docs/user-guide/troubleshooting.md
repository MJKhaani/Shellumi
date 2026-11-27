# <img src="../../resources/icon_pack/icons/48/regular/diagnostics.png" width="24" height="24" alt="Troubleshooting"> Troubleshooting & Diagnostics

[Docs Home](../README.md) / [User Guide](README.md) / Troubleshooting

---

## 1. Run `shellumi-cli doctor` First

```bash
shellumi-cli doctor --markdown
shellumi-cli doctor --json --db ~/vault.db
```

`doctor` inspects OS credential-store availability, vault database accessibility, and environment configuration without exposing secrets.

---

## 2. Symptom-Driven Troubleshooting

| Area | Symptom | Diagnosis & Safe Resolution |
| --- | --- | --- |
| **Startup / Vault Unlock** | Vault fails to unlock or manifest error | Verify you are using the correct vault passphrase or Recovery Code. Never delete vault `.db` or `.manifest.json` files without an external backup. |
| **Native Credential Store** | `secret-tool` / D-Bus keyring unavailable on Linux, or headless environment | Install `libsecret-tools` and unlock your desktop keyring, **or** set `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` (>= 12 chars) alongside an absolute `SHELLUMI_VAULT_PATH` (default `~/.local/share/Shellumi/credentials.enc`) to enable `EncryptedFileCredentialStore` fallback. (`SHELLUMI_CREDENTIAL_STORE=ephemeral` is in-memory only and non-durable.) |
| **SSH Host Key** | `Host key mismatch` | Server key changed or possible MitM. **Never disable host-key verification.** Verify the server's SHA-256 fingerprint out-of-band before updating `known_hosts`. |
| **SSH Askpass** | Auth helper fails or prompts fail | Verify the host's identity/credential exists in the current vault and run `shellumi-cli keychain status --json`. Never launch `SHELLUMI_ASKPASS_MODE=1` manually. |
| **Mosh** | Mosh connection fails immediately | Ensure `mosh-client` is installed locally, `mosh-server` is installed on the remote host, and UDP `60000–61000` is reachable. |
| **SFTP** | `sftp-server` subsystem error | Confirm `Subsystem sftp` is enabled in remote `/etc/ssh/sshd_config`, then test with `shellumi-cli ~/vault.db sftp remote-ls user@host /tmp`. |
| **Tunnels** | `Address already in use` or refused forward | Choose a free local bind port and verify `AllowTcpForwarding yes` on the SSH server. |
| **Remote Desktop** | Black screen, NLA failure, or resize timeout | See [Remote Desktop Troubleshooting](remote-desktop.md#4-troubleshooting) and [`RD-001`](../project/known-issues.md#rd-001--ambiguous-rdp-resize-timeouts-can-permit-overlapping-remote-negotiations). |
| **Google Drive Sync** | Device collision across desktops (`SYNC-006`) or token errors | Set a distinct `SHELLUMI_DEVICE_ID` per desktop machine; verify `SHELLUMI_GDRIVE_TOKEN` / `SHELLUMI_VAULT_PASSPHRASE`. |
| **Shellumi Server** | Sync mismatch or unresolved identity (`SYNC-001`..`005`) | Check [Known Issues](../project/known-issues.md); assign a valid local credential if a host is marked `auth_dependency_unresolved`. |

---

## 3. Collecting Safe Logs for Issue Reports

Launch Shellumi with `SHELLUMI_LOG=debug` (higher precedence than `RUST_LOG`). All sensitive fields use `Redacted<T>` / `redact_sensitive` ([Logging Guidelines](../development/logging.md)), but always review logs before attaching them to a public GitHub issue:

- Include: Shellumi version (`v0.0.8-alpha`), OS & architecture, `shellumi-cli doctor --markdown` output (with personal paths/hostnames redacted), and reproduction steps.

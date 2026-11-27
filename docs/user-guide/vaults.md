# <img src="../../resources/icon_pack/icons/48/regular/vaults.png" width="24" height="24" alt="Vaults"> Vaults & Multi-Window Isolation

[Docs Home](../README.md) / [User Guide](README.md) / Vaults

---

Every host, group, identity, snippet, tunnel, workspace, and session recording in Shellumi lives inside an isolated vault backed by its own physical encrypted Turso database (`default.db` or `<vault-id>.db`) and independent 256-bit Data Encryption Key (DEK).

## Local vs. Remote Vaults

- **Default Local Vault (`legacy-local`):** Created on first launch; stores your primary local inventory plus the encrypted **Discovery Directory** (`SyncVault.encrypted_directory`) listing other vaults' metadata (never their secrets or contents).
- **Additional Local Vaults:** Separate offline databases (e.g., `homelab`, `client-a`) with optional opt-in Google Drive personal E2EE sync.
- **Remote Vaults:** Bound to a specific self-hosted **Shellumi Server** deployment (`server_instance_id`, `server_vault_id`), keeping an independently encrypted local offline cache.

## Multi-Window Vault Binding

- Open any vault in an independent window using `Cmd+Shift+N` (macOS), the **Vault Switcher** dropdown (`🗗`), or the **Command Palette** (`vault:open-new-window`).
- Each window displays its active vault badge (and server chip for remote vaults) in the top bar.
- Switching vaults or closing one window never alters or terminates another window's sessions.
- Cross-vault copying/moving requires explicit user confirmation so company credentials never mix silently.

## Backup, Export, Rollback & CLI

```bash
shellumi-cli ~/vault.db vault status
shellumi-cli ~/vault.db vault export backup.shlv --passphrase "<pass>"
shellumi-cli ~/vault.db vault history
shellumi-cli ~/vault.db vault rollback <snapshot-id> --passphrase "<pass>"
```

See [`SYNC-004`](../project/known-issues.md#sync-004--full-vault-exportimport-does-not-consistently-preserve-stable-object-associations) regarding full-vault export/import associations and [`SEC-001`](../project/known-issues.md#platform-security--operational-gaps) regarding CLI `--passphrase` visibility.

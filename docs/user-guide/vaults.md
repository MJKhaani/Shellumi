# <img src="../../resources/icon_pack/icons/48/regular/vaults.png" width="24" height="24" alt="Vaults"> Vaults

[Docs Home](../README.md) / [User Guide](README.md) / Vaults

---

Each Shellumi vault is stored in its own physically separate, AES-256-GCM encrypted SQLite database (`vaults/default.db` for the default local vault, `vaults/<vault-id>.db` for secondary and remote vaults) with its own 256-bit Data Encryption Key (DEK).

## Key Operations

- **Create Secondary Vault:** Open **Vaults** from the sidebar or Vault Switcher, choose a name and master password, and save the generated recovery code.
- **Switch or Open in New Window:** Switch vaults in the current window or open any vault in an independent concurrent window (`Cmd+Shift+N` / `Ctrl+Shift+N`). Each window isolates its own hosts, identities, sync cursors, and WebSocket subscriptions.
- **Lock & Unlock:** Unlocking a vault requires your password, recovery code, or OS Keychain-stored DEK. Unlocking one vault never unlocks another.
- **Password Rotation:** Change a vault's password in the Vault Inspector (entering current password and new password). Rotation preserves the DEK and re-wraps the manifest atomically via a crash-safe journal.

See [Vault Architecture](../architecture/vault-architecture.md) and [Vault Isolation](../security/vault-isolation.md).

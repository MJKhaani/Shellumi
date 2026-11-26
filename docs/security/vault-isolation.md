# Vault Isolation

[Docs Home](../README.md) / [Security](README.md) / Vault Isolation

---

- **Separate Physical Databases:** `vaults/default.db` (default vault `legacy-local` + central registry) and `vaults/<vault-id>.db` (each secondary/remote vault) are encrypted with independent random 256-bit DEKs.
- **No Cross-Vault Key Leakage:** Unlocking one vault never unlocks another; opening a vault database without its DEK fails closed before any query executes.
- **Multi-Window Isolation:** Concurrent windows bound to different vaults isolate in-memory hosts, identities, `InspectorCoordinator` drafts, `ConnectionOpRegistry` operations, and WebSocket subscriptions.
- **Sync Boundary:** The default vault's Drive sync exports only encrypted registry discovery metadata (`SyncVault.encrypted_directory`), never secondary vault DEKs or credentials.

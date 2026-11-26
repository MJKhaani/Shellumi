# Standalone Mode (Offline Local)

[Docs Home](../README.md) / [Synchronization](README.md) / Standalone Mode

---

In **Standalone Mode**, Shellumi operates 100% locally with zero cloud accounts, telemetry, or external network connections:

```mermaid
flowchart LR
    App["Shellumi Desktop / CLI"] --> Manifest["vault_manifests/{vault_id}.manifest.json"]
    App --> Keychain["OS Credential Store (optional DEK + host secrets)"]
    App --> DB["vaults/default.db & vaults/{vault_id}.db (AES-256-GCM)"]
```

- **No Required External Configuration:** Only a master password (and optional OS Keychain storage) is needed.
- **Multi-Vault Support:** Create any number of isolated local secondary vaults (`vaults/<vault-id>.db`), each with its own DEK and recovery code.
- **Backups:** Export encrypted local snapshots at any time via Settings or `shellumi-cli vault export`.

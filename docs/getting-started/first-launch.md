[Documentation Home](../README.md) > [Getting Started](README.md) > **First Launch**

# First Launch & Setup Wizard

When you launch Shellumi for the first time, the **Setup Wizard** guides you through initializing your local-first vault without requiring any cloud account or internet connection.

## 1. Choose Your Operating Mode

1. **Standalone Local Vault (Recommended to Start):** Stores your encrypted vault database locally (`Shellumi/vaults/default.db`) with no external sync.
2. **Google Drive Personal Sync:** Opt-in end-to-end encrypted synchronization using your own Google Drive account. See [Google Drive Sync](../synchronization/google-drive.md).
3. **Self-Hosted Shellumi Server:** Connect to a self-hosted Shellumi Server for multi-device sync and team collaboration. See [Shellumi Server](../synchronization/shellumi-server.md).

## 2. Create a Vault Password & Recovery Code

- Shellumi generates an independent random 256-bit Data Encryption Key (DEK) for your vault and wraps it using **Argon2id** (`m=64 MiB, t=3, p=1`) + **AES-256-GCM**.
- Save your **Recovery Code** in a secure offline location. Because Shellumi is zero-knowledge, lost passwords cannot be reset by any server or support team.

## 3. Optional OS Keychain Integration

You can optionally store vault unlock material or server credentials in your operating system's native secret store:
- **macOS:** Keychain (`MacOsKeychainStore`)
- **Windows:** Credential Manager (`PasswordVault` — `NOT TESTED` interactively)
- **Linux:** Secret Service (`secret-tool` — `NOT TESTED` interactively)

## Next Steps

- Follow the [Quick Start Guide](quick-start.md) to add your first SSH host and open a terminal.

# <img src="../../resources/icon_pack/icons/48/regular/keychain.png" width="24" height="24" alt="Keychain"> Identities & Keychain

[Docs Home](../README.md) / [User Guide](README.md) / Identities & Keychain

---

Shellumi separates **reusable identities** from **host-specific credentials** and **Ask on Connect** prompts, storing secrets in your operating system's credential store or an encrypted fallback vault—never in plaintext config files or logs.

## Credential Modes & Semantics

1. **Reusable Identity (Independent or Inline):**
   - Create independently in the **Keychain** view (**New Identity**), or inline while adding/editing a host (**Enter Credentials → Save as Identity**).
   - Reference the same identity across multiple hosts and groups without duplicating secrets.
2. **Save for This Host (Host-Specific):**
   - Stores a vault-scoped credential reference (`keychain://...`) bound to that single host without cluttering the reusable identity list.
3. **Ask on Connect / Session Only:**
   - Prompts interactively when connecting and keeps no secret persisted on disk.
4. **Group Inheritance:**
   - Hosts with no explicit identity or host-specific credential can inherit their parent group's identity.

---

## Supported Authentication & Key Management

- **Password Authentication:** Username + password stored in OS credential store (`Redacted<T>` in memory/logs).
- **Private Keys & Passphrases:** OpenSSH-compatible Ed25519, ECDSA, and RSA-SHA2 keys, with optional encrypted key passphrase.
- **SSH Certificates:** Associate OpenSSH certificates (`--cert-path`) with an identity (`OpenSshCertificate`).
- **SSH Agent Integration:** Uses Unix `ssh-agent` (`SSH_AUTH_SOCK`) or compatible Windows SSH agents. Agent forwarding is **never** enabled silently and displays a security warning.
- **Key Generation:** Generate Ed25519 keypairs via the Keychain view or `shellumi-cli keygen [--comment <comment>]`.

### Editing Existing Credentials: Keep, Replace, and Remove

When editing a host or identity that already has a stored secret:
- **Keep:** Retains the existing credential reference unchanged (masked in the UI).
- **Replace:** Stages a new secret via the crash-safe `credential_operations` journal and cleans up the previous reference after commit.
- **Remove:** Clears the credential reference and schedules journaled deletion of the orphaned OS secret.
- **Visibility Toggle:** Use the masked-field eye toggle in the Inspector to temporarily reveal a typed password/passphrase locally before saving.

---

## Hardware Security: Available vs. Planned

- **Available Now:** Software Ed25519/ECDSA/RSA keys, OpenSSH certificates, OS SSH agent forwarding (explicit opt-in), and OS credential stores.
- **Planned (`REQ-CERT-002`, `REQ-CERT-003`):** Hardware FIDO2/U2F security-key presence confirmation, device-bound passkey SSH credentials, and platform biometric unlock (Touch ID / Windows Hello) are preserved requirements and **not yet implemented**.

---

## Cross-Device Availability & Native Credential Stores

- **Important:** `keychain://` credential references point to secrets stored in the **local machine's** OS credential store:
  - **macOS:** Keychain (`MacOsKeychainStore`) — verified on macOS.
  - **Windows:** Credential Manager (`PasswordVault`) — `NOT TESTED` interactively (`PLAT-001`).
  - **Linux:** Secret Service (`secret-tool` / D-Bus) — `NOT TESTED` interactively (`PLAT-002`).
  - **Encrypted File Fallback (`DaemonWithFallbackStore`):** When `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` (at least 12 characters) is set alongside an absolute `SHELLUMI_VAULT_PATH` (default `<base>/Shellumi/credentials.enc`), Shellumi pairs the native backend with `EncryptedFileCredentialStore`. (Note: `SHELLUMI_CREDENTIAL_STORE=ephemeral` selects a non-durable in-memory store.)
- Incremental server sync strips device-local secret refs (`host_secret_ref`, `host_passphrase_ref`, `host_key_path`) so secrets never leak unencrypted across devices. On a new device, re-bind local credentials or use a host-assigned secret stored in your encrypted vault where configured. See [`SYNC-003`](../project/known-issues.md#sync-003--identity-deletion-can-unintentionally-change-effective-authentication) and [`SYNC-005`](../project/known-issues.md#sync-005--incoming-synchronization-bypasses-journaled-cleanup-of-incompatible-credentials).

## Troubleshooting

- **Linux `secret-tool` Missing or No D-Bus Keyring:** Install `libsecret-tools` (`secret-tool`) and ensure GNOME Keyring or KWallet Secret Service is unlocked, or set `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` (>= 12 chars) to enable the `EncryptedFileCredentialStore` fallback at `SHELLUMI_VAULT_PATH` (default `~/.local/share/Shellumi/credentials.enc`).
- **CLI Status Check:** Run `shellumi-cli keychain status --json` or `shellumi-cli doctor` to inspect active credential-store availability.

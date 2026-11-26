# <img src="../../resources/icon_pack/icons/48/regular/identity_card.png" width="24" height="24" alt="Identities"> Identities and Keychain

[Docs Home](../README.md) / [User Guide](README.md) / Identities and Keychain

---

## Managing Reusable Identities

1. Open **Keychain** (`Cmd+Shift+Y` / `Ctrl+Shift+Y`) or create an identity inline while adding a host (**Enter Credentials → Save as Identity**).
2. Supported authentication methods:
   - **Password:** Stored in your OS credential store under a vault-scoped `keychain://` reference.
   - **Public Key / Certificate:** References a local private key path (and optional OpenSSH certificate `.pub`) plus an optional encrypted-key passphrase stored in the OS credential store.
   - **SSH Agent:** Delegates signing to your running `ssh-agent`.

## Keep, Replace, and Remove Credential Semantics

When editing an existing host or identity:
- Leaving a masked password/passphrase field blank **keeps** the existing stored secret untouched.
- Entering a new value **replaces** the secret atomically via the crash-safe credential journal.
- Clicking **Clear / Remove** deletes the stored secret reference and cleans up the orphaned credential once committed.

## Known Hosts & Local Keys

- **Keychain page:** Displays active OS credential store status, detected `~/.ssh/*.pub` keys, fingerprint inspection, `ssh-keygen`, and `ssh-add`.
- **Known Hosts page:** Inspects OpenSSH `known_hosts` entries and lets you explicitly revoke untrusted or stale host keys.

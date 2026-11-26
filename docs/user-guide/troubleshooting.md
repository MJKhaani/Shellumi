# <img src="../../resources/icon_pack/icons/48/regular/diagnostics.png" width="24" height="24" alt="Troubleshooting"> Troubleshooting

[Docs Home](../README.md) / [User Guide](README.md) / Troubleshooting

---

## Run Built-In Diagnostics

Run the diagnostics suite from the Command Palette (**Run Diagnostics**) or CLI:

```sh
shellumi-cli doctor --markdown
```

This checks CSPRNG entropy, AES-256-GCM AEAD round-trip encryption, OpenSSH binary availability, and loopback socket binding.

## Common Issues

- **Unresolved Synchronized Identity:** If a host shows *"Unresolved synchronized identity"*, sync the vault so the referenced identity arrives, or open the Host Inspector and assign a local identity. Shellumi intentionally blocks connection rather than guessing credentials.
- **Device-Local Credentials Required:** Synchronized hosts using host-private passwords or key paths do not copy secret references across devices. Open the Host Inspector on the new device and configure local credentials.
- **OS Keychain Unavailable:** On headless Linux or environments without `secret-tool` / D-Bus, configure an explicit encrypted credential store with `SHELLUMI_CREDENTIAL_STORE=encrypted-file` and `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` (at least 12 characters). See [Environment Variables](../reference/environment-variables.md).
- **Enable Verbose Logging:** Launch with `SHELLUMI_LOG=info` or `SHELLUMI_LOG=debug` (see [Logging](../development/logging.md)). Logs automatically redact secrets.

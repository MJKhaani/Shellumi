# Known Security Limitations

[Docs Home](../README.md) / [Security](README.md) / Known Limitations

---

1. **System OpenSSH Dependency:** SSH, SFTP, Mosh, and SSH Tunnels invoke external `ssh`/`mosh` binaries rather than an embedded Rust SSH implementation.
2. **CLI Passphrase Arguments:** `shellumi-cli` vault commands accept `--passphrase <pass>`, which can be visible in local process listings; interactive hidden TTY prompting is not yet wired for CLI vault subcommands.
3. **Native Windows & Linux Runtime Acceptance:** `WindowsCredentialManagerStore` and `LinuxSecretServiceStore` are unit-tested for script/argument construction and compile in CI, but interactive runtime acceptance on native Windows and Linux desktops remains `NOT TESTED`.
4. **Team Epoch Encryption & Server Revocation Atomicity:** Team vault epoch-key rotation in the desktop Team Inspector remains gated until end-to-end live Shellumi Server transactional key-envelope + epoch-update verification is completed (see [Archived Remediation Status](../archive/security-audits/remediation-status-v0.0.8-alpha.md)).
5. **Sync Object Identifier & Cleanup Gaps:** See [Object Identifiers](../synchronization/object-identifiers.md) for the six documented identifier/cleanup issues (Schema-32 backfill provenance, name-fallback host matching, identity deletion fallback, full-vault export `sync_id`, incoming credential journal cleanup, and Drive `"desktop-app"` fallback ID).
6. **Session Recordings:** Automated redaction in `.cast` recordings is best-effort and does not guarantee zero sensitive output capture.

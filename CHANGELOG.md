# Changelog

## Unreleased

- Add self-contained macOS OpenSSH/SFTP acceptance covering saved group identity
  resolution, encrypted-key askpass, listing, binary/empty transfers, reconnect,
  wrong-passphrase and changed/unknown server-key rejection.
- Require a matching successful final SFTP CLOSE response before reporting upload,
  download or directory-listing success; propagate close errors and disconnects.
- Add optional explicit OpenSSH client configuration for isolated SFTP acceptance.

- Add host/group GUI identity selection with preserved bindings and refresh.
- Apply saved identities to SSH, graphical/terminal SFTP, workspace restores and
  tunnels using a shared background resolver and validated launch policy.
- Support password/key-passphrase references through a separate OpenSSH askpass
  helper; require existing host trust, reject MFA/trust prompts, and keep secrets
  out of command arguments. Key/certificate paths support spaces and quotes.

- Reconcile progress documents with the actual SFTP, identity, keychain and Drive
  integration status; record missing live acceptance.
- Remove shared credential-fallback and Drive vault encryption passphrases;
  require explicit keys and standard Drive KDF settings.
- Disable SFTP agent forwarding and require previously trusted host keys.
- Pipe HTTP headers and UTF-8 bodies to curl through stdin rather than argv;
  reject malformed header input and ignore user curl configuration.
- Regression coverage: 301 workspace tests passed on macOS, two opt-in tests ignored;
  the self-contained real-server SSH/SFTP acceptance test passed separately.

## 0.0.1-alpha

First alpha milestone for Shellumi's local-first Rust/GPUI desktop.

### Added and connected

- Automatic platform data paths and database initialization.
- Navy/teal desktop navigation, host inventory and inspector, and host editing.
- Functional snippet/workspace editors, saved settings, host import/export and
  group defaults connected to local storage.
- Real SSH/local PTY sessions, measured terminal sizing, scrolling and split panes.
- Public SSH-key and trust-store tools, real tunnel sessions and local file browsing.
- Masked encrypted-backup export/restore with transactional error rollback.
- README app screenshots, current feature status and engineering roadmap.

### Corrected

- Preserve terminal text attributes across separate input chunks.
- Stop automatic dispatch of startup commands before authentication readiness is known.
- Remove simulated remote SFTP listings, transfer completion and tunnel traffic.
- Report unfinished cloud synchronization and network collaboration explicitly.
- Update release packaging for prerelease versions and exclude staging directories
  from uploaded assets and checksum generation.

### Verification and limits

The implementation passed 270 workspace tests, with one additional opt-in SSH
server test exercised separately. Formatting, Clippy and a local Intel macOS
release build passed. GUI checks covered local editing, shell launch, preferences,
long output, scrolling and split sizing.

This alpha is unsigned, not production-ready and not validated on every target
platform. Graphical remote SFTP, Drive transport, authenticated cloud discovery
and network collaboration remain unfinished. See `docs/GUI_STATUS.md`.

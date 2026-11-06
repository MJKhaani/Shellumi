# Changelog

## Unreleased

## 0.0.2-alpha

### Post-tag packaging repair

- Verified and published the unchanged `v0.0.2-alpha` tag using the repaired
  workflow: all four native build/package jobs passed, with 15 assets and
  validated checksums/source provenance.

- Repair Debian prerelease version conversion, validate package inputs before
  output cleanup, require both binaries, and generate root-owned Debian archives.
- Add three actual-helper packaging regression tests (321 workspace tests on main).
- Support manual release retries from unchanged source tags with corrected
  packaging tools and explicit source/helper provenance.

### Application changes

- Interrupt blocked SSH pipe reads/writes on cancellation and enforce one bounded
  deadline per complete packet (30 seconds by default), including SFTP handshake.
- Terminate the SSH child after pipe failure/interruption; keep cleanup warnings
  with remote staging paths when a connection cannot be reused safely.
- Add blocked/trickled-pipe and nonresponsive SSH connection acceptance tests.
- Align crate, FFI/diagnostic and package versions; require release tags to match crate versions.
- Patch the existing xattr 0.2.3 Linux errno compatibility failure while retaining
  upstream licenses and native behavior; native Linux app build remains for CI.


- Reject unsafe listing filenames before joining GUI transfer destinations.
- Align live SFTP reads with the codec’s 16 MiB frame limit before allocation.
- Stage SFTP downloads/uploads privately and publish only after validated transfer
  completion; preserve existing destinations on pre-publication failure/cancel.
- Add graphical queued/running Cancel actions, live byte progress, real worker
  limits and stable tab/listing updates. Cancellation interrupts SSH pipe waits; failed
  remote cleanup retains a visible temporary-file path.
- Prevent byte progress and CLI queue previews from claiming completed transfers.
- Extend OpenSSH acceptance with destination preservation and cancellation in
  both directions; cover EOF/close failures, symlink safety and commit races.

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
- Regression coverage: 318 workspace tests passed on macOS, three opt-in tests ignored;
  both self-contained SSH/SFTP acceptance tests passed separately. Native xattr
  verification passed 4 integration tests and 1 doc test.

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

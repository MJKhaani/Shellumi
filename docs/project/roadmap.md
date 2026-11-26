[Documentation Home](../README.md) > [Project](README.md) > **Roadmap**

# Engineering Roadmap

Shellumi has completed its foundational `v0.0.8-alpha` architecture (isolated encrypted vaults, multi-window GPUI workspace, unified inspector, SSH/SFTP, native VNC/RDP engines, Google Drive E2EE sync, and Shellumi Server E2EE client foundations). Historical implementation logs live in [CHANGELOG.md](../../CHANGELOG.md) and [Archive](../archive/README.md).

This roadmap tracks **forward-looking** priorities from the `v0.0.8-alpha` baseline onward.

## Near-Term (v0.0.x Stabilization)

1. **Interactive Platform Sign-Off (Windows & Linux):** Execute [Native Testing](../qa/native-testing.md) on physical/VM Windows and Linux desktops to clear `NOT TESTED` status for `PasswordVault`, `secret-tool`, askpass, and launcher behavior.
2. **Sync Identifier & Ordering Hardening:** Resolve the remaining object-identifier ordering and tombstone compaction items in [Known Issues](known-issues.md) and [Object Identifiers](../synchronization/object-identifiers.md).
3. **RDP Request-ID & Timeout Correlation:** Tighten `RemoteSessionKey` + request-ID correlation in `shellumi-rdp` and `shellumi-app` across rapid reconnect/resize cycles.
4. **Live Protocol & Server Testbed:** Add reproducible live-server test harnesses for `sshd`, `xrdp`, TigerVNC, and Shellumi Server integration.

## Medium-Term (v0.1.x Beta Readiness)

1. **Mosh Full Transport Integration:** Complete live `mosh-server` UDP SSP transport and roaming session recovery beyond current capability scaffolding.
2. **Team Epoch Rotation & Live Server QA:** Verify multi-client epoch rotation, member revocation, and encrypted collaboration relay against live Shellumi Server releases.
3. **CLI Secret Input Hardening:** Deprecate raw `--passphrase` CLI arguments in favor of stdin/prompt/fd input.
4. **Packaging & Auto-Update Polish:** Finalize signed macOS `.dmg` notarization, Windows MSI/winget packaging, and Linux `.deb`/AppImage distribution in `.github/workflows/release.yml`.

## Long-Term (Post-Desktop)

1. **Mobile Clients (iOS / Android):** Build native SwiftUI and Jetpack Compose frontends over portable `shellumi-core` bindings as evaluated in [Mobile Feasibility](mobile-feasibility.md).
2. **Independent Security Audit:** Commission external cryptographic and memory-safety review of vault envelopes, KDF parameters, and sync protocols.

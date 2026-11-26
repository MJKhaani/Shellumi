[Documentation Home](../README.md) > [QA](README.md) > **Protocol Testing**

# Protocol & Sync Testing

This document describes how Shellumi verifies each remote protocol and synchronization backend, separating deterministic test doubles from live server verification.

## SSH & SFTP

- **Unit & Integration:** Destination/IPv6/port validation (`shellumi-core`), SFTP protocol framing, directory listing, and remote editor dirty-state/save over cross-platform loopback TCP sockets (`crates/shellumi-core/src/sftp/tests.rs`).
- **Live `sshd` Verification:** Opt-in live SSH/SFTP integration tests against an `sshd` container or local daemon verify real key exchange, TOFU `known_hosts` enforcement, password/key/agent auth, and port forwarding. Run manually when an `sshd` target is configured.

## Mosh

- **Implemented:** Domain model and capability flags (`ProtocolCapabilities` in `crates/shellumi-core/src/protocol.rs`) and CLI/app dispatch scaffolding.
- **Live Mosh Server Verification:** `NOT TESTED` against a live `mosh-server` in CI.

## VNC (`shellumi-vnc`)

- **Deterministic Tests:** Mock RFB server handshake, security-type negotiation, pixel format conversion, framebuffer rectangle decoding, and malformed-frame rejection run in `cargo test -p shellumi-vnc`.
- **Live VNC Server Verification:** `NOT TESTED` in automated CI against TigerVNC, TightVNC, RealVNC, or Apple Screen Sharing.

## RDP (`shellumi-rdp`)

- **Deterministic Tests:** IronRDP integration state transitions, framebuffer sizing, and error mapping in `cargo test -p shellumi-rdp`.
- **Live RDP Server Verification:** `NOT TESTED` in automated CI against Windows RDP, xrdp, or FreeRDP.

## Google Drive Sync

- **Deterministic Tests:** Revision protocol state machine, encrypted payload serialization, and conflict/manifest handling in `shellumi-storage`.
- **Live Google Drive API Verification:** `NOT TESTED` in automated CI against live Google OAuth + Drive v3 endpoints.

## Shellumi Server Sync

- **Deterministic Tests:** Transaction atomicity, mutation queue replay, cross-device object-identifier resolution, and conflict-resolution tests in `crates/shellumi-storage/src/server_sync.rs`.
- **Live Shellumi Server Verification:** `NOT TESTED` in automated CI against a live Shellumi Server instance; see [Known Issues](../project/known-issues.md) for remaining multi-device edge cases.

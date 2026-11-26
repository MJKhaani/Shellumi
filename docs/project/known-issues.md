[Documentation Home](../README.md) > [Project](README.md) > **Known Issues**

# Known Issues & Open Gaps

This document transparently tracks unresolved defects, architectural edge cases, and unverified platform/protocol surfaces as of commit `3123db2`.

## Synchronization & Object Identifiers

See [Object Identifiers](../synchronization/object-identifiers.md) for technical details on the following 6 open identifier/sync gaps:

1. **Legacy Name Fallback on Import:** Legacy payloads lacking stable object UUIDs may still fall back to name matching on import when unambiguous.
2. **Concurrent Rename Conflict Convergence:** Simultaneous renames of the same entity across offline devices rely on LWW/conflict surface without a three-way field merge UI.
3. **Host-Group Re-parenting Ordering:** Out-of-order delta replay where a host references a newly created group before the group's creation record arrives requires deferred resolution.
4. **Tombstone Compaction Policy:** Long-lived vaults accumulate signed tombstones without an automated epoch-based compaction schedule.
5. **Partial Multi-Entity Rollback Messaging:** Failed batch sync commits surface generic retry status rather than per-entity diff details.
6. **Live Shellumi Server Interoperability:** End-to-end multi-device synchronization against a live Shellumi Server instance is `NOT TESTED` in CI.

## Remote Desktop (RDP / VNC)

- **RDP Request-ID & Timeout Correlation:** Ambiguous async request timeouts during rapid reconnect or resize cycles need stricter request-ID correlation.
- **Live Server Coverage:** Live RDP (`xrdp`, Windows RDP) and live VNC (TigerVNC, TightVNC, Apple Screen Sharing) visual/input sessions are `NOT TESTED` in CI.

## Security & Collaboration

- **Team Epoch Rotation Live Verification:** Forward-secure epoch rotation and member revocation have deterministic tests, but live multi-client Shellumi Server transaction verification is `NOT TESTED`.
- **CLI `--passphrase` Process-List Visibility:** Passing `--passphrase` on the command line is visible to local process inspection (`ps`); interactive prompt or stdin/env alternatives should be preferred for sensitive automation.

## Native Platform Interactive Verification

- **Windows Interactive Runtime:** Subsystem suppression (`windows_subsystem = "windows"`), `CREATE_NO_WINDOW` subprocesses, `PasswordVault` Credential Manager, and askpass pipe behavior are `NOT TESTED` interactively on Windows.
- **Linux Interactive Runtime:** Desktop entry launch under GNOME/KDE (X11 and Wayland) and `secret-tool` D-Bus Secret Service interaction are `NOT TESTED` interactively on Linux.

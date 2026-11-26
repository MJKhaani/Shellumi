[Documentation Home](../README.md) > **Quality Assurance**

# Quality Assurance & Verification

This section defines how Shellumi is tested, verified, and audited across platforms, protocols, security boundaries, and release baselines.

> **Verification Honesty:** Shellumi distinguishes strictly between automated CI checks, local mocked or deterministic tests, and live native/protocol testing. Capabilities that have not been exercised interactively on a target platform or against a live server are explicitly marked `NOT TESTED` in [Verification Status](verification-status.md).

## Contents

| Document | Purpose |
| --- | --- |
| [Test Strategy](test-strategy.md) | Layered testing model (unit, integration, deterministic state machine, GPUI UI, native OS, live protocol, security). |
| [Native Testing](native-testing.md) | Platform-specific verification checklists for macOS, Windows, and Linux desktop builds. |
| [Protocol Testing](protocol-testing.md) | Verification coverage for SSH, SFTP, Mosh, RDP, VNC, Google Drive sync, and Shellumi Server sync. |
| [Security Testing](security-testing.md) | Cryptography, vault isolation, crash recovery, credential journal, TOFU host-key, and log redaction checks. |
| [Verification Status](verification-status.md) | Current verified baseline (`3123db2`), CI run results, and explicit `NOT TESTED` boundaries. |
| [Acceptance Matrix](acceptance-matrix.md) | Detailed UI/UX, Inspector, Layout, Remote Desktop, and Data Safety traceability matrix. |

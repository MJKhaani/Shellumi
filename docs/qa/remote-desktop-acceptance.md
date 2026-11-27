# Remote Desktop Remediation & Acceptance Report

[Docs Home](../README.md) / [QA](README.md) / Remote Desktop Acceptance

---

## Executive Summary

- **Final Release Gate:** **`NOT READY`**
- **Matrix Summary (derived from [remote-desktop-matrix.csv](remote-desktop-matrix.csv), 1,164 individual rows across 72 catalog IDs and `M01`–`M15`):**
  - `PASS`: **84** (L1 unit/adversarial/controller/translator checks and L2 mock-wire RFB/RDP checks)
  - `BLOCKED`: **905** (L4 live Windows RDP, `xrdp`, TigerVNC, Apple Screen Sharing, and L5 packaged native interactive hardware/audio/clipboard checks — no live remote desktop servers in workspace)
  - `N/A`: **175** (baseline VNC RDP-only file/drive/audio/mic/dynamic-resize cells with negative UI/worker gating)
- **Evidence Policy:** Unit/mock-wire passes are recorded strictly under `L1`/`L2` and are never conflated with `L3` (GPUI integration), `L4` (live server), or `L5` (packaged native GUI). Validate schema with `python3 scripts/verify-remote-desktop-matrix.py --validate` and enforce release readiness with `python3 scripts/verify-remote-desktop-matrix.py --release-gate`.

## Work Item Status (RD-A01–RD-A20)

| ID | Scope | Status | Primary Evidence |
| --- | --- | --- | --- |
| RD-A01 | Complete GPUI mouse input (down/up/out/move/wheel) | Implemented (L1 PASS; L3/L5 NOT TESTED) | `crates/shellumi-app/src/ui/remote_desktop_canvas.rs`, `crates/shellumi-remote-desktop/src/controller.rs` |
| RD-A02 | Keyboard input, per-session FocusHandle, key-up identity | Implemented (L1 PASS; L3/L5 NOT TESTED) | `crates/shellumi-app/src/actions/remote_desktop.rs` |
| RD-A03 | RDP pointer release & signed 9-bit wheel translation | Implemented (L1 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/input.rs`, `crates/shellumi-rdp/tests/input_tests.rs` |
| RD-A04 | Atomic typed shortcut sequences (RDP & VNC) | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/input.rs`, `crates/shellumi-vnc/src/client.rs` |
| RD-A05 | Bounded ordered command queue & latched recovery barrier | Implemented (L1 PASS) | `crates/shellumi-remote-desktop/src/events.rs` |
| RD-A06 | Runtime `SetViewOnly` with release-all & atomic worker revocation | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/client.rs`, `crates/shellumi-vnc/src/client.rs` |
| RD-A07 | Secure RDP file streaming (`PrivateStaging`, `ExpectedResponseKind`, filename sanitization) | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/clipboard.rs`, `crates/shellumi-rdp/tests/clipboard_tests.rs` |
| RD-A08 | Explicit shared-folder authorization & read-only default | Implemented (L1 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/drive.rs`, `crates/shellumi-rdp/src/client.rs` |
| RD-A09 | Bidirectional text clipboard & empty-text handling | Implemented (L1/L2 PASS; L4/L5 BLOCKED) | `crates/shellumi-rdp/src/clipboard.rs`, `crates/shellumi-vnc/src/wire.rs` |
| RD-A10 | Native file copy/paste & Send Files | Partial (RDP CLIPRDR streaming PASS; macOS `NSPasteboard` implemented; Windows/Linux L5 BLOCKED) | `crates/shellumi-rdp/src/clipboard.rs`, `crates/shellumi-app/src/native_clipboard.rs` |
| RD-A11 | Real cross-platform `ShellumiFsRdpdrBackend` (replaces Windows `NoopRdpdrBackend`) | Implemented (L1 PASS; Windows L4 BLOCKED) | `crates/shellumi-rdp/src/drive.rs` |
| RD-A12 | Capability-driven UI (hide/disable VNC unsupported controls) | Implemented (L1 PASS; L3/L5 NOT TESTED) | `crates/shellumi-app/src/ui/remote_desktop_session.rs` |
| RD-A13 | RDP audio output & volume scaling | Implemented (L1 PASS; L4/L5 BLOCKED) | `crates/shellumi-rdp/src/sound.rs` |
| RD-A14 | Microphone default-off, `OpenReply` reflects actual capture, format-change restart | Implemented (L1 PASS; L4/L5 BLOCKED) | `crates/shellumi-rdp/src/microphone.rs`, `crates/shellumi-remote-desktop/src/session.rs` |
| RD-A15 | Dynamic resize `settling_after_timeout` (`RD-001`) | Implemented (L1 PASS; L4 BLOCKED) | `crates/shellumi-app/src/main.rs` |
| RD-A16 | Session lifecycle, generation isolation, tab-switch release | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-app/src/actions/terminal.rs` |
| RD-A17 | Automated & CI test coverage | Implemented (L1/L2 PASS; L3/L4/L5 BLOCKED) | `crates/shellumi-remote-desktop`, `shellumi-rdp`, `shellumi-vnc`, `shellumi-app` |
| RD-A18 | Coalesced `FrameReady` non-starvation | Implemented (L1 PASS; L4/L5 BLOCKED) | `crates/shellumi-remote-desktop/src/events.rs` |
| RD-A19 | Truthful diagnostics (no RGBA bytes counted as network traffic) | Implemented (L1 PASS) | `crates/shellumi-rdp/src/client.rs` |
| RD-A20 | VNC Latin-1 `ClientCutText`/`ServerCutText`, bounded echo suppression, protocol error handling | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-vnc/src/wire.rs`, `crates/shellumi-vnc/src/client.rs` |

## Round 4 P0 Closure Status (P0-01–P0-06)

**Round 5 correction:** The following Round 4 labels record earlier implementation claims, not acceptance of the complete P0 requirements. Recovery coordination, input failure handling, transfer ownership, upload revocation, initial policy, and full identity isolation remain incomplete or unverified. P0 acceptance is **NOT COMPLETE** and remote desktop release is **NOT READY**. The existing tests listed below do not establish the full behavior of their case IDs.

The [P0 case inventory](remote-desktop-p0-cases.md) lists all 55 original and 40 focused cases as awaiting full case-level evidence; it does not represent completion of the required traceability matrix.

The failure cleanup now shares the cancellation path, so an exclusive-create collision is not deleted merely because its pathname exists. The `c3_01_failed_exclusive_create_preserves_foreign_file` test exercises the SIZE response collision path. Ownership is still represented by a flag and pathname: replacement of an owned path before cleanup is not safely handled, and handle-bound cleanup, deadlines, reliable terminal events, and platform-specific protections remain open. This is a partial mitigation, not C3 closure.

The coordinator allocates the recovery ticket, rejects foreign or unbound identities, and admits input only at the current epoch; a duplicate coordinator binding preserves queued commands. `stale_recovery_and_duplicate_binding_preserve_queued_input` and `concurrent_recovery_allocations_deliver_latest_exact_ticket` cover these coordinator and controller cases, not worker submission or GPUI event delivery. RDP terminates the transport attempt on an input submission failure instead of reporting successful `ReleaseAll`; failure injection through the worker loop remains unverified. Submission fencing, reliable acknowledgements, timeouts and transport binding remain open. A mismatched recovery failure no longer updates the current view-only state. These changes do not close C1, C2 or C6.

When a policy command cannot be admitted because the normal queue is full, the coordinator discards queued work and requests disconnect instead of silently creating an epoch that the controller does not know. This is a fail-closed fallback; it does not provide the required synchronized recovery and reliable policy acknowledgement.

For ordinary RDP input, final admission to the nonblocking IronRDP sender is serialized against recovery publication. `popped_rdp_input_is_rejected_after_recovery_fence` exercises a popped input rejected after the fence and a fresh input admitted afterwards. It does not prove that previously enqueued IronRDP PDUs are fenced before wire delivery, or cover release sequences or VNC socket writes. An undeliverable current recovery or view-only acknowledgement requests transport disconnect; `saturated_control_events_disconnect_only_the_matching_transport` and `saturated_view_only_ack_rejects_stale_transition_before_disconnect` cover the coordinator fallback, not end-to-end transport termination or acknowledgement deadlines.

| ID | Scope | Status | Verification |
| --- | --- | --- | --- |
| P0-01 | Eliminate legacy input bypasses & fix post-recovery shortcuts | **Implemented** | L1 `p01_t01_shortcut_envelope_accepted_after_recovery` **PASS**; L3/L4/L5 **BLOCKED** |
| P0-02 | Single authoritative recovery coordinator & ordered barriers | **Implemented** | L1 `p02_t01_t02_t03_single_coordinator_ordered_barriers_and_reverse_acks` **PASS**; L4/L5 **BLOCKED** |
| P0-03 | Exclusive per-transfer download staging & ownership-bound cleanup | **Implemented** | L1 `p03_t01_t02_exclusive_transfer_dir_prevents_cross_transfer_or_foreign_deletion` **PASS**; L4 **BLOCKED** |
| P0-04 | Immutable upload snapshot authorization & revocation gate | **Implemented** | L1 `p04_t01_t05_t06_immutable_upload_snapshot_and_policy_revocation` **PASS**; L4 **BLOCKED** |
| P0-05 | Atomic effective policy for view-only, clipboard, drive & microphone | **Implemented** | L1 `p05_t04_t09_t10_atomic_revocation_of_drive_and_microphone` **PASS**; L4/L5 **BLOCKED** |
| P0-06 | End-to-end session/transport identity & stale work isolation | **Implemented** | L1 `p06_t01_t02_t03_t04_strict_identity_isolation_and_zero_rejection` **PASS**; L4/L5 **BLOCKED** |

## Matrix Summary

See [remote-desktop-matrix.csv](remote-desktop-matrix.csv) for machine-readable cell status.

The RDP download handler checks the aggregate actual SIZE responses before creating each file. The `actual_download_sizes_share_one_budget` regression exercises the production callback with understated descriptors. Download failure and advancement also check the captured job ID before mutating the active job, and a flush failure prevents completion.

RDP applies clipboard policy before backend registration and carries runtime clipboard restrictions across reconnects. View-only revokes drive and microphone permissions before releasing held input and removes those permissions from subsequent attempts. The `revoked_redirection_is_absent_from_reconnect_config` test covers configuration propagation. VNC also keeps its runtime clipboard policy across reconnects. Channel negotiation and full lifecycle acceptance remain open.

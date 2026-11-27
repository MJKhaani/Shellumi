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

## Matrix Summary

See [remote-desktop-matrix.csv](remote-desktop-matrix.csv) for machine-readable cell status.

# Remote Desktop Remediation & Acceptance Report

[Docs Home](../README.md) / [QA](README.md) / Remote Desktop Acceptance

---

## Executive Summary

- **Final Release Gate:** **NOT READY** (L1–L3 automated unit, adversarial, and mock-wire checks **PASS**; L4 live server and L5 packaged native interactive hardware tests are **BLOCKED** pending external Windows/xrdp/TigerVNC/Apple Screen Sharing targets and interactive desktop execution).
- **Evidence Policy:** Unit/mock-wire passes are recorded strictly under `L1`/`L2` and are never conflated with `L4` (live server) or `L5` (packaged native GUI).

## Work Item Status (RD-A01–RD-A20)

| ID | Scope | Status | Primary Evidence |
| --- | --- | --- | --- |
| RD-A01 | Complete GPUI mouse input (down/up/out/move/wheel) | Implemented (L1/L3 PASS; L5 BLOCKED) | `crates/shellumi-app/src/ui/remote_desktop_canvas.rs`, `crates/shellumi-remote-desktop/src/controller.rs` |
| RD-A02 | Keyboard input, per-session FocusHandle, key-up identity | Implemented (L1/L3 PASS; L5 BLOCKED) | `crates/shellumi-app/src/actions/remote_desktop.rs` |
| RD-A03 | RDP pointer release & signed 9-bit wheel translation | Implemented (L1 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/input.rs`, `crates/shellumi-rdp/tests/input_tests.rs` |
| RD-A04 | Atomic typed shortcut sequences (RDP & VNC) | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/input.rs`, `crates/shellumi-vnc/src/client.rs` |
| RD-A05 | Bounded coalescing command queue & separate frame slot | Implemented (L1 PASS) | `crates/shellumi-remote-desktop/src/events.rs` |
| RD-A06 | Runtime `SetViewOnly` with release-all & worker guard | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/client.rs`, `crates/shellumi-vnc/src/client.rs` |
| RD-A07 | Secure RDP file streaming (`ExpectedResponseKind`, private dir, filename sanitization) | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/clipboard.rs`, `crates/shellumi-rdp/tests/clipboard_tests.rs` |
| RD-A08 | Explicit shared-folder authorization & read-only default | Implemented (L1 PASS; L4 BLOCKED) | `crates/shellumi-rdp/src/drive.rs`, `crates/shellumi-rdp/src/client.rs` |
| RD-A09 | Bidirectional text clipboard & empty-text handling | Implemented (L1/L2 PASS; L4/L5 BLOCKED) | `crates/shellumi-rdp/src/clipboard.rs`, `crates/shellumi-vnc/src/wire.rs` |
| RD-A10 | Native file copy/paste & Send Files | Partial (RDP CLIPRDR streaming PASS; native OS pasteboard file-promise L5 BLOCKED) | `crates/shellumi-rdp/src/clipboard.rs` |
| RD-A11 | Real cross-platform `ShellumiFsRdpdrBackend` (replaces Windows `NoopRdpdrBackend`) | Implemented (L1 PASS; Windows L4 BLOCKED) | `crates/shellumi-rdp/src/drive.rs` |
| RD-A12 | Capability-driven UI (hide/disable VNC unsupported controls) | Implemented (L1/L3 PASS; L5 BLOCKED) | `crates/shellumi-app/src/ui/remote_desktop_session.rs` |
| RD-A13 | RDP audio output & volume scaling | Implemented (L1 PASS; L4/L5 BLOCKED) | `crates/shellumi-rdp/src/sound.rs` |
| RD-A14 | Microphone default-off, `OpenReply` reflects actual capture, format-change restart | Implemented (L1 PASS; L4/L5 BLOCKED) | `crates/shellumi-rdp/src/microphone.rs`, `crates/shellumi-remote-desktop/src/session.rs` |
| RD-A15 | Dynamic resize `settling_after_timeout` (`RD-001`) | Implemented (L1 PASS; L4 BLOCKED) | `crates/shellumi-app/src/main.rs` |
| RD-A16 | Session lifecycle, generation isolation, tab-switch release | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-app/src/actions/terminal.rs` |
| RD-A17 | Automated & CI test coverage | Implemented (L1–L3 PASS; L4/L5 BLOCKED) | `crates/shellumi-remote-desktop`, `shellumi-rdp`, `shellumi-vnc`, `shellumi-app` |
| RD-A18 | Coalesced `FrameReady` non-starvation | Implemented (L1 PASS; L4/L5 BLOCKED) | `crates/shellumi-remote-desktop/src/events.rs` |
| RD-A19 | Truthful diagnostics (no RGBA bytes counted as network traffic) | Implemented (L1 PASS) | `crates/shellumi-rdp/src/client.rs` |
| RD-A20 | VNC Latin-1 `ClientCutText`/`ServerCutText`, bounded echo suppression, protocol error handling | Implemented (L1/L2 PASS; L4 BLOCKED) | `crates/shellumi-vnc/src/wire.rs`, `crates/shellumi-vnc/src/client.rs` |

## Matrix Summary

See [remote-desktop-matrix.csv](remote-desktop-matrix.csv) for machine-readable cell status.

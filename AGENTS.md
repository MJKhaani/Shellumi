# AGENTS.md — Development Guidelines for AI Agents and Contributors

This document defines non-negotiable engineering principles, development workflows, and standards for AI agents and human contributors working on Shellumi.

---

## 1. Project Mission and Architecture Overview

Shellumi is a free, open-source (MIT), local-first desktop SSH client and remote infrastructure manager built in Rust with GPUI. Its primary mission is to offer a comprehensive alternative to proprietary clients (such as Termius) without forced cloud accounts or paid paywalls.

### Workspace Structure

```text
crates/shellumi-core/      UI-independent domain models and destination validation
crates/shellumi-terminal/  VT/ANSI terminal emulator engine & cross-platform PTY manager
crates/shellumi-storage/   Local async Turso database operations and migrations
crates/shellumi-cli/       CLI host management and connection dispatching
crates/shellumi-app/       GPUI 0.2.2 desktop user interface
docs/                      Architecture, roadmap, threat model, feature matrix
```

---

## 2. Non-Negotiable Rules for Agents

1. **Rust & GPUI Only:** All core business logic and user interface code must be written in Rust. The desktop UI uses GPUI exclusively; do not introduce webviews, Electron, Tauri, or React wrappers.
2. **Strict Component Decoupling:**
   - GUI code in `shellumi-app` must not contain SSH protocol implementations or database logic.
   - Core domain models and protocols must never depend on GPUI or UI crates.
   - Database operations must run on background tasks (`cx.background_executor()`) and never block the main UI thread.
3. **No Mock Implementations:** Never disguise a mock or stub as a finished implementation. If a capability is not yet implemented, document it clearly and leave the blocker visible.
4. **Security by Default:**
   - Never disable or bypass SSH host-key verification.
   - Never store passwords, private keys, or tokens in plaintext files, logs, or unencrypted storage.
   - Never pass arbitrary unvalidated strings to a shell.
5. **No Telemetry or Mandatory Accounts:** The application must remain fully functional offline and locally without an account.
6. **Verify Before Reporting:** Never claim a test passed or a feature works without compiling and running the test suite.

---

## 3. Coding and Formatting Standards

- **Rust Edition:** Rust 2021 edition.
- **Code Formatting:** Strictly adhere to `cargo fmt --all`.
- **Linting:** Must pass `cargo clippy --workspace --all-targets -- -D warnings` with zero warnings.
- **Imports:** Keep imports clean and organized; avoid wildcard re-exports unless standard for GPUI traits (`gpui::prelude::*`).
- **Dependencies:**
  - Before adding dependencies, verify their maintenance status, license compatibility (MIT/Apache-2.0 preferred), security history, and whether they compile offline.
  - Avoid unnecessary heavy dependencies when std or existing crates suffice.

---

## 4. Testing Requirements

Every increment must have automated test coverage:
1. **Core Domain:** Input validation, edge cases (IPv6, whitespace, invalid ports, injection attempts).
2. **Storage:** Migration transactions, database reopen persistence, future schema version rejection, CRUD correctness.
3. **CLI:** Process execution, argument parsing, error reporting.
4. **UI Components:** Data conversions and unit-testable helpers.

Run the test suite:
```sh
cargo test --offline --workspace
```

---

## 5. Reporting Task Completion

When finishing any task or increment, always report:
1. **What was implemented:** Detailed summary of new capabilities or fixes.
2. **Files changed:** List of modified, created, or deleted files.
3. **Tests executed:** Commands run and test pass/fail results.
4. **Remaining limitations:** Transparent disclosure of what is still pending.
5. **Next concrete engineering step:** The next logical increment from `docs/ROADMAP.md`.

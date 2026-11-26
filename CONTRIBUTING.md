# Contributing to Shellumi

Thank you for your interest in contributing to Shellumi! Shellumi is a community-driven, MIT-licensed SSH client and remote infrastructure management application built in Rust and GPUI.

---

## 1. Development Setup

### Prerequisites
- **Rust Toolchain:** Stable Rust toolchain (Rust 2021 edition, `rustfmt`, `clippy`).
- **macOS:** Xcode developer tools and Metal compiler (`xcrun --find metal`).
- **Linux:** `pkg-config`, C/C++ compiler, `libssl-dev`, `libfontconfig1-dev`, `libxkbcommon-dev`, `libxkbcommon-x11-dev`, `libwayland-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`, `libasound2-dev`.
- **Windows:** Visual Studio C++ Build Tools and MSVC toolchain.

### Clone and Build
```sh
git clone https://github.com/MJKhaani/Shellumi.git
cd Shellumi
cargo build --locked --workspace
```

See [Environment Setup](docs/development/environment-setup.md) and [Build and Run](docs/development/build-and-run.md) for detailed instructions.

---

## 2. Project Architecture Conventions

The Cargo workspace contains **8 crates**:
- **`shellumi-core`**: Domain models, cryptography, credential-store traits/backends, sync protocols, and validation. Zero GUI dependencies.
- **`shellumi-terminal`**: VT/ANSI terminal emulator engine and cross-platform PTY manager.
- **`shellumi-storage`**: Local async encrypted Turso database operations, schema migrations, and sync storage.
- **`shellumi-cli`**: Command-line interface for headless management, diagnostics, and connection dispatching.
- **`shellumi-remote-desktop`**: Shared remote desktop session abstractions, framebuffer, and input/event types.
- **`shellumi-vnc`**: Native RFB transport and VNC decoders.
- **`shellumi-rdp`**: Embedded IronRDP client integration (`MS-RDPEDISP`, `CLIPRDR`, `RDPSND`, `RDPDR`, `MS-RDPEAI`).
- **`shellumi-app`**: GPUI `0.2.2` desktop application.

See [Workspace & Crates](docs/architecture/workspace-and-crates.md) and [Coding Standards](docs/development/coding-standards.md).

### Mandatory Architectural Rules
1. **Decouple GUI from Business Logic:** Never embed network protocols, cryptography, or database queries directly inside GPUI view render methods.
2. **Asynchronous Execution:** Run I/O, database access, and protocol handshakes on background tasks.
3. **No Plaintext Secrets:** Passwords, tokens, and private keys must never be logged or stored unencrypted. Follow [Logging Guidelines](docs/development/logging.md).

---

## 3. Quality Checks Before Submitting Pull Requests

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
./scripts/verify-docs.sh
```

---

## 4. Pull Request Guidelines

1. **Keep PRs focused:** Submit small, reviewable increments.
2. **Include Tests:** Any new feature or bugfix should come with corresponding unit or integration tests.
3. **Update Documentation:** Update the authoritative documents under `docs/` in the same change set (see [`AGENTS.md`](AGENTS.md#5-mandatory-documentation-maintenance-requirements)).
4. **License:** All contributions are licensed under the MIT License ([`LICENSE`](LICENSE)).

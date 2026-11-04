# Contributing to Shellumi

Thank you for your interest in contributing to Shellumi! Shellumi is a community-driven, MIT-licensed SSH client and infrastructure management application.

---

## 1. Development Setup

### Prerequisites
- **Rust Toolchain:** A recent stable Rust toolchain (Rust 1.99+ recommended).
- **macOS:** Xcode developer tools and Metal compiler (`xcrun --find metal`).
- **Linux:** `pkg-config`, C/C++ compiler, fontconfig, FreeType, Wayland/X11, and libxkbcommon development headers.
- **Windows:** Visual Studio C++ Build Tools and MSVC toolchain.

### Clone and Build
```sh
git clone https://github.com/your-org/shellumi.git
cd shellumi
cargo build --locked --workspace
```

---

## 2. Project Architecture Conventions

- **`shellumi-core`**: Core domain types and validation. Pure Rust, zero I/O, zero GUI dependencies.
- **`shellumi-storage`**: Database persistence layer using local Turso. Manages metadata only.
- **`shellumi-cli`**: Command-line interface for headless management and scriptable operations.
- **`shellumi-app`**: GPUI desktop window application.

### Important Architectural Rules
1. **Decouple GUI from Business Logic:** Never embed network protocols, cryptography, or database queries directly inside GPUI view render methods.
2. **Asynchronous Execution:** All long-running operations (I/O, database access, SSH handshakes) must run on background tasks so the UI thread remains at 60/120 FPS.
3. **No Plaintext Secrets:** Passwords and private keys must never be logged or stored unencrypted.

---

## 3. Quality Checks Before Submitting Pull Requests

Before creating a commit or opening a PR, ensure that all automated checks pass:

```sh
# 1. Format code
cargo fmt --all -- --check

# 2. Run clippy linter with warnings treated as errors
cargo clippy --workspace --all-targets -- -D warnings

# 3. Run all tests
cargo test --workspace
```

---

## 4. Pull Request Guidelines

1. **Keep PRs focused:** Submit small, reviewable increments rather than sprawling multi-feature diffs.
2. **Include Tests:** Any new feature or bugfix should come with corresponding unit or integration tests.
3. **Update Documentation:** If you change user-facing behavior, CLI commands, or architecture, update `README.md`, `docs/ARCHITECTURE.md`, `docs/FEATURE_MATRIX.md`, and `docs/ROADMAP.md`.
4. **License:** All contributions are licensed under the MIT License.

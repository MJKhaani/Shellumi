# Shellumi

![Shellumi logo](resources/logo.png)

Shellumi is an early-stage, open-source desktop SSH client project written in
Rust. Its goal is to provide a fast, local-first tool for administering remote
infrastructure, with a native GPU-accelerated interface built using GPUI.
Shellumi is intended to remain free to use and does not require an account for
local operation.

**Status: foundation only.** The current GPUI window contains an informational
placeholder. Shellumi cannot yet open terminal or SSH sessions and is not ready
for everyday administration or production use. The design goals below are not
claims that those capabilities are already available.

## Current progress

| Area | Current state |
| --- | --- |
| Desktop interface | A minimal GPUI window is implemented; it has not been validated on all supported platforms. |
| Connection model | The core library validates SSH destination fields. It does not connect to servers or handle credentials. |
| Local storage | A separate async Turso Database crate can add, list, and delete host metadata, apply an initial transactional schema, and reject newer schema versions. It is not connected to the window. |
| Tests | Unit tests cover destination validation and storage operations, including file reopen and future-schema rejection. |
| Releases | A tag-triggered GitHub Actions workflow is configured to build binaries and publish a release; platform builds have not yet been verified. |

Turso runs locally and stores host metadata only. The current storage layer
does **not** provide encrypted credential storage, backup or recovery. Turso
Database is pre-1.0; its maintainers recommend independent backups. See the
[architecture](docs/ARCHITECTURE.md) and [threat model](docs/security/threat-model.md)
for design decisions and known gaps.

## Planned capabilities

The project aims to support secure SSH connections with explicit host-key
verification, an interactive terminal, organized hosts and identities, SFTP,
SSH tunnels, workspaces, snippets, and optional encrypted Google Drive sync.
Additional protocols, collaboration, infrastructure integrations, and mobile
clients are longer-term goals. None of these user-facing features is complete.

See [PROJECT.md](PROJECT.md) for the full requirements, [the roadmap](docs/ROADMAP.md)
for the next increments, and [the feature matrix](docs/FEATURE_MATRIX.md) for
current status. A feature is not considered complete until it works, is
integrated, and has appropriate security and test coverage.

## Repository structure

```text
crates/shellumi-app/      GPUI application window
crates/shellumi-core/     UI-independent connection model
crates/shellumi-storage/  Local Turso host metadata storage
docs/                     Architecture, roadmap, feature status, threat model
.github/workflows/        Tag-triggered release builds
```

The UI, core, and storage are separate workspace crates. The application does
not yet integrate the core or storage crates. Networking, terminal rendering,
credential management, and sync still need implementations.

## Build from source

Install a current stable [Rust toolchain](https://www.rust-lang.org/tools/install)
and the native dependencies required by [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).
The project uses a locked Cargo dependency graph and pins GPUI 0.2.2.

- **macOS:** Install Xcode, select its developer directory, and ensure
  `xcrun --find metal` succeeds. Command Line Tools alone may not provide the
  Metal compiler.
- **Linux:** Install a C/C++ build toolchain, `pkg-config`, and GPUI's font,
  Wayland, X11, and XKB development libraries. The release workflow lists the
  packages used on Ubuntu 24.04.
- **Windows:** Install the Rust MSVC toolchain and the Visual Studio C++ build
  tools required for native Rust builds.

From the repository root:

```sh
cargo run --locked -p shellumi-app
```

This opens the development placeholder, not a functional SSH client. macOS,
Windows, and Linux are target platforms; successful builds or runtime behavior
on all of them have not yet been established.

## Tests and development checks

```sh
cargo fmt --all -- --check
cargo test --locked -p shellumi-core -p shellumi-storage
cargo clippy --locked -p shellumi-core -p shellumi-storage --all-targets -- -D warnings
```

The GPUI app also needs the platform dependencies above. For a full workspace
check, use `cargo test --locked --workspace` and
`cargo clippy --locked --workspace --all-targets -- -D warnings` on a configured
machine. Tests of the individual core and storage crates do not validate the
desktop window or an SSH server connection.

## Releases

Pushing a Git tag triggers the [release workflow](.github/workflows/release.yml).
It targets Linux x86-64, Windows x86-64, and macOS Apple Silicon and Intel. If
all builds succeed, it publishes the standalone binaries and SHA-256 checksums
to a GitHub Release. These are unsigned development binaries, not installers;
the workflow and platform builds still need end-to-end validation.

## Security and contributions

Shellumi's security architecture is still being designed. Do not use the
current application to store secrets or rely on it for remote access. The
[threat model](docs/security/threat-model.md) tracks planned controls and gaps.
Contributions are welcome: discuss substantial changes first, keep protocol
and storage logic independent of GPUI, and include appropriate tests and
documentation. Report security issues privately to the repository maintainers.

## License

Shellumi is licensed under the [MIT License](LICENSE). Dependencies retain their
own licenses.

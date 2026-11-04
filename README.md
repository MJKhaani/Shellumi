# Shellumi

Shellumi is an early-stage Rust desktop SSH client project. The new GPUI
application currently opens a placeholder window; it does not yet provide SSH
or terminal functionality. The older `app/` directory contains a separate
egui prototype and is not the Shellumi implementation.

With stable Rust and platform build dependencies installed, run
`cargo run -p shellumi-app`. GPUI's platform requirements and examples are
documented [upstream](https://github.com/zed-industries/zed/tree/main/crates/gpui).
Desktop support has not yet been verified across platforms. See
[the roadmap](docs/ROADMAP.md), [feature status](docs/FEATURE_MATRIX.md), and
[initial threat model](docs/security/threat-model.md). Licensed under MIT;
dependencies retain their own licenses.

Pushing a Git tag runs the release workflow, which builds standalone binaries
for Linux x86-64, Windows x86-64, and macOS Apple Silicon and Intel. Once all
four builds succeed, it publishes them with SHA-256 checksums on a GitHub
Release. These are unsigned development builds, not platform installers.

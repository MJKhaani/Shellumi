# Shellumi roadmap and current state

The requirements and phases are in [PROJECT.md](../PROJECT.md). This is an
early project, not yet a functional SSH client. The GPUI window is a
placeholder and does not yet integrate the core or storage crates.

## Next reviewable increments

1. Build and run the pinned official GPUI 0.2.2 window on each desktop platform.
   GPUI itself is Apache-2.0; audit the complete transitive dependency set and
   security advisories before release. The registry is reachable outside the
   restricted sandbox. On this Mac, the build stopped because `xcrun` cannot
   find the Metal shader compiler; `xcode-select -p` points to CommandLineTools.
   Install or select an Xcode toolchain with Metal support, then retry the build.
2. Expand the Rust workspace: `shellumi-core` now validates SSH destination
   fields independently of GPUI, and `shellumi-storage` provides initial local
   host operations. Session adapters and UI integration are not yet implemented.
   Establish CI for format, lint, tests and platform builds.
3. Integrate a maintained terminal engine and PTY with interactive input,
   resizing, and lifecycle tests; then integrate SSH with explicit host-key
   confirmation and real-server tests.
4. Complete Turso-based local host storage, backup/recovery and credential-store
   integration before implementing persistent host and identity management.
5. Continue the remaining phases in PROJECT.md, with acceptance tests and
   platform verification for each capability.

Reference research: [upstream GPUI documentation](https://github.com/zed-industries/zed/tree/main/crates/gpui),
which describes GPUI as pre-1.0 and subject to breaking changes, and
[Rust release schedule](https://forge.rust-lang.org/), which lists Rust 1.99
as stable as of 2026-11-04. GPUI mobile support has not been validated.

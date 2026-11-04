# Resume checklist

- [x] Restore the native Rust desktop app with saved SSH hosts and a local terminal.
- [x] Normalize connection fields before validation and storage.
- [x] Clear pending terminal input when connecting or disconnecting.
- [x] Acquire PTY input/output handles before spawning a child to avoid leaving a child running if handle setup fails.
- [x] Compile the macOS debug executable.
- [x] Confirm the release build has completed; `target/release/harbor` was produced and the compiler exited.
- [ ] Verify visual similarity against the original. The prior live capture was blocked; no visual match is claimed.
- [ ] Obtain a successful `rebuild_check` result. Three calls, including after compilation, returned “No answer: no build is running for this project, or the check took too long.”
- [ ] Verify Windows and Linux builds on those systems.

The current scope is documented in README.md. Full terminal keyboard handling,
file transfer, and port forwarding remain unimplemented.

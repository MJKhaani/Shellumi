# AGENT.md — Development Guidelines for AI Agents and Contributors

See [`AGENTS.md`](AGENTS.md) for the complete engineering principles, rules, and guidelines for Shellumi contributors.

### Key Rules at a Glance

1. **Rust & GPUI Only:** The desktop UI uses GPUI exclusively; no webviews, Electron, Tauri, or React wrappers.
2. **Strict Component Decoupling:** Core models and protocols must not depend on UI crates.
3. **No Mock Implementations:** Never disguise a mock or stub as a finished implementation.
4. **Security by Default:** Never disable host-key verification; never store credentials in plaintext.
5. **No Telemetry or Mandatory Accounts:** The app must remain fully functional offline.
6. **Verify Before Reporting:** Run cargo test and clippy before reporting task completion.
7. **Authentic Remote Desktop State:** Real network I/O, live framebuffer rendering, no synthetic metrics.
8. **Mandatory Logging Compliance (`docs/LOGGING.md`):** All newly written or modified code must strictly follow [`docs/LOGGING.md`](docs/LOGGING.md). Structured logging via `tracing` (`error!`, `warn!`, `info!`, `debug!`, `trace!`) must be integrated across all operations and error paths, including initialization, UI, networking, authentication, synchronization, database operations, background tasks, and error handling. Never silently swallow errors. Never log passwords, tokens, private keys, passphrase credentials, or decryption keys in plaintext; sanitize and redact sensitive fields using `shellumi_core::logging::redact_sensitive` or `Redacted<T>`.

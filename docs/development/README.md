# <img src="../../resources/icon_pack/icons/48/regular/terminal.png" width="26" height="26" alt="Development"> Development Documentation

[Docs Home](../README.md) / Development

Build, coding, migration, logging, testing, packaging, and release procedures for Shellumi contributors.

---

| Document | Scope |
| :--- | :--- |
| [**Environment Setup**](environment-setup.md) | Rust toolchain and OS system dependencies (macOS, Linux, Windows) |
| [**Coding Standards**](coding-standards.md) | Rust 2021 rules, formatting, Clippy `-D warnings`, and decoupling |
| [**Build and Run**](build-and-run.md) | Building debug/release binaries and running desktop & CLI locally |
| [**Database Migrations**](database-migrations.md) | Turso Schema 32 rules, idempotent `ALTER TABLE`, and rollback tests |
| [**Logging**](logging.md) | Structured `tracing` logging, `SHELLUMI_LOG`, and secret redaction |
| [**Testing**](testing.md) | Workspace test suites, opt-in SSH/SFTP acceptance, and doc checks |
| [**SSH Integration Tests**](ssh-integration.md) | Isolated loopback `sshd` and SFTP acceptance test procedures |
| [**CI/CD**](ci-cd.md) | 3-platform GitHub Actions CI (`ci.yml`) and documentation checks |
| [**Packaging**](packaging.md) | macOS `.dmg`, Linux `.deb` + `.desktop`, and Windows `.zip` bundles |
| [**Releases**](releases.md) | Tag-driven release pipeline (`release.yml`), SBOM, and `SHA256SUMS.txt` |

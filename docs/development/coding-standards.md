# Coding Standards

[Docs Home](../README.md) / [Development](README.md) / Coding Standards

---

All contributors and AI agents must follow [`AGENTS.md`](../../AGENTS.md):

1. **Rust Edition:** Rust 2021 edition across all 8 workspace crates.
2. **Formatting & Linting:** Zero diffs under `cargo fmt --all -- --check` and zero warnings under `cargo clippy --locked --workspace --all-targets -- -D warnings`.
3. **Strict Decoupling:** `shellumi-core`, `shellumi-storage`, `shellumi-terminal`, `shellumi-remote-desktop`, `shellumi-vnc`, and `shellumi-rdp` must never depend on GPUI.
4. **Non-Blocking UI:** Run all SQLite queries, network I/O, and crypto on `cx.background_executor()`.
5. **Structured Redacted Logging:** Follow [Logging Guidelines](logging.md). Never swallow errors or log secrets in plaintext.
6. **Documentation Sync:** Update affected docs under `docs/` in the same change set (see [`AGENTS.md`](../../AGENTS.md#5-mandatory-documentation-maintenance-requirements)).

# CI/CD Workflows

[Docs Home](../README.md) / [Development](README.md) / CI/CD

---

## Pull Request & Main Branch CI (`.github/workflows/ci.yml`)

Runs three parallel jobs on every push and PR to `main`:

1. **`Verify (macOS)` (`macos-15`):** `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, release builds of `shellumi-app` and `shellumi-cli`.
2. **`Verify (Linux)` (`ubuntu-latest`):** Installs GUI/audio system deps, validates `packaging/linux/shellumi.desktop` (`desktop-file-validate` + `Terminal=false`), runs `fmt`, `clippy`, `test`, and release builds.
3. **`Verify (Windows)` (`windows-latest`):** Runs `fmt`, `clippy`, `test`, release builds, and verifies PE subsystems (`shellumi-app.exe` = `WINDOWS_GUI` `2`, `shellumi-cli.exe` = `WINDOWS_CUI` `3`).

## Documentation Verification (`.github/workflows/docs.yml` & `external-links.yml`)

- **Required Docs Workflow (`.github/workflows/docs.yml`):** Runs on documentation/asset/validator changes, installs pinned `markdown-it-py==3.0.0` and `scripts/docs-tools/` (`markdownlint@0.38.0`, `mermaid@11.6.0`), executes all 21 regression tests (`./scripts/test-verify-docs.sh`), and runs `./scripts/verify-docs.sh` (Markdown AST, HTML links/alt, exact case, symlink escape, anchors, Mermaid parsing, secret scanning, env-var drift). Keeps Rust application CI completely independent.
- **Advisory External Link Workflow (`.github/workflows/external-links.yml`):** Scheduled weekly (plus `workflow_dispatch`) with 10s bounded timeouts so third-party network flakiness never blocks PRs or releases.

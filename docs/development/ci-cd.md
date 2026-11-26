# CI/CD Workflows

[Docs Home](../README.md) / [Development](README.md) / CI/CD

---

## Pull Request & Main Branch CI (`.github/workflows/ci.yml`)

Runs three parallel jobs on every push and PR to `main`:

1. **`Verify (macOS)` (`macos-15`):** `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, release builds of `shellumi-app` and `shellumi-cli`.
2. **`Verify (Linux)` (`ubuntu-latest`):** Installs GUI/audio system deps, validates `packaging/linux/shellumi.desktop` (`desktop-file-validate` + `Terminal=false`), runs `fmt`, `clippy`, `test`, and release builds.
3. **`Verify (Windows)` (`windows-latest`):** Runs `fmt`, `clippy`, `test`, release builds, and verifies PE subsystems (`shellumi-app.exe` = `WINDOWS_GUI` `2`, `shellumi-cli.exe` = `WINDOWS_CUI` `3`).

## Documentation Verification

`./scripts/verify-docs.sh` (and `.github/workflows/docs.yml`) validates relative links, image references, directory indexes, unresolved placeholders, Mermaid block hygiene, and accidental secret patterns offline without network dependencies.

# Testing Guide

[Docs Home](../README.md) / [Development](README.md) / Testing

---

## Standard Workspace Suite

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

## Focused Crate Tests

```sh
cargo test --locked --offline -p shellumi-storage
cargo test --locked --offline -p shellumi-app connection::tests
cargo test --locked --offline -p shellumi-app actions::remote_desktop::tests
cargo test --locked --offline -p shellumi-app ui::inspector::
```

## Opt-In OpenSSH / SFTP Acceptance Tests

Isolated loopback `sshd` tests are marked `#[ignore]` in the default suite. See [SSH Integration Tests](ssh-integration.md) for setup and execution commands.

## Documentation Verification

Run the local documentation regression tests and full validator before committing doc changes:

```sh
./scripts/test-verify-docs.sh
./scripts/verify-docs.sh
```

- **Local Offline Engine:** Uses `python3` with `markdown-it-py==3.0.0`, `html.parser`, exact-case/symlink checks, table/heading linting, Mermaid grammar checks, and `SHELLUMI_*` env-var drift detection.
- **Full CI Engine (`.github/workflows/docs.yml`):** Additionally installs pinned Node packages in `scripts/docs-tools/` (`markdownlint@0.38.0` + `mermaid@11.6.0` with `jsdom`) and runs `scripts/docs-tools/lint-and-mermaid.mjs`.
- **Scoped Exceptions:** Narrowly scoped HTML allowances live in `scripts/docs-tools/.markdownlint.json`; archive replacement links are validated in the `Historical Archive Record` header block.

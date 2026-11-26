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

Run the local documentation verifier before committing doc changes:

```sh
./scripts/verify-docs.sh
```

# Build and Run

[Docs Home](../README.md) / [Development](README.md) / Build and Run

---

## Build Workspace

```sh
cargo build --locked --workspace
```

## Run Desktop Application

```sh
cargo run --locked -p shellumi-app
```

With verbose diagnostics logging:

```sh
SHELLUMI_LOG=info,shellumi_storage=debug cargo run --locked -p shellumi-app
```

## Run CLI

```sh
cargo run --locked -p shellumi-cli -- doctor --markdown
```

## Build Release Binaries

```sh
cargo build --locked --release -p shellumi-app
cargo build --locked --release -p shellumi-cli
```

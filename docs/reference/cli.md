# CLI Reference (`shellumi-cli`)

[Docs Home](../README.md) / [Reference](README.md) / CLI Reference

---

Derived directly from `crates/shellumi-cli/src/main.rs`.

## Host, Identity, Group & Tunnel Commands

```sh
shellumi-cli <database-path> list [--group <group>] [--tag <tag>] [--search <query>]
shellumi-cli <database-path> show <id>
shellumi-cli <database-path> add <name> <hostname> <username> <port> [--group <group>] [--tags <tag1,tag2>] [--jump-host <jump>] [--startup-snippet <id>] [--protocol <ssh|mosh|telnet|serial>] [--protocol-config <config>] [--identity-id <id>]
shellumi-cli <database-path> update <id> <name> <hostname> <username> <port> [...]
shellumi-cli <database-path> delete <id>

shellumi-cli <database-path> identity <add|list|show|delete> ...
shellumi-cli <database-path> group <add|list|show|delete> ...
shellumi-cli <database-path> tunnel <add|list|start|delete> ...
```

## SFTP, Snippets, Workspaces & Sessions

```sh
shellumi-cli <database-path> sftp <local-ls|remote-ls|get|put|history|clear-history|log|packet|queue> ...
shellumi-cli <database-path> snippet <list|add|show|render|batch|delete> ...
shellumi-cli <database-path> workspace <add|list|show|duplicate|delete> ...
shellumi-cli <database-path> session <list|show|play|export|import|delete> ...
```

## Vault, Team & Infrastructure Commands

```sh
shellumi-cli <database-path> vault <export|import|diff|history|rollback|status|sync|cloud-sync|oauth-url> ...
shellumi-cli <database-path> team <vault|member|audit|session> ...
shellumi-cli <database-path> infra <provider|discover|ansible> ...
```

## Connection Dispatch & Diagnostics

```sh
shellumi-cli <database-path> connect <id>
shellumi-cli <database-path> quick-connect <user@host[:port]>
shellumi-cli <database-path> mosh <user@host[:port]> [--ssh-port <port>] [--predict <adaptive|always|never|experimental>]
shellumi-cli <database-path> telnet <host> [port]
shellumi-cli <database-path> serial <port> [--baud <baud>]
shellumi-cli serial-ports
shellumi-cli <database-path> <import-ssh-config|export-ssh-config|import-csv|export-csv> ...
shellumi-cli keygen [--comment <comment>]
shellumi-cli doctor [--json] [--markdown] [--db <path>]
shellumi-cli bench [--json] [--markdown] [--category <category>]
```

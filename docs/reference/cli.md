# CLI Reference (`shellumi-cli`)

[Docs Home](../README.md) / [Reference](README.md) / CLI Reference

---

`shellumi-cli` provides scriptable host, identity, tunnel, SFTP, snippet, workspace, session recording, vault sync, team, infrastructure, and diagnostics management using the same `shellumi-core` and `shellumi-storage` engines as the desktop application.

> [!CAUTION]
> **Process-List Secret Visibility (`SEC-001`):**
> Passing secrets via `--password`, `--passphrase`, `--token`, or `keychain set ... <secret>` places those values in process command-line arguments visible to local users via `ps` / Process Explorer. Where supported, prefer environment variables such as `SHELLUMI_VAULT_PASSPHRASE` or `SHELLUMI_GDRIVE_TOKEN` instead of CLI flags.

Most commands accept `<database-path>` as their first positional argument (e.g., `~/.local/share/shellumi/vaults/default.db` or `~/Library/Application Support/Shellumi/vaults/default.db`). Global utility commands (`serial-ports`, `keygen`, `doctor`, `bench`, `mobile`, `complete`, `prompt-detect`, `shell-hook`, `keychain`) operate without a `<database-path>` (with optional `--db` / DB-scoped variants where shown).

---

## 1. Host Management

```text
shellumi-cli <database-path> list [--group <group>] [--tag <tag>] [--search <query>]
shellumi-cli <database-path> show <id>
shellumi-cli <database-path> add <name> <hostname> <username> <port> [--group <group>] [--tags <tag1,tag2>] [--jump-host <jump>] [--startup-snippet <id>] [--protocol <ssh|mosh|telnet|serial>] [--protocol-config <config>] [--identity-id <id>]
shellumi-cli <database-path> update <id> <name> <hostname> <username> <port> [--group <group>] [--tags <tag1,tag2>] [--jump-host <jump>] [--startup-snippet <id>] [--protocol <ssh|mosh|telnet|serial>] [--protocol-config <config>] [--identity-id <id>]
shellumi-cli <database-path> delete <id>
```

- **Defaults & Values:** `--protocol` accepts `ssh`, `mosh`, `telnet`, `serial` (defaults to `ssh`). `--tags` is comma-separated.
- **Example:**
  ```bash
  shellumi-cli ~/vault.db add prod-web 203.0.113.10 deploy 22 --group production --tags web,eu
  shellumi-cli ~/vault.db list --group production
  ```

## 2. Identities

```text
shellumi-cli <database-path> identity add <name> [--username <user>] [--auth <password|publickey|agent|certificate>] [--key-path <path>] [--cert-path <path>] [--password <secret>] [--passphrase <secret>] [--password-ref <uri>] [--passphrase-ref <uri>] [--tags <tag1,tag2>] [--comment <comment>]
shellumi-cli <database-path> identity list [--tag <tag>]
shellumi-cli <database-path> identity show <id>
shellumi-cli <database-path> identity delete <id>
```

- **Auth Modes:** `password`, `publickey`, `agent`, `certificate`.
- **Note:** Prefer `--password-ref` / `--passphrase-ref` (`keychain://...`) over `--password` / `--passphrase` (`SEC-001`).

## 3. Groups

```text
shellumi-cli <database-path> group add <name> [--username <user>] [--port <port>] [--tags <tag1,tag2>] [--identity-id <id>]
shellumi-cli <database-path> group list
shellumi-cli <database-path> group show <name>
shellumi-cli <database-path> group delete <name>
```

## 4. Port Forwarding Tunnels

```text
shellumi-cli <database-path> tunnel add <name> --type <local|remote|dynamic> --bind-port <port> [--bind-address <addr>] [--target-host <host>] [--target-port <port>] [--host-id <id>] [--auto-start]
shellumi-cli <database-path> tunnel list [--host-id <id>]
shellumi-cli <database-path> tunnel start <id> [--dry-run]
shellumi-cli <database-path> tunnel delete <id>
```

## 5. SFTP & Transfer Management

```text
shellumi-cli <database-path> sftp local-ls <path> [--all]
shellumi-cli <database-path> sftp remote-ls <destination> [path]
shellumi-cli <database-path> sftp get <destination> <remote-path> <local-path>
shellumi-cli <database-path> sftp put <destination> <local-path> <remote-path>
shellumi-cli <database-path> sftp history [--host-id <id>] [--limit <n>]
shellumi-cli <database-path> sftp clear-history [--host-id <id>]
shellumi-cli <database-path> sftp log <upload|download> <local-path> <remote-path> <bytes> [--total-bytes <bytes>] [--status <status>] [--host-id <id>]
shellumi-cli <database-path> sftp packet <encode-init|decode|encode-open> [...]
shellumi-cli <database-path> sftp queue <upload|download> <local-path> <remote-path> <bytes>
```

- `<destination>` accepts `user@host[:port]` or a saved host identifier.

## 6. Snippets & Batch Execution

```text
shellumi-cli <database-path> snippet list [--tag <tag>] [--search <query>]
shellumi-cli <database-path> snippet add <name> <command> [--description <desc>] [--tags <tag1,tag2>]
shellumi-cli <database-path> snippet show <id>
shellumi-cli <database-path> snippet render <id> [--var key=value] [--preview]
shellumi-cli <database-path> snippet batch <id> --hosts <host1,host2,...> [--var key=value] [--policy <seq|par>] [--concurrency <n>] [--dry-run]
shellumi-cli <database-path> snippet delete <id>
```

- `--policy` accepts `seq` (sequential) or `par` (parallel, bounded by `--concurrency`). Use `--dry-run` to inspect rendered commands before execution.

## 7. Workspaces & Command Palette

```text
shellumi-cli <database-path> workspace add <name> [--layout <single|vertical|horizontal|grid>] [--desc <description>] [--tags <tag1,tag2>]
shellumi-cli <database-path> workspace list
shellumi-cli <database-path> workspace show <id>
shellumi-cli <database-path> workspace duplicate <id> <new-name>
shellumi-cli <database-path> workspace delete <id>
shellumi-cli <database-path> palette search <query>
```

## 8. Session Recording & Playback

```text
shellumi-cli <database-path> session list [--host-id <id>]
shellumi-cli <database-path> session show <id>
shellumi-cli <database-path> session play <id> [--speed <multiplier>] [--raw]
shellumi-cli <database-path> session export <id> [--out <path>]
shellumi-cli <database-path> session import <cast-file> --title <title> [--host-id <id>] [--redact]
shellumi-cli <database-path> session delete <id>
```

## 9. Vault Export, Import, History & Cloud Sync

```text
shellumi-cli <database-path> vault export <output-path> --passphrase <pass> [--device-id <id>] [--iterations <n>]
shellumi-cli <database-path> vault import <input-path> --passphrase <pass>
shellumi-cli <database-path> vault diff <input-path> --passphrase <pass>
shellumi-cli <database-path> vault history
shellumi-cli <database-path> vault rollback <snapshot-id> --passphrase <pass>
shellumi-cli <database-path> vault status
shellumi-cli <database-path> vault sync [--remote <path>] --passphrase <pass> [--device-id <id>] [--dry-run]
shellumi-cli <database-path> vault cloud-sync --token <oauth-token> --passphrase <pass> [--resolve <keep-local|accept-remote>]
shellumi-cli <database-path> vault oauth-url [--client-id <id>] [--redirect-uri <uri>]
```

- `--resolve` accepts `keep-local` or `accept-remote` when resolving sync conflicts.

## 10. Team Vaults, Members & Collaboration Sessions

```text
shellumi-cli <database-path> team vault create <name> [--desc <desc>] [--owner <owner-id>]
shellumi-cli <database-path> team vault list
shellumi-cli <database-path> team vault show <id>
shellumi-cli <database-path> team vault rotate-key <id>
shellumi-cli <database-path> team vault delete <id>
shellumi-cli <database-path> team member invite <vault-id> <user-id> <name> <role> [--public-key <key>] [--invited-by <by>]
shellumi-cli <database-path> team member list <vault-id>
shellumi-cli <database-path> team member role <vault-id> <member-id> <new-role>
shellumi-cli <database-path> team member revoke <vault-id> <member-id>
shellumi-cli <database-path> team audit <vault-id> [--limit <n>]
shellumi-cli <database-path> team session start <session-id> <owner-id> [--vault-id <id>]
shellumi-cli <database-path> team session join <session-id> <user-id> <name>
shellumi-cli <database-path> team session grant <session-id> <requester-id> <target-user-id>
shellumi-cli <database-path> team session revoke <session-id> <requester-id>
shellumi-cli <database-path> team session status <session-id>
```

## 11. Infrastructure Providers & Discovery

```text
shellumi-cli <database-path> infra provider add <kind> --name <name> [--region <r>] [--profile <p>] [--token <t>] [--path <path>] [--format <ini|yaml>]
shellumi-cli <database-path> infra provider list
shellumi-cli <database-path> infra provider show <id>
shellumi-cli <database-path> infra provider delete <id>
shellumi-cli <database-path> infra provider sync <id>
shellumi-cli <database-path> infra discover <aws|azure|digitalocean|ansible> --file <file> [--private-ip] [--import-all]
shellumi-cli <database-path> infra ansible import --file <path> [--format <ini|yaml>]
shellumi-cli <database-path> infra ansible export [--format <ini|yaml>] [--output <path>]
```

## 12. Connection Dispatch & Config Import/Export

```text
shellumi-cli <database-path> connect <id>
shellumi-cli <database-path> quick-connect <user@host[:port]>
shellumi-cli <database-path> mosh <user@host[:port]> [--ssh-port <port>] [--predict <adaptive|always|never|experimental>]
shellumi-cli <database-path> telnet <host> [port]
shellumi-cli <database-path> serial <port> [--baud <baud>]
shellumi-cli serial-ports
shellumi-cli <database-path> import-ssh-config <config-path>
shellumi-cli <database-path> export-ssh-config
shellumi-cli <database-path> import-csv <csv-path>
shellumi-cli <database-path> export-csv
```

## 13. Keygen, Diagnostics, Benchmarks, Mobile, Shell Hooks & Keychain

```text
shellumi-cli keygen [--comment <comment>]
shellumi-cli doctor [--json] [--markdown] [--db <path>]
shellumi-cli bench [--json] [--markdown] [--category <category>]
shellumi-cli mobile <virtual-keys|lifecycle-sim|ffi-check|header|build-info>
shellumi-cli complete <query> [--dir <dir>]
shellumi-cli <database-path> complete <query>
shellumi-cli prompt-detect "<prompt-line>"
shellumi-cli shell-hook <bash|zsh|fish>
shellumi-cli keychain status [--json]
shellumi-cli keychain set <service> <account> <secret>
shellumi-cli keychain get <service> <account>
shellumi-cli keychain delete <service> <account>
shellumi-cli <database-path> keychain <status|set|get|delete>
```

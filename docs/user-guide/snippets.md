# <img src="../../resources/icon_pack/icons/48/regular/snippets.png" width="24" height="24" alt="Snippets"> Snippets & Batch Automation

[Docs Home](../README.md) / [User Guide](README.md) / Snippets

---

Snippets are reusable, parameterized command templates stored inside your encrypted vault (`SavedSnippet`). They can be run in an active terminal, pasted without executing, attached as a host **Startup Snippet**, or executed across multiple hosts via `BatchExecutionPlan`.

## Prerequisites & Access

- **GUI:** Open **Snippets** from the left navigation rail to create, edit, search, or delete snippets in the right-hand **Inspector**.
- **CLI:** Use `shellumi-cli <database-path> snippet ...` for rendering, previews, and sequential/parallel batch execution.

---

## Step-by-Step: Creating & Parameterizing Snippets

1. Click **New Snippet** in **Snippets**.
2. Enter **Name** (e.g., `check-service`), **Command** (e.g., `systemctl status {{service}}`), optional **Description**, and **Tags**.
3. Click **Save**.
4. To run on SSH connect, open a host in **Hosts → Edit** and select the snippet under **Startup Snippet**.

### Template Variables & Safety

- Explicit `{{variable}}` placeholders let you supply parameters at execution time (`--var key=value`).
- Always preview rendered commands (`--preview` or `--dry-run`) before running across production hosts.
- **Important Redaction Limit:** While Shellumi avoids logging secret input parameters in plaintext, **remote shell output is not guaranteed to be fully redacted** if a command echoes secrets back to stdout/stderr.

---

## Fictional Examples (CLI)

```bash
# Create a parameterized snippet
shellumi-cli ~/vault.db snippet add check-service "systemctl status {{service}}" --description "Check systemd unit" --tags ops

# Preview rendered command without executing
shellumi-cli ~/vault.db snippet render <snippet-id> --var service=nginx --preview

# Dry-run parallel batch execution across two hosts (max concurrency 2)
shellumi-cli ~/vault.db snippet batch <snippet-id> --hosts web-1,web-2 --var service=nginx --policy par --concurrency 2 --dry-run
```

- `--policy` accepts `seq` (sequential) or `par` (parallel, bounded by `--concurrency`).
- Batch execution supports cancellation before or between host runs.

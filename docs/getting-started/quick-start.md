[Documentation Home](../README.md) > [Getting Started](README.md) > **Quick Start**

# Quick Start

Once your vault is unlocked, you can connect to a remote host in under a minute using either the desktop GUI or `shellumi-cli`.

## Connect via Desktop GUI

1. Open the **Hosts** section from the left navigation rail.
2. Click **Add Host** to open the right-hand **Inspector**.
3. Enter a **Label**, **Hostname / IP**, **Port** (default `22`), and **Username**.
4. Choose or create a reusable **Identity** (SSH key, password, or SSH agent) and click **Save**.
5. Double-click the host (or click **Connect**) to open an interactive terminal tab.
6. Verify and accept the server's **TOFU host-key fingerprint** on first connection.

## Connect via CLI (`shellumi-cli`)

```bash
# Add a host
shellumi-cli add --name prod-web --host 203.0.113.10 --port 22 --user deploy

# List saved hosts
shellumi-cli list

# Connect interactively
shellumi-cli connect prod-web
```

See [CLI Reference](../reference/cli.md) for all subcommands and flags.

## Next Steps

- Explore [Hosts & Groups](../user-guide/hosts-and-groups.md), [SFTP](../user-guide/sftp.md), and [Remote Desktop](../user-guide/remote-desktop.md).
- Review [Keyboard Shortcuts](../reference/keyboard-shortcuts.md).

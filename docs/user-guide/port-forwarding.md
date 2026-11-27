# <img src="../../resources/icon_pack/icons/48/regular/port_forwarding.png" width="24" height="24" alt="Port Forwarding"> Port Forwarding & SSH Tunnels

[Docs Home](../README.md) / [User Guide](README.md) / Port Forwarding

---

Shellumi manages **Local (`-L`)**, **Remote (`-R`)**, and **Dynamic SOCKS (`-D`)** SSH port forwarding profiles through the Unified Inspector and `shellumi-cli`, dispatching via the system `ssh` binary.

## Prerequisites & Access

- At least one saved **SSH Host** with valid authentication in the current vault.
- Open **Port Forwarding** from the left navigation rail (or `Cmd+Shift+P` / `Ctrl+Shift+P`).

---

## Forwarding Modes

| Mode | SSH Flag | Direction | Typical Use Case |
| --- | --- | --- | --- |
| **Local (`local`)** | `-L` | Local bind port → SSH host → Remote target | Access an internal DB (`127.0.0.1:15432` → `db.internal:5432`) |
| **Remote (`remote`)** | `-R` | Remote SSH listener → Local workstation target | Expose a local dev service (`localhost:8080`) on a remote server |
| **Dynamic (`dynamic`)** | `-D` | Local SOCKS5 listener → SSH host | Route browser/CLI traffic through a bastion |

---

## Step-by-Step: Creating, Starting & Stopping a Tunnel

1. Click **New Tunnel** in **Port Forwarding**.
2. Enter **Name** and choose **Type** (`local`, `remote`, or `dynamic`).
3. Set **Bind Address** (defaults to `127.0.0.1`) and **Bind Port** (`1–65535`).
4. For `local` or `remote`, enter **Target Host** and **Target Port**.
5. Select the **SSH Host** (and its configured identity/credentials) and optionally toggle **Auto-Start**, then click **Save**.
6. Click **Start** to launch the tunnel subprocess, or **Stop** / **Delete** to terminate and remove it.

### Fictional Examples (CLI)

```bash
# Local forward: local 15432 -> db.internal:5432 via bastion host ID
shellumi-cli ~/vault.db tunnel add pg-prod --type local --bind-address 127.0.0.1 --bind-port 15432 --target-host db.internal --target-port 5432 --host-id <host-id>

# Remote forward: remote 9000 -> localhost:3000
shellumi-cli ~/vault.db tunnel add webhook-dev --type remote --bind-port 9000 --target-host 127.0.0.1 --target-port 3000 --host-id <host-id>

# Dynamic SOCKS5 proxy on local port 1080
shellumi-cli ~/vault.db tunnel add socks-eu --type dynamic --bind-address 127.0.0.1 --bind-port 1080 --host-id <host-id>

# Preview exact SSH command without spawning
shellumi-cli ~/vault.db tunnel start <tunnel-id> --dry-run
```

---

## Status, Security Warnings & Troubleshooting

- **Checking Running State:** The Port Forwarding view shows active/stopped process status for tunnels started in the current session. Note: Shellumi does **not** currently display live byte-throughput or packet telemetry for SSH tunnels.
- **Loopback vs. Non-Localhost Binding:** Binding to `0.0.0.0` or a non-loopback address exposes the forwarded port to your network. Shellumi warns explicitly before exposing services beyond `127.0.0.1`.
- **Common Failures:**
  - *Port Already in Use:* Another local process (or another tunnel) is bound to the same `Bind Port`; choose an unused port.
  - *Remote Forwarding Refused:* Ensure `AllowTcpForwarding yes` (and `GatewayPorts` if non-loopback) is enabled in `/etc/ssh/sshd_config`.
  - *Authentication Failure:* Verify the associated SSH host's credentials in [Identities & Keychain](identities-and-keychain.md).

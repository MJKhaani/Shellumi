# <img src="../../resources/icon_pack/icons/48/regular/hosts.png" width="24" height="24" alt="Hosts"> Hosts & Groups

[Docs Home](../README.md) / [User Guide](README.md) / Hosts & Groups

---

Shellumi organizes every remote endpoint—SSH, Mosh, Telnet, Serial, RDP, and VNC—into vault-scoped host profiles and hierarchical groups managed through the **Unified Right-Hand Inspector**.

## Prerequisites & Access

- An unlocked local or remote vault.
- Open **Hosts** from the left navigation rail (or press the Hosts shortcut in [Keyboard Shortcuts](../reference/keyboard-shortcuts.md)).
- Click **New Host** or **New Group** to open the right-hand Inspector, or click any existing host/group to view and edit it.

---

## Supported Protocols & Default Ports

| Protocol | Default Port | Key Configuration Fields |
| --- | --- | --- |
| **SSH** | `22` | Hostname/IPv4/IPv6, Port, Username, Auth (`Existing Identity`, `Enter Credentials`, `Ask on Connect`), Jump Host, Startup Snippet |
| **Mosh** | `22` (SSH bootstrap) + UDP `60000–61000` | Hostname/IP, SSH Port, Username, Prediction Mode (`adaptive`, `always`, `never`, `experimental`) |
| **Telnet** | `23` | Hostname/IP, Port (displays prominent **cleartext / unencrypted** warning) |
| **Serial** | N/A | Serial Device Path (e.g. `/dev/ttyUSB0`, `COM3`), Baud Rate (`9600`, `115200`, etc.) — hides network & remote credential tabs |
| **RDP** | `3389` | Hostname/IP, Port, Username, Domain (optional), Resolution, Color Depth, View-Only, Strict TLS, Audio (`RDPSND`), Microphone (`MS-RDPEAI`), Shared Drives (`RDPDR`) |
| **VNC** | `5900` | Hostname/IP, Port, Optional Username, Password / No-Auth, Resolution, Color Depth, View-Only |

> **Address Formats:** Shellumi validates DNS hostnames, IPv4 addresses (`203.0.113.10`), and IPv6 addresses (`2001:db8::10` or `[2001:db8::10]`), rejecting whitespace, control characters, and shell metacharacters.

---

## Step-by-Step: Creating a Host

1. Click **New Host** in the **Hosts** view.
2. Enter a **Label** (display name, e.g. `prod-api-01`) and choose the **Protocol**.
3. Enter **Hostname / IP** and verify or customize **Port**.
4. Optionally assign a **Group**, **Tags** (comma-separated, e.g. `prod,eu-west`), **Jump Host**, or **Startup Snippet**.
5. Under **Auth**, choose your credential source:
   - **Existing Identity:** Select a protocol-compatible reusable identity from the current vault.
   - **Enter Credentials → Save as Identity:** Create a new reusable identity inline (saved atomically with the host).
   - **Enter Credentials → Save for This Host:** Store a host-specific credential reference in the current vault without adding a reusable identity.
   - **Ask on Connect / Session Only:** Prompt at connection time without storing secrets.
6. Click **Save** in the Inspector footer.

### Fictional Protocol Examples

- **SSH:** Label `edge-gw`, Protocol `SSH`, Host `203.0.113.42`, Port `2222`, Username `ops`, Jump Host `bastion-eu`.
- **RDP:** Label `win-admin-01`, Protocol `RDP`, Host `198.51.100.25`, Port `3389`, Username `Administrator`, Domain `CORP`, Strict TLS enabled.
- **VNC:** Label `mac-build-02`, Protocol `VNC`, Host `192.0.2.88`, Port `5900`, Security `Auto-Negotiate` / `VncAuth`.
- **Mosh:** Label `roam-dev`, Protocol `Mosh`, Host `dev.example.invalid`, SSH Port `22`, Predict `adaptive`.
- **Serial:** Label `switch-console`, Protocol `Serial`, Device `/dev/tty.usbserial-0001` (macOS) or `COM3` (Windows), Baud `115200`.

---

## Groups, Inheritance, Tags & Organization

- **Creating & Selecting Groups:** Click **New Group** to define shared defaults (Username, Port, Identity, Tags). Select a group in the sidebar/list to filter hosts or edit group defaults in the Inspector.
- **Configuration Inheritance:** Hosts inside a group inherit unset fields (username, port, identity) from their parent group while displaying per-host overrides clearly.
- **Tags, Search & Sorting:** Filter hosts by group, tag, or search query in the Hosts toolbar.
- **Duplicating & Moving Hosts:** Duplicate an existing host to clone its protocol settings, or reassign its **Group** in the Inspector. Moving or copying hosts across *different* vaults requires an explicit cross-vault action so credentials never leak silently.

---

## Importing & Exporting Configurations

Use `shellumi-cli` (or the Hosts import/export actions) to migrate inventories:

```bash
# Import OpenSSH config or CSV into a vault database
shellumi-cli ~/vault.db import-ssh-config ~/.ssh/config
shellumi-cli ~/vault.db import-csv hosts.csv

# Export non-secret host definitions
shellumi-cli ~/vault.db export-ssh-config
shellumi-cli ~/vault.db export-csv
```

> **Security Note:** Plaintext private keys and passwords are never exported by `export-ssh-config` or `export-csv`. Encrypted full-vault export uses `vault export` (see [Known Issues](../project/known-issues.md#sync-004--full-vault-exportimport-does-not-consistently-preserve-stable-object-associations)).

---

## Unsaved-Change Protection & Troubleshooting

- **Dirty Edit Guard:** If you modify fields in the Inspector and click another host, switch tabs, switch vaults, or close the window, Shellumi presents a **Save / Discard / Keep Editing** guard so edits are never silently lost.
- **Common Errors:**
  - *Invalid Port / Hostname:* Ports must be `1–65535`; hostnames cannot contain spaces or shell metacharacters.
  - *Incompatible Identity:* Selecting an identity whose auth type is unsupported by the chosen protocol (e.g., SSH key on RDP) is blocked by `ProtocolCapabilities`.
  - *Unresolved Sync Dependency (`SYNC-003`):* If a synced host's identity is missing locally, authentication fails closed until you assign a valid credential.

## Related Documentation

- [Identities & Keychain](identities-and-keychain.md) · [Terminal](terminal.md) · [Remote Desktop](remote-desktop.md) · [CLI Reference](../reference/cli.md)

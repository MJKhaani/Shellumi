# <img src="../../resources/icon_pack/icons/48/regular/hosts.png" width="24" height="24" alt="Hosts"> Hosts and Groups

[Docs Home](../README.md) / [User Guide](README.md) / Hosts and Groups

---

## Creating and Editing Hosts

1. Click **New Host** (`Cmd+N` / `Ctrl+N`).
2. In the **Host Inspector**, configure:
   - **General:** Display name, hostname/IP (including IPv6), port, protocol (`SSH`, `Mosh`, `RDP`, `VNC`, `Telnet`, `Serial`, `Local`), group, tags, jump host, and optional startup snippet.
   - **Auth:** Protocol-appropriate authentication source (Existing Identity, Save as Identity, Save for This Host, Inherit from Group, Ask on Connect, or No Authentication where allowed).
   - **Protocol Options:** RDP/VNC display & TLS settings, Mosh predict/UDP port, or Serial baud/framing options.
3. Click **Save**.

## Groups & Filtering

- **Grid / List View:** Toggle between card grid and table list views in the Hosts toolbar.
- **Filter & Sort:** Filter by group, tag, or search query, and sort by name (`A–Z`, `Z–A`) or creation order.
- **Group Defaults:** Click a group card to open the Group Inspector and set a default username, default port, or default identity for hosts using **Inherit from Group**.
- **OpenSSH Config Import/Export:** Use the Hosts toolbar **Import** / **Export** actions (or `shellumi-cli import-ssh-config` / `export-ssh-config`) to transfer host entries.

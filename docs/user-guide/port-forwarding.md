# <img src="../../resources/icon_pack/icons/48/regular/port_forwarding.png" width="24" height="24" alt="Port Forwarding"> Port Forwarding

[Docs Home](../README.md) / [User Guide](README.md) / Port Forwarding

---

Open **Port Forwarding** (`Cmd+Shift+P` / `Ctrl+Shift+P`) to define and launch SSH tunnels:

- **Local (`-L`):** Forward a local bind address/port through an SSH host to a target host/port.
- **Remote (`-R`):** Expose a local service on a remote SSH server port.
- **Dynamic (`-D`):** Start a local SOCKS5 proxy through the selected SSH host.

Tunnels launch real `ssh` processes using the selected host's identity, host-key policy, and askpass environment.

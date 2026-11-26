# Device Trust, Host Keys & TLS

[Docs Home](../README.md) / [Security](README.md) / Device Trust

---

- **SSH Host-Key Verification:** Shellumi never disables `StrictHostKeyChecking` or bypasses `known_hosts`. Unknown or changed keys require explicit user decision; `Known Hosts` revocation verifies the original entry and refuses symlinks.
- **RDP & VNC Certificate Pinning:** Untrusted TLS/VeNCrypt certificates trigger an interactive Trust-On-First-Use (TOFU) SHA-256 fingerprint confirmation dialog before completing connection setup.
- **Shellumi Server Device Identity:** Enrolled devices generate independent Ed25519 signing and X25519 key-agreement keys stored in the OS credential store, bound to `server_instance_id` and `server_vault_id`.
- **Command Execution Safety:** Subprocesses (`ssh`, `mosh`, `sftp`, `curl`) pass discrete argument vectors without shell evaluation, using `--` before target hostnames and piped stdin/askpass for secrets.

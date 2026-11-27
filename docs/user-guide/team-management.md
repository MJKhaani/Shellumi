# <img src="../../resources/icon_pack/icons/48/regular/team_members.png" width="24" height="24" alt="Team Management"> Team Management & Shared Vaults

[Docs Home](../README.md) / [User Guide](README.md) / Team Management

---

Shared vaults, role-based access control, X25519 DEK epoch rotation, and real-time multiplayer terminal collaboration require an optional self-hosted **[Shellumi Server](https://github.com/MJKhaani/Shellumi-Server)** deployment. Personal Google Drive sync does **not** support multi-user vault sharing.

> [!WARNING]
> **Verification & Scope Boundary (`TEAM-001`, `SYNC-001`..`005`):**
> Client-side team models, X25519 DEK wrapping, epoch rotation, and encrypted collaboration relay pass deterministic and controlled two-server fixture tests, **but live multi-client acceptance against an external deployed Shellumi Server remains `NOT TESTED`**. Do not conflate local domain models with verified live production team collaboration.

---

## Roles, Device Enrollment & Key Rotation

1. **Device Enrollment:** Each participating device enrolls an Ed25519 signing key + X25519 key-agreement key with Shellumi Server (`EnrolledDevice`) using journaled recovery (`Schema 30`).
2. **Roles:** `Owner`, `Administrator`, `Editor`, `Viewer`.
3. **Invitations & DEK Wrapping:** Inviting a member wraps the vault DEK for their enrolled device's X25519 public key (`wrap_dek_for_recipient` / `unwrap_dek_for_device`).
4. **Epoch Rotation & Revocation:** Rotating a team vault's epoch (`team vault rotate-key`) issues a new DEK wrapped only for active members (`STALE_EPOCH` rejects revoked devices). *Limitation:* Epoch rotation cannot erase secrets a revoked member already decrypted prior to revocation.
5. **Offline Limitations:** A disconnected local cache cannot observe server-side revocation until it reconnects to Shellumi Server.

## Real-Time Terminal Collaboration

- **Single-Writer Fencing:** Collaboration sessions (`MultiplayerSession`) enforce one active writer at a time via monotonic lease tokens; participants view E2EE PTY frames (`AES-256-GCM`), and the session owner can grant, transfer, or immediately revoke write control or terminate the session.

## CLI Operations

```bash
shellumi-cli ~/vault.db team vault create ops-shared --owner alice
shellumi-cli ~/vault.db team member invite <vault-id> bob "Bob" editor --public-key <x25519-pub>
shellumi-cli ~/vault.db team vault rotate-key <vault-id>
shellumi-cli ~/vault.db team audit <vault-id> --limit 50
shellumi-cli ~/vault.db team session start <session-id> alice --vault-id <vault-id>
```

## Related Documentation

- [Shellumi Server Sync](../synchronization/shellumi-server.md) · [Device Trust](../security/device-trust.md) · [Known Issues](../project/known-issues.md)

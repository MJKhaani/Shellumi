# <img src="../../resources/icon_pack/icons/48/regular/team_members.png" width="24" height="24" alt="Team Management"> Team Management

[Docs Home](../README.md) / [User Guide](README.md) / Team Management

---

When connected to a self-hosted [Shellumi Server](https://github.com/MJKhaani/Shellumi-Server), Shellumi supports shared team vaults, role-based access (`Owner`, `Administrator`, `Editor`, `Viewer`), invitations, device enrollment, and encrypted terminal collaboration.

- **Team Inspector:** View members, roles, enrolled devices, and invitation status.
- **Invitations:** Invitation tokens are held only in zeroizing memory for the originating vault and member and are cleared on navigation, lock, or expiration.
- **Device Enrollment & Key Replacement:** See [Device Enrollment](../synchronization/device-enrollment.md).

> [!NOTE]
> Team key rotation and epoch-key distribution remain gated behind live server transaction verification; see [Known Security Limitations](../security/known-limitations.md).

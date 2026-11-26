# <img src="../../resources/icon_pack/icons/48/regular/device_enrollment.png" width="24" height="24" alt="Device Enrollment"> Device Enrollment & Trust Lifecycle

[Docs Home](../README.md) / [Synchronization](README.md) / Device Enrollment

---

Shellumi Server vaults require explicit cryptographic device enrollment per server connection:

```mermaid
sequenceDiagram
    participant C as Shellumi Client
    participant S as Shellumi Server
    C->>C: Generate Ed25519 signing key + X25519 agreement key (OS Credential Store)
    C->>S: Request enrollment challenge (verified server_instance_id & vault binding)
    S-->>C: Challenge nonce
    C->>C: Sign canonical challenge with Ed25519 key
    C->>S: Submit signed challenge + public keys
    S-->>C: Assign Device ID + register authorized device
```

- **Key Generation & Storage:** Generates an Ed25519 signing keypair and X25519 key-agreement keypair; private keys are persisted in the OS credential store under `keychain://org.shellumi.server/...` references.
- **Crash-Safe Enrollment Journal (Schema 28–30):** Records intended public keys and credential-store references before registration (`RegistrationPrepared`), enabling restart recovery via authoritative server account/recipient listings without replaying duplicate registrations.
- **Device Replacement & Revocation:** Replacing keys verifies local vault authorization, active account registration, and vault recipient listings before authorizing the new device and offering confirmed global revocation of the old device.
- **Invitations:** Acceptance binds the selected vault, server instance UUID, endpoint, and authenticated user; invitation tokens stay in zeroizing memory only.

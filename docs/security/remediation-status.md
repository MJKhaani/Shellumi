# Security audit remediation status

Baseline `95cac92e34d865fb96f56701e5c2257c40cde730`. This change set contains interim safeguards. The release acceptance tests below remain open.

| Issue | Change | Required evidence still missing |
| --- | --- | --- |
| Password changes | Implemented transactional `vault_rotation` coordinator across manifest, target DB, registry DB, and OS credential store with exclusive locking, durable journaling, pre-validation, DEK preservation, staged rollback, startup reconciliation, and `submit_unlock_vault` recovery-required flow. Re-enabled password changes in Vault Inspector with current-password & confirmation inputs. Verified across 14 fault-injection points, rollback failure, corrupted journal, unavailable keychain, concurrent lock, and multi-vault isolation. | None for local multi-store rotation coordinator. |
| Team enrollment | Implemented explicit `EnrollmentStatus` (7 states), `EnrolledDevice` table (schema 25), `AuthorizedRecipientDevice` contract, `validate_x25519_agreement_public_key` (rejecting OpenSSH & non-contributory keys), and `HostStore` authenticated enrollment/multi-device/key-replacement/revocation/legacy-migration/sync methods. Verified with stateful cryptographic `HttpTransport` (Ed25519 challenge verification, X25519 DEK wrap+unwrap correspondence, multi-device, restart persistence, keychain failure, device revocation, legacy migration). | End-to-end integration test against a live Shellumi Server binary/database (local server repository unavailable in this workspace). |
| Epoch encryption | The legacy server synchronization path queries remote vault metadata and only proceeds for a personal vault at epoch one. The Team modal and Inspector block rotation and revocation. Shared mutations remain queued. | Production encryption and decryption with device-authorized epoch keys, historical reads, atomic rotation, and a running three-device denial and catch-up test. |
| Quarantine | Copy fallback compares the bytes before publishing the destination. An interrupted quarantine retains its journal and requires intervention before normal startup. | Authenticated registry reconciliation and encrypted SQLite/WAL integrity verification after each interruption. |
| Native icons | The full asset pack remains embedded. Direct image use is still present in the command palette, host popovers, and other views. | Parent-hover, focus, selected and disabled rendering, six-icon visual review, and packaged native asset loading. |

## Key hierarchy

The local encrypted database retains its random per-vault DEK. A password or recovery code unwraps that DEK through the vault manifest. Password changes must preserve the DEK and its authenticated revision history. Google Drive uses its existing encrypted snapshot and revision protocol.

A team content key belongs to one remote vault and one committed epoch. X25519 recipient-device keys protect copies of that content key. SSH public keys identify authentication credentials and provide no proof of agreement-key ownership. New team writes require the committed content key. The current guard rejects the legacy passphrase path for team vaults until the content-key path and server enforcement are verified.

## Verification scope

The restart regression retains a truncated journal and checks that both the original password and recovery code can unwrap the saved manifest. Quarantine fixtures check preservation of files across repeated recovery attempts. These tests do not authenticate encrypted SQLite records after a multi-store transaction.

The server sync containment test checks the last HTTP request, the retained mutation, and the unchanged sync cursor using a controlled transport. The existing desktop test named `real_server_verification` uses `SimulatedServerState`. Neither test establishes deployed server authorization.

In Shellumi Server revision `600cb93`, the rotation service calls `save_envelopes` and `update_vault_key_epoch` separately. Release verification requires a transaction that checks the expected epoch and recipient membership while committing both changes, followed by tests against persistent server storage.

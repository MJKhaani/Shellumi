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

## Issue 2 client follow-up

The client changes build on audited revision `9b620e7110c05e558b9c280dd15865e95380df1e`.

- Invitation acceptance, device enrollment, and key replacement require a registry binding. Before accepting an invitation, the client checks the selected vault, connection endpoint, server-instance UUID, authenticated user, and any existing member record. Registration requires membership in the expected remote vault and a role that matches the invitation response.
- Key replacement checks local vault authorization, active account registration, and the vault recipient list for the old device. The client checks the replacement's Ed25519 and X25519 keys against the account and recipient listings before local authorization and global revocation. An uncertain revocation returns both device identities for reconciliation.
- Schema 28 preserves schema 27 enrollment rows. New rows bind each operation to the local vault, remote UUID, server connection and instance, member, and authenticated user. The client records the intended public keys and a credential-store identifier before registration. Recovery matches those keys against both server listings and proves possession of both protected private keys. A single transaction commits the device, vault authorization, member, and completed journal stage.
- A verified invitation can resume before registration. After a registration request may have been sent, recovery requires one matching active registration. Missing, conflicting, or legacy evidence calls for manual intervention. Recovery of a replacement authorizes the new device and leaves global revocation of the old device to a separate confirmed action.
- The Team Inspector holds invitation tokens in zeroizing memory and displays them only for the originating vault, member, and generation. Copy rechecks the current scope and expiration. Navigation, dismissal, lock, and quit clear the invitation state. The client currently has no Shellumi Server sign-out action.

The client regression uses local databases and simulated HTTP responses. Deployed server behavior is outside this verification scope.


## Client enrollment, invitation, and global revocation follow-up

- Schema 29 rebuilds the active-operation index to include `RegistrationPrepared`. Schema 27 and 28 enrollment records retain their device IDs and state. New intents bind the verified server instance, connection, remote vault, local vault, authenticated account, member, and both device public keys. Credentials are stored under fresh references before the prepared intent can issue a challenge.
- A prepared operation can request a fresh challenge after restart. A compare-and-set transition records that registration may have been dispatched. Recovery then uses authoritative account and vault-recipient listings to reconcile the device without repeating the registration request. It verifies both private keys and commits the member, device, vault authorization, and journal together. Conflicting or missing evidence leaves the operation pending and preserves existing device credentials. A lost registration response with no authoritative matching device still requires operator reconciliation.
- Initial connections journal enrollment in the encrypted registry and attach the protected device references in the same transaction as enrollment completion. The desktop attempts recovery for unlocked vaults on load. Invitation tokens stay in memory and are bound to the verified server, account, vault, Inspector target, request generation, and expiry. A stale response is discarded, and the request and response DTOs omit tokens from debug output.
- Direct global revocation and key replacement both verify the authenticated account, server instance, local authorization, live vault membership, and matching device identity in both server listings. Replacement checks ownership again before deletion. A per-process guard serializes operations on the same server device across windows. Vault-local removal continues to affect only local vault authorization. The server must enforce authorization on the global deletion request itself.
- The current client protocol exposes creation, listing, and acceptance of invitations. It has no invitation-specific revocation endpoint or Shellumi Server sign-out action.

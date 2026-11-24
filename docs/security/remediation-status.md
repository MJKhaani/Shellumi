# Security audit remediation status

Baseline `95cac92e34d865fb96f56701e5c2257c40cde730`. This change set contains interim safeguards. The release acceptance tests below remain open.

| Issue | Change | Required evidence still missing |
| --- | --- | --- |
| Password changes | The Inspector rejects password changes before opening storage. Startup preserves every rotation journal, including an unreadable one, and displays a recovery error. | An authenticated four-store coordinator, encrypted-record verification, concurrent changes, and before/after failure injection across process restarts. |
| Team enrollment | Server device enrollment saves newly generated private keys in the credential store. The Team views identify member rows as pending or unverified. Key wrapping rejects non-contributory X25519 inputs. | Authenticated member-to-device binding, multi-device enrollment and replacement, and legacy-member migration through the application. |
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

# Initial threat model

This document is preliminary; no security-sensitive Shellumi subsystem has
been reviewed or approved for production. Revisit it with each design change.

| Threat | Required control | Current status |
| --- | --- | --- |
| Host-key substitution | Verify known keys; explicitly confirm unknown keys; reject changed or revoked keys | SSH and host-key verification are not implemented |
| Stolen local or synced data | OS-protected keys and authenticated encryption; no plaintext secrets or sync | Not implemented |
| Malicious terminal output | Bound resource use; gate clipboard, hyperlinks and unsafe control sequences | Not assessed |
| Untrusted host configuration | Validate persisted and imported records; pass process arguments separately | Core and storage validate destinations; import and process launching are not implemented |
| Malicious imports or filenames | Validate structure, size and paths; avoid shell interpretation | Not implemented |
| OAuth token theft or sync replay | Protected tokens, state validation, authenticated versioned records and rollback detection | Not designed |
| Unsafe automation | Preview and explicit confirmation; isolate secrets and validate substitution | Not implemented |
| Unauthorized collaboration | Authenticate participants, enforce roles and audit cryptographic protocol | Not designed |

Do not store credentials in configuration or logs. No security guarantee is
made for the current foundation, and cross-platform behavior remains untested.

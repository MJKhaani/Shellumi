> [!NOTE]
> **Historical Archive Record — Google Drive Revision Protocol**
> - **Original Path:** `docs/DRIVE_REVISION_PROTOCOL.md`
> - **Original Baseline Commit:** `3123db229050a588e75cdc60147b7e39a5d0e13c`
> - **Status:** Preserved verbatim as a historical specification.
> - **Authoritative Replacement:** [Google Drive Synchronization](../../synchronization/google-drive.md).

# Authenticated Google Drive revision protocol

Drive synchronization now treats records, the password-wrapped vault key, and the
encrypted vault directory as independent changes. A password change does not need
a record edit. The encrypted default registry remains authoritative for each
vault's explicit Drive opt-in. Secondary vaults have separate publication and
component histories; server synchronization retains its existing protocol.

## Authentication and ordering

Optional `cloud_state` version 1 contains independent `key` and `metadata`
histories. Each node binds its protocol version, vault ID, component name,
generation, sorted parent IDs and SHA-256 content hash with HMAC-SHA-256 using the
vault DEK. Node IDs hash the same domain-separated preimage. Histories must contain
all ancestors, have increasing generations and authenticate successfully. A higher
counter alone grants no authority. An accepted ancestor cannot replace a descendant.
Divergent password or directory changes produce explicit conflicts. Resolution
creates a signed merge node retaining both parents; neither branch disappears.

A publication authenticates the entire serialized snapshot, including record
ciphertexts and metadata, plus its parent publications and a cloud/local domain
flag. Local encrypted protocol caches retain accepted publication IDs and
component heads across restarts. Missing accepted history, wrong vault ownership,
unknown versions, incomplete listings and invalid authentication fail closed.

The directory includes supported registry discovery metadata and sync opt-in
settings. Device-local Keychain references and credential values are excluded.
Manifest-backed storage exports rebuild the directory from the authoritative
registry, including CLI exports. Directory ciphertext is reused when only
generated timestamps change, avoiding
false metadata edits. Concurrent directories are explicitly resolved as whole
versions; unlisted local registry entries are preserved during import.

## Create-only optimistic concurrency

Every update is a new immutable Drive file named for its vault. The client lists
all pages, authenticates the publication DAG, and bases the next publication on its
observed heads. It obtains an ID with `files.generateIds`, creates that exact ID,
and verifies the full snapshot by reading it back. Google documents repeated
creates using a generated ID as HTTP 409. The request also includes
`If-None-Match: *`, but correctness does **not** depend on undocumented media-update
ETag behavior. No existing cloud object is patched, overwritten or deleted.

Sources: [generated IDs](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/generateIds)
and [create-file retry guidance](https://developers.google.com/workspace/drive/api/guides/create-file).

A lost acknowledgment is accepted only after exact read-back. Failed creates can
be retried without deleting history. After publication, the client lists again;
a racing divergent component is reported as a conflict. A publication arriving
after that read is discovered next cycle. This is optimistic branch reconciliation,
not a globally serialized transaction: concurrent successes can temporarily leave
multiple retained heads. They cannot destroy another writer's envelope. Conflict
choices in the desktop are bound to the reviewed local contents and cloud heads;
changed inputs require a fresh review.

## Password changes and recovery

Portable manifests carry the authenticated key history. Changing a password
appends a node and atomically saves the manifest with an exclusive writer lock;
stale or divergent manifest saves are rejected. The next sync detects this change
independently of record versions. Imported envelope revisions are authenticated
and checked against local high-water marks before manifest replacement.

Published record payloads use AES-256-GCM under the DEK, with vault, entity type
and object ID in associated data. Password rotation only rewraps the DEK; it does
not require the old password to decrypt published records. No plaintext DEK,
password, recovery code or Keychain secret enters a snapshot or revision history.
A fresh device needs only the current snapshot and its current password; existing
database and manifest destinations are refused.

## Compatibility and practical limits

Snapshots without `cloud_state` remain readable. Their full snapshot digest serves
as an immutable legacy parent; deterministic authenticated component roots are
created on upgrade, without replacing the original backup. Legacy record ciphers
are authenticated and converted before new cloud publication. If they used a
separate old record password, import with that password before rotating. Generic
file exports retain their supported legacy record encryption format. File sync
reconciles complete authenticated snapshots, sealing the merged envelope and
metadata rather than reconstructing an unsigned record-only result. No server
endpoint, signed server mutation or database schema changed.

Old clients that mutate a single Drive snapshot must be upgraded before writing
alongside this protocol. Modifying a retained ancestor causes a missing-history
error, not silent acceptance. No automatic cloud-history pruning is implemented;
immutable snapshots accumulate. Historical password envelopes remain in retained
backups: password rotation is not revocation of a previously disclosed DEK or
historic backup. A completely fresh device has no external trusted anchor to
detect a malicious provider replacing the entire history; previously initialized
devices reject removal of their accepted ancestors. Authentication protects
against cloud tampering, not an authorized device already holding the DEK.

## Verification

Automated tests use an HTTP transport that enforces generated-ID create-only
behavior and rejects PATCH/PUT/DELETE. They exercise 409 exact read-back, lost
acknowledgments, failed uploads, pagination, concurrent publications, tampered
revisions, missing history, independent vaults, password-only changes, explicit
password/directory conflicts, restart persistence, legacy upgrades and full
second-device restoration without Keychain access. These are protocol tests, not
live Google Drive acceptance. A test-account acceptance run remains a release
validation step.

# Conflict Resolution

[Docs Home](../README.md) / [Synchronization](README.md) / Conflict Resolution

---

Shellumi avoids blind Last-Write-Wins (LWW) overwriting for host and credential records:

- **Three-Way Merge (`determine_sync_action`):** Non-overlapping additive changes converge cleanly.
- **Google Drive Concurrent Branches:** Divergent `key` or `metadata` histories produce explicit conflict states bound to reviewed local contents and cloud heads, resolved interactively by choosing **Keep Local** or **Accept Remote** (creating a signed merge node retaining both parent IDs).
- **Shellumi Server OCC (HTTP 409):** Stale-revision CAS commits return `REVISION_MISMATCH`, preserving local pending mutations for user review or reconciliation.

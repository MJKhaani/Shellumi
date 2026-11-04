# Architecture (initial)

The root Cargo workspace contains `shellumi-app`, a GPUI window foundation,
and `shellumi-core`, a UI-independent library. `app/` is the separate Harbor
prototype and is not a workspace member. The core currently only validates an
SSH destination; it neither connects to a server nor handles credentials.

The intended dependency direction is UI → application services → protocol,
terminal and storage adapters. Protocol and domain code must not depend on GPUI.
Long-running I/O must not block the UI. Host-key decisions must be explicit and
enforced by the eventual SSH implementation. Credentials must be held in an
OS-backed or encrypted store, never embedded in connection definitions.

The storage crate uses the local, in-process Turso Database engine rather than
SQLite itself. It stores host metadata only and applies schema migrations in a
transaction. It is not yet connected to the UI; backup, recovery, encryption of
secrets, and cross-platform validation remain outstanding. Turso is pre-1.0 and
its upstream recommends independent backups; SQLite compatibility is incomplete.
See [Turso's documentation](https://github.com/tursodatabase/turso).

Before implementing SSH, select and audit a maintained
backend and test unknown, changed, and revoked server keys against real servers.
The SSH backend has not yet been selected or implemented.

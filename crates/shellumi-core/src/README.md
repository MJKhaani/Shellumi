# Core module map

The core crate is independent of the desktop UI. Its public exports remain in `lib.rs`.

- `sftp.rs`: filesystem models, paths, the SFTP client and remote editing.
  - `sftp/wire.rs`: SFTP v3 packet codecs and constants.
  - `sftp/transport.rs`: bounded SSH pipe I/O.
  - `sftp/transfer.rs`: transfer cancellation and deadlines.
  - `sftp/queue.rs`: transfer queue and progress aggregation.
- `infrastructure.rs`: provider models, discovery records and merge policy.
  - `infrastructure/aws.rs`, `azure.rs` and `digitalocean.rs`: provider response parsers.
  - `infrastructure/ansible.rs`: Ansible inventory parsing and export.
- `sync.rs`: encrypted synchronization models and orchestration.
  - `sync/http_transport.rs`: system HTTP transport and response decoding.
- `protocol.rs`: connection protocol definitions and wire helpers.

Feature test modules are alongside their implementation. Public types remain accessible through their existing paths.

# Shellumi — SFTP Implementation, GPUI Integration & Acceptance Progress

**Starting HEAD:** `5f631454dd499a9650a5e32b91ff2ab1bf2d5d8b` (pre-existing uncommitted terminal/recording changes preserved untouched)  
**Target GPUI:** `=0.2.2`  
**Scope:** Shellumi client (`shellumi-core`, `shellumi-app`), docs, and verification. `Shellumi-Server` untouched.

---

## 1. Phase Status

| Phase | Description | Status | Evidence & Notes |
|---|---|---|---|
| 0 | Baseline inventory & progress tracking | **DONE** | Baseline recorded; sandbox blocks `TcpListener::bind("127.0.0.1:0")` (`Operation not permitted`) |
| 1 | P0 filesystem safety & authorization | **DONE** | `endpoint.rs`, `path.rs`, `planner.rs`; 8/8 `phase1_safety` tests PASS |
| 2 | SFTP protocol & transport hardening | **DONE** | `protocol.rs`, `wire.rs` trailing-byte/bounds checks, `SftpClient` v3 check, checked response IDs, pipelined `Read`/`Write` with short-read/out-of-order handling, `OutcomeUnknown`; 6/6 `phase2_protocol` tests PASS |
| 3 | Persistent authenticated connection management | **DONE** | `connection.rs` (`SftpConnectionManager`, `PersistentSftpSession`, `classify_ssh_error`, `merge_connect_options`, SSH option allowlist), `connect_saved_sftp` vault-key resolution, `REALPATH(".")` home discovery |
| 4 | Unified transfer manager & all 4 directions | **DONE** | `manager.rs` (`UnifiedTransferManager`, `LocalToLocal`, `LocalToRemote`, `RemoteToLocal`, `relay_remote_to_remote` bounded streaming); 3/3 `phase3_4_manager` tests PASS |
| 5 | GPUI workspace & interactions | **DONE (core wiring)** | Immutable `SftpModalState` & `SftpDragPayload`, explicit folder-drop target, column sorting, narrow switcher, Transfer Drawer, vault-lock scope enforcement |
| 6 | Complete file management commands | **DONE (core + app)** | `operations.rs` (SHA-256, `plan_multi_rename`, `compare_directory_entries`, `plan_one_way_sync`, `search_local_recursive`, `local_disk_capacity`), `NewFile`, `Properties & SHA-256`, `delete_local_recursive`; 4/4 `phase6_7_ops_editor` tests PASS |
| 7 | Minimal integrated text/code editor | **DONE (core + GPUI)** | `editor.rs` + `sftp_editor.rs` (15 languages, syntax highlighting, strict non-lossy JSON + conservative YAML/TOML formatters, `EditorBuffer` undo/redo/find/replace/concurrent-save revision tracking, conflict detection) |
| 8 | Cross-platform & performance hardening | **PARTIAL** | Platform-aware local home, Windows reserved/ADS/trailing-dot path validation, non-blocking background I/O; native Windows/Linux GUI & multi-GiB/latency benchmarks `NOT TESTED` in macOS sandbox |
| 9 | Storage, CLI & documentation consistency | **DONE** | Updated `docs/user-guide/sftp.md`, `docs/project/requirements/terminal-and-sftp.md`, `docs/development/sftp-implementation-progress.md`; `./scripts/verify-docs.sh` PASS (149 files) |

---

## 2. Verification Commands & Results

| Command | Result | Notes |
|---|---|---|
| `cargo clippy --locked -p shellumi-core -p shellumi-app --all-targets -- -D warnings` | **PASS** (exit 0) | 0 warnings, 0 errors |
| `cargo test --locked -p shellumi-core sftp::tests::phase` | **PASS** (21/21 passed) | `phase1_safety` (8), `phase2_protocol` (6), `phase3_4_manager` (3), `phase6_7_ops_editor` (4) |
| `cargo test --locked -p shellumi-app sftp` | **PASS** (6/6 passed, 1 ignored) | Includes `app_sftp_unified_transfer_and_editor_engine_integration`; `ssh_sftp_acceptance` ignored (requires macOS `sshd` + loopback socket) |
| `cargo fmt --all -- --check` | **PASS** (exit 0) | Clean formatting |
| `./scripts/verify-docs.sh` | **PASS** (exit 0) | 149 markdown files checked |

---

## 3. Acceptance Checklist Summary (A01–G12)

- **Unit / Socket / App Tested (PASS):**
  - **A08, A10, A12, A13, A14:** `phase3_4_manager`, `connection.rs`, `vault_lock.rs`, `server_sync.rs`, `connect_saved_sftp`
  - **B01–B12:** `phase1_safety`, `phase2_protocol`, `codec`, `transfer`, `transfer_completion_tests`, `transport`
  - **C01, C02, C06–C11, C14–C20:** `phase1_safety`, `phase2_protocol`, `phase3_4_manager`, `phase6_7_ops_editor`, `app::tests::sftp`
  - **D01–D05, D08, D10–D12, D15, D16:** `phase2_protocol`, `phase3_4_manager`, `sftp_transfer::tests`
  - **E01, E02, E04, E08, E10–E15:** `sftp_ui/{browser,connected_browser,overlays,actions}.rs`, `tests::sftp`
  - **F01–F13, F16, F17:** `phase6_7_ops_editor`, `sftp_editor.rs`, `app::tests::sftp`
  - **G11, G12:** `clippy`, `fmt`, `verify-docs.sh`, `shellumi-core` & `shellumi-app` test runs
- **NOT TESTED (Environment / Sandbox / External Fixture Required):**
  - **A01–A07, A09, A11, C03–C05, C12, C13, D06, D07, D09, D13, D14, F14, F15, F18, G01–G10:** Require live disposable OpenSSH `sshd` with loopback TCP sockets (blocked in sandbox with `Operation not permitted`), multi-GiB / artificial-latency fixtures, or interactive native desktop runners on macOS, Linux (X11/Wayland), and Windows.
  - Reproduction for live macOS `sshd` acceptance outside sandbox:
    `cargo test --locked -p shellumi-app --test ssh_sftp_acceptance -- --ignored --nocapture`

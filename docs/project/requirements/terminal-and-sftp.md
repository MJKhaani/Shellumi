[Documentation Home](../../README.md) > [Requirements](README.md) > **Terminal & SFTP**

# Terminal, SFTP, Automation, AI & Session Logs Requirements

Recovered from §3.3, §3.7, §3.8, §3.9, §3.10, §3.11, and §3.12 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## Terminal Emulator & Workspaces (`REQ-TERM-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-TERM-001` | GPU-accelerated UTF-8 terminal with graphemes, wide chars, emoji, 256/Truecolor, alt-screen, bracketed paste, mouse reporting, OSC 8 hyperlinks, and Vim/tmux/htop support. | **Implemented** | Unit tested; Native GUI verified (macOS) | `shellumi-terminal`, GPUI renderer | `shellumi-terminal` VT/ANSI unit tests pass; Windows/Linux native GUI `NOT TESTED`. |
| `REQ-TERM-002` | Scrollback, regex/case-sensitive search, copy/paste, word/line selection, zoom, font/cursor/bell customization, clickable URLs, full-screen mode. | **Implemented** (core); **Partial** (opacity/copy-on-select) | Unit + GPUI tested | `shellumi-terminal`, `shellumi-app` | Scrollback, search, and `Preferences` ranges unit-tested; opacity/copy-on-select **Planned**. |
| `REQ-TERM-003` | Defensive escape-sequence handling (clipboard writes, suspicious hyperlinks). | **Implemented** | Unit tested | `shellumi-terminal` | Control-sequence boundary tests pass. |
| `REQ-TERM-004` | Multi-terminal management: tabs, multiple windows, vertical/horizontal/nested splits, rename/pin tabs, duplicate connections, close confirmation. | **Implemented** (tabs/splits/windows); **Partial** (drag-drop pane reorder) | Unit + GPUI tested | `shellumi-app` pane tree, `multi_window.rs` | Tab/split/close-guard tests (`INS-009`) pass; drag-drop pane reorder **Planned**. |
| `REQ-TERM-005` | Comparative terminal throughput/latency benchmarks against external Rust emulators. | **Partial** | Unit tested (`BenchmarkRunner`) | `shellumi-core::BenchmarkRunner`, CLI `bench` | Internal `bench` passes; external comparative benchmarks **Planned**. |
| `REQ-TERM-006` | Workspaces: create/rename/duplicate/delete, save layout/hosts/tabs/cwd/startup commands, reopen configs, templates. | **Implemented** (saved layouts/hosts); **Partial** (cwd/templates) | Unit + CLI tested | `shellumi-storage::SavedWorkspace`, `actions/workspaces.rs` | Workspace CRUD & CLI tests pass; workspaces launch new sessions, not live remote processes. |
| `REQ-TERM-007` | Broadcast input: explicit toggle, target pane indicators, pane exclusion, instant disable, confirmation for dangerous ops. | **Implemented** | Unit + GPUI tested | `shellumi-app` broadcast controller | Broadcast state & exclusion tests pass. |
| `REQ-TERM-008` | Session monitoring: active/disconnected states, awaiting-input indicators, connection errors, quick switching. | **Implemented** | GPUI + Source-inspected | `shellumi-app` session tabs | Tab status & error banner rendering verified on macOS. |

## SFTP File Manager (`REQ-SFTP-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-SFTP-001` | Dual-pane SFTP UI with breadcrumbs, path bar, back/forward, hidden-file toggle, file details, multiple SFTP tabs. | **Implemented** | Unit + App tested; Live macOS `sshd` opt-in | `shellumi-core::sftp`, `sftp_ui/` | `sftp::tests` & `app::tests::sftp` pass; live `sshd` & Win/Linux GUI `NOT TESTED` in sandbox. |
| `REQ-SFTP-002` | Operations: upload/download, remote-to-remote relay, local staged copy, recursive dirs, create/delete/rename/move/copy, chmod, symlink handling, SHA-256, compare/sync, search. | **Implemented** | Unit + App tested | `shellumi-core::sftp::{operations,planner,manager}` | Core unit/socket/planner/relay tests pass; live multi-server relay `NOT TESTED` in sandbox. |
| `REQ-SFTP-003` | Transfer queue: concurrent transfers, pipelined streaming, progress/speed/ETA, cancel/retry, conflict resolution, history, staged atomic uploads. | **Implemented** (pipelining/relay/cancel/retry); **Partial** (byte-range resume) | Unit + App tested | `UnifiedTransferManager`, `TransferQueue` | Pipelined out-of-order/short-read, `OutcomeUnknown`, and staged copy tests pass. |
| `REQ-SFTP-004` | Integrated administrative text/code editor: 15 language modes, syntax highlighting, conservative formatting, dirty revision tracking, conflict detection, vault isolation. | **Implemented** | Unit + App tested | `shellumi-core::sftp::editor`, `sftp_editor.rs` | Language detection, strict JSON/YAML/TOML formatting, concurrent-save & conflict tests pass. |

## Snippets, Shell Integration, AI & Logs (`REQ-AUTO-*`, `REQ-AI-*`, `REQ-LOG-*`)

| ID | Requirement | Status | Verification Level | Source Evidence | Acceptance Criterion |
| --- | --- | --- | --- | --- | --- |
| `REQ-AUTO-001` | Snippet CRUD, descriptions, tags, packages, search, host/group association, import/export, encrypted storage. | **Implemented** (CRUD/search); **Partial** (packages/import-export) | Unit + CLI tested | `SavedSnippet`, `actions/snippets.rs` | Snippet CRUD & vault persistence tests pass. |
| `REQ-AUTO-002` | Snippet execution: current terminal, paste-only, multi-host seq/par with concurrency limits, startup execution, dry-run, cancellation. | **Implemented** | Unit + CLI tested | `BatchExecutionPlan`, CLI `snippet batch` | Batch plan, concurrency, and `--dry-run` CLI tests pass. |
| `REQ-AUTO-003` | Snippet variables (user params, host props, env, secret refs, defaults, validation) with command preview and secret redaction. | **Implemented** | Unit tested | `render_command`, `render_preview` | Variable substitution & preview tests pass (shell output redaction is best-effort). |
| `REQ-AUTO-004` | Reversible shell integration for Bash, Zsh, Fish, PowerShell. | **Partial** | Unit + CLI tested | `generate_{bash,zsh,fish}_hook`, CLI `shell-hook` | Bash/Zsh/Fish CLI hook generation tested; PowerShell hook **Planned**. |
| `REQ-AUTO-005` | Intelligent autocomplete: history, frequent commands, args/options, paths, snippets, hostnames, fuzzy ranking. | **Partial** | Unit + CLI tested | `AutocompleteEngine`, CLI `complete` | Engine & CLI `complete` tested; full inline GUI popup **Planned**. |
| `REQ-AUTO-006` | Password prompt detection offering saved identities with explicit user permission. | **Partial** | Unit + CLI tested | `PasswordPromptDetector`, CLI `prompt-detect` | Detector & CLI tested; interactive GUI prompt offering **Partial**. |
| `REQ-AI-001`..`003` | Optional pluggable AI assistant (Ollama/OpenAI-compatible) with zero automatic transmission, redaction, and mandatory approval. | **Planned** | — | — | Not yet implemented. |
| `REQ-LOG-001` | Session recording & history: `.cast` playback, host/vault metadata, search, filter, bookmarks, comments, export, retention. | **Implemented** (CLI/storage); **Partial** (GUI playback/retention) | Unit + CLI tested | `CastPlaybackEngine`, `SessionRecordingRecord`, CLI `session` | CLI `session` import/list/show/play/export/delete tests pass. |
| `REQ-LOG-002` | Session log privacy: configurable recording, encrypted storage, best-effort redaction with documented limitations. | **Implemented** | Unit tested | `SessionRedactor` | Redaction unit tests pass; automated redaction is explicitly best-effort. |

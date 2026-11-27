[Documentation Home](../../README.md) > [Requirements](README.md) > **Terminal & SFTP**

# Terminal, SFTP, Automation, AI & Session Logs Requirements

Recovered from §3.3, §3.7, §3.8, §3.9, §3.10, §3.11, and §3.12 of [`master-project-specification.md`](../../archive/implementation-plans/master-project-specification.md).

## Terminal Emulator & Workspaces (`REQ-TERM-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-TERM-001` | GPU-accelerated UTF-8 terminal with graphemes, wide chars, emoji, 256/Truecolor, alt-screen, bracketed paste, mouse reporting, OSC 8 hyperlinks, and Vim/tmux/htop compatibility. | **Implemented** | `shellumi-terminal` + GPUI renderer. |
| `REQ-TERM-002` | Scrollback, regex/case-sensitive search, copy/paste, word/line selection, zoom, font/cursor/bell customization, clickable URLs, full-screen mode. | **Implemented** (core); **Partial** (opacity/copy-on-select) | `shellumi-terminal`, `shellumi-app`. |
| `REQ-TERM-003` | Defensive escape-sequence handling (clipboard writes, suspicious hyperlinks). | **Implemented** | Input/control filtering in `shellumi-terminal`. |
| `REQ-TERM-004` | Multi-terminal management: tabs, multiple windows, vertical/horizontal/nested splits, rename/pin tabs, duplicate connections, close confirmation. | **Implemented** (tabs/splits/windows); **Partial** (drag-drop pane reorder) | `shellumi-app` workspace & pane tree. |
| `REQ-TERM-005` | Benchmark terminal throughput and latency against established Rust emulators. | **Partial** | CLI `bench` framework exists; comparative external benchmarks pending. |
| `REQ-TERM-006` | Workspaces: create/rename/duplicate/delete, save layout/hosts/tabs/cwd/startup commands, reopen configs, templates. | **Implemented** | `Workspace` store & UI. |
| `REQ-TERM-007` | Broadcast input: explicit mode toggle, target pane indicators, pane exclusion, instant disable, confirmation for dangerous ops. | **Implemented** | Broadcast input controller in `shellumi-app`. |
| `REQ-TERM-008` | Session monitoring: active/disconnected states, awaiting-input indicators, connection errors, quick switching. | **Implemented** | Session tabs & status indicators. |

## SFTP File Manager (`REQ-SFTP-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-SFTP-001` | Dual-pane SFTP UI with breadcrumbs, path bar, back/forward, hidden-file toggle, file details, multiple SFTP tabs, drag-and-drop. | **Implemented** | `sftp_ui` in `shellumi-app`. |
| `REQ-SFTP-002` | Operations: upload/download, recursive directories, create/delete/rename/move/copy, chmod permissions, symlink handling, text preview, local open. | **Implemented** (core ops); **Partial** (remote-to-remote) | `SftpClient` in `shellumi-core`. |
| `REQ-SFTP-003` | Transfer queue: concurrent transfers, progress/speed/ETA, cancel/retry, conflict resolution (overwrite/skip/rename), history, staged atomic uploads. | **Implemented** (queue/cancel/history); **Partial** (partial-resume across arbitrary servers) | `TransferQueue`, `NewTransfer`. |
| `REQ-SFTP-004` | Remote editing: download to secure temp workspace, integrated/external edit, dirty tracking, remote modification detection, temp cleanup. | **Implemented** | Verified in `sftp/tests.rs`. |

## Snippets, Shell Integration, AI & Logs (`REQ-AUTO-*`, `REQ-AI-*`, `REQ-LOG-*`)

| ID | Requirement | Status | Evidence / Notes |
| --- | --- | --- | --- |
| `REQ-AUTO-001` | Snippet CRUD, descriptions, tags, packages, search, host/group association, import/export, encrypted storage. | **Implemented** | `Snippet` store & Inspector. |
| `REQ-AUTO-002` | Snippet execution: current terminal, paste-only, multi-host sequential/parallel with concurrency limits, startup execution, dry-run, cancellation. | **Implemented** | `BatchExecutionPlan`, CLI `snippet batch`. |
| `REQ-AUTO-003` | Snippet variables (user params, host props, env, secret refs, defaults, validation) with command preview and secret redaction. | **Implemented** | `render_command`, `render_preview`. |
| `REQ-AUTO-004` | Reversible shell integration for Bash, Zsh, Fish, PowerShell (cwd, prompt boundaries, exit status, history). | **Partial** | `shell-hook <bash|zsh|fish>` implemented; PowerShell planned. |
| `REQ-AUTO-005` | Intelligent autocomplete: history, frequent commands, args/options, paths, snippets, hostnames, fuzzy ranking, low-latency fallback. | **Partial** | `AutocompleteEngine` & CLI `complete` implemented; inline GUI popup partial. |
| `REQ-AUTO-006` | Password prompt detection offering saved identities with explicit user permission (no indiscriminate injection). | **Partial** | `PasswordPromptDetector` & CLI `prompt-detect`. |
| `REQ-AI-001` | Optional natural-language-to-shell command generation, error explanation, snippet generation. | **Planned** | Not yet implemented. |
| `REQ-AI-002` | Pluggable AI providers (local Ollama, OpenAI-compatible APIs) with user-supplied credentials. | **Planned** | Not yet implemented. |
| `REQ-AI-003` | AI security: zero automatic transmission, context preview, redaction, mandatory approval before execution. | **Planned** | Mandatory constraint for future AI implementation. |
| `REQ-LOG-001` | Session recording & history: timestamps, host/vault metadata, search, filter, bookmarks, comments, export, retention. | **Implemented** | `CastPlaybackEngine`, `SessionRecordingRecord`, CLI `session`. |
| `REQ-LOG-002` | Session log privacy: opt-in/configurable, encrypted storage, redaction with documented limitations. | **Implemented** | `SessionRedactor`. |

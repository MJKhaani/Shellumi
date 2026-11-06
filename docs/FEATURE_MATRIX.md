# Feature status — v0.0.1-alpha

This matrix describes the current application. [PROJECT.md](../PROJECT.md) is
the target scope; domain types and packaging prototypes alone do not constitute
finished user features. See [GUI_STATUS.md](GUI_STATUS.md) for workflow details.

| Feature | Alpha status | Evidence and remaining work |
| --- | --- | --- |
| Native desktop | Available on tested macOS build | Rust/GPUI navigation, host inspector and editors; accessibility and Linux/Windows runtime checks remain. |
| First launch | Implemented | Platform data path and database directories are created automatically; reopen/persistence tests. |
| Hosts and groups | Implemented locally | Host CRUD, search, groups/defaults/tags, jump-host validation, clipboard OpenSSH config import/export. Broader import previews and bulk operations remain. |
| SSH | Initial working integration | Real system OpenSSH through PTYs, strict host-key decisions and disabled agent forwarding; isolated-server authentication test. Full GUI authentication and platform acceptance testing remain. |
| Terminal | Initial working integration | Local/remote tabs, split/grid layouts, measured PTY resizing, scrollback, search, paste, ANSI styling and recording. macOS long-output/scroll/split checks performed; terminal compatibility testing remains. |
| Snippets | Implemented locally | Create/edit multi-line commands, parameters, tags/search, JSON import/export, deletion confirmation and manual terminal actions. Startup auto-execution is disabled. |
| Workspaces | Implemented locally | Create/edit/duplicate/delete and reopen saved session definitions. Remote processes and live session state are not restored. |
| Settings | Implemented subset | Persistent text sizes, scrollback, startup screen and hidden files; security/shortcut information and shell tools. Theme/language switching and launch-at-login remain. |
| Keychain / identities | Implemented | Reusable Identity model & Turso table (schema v14), OpenSSH certificate validation/principals, host/group binding with credential inheritance, Native OS Keychain daemons (macOS Security Framework, Linux Secret Service via `secret-tool` / D-Bus, Windows Credential Manager `PasswordVault` API) with AES-256-GCM authenticated vault fallback, URI reference binding (`keychain://`), OS Keychain daemon status card, and CLI management. |
| Known Hosts | Initial integration | Real trust-store inspection and explicit revocation with stale-record checks. Trust acceptance remains an explicit OpenSSH decision. |
| Tunnels | Initial working integration | Persisted rules, SSH-host selection, editing and real OpenSSH tunnel sessions. Traffic telemetry is not measured. |
| SFTP | Implemented | RFC SFTP v3 wire format codec, SftpClient over SSH subsystem (`ssh -s sftp`), graphical dual-pane browser, directory listing, upload/download queue, and RemoteEditorWorkspace with SHA-256 dirty checking. |
| Session logs | Initial integration | Opt-in recordings, library, playback and export tools. Redaction is not a guarantee that recordings contain no sensitive data. |
| Local storage | Implemented | Async Turso schema v14, transactional migrations, CRUD, reopen/future-version tests, identities support, and local preferences. |
| Encrypted backups | Implemented locally | GUI export/restore, masked passphrase entry, overwrite refusal, transactional rollback and round-trip tests. |
| Vaults and memberships | Local records only | Vault/member/audit managers; actual public keys required. No network enrollment or sharing. |
| Collaboration | Not connected | Domain/state models exist; network terminal transport is unfinished. |
| Infrastructure | Local inventory tools | Provider records, inventory parsing and Ansible tools exist; authenticated cloud API discovery is unfinished. |
| Google Drive sync | Implemented | Zero-knowledge cloud vault synchronization over Google Drive appDataFolder via SystemHttpTransport (system curl), encrypted diffing, visual conflict resolution modal, and desktop sync panel. |
| Mosh, Telnet, serial | Experimental | Models and dispatch paths exist; separate executables/hardware may be needed and acceptance testing remains. Telnet is unencrypted. |
| Mobile | Prototype | Core FFI and native examples only; no released mobile client. |
| Releases | Tag-driven CI | Four platform/architecture targets, native packaging, checksums and Cargo metadata manifest. The alpha tag is a prerelease; CI success and runtime validation are separate checks. |

The implementation passed 288 automated tests, with one additional opt-in
isolated SSH-server test ignored in the normal suite. That test was exercised
separately during development. See [ROADMAP.md](ROADMAP.md) for next steps.

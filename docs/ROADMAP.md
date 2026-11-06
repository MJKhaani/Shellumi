# Shellumi roadmap

The current milestone is **v0.0.1-alpha**. Requirements remain in
[PROJECT.md](../PROJECT.md); the [feature matrix](FEATURE_MATRIX.md) and
[GUI status](GUI_STATUS.md) describe implementation evidence and limitations.

## Alpha milestone

The following work is included in the alpha source:

- Native Rust/GPUI desktop with the supplied navy/teal visual direction,
  navigation, host inventory/inspector, centered editors and local-first startup.
- Automatic platform data paths, Turso database creation and migration,
  asynchronous storage, and persistent preferences.
- Host/group editing, filters, jump hosts, OpenSSH config import/export and
  explicit SSH/SFTP session dispatch.
- Local/remote PTYs, terminal tabs and split/grid layouts, viewport-based sizing,
  wheel scrollback, search, paste, ANSI styling and opt-in recordings.
- Functional snippet and workspace editors with storage persistence, template
  variables, import/export and confirmation for destructive library actions.
- Settings that apply and persist, public SSH-key/trust inspection, agent/key
  tools, saved forwarding rules and actual OpenSSH tunnel sessions.
- Encrypted local backup dialogs with overwrite refusal, transactional restore
  and wrong-passphrase rollback.
- Local inventory, vault membership and recording managers, with unfinished
  network transports identified rather than represented as successful actions.
- macOS development screenshots in the README and a tag-triggered prerelease
  workflow for Linux x86_64, Windows x86_64, macOS arm64 and macOS x86_64.

## Verification recorded for alpha

- 270 automated workspace tests passed; one additional isolated-server SSH test
  is opt-in and was exercised separately during development.
- Formatting and workspace Clippy with `-D warnings` passed.
- An optimized Intel macOS app was built locally.
- macOS GUI checks covered saving snippets/workspaces, launching a workspace
  shell, changing a terminal preference, long output, scrolling and split sizing.
- Linux, Windows and Apple Silicon runtime behavior is not established by these
  local checks. Tag-triggered CI build results must be reviewed separately.

## Next engineering increments

1. **SSH/terminal acceptance:** test password, encrypted-key and MFA prompts,
   changed/revoked keys, disconnect/reconnect failures, resize under full-screen
   terminal applications, larger output workloads and focus behavior.
2. **Graphical SFTP transport:** connect real remote directory and transfer
   operations to the browser/queue; add progress, cancellation, conflict handling
   and recursive operations. The system SFTP terminal remains available meanwhile.
3. **Desktop polish and accessibility:** native file pickers, keyboard traversal,
   screen-reader semantics, smaller-window layouts and remaining design dialogs;
   expand preferences only when each option has working behavior.
4. **Platform release validation:** review all CI assets, test clean installations
   and default directories on each OS, validate dependencies and decide signing/
   notarization before broader distribution.
5. **Credentials and identities:** complete reusable identity associations and
   native Windows/Linux credential backends, with platform-specific tests.
6. **Optional integrations:** implement authenticated cloud discovery and Drive
   OAuth/transport, then exercise encrypted sync/conflict/restore end to end.
7. **Network collaboration:** implement authenticated session transport and
   enrollment before exposing remote sharing/control as available.
8. **Longer-term scope:** additional protocol acceptance, theme/language options,
   mobile feasibility and remaining requirements from the project specification.

Packaging definitions, domain models, state-machine tests and prototype screens
are foundations for these increments; they are not completion evidence by themselves.

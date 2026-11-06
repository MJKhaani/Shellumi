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
- Reusable identity management with OpenSSH certificate parsing/validation,
  database persistence (schema v14), host/group GUI identity selection and inheritance,
  SSH/SFTP/workspace/tunnel launch policy, reference-only askpass, and Keychain display.
- Live graphical remote SFTP transport engine over SSH subsystem (`ssh -s sftp`)
  with RFC SFTP v3 wire codec, dual-pane remote browsing, file upload/download, and transfer queue.
- Initial Google Drive HTTP transport (`SystemHttpTransport`) for zero-knowledge
  cloud vault sync, encrypted diffing, conflict resolution modal, and desktop sync panel.
- Encrypted local backup dialogs with overwrite refusal, transactional restore
  and wrong-passphrase rollback.
- Local inventory, vault membership and recording managers, with unfinished
  network transports identified rather than represented as successful actions.
- macOS development screenshots in the README and a tag-triggered prerelease
  workflow for Linux x86_64, Windows x86_64, macOS arm64 and macOS x86_64.

## Verification recorded for alpha

- 298 automated workspace tests passed; one additional isolated-server SSH test
  is opt-in and was exercised separately during development.
- Local OpenSSH encrypted-key authentication through the app helper succeeded;
  the same setup refused an untrusted host. Live password/MFA/native-store
  and graphical SFTP acceptance remain pending.
- Formatting and workspace Clippy with `-D warnings` passed with 0 warnings.
- An optimized Intel macOS app was built locally.
- macOS GUI checks covered saving snippets/workspaces, launching a workspace
  shell, changing a terminal preference, long output, scrolling and split sizing.
- Linux, Windows and Apple Silicon runtime behavior is not established by these
  local checks. Tag-triggered CI build results must be reviewed separately.

## Next engineering increments

1. **SSH/terminal acceptance:** test password, encrypted-key and MFA prompts,
   changed/revoked keys, disconnect/reconnect failures, resize under full-screen
   terminal applications, larger output workloads and focus behavior.
2. **Desktop polish and accessibility:** native file pickers, keyboard traversal,
   screen-reader semantics, smaller-window layouts and remaining design dialogs;
   expand preferences only when each option has working behavior.
3. **Platform release validation:** review all CI assets, test clean installations
   and default directories on each OS, validate dependencies and decide signing/
   notarization before broader distribution.
4. **Credential and transport acceptance:** add stdin/masked CLI secret entry and GUI identity editing;
   verify saved-identity authentication and native keychains on actual Windows/Linux desktops. Exercise
   graphical SFTP against an isolated server and Drive against a test account.
   Secure credential fallback is opt-in with a user-supplied passphrase; Drive
   must not sync with a shared/default vault key.
5. **Optional infrastructure integrations:** implement authenticated AWS/GCP/DigitalOcean
   cloud discovery APIs to complement the offline inventory parsing.
6. **Network collaboration:** implement authenticated session transport and
   enrollment before exposing remote sharing/control as available.
7. **Longer-term scope:** additional protocol acceptance, theme/language options,
   mobile feasibility and remaining requirements from the project specification.

Packaging definitions, domain models, state-machine tests and prototype screens
are foundations for these increments; they are not completion evidence by themselves.

## Security hardening follow-up

Removed the fixed credential-fallback and Drive vault passphrases. The encrypted
credential fallback now requires `SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE` (at least
12 characters) and an absolute persistent path; otherwise native-store errors
remain visible. Existing fallback files are not automatically migrated using
the old shared key. Drive requires an explicit vault passphrase and uses the
standard KDF settings. Graphical SFTP disables agent forwarding and requires
already trusted server keys; establish trust through the interactive SSH path
first. HTTP authorization headers and UTF-8 bodies are piped to curl, not passed
in command arguments. These corrections do not establish live service acceptance.

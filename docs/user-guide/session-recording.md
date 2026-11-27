# <img src="../../resources/icon_pack/icons/48/regular/session_record.png" width="24" height="24" alt="Session Recording"> Session Recording & History

[Docs Home](../README.md) / [User Guide](README.md) / Session Recording

---

Shellumi records and plays back terminal sessions in asciicast (`.cast`) format (`CastPlaybackEngine`, `SessionRecordingRecord`), storing recordings inside your encrypted vault database.

## Prerequisites & Access

- **GUI:** Open **Session Logs** from the left navigation rail to browse recorded sessions in the active vault.
- **CLI:** Use `shellumi-cli <database-path> session ...` for full import, playback, export, and deletion workflows.

---

## Recording, Playback & Import/Export (GUI vs. CLI)

| Capability | Desktop GUI | `shellumi-cli` |
| --- | --- | --- |
| **Browse / List Recordings** | **Session Logs** view (filtered by active vault) | `session list [--host-id <id>]` |
| **Inspect Metadata** | Recording entry details | `session show <id>` |
| **Playback (`.cast`)** | Basic log view | `session play <id> [--speed <multiplier>] [--raw]` |
| **Import `.cast` File** | CLI only | `session import <cast-file> --title <title> [--host-id <id>] [--redact]` |
| **Export `.cast` File** | CLI only | `session export <id> [--out <path>]` |
| **Delete Recording** | CLI / vault management | `session delete <id>` |

### Practical CLI Examples

```bash
# Import an asciicast recording with best-effort secret redaction
shellumi-cli ~/vault.db session import deploy.cast --title "Prod Deploy #42" --host-id <host-id> --redact

# Play back at 1.5x speed
shellumi-cli ~/vault.db session play <session-id> --speed 1.5

# Export to a .cast file
shellumi-cli ~/vault.db session export <session-id> --out exported.cast
```

---

## Security, Redaction Limitations & Storage

- **Storage Location:** Recordings are stored inside your encrypted vault database (`<data_dir>/vaults/<vault-id>.db`), never as unencrypted plaintext files by default.
- **Redaction Limitations:** `SessionRedactor` applies best-effort pattern redaction when `--redact` is used, **but automated redaction is not foolproof**. Interactive output, custom prompts, or binary/base64 dumps can still capture sensitive data. Disable recording during sensitive credential operations.

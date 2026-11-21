# Logging Architecture & Guidelines (`docs/LOGGING.md`)

This document defines the cross-platform structured logging architecture, log level conventions, security rules, and implementation standards for Shellumi.

---

## 1. Architecture Overview

Shellumi uses a unified, asynchronous, non-blocking logging architecture built on Rust's [`tracing`](https://crates.io/crates/tracing) ecosystem:

- **Instrumentation Facade:** [`tracing`](https://crates.io/crates/tracing) (v0.1) provides zero-allocation macro dispatch (`error!`, `warn!`, `info!`, `debug!`, `trace!`). When verbose logging is disabled, overhead is virtually zero.
- **Subscriber & Registry:** [`tracing-subscriber`](https://crates.io/crates/tracing-subscriber) (v0.3) handles filtering, thread-safe dispatch, and layer orchestration.
- **Native OS Integration:** [`tracing-os-layer`](https://crates.io/crates/tracing-os-layer) (v0.2.1) routes structured events directly to platform-native logging systems:
  - **macOS:** Apple Unified Logging (`os_log`) and Activity Tracing (`os_activity`) under subsystem `dev.shellumi.app` or `dev.shellumi.cli`.
  - **Linux:** systemd journal (`tracing-journald`) via native socket.
  - **Windows:** Windows Event Log (`Application` log) under the registered source.
  - **Other / Stubs:** Graceful degradation without panic or crash when native backends are unavailable.
- **Console / Stderr Layer:** Formatted human-readable output to `std::io::stderr` for terminal sessions, diagnostics, and developer workflows.

```text
+---------------------------------------------------------------------------------+
|                        Application Modules & Crates                             |
| (shellumi-app, shellumi-cli, shellumi-storage, shellumi-vnc, shellumi-rdp, etc.)|
+---------------------------------------+-----------------------------------------+
                                        | tracing macros (error!, warn!, info!)
                                        v
+---------------------------------------------------------------------------------+
|                        tracing::subscriber::Registry                            |
|                 +---------------------------------------------+                 |
|                 | EnvFilter (Default: ERROR or SHELLUMI_LOG)  |                 |
|                 +----------------------+----------------------+                 |
|                                        |                                        |
|             +--------------------------+--------------------------+             |
|             v                                                     v             |
|   +-------------------+                                 +-------------------+   |
|   | fmt::layer        |                                 | OsLogLayer        |   |
|   | (stderr stream)   |                                 | (Native OS Log)   |   |
|   +-------------------+                                 +---------+---------+   |
|                                                                   |             |
+-------------------------------------------------------------------|-------------+
                                                                    v
                                               +--------------------+--------------------+
                                               | macOS: os_log / Apple Unified Log       |
                                               | Linux: systemd journald socket          |
                                               | Windows: Event Log (Application)        |
                                               | Fallback: Gracefully disabled (no-op)   |
                                               +-----------------------------------------+
```

---

## 2. Default Log Level & Configuration

### Default Level: `ERROR`
By default across **all builds** (both debug and release), Shellumi filters logging at `ERROR`. This guarantees:
- **Zero performance degradation** during high-throughput terminal sessions or remote desktop streaming.
- **Clean system logs** free from high-frequency frame or keystroke noise.
- **Maximum privacy** by preventing unnecessary operational telemetry from entering OS event logs.

### Overriding the Log Level
Users and developers can override the filtering behavior using environment variables:

1. **`SHELLUMI_LOG` (Primary):**
   ```bash
   # Set global log level to INFO
   SHELLUMI_LOG=info ./target/debug/shellumi-app

   # Set global level to ERROR, but enable DEBUG for VNC and RDP protocol crates
   SHELLUMI_LOG=error,shellumi_vnc=debug,shellumi_rdp=debug ./target/debug/shellumi-app

   # Deep trace of storage migrations and database transactions
   SHELLUMI_LOG=shellumi_storage=trace ./target/debug/shellumi-app
   ```

2. **`RUST_LOG` (Standard Fallback):**
   If `SHELLUMI_LOG` is unset, Shellumi checks `RUST_LOG` before falling back to `error`.

3. **Programmatic Configuration:**
   Using [`shellumi_core::logging::LoggingConfig`]:
   ```rust
   let mut config = shellumi_core::logging::LoggingConfig::for_app();
   config.default_level = tracing::level_filters::LevelFilter::INFO;
   shellumi_core::logging::init_logging(&config)?;
   ```

---

## 3. Initialization in Entry Points

Logging must be initialized once at the very entry point of the binary before running any application logic or spawning threads.

### Desktop Application (`crates/shellumi-app/src/main.rs`)
```rust
fn main() {
    // 1. Initialize application logging subsystem (dev.shellumi.app)
    if let Err(e) = shellumi_core::logging::init_app_logging() {
        eprintln!("Warning: Failed to initialize application logging: {e}");
    }

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        "Shellumi desktop application started"
    );

    // Continue application launch...
}
```

### CLI Tool (`crates/shellumi-cli/src/main.rs`)
```rust
#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Initialize CLI logging subsystem (dev.shellumi.cli)
    if let Err(e) = shellumi_core::logging::init_cli_logging() {
        eprintln!("Warning: Failed to initialize CLI logging: {e}");
    }

    if let Err(error) = run().await {
        tracing::error!(error = %error, "CLI command execution failed");
        eprintln!("{error}");
        std::process::exit(1);
    }
}
```

### Idempotency & Unit Tests
`init_logging` is safe to call across unit tests or multiple worker threads. If a subscriber is already installed in the process, it returns `Ok(())` without panicking.

---

## 4. Log Level Conventions

Every log entry must use an appropriate level according to this taxonomy:

| Level | Purpose | Frequency | Examples |
| :--- | :--- | :--- | :--- |
| **`ERROR`** | Unrecoverable failures, connection drops, decryption errors, schema mismatches, failed I/O. | Rare | Database migration failed, TLS certificate untrusted, RDP handshake aborted, network timeout. |
| **`WARN`** | Recoverable issues, fallbacks, transient retry attempts, deprecations. | Occasional | VNC transient drop triggering reconnect backoff, unsupported clipboard format ignored, file cache miss. |
| **`INFO`** | Major lifecycle milestones, connection established, app startup/shutdown, sync completed. | Low | Desktop app launched, VNC session connected (1920x1080), cloud sync cycle committed 4 changes. |
| **`DEBUG`** | Diagnostic steps, state transitions, negotiation steps, process spawning. | Moderate | State machine transition `Connecting -> Connected`, PTY child process spawned with PID, host record inserted. |
| **`TRACE`** | High-frequency protocol details, rectangle coordinates, packet headers, byte counters. | High | RFB FramebufferUpdateRequest blitted, PTY window resized, audio packet decoded (1024 samples). |

> [!IMPORTANT]
> **Never silently swallow errors.** If a `Result::Err` or unexpected condition is handled by falling back or returning early, emit at minimum a `tracing::warn!` or `tracing::error!` with structured error details.

---

## 5. Security & Sensitive Data Redaction

Security is non-negotiable. Shellumi is a security-critical infrastructure client handling passwords, private keys, passphrase credentials, and personal server destinations.

### Non-Negotiable Security Rules:
1. **NEVER log plaintext passwords, passphrase strings, or PINs.**
2. **NEVER log private keys, secret keys, or raw crypto envelopes.**
3. **NEVER log session tokens, API tokens, OAuth bearer tokens, or refresh tokens.**
4. **NEVER log raw user terminal input containing credentials** (e.g. `askpass` input or `sudo` prompt responses).
5. **Always sanitize or redact sensitive fields.**

### Using Redaction Helpers (`shellumi-core::logging`)
```rust
use shellumi_core::logging::{redact_sensitive, Redacted};

// Redacting ad-hoc sensitive strings:
let password = "super_secret_user_password";
tracing::debug!(password = redact_sensitive(password), "Authenticating against VNC server");
// Output: password="[REDACTED]"

// Using the Redacted wrapper to guard secrets from accidental {:?} display:
let token = Redacted::new("ghp_SecretGitHubToken12345");
tracing::info!(token = %token, "Loaded provider authentication token");
// Output: token="[REDACTED]"
```

---

## 6. Structured Logging Fields & Attributes

Always prefer structured key-value fields over unformatted string interpolation. Structured fields allow automated log filtering, parsing, and correlation across native OS log viewers (such as macOS `Console.app` or Linux `journalctl`).

### Standard Field Names:
- `host`: Target remote hostname or IP address (e.g. `host = %destination.host()`).
- `port`: Network port number (e.g. `port = destination.port().get()`).
- `subsystem`: Module or component domain (e.g. `subsystem = "vnc"`, `subsystem = "rdp"`, `subsystem = "storage"`).
- `session_id`: Unique identifier for the active session (e.g. `session_id = %session.id()`).
- `vault_id`: Vault identity string (e.g. `vault_id = %vault_id`).
- `error`: Formatted error object (e.g. `error = %err` using display formatting).
- `attempt`: Numeric retry or reconnection attempt index.

### Best Practice Example:
```rust
// ❌ BAD: String interpolation without structured fields, prone to format leakage
tracing::error!("Connection to {}:{} failed with error: {}", host, port, err);

// ✅ GOOD: Structured fields with typed display formatting
tracing::error!(
    host = %host,
    port = port,
    error = %err,
    attempt = attempt,
    "Failed to establish remote connection"
);
```

---

## 7. Platform-Specific Behavior & Native Backends

| Platform | Native Backend | Viewing Logs | Fallback Behavior |
| :--- | :--- | :--- | :--- |
| **macOS** | Apple Unified Logging (`os_log`) & Activity Tracing (`os_activity`) | Open `Console.app` and filter by Subsystem `dev.shellumi.app` or run `log stream --predicate 'subsystem == "dev.shellumi.app"'`. | None needed (`os_log` is always present on macOS). |
| **Linux** | systemd journal (`tracing-journald`) | Run `journalctl -t shellumi-app` or `journalctl _COMM=shellumi-app -f`. | If systemd journal socket `/run/systemd/journal/socket` is not available (e.g. in minimal Docker/OCI containers), `tracing-os-layer` returns `None` and Shellumi logs to `stderr` gracefully without crashing. |
| **Windows** | Windows Event Log (`Application` log) | Open Windows Event Viewer (`eventvwr.msc`) -> Windows Logs -> Application. | If event source registration is absent, gracefully falls back to `stderr` stream without crashing. |

---

## 8. How to Enable and View Debug Logs

### Enabling Debug Logs on macOS

1. **System-Wide & GUI Launches (Finder, Dock, Spotlight):**
   Set the environment variable in the user's `launchd` domain:
   ```bash
   launchctl setenv SHELLUMI_LOG debug
   ```
   To reset back to default `ERROR` level:
   ```bash
   launchctl unsetenv SHELLUMI_LOG
   ```

2. **Persistent Terminal Shells (`~/.zshrc` or `~/.bashrc`):**
   Add the following line to `~/.zshrc`:
   ```bash
   export SHELLUMI_LOG=debug
   ```

3. **Direct Binary / Terminal Launch (Live Stderr Output):**
   ```bash
   SHELLUMI_LOG=debug ./dist/Shellumi.app/Contents/MacOS/shellumi-app
   # Or for CLI commands:
   SHELLUMI_LOG=debug ./dist/shellumi-cli-macos-x86_64 doctor
   ```

### Viewing Logs on macOS

#### Option A: Terminal Live Stream (Apple Unified Logging)
```bash
log stream --predicate 'subsystem == "dev.shellumi.app"' --debug --info
```
To stream CLI events specifically:
```bash
log stream --predicate 'subsystem == "dev.shellumi.cli"' --debug --info
```

#### Option B: macOS Console Application (`Console.app`)
1. Launch **/Applications/Utilities/Console.app**.
2. In the search field (top right), type: `subsystem:dev.shellumi.app`.
3. In the menu bar, navigate to **Action** and verify:
   - [x] **Include Info Messages**
   - [x] **Include Debug Messages**
4. Click **Start Streaming** in the toolbar.

---

### Enabling and Viewing Logs on Linux

1. **Enable in Shell:**
   ```bash
   export SHELLUMI_LOG=debug
   ./shellumi-app
   ```
2. **View in systemd Journal:**
   ```bash
   journalctl _COMM=shellumi-app -f -p debug
   ```

---

### Enabling and Viewing Logs on Windows

1. **Enable in PowerShell:**
   ```powershell
   $env:SHELLUMI_LOG = "debug"
   .\shellumi-app.exe
   ```
2. **View in Windows Event Viewer:**
   Open `eventvwr.msc` -> **Windows Logs** -> **Application** -> Filter by source `dev.shellumi.app`.
   Or via PowerShell:
   ```powershell
   Get-WinEvent -ProviderName "dev.shellumi.app" -MaxEvents 50
   ```

---

## 9. Summary Checklist for Code Contributions

Before submitting code changes, verify:
- [ ] No `println!`, `eprintln!`, or `dbg!` left in production crates (`shellumi-storage`, `shellumi-terminal`, `shellumi-vnc`, `shellumi-rdp`, `shellumi-app`).
- [ ] Errors are captured with `tracing::error!` or `tracing::warn!` and not swallowed.
- [ ] No passwords, private keys, passphrase secrets, or tokens are logged.
- [ ] Structured field keys follow conventions (`host`, `port`, `error`, `session_id`).
- [ ] Default log level remains `ERROR` unless overridden.
- [ ] Tests and builds compile cleanly with `cargo clippy --workspace --all-targets -- -D warnings`.

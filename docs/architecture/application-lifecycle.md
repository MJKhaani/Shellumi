# Application Lifecycle

[Docs Home](../README.md) / [Architecture](README.md) / Application Lifecycle

---

## Startup Sequence (`crates/shellumi-app/src/main.rs`)

```mermaid
flowchart TD
    Start["main()"] --> Askpass{"SHELLUMI_ASKPASS_MODE == 1?"}
    Askpass -->|Yes| RunAskpass["Execute piped SSH askpass helper & exit"]
    Askpass -->|No| Logging["Init tracing logging (dev.shellumi.app)"]
    Logging --> Migration["Check MigrationEngine (legacy shellumi.db -> vaults/)"]
    Migration --> Recovery["Check interrupted vault_rotation / quarantine recovery"]
    Recovery --> GPUI["Start GPUI Application & bind OS keystrokes"]
    GPUI --> Window["Open main Shellumi window (1440x900 centered)"]
    Window --> Wizard{"vaults/default.db exists?"}
    Wizard -->|No| ShowWizard["Open Setup Wizard"]
    Wizard -->|Yes| LoadVault["Unlock / Load active vault via background_executor()"]
```

## Runtime Threading & Multi-Window Isolation

- **UI Thread (GPUI):** Renders views at 60/120 FPS. Database I/O, cryptography, and network operations never block the main UI thread.
- **Background Executors:** `cx.background_executor()` spawns async tasks running single-threaded or multi-threaded `tokio` runtimes (`features = ["rt", "rt-multi-thread"]`).
- **Multi-Window Vault Binding:** `open_vault_in_new_window` (`Cmd+Shift+N` / `Ctrl+Shift+N`) opens independent `Shellumi` root views in the same process, each bound to its own `active_vault_id`, host list, `InspectorCoordinator`, `ConnectionOpRegistry`, and WebSocket notification subscription.

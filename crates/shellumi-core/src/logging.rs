//! Cross-platform structured logging infrastructure for Shellumi.
//!
//! Provides unified logging using `tracing`, `tracing-subscriber`, and `tracing-os-layer`,
//! integrating with native OS logging facilities:
//! - macOS: Apple Unified Logging (`os_log`) and Activity Tracing (`os_activity`)
//! - Linux: systemd journal (`tracing-journald`)
//! - Windows: Windows Event Log (`Application` log)
//! - Other/Stubs: Graceful fallback to standard error
//!
//! # Default Log Level
//! By default, logging operates at `ERROR` level across all builds to ensure zero overhead
//! and minimal noise unless explicitly overridden via:
//! 1. `SHELLUMI_LOG` environment variable (e.g., `SHELLUMI_LOG=info`, `SHELLUMI_LOG=shellumi_rdp=debug,error`)
//! 2. `RUST_LOG` environment variable (fallback if `SHELLUMI_LOG` is unset)
//! 3. Explicit programmatic configuration via [`LoggingConfig`].

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt as sub_fmt, EnvFilter};

/// Standard redaction placeholder used across all log outputs.
pub const REDACTED_PLACEHOLDER: &str = "[REDACTED]";

static LOGGING_INITIALIZED: AtomicBool = 恒_false();
static INIT_ONCE: Once = Once::new();

const fn 恒_false() -> AtomicBool {
    AtomicBool::new(false)
}

/// Redacts sensitive values, always returning `"[REDACTED]"`.
#[inline]
pub fn redact_sensitive(_val: impl AsRef<str>) -> &'static str {
    REDACTED_PLACEHOLDER
}

/// A wrapper for sensitive values that masks its contents in both `Debug` and `Display`.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Redacted<T>(pub T);

impl<T> Redacted<T> {
    /// Wrap a sensitive value.
    pub fn new(val: T) -> Self {
        Self(val)
    }

    /// Access the underlying sensitive value explicitly.
    pub fn expose(&self) -> &T {
        &self.0
    }

    /// Consume the wrapper and return the inner sensitive value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", REDACTED_PLACEHOLDER)
    }
}

impl<T> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", REDACTED_PLACEHOLDER)
    }
}

/// Configuration options for initializing Shellumi logging.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Default log level when neither `SHELLUMI_LOG` nor `RUST_LOG` is specified.
    /// Non-negotiable default is `LevelFilter::ERROR`.
    pub default_level: LevelFilter,
    /// Optional programmatic override filter directive (e.g., `"info"`, `"shellumi_vnc=debug"`).
    pub env_filter: Option<String>,
    /// Whether to log formatted records to standard error (stderr).
    pub enable_stderr: bool,
    /// Whether to attach the native OS logging layer (`os_log` / `journald` / Windows Event Log).
    pub enable_os_log: bool,
    /// Subsystem identifier (e.g., `"dev.shellumi.app"` or `"dev.shellumi.cli"`).
    pub subsystem: String,
    /// Category identifier within the subsystem (e.g., `"desktop"`, `"cli"`, `"core"`).
    pub category: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            default_level: LevelFilter::ERROR,
            env_filter: None,
            enable_stderr: true,
            enable_os_log: true,
            subsystem: "dev.shellumi.app".to_string(),
            category: "general".to_string(),
        }
    }
}

impl LoggingConfig {
    /// Create a new configuration for the desktop application.
    pub fn for_app() -> Self {
        Self {
            subsystem: "dev.shellumi.app".to_string(),
            category: "desktop".to_string(),
            ..Default::default()
        }
    }

    /// Create a new configuration for the command-line interface.
    pub fn for_cli() -> Self {
        Self {
            subsystem: "dev.shellumi.cli".to_string(),
            category: "cli".to_string(),
            ..Default::default()
        }
    }

    /// Resolve the effective `EnvFilter` based on explicit config, environment variables,
    /// and the fallback default level (`ERROR`).
    pub fn resolve_env_filter(&self) -> EnvFilter {
        // 1. Explicit programmatic filter
        if let Some(ref custom) = self.env_filter {
            if let Ok(filter) = EnvFilter::try_new(custom) {
                return filter;
            }
        }

        // 2. SHELLUMI_LOG environment variable
        if let Ok(env_val) = std::env::var("SHELLUMI_LOG") {
            let trimmed = env_val.trim();
            if !trimmed.is_empty() {
                if let Ok(filter) = EnvFilter::try_new(trimmed) {
                    return filter;
                }
            }
        }

        // 3. RUST_LOG environment variable fallback
        if let Ok(env_val) = std::env::var("RUST_LOG") {
            let trimmed = env_val.trim();
            if !trimmed.is_empty() {
                if let Ok(filter) = EnvFilter::try_new(trimmed) {
                    return filter;
                }
            }
        }

        // 4. Fallback default level (ERROR)
        EnvFilter::default().add_directive(self.default_level.into())
    }
}

/// Error type returned during logging initialization failure.
#[derive(Debug)]
pub enum LoggingInitError {
    /// A global tracing subscriber was already registered.
    AlreadyInitialized,
    /// Underlying subscriber initialization error.
    SubscriberError(String),
}

impl fmt::Display for LoggingInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyInitialized => write!(f, "Logging subscriber is already initialized"),
            Self::SubscriberError(e) => write!(f, "Failed to initialize logging subscriber: {e}"),
        }
    }
}

impl std::error::Error for LoggingInitError {}

/// Initializes the global logging system with the specified configuration.
///
/// Thread-safe and idempotent: subsequent calls will return `Ok(())` without panicking,
/// allowing multiple integration tests and component initializations to succeed.
pub fn init_logging(config: &LoggingConfig) -> Result<(), LoggingInitError> {
    if LOGGING_INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }

    let mut result = Ok(());

    INIT_ONCE.call_once(|| {
        let filter = config.resolve_env_filter();

        // 1. Native OS Layer (Apple Unified Logging / journald / Windows Event Log)
        // Handled gracefully: returns None if the backend is unavailable.
        let os_layer = if config.enable_os_log {
            tracing_os_layer::OsLogLayer::try_new(&config.subsystem, &config.category)
        } else {
            None
        };

        // 2. Standard error formatting layer
        let stderr_layer = if config.enable_stderr {
            Some(
                sub_fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_thread_ids(false)
                    .with_file(false)
                    .with_line_number(false),
            )
        } else {
            None
        };

        // Assemble the subscriber registry
        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(os_layer)
            .with(stderr_layer);

        if let Err(e) = subscriber.try_init() {
            result = Err(LoggingInitError::SubscriberError(e.to_string()));
        } else {
            LOGGING_INITIALIZED.store(true, Ordering::Release);
        }
    });

    result
}

/// Initializes default application logging (`dev.shellumi.app`, `desktop` category).
pub fn init_app_logging() -> Result<(), LoggingInitError> {
    init_logging(&LoggingConfig::for_app())
}

/// Initializes default CLI logging (`dev.shellumi.cli`, `cli` category).
pub fn init_cli_logging() -> Result<(), LoggingInitError> {
    init_logging(&LoggingConfig::for_cli())
}

/// Returns true if the logging subsystem has been initialized.
pub fn is_logging_initialized() -> bool {
    LOGGING_INITIALIZED.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_log_level_is_error() {
        let config = LoggingConfig::default();
        assert_eq!(config.default_level, LevelFilter::ERROR);

        // Without env vars, the filter string must resolve to error
        let filter = config.resolve_env_filter();
        let filter_str = filter.to_string();
        assert_eq!(filter_str, "error");
    }

    #[test]
    fn test_app_and_cli_subsystems() {
        let app_cfg = LoggingConfig::for_app();
        assert_eq!(app_cfg.subsystem, "dev.shellumi.app");
        assert_eq!(app_cfg.category, "desktop");

        let cli_cfg = LoggingConfig::for_cli();
        assert_eq!(cli_cfg.subsystem, "dev.shellumi.cli");
        assert_eq!(cli_cfg.category, "cli");
    }

    #[test]
    fn test_redaction_helpers() {
        let secret = "super_secret_ssh_key_passphrase";
        assert_eq!(redact_sensitive(secret), "[REDACTED]");

        let wrapped = Redacted::new("my-api-token-12345");
        assert_eq!(format!("{wrapped}"), "[REDACTED]");
        assert_eq!(format!("{wrapped:?}"), "[REDACTED]");
        assert_eq!(wrapped.expose(), &"my-api-token-12345");
        assert_eq!(wrapped.into_inner(), "my-api-token-12345");
    }

    #[test]
    fn test_custom_env_filter_override() {
        let config = LoggingConfig {
            env_filter: Some("shellumi_rdp=debug,warn".to_string()),
            ..Default::default()
        };
        let filter = config.resolve_env_filter();
        assert_eq!(filter.to_string(), "shellumi_rdp=debug,warn");
    }

    #[test]
    fn test_idempotent_init() {
        let config = LoggingConfig {
            enable_stderr: false,
            enable_os_log: false,
            ..Default::default()
        };
        // Calling init multiple times must succeed without error
        let _ = init_logging(&config);
        let second = init_logging(&config);
        assert!(second.is_ok());
    }

    #[test]
    fn test_os_layer_try_new_does_not_crash() {
        // On macOS, try_new returns Some. On Linux/Windows/others, it returns Some or None.
        // It must never panic or crash.
        let layer = tracing_os_layer::OsLogLayer::try_new("dev.shellumi.test", "unit-tests");
        #[cfg(target_vendor = "apple")]
        assert!(layer.is_some());
        #[cfg(not(target_vendor = "apple"))]
        let _ = layer;
    }

    #[test]
    fn test_redacted_struct_invariants() {
        let secret_str = "private_key_pem_bytes".to_string();
        let redacted = Redacted::new(secret_str.clone());
        assert_eq!(format!("{redacted}"), REDACTED_PLACEHOLDER);
        assert_eq!(format!("{redacted:?}"), REDACTED_PLACEHOLDER);
        assert_eq!(redacted.expose(), &secret_str);
        let cloned = redacted.clone();
        assert_eq!(redacted, cloned);
        assert_eq!(cloned.into_inner(), secret_str);
    }

    #[test]
    fn test_logging_init_error_display() {
        let err1 = LoggingInitError::AlreadyInitialized;
        assert_eq!(
            format!("{err1}"),
            "Logging subscriber is already initialized"
        );
        let err2 = LoggingInitError::SubscriberError("failed to set global default".into());
        assert_eq!(
            format!("{err2}"),
            "Failed to initialize logging subscriber: failed to set global default"
        );
    }

    #[test]
    fn test_logging_state_query() {
        let _ = init_app_logging();
        assert!(is_logging_initialized());
    }
}

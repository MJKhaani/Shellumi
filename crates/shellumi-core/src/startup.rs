//! Secure host startup snippet verification, template resolution, and session readiness dispatch engine.
//!
//! Enforces safety-by-default rules:
//! - Validates and renders template variables (substituting host properties `{{host.hostname}}`, etc.)
//! - Blocks dangerous commands (destructive root filesystem changes, fork bombs, disk overwrites)
//! - Detects prompt-like output as a hint, not proof of authentication
//! - Requires explicit confirmation before dispatch; automatic dispatch is unavailable

use crate::autocomplete::PasswordPromptDetector;
use crate::workspace::check_dangerous_command;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupExecutionPolicy {
    /// Reserved for future authenticated session readiness; automatic dispatch is disabled.
    #[default]
    AutoAfterAuth,
    /// Require explicit 1-click user confirmation before sending command into the session.
    RequireConfirmation,
    /// Do not dispatch startup commands.
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupStatus {
    /// Waiting for remote host key / authentication sequence to conclude.
    PendingAuth,
    /// Prompt-like output observed; authentication is not verified.
    ReadyToDispatch,
    /// Command has been sent to the PTY stream.
    Dispatched,
    /// Execution confirmed via exit code or shell prompt mark.
    Completed,
    /// Command was rejected due to dangerous execution patterns.
    BlockedDangerous(String),
}

impl fmt::Display for StartupStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PendingAuth => f.write_str("Waiting for authentication"),
            Self::ReadyToDispatch => f.write_str("Ready to dispatch"),
            Self::Dispatched => f.write_str("Dispatched"),
            Self::Completed => f.write_str("Completed"),
            Self::BlockedDangerous(reason) => write!(f, "Blocked: {reason}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupError {
    EmptyCommand,
    DangerousCommand(String),
    MissingVariable(String),
    Serialization(String),
}

impl fmt::Display for StartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommand => f.write_str("startup command template cannot be empty"),
            Self::DangerousCommand(r) => write!(f, "destructive startup command blocked: {r}"),
            Self::MissingVariable(v) => write!(f, "missing startup variable: '{v}'"),
            Self::Serialization(e) => write!(f, "startup serialization error: {e}"),
        }
    }
}

impl std::error::Error for StartupError {}

/// Context properties supplied for host startup snippet variable rendering.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartupSnippetContext {
    pub host_id: Option<i64>,
    pub host_name: String,
    pub hostname: String,
    pub username: String,
    pub port: u16,
    pub custom_vars: HashMap<String, String>,
}

impl StartupSnippetContext {
    pub fn new(
        host_name: impl Into<String>,
        hostname: impl Into<String>,
        username: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            host_id: None,
            host_name: host_name.into(),
            hostname: hostname.into(),
            username: username.into(),
            port,
            custom_vars: HashMap::new(),
        }
    }

    pub fn with_host_id(mut self, id: i64) -> Self {
        self.host_id = Some(id);
        self
    }

    pub fn with_custom_var(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.custom_vars.insert(key.into(), val.into());
        self
    }
}

/// Resolves and renders a host startup command template, substituting host parameters
/// and performing destructive command safety interception.
pub fn prepare_startup_snippet(
    command_template: &str,
    context: &StartupSnippetContext,
) -> Result<String, StartupError> {
    let trimmed = command_template.trim();
    if trimmed.is_empty() {
        return Err(StartupError::EmptyCommand);
    }

    let mut rendered = trimmed.to_string();

    // 1. Substitute standard host properties
    if let Some(id) = context.host_id {
        rendered = rendered.replace("{{host.id}}", &id.to_string());
    }
    rendered = rendered.replace("{{host.name}}", &context.host_name);
    rendered = rendered.replace("{{host.hostname}}", &context.hostname);
    rendered = rendered.replace("{{host.user}}", &context.username);
    rendered = rendered.replace("{{host.username}}", &context.username);
    rendered = rendered.replace("{{host.port}}", &context.port.to_string());

    // 2. Substitute custom variables
    for (k, v) in &context.custom_vars {
        rendered = rendered.replace(&format!("{{{{{k}}}}}"), v);
    }

    // 3. Destructive safety check
    let safety = check_dangerous_command(&rendered);
    if let crate::workspace::BroadcastSafetyCheck::Dangerous {
        pattern,
        explanation,
    } = safety
    {
        return Err(StartupError::DangerousCommand(format!(
            "{pattern}: {explanation}"
        )));
    }

    Ok(rendered)
}

/// Live session state machine tracking shell authentication readiness
/// before dispatching startup automation commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupSessionTracker {
    pub snippet_id: Option<i64>,
    pub snippet_name: String,
    pub rendered_command: String,
    pub policy: StartupExecutionPolicy,
    pub status: StartupStatus,
    pub prompt_detected: bool,
    pub attempts: usize,
}

impl StartupSessionTracker {
    /// Creates a new startup session tracker in `PendingAuth` state.
    pub fn new(
        snippet_id: Option<i64>,
        snippet_name: impl Into<String>,
        rendered_command: impl Into<String>,
        policy: StartupExecutionPolicy,
    ) -> Self {
        Self {
            snippet_id,
            snippet_name: snippet_name.into(),
            rendered_command: rendered_command.into(),
            policy,
            status: StartupStatus::PendingAuth,
            prompt_detected: false,
            attempts: 0,
        }
    }

    /// Observes incoming terminal output bytes/text to inspect authentication status
    /// and detect shell prompt readiness.
    pub fn observe_output(&mut self, text: &str) {
        if self.status != StartupStatus::PendingAuth {
            return;
        }

        // Check if an authentication password prompt is currently active
        let detector = PasswordPromptDetector::detect(text);
        if detector.is_some() {
            // Authentication prompt active (sudo, ssh, key passphrase) - stay in PendingAuth
            return;
        }

        // Check for semantic prompt marker OSC 133;A (Prompt start)
        if text.contains("\x1b]133;A") || text.contains("\x1b]133;B") {
            self.prompt_detected = true;
            self.status = StartupStatus::ReadyToDispatch;
            return;
        }

        // Check for common shell prompt terminators on a line ($ , # , % , > )
        for line in text.lines() {
            let trimmed = line.trim();
            if (trimmed.ends_with('$')
                || trimmed.ends_with('#')
                || trimmed.ends_with('%')
                || trimmed.ends_with('>')
                || trimmed.contains("$ ")
                || trimmed.contains("# ")
                || trimmed.contains("% "))
                && !trimmed.contains("password:")
                && !trimmed.contains("Password:")
                && !trimmed.contains("passphrase:")
            {
                self.prompt_detected = true;
                self.status = StartupStatus::ReadyToDispatch;
                return;
            }
        }
    }

    /// Attempts to dispatch the startup command.
    ///
    /// Only an explicit confirmation can dispatch a command. Prompt-like output
    /// does not establish that SSH authentication or host-key verification completed.
    pub fn dispatch(&mut self, force: bool) -> Option<String> {
        if self.policy == StartupExecutionPolicy::Disabled
            || !force
            || self.status != StartupStatus::ReadyToDispatch
        {
            return None;
        }
        self.status = StartupStatus::Dispatched;
        self.attempts += 1;
        Some(format!("{}\n", self.rendered_command))
    }

    /// Marks the startup snippet execution as completed.
    pub fn mark_completed(&mut self) {
        self.status = StartupStatus::Completed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepares_valid_startup_snippet_and_substitutes_host_fields() {
        let ctx = StartupSnippetContext::new("Prod Bastion", "bastion.example.com", "ubuntu", 2222)
            .with_host_id(42)
            .with_custom_var("app", "nginx");

        let template = "echo 'Connecting to {{host.name}} ({{host.hostname}}:{{host.port}}) as {{host.user}} for {{app}}'";
        let rendered = prepare_startup_snippet(template, &ctx).expect("should succeed");
        assert_eq!(
            rendered,
            "echo 'Connecting to Prod Bastion (bastion.example.com:2222) as ubuntu for nginx'"
        );
    }

    #[test]
    fn blocks_destructive_startup_commands() {
        let ctx = StartupSnippetContext::new("Dangerous Host", "danger.local", "root", 22);
        let template = "rm -rf / --no-preserve-root";
        let err = prepare_startup_snippet(template, &ctx).unwrap_err();
        assert!(matches!(err, StartupError::DangerousCommand(_)));
    }

    #[test]
    fn session_tracker_waits_for_prompt_and_dispatches() {
        let mut tracker = StartupSessionTracker::new(
            Some(1),
            "Initialize Env",
            "source ~/.bashrc && export ENV=prod",
            StartupExecutionPolicy::AutoAfterAuth,
        );

        assert_eq!(tracker.status, StartupStatus::PendingAuth);
        assert!(tracker.dispatch(false).is_none());

        // Password prompt output should NOT transition to ready
        tracker.observe_output("admin@bastion.example.com's password: ");
        assert_eq!(tracker.status, StartupStatus::PendingAuth);
        assert!(tracker.dispatch(false).is_none());

        // Shell prompt output indicates authentication finished
        tracker.observe_output("Welcome to Ubuntu 24.04 LTS\r\nubuntu@bastion:~$ ");
        assert_eq!(tracker.status, StartupStatus::ReadyToDispatch);

        // A prompt is only a hint; it must not cause automatic dispatch.
        assert!(tracker.dispatch(false).is_none());
        let cmd = tracker
            .dispatch(true)
            .expect("explicitly confirmed payload");
        assert_eq!(cmd, "source ~/.bashrc && export ENV=prod\n");
        assert_eq!(tracker.status, StartupStatus::Dispatched);

        tracker.mark_completed();
        assert_eq!(tracker.status, StartupStatus::Completed);
    }

    #[test]
    fn session_tracker_respects_osc133_prompt_marks() {
        let mut tracker = StartupSessionTracker::new(
            Some(2),
            "Docker PS",
            "docker ps -a",
            StartupExecutionPolicy::AutoAfterAuth,
        );

        tracker.observe_output("\x1b]133;A\x07");
        assert_eq!(tracker.status, StartupStatus::ReadyToDispatch);
        assert!(tracker.dispatch(false).is_none());
        let cmd = tracker.dispatch(true).unwrap();
        assert_eq!(cmd, "docker ps -a\n");
    }

    #[test]
    fn banner_or_password_prompt_never_causes_automatic_dispatch() {
        let mut tracker = StartupSessionTracker::new(
            None,
            "test",
            "secret-command",
            StartupExecutionPolicy::AutoAfterAuth,
        );
        tracker.observe_output("Welcome to bastion >\nPassword: ");
        assert!(tracker.dispatch(false).is_none());
        tracker.observe_output("pre-login banner >");
        assert!(tracker.dispatch(false).is_none());
    }

    #[test]
    fn confirmation_cannot_dispatch_while_pending_or_disabled() {
        let mut tracker = StartupSessionTracker::new(
            None,
            "test",
            "echo safe",
            StartupExecutionPolicy::RequireConfirmation,
        );
        assert!(tracker.dispatch(true).is_none());
        tracker.observe_output("user@host:~$ ");
        assert_eq!(tracker.dispatch(true).as_deref(), Some("echo safe\n"));
        assert!(tracker.dispatch(true).is_none());
        let mut disabled =
            StartupSessionTracker::new(None, "test", "echo safe", StartupExecutionPolicy::Disabled);
        disabled.observe_output("user@host:~$ ");
        assert!(disabled.dispatch(true).is_none());
    }
}

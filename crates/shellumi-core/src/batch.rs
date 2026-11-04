use crate::snippet::{render_command, SnippetError};
use crate::SshDestination;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Target host descriptor for multi-host snippet execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchHostTarget {
    pub id: i64,
    pub name: String,
    pub destination: SshDestination,
    pub group_name: Option<String>,
    pub tags: Vec<String>,
    pub custom_variables: HashMap<String, String>,
}

impl BatchHostTarget {
    pub fn new(id: i64, name: impl Into<String>, destination: SshDestination) -> Self {
        Self {
            id,
            name: name.into(),
            destination,
            group_name: None,
            tags: Vec::new(),
            custom_variables: HashMap::new(),
        }
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group_name = Some(group.into());
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_variable(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.custom_variables.insert(key.into(), val.into());
        self
    }
}

/// Execution concurrency and sequencing policy for batch execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchPolicy {
    Sequential,
    Parallel { max_concurrency: usize },
}

impl Default for BatchPolicy {
    fn default() -> Self {
        Self::Parallel { max_concurrency: 5 }
    }
}

/// Status of an individual host execution within a batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchHostStatus {
    Queued,
    InProgress,
    Succeeded,
    Failed,
    Cancelled,
}

impl BatchHostStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::InProgress => "In Progress",
            Self::Succeeded => "Succeeded",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }

    pub fn badge_color(&self) -> (u8, u8, u8) {
        match self {
            Self::Queued => (0x6b, 0x72, 0x80),
            Self::InProgress => (0x25, 0x63, 0xeb),
            Self::Succeeded => (0x05, 0x96, 0x69),
            Self::Failed => (0xdc, 0x26, 0x26),
            Self::Cancelled => (0xd9, 0x77, 0x06),
        }
    }
}

/// Result of executing a rendered snippet on an individual host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchHostResult {
    pub host_id: i64,
    pub host_name: String,
    pub destination: String,
    pub status: BatchHostStatus,
    pub command_rendered: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// A comprehensive plan for executing a snippet across multiple hosts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchExecutionPlan {
    pub snippet_id: i64,
    pub snippet_name: String,
    pub template_command: String,
    pub global_variables: HashMap<String, String>,
    pub targets: Vec<BatchHostTarget>,
    pub policy: BatchPolicy,
}

impl BatchExecutionPlan {
    pub fn new(
        snippet_id: i64,
        snippet_name: impl Into<String>,
        template_command: impl Into<String>,
        targets: Vec<BatchHostTarget>,
    ) -> Self {
        Self {
            snippet_id,
            snippet_name: snippet_name.into(),
            template_command: template_command.into(),
            global_variables: HashMap::new(),
            targets,
            policy: BatchPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: BatchPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_global_variable(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.global_variables.insert(key.into(), value.into());
        self
    }

    pub fn with_global_variables(
        mut self,
        vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (k, v) in vars {
            self.global_variables.insert(k.into(), v.into());
        }
        self
    }

    /// Renders the snippet command for a specific target host by merging global variables,
    /// host custom variables, and automatic host property variables:
    /// - `{{host.id}}`
    /// - `{{host.name}}`
    /// - `{{host.hostname}}`
    /// - `{{host.username}}` / `{{host.user}}`
    /// - `{{host.port}}`
    /// - `{{host.group}}`
    /// - `{{host.tags}}`
    pub fn render_command_for_target(
        &self,
        target: &BatchHostTarget,
    ) -> Result<String, SnippetError> {
        let mut merged = self.global_variables.clone();

        // Target custom variables
        for (k, v) in &target.custom_variables {
            merged.insert(k.clone(), v.clone());
        }

        // Automatic host properties
        merged.insert("host.id".into(), target.id.to_string());
        merged.insert("host.name".into(), target.name.clone());
        merged.insert(
            "host.hostname".into(),
            target.destination.host().to_string(),
        );
        merged.insert(
            "host.username".into(),
            target.destination.username().to_string(),
        );
        merged.insert(
            "host.user".into(),
            target.destination.username().to_string(),
        );
        merged.insert(
            "host.port".into(),
            target.destination.port().get().to_string(),
        );
        merged.insert(
            "host.group".into(),
            target.group_name.clone().unwrap_or_default(),
        );
        merged.insert("host.tags".into(), target.tags.join(","));

        render_command(&self.template_command, &merged)
    }

    /// Previews rendered commands for all targets without execution.
    pub fn preview_all(&self) -> Vec<(BatchHostTarget, Result<String, SnippetError>)> {
        self.targets
            .iter()
            .map(|t| (t.clone(), self.render_command_for_target(t)))
            .collect()
    }
}

/// Aggregate summary metrics for a batch execution run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BatchExecutionSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub in_progress: usize,
    pub queued: usize,
    pub is_complete: bool,
    pub total_duration_ms: u64,
}

/// Orchestrates multi-host snippet execution with cancellation and summary tracking.
pub struct BatchRunner {
    plan: BatchExecutionPlan,
    results: Vec<BatchHostResult>,
    cancelled: bool,
}

impl BatchRunner {
    pub fn new(plan: BatchExecutionPlan) -> Self {
        let results = plan
            .targets
            .iter()
            .map(|t| BatchHostResult {
                host_id: t.id,
                host_name: t.name.clone(),
                destination: format!(
                    "{}@{}:{}",
                    t.destination.username(),
                    t.destination.host(),
                    t.destination.port()
                ),
                status: BatchHostStatus::Queued,
                command_rendered: String::new(),
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
                duration_ms: 0,
                error: None,
            })
            .collect();

        Self {
            plan,
            results,
            cancelled: false,
        }
    }

    pub fn plan(&self) -> &BatchExecutionPlan {
        &self.plan
    }

    pub fn results(&self) -> &[BatchHostResult] {
        &self.results
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        for res in &mut self.results {
            if res.status == BatchHostStatus::Queued {
                res.status = BatchHostStatus::Cancelled;
                res.error = Some("Execution cancelled before start".into());
            }
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Computes current progress summary.
    pub fn summary(&self) -> BatchExecutionSummary {
        let mut summary = BatchExecutionSummary {
            total: self.results.len(),
            ..Default::default()
        };

        for res in &self.results {
            match res.status {
                BatchHostStatus::Queued => summary.queued += 1,
                BatchHostStatus::InProgress => summary.in_progress += 1,
                BatchHostStatus::Succeeded => summary.succeeded += 1,
                BatchHostStatus::Failed => summary.failed += 1,
                BatchHostStatus::Cancelled => summary.cancelled += 1,
            }
            summary.total_duration_ms += res.duration_ms;
        }

        summary.is_complete = summary.queued == 0 && summary.in_progress == 0;
        summary
    }

    /// Executes the batch plan synchronously using the provided command runner closure.
    /// The closure signature: `(target, rendered_command) -> Result<(exit_code, stdout, stderr), error_message>`.
    pub fn execute_sync<F>(&mut self, mut runner: F) -> BatchExecutionSummary
    where
        F: FnMut(&BatchHostTarget, &str) -> Result<(i32, String, String), String>,
    {
        for idx in 0..self.plan.targets.len() {
            if self.cancelled {
                if self.results[idx].status == BatchHostStatus::Queued {
                    self.results[idx].status = BatchHostStatus::Cancelled;
                    self.results[idx].error = Some("Execution cancelled".into());
                }
                continue;
            }

            let target = &self.plan.targets[idx];
            self.results[idx].status = BatchHostStatus::InProgress;

            let rendered = match self.plan.render_command_for_target(target) {
                Ok(cmd) => cmd,
                Err(err) => {
                    self.results[idx].status = BatchHostStatus::Failed;
                    self.results[idx].error = Some(err.to_string());
                    continue;
                }
            };

            self.results[idx].command_rendered = rendered.clone();

            let start = std::time::Instant::now();
            match runner(target, &rendered) {
                Ok((exit_code, stdout, stderr)) => {
                    let duration = start.elapsed().as_millis() as u64;
                    self.results[idx].duration_ms = duration;
                    self.results[idx].stdout = stdout;
                    self.results[idx].stderr = stderr;
                    self.results[idx].exit_code = Some(exit_code);

                    if exit_code == 0 {
                        self.results[idx].status = BatchHostStatus::Succeeded;
                    } else {
                        self.results[idx].status = BatchHostStatus::Failed;
                        self.results[idx].error =
                            Some(format!("Command exited with non-zero code {exit_code}"));
                    }
                }
                Err(err) => {
                    let duration = start.elapsed().as_millis() as u64;
                    self.results[idx].duration_ms = duration;
                    self.results[idx].status = BatchHostStatus::Failed;
                    self.results[idx].error = Some(err);
                }
            }
        }

        self.summary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_target(id: i64, name: &str, host: &str, user: &str, port: u16) -> BatchHostTarget {
        let dest = SshDestination::new(host, user, port).unwrap();
        BatchHostTarget::new(id, name, dest)
    }

    #[test]
    fn verifies_target_property_substitution_in_snippet() {
        let t1 = sample_target(1, "web-01", "192.168.1.10", "deploy", 22)
            .with_group("Production")
            .with_tags(["web", "frontend"]);
        let t2 = sample_target(2, "db-01", "192.168.1.20", "postgres", 5432)
            .with_group("Database")
            .with_tags(["db"]);

        let plan = BatchExecutionPlan::new(
            100,
            "Health Check",
            "echo 'Checking {{host.name}} at {{host.hostname}}:{{host.port}} for {{host.user}} in {{host.group}} with {{check_type:-ping}}'",
            vec![t1.clone(), t2.clone()],
        ).with_global_variable("check_type", "deep_scan");

        let cmd1 = plan.render_command_for_target(&t1).unwrap();
        assert_eq!(
            cmd1,
            "echo 'Checking web-01 at 192.168.1.10:22 for deploy in Production with deep_scan'"
        );

        let cmd2 = plan.render_command_for_target(&t2).unwrap();
        assert_eq!(
            cmd2,
            "echo 'Checking db-01 at 192.168.1.20:5432 for postgres in Database with deep_scan'"
        );
    }

    #[test]
    fn verifies_custom_variable_override_and_missing_parameter_detection() {
        let t = sample_target(1, "custom-node", "10.0.0.5", "admin", 22)
            .with_variable("app_port", "8080");

        let plan = BatchExecutionPlan::new(
            101,
            "Restart App",
            "systemctl restart app@{{app_port}} --token={{auth_token}}",
            vec![t.clone()],
        );

        // Missing auth_token should fail
        let err = plan.render_command_for_target(&t).unwrap_err();
        assert!(matches!(err, SnippetError::MissingVariable(name) if name == "auth_token"));

        let plan_with_token = plan.with_global_variable("auth_token", "secret123");
        let rendered = plan_with_token.render_command_for_target(&t).unwrap();
        assert_eq!(rendered, "systemctl restart app@8080 --token=secret123");
    }

    #[test]
    fn verifies_batch_runner_execution_and_summary() {
        let t1 = sample_target(1, "node-1", "10.0.0.1", "root", 22);
        let t2 = sample_target(2, "node-2", "10.0.0.2", "root", 22);
        let t3 = sample_target(3, "node-3", "10.0.0.3", "root", 22);

        let plan = BatchExecutionPlan::new(200, "Uptime Check", "uptime", vec![t1, t2, t3]);

        let mut runner = BatchRunner::new(plan);
        assert_eq!(runner.summary().queued, 3);
        assert!(!runner.summary().is_complete);

        let summary = runner.execute_sync(|target, _cmd| {
            if target.id == 2 {
                Err("Connection timeout to 10.0.0.2".into())
            } else if target.id == 3 {
                Ok((1, String::new(), "Permission denied".into()))
            } else {
                Ok((0, "load average: 0.15, 0.20, 0.05".into(), String::new()))
            }
        });

        assert_eq!(summary.total, 3);
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.cancelled, 0);
        assert!(summary.is_complete);

        let results = runner.results();
        assert_eq!(results[0].status, BatchHostStatus::Succeeded);
        assert_eq!(results[0].stdout, "load average: 0.15, 0.20, 0.05");
        assert_eq!(results[1].status, BatchHostStatus::Failed);
        assert_eq!(
            results[1].error.as_deref(),
            Some("Connection timeout to 10.0.0.2")
        );
        assert_eq!(results[2].status, BatchHostStatus::Failed);
        assert_eq!(results[2].exit_code, Some(1));
    }

    #[test]
    fn verifies_batch_runner_cancellation() {
        let t1 = sample_target(1, "node-1", "10.0.0.1", "root", 22);
        let t2 = sample_target(2, "node-2", "10.0.0.2", "root", 22);

        let plan =
            BatchExecutionPlan::new(201, "Upgrade", "apt update && apt upgrade -y", vec![t1, t2]);

        let mut runner = BatchRunner::new(plan);
        runner.cancel();
        assert!(runner.is_cancelled());

        let summary = runner.summary();
        assert_eq!(summary.cancelled, 2);
        assert_eq!(summary.queued, 0);
        assert!(summary.is_complete);
    }
}

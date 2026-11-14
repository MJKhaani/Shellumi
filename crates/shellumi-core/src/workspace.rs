use crate::SshDestination;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Layout arrangement of terminal panes within a workspace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceLayout {
    #[default]
    Single,
    SplitVertical,
    SplitHorizontal,
    DualGrid,
}

impl WorkspaceLayout {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::SplitVertical => "vertical",
            Self::SplitHorizontal => "horizontal",
            Self::DualGrid => "grid",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "single" | "s" => Some(Self::Single),
            "split_vertical" | "vertical" | "v" => Some(Self::SplitVertical),
            "split_horizontal" | "horizontal" | "h" => Some(Self::SplitHorizontal),
            "dual_grid" | "grid" | "g" => Some(Self::DualGrid),
            _ => None,
        }
    }
}

/// Type of session running in a workspace pane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSessionKind {
    #[default]
    Ssh,
    Local,
    Sftp,
    Vnc,
    Rdp,
}

impl WorkspaceSessionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Local => "local",
            Self::Sftp => "sftp",
            Self::Vnc => "vnc",
            Self::Rdp => "rdp",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "ssh" => Some(Self::Ssh),
            "local" => Some(Self::Local),
            "sftp" => Some(Self::Sftp),
            "vnc" => Some(Self::Vnc),
            "rdp" => Some(Self::Rdp),
            _ => None,
        }
    }
}

/// Individual terminal session within a workspace layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSession {
    pub name: String,
    pub host_id: Option<i64>,
    pub destination: Option<SshDestination>,
    pub kind: WorkspaceSessionKind,
    pub working_directory: Option<String>,
    pub startup_command: Option<String>,
    pub pane_index: usize,
    pub excluded_from_broadcast: bool,
}

impl WorkspaceSession {
    pub fn new(name: impl Into<String>, kind: WorkspaceSessionKind, pane_index: usize) -> Self {
        Self {
            name: name.into(),
            host_id: None,
            destination: None,
            kind,
            working_directory: None,
            startup_command: None,
            pane_index,
            excluded_from_broadcast: false,
        }
    }

    pub fn with_host(mut self, host_id: i64) -> Self {
        self.host_id = Some(host_id);
        self
    }

    pub fn with_destination(mut self, destination: SshDestination) -> Self {
        self.destination = Some(destination);
        self
    }

    pub fn with_startup_command(mut self, cmd: impl Into<String>) -> Self {
        self.startup_command = Some(cmd.into());
        self
    }

    pub fn with_working_directory(mut self, dir: impl Into<String>) -> Self {
        self.working_directory = Some(dir.into());
        self
    }

    pub fn with_broadcast_excluded(mut self, excluded: bool) -> Self {
        self.excluded_from_broadcast = excluded;
        self
    }
}

/// Workspace organization unit managing multi-session configurations and layouts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub layout: WorkspaceLayout,
    pub sessions: Vec<WorkspaceSession>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Workspace {
    pub fn new(name: impl Into<String>) -> Result<Self, WorkspaceError> {
        let name_str = name.into();
        let trimmed = name_str.trim().to_string();
        if trimmed.is_empty() {
            return Err(WorkspaceError::InvalidName);
        }
        Ok(Self {
            id: 0,
            name: trimmed,
            description: None,
            layout: WorkspaceLayout::default(),
            sessions: Vec::new(),
            tags: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        })
    }

    pub fn with_layout(mut self, layout: WorkspaceLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_session(mut self, session: WorkspaceSession) -> Self {
        self.sessions.push(session);
        self
    }

    pub fn with_sessions(mut self, sessions: impl IntoIterator<Item = WorkspaceSession>) -> Self {
        self.sessions.extend(sessions);
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn duplicate(&self, new_name: impl Into<String>) -> Result<Self, WorkspaceError> {
        let mut dup = Self::new(new_name)?;
        dup.description = self.description.clone();
        dup.layout = self.layout;
        dup.sessions = self.sessions.clone();
        dup.tags = self.tags.clone();
        Ok(dup)
    }

    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.name.trim().is_empty() {
            return Err(WorkspaceError::InvalidName);
        }
        for (idx, s) in self.sessions.iter().enumerate() {
            if s.name.trim().is_empty() {
                return Err(WorkspaceError::InvalidSessionName(idx));
            }
        }
        Ok(())
    }
}

/// Broadcast manager tracking multi-terminal broadcast input and safety checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BroadcastManager {
    enabled: bool,
    total_panes: usize,
    excluded_panes: HashSet<usize>,
}

/// Safety evaluation result when typing or broadcasting input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BroadcastSafetyCheck {
    Safe,
    Dangerous {
        pattern: String,
        explanation: String,
    },
}

impl BroadcastManager {
    pub fn new(total_panes: usize) -> Self {
        Self {
            enabled: false,
            total_panes,
            excluded_panes: HashSet::new(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.enabled
    }

    pub fn set_total_panes(&mut self, total: usize) {
        self.total_panes = total;
        self.excluded_panes.retain(|&idx| idx < total);
    }

    pub fn is_pane_excluded(&self, pane_idx: usize) -> bool {
        self.excluded_panes.contains(&pane_idx)
    }

    pub fn toggle_pane_exclusion(&mut self, pane_idx: usize) {
        if !self.excluded_panes.remove(&pane_idx) {
            self.excluded_panes.insert(pane_idx);
        }
    }

    pub fn exclude_pane(&mut self, pane_idx: usize) {
        self.excluded_panes.insert(pane_idx);
    }

    pub fn include_pane(&mut self, pane_idx: usize) {
        self.excluded_panes.remove(&pane_idx);
    }

    pub fn receiving_panes_count(&self) -> usize {
        if !self.enabled || self.total_panes == 0 {
            0
        } else {
            self.total_panes.saturating_sub(self.excluded_panes.len())
        }
    }

    pub fn status_text(&self) -> String {
        if !self.enabled {
            "Broadcast: Off".to_string()
        } else {
            let active = self.receiving_panes_count();
            format!(
                "📡 Broadcast: ON ({}/{} panes receiving)",
                active, self.total_panes
            )
        }
    }

    /// Evaluates if an input string or command line represents a dangerous command
    /// across multiple broadcast terminals (Section 3.8: Require confirmation for dangerous multi-host operations).
    pub fn check_dangerous_command(input: &str) -> BroadcastSafetyCheck {
        let trimmed = input.trim();
        let lower = trimmed.to_lowercase();

        // Destructive filesystem removal
        if lower.starts_with("rm -rf /")
            || lower.starts_with("rm -fr /")
            || lower.contains(" rm -rf /")
            || lower.contains(" rm -fr /")
            || lower.starts_with("rm -rf /*")
            || lower.starts_with("rm -rf *")
            || lower == "rm -rf ."
        {
            return BroadcastSafetyCheck::Dangerous {
                pattern: "rm -rf /".to_string(),
                explanation: "Recursive root/wildcard file deletion".to_string(),
            };
        }

        // Filesystem formatting & raw block writes
        if lower.starts_with("mkfs") || lower.contains(" mkfs") {
            return BroadcastSafetyCheck::Dangerous {
                pattern: "mkfs".to_string(),
                explanation: "Filesystem format command detected".to_string(),
            };
        }
        if (lower.starts_with("dd ") || lower.contains(" dd ")) && lower.contains("of=/dev/") {
            return BroadcastSafetyCheck::Dangerous {
                pattern: "dd of=/dev/".to_string(),
                explanation: "Raw block device write command detected".to_string(),
            };
        }

        // Forkbomb
        if lower.contains(":(){ :|:& };:") {
            return BroadcastSafetyCheck::Dangerous {
                pattern: ":(){ :|:& };:".to_string(),
                explanation: "Bash fork bomb detected".to_string(),
            };
        }

        // System shutdown / reboot commands
        if lower == "reboot"
            || lower == "shutdown -h now"
            || lower == "shutdown -r now"
            || lower == "poweroff"
            || lower == "init 0"
            || lower == "init 6"
            || lower.starts_with("reboot ")
            || lower.starts_with("shutdown ")
        {
            return BroadcastSafetyCheck::Dangerous {
                pattern: trimmed.to_string(),
                explanation: "System shutdown/reboot command broadcast".to_string(),
            };
        }

        // Dangerous iptables / ufw flush
        if lower == "iptables -f" || lower == "ufw disable" {
            return BroadcastSafetyCheck::Dangerous {
                pattern: trimmed.to_string(),
                explanation: "Firewall flush / disable command".to_string(),
            };
        }

        BroadcastSafetyCheck::Safe
    }
}

/// Evaluates if an input command represents a dangerous broadcast operation.
pub fn check_dangerous_command(input: &str) -> BroadcastSafetyCheck {
    BroadcastManager::check_dangerous_command(input)
}

/// Errors related to workspace validation and lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceError {
    InvalidName,
    InvalidSessionName(usize),
    Serialization(String),
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName => write!(f, "Workspace name cannot be empty or only whitespace."),
            Self::InvalidSessionName(idx) => {
                write!(f, "Session name at index {idx} cannot be empty.")
            }
            Self::Serialization(err) => write!(f, "Session serialization error: {err}"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_workspace_creation_and_rejections() {
        assert_eq!(Workspace::new(""), Err(WorkspaceError::InvalidName));
        assert_eq!(Workspace::new("   "), Err(WorkspaceError::InvalidName));

        let ws = Workspace::new("Dev Environment").unwrap();
        assert_eq!(ws.name, "Dev Environment");
        assert_eq!(ws.layout, WorkspaceLayout::Single);
        assert!(ws.sessions.is_empty());
        assert!(ws.validate().is_ok());

        let invalid_session = WorkspaceSession::new("", WorkspaceSessionKind::Ssh, 0);
        let mut ws_invalid = Workspace::new("Test").unwrap();
        ws_invalid.sessions.push(invalid_session);
        assert_eq!(
            ws_invalid.validate(),
            Err(WorkspaceError::InvalidSessionName(0))
        );
    }

    #[test]
    fn parses_workspace_layout_and_session_kinds() {
        assert_eq!(
            WorkspaceLayout::parse("single"),
            Some(WorkspaceLayout::Single)
        );
        assert_eq!(
            WorkspaceLayout::parse("vertical"),
            Some(WorkspaceLayout::SplitVertical)
        );
        assert_eq!(
            WorkspaceLayout::parse("horizontal"),
            Some(WorkspaceLayout::SplitHorizontal)
        );
        assert_eq!(
            WorkspaceLayout::parse("grid"),
            Some(WorkspaceLayout::DualGrid)
        );
        assert_eq!(WorkspaceLayout::parse("invalid"), None);

        assert_eq!(
            WorkspaceSessionKind::parse("ssh"),
            Some(WorkspaceSessionKind::Ssh)
        );
        assert_eq!(
            WorkspaceSessionKind::parse("local"),
            Some(WorkspaceSessionKind::Local)
        );
        assert_eq!(
            WorkspaceSessionKind::parse("sftp"),
            Some(WorkspaceSessionKind::Sftp)
        );
        assert_eq!(
            WorkspaceSessionKind::parse("vnc"),
            Some(WorkspaceSessionKind::Vnc)
        );
        assert_eq!(
            WorkspaceSessionKind::parse("rdp"),
            Some(WorkspaceSessionKind::Rdp)
        );
        assert_eq!(WorkspaceSessionKind::parse("ftp"), None);
    }

    #[test]
    fn manages_broadcast_input_lifecycle_and_exclusions() {
        let mut bm = BroadcastManager::new(4);
        assert!(!bm.is_enabled());
        assert_eq!(bm.receiving_panes_count(), 0);
        assert_eq!(bm.status_text(), "Broadcast: Off");

        bm.enable();
        assert!(bm.is_enabled());
        assert_eq!(bm.receiving_panes_count(), 4);
        assert_eq!(bm.status_text(), "📡 Broadcast: ON (4/4 panes receiving)");

        // Exclude pane 1
        bm.exclude_pane(1);
        assert!(bm.is_pane_excluded(1));
        assert_eq!(bm.receiving_panes_count(), 3);
        assert_eq!(bm.status_text(), "📡 Broadcast: ON (3/4 panes receiving)");

        // Toggle exclusion back
        bm.toggle_pane_exclusion(1);
        assert!(!bm.is_pane_excluded(1));
        assert_eq!(bm.receiving_panes_count(), 4);

        // Resize total panes
        bm.exclude_pane(3);
        bm.set_total_panes(2);
        assert!(!bm.is_pane_excluded(3)); // Retained pane index cleaned up
        assert_eq!(bm.receiving_panes_count(), 2);
    }

    #[test]
    fn detects_dangerous_commands_in_broadcast_input() {
        // Safe commands
        assert_eq!(
            BroadcastManager::check_dangerous_command("ls -la"),
            BroadcastSafetyCheck::Safe
        );
        assert_eq!(
            BroadcastManager::check_dangerous_command("git status"),
            BroadcastSafetyCheck::Safe
        );
        assert_eq!(
            BroadcastManager::check_dangerous_command("tail -f /var/log/syslog"),
            BroadcastSafetyCheck::Safe
        );

        // Dangerous commands
        assert!(matches!(
            BroadcastManager::check_dangerous_command("rm -rf /"),
            BroadcastSafetyCheck::Dangerous { .. }
        ));
        assert!(matches!(
            BroadcastManager::check_dangerous_command("sudo rm -rf /*"),
            BroadcastSafetyCheck::Dangerous { .. }
        ));
        assert!(matches!(
            BroadcastManager::check_dangerous_command("mkfs.ext4 /dev/sda1"),
            BroadcastSafetyCheck::Dangerous { .. }
        ));
        assert!(matches!(
            BroadcastManager::check_dangerous_command("dd if=/dev/zero of=/dev/sda"),
            BroadcastSafetyCheck::Dangerous { .. }
        ));
        assert!(matches!(
            BroadcastManager::check_dangerous_command(":(){ :|:& };:"),
            BroadcastSafetyCheck::Dangerous { .. }
        ));
        assert!(matches!(
            BroadcastManager::check_dangerous_command("reboot"),
            BroadcastSafetyCheck::Dangerous { .. }
        ));
        assert!(matches!(
            BroadcastManager::check_dangerous_command("shutdown -h now"),
            BroadcastSafetyCheck::Dangerous { .. }
        ));
        assert!(matches!(
            BroadcastManager::check_dangerous_command("iptables -F"),
            BroadcastSafetyCheck::Dangerous { .. }
        ));
    }

    #[test]
    fn duplicates_workspace_correctly() {
        let session = WorkspaceSession::new("Web Node", WorkspaceSessionKind::Ssh, 0)
            .with_host(10)
            .with_startup_command("htop");
        let ws = Workspace::new("Cluster Ops")
            .unwrap()
            .with_layout(WorkspaceLayout::SplitVertical)
            .with_description("Multi-node operations")
            .with_session(session)
            .with_tags(["prod", "ops"]);

        let dup = ws.duplicate("Cluster Ops (Copy)").unwrap();
        assert_eq!(dup.name, "Cluster Ops (Copy)");
        assert_eq!(dup.description, ws.description);
        assert_eq!(dup.layout, WorkspaceLayout::SplitVertical);
        assert_eq!(dup.sessions.len(), 1);
        assert_eq!(dup.sessions[0].name, "Web Node");
        assert_eq!(dup.sessions[0].startup_command.as_deref(), Some("htop"));
        assert_eq!(dup.tags, vec!["prod", "ops"]);
    }
}

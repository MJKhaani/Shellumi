use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Errors arising during team collaboration and shared vault operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollaborationError {
    Unauthorized(String),
    MemberNotFound(String),
    MemberAlreadyExists(String),
    MemberRevoked(String),
    InvalidRole(String),
    InvalidAction(String),
    SessionNotActive,
    NoActiveWriter,
    AlreadyActiveWriter(String),
    InvalidVaultName(String),
    KeyRotationFailed(String),
}

impl fmt::Display for CollaborationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized(msg) => write!(f, "Unauthorized collaboration action: {msg}"),
            Self::MemberNotFound(id) => write!(f, "Team member not found: {id}"),
            Self::MemberAlreadyExists(id) => write!(f, "Team member already enrolled: {id}"),
            Self::MemberRevoked(id) => write!(f, "Team member access is revoked: {id}"),
            Self::InvalidRole(r) => write!(f, "Invalid team role: '{r}'"),
            Self::InvalidAction(a) => write!(f, "Invalid audit action: '{a}'"),
            Self::SessionNotActive => write!(f, "Multiplayer collaboration session is not active"),
            Self::NoActiveWriter => write!(f, "No participant currently holds keyboard control"),
            Self::AlreadyActiveWriter(id) => {
                write!(f, "User '{id}' already holds keyboard control")
            }
            Self::InvalidVaultName(name) => write!(f, "Invalid shared vault name: '{name}'"),
            Self::KeyRotationFailed(msg) => write!(f, "Vault key rotation failed: {msg}"),
        }
    }
}

impl std::error::Error for CollaborationError {}

/// Role hierarchy within a shared team vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    Viewer = 0,
    Editor = 1,
    Administrator = 2,
    Owner = 3,
}

impl TeamRole {
    /// Returns true if this role can manage team members (invite, change roles, revoke).
    pub fn can_manage_members(&self) -> bool {
        matches!(self, Self::Owner | Self::Administrator)
    }

    /// Returns true if this role can reassign owner or administrator privileges.
    pub fn can_manage_roles(&self) -> bool {
        matches!(self, Self::Owner)
    }

    /// Returns true if this role can trigger vault cryptographic key rotation.
    pub fn can_rotate_key(&self) -> bool {
        matches!(self, Self::Owner | Self::Administrator)
    }

    /// Returns true if this role can create, modify, or delete shared vault items.
    pub fn can_edit_records(&self) -> bool {
        matches!(self, Self::Owner | Self::Administrator | Self::Editor)
    }

    /// Returns true if this role can read shared vault items.
    pub fn can_view_records(&self) -> bool {
        true
    }

    /// Returns true if this role can inspect audit logs for the vault.
    pub fn can_view_audit_log(&self) -> bool {
        matches!(self, Self::Owner | Self::Administrator)
    }

    /// Returns true if this role can initiate multiplayer terminal sharing sessions.
    pub fn can_manage_terminal_session(&self) -> bool {
        matches!(self, Self::Owner | Self::Administrator | Self::Editor)
    }

    /// Returns human-readable UI badge title.
    pub fn badge_label(&self) -> &'static str {
        match self {
            Self::Owner => "👑 Owner",
            Self::Administrator => "🛡️ Admin",
            Self::Editor => "✏️ Editor",
            Self::Viewer => "👁️ Viewer",
        }
    }

    /// String identifier for serialization/storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Administrator => "admin",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }
}

impl fmt::Display for TeamRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for TeamRole {
    type Err = CollaborationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "owner" => Ok(Self::Owner),
            "admin" | "administrator" => Ok(Self::Administrator),
            "editor" | "write" => Ok(Self::Editor),
            "viewer" | "read" | "readonly" => Ok(Self::Viewer),
            other => Err(CollaborationError::InvalidRole(other.to_string())),
        }
    }
}

/// A team collaborator enrolled in a shared vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMember {
    pub id: String,
    pub vault_id: String,
    pub user_id: String,
    pub display_name: String,
    pub public_key: String,
    pub role: TeamRole,
    pub enrolled_at: String,
    pub invited_by: String,
    pub revoked: bool,
    pub revoked_at: Option<String>,
}

impl TeamMember {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        vault_id: impl Into<String>,
        user_id: impl Into<String>,
        display_name: impl Into<String>,
        public_key: impl Into<String>,
        role: TeamRole,
        enrolled_at: impl Into<String>,
        invited_by: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            vault_id: vault_id.into(),
            user_id: user_id.into(),
            display_name: display_name.into(),
            public_key: public_key.into(),
            role,
            enrolled_at: enrolled_at.into(),
            invited_by: invited_by.into(),
            revoked: false,
            revoked_at: None,
        }
    }

    /// Revokes member access to future vault operations.
    ///
    /// Note: Revocation cryptographically prevents reading future re-keyed epochs,
    /// but cannot erase records already downloaded prior to revocation.
    pub fn revoke(&mut self, timestamp: impl Into<String>) {
        self.revoked = true;
        self.revoked_at = Some(timestamp.into());
    }

    /// Changes the role assigned to this collaborator.
    pub fn change_role(&mut self, new_role: TeamRole) {
        self.role = new_role;
    }

    /// Returns true if this collaborator has active, non-revoked access.
    pub fn is_active(&self) -> bool {
        !self.revoked
    }
}

/// A shared team vault containing collaborative infrastructure definitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedVault {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub current_epoch: u64,
    pub owner_id: String,
    pub created_at: String,
    pub updated_at: String,
}

impl SharedVault {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: Option<String>,
        owner_id: impl Into<String>,
        created_at: impl Into<String>,
    ) -> Result<Self, CollaborationError> {
        let name_str = name.into();
        let trimmed = name_str.trim();
        if trimmed.is_empty() {
            return Err(CollaborationError::InvalidVaultName(
                "Vault name cannot be empty".to_string(),
            ));
        }

        let created_str = created_at.into();
        Ok(Self {
            id: id.into(),
            name: trimmed.to_string(),
            description,
            current_epoch: 1,
            owner_id: owner_id.into(),
            created_at: created_str.clone(),
            updated_at: created_str,
        })
    }

    /// Increments the cryptographic epoch counter upon key rotation.
    pub fn rotate_key(&mut self, updated_at: impl Into<String>) -> u64 {
        self.current_epoch += 1;
        self.updated_at = updated_at.into();
        self.current_epoch
    }

    /// Checks if a given user identifier is the primary owner of this vault.
    pub fn is_owner(&self, user_id: &str) -> bool {
        self.owner_id == user_id
    }
}

/// Action types recorded in the team sharing audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    VaultCreated,
    MemberInvited,
    MemberEnrolled,
    MemberRoleUpdated,
    MemberRevoked,
    KeyRotated,
    RecordShared,
    CollaborationSessionStarted,
    KeyboardControlGranted,
    KeyboardControlRevoked,
    CollaborationSessionEnded,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VaultCreated => "vault_created",
            Self::MemberInvited => "member_invited",
            Self::MemberEnrolled => "member_enrolled",
            Self::MemberRoleUpdated => "member_role_updated",
            Self::MemberRevoked => "member_revoked",
            Self::KeyRotated => "key_rotated",
            Self::RecordShared => "record_shared",
            Self::CollaborationSessionStarted => "collaboration_session_started",
            Self::KeyboardControlGranted => "keyboard_control_granted",
            Self::KeyboardControlRevoked => "keyboard_control_revoked",
            Self::CollaborationSessionEnded => "collaboration_session_ended",
        }
    }
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for AuditAction {
    type Err = CollaborationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "vault_created" => Ok(Self::VaultCreated),
            "member_invited" => Ok(Self::MemberInvited),
            "member_enrolled" => Ok(Self::MemberEnrolled),
            "member_role_updated" => Ok(Self::MemberRoleUpdated),
            "member_revoked" => Ok(Self::MemberRevoked),
            "key_rotated" => Ok(Self::KeyRotated),
            "record_shared" => Ok(Self::RecordShared),
            "collaboration_session_started" => Ok(Self::CollaborationSessionStarted),
            "keyboard_control_granted" => Ok(Self::KeyboardControlGranted),
            "keyboard_control_revoked" => Ok(Self::KeyboardControlRevoked),
            "collaboration_session_ended" => Ok(Self::CollaborationSessionEnded),
            other => Err(CollaborationError::InvalidAction(other.to_string())),
        }
    }
}

/// Tamper-evident audit record documenting administrative and sharing operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharingAuditRecord {
    pub id: String,
    pub vault_id: String,
    pub action: AuditAction,
    pub actor_id: String,
    pub target_id: Option<String>,
    pub details: Option<String>,
    pub timestamp_utc: String,
}

impl SharingAuditRecord {
    pub fn new(
        id: impl Into<String>,
        vault_id: impl Into<String>,
        action: AuditAction,
        actor_id: impl Into<String>,
        target_id: Option<String>,
        details: Option<String>,
        timestamp_utc: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            vault_id: vault_id.into(),
            action,
            actor_id: actor_id.into(),
            target_id,
            details,
            timestamp_utc: timestamp_utc.into(),
        }
    }

    /// Formats a human-readable audit log line.
    pub fn format_entry(&self) -> String {
        let target = self
            .target_id
            .as_deref()
            .map(|t| format!(" target={t}"))
            .unwrap_or_default();
        let details = self
            .details
            .as_deref()
            .map(|d| format!(" ({d})"))
            .unwrap_or_default();
        format!(
            "[{}] {} by actor={}{}{}",
            self.timestamp_utc,
            self.action.as_str(),
            self.actor_id,
            target,
            details
        )
    }
}

/// A participant in an active multiplayer terminal session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiplayerParticipant {
    pub user_id: String,
    pub display_name: String,
    pub is_writer: bool,
    pub joined_at: String,
}

/// State machine for a real-time multiplayer terminal session.
/// Enforces single-active-writer semantics with immediate owner revocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiplayerSession {
    pub session_id: String,
    pub vault_id: Option<String>,
    pub owner_id: String,
    pub current_controller: Option<String>,
    pub participants: Vec<MultiplayerParticipant>,
    pub is_active: bool,
    pub started_at: String,
}

impl MultiplayerSession {
    pub fn new(
        session_id: impl Into<String>,
        vault_id: Option<String>,
        owner_id: impl Into<String>,
        started_at: impl Into<String>,
    ) -> Self {
        let owner = owner_id.into();
        let started = started_at.into();
        let owner_participant = MultiplayerParticipant {
            user_id: owner.clone(),
            display_name: format!("{owner} (Host)"),
            is_writer: true,
            joined_at: started.clone(),
        };

        Self {
            session_id: session_id.into(),
            vault_id,
            owner_id: owner.clone(),
            current_controller: Some(owner),
            participants: vec![owner_participant],
            is_active: true,
            started_at: started,
        }
    }

    /// Registers a new participant as a viewer.
    pub fn add_participant(
        &mut self,
        user_id: impl Into<String>,
        display_name: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Result<(), CollaborationError> {
        let uid = user_id.into();
        if self.participants.iter().any(|p| p.user_id == uid) {
            return Err(CollaborationError::MemberAlreadyExists(uid));
        }

        self.participants.push(MultiplayerParticipant {
            user_id: uid,
            display_name: display_name.into(),
            is_writer: false,
            joined_at: timestamp.into(),
        });

        Ok(())
    }

    /// Removes a participant from the active session. If the user was holding control,
    /// control automatically returns to the session host.
    pub fn remove_participant(&mut self, user_id: &str) -> bool {
        let initial_len = self.participants.len();
        self.participants.retain(|p| p.user_id != user_id);
        let removed = self.participants.len() < initial_len;

        if removed && self.current_controller.as_deref() == Some(user_id) {
            self.current_controller = Some(self.owner_id.clone());
            if let Some(host) = self
                .participants
                .iter_mut()
                .find(|p| p.user_id == self.owner_id)
            {
                host.is_writer = true;
            }
        }

        removed
    }

    /// Grants temporary keyboard control to a collaborator.
    /// Only the session host or current controller can initiate transfer.
    pub fn grant_keyboard_control(
        &mut self,
        requester_id: &str,
        target_user_id: &str,
    ) -> Result<(), CollaborationError> {
        if !self.is_active {
            return Err(CollaborationError::SessionNotActive);
        }

        if requester_id != self.owner_id && self.current_controller.as_deref() != Some(requester_id)
        {
            return Err(CollaborationError::Unauthorized(
                "Only the session owner or current writer can grant keyboard control".to_string(),
            ));
        }

        if !self
            .participants
            .iter()
            .any(|p| p.user_id == target_user_id)
        {
            return Err(CollaborationError::MemberNotFound(
                target_user_id.to_string(),
            ));
        }

        // Set previous writer to viewer
        for p in &mut self.participants {
            p.is_writer = p.user_id == target_user_id;
        }

        self.current_controller = Some(target_user_id.to_string());
        Ok(())
    }

    /// Revokes keyboard control immediately, reverting active writing exclusively to the session owner.
    pub fn revoke_keyboard_control(
        &mut self,
        requester_id: &str,
    ) -> Result<(), CollaborationError> {
        if !self.is_active {
            return Err(CollaborationError::SessionNotActive);
        }

        if requester_id != self.owner_id {
            return Err(CollaborationError::Unauthorized(
                "Only the session owner can immediately revoke keyboard control".to_string(),
            ));
        }

        for p in &mut self.participants {
            p.is_writer = p.user_id == self.owner_id;
        }

        self.current_controller = Some(self.owner_id.clone());
        Ok(())
    }

    /// Checks if a user currently has typing permission.
    pub fn is_writer(&self, user_id: &str) -> bool {
        self.is_active && self.current_controller.as_deref() == Some(user_id)
    }

    /// Returns the total count of active connected participants.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Terminates the multiplayer collaboration session.
    pub fn terminate(&mut self, requester_id: &str) -> Result<(), CollaborationError> {
        if requester_id != self.owner_id {
            return Err(CollaborationError::Unauthorized(
                "Only the session owner can terminate the collaboration session".to_string(),
            ));
        }

        self.is_active = false;
        self.current_controller = None;
        for p in &mut self.participants {
            p.is_writer = false;
        }

        Ok(())
    }
}

/// Comment entry attached to a shared session log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogComment {
    pub id: String,
    pub author_id: String,
    pub line_number: u32,
    pub comment: String,
    pub created_at: String,
}

/// Bookmark entry attached to a shared session log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogBookmark {
    pub id: String,
    pub line_number: u32,
    pub label: String,
    pub created_at: String,
}

/// Shared audit-retained session log record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedSessionLog {
    pub id: String,
    pub vault_id: String,
    pub host_name: String,
    pub author_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub retention_days: u32,
    pub comments: Vec<LogComment>,
    pub bookmarks: Vec<LogBookmark>,
}

impl SharedSessionLog {
    pub fn new(
        id: impl Into<String>,
        vault_id: impl Into<String>,
        host_name: impl Into<String>,
        author_id: impl Into<String>,
        started_at: impl Into<String>,
        retention_days: u32,
    ) -> Self {
        Self {
            id: id.into(),
            vault_id: vault_id.into(),
            host_name: host_name.into(),
            author_id: author_id.into(),
            started_at: started_at.into(),
            ended_at: None,
            retention_days,
            comments: Vec::new(),
            bookmarks: Vec::new(),
        }
    }

    pub fn add_comment(&mut self, comment: LogComment) {
        self.comments.push(comment);
    }

    pub fn add_bookmark(&mut self, bookmark: LogBookmark) {
        self.bookmarks.push(bookmark);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_team_role_permissions_and_hierarchy() {
        assert!(TeamRole::Owner > TeamRole::Administrator);
        assert!(TeamRole::Administrator > TeamRole::Editor);
        assert!(TeamRole::Editor > TeamRole::Viewer);

        assert!(TeamRole::Owner.can_manage_roles());
        assert!(!TeamRole::Administrator.can_manage_roles());
        assert!(TeamRole::Administrator.can_manage_members());
        assert!(!TeamRole::Editor.can_manage_members());

        assert!(TeamRole::Editor.can_edit_records());
        assert!(!TeamRole::Viewer.can_edit_records());
        assert!(TeamRole::Viewer.can_view_records());

        assert_eq!(TeamRole::from_str("owner").unwrap(), TeamRole::Owner);
        assert_eq!(
            TeamRole::from_str("admin").unwrap(),
            TeamRole::Administrator
        );
        assert_eq!(TeamRole::from_str("write").unwrap(), TeamRole::Editor);
        assert_eq!(TeamRole::from_str("readonly").unwrap(), TeamRole::Viewer);
        assert!(TeamRole::from_str("superman").is_err());
    }

    #[test]
    fn manages_team_member_lifecycle_and_revocation() {
        let mut member = TeamMember::new(
            "m-1",
            "v-1",
            "alice@example.org",
            "Alice",
            "ed25519_pk_hex",
            TeamRole::Editor,
            "2026-11-05T00:00:00Z",
            "bob@example.org",
        );

        assert!(member.is_active());
        assert_eq!(member.role, TeamRole::Editor);

        member.change_role(TeamRole::Administrator);
        assert_eq!(member.role, TeamRole::Administrator);

        member.revoke("2026-11-05T01:00:00Z");
        assert!(!member.is_active());
        assert_eq!(member.revoked_at.as_deref(), Some("2026-11-05T01:00:00Z"));
    }

    #[test]
    fn manages_shared_vault_and_key_rotation() {
        let mut vault = SharedVault::new(
            "v-100",
            "Production DevOps",
            Some("Shared cloud infrastructure".to_string()),
            "alice@example.org",
            "2026-11-05T00:00:00Z",
        )
        .unwrap();

        assert_eq!(vault.current_epoch, 1);
        assert!(vault.is_owner("alice@example.org"));
        assert!(!vault.is_owner("charlie@example.org"));

        let next_epoch = vault.rotate_key("2026-11-05T02:00:00Z");
        assert_eq!(next_epoch, 2);
        assert_eq!(vault.current_epoch, 2);
        assert_eq!(vault.updated_at, "2026-11-05T02:00:00Z");

        assert!(SharedVault::new("v-2", "   ", None, "owner", "time").is_err());
    }

    #[test]
    fn manages_sharing_audit_record_formatting() {
        let audit = SharingAuditRecord::new(
            "a-1",
            "v-100",
            AuditAction::MemberInvited,
            "alice@example.org",
            Some("bob@example.org".to_string()),
            Some("role=editor".to_string()),
            "2026-11-05T00:05:00Z",
        );

        let formatted = audit.format_entry();
        assert!(
            formatted.contains("[2026-11-05T00:05:00Z] member_invited by actor=alice@example.org")
        );
        assert!(formatted.contains("target=bob@example.org"));
        assert!(formatted.contains("(role=editor)"));
    }

    #[test]
    fn manages_multiplayer_session_control_and_revocation() {
        let mut session = MultiplayerSession::new(
            "sess-1",
            Some("v-100".to_string()),
            "alice",
            "2026-11-05T00:00:00Z",
        );

        assert!(session.is_writer("alice"));
        assert_eq!(session.participant_count(), 1);

        session
            .add_participant("bob", "Bob", "2026-11-05T00:01:00Z")
            .unwrap();
        assert_eq!(session.participant_count(), 2);
        assert!(!session.is_writer("bob"));

        // Bob cannot transfer control to himself
        assert!(session.grant_keyboard_control("bob", "bob").is_err());

        // Alice grants control to Bob
        session.grant_keyboard_control("alice", "bob").unwrap();
        assert!(!session.is_writer("alice"));
        assert!(session.is_writer("bob"));

        // Alice revokes control immediately
        session.revoke_keyboard_control("alice").unwrap();
        assert!(session.is_writer("alice"));
        assert!(!session.is_writer("bob"));

        // Removing Bob while Bob was writer reverts to host
        session.grant_keyboard_control("alice", "bob").unwrap();
        assert!(session.is_writer("bob"));
        assert!(session.remove_participant("bob"));
        assert!(session.is_writer("alice"));

        // Termination
        session.terminate("alice").unwrap();
        assert!(!session.is_active);
        assert!(!session.is_writer("alice"));
    }

    #[test]
    fn manages_shared_session_log_bookmarks_and_comments() {
        let mut log = SharedSessionLog::new(
            "log-1",
            "v-1",
            "prod-bastion",
            "alice",
            "2026-11-05T00:00:00Z",
            60,
        );

        log.add_bookmark(LogBookmark {
            id: "b-1".to_string(),
            line_number: 42,
            label: "Kernel panic".to_string(),
            created_at: "2026-11-05T00:01:00Z".to_string(),
        });

        log.add_comment(LogComment {
            id: "c-1".to_string(),
            author_id: "bob".to_string(),
            line_number: 42,
            comment: "Investigating memory dump".to_string(),
            created_at: "2026-11-05T00:02:00Z".to_string(),
        });

        assert_eq!(log.bookmarks.len(), 1);
        assert_eq!(log.comments.len(), 1);
        assert_eq!(log.bookmarks[0].label, "Kernel panic");
        assert_eq!(log.comments[0].comment, "Investigating memory dump");
    }
}

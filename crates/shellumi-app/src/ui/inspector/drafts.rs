#![allow(dead_code, clippy::derivable_impls)]
//! Typed draft models for object creation and editing inside the Unified Inspector.

use shellumi_core::{ConnectionProtocol, ForwardingType};

/// Typed draft model for hosts.
#[derive(Debug, Clone, PartialEq)]
pub struct HostDraft {
    pub id: Option<i64>,
    pub name: String,
    pub destination: String,
    pub port: u16,
    pub protocol: ConnectionProtocol,
    pub group_id: Option<i64>,
    pub identity_id: Option<i64>,
    pub tags: Vec<String>,
    pub jump_host: Option<String>,
    pub jump_port: Option<u16>,
    pub startup_snippet_id: Option<i64>,
    pub is_dirty: bool,
}

impl Default for HostDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            destination: String::new(),
            port: 22,
            protocol: ConnectionProtocol::Ssh,
            group_id: None,
            identity_id: None,
            tags: Vec::new(),
            jump_host: None,
            jump_port: None,
            startup_snippet_id: None,
            is_dirty: false,
        }
    }
}

/// Typed draft model for host groups.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupDraft {
    pub id: Option<i64>,
    pub name: String,
    pub parent_group_id: Option<i64>,
    pub default_username: Option<String>,
    pub default_port: Option<u16>,
    pub default_identity_id: Option<i64>,
    pub tags: Vec<String>,
    pub is_dirty: bool,
}

impl Default for GroupDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            parent_group_id: None,
            default_username: None,
            default_port: None,
            default_identity_id: None,
            tags: Vec::new(),
            is_dirty: false,
        }
    }
}

/// Typed draft model for identities.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentityDraft {
    pub id: Option<i64>,
    pub name: String,
    pub username: String,
    pub key_type: String, // e.g. "ed25519", "rsa"
    pub key_path: Option<String>,
    pub certificate_path: Option<String>,
    pub passphrase: Option<String>,
    pub is_dirty: bool,
}

impl Default for IdentityDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            username: String::new(),
            key_type: "ed25519".into(),
            key_path: None,
            certificate_path: None,
            passphrase: None,
            is_dirty: false,
        }
    }
}

/// Typed draft model for snippets.
#[derive(Debug, Clone, PartialEq)]
pub struct SnippetDraft {
    pub id: Option<i64>,
    pub name: String,
    pub command: String,
    pub description: String,
    pub tags: Vec<String>,
    pub is_dirty: bool,
}

impl Default for SnippetDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            command: String::new(),
            description: String::new(),
            tags: Vec::new(),
            is_dirty: false,
        }
    }
}

/// Typed draft model for tunnels / port forwardings.
#[derive(Debug, Clone, PartialEq)]
pub struct TunnelDraft {
    pub id: Option<i64>,
    pub name: String,
    pub host_id: Option<i64>,
    pub forwarding_type: ForwardingType,
    pub bind_address: String,
    pub bind_port: u16,
    pub target_host: String,
    pub target_port: u16,
    pub is_dirty: bool,
}

impl Default for TunnelDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            host_id: None,
            forwarding_type: ForwardingType::Local,
            bind_address: "127.0.0.1".into(),
            bind_port: 8080,
            target_host: "127.0.0.1".into(),
            target_port: 80,
            is_dirty: false,
        }
    }
}

/// Typed draft model for workspaces.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceDraft {
    pub id: Option<i64>,
    pub name: String,
    pub layout: String,
    pub host_ids: Vec<i64>,
    pub is_dirty: bool,
}

impl Default for WorkspaceDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            layout: "split-horizontal".into(),
            host_ids: Vec::new(),
            is_dirty: false,
        }
    }
}

/// Typed draft model for vault details.
#[derive(Debug, Clone, PartialEq)]
pub struct VaultDraft {
    pub vault_id: String,
    pub name: String,
    pub storage_type: String,
    pub cloud_sync_enabled: bool,
    pub is_dirty: bool,
}

impl Default for VaultDraft {
    fn default() -> Self {
        Self {
            vault_id: String::new(),
            name: String::new(),
            storage_type: "local".into(),
            cloud_sync_enabled: false,
            is_dirty: false,
        }
    }
}

/// Typed draft model for cloud infrastructure providers.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderDraft {
    pub id: Option<i64>,
    pub name: String,
    pub provider_kind: String, // "aws", "azure", "digitalocean"
    pub credentials_ref: String,
    pub sync_interval_secs: u64,
    pub is_dirty: bool,
}

impl Default for ProviderDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            provider_kind: "aws".into(),
            credentials_ref: String::new(),
            sync_interval_secs: 3600,
            is_dirty: false,
        }
    }
}

/// Typed draft model for team members and invitations.
#[derive(Debug, Clone, PartialEq)]
pub struct TeamMemberDraft {
    pub id: Option<i64>,
    pub name: String,
    pub email: String,
    pub role: String, // "Admin", "Member", "Auditor"
    pub public_key: Option<String>,
    pub is_dirty: bool,
}

impl Default for TeamMemberDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            email: String::new(),
            role: "Member".into(),
            public_key: None,
            is_dirty: false,
        }
    }
}

/// Typed draft model for remote desktop session preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionDraft {
    pub session_id: String,
    pub display_mode: String, // "Fit", "Original", "Fill"
    pub sound_enabled: bool,
    pub microphone_enabled: bool,
    pub clipboard_sync_enabled: bool,
    pub shared_drive_path: Option<String>,
    pub is_dirty: bool,
}

impl Default for SessionDraft {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            display_mode: "Fit".into(),
            sound_enabled: true,
            microphone_enabled: false,
            clipboard_sync_enabled: true,
            shared_drive_path: None,
            is_dirty: false,
        }
    }
}

/// Composite enum holding any active draft in the Inspector.
#[derive(Debug, Clone, PartialEq)]
pub enum InspectorDraft {
    Host(HostDraft),
    Group(GroupDraft),
    Identity(IdentityDraft),
    Snippet(SnippetDraft),
    Tunnel(TunnelDraft),
    Workspace(WorkspaceDraft),
    Vault(VaultDraft),
    Provider(ProviderDraft),
    TeamMember(TeamMemberDraft),
    Session(SessionDraft),
}

impl InspectorDraft {
    pub fn is_dirty(&self) -> bool {
        match self {
            Self::Host(d) => d.is_dirty,
            Self::Group(d) => d.is_dirty,
            Self::Identity(d) => d.is_dirty,
            Self::Snippet(d) => d.is_dirty,
            Self::Tunnel(d) => d.is_dirty,
            Self::Workspace(d) => d.is_dirty,
            Self::Vault(d) => d.is_dirty,
            Self::Provider(d) => d.is_dirty,
            Self::TeamMember(d) => d.is_dirty,
            Self::Session(d) => d.is_dirty,
        }
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        match self {
            Self::Host(d) => d.is_dirty = dirty,
            Self::Group(d) => d.is_dirty = dirty,
            Self::Identity(d) => d.is_dirty = dirty,
            Self::Snippet(d) => d.is_dirty = dirty,
            Self::Tunnel(d) => d.is_dirty = dirty,
            Self::Workspace(d) => d.is_dirty = dirty,
            Self::Vault(d) => d.is_dirty = dirty,
            Self::Provider(d) => d.is_dirty = dirty,
            Self::TeamMember(d) => d.is_dirty = dirty,
            Self::Session(d) => d.is_dirty = dirty,
        }
    }
}

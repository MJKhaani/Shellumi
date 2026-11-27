//! Typed SFTP workspace, pane, endpoint, operation, and authorization identities.

use crate::SshDestination;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_raw_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        pub struct $name(pub u64);

        impl $name {
            pub fn new() -> Self {
                Self(next_raw_id())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id!(
    WorkspaceTabId,
    "Stable identifier for an SFTP workspace tab."
);
define_id!(PaneId, "Stable identifier for a file-browser pane.");
define_id!(
    EndpointId,
    "Stable identifier for a connected or local endpoint."
);
define_id!(OperationId, "Stable identifier for a file operation.");
define_id!(BatchId, "Stable identifier for a transfer batch.");
define_id!(JobId, "Stable identifier for a transfer job.");
define_id!(
    AttemptId,
    "Stable identifier for a single execution attempt of a job."
);
define_id!(
    EditorId,
    "Stable identifier for an integrated or external editor session."
);
define_id!(
    EntryId,
    "Stable identifier for a directory entry within a committed listing snapshot."
);

/// Vault authorization context scoping an endpoint, transfer, or editor session.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VaultScope {
    /// Durable vault identifier (e.g. vault UUID or database path canonical key).
    pub vault_id: String,
    /// Monotonic epoch incremented on vault lock, switch, or credential revocation.
    pub vault_epoch: u64,
}

impl VaultScope {
    pub fn local_default() -> Self {
        Self {
            vault_id: "local-default".to_string(),
            vault_epoch: 0,
        }
    }

    pub fn new(vault_id: impl Into<String>, vault_epoch: u64) -> Self {
        Self {
            vault_id: vault_id.into(),
            vault_epoch,
        }
    }

    /// Returns true if this scope is still valid under the active vault scope.
    pub fn is_authorized_under(&self, active: &VaultScope) -> bool {
        self.vault_id == active.vault_id && self.vault_epoch == active.vault_epoch
    }
}

/// Durable identity for a remote SSH/SFTP endpoint.
///
/// Never identified by mutable display name or numeric `host_id` alone;
/// always includes `vault_id`, `host_id`, canonical `destination`, and `connection_generation`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteEndpoint {
    pub endpoint_id: EndpointId,
    pub vault_scope: VaultScope,
    pub host_id: i64,
    pub host_uuid: Option<String>,
    pub display_name: String,
    pub destination: SshDestination,
    pub connection_generation: u64,
    pub database_path: Option<String>,
}

impl RemoteEndpoint {
    /// Canonical identity key for comparing two remote endpoints across vaults/hosts.
    pub fn canonical_host_key(&self) -> (&str, i64, Option<&str>, &str, u16, &str) {
        (
            &self.vault_scope.vault_id,
            self.host_id,
            self.host_uuid.as_deref(),
            self.destination.host(),
            self.destination.port().get(),
            self.destination.username(),
        )
    }
}

/// Endpoint descriptor independent of transfer direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndpointKind {
    Local {
        endpoint_id: EndpointId,
        vault_scope: VaultScope,
        root_hint: Option<PathBuf>,
    },
    Remote(RemoteEndpoint),
}

impl EndpointKind {
    pub fn local(vault_scope: VaultScope) -> Self {
        Self::Local {
            endpoint_id: EndpointId::new(),
            vault_scope,
            root_hint: None,
        }
    }

    pub fn endpoint_id(&self) -> EndpointId {
        match self {
            Self::Local { endpoint_id, .. } => *endpoint_id,
            Self::Remote(r) => r.endpoint_id,
        }
    }

    pub fn vault_scope(&self) -> &VaultScope {
        match self {
            Self::Local { vault_scope, .. } => vault_scope,
            Self::Remote(r) => &r.vault_scope,
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local { .. })
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote(_))
    }

    /// Returns true if both endpoints refer to the exact same underlying filesystem/host
    /// within the same vault (preventing numeric host_id collisions across vaults).
    pub fn same_underlying_target(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local { .. }, Self::Local { .. }) => true,
            (Self::Remote(a), Self::Remote(b)) => a.canonical_host_key() == b.canonical_host_key(),
            _ => false,
        }
    }
}

/// All four supported transfer directions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransferRoute {
    LocalToLocal,
    LocalToRemote,
    RemoteToLocal,
    RemoteToRemote,
}

impl TransferRoute {
    pub fn from_endpoints(src: &EndpointKind, dst: &EndpointKind) -> Self {
        match (src.is_remote(), dst.is_remote()) {
            (false, false) => Self::LocalToLocal,
            (false, true) => Self::LocalToRemote,
            (true, false) => Self::RemoteToLocal,
            (true, true) => Self::RemoteToRemote,
        }
    }
}

impl std::fmt::Display for TransferRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalToLocal => f.write_str("Local → Local"),
            Self::LocalToRemote => f.write_str("Local → Remote"),
            Self::RemoteToLocal => f.write_str("Remote → Local"),
            Self::RemoteToRemote => f.write_str("Remote → Remote"),
        }
    }
}

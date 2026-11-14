//! Vault and server identities for independently scoped application contexts.
//!
//! These models do not provide storage or credential isolation by themselves.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VaultId(String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ServerConnectionId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultIdentityError;

impl fmt::Display for VaultIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("identity must be nonempty and contain no whitespace or control characters")
    }
}

impl std::error::Error for VaultIdentityError {}

fn validate_id(value: &str) -> Result<(), VaultIdentityError> {
    if value.is_empty() || value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(VaultIdentityError);
    }
    Ok(())
}

impl VaultId {
    pub fn new(value: impl Into<String>) -> Result<Self, VaultIdentityError> {
        let value = value.into();
        validate_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ServerConnectionId {
    pub fn new(value: impl Into<String>) -> Result<Self, VaultIdentityError> {
        let value = value.into();
        validate_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A remote vault is identified by both its server connection and server vault ID.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VaultLocation {
    Local,
    Remote {
        server: ServerConnectionId,
        server_vault_id: VaultId,
    },
}

/// Local identity is distinct from a server's vault identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultContext {
    id: VaultId,
    location: VaultLocation,
}

impl VaultContext {
    pub fn new(id: VaultId, location: VaultLocation) -> Self {
        Self { id, location }
    }

    pub fn id(&self) -> &VaultId {
        &self.id
    }

    pub fn location(&self) -> &VaultLocation {
        &self.location
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_whitespace_and_control_characters() {
        for value in ["", " ", " vault", "vault\n", "vault\0id"] {
            assert!(VaultId::new(value).is_err());
            assert!(ServerConnectionId::new(value).is_err());
        }
        assert_eq!(VaultId::new("vault-1").unwrap().as_str(), "vault-1");
    }

    #[test]
    fn remote_identity_includes_server_and_window_contexts_are_independent() {
        let remote_id = VaultId::new("same-server-vault-id").unwrap();
        let first = VaultContext::new(
            VaultId::new("local-id-1").unwrap(),
            VaultLocation::Remote {
                server: ServerConnectionId::new("company-a").unwrap(),
                server_vault_id: remote_id.clone(),
            },
        );
        let second = VaultContext::new(
            VaultId::new("local-id-2").unwrap(),
            VaultLocation::Remote {
                server: ServerConnectionId::new("company-b").unwrap(),
                server_vault_id: remote_id,
            },
        );
        assert_ne!(first.location(), second.location());
        let mut first_window = first.clone();
        let second_window = second.clone();
        first_window = VaultContext::new(first_window.id().clone(), VaultLocation::Local);
        assert_eq!(first_window.location(), &VaultLocation::Local);
        assert_eq!(second_window, second);
        assert_eq!(first.id().as_str(), "local-id-1");
    }
}

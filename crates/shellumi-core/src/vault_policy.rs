//! Cross-vault isolation, transfer validation, and authorization policies.
//!
//! Conforming to the Shellumi Engineering Standard:
//! Cross-vault copy, move, sharing, or credential reuse strictly requires an explicit
//! user action, source and destination, authorization, and clear confirmation when sensitive
//! data is involved; credentials must never be silently reused or leaked across vaults.

use std::fmt;

use crate::vault::VaultId;

/// Action type for cross-vault item migration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossVaultAction {
    Copy,
    Move,
}

/// Errors resulting from invalid cross-vault transfer attempts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrossVaultError {
    SameSourceAndTargetVault,
    UnconfirmedCredentialExposure(&'static str),
    MissingAuthorization,
}

impl fmt::Display for CrossVaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SameSourceAndTargetVault => {
                f.write_str("source and destination vaults must be distinct")
            }
            Self::UnconfirmedCredentialExposure(msg) => {
                write!(f, "unconfirmed credential exposure: {msg}")
            }
            Self::MissingAuthorization => {
                f.write_str("explicit user authorization is required for cross-vault transfers")
            }
        }
    }
}

impl std::error::Error for CrossVaultError {}

/// Policy governing an explicit cross-vault item transfer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrossVaultTransferPolicy {
    pub source_vault: VaultId,
    pub target_vault: VaultId,
    pub action: CrossVaultAction,
    pub strip_credential_references: bool,
    pub confirm_sensitive_exposure: bool,
    pub authorized_by_user: bool,
}

impl CrossVaultTransferPolicy {
    pub fn new(source_vault: VaultId, target_vault: VaultId, action: CrossVaultAction) -> Self {
        Self {
            source_vault,
            target_vault,
            action,
            strip_credential_references: true, // Safe default: strip secrets unless explicitly instructed
            confirm_sensitive_exposure: false,
            authorized_by_user: false,
        }
    }

    /// Set user authorization.
    pub fn with_user_authorization(mut self, authorized: bool) -> Self {
        self.authorized_by_user = authorized;
        self
    }

    /// Set credential handling policy.
    pub fn with_credential_policy(
        mut self,
        strip_credentials: bool,
        confirm_exposure: bool,
    ) -> Self {
        self.strip_credential_references = strip_credentials;
        self.confirm_sensitive_exposure = confirm_exposure;
        self
    }

    /// Validate the policy parameters before executing any data transfers.
    pub fn validate(&self, has_attached_credentials: bool) -> Result<(), CrossVaultError> {
        if self.source_vault == self.target_vault {
            return Err(CrossVaultError::SameSourceAndTargetVault);
        }
        if !self.authorized_by_user {
            return Err(CrossVaultError::MissingAuthorization);
        }
        if has_attached_credentials
            && !self.strip_credential_references
            && !self.confirm_sensitive_exposure
        {
            return Err(CrossVaultError::UnconfirmedCredentialExposure(
                "transferring records with credentials to another vault requires explicit confirmation of sensitive exposure",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_vault_policy_enforcement() {
        let v_src = VaultId::new("vault-source").unwrap();
        let v_dst = VaultId::new("vault-dest").unwrap();

        // Same source and target rejected
        let policy_same =
            CrossVaultTransferPolicy::new(v_src.clone(), v_src.clone(), CrossVaultAction::Copy)
                .with_user_authorization(true);
        assert_eq!(
            policy_same.validate(false),
            Err(CrossVaultError::SameSourceAndTargetVault)
        );

        // Missing authorization rejected
        let policy_unauth =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Copy);
        assert_eq!(
            policy_unauth.validate(false),
            Err(CrossVaultError::MissingAuthorization)
        );

        // Safe transfer with credentials stripped passes
        let policy_safe =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Copy)
                .with_user_authorization(true)
                .with_credential_policy(true, false);
        assert!(policy_safe.validate(true).is_ok());

        // Retaining credentials without confirmation rejected
        let policy_unconfirmed =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Move)
                .with_user_authorization(true)
                .with_credential_policy(false, false);
        assert!(matches!(
            policy_unconfirmed.validate(true),
            Err(CrossVaultError::UnconfirmedCredentialExposure(_))
        ));

        // Retaining credentials with explicit confirmation passes
        let policy_confirmed =
            CrossVaultTransferPolicy::new(v_src.clone(), v_dst.clone(), CrossVaultAction::Move)
                .with_user_authorization(true)
                .with_credential_policy(false, true);
        assert!(policy_confirmed.validate(true).is_ok());
    }
}

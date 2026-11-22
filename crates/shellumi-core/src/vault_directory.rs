//! Encrypted directory describing known vaults.
//!
//! Conforming to the Shellumi Architecture Specification:
//! The vault directory contains only metadata needed to identify, display, and reconnect
//! to each vault (stable identity, name, location, and timestamps). It never contains
//! another vault's hosts, identities, secrets, or cached contents.

use serde::{Deserialize, Serialize};

use crate::sync::{EncryptedEnvelope, SyncError};
use crate::vault::VaultId;
use crate::vault_cipher::VaultCipher;
use crate::vault_key::VaultKey;

/// The location and server binding of a vault listed in the directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VaultDirectoryLocation {
    Local,
    Remote {
        server_connection_id: String,
        server_vault_id: String,
    },
}

/// Metadata describing a single known vault in the directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultDirectoryEntry {
    pub id: String,
    pub name: String,
    pub location: VaultDirectoryLocation,
    pub has_recovery_envelope: bool,
    #[serde(default)]
    pub sync_enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The collection of all known vaults, encrypted at rest under the default vault's key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultDirectory {
    pub entries: Vec<VaultDirectoryEntry>,
}

impl VaultDirectory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add or update a vault entry in the directory.
    pub fn upsert(&mut self, entry: VaultDirectoryEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.id == entry.id) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Remove a vault entry from the directory.
    pub fn remove(&mut self, vault_id: &str) -> bool {
        let initial_len = self.entries.len();
        self.entries.retain(|e| e.id != vault_id);
        self.entries.len() < initial_len
    }

    /// Find an entry by vault ID.
    pub fn get(&self, vault_id: &str) -> Option<&VaultDirectoryEntry> {
        self.entries.iter().find(|e| e.id == vault_id)
    }

    /// Encrypt the entire directory under the default local vault's key.
    pub fn encrypt(
        &self,
        default_vault_id: &VaultId,
        default_vault_key: &VaultKey,
    ) -> Result<EncryptedEnvelope, SyncError> {
        let cipher = VaultCipher::new(default_vault_key, default_vault_id);
        let serialized = serde_json::to_vec(self).map_err(|_| SyncError::EncryptionFailed)?;
        cipher.encrypt_record("vault_directory", &serialized)
    }

    /// Decrypt the directory using the default local vault's key.
    pub fn decrypt(
        default_vault_id: &VaultId,
        default_vault_key: &VaultKey,
        envelope: &EncryptedEnvelope,
    ) -> Result<Self, SyncError> {
        let cipher = VaultCipher::new(default_vault_key, default_vault_id);
        let decrypted = cipher.decrypt_record("vault_directory", envelope)?;
        serde_json::from_slice(&decrypted).map_err(|_| SyncError::DecryptionFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_directory_lifecycle_and_encryption() {
        let default_vault_id = VaultId::new("legacy-local").unwrap();
        let default_key = VaultKey::generate().unwrap();

        let mut dir = VaultDirectory::new();
        dir.upsert(VaultDirectoryEntry {
            id: "legacy-local".to_string(),
            name: "Personal".to_string(),
            location: VaultDirectoryLocation::Local,
            has_recovery_envelope: false,
            sync_enabled: true,
            created_at: 1000,
            updated_at: 1000,
        });

        dir.upsert(VaultDirectoryEntry {
            id: "corp-infra".to_string(),
            name: "Corporate Infrastructure".to_string(),
            location: VaultDirectoryLocation::Remote {
                server_connection_id: "corp-srv-1".to_string(),
                server_vault_id: "srv-vault-99".to_string(),
            },
            has_recovery_envelope: true,
            sync_enabled: false,
            created_at: 2000,
            updated_at: 2000,
        });

        assert_eq!(dir.entries.len(), 2);
        assert!(dir.get("corp-infra").is_some());

        // Encrypt directory
        let envelope = dir.encrypt(&default_vault_id, &default_key).unwrap();

        // Decrypt directory
        let decrypted_dir =
            VaultDirectory::decrypt(&default_vault_id, &default_key, &envelope).unwrap();
        assert_eq!(decrypted_dir, dir);

        // Removal
        let mut mutated = dir;
        assert!(mutated.remove("corp-infra"));
        assert_eq!(mutated.entries.len(), 1);
        assert!(mutated.get("corp-infra").is_none());
    }
}

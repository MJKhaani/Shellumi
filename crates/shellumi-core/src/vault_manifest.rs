//! Versioned portable vault manifest.
//!
//! Stores the wrapped Data Encryption Key (DEK), Argon2id parameters, salt,
//! nonce, and metadata outside the encrypted database so it can be accessed
//! before unlocking the database at rest.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::sync::{EncryptedEnvelope, SyncError};
use crate::vault::VaultId;
use crate::vault_key::VaultKey;

pub const MANIFEST_FORMAT_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultManifest {
    pub format_version: u32,
    pub vault_id: String,
    pub name: String,
    pub primary_envelope: EncryptedEnvelope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_envelope: Option<EncryptedEnvelope>,
    #[serde(default)]
    pub remember_in_keychain: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

impl VaultManifest {
    pub fn new(
        vault_id: String,
        name: String,
        primary_envelope: EncryptedEnvelope,
        recovery_envelope: Option<EncryptedEnvelope>,
        remember_in_keychain: bool,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Self {
            format_version: MANIFEST_FORMAT_VERSION,
            vault_id,
            name,
            primary_envelope,
            recovery_envelope,
            remember_in_keychain,
            created_at: now,
            updated_at: now,
        }
    }

    /// Unlock the vault using its primary password or recovery code.
    pub fn unlock(&self, password: &str) -> Result<VaultKey, SyncError> {
        let vid =
            VaultId::new(&self.vault_id).map_err(|e| SyncError::Serialization(e.to_string()))?;
        if let Ok(key) = VaultKey::unwrap_with_password(&vid, password, &self.primary_envelope) {
            return Ok(key);
        }
        if let Some(ref recovery) = self.recovery_envelope {
            if let Ok(key) = VaultKey::unwrap_with_password(&vid, password, recovery) {
                return Ok(key);
            }
        }
        Err(SyncError::DecryptionFailed)
    }

    /// Unlock the vault specifically using its recovery key/code.
    pub fn unlock_with_recovery(&self, recovery_key: &str) -> Result<VaultKey, SyncError> {
        let env = self
            .recovery_envelope
            .as_ref()
            .ok_or(SyncError::DecryptionFailed)?;
        let vid =
            VaultId::new(&self.vault_id).map_err(|e| SyncError::Serialization(e.to_string()))?;
        VaultKey::unwrap_with_password(&vid, recovery_key, env)
    }

    /// Change the vault password without re-encrypting the underlying database.
    pub fn change_password(
        &mut self,
        current_key: &VaultKey,
        new_password: &str,
    ) -> Result<(), SyncError> {
        let vid =
            VaultId::new(&self.vault_id).map_err(|e| SyncError::Serialization(e.to_string()))?;
        let new_envelope = current_key.wrap_with_password(&vid, new_password)?;
        self.primary_envelope = new_envelope;
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(())
    }

    /// Change password with custom Argon2id parameters.
    pub fn change_password_with_params(
        &mut self,
        current_key: &VaultKey,
        new_password: &str,
        m_cost: u32,
        t_cost: u32,
        p_cost: u32,
    ) -> Result<(), SyncError> {
        let vid =
            VaultId::new(&self.vault_id).map_err(|e| SyncError::Serialization(e.to_string()))?;
        let new_envelope =
            current_key.wrap_with_argon2_params(&vid, new_password, m_cost, t_cost, p_cost)?;
        self.primary_envelope = new_envelope;
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(())
    }

    /// Set or update the recovery key envelope.
    pub fn set_recovery_key(
        &mut self,
        current_key: &VaultKey,
        recovery_key: &str,
    ) -> Result<(), SyncError> {
        let vid =
            VaultId::new(&self.vault_id).map_err(|e| SyncError::Serialization(e.to_string()))?;
        let new_envelope = current_key.wrap_with_password(&vid, recovery_key)?;
        self.recovery_envelope = Some(new_envelope);
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(())
    }

    /// Set or update the recovery key envelope with custom parameters.
    pub fn set_recovery_key_with_params(
        &mut self,
        current_key: &VaultKey,
        recovery_key: &str,
        m_cost: u32,
        t_cost: u32,
        p_cost: u32,
    ) -> Result<(), SyncError> {
        let vid =
            VaultId::new(&self.vault_id).map_err(|e| SyncError::Serialization(e.to_string()))?;
        let new_envelope =
            current_key.wrap_with_argon2_params(&vid, recovery_key, m_cost, t_cost, p_cost)?;
        self.recovery_envelope = Some(new_envelope);
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(())
    }

    /// Serializes manifest to JSON.
    pub fn to_json(&self) -> Result<String, SyncError> {
        serde_json::to_string_pretty(self).map_err(|e| SyncError::Serialization(e.to_string()))
    }

    /// Deserializes manifest from JSON.
    pub fn from_json(json: &str) -> Result<Self, SyncError> {
        serde_json::from_str(json).map_err(|e| SyncError::Deserialization(e.to_string()))
    }

    /// Computes the default manifest file path for a vault relative to a base database path.
    pub fn manifest_path_for_vault(base_db_path: &Path, vault_id: &VaultId) -> PathBuf {
        let parent = base_db_path.parent().unwrap_or_else(|| Path::new("."));
        let root = if parent.file_name() == Some(std::ffi::OsStr::new("vaults")) {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        };
        let manifests_dir = root.join("vault_manifests");
        manifests_dir.join(format!("{}.manifest.json", vault_id.as_str()))
    }

    /// Load manifest from disk path.
    pub fn load_from_path(path: &Path) -> Result<Self, SyncError> {
        let content = fs::read_to_string(path)?;
        Self::from_json(&content)
    }

    /// Atomically save manifest to disk path via temporary file and rename.
    pub fn save_to_path(&self, path: &Path) -> Result<(), SyncError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = self.to_json()?;
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, json.as_bytes())?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_M_COST: u32 = 64; // Fast parameter for unit test execution
    const TEST_T_COST: u32 = 1;
    const TEST_P_COST: u32 = 1;

    #[test]
    fn manifest_creation_unlock_and_password_change() {
        let vid = VaultId::new("test-vault").unwrap();
        let dek = VaultKey::generate().unwrap();
        let password = "InitialPassword123#";
        let recovery_key = "RecoverySecret456!";

        let primary_env = dek
            .wrap_with_argon2_params(&vid, password, TEST_M_COST, TEST_T_COST, TEST_P_COST)
            .unwrap();
        let recovery_env = dek
            .wrap_with_argon2_params(&vid, recovery_key, TEST_M_COST, TEST_T_COST, TEST_P_COST)
            .unwrap();

        let mut manifest = VaultManifest::new(
            vid.as_str().to_string(),
            "Production".to_string(),
            primary_env,
            Some(recovery_env),
            true,
        );

        assert_eq!(manifest.format_version, 2);
        assert_eq!(manifest.vault_id, "test-vault");
        assert_eq!(manifest.name, "Production");

        // Unlock with password
        let unlocked = manifest.unlock(password).unwrap();
        assert_eq!(unlocked.expose_bytes(), dek.expose_bytes());

        // Unlock with recovery key
        let unlocked_rec = manifest.unlock_with_recovery(recovery_key).unwrap();
        assert_eq!(unlocked_rec.expose_bytes(), dek.expose_bytes());

        // Incorrect password fails
        assert!(manifest.unlock("WrongPassword999!").is_err());

        // Change password atomically without changing DEK
        let new_password = "UpdatedPassword789$";
        manifest
            .change_password_with_params(
                &unlocked,
                new_password,
                TEST_M_COST,
                TEST_T_COST,
                TEST_P_COST,
            )
            .unwrap();

        // Old password no longer works
        assert!(manifest.unlock(password).is_err());

        // New password recovers the exact same DEK
        let unlocked_new = manifest.unlock(new_password).unwrap();
        assert_eq!(unlocked_new.expose_bytes(), dek.expose_bytes());

        // Recovery key still works
        let unlocked_rec2 = manifest.unlock_with_recovery(recovery_key).unwrap();
        assert_eq!(unlocked_rec2.expose_bytes(), dek.expose_bytes());
    }

    #[test]
    fn manifest_serialization_and_disk_persistence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let manifest_path = temp_dir.path().join("vault.manifest.json");

        let vid = VaultId::new("disk-vault").unwrap();
        let dek = VaultKey::generate().unwrap();
        let password = "StrongPassword321@";

        let primary_env = dek
            .wrap_with_argon2_params(&vid, password, TEST_M_COST, TEST_T_COST, TEST_P_COST)
            .unwrap();
        let manifest = VaultManifest::new(
            vid.as_str().to_string(),
            "Disk Vault".to_string(),
            primary_env,
            None,
            false,
        );

        // Save to disk
        manifest.save_to_path(&manifest_path).unwrap();
        assert!(manifest_path.exists());

        // Load from disk
        let loaded = VaultManifest::load_from_path(&manifest_path).unwrap();
        assert_eq!(loaded.vault_id, "disk-vault");
        assert_eq!(loaded.name, "Disk Vault");
        assert!(!loaded.remember_in_keychain);

        // Unlock loaded manifest
        let recovered = loaded.unlock(password).unwrap();
        assert_eq!(recovered.expose_bytes(), dek.expose_bytes());
    }
}

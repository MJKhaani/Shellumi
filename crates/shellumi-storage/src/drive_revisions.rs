//! Local encrypted protocol cache; no keys or credential values are serialized.
use super::*;
use base64::Engine;
use shellumi_core::sync::revisions::{
    key_content, metadata_content, revision_key, CloudState, RevisionHistory, REVISION_VERSION,
};
use std::collections::BTreeMap;

#[derive(serde::Serialize, serde::Deserialize)]
struct Cache {
    vault: shellumi_core::SyncVault,
    digests: BTreeMap<String, String>,
}

fn cache_id(id: &str) -> String {
    format!("drive_revision_state:{id}")
}
fn record_id(record: &shellumi_core::SyncRecord) -> String {
    format!("{}:{}", record.entity_type, record.object_id)
}
fn payload_digest(
    record: &shellumi_core::SyncRecord,
    pass: &str,
    key: &shellumi_core::VaultKey,
) -> Result<String, StorageError> {
    if record.deleted {
        return Ok("deleted".into());
    }
    let envelope = record
        .envelope
        .as_ref()
        .ok_or_else(|| StorageError::Serialization("Live sync record lacks ciphertext".into()))?;
    let payload = shellumi_core::decrypt_entity::<serde_json::Value>(
        &base64::engine::general_purpose::STANDARD.encode(key.expose_bytes()),
        envelope,
    )
    .or_else(|_| shellumi_core::decrypt_entity(pass, envelope))
    .or_else(|_| shellumi_core::decrypt_entity(&hex::encode(key.expose_bytes()), envelope))?;
    let bytes =
        serde_json::to_vec(&payload).map_err(|e| StorageError::Serialization(e.to_string()))?;
    Ok(hex::encode(ring::digest::digest(
        &ring::digest::SHA256,
        &bytes,
    )))
}

fn error(message: &str) -> StorageError {
    tracing::warn!(reason = message, "Rejected local cloud revision transition");
    StorageError::Serialization(message.into())
}

impl HostStore {
    async fn drive_cache(&self) -> Result<Option<Cache>, StorageError> {
        self.get_encrypted_vault_directory(&cache_id(self.vault_id().as_str()))
            .await?
            .map(|json| {
                serde_json::from_str(&json)
                    .map_err(|_| error("Invalid encrypted drive revision cache"))
            })
            .transpose()
    }

    async fn save_drive_cache(
        &self,
        vault: &shellumi_core::SyncVault,
        pass: &str,
        key: &shellumi_core::VaultKey,
    ) -> Result<(), StorageError> {
        let mut digests = BTreeMap::new();
        for record in &vault.records {
            digests.insert(record_id(record), payload_digest(record, pass, key)?);
        }
        let json = serde_json::to_string(&Cache {
            vault: vault.clone(),
            digests,
        })
        .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.connection.execute("INSERT INTO vault_directories(id,encrypted_data) VALUES (?1,?2) ON CONFLICT(id) DO UPDATE SET encrypted_data=excluded.encrypted_data",(cache_id(&vault.vault_id),json)).await?;
        Ok(())
    }

    pub(super) async fn prepare_drive_export(
        &self,
        vault: &mut shellumi_core::SyncVault,
        pass: &str,
        manifest: Option<&shellumi_core::VaultManifest>,
    ) -> Result<(), StorageError> {
        let key = revision_key(vault, pass)?;
        let has_vault_key =
            vault.key_envelope.is_some() || hex::decode(pass).is_ok_and(|bytes| bytes.len() == 32);
        if self.vault_id().as_str() == LEGACY_LOCAL_VAULT_ID && has_vault_key {
            let directory = self.build_vault_directory().await?;
            vault.encrypted_directory = Some(
                self.save_vault_directory_from_model(self.vault_id(), &key, &directory)
                    .await?,
            );
        }
        let cached = self.drive_cache().await?;
        if let Some(cache) = &cached {
            if cache.vault.vault_id != vault.vault_id {
                return Err(error("Revision cache belongs to another vault"));
            }
            cache.vault.validate_cloud_revision(&key, false)?;
            vault.cloud_state = cache.vault.cloud_state.clone();
        }
        if let Some(manifest) = manifest {
            if manifest.vault_id != vault.vault_id {
                return Err(error("Portable manifest belongs to another vault"));
            }
            if let Some(history) = &manifest.key_revision {
                history.verify(&vault.vault_id, "key", &key)?;
                if history.content() != key_content(&vault.key_envelope)? {
                    return Err(error(
                        "Manifest envelope does not match its authenticated revision",
                    ));
                }
                if let Some(state) = vault.cloud_state.as_mut() {
                    if state.key.contains(&history.head) && state.key.head != history.head {
                        return Err(error("Portable manifest is older than the accepted cloud envelope; reload/recover it before syncing"));
                    }
                    state.key = history.clone();
                } else {
                    vault.cloud_state = Some(CloudState {
                        version: REVISION_VERSION,
                        key: history.clone(),
                        metadata: RevisionHistory::root(
                            &vault.vault_id,
                            "metadata",
                            metadata_content(vault)?,
                            &key,
                        )?,
                        base_publications: Vec::new(),
                        publication: None,
                    });
                }
            } else if cached
                .as_ref()
                .is_some_and(|c| c.vault.key_envelope != vault.key_envelope)
            {
                return Err(error("Unversioned manifest attempted to replace an accepted envelope; change the password using the current manifest"));
            }
        } else if cached
            .as_ref()
            .is_some_and(|c| c.vault.key_envelope != vault.key_envelope)
        {
            return Err(error(
                "Unversioned key envelope cannot replace a known revision",
            ));
        }
        vault.prepare_cloud_revision(&key)?;
        let mut digests = BTreeMap::new();
        for record in &mut vault.records {
            let id = record_id(record);
            let digest = payload_digest(record, pass, &key)?;
            if let Some(cache) = &cached {
                if let Some(previous) = cache.vault.records.iter().find(|r| record_id(r) == id) {
                    if cache.digests.get(&id) == Some(&digest) {
                        record.version = previous.version;
                        record.updated_at = previous.updated_at;
                        record.device_id = previous.device_id.clone();
                    } else {
                        record.version = previous
                            .version
                            .checked_add(1)
                            .ok_or_else(|| error("Record revision exhausted"))?;
                    }
                }
            }
            digests.insert(id, digest);
        }
        if let Some(cache) = &cached {
            for previous in &cache.vault.records {
                if !digests.contains_key(&record_id(previous)) {
                    let mut deleted = previous.clone();
                    if !deleted.deleted {
                        deleted.version = deleted
                            .version
                            .checked_add(1)
                            .ok_or_else(|| error("Record revision exhausted"))?;
                        deleted.deleted = true;
                        deleted.envelope = None;
                    }
                    vault.records.push(deleted);
                }
            }
        }
        let parents = vault
            .cloud_state
            .as_ref()
            .ok_or_else(|| error("Missing prepared cloud state"))?
            .base_publications
            .clone();
        vault.sign_local_snapshot(&key, parents)?;
        self.save_drive_cache(vault, pass, &key).await?;
        if let Some(manifest) = manifest {
            if manifest.key_revision.is_none() {
                let mut upgraded = manifest.clone();
                upgraded.key_revision = vault.cloud_state.as_ref().map(|s| s.key.clone());
                if let Some(path) = &self.db_path {
                    let path = shellumi_core::VaultManifest::manifest_path_for_vault(
                        std::path::Path::new(path),
                        self.vault_id(),
                    );
                    // Use the same path selected by export (portable sibling fallback).
                    let actual = if path.exists() {
                        path
                    } else {
                        std::path::PathBuf::from(format!(
                            "{}.manifest",
                            self.db_path.as_deref().unwrap_or_default()
                        ))
                    };
                    upgraded.save_to_path(&actual)?;
                }
            }
        }
        Ok(())
    }

    /// Validate high-water marks before any local data or manifest replacement.
    pub(super) async fn validate_drive_import(
        &self,
        vault: &shellumi_core::SyncVault,
        pass: &str,
    ) -> Result<Option<shellumi_core::VaultKey>, StorageError> {
        if vault.vault_id != self.vault_id().as_str() {
            return Err(error("Sync import vault ownership mismatch"));
        }
        let cache = self.drive_cache().await?;
        if vault.cloud_state.is_none() {
            if cache.is_some() {
                return Err(error(
                    "Legacy snapshot cannot roll back authenticated cloud state",
                ));
            }
            if let Some(db) = &self.db_path {
                let path = shellumi_core::VaultManifest::manifest_path_for_vault(
                    std::path::Path::new(db),
                    self.vault_id(),
                );
                if path.exists()
                    && shellumi_core::VaultManifest::load_from_path(&path)?
                        .key_revision
                        .is_some()
                {
                    return Err(error(
                        "Unversioned import cannot replace a revisioned local manifest",
                    ));
                }
            }
            return Ok(None);
        }
        let key = match revision_key(vault, pass) {
            Ok(key) => key,
            Err(original) => {
                if let Some(cache) = &cache {
                    revision_key(&cache.vault, pass)?
                } else {
                    return Err(original.into());
                }
            }
        };
        vault.validate_cloud_revision(&key, false)?;
        if vault
            .cloud_state
            .as_ref()
            .is_some_and(|s| s.publication.is_none())
        {
            return Err(error("Unsigned revised snapshot cannot be imported"));
        }
        if let Some(cache) = &cache {
            cache.vault.validate_cloud_revision(&key, false)?;
            let old = cache
                .vault
                .cloud_state
                .as_ref()
                .ok_or_else(|| error("Missing cache revisions"))?;
            let new = vault
                .cloud_state
                .as_ref()
                .ok_or_else(|| error("Missing imported revisions"))?;
            if !new.key.contains(&old.key.head) || !new.metadata.contains(&old.metadata.head) {
                return Err(error("Stale or divergent envelope/metadata import requires explicit conflict resolution"));
            }
        }
        if let Some(path) = &self.db_path {
            let manifest_path = shellumi_core::VaultManifest::manifest_path_for_vault(
                std::path::Path::new(path),
                self.vault_id(),
            );
            if manifest_path.exists() {
                let manifest = shellumi_core::VaultManifest::load_from_path(&manifest_path)?;
                if let Some(old) = &manifest.key_revision {
                    old.verify(&vault.vault_id, "key", &key)?;
                    if !vault
                        .cloud_state
                        .as_ref()
                        .ok_or_else(|| error("Missing imported revision"))?
                        .key
                        .contains(&old.head)
                    {
                        return Err(error(
                            "Remote envelope would roll back a local password change",
                        ));
                    }
                }
            }
        }
        Ok(Some(key))
    }

    pub(super) async fn finish_drive_import(
        &self,
        vault: &shellumi_core::SyncVault,
        pass: &str,
        key: Option<&shellumi_core::VaultKey>,
    ) -> Result<(), StorageError> {
        if let Some(key) = key {
            self.save_drive_cache(vault, pass, key).await?;
        }
        Ok(())
    }
}

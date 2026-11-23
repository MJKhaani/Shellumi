//! Authenticated component histories and immutable cloud publications.
use super::*;
use ring::hmac;
use std::collections::{BTreeMap, BTreeSet};

pub const REVISION_VERSION: u32 = 1;
const MAX_NODES: usize = 100_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionNode {
    pub version: u32,
    pub generation: u64,
    pub parents: Vec<String>,
    pub content: String,
    pub mac: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionHistory {
    pub head: String,
    pub nodes: BTreeMap<String, RevisionNode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Publication {
    pub version: u32,
    pub id: String,
    pub parents: Vec<String>,
    pub mac: String,
    pub cloud: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudState {
    pub version: u32,
    pub key: RevisionHistory,
    pub metadata: RevisionHistory,
    #[serde(default)]
    pub base_publications: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<Publication>,
}

pub(crate) fn protocol_error(message: &str) -> SyncError {
    tracing::warn!(reason = message, "Rejected cloud revision operation");
    SyncError::Api(message.into())
}

pub(crate) fn hash_json(value: &impl Serialize) -> Result<String, SyncError> {
    let data = serde_json::to_vec(value).map_err(|e| SyncError::Serialization(e.to_string()))?;
    Ok(hex::encode(digest(&SHA256, &data)))
}

fn node_bytes(vault: &str, component: &str, node: &RevisionNode) -> Result<Vec<u8>, SyncError> {
    serde_json::to_vec(&(
        "shellumi:cloud-component:v1",
        vault,
        component,
        node.version,
        node.generation,
        &node.parents,
        &node.content,
    ))
    .map_err(|e| SyncError::Serialization(e.to_string()))
}

fn mac(key: &crate::VaultKey, data: &[u8]) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key.expose_bytes());
    BASE64_STANDARD.encode(hmac::sign(&key, data))
}

fn verify_mac(key: &crate::VaultKey, data: &[u8], signature: &str) -> Result<(), SyncError> {
    let signature = BASE64_STANDARD
        .decode(signature)
        .map_err(|_| protocol_error("Invalid revision authentication encoding"))?;
    hmac::verify(
        &hmac::Key::new(hmac::HMAC_SHA256, key.expose_bytes()),
        data,
        &signature,
    )
    .map_err(|_| protocol_error("Cloud revision authentication or vault ownership failed"))
}

impl RevisionHistory {
    pub fn root(
        vault: &str,
        component: &str,
        content: String,
        key: &crate::VaultKey,
    ) -> Result<Self, SyncError> {
        Self::append(vault, component, content, &[], key)
    }

    pub fn append(
        vault: &str,
        component: &str,
        content: String,
        histories: &[&Self],
        key: &crate::VaultKey,
    ) -> Result<Self, SyncError> {
        let mut nodes = BTreeMap::new();
        let mut parents = BTreeSet::new();
        let mut generation = 0;
        for history in histories {
            history.verify(vault, component, key)?;
            generation = generation.max(
                history.nodes[&history.head]
                    .generation
                    .checked_add(1)
                    .ok_or_else(|| protocol_error("Revision generation exhausted"))?,
            );
            parents.insert(history.head.clone());
            for (id, node) in &history.nodes {
                nodes.insert(id.clone(), node.clone());
            }
        }
        let mut node = RevisionNode {
            version: REVISION_VERSION,
            generation,
            parents: parents.into_iter().collect(),
            content,
            mac: String::new(),
        };
        let data = node_bytes(vault, component, &node)?;
        let head = hex::encode(digest(&SHA256, &data));
        node.mac = mac(key, &data);
        nodes.insert(head.clone(), node);
        let history = Self { head, nodes };
        history.verify(vault, component, key)?;
        Ok(history)
    }

    pub fn verify(
        &self,
        vault: &str,
        component: &str,
        key: &crate::VaultKey,
    ) -> Result<(), SyncError> {
        if self.nodes.is_empty()
            || self.nodes.len() > MAX_NODES
            || !self.nodes.contains_key(&self.head)
        {
            return Err(protocol_error("Invalid component revision history"));
        }
        for (id, node) in &self.nodes {
            if node.version != REVISION_VERSION || node.parents.len() > MAX_NODES {
                return Err(protocol_error("Unsupported component revision version"));
            }
            let data = node_bytes(vault, component, node)?;
            if hex::encode(digest(&SHA256, &data)) != *id {
                return Err(protocol_error("Component revision identifier mismatch"));
            }
            verify_mac(key, &data, &node.mac)?;
            if node.parents.is_empty() && node.generation != 0 {
                return Err(protocol_error("Invalid revision root"));
            }
            for parent in &node.parents {
                let previous = self
                    .nodes
                    .get(parent)
                    .ok_or_else(|| protocol_error("Missing component revision ancestor"))?;
                if previous.generation >= node.generation {
                    return Err(protocol_error("Component revision rollback or cycle"));
                }
            }
        }
        let reachable = self.ancestors();
        if reachable.len() != self.nodes.len() {
            return Err(protocol_error("Unbound component history entries"));
        }
        Ok(())
    }

    pub fn content(&self) -> &str {
        &self.nodes[&self.head].content
    }

    pub fn contains(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    fn ancestors(&self) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut pending = vec![self.head.clone()];
        while let Some(id) = pending.pop() {
            if found.insert(id.clone()) {
                if let Some(node) = self.nodes.get(&id) {
                    pending.extend(node.parents.iter().cloned());
                }
            }
        }
        found
    }
}

pub fn key_content(envelope: &Option<EncryptedEnvelope>) -> Result<String, SyncError> {
    hash_json(envelope)
}
pub fn metadata_content(vault: &SyncVault) -> Result<String, SyncError> {
    hash_json(&vault.encrypted_directory)
}

pub fn validate_key_envelope(vault: &str, envelope: &EncryptedEnvelope) -> Result<(), SyncError> {
    let salt = BASE64_STANDARD
        .decode(&envelope.salt)
        .map_err(|_| protocol_error("Invalid key-envelope salt"))?;
    let nonce = BASE64_STANDARD
        .decode(&envelope.nonce)
        .map_err(|_| protocol_error("Invalid key-envelope nonce"))?;
    let cipher = BASE64_STANDARD
        .decode(&envelope.ciphertext)
        .map_err(|_| protocol_error("Invalid wrapped-key ciphertext"))?;
    if salt.len() != 16 || nonce.len() != 12 || cipher.len() != 48 {
        return Err(protocol_error("Invalid wrapped-key envelope lengths"));
    }
    let expected = match envelope.format_version {
        1 if envelope.suite == "argon2id_aes256gcm_legacy"
            && (DEFAULT_KDF_ITERATIONS..=2_000_000).contains(&envelope.kdf_iterations) =>
        {
            format!("shellumi:vault-key-recovery:v1:{vault}")
        }
        2 if envelope.suite == "argon2id_aes256gcm_v2" => {
            let (m, t, p) = (
                envelope.argon2_m_cost.unwrap_or(0),
                envelope.argon2_t_cost.unwrap_or(0),
                envelope.argon2_p_cost.unwrap_or(0),
            );
            if !(1..=16).contains(&p) || m < 8 * p || m > 262144 || !(1..=10).contains(&t) {
                return Err(protocol_error(
                    "Unsupported key-envelope resource parameters",
                ));
            }
            format!("shellumi:vault-envelope:v2:{vault}:argon2id_aes256gcm_v2:{m}:{t}:{p}")
        }
        _ => return Err(protocol_error("Unsupported key-envelope format")),
    };
    if envelope.associated_data != expected {
        return Err(protocol_error("Key-envelope vault ownership mismatch"));
    }
    Ok(())
}

/// Resolve a stable authentication key without serializing it into cloud state.
pub fn revision_key(vault: &SyncVault, passphrase: &str) -> Result<crate::VaultKey, SyncError> {
    if passphrase.len() == 64 {
        if let Ok(bytes) = hex::decode(passphrase) {
            if let Ok(bytes) = <[u8; 32]>::try_from(bytes) {
                return Ok(crate::VaultKey::from_bytes(bytes));
            }
        }
    }
    if let Some(envelope) = &vault.key_envelope {
        validate_key_envelope(&vault.vault_id, envelope)?;
        return crate::VaultKey::unwrap_with_password(
            &crate::VaultId::new(&vault.vault_id)
                .map_err(|e| SyncError::InvalidRecord(e.to_string()))?,
            passphrase,
            envelope,
        );
    }
    let salt = digest(
        &SHA256,
        format!("shellumi:legacy-revision-key:v1:{}", vault.vault_id).as_bytes(),
    );
    let key = derive_key(
        passphrase,
        &salt.as_ref()[..SALT_LEN],
        DEFAULT_KDF_ITERATIONS,
    )?;
    Ok(crate::VaultKey::from_bytes(*key))
}

impl SyncVault {
    /// Sign changed components independently. A detached local working snapshot
    /// carries the last accepted publication as a rollback anchor.
    pub fn prepare_cloud_revision(&mut self, key: &crate::VaultKey) -> Result<(), SyncError> {
        let key_hash = key_content(&self.key_envelope)?;
        let metadata_hash = metadata_content(self)?;
        if let Some(mut state) = self.cloud_state.take() {
            if state.version != REVISION_VERSION {
                return Err(protocol_error("Unsupported cloud revision version"));
            }
            state.key.verify(&self.vault_id, "key", key)?;
            state.metadata.verify(&self.vault_id, "metadata", key)?;
            if state.key.content() != key_hash {
                state.key =
                    RevisionHistory::append(&self.vault_id, "key", key_hash, &[&state.key], key)?;
            }
            if state.metadata.content() != metadata_hash {
                state.metadata = RevisionHistory::append(
                    &self.vault_id,
                    "metadata",
                    metadata_hash,
                    &[&state.metadata],
                    key,
                )?;
            }
            if let Some(publication) = state.publication.take() {
                if publication.cloud {
                    state.base_publications = vec![publication.id];
                }
            }
            self.cloud_state = Some(state);
        } else {
            self.cloud_state = Some(CloudState {
                version: REVISION_VERSION,
                key: RevisionHistory::root(&self.vault_id, "key", key_hash, key)?,
                metadata: RevisionHistory::root(&self.vault_id, "metadata", metadata_hash, key)?,
                base_publications: Vec::new(),
                publication: None,
            });
        }
        Ok(())
    }

    pub fn validate_cloud_revision(
        &self,
        key: &crate::VaultKey,
        require_publication: bool,
    ) -> Result<(), SyncError> {
        let state = self
            .cloud_state
            .as_ref()
            .ok_or_else(|| protocol_error("Missing cloud revision state"))?;
        if state.version != REVISION_VERSION {
            return Err(protocol_error("Unsupported cloud revision version"));
        }
        state.key.verify(&self.vault_id, "key", key)?;
        state.metadata.verify(&self.vault_id, "metadata", key)?;
        if let Some(envelope) = &self.key_envelope {
            validate_key_envelope(&self.vault_id, envelope)?;
        }
        let absent_key = key_content(&None)?;
        if self.key_envelope.is_none() && state.key.nodes.values().any(|n| n.content != absent_key)
        {
            return Err(protocol_error(
                "A wrapped vault key cannot be removed from its revision history",
            ));
        }
        if state.key.content() != key_content(&self.key_envelope)?
            || state.metadata.content() != metadata_content(self)?
        {
            return Err(protocol_error(
                "Cloud revision does not authenticate the supplied envelope or metadata",
            ));
        }
        if require_publication || state.publication.is_some() {
            let publication = state
                .publication
                .as_ref()
                .ok_or_else(|| protocol_error("Unsigned cloud publication"))?;
            if require_publication && !publication.cloud {
                return Err(protocol_error(
                    "Unpublished local snapshot was stored as a cloud publication",
                ));
            }
            if publication.version != REVISION_VERSION
                || publication.parents != state.base_publications
            {
                return Err(protocol_error("Invalid cloud publication ancestry"));
            }
            let data = self.publication_bytes(publication.cloud)?;
            if publication.id != hex::encode(digest(&SHA256, &data)) {
                return Err(protocol_error("Cloud publication identity mismatch"));
            }
            verify_mac(key, &data, &publication.mac)?;
        }
        Ok(())
    }

    fn publication_bytes(&self, cloud: bool) -> Result<Vec<u8>, SyncError> {
        let mut snapshot = self.clone();
        if let Some(state) = snapshot.cloud_state.as_mut() {
            state.publication = None;
        }
        serde_json::to_vec(&("shellumi:cloud-publication:v1", cloud, snapshot))
            .map_err(|e| SyncError::Serialization(e.to_string()))
    }

    pub fn sign_publication(
        &mut self,
        key: &crate::VaultKey,
        parents: Vec<String>,
    ) -> Result<(), SyncError> {
        self.seal_snapshot(key, parents, true)
    }
    pub fn sign_local_snapshot(
        &mut self,
        key: &crate::VaultKey,
        parents: Vec<String>,
    ) -> Result<(), SyncError> {
        self.seal_snapshot(key, parents, false)
    }
    fn seal_snapshot(
        &mut self,
        key: &crate::VaultKey,
        mut parents: Vec<String>,
        cloud: bool,
    ) -> Result<(), SyncError> {
        parents.sort();
        parents.dedup();
        self.cloud_state
            .as_mut()
            .ok_or_else(|| protocol_error("Missing cloud state"))?
            .base_publications = parents.clone();
        let data = self.publication_bytes(cloud)?;
        let publication = Publication {
            version: REVISION_VERSION,
            id: hex::encode(digest(&SHA256, &data)),
            parents,
            mac: mac(key, &data),
            cloud,
        };
        self.cloud_state
            .as_mut()
            .ok_or_else(|| protocol_error("Missing cloud state"))?
            .publication = Some(publication);
        Ok(())
    }

    pub fn publication_id(&self) -> Result<String, SyncError> {
        self.cloud_state
            .as_ref()
            .and_then(|s| s.publication.as_ref())
            .map(|p| p.id.clone())
            .map(Ok)
            .unwrap_or_else(|| hash_json(&("shellumi:legacy-publication:v1", self)))
    }
}

/// Convert legacy payload ciphers to a stable DEK cipher before publication.
/// A password change then needs only a new key envelope, never the old password.
pub fn normalize_records(
    vault: &mut SyncVault,
    key: &crate::VaultKey,
    supplied: &str,
) -> Result<(), SyncError> {
    let encoded = BASE64_STANDARD.encode(key.expose_bytes());
    let hex_key = hex::encode(key.expose_bytes());
    for record in &mut vault.records {
        if record.deleted {
            continue;
        }
        let envelope = record
            .envelope
            .as_ref()
            .ok_or_else(|| protocol_error("Live cloud record has no encrypted payload"))?;
        let payload = decrypt_entity::<serde_json::Value>(&encoded,envelope)
            .or_else(|_|decrypt_entity(supplied,envelope)).or_else(|_|decrypt_entity(&hex_key,envelope))
            .map_err(|_|protocol_error("Cannot authenticate legacy record; import using its original password before rotating it"))?;
        let aad = format!(
            "shellumi:drive-record:v1:{}:{}:{}",
            vault.vault_id, record.entity_type, record.object_id
        );
        if envelope.salt.is_empty()
            && envelope.kdf_iterations == 0
            && envelope.associated_data == aad
        {
            continue;
        }
        let nonce = generate_nonce()?;
        let data =
            serde_json::to_vec(&payload).map_err(|e| SyncError::Serialization(e.to_string()))?;
        record.envelope = Some(EncryptedEnvelope {
            format_version: 1,
            suite: "aes256gcm_dek_v1".into(),
            key_epoch: 0,
            salt: String::new(),
            nonce: BASE64_STANDARD.encode(nonce),
            ciphertext: BASE64_STANDARD.encode(seal_payload(
                key.expose_bytes(),
                &nonce,
                &data,
                aad.as_bytes(),
            )?),
            associated_data: aad,
            kdf_iterations: 0,
            argon2_m_cost: None,
            argon2_t_cost: None,
            argon2_p_cost: None,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_histories_reject_forged_generations_wrong_owners_and_versions() {
        let key = crate::VaultKey::from_bytes([7; 32]);
        let root = RevisionHistory::root("vault", "key", "old".into(), &key).unwrap();
        let next = RevisionHistory::append("vault", "key", "new".into(), &[&root], &key).unwrap();
        assert!(next.contains(&root.head));
        assert!(!root.contains(&next.head));
        assert!(next.verify("foreign", "key", &key).is_err());
        assert!(next.verify("vault", "metadata", &key).is_err());
        assert!(next
            .verify("vault", "key", &crate::VaultKey::from_bytes([8; 32]))
            .is_err());
        for field in ["generation", "version", "mac", "content"] {
            let mut altered = next.clone();
            let node = altered.nodes.get_mut(&next.head).unwrap();
            match field {
                "generation" => node.generation += 100,
                "version" => node.version += 1,
                "mac" => node.mac = "forged".into(),
                _ => node.content = "forged".into(),
            }
            assert!(altered.verify("vault", "key", &key).is_err());
        }
        let concurrent =
            RevisionHistory::append("vault", "key", "other".into(), &[&root], &key).unwrap();
        let resolved =
            RevisionHistory::append("vault", "key", "new".into(), &[&next, &concurrent], &key)
                .unwrap();
        assert!(resolved.contains(&next.head) && resolved.contains(&concurrent.head));
    }

    #[test]
    fn metadata_revisions_and_complete_snapshot_authentication_are_independent() {
        let key = crate::VaultKey::from_bytes([7; 32]);
        let mut vault = SyncVault::new("vault", 1);
        vault.prepare_cloud_revision(&key).unwrap();
        vault.sign_publication(&key, vec![]).unwrap();
        let old = vault.clone();
        let old_state = old.cloud_state.as_ref().unwrap();
        vault.encrypted_directory = Some("opaque-directory-ciphertext".into());
        vault.prepare_cloud_revision(&key).unwrap();
        let state = vault.cloud_state.as_ref().unwrap();
        assert_eq!(old_state.key, state.key);
        assert_ne!(old_state.metadata.head, state.metadata.head);
        assert!(state.metadata.contains(&old_state.metadata.head));
        vault
            .sign_publication(&key, vec![old.publication_id().unwrap()])
            .unwrap();
        vault.validate_cloud_revision(&key, true).unwrap();
        let file = DriveFileMetadata {
            id: "cloud".into(),
            name: drive_vault_filename("vault"),
            mime_type: "application/json".into(),
            modified_time: None,
            size: None,
        };
        assert!(matches!(
            plan_sync_cycle(&vault, Some(&file), Some(&old), None),
            SyncPlan::UploadLocalUpdates { .. }
        ));
        assert!(matches!(
            plan_sync_cycle(&old, Some(&file), Some(&vault), None),
            SyncPlan::ApplyRemoteUpdates { .. }
        ));
        assert!(matches!(
            plan_sync_cycle(&vault, Some(&file), Some(&vault), None),
            SyncPlan::AlreadySynchronized
        ));
        let mut altered = vault.clone();
        altered.updated_at += 1;
        assert!(altered.validate_cloud_revision(&key, true).is_err());
        let mut altered = vault.clone();
        altered.cloud_state.as_mut().unwrap().version += 1;
        assert!(altered.validate_cloud_revision(&key, true).is_err());
        let SyncCycleResult::Success { report, .. } =
            execute_snapshot_sync(&old, &vault, &hex::encode(key.expose_bytes()), None).unwrap()
        else {
            panic!("portable metadata update was lost")
        };
        let snapshot = report.snapshot.unwrap();
        assert_eq!(snapshot.encrypted_directory, vault.encrypted_directory);
        snapshot.validate_cloud_revision(&key, false).unwrap();
        let json = vault.to_json().unwrap();
        assert_eq!(SyncVault::from_json(&json).unwrap(), vault);
        let mut legacy = serde_json::to_value(&old).unwrap();
        legacy.as_object_mut().unwrap().remove("cloud_state");
        assert!(serde_json::from_value::<SyncVault>(legacy)
            .unwrap()
            .cloud_state
            .is_none());
    }
}

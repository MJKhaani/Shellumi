//! Append-only Drive synchronization. No existing cloud object is ever updated.
use super::revisions::{hash_json, protocol_error, revision_key};
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct CloudConflictResolution {
    pub context: String,
    pub choices: Vec<ConflictResolutionChoice>,
}

struct Remote {
    id: String,
    vault: SyncVault,
    parents: Vec<String>,
    legacy: bool,
}

fn matches_file(file: &DriveFileMetadata, vault: &str) -> bool {
    file.name == drive_vault_filename(vault)
        || ((vault == "legacy-local" || vault == "default")
            && file.name == DRIVE_LEGACY_APPDATA_FILENAME)
}

fn load<T: HttpTransport>(
    client: &GoogleDriveSyncClient<T>,
    token: &str,
    vault: &str,
) -> Result<Vec<SyncVault>, SyncError> {
    let files = client.list_vault_files(token)?;
    let mut snapshots = Vec::new();
    for file in files.iter().filter(|f| matches_file(f, vault)) {
        let snapshot = client.download_vault(token, &file.id)?;
        if snapshot.vault_id != vault {
            return Err(protocol_error(
                "Drive snapshot belongs to a different vault",
            ));
        }
        snapshots.push(snapshot);
    }
    Ok(snapshots)
}

fn auth_key(
    local: &SyncVault,
    remote: &[SyncVault],
    pass: &str,
) -> Result<crate::VaultKey, SyncError> {
    if pass.len() == 64 || local.key_envelope.is_some() {
        return revision_key(local, pass);
    }
    let mut envelopes: Vec<_> = remote.iter().filter(|v| v.key_envelope.is_some()).collect();
    envelopes.sort_by_key(|v| {
        std::cmp::Reverse(
            v.cloud_state
                .as_ref()
                .and_then(|s| s.key.nodes.get(&s.key.head))
                .map_or(0, |n| n.generation),
        )
    });
    let mut tried = BTreeSet::new();
    if !envelopes.is_empty() {
        for snapshot in envelopes {
            if !tried.insert(revisions::key_content(&snapshot.key_envelope)?) {
                continue;
            }
            if tried.len() > 32 {
                break;
            }
            if let Ok(key) = revision_key(snapshot, pass) {
                return Ok(key);
            }
        }
        return Err(SyncError::DecryptionFailed);
    }
    revision_key(local, pass)
}

fn authenticate(
    raw: Vec<SyncVault>,
    key: &crate::VaultKey,
) -> Result<(BTreeMap<String, Remote>, Vec<String>), SyncError> {
    let mut snapshots = BTreeMap::new();
    for mut vault in raw {
        let id = vault.publication_id()?;
        let legacy = vault.cloud_state.is_none();
        let parents = if let Some(state) = &vault.cloud_state {
            vault.validate_cloud_revision(key, true)?;
            state
                .publication
                .as_ref()
                .ok_or_else(|| protocol_error("Missing publication"))?
                .parents
                .clone()
        } else {
            vault.prepare_cloud_revision(key)?;
            Vec::new()
        };
        snapshots.entry(id.clone()).or_insert(Remote {
            id,
            vault,
            parents,
            legacy,
        });
    }
    let mut parents = BTreeSet::new();
    for snapshot in snapshots.values() {
        for parent in &snapshot.parents {
            if !snapshots.contains_key(parent) || parent == &snapshot.id {
                return Err(protocol_error("Cloud history is incomplete or rolled back; retained local state was not replaced"));
            }
            parents.insert(parent.clone());
        }
    }
    let heads: Vec<_> = snapshots
        .keys()
        .filter(|id| !parents.contains(*id))
        .cloned()
        .collect();
    let mut reachable = BTreeSet::new();
    let mut pending = heads.clone();
    while let Some(id) = pending.pop() {
        if reachable.insert(id.clone()) {
            pending.extend(snapshots[&id].parents.iter().cloned());
        }
    }
    if reachable.len() != snapshots.len() {
        return Err(protocol_error(
            "Cyclic or disconnected cloud publication history",
        ));
    }
    Ok((snapshots, heads))
}

fn component(vault: &SyncVault, key: bool) -> &RevisionHistory {
    let state = vault.cloud_state.as_ref().expect("prepared snapshot");
    if key {
        &state.key
    } else {
        &state.metadata
    }
}

fn select_component(
    current: &mut SyncVault,
    other: &SyncVault,
    key_component: bool,
    choice: Option<ConflictResolutionChoice>,
    key: &crate::VaultKey,
) -> Result<Option<ConflictItemView>, SyncError> {
    let local = component(current, key_component).clone();
    let remote = component(other, key_component);
    if local.head == remote.head || local.contains(&remote.head) {
        return Ok(None);
    }
    if remote.contains(&local.head) {
        if key_component {
            current.key_envelope = other.key_envelope.clone();
            current.cloud_state.as_mut().expect("prepared").key = remote.clone();
        } else {
            current.encrypted_directory = other.encrypted_directory.clone();
            current.cloud_state.as_mut().expect("prepared").metadata = remote.clone();
        }
        return Ok(None);
    }
    let name = if key_component { "key" } else { "metadata" };
    let view = ConflictItemView::new(
        format!("vault:{name}"),
        if key_component {
            SyncEntityType::Identity
        } else {
            SyncEntityType::Group
        },
        local.nodes[&local.head].generation,
        remote.nodes[&remote.head].generation,
        0,
        0,
        format!("Local encrypted vault {name}; branch {}", local.head),
        format!("Remote encrypted vault {name}; branch {}", remote.head),
    );
    if let Some(choice) = choice {
        if choice == ConflictResolutionChoice::AcceptRemote {
            if key_component {
                current.key_envelope = other.key_envelope.clone();
            } else {
                current.encrypted_directory = other.encrypted_directory.clone();
            }
        }
        let content = if key_component {
            revisions::key_content(&current.key_envelope)?
        } else {
            revisions::metadata_content(current)?
        };
        let merged =
            RevisionHistory::append(&current.vault_id, name, content, &[&local, remote], key)?;
        if key_component {
            current.cloud_state.as_mut().expect("prepared").key = merged;
        } else {
            current.cloud_state.as_mut().expect("prepared").metadata = merged;
        }
    }
    Ok(Some(view))
}

fn record_context(local: &SyncVault, pass: &str) -> Result<String, SyncError> {
    let mut records = Vec::new();
    for record in &local.records {
        let payload = record
            .envelope
            .as_ref()
            .map(|e| decrypt_entity::<serde_json::Value>(pass, e))
            .transpose()?;
        records.push((
            record.object_id.clone(),
            record.entity_type,
            record.version,
            record.deleted,
            payload,
        ));
    }
    records.sort_by(|a, b| a.0.cmp(&b.0));
    hash_json(&records)
}

struct Merge {
    vault: SyncVault,
    conflicts: Vec<ConflictItemView>,
    context: String,
}

fn merge(
    local: &SyncVault,
    snapshots: &BTreeMap<String, Remote>,
    heads: &[String],
    pass: &str,
    key: &crate::VaultKey,
    choices: Option<&[ConflictResolutionChoice]>,
) -> Result<Merge, SyncError> {
    let context = hash_json(&(
        component(local, true).head.clone(),
        component(local, false).head.clone(),
        record_context(local, pass)?,
        heads,
    ))?;
    let mut result = local.clone();
    let mut conflicts = Vec::new();
    let choice_at = |index: usize| {
        choices
            .and_then(|c| {
                if c.len() == 1 {
                    c.first()
                } else {
                    c.get(index)
                }
            })
            .copied()
    };
    let mut reviewed_components = BTreeSet::new();
    for id in heads {
        let other = &snapshots[id].vault;
        for kind in [true, false] {
            let candidate = component(other, kind);
            // Record forks may repeat an envelope or retain its predecessor.
            // Review each maximal component branch once, keeping choices stable.
            if !reviewed_components.insert((kind, candidate.head.clone()))
                || heads.iter().any(|head| {
                    let history = component(&snapshots[head].vault, kind);
                    history.head != candidate.head && history.contains(&candidate.head)
                })
            {
                continue;
            }
            if let Some(mut conflict) =
                select_component(&mut result, other, kind, choice_at(conflicts.len()), key)?
            {
                conflict.revision_context = Some(context.clone());
                conflicts.push(conflict);
            }
        }
        let mut views = build_conflict_item_views(&result.records, &other.records, Some(pass));
        let record_choices: Vec<_> = (0..views.len())
            .map(|i| choice_at(conflicts.len() + i).unwrap_or(ConflictResolutionChoice::KeepLocal))
            .collect();
        for view in &mut views {
            view.revision_context = Some(context.clone());
        }
        conflicts.extend(views);
        match execute_sync_cycle(&result.records, &other.records, pass, Some(&record_choices)) {
            SyncCycleResult::Success { merged_records, .. } => result.records = merged_records,
            SyncCycleResult::AlreadySynchronized => {
                // Reuse the accepted ciphertext for semantically identical rows.
                for record in &mut result.records {
                    if let Some(remote) = other.records.iter().find(|r| {
                        r.object_id == record.object_id && r.entity_type == record.entity_type
                    }) {
                        if diff_sync_records_with_passphrase(
                            std::slice::from_ref(record),
                            std::slice::from_ref(remote),
                            Some(pass),
                        )[0]
                        .status
                            == DiffStatus::Identical
                        {
                            *record = remote.clone();
                        }
                    }
                }
            }
            SyncCycleResult::Error(error) => return Err(protocol_error(&error)),
            SyncCycleResult::Conflicts(_) => {
                return Err(protocol_error(
                    "Record conflict choices changed; refresh the conflict view",
                ))
            }
        }
    }
    if choices.is_some_and(|c| c.len() != 1 && c.len() != conflicts.len()) {
        return Err(protocol_error(
            "Conflict choices do not match current revision branches",
        ));
    }
    Ok(Merge {
        vault: result,
        conflicts,
        context,
    })
}

fn report(
    vault: &SyncVault,
    local: usize,
    remote: usize,
    conflicts: usize,
    elapsed: u64,
) -> SyncCycleResult {
    SyncCycleResult::Success {
        merged_records: vault.records.clone(),
        report: SyncExecutionReport {
            local_updated: local,
            remote_updated: remote,
            conflicts_resolved: conflicts,
            duration_ms: elapsed,
            message: "Authenticated vault records, key envelope and metadata synchronized".into(),
            encrypted_directory: vault.encrypted_directory.clone(),
            key_envelope: vault.key_envelope.clone(),
            cloud_state: vault.cloud_state.clone(),
            snapshot: Some(Box::new(vault.clone())),
        },
    }
}

pub(super) fn execute<T: HttpTransport>(
    client: &GoogleDriveSyncClient<T>,
    token: &str,
    original: &SyncVault,
    pass: &str,
    choices: Option<&[ConflictResolutionChoice]>,
) -> Result<SyncCycleResult, SyncError> {
    execute_bound(client, token, original, pass, choices, None)
}

pub(super) fn execute_bound<T: HttpTransport>(
    client: &GoogleDriveSyncClient<T>,
    token: &str,
    original: &SyncVault,
    pass: &str,
    choices: Option<&[ConflictResolutionChoice]>,
    expected: Option<&str>,
) -> Result<SyncCycleResult, SyncError> {
    let start = std::time::Instant::now();
    let raw = load(client, token, &original.vault_id)?;
    let key = auth_key(original, &raw, pass)?;
    let (mut snapshots, heads) = authenticate(raw, &key)?;
    for id in &heads {
        if snapshots[id].legacy {
            revisions::normalize_records(
                &mut snapshots.get_mut(id).expect("known head").vault,
                &key,
                pass,
            )?;
        }
    }
    let bootstrap = original.cloud_state.is_none()
        && original.records.is_empty()
        && original.key_envelope.is_none()
        && original.encrypted_directory.is_none()
        && !heads.is_empty();
    let mut local = if bootstrap {
        snapshots[&heads[0]].vault.clone()
    } else {
        original.clone()
    };
    local.prepare_cloud_revision(&key)?;
    revisions::normalize_records(&mut local, &key, pass)?;
    let record_pass = BASE64_STANDARD.encode(key.expose_bytes());
    for base in &local
        .cloud_state
        .as_ref()
        .expect("prepared")
        .base_publications
    {
        if !snapshots.contains_key(base) {
            return Err(protocol_error("Previously accepted cloud publication is missing; refusing rollback or history replacement"));
        }
    }
    let mut merged = merge(&local, &snapshots, &heads, &record_pass, &key, choices)?;
    if expected.is_some_and(|expected| expected != merged.context) {
        return Err(protocol_error(
            "Cloud or local revisions changed after conflict review; refresh before resolving",
        ));
    }
    if !merged.conflicts.is_empty() && choices.is_none() {
        return Ok(SyncCycleResult::Conflicts(merged.conflicts));
    }
    let mut needs_upload = heads.is_empty() || heads.len() > 1 || !merged.conflicts.is_empty();
    let mut local_changes = 0;
    let mut remote_changes = merged.vault.records.len();
    if heads.len() == 1 {
        let remote = &snapshots[&heads[0]];
        let metadata = DriveFileMetadata {
            id: remote.id.clone(),
            name: drive_vault_filename(&local.vault_id),
            mime_type: "application/json".into(),
            modified_time: None,
            size: None,
        };
        let plan = plan_sync_cycle(
            &merged.vault,
            Some(&metadata),
            Some(&remote.vault),
            Some(&record_pass),
        );
        needs_upload |= !matches!(
            plan,
            SyncPlan::AlreadySynchronized | SyncPlan::ApplyRemoteUpdates { .. }
        );
        let diffs = diff_sync_records_with_passphrase(
            &merged.vault.records,
            &remote.vault.records,
            Some(&record_pass),
        );
        remote_changes = diffs
            .iter()
            .filter(|d| d.status != DiffStatus::Identical)
            .count();
        needs_upload |= remote.legacy
            || remote_changes > 0
            || component(&merged.vault, true).head != component(&remote.vault, true).head
            || component(&merged.vault, false).head != component(&remote.vault, false).head;
        local_changes = diff_sync_records_with_passphrase(
            &local.records,
            &remote.vault.records,
            Some(&record_pass),
        )
        .iter()
        .filter(|d| d.status != DiffStatus::Identical)
        .count();
        if !needs_upload {
            let accepted = &remote.vault;
            let accepted_locally = !bootstrap
                && local
                    .cloud_state
                    .as_ref()
                    .expect("prepared")
                    .base_publications
                    == heads;
            if accepted_locally
                && component(&local, true).head == component(accepted, true).head
                && component(&local, false).head == component(accepted, false).head
                && local_changes == 0
            {
                return Ok(SyncCycleResult::AlreadySynchronized);
            }
            return Ok(report(
                accepted,
                if bootstrap {
                    accepted.records.len()
                } else {
                    local_changes
                },
                0,
                0,
                start.elapsed().as_millis() as u64,
            ));
        }
    }
    merged.vault.sign_publication(&key, heads.clone())?;
    client.publish_immutable(token, &merged.vault)?;
    // Re-read after publication. A racing fork is retained and reported, never
    // overwritten. A later-arriving publication is detected on the next cycle.
    let (after, after_heads) = authenticate(load(client, token, &local.vault_id)?, &key)?;
    let id = merged.vault.publication_id()?;
    if !after.contains_key(&id) {
        return Err(protocol_error(
            "Published snapshot is not visible; retry without changing local revision state",
        ));
    }
    let checked = merge(
        &merged.vault,
        &after,
        &after_heads,
        &record_pass,
        &key,
        None,
    )?;
    if !checked.conflicts.is_empty() {
        return Ok(SyncCycleResult::Conflicts(checked.conflicts));
    }
    tracing::info!(vault_id=%local.vault_id,"Published immutable authenticated cloud revision");
    Ok(report(
        &merged.vault,
        local_changes,
        remote_changes,
        merged.conflicts.len(),
        start.elapsed().as_millis() as u64,
    ))
}

impl<T: HttpTransport> GoogleDriveSyncClient<T> {
    /// Create-only publication with a provider-generated ID. Drive documents
    /// retrying a generated ID as 409, so neither retries nor races replace bytes.
    pub fn publish_immutable(&self, token: &str, vault: &SyncVault) -> Result<(), SyncError> {
        let response = self.transport.send(HttpRequest::get("https://www.googleapis.com/drive/v3/files/generateIds?count=1&space=appDataFolder&type=files")
            .with_header("Authorization",format!("Bearer {token}")))?;
        if response.status_code != 200 {
            return Err(SyncError::Http(
                response.status_code,
                "Could not allocate immutable Drive publication ID".into(),
            ));
        }
        #[derive(Deserialize)]
        struct Ids {
            ids: Vec<String>,
        }
        let ids: Ids = serde_json::from_slice(&response.body)
            .map_err(|e| SyncError::Deserialization(e.to_string()))?;
        let id = ids
            .ids
            .first()
            .filter(|id| {
                !id.is_empty()
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            })
            .ok_or_else(|| protocol_error("Invalid generated Drive publication ID"))?;
        let json = vault.to_json()?;
        let boundary = "SHELLUMI_IMMUTABLE_REVISION";
        let metadata = serde_json::json!({"id":id,"name":drive_vault_filename(&vault.vault_id),"parents":["appDataFolder"]});
        let body = format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/json\r\n\r\n{json}\r\n--{boundary}--\r\n").into_bytes();
        let request = HttpRequest::post(
            "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart",
            body,
        )
        .with_header("Authorization", format!("Bearer {token}"))
        .with_header(
            "Content-Type",
            format!("multipart/related; boundary={boundary}"),
        )
        .with_header("If-None-Match", "*");
        let sent = self.transport.send(request);
        if let Ok(response) = &sent {
            if ![200, 201, 409].contains(&response.status_code) {
                return Err(SyncError::Http(
                    response.status_code,
                    "Immutable Drive publication failed; retry safely".into(),
                ));
            }
        }
        // A missing response may mean the server committed. Verify the exact ID
        // and bytes before acknowledging anything locally.
        let stored = self.download_vault(token, id)?;
        if stored != *vault {
            return Err(protocol_error(
                "Immutable publication read-back failed; remote bytes were not replaced",
            ));
        }
        Ok(())
    }

    /// Read the current authenticated envelope/metadata, refusing concurrent forks.
    pub fn download_current_vault(
        &self,
        token: &str,
        vault_id: &str,
        password: &str,
    ) -> Result<SyncVault, SyncError> {
        let raw = load(self, token, vault_id)?;
        if raw.is_empty() {
            return Err(protocol_error("No cloud snapshot for this vault"));
        }
        let empty = SyncVault::new(vault_id, 0);
        let key = auth_key(&empty, &raw, password)?;
        let (mut snapshots, heads) = authenticate(raw, &key)?;
        for id in &heads {
            if snapshots[id].legacy {
                revisions::normalize_records(
                    &mut snapshots.get_mut(id).expect("known head").vault,
                    &key,
                    password,
                )?;
            }
        }
        let mut base = snapshots[&heads[0]].vault.clone();
        base.prepare_cloud_revision(&key)?;
        let merged = merge(
            &base,
            &snapshots,
            &heads,
            &BASE64_STANDARD.encode(key.expose_bytes()),
            &key,
            None,
        )?;
        if !merged.conflicts.is_empty() {
            return Err(protocol_error(
                "Concurrent cloud revisions require explicit resolution before restoring",
            ));
        }
        if heads.len() == 1 && !snapshots[&heads[0]].legacy {
            return Ok(snapshots[&heads[0]].vault.clone());
        }
        let mut combined = merged.vault;
        combined.sign_local_snapshot(&key, heads)?;
        Ok(combined)
    }
}

/// Reconcile portable snapshots with the same authenticated component protocol.
/// File snapshots are locally sealed; they do not invent accepted Drive parents.
pub fn execute_snapshot_sync(
    original: &SyncVault,
    remote: &SyncVault,
    pass: &str,
    choices: Option<&[ConflictResolutionChoice]>,
) -> Result<SyncCycleResult, SyncError> {
    if original.vault_id != remote.vault_id {
        return Err(protocol_error(
            "Portable snapshots belong to different vaults",
        ));
    }
    let key = auth_key(original, std::slice::from_ref(remote), pass)?;
    let mut local = original.clone();
    let mut other = remote.clone();
    for vault in [&mut local, &mut other] {
        if vault.cloud_state.is_some() {
            vault.validate_cloud_revision(&key, false)?;
        }
        vault.prepare_cloud_revision(&key)?;
        revisions::normalize_records(vault, &key, pass)?;
    }
    let id = remote.publication_id()?;
    let mut snapshots = BTreeMap::new();
    snapshots.insert(
        id.clone(),
        Remote {
            id: id.clone(),
            vault: other.clone(),
            parents: Vec::new(),
            legacy: remote.cloud_state.is_none(),
        },
    );
    let encoded = BASE64_STANDARD.encode(key.expose_bytes());
    let merged = merge(&local, &snapshots, &[id], &encoded, &key, choices)?;
    if !merged.conflicts.is_empty() && choices.is_none() {
        return Ok(SyncCycleResult::Conflicts(merged.conflicts));
    }
    let identical = |left: &SyncVault, right: &SyncVault| {
        component(left, true).head == component(right, true).head
            && component(left, false).head == component(right, false).head
            && diff_sync_records_with_passphrase(&left.records, &right.records, Some(&encoded))
                .iter()
                .all(|d| d.status == DiffStatus::Identical)
    };
    if identical(&local, &other) {
        return Ok(SyncCycleResult::AlreadySynchronized);
    }
    let mut combined = merged.vault;
    let parents = [&local, &other]
        .into_iter()
        .flat_map(|v| {
            v.cloud_state
                .as_ref()
                .expect("prepared snapshot")
                .base_publications
                .clone()
        })
        .collect();
    combined.sign_local_snapshot(&key, parents)?;
    tracing::info!(vault_id=%local.vault_id, "Reconciled authenticated portable snapshots");
    Ok(report(
        &combined,
        combined.records.len(),
        combined.records.len(),
        merged.conflicts.len(),
        0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_and_ancestor_components_across_record_forks_have_one_review_choice() {
        let key = crate::VaultKey::from_bytes([3; 32]);
        let mut base = SyncVault::new("vault", 1);
        base.prepare_cloud_revision(&key).unwrap();
        let mut local = base.clone();
        local.encrypted_directory = Some("local ciphertext".into());
        local.prepare_cloud_revision(&key).unwrap();
        let mut ancestor = base.clone();
        ancestor.encrypted_directory = Some("remote first ciphertext".into());
        ancestor.prepare_cloud_revision(&key).unwrap();
        let mut remote = ancestor.clone();
        remote.encrypted_directory = Some("remote latest ciphertext".into());
        remote.prepare_cloud_revision(&key).unwrap();
        for earlier in [remote.clone(), ancestor] {
            let mut snapshots = BTreeMap::new();
            for (id, vault) in [("one", earlier), ("two", remote.clone())] {
                snapshots.insert(
                    id.into(),
                    Remote {
                        id: id.into(),
                        vault,
                        parents: vec![],
                        legacy: false,
                    },
                );
            }
            let heads = vec!["one".into(), "two".into()];
            let pass = BASE64_STANDARD.encode(key.expose_bytes());
            assert_eq!(
                merge(&local, &snapshots, &heads, &pass, &key, None)
                    .unwrap()
                    .conflicts
                    .len(),
                1
            );
            let resolved = merge(
                &local,
                &snapshots,
                &heads,
                &pass,
                &key,
                Some(&[ConflictResolutionChoice::AcceptRemote]),
            )
            .unwrap();
            assert_eq!(
                resolved.vault.encrypted_directory,
                remote.encrypted_directory
            );
            assert!(component(&resolved.vault, false).contains(&component(&local, false).head));
        }
    }
}

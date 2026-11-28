//! Unified SFTP transfer manager supporting all four endpoint directions
//! (`Local→Local`, `Local→Remote`, `Remote→Local`, `Remote→Remote`),
//! bounded concurrency, destination reservations, pause/resume/cancel/retry, and vault isolation.

use super::endpoint::{AttemptId, BatchId, EndpointKind, JobId, TransferRoute, VaultScope};
use super::path::{CanonicalDestKey, PathSemantics, RemotePath};
use super::planner::{
    ensure_local_dir_no_follow, ensure_remote_dir_no_follow, plan_local_tree, plan_remote_tree,
    staged_local_copy_file_with_policy, validate_non_overlapping_local_copy,
    validate_non_overlapping_same_endpoint_remote, ConflictAction, ConflictContext,
    ConflictDecision, ConflictPolicy, DestinationReservationRegistry, PlanLimits, SymlinkPolicy,
};
use super::{
    ConflictDetails, ConflictReason, FileKind, JobCancellation, PublicationPolicy, SftpClient,
    SftpConnectOptions, SftpError, TransferControl,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Instant, UNIX_EPOCH};

/// Full lifecycle state machine for a unified transfer job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferJobState {
    Planning,
    AwaitingConflict,
    Queued,
    Connecting,
    Transferring,
    Pausing,
    Paused,
    Cancelling,
    Verifying,
    Committing,
    Completed,
    Skipped,
    Cancelled,
    Failed,
    OutcomeUnknown,
    CleanupPending,
}

impl TransferJobState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Skipped | Self::Cancelled | Self::Failed | Self::OutcomeUnknown
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Planning
                | Self::Connecting
                | Self::Transferring
                | Self::Pausing
                | Self::Cancelling
                | Self::Verifying
                | Self::Committing
        )
    }
}

/// Endpoint-specific path target for a transfer job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndpointPath {
    Local(PathBuf),
    Remote(RemotePath),
}

impl EndpointPath {
    pub fn display(&self) -> String {
        match self {
            Self::Local(p) => p.display().to_string(),
            Self::Remote(r) => r.display().to_string(),
        }
    }
}

/// Immutable intent snapshot captured when a transfer job is created.
#[derive(Clone, Debug)]
pub struct TransferIntent {
    pub job_id: JobId,
    pub batch_id: BatchId,
    pub vault_scope: VaultScope,
    pub route: TransferRoute,
    pub source_endpoint: EndpointKind,
    pub destination_endpoint: EndpointKind,
    pub source_path: EndpointPath,
    pub destination_path: EndpointPath,
    pub source_options: Option<SftpConnectOptions>,
    pub destination_options: Option<SftpConnectOptions>,
    pub conflict_policy: ConflictPolicy,
    pub symlink_policy: SymlinkPolicy,
    pub preserve_metadata: bool,
    pub delete_source_after_commit: bool,
    pub expected_bytes: Option<u64>,
}

impl TransferIntent {
    pub fn canonical_destination_key(&self) -> CanonicalDestKey {
        match (&self.destination_endpoint, &self.destination_path) {
            (EndpointKind::Local { .. }, EndpointPath::Local(p)) => CanonicalDestKey::for_local(p),
            (EndpointKind::Remote(r), EndpointPath::Remote(rp)) => {
                CanonicalDestKey::for_remote(&r.vault_scope.vault_id, r.host_id, rp)
            }
            _ => CanonicalDestKey {
                endpoint_key: "invalid".into(),
                normalized_path: self.destination_path.display(),
            },
        }
    }
}

/// Live progress and attempt metadata for a scheduled transfer job.
#[derive(Clone, Debug)]
pub struct TransferJobSnapshot {
    pub intent: TransferIntent,
    pub attempt_id: AttemptId,
    pub state: TransferJobState,
    pub bytes_transferred: u64,
    pub total_bytes: Option<u64>,
    pub speed_bps: f64,
    pub eta_seconds: Option<u64>,
    pub error: Option<String>,
    pub cleanup_warning: Option<String>,
    pub conflict: Option<ConflictContext>,
}

/// Bounded streaming relay between two authorized remote SFTP channels.
/// Uses a bounded 16-chunk (512 KiB) buffer with backpressure — never spools
/// the entire file to plaintext local disk.
pub fn relay_remote_to_remote(
    src_client: &mut SftpClient,
    src_path: &str,
    dst_client: &mut SftpClient,
    dst_path: &str,
    control: &TransferControl,
    on_progress: Option<&dyn Fn(u64, u64)>,
) -> Result<u64, SftpError> {
    relay_remote_to_remote_with_policy(
        src_client,
        src_path,
        dst_client,
        dst_path,
        control,
        &PublicationPolicy::ReplaceConfirmed {
            expected_len: None,
            expected_mtime: None,
        },
        on_progress,
    )
}

/// Bounded remote-to-remote streaming relay with explicit [`PublicationPolicy`].
pub fn relay_remote_to_remote_with_policy(
    src_client: &mut SftpClient,
    src_path: &str,
    dst_client: &mut SftpClient,
    dst_path: &str,
    control: &TransferControl,
    policy: &PublicationPolicy,
    on_progress: Option<&dyn Fn(u64, u64)>,
) -> Result<u64, SftpError> {
    control.check()?;
    src_client.controlled(control, |src_client| {
        dst_client.controlled(control, |dst_client| {
            relay_remote_to_remote_controlled(
                src_client,
                src_path,
                dst_client,
                dst_path,
                control,
                policy,
                on_progress,
            )
        })
    })
}

fn relay_remote_to_remote_controlled(
    src_client: &mut SftpClient,
    src_path: &str,
    dst_client: &mut SftpClient,
    dst_path: &str,
    control: &TransferControl,
    policy: &PublicationPolicy,
    on_progress: Option<&dyn Fn(u64, u64)>,
) -> Result<u64, SftpError> {
    use super::transfer;
    use super::wire::{
        FileAttributes, SftpPacket, StatusCode, SSH_FXF_CREAT, SSH_FXF_EXCL, SSH_FXF_READ,
        SSH_FXF_WRITE,
    };

    control.check()?;
    let initial_size = src_client.stat(src_path)?.size;

    // Open source for reading
    let open_src_id = src_client.next_id();
    src_client.send_packet(&SftpPacket::Open {
        id: open_src_id,
        filename: src_path.to_string(),
        pflags: SSH_FXF_READ,
        attrs: FileAttributes::new(),
    })?;
    let src_handle = src_client.expect_handle(open_src_id, src_path)?;

    // Open destination staging file exclusively
    let directory = dst_path
        .rsplit_once('/')
        .map(|(parent, _)| format!("{parent}/"))
        .unwrap_or_default();
    let staging = format!("{directory}.shellumi-relay-{}.part", transfer::suffix()?);
    let open_dst_id = dst_client.next_id();
    dst_client.send_packet(&SftpPacket::Open {
        id: open_dst_id,
        filename: staging.clone(),
        pflags: SSH_FXF_WRITE | SSH_FXF_CREAT | SSH_FXF_EXCL,
        attrs: FileAttributes::new().with_permissions(0o600),
    })?;
    let dst_handle = match dst_client.read_response_for(open_dst_id) {
        Ok(SftpPacket::Handle { handle, .. }) => handle,
        Ok(SftpPacket::Status { code, message, .. }) => {
            let _ = src_client.close_handle(src_handle);
            return Err(SftpError::Status { code, message });
        }
        Ok(other) => {
            let _ = src_client.close_handle(src_handle);
            return Err(SftpError::UnexpectedResponse(format!("{other:?}")));
        }
        Err(err) => {
            let _ = src_client.close_handle(src_handle);
            return Err(err);
        }
    };

    const CHUNK_SIZE: u32 = 32 * 1024;
    let mut offset: u64 = 0;

    let relay_res = (|| -> Result<u64, SftpError> {
        loop {
            control.check()?;
            let rid = src_client.next_id();
            src_client.send_packet(&SftpPacket::Read {
                id: rid,
                handle: src_handle.clone(),
                offset,
                len: CHUNK_SIZE,
            })?;
            match src_client.read_response_for(rid)? {
                SftpPacket::Data { data, .. } if !data.is_empty() => {
                    let len = data.len() as u64;
                    let wid = dst_client.next_id();
                    dst_client.send_packet(&SftpPacket::Write {
                        id: wid,
                        handle: dst_handle.clone(),
                        offset,
                        data,
                    })?;
                    dst_client.expect_status(wid, Some(&staging))?;
                    offset = offset.saturating_add(len);
                    if let Some(cb) = on_progress {
                        cb(offset, initial_size.unwrap_or(offset));
                    }
                }
                SftpPacket::Status {
                    code: StatusCode::Eof,
                    ..
                } => break,
                SftpPacket::Status { code, message, .. } => {
                    return Err(SftpError::Status { code, message });
                }
                other => return Err(SftpError::UnexpectedResponse(format!("{other:?}"))),
            }
        }
        if initial_size.is_some_and(|exp| exp != offset) {
            return Err(SftpError::UnexpectedResponse(
                "Source file size changed during remote-to-remote relay".into(),
            ));
        }
        Ok(offset)
    })();

    let _ = src_client.close_handle(src_handle);
    let fsync_res = if relay_res.is_ok() && dst_client.capabilities().fsync {
        dst_client.fsync_handle(&dst_handle)
    } else {
        Ok(())
    };
    let close_dst = dst_client.close_handle(dst_handle);

    let bytes = match relay_res.and_then(|b| {
        fsync_res?;
        close_dst?;
        Ok(b)
    }) {
        Ok(b) => b,
        Err(err) => {
            if !dst_client.is_corrupted() {
                if let Err(cleanup) = dst_client.remove_file(&staging) {
                    return Err(SftpError::CleanupFailed {
                        path: staging,
                        message: format!("{err}; relay staging cleanup failed: {cleanup}"),
                    });
                }
            }
            return Err(err);
        }
    };

    if let PublicationPolicy::ReplaceConfirmed {
        expected_len,
        expected_mtime,
    } = policy
    {
        if expected_len.is_some() || expected_mtime.is_some() {
            match dst_client.lstat(dst_path) {
                Ok(attrs) => {
                    if attrs.is_symlink() || !attrs.is_file() {
                        return Err(SftpError::ConflictRequired(Box::new(ConflictDetails {
                            source_path: src_path.to_string(),
                            destination_path: dst_path.to_string(),
                            relative_path: dst_path
                                .rsplit('/')
                                .next()
                                .unwrap_or_default()
                                .to_string(),
                            item_kind: if attrs.is_symlink() {
                                FileKind::Symlink { target: None }
                            } else {
                                FileKind::Directory
                            },
                            source_size: initial_size,
                            destination_size: attrs.size,
                            destination_mtime: attrs.mtime.map(u64::from),
                            reason: ConflictReason::TypeMismatch,
                        })));
                    }
                    let actual_mtime = attrs.mtime.map(u64::from);
                    if expected_len.is_some_and(|exp| Some(exp) != attrs.size)
                        || expected_mtime.is_some_and(|exp| Some(exp) != actual_mtime)
                    {
                        return Err(SftpError::ConflictRequired(Box::new(ConflictDetails {
                            source_path: src_path.to_string(),
                            destination_path: dst_path.to_string(),
                            relative_path: dst_path
                                .rsplit('/')
                                .next()
                                .unwrap_or_default()
                                .to_string(),
                            item_kind: FileKind::File,
                            source_size: initial_size,
                            destination_size: attrs.size,
                            destination_mtime: actual_mtime,
                            reason: ConflictReason::TargetChanged,
                        })));
                    }
                }
                Err(SftpError::FileNotFound(_)) => {
                    return Err(SftpError::ConflictRequired(Box::new(ConflictDetails {
                        source_path: src_path.to_string(),
                        destination_path: dst_path.to_string(),
                        relative_path: dst_path.rsplit('/').next().unwrap_or_default().to_string(),
                        item_kind: FileKind::File,
                        source_size: initial_size,
                        destination_size: None,
                        destination_mtime: None,
                        reason: ConflictReason::TargetDisappeared,
                    })));
                }
                Err(err) => return Err(err),
            }
        }
    }

    control.begin_commit()?;
    match dst_client.publish_upload_with_policy(&staging, dst_path, policy) {
        Ok(()) => Ok(bytes),
        Err(SftpError::Io(e)) => Err(SftpError::OutcomeUnknown {
            path: dst_path.to_string(),
            message: format!(
                "transport error awaiting relay commit reply (staging: {staging}): {e}"
            ),
        }),
        Err(err) => {
            if !dst_client.is_corrupted() {
                let _ = dst_client.remove_file(&staging);
            }
            Err(err)
        }
    }
}

/// Unified transfer scheduler enforcing bounded global concurrency, destination
/// reservations, attempt isolation, pause/resume/cancel/retry, and vault scoping.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ItemOutcome {
    CommittedLocal {
        source_path: PathBuf,
        source_len: u64,
        source_mtime: Option<u64>,
    },
    CommittedRemote {
        source_path: String,
        source_len: u64,
        source_mtime: Option<u64>,
    },
    Skipped,
}

#[derive(Clone, Debug, Default)]
struct JobExecutionCursor {
    outcomes: HashMap<String, ItemOutcome>,
    per_item_decisions: HashMap<String, ConflictPolicy>,
    committed_bytes: u64,
    had_skipped_or_uncommitted: bool,
    paused: bool,
    dir_order_local: Vec<PathBuf>,
    dir_order_remote: Vec<String>,
}

pub struct UnifiedTransferManager {
    max_concurrent: usize,
    reservations: DestinationReservationRegistry,
    jobs: Arc<Mutex<HashMap<JobId, TransferJobSnapshot>>>,
    controls: Arc<Mutex<HashMap<AttemptId, TransferControl>>>,
    job_cancellations: Arc<Mutex<HashMap<AttemptId, JobCancellation>>>,
    cursors: Arc<Mutex<HashMap<JobId, JobExecutionCursor>>>,
    queue: Arc<Mutex<VecDeque<JobId>>>,
    last_progress_emit: Arc<Mutex<HashMap<AttemptId, Instant>>>,
}

impl Default for UnifiedTransferManager {
    fn default() -> Self {
        Self::new(3)
    }
}

impl UnifiedTransferManager {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            max_concurrent: max_concurrent.clamp(1, 8),
            reservations: DestinationReservationRegistry::new(),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            controls: Arc::new(Mutex::new(HashMap::new())),
            job_cancellations: Arc::new(Mutex::new(HashMap::new())),
            cursors: Arc::new(Mutex::new(HashMap::new())),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            last_progress_emit: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Return (or create) the [`JobCancellation`] handle for `attempt_id`.
    pub fn job_cancellation_for(&self, attempt_id: AttemptId) -> JobCancellation {
        let mut guard = self
            .job_cancellations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.entry(attempt_id).or_default().clone()
    }

    pub fn reservations(&self) -> &DestinationReservationRegistry {
        &self.reservations
    }

    /// Enqueue a validated transfer intent. Rejects self/descendant copies on the same endpoint.
    pub fn enqueue(&self, intent: TransferIntent) -> Result<JobId, SftpError> {
        match (&intent.source_path, &intent.destination_path) {
            (EndpointPath::Local(s), EndpointPath::Local(d)) => {
                validate_non_overlapping_local_copy(s, d)?;
            }
            (EndpointPath::Remote(s), EndpointPath::Remote(d)) => {
                validate_non_overlapping_same_endpoint_remote(
                    &intent.source_endpoint,
                    &intent.destination_endpoint,
                    s,
                    d,
                )?;
            }
            _ => {}
        }

        let job_id = intent.job_id;
        let attempt_id = AttemptId::new();
        let snapshot = TransferJobSnapshot {
            total_bytes: intent.expected_bytes,
            intent,
            attempt_id,
            state: TransferJobState::Queued,
            bytes_transferred: 0,
            speed_bps: 0.0,
            eta_seconds: None,
            error: None,
            cleanup_warning: None,
            conflict: None,
        };

        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.insert(job_id, snapshot);
        }
        if let Ok(mut q) = self.queue.lock() {
            q.push_back(job_id);
        }
        Ok(job_id)
    }

    /// Pop up to `max_concurrent` queued jobs that can run concurrently.
    pub fn dispatch_ready(
        &self,
        active_vault: &VaultScope,
    ) -> Vec<(TransferIntent, AttemptId, TransferControl)> {
        let mut ready = Vec::new();
        let Ok(mut jobs) = self.jobs.lock() else {
            return ready;
        };
        let active_count = jobs.values().filter(|j| j.state.is_active()).count();
        let mut slots = self.max_concurrent.saturating_sub(active_count);
        if slots == 0 {
            return ready;
        }
        let Ok(mut q) = self.queue.lock() else {
            return ready;
        };
        let mut deferred = VecDeque::new();
        while slots > 0 {
            let Some(job_id) = q.pop_front() else {
                break;
            };
            let Some(job) = jobs.get_mut(&job_id) else {
                continue;
            };
            if job.state != TransferJobState::Queued {
                continue;
            }
            if !job.intent.vault_scope.is_authorized_under(active_vault) {
                job.state = TransferJobState::Cancelled;
                job.error = Some("Cancelled: vault scope changed or locked".into());
                continue;
            }
            let attempt_id = AttemptId::new();
            let job_cancel = self.job_cancellation_for(attempt_id);
            let control = job_cancel.new_item_control().unwrap_or_default();
            job.attempt_id = attempt_id;
            job.state = TransferJobState::Connecting;
            if let Ok(mut ctrls) = self.controls.lock() {
                ctrls.insert(attempt_id, control.clone());
            }
            ready.push((job.intent.clone(), attempt_id, control));
            slots -= 1;
        }
        while let Some(id) = deferred.pop_front() {
            q.push_back(id);
        }
        ready
    }

    /// Update progress for `attempt_id`, rejecting stale attempts and coalescing rapid updates.
    pub fn update_progress(
        &self,
        job_id: JobId,
        attempt_id: AttemptId,
        bytes: u64,
        total: Option<u64>,
        elapsed: std::time::Duration,
    ) {
        let now = Instant::now();
        if let Ok(mut emit) = self.last_progress_emit.lock() {
            if let Some(prev) = emit.get(&attempt_id) {
                if now.duration_since(*prev).as_millis() < 100 && total.is_none_or(|t| bytes < t) {
                    return;
                }
            }
            emit.insert(attempt_id, now);
        }
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(&job_id) {
                if job.attempt_id != attempt_id || job.state.is_terminal() {
                    return;
                }
                job.state = TransferJobState::Transferring;
                // Monotonic within an attempt
                if bytes >= job.bytes_transferred {
                    job.bytes_transferred = bytes;
                }
                if let Some(t) = total {
                    job.total_bytes = Some(t);
                }
                let secs = elapsed.as_secs_f64();
                if secs > 0.05 {
                    let speed = job.bytes_transferred as f64 / secs;
                    job.speed_bps = speed;
                    if let Some(t) = job.total_bytes {
                        if t > job.bytes_transferred && speed > 1.0 {
                            job.eta_seconds =
                                Some(((t - job.bytes_transferred) as f64 / speed) as u64);
                        } else {
                            job.eta_seconds = Some(0);
                        }
                    }
                }
            }
        }
    }

    /// Cancel a queued or active job; returns true if cancellation was accepted.
    pub fn cancel_job(&self, job_id: JobId) -> bool {
        let Ok(mut jobs) = self.jobs.lock() else {
            return false;
        };
        let Some(job) = jobs.get_mut(&job_id) else {
            return false;
        };
        if job.state.is_terminal() {
            return false;
        }
        if job.state == TransferJobState::Queued || job.state == TransferJobState::AwaitingConflict
        {
            job.state = TransferJobState::Cancelled;
            job.conflict = None;
            return true;
        }
        let attempt_id = job.attempt_id;
        let mut accepted = false;
        if let Ok(jcs) = self.job_cancellations.lock() {
            if let Some(jc) = jcs.get(&attempt_id) {
                accepted |= jc.cancel();
            }
        }
        if let Ok(ctrls) = self.controls.lock() {
            if let Some(ctrl) = ctrls.get(&attempt_id) {
                accepted |= ctrl.cancel();
            }
        }
        if accepted {
            job.state = TransferJobState::Cancelling;
        }
        accepted
    }

    /// Request cooperative pause at the next pre-publication item boundary.
    pub fn pause_job(&self, job_id: JobId) -> bool {
        let Ok(mut jobs) = self.jobs.lock() else {
            return false;
        };
        let Some(job) = jobs.get_mut(&job_id) else {
            return false;
        };
        if job.state.is_terminal() || job.state == TransferJobState::Paused {
            return false;
        }
        if let Ok(mut cursors) = self.cursors.lock() {
            cursors.entry(job_id).or_default().paused = true;
        }
        if job.state == TransferJobState::Queued || job.state == TransferJobState::AwaitingConflict
        {
            job.state = TransferJobState::Paused;
        } else {
            job.state = TransferJobState::Pausing;
        }
        true
    }

    /// Resume a paused job under `active_vault`, preserving already-committed items.
    pub fn resume_job(
        &self,
        job_id: JobId,
        active_vault: &VaultScope,
    ) -> Result<AttemptId, SftpError> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| SftpError::UnexpectedResponse("transfer manager lock poisoned".into()))?;
        let job = jobs
            .get_mut(&job_id)
            .ok_or_else(|| SftpError::FileNotFound(format!("transfer job {}", job_id.0)))?;
        if job.state != TransferJobState::Paused {
            return Err(SftpError::UnexpectedResponse("Job is not paused".into()));
        }
        if !job.intent.vault_scope.is_authorized_under(active_vault) {
            return Err(SftpError::PermissionDenied(
                "Vault scope changed before resume".into(),
            ));
        }
        if let Ok(mut cursors) = self.cursors.lock() {
            if let Some(cur) = cursors.get_mut(&job_id) {
                cur.paused = false;
            }
        }
        let new_attempt = AttemptId::new();
        job.attempt_id = new_attempt;
        job.state = TransferJobState::Queued;
        job.error = None;
        if let Ok(mut q) = self.queue.lock() {
            q.push_back(job_id);
        }
        Ok(new_attempt)
    }

    pub fn is_job_paused(&self, job_id: JobId) -> bool {
        self.cursors
            .lock()
            .ok()
            .is_some_and(|c| c.get(&job_id).is_some_and(|cur| cur.paused))
    }

    /// Remove terminal jobs belonging to `active_vault` without touching active/paused/recovery jobs.
    pub fn clear_terminal_history(&self, active_vault: &VaultScope) -> usize {
        let mut removed = 0usize;
        if let Ok(mut jobs) = self.jobs.lock() {
            let to_remove: Vec<JobId> = jobs
                .iter()
                .filter(|(_, j)| {
                    j.state.is_terminal() && j.intent.vault_scope.is_authorized_under(active_vault)
                })
                .map(|(id, _)| *id)
                .collect();
            for id in to_remove {
                jobs.remove(&id);
                if let Ok(mut cursors) = self.cursors.lock() {
                    cursors.remove(&id);
                }
                removed += 1;
            }
        }
        removed
    }

    /// Retry a failed or cancelled job with a fresh `AttemptId` and `TransferControl`.
    pub fn retry_job(
        &self,
        job_id: JobId,
        active_vault: &VaultScope,
    ) -> Result<AttemptId, SftpError> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| SftpError::UnexpectedResponse("transfer manager lock poisoned".into()))?;
        let job = jobs
            .get_mut(&job_id)
            .ok_or_else(|| SftpError::FileNotFound(format!("transfer job {}", job_id.0)))?;
        if !job.state.is_terminal() {
            return Err(SftpError::UnexpectedResponse(
                "Cannot retry a job that is still active".into(),
            ));
        }
        if !job.intent.vault_scope.is_authorized_under(active_vault) {
            return Err(SftpError::PermissionDenied(
                "Cannot retry transfer from an inactive or locked vault".into(),
            ));
        }
        let new_attempt = AttemptId::new();
        job.attempt_id = new_attempt;
        job.state = TransferJobState::Queued;
        job.bytes_transferred = 0;
        job.speed_bps = 0.0;
        job.eta_seconds = None;
        job.error = None;
        job.cleanup_warning = None;
        if let Ok(mut q) = self.queue.lock() {
            q.push_back(job_id);
        }
        Ok(new_attempt)
    }

    /// Record terminal completion for `attempt_id`, rejecting stale/duplicate completions.
    pub fn complete_attempt(
        &self,
        job_id: JobId,
        attempt_id: AttemptId,
        result: Result<u64, SftpError>,
    ) {
        if let Ok(mut ctrls) = self.controls.lock() {
            ctrls.remove(&attempt_id);
        }
        if let Ok(mut emit) = self.last_progress_emit.lock() {
            emit.remove(&attempt_id);
        }
        let Ok(mut jobs) = self.jobs.lock() else {
            return;
        };
        let Some(job) = jobs.get_mut(&job_id) else {
            return;
        };
        if job.attempt_id != attempt_id || job.state.is_terminal() {
            return;
        }
        match result {
            Ok(bytes) => {
                job.bytes_transferred = bytes;
                job.total_bytes = Some(bytes);
                job.state = TransferJobState::Completed;
                job.error = None;
            }
            Err(SftpError::Cancelled) => {
                job.state = TransferJobState::Cancelled;
            }
            Err(SftpError::Paused) => {
                job.state = TransferJobState::Paused;
            }
            Err(SftpError::OutcomeUnknown { path, message }) => {
                job.state = TransferJobState::OutcomeUnknown;
                job.error = Some(format!("Outcome unknown for {path}: {message}"));
            }
            Err(SftpError::CleanupFailed { path, message }) => {
                job.state = if job.state == TransferJobState::Cancelling {
                    TransferJobState::Cancelled
                } else {
                    TransferJobState::Failed
                };
                job.cleanup_warning = Some(format!("{message} (remnant: {path})"));
                job.error = Some(message);
            }
            Err(err) => {
                job.state = TransferJobState::Failed;
                job.error = Some(err.to_string());
            }
        }
    }

    /// Transition a job into `AwaitingConflict` with captured immutable conflict metadata.
    pub fn mark_awaiting_conflict(
        &self,
        job_id: JobId,
        attempt_id: AttemptId,
        conflict: ConflictContext,
    ) {
        if let Ok(mut ctrls) = self.controls.lock() {
            ctrls.remove(&attempt_id);
        }
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(&job_id) {
                if job.attempt_id == attempt_id && !job.state.is_terminal() {
                    job.state = TransferJobState::AwaitingConflict;
                    job.conflict = Some(conflict);
                }
            }
        }
    }

    /// Resolve a pending conflict decision for `job_id`, either cancelling/skipping
    /// or updating the job's policy and re-queuing it for execution.
    pub fn resolve_conflict(
        &self,
        job_id: JobId,
        decision: ConflictDecision,
        active_vault: &VaultScope,
    ) -> Result<(), SftpError> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| SftpError::UnexpectedResponse("transfer manager lock poisoned".into()))?;
        let job = jobs
            .get_mut(&job_id)
            .ok_or_else(|| SftpError::FileNotFound(format!("transfer job {}", job_id.0)))?;
        if job.state != TransferJobState::AwaitingConflict {
            return Err(SftpError::UnexpectedResponse(
                "Job is not awaiting conflict resolution".into(),
            ));
        }
        if !job.intent.vault_scope.is_authorized_under(active_vault) {
            job.state = TransferJobState::Cancelled;
            job.conflict = None;
            return Err(SftpError::PermissionDenied(
                "Vault scope changed before conflict resolution".into(),
            ));
        }

        let batch_id = job.intent.batch_id;
        let conflict_rel_key = job
            .conflict
            .as_ref()
            .and_then(|c| c.relative_path.clone())
            .unwrap_or_default();
        let new_policy = match decision.action {
            ConflictAction::Overwrite => ConflictPolicy::Overwrite,
            ConflictAction::Skip => ConflictPolicy::Skip,
            ConflictAction::KeepBoth => ConflictPolicy::KeepBoth,
            ConflictAction::Cancel => {
                job.state = TransferJobState::Cancelled;
                job.conflict = None;
                return Ok(());
            }
        };

        if decision.apply_to_all {
            job.intent.conflict_policy = new_policy;
        } else if !conflict_rel_key.is_empty() {
            if let Ok(mut cursors) = self.cursors.lock() {
                cursors
                    .entry(job_id)
                    .or_default()
                    .per_item_decisions
                    .insert(conflict_rel_key, new_policy);
            }
        }

        let is_root_only_skip = new_policy == ConflictPolicy::Skip
            && !decision.apply_to_all
            && self
                .cursors
                .lock()
                .ok()
                .and_then(|c| c.get(&job_id).map(|cur| cur.outcomes.is_empty()))
                .unwrap_or(true)
            && !matches!(
                job.conflict.as_ref().map(|c| c.source_kind.is_dir()),
                Some(true)
            );

        if is_root_only_skip
            && job.intent.source_path.display()
                == job
                    .conflict
                    .as_ref()
                    .map(|c| c.source_display.as_str())
                    .unwrap_or("")
        {
            job.state = TransferJobState::Skipped;
            job.conflict = None;
        } else {
            job.conflict = None;
            job.state = TransferJobState::Queued;
            if let Ok(mut q) = self.queue.lock() {
                q.push_back(job_id);
            }
        }

        if decision.apply_to_all {
            let mut requeue_ids = Vec::new();
            for other in jobs.values_mut() {
                if other.intent.batch_id == batch_id
                    && other.intent.job_id != job_id
                    && other.intent.conflict_policy == ConflictPolicy::Ask
                {
                    other.intent.conflict_policy = new_policy;
                    if other.state == TransferJobState::AwaitingConflict {
                        other.conflict = None;
                        other.state = TransferJobState::Queued;
                        requeue_ids.push(other.intent.job_id);
                    }
                }
            }
            if let Ok(mut q) = self.queue.lock() {
                for id in requeue_ids {
                    q.push_back(id);
                }
            }
        }
        Ok(())
    }

    /// Generate a unique sibling local path for `ConflictPolicy::KeepBoth`.
    fn keep_both_local_path(dst: &std::path::Path) -> std::path::PathBuf {
        let parent = dst.parent().unwrap_or_else(|| std::path::Path::new("."));
        let stem = dst.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = dst.extension().and_then(|e| e.to_str());
        for idx in 1..10_000 {
            let candidate_name = match ext {
                Some(e) => format!("{stem} ({idx}).{e}"),
                None => format!("{stem} ({idx})"),
            };
            let candidate = parent.join(candidate_name);
            if std::fs::symlink_metadata(&candidate).is_err() {
                return candidate;
            }
        }
        parent.join(format!("{stem}.{}", uuid::Uuid::new_v4().simple()))
    }

    /// Generate a unique sibling remote path for `ConflictPolicy::KeepBoth`.
    fn keep_both_remote_path(client: &mut SftpClient, dst: &RemotePath) -> RemotePath {
        let display = dst.display();
        let (dir, base) = match display.rsplit_once('/') {
            Some((d, b)) => (d, b),
            None => ("", display),
        };
        let (stem, ext) = match base.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() => (s, Some(e)),
            _ => (base, None),
        };
        for idx in 1..10_000 {
            let name = match ext {
                Some(e) => format!("{stem} ({idx}).{e}"),
                None => format!("{stem} ({idx})"),
            };
            let candidate = if dir.is_empty() {
                format!("/{name}")
            } else {
                format!("{dir}/{name}")
            };
            if matches!(client.lstat(&candidate), Err(SftpError::FileNotFound(_))) {
                return RemotePath::from_posix_str(&candidate);
            }
        }
        RemotePath::from_posix_str(&format!("{dir}/{stem}.{}", uuid::Uuid::new_v4().simple()))
    }

    fn effective_policy_for_item(
        &self,
        job_id: JobId,
        rel_key: &str,
        default_policy: ConflictPolicy,
    ) -> ConflictPolicy {
        self.cursors
            .lock()
            .ok()
            .and_then(|c| {
                c.get(&job_id)
                    .and_then(|cur| cur.per_item_decisions.get(rel_key).copied())
            })
            .unwrap_or(default_policy)
    }

    fn item_already_done(&self, job_id: JobId, rel_key: &str) -> bool {
        self.cursors.lock().ok().is_some_and(|c| {
            c.get(&job_id)
                .is_some_and(|cur| cur.outcomes.contains_key(rel_key))
        })
    }

    fn record_outcome(&self, job_id: JobId, rel_key: String, outcome: ItemOutcome, bytes: u64) {
        if let Ok(mut cursors) = self.cursors.lock() {
            let cur = cursors.entry(job_id).or_default();
            if matches!(outcome, ItemOutcome::Skipped) {
                cur.had_skipped_or_uncommitted = true;
            }
            cur.committed_bytes = cur.committed_bytes.saturating_add(bytes);
            cur.outcomes.insert(rel_key, outcome);
        }
    }

    fn decide_local_target(
        &self,
        job_id: JobId,
        rel_key: &str,
        default_policy: ConflictPolicy,
        src_display: &str,
        src_len: u64,
        target: &std::path::Path,
    ) -> Result<Option<(std::path::PathBuf, PublicationPolicy)>, SftpError> {
        let policy = self.effective_policy_for_item(job_id, rel_key, default_policy);
        match std::fs::symlink_metadata(target) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok(Some((target.to_path_buf(), PublicationPolicy::NoReplace)))
            }
            Err(err) => Err(err.into()),
            Ok(meta) => {
                if meta.file_type().is_symlink() || meta.file_type().is_dir() {
                    return Err(SftpError::ConflictRequired(Box::new(ConflictDetails {
                        source_path: src_display.to_string(),
                        destination_path: target.display().to_string(),
                        relative_path: rel_key.to_string(),
                        item_kind: if meta.file_type().is_symlink() {
                            FileKind::Symlink { target: None }
                        } else {
                            FileKind::Directory
                        },
                        source_size: Some(src_len),
                        destination_size: Some(meta.len()),
                        destination_mtime: meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map(|d| d.as_secs()),
                        reason: ConflictReason::TypeMismatch,
                    })));
                }
                match policy {
                    ConflictPolicy::Skip => Ok(None),
                    ConflictPolicy::Cancel => Err(SftpError::Cancelled),
                    ConflictPolicy::KeepBoth => Ok(Some((
                        Self::keep_both_local_path(target),
                        PublicationPolicy::NoReplace,
                    ))),
                    ConflictPolicy::Ask => {
                        Err(SftpError::ConflictRequired(Box::new(ConflictDetails {
                            source_path: src_display.to_string(),
                            destination_path: target.display().to_string(),
                            relative_path: rel_key.to_string(),
                            item_kind: FileKind::File,
                            source_size: Some(src_len),
                            destination_size: Some(meta.len()),
                            destination_mtime: meta
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                                .map(|d| d.as_secs()),
                            reason: ConflictReason::TargetExists,
                        })))
                    }
                    ConflictPolicy::Overwrite => Ok(Some((
                        target.to_path_buf(),
                        PublicationPolicy::ReplaceConfirmed {
                            expected_len: Some(meta.len()),
                            expected_mtime: meta
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                                .map(|d| d.as_secs()),
                        },
                    ))),
                }
            }
        }
    }

    fn decide_remote_target(
        &self,
        intent: &TransferIntent,
        rel_key: &str,
        src_display: &str,
        src_len: u64,
        client: &mut SftpClient,
        target: &RemotePath,
    ) -> Result<Option<(RemotePath, PublicationPolicy)>, SftpError> {
        let policy = self.effective_policy_for_item(intent.job_id, rel_key, intent.conflict_policy);
        match client.lstat(target.display()) {
            Err(SftpError::FileNotFound(_)) => {
                Ok(Some((target.clone(), PublicationPolicy::NoReplace)))
            }
            Err(err) => Err(err),
            Ok(attrs) => {
                if attrs.is_symlink() || attrs.is_dir() {
                    return Err(SftpError::ConflictRequired(Box::new(ConflictDetails {
                        source_path: src_display.to_string(),
                        destination_path: target.display().to_string(),
                        relative_path: rel_key.to_string(),
                        item_kind: if attrs.is_symlink() {
                            FileKind::Symlink { target: None }
                        } else {
                            FileKind::Directory
                        },
                        source_size: Some(src_len),
                        destination_size: attrs.size,
                        destination_mtime: attrs.mtime.map(u64::from),
                        reason: ConflictReason::TypeMismatch,
                    })));
                }
                match policy {
                    ConflictPolicy::Skip => Ok(None),
                    ConflictPolicy::Cancel => Err(SftpError::Cancelled),
                    ConflictPolicy::KeepBoth => Ok(Some((
                        Self::keep_both_remote_path(client, target),
                        PublicationPolicy::NoReplace,
                    ))),
                    ConflictPolicy::Ask => {
                        Err(SftpError::ConflictRequired(Box::new(ConflictDetails {
                            source_path: src_display.to_string(),
                            destination_path: target.display().to_string(),
                            relative_path: rel_key.to_string(),
                            item_kind: FileKind::File,
                            source_size: Some(src_len),
                            destination_size: attrs.size,
                            destination_mtime: attrs.mtime.map(u64::from),
                            reason: ConflictReason::TargetExists,
                        })))
                    }
                    ConflictPolicy::Overwrite => {
                        if !client.capabilities().posix_rename {
                            return Err(SftpError::ConflictRequired(Box::new(ConflictDetails {
                                source_path: src_display.to_string(),
                                destination_path: target.display().to_string(),
                                relative_path: rel_key.to_string(),
                                item_kind: FileKind::File,
                                source_size: Some(src_len),
                                destination_size: attrs.size,
                                destination_mtime: attrs.mtime.map(u64::from),
                                reason: ConflictReason::UnsupportedReplace,
                            })));
                        }
                        Ok(Some((
                            target.clone(),
                            PublicationPolicy::ReplaceConfirmed {
                                expected_len: attrs.size,
                                expected_mtime: attrs.mtime.map(u64::from),
                            },
                        )))
                    }
                }
            }
        }
    }

    fn delete_committed_local_sources(
        &self,
        job_id: JobId,
        control: &JobCancellation,
    ) -> Result<(), SftpError> {
        let cursor = self
            .cursors
            .lock()
            .ok()
            .and_then(|c| c.get(&job_id).cloned())
            .unwrap_or_default();
        for outcome in cursor.outcomes.values() {
            control.check()?;
            if let ItemOutcome::CommittedLocal {
                source_path,
                source_len,
                source_mtime,
            } = outcome
            {
                let meta = std::fs::symlink_metadata(source_path)?;
                if meta.file_type().is_symlink() || !meta.file_type().is_file() {
                    continue;
                }
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
                if meta.len() != *source_len || mtime != *source_mtime {
                    return Err(SftpError::UnexpectedResponse(format!(
                        "Source '{}' changed after copy; retained",
                        source_path.display()
                    )));
                }
                std::fs::remove_file(source_path)?;
            }
        }
        if !cursor.had_skipped_or_uncommitted {
            for dir in cursor.dir_order_local.iter().rev() {
                control.check()?;
                // Only remove if genuinely empty (unexpected new files cause remove_dir to fail safely).
                let _ = std::fs::remove_dir(dir);
            }
        }
        Ok(())
    }

    fn delete_committed_remote_sources(
        &self,
        job_id: JobId,
        client: &mut SftpClient,
        control: &JobCancellation,
    ) -> Result<(), SftpError> {
        let cursor = self
            .cursors
            .lock()
            .ok()
            .and_then(|c| c.get(&job_id).cloned())
            .unwrap_or_default();
        for outcome in cursor.outcomes.values() {
            control.check()?;
            if let ItemOutcome::CommittedRemote {
                source_path,
                source_len,
                source_mtime,
            } = outcome
            {
                let attrs = client.lstat(source_path)?;
                if attrs.is_symlink() || !attrs.is_file() {
                    continue;
                }
                if attrs.size != Some(*source_len) || attrs.mtime.map(u64::from) != *source_mtime {
                    return Err(SftpError::UnexpectedResponse(format!(
                        "Remote source '{source_path}' changed after copy; retained"
                    )));
                }
                client.remove_file(source_path)?;
            }
        }
        if !cursor.had_skipped_or_uncommitted {
            for dir in cursor.dir_order_remote.iter().rev() {
                control.check()?;
                let _ = client.rmdir(dir);
            }
        }
        Ok(())
    }

    /// Execute a `LocalToLocal` job (single file or recursive directory tree) with
    /// destination reservation, conflict policy, empty directory preservation, and safe move.
    pub fn execute_local_to_local(
        &self,
        intent: &TransferIntent,
        attempt_id: AttemptId,
        control: &TransferControl,
    ) -> Result<u64, SftpError> {
        let (EndpointPath::Local(src), EndpointPath::Local(dst)) =
            (&intent.source_path, &intent.destination_path)
        else {
            return Err(SftpError::UnexpectedResponse(
                "execute_local_to_local requires local endpoints".into(),
            ));
        };
        let _guard = self
            .reservations
            .reserve(intent.canonical_destination_key(), intent.job_id)?;

        let job_cancel = self.job_cancellation_for(attempt_id);
        if control.is_cancelled() {
            job_cancel.cancel();
        }
        let plan = plan_local_tree(
            src,
            PathSemantics::local_native(),
            intent.symlink_policy,
            PlanLimits::default(),
            control,
            None,
        )?;
        let total_bytes = plan.total_bytes;
        let mut copied_total = self
            .cursors
            .lock()
            .ok()
            .and_then(|c| c.get(&intent.job_id).map(|cur| cur.committed_bytes))
            .unwrap_or(0);
        let started = Instant::now();

        for item in &plan.items {
            if self.is_job_paused(intent.job_id) {
                return Err(SftpError::Paused);
            }
            job_cancel.check()?;
            let rel_key = item.relative_components.join("/");
            if self.item_already_done(intent.job_id, &rel_key) {
                continue;
            }
            let item_ctrl = job_cancel.new_item_control()?;
            match item.kind {
                FileKind::Directory => {
                    ensure_local_dir_no_follow(dst, &item.relative_components)?;
                    if let Ok(mut cursors) = self.cursors.lock() {
                        cursors
                            .entry(intent.job_id)
                            .or_default()
                            .dir_order_local
                            .push(item.source_path.clone());
                    }
                }
                FileKind::File => {
                    let target = if item.relative_components.is_empty() {
                        let parent = dst.parent().ok_or_else(|| {
                            SftpError::UnexpectedResponse("Destination has no parent".into())
                        })?;
                        ensure_local_dir_no_follow(parent, &[])?;
                        dst.clone()
                    } else {
                        let parent_rel =
                            &item.relative_components[..item.relative_components.len() - 1];
                        let parent_dir = ensure_local_dir_no_follow(dst, parent_rel)?;
                        let leaf = &item.relative_components[item.relative_components.len() - 1];
                        parent_dir.join(leaf)
                    };
                    let src_display = item.source_path.display().to_string();
                    let Some((effective_target, pub_policy)) = self.decide_local_target(
                        intent.job_id,
                        &rel_key,
                        intent.conflict_policy,
                        &src_display,
                        item.size,
                        &target,
                    )?
                    else {
                        self.record_outcome(intent.job_id, rel_key, ItemOutcome::Skipped, 0);
                        continue;
                    };

                    let src_meta = std::fs::symlink_metadata(&item.source_path)?;
                    let src_mtime = src_meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs());

                    let base_offset = copied_total;
                    let mut cb = |bytes: u64, _item_total: u64| {
                        self.update_progress(
                            intent.job_id,
                            attempt_id,
                            base_offset.saturating_add(bytes),
                            Some(total_bytes),
                            started.elapsed(),
                        );
                    };
                    let n = staged_local_copy_file_with_policy(
                        &item.source_path,
                        &effective_target,
                        &pub_policy,
                        &item_ctrl,
                        Some(&mut cb),
                    )?;
                    copied_total = copied_total.saturating_add(n);
                    self.record_outcome(
                        intent.job_id,
                        rel_key,
                        ItemOutcome::CommittedLocal {
                            source_path: item.source_path.clone(),
                            source_len: src_meta.len(),
                            source_mtime: src_mtime,
                        },
                        n,
                    );
                }
                _ => {
                    self.record_outcome(intent.job_id, rel_key, ItemOutcome::Skipped, 0);
                }
            }
        }

        if intent.delete_source_after_commit {
            self.delete_committed_local_sources(intent.job_id, &job_cancel)?;
        }
        Ok(copied_total)
    }

    /// Execute a `LocalToRemote` job (single file or recursive directory tree) via `SftpClient`.
    pub fn execute_local_to_remote(
        &self,
        intent: &TransferIntent,
        attempt_id: AttemptId,
        client: &mut SftpClient,
        control: &TransferControl,
    ) -> Result<u64, SftpError> {
        let (EndpointPath::Local(src), EndpointPath::Remote(dst)) =
            (&intent.source_path, &intent.destination_path)
        else {
            return Err(SftpError::UnexpectedResponse(
                "execute_local_to_remote requires local source and remote destination".into(),
            ));
        };
        let _guard = self
            .reservations
            .reserve(intent.canonical_destination_key(), intent.job_id)?;

        let job_cancel = self.job_cancellation_for(attempt_id);
        if control.is_cancelled() {
            job_cancel.cancel();
        }
        let plan = plan_local_tree(
            src,
            dst.semantics(),
            intent.symlink_policy,
            PlanLimits::default(),
            control,
            None,
        )?;
        let total_bytes = plan.total_bytes;
        let mut uploaded_total = self
            .cursors
            .lock()
            .ok()
            .and_then(|c| c.get(&intent.job_id).map(|cur| cur.committed_bytes))
            .unwrap_or(0);
        let started = Instant::now();

        for item in &plan.items {
            if self.is_job_paused(intent.job_id) {
                return Err(SftpError::Paused);
            }
            job_cancel.check()?;
            let rel_comps = &item.relative_components;
            let rel_key = rel_comps.join("/");
            if self.item_already_done(intent.job_id, &rel_key) {
                continue;
            }
            let item_ctrl = job_cancel.new_item_control()?;
            match item.kind {
                FileKind::Directory => {
                    ensure_remote_dir_no_follow(client, dst.display(), rel_comps, &item_ctrl)?;
                    if let Ok(mut cursors) = self.cursors.lock() {
                        cursors
                            .entry(intent.job_id)
                            .or_default()
                            .dir_order_local
                            .push(item.source_path.clone());
                    }
                }
                FileKind::File => {
                    let target = if rel_comps.is_empty() {
                        let parent_str = dst
                            .display()
                            .rsplit_once('/')
                            .map(|(p, _)| if p.is_empty() { "/" } else { p })
                            .unwrap_or("/");
                        ensure_remote_dir_no_follow(client, parent_str, &[], &item_ctrl)?;
                        dst.clone()
                    } else {
                        let parent_rel = &rel_comps[..rel_comps.len() - 1];
                        let parent_remote = ensure_remote_dir_no_follow(
                            client,
                            dst.display(),
                            parent_rel,
                            &item_ctrl,
                        )?;
                        let leaf = &rel_comps[rel_comps.len() - 1];
                        RemotePath::from_posix_str(&parent_remote)
                            .join_child_str(leaf)
                            .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?
                    };
                    let src_display = item.source_path.display().to_string();
                    let Some((effective_target, pub_policy)) = self.decide_remote_target(
                        intent,
                        &rel_key,
                        &src_display,
                        item.size,
                        client,
                        &target,
                    )?
                    else {
                        self.record_outcome(intent.job_id, rel_key, ItemOutcome::Skipped, 0);
                        continue;
                    };

                    let src_meta = std::fs::symlink_metadata(&item.source_path)?;
                    let src_mtime = src_meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs());

                    let base_offset = uploaded_total;
                    let mgr = self;
                    let job_id = intent.job_id;
                    let n = client.upload_file_with_policy(
                        &item.source_path,
                        effective_target.display(),
                        &item_ctrl,
                        &pub_policy,
                        Some(|bytes: u64, _item_total: u64| {
                            mgr.update_progress(
                                job_id,
                                attempt_id,
                                base_offset.saturating_add(bytes),
                                Some(total_bytes),
                                started.elapsed(),
                            );
                        }),
                    )?;
                    uploaded_total = uploaded_total.saturating_add(n);
                    self.record_outcome(
                        intent.job_id,
                        rel_key,
                        ItemOutcome::CommittedLocal {
                            source_path: item.source_path.clone(),
                            source_len: src_meta.len(),
                            source_mtime: src_mtime,
                        },
                        n,
                    );
                }
                _ => {
                    self.record_outcome(intent.job_id, rel_key, ItemOutcome::Skipped, 0);
                }
            }
        }

        if intent.delete_source_after_commit {
            self.delete_committed_local_sources(intent.job_id, &job_cancel)?;
        }
        Ok(uploaded_total)
    }

    /// Execute a `RemoteToLocal` job (single file or recursive directory tree) via `SftpClient`.
    pub fn execute_remote_to_local(
        &self,
        intent: &TransferIntent,
        attempt_id: AttemptId,
        client: &mut SftpClient,
        control: &TransferControl,
    ) -> Result<u64, SftpError> {
        let (EndpointPath::Remote(src), EndpointPath::Local(dst)) =
            (&intent.source_path, &intent.destination_path)
        else {
            return Err(SftpError::UnexpectedResponse(
                "execute_remote_to_local requires remote source and local destination".into(),
            ));
        };
        let _guard = self
            .reservations
            .reserve(intent.canonical_destination_key(), intent.job_id)?;

        let job_cancel = self.job_cancellation_for(attempt_id);
        if control.is_cancelled() {
            job_cancel.cancel();
        }
        let plan = plan_remote_tree(
            client,
            src,
            PathSemantics::local_native(),
            intent.symlink_policy,
            PlanLimits::default(),
            control,
        )?;
        let total_bytes = plan.total_bytes;
        let mut downloaded_total = self
            .cursors
            .lock()
            .ok()
            .and_then(|c| c.get(&intent.job_id).map(|cur| cur.committed_bytes))
            .unwrap_or(0);
        let started = Instant::now();

        for item in &plan.items {
            if self.is_job_paused(intent.job_id) {
                return Err(SftpError::Paused);
            }
            job_cancel.check()?;
            let rel_key = item.relative_components.join("/");
            if self.item_already_done(intent.job_id, &rel_key) {
                continue;
            }
            let item_ctrl = job_cancel.new_item_control()?;
            match item.kind {
                FileKind::Directory => {
                    ensure_local_dir_no_follow(dst, &item.relative_components)?;
                    if let Ok(mut cursors) = self.cursors.lock() {
                        cursors
                            .entry(intent.job_id)
                            .or_default()
                            .dir_order_remote
                            .push(item.source_path.display().to_string());
                    }
                }
                FileKind::File => {
                    let target = if item.relative_components.is_empty() {
                        let parent = dst.parent().ok_or_else(|| {
                            SftpError::UnexpectedResponse("Destination has no parent".into())
                        })?;
                        ensure_local_dir_no_follow(parent, &[])?;
                        dst.clone()
                    } else {
                        let parent_rel =
                            &item.relative_components[..item.relative_components.len() - 1];
                        let parent_dir = ensure_local_dir_no_follow(dst, parent_rel)?;
                        let leaf = &item.relative_components[item.relative_components.len() - 1];
                        parent_dir.join(leaf)
                    };
                    let src_display = item.source_path.display().to_string();
                    let Some((effective_target, pub_policy)) = self.decide_local_target(
                        intent.job_id,
                        &rel_key,
                        intent.conflict_policy,
                        &src_display,
                        item.size,
                        &target,
                    )?
                    else {
                        self.record_outcome(intent.job_id, rel_key, ItemOutcome::Skipped, 0);
                        continue;
                    };

                    let src_attrs = client.lstat(&src_display)?;
                    let src_len = src_attrs.size.unwrap_or(item.size);
                    let src_mtime = src_attrs.mtime.map(u64::from);

                    let base_offset = downloaded_total;
                    let mgr = self;
                    let job_id = intent.job_id;
                    let n = client.download_file_with_policy(
                        &src_display,
                        &effective_target,
                        &item_ctrl,
                        &pub_policy,
                        Some(|bytes: u64, _item_total: u64| {
                            mgr.update_progress(
                                job_id,
                                attempt_id,
                                base_offset.saturating_add(bytes),
                                Some(total_bytes),
                                started.elapsed(),
                            );
                        }),
                    )?;
                    downloaded_total = downloaded_total.saturating_add(n);
                    self.record_outcome(
                        intent.job_id,
                        rel_key,
                        ItemOutcome::CommittedRemote {
                            source_path: src_display,
                            source_len: src_len,
                            source_mtime: src_mtime,
                        },
                        n,
                    );
                }
                _ => {
                    self.record_outcome(intent.job_id, rel_key, ItemOutcome::Skipped, 0);
                }
            }
        }

        if intent.delete_source_after_commit {
            self.delete_committed_remote_sources(intent.job_id, client, &job_cancel)?;
        }
        Ok(downloaded_total)
    }

    /// Execute a `RemoteToRemote` job (single file or recursive directory tree) via bounded relay.
    pub fn execute_remote_to_remote(
        &self,
        intent: &TransferIntent,
        attempt_id: AttemptId,
        src_client: &mut SftpClient,
        dst_client: &mut SftpClient,
        control: &TransferControl,
    ) -> Result<u64, SftpError> {
        let (EndpointPath::Remote(src), EndpointPath::Remote(dst)) =
            (&intent.source_path, &intent.destination_path)
        else {
            return Err(SftpError::UnexpectedResponse(
                "execute_remote_to_remote requires remote source and remote destination".into(),
            ));
        };
        let _guard = self
            .reservations
            .reserve(intent.canonical_destination_key(), intent.job_id)?;

        let job_cancel = self.job_cancellation_for(attempt_id);
        if control.is_cancelled() {
            job_cancel.cancel();
        }
        let plan = plan_remote_tree(
            src_client,
            src,
            dst.semantics(),
            intent.symlink_policy,
            PlanLimits::default(),
            control,
        )?;
        let total_bytes = plan.total_bytes;
        let mut relayed_total = self
            .cursors
            .lock()
            .ok()
            .and_then(|c| c.get(&intent.job_id).map(|cur| cur.committed_bytes))
            .unwrap_or(0);
        let started = Instant::now();

        for item in &plan.items {
            if self.is_job_paused(intent.job_id) {
                return Err(SftpError::Paused);
            }
            job_cancel.check()?;
            let rel_comps = &item.relative_components;
            let rel_key = rel_comps.join("/");
            if self.item_already_done(intent.job_id, &rel_key) {
                continue;
            }
            let item_ctrl = job_cancel.new_item_control()?;
            match item.kind {
                FileKind::Directory => {
                    ensure_remote_dir_no_follow(dst_client, dst.display(), rel_comps, &item_ctrl)?;
                    if let Ok(mut cursors) = self.cursors.lock() {
                        cursors
                            .entry(intent.job_id)
                            .or_default()
                            .dir_order_remote
                            .push(item.source_path.display().to_string());
                    }
                }
                FileKind::File => {
                    let target = if rel_comps.is_empty() {
                        let parent_str = dst
                            .display()
                            .rsplit_once('/')
                            .map(|(p, _)| if p.is_empty() { "/" } else { p })
                            .unwrap_or("/");
                        ensure_remote_dir_no_follow(dst_client, parent_str, &[], &item_ctrl)?;
                        dst.clone()
                    } else {
                        let parent_rel = &rel_comps[..rel_comps.len() - 1];
                        let parent_remote = ensure_remote_dir_no_follow(
                            dst_client,
                            dst.display(),
                            parent_rel,
                            &item_ctrl,
                        )?;
                        let leaf = &rel_comps[rel_comps.len() - 1];
                        RemotePath::from_posix_str(&parent_remote)
                            .join_child_str(leaf)
                            .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?
                    };
                    let src_display = item.source_path.display().to_string();
                    let Some((effective_target, pub_policy)) = self.decide_remote_target(
                        intent,
                        &rel_key,
                        &src_display,
                        item.size,
                        dst_client,
                        &target,
                    )?
                    else {
                        self.record_outcome(intent.job_id, rel_key, ItemOutcome::Skipped, 0);
                        continue;
                    };

                    let src_attrs = src_client.lstat(&src_display)?;
                    let src_len = src_attrs.size.unwrap_or(item.size);
                    let src_mtime = src_attrs.mtime.map(u64::from);

                    let base_offset = relayed_total;
                    let mgr = self;
                    let job_id = intent.job_id;
                    let cb = |bytes: u64, _item_total: u64| {
                        mgr.update_progress(
                            job_id,
                            attempt_id,
                            base_offset.saturating_add(bytes),
                            Some(total_bytes),
                            started.elapsed(),
                        );
                    };
                    let n = relay_remote_to_remote_with_policy(
                        src_client,
                        &src_display,
                        dst_client,
                        effective_target.display(),
                        &item_ctrl,
                        &pub_policy,
                        Some(&cb),
                    )?;
                    relayed_total = relayed_total.saturating_add(n);
                    self.record_outcome(
                        intent.job_id,
                        rel_key,
                        ItemOutcome::CommittedRemote {
                            source_path: src_display,
                            source_len: src_len,
                            source_mtime: src_mtime,
                        },
                        n,
                    );
                }
                _ => {
                    self.record_outcome(intent.job_id, rel_key, ItemOutcome::Skipped, 0);
                }
            }
        }

        if intent.delete_source_after_commit {
            self.delete_committed_remote_sources(intent.job_id, src_client, &job_cancel)?;
        }
        Ok(relayed_total)
    }

    /// Cancel all jobs belonging to vaults that are no longer authorized.
    pub fn cancel_unauthorized_vaults(&self, active_vault: Option<&VaultScope>) {
        let Ok(mut jobs) = self.jobs.lock() else {
            return;
        };
        for job in jobs.values_mut() {
            if job.state.is_terminal() {
                continue;
            }
            let ok = active_vault.is_some_and(|v| job.intent.vault_scope.is_authorized_under(v));
            if !ok {
                if let Ok(jcs) = self.job_cancellations.lock() {
                    if let Some(jc) = jcs.get(&job.attempt_id) {
                        jc.cancel();
                    }
                }
                if let Ok(ctrls) = self.controls.lock() {
                    if let Some(ctrl) = ctrls.get(&job.attempt_id) {
                        ctrl.cancel();
                    }
                }
                if job.state == TransferJobState::Queued
                    || job.state == TransferJobState::AwaitingConflict
                {
                    job.state = TransferJobState::Cancelled;
                    job.conflict = None;
                } else {
                    job.state = TransferJobState::Cancelling;
                }
                job.error = Some("Cancelled due to vault lock or switch".into());
            }
        }
    }

    pub fn snapshot(&self, job_id: JobId) -> Option<TransferJobSnapshot> {
        self.jobs.lock().ok()?.get(&job_id).cloned()
    }

    pub fn all_snapshots(&self) -> Vec<TransferJobSnapshot> {
        self.jobs
            .lock()
            .map(|g| g.values().cloned().collect())
            .unwrap_or_default()
    }
}

//! Unified SFTP transfer manager supporting all four endpoint directions
//! (`Local→Local`, `Local→Remote`, `Remote→Local`, `Remote→Remote`),
//! bounded concurrency, destination reservations, pause/resume/cancel/retry, and vault isolation.

use super::endpoint::{AttemptId, BatchId, EndpointKind, JobId, TransferRoute, VaultScope};
use super::path::{CanonicalDestKey, PathSemantics, RemotePath};
use super::planner::{
    delete_local_recursive, delete_remote_recursive, plan_local_tree, plan_remote_tree,
    staged_local_copy_file, validate_non_overlapping_local_copy,
    validate_non_overlapping_same_endpoint_remote, ConflictAction, ConflictContext,
    ConflictDecision, ConflictPolicy, DestinationReservationRegistry, PlanLimits, SymlinkPolicy,
};
use super::{FileKind, SftpClient, SftpConnectOptions, SftpError, TransferControl};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

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
    control.check()?;
    src_client.controlled(control, |src_client| {
        dst_client.controlled(control, |dst_client| {
            relay_remote_to_remote_controlled(
                src_client,
                src_path,
                dst_client,
                dst_path,
                control,
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

    control.begin_commit()?;
    match dst_client.publish_upload(&staging, dst_path) {
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
pub struct UnifiedTransferManager {
    max_concurrent: usize,
    reservations: DestinationReservationRegistry,
    jobs: Arc<Mutex<HashMap<JobId, TransferJobSnapshot>>>,
    controls: Arc<Mutex<HashMap<AttemptId, TransferControl>>>,
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
            queue: Arc::new(Mutex::new(VecDeque::new())),
            last_progress_emit: Arc::new(Mutex::new(HashMap::new())),
        }
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
            let control = TransferControl::default();
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
            return true;
        }
        let attempt_id = job.attempt_id;
        if let Ok(ctrls) = self.controls.lock() {
            if let Some(ctrl) = ctrls.get(&attempt_id) {
                if ctrl.cancel() {
                    job.state = TransferJobState::Cancelling;
                    return true;
                }
            }
        }
        false
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
        let new_policy = match decision.action {
            ConflictAction::Overwrite => ConflictPolicy::Overwrite,
            ConflictAction::Skip => {
                job.state = TransferJobState::Skipped;
                job.conflict = None;
                return Ok(());
            }
            ConflictAction::KeepBoth => ConflictPolicy::KeepBoth,
            ConflictAction::Cancel => {
                job.state = TransferJobState::Cancelled;
                job.conflict = None;
                return Ok(());
            }
        };

        job.intent.conflict_policy = new_policy;
        job.conflict = None;
        job.state = TransferJobState::Queued;
        if let Ok(mut q) = self.queue.lock() {
            q.push_back(job_id);
        }

        if decision.apply_to_all {
            for other in jobs.values_mut() {
                if other.intent.batch_id == batch_id
                    && other.intent.job_id != job_id
                    && other.intent.conflict_policy == ConflictPolicy::Ask
                {
                    other.intent.conflict_policy = new_policy;
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

        let plan = plan_local_tree(
            src,
            PathSemantics::local_native(),
            intent.symlink_policy,
            PlanLimits::default(),
            control,
            None,
        )?;
        let total_bytes = plan.total_bytes;
        let mut copied_total = 0u64;
        let started = Instant::now();

        for item in &plan.items {
            control.check()?;
            let mut target = dst.clone();
            for comp in &item.relative_components {
                target = target.join(comp);
            }
            match item.kind {
                FileKind::Directory => {
                    std::fs::create_dir_all(&target)?;
                }
                FileKind::File => {
                    let exists = std::fs::symlink_metadata(&target).is_ok();
                    if exists {
                        match intent.conflict_policy {
                            ConflictPolicy::Skip => continue,
                            ConflictPolicy::Cancel => return Err(SftpError::Cancelled),
                            ConflictPolicy::KeepBoth => {
                                target = Self::keep_both_local_path(&target);
                            }
                            ConflictPolicy::Ask => {
                                return Err(SftpError::UnexpectedResponse(format!(
                                    "Destination '{}' already exists (conflict decision required)",
                                    target.display()
                                )));
                            }
                            ConflictPolicy::Overwrite => {}
                        }
                    }
                    let overwrite = matches!(intent.conflict_policy, ConflictPolicy::Overwrite);
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
                    let n = staged_local_copy_file(
                        &item.source_path,
                        &target,
                        overwrite,
                        control,
                        Some(&mut cb),
                    )?;
                    copied_total = copied_total.saturating_add(n);
                }
                _ => {}
            }
        }

        if intent.delete_source_after_commit {
            delete_local_recursive(src, control)?;
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

        let plan = plan_local_tree(
            src,
            dst.semantics(),
            intent.symlink_policy,
            PlanLimits::default(),
            control,
            None,
        )?;
        let total_bytes = plan.total_bytes;
        let mut uploaded_total = 0u64;
        let started = Instant::now();

        for item in &plan.items {
            control.check()?;
            let mut target = dst.clone();
            for comp in &item.relative_components {
                target = target
                    .join_child_str(comp)
                    .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?;
            }
            match item.kind {
                FileKind::Directory => match client.lstat(target.display()) {
                    Ok(_) => {}
                    Err(SftpError::FileNotFound(_)) => client.mkdir(target.display())?,
                    Err(err) => return Err(err),
                },
                FileKind::File => {
                    let exists = match client.lstat(target.display()) {
                        Ok(_) => true,
                        Err(SftpError::FileNotFound(_)) => false,
                        Err(err) => return Err(err),
                    };
                    if exists {
                        match intent.conflict_policy {
                            ConflictPolicy::Skip => continue,
                            ConflictPolicy::Cancel => return Err(SftpError::Cancelled),
                            ConflictPolicy::KeepBoth => {
                                target = Self::keep_both_remote_path(client, &target);
                            }
                            ConflictPolicy::Ask => {
                                return Err(SftpError::UnexpectedResponse(format!(
                                    "Remote destination '{}' already exists (conflict decision required)",
                                    target.display()
                                )));
                            }
                            ConflictPolicy::Overwrite => {
                                if !client.capabilities().posix_rename {
                                    return Err(SftpError::UnexpectedResponse(format!(
                                        "Server does not support atomic overwrite (posix-rename@openssh.com) for '{}'; choose Keep Both or Skip.",
                                        target.display()
                                    )));
                                }
                            }
                        }
                    }
                    let base_offset = uploaded_total;
                    let mgr = self;
                    let job_id = intent.job_id;
                    let n = client.upload_file_with_control(
                        &item.source_path,
                        target.display(),
                        control,
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
                }
                _ => {}
            }
        }

        if intent.delete_source_after_commit {
            delete_local_recursive(src, control)?;
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

        let plan = plan_remote_tree(
            client,
            src,
            PathSemantics::local_native(),
            intent.symlink_policy,
            PlanLimits::default(),
            control,
        )?;
        let total_bytes = plan.total_bytes;
        let mut downloaded_total = 0u64;
        let started = Instant::now();

        for item in &plan.items {
            control.check()?;
            let mut target = dst.clone();
            for comp in &item.relative_components {
                target = target.join(comp);
            }
            match item.kind {
                FileKind::Directory => {
                    std::fs::create_dir_all(&target)?;
                }
                FileKind::File => {
                    let exists = std::fs::symlink_metadata(&target).is_ok();
                    if exists {
                        match intent.conflict_policy {
                            ConflictPolicy::Skip => continue,
                            ConflictPolicy::Cancel => return Err(SftpError::Cancelled),
                            ConflictPolicy::KeepBoth => {
                                target = Self::keep_both_local_path(&target);
                            }
                            ConflictPolicy::Ask => {
                                return Err(SftpError::UnexpectedResponse(format!(
                                    "Destination '{}' already exists (conflict decision required)",
                                    target.display()
                                )));
                            }
                            ConflictPolicy::Overwrite => {}
                        }
                    }
                    let base_offset = downloaded_total;
                    let mgr = self;
                    let job_id = intent.job_id;
                    let n = client.download_file_with_control(
                        item.source_path.display(),
                        &target,
                        control,
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
                }
                _ => {}
            }
        }

        if intent.delete_source_after_commit {
            delete_remote_recursive(client, src, control)?;
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

        let plan = plan_remote_tree(
            src_client,
            src,
            dst.semantics(),
            intent.symlink_policy,
            PlanLimits::default(),
            control,
        )?;
        let total_bytes = plan.total_bytes;
        let mut relayed_total = 0u64;
        let started = Instant::now();

        for item in &plan.items {
            control.check()?;
            let mut target = dst.clone();
            for comp in &item.relative_components {
                target = target
                    .join_child_str(comp)
                    .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?;
            }
            match item.kind {
                FileKind::Directory => match dst_client.lstat(target.display()) {
                    Ok(_) => {}
                    Err(SftpError::FileNotFound(_)) => dst_client.mkdir(target.display())?,
                    Err(err) => return Err(err),
                },
                FileKind::File => {
                    let exists = match dst_client.lstat(target.display()) {
                        Ok(_) => true,
                        Err(SftpError::FileNotFound(_)) => false,
                        Err(err) => return Err(err),
                    };
                    if exists {
                        match intent.conflict_policy {
                            ConflictPolicy::Skip => continue,
                            ConflictPolicy::Cancel => return Err(SftpError::Cancelled),
                            ConflictPolicy::KeepBoth => {
                                target = Self::keep_both_remote_path(dst_client, &target);
                            }
                            ConflictPolicy::Ask => {
                                return Err(SftpError::UnexpectedResponse(format!(
                                    "Remote destination '{}' already exists (conflict decision required)",
                                    target.display()
                                )));
                            }
                            ConflictPolicy::Overwrite => {
                                if !dst_client.capabilities().posix_rename {
                                    return Err(SftpError::UnexpectedResponse(format!(
                                        "Destination server does not support atomic overwrite for '{}'",
                                        target.display()
                                    )));
                                }
                            }
                        }
                    }
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
                    let n = relay_remote_to_remote(
                        src_client,
                        item.source_path.display(),
                        dst_client,
                        target.display(),
                        control,
                        Some(&cb),
                    )?;
                    relayed_total = relayed_total.saturating_add(n);
                }
                _ => {}
            }
        }

        if intent.delete_source_after_commit {
            delete_remote_recursive(src_client, src, control)?;
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
                if let Ok(ctrls) = self.controls.lock() {
                    if let Some(ctrl) = ctrls.get(&job.attempt_id) {
                        ctrl.cancel();
                    }
                }
                if job.state == TransferJobState::Queued {
                    job.state = TransferJobState::Cancelled;
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

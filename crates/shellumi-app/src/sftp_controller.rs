//! Unified SFTP controller & authorization boundary for `shellumi-app`.
//! Every graphical SFTP action (connect, list, transfer, mutate, editor, search, sync)
//! registers an operation with `VaultScope` and `VaultAuthGate`, preventing stale
//! callbacks or cross-vault leakage.

use shellumi_core::{
    EndpointId, EndpointKind, JobId, OperationId, PersistentSftpSession, SftpClient,
    SftpConnectOptions, SftpConnectionManager, SftpError, TransferControl, TransferIntent,
    UnifiedTransferManager, VaultAuthGate, VaultScope,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct SftpController {
    pub auth_gate: VaultAuthGate,
    pub connections: Arc<SftpConnectionManager>,
    pub transfers: Arc<UnifiedTransferManager>,
    active_ops: Arc<Mutex<HashMap<OperationId, (VaultScope, TransferControl)>>>,
}

impl SftpController {
    pub fn new(initial_scope: VaultScope) -> Self {
        Self {
            auth_gate: VaultAuthGate::new(Some(initial_scope)),
            connections: Arc::new(SftpConnectionManager::new()),
            transfers: Arc::new(UnifiedTransferManager::new(3)),
            active_ops: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a cancellable, vault-scoped operation. Fails closed if `scope` is
    /// no longer authorized under `auth_gate`.
    pub fn register_op(
        &self,
        scope: &VaultScope,
    ) -> Result<(OperationId, TransferControl), SftpError> {
        if !self.auth_gate.is_authorized(scope) {
            return Err(SftpError::PermissionDenied(
                "Vault scope is locked or no longer active".into(),
            ));
        }
        let op_id = OperationId::new();
        let ctrl = TransferControl::default();
        if let Ok(mut guard) = self.active_ops.lock() {
            // Re-check inside lock to prevent revocation racing registration.
            if !self.auth_gate.is_authorized(scope) {
                return Err(SftpError::PermissionDenied(
                    "Vault scope revoked during registration".into(),
                ));
            }
            guard.insert(op_id, (scope.clone(), ctrl.clone()));
        }
        Ok((op_id, ctrl))
    }

    pub fn complete_op(&self, op_id: OperationId) {
        if let Ok(mut guard) = self.active_ops.lock() {
            guard.remove(&op_id);
        }
    }

    pub fn is_op_authorized(&self, op_id: OperationId, scope: &VaultScope) -> bool {
        if !self.auth_gate.is_authorized(scope) {
            return false;
        }
        self.active_ops
            .lock()
            .ok()
            .and_then(|g| g.get(&op_id).map(|(s, c)| s == scope && !c.is_cancelled()))
            .unwrap_or(false)
    }

    /// Atomically update the active vault scope, cancelling all unauthorized operations,
    /// closing unauthorized persistent SFTP sessions, and cancelling unauthorized transfers.
    pub fn revoke_unauthorized(&self, active_scope: Option<&VaultScope>) {
        self.auth_gate.set_active(active_scope.cloned());
        if let Ok(mut guard) = self.active_ops.lock() {
            let mut remove_ids = Vec::new();
            for (op_id, (scope, ctrl)) in guard.iter() {
                let ok = active_scope.is_some_and(|active| scope.is_authorized_under(active));
                if !ok {
                    ctrl.cancel();
                    remove_ids.push(*op_id);
                }
            }
            for id in remove_ids {
                guard.remove(&id);
            }
        }
        self.connections.enforce_vault_scope(active_scope);
        self.transfers.cancel_unauthorized_vaults(active_scope);
    }

    /// Open a worker `SftpClient` from an existing registered `PersistentSftpSession`
    /// (preferred) or fall back to an authenticated connection if the session was not
    /// yet registered. Never bypasses `VaultAuthGate`.
    pub fn open_worker_client(
        &self,
        scope: &VaultScope,
        endpoint: &EndpointKind,
        fallback_opts: Option<&SftpConnectOptions>,
        control: &TransferControl,
    ) -> Result<SftpClient, SftpError> {
        if !self.auth_gate.is_authorized(scope) {
            return Err(SftpError::PermissionDenied(
                "Vault scope is no longer authorized".into(),
            ));
        }
        control.check()?;
        let EndpointKind::Remote(remote) = endpoint else {
            return Err(SftpError::UnexpectedResponse(
                "open_worker_client requires a remote endpoint".into(),
            ));
        };
        if let Some(sess_arc) = self.connections.get_session(remote.endpoint_id) {
            // Clone options/session without holding the session mutex during SSH connect!
            let sess_clone_res = sess_arc.lock().map(|s| s.open_worker_client(control));
            if let Ok(res) = sess_clone_res {
                return res;
            }
        }
        let opts = fallback_opts.cloned().unwrap_or_default();
        super::connect_saved_sftp(
            remote.database_path.clone(),
            Some(&scope.vault_id),
            None,
            remote.host_id,
            &remote.destination,
            &opts,
        )
    }

    /// Execute a single dispatched `TransferIntent` across ANY of the four routes
    /// (`LocalToLocal`, `LocalToRemote`, `RemoteToLocal`, `RemoteToRemote`) using
    /// `UnifiedTransferManager` and `SftpConnectionManager` worker channels.
    pub fn execute_dispatched_job(
        &self,
        intent: &TransferIntent,
        attempt_id: shellumi_core::AttemptId,
        control: &TransferControl,
    ) -> Result<u64, SftpError> {
        if !self.auth_gate.is_authorized(&intent.vault_scope) {
            return Err(SftpError::Cancelled);
        }
        control.check()?;

        match (&intent.source_endpoint, &intent.destination_endpoint) {
            (EndpointKind::Local { .. }, EndpointKind::Local { .. }) => self
                .transfers
                .execute_local_to_local(intent, attempt_id, control),
            (EndpointKind::Local { .. }, EndpointKind::Remote(_)) => {
                let mut dst_client = self.open_worker_client(
                    &intent.vault_scope,
                    &intent.destination_endpoint,
                    intent.destination_options.as_ref(),
                    control,
                )?;
                self.transfers
                    .execute_local_to_remote(intent, attempt_id, &mut dst_client, control)
            }
            (EndpointKind::Remote(_), EndpointKind::Local { .. }) => {
                let mut src_client = self.open_worker_client(
                    &intent.vault_scope,
                    &intent.source_endpoint,
                    intent.source_options.as_ref(),
                    control,
                )?;
                self.transfers
                    .execute_remote_to_local(intent, attempt_id, &mut src_client, control)
            }
            (EndpointKind::Remote(_), EndpointKind::Remote(_)) => {
                let mut src_client = self.open_worker_client(
                    &intent.vault_scope,
                    &intent.source_endpoint,
                    intent.source_options.as_ref(),
                    control,
                )?;
                let mut dst_client = self.open_worker_client(
                    &intent.vault_scope,
                    &intent.destination_endpoint,
                    intent.destination_options.as_ref(),
                    control,
                )?;
                self.transfers.execute_remote_to_remote(
                    intent,
                    attempt_id,
                    &mut src_client,
                    &mut dst_client,
                    control,
                )
            }
        }
    }

    pub fn get_session(&self, id: EndpointId) -> Option<Arc<Mutex<PersistentSftpSession>>> {
        self.connections.get_session(id)
    }

    /// Return only transfer snapshots belonging to the currently authorized vault scope
    /// (preventing cross-vault history/path leakage in the Transfer Drawer).
    pub fn authorized_snapshots(&self) -> Vec<shellumi_core::TransferJobSnapshot> {
        let Some(active) = self.auth_gate.current() else {
            return Vec::new();
        };
        self.transfers
            .all_snapshots()
            .into_iter()
            .filter(|s| s.intent.vault_scope.is_authorized_under(&active))
            .collect()
    }

    /// Convenience helper to enqueue a batch of `TransferIntent`s and return their `JobId`s.
    pub fn enqueue_batch(&self, intents: Vec<TransferIntent>) -> Vec<Result<JobId, SftpError>> {
        intents
            .into_iter()
            .map(|intent| {
                if !self.auth_gate.is_authorized(&intent.vault_scope) {
                    return Err(SftpError::PermissionDenied(
                        "Cannot enqueue transfer for unauthorized vault".into(),
                    ));
                }
                self.transfers.enqueue(intent)
            })
            .collect()
    }
}

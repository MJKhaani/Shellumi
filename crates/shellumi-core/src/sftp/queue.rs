//! Transfer queue scheduling and progress aggregation.
use super::{TransferDirection, TransferStatus, TransferTask};

// ---------------------------------------------------------------------------
// Background Transfer Queue & Progress Aggregation
// ---------------------------------------------------------------------------

/// Manages multiple concurrent SFTP file transfers with concurrency limits.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferQueue {
    tasks: Vec<TransferTask>,
    concurrency_limit: usize,
    next_task_id: u64,
}

impl TransferQueue {
    pub fn new(concurrency_limit: usize) -> Self {
        Self {
            tasks: Vec::new(),
            concurrency_limit: concurrency_limit.max(1),
            next_task_id: 1,
        }
    }

    pub fn tasks(&self) -> &[TransferTask] {
        &self.tasks
    }

    pub fn tasks_mut(&mut self) -> &mut [TransferTask] {
        &mut self.tasks
    }

    pub fn enqueue(
        &mut self,
        direction: TransferDirection,
        local_path: impl Into<String>,
        remote_path: impl Into<String>,
        total_bytes: u64,
    ) -> u64 {
        let id = self.next_task_id;
        self.next_task_id += 1;
        self.tasks.push(TransferTask::new(
            id,
            direction,
            local_path,
            remote_path,
            total_bytes,
        ));
        id
    }

    /// Dispatches queued tasks up to the concurrency limit, transitioning them
    /// to `InProgress`. Returns the IDs of the newly started tasks.
    pub fn dispatch_next(&mut self) -> Vec<u64> {
        let active_count = self
            .tasks
            .iter()
            .filter(|t| matches!(t.status, TransferStatus::InProgress))
            .count();

        let available_slots = self.concurrency_limit.saturating_sub(active_count);
        let mut started = Vec::new();

        if available_slots > 0 {
            for task in &mut self.tasks {
                if started.len() >= available_slots {
                    break;
                }
                if matches!(task.status, TransferStatus::Queued) {
                    task.status = TransferStatus::InProgress;
                    started.push(task.id);
                }
            }
        }

        started
    }

    pub fn update_progress(&mut self, id: u64, bytes: u64) {
        if let Some(task) = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id && t.status == TransferStatus::InProgress)
        {
            task.bytes_transferred = bytes.min(task.total_bytes);
        }
    }

    pub fn mark_completed(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.bytes_transferred = task.total_bytes;
            task.status = TransferStatus::Completed;
        }
    }

    pub fn mark_failed(&mut self, id: u64, reason: String) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.status = TransferStatus::Failed(reason);
        }
    }

    pub fn cancel(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.status = TransferStatus::Cancelled;
        }
    }

    pub fn retry(&mut self, id: u64) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            if matches!(
                task.status,
                TransferStatus::Failed(_) | TransferStatus::Cancelled
            ) {
                task.status = TransferStatus::Queued;
                task.bytes_transferred = 0;
                return true;
            }
        }
        false
    }

    pub fn clear_completed(&mut self) {
        self.tasks
            .retain(|t| !matches!(t.status, TransferStatus::Completed));
    }

    pub fn summary(&self) -> TransferQueueSummary {
        let mut pending = 0;
        let mut in_progress = 0;
        let mut completed = 0;
        let mut failed = 0;
        let mut cancelled = 0;
        let mut total_bytes = 0;
        let mut transferred_bytes = 0;

        for task in &self.tasks {
            total_bytes += task.total_bytes;
            transferred_bytes += task.bytes_transferred;
            match task.status {
                TransferStatus::Queued => pending += 1,
                TransferStatus::InProgress => in_progress += 1,
                TransferStatus::Completed => completed += 1,
                TransferStatus::Failed(_) => failed += 1,
                TransferStatus::Cancelled => cancelled += 1,
            }
        }

        let overall_percentage = if total_bytes == 0 {
            if completed > 0 && pending == 0 && in_progress == 0 {
                100.0
            } else {
                0.0
            }
        } else {
            ((transferred_bytes as f64 / total_bytes as f64) * 100.0) as f32
        };

        TransferQueueSummary {
            total_tasks: self.tasks.len(),
            pending_tasks: pending,
            in_progress_tasks: in_progress,
            completed_tasks: completed,
            failed_tasks: failed,
            cancelled_tasks: cancelled,
            total_bytes,
            transferred_bytes,
            overall_percentage,
        }
    }
}

impl Default for TransferQueue {
    fn default() -> Self {
        Self::new(2)
    }
}

/// Aggregated transfer status across all tasks in a transfer queue.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferQueueSummary {
    pub total_tasks: usize,
    pub pending_tasks: usize,
    pub in_progress_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub cancelled_tasks: usize,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub overall_percentage: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_transfer_queue_lifecycle_and_summary() {
        let mut queue = TransferQueue::new(2);
        assert_eq!(queue.concurrency_limit, 2);

        let t1 = queue.enqueue(TransferDirection::Upload, "/local/a", "/remote/a", 1000);
        let t2 = queue.enqueue(TransferDirection::Download, "/local/b", "/remote/b", 2000);
        let t3 = queue.enqueue(TransferDirection::Upload, "/local/c", "/remote/c", 3000);

        let summary = queue.summary();
        assert_eq!(summary.total_tasks, 3);
        assert_eq!(summary.pending_tasks, 3);
        assert_eq!(summary.in_progress_tasks, 0);
        assert_eq!(summary.total_bytes, 6000);
        assert_eq!(summary.transferred_bytes, 0);
        assert_eq!(summary.overall_percentage, 0.0);

        // Dispatch with concurrency limit 2
        let started = queue.dispatch_next();
        assert_eq!(started, vec![t1, t2]);
        assert_eq!(queue.summary().in_progress_tasks, 2);
        assert_eq!(queue.summary().pending_tasks, 1);

        // Progress on t1
        queue.update_progress(t1, 500);
        assert_eq!(queue.summary().transferred_bytes, 500);
        assert!((queue.summary().overall_percentage - (500.0 / 6000.0 * 100.0)).abs() < 0.1);

        // Complete t1
        queue.mark_completed(t1);
        assert_eq!(queue.summary().completed_tasks, 1);
        assert_eq!(queue.summary().in_progress_tasks, 1);

        // Next dispatch starts t3
        let started2 = queue.dispatch_next();
        assert_eq!(started2, vec![t3]);
        assert_eq!(queue.summary().in_progress_tasks, 2);

        // Fail t2
        queue.mark_failed(t2, "Connection reset by peer".to_string());
        assert_eq!(queue.summary().failed_tasks, 1);

        // Cancel t3
        queue.cancel(t3);
        assert_eq!(queue.summary().cancelled_tasks, 1);

        // Retry t2
        assert!(queue.retry(t2));
        assert_eq!(queue.summary().pending_tasks, 1);
        assert_eq!(queue.summary().failed_tasks, 0);

        // Clear completed
        queue.clear_completed();
        assert_eq!(queue.summary().total_tasks, 2);
    }
}

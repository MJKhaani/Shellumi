//! Graphical transfer scheduling. File/network IO stays in the core background worker.
use super::*;
use shellumi_core::TransferControl;

#[derive(Clone)]
pub(super) struct TransferJob {
    pub control: TransferControl,
    destination: SshDestination,
    options: SftpConnectOptions,
    local: std::path::PathBuf,
    remote: String,
    direction: TransferDirection,
    name: String,
}

/// Preserve cancellation as a distinct outcome and retain any remote cleanup warning.
fn finish_transfer(
    queue: &mut TransferQueue,
    id: u64,
    control: &TransferControl,
    result: Result<u64, SftpError>,
) -> (String, Option<String>) {
    match result {
        Ok(bytes) => {
            if let Some(task) = queue.tasks_mut().iter_mut().find(|task| task.id == id) {
                task.total_bytes = bytes;
            }
            queue.mark_completed(id);
            (format!("Transfer completed ({bytes} bytes)."), None)
        }
        Err(error) if control.is_cancelled() => {
            if matches!(error, SftpError::CleanupFailed { .. }) {
                let message = error.to_string();
                queue.mark_failed(id, message.clone());
                (
                    "Transfer cancelled; staging cleanup needs attention.".into(),
                    Some(message),
                )
            } else {
                let warning = (!matches!(error, SftpError::Cancelled)).then(|| error.to_string());
                queue.cancel(id);
                ("Transfer cancelled.".into(), warning)
            }
        }
        Err(error) => {
            let message = error.to_string();
            queue.mark_failed(id, message.clone());
            ("Transfer failed.".into(), Some(message))
        }
    }
}
impl Shellumi {
    pub(super) fn sftp_upload_selected(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        self.sftp_enqueue(tab_idx, TransferDirection::Upload, cx);
    }
    pub(super) fn sftp_download_selected(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        self.sftp_enqueue(tab_idx, TransferDirection::Download, cx);
    }
    fn sftp_enqueue(
        &mut self,
        tab_idx: usize,
        direction: TransferDirection,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.get_mut(tab_idx) else {
            return;
        };
        let tab_id = tab.id;
        let Some(browser) = &mut tab.sftp_state else {
            return;
        };
        let (selection, entries) = if direction == TransferDirection::Upload {
            (&browser.selected_local, &browser.local_entries)
        } else {
            (&browser.selected_remote, &browser.remote_entries)
        };
        let entry = selection.as_ref().and_then(|name| {
            entries
                .iter()
                .find(|entry| &entry.name == name && entry.kind == FileKind::File)
        });
        let Some(entry) = entry else {
            browser.error = Some("Select a regular file to transfer.".into());
            cx.notify();
            return;
        };
        let name = entry.name.clone();
        if !PathUtils::is_safe_file_name(&name) {
            browser.error = Some(
                "The selected entry has an unsafe filename (separator or control character)."
                    .into(),
            );
            cx.notify();
            return;
        }
        let size = entry.size;
        let Some(destination) = browser.destination.clone() else {
            browser.error = Some("Remote SFTP destination is not configured.".into());
            cx.notify();
            return;
        };
        let local = PathUtils::join_path(&browser.local_path, &name);
        let remote = PathUtils::join_path(&browser.remote_path, &name);
        // One destination at a time, including queued work, avoids racing replacements.
        if self.sftp_jobs.values().any(|job| {
            job.direction == direction
                && if direction == TransferDirection::Download {
                    job.local == std::path::Path::new(&local)
                } else {
                    job.destination == destination && job.remote == remote
                }
        }) {
            browser.error =
                Some("A transfer to this destination is already queued or running.".into());
            cx.notify();
            return;
        }
        let task_id = browser
            .queue
            .enqueue(direction, local.clone(), remote.clone(), size);
        let mut options = browser.connection_options.clone();
        options.port = Some(destination.port().get());
        if let Some(jump) = &browser.jump_host {
            options
                .extra_ssh_options
                .push(("ProxyJump".into(), jump.clone()));
        }
        self.sftp_jobs.insert(
            (tab_id, task_id),
            TransferJob {
                control: TransferControl::default(),
                destination,
                options,
                local: local.into(),
                remote,
                direction,
                name,
            },
        );
        browser.error = None;
        browser.status_message = Some("Transfer queued.".into());
        self.sftp_start_queued(tab_id, cx);
        cx.notify();
    }
    fn sftp_start_queued(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let started = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.sftp_state.as_mut())
            .map(|browser| browser.queue.dispatch_next())
            .unwrap_or_default();
        for task_id in started {
            let Some(job) = self.sftp_jobs.get(&(tab_id, task_id)).cloned() else {
                continue;
            };
            let control = job.control.clone();
            let direction = job.direction;
            let name = job.name.clone();
            let (progress_tx, progress_rx) = async_channel::bounded(1);
            cx.spawn(async move |app, cx| {
                while let Ok((bytes, total)) = progress_rx.recv().await {
                    if app
                        .update(cx, |app, cx| {
                            if let Some(browser) = app
                                .tabs
                                .iter_mut()
                                .find(|tab| tab.id == tab_id)
                                .and_then(|tab| tab.sftp_state.as_mut())
                            {
                                if let Some(task) =
                                    browser.queue.tasks_mut().iter_mut().find(|task| {
                                        task.id == task_id
                                            && task.status == TransferStatus::InProgress
                                    })
                                {
                                    task.total_bytes = total;
                                }
                                browser.queue.update_progress(task_id, bytes);
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
            let task = cx.background_executor().spawn(async move {
                job.control.check()?;
                let mut client = SftpClient::connect(&job.destination, &job.options)?;
                let callback = |bytes, total| {
                    let _ = progress_tx.try_send((bytes, total));
                };
                match job.direction {
                    TransferDirection::Upload => client.upload_file_with_control(
                        &job.local,
                        &job.remote,
                        &job.control,
                        Some(callback),
                    ),
                    TransferDirection::Download => client.download_file_with_control(
                        &job.remote,
                        &job.local,
                        &job.control,
                        Some(callback),
                    ),
                }
            });
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    let succeeded = result.is_ok();
                    app.sftp_jobs.remove(&(tab_id, task_id));
                    let Some(index) = app.tabs.iter().position(|tab| tab.id == tab_id) else {
                        return;
                    };
                    if let Some(browser) = &mut app.tabs[index].sftp_state {
                        let (message, error) =
                            finish_transfer(&mut browser.queue, task_id, &control, result);
                        browser.status_message = Some(format!("{name}: {message}"));
                        browser.error = error;
                    }
                    // Refresh local metadata on the background executor, never on GPUI's thread.
                    if direction == TransferDirection::Download {
                        app.sftp_reload_local(tab_id, cx);
                    } else if succeeded {
                        app.sftp_refresh_remote(index, cx);
                    }
                    app.sftp_start_queued(tab_id, cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }
    pub(super) fn sftp_cancel_transfer(
        &mut self,
        tab_id: u64,
        task_id: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(job) = self.sftp_jobs.get(&(tab_id, task_id)) else {
            return;
        };
        let accepted = job.control.cancel();
        if let Some(browser) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.sftp_state.as_mut())
        {
            let queued = browser
                .queue
                .tasks()
                .iter()
                .any(|task| task.id == task_id && task.status == TransferStatus::Queued);
            if queued {
                browser.queue.cancel(task_id);
                self.sftp_jobs.remove(&(tab_id, task_id));
            }
            browser.status_message = Some(
                if !accepted {
                    "Transfer is finishing; publication has already begun."
                } else if queued {
                    "Queued transfer cancelled."
                } else {
                    "Cancelling at the next packet boundary…"
                }
                .into(),
            );
        }
        self.sftp_start_queued(tab_id, cx);
        cx.notify();
    }
    fn sftp_reload_local(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(browser) = self
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.sftp_state.as_ref())
        else {
            return;
        };
        let path = browser.local_path.clone();
        let show_hidden = browser.show_hidden;
        let requested = path.clone();
        let task = cx
            .background_executor()
            .spawn(async move { list_local_directory(&path, show_hidden) });
        cx.spawn(async move |app, cx| {
            let entries = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(browser) = app
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.id == tab_id)
                    .and_then(|tab| tab.sftp_state.as_mut())
                    .filter(|browser| {
                        browser.local_path == requested && browser.show_hidden == show_hidden
                    })
                {
                    if let Ok(entries) = entries {
                        browser.local_entries = entries;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}
impl Drop for Shellumi {
    fn drop(&mut self) {
        for job in self.sftp_jobs.values() {
            job.control.cancel();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_count_is_not_completion_and_cancel_acknowledges_worker_result() {
        let mut queue = TransferQueue::new(1);
        let id = queue.enqueue(TransferDirection::Download, "/local", "/remote", 10);
        queue.dispatch_next();
        queue.update_progress(id, 10);
        assert_eq!(queue.tasks()[0].status, TransferStatus::InProgress);
        let control = TransferControl::default();
        assert!(control.cancel());
        let (_, warning) = finish_transfer(&mut queue, id, &control, Err(SftpError::Cancelled));
        assert_eq!(queue.tasks()[0].status, TransferStatus::Cancelled);
        assert!(warning.is_none());
        queue.update_progress(id, 10);
        assert_eq!(queue.tasks()[0].status, TransferStatus::Cancelled);
    }
    #[test]
    fn cleanup_warnings_survive_cancellation() {
        let mut queue = TransferQueue::default();
        let id = queue.enqueue(TransferDirection::Upload, "/local", "/remote", 10);
        queue.dispatch_next();
        let control = TransferControl::default();
        control.cancel();
        let (_, warning) = finish_transfer(
            &mut queue,
            id,
            &control,
            Err(SftpError::CleanupFailed {
                path: "/hidden.part".into(),
                message: "offline".into(),
            }),
        );
        assert!(
            matches!(&queue.tasks()[0].status, TransferStatus::Failed(message) if message.contains("/hidden.part"))
        );
        assert!(warning.unwrap().contains("/hidden.part"));
    }
    #[test]
    fn completed_byte_totals_use_actual_transfer_size() {
        let mut queue = TransferQueue::default();
        let id = queue.enqueue(TransferDirection::Upload, "/local", "/remote", 100);
        queue.dispatch_next();
        finish_transfer(&mut queue, id, &TransferControl::default(), Ok(0));
        assert_eq!(queue.tasks()[0].status, TransferStatus::Completed);
        assert_eq!(queue.tasks()[0].total_bytes, 0);
        assert_eq!(queue.tasks()[0].bytes_transferred, 0);
    }
}

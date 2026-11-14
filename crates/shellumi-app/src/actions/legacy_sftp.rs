//! Legacy sftp actions.
use super::*;

impl Shellumi {
    pub(crate) fn connect_sftp(
        &mut self,
        host: &SavedHost,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.launch_saved_connection(host, true, window, cx);
    }

    pub(crate) fn sftp_select_local(
        &mut self,
        tab_idx: usize,
        name: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.selected_local = if sftp.selected_local.as_ref() == Some(&name) {
                    None
                } else {
                    Some(name)
                };
                cx.notify();
            }
        }
    }

    pub(crate) fn sftp_select_remote(
        &mut self,
        tab_idx: usize,
        name: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.selected_remote = if sftp.selected_remote.as_ref() == Some(&name) {
                    None
                } else {
                    Some(name)
                };
                cx.notify();
            }
        }
    }

    pub(crate) fn sftp_open_local_dir(
        &mut self,
        tab_idx: usize,
        dir_name: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                let new_path = PathUtils::join_path(&sftp.local_path, &dir_name);
                match list_local_directory(&new_path, sftp.show_hidden) {
                    Ok(entries) => {
                        sftp.local_path = new_path;
                        sftp.local_entries = entries;
                        sftp.selected_local = None;
                        sftp.error = None;
                    }
                    Err(e) => {
                        sftp.error = Some(format!("Failed to open local directory: {e}"));
                    }
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn sftp_navigate_local(
        &mut self,
        tab_idx: usize,
        target_path: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                match list_local_directory(&target_path, sftp.show_hidden) {
                    Ok(entries) => {
                        sftp.local_path = target_path;
                        sftp.local_entries = entries;
                        sftp.selected_local = None;
                        sftp.error = None;
                    }
                    Err(e) => {
                        sftp.error = Some(format!("Failed to open local directory: {e}"));
                    }
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn sftp_navigate_local_up(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                if let Some(parent) = PathUtils::parent_path(&sftp.local_path) {
                    match list_local_directory(&parent, sftp.show_hidden) {
                        Ok(entries) => {
                            sftp.local_path = parent;
                            sftp.local_entries = entries;
                            sftp.selected_local = None;
                            sftp.error = None;
                        }
                        Err(e) => {
                            sftp.error = Some(format!("Failed to navigate up: {e}"));
                        }
                    }
                    cx.notify();
                }
            }
        }
    }

    pub(crate) fn sftp_refresh_remote(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        let (tab_id, generation, dest, jump_host, remote_path, show_hidden) = {
            let Some(tab) = self.tabs.get_mut(tab_idx) else {
                return;
            };
            let Some(sftp) = &mut tab.sftp_state else {
                return;
            };
            let Some(dest) = sftp.destination.clone() else {
                sftp.status_message =
                    Some("No remote host destination configured for graphical browsing.".into());
                cx.notify();
                return;
            };
            sftp.listing_generation = sftp.listing_generation.wrapping_add(1);
            sftp.loading = true;
            sftp.error = None;
            cx.notify();
            (
                tab.id,
                sftp.listing_generation,
                dest,
                sftp.jump_host.clone(),
                sftp.remote_path.clone(),
                sftp.show_hidden,
            )
        };

        let connection_options = self
            .tabs
            .get(tab_idx)
            .and_then(|tab| tab.sftp_state.as_ref())
            .map(|browser| browser.connection_options.clone())
            .unwrap_or_default();
        let task = cx.background_executor().spawn(async move {
            let mut opts = connection_options;
            opts.port = Some(dest.port().get());
            if let Some(jump) = jump_host {
                opts.extra_ssh_options.push(("ProxyJump".to_string(), jump));
            }
            let mut client = SftpClient::connect(&dest, &opts)?;
            let mut entries = client.list_remote_directory(&remote_path)?;
            if !show_hidden {
                entries.retain(|e| !e.is_hidden());
            }
            Ok::<Vec<FileEntry>, SftpError>(entries)
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(tab) = app.tabs.iter_mut().find(|tab| tab.id == tab_id) {
                    if let Some(sftp) = &mut tab.sftp_state {
                        if sftp.listing_generation != generation {
                            return;
                        }
                        sftp.loading = false;
                        match res {
                            Ok(entries) => {
                                sftp.remote_entries = entries;
                                sftp.error = None;
                                sftp.status_message = Some(format!(
                                    "Fetched {} remote items.",
                                    sftp.remote_entries.len()
                                ));
                            }
                            Err(err) => {
                                sftp.error = Some(format!("Remote SFTP error: {err}"));
                            }
                        }
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn sftp_open_remote_dir(
        &mut self,
        tab_idx: usize,
        dir_name: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.remote_path = PathUtils::join_path(&sftp.remote_path, &dir_name);
                sftp.selected_remote = None;
            }
        }
        self.sftp_refresh_remote(tab_idx, cx);
    }

    pub(crate) fn sftp_navigate_remote(
        &mut self,
        tab_idx: usize,
        target_path: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.remote_path = target_path;
                sftp.selected_remote = None;
            }
        }
        self.sftp_refresh_remote(tab_idx, cx);
    }

    pub(crate) fn sftp_navigate_remote_up(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        let has_parent = if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                if let Some(parent) = PathUtils::parent_path(&sftp.remote_path) {
                    sftp.remote_path = parent;
                    sftp.selected_remote = None;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        if has_parent {
            self.sftp_refresh_remote(tab_idx, cx);
        }
    }

    pub(crate) fn sftp_toggle_hidden(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.show_hidden = !sftp.show_hidden;
                if let Ok(entries) = list_local_directory(&sftp.local_path, sftp.show_hidden) {
                    sftp.local_entries = entries;
                }
                cx.notify();
            }
        }
        self.sftp_refresh_remote(tab_idx, cx);
    }

    pub(crate) fn sftp_toggle_mode(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(browser) = self
            .tabs
            .get_mut(tab_idx)
            .and_then(|tab| tab.sftp_state.as_mut())
        {
            browser.mode = match browser.mode {
                SftpTabMode::DualPane => SftpTabMode::Terminal,
                SftpTabMode::Terminal => SftpTabMode::DualPane,
            };
            browser.error = None;
        }
        cx.notify();
    }

    pub(crate) fn sftp_refresh(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                if let Ok(entries) = list_local_directory(&sftp.local_path, sftp.show_hidden) {
                    sftp.local_entries = entries;
                }
                sftp.status_message = Some("Directory listings refreshed.".to_string());
                sftp.error = None;
                cx.notify();
            }
        }
        self.sftp_refresh_remote(tab_idx, cx);
    }

    pub(crate) fn sftp_clear_completed(&mut self, tab_idx: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            if let Some(sftp) = &mut tab.sftp_state {
                sftp.queue.clear_completed();
                cx.notify();
            }
        }
    }
}

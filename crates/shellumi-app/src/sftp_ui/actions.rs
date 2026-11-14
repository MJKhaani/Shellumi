//! SFTP actions UI.
use super::*;

impl Shellumi {
    pub(crate) fn sftp_select_host(
        &mut self,
        is_left: bool,
        host: SftpPaneHost,
        cx: &mut Context<Self>,
    ) {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.host = Some(host);
        pane.is_selecting_host = false;
        pane.actions_open = false;
        pane.selected_file = None;
        pane.selected_files.clear();
        pane.history_back.clear();
        pane.history_forward.clear();
        pane.filter = String::new();
        pane.filter_active = false;
        match &pane.host {
            Some(SftpPaneHost::Local) => {
                pane.current_path = home_dir;
            }
            Some(SftpPaneHost::Remote(h)) => {
                let username = h.destination.username();
                pane.current_path = if username == "root" {
                    "/root".to_string()
                } else {
                    format!("/home/{}", username)
                };
            }
            None => {}
        }
        self.refresh_sftp_pane(is_left, cx);
        cx.notify();
    }

    pub(crate) fn sftp_close_host(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.host = None;
        pane.is_selecting_host = false;
        pane.actions_open = false;
        pane.selected_file = None;
        pane.selected_files.clear();
        pane.history_back.clear();
        pane.history_forward.clear();
        cx.notify();
    }

    pub(crate) fn sftp_navigate_dir(
        &mut self,
        is_left: bool,
        new_path: String,
        cx: &mut Context<Self>,
    ) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        if pane.current_path != new_path {
            pane.history_back.push(pane.current_path.clone());
            pane.history_forward.clear();
            pane.current_path = new_path;
            pane.selected_file = None;
            pane.selected_files.clear();
        }
        self.refresh_sftp_pane(is_left, cx);
        cx.notify();
    }

    pub(crate) fn sftp_navigate_parent(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let current_path = {
            let pane = if is_left {
                &self.sftp_left_pane
            } else {
                &self.sftp_right_pane
            };
            pane.current_path.clone()
        };
        let p = std::path::Path::new(&current_path);
        if let Some(parent) = p.parent() {
            let parent_str = parent.to_string_lossy().into_owned();
            if !parent_str.is_empty() {
                self.sftp_navigate_dir(is_left, parent_str, cx);
            }
        }
    }

    pub(crate) fn sftp_navigate_history_back(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let (target, curr) = {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            if let Some(prev) = pane.history_back.pop() {
                let curr = pane.current_path.clone();
                (Some(prev), curr)
            } else {
                (None, String::new())
            }
        };
        if let Some(target) = target {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            pane.history_forward.push(curr);
            pane.current_path = target;
            pane.selected_file = None;
            pane.selected_files.clear();
            self.refresh_sftp_pane(is_left, cx);
            cx.notify();
        }
    }

    pub(crate) fn sftp_navigate_history_forward(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let (target, curr) = {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            if let Some(next) = pane.history_forward.pop() {
                let curr = pane.current_path.clone();
                (Some(next), curr)
            } else {
                (None, String::new())
            }
        };
        if let Some(target) = target {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            pane.history_back.push(curr);
            pane.current_path = target;
            pane.selected_file = None;
            pane.selected_files.clear();
            self.refresh_sftp_pane(is_left, cx);
            cx.notify();
        }
    }

    pub(crate) fn sftp_toggle_pane_hidden(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.show_hidden = !pane.show_hidden;
        pane.actions_open = false;
        self.refresh_sftp_pane(is_left, cx);
        cx.notify();
    }

    pub(crate) fn sftp_toggle_actions(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.actions_open = !pane.actions_open;
        cx.notify();
    }

    #[allow(dead_code)]
    fn sftp_toggle_file_selection(
        &mut self,
        is_left: bool,
        file_name: String,
        cx: &mut Context<Self>,
    ) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        if pane.selected_files.contains(&file_name) {
            pane.selected_files.remove(&file_name);
        } else {
            pane.selected_files.insert(file_name);
        }
        cx.notify();
    }

    #[allow(dead_code)]
    fn sftp_toggle_all_selection(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        if pane.selected_files.len() == pane.entries.len() && !pane.entries.is_empty() {
            pane.selected_files.clear();
        } else {
            pane.selected_files = pane.entries.iter().map(|e| e.name.clone()).collect();
        }
        cx.notify();
    }

    pub(crate) fn sftp_transfer_files(
        &mut self,
        from_left: bool,
        files: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let (src_pane_host, src_path, dst_pane_host, dst_path) = {
            let (src, dst) = if from_left {
                (&mut self.sftp_left_pane, &mut self.sftp_right_pane)
            } else {
                (&mut self.sftp_right_pane, &mut self.sftp_left_pane)
            };
            src.actions_open = false;
            (
                src.host.clone(),
                src.current_path.clone(),
                dst.host.clone(),
                dst.current_path.clone(),
            )
        };
        self.sftp_context_menu = None;
        if files.is_empty() {
            self.desktop.message = Some("Select at least one file to transfer.".into());
            cx.notify();
            return;
        }

        if !sftp_valid_transfer_names(&files) {
            self.desktop.message = Some("Cannot transfer an unsafe filename.".into());
            cx.notify();
            return;
        }
        for pane in [&mut self.sftp_left_pane, &mut self.sftp_right_pane] {
            pane.selected_file = None;
            pane.selected_files.clear();
        }
        let src_dir = std::path::PathBuf::from(&src_path);
        let dst_dir = std::path::PathBuf::from(&dst_path);

        match (src_pane_host, dst_pane_host) {
            (Some(SftpPaneHost::Local), Some(SftpPaneHost::Local)) => {
                let mut copied = 0;
                let mut failed = 0;
                for file_name in &files {
                    let src_file = src_dir.join(file_name);
                    let dst_file = dst_dir.join(file_name);
                    if sftp_local_copy_recursive(&src_file, &dst_file).is_ok() {
                        copied += 1;
                    } else {
                        failed += 1;
                    }
                }
                self.desktop.message = Some(format!(
                    "Transferred {copied} item(s) to {} (failed: {failed})",
                    dst_dir.display()
                ));
                self.refresh_sftp_pane(!from_left, cx);
                self.refresh_sftp_pane(from_left, cx);
                cx.notify();
            }
            (Some(SftpPaneHost::Local), Some(SftpPaneHost::Remote(host))) => {
                let dest = host.destination.clone();
                let mut opts = SftpConnectOptions {
                    port: Some(dest.port().get()),
                    ..Default::default()
                };
                if let Some(jump) = &host.jump_host {
                    opts.extra_ssh_options
                        .push(("ProxyJump".to_string(), jump.clone()));
                }
                if let Some(ident_id) = &host.identity_id {
                    if let Some(ident) = self.identities.iter().find(|i| &i.id == ident_id) {
                        if let Some(kp) = &ident.key_path {
                            opts.identity_file = Some(kp.clone());
                        }
                    }
                }
                let count = files.len();
                let files_to_up = files.clone();
                let database = self.database.clone();
                let host_id = host.id;
                let to_is_left = !from_left;
                self.desktop.message = Some(format!("Uploading {count} file(s) to {}…", host.name));
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut client = connect_saved_sftp(database, host_id, &dest, &opts)?;
                    for f in &files_to_up {
                        let local_p = src_dir.join(f);
                        let remote_p = format!("{}/{}", dst_dir.display(), f);
                        sftp_upload_recursive(&mut client, &local_p, &remote_p)?;
                    }
                    Ok::<(), SftpError>(())
                });

                cx.spawn(async move |app, cx| {
                    let res = task.await;
                    let _ = app.update(cx, |app, cx| {
                        match res {
                            Ok(()) => {
                                app.desktop.message =
                                    Some(format!("Uploaded {count} file(s) successfully."));
                            }
                            Err(err) => {
                                app.desktop.message =
                                    Some(format!("Upload finished (status: {err})"));
                            }
                        }
                        app.refresh_sftp_pane(to_is_left, cx);
                        cx.notify();
                    });
                })
                .detach();
            }
            (Some(SftpPaneHost::Remote(host)), Some(SftpPaneHost::Local)) => {
                let dest = host.destination.clone();
                let mut opts = SftpConnectOptions {
                    port: Some(dest.port().get()),
                    ..Default::default()
                };
                if let Some(jump) = &host.jump_host {
                    opts.extra_ssh_options
                        .push(("ProxyJump".to_string(), jump.clone()));
                }
                if let Some(ident_id) = &host.identity_id {
                    if let Some(ident) = self.identities.iter().find(|i| &i.id == ident_id) {
                        if let Some(kp) = &ident.key_path {
                            opts.identity_file = Some(kp.clone());
                        }
                    }
                }
                let count = files.len();
                let files_to_down = files.clone();
                let database = self.database.clone();
                let host_id = host.id;
                let to_is_left = !from_left;
                self.desktop.message =
                    Some(format!("Downloading {count} file(s) from {}…", host.name));
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut client = connect_saved_sftp(database, host_id, &dest, &opts)?;
                    for f in &files_to_down {
                        let remote_p = format!("{}/{}", src_dir.display(), f);
                        let local_p = dst_dir.join(f);
                        sftp_download_recursive(&mut client, &remote_p, &local_p)?;
                    }
                    Ok::<(), SftpError>(())
                });

                cx.spawn(async move |app, cx| {
                    let res = task.await;
                    let _ = app.update(cx, |app, cx| {
                        match res {
                            Ok(()) => {
                                app.desktop.message =
                                    Some(format!("Downloaded {count} file(s) successfully."));
                            }
                            Err(err) => {
                                app.desktop.message =
                                    Some(format!("Download finished (status: {err})"));
                            }
                        }
                        app.refresh_sftp_pane(to_is_left, cx);
                        cx.notify();
                    });
                })
                .detach();
            }
            (Some(SftpPaneHost::Remote(src_h)), Some(SftpPaneHost::Remote(dst_h))) => {
                let count = files.len();
                let src_dest = src_h.destination.clone();
                let mut src_opts = SftpConnectOptions {
                    port: Some(src_dest.port().get()),
                    ..Default::default()
                };
                if let Some(jump) = &src_h.jump_host {
                    src_opts
                        .extra_ssh_options
                        .push(("ProxyJump".to_string(), jump.clone()));
                }
                if let Some(ident_id) = &src_h.identity_id {
                    if let Some(ident) = self.identities.iter().find(|i| &i.id == ident_id) {
                        if let Some(kp) = &ident.key_path {
                            src_opts.identity_file = Some(kp.clone());
                        }
                    }
                }
                let dst_dest = dst_h.destination.clone();
                let mut dst_opts = SftpConnectOptions {
                    port: Some(dst_dest.port().get()),
                    ..Default::default()
                };
                if let Some(jump) = &dst_h.jump_host {
                    dst_opts
                        .extra_ssh_options
                        .push(("ProxyJump".to_string(), jump.clone()));
                }
                if let Some(ident_id) = &dst_h.identity_id {
                    if let Some(ident) = self.identities.iter().find(|i| &i.id == ident_id) {
                        if let Some(kp) = &ident.key_path {
                            dst_opts.identity_file = Some(kp.clone());
                        }
                    }
                }
                let files_to_sync = files.clone();
                let src_database = self.database.clone();
                let dst_database = self.database.clone();
                let src_id = src_h.id;
                let dst_id = dst_h.id;
                let to_is_left = !from_left;
                self.desktop.message = Some(format!(
                    "Transferring {count} item(s) ({} ➔ {})…",
                    src_h.name, dst_h.name
                ));
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut src_client =
                        connect_saved_sftp(src_database, src_id, &src_dest, &src_opts)?;
                    let mut dst_client =
                        connect_saved_sftp(dst_database, dst_id, &dst_dest, &dst_opts)?;
                    let temp_dir = tempfile::Builder::new()
                        .prefix("shellumi-sftp-")
                        .tempdir()?;
                    for f in &files_to_sync {
                        let remote_src = format!("{}/{}", src_dir.display(), f);
                        let staging_local = temp_dir.path().join(f);
                        let remote_dst = format!("{}/{}", dst_dir.display(), f);
                        sftp_download_recursive(&mut src_client, &remote_src, &staging_local)?;
                        sftp_upload_recursive(&mut dst_client, &staging_local, &remote_dst)?;
                    }
                    temp_dir.close()?;
                    Ok::<(), SftpError>(())
                });

                cx.spawn(async move |app, cx| {
                    let res = task.await;
                    let _ = app.update(cx, |app, cx| {
                        match res {
                            Ok(()) => {
                                app.desktop.message =
                                    Some(format!("Transferred {count} item(s) successfully."));
                            }
                            Err(err) => {
                                app.desktop.message =
                                    Some(format!("Transfer finished (status: {err})"));
                            }
                        }
                        app.refresh_sftp_pane(to_is_left, cx);
                        cx.notify();
                    });
                })
                .detach();
            }
            _ => {
                self.desktop.message =
                    Some("Please connect both SFTP panes before transferring files.".into());
                cx.notify();
            }
        }
    }

    pub(crate) fn sftp_copy_to_target(&mut self, from_left: bool, cx: &mut Context<Self>) {
        let (src_pane, _) = if from_left {
            (&mut self.sftp_left_pane, &mut self.sftp_right_pane)
        } else {
            (&mut self.sftp_right_pane, &mut self.sftp_left_pane)
        };
        src_pane.actions_open = false;
        let mut files_to_transfer: Vec<String> = src_pane.selected_files.iter().cloned().collect();
        if files_to_transfer.is_empty() {
            if let Some(sel_name) = &src_pane.selected_file {
                files_to_transfer.push(sel_name.clone());
            }
        }
        if files_to_transfer.is_empty() {
            self.desktop.message =
                Some("Select a file first to copy to the target directory.".into());
            cx.notify();
            return;
        }
        self.sftp_transfer_files(from_left, files_to_transfer, cx);
    }

    pub(crate) fn refresh_sftp_pane(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let (host, current_path, show_hidden, generation) = {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            pane.listing_generation += 1;
            (
                pane.host.clone(),
                pane.current_path.clone(),
                pane.show_hidden,
                pane.listing_generation,
            )
        };

        match host {
            Some(SftpPaneHost::Local) => {
                let pane = if is_left {
                    &mut self.sftp_left_pane
                } else {
                    &mut self.sftp_right_pane
                };
                if let Ok(entries) = list_local_directory(&current_path, show_hidden) {
                    pane.entries = entries;
                }
                pane.loading = false;
                pane.error = None;
                cx.notify();
            }
            Some(SftpPaneHost::Remote(saved_host)) => {
                let pane = if is_left {
                    &mut self.sftp_left_pane
                } else {
                    &mut self.sftp_right_pane
                };
                pane.loading = true;
                pane.error = None;
                cx.notify();

                let dest = saved_host.destination.clone();
                let mut opts = SftpConnectOptions {
                    port: Some(dest.port().get()),
                    ..Default::default()
                };
                if let Some(jump) = &saved_host.jump_host {
                    opts.extra_ssh_options
                        .push(("ProxyJump".to_string(), jump.clone()));
                }
                if let Some(ident_id) = &saved_host.identity_id {
                    if let Some(ident) = self.identities.iter().find(|i| &i.id == ident_id) {
                        if let Some(kp) = &ident.key_path {
                            opts.identity_file = Some(kp.clone());
                        }
                    }
                }

                let database = self.database.clone();
                let host_id = saved_host.id;
                let req_path = current_path.clone();
                let task = cx.background_executor().spawn(async move {
                    let mut client = connect_saved_sftp(database, host_id, &dest, &opts)?;
                    let mut entries = client.list_remote_directory(&req_path)?;
                    if !show_hidden {
                        entries.retain(|e| !e.is_hidden());
                    }
                    Ok::<Vec<FileEntry>, SftpError>(entries)
                });

                cx.spawn(async move |app, cx| {
                    let res = task.await;
                    let _ = app.update(cx, |app, cx| {
                        let pane = if is_left {
                            &mut app.sftp_left_pane
                        } else {
                            &mut app.sftp_right_pane
                        };
                        if pane.listing_generation != generation {
                            return;
                        }
                        pane.loading = false;
                        match res {
                            Ok(entries) => {
                                pane.entries = entries;
                                pane.error = None;
                            }
                            Err(err) => {
                                pane.error = Some(format!("Remote SFTP: {err}"));
                                pane.entries.clear();
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            None => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sftp_open_context_menu(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        is_dir: bool,
        target_idx: usize,
        cursor_x: f32,
        cursor_y: f32,
        cx: &mut Context<Self>,
    ) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.selected_file = Some(entry_name.clone());
        self.sftp_context_menu = Some(SftpContextMenuState {
            is_left,
            entry_name,
            entry_path,
            is_dir,
            target_idx,
            cursor_x,
            cursor_y,
        });
        cx.notify();
    }

    pub(crate) fn sftp_close_context_menu(&mut self, cx: &mut Context<Self>) {
        self.sftp_context_menu = None;
        cx.notify();
    }

    pub(crate) fn sftp_open_rename_modal(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        cx: &mut Context<Self>,
    ) {
        self.sftp_context_menu = None;
        let input = cx.new(|cx| TextInput::new(cx, &entry_name, "New file name"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            action: SftpModalAction::Rename {
                entry_name,
                entry_path,
                input,
            },
            error: None,
        });
        cx.notify();
    }

    pub(crate) fn sftp_open_delete_modal(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        is_dir: bool,
        cx: &mut Context<Self>,
    ) {
        self.sftp_context_menu = None;
        self.sftp_modal = Some(SftpModalState {
            is_left,
            action: SftpModalAction::Delete {
                entry_name,
                entry_path,
                is_dir,
            },
            error: None,
        });
        cx.notify();
    }

    pub(crate) fn sftp_open_new_folder_modal(&mut self, is_left: bool, cx: &mut Context<Self>) {
        self.sftp_context_menu = None;
        let input = cx.new(|cx| TextInput::new(cx, "New Folder", "Folder name"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            action: SftpModalAction::NewFolder { input },
            error: None,
        });
        cx.notify();
    }

    pub(crate) fn sftp_open_permissions_modal(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        cx: &mut Context<Self>,
    ) {
        self.sftp_context_menu = None;
        let input = cx.new(|cx| TextInput::new(cx, "0755", "Permissions (e.g. 0755)"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            action: SftpModalAction::EditPermissions {
                entry_name,
                entry_path,
                input,
            },
            error: None,
        });
        cx.notify();
    }

    pub(crate) fn sftp_close_modal(&mut self, cx: &mut Context<Self>) {
        self.sftp_modal = None;
        cx.notify();
    }

    pub(crate) fn sftp_submit_modal(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.sftp_modal.take() else {
            return;
        };
        let is_left = modal.is_left;
        let is_local = {
            let pane = if is_left {
                &self.sftp_left_pane
            } else {
                &self.sftp_right_pane
            };
            matches!(pane.host, Some(SftpPaneHost::Local))
        };
        let curr_dir = {
            let pane = if is_left {
                &self.sftp_left_pane
            } else {
                &self.sftp_right_pane
            };
            pane.current_path.clone()
        };

        let remote_host = {
            let pane = if is_left {
                &self.sftp_left_pane
            } else {
                &self.sftp_right_pane
            };
            match &pane.host {
                Some(SftpPaneHost::Remote(host)) => Some(host.clone()),
                _ => None,
            }
        };
        if let Some(host) = remote_host {
            let name_path = |name: &str| format!("{}/{}", curr_dir.trim_end_matches('/'), name);
            let operation = match &modal.action {
                SftpModalAction::Rename {
                    entry_path, input, ..
                } => {
                    let name = input.read(cx).value().trim().to_owned();
                    if !PathUtils::is_safe_file_name(&name) {
                        self.sftp_modal = Some(modal);
                        if let Some(modal) = &mut self.sftp_modal {
                            modal.error = Some("Enter a valid single filename.".into());
                        }
                        cx.notify();
                        return;
                    }
                    RemoteFileOperation::Rename {
                        source: entry_path.clone(),
                        destination: name_path(&name),
                    }
                }
                SftpModalAction::Delete {
                    entry_path, is_dir, ..
                } => RemoteFileOperation::Delete {
                    path: entry_path.clone(),
                    is_dir: *is_dir,
                },
                SftpModalAction::NewFolder { input } => {
                    let name = input.read(cx).value().trim().to_owned();
                    if !PathUtils::is_safe_file_name(&name) {
                        self.sftp_modal = Some(modal);
                        if let Some(modal) = &mut self.sftp_modal {
                            modal.error = Some("Enter a valid single folder name.".into());
                        }
                        cx.notify();
                        return;
                    }
                    RemoteFileOperation::Mkdir {
                        path: name_path(&name),
                    }
                }
                SftpModalAction::EditPermissions {
                    entry_path, input, ..
                } => {
                    let value = input.read(cx).value();
                    let Some(mode) = parse_sftp_permissions(value) else {
                        self.sftp_modal = Some(modal);
                        if let Some(modal) = &mut self.sftp_modal {
                            modal.error = Some("Enter octal permissions, for example 0755.".into());
                        }
                        cx.notify();
                        return;
                    };
                    RemoteFileOperation::Permissions {
                        path: entry_path.clone(),
                        mode,
                    }
                }
            };
            let dest = host.destination.clone();
            let mut options = SftpConnectOptions {
                port: Some(dest.port().get()),
                ..Default::default()
            };
            if let Some(jump) = host.jump_host {
                options.extra_ssh_options.push(("ProxyJump".into(), jump));
            }
            if let Some(identity) = host
                .identity_id
                .and_then(|id| self.identities.iter().find(|i| i.id == id))
            {
                options.identity_file = identity.key_path.clone();
                options.certificate_file = identity.certificate_path.clone();
            }
            self.desktop.message = Some("Applying SFTP operation…".into());
            let database = self.database.clone();
            let host_id = host.id;
            let task = cx.background_executor().spawn(async move {
                let mut client = connect_saved_sftp(database, host_id, &dest, &options)?;
                match operation {
                    RemoteFileOperation::Rename {
                        source,
                        destination,
                    } => client.rename(&source, &destination),
                    RemoteFileOperation::Delete { path, is_dir } => {
                        if is_dir {
                            client.rmdir(&path)
                        } else {
                            client.remove_file(&path)
                        }
                    }
                    RemoteFileOperation::Mkdir { path } => client.mkdir(&path),
                    RemoteFileOperation::Permissions { path, mode } => {
                        client.set_permissions(&path, mode)
                    }
                }
            });
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    app.desktop.message = Some(match result {
                        Ok(()) => "SFTP operation completed.".into(),
                        Err(err) => format!("SFTP operation failed: {err}"),
                    });
                    app.refresh_sftp_pane(is_left, cx);
                    cx.notify();
                });
            })
            .detach();
            cx.notify();
            return;
        }
        if !is_local {
            self.desktop.message = Some("Connect a pane before changing files.".into());
            cx.notify();
            return;
        }
        let result: Result<String, String> = match modal.action {
            SftpModalAction::Rename {
                entry_name,
                entry_path,
                input,
            } => {
                let name = input.read(cx).value().trim().to_owned();
                if !PathUtils::is_safe_file_name(&name) {
                    Err("Enter a valid single filename.".into())
                } else if name == entry_name {
                    Ok("Name unchanged.".into())
                } else {
                    let src = std::path::PathBuf::from(entry_path);
                    let dst = src.with_file_name(&name);
                    if dst.exists() {
                        Err("The destination already exists.".into())
                    } else {
                        std::fs::rename(src, dst)
                            .map(|_| format!("Renamed to '{name}'."))
                            .map_err(|e| e.to_string())
                    }
                }
            }
            SftpModalAction::Delete {
                entry_name,
                entry_path,
                is_dir,
            } => {
                let result = if is_dir {
                    std::fs::remove_dir_all(entry_path)
                } else {
                    std::fs::remove_file(entry_path)
                };
                result
                    .map(|_| format!("Deleted '{entry_name}'."))
                    .map_err(|e| e.to_string())
            }
            SftpModalAction::NewFolder { input } => {
                let name = input.read(cx).value().trim().to_owned();
                if !PathUtils::is_safe_file_name(&name) {
                    Err("Enter a valid single folder name.".into())
                } else {
                    std::fs::create_dir(std::path::PathBuf::from(&curr_dir).join(&name))
                        .map(|_| format!("Created folder '{name}'."))
                        .map_err(|e| e.to_string())
                }
            }
            SftpModalAction::EditPermissions {
                entry_name,
                entry_path,
                input,
            } => {
                let value = input.read(cx).value();
                if let Some(mode) = parse_sftp_permissions(value) {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(entry_path, std::fs::Permissions::from_mode(mode))
                            .map(|_| format!("Permissions updated for '{entry_name}'."))
                            .map_err(|e| e.to_string())
                    }
                    #[cfg(not(unix))]
                    {
                        let _ = (entry_name, entry_path, mode);
                        Err("Changing Unix permissions is unsupported on this platform.".into())
                    }
                } else {
                    Err("Enter octal permissions, for example 0755.".into())
                }
            }
        };
        self.desktop.message = Some(match result {
            Ok(message) => message,
            Err(error) => format!("File operation failed: {error}"),
        });
        self.refresh_sftp_pane(is_left, cx);
        cx.notify();
    }
}

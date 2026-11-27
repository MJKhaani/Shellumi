//! SFTP actions UI.
use super::*;

impl Shellumi {
    pub(crate) fn current_vault_scope(&self) -> shellumi_core::VaultScope {
        shellumi_core::VaultScope::new(self.active_vault_id.clone(), self.vault_load_generation)
    }

    pub(crate) fn enforce_sftp_vault_scope(&mut self, locked: bool) {
        let active_scope = if locked {
            None
        } else {
            Some(self.current_vault_scope())
        };
        self.sftp_connection_manager
            .enforce_vault_scope(active_scope.as_ref());
        self.sftp_transfer_manager
            .cancel_unauthorized_vaults(active_scope.as_ref());

        for pane in [&mut self.sftp_left_pane, &mut self.sftp_right_pane] {
            if matches!(pane.host, Some(SftpPaneHost::Remote(_))) {
                pane.listing_generation = pane.listing_generation.wrapping_add(1);
                pane.host = None;
                pane.endpoint_id = None;
                pane.entries.clear();
                pane.selected_file = None;
                pane.selected_files.clear();
                pane.history_back.clear();
                pane.history_forward.clear();
                pane.loading = false;
                pane.error = None;
            }
        }
        self.sftp_context_menu = None;
        self.sftp_modal = None;
        if locked {
            // Protect/clear remote editor plaintext on explicit vault lock
            self.sftp_editors.retain(|ed| ed.meta.endpoint.is_local());
            if let Some(idx) = self.active_sftp_editor_idx {
                if idx >= self.sftp_editors.len() {
                    self.active_sftp_editor_idx = None;
                }
            }
        }
    }

    pub(crate) fn sftp_select_host(
        &mut self,
        is_left: bool,
        host: SftpPaneHost,
        cx: &mut Context<Self>,
    ) {
        let home_dir = shellumi_core::local_home_dir()
            .to_string_lossy()
            .into_owned();
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.host = Some(host.clone());
        pane.endpoint_id = Some(shellumi_core::EndpointId::new());
        pane.is_selecting_host = false;
        pane.actions_open = false;
        pane.selected_file = None;
        pane.selected_files.clear();
        pane.history_back.clear();
        pane.history_forward.clear();
        pane.filter = String::new();
        pane.filter_active = false;
        match &host {
            SftpPaneHost::Local => {
                pane.current_path = home_dir;
                self.refresh_sftp_pane(is_left, cx);
            }
            SftpPaneHost::Remote(h) => {
                // Discover real remote home via REALPATH(".") rather than synthesizing /home/<user>.
                pane.listing_generation = pane.listing_generation.wrapping_add(1);
                let generation = pane.listing_generation;
                pane.loading = true;
                pane.error = None;
                pane.entries.clear();
                pane.current_path = ".".to_string();

                let dest = h.destination.clone();
                let mut opts = SftpConnectOptions {
                    port: Some(dest.port().get()),
                    ..Default::default()
                };
                if let Some(jump) = &h.jump_host {
                    opts.extra_ssh_options
                        .push(("ProxyJump".to_string(), jump.clone()));
                }
                if let Some(ident_id) = &h.identity_id {
                    if let Some(ident) = self.identities.iter().find(|i| &i.id == ident_id) {
                        if let Some(kp) = &ident.key_path {
                            opts.identity_file = Some(kp.clone());
                        }
                        if let Some(cp) = &ident.certificate_path {
                            opts.certificate_file = Some(cp.clone());
                        }
                    }
                }
                let show_hidden = pane.show_hidden;
                let database = self.database.clone();
                let active_vault_id = self.active_vault_id.clone();
                let active_vault_key = self.active_vault_key();
                let host_id = h.id;
                let task = cx.background_executor().spawn(async move {
                    let mut client = connect_saved_sftp(
                        database,
                        Some(&active_vault_id),
                        active_vault_key.as_ref(),
                        host_id,
                        &dest,
                        &opts,
                    )?;
                    let real_home = client.realpath(".")?;
                    let mut entries = client.list_remote_directory(&real_home)?;
                    if !show_hidden {
                        entries.retain(|e| !e.is_hidden());
                    }
                    Ok::<(String, Vec<FileEntry>), SftpError>((real_home, entries))
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
                            Ok((real_home, entries)) => {
                                pane.current_path = real_home;
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
        }
        cx.notify();
    }

    pub(crate) fn sftp_close_host(&mut self, is_left: bool, cx: &mut Context<Self>) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.listing_generation = pane.listing_generation.wrapping_add(1);
        pane.host = None;
        pane.endpoint_id = None;
        pane.is_selecting_host = false;
        pane.actions_open = false;
        pane.entries.clear();
        pane.selected_file = None;
        pane.selected_files.clear();
        pane.history_back.clear();
        pane.history_forward.clear();
        pane.loading = false;
        pane.error = None;
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
        let (is_local, current_path) = {
            let pane = if is_left {
                &self.sftp_left_pane
            } else {
                &self.sftp_right_pane
            };
            (
                matches!(pane.host, Some(SftpPaneHost::Local)),
                pane.current_path.clone(),
            )
        };
        let parent_opt = if is_local {
            shellumi_core::local_parent(std::path::Path::new(&current_path))
                .map(|p| p.to_string_lossy().into_owned())
        } else {
            shellumi_core::RemotePath::from_posix_str(&current_path)
                .parent()
                .map(|p| p.display().to_string())
        };
        if let Some(parent_str) = parent_opt {
            if !parent_str.is_empty() && parent_str != current_path {
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

    pub(crate) fn sftp_toggle_sort(
        &mut self,
        is_left: bool,
        field: SftpSortField,
        cx: &mut Context<Self>,
    ) {
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        if pane.sort_field == field {
            pane.sort_ascending = !pane.sort_ascending;
        } else {
            pane.sort_field = field;
            pane.sort_ascending = true;
        }
        let asc = pane.sort_ascending;
        pane.entries.sort_by(|a, b| {
            let dir_ord = match (a.kind.is_dir(), b.kind.is_dir()) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };
            if dir_ord != std::cmp::Ordering::Equal {
                return dir_ord;
            }
            let ord = match field {
                SftpSortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SftpSortField::Size => a.size.cmp(&b.size),
                SftpSortField::Modified => a.modified.cmp(&b.modified),
                SftpSortField::Kind => format!("{:?}", a.kind).cmp(&format!("{:?}", b.kind)),
            };
            let ord = if asc { ord } else { ord.reverse() };
            ord.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        cx.notify();
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
        self.sftp_transfer_files_to_dir(from_left, files, None, cx);
    }

    pub(crate) fn sftp_transfer_files_to_dir(
        &mut self,
        from_left: bool,
        files: Vec<String>,
        explicit_dst_dir: Option<String>,
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
                explicit_dst_dir.unwrap_or_else(|| dst.current_path.clone()),
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
                let mgr = std::sync::Arc::clone(&self.sftp_transfer_manager);
                let vault_scope = self.current_vault_scope();
                let batch_id = shellumi_core::BatchId::new();
                let files_clone = files.clone();
                let to_is_left = !from_left;
                self.sftp_transfer_drawer_open = true;
                self.desktop.message = Some(format!(
                    "Queued {} local staged copy item(s)…",
                    files_clone.len()
                ));
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut copied = 0usize;
                    let mut failed = 0usize;
                    for file_name in &files_clone {
                        let Ok(src_file) = shellumi_core::local_join_child(&src_dir, file_name)
                        else {
                            failed += 1;
                            continue;
                        };
                        let Ok(dst_file) = shellumi_core::local_join_child(&dst_dir, file_name)
                        else {
                            failed += 1;
                            continue;
                        };
                        let job_id = shellumi_core::JobId::new();
                        let intent = shellumi_core::TransferIntent {
                            job_id,
                            batch_id,
                            vault_scope: vault_scope.clone(),
                            route: shellumi_core::TransferRoute::LocalToLocal,
                            source_endpoint: shellumi_core::EndpointKind::local(
                                vault_scope.clone(),
                            ),
                            destination_endpoint: shellumi_core::EndpointKind::local(
                                vault_scope.clone(),
                            ),
                            source_path: shellumi_core::EndpointPath::Local(src_file.clone()),
                            destination_path: shellumi_core::EndpointPath::Local(dst_file.clone()),
                            source_options: None,
                            destination_options: None,
                            conflict_policy: shellumi_core::ConflictPolicy::Ask,
                            symlink_policy: shellumi_core::SymlinkPolicy::Skip,
                            preserve_metadata: true,
                            delete_source_after_commit: false,
                            expected_bytes: std::fs::symlink_metadata(&src_file)
                                .ok()
                                .map(|m| m.len()),
                        };
                        if mgr.enqueue(intent).is_ok() {
                            for (dispatched, attempt_id, ctrl) in mgr.dispatch_ready(&vault_scope) {
                                let res =
                                    mgr.execute_local_to_local(&dispatched, attempt_id, &ctrl);
                                if res.is_ok() {
                                    copied += 1;
                                } else {
                                    failed += 1;
                                }
                                mgr.complete_attempt(dispatched.job_id, attempt_id, res);
                            }
                        } else {
                            failed += 1;
                        }
                    }
                    (copied, failed)
                });
                cx.spawn(async move |app, cx| {
                    let (copied, failed) = task.await;
                    let _ = app.update(cx, |app, cx| {
                        app.desktop.message = Some(format!(
                            "Local copy finished: {copied} succeeded, {failed} failed."
                        ));
                        app.refresh_sftp_pane(to_is_left, cx);
                        app.refresh_sftp_pane(from_left, cx);
                        cx.notify();
                    });
                })
                .detach();
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
                let active_vault_id = self.active_vault_id.clone();
                let active_vault_key = self.active_vault_key();
                let host_id = host.id;
                let to_is_left = !from_left;
                self.sftp_transfer_drawer_open = true;
                self.desktop.message = Some(format!("Uploading {count} file(s) to {}…", host.name));
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut client = connect_saved_sftp(
                        database,
                        Some(&active_vault_id),
                        active_vault_key.as_ref(),
                        host_id,
                        &dest,
                        &opts,
                    )?;
                    for f in &files_to_up {
                        let local_p = shellumi_core::local_join_child(&src_dir, f)
                            .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?;
                        let remote_p = format!("{}/{}", dst_dir.display(), f);
                        client.upload_file(&local_p, &remote_p, None::<fn(u64, u64)>)?;
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
                let active_vault_id = self.active_vault_id.clone();
                let active_vault_key = self.active_vault_key();
                let host_id = host.id;
                let to_is_left = !from_left;
                self.sftp_transfer_drawer_open = true;
                self.desktop.message =
                    Some(format!("Downloading {count} file(s) from {}…", host.name));
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut client = connect_saved_sftp(
                        database,
                        Some(&active_vault_id),
                        active_vault_key.as_ref(),
                        host_id,
                        &dest,
                        &opts,
                    )?;
                    for f in &files_to_down {
                        let remote_p = format!("{}/{}", src_dir.display(), f);
                        let local_p = shellumi_core::local_join_child(&dst_dir, f)
                            .map_err(|e| SftpError::UnexpectedResponse(e.to_string()))?;
                        client.download_file(&remote_p, &local_p, None::<fn(u64, u64)>)?;
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
                let active_vault_id = self.active_vault_id.clone();
                let active_vault_key = self.active_vault_key();
                let src_id = src_h.id;
                let dst_id = dst_h.id;
                let to_is_left = !from_left;
                self.sftp_transfer_drawer_open = true;
                self.desktop.message = Some(format!(
                    "Relaying {count} item(s) ({} ➔ {})…",
                    src_h.name, dst_h.name
                ));
                cx.notify();

                let task = cx.background_executor().spawn(async move {
                    let mut src_client = connect_saved_sftp(
                        src_database,
                        Some(&active_vault_id),
                        active_vault_key.as_ref(),
                        src_id,
                        &src_dest,
                        &src_opts,
                    )?;
                    let mut dst_client = connect_saved_sftp(
                        dst_database,
                        Some(&active_vault_id),
                        active_vault_key.as_ref(),
                        dst_id,
                        &dst_dest,
                        &dst_opts,
                    )?;
                    let ctrl = shellumi_core::TransferControl::default();
                    for f in &files_to_sync {
                        let remote_src = format!("{}/{}", src_dir.display(), f);
                        let remote_dst = format!("{}/{}", dst_dir.display(), f);
                        shellumi_core::relay_remote_to_remote(
                            &mut src_client,
                            &remote_src,
                            &mut dst_client,
                            &remote_dst,
                            &ctrl,
                            None,
                        )?;
                    }
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
                let cap = shellumi_core::local_disk_capacity(std::path::Path::new(&current_path));
                let res = list_local_directory(&current_path, show_hidden);
                let pane = if is_left {
                    &mut self.sftp_left_pane
                } else {
                    &mut self.sftp_right_pane
                };
                if pane.listing_generation != generation {
                    return;
                }
                pane.loading = false;
                pane.capacity = cap;
                match res {
                    Ok(entries) => {
                        pane.entries = entries;
                        pane.error = None;
                    }
                    Err(err) => {
                        pane.error = Some(format!("Local directory error: {err}"));
                        pane.entries.clear();
                    }
                }
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
                let active_vault_id = self.active_vault_id.clone();
                let active_vault_key = self.active_vault_key();
                let host_id = saved_host.id;
                let req_path = current_path.clone();
                let task = cx.background_executor().spawn(async move {
                    let mut client = connect_saved_sftp(
                        database,
                        Some(&active_vault_id),
                        active_vault_key.as_ref(),
                        host_id,
                        &dest,
                        &opts,
                    )?;
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

    fn capture_modal_scope(&self, is_left: bool) -> (u64, String) {
        let pane = if is_left {
            &self.sftp_left_pane
        } else {
            &self.sftp_right_pane
        };
        (pane.listing_generation, pane.current_path.clone())
    }

    pub(crate) fn sftp_open_rename_modal(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        cx: &mut Context<Self>,
    ) {
        self.sftp_context_menu = None;
        let (endpoint_generation, target_dir) = self.capture_modal_scope(is_left);
        let input = cx.new(|cx| TextInput::new(cx, &entry_name, "New file name"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            endpoint_generation,
            target_dir,
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
        let (endpoint_generation, target_dir) = self.capture_modal_scope(is_left);
        self.sftp_modal = Some(SftpModalState {
            is_left,
            endpoint_generation,
            target_dir,
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
        let (endpoint_generation, target_dir) = self.capture_modal_scope(is_left);
        let input = cx.new(|cx| TextInput::new(cx, "New Folder", "Folder name"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            endpoint_generation,
            target_dir,
            action: SftpModalAction::NewFolder { input },
            error: None,
        });
        cx.notify();
    }

    pub(crate) fn sftp_open_new_file_modal(&mut self, is_left: bool, cx: &mut Context<Self>) {
        self.sftp_context_menu = None;
        let (endpoint_generation, target_dir) = self.capture_modal_scope(is_left);
        let input = cx.new(|cx| TextInput::new(cx, "untitled.conf", "File name"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            endpoint_generation,
            target_dir,
            action: SftpModalAction::NewFile { input },
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
        let (endpoint_generation, target_dir) = self.capture_modal_scope(is_left);
        let input = cx.new(|cx| TextInput::new(cx, "0755", "Permissions (e.g. 0755)"));
        self.sftp_modal = Some(SftpModalState {
            is_left,
            endpoint_generation,
            target_dir,
            action: SftpModalAction::EditPermissions {
                entry_name,
                entry_path,
                input,
            },
            error: None,
        });
        cx.notify();
    }

    pub(crate) fn sftp_open_properties_modal(
        &mut self,
        is_left: bool,
        entry_name: String,
        entry_path: String,
        cx: &mut Context<Self>,
    ) {
        self.sftp_context_menu = None;
        let (endpoint_generation, target_dir) = self.capture_modal_scope(is_left);
        let pane = if is_left {
            &self.sftp_left_pane
        } else {
            &self.sftp_right_pane
        };
        let entry = pane
            .entries
            .iter()
            .find(|e| e.path == entry_path || e.name == entry_name);
        let size = entry.map(|e| e.size).unwrap_or(0);
        let permissions = entry
            .map(|e| e.formatted_permissions())
            .unwrap_or_else(|| "-".into());
        let sha256 = if matches!(pane.host, Some(SftpPaneHost::Local))
            && entry.is_some_and(|e| e.kind.is_file())
        {
            shellumi_core::compute_local_sha256(
                std::path::Path::new(&entry_path),
                &shellumi_core::TransferControl::default(),
            )
            .ok()
        } else {
            None
        };
        self.sftp_modal = Some(SftpModalState {
            is_left,
            endpoint_generation,
            target_dir,
            action: SftpModalAction::Properties {
                entry_name,
                entry_path,
                size,
                permissions,
                sha256,
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
        let (is_local, current_gen, remote_host) = {
            let pane = if is_left {
                &self.sftp_left_pane
            } else {
                &self.sftp_right_pane
            };
            (
                matches!(pane.host, Some(SftpPaneHost::Local)),
                pane.listing_generation,
                match &pane.host {
                    Some(SftpPaneHost::Remote(host)) => Some(host.clone()),
                    _ => None,
                },
            )
        };
        if current_gen != modal.endpoint_generation {
            self.desktop.message =
                Some("Operation cancelled: pane directory or host changed.".into());
            cx.notify();
            return;
        }
        let curr_dir = modal.target_dir.clone();

        if let Some(host) = remote_host {
            let name_path = |name: &str| {
                if curr_dir == "/" {
                    format!("/{name}")
                } else {
                    format!("{}/{name}", curr_dir.trim_end_matches('/'))
                }
            };
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
                SftpModalAction::NewFile { input } => {
                    let name = input.read(cx).value().trim().to_owned();
                    if !PathUtils::is_safe_file_name(&name) {
                        self.sftp_modal = Some(modal);
                        if let Some(modal) = &mut self.sftp_modal {
                            modal.error = Some("Enter a valid single filename.".into());
                        }
                        cx.notify();
                        return;
                    }
                    RemoteFileOperation::CreateFile {
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
                SftpModalAction::Properties { .. } => {
                    cx.notify();
                    return;
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
            let active_vault_id = self.active_vault_id.clone();
            let active_vault_key = self.active_vault_key();
            let host_id = host.id;
            let task = cx.background_executor().spawn(async move {
                let mut client = connect_saved_sftp(
                    database,
                    Some(&active_vault_id),
                    active_vault_key.as_ref(),
                    host_id,
                    &dest,
                    &options,
                )?;
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
                    RemoteFileOperation::CreateFile { path } => {
                        client.create_exclusive_file(&path, 0o644)
                    }
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
                    if std::fs::symlink_metadata(&dst).is_ok() {
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
                ..
            } => {
                let ctrl = shellumi_core::TransferControl::default();
                shellumi_core::delete_local_recursive(std::path::Path::new(&entry_path), &ctrl)
                    .map(|count| format!("Deleted '{entry_name}' ({count} item(s))."))
                    .map_err(|e| e.to_string())
            }
            SftpModalAction::NewFolder { input } => {
                let name = input.read(cx).value().trim().to_owned();
                if !PathUtils::is_safe_file_name(&name) {
                    Err("Enter a valid single folder name.".into())
                } else {
                    match shellumi_core::local_join_child(std::path::Path::new(&curr_dir), &name) {
                        Ok(target) => std::fs::create_dir(target)
                            .map(|_| format!("Created folder '{name}'."))
                            .map_err(|e| e.to_string()),
                        Err(e) => Err(e.to_string()),
                    }
                }
            }
            SftpModalAction::NewFile { input } => {
                let name = input.read(cx).value().trim().to_owned();
                if !PathUtils::is_safe_file_name(&name) {
                    Err("Enter a valid single filename.".into())
                } else {
                    match shellumi_core::local_join_child(std::path::Path::new(&curr_dir), &name) {
                        Ok(target) => std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(target)
                            .map(|_| format!("Created file '{name}'."))
                            .map_err(|e| e.to_string()),
                        Err(e) => Err(e.to_string()),
                    }
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
            SftpModalAction::Properties { .. } => Ok(String::new()),
        };
        if !matches!(result, Ok(ref s) if s.is_empty()) {
            self.desktop.message = Some(match result {
                Ok(message) => message,
                Err(error) => format!("File operation failed: {error}"),
            });
        }
        self.refresh_sftp_pane(is_left, cx);
        cx.notify();
    }
}

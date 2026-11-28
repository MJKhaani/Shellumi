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
        self.sftp_controller
            .revoke_unauthorized(active_scope.as_ref());

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

        let active_id = self
            .active_sftp_editor_idx
            .and_then(|idx| self.sftp_editors.get(idx))
            .map(|ed| ed.meta.editor_id);
        self.sftp_editors.retain(|ed| {
            ed.meta.endpoint.is_local()
                && active_scope
                    .as_ref()
                    .is_some_and(|s| ed.meta.vault_scope.is_authorized_under(s))
        });
        self.active_sftp_editor_idx = active_id.and_then(|id| {
            self.sftp_editors
                .iter()
                .position(|e| e.meta.editor_id == id)
        });
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
        let endpoint_id = shellumi_core::EndpointId::new();
        let scope = self.current_vault_scope();
        let database = self.database.clone();
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_key = self.active_vault_key();
        let pane = if is_left {
            &mut self.sftp_left_pane
        } else {
            &mut self.sftp_right_pane
        };
        pane.host = Some(host.clone());
        pane.endpoint_id = Some(endpoint_id);
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
                let host_id = h.id;
                let remote_endpoint = shellumi_core::RemoteEndpoint {
                    endpoint_id,
                    vault_scope: scope.clone(),
                    host_id,
                    host_uuid: None,
                    display_name: h.name.clone(),
                    destination: dest.clone(),
                    connection_generation: generation,
                    database_path: database.clone(),
                };
                let controller = self.sftp_controller.clone();
                let Ok(lease) = controller.register_lease(&scope) else {
                    pane.loading = false;
                    pane.error = Some("Vault is locked or inactive.".into());
                    cx.notify();
                    return;
                };
                let _op_id = lease.op_id;

                let task = cx.background_executor().spawn(async move {
                    let _lease = lease;
                    _lease.check()?;
                    let resolved_opts = if let Some(ref db) = database {
                        let runtime = tokio::runtime::Builder::new_current_thread().build()?;
                        let plan = runtime
                            .block_on(shellumi_storage::desktop::resolve_connection_with_key(
                                db,
                                host_id,
                                Some(&active_vault_id),
                                active_vault_key.as_ref(),
                            ))
                            .map_err(SftpError::ProcessSpawn)?;
                        let command = askpass::command("ssh", Vec::new(), &plan)
                            .map_err(SftpError::ProcessSpawn)?;
                        let base = SftpConnectOptions {
                            port: Some(plan.destination.port().get()),
                            identity_file: plan.identity_file,
                            certificate_file: plan.certificate_file,
                            extra_ssh_options: plan.authentication_options,
                            environment: command.env.into_iter().collect(),
                            ..Default::default()
                        };
                        let mut merged = shellumi_core::merge_connect_options(&base, &opts);
                        if let Some(jump) = plan.jump_host {
                            if !merged
                                .extra_ssh_options
                                .iter()
                                .any(|(k, _)| k.eq_ignore_ascii_case("ProxyJump"))
                            {
                                merged.extra_ssh_options.push(("ProxyJump".into(), jump));
                            }
                        }
                        merged
                    } else {
                        opts
                    };
                    _lease.check()?;
                    let sess_arc = controller
                        .connections
                        .register_session(remote_endpoint, resolved_opts);
                    let res = {
                        let mut sess = sess_arc.lock().map_err(|_| {
                            SftpError::UnexpectedResponse("session lock poisoned".into())
                        })?;
                        let real_home = sess.connect()?.display().to_string();
                        _lease.check()?;
                        let mut entries =
                            sess.with_browse_client(|c| c.list_remote_directory(&real_home))?;
                        if !show_hidden {
                            entries.retain(|e| !e.is_hidden());
                        }
                        Ok::<(String, Vec<FileEntry>), SftpError>((real_home, entries))
                    };
                    res
                });
                let controller_ui = self.sftp_controller.clone();
                cx.spawn(async move |app, cx| {
                    let res = task.await;
                    let _ = app.update(cx, |app, cx| {
                        if !controller_ui.auth_gate.is_authorized(&scope) {
                            return;
                        }
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

    fn build_endpoint_and_opts(
        &self,
        pane: &SftpPaneView,
        scope: &shellumi_core::VaultScope,
    ) -> Option<(shellumi_core::EndpointKind, Option<SftpConnectOptions>)> {
        match &pane.host {
            Some(SftpPaneHost::Local) => {
                Some((shellumi_core::EndpointKind::local(scope.clone()), None))
            }
            Some(SftpPaneHost::Remote(h)) => {
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
                let ep = shellumi_core::EndpointKind::Remote(shellumi_core::RemoteEndpoint {
                    endpoint_id: pane.endpoint_id.unwrap_or_default(),
                    vault_scope: scope.clone(),
                    host_id: h.id,
                    host_uuid: None,
                    display_name: h.name.clone(),
                    destination: dest,
                    connection_generation: pane.listing_generation,
                    database_path: self.database.clone(),
                });
                Some((ep, Some(opts)))
            }
            None => None,
        }
    }

    pub(crate) fn sftp_transfer_files_to_dir(
        &mut self,
        from_left: bool,
        files: Vec<String>,
        explicit_dst_dir: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let scope = self.current_vault_scope();
        let (src_info, src_path, dst_info, dst_path, src_entries) = {
            let (src, dst) = if from_left {
                (&self.sftp_left_pane, &self.sftp_right_pane)
            } else {
                (&self.sftp_right_pane, &self.sftp_left_pane)
            };
            (
                self.build_endpoint_and_opts(src, &scope),
                src.current_path.clone(),
                self.build_endpoint_and_opts(dst, &scope),
                explicit_dst_dir.unwrap_or_else(|| dst.current_path.clone()),
                src.entries.clone(),
            )
        };
        if from_left {
            self.sftp_left_pane.actions_open = false;
        } else {
            self.sftp_right_pane.actions_open = false;
        }
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
        let (Some((src_ep, src_opts)), Some((dst_ep, dst_opts))) = (src_info, dst_info) else {
            self.desktop.message =
                Some("Please connect both SFTP panes before transferring files.".into());
            cx.notify();
            return;
        };

        for pane in [&mut self.sftp_left_pane, &mut self.sftp_right_pane] {
            pane.selected_file = None;
            pane.selected_files.clear();
        }

        let route = shellumi_core::TransferRoute::from_endpoints(&src_ep, &dst_ep);
        let batch_id = shellumi_core::BatchId::new();
        let mut intents = Vec::with_capacity(files.len());

        for name in &files {
            let src_full = match &src_ep {
                shellumi_core::EndpointKind::Local { .. } => {
                    let Ok(p) =
                        shellumi_core::local_join_child(std::path::Path::new(&src_path), name)
                    else {
                        self.desktop.message = Some(format!("Unsafe source filename '{name}'."));
                        cx.notify();
                        return;
                    };
                    shellumi_core::EndpointPath::Local(p)
                }
                shellumi_core::EndpointKind::Remote(_) => {
                    let Ok(rp) =
                        shellumi_core::RemotePath::from_posix_str(&src_path).join_child_str(name)
                    else {
                        self.desktop.message = Some(format!("Unsafe source filename '{name}'."));
                        cx.notify();
                        return;
                    };
                    shellumi_core::EndpointPath::Remote(rp)
                }
            };
            let dst_full = match &dst_ep {
                shellumi_core::EndpointKind::Local { .. } => {
                    let Ok(p) =
                        shellumi_core::local_join_child(std::path::Path::new(&dst_path), name)
                    else {
                        self.desktop.message =
                            Some(format!("Unsafe destination filename '{name}'."));
                        cx.notify();
                        return;
                    };
                    shellumi_core::EndpointPath::Local(p)
                }
                shellumi_core::EndpointKind::Remote(_) => {
                    let Ok(rp) =
                        shellumi_core::RemotePath::from_posix_str(&dst_path).join_child_str(name)
                    else {
                        self.desktop.message =
                            Some(format!("Unsafe destination filename '{name}'."));
                        cx.notify();
                        return;
                    };
                    shellumi_core::EndpointPath::Remote(rp)
                }
            };
            let expected_bytes = src_entries.iter().find(|e| e.name == *name).map(|e| e.size);

            intents.push(shellumi_core::TransferIntent {
                job_id: shellumi_core::JobId::new(),
                batch_id,
                vault_scope: scope.clone(),
                route,
                source_endpoint: src_ep.clone(),
                destination_endpoint: dst_ep.clone(),
                source_path: src_full,
                destination_path: dst_full,
                source_options: src_opts.clone(),
                destination_options: dst_opts.clone(),
                conflict_policy: shellumi_core::ConflictPolicy::Ask,
                symlink_policy: shellumi_core::SymlinkPolicy::Skip,
                preserve_metadata: true,
                delete_source_after_commit: false,
                expected_bytes,
            });
        }

        let results = self.sftp_controller.enqueue_batch(intents);
        let enqueued = results.iter().filter(|r| r.is_ok()).count();
        let failed = results.len() - enqueued;
        self.sftp_transfer_drawer_open = true;
        self.desktop.message = Some(if failed == 0 {
            format!("Queued {enqueued} {route} transfer(s)…")
        } else {
            format!("Queued {enqueued} transfer(s) ({failed} rejected).")
        });
        self.sftp_pump_transfers(cx);
        cx.notify();
    }

    /// Unified scheduler pump for ALL four transfer routes (`LocalToLocal`, `LocalToRemote`,
    /// `RemoteToLocal`, `RemoteToRemote`).
    pub(crate) fn sftp_pump_transfers(&mut self, cx: &mut Context<Self>) {
        let scope = self.current_vault_scope();
        let ready = self.sftp_controller.transfers.dispatch_ready(&scope);
        for (intent, attempt_id, ctrl) in ready {
            let controller = self.sftp_controller.clone();
            let job_id = intent.job_id;
            let src_ep = intent.source_endpoint.clone();
            let dst_ep = intent.destination_endpoint.clone();
            let src_disp = intent.source_path.display();
            let dst_disp = intent.destination_path.display();
            let scope_clone = scope.clone();

            let task = cx.background_executor().spawn(async move {
                controller.execute_dispatched_job(&intent, attempt_id, &ctrl)
            });

            let controller_done = self.sftp_controller.clone();
            cx.spawn(async move |app, cx| {
                let res = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Err(SftpError::ConflictRequired(details)) = &res {
                        controller_done.transfers.mark_awaiting_conflict(
                            job_id,
                            attempt_id,
                            shellumi_core::ConflictContext {
                                job_id,
                                route: shellumi_core::TransferRoute::from_endpoints(
                                    &src_ep, &dst_ep,
                                ),
                                source_display: details.source_path.clone(),
                                destination_display: details.destination_path.clone(),
                                source_size: details.source_size,
                                destination_size: details.destination_size,
                                source_modified: None,
                                destination_modified: details.destination_mtime,
                                source_kind: FileKind::File,
                                destination_kind: details.item_kind.clone(),
                                is_type_mismatch: matches!(
                                    details.reason,
                                    shellumi_core::ConflictReason::TypeMismatch
                                ),
                                relative_path: Some(details.relative_path.clone()),
                            },
                        );
                    } else {
                        controller_done
                            .transfers
                            .complete_attempt(job_id, attempt_id, res);
                    }
                    let _ = (src_disp, dst_disp);
                    if controller_done.auth_gate.is_authorized(&scope_clone) {
                        app.refresh_sftp_pane(true, cx);
                        app.refresh_sftp_pane(false, cx);
                        app.sftp_pump_transfers(cx);
                        app.sftp_surface_next_conflict(cx);
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// Surface the next queued `AwaitingConflict` job if no modal is currently open.
    pub(crate) fn sftp_surface_next_conflict(&mut self, cx: &mut Context<Self>) {
        if self.sftp_modal.is_some() {
            return;
        }
        let scope = self.current_vault_scope();
        if !self.sftp_controller.auth_gate.is_authorized(&scope) {
            return;
        }
        if let Some(snap) = self
            .sftp_controller
            .authorized_snapshots()
            .into_iter()
            .find(|s| s.state == shellumi_core::TransferJobState::AwaitingConflict)
        {
            if let Some(ctx) = snap.conflict {
                self.sftp_modal = Some(SftpModalState {
                    is_left: true,
                    endpoint_generation: 0,
                    target_dir: String::new(),
                    action: SftpModalAction::Conflict {
                        job_id: snap.intent.job_id,
                        source_display: ctx.source_display,
                        destination_display: ctx.destination_display,
                        apply_to_all: false,
                    },
                    error: None,
                });
                cx.notify();
            }
        }
    }

    pub(crate) fn sftp_resolve_conflict(
        &mut self,
        job_id: shellumi_core::JobId,
        action: shellumi_core::ConflictAction,
        apply_to_all: bool,
        cx: &mut Context<Self>,
    ) {
        self.sftp_modal = None;
        let scope = self.current_vault_scope();
        let decision = shellumi_core::ConflictDecision {
            action,
            apply_to_all,
        };
        if let Err(err) = self
            .sftp_controller
            .transfers
            .resolve_conflict(job_id, decision, &scope)
        {
            self.desktop.message = Some(format!("Conflict resolution failed: {err}"));
        } else {
            self.sftp_pump_transfers(cx);
        }
        self.sftp_surface_next_conflict(cx);
        cx.notify();
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
        let scope = self.current_vault_scope();
        let (host, endpoint_id, current_path, show_hidden, generation) = {
            let pane = if is_left {
                &mut self.sftp_left_pane
            } else {
                &mut self.sftp_right_pane
            };
            pane.listing_generation += 1;
            pane.loading = true;
            pane.error = None;
            (
                pane.host.clone(),
                pane.endpoint_id,
                pane.current_path.clone(),
                pane.show_hidden,
                pane.listing_generation,
            )
        };
        cx.notify();

        match host {
            Some(SftpPaneHost::Local) => {
                let req_path = current_path.clone();
                let task = cx.background_executor().spawn(async move {
                    let cap = shellumi_core::local_disk_capacity(std::path::Path::new(&req_path));
                    let res = list_local_directory(&req_path, show_hidden);
                    (cap, res)
                });
                cx.spawn(async move |app, cx| {
                    let (cap, res) = task.await;
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
                    });
                })
                .detach();
            }
            Some(SftpPaneHost::Remote(saved_host)) => {
                let controller = self.sftp_controller.clone();
                let Ok(lease) = controller.register_lease(&scope) else {
                    let pane = if is_left {
                        &mut self.sftp_left_pane
                    } else {
                        &mut self.sftp_right_pane
                    };
                    pane.loading = false;
                    pane.error = Some("Vault is locked or inactive.".into());
                    cx.notify();
                    return;
                };
                let _op_id = lease.op_id;
                let req_path = current_path.clone();
                let dest = saved_host.destination.clone();
                let database = self.database.clone();
                let active_vault_id = self.active_vault_id.clone();
                let active_vault_key = self.active_vault_key();
                let host_id = saved_host.id;

                let task = cx.background_executor().spawn(async move {
                    let _lease = lease;
                    _lease.check()?;
                    let mut entries = if let Some(sess_arc) =
                        endpoint_id.and_then(|id| controller.get_session(id))
                    {
                        let mut sess = sess_arc.lock().map_err(|_| {
                            SftpError::UnexpectedResponse("session lock poisoned".into())
                        })?;
                        sess.with_browse_client(|c| c.list_remote_directory(&req_path))?
                    } else {
                        _lease.check()?;
                        let mut client = connect_saved_sftp(
                            database,
                            Some(&active_vault_id),
                            active_vault_key.as_ref(),
                            host_id,
                            &dest,
                            &SftpConnectOptions::default(),
                        )?;
                        client.list_remote_directory(&req_path)?
                    };
                    _lease.check()?;
                    if !show_hidden {
                        entries.retain(|e| !e.is_hidden());
                    }
                    Ok::<Vec<FileEntry>, SftpError>(entries)
                });

                let controller_ui = self.sftp_controller.clone();
                cx.spawn(async move |app, cx| {
                    let res = task.await;
                    let _ = app.update(cx, |app, cx| {
                        if !controller_ui.auth_gate.is_authorized(&scope) {
                            return;
                        }
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
                SftpModalAction::Conflict { job_id, .. } => {
                    let _ = self.sftp_controller.transfers.resolve_conflict(
                        *job_id,
                        shellumi_core::ConflictDecision {
                            action: shellumi_core::ConflictAction::Cancel,
                            apply_to_all: false,
                        },
                        &self.current_vault_scope(),
                    );
                    cx.notify();
                    return;
                }
            };
            let scope = self.current_vault_scope();
            let controller = self.sftp_controller.clone();
            let endpoint_id = if is_left {
                self.sftp_left_pane.endpoint_id
            } else {
                self.sftp_right_pane.endpoint_id
            };
            let Ok((op_id, ctrl)) = controller.register_op(&scope) else {
                self.desktop.message = Some("Vault is locked or inactive.".into());
                cx.notify();
                return;
            };
            let dest = host.destination.clone();
            self.desktop.message = Some("Applying SFTP operation…".into());
            let database = self.database.clone();
            let active_vault_id = self.active_vault_id.clone();
            let active_vault_key = self.active_vault_key();
            let host_id = host.id;
            let scope_worker = scope.clone();
            let task = cx.background_executor().spawn(async move {
                if !controller.is_op_authorized(op_id, &scope_worker) {
                    return Err(SftpError::Cancelled);
                }
                let run_op = |client: &mut SftpClient| match &operation {
                    RemoteFileOperation::Rename {
                        source,
                        destination,
                    } => client.rename(source, destination),
                    RemoteFileOperation::Delete { path, is_dir } => {
                        if *is_dir {
                            let rp = shellumi_core::RemotePath::from_posix_str(path);
                            shellumi_core::delete_remote_recursive(client, &rp, &ctrl).map(|_| ())
                        } else {
                            client.remove_file(path)
                        }
                    }
                    RemoteFileOperation::Mkdir { path } => client.mkdir(path),
                    RemoteFileOperation::CreateFile { path } => {
                        client.create_exclusive_file(path, 0o644)
                    }
                    RemoteFileOperation::Permissions { path, mode } => {
                        client.set_permissions(path, *mode)
                    }
                };
                let res =
                    if let Some(sess_arc) = endpoint_id.and_then(|id| controller.get_session(id)) {
                        let mut sess = sess_arc.lock().map_err(|_| {
                            SftpError::UnexpectedResponse("session lock poisoned".into())
                        })?;
                        sess.with_browse_client(run_op)
                    } else {
                        let mut client = connect_saved_sftp(
                            database,
                            Some(&active_vault_id),
                            active_vault_key.as_ref(),
                            host_id,
                            &dest,
                            &SftpConnectOptions::default(),
                        )?;
                        run_op(&mut client)
                    };
                controller.complete_op(op_id);
                res
            });
            let controller_ui = self.sftp_controller.clone();
            cx.spawn(async move |app, cx| {
                let result = task.await;
                let _ = app.update(cx, |app, cx| {
                    if !controller_ui.auth_gate.is_authorized(&scope) {
                        return;
                    }
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
        if let SftpModalAction::Conflict { job_id, .. } = &modal.action {
            let _ = self.sftp_controller.transfers.resolve_conflict(
                *job_id,
                shellumi_core::ConflictDecision {
                    action: shellumi_core::ConflictAction::Cancel,
                    apply_to_all: false,
                },
                &self.current_vault_scope(),
            );
            cx.notify();
            return;
        }
        let local_op = match modal.action {
            SftpModalAction::Rename {
                entry_name,
                entry_path,
                input,
            } => {
                let name = input.read(cx).value().trim().to_owned();
                if !PathUtils::is_safe_file_name(&name) {
                    self.desktop.message =
                        Some("File operation failed: Enter a valid single filename.".into());
                    cx.notify();
                    return;
                }
                Ok(Some(("rename", entry_name, entry_path, name, 0u32)))
            }
            SftpModalAction::Delete {
                entry_name,
                entry_path,
                ..
            } => Ok(Some(("delete", entry_name, entry_path, String::new(), 0))),
            SftpModalAction::NewFolder { input } => {
                let name = input.read(cx).value().trim().to_owned();
                if !PathUtils::is_safe_file_name(&name) {
                    self.desktop.message =
                        Some("File operation failed: Enter a valid single folder name.".into());
                    cx.notify();
                    return;
                }
                Ok(Some(("mkdir", name.clone(), curr_dir.clone(), name, 0)))
            }
            SftpModalAction::NewFile { input } => {
                let name = input.read(cx).value().trim().to_owned();
                if !PathUtils::is_safe_file_name(&name) {
                    self.desktop.message =
                        Some("File operation failed: Enter a valid single filename.".into());
                    cx.notify();
                    return;
                }
                Ok(Some(("mkfile", name.clone(), curr_dir.clone(), name, 0)))
            }
            SftpModalAction::EditPermissions {
                entry_name,
                entry_path,
                input,
            } => {
                let value = input.read(cx).value();
                if let Some(mode) = parse_sftp_permissions(value) {
                    Ok(Some(("chmod", entry_name, entry_path, String::new(), mode)))
                } else {
                    Err("Enter octal permissions, for example 0755.".to_string())
                }
            }
            SftpModalAction::Properties { .. } | SftpModalAction::Conflict { .. } => Ok(None),
        };
        match local_op {
            Err(err) => {
                self.desktop.message = Some(format!("File operation failed: {err}"));
                cx.notify();
            }
            Ok(None) => {
                cx.notify();
            }
            Ok(Some((kind, entry_name, entry_path, new_name, mode))) => {
                let task = cx.background_executor().spawn(async move {
                    match kind {
                        "rename" => {
                            if new_name == entry_name {
                                Ok("Name unchanged.".to_string())
                            } else {
                                let src = std::path::PathBuf::from(&entry_path);
                                let dst = src.with_file_name(&new_name);
                                if std::fs::symlink_metadata(&dst).is_ok() {
                                    Err("The destination already exists.".to_string())
                                } else {
                                    std::fs::rename(src, dst)
                                        .map(|_| format!("Renamed to '{new_name}'."))
                                        .map_err(|e| e.to_string())
                                }
                            }
                        }
                        "delete" => {
                            let ctrl = shellumi_core::TransferControl::default();
                            shellumi_core::delete_local_recursive(
                                std::path::Path::new(&entry_path),
                                &ctrl,
                            )
                            .map(|count| format!("Deleted '{entry_name}' ({count} item(s))."))
                            .map_err(|e| e.to_string())
                        }
                        "mkdir" => {
                            let target = shellumi_core::local_join_child(
                                std::path::Path::new(&entry_path),
                                &new_name,
                            )
                            .map_err(|e| e.to_string())?;
                            std::fs::create_dir(target)
                                .map(|_| format!("Created folder '{new_name}'."))
                                .map_err(|e| e.to_string())
                        }
                        "mkfile" => {
                            let target = shellumi_core::local_join_child(
                                std::path::Path::new(&entry_path),
                                &new_name,
                            )
                            .map_err(|e| e.to_string())?;
                            std::fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(target)
                                .map(|_| format!("Created file '{new_name}'."))
                                .map_err(|e| e.to_string())
                        }
                        "chmod" => {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                std::fs::set_permissions(
                                    entry_path,
                                    std::fs::Permissions::from_mode(mode),
                                )
                                .map(|_| format!("Permissions updated for '{entry_name}'."))
                                .map_err(|e| e.to_string())
                            }
                            #[cfg(not(unix))]
                            {
                                let _ = (entry_name, entry_path, mode);
                                Err("Changing Unix permissions is unsupported on this platform."
                                    .to_string())
                            }
                        }
                        _ => Ok(String::new()),
                    }
                });
                cx.spawn(async move |app, cx| {
                    let result = task.await;
                    let _ = app.update(cx, |app, cx| {
                        app.desktop.message = Some(match result {
                            Ok(msg) => msg,
                            Err(err) => format!("File operation failed: {err}"),
                        });
                        app.refresh_sftp_pane(is_left, cx);
                        cx.notify();
                    });
                })
                .detach();
            }
        }
    }
}

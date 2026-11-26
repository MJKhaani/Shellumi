use super::*;
impl crate::ConnectionOpRegistry {
    pub(crate) fn bump_host_revision(&mut self, host_id: i64) {
        let entry = self.host_revisions.entry(host_id).or_insert(0);
        *entry = entry.wrapping_add(1);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register(
        &mut self,
        window_id: gpui::WindowId,
        vault_id: String,
        vault_load_generation: u64,
        host_id: i64,
        tab_id: Option<u64>,
        connection_generation: Option<u64>,
        credential_edit_generation: u64,
    ) -> crate::ConnectionOperationContext {
        for rec in self.ops.values_mut() {
            if rec.state == crate::ConnectionOperationState::Pending
                && rec.ctx.vault_id == vault_id
                && ((tab_id.is_some() && rec.ctx.tab_id == tab_id)
                    || (tab_id.is_none() && rec.ctx.tab_id.is_none() && rec.ctx.host_id == host_id))
            {
                rec.state = crate::ConnectionOperationState::Superseded;
            }
        }
        let op_id = self.next_op_id;
        self.next_op_id = self.next_op_id.wrapping_add(1);
        let host_revision = self.host_revisions.get(&host_id).copied().unwrap_or(0);
        let ctx = crate::ConnectionOperationContext {
            window_id,
            vault_id,
            vault_load_generation,
            host_id,
            host_revision,
            operation_id: op_id,
            tab_id,
            connection_generation,
            credential_edit_generation,
        };
        self.ops.insert(
            op_id,
            crate::ConnectionOperationRecord {
                ctx: ctx.clone(),
                state: crate::ConnectionOperationState::Pending,
            },
        );
        if self.ops.len() > 256 {
            self.ops
                .retain(|_, v| v.state == crate::ConnectionOperationState::Pending);
        }
        ctx
    }

    pub(crate) fn cancel_for_vault(&mut self) {
        for rec in self.ops.values_mut() {
            if rec.state == crate::ConnectionOperationState::Pending {
                rec.state = crate::ConnectionOperationState::Canceled;
            }
        }
    }

    pub(crate) fn cancel_for_tab(&mut self, tab_id: u64) {
        for rec in self.ops.values_mut() {
            if rec.state == crate::ConnectionOperationState::Pending
                && rec.ctx.tab_id == Some(tab_id)
            {
                rec.state = crate::ConnectionOperationState::Canceled;
            }
        }
    }

    pub(crate) fn cancel_for_host(&mut self, host_id: i64) {
        for rec in self.ops.values_mut() {
            if rec.state == crate::ConnectionOperationState::Pending && rec.ctx.host_id == host_id {
                rec.state = crate::ConnectionOperationState::Canceled;
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn cancel_for_window(&mut self, window_id: gpui::WindowId) {
        for rec in self.ops.values_mut() {
            if rec.state == crate::ConnectionOperationState::Pending
                && rec.ctx.window_id == window_id
            {
                rec.state = crate::ConnectionOperationState::Canceled;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn authorize_and_consume(
        &mut self,
        ctx: &crate::ConnectionOperationContext,
        current_window: Option<gpui::WindowId>,
        active_vault_id: &str,
        vault_load_generation: u64,
        vault_locked: bool,
        host_exists: bool,
        tab_state: Option<(bool, Option<u64>, u64)>,
    ) -> Result<(), crate::ConnectionOperationError> {
        let Some(rec) = self.ops.get(&ctx.operation_id) else {
            return Err(crate::ConnectionOperationError(
                "connection operation not registered",
            ));
        };
        match rec.state {
            crate::ConnectionOperationState::Pending => {}
            crate::ConnectionOperationState::Consumed => {
                return Err(crate::ConnectionOperationError(
                    "connection operation already consumed",
                ));
            }
            crate::ConnectionOperationState::Canceled => {
                return Err(crate::ConnectionOperationError(
                    "connection operation was canceled",
                ));
            }
            crate::ConnectionOperationState::Superseded => {
                return Err(crate::ConnectionOperationError(
                    "connection operation was superseded by a newer request",
                ));
            }
        }
        if current_window.is_some_and(|w| w != ctx.window_id) {
            return Err(crate::ConnectionOperationError(
                "originating window closed or changed",
            ));
        }
        if active_vault_id != ctx.vault_id {
            return Err(crate::ConnectionOperationError("active vault changed"));
        }
        if vault_load_generation != ctx.vault_load_generation {
            return Err(crate::ConnectionOperationError("vault reloaded or locked"));
        }
        if vault_locked {
            return Err(crate::ConnectionOperationError("vault is locked"));
        }
        if !host_exists {
            return Err(crate::ConnectionOperationError(
                "host no longer exists in active vault",
            ));
        }
        let current_host_rev = self.host_revisions.get(&ctx.host_id).copied().unwrap_or(0);
        if current_host_rev != ctx.host_revision {
            return Err(crate::ConnectionOperationError(
                "host configuration changed while credential resolution was pending",
            ));
        }
        if ctx.tab_id.is_some() {
            let Some((tab_exists, rd_conn_gen, rd_cred_edit_gen)) = tab_state else {
                return Err(crate::ConnectionOperationError("session tab closed"));
            };
            if !tab_exists {
                return Err(crate::ConnectionOperationError("session tab closed"));
            }
            if let Some(expected_gen) = ctx.connection_generation {
                if rd_conn_gen != Some(expected_gen) {
                    return Err(crate::ConnectionOperationError(
                        "session reconnected or disconnected",
                    ));
                }
            }
            if rd_cred_edit_gen != ctx.credential_edit_generation {
                return Err(crate::ConnectionOperationError(
                    "credentials edited manually while resolution was pending",
                ));
            }
        }
        if let Some(rec) = self.ops.get_mut(&ctx.operation_id) {
            rec.state = crate::ConnectionOperationState::Consumed;
        }
        Ok(())
    }
}

impl Shellumi {
    pub(crate) fn register_connection_op(
        &mut self,
        host_id: i64,
        window: &Window,
        tab_id: Option<u64>,
        connection_generation: Option<u64>,
        credential_edit_generation: u64,
    ) -> crate::ConnectionOperationContext {
        self.connection_ops.register(
            window.window_handle().window_id(),
            self.active_vault_id.clone(),
            self.vault_load_generation,
            host_id,
            tab_id,
            connection_generation,
            credential_edit_generation,
        )
    }

    pub(crate) fn authorize_and_consume_connection_op(
        &mut self,
        ctx: &crate::ConnectionOperationContext,
        current_window: Option<gpui::WindowId>,
    ) -> Result<(), crate::ConnectionOperationError> {
        let host_exists = match &self.hosts {
            HostState::Loaded(hosts) => hosts.iter().any(|h| h.id == ctx.host_id),
            _ => false,
        };
        let tab_state = ctx.tab_id.map(|tid| {
            let tab = self.tabs.iter().find(|t| t.id == tid);
            let tab_exists = tab.is_some();
            let rd_conn_gen = tab
                .and_then(|t| t.remote_desktop_state.as_ref())
                .map(|rd| rd.connection_generation);
            let edit_gen = self.rd_credential_edit_gen.get(&tid).copied().unwrap_or(0);
            (tab_exists, rd_conn_gen, edit_gen)
        });
        self.connection_ops.authorize_and_consume(
            ctx,
            current_window,
            &self.active_vault_id,
            self.vault_load_generation,
            self.unlock_vault_modal.is_some(),
            host_exists,
            tab_state,
        )
    }

    pub(super) fn launch_saved_connection(
        &mut self,
        host: &SavedHost,
        sftp: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let op_ctx = self.register_connection_op(host.id, window, None, None, 0);
        let host = host.clone();
        let host_id = host.id;
        let active_vault_id = op_ctx.vault_id.clone();
        let active_key = self.active_vault_key();
        let handle = window.window_handle();
        self.desktop.message = Some("Resolving connection identity…".into());
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(shellumi_storage::desktop::resolve_connection_with_key(
                &path,
                host_id,
                Some(&active_vault_id),
                active_key.as_ref(),
            ))
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = app.update(cx, |app, cx| {
                    if let Err(reason) = app.authorize_and_consume_connection_op(
                        &op_ctx,
                        Some(window.window_handle().window_id()),
                    ) {
                        tracing::warn!(
                            host_id = op_ctx.host_id,
                            operation_id = op_ctx.operation_id,
                            reason = %reason,
                            "Discarded stale SSH/SFTP connection resolution"
                        );
                        return;
                    }
                    let result = result.and_then(|plan| {
                        let program = if sftp { "sftp" } else { "ssh" };
                        let arguments = if sftp {
                            plan.sftp_arguments.clone()
                        } else {
                            plan.ssh_arguments.clone()
                        };
                        Ok((askpass::command(program, arguments, &plan)?, plan))
                    });
                    match result {
                        Err(error) => {
                            app.desktop.message =
                                Some(format!("Connection could not start: {error}"));
                            cx.notify();
                        }
                        Ok((command, plan)) => {
                            app.desktop.message = None;
                            let helper_environment = command
                                .env
                                .iter()
                                .map(|(key, value)| (key.clone(), value.clone()))
                                .collect();
                            let startup = if sftp {
                                None
                            } else {
                                host.startup_snippet_id.map(|id| {
                                    app.snippets
                                        .iter()
                                        .find(|snippet| snippet.id == id)
                                        .map(|snippet| snippet.name.clone())
                                        .unwrap_or_else(|| format!("#{id}"))
                                })
                            };
                            app.start_session_with_startup(
                                host.name.clone(),
                                if sftp {
                                    SessionKind::Sftp
                                } else {
                                    SessionKind::Ssh
                                },
                                command,
                                startup,
                                window,
                                cx,
                            );
                            if sftp {
                                if let Some(browser) =
                                    app.tabs.last_mut().and_then(|tab| tab.sftp_state.as_mut())
                                {
                                    browser.destination = Some(plan.destination.clone());
                                    browser.jump_host = plan.jump_host.clone();
                                    browser.connection_options = SftpConnectOptions {
                                        port: Some(plan.destination.port().get()),
                                        identity_file: plan.identity_file.clone(),
                                        certificate_file: plan.certificate_file.clone(),
                                        extra_ssh_options: plan.authentication_options.clone(),
                                        environment: helper_environment,
                                        ..Default::default()
                                    };
                                    // Stored secrets use a separate askpass process and require existing host trust.
                                    browser.mode = SftpTabMode::DualPane;
                                }
                                let index = app.tabs.len().saturating_sub(1);
                                app.sftp_refresh_remote(index, cx);
                            }
                        }
                    }
                });
            });
        })
        .detach();
    }
}

impl Shellumi {
    pub(super) fn launch_saved_tunnel(
        &mut self,
        host: SavedHost,
        name: String,
        mut arguments: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(database) = self.active_vault_db_path() else {
            return;
        };
        let op_ctx = self.register_connection_op(host.id, window, None, None, 0);
        let active_vault_id = op_ctx.vault_id.clone();
        let active_key = self.active_vault_key();
        let handle = window.window_handle();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            let plan = runtime.block_on(shellumi_storage::desktop::resolve_connection_with_key(
                &database,
                host.id,
                Some(&active_vault_id),
                active_key.as_ref(),
            ))?;
            arguments.extend(plan.ssh_arguments.clone());
            askpass::command("ssh", arguments, &plan)
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = app.update(cx, |app, cx| {
                    if let Err(reason) = app.authorize_and_consume_connection_op(
                        &op_ctx,
                        Some(window.window_handle().window_id()),
                    ) {
                        tracing::warn!(
                            host_id = op_ctx.host_id,
                            operation_id = op_ctx.operation_id,
                            reason = %reason,
                            "Discarded stale tunnel credential resolution"
                        );
                        return;
                    }
                    match result {
                        Ok(command) => {
                            app.start_session(name, SessionKind::Ssh, command, window, cx)
                        }
                        Err(error) => {
                            app.desktop.message = Some(format!("Tunnel could not start: {error}"));
                            cx.notify();
                        }
                    }
                });
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use crate::{ConnectionOpRegistry, ConnectionOperationState};

    #[test]
    fn test_connection_op_supersession_and_independent_rd_sessions() {
        let mut reg = ConnectionOpRegistry {
            next_op_id: 1,
            ..Default::default()
        };
        let w1 = gpui::WindowId::from(1);
        let op_a = reg.register(w1, "vault-a".into(), 1, 1, None, None, 0);
        let op_b = reg.register(w1, "vault-a".into(), 1, 1, None, None, 0);
        assert_eq!(
            reg.ops.get(&op_a.operation_id).unwrap().state,
            ConnectionOperationState::Superseded
        );
        assert!(reg
            .authorize_and_consume(&op_a, Some(w1), "vault-a", 1, false, true, None)
            .is_err());
        assert!(reg
            .authorize_and_consume(&op_b, Some(w1), "vault-a", 1, false, true, None)
            .is_ok());

        let rd1 = reg.register(w1, "vault-a".into(), 1, 1, Some(10), Some(0), 0);
        let rd2 = reg.register(w1, "vault-a".into(), 1, 1, Some(11), Some(0), 0);
        assert!(reg
            .authorize_and_consume(
                &rd2,
                Some(w1),
                "vault-a",
                1,
                false,
                true,
                Some((true, Some(0), 0))
            )
            .is_ok());
        assert!(reg
            .authorize_and_consume(
                &rd1,
                Some(w1),
                "vault-a",
                1,
                false,
                true,
                Some((true, Some(0), 0))
            )
            .is_ok());
    }

    #[test]
    fn test_connection_op_rejects_vault_switch_lock_host_edit_reconnect_manual_edit_and_window_close(
    ) {
        let mut reg = ConnectionOpRegistry {
            next_op_id: 1,
            ..Default::default()
        };
        let w1 = gpui::WindowId::from(1);
        let w2 = gpui::WindowId::from(2);

        let op = reg.register(w1, "vault-a".into(), 1, 1, Some(20), Some(0), 0);
        assert!(reg
            .authorize_and_consume(
                &op,
                Some(w1),
                "vault-b",
                1,
                false,
                true,
                Some((true, Some(0), 0))
            )
            .is_err());
        assert!(reg
            .authorize_and_consume(
                &op,
                Some(w1),
                "vault-a",
                1,
                true,
                true,
                Some((true, Some(0), 0))
            )
            .is_err());
        reg.bump_host_revision(1);
        assert!(reg
            .authorize_and_consume(
                &op,
                Some(w1),
                "vault-a",
                1,
                false,
                true,
                Some((true, Some(0), 0))
            )
            .is_err());

        let op2 = reg.register(w1, "vault-a".into(), 2, 1, Some(20), Some(0), 0);
        assert!(reg
            .authorize_and_consume(
                &op2,
                Some(w1),
                "vault-a",
                2,
                false,
                true,
                Some((true, Some(1), 0))
            )
            .is_err());
        assert!(reg
            .authorize_and_consume(
                &op2,
                Some(w1),
                "vault-a",
                2,
                false,
                true,
                Some((true, Some(0), 1))
            )
            .is_err());
        assert!(reg
            .authorize_and_consume(
                &op2,
                Some(w2),
                "vault-a",
                2,
                false,
                true,
                Some((true, Some(0), 0))
            )
            .is_err());

        reg.cancel_for_window(w1);
        assert!(reg
            .authorize_and_consume(
                &op2,
                Some(w1),
                "vault-a",
                2,
                false,
                true,
                Some((true, Some(0), 0))
            )
            .is_err());
    }
}

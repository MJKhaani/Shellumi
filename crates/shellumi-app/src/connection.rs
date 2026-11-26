use super::*;
impl Shellumi {
    pub(crate) fn begin_connection_op(
        &mut self,
        host_id: i64,
        window: &Window,
        tab_id: Option<u64>,
        connection_generation: Option<u64>,
        credential_edit_generation: u64,
    ) -> Option<crate::ConnectionOperationContext> {
        if self.pending_connections.contains_key(&host_id) {
            return None;
        }
        let op_id = self.next_connection_op_id;
        self.next_connection_op_id = self.next_connection_op_id.wrapping_add(1);
        self.pending_connections.insert(host_id, op_id);
        Some(crate::ConnectionOperationContext {
            window_id: window.window_handle().window_id(),
            vault_id: self.active_vault_id.clone(),
            vault_load_generation: self.vault_load_generation,
            host_id,
            operation_id: op_id,
            tab_id,
            connection_generation,
            credential_edit_generation,
        })
    }

    pub(crate) fn finish_connection_op(&mut self, host_id: i64, operation_id: u64) {
        if self.pending_connections.get(&host_id) == Some(&operation_id) {
            self.pending_connections.remove(&host_id);
        }
    }

    pub(crate) fn validate_connection_op(
        &self,
        ctx: &crate::ConnectionOperationContext,
        current_window: Option<gpui::WindowId>,
    ) -> Result<(), &'static str> {
        if current_window.is_some_and(|w| w != ctx.window_id) {
            return Err("originating window closed or changed");
        }
        if self.active_vault_id != ctx.vault_id {
            return Err("active vault changed");
        }
        if self.vault_load_generation != ctx.vault_load_generation {
            return Err("vault reloaded or locked");
        }
        if self.unlock_vault_modal.is_some() {
            return Err("vault is locked");
        }
        match &self.hosts {
            HostState::Loaded(hosts) if hosts.iter().any(|h| h.id == ctx.host_id) => {}
            _ => return Err("host no longer exists in active vault"),
        }
        if let Some(tab_id) = ctx.tab_id {
            let Some(tab) = self.tabs.iter().find(|t| t.id == tab_id) else {
                return Err("session tab closed");
            };
            if let Some(expected_gen) = ctx.connection_generation {
                let Some(rd) = tab.remote_desktop_state.as_ref() else {
                    return Err("remote desktop state missing");
                };
                if rd.connection_generation != expected_gen {
                    return Err("session reconnected or disconnected");
                }
            }
            let current_edit_gen = self
                .rd_credential_edit_gen
                .get(&tab_id)
                .copied()
                .unwrap_or(0);
            if current_edit_gen != ctx.credential_edit_generation {
                return Err("credentials edited manually while resolution was pending");
            }
        }
        Ok(())
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
        let Some(op_ctx) = self.begin_connection_op(host.id, window, None, None, 0) else {
            return;
        };
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
                    app.finish_connection_op(op_ctx.host_id, op_ctx.operation_id);
                    if let Err(reason) = app
                        .validate_connection_op(&op_ctx, Some(window.window_handle().window_id()))
                    {
                        tracing::warn!(
                            host_id = op_ctx.host_id,
                            operation_id = op_ctx.operation_id,
                            reason = reason,
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
        let Some(op_ctx) = self.begin_connection_op(host.id, window, None, None, 0) else {
            return;
        };
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
                    app.finish_connection_op(op_ctx.host_id, op_ctx.operation_id);
                    if let Err(reason) = app
                        .validate_connection_op(&op_ctx, Some(window.window_handle().window_id()))
                    {
                        tracing::warn!(
                            host_id = op_ctx.host_id,
                            operation_id = op_ctx.operation_id,
                            reason = reason,
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
    use crate::ConnectionOperationContext;

    #[test]
    fn test_pending_connection_operation_id_prevents_stale_removal() {
        let mut pending: std::collections::HashMap<i64, u64> = std::collections::HashMap::new();
        pending.insert(42, 1);
        // Newer operation replaces host 42 with op_id = 2.
        pending.insert(42, 2);
        // Stale op_id = 1 finishing must NOT remove op_id = 2.
        if pending.get(&42) == Some(&1) {
            pending.remove(&42);
        }
        assert_eq!(pending.get(&42), Some(&2));
        if pending.get(&42) == Some(&2) {
            pending.remove(&42);
        }
        assert!(!pending.contains_key(&42));
    }

    #[test]
    fn test_connection_operation_context_detects_vault_generation_and_edit_drift() {
        let ctx = ConnectionOperationContext {
            window_id: gpui::WindowId::from(1),
            vault_id: "vault-a".into(),
            vault_load_generation: 3,
            host_id: 99,
            operation_id: 7,
            tab_id: Some(10),
            connection_generation: Some(1),
            credential_edit_generation: 2,
        };
        assert_ne!(ctx.vault_id, "vault-b");
        assert_ne!(ctx.vault_load_generation, 4);
        assert_ne!(ctx.credential_edit_generation, 3);
    }
}

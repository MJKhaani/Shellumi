use super::*;
impl Shellumi {
    pub(super) fn launch_saved_connection(
        &mut self,
        host: &SavedHost,
        sftp: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.database.clone() else {
            return;
        };
        if !self.connecting_hosts.insert(host.id) {
            return;
        }
        let host = host.clone();
        let host_id = host.id;
        let handle = window.window_handle();
        self.desktop.message = Some("Resolving connection identity…".into());
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(shellumi_storage::desktop::resolve_connection(
                &path, host_id,
            ))
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = app.update(cx, |app, cx| {
                    app.connecting_hosts.remove(&host_id);
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
        let Some(database) = self.database.clone() else {
            return;
        };
        let handle = window.window_handle();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            let plan = runtime.block_on(shellumi_storage::desktop::resolve_connection(
                &database, host.id,
            ))?;
            arguments.extend(plan.ssh_arguments.clone());
            askpass::command("ssh", arguments, &plan)
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = app.update(cx, |app, cx| match result {
                    Ok(command) => app.start_session(name, SessionKind::Ssh, command, window, cx),
                    Err(error) => {
                        app.desktop.message = Some(format!("Tunnel could not start: {error}"));
                        cx.notify();
                    }
                });
            });
        })
        .detach();
    }
}

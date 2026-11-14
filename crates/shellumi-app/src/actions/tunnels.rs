//! Tunnels actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_tunnel_modal(&mut self, cx: &mut Context<Self>) {
        let name_input = cx.new(|cx| TextInput::new(cx, "", "Tunnel Name (e.g. Postgres Dev)"));
        let bind_port_input = cx.new(|cx| TextInput::new(cx, "", "Bind Port (e.g. 5432)"));
        let target_host_input = cx.new(|cx| TextInput::new(cx, "", "Target Host (e.g. 10.0.0.5)"));
        let target_port_input = cx.new(|cx| TextInput::new(cx, "", "Target Port (e.g. 5432)"));

        self.tunnel_modal = Some(TunnelModalState {
            editing_id: None,
            tunnels: Vec::new(),
            selected_index: 0,
            active_states: std::collections::HashMap::new(),
            name_input,
            bind_port_input,
            target_host_input,
            target_port_input,
            forwarding_type: ForwardingType::Local,
            selected_host_id: match &self.hosts {
                HostState::Loaded(hosts) => {
                    hosts.iter().find(|host| host.is_ssh()).map(|host| host.id)
                }
                _ => None,
            },
            auto_start: false,
            error: None,
            status_message: None,
            is_loading: true,
        });
        cx.notify();

        if let Some(path) = self.database.clone() {
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    store.list_tunnels().await.map_err(|e| e.to_string())
                })
            });

            cx.spawn(async move |app, cx| {
                let res = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Some(ref mut modal) = app.tunnel_modal {
                        modal.is_loading = false;
                        match res {
                            Ok(tunnels) => {
                                for t in &tunnels {
                                    modal.active_states.entry(t.id).or_insert_with(|| {
                                        ActiveTunnelState::new(
                                            t.id,
                                            t.host_id,
                                            t.name.clone(),
                                            t.rule.clone(),
                                            t.auto_start,
                                        )
                                    });
                                }
                                modal.tunnels = tunnels;
                            }
                            Err(e) => {
                                modal.error = Some(e);
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        } else if let Some(ref mut modal) = self.tunnel_modal {
            modal.is_loading = false;
        }
    }

    pub(crate) fn dismiss_tunnel_modal(&mut self, cx: &mut Context<Self>) {
        self.tunnel_modal = None;
        cx.notify();
    }

    pub(crate) fn select_tunnel_modal_item(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(ref mut modal) = self.tunnel_modal {
            if idx < modal.tunnels.len() {
                modal.selected_index = idx;
                cx.notify();
            }
        }
    }

    pub(crate) fn set_tunnel_forwarding_type(
        &mut self,
        ftype: ForwardingType,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref mut modal) = self.tunnel_modal {
            modal.forwarding_type = ftype;
            cx.notify();
        }
    }

    pub(crate) fn toggle_tunnel_state(
        &mut self,
        tunnel_id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tunnel) = self
            .tunnel_modal
            .as_ref()
            .and_then(|modal| modal.tunnels.iter().find(|tunnel| tunnel.id == tunnel_id))
            .cloned()
        else {
            return;
        };
        let session_name = format!("Tunnel: {}", tunnel.name);
        if let Some(tab_id) = self
            .tabs
            .iter()
            .find(|tab| tab.name == session_name)
            .map(|tab| tab.id)
        {
            self.close_tab(tab_id, window, cx);
            return;
        }
        let host = match &self.hosts {
            HostState::Loaded(hosts) => hosts
                .iter()
                .find(|host| Some(host.id) == tunnel.host_id)
                .cloned(),
            _ => None,
        };
        let Some(host) = host else {
            if let Some(modal) = &mut self.tunnel_modal {
                modal.error = Some("Choose a saved SSH host for this tunnel.".into());
            }
            cx.notify();
            return;
        };
        let (flag, value) = tunnel.rule.openssh_argument();
        let args = vec![
            "-N".to_string(),
            "-o".into(),
            "ExitOnForwardFailure=yes".into(),
            flag.into(),
            value,
        ];
        self.tunnel_modal = None;
        self.launch_saved_tunnel(host, session_name, args, window, cx);
    }

    pub(crate) fn save_new_tunnel_modal(&mut self, cx: &mut Context<Self>) {
        let (name, bind_port_str, target_host_str, target_port_str, ftype, host_id, auto_start) = {
            if let Some(ref modal) = self.tunnel_modal {
                (
                    modal.name_input.read(cx).value().trim().to_string(),
                    modal.bind_port_input.read(cx).value().trim().to_string(),
                    modal.target_host_input.read(cx).value().trim().to_string(),
                    modal.target_port_input.read(cx).value().trim().to_string(),
                    modal.forwarding_type,
                    modal.selected_host_id,
                    modal.auto_start,
                )
            } else {
                return;
            }
        };

        if name.is_empty() {
            if let Some(ref mut m) = self.tunnel_modal {
                m.error = Some("Tunnel name is required.".into());
                cx.notify();
            }
            return;
        }

        let bind_port: u16 = match bind_port_str.parse() {
            Ok(p) if p > 0 => p,
            _ => {
                if let Some(ref mut m) = self.tunnel_modal {
                    m.error = Some("Invalid bind port (must be between 1 and 65535).".into());
                    cx.notify();
                }
                return;
            }
        };

        let rule = match ftype {
            ForwardingType::Dynamic => {
                match PortForwardRule::dynamic(Some("127.0.0.1"), bind_port) {
                    Ok(r) => r,
                    Err(e) => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some(e.to_string());
                            cx.notify();
                        }
                        return;
                    }
                }
            }
            ForwardingType::Local => {
                let target_port: u16 = match target_port_str.parse() {
                    Ok(p) if p > 0 => p,
                    _ => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some("Target port required for local forwarding.".into());
                            cx.notify();
                        }
                        return;
                    }
                };
                let target_host = if target_host_str.is_empty() {
                    "localhost"
                } else {
                    &target_host_str
                };
                match PortForwardRule::local(Some("127.0.0.1"), bind_port, target_host, target_port)
                {
                    Ok(r) => r,
                    Err(e) => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some(e.to_string());
                            cx.notify();
                        }
                        return;
                    }
                }
            }
            ForwardingType::Remote => {
                let target_port: u16 = match target_port_str.parse() {
                    Ok(p) if p > 0 => p,
                    _ => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some("Target port required for remote forwarding.".into());
                            cx.notify();
                        }
                        return;
                    }
                };
                let target_host = if target_host_str.is_empty() {
                    "localhost"
                } else {
                    &target_host_str
                };
                match PortForwardRule::remote(
                    Some("127.0.0.1"),
                    bind_port,
                    target_host,
                    target_port,
                ) {
                    Ok(r) => r,
                    Err(e) => {
                        if let Some(ref mut m) = self.tunnel_modal {
                            m.error = Some(e.to_string());
                            cx.notify();
                        }
                        return;
                    }
                }
            }
        };

        if host_id.is_none() {
            if let Some(modal) = &mut self.tunnel_modal {
                modal.error = Some("Select a saved SSH host.".into());
            }
            cx.notify();
            return;
        }
        let editing_id = self
            .tunnel_modal
            .as_ref()
            .and_then(|modal| modal.editing_id);
        if let Some(path) = self.database.clone() {
            let name_clone = name.clone();
            let rule_clone = rule.clone();
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    if let Some(id) = editing_id {
                        store
                            .update_tunnel(id, &name_clone, host_id, &rule_clone, auto_start)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok(id)
                    } else {
                        store
                            .add_tunnel(&name_clone, host_id, &rule_clone, auto_start)
                            .await
                            .map_err(|e| e.to_string())
                    }
                })
            });

            cx.spawn(async move |app, cx| {
                let res = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Some(ref mut modal) = app.tunnel_modal {
                        match res {
                            Ok(new_id) => {
                                let new_tunnel = SavedTunnel {
                                    id: new_id,
                                    host_id,
                                    name: name.clone(),
                                    rule: rule.clone(),
                                    auto_start,
                                };
                                modal.active_states.insert(
                                    new_id,
                                    ActiveTunnelState::new(
                                        new_id,
                                        host_id,
                                        name.clone(),
                                        rule,
                                        auto_start,
                                    ),
                                );
                                modal.tunnels.retain(|tunnel| tunnel.id != new_id);
                                modal.tunnels.push(new_tunnel);
                                modal.editing_id = None;
                                modal.selected_index = modal.tunnels.len().saturating_sub(1);
                                modal.status_message =
                                    Some(format!("Saved tunnel '{name}' (ID {new_id})"));
                                modal.error = None;
                            }
                            Err(e) => {
                                modal.error = Some(e);
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub(crate) fn edit_saved_tunnel(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Some(modal) = &mut self.tunnel_modal {
            if let Some(tunnel) = modal.tunnels.iter().find(|tunnel| tunnel.id == id).cloned() {
                modal.editing_id = Some(id);
                modal.selected_host_id = tunnel.host_id;
                modal.forwarding_type = tunnel.rule.forwarding_type;
                modal
                    .name_input
                    .update(cx, |input, cx| input.set_value(&tunnel.name, cx));
                modal.bind_port_input.update(cx, |input, cx| {
                    input.set_value(&tunnel.rule.bind_port.to_string(), cx)
                });
                modal.target_host_input.update(cx, |input, cx| {
                    input.set_value(tunnel.rule.target_host.as_deref().unwrap_or(""), cx)
                });
                modal.target_port_input.update(cx, |input, cx| {
                    input.set_value(
                        &tunnel
                            .rule
                            .target_port
                            .map(|port| port.to_string())
                            .unwrap_or_default(),
                        cx,
                    )
                });
                modal.status_message = Some(
                    "Editing the saved rule. Reopen its tunnel session to apply changes.".into(),
                );
            }
        }
        cx.notify();
    }

    pub(crate) fn delete_tunnel_modal(&mut self, tunnel_id: i64, cx: &mut Context<Self>) {
        if let Some(path) = self.database.clone() {
            let task = cx.background_executor().spawn(async move {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .map_err(|e| e.to_string())?;
                runtime.block_on(async {
                    let mut store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                    store
                        .delete_tunnel(tunnel_id)
                        .await
                        .map_err(|e| e.to_string())
                })
            });

            cx.spawn(async move |app, cx| {
                let res = task.await;
                let _ = app.update(cx, |app, cx| {
                    if let Some(ref mut modal) = app.tunnel_modal {
                        match res {
                            Ok(true) => {
                                modal.tunnels.retain(|t| t.id != tunnel_id);
                                modal.active_states.remove(&tunnel_id);
                                if modal.selected_index >= modal.tunnels.len() {
                                    modal.selected_index = modal.tunnels.len().saturating_sub(1);
                                }
                                modal.status_message = Some(format!("Tunnel {tunnel_id} deleted."));
                                modal.error = None;
                            }
                            Ok(false) => {
                                modal.error = Some("Tunnel not found.".into());
                            }
                            Err(e) => {
                                modal.error = Some(e);
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
}

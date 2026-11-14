//! Hosts actions.
use super::*;

impl Shellumi {
    pub(crate) fn edit_host(&mut self, host: Option<&SavedHost>, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let (
            id,
            name,
            hostname,
            username,
            port,
            group,
            tags,
            jump_host,
            startup_snip,
            protocol,
            protocol_config,
        ) = match host {
            Some(host) => (
                Some(host.id),
                host.name.clone(),
                host.destination.host().to_owned(),
                host.destination.username().to_owned(),
                host.destination.port().to_string(),
                host.group_name.clone().unwrap_or_default(),
                host.tags.join(", "),
                host.jump_host.clone().unwrap_or_default(),
                host.startup_snippet_id
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                host.protocol(),
                host.protocol_config.clone(),
            ),
            None => (
                None,
                String::new(),
                String::new(),
                String::new(),
                "22".into(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                ConnectionProtocol::Ssh,
                None,
            ),
        };
        self.form = Some(HostForm {
            active_category: HostEditorCategory::General,
            identity_id: host.and_then(|host| host.identity_id),
            id,
            name: cx.new(|cx| TextInput::new(cx, &name, "Display name")),
            hostname: cx.new(|cx| TextInput::new(cx, &hostname, "Hostname or IP")),
            username: cx.new(|cx| TextInput::new(cx, &username, "Username")),
            port: cx.new(|cx| TextInput::new(cx, &port, "22")),
            group: cx.new(|cx| TextInput::new(cx, &group, "Group (e.g. Production)")),
            tags: cx
                .new(|cx| TextInput::new(cx, &tags, "Tags (comma-separated, e.g. web, us-east)")),
            jump_host: cx
                .new(|cx| TextInput::new(cx, &jump_host, "Jump host (e.g. jump.corp:2222)")),
            startup_snippet: cx
                .new(|cx| TextInput::new(cx, &startup_snip, "Startup snippet ID (optional)")),
            protocol,
            protocol_config,
        });
        self.form_error = None;
        cx.notify();
        self.load_identities(cx);
    }

    pub(crate) fn set_host_editor_category(
        &mut self,
        cat: HostEditorCategory,
        cx: &mut Context<Self>,
    ) {
        if let Some(form) = &mut self.form {
            form.active_category = cat;
            cx.notify();
        }
    }

    pub(crate) fn toggle_sidebar_collapsed(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    pub(crate) fn set_form_protocol(
        &mut self,
        protocol: ConnectionProtocol,
        cx: &mut Context<Self>,
    ) {
        if let Some(form) = &mut self.form {
            form.protocol = protocol;
            match protocol {
                ConnectionProtocol::Telnet => {
                    let cur = form.port.read(cx).value();
                    if cur == "22" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("23", cx);
                        });
                    }
                }
                ConnectionProtocol::Ssh | ConnectionProtocol::Mosh => {
                    let cur = form.port.read(cx).value();
                    if cur == "23" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("22", cx);
                        });
                    }
                }
                ConnectionProtocol::Serial => {
                    let cur = form.port.read(cx).value();
                    if cur == "22" || cur == "23" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("1", cx);
                        });
                    }
                }
                ConnectionProtocol::Vnc => {
                    let cur = form.port.read(cx).value();
                    if cur == "22" || cur == "23" || cur == "3389" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("5900", cx);
                        });
                    }
                }
                ConnectionProtocol::Rdp => {
                    let cur = form.port.read(cx).value();
                    if cur == "22" || cur == "23" || cur == "5900" || cur.is_empty() {
                        form.port.update(cx, |input, cx| {
                            input.set_value("3389", cx);
                        });
                    }
                }
                _ => {}
            }
            cx.notify();
        }
    }

    pub(crate) fn save_host(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(form) = &self.form else {
            return;
        };
        let name = form.name.read(cx).value().to_owned();
        let host = form.hostname.read(cx).value().to_owned();
        let username = form.username.read(cx).value().to_owned();
        let port = form.port.read(cx).value().parse::<u16>();
        let group_raw = form.group.read(cx).value().trim().to_string();
        let group = if group_raw.is_empty() {
            None
        } else {
            Some(group_raw)
        };
        let tags: Vec<String> = form
            .tags
            .read(cx)
            .value()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let jump_raw = form.jump_host.read(cx).value().trim().to_string();
        let jump_host = if jump_raw.is_empty() {
            None
        } else {
            match validate_jump_host(&jump_raw) {
                Ok(j) => Some(j),
                Err(err) => {
                    self.form_error = Some(format!("Invalid jump host: {err}"));
                    cx.notify();
                    return;
                }
            }
        };
        let snip_raw = form.startup_snippet.read(cx).value().trim().to_string();
        let startup_snippet_id = if snip_raw.is_empty() {
            None
        } else {
            match snip_raw.parse::<i64>() {
                Ok(id) => Some(id),
                Err(_) => {
                    self.form_error =
                        Some("Startup snippet must be a numeric ID (or empty).".into());
                    cx.notify();
                    return;
                }
            }
        };

        let username = if username.trim().is_empty() && form.protocol == ConnectionProtocol::Ssh {
            let inherited_id = form.identity_id.or_else(|| {
                group
                    .as_ref()
                    .and_then(|name| self.desktop.groups.iter().find(|g| &g.name == name))
                    .and_then(|g| g.identity_id)
            });
            self.identities
                .iter()
                .find(|identity| Some(identity.id) == inherited_id)
                .map(|identity| identity.username.clone())
                .or_else(|| {
                    group
                        .as_ref()
                        .and_then(|name| self.desktop.groups.iter().find(|g| &g.name == name))
                        .and_then(|g| g.default_username.clone())
                })
                .unwrap_or(username)
        } else {
            username
        };
        let destination = match port
            .map_err(|_| "Enter a port between 1 and 65535.".to_owned())
            .and_then(|port| {
                SshDestination::new(&host, &username, port).map_err(|error| error.to_string())
            }) {
            Ok(destination) => destination,
            Err(error) => {
                self.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            self.form_error = Some("Enter a display name without control characters.".into());
            cx.notify();
            return;
        }
        let proto = form.protocol;
        let proto_config = form.protocol_config.clone();
        let id = form.id;
        let identity_id = form.identity_id;
        let Some(path) = self.database.clone() else {
            return;
        };
        self.saving = true;
        self.form_error = None;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path)
                    .await
                    .map_err(|error| error.to_string())?;
                match id {
                    Some(id) => {
                        if !store
                            .update_full(
                                id,
                                &name,
                                &destination,
                                group.as_deref(),
                                &tags,
                                jump_host.as_deref(),
                                startup_snippet_id,
                                proto.as_str(),
                                proto_config.as_deref(),
                                identity_id,
                            )
                            .await
                            .map_err(|error| error.to_string())?
                        {
                            return Err("Host no longer exists.".into());
                        }
                    }
                    None => {
                        store
                            .add_full(
                                &name,
                                &destination,
                                group.as_deref(),
                                &tags,
                                jump_host.as_deref(),
                                startup_snippet_id,
                                proto.as_str(),
                                proto_config.as_deref(),
                                identity_id,
                            )
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                }
                Ok(())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                app.saving = false;
                match result {
                    Ok(()) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        app.sync_scheduler.notify_local_mutation(now);
                        app.form = None;
                        app.load_hosts(cx);
                    }
                    Err(error) => app.form_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn delete_host(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open(&path)
                    .await
                    .map_err(|error| error.to_string())?;
                store.delete(id).await.map_err(|error| error.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Ok(true) = result {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    app.sync_scheduler.notify_local_mutation(now);
                    app.load_hosts(cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn load_hosts(&mut self, cx: &mut Context<Self>) {
        if matches!(self.hosts, HostState::Loading) {
            return;
        }
        let Some(path) = self.database.clone() else {
            return;
        };
        self.hosts = HostState::Loading;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            data_path::prepare_parent(std::path::Path::new(&path))?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let store = HostStore::open(&path).await.map_err(|e| e.to_string())?;
                store.list().await.map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                app.hosts = match result {
                    Ok(hosts) => HostState::Loaded(hosts),
                    Err(error) => HostState::Error(error),
                };
                cx.notify();
            });
        })
        .detach();
    }
}

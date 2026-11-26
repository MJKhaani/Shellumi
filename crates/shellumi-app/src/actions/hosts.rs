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

        let (
            rd_domain,
            rd_width,
            rd_height,
            rd_color_depth,
            rd_view_only,
            rd_tls_strict,
            rd_enable_audio,
            rd_enable_microphone,
        ) = match host {
            Some(h) if h.is_vnc() => {
                let cfg = h.vnc_config().unwrap_or_default();
                (
                    String::new(),
                    cfg.width.to_string(),
                    cfg.height.to_string(),
                    cfg.color_depth,
                    cfg.view_only,
                    true,
                    false,
                    false,
                )
            }
            Some(h) if h.is_rdp() => {
                let cfg = h.rdp_config().unwrap_or_default();
                (
                    cfg.domain.unwrap_or_default(),
                    cfg.screen_width.to_string(),
                    cfg.screen_height.to_string(),
                    cfg.color_depth,
                    cfg.view_only,
                    cfg.tls_strict,
                    cfg.enable_audio,
                    cfg.enable_microphone,
                )
            }
            _ => (
                String::new(),
                "1920".into(),
                "1080".into(),
                32,
                false,
                true,
                false,
                false,
            ),
        };

        let serial_cfg = protocol_config
            .as_deref()
            .and_then(|c| serde_json::from_str::<shellumi_core::SerialConfig>(c).ok())
            .unwrap_or_default();
        let auth_meta = protocol_config
            .as_deref()
            .and_then(|c| serde_json::from_str::<HostAuthMetadata>(c).ok())
            .unwrap_or_default();
        let host_secret_ref = auth_meta.host_secret_ref;
        let has_host_secret = host_secret_ref.is_some();

        let caps = shellumi_core::ProtocolCapabilities::for_protocol(protocol);
        let identity_id = host.and_then(|host| host.identity_id);
        let auth_source_kind = if !caps.allows_saved_identity && !caps.allows_ask_on_connect {
            if caps.allows_no_auth {
                HostAuthSourceKind::NoAuthentication
            } else {
                HostAuthSourceKind::AskOnConnect
            }
        } else if identity_id.is_some() {
            HostAuthSourceKind::ExistingIdentity
        } else if has_host_secret {
            HostAuthSourceKind::EnterCredentials
        } else if auth_meta.auth_mode.as_deref() == Some("no_auth") && caps.allows_no_auth {
            HostAuthSourceKind::NoAuthentication
        } else {
            HostAuthSourceKind::AskOnConnect
        };
        let credential_kind = caps
            .supported_credentials
            .first()
            .copied()
            .unwrap_or(shellumi_core::CredentialKind::None);
        let persistence_choice = if has_host_secret {
            HostPersistenceChoice::SaveForThisHost
        } else {
            HostPersistenceChoice::SessionOnly
        };

        let draft_generation = self
            .form
            .as_ref()
            .map(|f| f.draft_generation.wrapping_add(1))
            .unwrap_or(1);
        let port_is_manual = host.is_some()
            && caps
                .default_port
                .is_some_and(|dp| dp.to_string() != port.trim());

        let form = HostForm {
            draft_generation,
            active_category: HostEditorCategory::General,
            identity_id,
            id,
            name: cx.new(|cx| TextInput::new(cx, &name, "Display name")),
            hostname: cx.new(|cx| TextInput::new(cx, &hostname, "Hostname or IP")),
            username: cx.new(|cx| TextInput::new(cx, &username, "Username")),
            port: cx.new(|cx| TextInput::new(cx, &port, "22")),
            port_is_manual,
            group: cx.new(|cx| TextInput::new(cx, &group, "Group (e.g. Production)")),
            tags: cx
                .new(|cx| TextInput::new(cx, &tags, "Tags (comma-separated, e.g. web, us-east)")),
            jump_host: cx
                .new(|cx| TextInput::new(cx, &jump_host, "Jump host (e.g. jump.corp:2222)")),
            startup_snippet: cx
                .new(|cx| TextInput::new(cx, &startup_snip, "Startup snippet ID (optional)")),
            protocol,
            protocol_config,
            auth_source_kind,
            credential_kind,
            persistence_choice,
            auth_password: cx.new(|cx| {
                TextInput::new(
                    cx,
                    "",
                    if has_host_secret {
                        "Credential configured (enter to replace)"
                    } else {
                        "Password…"
                    },
                )
                .masked()
            }),
            auth_key_path: cx.new(|cx| TextInput::new(cx, "", "Key path (e.g. ~/.ssh/id_ed25519)")),
            auth_passphrase: cx
                .new(|cx| TextInput::new(cx, "", "Key passphrase (optional)").masked()),
            new_identity_name: cx.new(|cx| TextInput::new(cx, "", "New identity name")),
            new_identity_tags: cx.new(|cx| TextInput::new(cx, "", "Tags (comma-separated)")),
            secret_update_action: shellumi_core::SecretUpdateAction::Keep,
            host_secret_ref,
            has_host_secret,
            rd_domain: cx.new(|cx| TextInput::new(cx, &rd_domain, "Domain (optional)")),
            rd_width: cx.new(|cx| TextInput::new(cx, &rd_width, "1920")),
            rd_height: cx.new(|cx| TextInput::new(cx, &rd_height, "1080")),
            rd_color_depth,
            rd_view_only,
            rd_tls_strict,
            rd_enable_audio,
            rd_enable_microphone,
            serial_baud: cx
                .new(|cx| TextInput::new(cx, &serial_cfg.baud_rate.to_string(), "115200")),
            serial_data_bits: serial_cfg.data_bits.as_u8(),
            serial_parity: serial_cfg.parity,
            serial_stop_bits: serial_cfg.stop_bits,
            serial_flow_control: serial_cfg.flow_control,
        };

        // Connect dirty-state tracking to inspector coordinator
        cx.observe(&form.name, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&form.hostname, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&form.username, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&form.port, |app, _, cx| {
            if let Some(f) = &mut app.form {
                f.port_is_manual = true;
            }
            app.inspector_mark_dirty(cx);
        })
        .detach();
        cx.observe(&form.group, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&form.tags, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&form.jump_host, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&form.startup_snippet, |app, _, cx| {
            app.inspector_mark_dirty(cx)
        })
        .detach();
        cx.observe(&form.auth_password, |app, _, cx| {
            app.inspector_mark_dirty(cx)
        })
        .detach();
        cx.observe(&form.auth_key_path, |app, _, cx| {
            app.inspector_mark_dirty(cx)
        })
        .detach();
        cx.observe(&form.auth_passphrase, |app, _, cx| {
            app.inspector_mark_dirty(cx)
        })
        .detach();
        cx.observe(&form.new_identity_name, |app, _, cx| {
            app.inspector_mark_dirty(cx)
        })
        .detach();
        cx.observe(&form.new_identity_tags, |app, _, cx| {
            app.inspector_mark_dirty(cx)
        })
        .detach();
        cx.observe(&form.rd_domain, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&form.rd_width, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&form.rd_height, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();
        cx.observe(&form.serial_baud, |app, _, cx| app.inspector_mark_dirty(cx))
            .detach();

        self.form = Some(form);
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

    /// Unified mutation entry point for all non-text Host Inspector edits.
    /// Validates draft generation to reject stale callbacks and marks the coordinator dirty.
    pub(crate) fn apply_host_edit(
        &mut self,
        draft_gen: u64,
        event: HostEditEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = &mut self.form else {
            return;
        };
        if form.draft_generation != draft_gen {
            tracing::debug!(
                expected_gen = form.draft_generation,
                received_gen = draft_gen,
                "Rejected stale host edit event"
            );
            return;
        }
        match event {
            HostEditEvent::SetProtocol(protocol) => {
                self.set_form_protocol(protocol, cx);
                return;
            }
            HostEditEvent::SelectIdentity(id) => {
                form.identity_id = id;
                if id.is_some() {
                    form.auth_source_kind = HostAuthSourceKind::ExistingIdentity;
                }
            }
            HostEditEvent::SetAuthSource(source) => {
                form.auth_source_kind = source;
                if source != HostAuthSourceKind::ExistingIdentity {
                    form.identity_id = None;
                }
            }
            HostEditEvent::SetCredentialKind(kind) => {
                form.credential_kind = kind;
            }
            HostEditEvent::SetPersistence(choice) => {
                form.persistence_choice = choice;
            }
            HostEditEvent::SetSecretUpdateAction(action) => {
                form.secret_update_action = action;
            }
            HostEditEvent::SetRdColorDepth(depth) => {
                form.rd_color_depth = depth;
            }
            HostEditEvent::ToggleRdViewOnly => {
                form.rd_view_only = !form.rd_view_only;
            }
            HostEditEvent::ToggleRdTlsStrict => {
                form.rd_tls_strict = !form.rd_tls_strict;
            }
            HostEditEvent::ToggleRdAudio => {
                form.rd_enable_audio = !form.rd_enable_audio;
            }
            HostEditEvent::ToggleRdMicrophone => {
                form.rd_enable_microphone = !form.rd_enable_microphone;
            }
            HostEditEvent::SetSerialDataBits(bits) => {
                form.serial_data_bits = bits;
            }
            HostEditEvent::SetSerialParity(parity) => {
                form.serial_parity = parity;
            }
            HostEditEvent::SetSerialStopBits(stop) => {
                form.serial_stop_bits = stop;
            }
            HostEditEvent::SetSerialFlowControl(flow) => {
                form.serial_flow_control = flow;
            }
        }
        self.inspector_mark_dirty(cx);
        cx.notify();
    }

    pub(crate) fn set_form_protocol(
        &mut self,
        protocol: ConnectionProtocol,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = &mut self.form else {
            return;
        };
        if form.protocol == protocol {
            return;
        }
        let old_caps = shellumi_core::ProtocolCapabilities::for_protocol(form.protocol);
        let new_caps = shellumi_core::ProtocolCapabilities::for_protocol(protocol);
        form.protocol = protocol;

        // Clear incompatible secret inputs when switching protocols (never transfer secrets across protocols).
        form.auth_password
            .update(cx, |input, cx| input.clear_secret(cx));
        form.auth_passphrase
            .update(cx, |input, cx| input.clear_secret(cx));

        // Clear incompatible identity if it is not compatible with the new protocol.
        if let Some(id_val) = form.identity_id {
            let compatible = self
                .identities
                .iter()
                .find(|i| i.id == id_val)
                .is_some_and(|i| new_caps.is_auth_method_compatible(&i.auth_method));
            if !compatible {
                form.identity_id = None;
                if form.auth_source_kind == HostAuthSourceKind::ExistingIdentity {
                    form.auth_source_kind = if new_caps.allows_ask_on_connect {
                        HostAuthSourceKind::AskOnConnect
                    } else if new_caps.allows_no_auth {
                        HostAuthSourceKind::NoAuthentication
                    } else {
                        HostAuthSourceKind::AskOnConnect
                    };
                }
            }
        }

        if !new_caps.supports_domain {
            form.rd_domain
                .update(cx, |input, cx| input.set_value("", cx));
        }
        if !new_caps.supports_jump_host {
            form.jump_host
                .update(cx, |input, cx| input.set_value("", cx));
        }
        if !new_caps.supports_startup_snippet {
            form.startup_snippet
                .update(cx, |input, cx| input.set_value("", cx));
        }

        // Update credential kind to a valid default for the new protocol.
        if !new_caps
            .supported_credentials
            .contains(&form.credential_kind)
        {
            form.credential_kind = new_caps
                .supported_credentials
                .first()
                .copied()
                .unwrap_or(shellumi_core::CredentialKind::None);
        }

        // Replace default ports cleanly; never apply network ports to Serial/Local.
        if !new_caps.uses_network_port {
            form.port.update(cx, |input, cx| input.set_value("", cx));
            form.port_is_manual = false;
        } else {
            let cur_port = form.port.read(cx).value().trim().to_string();
            let was_old_default = old_caps
                .default_port
                .is_some_and(|dp| dp.to_string() == cur_port);
            if !form.port_is_manual
                || cur_port.is_empty()
                || was_old_default
                || !old_caps.uses_network_port
            {
                let next_port = new_caps.default_port_str();
                form.port
                    .update(cx, |input, cx| input.set_value(next_port, cx));
                form.port_is_manual = false;
            }
        }

        self.inspector_mark_dirty(cx);
        cx.notify();
    }

    #[allow(dead_code)]
    pub(crate) fn set_host_rd_resolution(
        &mut self,
        width: u16,
        height: u16,
        cx: &mut Context<Self>,
    ) {
        if let Some(form) = &mut self.form {
            form.rd_width.update(cx, |input, cx| {
                input.set_value(&width.to_string(), cx);
            });
            form.rd_height.update(cx, |input, cx| {
                input.set_value(&height.to_string(), cx);
            });
            cx.notify();
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_host_rd_color_depth(&mut self, depth: u8, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.form {
            form.rd_color_depth = depth;
            cx.notify();
        }
    }

    #[allow(dead_code)]
    pub(crate) fn toggle_host_rd_view_only(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.form {
            form.rd_view_only = !form.rd_view_only;
            cx.notify();
        }
    }

    #[allow(dead_code)]
    pub(crate) fn toggle_host_rd_tls_strict(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = &mut self.form {
            form.rd_tls_strict = !form.rd_tls_strict;
            cx.notify();
        }
    }

    pub(crate) fn save_host(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(gen) = self.inspector_coordinator.start_save() else {
            return;
        };
        let Some(form) = &self.form else {
            return;
        };
        let active_vid = self.active_vault_id.clone();
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
                    self.inspector_coordinator.fail_save(
                        &active_vid,
                        gen,
                        format!("Invalid jump host: {err}"),
                    );
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
                    self.inspector_coordinator.fail_save(
                        &active_vid,
                        gen,
                        "Startup snippet must be a numeric ID (or empty).".into(),
                    );
                    cx.notify();
                    return;
                }
            }
        };

        let proto = form.protocol;
        let caps = shellumi_core::ProtocolCapabilities::for_protocol(proto);

        // Protocol-specific validation before starting async writes
        let destination = if proto == ConnectionProtocol::Local {
            let local_host = if host.trim().is_empty() {
                "localhost"
            } else {
                host.trim()
            };
            let local_user = if username.trim().is_empty() {
                "local"
            } else {
                username.trim()
            };
            match SshDestination::new(local_host, local_user, 22) {
                Ok(d) => d,
                Err(error) => {
                    let msg = error.to_string();
                    self.form_error = Some(msg.clone());
                    self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                    cx.notify();
                    return;
                }
            }
        } else if proto == ConnectionProtocol::Serial {
            let dev_path = host.trim();
            if dev_path.is_empty() || dev_path.chars().any(char::is_control) {
                let msg =
                    "Enter a valid serial device path (e.g. /dev/ttyUSB0 or COM3).".to_string();
                self.form_error = Some(msg.clone());
                self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                cx.notify();
                return;
            }
            let baud_ok = form
                .serial_baud
                .read(cx)
                .value()
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|&b| b > 0);
            if baud_ok.is_none() {
                let msg = "Enter a valid serial baud rate (e.g. 9600, 115200).".to_string();
                self.form_error = Some(msg.clone());
                self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                cx.notify();
                return;
            }
            match SshDestination::new(dev_path, "serial", 1) {
                Ok(d) => d,
                Err(error) => {
                    let msg = error.to_string();
                    self.form_error = Some(msg.clone());
                    self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                    cx.notify();
                    return;
                }
            }
        } else {
            let username = if username.trim().is_empty()
                && (proto == ConnectionProtocol::Ssh || proto == ConnectionProtocol::Mosh)
            {
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
            match port
                .map_err(|_| "Enter a port between 1 and 65535.".to_owned())
                .and_then(|port| {
                    SshDestination::new(&host, &username, port).map_err(|error| error.to_string())
                }) {
                Ok(destination) => destination,
                Err(error) => {
                    self.form_error = Some(error.clone());
                    self.inspector_coordinator
                        .fail_save(&active_vid, gen, error);
                    cx.notify();
                    return;
                }
            }
        };

        if name.trim().is_empty() || name.chars().any(char::is_control) {
            self.form_error = Some("Enter a display name without control characters.".into());
            self.inspector_coordinator.fail_save(
                &active_vid,
                gen,
                "Enter a display name without control characters.".into(),
            );
            cx.notify();
            return;
        }

        // Validate RDP/VNC display dimensions when applicable
        if caps.supports_display_options {
            let w_ok = form
                .rd_width
                .read(cx)
                .value()
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|&w| w >= 320);
            let h_ok = form
                .rd_height
                .read(cx)
                .value()
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|&h| h >= 200);
            if w_ok.is_none() || h_ok.is_none() {
                let msg = "Display resolution must be at least 320x200.".to_string();
                self.form_error = Some(msg.clone());
                self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                cx.notify();
                return;
            }
        }

        // Gather authentication / credential choices
        let auth_source_kind = form.auth_source_kind;
        let credential_kind = form.credential_kind;
        let persistence_choice = form.persistence_choice;
        let secret_update_action = form.secret_update_action;
        let existing_host_secret_ref = form.host_secret_ref.clone();
        // Preserve exact password/passphrase whitespace!
        let raw_password = form.auth_password.read(cx).value().to_string();
        let raw_passphrase = form.auth_passphrase.read(cx).value().to_string();
        let key_path = form.auth_key_path.read(cx).value().trim().to_string();
        let new_identity_name = form.new_identity_name.read(cx).value().trim().to_string();
        let new_identity_tags: Vec<String> = form
            .new_identity_tags
            .read(cx)
            .value()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let mut identity_id = if auth_source_kind == HostAuthSourceKind::ExistingIdentity {
            form.identity_id
        } else {
            None
        };

        if auth_source_kind == HostAuthSourceKind::ExistingIdentity {
            let Some(sel_id) = identity_id else {
                let msg =
                    "Select an existing identity or choose another credential source.".to_string();
                self.form_error = Some(msg.clone());
                self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                cx.notify();
                return;
            };
            let compatible = self
                .identities
                .iter()
                .find(|i| i.id == sel_id)
                .is_some_and(|i| caps.is_auth_method_compatible(&i.auth_method));
            if !compatible {
                let msg = "Selected identity is not compatible with this protocol.".to_string();
                self.form_error = Some(msg.clone());
                self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                cx.notify();
                return;
            }
        }

        if auth_source_kind == HostAuthSourceKind::EnterCredentials
            && persistence_choice == HostPersistenceChoice::SaveAsIdentity
        {
            if new_identity_name.is_empty() || new_identity_name.chars().any(char::is_control) {
                let msg = "Enter a valid name for the new reusable identity.".to_string();
                self.form_error = Some(msg.clone());
                self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                cx.notify();
                return;
            }
            if credential_kind == shellumi_core::CredentialKind::SshKey && key_path.is_empty() {
                let msg = "Enter a private key path for the SSH key identity.".to_string();
                self.form_error = Some(msg.clone());
                self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                cx.notify();
                return;
            }
            if matches!(
                credential_kind,
                shellumi_core::CredentialKind::Password
                    | shellumi_core::CredentialKind::RdpPassword
                    | shellumi_core::CredentialKind::VncPassword
                    | shellumi_core::CredentialKind::VncUsernamePassword
            ) && raw_password.is_empty()
            {
                let msg = "Enter a password to save in the new identity.".to_string();
                self.form_error = Some(msg.clone());
                self.inspector_coordinator.fail_save(&active_vid, gen, msg);
                cx.notify();
                return;
            }
        }

        let Some(path) = self.active_vault_db_path() else {
            let msg = "Active vault database path is unavailable.".to_string();
            self.form_error = Some(msg.clone());
            self.inspector_coordinator.fail_save(&active_vid, gen, msg);
            cx.notify();
            return;
        };

        let mut proto_config = match proto {
            ConnectionProtocol::Vnc => {
                let mut cfg = form
                    .protocol_config
                    .as_deref()
                    .and_then(|c| serde_json::from_str::<shellumi_core::VncConfig>(c).ok())
                    .unwrap_or_else(|| {
                        shellumi_core::VncConfig::new(destination.host(), destination.port().get())
                            .unwrap_or_default()
                    });
                cfg.host = destination.host().to_string();
                cfg.port = destination.port().get();
                cfg.view_only = form.rd_view_only;
                cfg.color_depth = form.rd_color_depth;
                if let Ok(w) = form.rd_width.read(cx).value().trim().parse::<u16>() {
                    cfg.width = w;
                }
                if let Ok(h) = form.rd_height.read(cx).value().trim().parse::<u16>() {
                    cfg.height = h;
                }
                serde_json::to_string(&cfg).ok()
            }
            ConnectionProtocol::Rdp => {
                let mut cfg = form
                    .protocol_config
                    .as_deref()
                    .and_then(|c| serde_json::from_str::<shellumi_core::RdpConfig>(c).ok())
                    .unwrap_or_else(|| {
                        shellumi_core::RdpConfig::new(destination.host(), destination.port().get())
                            .unwrap_or_default()
                    });
                cfg.host = destination.host().to_string();
                cfg.port = destination.port().get();
                cfg.username = if destination.username().is_empty() {
                    None
                } else {
                    Some(destination.username().to_string())
                };
                let dom = form.rd_domain.read(cx).value().trim().to_string();
                cfg.domain = if dom.is_empty() { None } else { Some(dom) };
                if let Ok(w) = form.rd_width.read(cx).value().trim().parse::<u16>() {
                    cfg.screen_width = w;
                }
                if let Ok(h) = form.rd_height.read(cx).value().trim().parse::<u16>() {
                    cfg.screen_height = h;
                }
                cfg.color_depth = form.rd_color_depth;
                cfg.view_only = form.rd_view_only;
                cfg.tls_strict = form.rd_tls_strict;
                cfg.enable_audio = form.rd_enable_audio;
                cfg.enable_microphone = form.rd_enable_microphone;
                serde_json::to_string(&cfg).ok()
            }
            ConnectionProtocol::Serial => {
                let baud = form
                    .serial_baud
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<u32>()
                    .unwrap_or(115200);
                let mut cfg =
                    shellumi_core::SerialConfig::new(destination.host(), baud).unwrap_or_default();
                cfg.data_bits = match form.serial_data_bits {
                    5 => shellumi_core::SerialDataBits::Five,
                    6 => shellumi_core::SerialDataBits::Six,
                    7 => shellumi_core::SerialDataBits::Seven,
                    _ => shellumi_core::SerialDataBits::Eight,
                };
                cfg.parity = form.serial_parity;
                cfg.stop_bits = form.serial_stop_bits;
                cfg.flow_control = form.serial_flow_control;
                serde_json::to_string(&cfg).ok()
            }
            _ => form.protocol_config.clone(),
        };

        let id = form.id;
        self.saving = true;
        self.form_error = None;
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
        let is_remote = self.is_active_vault_remote();
        let vault_key = self.active_vault_key();
        let wants_new_identity = auth_source_kind == HostAuthSourceKind::EnterCredentials
            && persistence_choice == HostPersistenceChoice::SaveAsIdentity;
        let wants_host_secret = auth_source_kind == HostAuthSourceKind::EnterCredentials
            && persistence_choice == HostPersistenceChoice::SaveForThisHost;
        let auth_mode_str = match auth_source_kind {
            HostAuthSourceKind::NoAuthentication => Some("no_auth".to_string()),
            HostAuthSourceKind::AskOnConnect => Some("ask".to_string()),
            _ => None,
        };
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, vault_key.as_ref())
                    .await
                    .map_err(|error| error.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id_task) {
                    store.set_vault_id(vid);
                }

                let credential_store = shellumi_core::default_credential_store();
                let nonce = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos();
                let mut staged_refs: Vec<String> = Vec::new();

                let mut new_identity: Option<shellumi_core::NewIdentity> = None;
                if wants_new_identity {
                    let ident_account = format!(
                        "desktop-identity-{}-{}-{nonce}",
                        active_vault_id_task,
                        std::process::id()
                    );
                    let password_ref = if !raw_password.is_empty() {
                        match shellumi_core::store_secret_to_ref(
                            credential_store.as_ref(),
                            shellumi_core::DEFAULT_KEYCHAIN_SERVICE,
                            &format!("{ident_account}:password"),
                            &raw_password,
                        ) {
                            Ok(r) => {
                                staged_refs.push(r.clone());
                                Some(r)
                            }
                            Err(e) => {
                                return Err(format!("Failed to store credential: {e}"));
                            }
                        }
                    } else {
                        None
                    };
                    let passphrase_ref = if !raw_passphrase.is_empty() {
                        match shellumi_core::store_secret_to_ref(
                            credential_store.as_ref(),
                            shellumi_core::DEFAULT_KEYCHAIN_SERVICE,
                            &format!("{ident_account}:passphrase"),
                            &raw_passphrase,
                        ) {
                            Ok(r) => {
                                staged_refs.push(r.clone());
                                Some(r)
                            }
                            Err(e) => {
                                for r in &staged_refs {
                                    let _ = shellumi_core::delete_secret_by_ref(
                                        credential_store.as_ref(),
                                        r,
                                    );
                                }
                                return Err(format!("Failed to store key passphrase: {e}"));
                            }
                        }
                    } else {
                        None
                    };
                    let auth_method = match credential_kind {
                        shellumi_core::CredentialKind::SshKey => {
                            shellumi_core::AuthMethod::PublicKey {
                                key_name: new_identity_name.clone(),
                            }
                        }
                        shellumi_core::CredentialKind::SshAgent => shellumi_core::AuthMethod::Agent,
                        shellumi_core::CredentialKind::Interactive => {
                            shellumi_core::AuthMethod::Interactive
                        }
                        _ => shellumi_core::AuthMethod::Password,
                    };
                    new_identity = Some(shellumi_core::NewIdentity {
                        name: new_identity_name.clone(),
                        username: destination.username().to_string(),
                        auth_method,
                        password_secret_ref: password_ref,
                        key_path: if key_path.is_empty() {
                            None
                        } else {
                            Some(key_path)
                        },
                        passphrase_secret_ref: passphrase_ref,
                        certificate_path: None,
                        tags: new_identity_tags,
                        comment: None,
                    });
                    identity_id = None;
                }

                // Handle host-scoped secret (SaveForThisHost) via non-secret ref in protocol_config
                let mut host_secret_ref_to_save = existing_host_secret_ref.clone();
                if wants_host_secret {
                    match secret_update_action {
                        shellumi_core::SecretUpdateAction::Remove => {
                            host_secret_ref_to_save = None;
                        }
                        shellumi_core::SecretUpdateAction::Keep
                            if existing_host_secret_ref.is_some() && raw_password.is_empty() => {}
                        _ => {
                            if !raw_password.is_empty() {
                                let host_account = format!(
                                    "desktop-host-{}-{}-{nonce}",
                                    active_vault_id_task,
                                    std::process::id()
                                );
                                match shellumi_core::store_secret_to_ref(
                                    credential_store.as_ref(),
                                    shellumi_core::DEFAULT_KEYCHAIN_SERVICE,
                                    &host_account,
                                    &raw_password,
                                ) {
                                    Ok(r) => {
                                        staged_refs.push(r.clone());
                                        host_secret_ref_to_save = Some(r);
                                    }
                                    Err(e) => {
                                        for r in &staged_refs {
                                            let _ = shellumi_core::delete_secret_by_ref(
                                                credential_store.as_ref(),
                                                r,
                                            );
                                        }
                                        return Err(format!(
                                            "Failed to store host credential: {e}"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                } else if auth_source_kind != HostAuthSourceKind::EnterCredentials {
                    host_secret_ref_to_save = None;
                }

                // Merge non-secret auth metadata into protocol_config JSON
                if host_secret_ref_to_save.is_some() || auth_mode_str.is_some() {
                    let mut map = proto_config
                        .as_deref()
                        .and_then(|s| {
                            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(s)
                                .ok()
                        })
                        .unwrap_or_default();
                    if let Some(ref r) = host_secret_ref_to_save {
                        map.insert(
                            "host_secret_ref".to_string(),
                            serde_json::Value::String(r.clone()),
                        );
                    } else {
                        map.remove("host_secret_ref");
                    }
                    if let Some(ref m) = auth_mode_str {
                        map.insert(
                            "auth_mode".to_string(),
                            serde_json::Value::String(m.clone()),
                        );
                    } else {
                        map.remove("auth_mode");
                    }
                    proto_config = serde_json::to_string(&map).ok();
                }

                let tx_res = store
                    .save_host_with_identity_tx(
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
                        new_identity.as_ref(),
                    )
                    .await;

                let (saved_host_id, saved_identity_id) = match tx_res {
                    Ok(pair) => pair,
                    Err(error) => {
                        for r in &staged_refs {
                            let _ =
                                shellumi_core::delete_secret_by_ref(credential_store.as_ref(), r);
                        }
                        return Err(error.to_string());
                    }
                };

                if is_remote {
                    let payload = shellumi_storage::SyncHostPayload {
                        name: name.clone(),
                        hostname: destination.host().to_string(),
                        username: destination.username().to_string(),
                        port: destination.port().into(),
                        group_name: group.clone(),
                        tags: tags.clone(),
                        jump_host: jump_host.clone(),
                        startup_snippet_id,
                        protocol: proto.to_string(),
                        protocol_config: proto_config.clone(),
                        identity_id: saved_identity_id,
                    };
                    let json = serde_json::to_string(&payload).ok();
                    let _ = store
                        .record_pending_mutation(
                            &active_vault_id_task,
                            &name,
                            "host",
                            0,
                            false,
                            json.as_deref(),
                        )
                        .await;
                }
                Ok::<(i64, Option<i64>), String>((saved_host_id, saved_identity_id))
            })
        });
        cx.spawn(async move |app, cx| {
            let result: Result<(i64, Option<i64>), String> = task.await;
            let _ = app.update(cx, |app, cx| {
                app.saving = false;
                match result {
                    Ok((saved_host_id, _saved_identity_id)) => {
                        let final_host_id = Some(saved_host_id);
                        let target =
                            crate::ui::inspector::coordinator::InspectorTarget::Host(final_host_id);
                        let ok =
                            app.inspector_coordinator
                                .complete_save(&active_vault_id, gen, target);
                        if !ok {
                            tracing::warn!(
                                originating_vault = %active_vault_id,
                                current_vault = %app.active_vault_id,
                                gen = gen,
                                "Discarded stale save completion for host"
                            );
                            return;
                        }

                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        app.sync_scheduler.notify_local_mutation(now);
                        app.form = None;
                        app.selected_host_id = final_host_id;
                        app.load_hosts(cx);
                        app.load_identities(cx);
                        if app.is_active_vault_remote() {
                            app.sync_active_vault(cx);
                        }

                        if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                            app.apply_navigation_request(req, None, cx);
                        } else {
                            app.open_host_inspector(
                                final_host_id,
                                crate::ui::inspector::InspectorMode::View,
                                cx,
                            );
                        }
                    }
                    Err(error) => {
                        app.form_error = Some(error.clone());
                        app.inspector_coordinator.fail_save(&active_vid, gen, error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn delete_host(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let is_remote = self.is_active_vault_remote();
        let vault_key = self.active_vault_key();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, vault_key.as_ref())
                    .await
                    .map_err(|error| error.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id) {
                    store.set_vault_id(vid);
                }
                let host_name = store.get_host(id).await.ok().flatten().map(|h| h.name);
                let deleted = store.delete(id).await.map_err(|error| error.to_string())?;
                if is_remote && deleted {
                    if let Some(hname) = host_name {
                        let _ = store
                            .record_pending_mutation(
                                &active_vault_id,
                                &hname,
                                "host",
                                0,
                                true,
                                None,
                            )
                            .await;
                    }
                }
                Ok::<_, String>(deleted)
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| match result {
                Ok(true) => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    app.sync_scheduler.notify_local_mutation(now);
                    app.load_hosts(cx);
                    if app.is_active_vault_remote() {
                        app.sync_active_vault(cx);
                    }
                }
                Ok(false) => {
                    app.desktop.message = Some(format!("Host with ID {id} not found"));
                }
                Err(e) => {
                    app.desktop.message = Some(format!("Failed to delete host: {e}"));
                }
            });
        })
        .detach();
    }

    pub(crate) fn load_hosts(&mut self, cx: &mut Context<Self>) {
        self.vault_load_generation += 1;
        let gen = self.vault_load_generation;
        let Some(path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
        let vault_key = self.active_vault_key();
        self.hosts = HostState::Loading;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            data_path::prepare_parent(std::path::Path::new(&path))?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id_task) {
                    store.set_vault_id(vid);
                }
                store.list().await.map_err(|e| e.to_string())
            })
        });
        cx.spawn(async move |app, cx| {
            let result = task.await;
            let _ = app.update(cx, |app, cx| {
                if app.active_vault_id == active_vault_id && app.vault_load_generation == gen {
                    app.hosts = match result {
                        Ok(hosts) => HostState::Loaded(hosts),
                        Err(error) => HostState::Error(error),
                    };
                    cx.notify();
                } else {
                    tracing::warn!(
                        task_vault = %active_vault_id,
                        current_vault = %app.active_vault_id,
                        task_gen = gen,
                        current_gen = app.vault_load_generation,
                        "Discarded stale host load result from superseded vault or operation"
                    );
                }
            });
        })
        .detach();
    }
}

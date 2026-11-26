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
        let persisted = crate::PersistedHostAuth::from_host(
            protocol,
            host.and_then(|h| h.identity_id),
            protocol_config.as_deref(),
        );
        let caps = shellumi_core::ProtocolCapabilities::for_protocol(protocol);
        let identity_id = host.and_then(|host| host.identity_id);
        let auth_source_kind = persisted.auth_source_kind;
        let credential_kind = persisted.credential_kind;
        let persistence_choice = persisted.persistence_choice;
        let host_secret_ref = persisted.host_secret_ref;
        let host_passphrase_ref = persisted.host_passphrase_ref;
        let has_host_passphrase = persisted.has_host_passphrase;
        let initial_key_path = persisted.host_key_path.unwrap_or_default();

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
                    if host_secret_ref.is_some() {
                        "Credential configured (enter to replace)"
                    } else {
                        "Password…"
                    },
                )
                .masked()
            }),
            auth_key_path: cx.new(|cx| {
                TextInput::new(cx, &initial_key_path, "Key path (e.g. ~/.ssh/id_ed25519)")
            }),
            auth_passphrase: cx.new(|cx| {
                TextInput::new(
                    cx,
                    "",
                    if has_host_passphrase {
                        "Passphrase configured (enter to replace)"
                    } else {
                        "Key passphrase (optional)"
                    },
                )
                .masked()
            }),
            new_identity_name: cx.new(|cx| TextInput::new(cx, "", "New identity name")),
            new_identity_tags: cx.new(|cx| TextInput::new(cx, "", "Tags (comma-separated)")),
            secret_update_action: shellumi_core::SecretUpdateAction::Keep,
            passphrase_update_action: shellumi_core::SecretUpdateAction::Keep,
            host_secret_ref,
            host_passphrase_ref,
            has_host_passphrase,
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
            HostEditEvent::SetPassphraseUpdateAction(action) => {
                form.passphrase_update_action = action;
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
        let passphrase_update_action = form.passphrase_update_action;
        let existing_host_secret_ref = form.host_secret_ref.clone();
        let existing_host_passphrase_ref = form.host_passphrase_ref.clone();
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
            HostAuthSourceKind::InheritGroup => Some("inherit_group".to_string()),
            _ => None,
        };
        let host_key_path_to_save = if wants_host_secret
            && credential_kind == shellumi_core::CredentialKind::SshKey
            && !key_path.is_empty()
        {
            Some(key_path.clone())
        } else {
            None
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
                let vid = shellumi_core::VaultId::new(&active_vault_id_task)
                    .map_err(|e| e.to_string())?;
                store.set_vault_id(vid.clone());

                let credential_store = shellumi_core::default_credential_store();
                let vault_store =
                    shellumi_core::VaultCredentialStore::new(credential_store.as_ref(), &vid);
                let op_id = shellumi_core::VaultCredentialStore::generate_credential_id()
                    .map_err(|e| e.to_string())?;
                let now_ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                let mut planned_new_refs: Vec<String> = Vec::new();
                let mut old_refs: Vec<String> = Vec::new();
                let mut staged_secrets: Vec<(String, String)> = Vec::new();

                let mut new_identity: Option<shellumi_core::NewIdentity> = None;
                if wants_new_identity {
                    let password_ref = if !raw_password.is_empty() {
                        let r = vault_store
                            .plan_unique_ref("identity-password")
                            .map_err(|e| e.to_string())?;
                        planned_new_refs.push(r.clone());
                        staged_secrets.push((r.clone(), raw_password.clone()));
                        Some(r)
                    } else {
                        None
                    };
                    let passphrase_ref = if !raw_passphrase.is_empty() {
                        let r = vault_store
                            .plan_unique_ref("identity-passphrase")
                            .map_err(|e| e.to_string())?;
                        planned_new_refs.push(r.clone());
                        staged_secrets.push((r.clone(), raw_passphrase.clone()));
                        Some(r)
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
                let mut host_passphrase_ref_to_save = existing_host_passphrase_ref.clone();
                if wants_host_secret {
                    if credential_kind == shellumi_core::CredentialKind::SshKey {
                        if let Some(prev) = existing_host_secret_ref.clone() {
                            old_refs.push(prev);
                        }
                        host_secret_ref_to_save = None;
                        plan_host_secret_update(
                            &vault_store,
                            "host-passphrase",
                            passphrase_update_action,
                            &raw_passphrase,
                            &mut host_passphrase_ref_to_save,
                            &mut old_refs,
                            &mut planned_new_refs,
                            &mut staged_secrets,
                        )?;
                    } else {
                        if let Some(prev_pass) = existing_host_passphrase_ref.clone() {
                            old_refs.push(prev_pass);
                        }
                        host_passphrase_ref_to_save = None;
                        plan_host_secret_update(
                            &vault_store,
                            "host-password",
                            secret_update_action,
                            &raw_password,
                            &mut host_secret_ref_to_save,
                            &mut old_refs,
                            &mut planned_new_refs,
                            &mut staged_secrets,
                        )?;
                    }
                } else {
                    if let Some(prev) = existing_host_secret_ref.clone() {
                        old_refs.push(prev);
                    }
                    if let Some(prev_pass) = existing_host_passphrase_ref.clone() {
                        old_refs.push(prev_pass);
                    }
                    host_secret_ref_to_save = None;
                    host_passphrase_ref_to_save = None;
                }

                // Unconditionally normalize non-secret auth metadata in protocol_config JSON
                {
                    let mut map = proto_config
                        .as_deref()
                        .and_then(|s| {
                            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(s)
                                .ok()
                        })
                        .unwrap_or_default();
                    for stale_key in [
                        "host_secret_ref",
                        "host_key_path",
                        "host_passphrase_ref",
                        "auth_mode",
                    ] {
                        map.remove(stale_key);
                    }
                    if let Some(ref r) = host_secret_ref_to_save {
                        map.insert(
                            "host_secret_ref".to_string(),
                            serde_json::Value::String(r.clone()),
                        );
                    }
                    if let Some(ref kp) = host_key_path_to_save {
                        map.insert(
                            "host_key_path".to_string(),
                            serde_json::Value::String(kp.clone()),
                        );
                    }
                    if let Some(ref pr) = host_passphrase_ref_to_save {
                        map.insert(
                            "host_passphrase_ref".to_string(),
                            serde_json::Value::String(pr.clone()),
                        );
                    }
                    if let Some(ref m) = auth_mode_str {
                        map.insert(
                            "auth_mode".to_string(),
                            serde_json::Value::String(m.clone()),
                        );
                    }
                    proto_config = if map.is_empty() {
                        None
                    } else {
                        serde_json::to_string(&map).ok()
                    };
                }

                // Step 2: Persist durable credential operation journal (Planned)
                let _op_guard = HostStore::acquire_credential_op_guard(&op_id);
                let has_journal = !planned_new_refs.is_empty() || !old_refs.is_empty();
                if has_journal {
                    let op_record = shellumi_storage::CredentialOperationRecord {
                        operation_id: op_id.clone(),
                        vault_id: active_vault_id_task.clone(),
                        target_host_id: id,
                        target_identity_id: identity_id,
                        operation_type: if wants_new_identity {
                            "save_host_with_new_identity".to_string()
                        } else {
                            "save_host_credentials".to_string()
                        },
                        old_refs: old_refs.clone(),
                        planned_new_refs: planned_new_refs.clone(),
                        phase: shellumi_storage::CredentialOpPhase::Planned,
                        cleanup_status: shellumi_storage::CredentialCleanupStatus::Pending,
                        last_error: None,
                        updated_at: now_ts,
                    };
                    store
                        .record_credential_operation(&op_record)
                        .await
                        .map_err(|e| e.to_string())?;
                }

                // Step 3: Stage new credentials in the vault-scoped secure store
                let mut staged_refs: Vec<String> = Vec::new();
                for (planned_ref, secret) in &staged_secrets {
                    if let Err(e) = vault_store.store_planned(planned_ref, secret) {
                        for r in &staged_refs {
                            let _ = vault_store.delete(r);
                        }
                        if has_journal {
                            let _ = store
                                .update_credential_operation_phase(
                                    &op_id,
                                    shellumi_storage::CredentialOpPhase::RolledBack,
                                    shellumi_storage::CredentialCleanupStatus::Completed,
                                    Some(&e.to_string()),
                                )
                                .await;
                        }
                        return Err(format!("Failed to store credential: {e}"));
                    }
                    staged_refs.push(planned_ref.clone());
                }
                if has_journal && !staged_refs.is_empty() {
                    store
                        .update_credential_operation_phase(
                            &op_id,
                            shellumi_storage::CredentialOpPhase::Staged,
                            shellumi_storage::CredentialCleanupStatus::Pending,
                            None,
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                }

                let tx_res = store
                    .save_host_with_identity_and_journal_tx(
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
                        if has_journal {
                            Some(op_id.as_str())
                        } else {
                            None
                        },
                        is_remote,
                    )
                    .await;

                let (saved_host_id, saved_identity_id) = match tx_res {
                    Ok(pair) => pair,
                    Err(error) => {
                        let mut rollback_ok = true;
                        let mut rollback_err = error.to_string();
                        for r in &staged_refs {
                            if let Err(del_err) = vault_store.delete(r) {
                                rollback_ok = false;
                                rollback_err =
                                    format!("{rollback_err}; rollback delete failed: {del_err}");
                            }
                        }
                        if has_journal {
                            let _ = store
                                .update_credential_operation_phase(
                                    &op_id,
                                    shellumi_storage::CredentialOpPhase::RolledBack,
                                    if rollback_ok {
                                        shellumi_storage::CredentialCleanupStatus::Completed
                                    } else {
                                        shellumi_storage::CredentialCleanupStatus::RetryPending
                                    },
                                    Some(&rollback_err),
                                )
                                .await;
                        }
                        return Err(error.to_string());
                    }
                };

                if has_journal {
                    let mut cleanup_ok = true;
                    let mut cleanup_err = None;
                    for old_r in &old_refs {
                        if !store.is_credential_ref_in_use(old_r).await.unwrap_or(true)
                            && vault_store.validate_reference(old_r).is_ok()
                        {
                            if let Err(e) = vault_store.delete(old_r) {
                                cleanup_ok = false;
                                cleanup_err = Some(e.to_string());
                            }
                        }
                    }
                    let _ = store
                        .update_credential_operation_phase(
                            &op_id,
                            if cleanup_ok {
                                shellumi_storage::CredentialOpPhase::Completed
                            } else {
                                shellumi_storage::CredentialOpPhase::Committed
                            },
                            if cleanup_ok {
                                shellumi_storage::CredentialCleanupStatus::Completed
                            } else {
                                shellumi_storage::CredentialCleanupStatus::RetryPending
                            },
                            cleanup_err.as_deref(),
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
                let vid =
                    shellumi_core::VaultId::new(&active_vault_id).map_err(|e| e.to_string())?;
                store.set_vault_id(vid.clone());
                let existing_host = store.get_host(id).await.map_err(|e| e.to_string())?;
                let host_name = existing_host.as_ref().map(|h| h.name.clone());
                let mut old_refs = Vec::new();
                if let Some(cfg) = existing_host
                    .as_ref()
                    .and_then(|h| h.protocol_config.as_deref())
                    .and_then(|c| serde_json::from_str::<crate::HostAuthMetadata>(c).ok())
                {
                    if let Some(r) = cfg.host_secret_ref {
                        old_refs.push(r);
                    }
                    if let Some(r) = cfg.host_passphrase_ref {
                        old_refs.push(r);
                    }
                }
                let op_id = shellumi_core::VaultCredentialStore::generate_credential_id()
                    .map_err(|e| e.to_string())?;
                let _op_guard = HostStore::acquire_credential_op_guard(&op_id);
                let has_journal = !old_refs.is_empty();
                if has_journal {
                    let now_ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    store
                        .record_credential_operation(&shellumi_storage::CredentialOperationRecord {
                            operation_id: op_id.clone(),
                            vault_id: active_vault_id.clone(),
                            target_host_id: Some(id),
                            target_identity_id: None,
                            operation_type: "delete_host".to_string(),
                            old_refs: old_refs.clone(),
                            planned_new_refs: Vec::new(),
                            phase: shellumi_storage::CredentialOpPhase::Planned,
                            cleanup_status: shellumi_storage::CredentialCleanupStatus::Pending,
                            last_error: None,
                            updated_at: now_ts,
                        })
                        .await
                        .map_err(|e| e.to_string())?;
                }
                let sync_mutation = if is_remote {
                    host_name
                        .as_deref()
                        .map(|hname| shellumi_storage::PendingSyncMutation {
                            entity_id: hname,
                            entity_type: "host",
                            version: 0,
                            deleted: true,
                            payload_json: None,
                        })
                } else {
                    None
                };
                let deleted = store
                    .delete_host_with_journal_tx(
                        id,
                        if has_journal {
                            Some(op_id.as_str())
                        } else {
                            None
                        },
                        sync_mutation.as_ref(),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                if has_journal && deleted {
                    let cred_store = shellumi_core::default_credential_store();
                    let vault_store =
                        shellumi_core::VaultCredentialStore::new(cred_store.as_ref(), &vid);
                    let mut cleanup_ok = true;
                    let mut cleanup_err = None;
                    for r in &old_refs {
                        if !store.is_credential_ref_in_use(r).await.unwrap_or(true)
                            && vault_store.validate_reference(r).is_ok()
                        {
                            if let Err(e) = vault_store.delete(r) {
                                cleanup_ok = false;
                                cleanup_err = Some(e.to_string());
                            }
                        }
                    }
                    let _ = store
                        .update_credential_operation_phase(
                            &op_id,
                            if cleanup_ok {
                                shellumi_storage::CredentialOpPhase::Completed
                            } else {
                                shellumi_storage::CredentialOpPhase::Committed
                            },
                            if cleanup_ok {
                                shellumi_storage::CredentialCleanupStatus::Completed
                            } else {
                                shellumi_storage::CredentialCleanupStatus::RetryPending
                            },
                            cleanup_err.as_deref(),
                        )
                        .await;
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
                let cred_store = shellumi_core::default_credential_store();
                if let Err(err) = store
                    .reconcile_credential_operations(cred_store.as_ref())
                    .await
                {
                    tracing::warn!(
                        vault_id = %active_vault_id_task,
                        error = %err,
                        "Credential operation reconciliation encountered an error"
                    );
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

#[allow(clippy::too_many_arguments)]
fn plan_host_secret_update(
    vault_store: &shellumi_core::VaultCredentialStore<'_>,
    purpose: &str,
    action: shellumi_core::SecretUpdateAction,
    raw_secret: &str,
    target_ref: &mut Option<String>,
    old_refs: &mut Vec<String>,
    planned_new_refs: &mut Vec<String>,
    staged_secrets: &mut Vec<(String, String)>,
) -> Result<(), String> {
    match action {
        shellumi_core::SecretUpdateAction::Remove => {
            if let Some(prev) = target_ref.take() {
                old_refs.push(prev);
            }
        }
        shellumi_core::SecretUpdateAction::Keep
            if target_ref.is_some() && raw_secret.is_empty() => {}
        _ => {
            if !raw_secret.is_empty() {
                if let Some(prev) = target_ref.take() {
                    old_refs.push(prev);
                }
                let new_ref = vault_store
                    .plan_unique_ref(purpose)
                    .map_err(|e| e.to_string())?;
                planned_new_refs.push(new_ref.clone());
                staged_secrets.push((new_ref.clone(), raw_secret.to_string()));
                *target_ref = Some(new_ref);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HostAuthMetadata, HostAuthSourceKind, HostPersistenceChoice, PersistedHostAuth};
    use shellumi_core::{
        ConnectionProtocol, CredentialKind, EphemeralCredentialStore, SecretUpdateAction,
        VaultCredentialStore, VaultId,
    };

    #[test]
    fn test_persisted_host_auth_restores_ssh_key_passphrase_and_inherit_group() {
        let key_meta = serde_json::to_string(&HostAuthMetadata {
            host_secret_ref: None,
            host_key_path: Some("~/.ssh/id_ed25519".to_string()),
            host_passphrase_ref: Some("keychain:shellumi:vault-v1-abc".to_string()),
            auth_mode: None,
        })
        .unwrap();
        let restored = PersistedHostAuth::from_host(ConnectionProtocol::Ssh, None, Some(&key_meta));
        assert_eq!(
            restored.auth_source_kind,
            HostAuthSourceKind::EnterCredentials
        );
        assert_eq!(restored.credential_kind, CredentialKind::SshKey);
        assert_eq!(
            restored.persistence_choice,
            HostPersistenceChoice::SaveForThisHost
        );
        assert_eq!(restored.host_key_path.as_deref(), Some("~/.ssh/id_ed25519"));
        assert_eq!(
            restored.host_passphrase_ref.as_deref(),
            Some("keychain:shellumi:vault-v1-abc")
        );
        assert!(restored.has_host_secret);
        assert!(restored.has_host_passphrase);

        let inherit_meta = serde_json::to_string(&HostAuthMetadata {
            auth_mode: Some("inherit_group".to_string()),
            ..Default::default()
        })
        .unwrap();
        let inherited =
            PersistedHostAuth::from_host(ConnectionProtocol::Rdp, None, Some(&inherit_meta));
        assert_eq!(inherited.auth_source_kind, HostAuthSourceKind::InheritGroup);
    }

    #[test]
    fn test_plan_host_secret_update_keep_replace_remove_semantics() {
        let store = EphemeralCredentialStore::new();
        let vid = VaultId::new("vault-host-edit").unwrap();
        let vault_store = VaultCredentialStore::new(&store, &vid);
        let initial_ref = vault_store
            .store_unique("host-passphrase", "initial-pass")
            .unwrap();

        // Keep with empty input preserves existing ref without staging or deleting.
        let mut target = Some(initial_ref.clone());
        let mut old_refs = Vec::new();
        let mut planned = Vec::new();
        let mut staged = Vec::new();
        plan_host_secret_update(
            &vault_store,
            "host-passphrase",
            SecretUpdateAction::Keep,
            "",
            &mut target,
            &mut old_refs,
            &mut planned,
            &mut staged,
        )
        .unwrap();
        assert_eq!(target.as_deref(), Some(initial_ref.as_str()));
        assert!(old_refs.is_empty());
        assert!(planned.is_empty());

        // Replace stages a new ref and schedules old ref for post-commit cleanup.
        plan_host_secret_update(
            &vault_store,
            "host-passphrase",
            SecretUpdateAction::Replace,
            "rotated-pass",
            &mut target,
            &mut old_refs,
            &mut planned,
            &mut staged,
        )
        .unwrap();
        assert_ne!(target.as_deref(), Some(initial_ref.as_str()));
        assert_eq!(old_refs, vec![initial_ref.clone()]);
        assert_eq!(planned.len(), 1);
        assert_eq!(staged.len(), 1);

        // Remove clears the ref and schedules it for cleanup.
        let replaced_ref = target.clone().unwrap();
        old_refs.clear();
        planned.clear();
        staged.clear();
        plan_host_secret_update(
            &vault_store,
            "host-passphrase",
            SecretUpdateAction::Remove,
            "",
            &mut target,
            &mut old_refs,
            &mut planned,
            &mut staged,
        )
        .unwrap();
        assert_eq!(target, None);
        assert_eq!(old_refs, vec![replaced_ref]);
    }
}

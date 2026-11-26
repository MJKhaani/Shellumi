#![allow(dead_code)]
//! Host inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{inline_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout;
use crate::{format_endpoint, ConnectionProtocol, HostEditorCategory, SavedHost, Shellumi};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};

impl Shellumi {
    /// Render the unified Host Inspector for viewing or editing a host.
    pub(crate) fn render_host_inspector(
        &mut self,
        host_id: Option<i64>,
        mode: InspectorMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_host_view_inspector(host_id, window, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_host_edit_inspector(host_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render host inspector in View mode.
    fn render_host_view_inspector(
        &mut self,
        host_id: Option<i64>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let hosts = match &self.hosts {
            crate::HostState::Loaded(hosts) => hosts.as_slice(),
            _ => &[],
        };

        let host = host_id.and_then(|id| hosts.iter().find(|h| h.id == id));

        let Some(host) = host else {
            return InspectorShell::new(
                div().p_4().child("Host not found"),
                div()
                    .p_4()
                    .child("The selected host does not exist or was deleted."),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let host_clone = host.clone();
        let sftp_host = host.clone();
        let dup_host = host.clone();
        let host_name = host.name.clone();
        let id_val = host.id;
        let supports_sftp = host.protocol().supports_sftp();
        let protocol_display = host.protocol().display_name().to_string();

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x1e3a5f))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0x389bf2))
            .child(protocol_display.clone());

        let edit_btn = div()
            .id("inspector-edit-host-btn")
            .px_2p5()
            .py_1()
            .rounded_md()
            .bg(surface())
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(text_primary())
            .cursor_pointer()
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(move |app, _, _, cx| {
                app.open_host_inspector(Some(id_val), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let close_btn = InspectorShell::close_button(
            "close-host-inspector-btn",
            false,
            |app, _, _, cx| {
                app.inspector_close(cx);
            },
            cx,
        );

        let header = InspectorShell::header(
            host.name.clone(),
            Some(format_endpoint(
                host.destination.username(),
                host.destination.host(),
                host.destination.port().get(),
            )),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            Some(close_btn),
        );

        // Body content
        let mut body = div().flex().flex_col().gap_4();

        // Action buttons row
        let mut actions_row = div().flex().gap_2();

        // Connect button
        let connect_label = if host.protocol().is_graphical() {
            "Connect Desktop"
        } else {
            "Connect"
        };
        actions_row = actions_row.child(
            AppButton::new(
                "inspector-connect-btn",
                connect_label,
                ButtonVariant::Primary,
            )
            .on_click(cx.listener(move |app, _, window, cx| {
                app.connect(&host_clone, window, cx);
            })),
        );

        // SFTP button (if supported)
        if supports_sftp {
            actions_row = actions_row.child(
                AppButton::new("inspector-sftp-btn", "SFTP", ButtonVariant::Secondary).on_click(
                    cx.listener(move |app, _, window, cx| {
                        app.connect_sftp(&sftp_host, window, cx);
                    }),
                ),
            );
        }

        // Duplicate button
        actions_row = actions_row.child(
            AppButton::new("inspector-duplicate-btn", "Duplicate", ButtonVariant::Ghost).on_click(
                cx.listener(move |app, _, _, cx| {
                    app.duplicate_host(&dup_host, cx);
                }),
            ),
        );

        body = body.child(actions_row);

        // Connection section
        let mut conn_items = div().flex().flex_col().gap_1();
        conn_items = conn_items
            .child(self.render_detail_row(
                "Destination",
                format_endpoint(
                    host.destination.username(),
                    host.destination.host(),
                    host.destination.port().get(),
                ),
            ))
            .child(self.render_detail_row("Protocol", protocol_display))
            .child(self.render_detail_row("Port", host.destination.port().to_string()));
        if !host.destination.username().is_empty() {
            conn_items = conn_items
                .child(self.render_detail_row("Username", host.destination.username().to_string()));
        }
        body = body.child(simple_section("Connection Details", conn_items));

        // Group & Tags section
        let group_name_str = host.group_name.clone().unwrap_or_else(|| "— (Root)".into());
        let group_for_nav = host.group_name.clone();

        let group_row = div()
            .flex()
            .items_center()
            .justify_between()
            .py_1()
            .child(div().text_xs().text_color(text_secondary()).child("Group"))
            .child(
                div()
                    .id("inspector-host-group-link")
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(accent_blue())
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if let Some(ref g_name) = group_for_nav {
                            app.inspector_select_group(g_name, cx);
                        }
                    }))
                    .child(group_name_str),
            );

        let tags_str = if host.tags.is_empty() {
            "—".into()
        } else {
            host.tags.join(", ")
        };
        let mut org_items = div().flex().flex_col().gap_1();
        org_items = org_items
            .child(group_row)
            .child(self.render_detail_row("Tags", tags_str));
        body = body.child(simple_section("Organization", org_items));

        // Authentication & Security (protocol-aware)
        let caps = shellumi_core::ProtocolCapabilities::for_protocol(host.protocol());
        let auth_meta = host
            .protocol_config
            .as_deref()
            .and_then(|c| serde_json::from_str::<crate::HostAuthMetadata>(c).ok())
            .unwrap_or_default();

        let (auth_label, auth_value) = if !caps.allows_saved_identity && !caps.allows_ask_on_connect
        {
            if caps.allows_no_auth {
                ("Authentication", "No remote authentication".to_string())
            } else {
                ("Authentication", "Interactive remote login".to_string())
            }
        } else if let Some(identity_id) = host.identity_id {
            let id_summary = self
                .identities
                .iter()
                .find(|i| i.id == identity_id)
                .map(|i| format!("{} ({})", i.name, i.auth_method))
                .unwrap_or_else(|| format!("Identity #{}", identity_id));
            ("Saved Identity", id_summary)
        } else if auth_meta.host_secret_ref.is_some()
            || auth_meta.host_key_path.is_some()
            || auth_meta.host_passphrase_ref.is_some()
        {
            (
                "Credential",
                if let Some(kp) = &auth_meta.host_key_path {
                    format!("Host-private SSH key ({kp})")
                } else {
                    "Host-private credential configured".to_string()
                },
            )
        } else if auth_meta.auth_mode.as_deref() == Some("inherit_group") {
            ("Authentication", "Inherit from host group".to_string())
        } else if auth_meta.auth_mode.as_deref() == Some("no_auth") && caps.allows_no_auth {
            ("Authentication", "No authentication".to_string())
        } else {
            ("Authentication", "Ask on connect".to_string())
        };

        let mut auth_items = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(self.render_detail_row(auth_label, auth_value));
        if caps.supports_username && !host.destination.username().is_empty() {
            auth_items = auth_items
                .child(self.render_detail_row("Username", host.destination.username().to_string()));
        }
        if host.is_rdp() {
            if let Some(cfg) = host.rdp_config() {
                if let Some(ref dom) = cfg.domain {
                    if !dom.is_empty() {
                        auth_items =
                            auth_items.child(self.render_detail_row("Domain", dom.clone()));
                    }
                }
            }
        }
        body = body.child(simple_section("Authentication", auth_items));

        // Proxy / Jump Host (if present)
        if let Some(ref jump) = host.jump_host {
            if !jump.trim().is_empty() {
                let proxy_items = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(self.render_detail_row("Jump Host", jump.clone()));
                body = body.child(simple_section("Proxy", proxy_items));
            }
        }

        // Protocol Specifics (RDP / VNC)
        if host.is_rdp() {
            if let Some(cfg) = host.rdp_config() {
                let rdp_items = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(self.render_detail_row(
                        "Resolution",
                        format!("{}x{}", cfg.screen_width, cfg.screen_height),
                    ))
                    .child(
                        self.render_detail_row("Color Depth", format!("{} bit", cfg.color_depth)),
                    )
                    .child(self.render_detail_row(
                        "View Only",
                        if cfg.view_only { "Yes" } else { "No" }.to_string(),
                    ))
                    .child(
                        self.render_detail_row(
                            "TLS Strict",
                            if cfg.tls_strict {
                                "Enabled"
                            } else {
                                "Disabled"
                            }
                            .to_string(),
                        ),
                    );
                body = body.child(simple_section("RDP Options", rdp_items));
            }
        } else if host.is_vnc() {
            if let Some(cfg) = host.vnc_config() {
                let vnc_items = div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        self.render_detail_row(
                            "Resolution",
                            format!("{}x{}", cfg.width, cfg.height),
                        ),
                    )
                    .child(
                        self.render_detail_row("Color Depth", format!("{} bit", cfg.color_depth)),
                    )
                    .child(self.render_detail_row(
                        "View Only",
                        if cfg.view_only { "Yes" } else { "No" }.to_string(),
                    ));
                body = body.child(simple_section("VNC Options", vnc_items));
            }
        }

        // Destructive Action: Delete host
        let delete_row = div()
            .pt_4()
            .border_t_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(
                AppButton::new(
                    "inspector-delete-host-btn",
                    "Delete Host",
                    ButtonVariant::Danger,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.delete_confirm = Some(crate::DeleteTarget::Host(id_val, host_name.clone()));
                    cx.notify();
                })),
            );
        body = body.child(delete_row);

        InspectorShell::new(header, body, None::<Div>, self.active_inspector_width())
    }

    /// Render host inspector in Edit or Create mode.
    fn render_host_edit_inspector(
        &mut self,
        _host_id: Option<i64>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(form) = &self.form else {
            return InspectorShell::new(
                div().p_4().child("Form not initialized"),
                div().p_4().child("Host form data is missing."),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let title = if is_create { "New Host" } else { "Edit Host" };

        let back_btn = InspectorShell::close_button(
            "close-host-edit-inspector-btn",
            true,
            |app, _, _, cx| {
                app.inspector_back(cx);
            },
            cx,
        );

        let header = InspectorShell::header(
            title,
            Some("Configure connection and credentials"),
            None,
            None,
            Some(back_btn),
        );

        let proto = form.protocol;
        let draft_gen = form.draft_generation;
        let caps = shellumi_core::ProtocolCapabilities::for_protocol(proto);

        let mut editor_categories = vec![
            (HostEditorCategory::General, "General"),
            (HostEditorCategory::Connection, "Connection"),
        ];
        if caps.allows_saved_identity
            || caps.allows_ask_on_connect
            || caps.cleartext_warning.is_some()
        {
            editor_categories.push((HostEditorCategory::Authentication, "Auth"));
        }
        if caps.supports_jump_host {
            editor_categories.push((HostEditorCategory::Proxy, "Proxy"));
        }
        if caps.supports_startup_snippet || caps.supports_display_options {
            editor_categories.push((HostEditorCategory::Advanced, "Advanced"));
        }
        let active_cat = if editor_categories
            .iter()
            .any(|(c, _)| *c == form.active_category)
        {
            form.active_category
        } else {
            HostEditorCategory::General
        };

        let mut cat_tabs = div()
            .flex()
            .items_center()
            .gap_1()
            .pb_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER));

        for (cat, label) in editor_categories {
            let is_sel = cat == active_cat;
            cat_tabs = cat_tabs.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "tab-{}",
                        label.to_lowercase()
                    )))
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .font_weight(if is_sel {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if is_sel {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        surface()
                    })
                    .text_color(if is_sel {
                        accent_blue()
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.set_host_editor_category(cat, cx);
                    }))
                    .child(label),
            );
        }

        // Form category body
        let mut form_body = div().flex().flex_col().gap_3().child(cat_tabs);

        if let Some(warning_msg) = caps.cleartext_warning {
            form_body = form_body.child(crate::ui::components::inline_notice(
                None::<gpui::AnyElement>,
                warning_msg,
                true,
            ));
        }

        match active_cat {
            HostEditorCategory::General => {
                form_body = form_body
                    .child(self.render_protocol_selector(proto, draft_gen, cx))
                    .child(simple_field("Host Name", form.name.clone()))
                    .child(simple_field("Group (optional)", form.group.clone()))
                    .child(simple_field("Tags (comma separated)", form.tags.clone()));
            }
            HostEditorCategory::Connection => {
                form_body = form_body.child(self.render_protocol_selector(proto, draft_gen, cx));
                if proto == ConnectionProtocol::Serial {
                    form_body = form_body
                        .child(simple_field("Serial Device Path", form.hostname.clone()))
                        .child(simple_field("Baud Rate", form.serial_baud.clone()))
                        .child(self.render_serial_options(
                            draft_gen,
                            form.serial_data_bits,
                            form.serial_parity,
                            form.serial_stop_bits,
                            form.serial_flow_control,
                            cx,
                        ));
                } else if proto == ConnectionProtocol::Local {
                    form_body = form_body.child(crate::ui::components::inline_notice(
                        None::<gpui::AnyElement>,
                        "Launches your local system shell without remote network credentials.",
                        false,
                    ));
                } else {
                    form_body = form_body
                        .child(simple_field("Hostname or IP", form.hostname.clone()))
                        .child(simple_field(
                            if proto == ConnectionProtocol::Mosh {
                                "SSH Bootstrap Port"
                            } else {
                                "Port"
                            },
                            form.port.clone(),
                        ));
                }
            }
            HostEditorCategory::Authentication => {
                form_body = form_body.child(self.render_host_auth_section(form, &caps, cx));
            }
            HostEditorCategory::Proxy => {
                form_body =
                    form_body.child(simple_field("Jump Host / Bastion", form.jump_host.clone()));
            }
            HostEditorCategory::Advanced => {
                if caps.supports_startup_snippet {
                    form_body = form_body.child(simple_field(
                        "Startup Snippet",
                        form.startup_snippet.clone(),
                    ));
                }

                if caps.supports_display_options {
                    form_body = form_body
                        .child(simple_field("Display Width", form.rd_width.clone()))
                        .child(simple_field("Display Height", form.rd_height.clone()))
                        .child(self.render_rd_options(form, cx));
                }
            }
        }

        // Footer with Cancel, Save, and optional error notice
        let mut footer = div()
            .p_4()
            .border_t_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(surface())
            .flex()
            .flex_col()
            .gap_2();

        if let Some(ref err) = self.form_error {
            footer = footer.child(crate::ui::components::error_notice(err.clone()));
        }

        let is_saving = self.saving;
        let buttons_row = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(
                AppButton::new("inspector-cancel-btn", "Cancel", ButtonVariant::Ghost).on_click(
                    cx.listener(|app, _, _, cx| {
                        if !app.saving {
                            app.inspector_back(cx);
                        }
                    }),
                ),
            )
            .child(if is_saving {
                AppButton::new("inspector-save-btn", "Saving…", ButtonVariant::Secondary)
            } else {
                AppButton::new("inspector-save-btn", "Save", ButtonVariant::Primary).on_click(
                    cx.listener(|app, _, _, cx| {
                        app.save_host(cx);
                    }),
                )
            });

        footer = footer.child(buttons_row);

        InspectorShell::new(
            header,
            form_body,
            Some(footer),
            self.active_inspector_width(),
        )
    }

    /// Helper to render protocol selector pills.
    fn render_protocol_selector(
        &self,
        current: ConnectionProtocol,
        draft_gen: u64,
        cx: &mut Context<Self>,
    ) -> Div {
        let protocols = [
            ConnectionProtocol::Ssh,
            ConnectionProtocol::Rdp,
            ConnectionProtocol::Vnc,
            ConnectionProtocol::Serial,
            ConnectionProtocol::Telnet,
            ConnectionProtocol::Mosh,
            ConnectionProtocol::Local,
        ];

        let mut row = div().flex().flex_wrap().gap_1p5().py_1();

        for proto in protocols {
            let is_sel = proto == current;
            row = row.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "proto-opt-{}",
                        proto.display_name()
                    )))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .font_weight(if is_sel {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if is_sel {
                        accent_blue()
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .hover(|s| {
                        if !is_sel {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        } else {
                            s
                        }
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(
                            draft_gen,
                            crate::HostEditEvent::SetProtocol(proto),
                            cx,
                        );
                    }))
                    .child(proto.display_name()),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(text_secondary())
                    .child("Protocol"),
            )
            .child(row)
    }

    /// Render capability-driven Authentication section in Host Edit/Create mode.
    fn render_host_auth_section(
        &self,
        form: &crate::HostForm,
        caps: &shellumi_core::ProtocolCapabilities,
        cx: &mut Context<Self>,
    ) -> Div {
        let draft_gen = form.draft_generation;
        let mut section = div().flex().flex_col().gap_3();

        if caps.supports_username {
            section = section.child(simple_field("Username", form.username.clone()));
        }
        if caps.supports_domain {
            section = section.child(simple_field("Domain (optional)", form.rd_domain.clone()));
        }

        if !caps.allows_saved_identity && !caps.allows_ask_on_connect {
            return section.child(crate::ui::components::inline_notice(
                None::<gpui::AnyElement>,
                "Credentials are entered directly in the remote session after connecting.",
                false,
            ));
        }

        // Credential source selector
        let mut sources = Vec::new();
        if caps.allows_saved_identity {
            sources.push((
                crate::HostAuthSourceKind::ExistingIdentity,
                "Existing Identity",
            ));
            sources.push((
                crate::HostAuthSourceKind::EnterCredentials,
                "Enter Credentials",
            ));
        }
        if caps.allows_ask_on_connect {
            sources.push((crate::HostAuthSourceKind::AskOnConnect, "Ask on Connect"));
        }
        if caps.allows_group_inheritance {
            sources.push((crate::HostAuthSourceKind::InheritGroup, "Inherit Group"));
        }
        if caps.allows_no_auth {
            sources.push((crate::HostAuthSourceKind::NoAuthentication, "No Auth"));
        }

        let mut source_row = div().flex().flex_wrap().gap_1p5();
        for (idx, (src, label)) in sources.into_iter().enumerate() {
            let is_sel = form.auth_source_kind == src;
            source_row = source_row.child(
                div()
                    .id(("host-auth-src", idx))
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .font_weight(if is_sel {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if is_sel {
                        accent_blue()
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(
                            draft_gen,
                            crate::HostEditEvent::SetAuthSource(src),
                            cx,
                        );
                    }))
                    .child(label),
            );
        }

        section = section.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(text_secondary())
                        .child("Credential Source"),
                )
                .child(source_row),
        );

        match form.auth_source_kind {
            crate::HostAuthSourceKind::ExistingIdentity => {
                section = section.child(self.render_compatible_identity_picker(
                    form.identity_id,
                    draft_gen,
                    caps,
                    cx,
                ));
            }
            crate::HostAuthSourceKind::EnterCredentials => {
                // Method selector when multiple methods exist
                if caps.supported_credentials.len() > 1 {
                    let mut method_row = div().flex().flex_wrap().gap_1p5();
                    for (idx, &kind) in caps.supported_credentials.iter().enumerate() {
                        if kind == shellumi_core::CredentialKind::None {
                            continue;
                        }
                        let is_sel = form.credential_kind == kind;
                        method_row = method_row.child(
                            div()
                                .id(("host-cred-kind", idx))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .bg(if is_sel {
                                    rgb(MacTheme::SURFACE_ACTIVE)
                                } else {
                                    rgb(MacTheme::INPUT_BG)
                                })
                                .text_color(if is_sel {
                                    accent_blue()
                                } else {
                                    text_secondary()
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.apply_host_edit(
                                        draft_gen,
                                        crate::HostEditEvent::SetCredentialKind(kind),
                                        cx,
                                    );
                                }))
                                .child(kind.display_name()),
                        );
                    }
                    section = section.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text_secondary())
                                    .child("Authentication Method"),
                            )
                            .child(method_row),
                    );
                }

                if form.host_secret_ref.is_some()
                    && form.credential_kind != shellumi_core::CredentialKind::SshKey
                {
                    let mut action_row = div().flex().gap_1p5();
                    for (idx, (act, label)) in [
                        (shellumi_core::SecretUpdateAction::Keep, "Keep Saved"),
                        (shellumi_core::SecretUpdateAction::Replace, "Replace"),
                        (shellumi_core::SecretUpdateAction::Remove, "Remove"),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let is_sel = form.secret_update_action == act;
                        action_row = action_row.child(
                            div()
                                .id(("host-secret-act", idx))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .bg(if is_sel {
                                    rgb(MacTheme::SURFACE_ACTIVE)
                                } else {
                                    rgb(MacTheme::INPUT_BG)
                                })
                                .text_color(if is_sel {
                                    accent_blue()
                                } else {
                                    text_secondary()
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.apply_host_edit(
                                        draft_gen,
                                        crate::HostEditEvent::SetSecretUpdateAction(act),
                                        cx,
                                    );
                                }))
                                .child(label),
                        );
                    }
                    section = section.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text_secondary())
                                    .child("Saved Host Password"),
                            )
                            .child(action_row),
                    );
                }

                if form.has_host_passphrase
                    && form.credential_kind == shellumi_core::CredentialKind::SshKey
                {
                    let mut pass_action_row = div().flex().gap_1p5();
                    for (idx, (act, label)) in [
                        (shellumi_core::SecretUpdateAction::Keep, "Keep Saved"),
                        (shellumi_core::SecretUpdateAction::Replace, "Replace"),
                        (shellumi_core::SecretUpdateAction::Remove, "Remove"),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let is_sel = form.passphrase_update_action == act;
                        pass_action_row = pass_action_row.child(
                            div()
                                .id(("host-passphrase-act", idx))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .bg(if is_sel {
                                    rgb(MacTheme::SURFACE_ACTIVE)
                                } else {
                                    rgb(MacTheme::INPUT_BG)
                                })
                                .text_color(if is_sel {
                                    accent_blue()
                                } else {
                                    text_secondary()
                                })
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.apply_host_edit(
                                        draft_gen,
                                        crate::HostEditEvent::SetPassphraseUpdateAction(act),
                                        cx,
                                    );
                                }))
                                .child(label),
                        );
                    }
                    section = section.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text_secondary())
                                    .child("Saved Key Passphrase"),
                            )
                            .child(pass_action_row),
                    );
                }

                match form.credential_kind {
                    shellumi_core::CredentialKind::SshKey => {
                        section = section
                            .child(crate::ui::components::TextField::new(
                                "Private Key Path",
                                form.auth_key_path.clone(),
                            ))
                            .child(crate::ui::components::PasswordField::new(
                                "Key Passphrase (optional)",
                                form.auth_passphrase.clone(),
                            ));
                    }
                    shellumi_core::CredentialKind::SshAgent
                    | shellumi_core::CredentialKind::Interactive
                    | shellumi_core::CredentialKind::None => {}
                    _ => {
                        section = section.child(crate::ui::components::PasswordField::new(
                            "Password",
                            form.auth_password.clone(),
                        ));
                    }
                }

                // Persistence choice
                let mut persist_row = div().flex().flex_wrap().gap_1p5();
                for (idx, (choice, label)) in [
                    (crate::HostPersistenceChoice::SessionOnly, "Session Only"),
                    (
                        crate::HostPersistenceChoice::SaveForThisHost,
                        "Save for This Host",
                    ),
                    (
                        crate::HostPersistenceChoice::SaveAsIdentity,
                        "Save as Identity",
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let is_sel = form.persistence_choice == choice;
                    persist_row = persist_row.child(
                        div()
                            .id(("host-persist-opt", idx))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .bg(if is_sel {
                                accent_blue()
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(if is_sel {
                                rgb(0xffffff)
                            } else {
                                text_secondary()
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.apply_host_edit(
                                    draft_gen,
                                    crate::HostEditEvent::SetPersistence(choice),
                                    cx,
                                );
                            }))
                            .child(label),
                    );
                }
                section = section.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(text_secondary())
                                .child("Credential Persistence"),
                        )
                        .child(persist_row),
                );

                if form.persistence_choice == crate::HostPersistenceChoice::SessionOnly {
                    section = section.child(crate::ui::components::inline_notice(
                        None::<gpui::AnyElement>,
                        "Entered secrets are not saved to the vault; future sessions will prompt again.",
                        false,
                    ));
                } else if form.persistence_choice == crate::HostPersistenceChoice::SaveAsIdentity {
                    section = section
                        .child(
                            crate::ui::components::TextField::new(
                                "New Identity Name",
                                form.new_identity_name.clone(),
                            )
                            .required(true),
                        )
                        .child(crate::ui::components::TextField::new(
                            "Identity Tags (optional)",
                            form.new_identity_tags.clone(),
                        ));
                }
            }
            crate::HostAuthSourceKind::AskOnConnect => {
                section = section.child(crate::ui::components::inline_notice(
                    None::<gpui::AnyElement>,
                    "You will be prompted for credentials when connecting.",
                    false,
                ));
            }
            crate::HostAuthSourceKind::InheritGroup => {
                section = section.child(crate::ui::components::inline_notice(
                    None::<gpui::AnyElement>,
                    "Uses the parent group's default identity and username if configured.",
                    false,
                ));
            }
            crate::HostAuthSourceKind::NoAuthentication => {
                section = section.child(crate::ui::components::inline_notice(
                    None::<gpui::AnyElement>,
                    "Connects without sending a password or key.",
                    true,
                ));
            }
        }

        section
    }

    /// Helper to render compatible identity picker list for the active protocol.
    fn render_compatible_identity_picker(
        &self,
        selected_id: Option<i64>,
        draft_gen: u64,
        caps: &shellumi_core::ProtocolCapabilities,
        cx: &mut Context<Self>,
    ) -> Div {
        let compatible: Vec<_> = self
            .identities
            .iter()
            .filter(|id| caps.is_auth_method_compatible(&id.auth_method))
            .collect();

        let mut list = div()
            .id("host-inspector-identity-picker")
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(140.0))
            .overflow_y_scroll()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .rounded_md()
            .p_1();

        if compatible.is_empty() {
            list = list.child(
                div()
                    .px_2()
                    .py_1p5()
                    .text_xs()
                    .text_color(text_secondary())
                    .child("No compatible identities in this vault. Choose 'Enter Credentials' -> 'Save as Identity' to create one."),
            );
        } else {
            for (idx, id) in compatible.into_iter().enumerate() {
                let is_sel = selected_id == Some(id.id);
                let id_val = id.id;
                let user_suffix = if id.username.is_empty() {
                    String::new()
                } else {
                    format!(" • {}", id.username)
                };
                list = list.child(
                    div()
                        .id(("host-id-opt", idx))
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .bg(if is_sel {
                            rgb(MacTheme::SURFACE_ACTIVE)
                        } else {
                            rgb(MacTheme::INPUT_BG)
                        })
                        .text_color(if is_sel {
                            accent_blue()
                        } else {
                            text_primary()
                        })
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.apply_host_edit(
                                draft_gen,
                                crate::HostEditEvent::SelectIdentity(Some(id_val)),
                                cx,
                            );
                        }))
                        .child(format!(
                            "{} {}{} ({})",
                            if is_sel { "✓" } else { " " },
                            id.name,
                            user_suffix,
                            id.auth_method
                        )),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(text_secondary())
                    .child("Compatible Vault Identities"),
            )
            .child(list)
    }

    fn render_rd_options(&self, form: &crate::HostForm, cx: &mut Context<Self>) -> Div {
        let draft_gen = form.draft_generation;
        let proto = form.protocol;
        let color_depth = form.rd_color_depth;
        let view_only = form.rd_view_only;
        let tls_strict = form.rd_tls_strict;
        let enable_audio = form.rd_enable_audio;
        let enable_microphone = form.rd_enable_microphone;
        let mut depth_row = div().flex().gap_1p5();
        for depth in [16u8, 24, 32] {
            let is_sel = color_depth == depth;
            depth_row = depth_row.child(
                div()
                    .id(("rd-depth-opt", depth as usize))
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .bg(if is_sel {
                        accent_blue()
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(
                            draft_gen,
                            crate::HostEditEvent::SetRdColorDepth(depth),
                            cx,
                        );
                    }))
                    .child(format!("{depth}-bit")),
            );
        }

        let mut container = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_secondary())
                            .child("Color Depth"),
                    )
                    .child(depth_row),
            )
            .child(
                crate::ui::components::toggle_switch("rd-view-only-toggle", view_only, "View Only")
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(draft_gen, crate::HostEditEvent::ToggleRdViewOnly, cx);
                    })),
            );

        if proto == ConnectionProtocol::Rdp {
            container = container
                .child(
                    crate::ui::components::toggle_switch(
                        "rd-tls-strict-toggle",
                        tls_strict,
                        "Strict TLS / NLA Verification",
                    )
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(draft_gen, crate::HostEditEvent::ToggleRdTlsStrict, cx);
                    })),
                )
                .child(
                    crate::ui::components::toggle_switch(
                        "rd-audio-toggle",
                        enable_audio,
                        "Audio Redirection",
                    )
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(draft_gen, crate::HostEditEvent::ToggleRdAudio, cx);
                    })),
                )
                .child(
                    crate::ui::components::toggle_switch(
                        "rd-mic-toggle",
                        enable_microphone,
                        "Microphone Redirection",
                    )
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(
                            draft_gen,
                            crate::HostEditEvent::ToggleRdMicrophone,
                            cx,
                        );
                    })),
                );
        }

        container
    }

    fn render_serial_options(
        &self,
        draft_gen: u64,
        data_bits: u8,
        parity: shellumi_core::SerialParity,
        stop_bits: shellumi_core::SerialStopBits,
        flow_control: shellumi_core::SerialFlowControl,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut bits_row = div().flex().gap_1p5();
        for bits in [5u8, 6, 7, 8] {
            let is_sel = data_bits == bits;
            bits_row = bits_row.child(
                div()
                    .id(("serial-bits", bits as usize))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .bg(if is_sel {
                        accent_blue()
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(
                            draft_gen,
                            crate::HostEditEvent::SetSerialDataBits(bits),
                            cx,
                        );
                    }))
                    .child(format!("{bits}")),
            );
        }

        let mut parity_row = div().flex().gap_1p5();
        for (idx, (p, label)) in [
            (shellumi_core::SerialParity::None, "None"),
            (shellumi_core::SerialParity::Even, "Even"),
            (shellumi_core::SerialParity::Odd, "Odd"),
        ]
        .into_iter()
        .enumerate()
        {
            let is_sel = parity == p;
            parity_row = parity_row.child(
                div()
                    .id(("serial-parity", idx))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .bg(if is_sel {
                        accent_blue()
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(
                            draft_gen,
                            crate::HostEditEvent::SetSerialParity(p),
                            cx,
                        );
                    }))
                    .child(label),
            );
        }

        let mut stop_row = div().flex().gap_1p5();
        for (idx, (s, label)) in [
            (shellumi_core::SerialStopBits::One, "1"),
            (shellumi_core::SerialStopBits::Two, "2"),
        ]
        .into_iter()
        .enumerate()
        {
            let is_sel = stop_bits == s;
            stop_row = stop_row.child(
                div()
                    .id(("serial-stop", idx))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .bg(if is_sel {
                        accent_blue()
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(
                            draft_gen,
                            crate::HostEditEvent::SetSerialStopBits(s),
                            cx,
                        );
                    }))
                    .child(label),
            );
        }

        let mut flow_row = div().flex().gap_1p5();
        for (idx, (f, label)) in [
            (shellumi_core::SerialFlowControl::None, "None"),
            (shellumi_core::SerialFlowControl::Software, "XON/XOFF"),
            (shellumi_core::SerialFlowControl::Hardware, "RTS/CTS"),
        ]
        .into_iter()
        .enumerate()
        {
            let is_sel = flow_control == f;
            flow_row = flow_row.child(
                div()
                    .id(("serial-flow", idx))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .bg(if is_sel {
                        accent_blue()
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_sel {
                        rgb(0xffffff)
                    } else {
                        text_secondary()
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.apply_host_edit(
                            draft_gen,
                            crate::HostEditEvent::SetSerialFlowControl(f),
                            cx,
                        );
                    }))
                    .child(label),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(text_secondary())
                    .child("Data Bits"),
            )
            .child(bits_row)
            .child(div().text_xs().text_color(text_secondary()).child("Parity"))
            .child(parity_row)
            .child(
                div()
                    .text_xs()
                    .text_color(text_secondary())
                    .child("Stop Bits"),
            )
            .child(stop_row)
            .child(
                div()
                    .text_xs()
                    .text_color(text_secondary())
                    .child("Flow Control"),
            )
            .child(flow_row)
    }

    /// Helper to render a labeled detail row.
    pub(crate) fn render_detail_row(&self, label: &str, value: String) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py_1()
            .child(
                div()
                    .text_xs()
                    .text_color(text_secondary())
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_primary())
                    .child(value),
            )
    }

    /// Duplicate a host with "(Copy)" suffix.
    pub(crate) fn duplicate_host(&mut self, source: &SavedHost, cx: &mut Context<Self>) {
        let mut copy = source.clone();
        copy.id = 0;
        copy.name = format!("{} (Copy)", source.name);
        self.edit_host(Some(&copy), cx);
        if let Some(form) = &mut self.form {
            form.id = None;
            form.name.update(cx, |input, cx| {
                input.set_value(&format!("{} (Copy)", source.name), cx);
            });
        }
        let vault_id = self.active_vault_id.clone();
        self.inspector_coordinator.open(
            vault_id,
            InspectorTarget::Host(None),
            InspectorMode::Create,
            None,
        );
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::inspector::coordinator::InspectorCoordinator;

    #[test]
    fn test_ins_003_open_group_then_host_transitions_inspector() {
        let mut coord = InspectorCoordinator::new();

        // 1. Open group inspector
        assert!(coord.open(
            "vault-1",
            InspectorTarget::Group(Some("web-servers".into())),
            InspectorMode::View,
            None
        ));
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Group(Some("web-servers".into()))
        );
        assert!(coord.is_open());

        // 2. Open host inspector -> host replaces group; only 1 inspector remains!
        assert!(coord.open(
            "vault-1",
            InspectorTarget::Host(Some(42)),
            InspectorMode::View,
            None
        ));
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Host(Some(42))
        );
        assert!(coord.is_open());
    }

    #[test]
    fn test_ins_004_host_crud_and_modes() {
        let mut coord = InspectorCoordinator::new();

        // View mode
        assert!(coord.open(
            "v1",
            InspectorTarget::Host(Some(10)),
            InspectorMode::View,
            None
        ));
        assert_eq!(coord.active().unwrap().mode, InspectorMode::View);

        // Edit mode
        coord.set_mode(InspectorMode::Edit);
        assert_eq!(coord.active().unwrap().mode, InspectorMode::Edit);

        // Create mode
        assert!(coord.open(
            "v1",
            InspectorTarget::Host(None),
            InspectorMode::Create,
            None
        ));
        assert_eq!(coord.active().unwrap().target, InspectorTarget::Host(None));
        assert_eq!(coord.active().unwrap().mode, InspectorMode::Create);
    }

    #[test]
    fn test_ins_005_group_crud_and_modes() {
        let mut coord = InspectorCoordinator::new();

        // View mode
        assert!(coord.open(
            "v1",
            InspectorTarget::Group(Some("database".into())),
            InspectorMode::View,
            None
        ));
        assert_eq!(coord.active().unwrap().mode, InspectorMode::View);

        // Edit mode
        coord.set_mode(InspectorMode::Edit);
        assert_eq!(coord.active().unwrap().mode, InspectorMode::Edit);

        // Create mode
        assert!(coord.open(
            "v1",
            InspectorTarget::Group(None),
            InspectorMode::Create,
            None
        ));
        assert_eq!(coord.active().unwrap().target, InspectorTarget::Group(None));
        assert_eq!(coord.active().unwrap().mode, InspectorMode::Create);
    }

    #[test]
    fn test_ui_01_at_most_one_inspector_active() {
        let mut coord = InspectorCoordinator::new();
        assert!(!coord.is_open());

        coord.open(
            "v1",
            InspectorTarget::Host(Some(1)),
            InspectorMode::View,
            None,
        );
        assert!(coord.is_open());

        coord.open(
            "v1",
            InspectorTarget::Group(Some("backend".into())),
            InspectorMode::View,
            None,
        );
        assert!(coord.is_open());
        assert_eq!(
            coord.active().unwrap().target,
            InspectorTarget::Group(Some("backend".into()))
        );

        // Closing clears inspector completely
        coord.close();
        assert!(!coord.is_open());
        assert!(coord.active().is_none());
    }

    #[test]
    fn test_ui_02_layout_calculates_single_inspector_deduction() {
        // Standard laptop 1280x800 with 184px sidebar
        let layout_closed = layout::calculate_layout(1280.0, 184.0, false, None);
        assert_eq!(layout_closed.inspector_width, 0.0);
        assert_eq!(layout_closed.primary_content_width, 1096.0);

        // Standard laptop with inspector open: deducts 400px inspector once!
        let layout_open = layout::calculate_layout(1280.0, 184.0, true, None);
        assert_eq!(layout_open.inspector_width, 400.0);
        assert_eq!(layout_open.primary_content_width, 696.0);
        assert!(layout_open.primary_content_width >= layout::MIN_PRIMARY_CONTENT_WIDTH);
    }
}

#![allow(dead_code)]
//! Tunnel / Port Forwarding inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{error_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout;
use crate::{SavedTunnel, Shellumi, TextInput, TunnelFormState};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};
use shellumi_core::{ForwardingType, PortForwardRule};
use shellumi_storage::HostStore;

impl Shellumi {
    /// Render the unified Tunnel Inspector for viewing, running, or editing a port forward rule.
    pub(crate) fn render_tunnel_inspector(
        &mut self,
        tunnel_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_tunnel_view_inspector(tunnel_id, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_tunnel_edit_inspector(tunnel_id, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render tunnel inspector in View mode.
    fn render_tunnel_view_inspector(
        &mut self,
        tunnel_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let tunnel = tunnel_id.and_then(|id| self.tunnels.iter().find(|t| t.id == id));

        let Some(tunnel) = tunnel else {
            return InspectorShell::new(
                div().p_4().child("Port Forward not found"),
                div()
                    .p_4()
                    .child("The selected port forward rule does not exist or was deleted."),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let id_val = tunnel.id;
        let t_name = tunnel.name.clone();
        let ftype = tunnel.rule.forwarding_type;
        let type_display = match ftype {
            ForwardingType::Local => "Local (-L)",
            ForwardingType::Remote => "Remote (-R)",
            ForwardingType::Dynamic => "Dynamic SOCKS (-D)",
        };

        let bind_display = format!(
            "{}:{}",
            tunnel.rule.bind_address.as_deref().unwrap_or("127.0.0.1"),
            tunnel.rule.bind_port
        );
        let target_display = if ftype == ForwardingType::Dynamic {
            "SOCKS5 Proxy (dynamic destination)".into()
        } else {
            format!(
                "{}:{}",
                tunnel.rule.target_host.as_deref().unwrap_or("—"),
                tunnel
                    .rule
                    .target_port
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "—".into())
            )
        };

        let host_name = if let Some(hid) = tunnel.host_id {
            match &self.hosts {
                crate::HostState::Loaded(hosts) => hosts
                    .iter()
                    .find(|h| h.id == hid)
                    .map(|h| h.name.clone())
                    .unwrap_or_else(|| format!("Host #{}", hid)),
                _ => format!("Host #{}", hid),
            }
        } else {
            "— (None)".into()
        };

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x351f42))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0xbb86fc))
            .child("Tunnel");

        let edit_btn = div()
            .id("inspector-edit-tunnel-btn")
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
                app.open_tunnel_inspector_mode(Some(id_val), InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let close_btn = InspectorShell::close_button(
            "close-tunnel-inspector-btn",
            false,
            |app, _, _, cx| {
                app.inspector_close(cx);
            },
            cx,
        );

        let header = InspectorShell::header(
            t_name.clone(),
            Some(type_display),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            Some(close_btn),
        );

        let mut body = div().flex().flex_col().gap_4();

        let is_running = self
            .tabs
            .iter()
            .any(|t| t.name == format!("Tunnel: {}", t_name));

        // Action buttons
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "tunnel-view-toggle-btn",
                    if is_running {
                        "Stop Tunnel"
                    } else {
                        "Start Tunnel"
                    },
                    if is_running {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Primary
                    },
                )
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.toggle_saved_tunnel(id_val, window, cx);
                })),
            )
            .child(
                AppButton::new(
                    "tunnel-view-delete-btn",
                    "Delete Port Forward",
                    ButtonVariant::Danger,
                )
                .on_click({
                    let name_for_del = t_name.clone();
                    cx.listener(move |app, _, _, cx| {
                        app.delete_confirm =
                            Some(crate::DeleteTarget::Tunnel(id_val, name_for_del.clone()));
                        cx.notify();
                    })
                }),
            );
        body = body.child(actions_row);

        // Configuration details
        let mut config_items = div().flex().flex_col().gap_1();
        config_items = config_items
            .child(self.render_detail_row("Type", type_display.into()))
            .child(self.render_detail_row("Local Bind", bind_display))
            .child(self.render_detail_row("Destination", target_display))
            .child(self.render_detail_row("SSH Gateway Host", host_name))
            .child(
                self.render_detail_row(
                    "Auto Start",
                    if tunnel.auto_start {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                    .into(),
                ),
            );
        body = body.child(simple_section("Port Forward Configuration", config_items));

        InspectorShell::new(header, body, None::<Div>, self.active_inspector_width())
    }

    /// Render tunnel inspector in Edit or Create mode.
    fn render_tunnel_edit_inspector(
        &mut self,
        _tunnel_id: Option<i64>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(form) = &self.tunnel_form else {
            return InspectorShell::new(
                div().p_4().child("Form not initialized"),
                div().p_4().child("Tunnel draft state is missing."),
                None::<Div>,
                self.active_inspector_width(),
            );
        };

        let title = if is_create {
            "New Port Forward"
        } else {
            "Edit Port Forward"
        };

        let back_btn = InspectorShell::close_button(
            "close-tunnel-edit-inspector-btn",
            true,
            |app, _, _, cx| {
                app.inspector_back(cx);
            },
            cx,
        );

        let header = InspectorShell::header(
            title,
            Some("Configure SSH local, remote, or dynamic socks forwarding"),
            None,
            None,
            Some(back_btn),
        );

        let current_ftype = form.forwarding_type;
        let mut type_selector = div().flex().gap_1p5().py_1();
        for (ft, label) in [
            (ForwardingType::Local, "Local (-L)"),
            (ForwardingType::Remote, "Remote (-R)"),
            (ForwardingType::Dynamic, "Dynamic (-D)"),
        ] {
            let is_sel = ft == current_ftype;
            type_selector = type_selector.child(
                div()
                    .id(("tunnel-ft", ft as usize))
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
                        if let Some(ref mut f) = app.tunnel_form {
                            f.forwarding_type = ft;
                            app.inspector_mark_dirty(cx);
                        }
                    }))
                    .child(label),
            );
        }

        let is_auto = form.auto_start;
        let auto_toggle = div()
            .id("tunnel-auto-start-toggle")
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(if is_auto { rgb(0x103d3d) } else { surface() })
            .text_color(if is_auto {
                rgb(0x3de0d8)
            } else {
                text_secondary()
            })
            .cursor_pointer()
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(|app, _, _, cx| {
                if let Some(ref mut f) = app.tunnel_form {
                    f.auto_start = !f.auto_start;
                    app.inspector_mark_dirty(cx);
                }
            }))
            .child(if is_auto {
                "⚡ Auto-Start on Host Connect: Enabled"
            } else {
                "○ Auto-Start on Host Connect: Disabled"
            });

        let mut form_body = div().flex().flex_col().gap_3();

        form_body = form_body
            .child(simple_field("Tunnel Name", form.name.clone()))
            .child(simple_field("Forwarding Type", type_selector))
            .child(simple_field(
                "Bind Port (e.g. 8080)",
                form.bind_port.clone(),
            ));

        if current_ftype != ForwardingType::Dynamic {
            form_body = form_body
                .child(simple_field(
                    "Target Host (e.g. 127.0.0.1 or db.internal)",
                    form.target_host.clone(),
                ))
                .child(simple_field(
                    "Target Port (e.g. 5432)",
                    form.target_port.clone(),
                ));
        }

        form_body = form_body
            .child(self.render_tunnel_host_picker(form.host_id, cx))
            .child(simple_field("Automation", auto_toggle));

        let mut footer = div()
            .p_4()
            .border_t_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(surface())
            .flex()
            .flex_col()
            .gap_2();

        if let Some(ref err) = form.error {
            footer = footer.child(error_notice(err.clone()));
        }

        let is_saving = form.saving;
        let buttons_row = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(
                AppButton::new(
                    "tunnel-inspector-cancel-btn",
                    "Cancel",
                    ButtonVariant::Ghost,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    if !app.tunnel_form.as_ref().is_some_and(|f| f.saving) {
                        app.inspector_back(cx);
                    }
                })),
            )
            .child(if is_saving {
                AppButton::new(
                    "tunnel-inspector-save-btn",
                    "Saving…",
                    ButtonVariant::Secondary,
                )
            } else {
                AppButton::new("tunnel-inspector-save-btn", "Save", ButtonVariant::Primary)
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.save_tunnel_inspector(cx);
                    }))
            });

        footer = footer.child(buttons_row);

        InspectorShell::new(
            header,
            form_body,
            Some(footer),
            self.active_inspector_width(),
        )
    }

    /// Render SSH host picker for tunnel forwarding.
    fn render_tunnel_host_picker(&self, selected_id: Option<i64>, cx: &mut Context<Self>) -> Div {
        let hosts = match &self.hosts {
            crate::HostState::Loaded(hosts) => hosts.as_slice(),
            _ => &[],
        };

        let mut list = div()
            .id("tunnel-inspector-host-picker")
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

        for host in hosts.iter().filter(|h| h.is_ssh()) {
            let hid = host.id;
            let hname = host.name.clone();
            let is_sel = selected_id == Some(hid);

            list = list.child(
                div()
                    .id(("tunnel-host-pick", hid as usize))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_xs()
                    .bg(if is_sel {
                        rgb(MacTheme::SURFACE_ACTIVE)
                    } else {
                        surface()
                    })
                    .text_color(if is_sel {
                        accent_blue()
                    } else {
                        text_primary()
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if let Some(ref mut f) = app.tunnel_form {
                            f.host_id = Some(hid);
                            app.inspector_mark_dirty(cx);
                        }
                    }))
                    .child(hname),
            );
        }

        simple_field("SSH Gateway Host", list)
    }

    /// Open tunnel inspector in a specific mode.
    pub(crate) fn open_tunnel_inspector_mode(
        &mut self,
        tunnel_id: Option<i64>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Tunnel(tunnel_id);
        if self
            .inspector_coordinator
            .open(vault_id, target, mode, None)
            && (mode == InspectorMode::Edit || mode == InspectorMode::Create)
        {
            let existing = tunnel_id.and_then(|id| self.tunnels.iter().find(|t| t.id == id));
            let name_val = existing.map(|t| t.name.as_str()).unwrap_or("");
            let ftype_val = existing
                .map(|t| t.rule.forwarding_type)
                .unwrap_or(ForwardingType::Local);
            let bind_val = existing
                .map(|t| t.rule.bind_port.to_string())
                .unwrap_or_else(|| "8080".into());
            let target_host_val = existing
                .and_then(|t| t.rule.target_host.clone())
                .unwrap_or_else(|| "127.0.0.1".into());
            let target_port_val = existing
                .and_then(|t| t.rule.target_port)
                .map(|p| p.to_string())
                .unwrap_or_else(|| "80".into());
            let host_id_val = existing
                .and_then(|t| t.host_id)
                .or_else(|| match &self.hosts {
                    crate::HostState::Loaded(hosts) => {
                        hosts.iter().find(|h| h.is_ssh()).map(|h| h.id)
                    }
                    _ => None,
                });
            let auto_val = existing.map(|t| t.auto_start).unwrap_or(false);

            let name = cx.new(|cx| TextInput::new(cx, name_val, "Tunnel name"));
            let bind_port = cx.new(|cx| TextInput::new(cx, &bind_val, "Bind port"));
            let target_host = cx.new(|cx| TextInput::new(cx, &target_host_val, "Target host"));
            let target_port = cx.new(|cx| TextInput::new(cx, &target_port_val, "Target port"));

            cx.observe(&name, |app, _, cx| app.inspector_mark_dirty(cx))
                .detach();
            cx.observe(&bind_port, |app, _, cx| app.inspector_mark_dirty(cx))
                .detach();
            cx.observe(&target_host, |app, _, cx| app.inspector_mark_dirty(cx))
                .detach();
            cx.observe(&target_port, |app, _, cx| app.inspector_mark_dirty(cx))
                .detach();

            self.tunnel_form = Some(TunnelFormState {
                id: tunnel_id,
                name,
                forwarding_type: ftype_val,
                bind_port,
                target_host,
                target_port,
                host_id: host_id_val,
                auto_start: auto_val,
                error: None,
                saving: false,
            });
        }
        cx.notify();
    }

    /// Save tunnel asynchronously.
    pub(crate) fn save_tunnel_inspector(&mut self, cx: &mut Context<Self>) {
        let Some(gen) = self.inspector_coordinator.start_save() else {
            return;
        };

        let Some(form) = &mut self.tunnel_form else {
            return;
        };
        if form.saving {
            return;
        }

        let active_vid = self.active_vault_id.clone();
        let name = form.name.read(cx).value().trim().to_string();
        let bind_str = form.bind_port.read(cx).value().trim().to_string();
        let target_host_str = form.target_host.read(cx).value().trim().to_string();
        let target_port_str = form.target_port.read(cx).value().trim().to_string();
        let ftype = form.forwarding_type;
        let host_id = form.host_id;
        let auto_start = form.auto_start;
        let id = form.id;

        if name.is_empty() {
            form.error = Some("Tunnel name is required.".into());
            self.inspector_coordinator.fail_save(
                &active_vid,
                gen,
                "Tunnel name is required.".into(),
            );
            cx.notify();
            return;
        }

        let bind_port: u16 = match bind_str.parse() {
            Ok(p) if p > 0 => p,
            _ => {
                form.error = Some("Invalid bind port (must be between 1 and 65535).".into());
                self.inspector_coordinator.fail_save(
                    &active_vid,
                    gen,
                    "Invalid bind port (must be between 1 and 65535).".into(),
                );
                cx.notify();
                return;
            }
        };

        let rule_res = match ftype {
            ForwardingType::Dynamic => PortForwardRule::dynamic(Some("127.0.0.1"), bind_port),
            ForwardingType::Local => {
                let target_port: u16 = match target_port_str.parse() {
                    Ok(p) if p > 0 => p,
                    _ => {
                        form.error = Some("Target port required for local forwarding.".into());
                        self.inspector_coordinator.fail_save(
                            &active_vid,
                            gen,
                            "Target port required for local forwarding.".into(),
                        );
                        cx.notify();
                        return;
                    }
                };
                PortForwardRule::local(Some("127.0.0.1"), bind_port, &target_host_str, target_port)
            }
            ForwardingType::Remote => {
                let target_port: u16 = match target_port_str.parse() {
                    Ok(p) if p > 0 => p,
                    _ => {
                        form.error = Some("Target port required for remote forwarding.".into());
                        self.inspector_coordinator.fail_save(
                            &active_vid,
                            gen,
                            "Target port required for remote forwarding.".into(),
                        );
                        cx.notify();
                        return;
                    }
                };
                PortForwardRule::remote(Some("127.0.0.1"), bind_port, &target_host_str, target_port)
            }
        };

        let rule = match rule_res {
            Ok(r) => r,
            Err(e) => {
                if let Some(ref mut f) = self.tunnel_form {
                    f.error = Some(e.to_string());
                    self.inspector_coordinator
                        .fail_save(&active_vid, gen, e.to_string());
                    cx.notify();
                }
                return;
            }
        };

        form.saving = true;
        form.error = None;
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
        let vault_key = self.active_vault_key();
        let name_for_msg = name.clone();
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id_task) {
                    store.set_vault_id(vid);
                }
                let saved_id = match id {
                    Some(existing_id) => {
                        store
                            .update_tunnel(existing_id, &name, host_id, &rule, auto_start)
                            .await
                            .map_err(|e| e.to_string())?;
                        existing_id
                    }
                    None => store
                        .add_tunnel(&name, host_id, &rule, auto_start)
                        .await
                        .map_err(|e| e.to_string())?,
                };
                Ok::<i64, String>(saved_id)
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                if let Some(ref mut f) = app.tunnel_form {
                    f.saving = false;
                }
                match res {
                    Ok(saved_id) => {
                        let target = InspectorTarget::Tunnel(Some(saved_id));
                        let ok =
                            app.inspector_coordinator
                                .complete_save(&active_vault_id, gen, target);
                        if !ok {
                            tracing::warn!(
                                originating_vault = %active_vault_id,
                                current_vault = %app.active_vault_id,
                                gen = gen,
                                "Discarded stale save completion for tunnel"
                            );
                            return;
                        }

                        app.tunnel_form = None;
                        app.load_hosts(cx);
                        app.desktop.message = Some(format!(
                            "Port forward '{}' saved successfully",
                            name_for_msg
                        ));

                        if let Some(req) = app.inspector_coordinator.take_pending_guard() {
                            app.apply_navigation_request(req, None, cx);
                        } else {
                            app.open_tunnel_inspector_mode(
                                Some(saved_id),
                                crate::ui::inspector::InspectorMode::View,
                                cx,
                            );
                        }
                    }
                    Err(err) => {
                        if let Some(ref mut f) = app.tunnel_form {
                            f.error = Some(err.clone());
                        }
                        app.inspector_coordinator
                            .fail_save(&active_vault_id, gen, err);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete tunnel asynchronously.
    pub(crate) fn delete_tunnel_inspector(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(db_path) = self.active_vault_db_path() else {
            return;
        };
        let active_vault_id = self.active_vault_id.clone();
        let active_vault_id_task = active_vault_id.clone();
        let vault_key = self.active_vault_key();
        let task = cx.background_executor().spawn(async move {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .map_err(|e| e.to_string())?;
            runtime.block_on(async {
                let mut store = HostStore::open_with_key(&db_path, vault_key.as_ref())
                    .await
                    .map_err(|e| e.to_string())?;
                if let Ok(vid) = shellumi_core::VaultId::new(&active_vault_id_task) {
                    store.set_vault_id(vid);
                }
                let deleted = store.delete_tunnel(id).await.map_err(|e| e.to_string())?;
                if !deleted {
                    return Err(format!("Port forward with ID {id} not found"));
                }
                Ok::<(), String>(())
            })
        });

        cx.spawn(async move |app, cx| {
            let res = task.await;
            let _ = app.update(cx, |app, cx| {
                match res {
                    Ok(()) => {
                        if app.active_vault_id == active_vault_id {
                            app.load_hosts(cx);
                            app.inspector_close(cx);
                            app.desktop.message = Some("Port forward deleted".into());
                        }
                    }
                    Err(e) => {
                        app.desktop.message = Some(format!("Failed to delete port forward: {e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

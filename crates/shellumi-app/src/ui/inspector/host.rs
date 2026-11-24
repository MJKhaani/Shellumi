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
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
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
                            app.open_group_inspector(g_name, cx);
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

        // Authentication & Security
        let id_name = if let Some(identity_id) = host.identity_id {
            self.identities
                .iter()
                .find(|i| i.id == identity_id)
                .map(|i| i.name.clone())
                .unwrap_or_else(|| format!("Identity #{}", identity_id))
        } else {
            "Password / Default Agent".into()
        };
        let auth_items = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(self.render_detail_row("Identity Key", id_name));
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

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
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
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let active_cat = form.active_category;
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

        // Category tab selector
        let editor_categories = [
            (HostEditorCategory::General, "General"),
            (HostEditorCategory::Connection, "Connection"),
            (HostEditorCategory::Authentication, "Auth"),
            (HostEditorCategory::Proxy, "Proxy"),
            (HostEditorCategory::Advanced, "Advanced"),
        ];

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

        match active_cat {
            HostEditorCategory::General => {
                form_body = form_body
                    .child(simple_field("Host Name", form.name.clone()))
                    .child(simple_field("Group (optional)", form.group.clone()))
                    .child(simple_field("Tags (comma separated)", form.tags.clone()));
            }
            HostEditorCategory::Connection => {
                let proto = form.protocol;
                form_body = form_body
                    .child(self.render_protocol_selector(proto, cx))
                    .child(simple_field("Hostname or IP", form.hostname.clone()))
                    .child(simple_field("Port", form.port.clone()));
            }
            HostEditorCategory::Authentication => {
                form_body = form_body
                    .child(simple_field("Username", form.username.clone()))
                    .child(self.render_identity_picker(form.identity_id, cx));
            }
            HostEditorCategory::Proxy => {
                form_body =
                    form_body.child(simple_field("Jump Host / Bastion", form.jump_host.clone()));
            }
            HostEditorCategory::Advanced => {
                form_body = form_body.child(simple_field(
                    "Startup Snippet",
                    form.startup_snippet.clone(),
                ));

                if form.protocol == ConnectionProtocol::Rdp {
                    form_body = form_body
                        .child(simple_field("Domain", form.rd_domain.clone()))
                        .child(simple_field("Width", form.rd_width.clone()))
                        .child(simple_field("Height", form.rd_height.clone()));
                } else if form.protocol == ConnectionProtocol::Vnc {
                    form_body = form_body
                        .child(simple_field("Width", form.rd_width.clone()))
                        .child(simple_field("Height", form.rd_height.clone()));
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
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Helper to render protocol selector pills.
    fn render_protocol_selector(&self, current: ConnectionProtocol, cx: &mut Context<Self>) -> Div {
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
                        app.set_form_protocol(proto, cx);
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

    /// Helper to render identity picker dropdown/list.
    fn render_identity_picker(&self, selected_id: Option<i64>, cx: &mut Context<Self>) -> Div {
        let none_sel = selected_id.is_none();
        let mut list = div()
            .id("host-inspector-identity-picker")
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(120.0))
            .overflow_y_scroll()
            .bg(rgb(MacTheme::INPUT_BG))
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .rounded_md()
            .p_1();

        list = list.child(
            div()
                .id("host-id-opt-none")
                .px_2()
                .py_1()
                .rounded_sm()
                .text_xs()
                .bg(if none_sel {
                    rgb(MacTheme::SURFACE_ACTIVE)
                } else {
                    rgb(MacTheme::INPUT_BG)
                })
                .text_color(if none_sel {
                    accent_blue()
                } else {
                    text_secondary()
                })
                .cursor_pointer()
                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(form) = &mut app.form {
                        form.identity_id = None;
                        cx.notify();
                    }
                }))
                .child(if none_sel {
                    "✓ None (Password or default agent)"
                } else {
                    "None (Password or default agent)"
                }),
        );

        for (idx, id) in self.identities.iter().enumerate() {
            let is_sel = selected_id == Some(id.id);
            let id_val = id.id;
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
                        if let Some(form) = &mut app.form {
                            form.identity_id = Some(id_val);
                            cx.notify();
                        }
                    }))
                    .child(format!("{} {}", if is_sel { "✓" } else { " " }, id.name)),
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
                    .child("Key / Identity"),
            )
            .child(list)
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

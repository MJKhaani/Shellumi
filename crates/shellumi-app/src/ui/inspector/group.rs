#![allow(dead_code)]
//! Group inspector rendering and actions for View, Edit, and Create modes.

use crate::theme::{
    accent_blue, caption, danger, subheader, surface, text_primary, text_secondary, MacTheme,
};
use crate::ui::components::button::{AppButton, ButtonVariant};
use crate::ui::components::form::{inline_notice, simple_field, simple_section};
use crate::ui::components::inspector_shell::InspectorShell;
use crate::ui::inspector::coordinator::{InspectorMode, InspectorTarget};
use crate::ui::layout;
use crate::{SavedHost, Shellumi};
use gpui::{
    div, prelude::*, px, rgb, Context, Div, FontWeight, IntoElement, ParentElement, Stateful,
    Styled, Window,
};

impl Shellumi {
    /// Render the unified Group Inspector for viewing or editing a group.
    pub(crate) fn render_group_inspector(
        &mut self,
        group_name: Option<String>,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) -> Div {
        match mode {
            InspectorMode::View => self.render_group_view_inspector(group_name, cx),
            InspectorMode::Edit | InspectorMode::Create => {
                self.render_group_edit_inspector(group_name, mode == InspectorMode::Create, cx)
            }
        }
    }

    /// Render group inspector in View mode.
    fn render_group_view_inspector(
        &mut self,
        group_name: Option<String>,
        cx: &mut Context<Self>,
    ) -> Div {
        let name_str = group_name.unwrap_or_default();
        let existing = self.desktop.groups.iter().find(|g| g.name == name_str);

        let default_user = existing
            .and_then(|g| g.default_username.clone())
            .unwrap_or_else(|| "—".into());
        let default_port = existing
            .and_then(|g| g.default_port)
            .map(|p| p.get().to_string())
            .unwrap_or_else(|| "22 (Default)".into());
        let identity_id = existing.and_then(|g| g.identity_id);

        let identity_name = if let Some(id) = identity_id {
            self.identities
                .iter()
                .find(|i| i.id == id)
                .map(|i| i.name.clone())
                .unwrap_or_else(|| format!("Identity #{}", id))
        } else {
            "— (None)".into()
        };

        let hosts = match &self.hosts {
            crate::HostState::Loaded(hosts) => hosts.as_slice(),
            _ => &[],
        };

        // Find member hosts belonging to this group
        let member_hosts: Vec<SavedHost> = hosts
            .iter()
            .filter(|h| h.group_name.as_deref() == Some(&name_str))
            .cloned()
            .collect();

        let member_count = member_hosts.len();
        let name_for_edit = name_str.clone();
        let name_for_delete = name_str.clone();

        let badge = div()
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(rgb(0x2d2518))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(0xf5a623))
            .child("Group");

        let edit_btn = div()
            .id("inspector-edit-group-btn")
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
                app.open_group_inspector_mode(&name_for_edit, InspectorMode::Edit, cx);
            }))
            .child("Edit");

        let close_btn = InspectorShell::close_button(
            "close-group-inspector-btn",
            false,
            |app, _, _, cx| {
                app.inspector_close(cx);
            },
            cx,
        );

        let header = InspectorShell::header(
            name_str.clone(),
            Some(format!(
                "{} member host{}",
                member_count,
                if member_count == 1 { "" } else { "s" }
            )),
            Some(badge.into_any_element()),
            Some(edit_btn.into_any_element()),
            Some(close_btn),
        );

        let mut body = div().flex().flex_col().gap_4();

        // Action buttons
        let actions_row = div()
            .flex()
            .gap_2()
            .child(
                AppButton::new(
                    "group-view-edit-btn",
                    "Edit Group",
                    ButtonVariant::Secondary,
                )
                .on_click(cx.listener({
                    let name_for_edit = name_str.clone();
                    move |app, _, _, cx| {
                        app.open_group_inspector_mode(&name_for_edit, InspectorMode::Edit, cx);
                    }
                })),
            )
            .child(
                AppButton::new(
                    "group-view-delete-btn",
                    "Remove Group",
                    ButtonVariant::Danger,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.delete_confirm = Some(crate::DeleteTarget::Group(name_for_delete.clone()));
                    cx.notify();
                })),
            );
        body = body.child(actions_row);

        // Group inheritance and defaults section
        let mut defaults_items = div().flex().flex_col().gap_1();
        defaults_items = defaults_items
            .child(self.render_detail_row("Group Name", name_str.clone()))
            .child(self.render_detail_row("Default Username", default_user))
            .child(self.render_detail_row("Default Port", default_port))
            .child(self.render_detail_row("Default Identity", identity_name));
        body = body.child(simple_section(
            "Default Connection Settings",
            defaults_items,
        ));

        // Member hosts section
        let mut member_items = div().flex().flex_col().gap_1();

        if member_hosts.is_empty() {
            member_items = member_items.child(
                div()
                    .text_xs()
                    .text_color(text_secondary())
                    .p_2()
                    .child("No hosts assigned to this group yet."),
            );
        } else {
            let mut host_list = div()
                .id("group-member-hosts-list")
                .flex()
                .flex_col()
                .gap_1()
                .max_h(px(240.0))
                .overflow_y_scroll();

            for (idx, host) in member_hosts.into_iter().enumerate() {
                let h_id = host.id;
                let h_name = host.name.clone();
                let proto_icon = match host.protocol() {
                    crate::ConnectionProtocol::Ssh => "🖧",
                    crate::ConnectionProtocol::Rdp => "🪟",
                    crate::ConnectionProtocol::Vnc => "🖥️",
                    _ => ">_",
                };

                host_list = host_list.child(
                    div()
                        .id(("group-member-host", idx))
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_2p5()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(MacTheme::INPUT_BG))
                        .border_1()
                        .border_color(rgb(MacTheme::BORDER))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                        .on_click(cx.listener(move |app, _, _, cx| {
                            // INS-001: Selecting host from group replaces group inspector!
                            app.inspector_select_host(h_id, cx);
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().text_sm().child(proto_icon))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(text_primary())
                                        .child(h_name),
                                ),
                        )
                        .child(div().text_xs().text_color(accent_blue()).child("Inspect →")),
                );
            }
            member_items = member_items.child(host_list);
        }

        body = body.child(simple_section(
            format!("Member Hosts ({})", member_count),
            member_items,
        ));

        InspectorShell::new(
            header,
            body,
            None::<Div>,
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render group inspector in Edit or Create mode.
    fn render_group_edit_inspector(
        &mut self,
        _group_name: Option<String>,
        is_create: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(ins) = &self.group_inspector else {
            return InspectorShell::new(
                div().p_4().child("Group inspector not initialized"),
                div().p_4().child("Group draft state is missing."),
                None::<Div>,
                Some(layout::INSPECTOR_PREFERRED_WIDTH),
            );
        };

        let title = if is_create { "New Group" } else { "Edit Group" };

        let back_btn = InspectorShell::close_button(
            "close-group-edit-inspector-btn",
            true,
            |app, _, _, cx| {
                app.inspector_back(cx);
            },
            cx,
        );

        let header = InspectorShell::header(
            title,
            Some("Configure group name and inherited defaults"),
            None,
            None,
            Some(back_btn),
        );

        let is_cloud_sync = ins.cloud_sync;
        let sync_toggle = div()
            .id("group-cloud-sync-toggle")
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(if is_cloud_sync {
                rgb(0x103d3d)
            } else {
                surface()
            })
            .text_color(if is_cloud_sync {
                rgb(0x3de0d8)
            } else {
                text_secondary()
            })
            .cursor_pointer()
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
            .on_click(cx.listener(|app, _, _, cx| {
                if let Some(ins) = &mut app.group_inspector {
                    ins.cloud_sync = !ins.cloud_sync;
                    app.inspector_mark_dirty(cx);
                }
            }))
            .child(if is_cloud_sync {
                "☁ Cloud Sync: Enabled"
            } else {
                "○ Cloud Sync: Disabled"
            });

        let mut form_body = div().flex().flex_col().gap_3();

        form_body = form_body
            .child(simple_field("Group Name", ins.name.clone()))
            .child(simple_field(
                "Parent Group (optional)",
                ins.parent_group.clone(),
            ))
            .child(simple_field(
                "Default Username (optional)",
                ins.default_username.clone(),
            ))
            .child(simple_field(
                "Default Port (optional, 22)",
                ins.default_port.clone(),
            ))
            .child(self.render_group_identity_picker(ins.identity_id, cx))
            .child(simple_field("Synchronization", sync_toggle));

        let mut footer = div()
            .p_4()
            .border_t_1()
            .border_color(rgb(MacTheme::BORDER))
            .bg(surface())
            .flex()
            .flex_col()
            .gap_2();

        if let Some(ref err) = ins.error {
            footer = footer.child(crate::ui::components::error_notice(err.clone()));
        }

        let buttons_row = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(
                AppButton::new("group-inspector-cancel-btn", "Cancel", ButtonVariant::Ghost)
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.inspector_back(cx);
                    })),
            )
            .child(
                AppButton::new("group-inspector-save-btn", "Save", ButtonVariant::Primary)
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.save_group_inspector(cx);
                    })),
            );

        footer = footer.child(buttons_row);

        InspectorShell::new(
            header,
            form_body,
            Some(footer),
            Some(layout::INSPECTOR_PREFERRED_WIDTH),
        )
    }

    /// Render identity picker inside Group Inspector.
    fn render_group_identity_picker(
        &self,
        selected_id: Option<i64>,
        cx: &mut Context<Self>,
    ) -> Div {
        let none_sel = selected_id.is_none();
        let mut list = div()
            .id("group-inspector-identity-picker")
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
                .id("group-id-opt-none")
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
                    if let Some(ins) = &mut app.group_inspector {
                        ins.identity_id = None;
                        cx.notify();
                    }
                }))
                .child(if none_sel {
                    "✓ None (Inherit defaults)"
                } else {
                    "None (Inherit defaults)"
                }),
        );

        for (idx, id) in self.identities.iter().enumerate() {
            let is_sel = selected_id == Some(id.id);
            let id_val = id.id;
            list = list.child(
                div()
                    .id(("group-id-opt", idx))
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
                        if let Some(ins) = &mut app.group_inspector {
                            ins.identity_id = Some(id_val);
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
                    .child("Default Identity"),
            )
            .child(list)
    }

    /// Helper to open a group inspector in a specific mode.
    pub(crate) fn open_group_inspector_mode(
        &mut self,
        group_name: &str,
        mode: InspectorMode,
        cx: &mut Context<Self>,
    ) {
        let vault_id = self.active_vault_id.clone();
        let target = InspectorTarget::Group(Some(group_name.to_string()));
        if self
            .inspector_coordinator
            .open(vault_id, target, mode, None)
        {
            self.selected_host_id = None;
            self.open_group_inspector(group_name, cx);
        }
        cx.notify();
    }
}

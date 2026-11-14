//! Team modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_team_modal(
        &self,
        modal: &TeamModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("team-modal-overlay")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center()
            .p_6();

        let mut modal_box = div()
            .id("team-modal-container")
            .w(px(780.0))
            .max_h(px(680.0))
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .rounded_xl()
            .p_5()
            .gap_4();

        // Header
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        if let Some(p) = resolve_icon_path("invite_member", false, 48) {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().child("👥").into_any_element()
                        },
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Team Collaboration & Shared Vaults"),
                    ),
            )
            .child(
                div()
                    .id("dismiss-team-modal-btn")
                    .when(self.page == Some("Vaults"), |element| element.hidden())
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .bg(rgb(0x323236))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_team_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Navigation Tabs: Vaults, Members, AuditLog
        let mut tab_strip = div().flex().items_center().gap_2();

        let is_vaults = modal.active_tab == TeamModalTab::Vaults;
        tab_strip = tab_strip.child(
            div()
                .id("team-tab-vaults")
                .px_3()
                .py_1p5()
                .rounded_md()
                .border_1()
                .border_color(if is_vaults {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::SEPARATOR)
                })
                .bg(if is_vaults {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(0x323236)
                })
                .text_sm()
                .font_weight(if is_vaults {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_vaults {
                    rgb(0xffffff)
                } else {
                    rgb(MacTheme::TEXT_SECONDARY)
                })
                .cursor_pointer()
                .hover(move |s| {
                    if is_vaults {
                        s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                    } else {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                    }
                })
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(m) = &mut app.team_modal {
                        m.active_tab = TeamModalTab::Vaults;
                        cx.notify();
                    }
                }))
                .child(format!("📁 Shared Vaults ({})", modal.vaults.len())),
        );

        let is_members = modal.active_tab == TeamModalTab::Members;
        tab_strip = tab_strip.child(
            div()
                .id("team-tab-members")
                .px_3()
                .py_1p5()
                .rounded_md()
                .border_1()
                .border_color(if is_members {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::SEPARATOR)
                })
                .bg(if is_members {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(0x323236)
                })
                .text_sm()
                .font_weight(if is_members {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_members {
                    rgb(0xffffff)
                } else {
                    rgb(MacTheme::TEXT_SECONDARY)
                })
                .cursor_pointer()
                .hover(move |s| {
                    if is_members {
                        s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                    } else {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                    }
                })
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(m) = &mut app.team_modal {
                        m.active_tab = TeamModalTab::Members;
                        cx.notify();
                    }
                }))
                .child(format!("👥 Members ({})", modal.members.len())),
        );

        let is_audit = modal.active_tab == TeamModalTab::AuditLog;
        tab_strip = tab_strip.child(
            div()
                .id("team-tab-audit")
                .px_3()
                .py_1p5()
                .rounded_md()
                .border_1()
                .border_color(if is_audit {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::SEPARATOR)
                })
                .bg(if is_audit {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(0x323236)
                })
                .text_sm()
                .font_weight(if is_audit {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_audit {
                    rgb(0xffffff)
                } else {
                    rgb(MacTheme::TEXT_SECONDARY)
                })
                .cursor_pointer()
                .hover(move |s| {
                    if is_audit {
                        s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                    } else {
                        s.bg(rgb(MacTheme::SURFACE_HOVER))
                    }
                })
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(m) = &mut app.team_modal {
                        m.active_tab = TeamModalTab::AuditLog;
                        cx.notify();
                    }
                }))
                .child(format!("📜 Audit Log ({})", modal.audit_records.len())),
        );

        modal_box = modal_box.child(tab_strip);

        // Status or Error notifications
        if let Some(msg) = &modal.status_message {
            modal_box = modal_box.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x133820))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(format!("✓ {msg}")),
            );
        }
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("⚠ {err}")),
            );
        }

        // Tab Content
        match modal.active_tab {
            TeamModalTab::Vaults => {
                let mut list = div()
                    .id("vaults-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_h(px(260.0))
                    .overflow_y_scroll();

                if modal.vaults.is_empty() {
                    list = list.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_sm()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("No shared team vaults found. Create one below to collaborate."),
                    );
                } else {
                    for (v_idx, v) in modal.vaults.iter().enumerate() {
                        let vid = v.id.clone();
                        let is_selected = modal.selected_vault_id.as_deref() == Some(&v.id);
                        let epoch = v.current_epoch;
                        let vid_rotate = v.id.clone();
                        let vid_del = v.id.clone();

                        let mut card = div()
                            .id(("vault-card", v_idx))
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_md()
                            .bg(if is_selected {
                                rgb(MacTheme::INPUT_BG)
                            } else {
                                rgb(MacTheme::SURFACE)
                            })
                            .border_1()
                            .border_color(if is_selected {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            });

                        let left = div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child(v.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x183048))
                                            .text_xs()
                                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                                            .child(format!("Epoch {epoch}")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child(format!("Owner: {}", v.owner_id)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(
                                        v.description
                                            .clone()
                                            .unwrap_or_else(|| "No description".into()),
                                    ),
                            );

                        let mut right = div().flex().items_center().gap_2();
                        if !is_selected {
                            let vid_sel = vid.clone();
                            right = right.child(
                                div()
                                    .id(("select-vault-btn", v_idx))
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        if let Some(m) = &mut app.team_modal {
                                            m.selected_vault_id = Some(vid_sel.clone());
                                        }
                                        app.refresh_team_modal_vault(&vid_sel, cx);
                                    }))
                                    .child("Select"),
                            );
                        } else {
                            right = right.child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::SUCCESS))
                                    .bg(rgb(0x133820))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::SUCCESS))
                                    .child("Active"),
                            );
                        }

                        right = right.child(
                            div()
                                .id(("rotate-key-btn", v_idx))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .bg(rgb(0x323236))
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.rotate_team_vault_key(&vid_rotate, cx);
                                }))
                                .child("🔄 Rotate Key"),
                        );

                        right = right.child(
                            div()
                                .id(("delete-vault-btn", v_idx))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .bg(rgb(0x323236))
                                .text_xs()
                                .text_color(rgb(MacTheme::DANGER))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x4a1818)))
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.delete_team_vault(&vid_del, cx);
                                }))
                                .child("🗑"),
                        );

                        card = card.child(left).child(right);
                        list = list.child(card);
                    }
                }
                modal_box = modal_box.child(list);

                // Create Vault Form
                let create_form = div()
                    .id("create-vault-box")
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("CREATE NEW SHARED VAULT"),
                    )
                    .child(modal.new_vault_name.clone())
                    .child(modal.new_vault_desc.clone())
                    .child(
                        div().flex().justify_end().child(
                            div()
                                .id("create-vault-submit-btn")
                                .px_4()
                                .py_1p5()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(MacTheme::ACCENT_BLUE))
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.create_team_vault(cx);
                                }))
                                .child("+ Create Vault"),
                        ),
                    );
                modal_box = modal_box.child(create_form);
            }
            TeamModalTab::Members => {
                let Some(selected_vid) = &modal.selected_vault_id else {
                    modal_box = modal_box.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(0x332810))
                            .border_1()
                            .border_color(rgb(MacTheme::WARNING))
                            .text_sm()
                            .text_color(rgb(MacTheme::WARNING))
                            .child(
                                "⚠️ Please select a shared vault from the 'Shared Vaults' tab first.",
                            ),
                    );
                    return overlay.child(modal_box).into_any_element();
                };

                let selected_vault_name = modal
                    .vaults
                    .iter()
                    .find(|v| &v.id == selected_vid)
                    .map(|v| v.name.as_str())
                    .unwrap_or("Selected Vault");

                modal_box = modal_box.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child(format!("Enrolled Collaborators for: {selected_vault_name}")),
                );

                let mut members_list = div()
                    .id("team-members-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_h(px(220.0))
                    .overflow_y_scroll();

                if modal.members.is_empty() {
                    members_list = members_list.child(
                        div().p_3().rounded_md().bg(rgb(MacTheme::INPUT_BG)).border_1().border_color(rgb(MacTheme::SEPARATOR)).text_xs().text_color(
                            rgb(MacTheme::TEXT_SECONDARY),
                        ).child(
                            "No additional collaborators enrolled yet. Invite team members below.",
                        ),
                    );
                } else {
                    for (m_idx, m) in modal.members.iter().enumerate() {
                        let mid_revoke = m.id.clone();
                        let mid_role = m.id.clone();
                        let is_revoked = m.revoked;

                        let mut card = div()
                            .id(("member-card", m_idx))
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE))
                            .border_1()
                            .border_color(if is_revoked {
                                rgb(MacTheme::DANGER)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            });

                        let (badge_bg, badge_fg) = match m.role {
                            TeamRole::Owner => (rgb(0x3a2c10), rgb(MacTheme::WARNING)),
                            TeamRole::Administrator => (rgb(0x183048), rgb(MacTheme::ACCENT_BLUE)),
                            TeamRole::Editor => (rgb(0x133820), rgb(MacTheme::SUCCESS)),
                            TeamRole::Viewer => (rgb(0x323236), rgb(MacTheme::TEXT_SECONDARY)),
                        };

                        let left = div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if is_revoked {
                                                rgb(MacTheme::TEXT_SECONDARY)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
                                            })
                                            .child(m.display_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(badge_bg)
                                            .text_xs()
                                            .text_color(badge_fg)
                                            .child(m.role.badge_label()),
                                    )
                                    .child({
                                        if is_revoked {
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0x4a1818))
                                                .text_xs()
                                                .text_color(rgb(MacTheme::DANGER))
                                                .child("⛔ Revoked")
                                        } else {
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0x133820))
                                                .text_xs()
                                                .text_color(rgb(MacTheme::SUCCESS))
                                                .child("Active")
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(format!(
                                        "User: {} • Invited by {}",
                                        m.user_id, m.invited_by
                                    )),
                            );

                        let mut right = div().flex().items_center().gap_2();
                        if !is_revoked && m.role != TeamRole::Owner {
                            let next_role = match m.role {
                                TeamRole::Viewer => TeamRole::Editor,
                                TeamRole::Editor => TeamRole::Administrator,
                                TeamRole::Administrator => TeamRole::Viewer,
                                TeamRole::Owner => TeamRole::Owner,
                            };
                            let role_label = format!("Set {}", next_role.as_str());
                            right = right.child(
                                div()
                                    .id(("toggle-role-btn", m_idx))
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::SEPARATOR))
                                    .bg(rgb(0x323236))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.update_member_role(&mid_role, next_role, cx);
                                    }))
                                    .child(role_label),
                            );

                            right = right.child(
                                div()
                                    .id(("revoke-member-btn", m_idx))
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::SEPARATOR))
                                    .bg(rgb(0x323236))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::DANGER))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0x4a1818)))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.revoke_team_member(&mid_revoke, cx);
                                    }))
                                    .child("⛔ Revoke"),
                            );
                        }

                        card = card.child(left).child(right);
                        members_list = members_list.child(card);
                    }
                }
                modal_box = modal_box.child(members_list);

                // Invite Member Form
                let current_role = modal.invite_role;
                let invite_form = div()
                    .id("invite-member-box")
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("ADD LOCAL MEMBERSHIP RECORD"),
                    )
                    .child(modal.invite_user_id.clone())
                    .child(modal.invite_name.clone())
                    .child(modal.invite_public_key.clone())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child("Role:"),
                                    )
                                    .child(
                                        div()
                                            .id("invite-role-viewer")
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(if current_role == TeamRole::Viewer {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(MacTheme::SEPARATOR)
                                            })
                                            .bg(if current_role == TeamRole::Viewer {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(0x323236)
                                            })
                                            .text_xs()
                                            .text_color(if current_role == TeamRole::Viewer {
                                                rgb(0xffffff)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
                                            })
                                            .cursor_pointer()
                                            .hover(move |s| {
                                                if current_role == TeamRole::Viewer {
                                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                                } else {
                                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                                }
                                            })
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                if let Some(m) = &mut app.team_modal {
                                                    m.invite_role = TeamRole::Viewer;
                                                    cx.notify();
                                                }
                                            }))
                                            .child("Viewer"),
                                    )
                                    .child(
                                        div()
                                            .id("invite-role-editor")
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(if current_role == TeamRole::Editor {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(MacTheme::SEPARATOR)
                                            })
                                            .bg(if current_role == TeamRole::Editor {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(0x323236)
                                            })
                                            .text_xs()
                                            .text_color(if current_role == TeamRole::Editor {
                                                rgb(0xffffff)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
                                            })
                                            .cursor_pointer()
                                            .hover(move |s| {
                                                if current_role == TeamRole::Editor {
                                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                                } else {
                                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                                }
                                            })
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                if let Some(m) = &mut app.team_modal {
                                                    m.invite_role = TeamRole::Editor;
                                                    cx.notify();
                                                }
                                            }))
                                            .child("Editor"),
                                    )
                                    .child(
                                        div()
                                            .id("invite-role-admin")
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(
                                                if current_role == TeamRole::Administrator {
                                                    rgb(MacTheme::ACCENT_BLUE)
                                                } else {
                                                    rgb(MacTheme::SEPARATOR)
                                                },
                                            )
                                            .bg(if current_role == TeamRole::Administrator {
                                                rgb(MacTheme::ACCENT_BLUE)
                                            } else {
                                                rgb(0x323236)
                                            })
                                            .text_xs()
                                            .text_color(
                                                if current_role == TeamRole::Administrator {
                                                    rgb(0xffffff)
                                                } else {
                                                    rgb(MacTheme::TEXT_PRIMARY)
                                                },
                                            )
                                            .cursor_pointer()
                                            .hover(move |s| {
                                                if current_role == TeamRole::Administrator {
                                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                                } else {
                                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                                }
                                            })
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                if let Some(m) = &mut app.team_modal {
                                                    m.invite_role = TeamRole::Administrator;
                                                    cx.notify();
                                                }
                                            }))
                                            .child("Admin"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("invite-submit-btn")
                                    .px_4()
                                    .py_1p5()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::ACCENT_BLUE))
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.invite_team_member(cx);
                                    }))
                                    .child("✉️ Invite Member"),
                            ),
                    );
                modal_box = modal_box.child(invite_form);
            }
            TeamModalTab::AuditLog => {
                let Some(selected_vid) = &modal.selected_vault_id else {
                    modal_box = modal_box.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(0x332810))
                            .border_1()
                            .border_color(rgb(MacTheme::WARNING))
                            .text_sm()
                            .text_color(rgb(MacTheme::WARNING))
                            .child(
                                "⚠️ Please select a shared vault from the 'Shared Vaults' tab first.",
                            ),
                    );
                    return overlay.child(modal_box).into_any_element();
                };

                let selected_vault_name = modal
                    .vaults
                    .iter()
                    .find(|v| &v.id == selected_vid)
                    .map(|v| v.name.as_str())
                    .unwrap_or("Selected Vault");

                modal_box = modal_box.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child(format!(
                            "Tamper-Evident Audit Trail for: {selected_vault_name}"
                        )),
                );

                let mut audit_list = div()
                    .id("team-audit-list")
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .max_h(px(380.0))
                    .overflow_y_scroll();

                if modal.audit_records.is_empty() {
                    audit_list = audit_list.child(
                        div()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("No audit records logged yet for this vault."),
                    );
                } else {
                    for record in &modal.audit_records {
                        let formatted = record.format_entry();
                        let row = div()
                            .px_3()
                            .py_1p5()
                            .rounded_sm()
                            .bg(rgb(MacTheme::CONTENT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .font_family(MONOSPACE_FONT)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(formatted);
                        audit_list = audit_list.child(row);
                    }
                }
                modal_box = modal_box.child(audit_list);
            }
        }

        if self.page == Some("Vaults") {
            div()
                .w_full()
                .child(modal_box.w_full().max_h(px(1800.0)))
                .into_any_element()
        } else {
            overlay.child(modal_box).into_any_element()
        }
    }
}

//! Infra modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_infra_modal(
        &self,
        modal: &InfraModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("infra-modal-overlay")
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
            .id("infra-modal-container")
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
                        if let Some(p) = resolve_icon_path("infrastructure", false, 48) {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .child(gpui::img(p).size_full())
                                .into_any_element()
                        } else {
                            div().child("☁️").into_any_element()
                        },
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Infrastructure inventory"),
                    ),
            )
            .child(
                div()
                    .id("dismiss-infra-modal-btn")
                    .when(self.page == Some("Integrations"), |element| {
                        element.hidden()
                    })
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
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.dismiss_infra_modal(cx);
                    }))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Feedback / Error Banner
        if let Some(err) = &modal.error {
            modal_box = modal_box.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x4a1818))
                    .border_1()
                    .border_color(rgb(MacTheme::DANGER))
                    .text_xs()
                    .text_color(rgb(MacTheme::DANGER))
                    .child(format!("⚠️ {err}")),
            );
        }
        if let Some(status) = &modal.status_message {
            modal_box = modal_box.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(rgb(0x133820))
                    .border_1()
                    .border_color(rgb(MacTheme::SUCCESS))
                    .text_xs()
                    .text_color(rgb(MacTheme::SUCCESS))
                    .child(format!("ℹ️ {status}")),
            );
        }

        // Tab Strip
        let current_tab = modal.active_tab;
        let tabs = div()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .pb_2()
            .child(
                div()
                    .id("infra-tab-providers")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(if current_tab == InfraModalTab::Providers {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if current_tab == InfraModalTab::Providers {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if current_tab == InfraModalTab::Providers {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if current_tab == InfraModalTab::Providers {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(m) = &mut app.infra_modal {
                            m.active_tab = InfraModalTab::Providers;
                            cx.notify();
                        }
                    }))
                    .child("☁️ Providers"),
            )
            .child(
                div()
                    .id("infra-tab-discovered")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(if current_tab == InfraModalTab::Discovered {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if current_tab == InfraModalTab::Discovered {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if current_tab == InfraModalTab::Discovered {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if current_tab == InfraModalTab::Discovered {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(m) = &mut app.infra_modal {
                            m.active_tab = InfraModalTab::Discovered;
                            cx.notify();
                        }
                    }))
                    .child(format!("🖥 Discovered Hosts ({})", modal.discovered.len())),
            )
            .child(
                div()
                    .id("infra-tab-ansible")
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .border_1()
                    .border_color(if current_tab == InfraModalTab::Ansible {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::SEPARATOR)
                    })
                    .bg(if current_tab == InfraModalTab::Ansible {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(0x323236)
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if current_tab == InfraModalTab::Ansible {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .cursor_pointer()
                    .hover(move |s| {
                        if current_tab == InfraModalTab::Ansible {
                            s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                        } else {
                            s.bg(rgb(MacTheme::SURFACE_HOVER))
                        }
                    })
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(m) = &mut app.infra_modal {
                            m.active_tab = InfraModalTab::Ansible;
                            cx.notify();
                        }
                    }))
                    .child("🅰️ Ansible Inventory"),
            );
        modal_box = modal_box.child(tabs);

        match current_tab {
            InfraModalTab::Providers => {
                // Add Provider Form
                let current_kind = modal.new_provider_kind;
                let kind_picker = div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("kind-aws")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if current_kind == CloudProviderKind::Aws {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if current_kind == CloudProviderKind::Aws {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if current_kind == CloudProviderKind::Aws {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if current_kind == CloudProviderKind::Aws {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(m) = &mut app.infra_modal {
                                    m.new_provider_kind = CloudProviderKind::Aws;
                                    cx.notify();
                                }
                            }))
                            .child("☁️ AWS"),
                    )
                    .child(
                        div()
                            .id("kind-azure")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if current_kind == CloudProviderKind::Azure {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if current_kind == CloudProviderKind::Azure {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if current_kind == CloudProviderKind::Azure {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if current_kind == CloudProviderKind::Azure {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(m) = &mut app.infra_modal {
                                    m.new_provider_kind = CloudProviderKind::Azure;
                                    cx.notify();
                                }
                            }))
                            .child("🔷 Azure"),
                    )
                    .child(
                        div()
                            .id("kind-do")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if current_kind == CloudProviderKind::DigitalOcean {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if current_kind == CloudProviderKind::DigitalOcean {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if current_kind == CloudProviderKind::DigitalOcean {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if current_kind == CloudProviderKind::DigitalOcean {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(m) = &mut app.infra_modal {
                                    m.new_provider_kind = CloudProviderKind::DigitalOcean;
                                    cx.notify();
                                }
                            }))
                            .child("🌊 DigitalOcean"),
                    )
                    .child(
                        div()
                            .id("kind-ansible")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if current_kind == CloudProviderKind::Ansible {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::SEPARATOR)
                            })
                            .bg(if current_kind == CloudProviderKind::Ansible {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(0x323236)
                            })
                            .text_xs()
                            .text_color(if current_kind == CloudProviderKind::Ansible {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_PRIMARY)
                            })
                            .cursor_pointer()
                            .hover(move |s| {
                                if current_kind == CloudProviderKind::Ansible {
                                    s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER))
                                } else {
                                    s.bg(rgb(MacTheme::SURFACE_HOVER))
                                }
                            })
                            .on_click(cx.listener(|app, _, _, cx| {
                                if let Some(m) = &mut app.infra_modal {
                                    m.new_provider_kind = CloudProviderKind::Ansible;
                                    cx.notify();
                                }
                            }))
                            .child("🅰️ Ansible"),
                    );

                let add_form = div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Connect New Infrastructure Provider:"),
                    )
                    .child(kind_picker)
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_1().child(modal.new_provider_name.clone()))
                            .child(div().w(px(220.0)).child(modal.new_provider_region.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_1().child(modal.new_provider_path.clone()))
                            .child(
                                div()
                                    .id("create-provider-btn")
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
                                        app.create_infra_provider(cx);
                                    }))
                                    .child("+ Add Provider"),
                            ),
                    );
                modal_box = modal_box.child(add_form);

                // Configured Providers List
                let mut prov_list = div()
                    .id("providers-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_h(px(240.0))
                    .overflow_y_scroll();

                if modal.providers.is_empty() {
                    prov_list = prov_list.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("No infrastructure providers configured yet."),
                    );
                } else {
                    for (p_idx, p) in modal.providers.iter().enumerate() {
                        let pid = p.id.clone();
                        let sync_pid = p.id.clone();
                        let icon = match p.provider_kind.as_str() {
                            "aws" => "☁️",
                            "azure" => "🔷",
                            "digitalocean" => "🌊",
                            "ansible" => "🅰️",
                            _ => "🌐",
                        };

                        let card = div()
                            .id(("provider-card", p_idx))
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .child(
                                div().flex().flex_col().gap_0p5().child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(div().text_sm().child(icon))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                .child(p.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .px_1p5()
                                                .py_0p5()
                                                .rounded_sm()
                                                .bg(rgb(0x183048))
                                                .text_xs()
                                                .text_color(rgb(MacTheme::ACCENT_BLUE))
                                                .child(p.provider_kind.to_uppercase()),
                                        ),
                                ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id(("sync-prov-btn", p_idx))
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
                                                app.sync_infra_provider(sync_pid.clone(), cx);
                                            }))
                                            .child("⚡ Sync"),
                                    )
                                    .child(
                                        div()
                                            .id(("del-prov-btn", p_idx))
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
                                                app.delete_infra_provider(pid.clone(), cx);
                                            }))
                                            .child("🗑 Delete"),
                                    ),
                            );
                        prov_list = prov_list.child(card);
                    }
                }
                modal_box = modal_box.child(prov_list);
            }
            InfraModalTab::Discovered => {
                let mut disc_list = div()
                    .id("discovered-hosts-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_h(px(420.0))
                    .overflow_y_scroll();

                if modal.discovered.is_empty() {
                    disc_list = disc_list.child(
                        div()
                            .p_4()
                            .rounded_md()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("No hosts discovered yet. Connect a provider and trigger Sync, or import an inventory file."),
                    );
                } else {
                    for (h_idx, h) in modal.discovered.iter().enumerate() {
                        let is_imported = h.imported_host_id.is_some();
                        let state_badge = if h.state.eq_ignore_ascii_case("running")
                            || h.state.eq_ignore_ascii_case("active")
                        {
                            "🟢 Running"
                        } else {
                            "🔴 Stopped"
                        };

                        let row = div()
                            .id(("disc-host-card", h_idx))
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_3()
                            .rounded_md()
                            .bg(rgb(MacTheme::SURFACE))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
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
                                                    .child(h.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(rgb(MacTheme::INPUT_BG))
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                                    .child(state_badge),
                                            )
                                            .child({
                                                let reg = h.region.as_deref().unwrap_or("-");
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                                    .child(format!("[{reg}]"))
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(MONOSPACE_FONT)
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child(format!(
                                                "{}@{}:{}",
                                                h.username.as_deref().unwrap_or("user"),
                                                h.hostname,
                                                h.port
                                            )),
                                    ),
                            )
                            .child({
                                if is_imported {
                                    div()
                                        .id(("imported-badge", h_idx))
                                        .px_2p5()
                                        .py_1()
                                        .rounded_md()
                                        .bg(rgb(0x133820))
                                        .border_1()
                                        .border_color(rgb(MacTheme::SUCCESS))
                                        .text_xs()
                                        .text_color(rgb(MacTheme::SUCCESS))
                                        .child(format!(
                                            "✅ Imported (ID: {})",
                                            h.imported_host_id.unwrap_or(0)
                                        ))
                                } else {
                                    div()
                                        .id(("import-host-btn", h_idx))
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(rgb(MacTheme::ACCENT_BLUE))
                                        .bg(rgb(MacTheme::ACCENT_BLUE))
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xffffff))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(MacTheme::ACCENT_BLUE_HOVER)))
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            app.import_discovered_host_into_inventory(h_idx, cx);
                                        }))
                                        .child("📥 Import Host")
                                }
                            });
                        disc_list = disc_list.child(row);
                    }
                }
                modal_box = modal_box.child(disc_list);
            }
            InfraModalTab::Ansible => {
                let ansible_section = div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .rounded_md()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Export Shellumi Inventory to Ansible"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Generate fully compliant Ansible static and dynamic inventory formats from your current Shellumi hosts, groups, and tags."),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                div()
                                    .id("export-ansible-ini-btn")
                                    .px_4()
                                    .py_2()
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
                                        app.export_ansible_inventory_file(false, cx);
                                    }))
                                    .child("📤 Export to Ansible INI"),
                            )
                            .child(
                                div()
                                    .id("export-ansible-yaml-btn")
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(MacTheme::SEPARATOR))
                                    .bg(rgb(0x323236))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.export_ansible_inventory_file(true, cx);
                                    }))
                                    .child("📤 Export to Ansible YAML"),
                            ),
                    );
                modal_box = modal_box.child(ansible_section);
            }
        }

        if self.page == Some("Integrations") {
            div()
                .w_full()
                .child(modal_box.w_full().max_h(px(1800.0)))
                .into_any_element()
        } else {
            overlay.child(modal_box).into_any_element()
        }
    }
}

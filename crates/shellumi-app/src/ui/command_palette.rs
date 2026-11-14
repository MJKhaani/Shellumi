//! Command palette rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_command_palette(
        &self,
        palette: &CommandPaletteState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("command-palette-overlay")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000bb))
            .flex()
            .items_start()
            .justify_center()
            .pt_16();

        let query = palette.input.read(cx).value();
        let matches = palette.registry.search(query);

        let mut modal_box = div()
            .id("command-palette-card")
            .w(px(640.0))
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .shadow_2xl()
            .overflow_hidden()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::handle_palette_key_down));

        // Search Input Header
        let search_icon = if let Some(p) = resolve_icon_path("search", false, 48) {
            div()
                .w(px(20.0))
                .h(px(20.0))
                .child(gpui::img(p).size_full())
                .into_any_element()
        } else {
            div().text_lg().child("🔍").into_any_element()
        };

        let search_header = div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .bg(rgb(MacTheme::SIDEBAR_BG))
            .border_b_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(search_icon)
            .child(div().flex_1().child(palette.input.clone()))
            .child(
                div()
                    .id("dismiss-palette-btn")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_command_palette(cx)))
                    .child("Esc"),
            );
        modal_box = modal_box.child(search_header);

        // Results Container
        if matches.is_empty() {
            let empty_icon = if let Some(p) = resolve_icon_path("search", false, 64) {
                div()
                    .w(px(36.0))
                    .h(px(36.0))
                    .opacity(0.5)
                    .child(gpui::img(p).size_full())
                    .into_any_element()
            } else {
                div().text_2xl().child("🔍").into_any_element()
            };
            let empty_view = div()
                .p_8()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(empty_icon)
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("No commands, hosts, or snippets match your query."),
                );
            modal_box = modal_box.child(empty_view);
        } else {
            let mut results_list = div()
                .id("palette-results-list")
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .max_h(px(380.0))
                .overflow_y_scroll();

            for (idx, mat) in matches.iter().enumerate() {
                let is_selected = idx == palette.selected_idx;
                let (bg_color, border_left) = if is_selected {
                    (rgb(MacTheme::ACCENT_BLUE), rgb(MacTheme::ACCENT_BLUE_HOVER))
                } else {
                    (rgb(MacTheme::SURFACE), rgb(MacTheme::SURFACE))
                };

                let cat = mat.command.category;
                let (cat_bg, cat_fg) = match cat {
                    PaletteCategory::Host => (rgb(0x064e3b), rgb(0xa7f3d0)),
                    PaletteCategory::Snippet => (rgb(0x581c87), rgb(0xe9d5ff)),
                    PaletteCategory::Terminal => (rgb(0x1e3a8a), rgb(0xbfdbfe)),
                    PaletteCategory::Sync => (rgb(0x78350f), rgb(0xfde68a)),
                    PaletteCategory::Tunnel => (rgb(0x831843), rgb(0xfbcfe8)),
                    PaletteCategory::Sftp => (rgb(0x14532d), rgb(0xbbf7d0)),
                    PaletteCategory::Navigation => (rgb(0x374151), rgb(0xf3f4f6)),
                    PaletteCategory::Workspace => (rgb(0x312e81), rgb(0xc7d2fe)),
                    PaletteCategory::Collaboration => (rgb(0x4338ca), rgb(0xe0e7ff)),
                    PaletteCategory::Infrastructure => (rgb(0x0e7490), rgb(0xcffafe)),
                    PaletteCategory::Diagnostics => (rgb(0x0f766e), rgb(0xccfbf1)),
                    PaletteCategory::Mobile => (rgb(0x831843), rgb(0xfce7f3)),
                };

                let action = mat.command.action.clone();
                let action_icon_name = match &action {
                    PaletteAction::ConnectHost { .. } => "hosts",
                    PaletteAction::ConnectSftp { .. } => "sftp",
                    PaletteAction::ExecuteSnippet { .. }
                    | PaletteAction::ExecuteBatchSnippet { .. } => "snippets",
                    PaletteAction::QuickConnect => "connect",
                    PaletteAction::TriggerSync | PaletteAction::OpenConflictResolver => {
                        "cloud_sync"
                    }
                    PaletteAction::NewLocalTerminal => "terminal",
                    PaletteAction::SplitVertical | PaletteAction::SplitHorizontal => "split_view",
                    PaletteAction::CloseActiveTab => "delete",
                    PaletteAction::ToggleBroadcastInput => "terminal",
                    PaletteAction::SaveCurrentLayoutAsWorkspace
                    | PaletteAction::LoadWorkspace { .. } => "workspaces",
                    PaletteAction::OpenSnippetsDrawer => "snippets",
                    PaletteAction::AddNewHost => "add_host",
                    PaletteAction::AddNewSnippet => "edit",
                    PaletteAction::AddNewTunnel => "port_forwarding",
                    PaletteAction::NavigateToHosts => "hosts",
                    PaletteAction::SwitchToTab(_) => "terminal",
                    PaletteAction::OpenTeamCollaboration
                    | PaletteAction::ToggleMultiplayerCollaboration => "invite_member",
                    PaletteAction::OpenInfrastructureManager
                    | PaletteAction::SyncCloudInfrastructure => "infrastructure",
                    PaletteAction::ImportAnsibleInventory => "import",
                    PaletteAction::ExportAnsibleInventory => "export",
                    PaletteAction::RunDiagnostics | PaletteAction::RunBenchmarks => "diagnostics",
                    PaletteAction::OpenMobileSimulator => "mobile",
                    PaletteAction::OpenShellIntegrationModal => "terminal",
                    PaletteAction::OpenSessionRecordings => "log",
                };

                let cat_icon_name = match cat {
                    PaletteCategory::Host => "hosts",
                    PaletteCategory::Snippet => "snippets",
                    PaletteCategory::Terminal => "terminal",
                    PaletteCategory::Sync => "cloud_sync",
                    PaletteCategory::Tunnel => "port_forwarding",
                    PaletteCategory::Sftp => "sftp",
                    PaletteCategory::Navigation => "hosts",
                    PaletteCategory::Workspace => "workspaces",
                    PaletteCategory::Collaboration => "invite_member",
                    PaletteCategory::Infrastructure => "infrastructure",
                    PaletteCategory::Diagnostics => "diagnostics",
                    PaletteCategory::Mobile => "mobile",
                };

                let action_icon_el =
                    if let Some(p) = resolve_icon_path(action_icon_name, is_selected, 48) {
                        div()
                            .w(px(26.0))
                            .h(px(26.0))
                            .rounded_md()
                            .bg(if is_selected {
                                rgb(MacTheme::SURFACE_ACTIVE)
                            } else {
                                rgb(0x232733)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .w(px(18.0))
                                    .h(px(18.0))
                                    .child(gpui::img(p).size_full()),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .w(px(26.0))
                            .h(px(26.0))
                            .rounded_md()
                            .bg(rgb(0x232733))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .child(cat.badge_icon())
                            .into_any_element()
                    };

                let cat_icon_el = if let Some(p) = resolve_icon_path(cat_icon_name, false, 32) {
                    div()
                        .w(px(14.0))
                        .h(px(14.0))
                        .child(gpui::img(p).size_full())
                        .into_any_element()
                } else {
                    div().child(cat.badge_icon()).into_any_element()
                };

                let mut row = div()
                    .id(("palette-item", idx))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(bg_color)
                    .border_l_4()
                    .border_color(border_left)
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.execute_palette_action(action.clone(), window, cx);
                    }));

                let left_content = div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(action_icon_el)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_selected {
                                        rgb(0xffffff)
                                    } else {
                                        rgb(0xe5e7eb)
                                    })
                                    .child(mat.command.title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if is_selected {
                                        rgb(0xd1d5db)
                                    } else {
                                        rgb(0x9ca3af)
                                    })
                                    .child(mat.command.description.clone()),
                            ),
                    );
                row = row.child(left_content);

                let mut right_content = div().flex().items_center().gap_2();
                right_content = right_content.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .bg(cat_bg)
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cat_fg)
                        .child(cat_icon_el)
                        .child(cat.as_str()),
                );

                if let Some(sc) = &mat.command.shortcut {
                    right_content = right_content.child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(rgb(0x323236))
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .text_xs()
                            .font_family(MONOSPACE_FONT)
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(sc.clone()),
                    );
                }

                row = row.child(right_content);

                results_list = results_list.child(row);
            }

            modal_box = modal_box.child(results_list);
        }

        // Footer
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_2()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .bg(rgb(MacTheme::INPUT_BG))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("↑↓ navigate  •  ↵ execute  •  esc dismiss"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child(format!("{} actions available", matches.len())),
            );
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }
}

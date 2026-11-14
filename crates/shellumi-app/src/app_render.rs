//! Main application rendering.
use super::*;

impl Render for Shellumi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .id("shellumi-root")
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::CONTENT_BG))
            .text_color(rgb(MacTheme::TEXT_PRIMARY))
            .text_size(px(self.desktop.preferences.ui_font_size as f32))
            .on_key_down(cx.listener(Self::handle_palette_key_down))
            .on_action(cx.listener(|app, _: &RefreshHosts, _, cx| app.load_hosts(cx)))
            .on_action(cx.listener(|app, _: &ToggleCommandPalette, window, cx| {
                app.toggle_command_palette(window, cx);
            }))
            .on_action(cx.listener(|app, _: &NewTab, window, cx| {
                app.open_local_terminal(window, cx);
            }))
            .on_action(cx.listener(|app, _: &OpenLocalTerminal, window, cx| {
                app.open_local_terminal(window, cx);
            }))
            .on_action(cx.listener(|app, _: &CloseTab, window, cx| {
                app.close_active_tab(window, cx);
            }))
            .on_action(cx.listener(|app, _: &NextTab, window, cx| {
                app.next_tab(window, cx);
            }))
            .on_action(cx.listener(|app, _: &PrevTab, window, cx| {
                app.prev_tab(window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab1, window, cx| {
                app.switch_to_tab(0, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab2, window, cx| {
                app.switch_to_tab(1, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab3, window, cx| {
                app.switch_to_tab(2, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab4, window, cx| {
                app.switch_to_tab(3, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab5, window, cx| {
                app.switch_to_tab(4, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab6, window, cx| {
                app.switch_to_tab(5, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab7, window, cx| {
                app.switch_to_tab(6, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab8, window, cx| {
                app.switch_to_tab(7, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SelectTab9, window, cx| {
                app.switch_to_tab(8, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SplitVertical, window, cx| {
                app.split_pane(SplitDirection::Vertical, window, cx);
            }))
            .on_action(cx.listener(|app, _: &SplitHorizontal, window, cx| {
                app.split_pane(SplitDirection::Horizontal, window, cx);
            }))
            .on_action(cx.listener(|app, _: &FocusNextPane, _, cx| {
                app.focus_next_pane(cx);
            }))
            .on_action(cx.listener(|app, _: &FocusPrevPane, _, cx| {
                app.focus_prev_pane(cx);
            }))
            .on_action(cx.listener(|app, _: &ClosePane, window, cx| {
                app.close_active_pane(window, cx);
            }))
            .on_action(cx.listener(|app, _: &ToggleFocusMode, _, cx| {
                app.toggle_focus_mode(cx);
            }))
            .on_action(cx.listener(|app, _: &ToggleBroadcast, _, cx| {
                app.toggle_broadcast_mode(cx);
            }))
            .on_action(cx.listener(|app, _: &ToggleSnippets, _, cx| {
                app.snippets_panel_open = !app.snippets_panel_open;
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &FindInTerminal, _, cx| {
                app.trigger_find_in_terminal(cx);
            }))
            .on_action(cx.listener(|app, _: &ZoomIn, _, cx| {
                app.desktop.preferences.terminal_font_size =
                    (app.desktop.preferences.terminal_font_size + 1).min(32);
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &ZoomOut, _, cx| {
                app.desktop.preferences.terminal_font_size =
                    (app.desktop.preferences.terminal_font_size.saturating_sub(1)).max(8);
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &ResetZoom, _, cx| {
                app.desktop.preferences.terminal_font_size = 14;
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &QuickConnect, window, cx| {
                app.trigger_quick_connect(window, cx);
            }))
            .on_action(cx.listener(|app, _: &NewHost, _, cx| {
                app.edit_host(None, cx);
            }))
            .on_action(cx.listener(|app, _: &ReconnectSession, window, cx| {
                app.reconnect_active_session(window, cx);
            }))
            .on_action(cx.listener(|app, _: &ToggleSidebar, _, cx| {
                app.sidebar_visible = !app.sidebar_visible;
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToHosts, window, cx| {
                app.page = None;
                app.select_tab(None, window, cx);
            }))
            .on_action(cx.listener(|app, _: &GoToOverview, _, cx| {
                app.page = Some("Overview");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToWorkspaces, _, cx| {
                app.page = Some("Workspaces");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToSftp, _, cx| {
                app.page = Some("SFTP Files");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToKeychain, _, cx| {
                app.page = Some("Keychain");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &GoToPortForwarding, _, cx| {
                app.page = Some("Port Forwarding");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &OpenSettings, _, cx| {
                app.page = Some("Settings");
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &ToggleSidebarCollapse, _, cx| {
                app.toggle_sidebar_collapsed(cx);
            }))
            .on_action(cx.listener(|_app, _: &Quit, _, cx| cx.quit()))
            .on_action(cx.listener(|app, _: &AboutApp, window, cx| {
                app.navigate_page("Settings", window, cx);
            }))
            .on_action(cx.listener(|app, _: &OpenVaultsMenu, _, cx| {
                app.vault_dropdown_open = true;
                cx.notify();
            }))
            .on_action(cx.listener(|app, _: &OpenSftpMenu, window, cx| {
                app.navigate_page("SFTP Files", window, cx);
            }))
            .on_action(cx.listener(|app, _: &OpenSettingsMenu, window, cx| {
                app.navigate_page("Settings", window, cx);
            }))
            .on_action(cx.listener(|app, _: &OpenShortcutsHelp, window, cx| {
                app.navigate_page("Settings", window, cx);
            }));

        root = root.child(self.render_unified_top_bar(cx));

        if let Some(page) = self.page {
            let body = self.render_desktop_page(page, cx);
            let mut container = div().flex().flex_1().min_h_0().overflow_hidden();
            let show_sidebar = self.sidebar_visible && page != "SFTP Files";
            if show_sidebar {
                container = container.child(self.workspace_sidebar(page, cx));
            }
            let mut result = root.child(container.child(body));
            if let Some(dropdown) = self.render_vault_dropdown_overlay(cx) {
                result = result.child(dropdown);
            }
            return self.desktop_overlays(result, cx);
        }

        if let Some(active_idx) = self.active_tab_idx {
            if let Some(tab) = self.tabs.get(active_idx) {
                let is_sftp = tab.kind == SessionKind::Sftp;
                let is_sftp_dual_pane = is_sftp
                    && tab
                        .sftp_state
                        .as_ref()
                        .map(|s| s.mode == SftpTabMode::DualPane)
                        .unwrap_or(false);

                let header_title = match tab.kind {
                    SessionKind::Ssh => format!("SSH · {}", tab.name),
                    SessionKind::Mosh => format!("Mosh · {}", tab.name),
                    SessionKind::Telnet => format!("⚠️ Telnet · {}", tab.name),
                    SessionKind::Serial => format!("Serial · {}", tab.name),
                    SessionKind::Local => format!("Local · {}", tab.name),
                    SessionKind::Sftp => format!("SFTP · {}", tab.name),
                    SessionKind::RemoteDesktop => {
                        if tab
                            .remote_desktop_state
                            .as_ref()
                            .is_some_and(|r| r.protocol == RemoteDesktopProtocol::Vnc)
                        {
                            format!("VNC Remote Desktop · {}", tab.name)
                        } else {
                            format!("RDP Remote Desktop · {}", tab.name)
                        }
                    }
                };
                let header_subtitle = match tab.kind {
                    SessionKind::Ssh => {
                        "Verify the server fingerprint before accepting an unknown host key."
                    }
                    SessionKind::Mosh => {
                        "Mobile Shell roaming UDP connection with local echo prediction."
                    }
                    SessionKind::Telnet => {
                        "⚠️ UNENCRYPTED PROTOCOL: Passwords and session data are transmitted in cleartext over the network."
                    }
                    SessionKind::Serial => {
                        "Physical serial line / UART connection."
                    }
                    SessionKind::Local => "Local shell session.",
                    SessionKind::Sftp => {
                        if is_sftp_dual_pane {
                            "Dual-pane graphical SFTP file manager. Select files to transfer or navigate directories."
                        } else {
                            "Secure File Transfer session. Use standard SFTP commands (ls, get, put, cd, pwd, help)."
                        }
                    }
                    SessionKind::RemoteDesktop => {
                        "Interactive graphical remote desktop session (RFB / MS-RDPBCGR)."
                    }
                };

                let mut body = div()
                    .id("terminal-view")
                    .min_h_0()
                    .overflow_hidden()
                    .track_focus(&self.focus)
                    .on_action(cx.listener(|app, _: &ToggleCommandPalette, window, cx| {
                        app.toggle_command_palette(window, cx);
                    }))
                    .on_key_down(cx.listener(Self::terminal_input))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_6()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_2xl().child(header_title))
                                    .child(
                                        div()
                                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                            .child(header_subtitle),
                                    ),
                            )
                            .child({
                                let mut actions_bar = div().flex().items_center().gap_2();

                                if tab.kind == SessionKind::Telnet {
                                    actions_bar = actions_bar.child(
                                        div()
                                            .id("telnet-warning-pill")
                                            .px_3()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(rgb(0x4a1818))
                                            .text_color(rgb(MacTheme::DANGER))
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .child("⚠️ UNENCRYPTED TELNET"),
                                    );
                                }

                                if is_sftp {
                                    let mode_label = if is_sftp_dual_pane {
                                        "⌨ Terminal Mode"
                                    } else {
                                        "📂 Dual-Pane Mode"
                                    };
                                    actions_bar = actions_bar.child(
                                        div()
                                            .id("sftp-toggle-mode")
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(rgb(MacTheme::ACCENT_BLUE))
                                            .text_sm()
                                            .text_color(rgb(0xffffff))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |app, _, _, cx| {
                                                app.sftp_toggle_mode(active_idx, cx);
                                            }))
                                            .child(mode_label),
                                    );

                                    if is_sftp_dual_pane {
                                        let show_hidden = tab
                                            .sftp_state
                                            .as_ref()
                                            .map(|s| s.show_hidden)
                                            .unwrap_or(false);
                                        let hidden_label = if show_hidden {
                                            "👁 Hide Hidden"
                                        } else {
                                            "👁 Show Hidden"
                                        };
                                        actions_bar = actions_bar.child(
                                            div()
                                                .id("sftp-toggle-hidden")
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(if show_hidden {
                                                    rgb(MacTheme::SURFACE_HOVER)
                                                } else {
                                                    rgb(MacTheme::INPUT_BG)
                                                })
                                                .text_sm()
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                .cursor_pointer()
                                                .on_click(cx.listener(move |app, _, _, cx| {
                                                    app.sftp_toggle_hidden(active_idx, cx);
                                                }))
                                                .child(hidden_label),
                                        );
                                        actions_bar = actions_bar.child(
                                            div()
                                                .id("sftp-refresh")
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(rgb(MacTheme::INPUT_BG))
                                                .text_sm()
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                .cursor_pointer()
                                                .on_click(cx.listener(move |app, _, _, cx| {
                                                    app.sftp_refresh(active_idx, cx);
                                                }))
                                                .child("↻ Refresh"),
                                        );
                                    }
                                }

                                if !is_sftp_dual_pane {
                                    actions_bar = actions_bar
                                        .child(
                                            div()
                                                .id("split-vertical")
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(rgb(MacTheme::INPUT_BG))
                                                .text_sm()
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                .cursor_pointer()
                                                .on_click(cx.listener(|app, _, window, cx| {
                                                    app.split_pane(
                                                        SplitDirection::Vertical,
                                                        window,
                                                        cx,
                                                    )
                                                }))
                                                .child("Split vertical"),
                                        )
                                        .child(
                                            div()
                                                .id("split-horizontal")
                                                .px_3()
                                                .py_2()
                                                .rounded_md()
                                                .bg(rgb(MacTheme::INPUT_BG))
                                                .text_sm()
                                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                                .cursor_pointer()
                                                .on_click(cx.listener(|app, _, window, cx| {
                                                    app.split_pane(
                                                        SplitDirection::Horizontal,
                                                        window,
                                                        cx,
                                                    )
                                                }))
                                                .child("Split horizontal"),
                                        )
                                        .child(
                                            div()
                                                .id("toggle-snippets")
                                                .p_2()
                                                .rounded_md()
                                                .bg(if self.snippets_panel_open {
                                                    rgb(MacTheme::ACCENT_BLUE)
                                                } else {
                                                    rgb(MacTheme::INPUT_BG)
                                                })
                                                .text_sm()
                                                .text_color(if self.snippets_panel_open {
                                                    rgb(0xffffff)
                                                } else {
                                                    rgb(MacTheme::TEXT_PRIMARY)
                                                })
                                                .cursor_pointer()
                                                .on_click(cx.listener(|app, _, _, cx| {
                                                    app.snippets_panel_open =
                                                        !app.snippets_panel_open;
                                                    cx.notify();
                                                }))
                                                .child("Snippets"),
                                        );
                                }

                                if tab.panes.len() > 1 || self.broadcast_manager.is_enabled() {
                                    let is_on = self.broadcast_manager.is_enabled();
                                    let total = tab.panes.len();
                                    let count = if is_on {
                                        total.saturating_sub(
                                            (0..total)
                                                .filter(|&i| {
                                                    self.broadcast_manager.is_pane_excluded(i)
                                                })
                                                .count(),
                                        )
                                    } else {
                                        0
                                    };
                                    actions_bar = actions_bar.child(
                                        div()
                                            .id("toggle-broadcast-input")
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .bg(if is_on {
                                                rgb(MacTheme::DANGER)
                                            } else {
                                                rgb(MacTheme::INPUT_BG)
                                            })
                                            .text_sm()
                                            .text_color(if is_on {
                                                rgb(0xffffff)
                                            } else {
                                                rgb(MacTheme::TEXT_PRIMARY)
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(|app, _, _, cx| {
                                                let total = app
                                                    .active_tab_idx
                                                    .and_then(|i| app.tabs.get(i))
                                                    .map(|t| t.panes.len())
                                                    .unwrap_or(0);
                                                app.broadcast_manager.set_total_panes(total);
                                                let on = app.broadcast_manager.toggle();
                                                if !on {
                                                    app.broadcast_warning = None;
                                                }
                                                cx.notify();
                                            }))
                                            .child(if is_on {
                                                format!("📡 Broadcast: ON ({count}/{total})")
                                            } else {
                                                "📡 Broadcast: Off".to_string()
                                            }),
                                    );
                                }

                                actions_bar
                            }),
                    );

                if let Some(multiplayer) = &tab.multiplayer {
                    let controller = multiplayer.current_controller.as_deref().unwrap_or("None");
                    let owner = &multiplayer.owner_id;
                    let participants_count = multiplayer.participants.len();

                    let mut mp_strip = div()
                        .id("multiplayer-control-strip")
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(MacTheme::SURFACE))
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR));

                    let mut info_left = div().flex().items_center().gap_3();
                    info_left = info_left.child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .child(format!("👥 Session: {}", multiplayer.session_id)),
                    );
                    info_left = info_left.child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(format!("Host: @{owner}")),
                    );
                    info_left = info_left.child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(if controller == "local_user" {
                                rgb(MacTheme::SUCCESS)
                            } else {
                                rgb(MacTheme::WARNING)
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if controller == "local_user" {
                                rgb(0xffffff)
                            } else {
                                rgb(0x000000)
                            })
                            .child(format!("✍️ Active Writer: @{controller}")),
                    );
                    info_left = info_left.child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(format!("{participants_count} participant(s)")),
                    );

                    mp_strip = mp_strip.child(info_left);

                    let mut mp_actions = div().flex().items_center().gap_2();

                    if participants_count > 1 {
                        let next_target = multiplayer
                            .participants
                            .iter()
                            .find(|p| Some(&p.user_id) != multiplayer.current_controller.as_ref())
                            .map(|p| p.user_id.clone())
                            .unwrap_or_else(|| owner.clone());
                        let target_label = next_target.clone();

                        mp_actions = mp_actions.child(
                            div()
                                .id("transfer-writer-btn")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::ACCENT_BLUE))
                                .text_xs()
                                .text_color(rgb(0xffffff))
                                .cursor_pointer()
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.grant_multiplayer_control(&next_target, cx);
                                }))
                                .child(format!("Grant Control to @{target_label}")),
                        );
                    }

                    if multiplayer.current_controller.as_deref() != Some(owner) {
                        mp_actions = mp_actions.child(
                            div()
                                .id("revoke-writer-btn")
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .bg(rgb(MacTheme::WARNING))
                                .text_xs()
                                .text_color(rgb(0x000000))
                                .cursor_pointer()
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.revoke_multiplayer_control(cx);
                                }))
                                .child("⛔ Revoke to Host"),
                        );
                    }

                    mp_actions = mp_actions.child(
                        div()
                            .id("add-demo-collaborator-btn")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.add_multiplayer_demo_participant(cx);
                            }))
                            .child("+ Add Teammate"),
                    );

                    mp_actions = mp_actions.child(
                        div()
                            .id("end-multiplayer-session-btn")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(MacTheme::DANGER))
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.toggle_multiplayer_on_active_tab(cx);
                            }))
                            .child("✕ End"),
                    );

                    mp_strip = mp_strip.child(mp_actions);
                    body = body.child(mp_strip);
                }

                if !is_sftp_dual_pane && self.snippets_panel_open {
                    let mut snippets_bar = div()
                        .id("snippets-drawer")
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .p_3()
                        .bg(rgb(MacTheme::SURFACE))
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(MacTheme::SEPARATOR));

                    if self.snippets.is_empty() {
                        snippets_bar = snippets_bar.child(
                            div()
                                .text_xs()
                                .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                .child(
                                "No saved snippets found. Add snippets via `shellumi snippet add`.",
                            ),
                        );
                    } else {
                        for snippet in &self.snippets {
                            let snip_id = snippet.id;
                            let snip_name = snippet.name.clone();
                            let snip_cmd = snippet.command.clone();
                            let item = div()
                                .id(("snippet-item", snip_id as usize))
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .bg(rgb(MacTheme::INPUT_BG))
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(MacTheme::SEPARATOR))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .child(snip_name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                                        .font_family(MONOSPACE_FONT)
                                        .child(if snip_cmd.len() > 30 {
                                            format!("{}…", &snip_cmd[..30])
                                        } else {
                                            snip_cmd
                                        }),
                                )
                                .child(
                                    div()
                                        .id(("snippet-paste", snip_id as usize))
                                        .px_2()
                                        .py_0p5()
                                        .bg(rgb(MacTheme::SURFACE_HOVER))
                                        .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                        .rounded_sm()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            app.insert_snippet(snip_id, false, cx);
                                            cx.stop_propagation();
                                        }))
                                        .child("Paste"),
                                )
                                .child(
                                    div()
                                        .id(("snippet-run", snip_id as usize))
                                        .px_2()
                                        .py_0p5()
                                        .bg(rgb(MacTheme::SUCCESS))
                                        .text_color(rgb(0xffffff))
                                        .rounded_sm()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            app.insert_snippet(snip_id, true, cx);
                                            cx.stop_propagation();
                                        }))
                                        .child("Run"),
                                )
                                .child(
                                    div()
                                        .id(("snippet-batch", snip_id as usize))
                                        .px_2()
                                        .py_0p5()
                                        .bg(rgb(MacTheme::ACCENT_BLUE))
                                        .text_color(rgb(0xffffff))
                                        .rounded_sm()
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |app, _, _, cx| {
                                            app.open_batch_modal(snip_id, cx);
                                            cx.stop_propagation();
                                        }))
                                        .child("⚡ Multi-Host"),
                                );
                            snippets_bar = snippets_bar.child(item);
                        }
                    }
                    body = body.child(snippets_bar);
                }

                if let Some(warning) = &self.broadcast_warning {
                    let warning_msg = warning.clone();
                    body = body.child(
                        div()
                            .id("broadcast-warning-banner")
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x4a1818))
                            .border_1()
                            .border_color(rgb(MacTheme::DANGER))
                            .text_color(rgb(MacTheme::DANGER))
                            .text_sm()
                            .child(format!("⚠️ {warning_msg}"))
                            .child(
                                div()
                                    .id("dismiss-broadcast-warning")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(MacTheme::DANGER))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|app, _, _, cx| {
                                        app.broadcast_warning = None;
                                        cx.notify();
                                    }))
                                    .child("Dismiss"),
                            ),
                    );
                }

                let mut panes_container = match tab.split {
                    Some(SplitDirection::Grid) => div()
                        .grid()
                        .grid_cols(2)
                        .grid_rows(2)
                        .gap_3()
                        .flex_1()
                        .min_h_0()
                        .min_w_0(),
                    Some(SplitDirection::Vertical) => {
                        div().min_h_0().min_w_0().flex().flex_row().gap_3().flex_1()
                    }
                    Some(SplitDirection::Horizontal) => {
                        div().min_h_0().min_w_0().flex().flex_col().gap_3().flex_1()
                    }
                    None => div().min_h_0().min_w_0().flex().flex_col().flex_1(),
                };

                for (pane_idx, pane) in tab.panes.iter().enumerate() {
                    let pane_id = pane.id;
                    let is_active_pane = tab.active_pane_idx == pane_idx;
                    let grid = pane.terminal.grid();

                    let mut pane_el = div()
                        .id(("terminal-pane", pane_id))
                        .min_h_0()
                        .min_w_0()
                        .overflow_hidden()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(MacTheme::TERMINAL_BG))
                        .border_1()
                        .border_color(if is_active_pane {
                            rgb(MacTheme::ACCENT_BLUE)
                        } else {
                            rgb(MacTheme::BORDER)
                        })
                        .on_click(cx.listener(move |app, _, _, cx| {
                            if let Some(active_idx) = app.active_tab_idx {
                                if let Some(tab) = app.tabs.get_mut(active_idx) {
                                    tab.active_pane_idx = pane_idx;
                                    cx.notify();
                                }
                            }
                        }));

                    // Pane controls header
                    let mut pane_controls = div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_2()
                        .py_1()
                        .bg(rgb(MacTheme::SURFACE))
                        .rounded_sm();

                    let mut pane_info_group = div().flex().items_center().gap_2();

                    let pane_info = format!(
                        "{} · {}x{} · Cursor: row {}, col {}",
                        pane.name,
                        grid.cols(),
                        grid.rows(),
                        grid.cursor().row + 1,
                        grid.cursor().col + 1
                    );
                    pane_info_group = pane_info_group.child(
                        div()
                            .text_xs()
                            .text_color(if is_active_pane {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .child(pane_info),
                    );

                    // Semantic CWD badge
                    if let Some(cwd) = pane.terminal.current_working_directory() {
                        let display_cwd = if cwd.len() > 25 {
                            format!("…{}", &cwd[cwd.len() - 24..])
                        } else {
                            cwd.to_string()
                        };
                        pane_info_group = pane_info_group.child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(rgb(MacTheme::INPUT_BG))
                                .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                .text_xs()
                                .child(format!("📂 {}", display_cwd)),
                        );
                    }

                    // Exit code badge
                    if let Some(code) = pane.terminal.last_exit_code() {
                        let (bg_color, fg_color, text) = if code == 0 {
                            (rgb(0x133820), rgb(MacTheme::SUCCESS), "✓ 0".to_string())
                        } else {
                            (rgb(0x4a1818), rgb(MacTheme::DANGER), format!("✗ {}", code))
                        };
                        pane_info_group = pane_info_group.child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(bg_color)
                                .text_color(fg_color)
                                .text_xs()
                                .child(text),
                        );
                    }

                    // Passive password prompt detection
                    let cursor_row = grid.cursor().row;
                    if let Some(current_line) = pane.terminal.row_text(cursor_row) {
                        if let Some(prompt_match) = PasswordPromptDetector::detect(&current_line) {
                            pane_info_group = pane_info_group.child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x3a2817))
                                    .text_color(rgb(MacTheme::WARNING))
                                    .text_xs()
                                    .child(format!("🔑 Prompt: {:?}", prompt_match.kind)),
                            );
                        }
                    }

                    pane_controls = pane_controls.child(pane_info_group);

                    let mut pane_btn_group = div().flex().items_center().gap_1();
                    let is_recording = pane.terminal.is_recording();
                    let record_btn_label = if is_recording {
                        let dur = pane.terminal.recording_duration();
                        format!("⏹ Stop ({})", format_duration_hms(dur.as_secs_f64()))
                    } else {
                        "⏺ Record".to_string()
                    };

                    pane_btn_group = pane_btn_group.child(
                        div()
                            .id(("toggle-pane-recording", pane_id))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .text_xs()
                            .cursor_pointer()
                            .bg(if is_recording {
                                rgb(MacTheme::DANGER)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_pane_recording(pane_id, cx);
                                cx.stop_propagation();
                            }))
                            .child(record_btn_label),
                    );

                    pane_btn_group = pane_btn_group.child(
                        div()
                            .id(("toggle-search", pane_id))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .text_xs()
                            .cursor_pointer()
                            .bg(if pane.search_open {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.toggle_search(pane_id, cx);
                                cx.stop_propagation();
                            }))
                            .child("Search"),
                    );

                    if tab.panes.len() > 1 && self.broadcast_manager.is_enabled() {
                        let is_excluded = self.broadcast_manager.is_pane_excluded(pane_idx);
                        pane_btn_group = pane_btn_group.child(
                            div()
                                .id(("toggle-pane-broadcast", pane_id))
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .text_xs()
                                .cursor_pointer()
                                .bg(if is_excluded {
                                    rgb(MacTheme::SURFACE_HOVER)
                                } else {
                                    rgb(MacTheme::SUCCESS)
                                })
                                .text_color(rgb(0xffffff))
                                .on_click(cx.listener(move |app, _, _, cx| {
                                    app.broadcast_manager.toggle_pane_exclusion(pane_idx);
                                    cx.notify();
                                    cx.stop_propagation();
                                }))
                                .child(if is_excluded {
                                    "🔇 Excluded"
                                } else {
                                    "📡 Broadcast"
                                }),
                        );
                    }

                    if tab.panes.len() > 1 {
                        pane_btn_group = pane_btn_group.child(
                            div()
                                .id(("close-pane", pane_id))
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .text_xs()
                                .cursor_pointer()
                                .bg(rgb(MacTheme::DANGER))
                                .text_color(rgb(0xffffff))
                                .on_click(cx.listener(move |app, _, window, cx| {
                                    app.close_pane(pane_id, window, cx);
                                    cx.stop_propagation();
                                }))
                                .child("✕ Close"),
                        );
                    }
                    pane_controls = pane_controls.child(pane_btn_group);
                    pane_el = pane_el.child(pane_controls);

                    // Search toolbar (if open)
                    if pane.search_open {
                        let match_count = pane.search_matches.len();
                        let match_label = match (match_count, pane.current_match_idx) {
                            (0, _) => "No matches".to_string(),
                            (c, Some(i)) => format!("{}/{} matches", i + 1, c),
                            (c, None) => format!("{c} matches"),
                        };
                        let search_bar = div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .p_2()
                            .bg(rgb(MacTheme::SURFACE))
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(MacTheme::SEPARATOR))
                            .child(div().flex_1().child(pane.search_input.clone()))
                            .child(
                                div()
                                    .id(("exec-search", pane_id))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(MacTheme::ACCENT_BLUE))
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.execute_search(pane_id, cx);
                                        cx.stop_propagation();
                                    }))
                                    .child("Find"),
                            )
                            .child(
                                div()
                                    .id(("prev-search", pane_id))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(MacTheme::INPUT_BG))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.search_prev(pane_id, cx);
                                        cx.stop_propagation();
                                    }))
                                    .child("↑"),
                            )
                            .child(
                                div()
                                    .id(("next-search", pane_id))
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(MacTheme::INPUT_BG))
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.search_next(pane_id, cx);
                                        cx.stop_propagation();
                                    }))
                                    .child("↓"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(match_label),
                            );
                        pane_el = pane_el.child(search_bar);
                    }

                    if let Some(message) = &pane.message {
                        pane_el = pane_el.child(
                            div()
                                .px_2()
                                .py_1()
                                .text_sm()
                                .text_color(rgb(MacTheme::DANGER))
                                .child(message.clone()),
                        );
                    }

                    // Terminal screen rendering with search highlights
                    let font_size = self.desktop.preferences.terminal_font_size as f32;
                    let line_height = font_size * 1.5;
                    let mut screen = div()
                        .id(("terminal-screen", pane_id))
                        .line_height(px(line_height))
                        .track_scroll(&pane.scroll)
                        .text_size(px(self.desktop.preferences.terminal_font_size as f32))
                        .min_h_0()
                        .min_w_0()
                        .flex_1()
                        .overflow_y_scroll()
                        .font_family(MONOSPACE_FONT)
                        .bg(rgb(MacTheme::TERMINAL_BG))
                        .p_3()
                        .flex()
                        .flex_col();

                    let scrollback_len = pane.terminal.scrollback_count();
                    let start_scrollback =
                        scrollback_len.saturating_sub(self.desktop.preferences.scrollback_lines);

                    for i in start_scrollback..scrollback_len {
                        let mut highlight_ranges = Vec::new();
                        if !pane.search_query.is_empty() {
                            for m in &pane.search_matches {
                                if m.buffer == SearchBuffer::Scrollback(i) {
                                    highlight_ranges.push(m.col_start..m.col_end);
                                }
                            }
                        }
                        if let Some(spans) = if highlight_ranges.is_empty() {
                            pane.terminal.scrollback_row_spans(i)
                        } else {
                            pane.terminal
                                .scrollback_row_spans_highlighted(i, &highlight_ranges)
                        } {
                            screen = screen.child(render_terminal_spans(spans, line_height));
                        }
                    }

                    for r in 0..grid.rows() {
                        let mut highlight_ranges = Vec::new();
                        if !pane.search_query.is_empty() {
                            for m in &pane.search_matches {
                                if m.buffer == SearchBuffer::Screen(r) {
                                    highlight_ranges.push(m.col_start..m.col_end);
                                }
                            }
                        }
                        if let Some(spans) = if highlight_ranges.is_empty() {
                            pane.terminal.row_spans(r)
                        } else {
                            pane.terminal.row_spans_highlighted(r, &highlight_ranges)
                        } {
                            screen = screen.child(render_terminal_spans(spans, line_height));
                        }
                    }

                    let view = cx.entity().downgrade();
                    let old_dimensions = (grid.rows(), grid.cols());
                    let resize_sensor = gpui::canvas(
                        move |bounds, window, cx| {
                            let font = gpui::font(MONOSPACE_FONT);
                            let font_id = window.text_system().resolve_font(&font);
                            let width = window
                                .text_system()
                                .advance(font_id, px(font_size), 'M')
                                .map(|s| f32::from(s.width))
                                .unwrap_or(font_size * 0.6);
                            let dimensions = desktop::terminal_dimensions(
                                f32::from(bounds.size.width) - 24.0,
                                f32::from(bounds.size.height) - 24.0,
                                width,
                                line_height,
                            );
                            if dimensions != old_dimensions {
                                cx.defer(move |cx| {
                                    let _ = view.update(cx, |app, cx| {
                                        if let Some(pane) = app
                                            .tabs
                                            .iter_mut()
                                            .flat_map(|tab| &mut tab.panes)
                                            .find(|pane| pane.id == pane_id)
                                        {
                                            if (
                                                pane.terminal.grid().rows(),
                                                pane.terminal.grid().cols(),
                                            ) != dimensions
                                            {
                                                pane.terminal.resize(dimensions.0, dimensions.1);
                                                if let Some(session) = &pane.session {
                                                    let _ = session.resize(
                                                        dimensions.0 as u16,
                                                        dimensions.1 as u16,
                                                    );
                                                }
                                                cx.notify();
                                            }
                                        }
                                    });
                                });
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0();
                    let scroll_handle = pane.scroll.clone();
                    let scrollbar = gpui::canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            let maximum = f32::from(scroll_handle.max_offset().height);
                            if maximum > 0.0 {
                                let height = f32::from(bounds.size.height);
                                let thumb_height =
                                    (height * height / (height + maximum)).max(24.0).min(height);
                                let fraction = (-f32::from(scroll_handle.offset().y) / maximum)
                                    .clamp(0.0, 1.0);
                                let origin = gpui::point(
                                    bounds.origin.x,
                                    bounds.origin.y + px((height - thumb_height) * fraction),
                                );
                                window.paint_quad(gpui::fill(
                                    gpui::Bounds::new(origin, size(px(4.0), px(thumb_height))),
                                    rgb(MacTheme::SURFACE_HOVER),
                                ));
                            }
                        },
                    )
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .w(px(5.0));
                    pane_el = pane_el.child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .flex()
                            .child(screen)
                            .child(resize_sensor)
                            .child(scrollbar),
                    );
                    panes_container = panes_container.child(pane_el);
                }

                let mut result = if is_sftp_dual_pane {
                    let sftp_state = tab.sftp_state.as_ref().unwrap();
                    let row = div().flex().flex_1().min_h_0();
                    root.child(
                        row.child(
                            body.child(self.render_sftp_dual_pane(active_idx, sftp_state, cx)),
                        ),
                    )
                } else if let Some(rd_state) = &tab.remote_desktop_state {
                    let mut row = div().flex().flex_1().min_h_0();
                    if self.sidebar_visible {
                        row = row.child(self.workspace_sidebar("Remote Desktop", cx));
                    }
                    root.child(row.child(
                        body.child(self.render_remote_desktop_session(active_idx, rd_state, cx)),
                    ))
                } else {
                    let mut row = div().flex().flex_1().min_h_0();
                    if self.sidebar_visible {
                        row = row.child(self.workspace_sidebar("Terminal", cx));
                    }
                    root.child(row.child(body.child(panes_container)))
                };
                if let Some(modal) = &self.conflict_modal {
                    result = result.child(self.render_conflict_modal(modal, cx));
                }
                if let Some(modal) = &self.snippet_modal {
                    result = result.child(self.render_snippet_modal(modal, cx));
                }
                if let Some(modal) = &self.batch_modal {
                    result = result.child(self.render_batch_modal(modal, cx));
                }
                if let Some(palette) = &self.command_palette {
                    result = result.child(self.render_command_palette(palette, cx));
                }
                if let Some(modal) = &self.team_modal {
                    result = result.child(self.render_team_modal(modal, cx));
                }
                if let Some(modal) = &self.infra_modal {
                    result = result.child(self.render_infra_modal(modal, cx));
                }
                if let Some(modal) = &self.diagnostics_modal {
                    result = result.child(self.render_diagnostics_modal(modal, cx));
                }
                if let Some(modal) = &self.mobile_modal {
                    result = result.child(self.render_mobile_modal(modal, cx));
                }
                if let Some(modal) = &self.shell_integration_modal {
                    result = result.child(self.render_shell_integration_modal(modal, cx));
                }
                if let Some(modal) = &self.session_replay_modal {
                    result = result.child(self.render_session_replay_modal(modal, cx));
                }
                if let Some(modal) = &self.tunnel_modal {
                    result = result.child(self.render_tunnel_modal(modal, cx));
                }
                if let Some(modal) = &self.create_vault_modal {
                    result = result.child(self.render_create_vault_modal(modal, cx));
                }
                return result;
            }
        }
        self.render_hosts_view(root, window, cx)
    }
}

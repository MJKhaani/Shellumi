//! Shell integration modal rendering.
use super::*;

impl Shellumi {
    pub(crate) fn render_shell_integration_modal(
        &self,
        modal: &ShellIntegrationModalState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay = div()
            .id("shell-integration-modal-overlay")
            .occlude()
            .absolute()
            .inset_0()
            .bg(rgba(0x000000bb))
            .flex()
            .items_center()
            .justify_center();

        let mut modal_box = div()
            .id("shell-integration-modal-box")
            .w(px(780.0))
            .max_h(px(640.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(MacTheme::SURFACE))
            .border_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .shadow_lg()
            .flex()
            .flex_col()
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
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("⚡ Shell Integration & Autocomplete (Phase 13)"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child("Semantic OSC 133 prompts, CWD tracking, password prompt classification, and offline autocomplete"),
                    ),
            )
            .child(
                div()
                    .id("dismiss-shell-integration-modal-btn")
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(MacTheme::SURFACE_HOVER))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .cursor_pointer()
                    .on_click(cx.listener(|app, _, _, cx| app.dismiss_shell_integration_modal(cx)))
                    .child("✕ Close"),
            );
        modal_box = modal_box.child(header);

        // Tab Navigation
        let tab_buttons = [
            (ShellIntegrationTab::Hooks, "🐚 Shell Hooks"),
            (ShellIntegrationTab::OscMarkers, "🏷️ OSC 133 & CWD"),
            (
                ShellIntegrationTab::PasswordDetection,
                "🔑 Password Prompts",
            ),
            (ShellIntegrationTab::Autocomplete, "💡 Autocomplete Engine"),
        ];

        let mut tab_bar = div().flex().items_center().gap_2();
        for (tab, label) in tab_buttons {
            let is_active = modal.active_tab == tab;
            tab_bar = tab_bar.child(
                div()
                    .id(("shell-tab", tab as usize))
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(if is_active {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .bg(if is_active {
                        rgb(MacTheme::ACCENT_BLUE)
                    } else {
                        rgb(MacTheme::INPUT_BG)
                    })
                    .text_color(if is_active {
                        rgb(0xffffff)
                    } else {
                        rgb(MacTheme::TEXT_SECONDARY)
                    })
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.set_shell_integration_tab(tab, cx);
                    }))
                    .child(label),
            );
        }
        modal_box = modal_box.child(tab_bar);

        // Body per active tab
        match modal.active_tab {
            ShellIntegrationTab::Hooks => {
                let mut hooks_body = div().flex().flex_col().gap_3();

                // Shell selector pills
                let mut shell_pills = div().flex().items_center().gap_2();
                shell_pills = shell_pills.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Target Shell:"),
                );
                for (idx, shell) in ["zsh", "bash", "fish"].into_iter().enumerate() {
                    let is_sel = modal.selected_shell == shell;
                    shell_pills = shell_pills.child(
                        div()
                            .id(("shell-pill", idx))
                            .px_2p5()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_xs()
                            .bg(if is_sel {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(if is_sel {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.set_shell_integration_shell(shell, cx);
                            }))
                            .child(shell.to_uppercase()),
                    );
                }
                hooks_body = hooks_body.child(shell_pills);

                // Generated Hook script display
                let hook_code = match modal.selected_shell.as_str() {
                    "bash" => generate_bash_hook(),
                    "fish" => generate_fish_hook(),
                    _ => generate_zsh_hook(),
                };

                let script_box = div()
                    .id("hook-script-box")
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .max_h(px(220.0))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(hook_code),
                    );
                hooks_body = hooks_body.child(script_box);

                // Instructions
                let install_cmd =
                    format!("eval \"$(shellumi shell-hook {})\"", modal.selected_shell);
                let instructions = div()
                    .p_2p5()
                    .rounded_md()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_PRIMARY))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child("Installation Instructions:"),
                    )
                    .child(
                        div()
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(format!(
                                "Add the following to your ~/.{}rc or config file:",
                                modal.selected_shell
                            )),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(MacTheme::CONTENT_BG))
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .child(install_cmd),
                    );
                hooks_body = hooks_body.child(instructions);

                modal_box = modal_box.child(hooks_body);
            }
            ShellIntegrationTab::OscMarkers => {
                let mut markers_body = div().flex().flex_col().gap_3();

                // Explanatory breakdown
                let explanation = div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::INPUT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child("Semantic Prompt Escape Sequences (OSC 133 & OSC 7):"),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
                            "• OSC 133 ; A ST — Prompt Start: Signals the terminal that the interactive shell prompt has begun rendering.",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
                            "• OSC 133 ; B ST — Command Start: Signals the user has begun typing command input.",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
                            "• OSC 133 ; C ST — Command Executed: User pressed Enter; command process is now running.",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
                            "• OSC 133 ; D [; exitcode] ST — Command Finished: Command exited with status code (e.g. 0 for success).",
                        ),
                    )
                    .child(
                        div().text_xs().text_color(rgb(MacTheme::TEXT_SECONDARY)).child(
                            "• OSC 7 file://hostname/path ST — CWD Notification: Updates the active working directory badge.",
                        ),
                    );
                markers_body = markers_body.child(explanation);

                // Active Pane State Inspector
                let active_tab_info = if let Some(tab_idx) = self.active_tab_idx {
                    if let Some(tab) = self.tabs.get(tab_idx) {
                        if let Some(pane) = tab.panes.get(tab.active_pane_idx) {
                            let cwd = pane
                                .terminal
                                .current_working_directory()
                                .unwrap_or("(none reported)");
                            let exit_code = pane
                                .terminal
                                .last_exit_code()
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "(none)".to_string());
                            let mark = format!("{:?}", pane.terminal.prompt_mark());
                            let history_len = pane.terminal.command_history().len();

                            format!(
                                "Active Pane '{}': CWD = {} | Last Exit = {} | Mark = {} | Captured Commands = {}",
                                pane.name, cwd, exit_code, mark, history_len
                            )
                        } else {
                            "No active terminal pane".into()
                        }
                    } else {
                        "No active tab".into()
                    }
                } else {
                    "No active tab".into()
                };

                let inspector = div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(MacTheme::ACCENT_BLUE))
                            .child("🔍 Live Session State Inspector:"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(MacTheme::TEXT_SECONDARY))
                            .child(active_tab_info),
                    );
                markers_body = markers_body.child(inspector);

                modal_box = modal_box.child(markers_body);
            }
            ShellIntegrationTab::PasswordDetection => {
                let mut pw_body = div().flex().flex_col().gap_3();

                // Security Banner
                let sec_banner = div()
                    .p_2p5()
                    .rounded_md()
                    .bg(rgb(0x3a2817))
                    .border_1()
                    .border_color(rgb(MacTheme::WARNING))
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().child("🔒"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::WARNING))
                            .child("Security Guarantee: Password prompt detection is strictly passive. Shellumi NEVER auto-injects credentials into arbitrary terminal streams without explicit user confirmation."),
                    );
                pw_body = pw_body.child(sec_banner);

                // Sample prompt buttons
                let mut sample_buttons = div().flex().flex_col().gap_1p5();
                sample_buttons = sample_buttons.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Click a sample prompt line to test classification:"),
                );

                let samples = [
                    ("[sudo] password for dev:", "Sudo"),
                    ("user@prod-server.corp's password:", "SSH Password"),
                    (
                        "Enter passphrase for key '/Users/user/.ssh/id_ed25519':",
                        "Key Passphrase",
                    ),
                    ("Enter 6-digit MFA verification code:", "MFA Code"),
                    ("Password: ", "Generic Password"),
                ];

                let mut sample_row = div().flex().flex_wrap().gap_2();
                for (idx, (prompt_sample, label)) in samples.into_iter().enumerate() {
                    sample_row = sample_row.child(
                        div()
                            .id(("sample-prompt", idx))
                            .px_2p5()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(MacTheme::INPUT_BG))
                            .text_xs()
                            .cursor_pointer()
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .hover(|s| s.bg(rgb(MacTheme::SURFACE_HOVER)))
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.update_test_prompt_input(prompt_sample, cx);
                            }))
                            .child(label),
                    );
                }
                sample_buttons = sample_buttons.child(sample_row);
                pw_body = pw_body.child(sample_buttons);

                // Detection Output Box
                let detected_label = match modal.detected_prompt_kind {
                    Some(PasswordPromptKind::Sudo) => {
                        "Detected: [Sudo] Privilege escalation password prompt"
                    }
                    Some(PasswordPromptKind::Ssh) => {
                        "Detected: [SSH] Remote authentication password prompt"
                    }
                    Some(PasswordPromptKind::KeyPassphrase) => {
                        "Detected: [Key Passphrase] Encrypted private key passphrase prompt"
                    }
                    Some(PasswordPromptKind::MfaCode) => {
                        "Detected: [MFA Code] Multi-factor authentication one-time passcode prompt"
                    }
                    Some(PasswordPromptKind::GenericPassword) => {
                        "Detected: [Generic Password] Standard password input prompt"
                    }
                    None => "No password prompt detected (Safe standard output)",
                };

                let output_box = div()
                    .p_3()
                    .rounded_md()
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("Current Prompt Line:"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                            .child(modal.test_prompt_input.clone()),
                    )
                    .child(
                        div()
                            .mt_1()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .bg(if modal.detected_prompt_kind.is_some() {
                                rgb(0x3a2817)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(if modal.detected_prompt_kind.is_some() {
                                rgb(MacTheme::WARNING)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .child(detected_label),
                    );
                pw_body = pw_body.child(output_box);

                modal_box = modal_box.child(pw_body);
            }
            ShellIntegrationTab::Autocomplete => {
                let mut ac_body = div().flex().flex_col().gap_3();

                // Sample query buttons
                let mut sample_queries = div().flex().items_center().gap_2();
                sample_queries = sample_queries.child(
                    div()
                        .text_xs()
                        .text_color(rgb(MacTheme::TEXT_SECONDARY))
                        .child("Sample Queries:"),
                );
                for (idx, query) in ["git ", "docker ", "cargo ", "ssh ", "systemctl "]
                    .into_iter()
                    .enumerate()
                {
                    let is_active = modal.test_query_input == query;
                    sample_queries = sample_queries.child(
                        div()
                            .id(("sample-query", idx))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .cursor_pointer()
                            .text_xs()
                            .bg(if is_active {
                                rgb(MacTheme::ACCENT_BLUE)
                            } else {
                                rgb(MacTheme::INPUT_BG)
                            })
                            .text_color(if is_active {
                                rgb(0xffffff)
                            } else {
                                rgb(MacTheme::TEXT_SECONDARY)
                            })
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.update_test_autocomplete_query(query, cx);
                            }))
                            .child(query.trim()),
                    );
                }
                ac_body = ac_body.child(sample_queries);

                // Completions List
                let mut list_box = div()
                    .id("autocomplete-results-box")
                    .p_2p5()
                    .rounded_md()
                    .bg(rgb(MacTheme::CONTENT_BG))
                    .border_1()
                    .border_color(rgb(MacTheme::SEPARATOR))
                    .max_h(px(250.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1();

                if modal.autocomplete_results.is_empty() {
                    list_box = list_box.child(
                        div()
                            .text_xs()
                            .text_color(rgb(MacTheme::TEXT_TERTIARY))
                            .child("No autocomplete suggestions found for query."),
                    );
                } else {
                    for item in &modal.autocomplete_results {
                        let kind_color = match item.kind {
                            CompletionKind::Command => rgb(MacTheme::ACCENT_BLUE),
                            CompletionKind::Subcommand => rgb(MacTheme::SUCCESS),
                            CompletionKind::Flag => rgb(MacTheme::WARNING),
                            CompletionKind::FilePath => rgb(0xbf5af2),
                            CompletionKind::Host => rgb(0xff375f),
                            CompletionKind::Snippet => rgb(0xff9f0a),
                            CompletionKind::History => rgb(MacTheme::TEXT_SECONDARY),
                        };

                        let row = div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(MacTheme::SURFACE))
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
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(MacTheme::INPUT_BG))
                                            .text_xs()
                                            .text_color(kind_color)
                                            .child(format!("{:?}", item.kind)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text_color(rgb(MacTheme::TEXT_PRIMARY))
                                            .child(item.display_label.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                                    .child(item.description.clone().unwrap_or_default()),
                            );
                        list_box = list_box.child(row);
                    }
                }
                ac_body = ac_body.child(list_box);

                modal_box = modal_box.child(ac_body);
            }
        }

        // Footer
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .pt_2()
            .border_t_1()
            .border_color(rgb(MacTheme::SEPARATOR))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_TERTIARY))
                    .child("Local-First & Offline Engine: No telemetry or remote dependency"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MacTheme::TEXT_SECONDARY))
                    .child("CLI: shellumi shell-hook / complete / prompt-detect"),
            );
        modal_box = modal_box.child(footer);

        overlay.child(modal_box)
    }
}

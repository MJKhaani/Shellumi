//! Desktop settings page.
use super::*;

impl Shellumi {
    pub(super) fn settings_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut categories = panel()
            .w(px(190.0))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(muted("PREFERENCES"));
        for section in [
            "General",
            "Appearance",
            "Terminal",
            "Connections",
            "Files",
            "Security",
            "Synchronization",
            "Keyboard Shortcuts",
            "Advanced",
        ] {
            let is_active = self.desktop.settings_section == section;
            categories = categories.child(
                button(
                    gpui::SharedString::from(format!("setting-section-{section}")),
                    section,
                )
                .bg(if is_active {
                    rgb(MacTheme::ACCENT_BLUE)
                } else {
                    rgb(MacTheme::SURFACE)
                })
                .text_color(if is_active {
                    rgb(0xffffff)
                } else {
                    rgb(MacTheme::TEXT_PRIMARY)
                })
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.desktop.settings_section = section;
                    cx.notify();
                })),
            );
        }
        let prefs = &self.desktop.preferences;
        let mut content = panel()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(heading(self.desktop.settings_section))
                    .child(button("reset-prefs", "Reset preferences").on_click(
                        cx.listener(|app, _, _, cx| app.change_preference("reset", 0, cx)),
                    )),
            )
            .child(muted(
                "Changes are saved on this device and apply immediately.",
            ));
        let row = |title: &'static str, description: String, control: gpui::AnyElement| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .py_3()
                .border_b_1()
                .border_color(rgb(MacTheme::SEPARATOR))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(heading(title))
                        .child(muted(description)),
                )
                .child(control)
        };
        match self.desktop.settings_section {
            "General" => {
                content = content
                    .child(row(
                        "Default startup screen",
                        "Choose what appears when Shellumi opens.".into(),
                        button(
                            "startup-pref",
                            if prefs.startup_overview {
                                "Overview"
                            } else {
                                "Hosts"
                            },
                        )
                        .on_click(
                            cx.listener(|app, _, _, cx| app.change_preference("startup", 0, cx)),
                        )
                        .into_any_element(),
                    ))
                    .child(row(
                        "Local data",
                        self.database.clone().unwrap_or_default(),
                        muted("Local only").into_any_element(),
                    ));
            }
            "Appearance" => {
                content = content
                    .child(row(
                        "Interface text size",
                        "Adjust the desktop interface font size.".into(),
                        self.preference_stepper("ui", prefs.ui_font_size.to_string(), 1, cx)
                            .into_any_element(),
                    ))
                    .child(row(
                        "Color scheme",
                        "The current desktop design uses the supplied dark navy palette.".into(),
                        muted("Dark").into_any_element(),
                    ));
            }
            "Terminal" => {
                content = content
                    .child(row(
                        "Font size",
                        "Applies to terminal text; SSH and local shell use the same setting."
                            .into(),
                        self.preference_stepper(
                            "font",
                            prefs.terminal_font_size.to_string(),
                            1,
                            cx,
                        )
                        .into_any_element(),
                    ))
                    .child(row(
                        "Scrollback lines",
                        "Maximum retained terminal history per pane.".into(),
                        self.preference_stepper(
                            "scrollback",
                            prefs.scrollback_lines.to_string(),
                            1000,
                            cx,
                        )
                        .into_any_element(),
                    ))
                    .child(
                        button(
                            "open-integration-settings",
                            "Shell integration and autocomplete",
                        )
                        .on_click(
                            cx.listener(|app, _, _, cx| app.open_shell_integration_modal(cx)),
                        ),
                    );
            }
            "Files" => {
                content = content.child(row(
                    "Show hidden files",
                    "Used when opening the local SFTP file browser.".into(),
                    button(
                        "hidden-pref",
                        if prefs.show_hidden_files { "On" } else { "Off" },
                    )
                    .on_click(cx.listener(|app, _, _, cx| app.change_preference("hidden", 0, cx)))
                    .into_any_element(),
                ));
            }
            "Connections" | "Security" => {
                content=content.child(row("SSH host-key verification","Unknown servers require an explicit decision. Changed keys are rejected by OpenSSH.".into(),muted("Required").into_any_element())).child(row("Agent forwarding","SSH sessions do not forward your authentication agent.".into(),muted("Disabled").into_any_element())).child(row("Authentication","System OpenSSH uses your local SSH config, agent and interactive authentication prompts.".into(),muted("OpenSSH").into_any_element()));
            }
            "Synchronization" => {
                content=content.child(heading("Encrypted Google Drive Synchronization")).child(muted("End-to-end encrypted synchronization over HTTPS using system curl transport to your private Google Drive appDataFolder. Passphrases never leave your machine and Google cannot inspect payload ciphertext."));
            }
            "Keyboard Shortcuts" => {
                for (name, key) in [
                    ("Command palette", "Cmd/Ctrl+K"),
                    ("Refresh hosts", "Cmd/Ctrl+R"),
                    ("Paste into terminal", "Cmd+V / Ctrl+Shift+V"),
                    ("Terminal control keys", "Ctrl+A … Ctrl+Z"),
                ] {
                    content = content.child(row(name, "".into(), muted(key).into_any_element()));
                }
            }
            _ => {
                content=content.child(button("run-diagnostics-settings","Run diagnostics").on_click(cx.listener(|app,_,_,cx|app.open_diagnostics_modal(cx)))).child(muted("No telemetry. No mandatory account. Credentials are not stored in the host database."));
            }
        }
        let action_bar = div()
            .id("settings-action-bar")
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_3()
            .border_b_1()
            .border_color(rgb(MacTheme::BORDER))
            .child(heading("Settings"));

        let content_scroll = div()
            .id("settings-content-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_6()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_4()
                    .child(categories)
                    .child(content),
            );

        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(MacTheme::CONTENT_BG))
            .child(action_bar)
            .child(content_scroll)
    }
    pub(super) fn preference_stepper(
        &self,
        key: &'static str,
        value: String,
        step: i32,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                button(gpui::SharedString::from(format!("pref-minus-{key}")), "−").on_click(
                    cx.listener(move |app, _, _, cx| app.change_preference(key, -step, cx)),
                ),
            )
            .child(muted(value))
            .child(
                button(gpui::SharedString::from(format!("pref-plus-{key}")), "+").on_click(
                    cx.listener(move |app, _, _, cx| app.change_preference(key, step, cx)),
                ),
            )
    }
}

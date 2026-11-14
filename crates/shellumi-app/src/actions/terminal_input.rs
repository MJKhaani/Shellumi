//! Terminal input actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_local_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_session(
            "Local terminal".into(),
            SessionKind::Local,
            TerminalCommand::default_shell(),
            window,
            cx,
        );
    }

    pub(crate) fn terminal_input(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" && self.dismiss_top_dialog(cx) {
            cx.stop_propagation();
            return;
        }
        if self.command_palette.is_some() {
            match event.keystroke.key.as_str() {
                "escape" => {
                    self.dismiss_command_palette(cx);
                    cx.stop_propagation();
                    return;
                }
                "down" => {
                    self.command_palette_select_next(cx);
                    cx.stop_propagation();
                    return;
                }
                "up" => {
                    self.command_palette_select_prev(cx);
                    cx.stop_propagation();
                    return;
                }
                "enter" => {
                    self.execute_command_palette_selected(window, cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        if event.keystroke.key == "tab" {
            if let Some(form) = &self.form {
                let handles = [
                    form.name.read(cx).focus_handle(cx),
                    form.group.read(cx).focus_handle(cx),
                    form.hostname.read(cx).focus_handle(cx),
                    form.port.read(cx).focus_handle(cx),
                    form.username.read(cx).focus_handle(cx),
                    form.tags.read(cx).focus_handle(cx),
                    form.jump_host.read(cx).focus_handle(cx),
                    form.startup_snippet.read(cx).focus_handle(cx),
                ];
                let current_idx = handles.iter().position(|h| h.is_focused(window));
                let next_idx = if event.keystroke.modifiers.shift {
                    match current_idx {
                        Some(0) => handles.len() - 1,
                        Some(i) => i - 1,
                        None => handles.len() - 1,
                    }
                } else {
                    match current_idx {
                        Some(i) => (i + 1) % handles.len(),
                        None => 0,
                    }
                };
                window.focus(&handles[next_idx]);
                cx.stop_propagation();
                return;
            }
            if let Some(editor) = &self.desktop.editor {
                let mut handles = vec![
                    editor.name.read(cx).focus_handle(cx),
                    editor.description.read(cx).focus_handle(cx),
                    editor.tags.read(cx).focus_handle(cx),
                ];
                for line in &editor.lines {
                    handles.push(line.read(cx).focus_handle(cx));
                }
                let current_idx = handles.iter().position(|h| h.is_focused(window));
                let next_idx = if event.keystroke.modifiers.shift {
                    match current_idx {
                        Some(0) => handles.len() - 1,
                        Some(i) => i - 1,
                        None => handles.len() - 1,
                    }
                } else {
                    match current_idx {
                        Some(i) => (i + 1) % handles.len(),
                        None => 0,
                    }
                };
                window.focus(&handles[next_idx]);
                cx.stop_propagation();
                return;
            }
        }

        if self.handle_app_shortcut(&event.keystroke, window, cx) {
            cx.stop_propagation();
            return;
        }

        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        let active_pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
        if tab
            .panes
            .get(active_pane_idx)
            .and_then(|p| p.session.as_ref())
            .is_none()
        {
            return;
        }

        if let Some(mp) = &tab.multiplayer {
            if mp.is_active
                && mp.current_controller.as_deref() != Some("local_user")
                && mp.owner_id != "local_user"
            {
                // Keystrokes suppressed: another participant holds exclusive writer control.
                cx.stop_propagation();
                return;
            }
        }

        let broadcast = self.broadcast_manager.is_enabled();

        if is_paste_keystroke(&event.keystroke) {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                if broadcast {
                    if let BroadcastSafetyCheck::Dangerous {
                        pattern,
                        explanation,
                    } = check_dangerous_command(&text)
                    {
                        self.broadcast_warning = Some(format!(
                            "Dangerous broadcast command blocked: '{pattern}' ({explanation})"
                        ));
                        cx.notify();
                        cx.stop_propagation();
                        return;
                    }
                }
                let bytes = text.replace("\r\n", "\r").replace('\n', "\r").into_bytes();
                for (idx, p) in tab.panes.iter_mut().enumerate() {
                    let should_send = if idx == active_pane_idx {
                        true
                    } else {
                        broadcast && !self.broadcast_manager.is_pane_excluded(idx)
                    };
                    if should_send {
                        if let Some(sess) = &p.session {
                            if let Err(error) = sess.input(bytes.clone()) {
                                p.message = Some(error.into());
                                p.session = None;
                            }
                        }
                    }
                }
                cx.notify();
                cx.stop_propagation();
                return;
            }
        }
        if let Some(bytes) = key_bytes(&event.keystroke) {
            if broadcast && (bytes == *b"\r" || bytes == *b"\n") {
                if let Some(active_p) = tab.panes.get(active_pane_idx) {
                    let cursor = active_p.terminal.grid().cursor();
                    let row_text = active_p
                        .terminal
                        .grid()
                        .row_spans(cursor.row)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|s| s.text)
                        .collect::<String>();
                    if let BroadcastSafetyCheck::Dangerous {
                        pattern,
                        explanation,
                    } = check_dangerous_command(&row_text)
                    {
                        self.broadcast_warning = Some(format!(
                            "Dangerous broadcast command blocked: '{pattern}' ({explanation})"
                        ));
                        cx.notify();
                        cx.stop_propagation();
                        return;
                    }
                }
            }

            for (idx, p) in tab.panes.iter_mut().enumerate() {
                let should_send = if idx == active_pane_idx {
                    true
                } else {
                    broadcast && !self.broadcast_manager.is_pane_excluded(idx)
                };
                if should_send {
                    if let Some(sess) = &p.session {
                        if let Err(error) = sess.input(bytes.clone()) {
                            p.message = Some(error.into());
                            p.session = None;
                        }
                    }
                }
            }
            cx.notify();
            cx.stop_propagation();
        }
    }
}

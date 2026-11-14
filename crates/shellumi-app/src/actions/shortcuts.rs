//! Shortcuts actions.
use super::*;

impl Shellumi {
    pub(crate) fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() {
            self.form = None;
            self.form_error = None;
            cx.notify();
            return;
        }
        if self.command_palette.is_some() {
            self.dismiss_command_palette(cx);
            return;
        }
        if self.page.is_some() {
            self.page = None;
            cx.notify();
            return;
        }
        if let Some(active_idx) = self.active_tab_idx {
            if let Some(tab) = self.tabs.get(active_idx) {
                let id = tab.id;
                self.close_tab(id, window, cx);
            }
        }
    }

    pub(crate) fn next_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }
        self.page = None;
        let total = self.tabs.len();
        let next = match self.active_tab_idx {
            Some(idx) => (idx + 1) % total,
            None => 0,
        };
        self.select_tab(Some(next), window, cx);
    }

    pub(crate) fn prev_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.is_empty() {
            return;
        }
        self.page = None;
        let total = self.tabs.len();
        let prev = match self.active_tab_idx {
            Some(0) | None => total - 1,
            Some(idx) => idx - 1,
        };
        self.select_tab(Some(prev), window, cx);
    }

    pub(crate) fn switch_to_tab(
        &mut self,
        idx: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.page = None;
        if idx < self.tabs.len() {
            self.select_tab(Some(idx), window, cx);
        } else if idx == 0 {
            self.select_tab(None, window, cx);
        }
    }

    pub(crate) fn close_active_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get(active_idx) else {
            return;
        };
        if tab.panes.len() > 1 {
            let pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
            if let Some(pane) = tab.panes.get(pane_idx) {
                let pane_id = pane.id;
                self.close_pane(pane_id, window, cx);
            }
        } else {
            let tab_id = tab.id;
            self.close_tab(tab_id, window, cx);
        }
    }

    pub(crate) fn focus_next_pane(&mut self, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if tab.panes.len() > 1 {
            tab.active_pane_idx = (tab.active_pane_idx + 1) % tab.panes.len();
            cx.notify();
        }
    }

    pub(crate) fn focus_prev_pane(&mut self, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if tab.panes.len() > 1 {
            tab.active_pane_idx = if tab.active_pane_idx == 0 {
                tab.panes.len() - 1
            } else {
                tab.active_pane_idx - 1
            };
            cx.notify();
        }
    }

    pub(crate) fn toggle_focus_mode(&mut self, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        if tab.panes.len() > 1 {
            if tab.split.is_some() {
                tab.split = None;
            } else {
                tab.split = Some(SplitDirection::Vertical);
            }
            cx.notify();
        }
    }

    pub(crate) fn toggle_broadcast_mode(&mut self, cx: &mut Context<Self>) {
        let total = self
            .active_tab_idx
            .and_then(|i| self.tabs.get(i))
            .map(|t| t.panes.len())
            .unwrap_or(0);
        self.broadcast_manager.set_total_panes(total);
        self.broadcast_manager.toggle();
        cx.notify();
    }

    pub(crate) fn trigger_find_in_terminal(&mut self, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get(active_idx) else {
            return;
        };
        let pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
        if let Some(pane) = tab.panes.get(pane_idx) {
            let pane_id = pane.id;
            self.toggle_search(pane_id, cx);
        }
    }

    pub(crate) fn reconnect_active_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get(active_idx) else {
            return;
        };
        if tab.kind == SessionKind::Local {
            let tab_id = tab.id;
            self.close_tab(tab_id, window, cx);
            self.open_local_terminal(window, cx);
        } else {
            self.trigger_mobile_lifecycle(MobileLifecycleEvent::NetworkRestored, cx);
        }
    }

    pub(crate) fn trigger_quick_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = None;
        self.select_tab(None, window, cx);
        window.focus(&self.quick_connect_input.read(cx).focus_handle(cx));
        cx.notify();
    }

    pub(crate) fn handle_app_shortcut(
        &mut self,
        key: &gpui::Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let is_primary =
            key.modifiers.platform || (key.modifiers.control && !key.modifiers.platform);
        let shift = key.modifiers.shift;
        let alt = key.modifiers.alt;
        let k = key.key.to_ascii_lowercase();

        // 1. Command Palette: Cmd+K / Ctrl+K (without Shift/Alt)
        if is_primary && !shift && !alt && k == "k" {
            self.toggle_command_palette(window, cx);
            return true;
        }

        // 2. New Tab / Local Terminal: Cmd+T / Ctrl+T, Cmd+L / Ctrl+L
        if is_primary && !shift && !alt && (k == "t" || k == "l") {
            self.open_local_terminal(window, cx);
            return true;
        }

        // 3. Close Tab: Cmd+W / Ctrl+W (without Shift)
        if is_primary && !shift && !alt && k == "w" {
            self.close_active_tab(window, cx);
            return true;
        }

        // 4. Close Pane: Cmd+Shift+W / Ctrl+Shift+W
        if is_primary && shift && !alt && k == "w" {
            self.close_active_pane(window, cx);
            return true;
        }

        // 5. Next Tab: Cmd+Shift+] / Ctrl+Tab / Ctrl+PageDown
        if (is_primary && shift && (k == "]" || k == "}"))
            || (key.modifiers.control && !shift && (k == "tab" || k == "pagedown"))
        {
            self.next_tab(window, cx);
            return true;
        }

        // 6. Previous Tab: Cmd+Shift+[ / Ctrl+Shift+Tab / Ctrl+PageUp
        if (is_primary && shift && (k == "[" || k == "{"))
            || (key.modifiers.control && shift && k == "tab")
            || (key.modifiers.control && !shift && k == "pageup")
        {
            self.prev_tab(window, cx);
            return true;
        }

        // 7. Tab 1-9: Cmd+1..9 / Ctrl+1..9
        if is_primary && !shift && !alt && k.len() == 1 {
            if let Some(ch) = k.chars().next() {
                if ('1'..='9').contains(&ch) {
                    let idx = (ch as usize) - ('1' as usize);
                    self.switch_to_tab(idx, window, cx);
                    return true;
                }
            }
        }

        // 8. Split Vertical: Cmd+D / Ctrl+Shift+E
        if (is_primary && !shift && !alt && k == "d")
            || (key.modifiers.control && shift && !alt && k == "e")
        {
            self.split_pane(SplitDirection::Vertical, window, cx);
            return true;
        }

        // 9. Split Horizontal: Cmd+Shift+D / Ctrl+Shift+O
        if (is_primary && shift && !alt && k == "d")
            || (key.modifiers.control && shift && !alt && k == "o")
        {
            self.split_pane(SplitDirection::Horizontal, window, cx);
            return true;
        }

        // 10. Focus Next Pane: Cmd+] / Alt+Right
        if (is_primary && !shift && !alt && k == "]")
            || (alt && !shift && !is_primary && k == "right")
        {
            self.focus_next_pane(cx);
            return true;
        }

        // 11. Focus Previous Pane: Cmd+[ / Alt+Left
        if (is_primary && !shift && !alt && k == "[")
            || (alt && !shift && !is_primary && k == "left")
        {
            self.focus_prev_pane(cx);
            return true;
        }

        // 12. Switch Focus Mode (Toggle Split / Single): Cmd+Shift+M / Ctrl+Alt+M
        if (is_primary && shift && !alt && k == "m")
            || (key.modifiers.control && alt && !shift && k == "m")
        {
            self.toggle_focus_mode(cx);
            return true;
        }

        // 13. Toggle Broadcast: Cmd+B / Cmd+Shift+B / Ctrl+Alt+B
        if (is_primary && !alt && k == "b") || (key.modifiers.control && alt && k == "b") {
            self.toggle_broadcast_mode(cx);
            return true;
        }

        // 14. Toggle Snippets: Cmd+. / Ctrl+.
        if is_primary && !shift && !alt && k == "." {
            self.snippets_panel_open = !self.snippets_panel_open;
            cx.notify();
            return true;
        }

        // 15. Find in Terminal: Cmd+F / Ctrl+F
        if is_primary && !shift && !alt && k == "f" {
            self.trigger_find_in_terminal(cx);
            return true;
        }

        // 16. Zoom In: Cmd+= / Cmd++ / Ctrl+= / Ctrl++
        if is_primary && !alt && (k == "=" || k == "+") {
            self.desktop.preferences.terminal_font_size =
                (self.desktop.preferences.terminal_font_size + 1).min(32);
            cx.notify();
            return true;
        }

        // 17. Zoom Out: Cmd+- / Ctrl+-
        if is_primary && !alt && k == "-" {
            self.desktop.preferences.terminal_font_size = (self
                .desktop
                .preferences
                .terminal_font_size
                .saturating_sub(1))
            .max(8);
            cx.notify();
            return true;
        }

        // 18. Reset Zoom: Cmd+0 / Ctrl+0
        if is_primary && !alt && k == "0" {
            self.desktop.preferences.terminal_font_size = 14;
            cx.notify();
            return true;
        }

        // 19. Quick Connect: Cmd+O / Ctrl+O (without Shift)
        if is_primary && !shift && !alt && k == "o" {
            self.trigger_quick_connect(window, cx);
            return true;
        }

        // 20. New Host: Cmd+N / Ctrl+N
        if is_primary && !shift && !alt && k == "n" {
            self.edit_host(None, cx);
            return true;
        }

        // 21. Refresh: Cmd+R / Ctrl+R (without Shift)
        if is_primary && !shift && !alt && k == "r" {
            self.load_hosts(cx);
            return true;
        }

        // 22. Reconnect: Cmd+Shift+R / Ctrl+Shift+R
        if is_primary && shift && !alt && k == "r" {
            self.reconnect_active_session(window, cx);
            return true;
        }

        // 23. Toggle Sidebar: Cmd+\ / Ctrl+\
        if is_primary && !alt && (k == "\\" || k == "|") {
            self.sidebar_visible = !self.sidebar_visible;
            cx.notify();
            return true;
        }

        // 24. Settings: Cmd+, / Ctrl+,
        if is_primary && !alt && k == "," {
            self.page = Some("Settings");
            cx.notify();
            return true;
        }

        // 25. Navigation shortcuts:
        if is_primary && shift && !alt {
            match k.as_str() {
                "h" => {
                    self.page = None;
                    self.select_tab(None, window, cx);
                    return true;
                }
                "o" => {
                    self.page = Some("Overview");
                    cx.notify();
                    return true;
                }
                "k" => {
                    self.page = Some("Workspaces");
                    cx.notify();
                    return true;
                }
                "u" => {
                    self.page = Some("SFTP Files");
                    cx.notify();
                    return true;
                }
                "y" => {
                    self.page = Some("Keychain");
                    cx.notify();
                    return true;
                }
                "p" => {
                    self.page = Some("Port Forwarding");
                    cx.notify();
                    return true;
                }
                _ => {}
            }
        }

        false
    }

    pub(crate) fn dismiss_top_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        let dismissed = if self.desktop.confirm.take().is_some()
            || dismiss_editor(&mut self.desktop.editor, self.desktop.busy)
            || self.identity_modal.take().is_some()
            || self.invite_modal.take().is_some()
            || self.sftp_modal.take().is_some()
            || self.tunnel_modal.take().is_some()
            || self.session_replay_modal.take().is_some()
            || self.shell_integration_modal.take().is_some()
            || self.mobile_modal.take().is_some()
            || self.diagnostics_modal.take().is_some()
            || self.infra_modal.take().is_some()
            || self.team_modal.take().is_some()
            || self.command_palette.take().is_some()
            || self.batch_modal.take().is_some()
            || self.snippet_modal.take().is_some()
            || self.conflict_modal.take().is_some()
        {
            true
        } else if dismiss_editor(&mut self.form, self.saving) {
            if self.form.is_none() {
                self.form_error = None;
            }
            true
        } else {
            false
        };
        if dismissed {
            cx.notify();
        }
        dismissed
    }

    pub(crate) fn handle_palette_key_down(
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
        if self.handle_app_shortcut(&event.keystroke, window, cx) {
            cx.stop_propagation();
        }
    }
}

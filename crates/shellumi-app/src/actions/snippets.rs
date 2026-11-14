//! Snippets actions.
use super::*;

impl Shellumi {
    pub(crate) fn insert_snippet(
        &mut self,
        snippet_id: i64,
        execute: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(snippet) = self.snippets.iter().find(|s| s.id == snippet_id).cloned() else {
            return;
        };

        if !snippet.variables.is_empty() {
            self.open_snippet_modal(&snippet, execute, cx);
            return;
        }

        let Some(active_idx) = self.active_tab_idx else {
            return;
        };
        let Some(tab) = self.tabs.get_mut(active_idx) else {
            return;
        };
        let active_pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
        let Some(pane) = tab.panes.get_mut(active_pane_idx) else {
            return;
        };
        let Some(session) = &pane.session else {
            return;
        };

        let params = std::collections::HashMap::new();
        if let Ok(cmd) = shellumi_core::render_command(&snippet.command, &params) {
            let bytes = if execute {
                format!("{cmd}\r").into_bytes()
            } else {
                cmd.into_bytes()
            };
            if let Err(error) = session.input(bytes) {
                pane.message = Some(error.into());
                pane.session = None;
                cx.notify();
            }
        }
    }

    pub(crate) fn open_snippet_modal(
        &mut self,
        snippet: &SavedSnippet,
        execute: bool,
        cx: &mut Context<Self>,
    ) {
        let variables = snippet
            .variables
            .iter()
            .map(|v| {
                let default_val = v.default_value.clone();
                let initial_val = default_val.clone().unwrap_or_default();
                let placeholder = default_val.as_deref().unwrap_or("Value required");
                let input = cx.new(|cx| TextInput::new(cx, &initial_val, placeholder));
                SnippetModalVariable {
                    name: v.name.clone(),
                    is_secret: v.is_secret,
                    default_value: default_val,
                    input,
                }
            })
            .collect();

        self.snippet_modal = Some(SnippetModalState {
            snippet_id: snippet.id,
            snippet_name: snippet.name.clone(),
            command_template: snippet.command.clone(),
            execute,
            variables,
            error: None,
        });
        cx.notify();
    }

    pub(crate) fn dismiss_snippet_modal(&mut self, cx: &mut Context<Self>) {
        self.snippet_modal = None;
        cx.notify();
    }

    pub(crate) fn apply_snippet_modal(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = &self.snippet_modal else {
            return;
        };

        let mut params = std::collections::HashMap::new();
        for var in &modal.variables {
            let val = var.input.read(cx).value().trim().to_string();
            if !val.is_empty() {
                params.insert(var.name.clone(), val);
            }
        }

        match shellumi_core::render_command(&modal.command_template, &params) {
            Ok(cmd) => {
                let execute = modal.execute;
                let Some(active_idx) = self.active_tab_idx else {
                    self.snippet_modal = None;
                    cx.notify();
                    return;
                };
                let Some(tab) = self.tabs.get_mut(active_idx) else {
                    self.snippet_modal = None;
                    cx.notify();
                    return;
                };
                let active_pane_idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
                let Some(pane) = tab.panes.get_mut(active_pane_idx) else {
                    self.snippet_modal = None;
                    cx.notify();
                    return;
                };
                let Some(session) = &pane.session else {
                    self.snippet_modal = None;
                    cx.notify();
                    return;
                };

                let bytes = if execute {
                    format!("{cmd}\r").into_bytes()
                } else {
                    cmd.into_bytes()
                };
                if let Err(error) = session.input(bytes) {
                    pane.message = Some(error.into());
                    pane.session = None;
                }
                self.snippet_modal = None;
                cx.notify();
            }
            Err(err) => {
                if let Some(m) = &mut self.snippet_modal {
                    m.error = Some(err.to_string());
                    cx.notify();
                }
            }
        }
    }
}

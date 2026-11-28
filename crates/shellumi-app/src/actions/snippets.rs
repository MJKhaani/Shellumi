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
            match session.input(bytes) {
                Ok(()) => {}
                Err(crate::session::SessionInputError::Closed) => {
                    pane.message = Some("Session closed.".to_string());
                    pane.session = None;
                    cx.notify();
                }
                Err(error) => {
                    pane.message = Some(error.to_string());
                    cx.notify();
                }
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
                let input = if v.is_secret {
                    let placeholder = if default_val.is_some() {
                        "Default configured (enter to override)"
                    } else {
                        "Secret value required"
                    };
                    cx.new(|cx| TextInput::new(cx, &initial_val, placeholder).masked())
                } else {
                    let placeholder = default_val.as_deref().unwrap_or("Value required");
                    cx.new(|cx| TextInput::new(cx, &initial_val, placeholder))
                };
                SnippetModalVariable {
                    name: v.name.clone(),
                    is_secret: v.is_secret,
                    default_value: default_val,
                    input,
                }
            })
            .collect();

        let (target_pane_id, target_session_generation) = self
            .active_tab_idx
            .and_then(|i| self.tabs.get(i))
            .and_then(|tab| {
                let idx = tab.active_pane_idx.min(tab.panes.len().saturating_sub(1));
                tab.panes
                    .get(idx)
                    .map(|p| (Some(p.id), p.session_generation))
            })
            .unwrap_or((None, 0));

        self.snippet_modal = Some(SnippetModalState {
            snippet_id: snippet.id,
            snippet_name: snippet.name.clone(),
            command_template: snippet.command.clone(),
            execute,
            variables,
            error: None,
            target_pane_id,
            target_session_generation,
            target_vault_id: self.active_vault_id.clone(),
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
            let raw = var.input.read(cx).value();
            let val = if var.is_secret {
                raw.to_string()
            } else {
                raw.trim().to_string()
            };
            if !val.is_empty() {
                params.insert(var.name.clone(), val);
            }
        }

        match shellumi_core::render_command(&modal.command_template, &params) {
            Ok(cmd) => {
                let execute = modal.execute;
                let Some(target_pane_id) = modal.target_pane_id else {
                    if let Some(m) = &mut self.snippet_modal {
                        m.error =
                            Some("No target terminal pane captured for this snippet.".to_string());
                    }
                    cx.notify();
                    return;
                };
                let target_gen = modal.target_session_generation;
                let target_vault = modal.target_vault_id.clone();
                if self.active_vault_id != target_vault {
                    if let Some(m) = &mut self.snippet_modal {
                        m.error =
                            Some("Active vault changed; snippet execution aborted.".to_string());
                    }
                    cx.notify();
                    return;
                }
                let Some(pane) = self.tabs.iter_mut().flat_map(|t| &mut t.panes).find(|p| {
                    p.id == target_pane_id
                        && p.session_generation == target_gen
                        && p.vault_id == target_vault
                }) else {
                    if let Some(m) = &mut self.snippet_modal {
                        m.error = Some("Target terminal session is no longer active; snippet execution aborted.".to_string());
                    }
                    cx.notify();
                    return;
                };
                let Some(session) = &pane.session else {
                    if let Some(m) = &mut self.snippet_modal {
                        m.error = Some("Target terminal session is closed.".to_string());
                    }
                    cx.notify();
                    return;
                };

                let bytes = if execute {
                    format!("{cmd}\r").into_bytes()
                } else {
                    cmd.into_bytes()
                };
                match session.input(bytes) {
                    Ok(()) => {}
                    Err(crate::session::SessionInputError::Closed) => {
                        pane.message = Some("Session closed.".to_string());
                        pane.session = None;
                    }
                    Err(error) => {
                        pane.message = Some(error.to_string());
                    }
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

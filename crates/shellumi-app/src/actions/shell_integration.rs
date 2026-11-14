//! Shell integration actions.
use super::*;

impl Shellumi {
    pub(crate) fn open_shell_integration_modal(&mut self, cx: &mut Context<Self>) {
        let engine = AutocompleteEngine::new();
        let initial_completions = engine.complete(&AutocompleteContext::new("git ", 4));
        self.shell_integration_modal = Some(ShellIntegrationModalState {
            active_tab: ShellIntegrationTab::Hooks,
            selected_shell: "zsh".to_string(),
            test_prompt_input: "[sudo] password for admin:".to_string(),
            detected_prompt_kind: Some(PasswordPromptKind::Sudo),
            test_query_input: "git ".to_string(),
            autocomplete_results: initial_completions,
        });
        cx.notify();
    }

    pub(crate) fn dismiss_shell_integration_modal(&mut self, cx: &mut Context<Self>) {
        self.shell_integration_modal = None;
        cx.notify();
    }

    pub(crate) fn set_shell_integration_tab(
        &mut self,
        tab: ShellIntegrationTab,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref mut m) = self.shell_integration_modal {
            m.active_tab = tab;
        }
        cx.notify();
    }

    pub(crate) fn set_shell_integration_shell(&mut self, shell: &str, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.shell_integration_modal {
            m.selected_shell = shell.to_string();
        }
        cx.notify();
    }

    pub(crate) fn update_test_prompt_input(&mut self, input: &str, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.shell_integration_modal {
            m.test_prompt_input = input.to_string();
            m.detected_prompt_kind = PasswordPromptDetector::detect(input).map(|p| p.kind);
        }
        cx.notify();
    }

    pub(crate) fn update_test_autocomplete_query(&mut self, query: &str, cx: &mut Context<Self>) {
        if let Some(ref mut m) = self.shell_integration_modal {
            m.test_query_input = query.to_string();
            let engine = AutocompleteEngine::new();
            let mut ctx = AutocompleteContext::new(query, query.len());
            if let HostState::Loaded(ref hosts) = self.hosts {
                for h in hosts {
                    ctx.host_candidates.push(h.name.clone());
                }
            }
            for s in &self.snippets {
                ctx.snippet_candidates
                    .push((s.name.clone(), s.command.clone()));
            }
            m.autocomplete_results = engine.complete(&ctx);
        }
        cx.notify();
    }

    // =========================================================================
    // Terminal Session Logging, Audit Recording & Cast Playback Engine
    // =========================================================================
}

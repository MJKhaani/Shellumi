use serde::{Deserialize, Serialize};

/// Category classification for a command palette entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaletteCategory {
    Navigation,
    Host,
    Snippet,
    Tunnel,
    Sftp,
    Sync,
    Terminal,
    Workspace,
}

impl PaletteCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Navigation => "Navigation",
            Self::Host => "Hosts",
            Self::Snippet => "Snippets",
            Self::Tunnel => "Tunnels",
            Self::Sftp => "SFTP",
            Self::Sync => "Cloud Sync",
            Self::Terminal => "Terminal",
            Self::Workspace => "Workspaces",
        }
    }

    pub fn badge_icon(&self) -> &'static str {
        match self {
            Self::Navigation => "🧭",
            Self::Host => "🖥",
            Self::Snippet => "📜",
            Self::Tunnel => "🔀",
            Self::Sftp => "📂",
            Self::Sync => "☁",
            Self::Terminal => "⌨",
            Self::Workspace => "📐",
        }
    }
}

/// Action to dispatch when a palette command is selected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteAction {
    ConnectHost {
        host_id: i64,
        host_name: String,
    },
    ConnectSftp {
        host_id: i64,
        host_name: String,
    },
    ExecuteSnippet {
        snippet_id: i64,
        snippet_name: String,
    },
    ExecuteBatchSnippet {
        snippet_id: i64,
        snippet_name: String,
    },
    QuickConnect,
    TriggerSync,
    OpenConflictResolver,
    NewLocalTerminal,
    SplitVertical,
    SplitHorizontal,
    CloseActiveTab,
    ToggleBroadcastInput,
    SaveCurrentLayoutAsWorkspace,
    LoadWorkspace {
        workspace_id: i64,
        workspace_name: String,
    },
    OpenSnippetsDrawer,
    AddNewHost,
    AddNewSnippet,
    AddNewTunnel,
    NavigateToHosts,
    SwitchToTab(usize),
}

/// A searchable command palette item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteCommand {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: PaletteCategory,
    pub shortcut: Option<String>,
    pub action: PaletteAction,
    pub keywords: Vec<String>,
}

impl PaletteCommand {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        category: PaletteCategory,
        action: PaletteAction,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            category,
            shortcut: None,
            action,
            keywords: Vec::new(),
        }
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn with_keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    pub fn batch_from_snippet(snippet_id: i64, snippet_name: &str) -> Self {
        Self::new(
            format!("batch_snippet_{snippet_id}"),
            format!("Batch Run Snippet: {snippet_name}"),
            format!("Execute snippet '{snippet_name}' across multiple remote hosts"),
            PaletteCategory::Snippet,
            PaletteAction::ExecuteBatchSnippet {
                snippet_id,
                snippet_name: snippet_name.to_string(),
            },
        )
        .with_keywords(["batch", "multi-host", "parallel", "cluster", snippet_name])
    }

    pub fn workspace_from_saved(workspace_id: i64, workspace_name: &str, layout: &str) -> Self {
        Self::new(
            format!("workspace_{workspace_id}"),
            format!("Load Workspace: {workspace_name}"),
            format!("Restore {layout} layout and launch sessions for '{workspace_name}'"),
            PaletteCategory::Workspace,
            PaletteAction::LoadWorkspace {
                workspace_id,
                workspace_name: workspace_name.to_string(),
            },
        )
        .with_keywords(["workspace", "layout", "preset", layout, workspace_name])
    }
}

/// A matched command with relevance score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteMatch {
    pub command: PaletteCommand,
    pub score: i32,
}

/// Registry of palette commands with smart fuzzy/token search scoring.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaletteRegistry {
    commands: Vec<PaletteCommand>,
}

impl PaletteRegistry {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Pre-populates standard static actions (navigation, sync, terminal, forms).
    pub fn with_static_actions() -> Self {
        let mut registry = Self::new();
        registry.register_static_actions();
        registry
    }

    pub fn register(&mut self, command: PaletteCommand) {
        self.commands.push(command);
    }

    pub fn commands(&self) -> &[PaletteCommand] {
        &self.commands
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn register_static_actions(&mut self) {
        self.register(
            PaletteCommand::new(
                "action:quick_connect",
                "Quick Connect",
                "Open immediate SSH connection to user@host[:port] without saving",
                PaletteCategory::Host,
                PaletteAction::QuickConnect,
            )
            .with_shortcut("Cmd+O")
            .with_keywords(["ssh", "adhoc", "connect", "prompt"]),
        );

        self.register(
            PaletteCommand::new(
                "action:new_local_terminal",
                "New Local Terminal",
                "Spawn a new local shell terminal session tab",
                PaletteCategory::Terminal,
                PaletteAction::NewLocalTerminal,
            )
            .with_shortcut("Cmd+T")
            .with_keywords(["shell", "local", "pty", "bash", "zsh"]),
        );

        self.register(
            PaletteCommand::new(
                "action:split_vertical",
                "Split Terminal Vertically",
                "Divide active session into side-by-side terminal panes",
                PaletteCategory::Terminal,
                PaletteAction::SplitVertical,
            )
            .with_shortcut("Cmd+D")
            .with_keywords(["pane", "split", "vertical", "side-by-side"]),
        );

        self.register(
            PaletteCommand::new(
                "action:split_horizontal",
                "Split Terminal Horizontally",
                "Divide active session into stacked terminal panes",
                PaletteCategory::Terminal,
                PaletteAction::SplitHorizontal,
            )
            .with_shortcut("Cmd+Shift+D")
            .with_keywords(["pane", "split", "horizontal", "stacked"]),
        );

        self.register(
            PaletteCommand::new(
                "action:close_tab",
                "Close Active Tab",
                "Terminate active session or split panes and close tab",
                PaletteCategory::Terminal,
                PaletteAction::CloseActiveTab,
            )
            .with_shortcut("Cmd+W")
            .with_keywords(["close", "exit", "kill", "terminate"]),
        );

        self.register(
            PaletteCommand::new(
                "action:toggle_broadcast",
                "Toggle Broadcast Input",
                "Replicate terminal keyboard typing across all active panes simultaneously",
                PaletteCategory::Terminal,
                PaletteAction::ToggleBroadcastInput,
            )
            .with_shortcut("Cmd+Alt+B")
            .with_keywords(["broadcast", "sync", "multi-pane", "replicate", "mirror"]),
        );

        self.register(
            PaletteCommand::new(
                "action:save_workspace",
                "Save Current Layout as Workspace",
                "Persist active split pane session layout as a reusable workspace preset",
                PaletteCategory::Workspace,
                PaletteAction::SaveCurrentLayoutAsWorkspace,
            )
            .with_shortcut("Cmd+Shift+S")
            .with_keywords(["workspace", "save", "layout", "preset", "snapshot"]),
        );

        self.register(
            PaletteCommand::new(
                "action:trigger_sync",
                "Trigger Cloud Sync Now",
                "Initiate immediate zero-knowledge Google Drive vault synchronization",
                PaletteCategory::Sync,
                PaletteAction::TriggerSync,
            )
            .with_shortcut("Cmd+S")
            .with_keywords(["cloud", "e2ee", "vault", "drive", "backup"]),
        );

        self.register(
            PaletteCommand::new(
                "action:resolve_conflicts",
                "Resolve Sync Conflicts",
                "Open side-by-side visual conflict resolution modal",
                PaletteCategory::Sync,
                PaletteAction::OpenConflictResolver,
            )
            .with_keywords(["merge", "diff", "conflict", "resolver"]),
        );

        self.register(
            PaletteCommand::new(
                "action:open_snippets",
                "Open Snippets Drawer",
                "View and manage saved shell automation snippets",
                PaletteCategory::Snippet,
                PaletteAction::OpenSnippetsDrawer,
            )
            .with_shortcut("Cmd+Shift+P")
            .with_keywords(["automation", "script", "template", "variables"]),
        );

        self.register(
            PaletteCommand::new(
                "action:add_host",
                "Add New Host",
                "Open form to configure a new SSH host destination",
                PaletteCategory::Host,
                PaletteAction::AddNewHost,
            )
            .with_shortcut("Cmd+N")
            .with_keywords(["create", "server", "destination", "remote"]),
        );

        self.register(
            PaletteCommand::new(
                "action:add_tunnel",
                "Add New SSH Tunnel",
                "Configure local, remote, or dynamic SOCKS5 port forwarding",
                PaletteCategory::Tunnel,
                PaletteAction::AddNewTunnel,
            )
            .with_keywords(["forward", "port", "socks5", "proxy"]),
        );

        self.register(
            PaletteCommand::new(
                "action:nav_hosts",
                "Go to Hosts Dashboard",
                "Switch view to saved infrastructure host inventory",
                PaletteCategory::Navigation,
                PaletteAction::NavigateToHosts,
            )
            .with_shortcut("Cmd+1")
            .with_keywords(["dashboard", "inventory", "servers", "home"]),
        );
    }

    /// Searches commands by query with fuzzy and tokenized scoring.
    pub fn search(&self, query: &str) -> Vec<PaletteMatch> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self
                .commands
                .iter()
                .cloned()
                .map(|cmd| PaletteMatch {
                    command: cmd,
                    score: 0,
                })
                .collect();
        }

        let query_lower = trimmed.to_lowercase();
        let tokens: Vec<&str> = query_lower.split_whitespace().collect();

        let mut matches = Vec::new();

        for cmd in &self.commands {
            let title_lower = cmd.title.to_lowercase();
            let desc_lower = cmd.description.to_lowercase();
            let cat_lower = cmd.category.as_str().to_lowercase();
            let keywords_lower: Vec<String> =
                cmd.keywords.iter().map(|k| k.to_lowercase()).collect();

            let mut score = 0i32;
            let mut all_tokens_found = true;

            // Full exact or prefix match on title
            if title_lower == query_lower {
                score += 250;
            } else if title_lower.starts_with(&query_lower) {
                score += 150;
            } else if title_lower.contains(&query_lower) {
                score += 80;
            }

            // Category match
            if cat_lower == query_lower || cat_lower.starts_with(&query_lower) {
                score += 50;
            }

            // Acronym match (e.g. "qc" for "Quick Connect", "sv" for "Split Vertical")
            let acronym: String = title_lower
                .split_whitespace()
                .filter_map(|w| w.chars().next())
                .collect();
            let is_acronym_match = acronym == query_lower;
            if is_acronym_match {
                score += 120;
            }

            // Per-token scoring
            for &token in &tokens {
                let mut token_found = false;

                if title_lower.contains(token) {
                    score += 40;
                    token_found = true;
                    // Check word boundary
                    for word in title_lower.split_whitespace() {
                        if word.starts_with(token) {
                            score += 25;
                            break;
                        }
                    }
                }

                if desc_lower.contains(token) {
                    score += 20;
                    token_found = true;
                }

                if cat_lower.contains(token) {
                    score += 15;
                    token_found = true;
                }

                if keywords_lower.iter().any(|k| k.contains(token)) {
                    score += 30;
                    token_found = true;
                }

                if !token_found {
                    all_tokens_found = false;
                }
            }

            // Only match if all tokens matched, query was substring in title, or acronym matched
            if (all_tokens_found || title_lower.contains(&query_lower) || is_acronym_match)
                && score > 0
            {
                // Minor length penalty to favor concise matches
                let len_penalty = (title_lower.len() as i32) / 6;
                score -= len_penalty;

                matches.push(PaletteMatch {
                    command: cmd.clone(),
                    score,
                });
            }
        }

        matches.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.command.title.cmp(&b.command.title))
        });
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_palette_static_actions_registration() {
        let registry = PaletteRegistry::with_static_actions();
        assert!(!registry.commands().is_empty());
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::QuickConnect));
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::TriggerSync));
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::SplitVertical));
    }

    #[test]
    fn verifies_empty_query_returns_all_commands() {
        let registry = PaletteRegistry::with_static_actions();
        let results = registry.search("");
        assert_eq!(results.len(), registry.commands().len());
    }

    #[test]
    fn verifies_exact_title_and_prefix_scoring() {
        let registry = PaletteRegistry::with_static_actions();
        let results = registry.search("Quick Connect");
        assert!(!results.is_empty());
        assert_eq!(results[0].command.title, "Quick Connect");
        assert!(results[0].score >= 200);

        let prefix_results = registry.search("Split");
        assert!(prefix_results.len() >= 2);
        assert!(prefix_results[0].command.title.starts_with("Split"));
    }

    #[test]
    fn verifies_acronym_and_keyword_matching() {
        let registry = PaletteRegistry::with_static_actions();
        // "qc" -> "Quick Connect"
        let qc_results = registry.search("qc");
        assert!(!qc_results.is_empty());
        assert_eq!(qc_results[0].command.title, "Quick Connect");

        // Keyword "socks5" -> "Add New SSH Tunnel"
        let socks_results = registry.search("socks5");
        assert!(!socks_results.is_empty());
        assert_eq!(socks_results[0].command.title, "Add New SSH Tunnel");

        // Category "sync"
        let sync_results = registry.search("sync");
        assert!(!sync_results.is_empty());
        assert_eq!(sync_results[0].command.category, PaletteCategory::Sync);
    }

    #[test]
    fn verifies_dynamic_host_and_snippet_commands() {
        let mut registry = PaletteRegistry::new();
        registry.register(
            PaletteCommand::new(
                "host:1",
                "Connect to prod-db-01",
                "SSH postgres@db.internal:5432 in Production",
                PaletteCategory::Host,
                PaletteAction::ConnectHost {
                    host_id: 1,
                    host_name: "prod-db-01".into(),
                },
            )
            .with_keywords(["postgres", "database", "production"]),
        );

        registry.register(
            PaletteCommand::new(
                "snippet:42",
                "Run Snippet: Docker Cleanup",
                "docker system prune -af",
                PaletteCategory::Snippet,
                PaletteAction::ExecuteSnippet {
                    snippet_id: 42,
                    snippet_name: "Docker Cleanup".into(),
                },
            )
            .with_keywords(["containers", "docker"]),
        );

        let host_search = registry.search("prod-db");
        assert_eq!(host_search.len(), 1);
        assert_eq!(host_search[0].command.id, "host:1");

        let snippet_search = registry.search("prune");
        assert_eq!(snippet_search.len(), 1);
        assert_eq!(snippet_search[0].command.id, "snippet:42");
    }

    #[test]
    fn verifies_workspace_and_broadcast_palette_actions() {
        let mut registry = PaletteRegistry::with_static_actions();
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::ToggleBroadcastInput));
        assert!(registry
            .commands()
            .iter()
            .any(|c| c.action == PaletteAction::SaveCurrentLayoutAsWorkspace));

        let ws_cmd = PaletteCommand::workspace_from_saved(42, "SRE Cluster View", "vertical");
        assert_eq!(ws_cmd.category, PaletteCategory::Workspace);
        assert_eq!(
            ws_cmd.action,
            PaletteAction::LoadWorkspace {
                workspace_id: 42,
                workspace_name: "SRE Cluster View".to_string()
            }
        );
        registry.register(ws_cmd);

        let search_results = registry.search("broadcast");
        assert!(!search_results.is_empty());
        assert_eq!(
            search_results[0].command.action,
            PaletteAction::ToggleBroadcastInput
        );

        let ws_results = registry.search("SRE Cluster");
        assert!(!ws_results.is_empty());
        assert_eq!(ws_results[0].command.category, PaletteCategory::Workspace);
    }
}

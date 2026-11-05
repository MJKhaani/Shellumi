//! Intelligent in-terminal autocomplete and password prompt detection.
//!
//! Provides context-aware completions across commands, subcommands, flags, file paths,
//! saved snippets, inventory hostnames, and history, along with safe prompt classification
//! according to Section 3.10 of PROJECT.md.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Classification of an autocomplete suggestion candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompletionKind {
    /// Command executable or shell builtin (e.g. `git`, `docker`, `cargo`, `cd`, `ls`).
    Command,
    /// Command subcommand (e.g. `git status`, `docker compose`, `cargo test`).
    Subcommand,
    /// Command line flag or option (e.g. `--all`, `-v`, `--help`).
    Flag,
    /// File system path or directory.
    FilePath,
    /// Remote host destination or alias from inventory.
    Host,
    /// Reusable snippet from library.
    Snippet,
    /// Previously executed command from history.
    History,
}

impl CompletionKind {
    pub fn badge_icon(&self) -> &'static str {
        match self {
            Self::Command => "⚡",
            Self::Subcommand => "⚙️",
            Self::Flag => "🚩",
            Self::FilePath => "📁",
            Self::Host => "🖥️",
            Self::Snippet => "📜",
            Self::History => "🕒",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Command => "Command",
            Self::Subcommand => "Subcommand",
            Self::Flag => "Flag",
            Self::FilePath => "Path",
            Self::Host => "Host",
            Self::Snippet => "Snippet",
            Self::History => "History",
        }
    }
}

/// A single autocomplete suggestion item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionItem {
    /// Replacement or completion text to insert into the command line.
    pub value: String,
    /// Text to display in the autocomplete popup.
    pub display_label: String,
    /// Optional documentation or detail describing the item.
    pub description: Option<String>,
    /// Kind of completion.
    pub kind: CompletionKind,
    /// Match score (higher is better).
    pub score: i64,
}

/// Context provided to the autocomplete engine to evaluate suggestions.
#[derive(Clone, Debug, Default)]
pub struct AutocompleteContext {
    /// The entire current input line being typed at the prompt.
    pub input_line: String,
    /// Character offset of the cursor within `input_line`.
    pub cursor_pos: usize,
    /// Active current working directory reported via OSC 7, if available.
    pub current_dir: Option<String>,
    /// Host identifiers or aliases available from inventory.
    pub host_candidates: Vec<String>,
    /// Snippets available from library (name, command).
    pub snippet_candidates: Vec<(String, String)>,
    /// Recent command history.
    pub history_candidates: Vec<String>,
}

impl AutocompleteContext {
    /// Creates a new autocomplete context with input line and cursor position.
    pub fn new(input_line: impl Into<String>, cursor_pos: usize) -> Self {
        Self {
            input_line: input_line.into(),
            cursor_pos,
            current_dir: None,
            host_candidates: Vec::new(),
            snippet_candidates: Vec::new(),
            history_candidates: Vec::new(),
        }
    }
}

/// Internal dictionary entry describing subcommands and flags of known CLI tools.
struct CommandSpec {
    subcommands: Vec<(&'static str, &'static str)>,
    flags: Vec<(&'static str, &'static str)>,
}

/// Autocomplete engine providing fuzzy-matched, ranked suggestions.
pub struct AutocompleteEngine {
    known_commands: HashMap<&'static str, CommandSpec>,
}

impl Default for AutocompleteEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AutocompleteEngine {
    /// Constructs a new autocomplete engine with preloaded common CLI command specs.
    pub fn new() -> Self {
        let mut known_commands = HashMap::new();

        // git
        known_commands.insert(
            "git",
            CommandSpec {
                subcommands: vec![
                    ("status", "Show the working tree status"),
                    ("commit", "Record changes to the repository"),
                    ("push", "Update remote refs along with associated objects"),
                    ("pull", "Fetch from and integrate with another repository"),
                    ("checkout", "Switch branches or restore working tree files"),
                    ("branch", "List, create, or delete branches"),
                    (
                        "diff",
                        "Show changes between commits, commit and working tree",
                    ),
                    ("log", "Show commit logs"),
                    ("fetch", "Download objects and refs from another repository"),
                    ("rebase", "Reapply commits on top of another base tip"),
                    ("merge", "Join two or more development histories together"),
                    ("clone", "Clone a repository into a new directory"),
                    (
                        "stash",
                        "Stash the changes in a dirty working directory away",
                    ),
                ],
                flags: vec![
                    ("--help", "Display help information"),
                    ("--version", "Display version information"),
                    ("-v", "Verbose output"),
                    ("-m", "Commit message"),
                    ("-a", "All changes"),
                    ("-b", "Branch name"),
                ],
            },
        );

        // docker
        known_commands.insert(
            "docker",
            CommandSpec {
                subcommands: vec![
                    ("ps", "List containers"),
                    ("run", "Run a command in a new container"),
                    ("stop", "Stop one or more running containers"),
                    ("start", "Start one or more stopped containers"),
                    ("restart", "Restart one or more containers"),
                    ("logs", "Fetch the logs of a container"),
                    ("exec", "Run a command in a running container"),
                    ("build", "Build an image from a Dockerfile"),
                    ("pull", "Download an image from a registry"),
                    ("push", "Upload an image to a registry"),
                    ("images", "List images"),
                    ("compose", "Docker Compose multi-container tool"),
                    ("network", "Manage networks"),
                    ("volume", "Manage volumes"),
                ],
                flags: vec![
                    ("-d", "Run container in background (detached mode)"),
                    ("-it", "Allocate pseudo-TTY with stdin open"),
                    ("-p", "Publish a container's port(s) to the host"),
                    ("-v", "Bind mount a volume"),
                    ("--rm", "Automatically remove container when it exits"),
                    ("--name", "Assign a name to the container"),
                ],
            },
        );

        // cargo
        known_commands.insert(
            "cargo",
            CommandSpec {
                subcommands: vec![
                    ("build", "Compile the current package"),
                    (
                        "check",
                        "Analyze the current package without code generation",
                    ),
                    ("test", "Run the tests"),
                    ("run", "Run a binary or example of the local package"),
                    ("clippy", "Run linter checks"),
                    ("fmt", "Format Rust code according to style guidelines"),
                    ("update", "Update dependencies listed in Cargo.lock"),
                    ("add", "Add dependencies to a Cargo.toml manifest file"),
                    ("clean", "Remove generated target artifacts"),
                    (
                        "doc",
                        "Build this package's and its dependencies' documentation",
                    ),
                ],
                flags: vec![
                    (
                        "--release",
                        "Build artifacts in release mode, with optimizations",
                    ),
                    ("--workspace", "Build or test all packages in the workspace"),
                    ("--all-targets", "Check or test all targets"),
                    ("--offline", "Run without accessing the network"),
                    ("-p", "Package to build or test"),
                ],
            },
        );

        // systemctl
        known_commands.insert(
            "systemctl",
            CommandSpec {
                subcommands: vec![
                    ("status", "Show runtime status of one or more units"),
                    ("start", "Start (activate) one or more units"),
                    ("stop", "Stop (deactivate) one or more units"),
                    ("restart", "Start or restart one or more units"),
                    ("reload", "Reload one or more units"),
                    ("enable", "Enable one or more unit files"),
                    ("disable", "Disable one or more unit files"),
                    ("daemon-reload", "Reload systemd manager configuration"),
                ],
                flags: vec![
                    ("--user", "Connect to user service manager"),
                    ("-l", "Do not ellipsize output"),
                    ("--no-pager", "Do not pipe output into a pager"),
                ],
            },
        );

        // ssh
        known_commands.insert(
            "ssh",
            CommandSpec {
                subcommands: vec![],
                flags: vec![
                    ("-p", "Port to connect to on the remote host"),
                    ("-i", "Selects a file from which the identity (private key) is read"),
                    ("-J", "Connect to the target host by first making an SSH connection to jump host"),
                    ("-L", "Specifies that connections to a given TCP port on the local host are to be forwarded"),
                    ("-R", "Specifies that connections to a given TCP port on the remote host are to be forwarded"),
                    ("-D", "Specifies a local dynamic SOCKS port forwarding"),
                    ("-v", "Verbose mode"),
                    ("-N", "Do not execute a remote command (port forwarding only)"),
                ],
            },
        );

        // kubectl
        known_commands.insert(
            "kubectl",
            CommandSpec {
                subcommands: vec![
                    ("get", "Display one or many resources"),
                    (
                        "describe",
                        "Show details of a specific resource or group of resources",
                    ),
                    ("logs", "Print the logs for a container in a pod"),
                    (
                        "apply",
                        "Apply a configuration to a resource by file name or stdin",
                    ),
                    (
                        "delete",
                        "Delete resources by file names, stdin, resources and names",
                    ),
                    ("exec", "Execute a command in a container"),
                    ("port-forward", "Forward one or more local ports to a pod"),
                    ("cluster-info", "Display cluster info"),
                    ("config", "Modify kubeconfig files"),
                ],
                flags: vec![
                    ("-n", "Kubernetes namespace"),
                    ("-A", "All namespaces"),
                    ("-o", "Output format (yaml, json, wide)"),
                    (
                        "-f",
                        "Filename, directory, or URL to files used to view the configuration",
                    ),
                    ("--dry-run", "Must be 'none', 'server', or 'client'"),
                ],
            },
        );

        Self { known_commands }
    }

    /// Evaluates suggestions given the typing context (alias for `suggest`).
    pub fn complete(&self, ctx: &AutocompleteContext) -> Vec<CompletionItem> {
        self.suggest(ctx)
    }

    /// Evaluates suggestions given the typing context.
    pub fn suggest(&self, ctx: &AutocompleteContext) -> Vec<CompletionItem> {
        let prefix = if ctx.cursor_pos <= ctx.input_line.len() {
            &ctx.input_line[..ctx.cursor_pos]
        } else {
            &ctx.input_line
        };

        let trimmed_prefix = prefix.trim_start();
        if trimmed_prefix.is_empty() {
            return self.default_empty_suggestions(ctx);
        }

        let words: Vec<&str> = trimmed_prefix.split_whitespace().collect();
        let ends_with_space = prefix.ends_with(' ');

        let mut results = Vec::new();

        if words.len() == 1 && !ends_with_space {
            // Typing the first word: complete command names, snippets, history, and executable builtins
            let query = words[0];
            results.extend(self.match_command_names(query));
            results.extend(self.match_snippets(query, &ctx.snippet_candidates));
            results.extend(self.match_history(query, &ctx.history_candidates));
        } else {
            // Typing arguments or subcommands
            let leading_cmd = words[0];
            let active_word = if ends_with_space {
                ""
            } else {
                words.last().copied().unwrap_or("")
            };

            // 1. If leading command is known, check subcommands & flags
            if let Some(spec) = self.known_commands.get(leading_cmd) {
                // If on second word and not a flag, check subcommands
                if (words.len() == 2 && !ends_with_space) || (words.len() == 1 && ends_with_space) {
                    for (subcmd, desc) in &spec.subcommands {
                        if subcmd.starts_with(active_word) {
                            let score = if *subcmd == active_word { 100 } else { 80 };
                            results.push(CompletionItem {
                                value: (*subcmd).to_string(),
                                display_label: (*subcmd).to_string(),
                                description: Some((*desc).to_string()),
                                kind: CompletionKind::Subcommand,
                                score,
                            });
                        }
                    }
                }

                // If active word starts with '-', suggest flags
                if active_word.starts_with('-') || ends_with_space {
                    for (flag, desc) in &spec.flags {
                        if flag.starts_with(active_word) {
                            let score = if *flag == active_word { 95 } else { 75 };
                            results.push(CompletionItem {
                                value: (*flag).to_string(),
                                display_label: (*flag).to_string(),
                                description: Some((*desc).to_string()),
                                kind: CompletionKind::Flag,
                                score,
                            });
                        }
                    }
                }
            }

            // 2. If leading command is `ssh` or connection command, match host candidates
            if leading_cmd == "ssh" || leading_cmd == "connect" {
                results.extend(self.match_hosts(active_word, &ctx.host_candidates));
            }

            // 3. Match history matching the whole prefix
            for h in &ctx.history_candidates {
                if h.starts_with(trimmed_prefix) && h != trimmed_prefix {
                    results.push(CompletionItem {
                        value: h.clone(),
                        display_label: h.clone(),
                        description: Some("From history".to_string()),
                        kind: CompletionKind::History,
                        score: 70,
                    });
                }
            }

            // 4. File path heuristics
            if active_word.starts_with('/')
                || active_word.starts_with("./")
                || active_word.starts_with("../")
                || active_word.starts_with('~')
            {
                results.extend(self.suggest_paths(active_word));
            }
        }

        // Deduplicate by value and sort by score descending
        let mut seen = HashSet::new();
        results.retain(|item| seen.insert(item.value.clone()));
        results.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.value.cmp(&b.value)));
        results.truncate(15);
        results
    }

    fn default_empty_suggestions(&self, ctx: &AutocompleteContext) -> Vec<CompletionItem> {
        let mut out = Vec::new();
        // Offer up to 3 most recent history items
        for h in ctx.history_candidates.iter().rev().take(3) {
            out.push(CompletionItem {
                value: h.clone(),
                display_label: h.clone(),
                description: Some("Recent history".to_string()),
                kind: CompletionKind::History,
                score: 50,
            });
        }
        // Offer up to 3 snippets
        for (name, cmd) in ctx.snippet_candidates.iter().take(3) {
            out.push(CompletionItem {
                value: cmd.clone(),
                display_label: name.clone(),
                description: Some(cmd.clone()),
                kind: CompletionKind::Snippet,
                score: 45,
            });
        }
        out
    }

    fn match_command_names(&self, query: &str) -> Vec<CompletionItem> {
        let mut out = Vec::new();
        let query_lower = query.to_ascii_lowercase();

        let standard_builtins = [
            ("cd", "Change current directory"),
            ("ls", "List directory contents"),
            ("cat", "Concatenate and display files"),
            ("grep", "Search file(s) for lines matching a given pattern"),
            ("ssh", "OpenSSH remote login client"),
            ("sftp", "OpenSSH secure file transfer client"),
            ("scp", "OpenSSH secure copy"),
            ("tail", "Output the last part of files"),
            ("head", "Output the first part of files"),
            ("find", "Search for files in a directory hierarchy"),
            ("vim", "Vi IMproved text editor"),
            ("nano", "Nano text editor"),
            ("curl", "Transfer data from or to a server"),
            ("ping", "Send ICMP ECHO_REQUEST packets to network hosts"),
            ("top", "Display Linux processes"),
            ("htop", "Interactive process viewer"),
            ("df", "Report file system disk space usage"),
            ("du", "Estimate file space usage"),
        ];

        for (cmd, desc) in &standard_builtins {
            if cmd.starts_with(&query_lower) {
                let score = if *cmd == query_lower { 100 } else { 85 };
                out.push(CompletionItem {
                    value: (*cmd).to_string(),
                    display_label: (*cmd).to_string(),
                    description: Some((*desc).to_string()),
                    kind: CompletionKind::Command,
                    score,
                });
            }
        }

        for &cmd in self.known_commands.keys() {
            if cmd.starts_with(&query_lower) {
                let score = if cmd == query_lower { 100 } else { 90 };
                out.push(CompletionItem {
                    value: cmd.to_string(),
                    display_label: cmd.to_string(),
                    description: Some("CLI tool".to_string()),
                    kind: CompletionKind::Command,
                    score,
                });
            }
        }

        out
    }

    fn match_snippets(&self, query: &str, snippets: &[(String, String)]) -> Vec<CompletionItem> {
        let mut out = Vec::new();
        let query_lower = query.to_ascii_lowercase();
        for (name, cmd) in snippets {
            if name.to_ascii_lowercase().contains(&query_lower)
                || cmd.to_ascii_lowercase().contains(&query_lower)
            {
                out.push(CompletionItem {
                    value: cmd.clone(),
                    display_label: name.clone(),
                    description: Some(cmd.clone()),
                    kind: CompletionKind::Snippet,
                    score: 75,
                });
            }
        }
        out
    }

    fn match_hosts(&self, query: &str, hosts: &[String]) -> Vec<CompletionItem> {
        let mut out = Vec::new();
        let query_lower = query.to_ascii_lowercase();
        for h in hosts {
            if h.to_ascii_lowercase().contains(&query_lower) {
                let score = if h.starts_with(query) { 90 } else { 60 };
                out.push(CompletionItem {
                    value: h.clone(),
                    display_label: h.clone(),
                    description: Some("Saved host".to_string()),
                    kind: CompletionKind::Host,
                    score,
                });
            }
        }
        out
    }

    fn match_history(&self, query: &str, history: &[String]) -> Vec<CompletionItem> {
        let mut out = Vec::new();
        let query_lower = query.to_ascii_lowercase();
        for h in history {
            if h.to_ascii_lowercase().starts_with(&query_lower) {
                out.push(CompletionItem {
                    value: h.clone(),
                    display_label: h.clone(),
                    description: Some("Command history".to_string()),
                    kind: CompletionKind::History,
                    score: 80,
                });
            }
        }
        out
    }

    fn suggest_paths(&self, path_query: &str) -> Vec<CompletionItem> {
        let mut out = Vec::new();
        // Common standard UNIX directories when starting with '/'
        if path_query == "/" {
            for dir in &["/etc", "/var", "/usr", "/home", "/opt", "/tmp", "/var/log"] {
                out.push(CompletionItem {
                    value: (*dir).to_string(),
                    display_label: (*dir).to_string(),
                    description: Some("Directory".to_string()),
                    kind: CompletionKind::FilePath,
                    score: 65,
                });
            }
        } else if path_query.starts_with("/etc") {
            for sub in &["/etc/hosts", "/etc/resolv.conf", "/etc/ssh", "/etc/nginx"] {
                if sub.starts_with(path_query) {
                    out.push(CompletionItem {
                        value: (*sub).to_string(),
                        display_label: (*sub).to_string(),
                        description: Some("System path".to_string()),
                        kind: CompletionKind::FilePath,
                        score: 70,
                    });
                }
            }
        } else if path_query.starts_with("/var/log") {
            for sub in &["/var/log/syslog", "/var/log/auth.log", "/var/log/nginx"] {
                if sub.starts_with(path_query) {
                    out.push(CompletionItem {
                        value: (*sub).to_string(),
                        display_label: (*sub).to_string(),
                        description: Some("Log file".to_string()),
                        kind: CompletionKind::FilePath,
                        score: 70,
                    });
                }
            }
        }
        out
    }
}

/// Category of detected password prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasswordPromptKind {
    /// Sudo privilege elevation password prompt (e.g. `[sudo] password for user:`).
    Sudo,
    /// SSH remote server password prompt (e.g. `user@host's password:`).
    Ssh,
    /// SSH private key passphrase prompt (e.g. `Enter passphrase for key '...'`).
    KeyPassphrase,
    /// Multi-factor verification code (e.g. `Verification code:`).
    MfaCode,
    /// Generic password prompt (e.g. `Password:`).
    GenericPassword,
}

impl PasswordPromptKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sudo => "Sudo Password",
            Self::Ssh => "SSH Password",
            Self::KeyPassphrase => "Key Passphrase",
            Self::MfaCode => "MFA Verification Code",
            Self::GenericPassword => "Password",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Sudo => "🛡️",
            Self::Ssh => "🔒",
            Self::KeyPassphrase => "🔑",
            Self::MfaCode => "📱",
            Self::GenericPassword => "🔐",
        }
    }
}

/// Result of password prompt detection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordPromptMatch {
    pub kind: PasswordPromptKind,
    pub target_user: Option<String>,
    pub raw_prompt: String,
}

/// Safe password prompt detector that classifies terminal prompt strings without auto-injecting secrets.
pub struct PasswordPromptDetector;

impl PasswordPromptDetector {
    /// Evaluates the end of a line or the last non-empty line on the screen.
    pub fn detect(line: &str) -> Option<PasswordPromptMatch> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        // 1. Sudo prompt: "[sudo] password for user:" or "Password for user:"
        if trimmed.starts_with("[sudo] password for") || trimmed.starts_with("[sudo] Password for")
        {
            let user = trimmed
                .split_whitespace()
                .last()
                .map(|s| s.trim_end_matches(':').to_string());
            return Some(PasswordPromptMatch {
                kind: PasswordPromptKind::Sudo,
                target_user: user,
                raw_prompt: trimmed.to_string(),
            });
        }

        // 2. SSH password: "user@host's password:" or "root@10.0.0.1's password:"
        if (trimmed.ends_with("'s password:") || trimmed.ends_with("'s Password:"))
            && trimmed.contains('@')
        {
            let user = trimmed.split('@').next().map(|s| s.to_string());
            return Some(PasswordPromptMatch {
                kind: PasswordPromptKind::Ssh,
                target_user: user,
                raw_prompt: trimmed.to_string(),
            });
        }

        // 3. Key passphrase: "Enter passphrase for key '/home/...':"
        if trimmed.contains("passphrase for key") || trimmed.contains("Passphrase for key") {
            return Some(PasswordPromptMatch {
                kind: PasswordPromptKind::KeyPassphrase,
                target_user: None,
                raw_prompt: trimmed.to_string(),
            });
        }

        // 4. MFA code: "Verification code:" or "One-time password:" or prompts mentioning MFA/2FA/TOTP
        let lower = trimmed.to_lowercase();
        if lower.contains("verification code")
            || lower.contains("one-time password")
            || lower.contains("2fa")
            || lower.contains("mfa")
            || lower.contains("totp")
        {
            return Some(PasswordPromptMatch {
                kind: PasswordPromptKind::MfaCode,
                target_user: None,
                raw_prompt: trimmed.to_string(),
            });
        }

        // 5. Generic password: "Password:" or "password:"
        if trimmed == "Password:"
            || trimmed == "password:"
            || trimmed.ends_with("Password:")
            || trimmed.ends_with("password:")
        {
            return Some(PasswordPromptMatch {
                kind: PasswordPromptKind::GenericPassword,
                target_user: None,
                raw_prompt: trimmed.to_string(),
            });
        }

        None
    }
}

/// Generates reversible shell integration hook scripts for Bash.
pub fn generate_bash_hook() -> &'static str {
    r#"# Shellumi Semantic Shell Integration for Bash
# Emits OSC 133 semantic prompt demarcation and OSC 7 CWD reporting
__shellumi_prompt_start() {
    printf "\033]133;A\007\033]7;file://%s%s\007" "$HOSTNAME" "$PWD"
}
__shellumi_preexec() {
    printf "\033]133;C\007"
}
PROMPT_COMMAND="__shellumi_prompt_start; ${PROMPT_COMMAND:-}"
PS1="\[\033]133;B\007\]${PS1:-}"
"#
}

/// Generates reversible shell integration hook scripts for Zsh.
pub fn generate_zsh_hook() -> &'static str {
    r#"# Shellumi Semantic Shell Integration for Zsh
# Emits OSC 133 semantic prompt demarcation and OSC 7 CWD reporting
autoload -Uz add-zsh-hook
__shellumi_precmd() {
    local exit_code=$?
    printf "\033]133;D;%d\007" "$exit_code"
    printf "\033]133;A\007\033]7;file://%s%s\007" "$HOST" "$PWD"
}
__shellumi_preexec() {
    printf "\033]133;C\007\033]133;P;cl=%s\007" "$1"
}
add-zsh-hook precmd __shellumi_precmd
add-zsh-hook preexec __shellumi_preexec
PS1="%{\033]133;B\007%}${PS1:-}"
"#
}

/// Generates reversible shell integration hook scripts for Fish.
pub fn generate_fish_hook() -> &'static str {
    r#"# Shellumi Semantic Shell Integration for Fish
function __shellumi_precmd --on-event fish_prompt
    set -l exit_code $status
    printf "\033]133;D;%d\007" $exit_code
    printf "\033]133;A\007\033]7;file://%s%s\007" (hostname) $PWD
end
function __shellumi_preexec --on-event fish_preexec
    printf "\033]133;C\007\033]133;P;cl=%s\007" $argv[1]
end
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_first_word_command_names() {
        let engine = AutocompleteEngine::new();
        let ctx = AutocompleteContext {
            input_line: "gi".to_string(),
            cursor_pos: 2,
            ..Default::default()
        };

        let suggestions = engine.suggest(&ctx);
        assert!(!suggestions.is_empty());
        let git_item = suggestions.iter().find(|i| i.value == "git");
        assert!(git_item.is_some());
        assert_eq!(git_item.unwrap().kind, CompletionKind::Command);
    }

    #[test]
    fn completes_subcommands_for_known_tools() {
        let engine = AutocompleteEngine::new();
        let ctx = AutocompleteContext {
            input_line: "git st".to_string(),
            cursor_pos: 6,
            ..Default::default()
        };

        let suggestions = engine.suggest(&ctx);
        let status_item = suggestions.iter().find(|i| i.value == "status");
        assert!(status_item.is_some());
        assert_eq!(status_item.unwrap().kind, CompletionKind::Subcommand);

        let stash_item = suggestions.iter().find(|i| i.value == "stash");
        assert!(stash_item.is_some());
    }

    #[test]
    fn completes_flags_for_docker() {
        let engine = AutocompleteEngine::new();
        let ctx = AutocompleteContext {
            input_line: "docker run -".to_string(),
            cursor_pos: 12,
            ..Default::default()
        };

        let suggestions = engine.suggest(&ctx);
        let rm_flag = suggestions.iter().find(|i| i.value == "--rm");
        assert!(rm_flag.is_some());
        assert_eq!(rm_flag.unwrap().kind, CompletionKind::Flag);
    }

    #[test]
    fn completes_hosts_for_ssh() {
        let engine = AutocompleteEngine::new();
        let ctx = AutocompleteContext {
            input_line: "ssh ubun".to_string(),
            cursor_pos: 8,
            host_candidates: vec!["ubuntu-prod".to_string(), "debian-dev".to_string()],
            ..Default::default()
        };

        let suggestions = engine.suggest(&ctx);
        let host_item = suggestions.iter().find(|i| i.value == "ubuntu-prod");
        assert!(host_item.is_some());
        assert_eq!(host_item.unwrap().kind, CompletionKind::Host);
    }

    #[test]
    fn detects_various_password_prompts() {
        // Sudo prompt
        let sudo_match = PasswordPromptDetector::detect("[sudo] password for admin:");
        assert!(sudo_match.is_some());
        let m = sudo_match.unwrap();
        assert_eq!(m.kind, PasswordPromptKind::Sudo);
        assert_eq!(m.target_user.as_deref(), Some("admin"));

        // SSH prompt
        let ssh_match = PasswordPromptDetector::detect("deploy@192.168.1.50's password: ");
        assert!(ssh_match.is_some());
        let m = ssh_match.unwrap();
        assert_eq!(m.kind, PasswordPromptKind::Ssh);
        assert_eq!(m.target_user.as_deref(), Some("deploy"));

        // Key passphrase
        let key_match = PasswordPromptDetector::detect(
            "Enter passphrase for key '/Users/mjk/.ssh/id_ed25519':",
        );
        assert!(key_match.is_some());
        assert_eq!(key_match.unwrap().kind, PasswordPromptKind::KeyPassphrase);

        // Verification code
        let mfa_match = PasswordPromptDetector::detect("Verification code:");
        assert!(mfa_match.is_some());
        assert_eq!(mfa_match.unwrap().kind, PasswordPromptKind::MfaCode);

        // Generic password
        let gen_match = PasswordPromptDetector::detect("Password:");
        assert!(gen_match.is_some());
        assert_eq!(gen_match.unwrap().kind, PasswordPromptKind::GenericPassword);

        // Regular prompt should NOT match
        assert!(PasswordPromptDetector::detect("dev@ubuntu-server:~$ ls -la").is_none());
    }

    #[test]
    fn generates_shell_hooks() {
        let bash = generate_bash_hook();
        assert!(bash.contains("133;A"));
        assert!(bash.contains("PROMPT_COMMAND"));

        let zsh = generate_zsh_hook();
        assert!(zsh.contains("add-zsh-hook precmd"));
        assert!(zsh.contains("133;D"));

        let fish = generate_fish_hook();
        assert!(fish.contains("on-event fish_prompt"));
    }
}

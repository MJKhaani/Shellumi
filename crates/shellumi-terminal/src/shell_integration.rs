//! Semantic Shell Integration and Prompt Demarcation (OSC 133, OSC 7, and OSC 9;9).
//!
//! Implements protocol standards for modern intelligent terminal productivity:
//! - **OSC 133 (FinalTerm / iTerm2 / VS Code / Kitty Semantic Prompt Protocol)**:
//!   - `133;A`: Prompt Start (shell is displaying a new prompt)
//!   - `133;B`: Command Start / Prompt End (prompt rendered; ready for command entry)
//!   - `133;C`: Command Executed (user pressed Return; command execution underway)
//!   - `133;D[;<exit_code>]`: Command Finished with exit code
//!   - `133;P;k=v`: Property setting (e.g. `cl=<command>` for command line capturing)
//! - **OSC 7**: Current Working Directory reporting (`file://[hostname]/path`).
//! - **OSC 9;9**: ConEmu / Windows Terminal style CWD reporting.

/// Semantic prompt demarcation marks according to the FinalTerm / OSC 133 spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptSemanticMark {
    /// OSC 133;A: Shell prompt begins rendering.
    PromptStart,
    /// OSC 133;B: Shell prompt has finished rendering; command line input begins.
    CommandStart,
    /// OSC 133;C: User submitted the command line; execution has started.
    CommandExecuted,
    /// OSC 133;D: Command execution completed with optional integer exit code.
    CommandFinished { exit_code: Option<i32> },
}

impl PromptSemanticMark {
    pub fn is_prompt(&self) -> bool {
        matches!(self, Self::PromptStart | Self::CommandStart)
    }

    pub fn is_executing(&self) -> bool {
        matches!(self, Self::CommandExecuted)
    }
}

/// State tracking semantic shell integration, active CWD, and command history.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShellIntegrationState {
    /// Active current working directory reported by the shell (via OSC 7 or OSC 9;9).
    pub current_dir: Option<String>,
    /// Exit code of the most recently finished command (via OSC 133;D).
    pub last_exit_code: Option<i32>,
    /// Current prompt demarcation mark.
    pub current_mark: Option<PromptSemanticMark>,
    /// Command line captured via OSC 133;P or inferred at command execution.
    pub current_command_line: Option<String>,
    /// Recent command history captured via semantic shell integration marks.
    pub command_history: Vec<String>,
}

impl ShellIntegrationState {
    /// Creates an empty shell integration state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles OSC 133 semantic prompt sequences.
    ///
    /// Parameters:
    /// - `params`: Sub-parameters following the `133` identifier (e.g. `["A"]`, `["D", "0"]`, `["P", "cl=git status"]`).
    pub fn handle_osc_133(&mut self, params: &[&[u8]]) {
        if params.is_empty() {
            return;
        }

        let action = match std::str::from_utf8(params[0]) {
            Ok(s) => s.trim(),
            Err(_) => return,
        };

        match action {
            "A" => {
                // Prompt Start
                self.current_mark = Some(PromptSemanticMark::PromptStart);
            }
            "B" => {
                // Command Start (Prompt End)
                self.current_mark = Some(PromptSemanticMark::CommandStart);
            }
            "C" => {
                // Command Executed
                self.current_mark = Some(PromptSemanticMark::CommandExecuted);
                if let Some(cmd) = self.current_command_line.take() {
                    let trimmed = cmd.trim();
                    if !trimmed.is_empty()
                        && self.command_history.last().map(|s| s.as_str()) != Some(trimmed)
                    {
                        self.command_history.push(trimmed.to_string());
                        if self.command_history.len() > 1000 {
                            self.command_history.remove(0);
                        }
                    }
                }
            }
            "D" => {
                // Command Finished with optional exit code (param 1)
                let exit_code = if params.len() > 1 {
                    std::str::from_utf8(params[1])
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok())
                } else {
                    None
                };

                self.last_exit_code = exit_code;
                self.current_mark = Some(PromptSemanticMark::CommandFinished { exit_code });
            }
            "P" if params.len() > 1 => {
                // Property setter: e.g. "cl=git commit -m \"foo\""
                if let Ok(prop_str) = std::str::from_utf8(params[1]) {
                    if let Some((k, v)) = prop_str.split_once('=') {
                        if k.trim() == "cl" {
                            self.current_command_line = Some(v.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Handles OSC 7 Current Working Directory sequences (`7;file://[hostname]/path`).
    pub fn handle_osc_7(&mut self, url_bytes: &[u8]) {
        if let Ok(raw_url) = std::str::from_utf8(url_bytes) {
            if let Some(path) = parse_osc7_path(raw_url) {
                self.current_dir = Some(path);
            }
        }
    }

    /// Handles OSC 9;9 Current Working Directory sequences (`9;9;"path"`).
    pub fn handle_osc_9_9(&mut self, path_bytes: &[u8]) {
        if let Ok(raw_path) = std::str::from_utf8(path_bytes) {
            let clean = raw_path.trim().trim_matches('"');
            if !clean.is_empty() {
                self.current_dir = Some(clean.to_string());
            }
        }
    }

    /// Returns the active current working directory, if known.
    pub fn current_dir(&self) -> Option<&str> {
        self.current_dir.as_deref()
    }

    /// Returns the last command exit code, if known.
    pub fn last_exit_code(&self) -> Option<i32> {
        self.last_exit_code
    }

    /// Returns whether the shell is currently rendering or waiting at a prompt.
    pub fn is_at_prompt(&self) -> bool {
        self.current_mark
            .as_ref()
            .map(|m| m.is_prompt())
            .unwrap_or(false)
    }

    /// Returns whether a command is currently actively executing.
    pub fn is_executing(&self) -> bool {
        self.current_mark
            .as_ref()
            .map(|m| m.is_executing())
            .unwrap_or(false)
    }
}

/// Parses a file URL formatted for OSC 7 into a normalized POSIX or Windows file path.
/// Examples:
/// - `file://hostname/Users/mjk/projects` -> `/Users/mjk/projects`
/// - `file:///home/ubuntu/repo` -> `/home/ubuntu/repo`
/// - `file://localhost/C:/Users/dev` -> `C:/Users/dev`
pub fn parse_osc7_path(raw_url: &str) -> Option<String> {
    let clean = raw_url.trim().trim_matches('"');
    if !clean.starts_with("file://") {
        return None;
    }

    let without_scheme = &clean["file://".len()..];
    // Find the slash following the optional hostname
    if let Some(slash_idx) = without_scheme.find('/') {
        let path_part = &without_scheme[slash_idx..];
        // Percent-decode standard URL encoding
        let decoded = url_percent_decode(path_part);
        // On Windows if formatted like /C:/..., strip leading slash
        #[cfg(windows)]
        {
            if decoded.len() >= 3 && decoded.starts_with('/') && decoded.chars().nth(2) == Some(':')
            {
                return Some(decoded[1..].to_string());
            }
        }
        Some(decoded)
    } else {
        None
    }
}

/// Simple percent-decoding helper for file URLs without third-party dependencies.
fn url_percent_decode(input: &str) -> String {
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(val);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_osc7_file_urls_correctly() {
        assert_eq!(
            parse_osc7_path("file://my-host/home/dev/projects").as_deref(),
            Some("/home/dev/projects")
        );
        assert_eq!(
            parse_osc7_path("file:///var/log/syslog").as_deref(),
            Some("/var/log/syslog")
        );
        assert_eq!(
            parse_osc7_path("file://ubuntu-server/home/dev/My%20Documents").as_deref(),
            Some("/home/dev/My Documents")
        );
        assert_eq!(parse_osc7_path("http://example.org/path"), None);
    }

    #[test]
    fn tracks_osc_133_prompt_and_command_lifecycle() {
        let mut state = ShellIntegrationState::new();
        assert!(!state.is_at_prompt());
        assert!(!state.is_executing());

        // 1. Prompt Start (133;A)
        state.handle_osc_133(&[b"A"]);
        assert_eq!(state.current_mark, Some(PromptSemanticMark::PromptStart));
        assert!(state.is_at_prompt());
        assert!(!state.is_executing());

        // 2. Command Start (133;B)
        state.handle_osc_133(&[b"B"]);
        assert_eq!(state.current_mark, Some(PromptSemanticMark::CommandStart));
        assert!(state.is_at_prompt());

        // 3. Command property set (133;P;cl=git status)
        state.handle_osc_133(&[b"P", b"cl=git status"]);
        assert_eq!(state.current_command_line.as_deref(), Some("git status"));

        // 4. Command Executed (133;C)
        state.handle_osc_133(&[b"C"]);
        assert_eq!(
            state.current_mark,
            Some(PromptSemanticMark::CommandExecuted)
        );
        assert!(!state.is_at_prompt());
        assert!(state.is_executing());
        assert_eq!(state.command_history, vec!["git status".to_string()]);

        // 5. Command Finished with Exit Code 0 (133;D;0)
        state.handle_osc_133(&[b"D", b"0"]);
        assert_eq!(
            state.current_mark,
            Some(PromptSemanticMark::CommandFinished { exit_code: Some(0) })
        );
        assert_eq!(state.last_exit_code, Some(0));
        assert!(!state.is_executing());

        // 6. Command Finished with Exit Code 127
        state.handle_osc_133(&[b"D", b"127"]);
        assert_eq!(state.last_exit_code, Some(127));
    }

    #[test]
    fn tracks_cwd_via_osc7_and_osc9_9() {
        let mut state = ShellIntegrationState::new();

        // OSC 7
        state.handle_osc_7(b"file://server-1/etc/nginx");
        assert_eq!(state.current_dir.as_deref(), Some("/etc/nginx"));

        // OSC 9;9
        state.handle_osc_9_9(b"\"/opt/app\"");
        assert_eq!(state.current_dir.as_deref(), Some("/opt/app"));
    }
}

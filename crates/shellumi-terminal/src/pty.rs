//! PTY process spawning and lifecycle management.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtyPair, PtySize};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Configuration options for spawning a local terminal session.
#[derive(Clone, Debug)]
pub struct TerminalCommand {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
}

impl Default for TerminalCommand {
    fn default() -> Self {
        Self::default_shell()
    }
}

impl TerminalCommand {
    /// Detects the user's default system shell.
    pub fn default_shell() -> Self {
        #[cfg(windows)]
        let program = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());

        #[cfg(not(windows))]
        let program = std::env::var("SHELL").unwrap_or_else(|_| {
            if Path::new("/bin/zsh").exists() {
                "/bin/zsh".to_string()
            } else {
                "/bin/bash".to_string()
            }
        });

        Self {
            program,
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
        }
    }

    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for a in args {
            self.args.push(a.into());
        }
        self
    }

    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

/// Resolves the executable path across platforms, appending `.exe` and checking PATH and standard system locations on Windows.
pub fn resolve_command_program(program: &str) -> String {
    #[cfg(windows)]
    {
        let path = Path::new(program);
        if path.is_file() {
            return program.to_string();
        }

        let with_exe = if path.extension().is_none() {
            format!("{program}.exe")
        } else {
            program.to_string()
        };

        if Path::new(&with_exe).is_file() {
            return with_exe;
        }

        if let Ok(resolved) = which::which(program) {
            return resolved.to_string_lossy().to_string();
        }
        if let Ok(resolved) = which::which(&with_exe) {
            return resolved.to_string_lossy().to_string();
        }

        let sysroot = std::env::var("SystemRoot")
            .or_else(|_| std::env::var("windir"))
            .unwrap_or_else(|_| "C:\\Windows".to_string());

        let sys32_openssh = Path::new(&sysroot)
            .join("System32")
            .join("OpenSSH")
            .join(&with_exe);
        if sys32_openssh.is_file() {
            return sys32_openssh.to_string_lossy().to_string();
        }

        let sys32 = Path::new(&sysroot).join("System32").join(&with_exe);
        if sys32.is_file() {
            return sys32.to_string_lossy().to_string();
        }

        let prog_files =
            std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
        let git_ssh = Path::new(&prog_files)
            .join("Git")
            .join("usr")
            .join("bin")
            .join(&with_exe);
        if git_ssh.is_file() {
            return git_ssh.to_string_lossy().to_string();
        }

        with_exe
    }
    #[cfg(not(windows))]
    {
        program.to_string()
    }
}

/// A running pseudo-terminal session.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
}

impl PtySession {
    /// Spawns a child process in a newly allocated pseudo-terminal.
    pub fn spawn(cmd: &TerminalCommand, rows: u16, cols: u16) -> Result<Self, io::Error> {
        tracing::debug!(
            program = %cmd.program,
            rows,
            cols,
            "Spawning PTY process"
        );
        let pty_system = native_pty_system();
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let PtyPair { master, slave } = pty_system.openpty(size).map_err(|e| {
            tracing::error!(error = %e, "Failed to open PTY pair");
            io::Error::other(format!("failed to open PTY: {e}"))
        })?;

        let program_path = resolve_command_program(&cmd.program);
        let mut builder = CommandBuilder::new(program_path);
        builder.args(&cmd.args);
        if let Some(ref cwd) = cmd.cwd {
            builder.cwd(cwd);
        }
        for (k, v) in &cmd.env {
            builder.env(k, v);
        }

        // Set standard terminal environment variables
        builder.env("TERM", "xterm-256color");
        builder.env("COLORTERM", "truecolor");

        let child = slave.spawn_command(builder).map_err(|e| {
            tracing::error!(
                program = %cmd.program,
                error = %e,
                "Failed to spawn PTY child"
            );
            io::Error::other(format!("failed to spawn PTY child: {e}"))
        })?;

        let writer = master.take_writer().map_err(|e| {
            tracing::error!(error = %e, "Failed to take PTY writer");
            io::Error::other(format!("failed to take PTY writer: {e}"))
        })?;

        tracing::info!(program = %cmd.program, "PTY session spawned successfully");

        Ok(Self {
            master,
            child,
            writer,
        })
    }

    /// Obtains a reader for incoming bytes from the child process.
    pub fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, io::Error> {
        self.master
            .try_clone_reader()
            .map_err(|e| io::Error::other(format!("failed to clone PTY reader: {e}")))
    }

    /// Sends input bytes (e.g. keystrokes, escape sequences) to the child process.
    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), io::Error> {
        self.writer.write_all(buf)?;
        self.writer.flush()
    }

    /// Resizes the PTY window geometry.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), io::Error> {
        tracing::trace!(rows, cols, "Resizing PTY window");
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| {
                tracing::warn!(rows, cols, error = %e, "Failed to resize PTY");
                io::Error::other(format!("failed to resize PTY: {e}"))
            })
    }

    /// Checks if the child process has exited.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Terminates the child process.
    pub fn kill(&mut self) -> Result<(), io::Error> {
        self.child
            .kill()
            .map_err(|e| io::Error::other(format!("failed to kill child: {e}")))
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_command_program() {
        let cmd = resolve_command_program("echo");
        assert!(!cmd.is_empty());

        let ssh = resolve_command_program("ssh");
        assert!(!ssh.is_empty());
    }
}

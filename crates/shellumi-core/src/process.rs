//! Platform-aware background subprocess builder.
//!
//! Non-interactive helper subprocesses (credential store helpers, diagnostics,
//! background SSH batch/SFTP/OAuth-browser launchers) must not flash a console
//! window on Windows when spawned from the GUI (`windows_subsystem = "windows"`).
//! Interactive PTY/ConPTY sessions in `shellumi-terminal` intentionally do NOT
//! use this helper.

use std::ffi::OsStr;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Win32 `CREATE_NO_WINDOW` process creation flag.
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a `std::process::Command` configured for non-interactive background
/// execution without spawning a visible console window on Windows.
pub fn background_command<S: AsRef<OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_command_preserves_program_and_arguments() {
        let mut cmd = background_command("git");
        cmd.arg("--version");
        assert_eq!(cmd.get_program(), "git");
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, vec!["--version"]);
    }
}

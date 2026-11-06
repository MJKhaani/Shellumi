//! OpenSSH-only credential helper. It never examines terminal output or confirms trust.
use shellumi_core::connection::{askpass_reference, ConnectionPlan};
use shellumi_terminal::TerminalCommand;

pub(super) fn environment(plan: &ConnectionPlan) -> Result<Vec<(String, String)>, String> {
    if !plan.uses_askpass() {
        return Ok(Vec::new());
    }
    let executable = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .into_os_string()
        .into_string()
        .map_err(|_| "The askpass executable path is not UTF-8.")?;
    let mut env = vec![
        ("SSH_ASKPASS".into(), executable),
        ("SSH_ASKPASS_REQUIRE".into(), "force".into()),
        ("DISPLAY".into(), "shellumi:0".into()),
        ("SHELLUMI_ASKPASS_MODE".into(), "1".into()),
        (
            "SHELLUMI_ASKPASS_TARGET".into(),
            format!(
                "{}@{}",
                plan.destination.username(),
                plan.destination.host()
            ),
        ),
    ];
    for (key, value) in [
        ("SHELLUMI_ASKPASS_PASSWORD_REF", plan.password_ref.as_ref()),
        (
            "SHELLUMI_ASKPASS_PASSPHRASE_REF",
            plan.passphrase_ref.as_ref(),
        ),
        ("SHELLUMI_ASKPASS_KEY", plan.identity_file.as_ref()),
    ] {
        env.push((key.into(), value.cloned().unwrap_or_default()));
    }
    // Clear the confirmation flag: only OpenSSH's own invocation may set it.
    env.push(("SSH_ASKPASS_PROMPT".into(), String::new()));
    Ok(env)
}
pub(super) fn command(
    program: &str,
    arguments: Vec<String>,
    plan: &ConnectionPlan,
) -> Result<TerminalCommand, String> {
    let mut command = TerminalCommand::new(program).args(arguments);
    for (key, value) in environment(plan)? {
        command = command.env(key, value);
    }
    Ok(command)
}
pub(super) fn run() -> i32 {
    fn invoke() -> Result<(), String> {
        let prompt = std::env::args().nth(1).ok_or("Missing askpass prompt.")?;
        let env = |name| std::env::var(name).ok().filter(|value| !value.is_empty());
        let target = env("SHELLUMI_ASKPASS_TARGET").ok_or("Missing authentication target.")?;
        let key = env("SHELLUMI_ASKPASS_KEY");
        let password = env("SHELLUMI_ASKPASS_PASSWORD_REF");
        let passphrase = env("SHELLUMI_ASKPASS_PASSPHRASE_REF");
        let kind = env("SSH_ASKPASS_PROMPT");
        let reference = askpass_reference(
            &prompt,
            kind.as_deref(),
            &target,
            key.as_deref(),
            password.as_deref(),
            passphrase.as_deref(),
        )
        .ok_or("Unsupported authentication prompt.")?;
        let store = shellumi_core::default_credential_store();
        let secret = shellumi_core::resolve_secret_from_ref(&*store, reference)
            .map_err(|_| "Could not resolve credential.")?
            .ok_or("Credential is unavailable.")?;
        if secret.expose_secret().contains(['\r', '\n', '\0']) {
            return Err("Unsupported credential format.".into());
        }
        use std::io::Write;
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(secret.expose_secret().as_bytes())
            .map_err(|_| "Could not supply credential.")?;
        stdout
            .write_all(b"\n")
            .map_err(|_| "Could not supply credential.")?;
        Ok(())
    }
    if invoke().is_ok() {
        0
    } else {
        1
    }
}

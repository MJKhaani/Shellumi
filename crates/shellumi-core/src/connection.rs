//! UI-independent identity selection and OpenSSH launch policy.
use crate::{AuthMethod, Identity, SshDestination};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionPlan {
    pub destination: SshDestination,
    pub ssh_arguments: Vec<String>,
    pub sftp_arguments: Vec<String>,
    pub jump_host: Option<String>,
    pub identity_file: Option<String>,
    pub certificate_file: Option<String>,
    pub authentication_options: Vec<(String, String)>,
    pub password_ref: Option<String>,
    pub passphrase_ref: Option<String>,
}
impl ConnectionPlan {
    pub fn new(
        destination: SshDestination,
        identity: Option<&Identity>,
        jump: Option<&str>,
    ) -> Result<Self, String> {
        let mut plan = Self {
            destination,
            ssh_arguments: Vec::new(),
            sftp_arguments: Vec::new(),
            jump_host: jump
                .map(crate::validate_jump_host)
                .transpose()
                .map_err(|e| e.to_string())?,
            identity_file: None,
            certificate_file: None,
            authentication_options: Vec::new(),
            password_ref: None,
            passphrase_ref: None,
        };
        if let Some(identity) = identity {
            identity.validate().map_err(|e| e.to_string())?;
            match &identity.auth_method {
                AuthMethod::PublicKey { key_name } => {
                    plan.identity_file = Some(
                        identity
                            .key_path
                            .clone()
                            .or_else(|| {
                                (key_name.contains('/') || key_name.contains('\\'))
                                    .then(|| key_name.clone())
                            })
                            .ok_or("The selected public-key identity needs a key file path.")?,
                    );
                    plan.passphrase_ref = identity.passphrase_secret_ref.clone();
                    plan.authentication_options.extend([
                        ("IdentitiesOnly".into(), "yes".into()),
                        ("PreferredAuthentications".into(), "publickey".into()),
                    ]);
                }
                AuthMethod::Certificate {
                    cert_path,
                    key_path,
                } => {
                    plan.identity_file = Some(
                        identity
                            .key_path
                            .clone()
                            .or_else(|| key_path.clone())
                            .ok_or(
                                "The selected certificate identity needs its private key path.",
                            )?,
                    );
                    plan.certificate_file = Some(
                        identity
                            .certificate_path
                            .clone()
                            .unwrap_or_else(|| cert_path.clone()),
                    );
                    plan.passphrase_ref = identity.passphrase_secret_ref.clone();
                    plan.authentication_options.extend([
                        ("IdentitiesOnly".into(), "yes".into()),
                        ("PreferredAuthentications".into(), "publickey".into()),
                    ]);
                }
                AuthMethod::Password => {
                    plan.password_ref = identity.password_secret_ref.clone();
                    plan.authentication_options
                        .push(("PreferredAuthentications".into(), "password".into()));
                }
                AuthMethod::Agent => {
                    plan.authentication_options.extend([
                        ("PreferredAuthentications".into(), "publickey".into()),
                        ("IdentitiesOnly".into(), "no".into()),
                        ("IdentityFile".into(), "none".into()),
                    ]);
                }
                AuthMethod::Interactive => {
                    plan.authentication_options.push((
                        "PreferredAuthentications".into(),
                        "keyboard-interactive,password".into(),
                    ));
                }
            }
        }
        for path in [plan.identity_file.as_ref(), plan.certificate_file.as_ref()]
            .into_iter()
            .flatten()
        {
            if path.trim().is_empty() || path.chars().any(char::is_control) {
                return Err("Invalid SSH identity file path.".into());
            }
        }
        for reference in [plan.password_ref.as_ref(), plan.passphrase_ref.as_ref()]
            .into_iter()
            .flatten()
        {
            if reference.starts_with("keychain://") {
                crate::parse_keychain_ref(reference).map_err(|e| e.to_string())?;
            } else if let Some(name) = reference.strip_prefix("env://") {
                if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    return Err("Invalid credential environment reference.".into());
                }
            } else {
                return Err("Credentials must use keychain:// or env:// references.".into());
            }
        }
        let mut base = plan
            .destination
            .openssh_arguments_with_jump(jump)
            .map_err(|e| e.to_string())?;
        if plan.uses_askpass() {
            for arg in &mut base {
                if arg == "StrictHostKeyChecking=ask" {
                    *arg = "StrictHostKeyChecking=yes".into();
                }
            }
        }
        // Identity options precede the destination and all paths are separate argv values.
        let mut options = Vec::new();
        if let Some(path) = &plan.identity_file {
            options.extend(["-i".into(), path.clone()]);
        }
        if let Some(path) = &plan.certificate_file {
            options.extend(["-o".into(), certificate_option(path)]);
        }
        for (key, value) in &plan.authentication_options {
            options.extend(["-o".into(), format!("{key}={value}")]);
        }
        let target = base.pop().ok_or("Missing SSH destination.")?;
        if base.last().is_some_and(|arg| arg == "--") {
            base.pop();
        }
        base.extend(options.clone());
        base.extend(["--".into(), target]);
        plan.ssh_arguments = base;
        let strict = if plan.uses_askpass() { "yes" } else { "ask" };
        let mut sftp = vec![
            "-o".into(),
            format!("StrictHostKeyChecking={strict}"),
            "-o".into(),
            "ForwardAgent=no".into(),
            "-P".into(),
            plan.destination.port().to_string(),
        ];
        if let Some(jump) = jump {
            sftp.extend([
                "-J".into(),
                crate::validate_jump_host(jump).map_err(|e| e.to_string())?,
            ]);
        }
        sftp.extend(options);
        let host = plan.destination.host();
        let host = if host.parse::<std::net::Ipv6Addr>().is_ok() {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        sftp.extend([
            "--".into(),
            format!("{}@{host}", plan.destination.username()),
        ]);
        plan.sftp_arguments = sftp;
        Ok(plan)
    }
    pub fn uses_askpass(&self) -> bool {
        self.password_ref.is_some() || self.passphrase_ref.is_some()
    }
}

/// Quote a path for OpenSSH's own `-o` configuration parser (not a shell).
pub fn certificate_option(path: &str) -> String {
    format!(
        "CertificateFile=\"{}\"",
        path.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

/// Authorize only a client-generated password/key-passphrase prompt. Host trust,
/// confirmation, keyboard-interactive challenges and arbitrary remote prompts fail closed.
pub fn askpass_reference<'a>(
    prompt: &str,
    kind: Option<&str>,
    target: &str,
    key: Option<&str>,
    password_ref: Option<&'a str>,
    passphrase_ref: Option<&'a str>,
) -> Option<&'a str> {
    if kind.is_some() {
        return None;
    }
    let prompt = prompt.trim_end();
    if prompt == format!("{target}'s password:") {
        return password_ref;
    }
    if let Some(key) = key {
        if prompt == format!("Enter passphrase for key '{key}':") {
            return passphrase_ref;
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    fn destination() -> SshDestination {
        SshDestination::new("2001:db8::1", "deploy", 2222).unwrap()
    }
    #[test]
    fn explicit_keys_and_certificates_are_separate_arguments_and_keep_verification() {
        let mut identity = crate::NewIdentity::new(
            "Certificate",
            "deploy",
            AuthMethod::Certificate {
                cert_path: "/keys/cert.pub".into(),
                key_path: Some("/keys/key with spaces".into()),
            },
        )
        .unwrap();
        identity.passphrase_secret_ref = Some("keychain://shellumi/key".into());
        let saved = Identity {
            id: 1,
            name: identity.name,
            username: identity.username,
            auth_method: identity.auth_method,
            password_secret_ref: None,
            key_path: None,
            passphrase_secret_ref: identity.passphrase_secret_ref,
            certificate_path: None,
            tags: vec![],
            comment: None,
            created_at: None,
            updated_at: None,
        };
        let plan = ConnectionPlan::new(destination(), Some(&saved), Some("jump.example")).unwrap();
        assert!(plan.ssh_arguments.contains(&"/keys/key with spaces".into()));
        assert!(plan
            .ssh_arguments
            .contains(&certificate_option("/keys/cert.pub")));
        assert!(plan
            .ssh_arguments
            .contains(&"StrictHostKeyChecking=yes".into()));
        assert!(plan.ssh_arguments.contains(&"ForwardAgent=no".into()));
        assert!(plan.sftp_arguments.contains(&"-P".into()));
        assert!(!plan.sftp_arguments.contains(&"-p".into()));
        assert!(!plan
            .ssh_arguments
            .iter()
            .any(|arg| arg.contains("keychain://")));
    }
    #[test]
    fn default_connections_and_authentication_options_precede_destination() {
        let plan = ConnectionPlan::new(destination(), None, None).unwrap();
        assert_eq!(plan.ssh_arguments, destination().openssh_arguments());
        assert!(!plan.uses_askpass());
        let separator = plan
            .sftp_arguments
            .iter()
            .position(|arg| arg == "--")
            .unwrap();
        assert_eq!(separator, plan.sftp_arguments.len() - 2);
        assert_eq!(plan.sftp_arguments.last().unwrap(), "deploy@[2001:db8::1]");
        assert!(!plan.sftp_arguments.contains(&"-l".into()));
        assert!(
            ConnectionPlan::new(destination(), None, Some("jump; touch /tmp/injected")).is_err()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn certificate_paths_survive_openssh_configuration_parser() {
        let path = "/keys/certificate with spaces and \"quotes\".pub";
        let output = std::process::Command::new("/usr/bin/ssh")
            .args([
                "-G",
                "-F",
                "/dev/null",
                "-o",
                &certificate_option(path),
                "example.test",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(&format!("certificatefile {path}"))
        );
    }

    #[test]
    fn askpass_rejects_host_keys_mfa_and_other_identities() {
        assert_eq!(
            askpass_reference(
                "deploy@host's password: ",
                None,
                "deploy@host",
                None,
                Some("keychain://a/b"),
                None
            ),
            Some("keychain://a/b")
        );
        for prompt in [
            "Are you sure you want to continue connecting (yes/no)?",
            "Password:",
            "Verification code:",
            "root@other's password:",
        ] {
            assert!(askpass_reference(
                prompt,
                None,
                "deploy@host",
                None,
                Some("keychain://a/b"),
                None
            )
            .is_none());
        }
        assert!(askpass_reference(
            "deploy@host's password:",
            Some("confirm"),
            "deploy@host",
            None,
            Some("keychain://a/b"),
            None
        )
        .is_none());
        assert_eq!(
            askpass_reference(
                "Enter passphrase for key '/keys/key':",
                None,
                "deploy@host",
                Some("/keys/key"),
                None,
                Some("env://KEY_PASS")
            ),
            Some("env://KEY_PASS")
        );
        assert!(askpass_reference(
            "Enter passphrase for key '/other':",
            None,
            "deploy@host",
            Some("/keys/key"),
            None,
            Some("env://KEY_PASS")
        )
        .is_none());
    }
}

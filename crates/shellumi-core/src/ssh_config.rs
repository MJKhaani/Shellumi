//! Parser and serializer for OpenSSH client configuration files (~/.ssh/config).

use crate::SshDestination;

/// A host entry representing a named destination ready for saving or importing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostConfigEntry {
    pub name: String,
    pub destination: SshDestination,
}

impl HostConfigEntry {
    pub fn new(name: impl Into<String>, destination: SshDestination) -> Self {
        Self {
            name: name.into(),
            destination,
        }
    }
}

/// Parses an OpenSSH config string and returns valid host configuration entries.
///
/// Wildcard blocks (e.g. `Host *`) are ignored.
/// If `default_user` is provided, it is used when a host block omits the `User` directive.
/// If `Port` is omitted, standard port 22 is used.
/// If `HostName` is omitted, the `Host` alias itself is tested as the hostname.
pub fn parse_ssh_config(content: &str, default_user: Option<&str>) -> Vec<HostConfigEntry> {
    let mut entries = Vec::new();
    let mut current_aliases: Vec<String> = Vec::new();
    let mut current_hostname: Option<String> = None;
    let mut current_user: Option<String> = None;
    let mut current_port: Option<u16> = None;

    let flush_block = |entries: &mut Vec<HostConfigEntry>,
                       aliases: &[String],
                       hostname: Option<&str>,
                       user: Option<&str>,
                       port: Option<u16>| {
        let user = user.or(default_user);
        let port = port.unwrap_or(22);
        for alias in aliases {
            if alias.contains('*') || alias.contains('?') || alias.starts_with('!') {
                continue;
            }
            let host_to_use = hostname.unwrap_or(alias.as_str());
            if let Some(username) = user {
                if let Ok(destination) = SshDestination::new(host_to_use, username, port) {
                    entries.push(HostConfigEntry {
                        name: alias.clone(),
                        destination,
                    });
                }
            }
        }
    };

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (key, value) = if let Some((k, v)) = line.split_once('=') {
            (k.trim(), v.trim())
        } else if let Some((k, v)) = line.split_once(char::is_whitespace) {
            (k.trim(), v.trim())
        } else {
            (line, "")
        };

        let clean_value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            &value[1..value.len() - 1]
        } else {
            value
        };

        if key.eq_ignore_ascii_case("Host") {
            flush_block(
                &mut entries,
                &current_aliases,
                current_hostname.as_deref(),
                current_user.as_deref(),
                current_port,
            );
            current_aliases = clean_value
                .split_whitespace()
                .map(|s| s.trim_matches('"').to_string())
                .collect();
            current_hostname = None;
            current_user = None;
            current_port = None;
        } else if key.eq_ignore_ascii_case("HostName") {
            current_hostname = Some(clean_value.to_string());
        } else if key.eq_ignore_ascii_case("User") {
            current_user = Some(clean_value.to_string());
        } else if key.eq_ignore_ascii_case("Port") {
            if let Ok(p) = clean_value.parse::<u16>() {
                current_port = Some(p);
            }
        }
    }

    flush_block(
        &mut entries,
        &current_aliases,
        current_hostname.as_deref(),
        current_user.as_deref(),
        current_port,
    );

    entries
}

/// Serializes host entries to OpenSSH configuration syntax.
pub fn export_ssh_config(entries: &[HostConfigEntry]) -> String {
    let mut out = String::new();
    for entry in entries {
        out.push_str(&format!("Host {}\n", entry.name));
        out.push_str(&format!("    HostName {}\n", entry.destination.host()));
        out.push_str(&format!("    User {}\n", entry.destination.username()));
        out.push_str(&format!("    Port {}\n\n", entry.destination.port()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_ssh_config_file() {
        let config = r#"
# Global configuration
Host *
    ForwardAgent no
    ServerAliveInterval 60

Host prod-web
    HostName 198.51.100.10
    User ubuntu
    Port 2222

Host staging-db
    HostName db.staging.internal
    User postgres

Host direct.example.com
    User admin
"#;

        let entries = parse_ssh_config(config, None);
        assert_eq!(entries.len(), 3);

        assert_eq!(entries[0].name, "prod-web");
        assert_eq!(entries[0].destination.host(), "198.51.100.10");
        assert_eq!(entries[0].destination.username(), "ubuntu");
        assert_eq!(entries[0].destination.port().get(), 2222);

        assert_eq!(entries[1].name, "staging-db");
        assert_eq!(entries[1].destination.host(), "db.staging.internal");
        assert_eq!(entries[1].destination.username(), "postgres");
        assert_eq!(entries[1].destination.port().get(), 22); // Default port

        assert_eq!(entries[2].name, "direct.example.com");
        assert_eq!(entries[2].destination.host(), "direct.example.com"); // Used alias as HostName
        assert_eq!(entries[2].destination.username(), "admin");
    }

    #[test]
    fn roundtrip_export_and_import() {
        let original = vec![
            HostConfigEntry::new(
                "bastion",
                SshDestination::new("10.0.0.1", "jump", 22).unwrap(),
            ),
            HostConfigEntry::new(
                "web",
                SshDestination::new("example.com", "deploy", 2222).unwrap(),
            ),
        ];

        let serialized = export_ssh_config(&original);
        let parsed = parse_ssh_config(&serialized, None);
        assert_eq!(original, parsed);
    }
}

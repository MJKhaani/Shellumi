//! OpenSSH `known_hosts` parsing and host-key verification.
//!
//! Enforces strict host-key verification:
//! - Detects changed host keys (potential MITM attacks).
//! - Identifies unknown host keys requiring explicit user confirmation (TOFU).
//! - Rejects revoked host keys.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKeyMarker {
    CertAuthority,
    Revoked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownHostEntry {
    pub line_number: usize,
    pub marker: Option<HostKeyMarker>,
    pub hosts: Vec<String>,
    pub key_type: String,
    pub key_base64: String,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostKeyVerification {
    /// Host key matches an existing trusted record.
    Trusted { line_number: usize },
    /// No existing record for this host. Explicit confirmation (TOFU) required.
    NotFound { fingerprint: String },
    /// The host key DOES NOT MATCH the recorded host key for this host!
    /// Potential man-in-the-middle attack.
    Changed {
        line_number: usize,
        expected_key_type: String,
        expected_key_base64: String,
        actual_fingerprint: String,
    },
    /// The host key is marked as @revoked. Connection must be rejected.
    Revoked { line_number: usize },
}

#[derive(Debug, PartialEq, Eq)]
pub enum KnownHostsError {
    InvalidEntry(usize),
}

impl fmt::Display for KnownHostsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEntry(line) => write!(f, "invalid known_hosts entry on line {line}"),
        }
    }
}

impl std::error::Error for KnownHostsError {}

fn host_matches_pattern(pattern: &str, host: &str, port: u16) -> bool {
    let target = if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    };

    if pattern == target {
        return true;
    }

    // Pattern might also match plain host even if non-standard port or vice-versa
    if pattern == host {
        return true;
    }

    // Bracketed host pattern without port or with port
    if let Some(stripped) = pattern.strip_prefix('[') {
        if let Some((pat_host, pat_port)) = stripped.split_once("]:") {
            if pat_host == host && pat_port.parse::<u16>().ok() == Some(port) {
                return true;
            }
        }
    }

    false
}

/// Parse an OpenSSH `known_hosts` file content.
pub fn parse_known_hosts(content: &str) -> Vec<KnownHostEntry> {
    let mut entries = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let line_number = idx + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        let (marker, start_idx) = match tokens[0] {
            "@cert-authority" => (Some(HostKeyMarker::CertAuthority), 1),
            "@revoked" => (Some(HostKeyMarker::Revoked), 1),
            tok if tok.starts_with('@') => {
                // Unknown marker, skip or ignore
                continue;
            }
            _ => (None, 0),
        };

        if tokens.len() < start_idx + 3 {
            continue;
        }

        let hosts = tokens[start_idx]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let key_type = tokens[start_idx + 1].to_string();
        let key_base64 = tokens[start_idx + 2].to_string();
        let comment = if tokens.len() > start_idx + 3 {
            Some(tokens[start_idx + 3..].join(" "))
        } else {
            None
        };

        entries.push(KnownHostEntry {
            line_number,
            marker,
            hosts,
            key_type,
            key_base64,
            comment,
        });
    }

    entries
}

/// Verify a server's presented host key against parsed `known_hosts` entries.
pub fn verify_host_key(
    entries: &[KnownHostEntry],
    host: &str,
    port: u16,
    key_type: &str,
    key_base64: &str,
    actual_fingerprint: &str,
) -> HostKeyVerification {
    let mut matched_host_different_key: Option<&KnownHostEntry> = None;

    for entry in entries {
        let host_matches = entry
            .hosts
            .iter()
            .any(|pat| host_matches_pattern(pat, host, port));

        if !host_matches {
            continue;
        }

        if entry.marker == Some(HostKeyMarker::Revoked)
            && entry.key_type == key_type
            && entry.key_base64 == key_base64
        {
            return HostKeyVerification::Revoked {
                line_number: entry.line_number,
            };
        }

        if entry.key_type == key_type {
            if entry.key_base64 == key_base64 {
                return HostKeyVerification::Trusted {
                    line_number: entry.line_number,
                };
            } else {
                matched_host_different_key = Some(entry);
            }
        }
    }

    if let Some(differing) = matched_host_different_key {
        return HostKeyVerification::Changed {
            line_number: differing.line_number,
            expected_key_type: differing.key_type.clone(),
            expected_key_base64: differing.key_base64.clone(),
            actual_fingerprint: actual_fingerprint.to_string(),
        };
    }

    HostKeyVerification::NotFound {
        fingerprint: actual_fingerprint.to_string(),
    }
}

/// Format an accepted host key for appending to `~/.ssh/known_hosts`.
pub fn format_known_host_entry(
    host: &str,
    port: u16,
    key_type: &str,
    key_base64: &str,
    comment: Option<&str>,
) -> String {
    let host_pattern = if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    };

    match comment {
        Some(c) if !c.trim().is_empty() => {
            format!("{host_pattern} {key_type} {key_base64} {}\n", c.trim())
        }
        _ => format!("{host_pattern} {key_type} {key_base64}\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_KNOWN_HOSTS: &str = r#"
# Comment line
example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExampleKey1 dev-server
[192.168.1.50]:2222 ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIExampleKey2
@revoked compromised.org ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBadKey
"#;

    #[test]
    fn parses_known_hosts_entries_and_markers() {
        let entries = parse_known_hosts(SAMPLE_KNOWN_HOSTS);
        assert_eq!(entries.len(), 3);

        assert_eq!(entries[0].hosts, vec!["example.com"]);
        assert_eq!(entries[0].key_type, "ssh-ed25519");
        assert_eq!(
            entries[0].key_base64,
            "AAAAC3NzaC1lZDI1NTE5AAAAIExampleKey1"
        );
        assert_eq!(entries[0].comment.as_deref(), Some("dev-server"));
        assert_eq!(entries[0].marker, None);

        assert_eq!(entries[1].hosts, vec!["[192.168.1.50]:2222"]);
        assert_eq!(entries[1].key_type, "ecdsa-sha2-nistp256");

        assert_eq!(entries[2].hosts, vec!["compromised.org"]);
        assert_eq!(entries[2].marker, Some(HostKeyMarker::Revoked));
    }

    #[test]
    fn verifies_trusted_host_key() {
        let entries = parse_known_hosts(SAMPLE_KNOWN_HOSTS);
        let result = verify_host_key(
            &entries,
            "example.com",
            22,
            "ssh-ed25519",
            "AAAAC3NzaC1lZDI1NTE5AAAAIExampleKey1",
            "SHA256:fingerprint",
        );
        assert_eq!(result, HostKeyVerification::Trusted { line_number: 3 });
    }

    #[test]
    fn detects_changed_host_key_mitm_risk() {
        let entries = parse_known_hosts(SAMPLE_KNOWN_HOSTS);
        let result = verify_host_key(
            &entries,
            "example.com",
            22,
            "ssh-ed25519",
            "AAAAC3NzaC1lZDI1NTE5AAAAIDifferentAttackerKey",
            "SHA256:attackersfingerprint",
        );
        assert!(matches!(
            result,
            HostKeyVerification::Changed {
                line_number: 3,
                ref expected_key_base64,
                ref actual_fingerprint,
                ..
            } if expected_key_base64 == "AAAAC3NzaC1lZDI1NTE5AAAAIExampleKey1"
                && actual_fingerprint == "SHA256:attackersfingerprint"
        ));
    }

    #[test]
    fn detects_not_found_requiring_tofu() {
        let entries = parse_known_hosts(SAMPLE_KNOWN_HOSTS);
        let result = verify_host_key(
            &entries,
            "new-server.internal",
            22,
            "ssh-ed25519",
            "AAAAC3NzaC1lZDI1NTE5AAAAINewKey",
            "SHA256:newfingerprint",
        );
        assert_eq!(
            result,
            HostKeyVerification::NotFound {
                fingerprint: "SHA256:newfingerprint".into()
            }
        );
    }

    #[test]
    fn detects_revoked_host_key() {
        let entries = parse_known_hosts(SAMPLE_KNOWN_HOSTS);
        let result = verify_host_key(
            &entries,
            "compromised.org",
            22,
            "ssh-ed25519",
            "AAAAC3NzaC1lZDI1NTE5AAAAIBadKey",
            "SHA256:badfingerprint",
        );
        assert_eq!(result, HostKeyVerification::Revoked { line_number: 5 });
    }

    #[test]
    fn formats_known_host_entry_correctly() {
        let line22 =
            format_known_host_entry("myserver.net", 22, "ssh-ed25519", "AAAAB3NzaC1", None);
        assert_eq!(line22, "myserver.net ssh-ed25519 AAAAB3NzaC1\n");

        let line2222 = format_known_host_entry(
            "myserver.net",
            2222,
            "ssh-ed25519",
            "AAAAB3NzaC1",
            Some("backup"),
        );
        assert_eq!(
            line2222,
            "[myserver.net]:2222 ssh-ed25519 AAAAB3NzaC1 backup\n"
        );
    }
}

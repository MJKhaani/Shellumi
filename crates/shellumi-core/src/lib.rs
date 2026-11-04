//! UI-independent connection definitions. No network connections are made here.

use std::fmt;
use std::num::NonZeroU16;

/// A validated SSH destination. Credentials are deliberately excluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshDestination {
    host: String,
    username: String,
    port: NonZeroU16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationError {
    InvalidHost,
    InvalidUsername,
    InvalidPort,
}

impl fmt::Display for DestinationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidHost => {
                "Enter a hostname or IP address without whitespace or control characters."
            }
            Self::InvalidUsername => "Enter a username without whitespace or control characters.",
            Self::InvalidPort => "Enter a port between 1 and 65535.",
        })
    }
}

impl std::error::Error for DestinationError {}

impl SshDestination {
    pub fn new(host: &str, username: &str, port: u16) -> Result<Self, DestinationError> {
        let host = host.trim();
        let username = username.trim();
        if host.is_empty()
            || host.starts_with('-')
            || host.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(DestinationError::InvalidHost);
        }
        if username.is_empty()
            || username
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(DestinationError::InvalidUsername);
        }
        let port = NonZeroU16::new(port).ok_or(DestinationError::InvalidPort)?;
        Ok(Self {
            host: host.to_owned(),
            username: username.to_owned(),
            port,
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn port(&self) -> NonZeroU16 {
        self.port
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_dns_and_ipv6_and_normalizes_outer_whitespace() {
        for host in ["example.org", "2001:db8::1"] {
            let destination = SshDestination::new(&format!(" {host} "), " alice ", 22).unwrap();
            assert_eq!(destination.host(), host);
            assert_eq!(destination.username(), "alice");
            assert_eq!(destination.port().get(), 22);
        }
    }

    #[test]
    fn rejects_unsafe_or_missing_fields() {
        for host in ["", "-oProxyCommand=bad", "some host", "host\0name"] {
            assert_eq!(
                SshDestination::new(host, "alice", 22),
                Err(DestinationError::InvalidHost)
            );
        }
        for username in ["", "two users", "alice\0bad"] {
            assert_eq!(
                SshDestination::new("host", username, 22),
                Err(DestinationError::InvalidUsername)
            );
        }
        assert_eq!(
            SshDestination::new("host", "alice", 0),
            Err(DestinationError::InvalidPort)
        );
    }
}

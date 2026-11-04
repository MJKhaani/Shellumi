//! SSH Port Forwarding (Tunneling) and Jump Host domain models.
//!
//! Supports:
//! - Local port forwarding (`-L [bind_address:]bind_port:target_host:target_port`)
//! - Remote port forwarding (`-R [bind_address:]bind_port:target_host:target_port`)
//! - Dynamic port forwarding (SOCKS5 `-D [bind_address:]bind_port`)
//! - Jump host chains (`-J [user@]host[:port][,...]`)
//! - Security warnings for bindings exposed beyond localhost

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForwardingType {
    Local,
    Remote,
    Dynamic,
}

impl ForwardingType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
            Self::Dynamic => "dynamic",
        }
    }

    pub fn flag(&self) -> &'static str {
        match self {
            Self::Local => "-L",
            Self::Remote => "-R",
            Self::Dynamic => "-D",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "local" | "-l" | "l" => Some(Self::Local),
            "remote" | "-r" | "r" => Some(Self::Remote),
            "dynamic" | "socks" | "socks5" | "-d" | "d" => Some(Self::Dynamic),
            _ => None,
        }
    }
}

impl fmt::Display for ForwardingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TunnelError {
    InvalidPort(String),
    InvalidHost(String),
    InvalidAddress(String),
    InvalidSpec(String),
    MissingTarget,
    InvalidJumpHost(String),
}

impl fmt::Display for TunnelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPort(p) => write!(f, "invalid port: {p}"),
            Self::InvalidHost(h) => write!(f, "invalid host: {h}"),
            Self::InvalidAddress(a) => write!(f, "invalid bind address: {a}"),
            Self::InvalidSpec(s) => write!(f, "invalid tunnel specification: {s}"),
            Self::MissingTarget => {
                f.write_str("target host and port required for local/remote forwarding")
            }
            Self::InvalidJumpHost(j) => write!(f, "invalid jump host specification: {j}"),
        }
    }
}

impl std::error::Error for TunnelError {}

/// A validated SSH port forwarding rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortForwardRule {
    pub forwarding_type: ForwardingType,
    pub bind_address: Option<String>,
    pub bind_port: u16,
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
}

fn validate_host_or_addr(name: &str, is_target: bool) -> Result<String, TunnelError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(if is_target {
            TunnelError::InvalidHost("must not be empty".into())
        } else {
            TunnelError::InvalidAddress("must not be empty".into())
        });
    }
    if trimmed.starts_with('-') {
        return Err(if is_target {
            TunnelError::InvalidHost("must not start with a hyphen".into())
        } else {
            TunnelError::InvalidAddress("must not start with a hyphen".into())
        });
    }
    if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(if is_target {
            TunnelError::InvalidHost("must not contain whitespace or control characters".into())
        } else {
            TunnelError::InvalidAddress("must not contain whitespace or control characters".into())
        });
    }
    Ok(trimmed.to_string())
}

fn parse_port(s: &str) -> Result<u16, TunnelError> {
    let p = s
        .trim()
        .parse::<u16>()
        .map_err(|_| TunnelError::InvalidPort(s.to_string()))?;
    if p == 0 {
        return Err(TunnelError::InvalidPort(
            "port must be between 1 and 65535".into(),
        ));
    }
    Ok(p)
}

impl PortForwardRule {
    /// Create a local forwarding rule (`-L [bind_address:]bind_port:target_host:target_port`).
    pub fn local(
        bind_address: Option<&str>,
        bind_port: u16,
        target_host: &str,
        target_port: u16,
    ) -> Result<Self, TunnelError> {
        if bind_port == 0 {
            return Err(TunnelError::InvalidPort("bind port must be > 0".into()));
        }
        if target_port == 0 {
            return Err(TunnelError::InvalidPort("target port must be > 0".into()));
        }
        let bind_address = match bind_address {
            Some(a) => Some(validate_host_or_addr(a, false)?),
            None => None,
        };
        let target_host = validate_host_or_addr(target_host, true)?;

        Ok(Self {
            forwarding_type: ForwardingType::Local,
            bind_address,
            bind_port,
            target_host: Some(target_host),
            target_port: Some(target_port),
        })
    }

    /// Create a remote forwarding rule (`-R [bind_address:]bind_port:target_host:target_port`).
    pub fn remote(
        bind_address: Option<&str>,
        bind_port: u16,
        target_host: &str,
        target_port: u16,
    ) -> Result<Self, TunnelError> {
        if bind_port == 0 {
            return Err(TunnelError::InvalidPort("bind port must be > 0".into()));
        }
        if target_port == 0 {
            return Err(TunnelError::InvalidPort("target port must be > 0".into()));
        }
        let bind_address = match bind_address {
            Some(a) => Some(validate_host_or_addr(a, false)?),
            None => None,
        };
        let target_host = validate_host_or_addr(target_host, true)?;

        Ok(Self {
            forwarding_type: ForwardingType::Remote,
            bind_address,
            bind_port,
            target_host: Some(target_host),
            target_port: Some(target_port),
        })
    }

    /// Create a dynamic (SOCKS5) forwarding rule (`-D [bind_address:]bind_port`).
    pub fn dynamic(bind_address: Option<&str>, bind_port: u16) -> Result<Self, TunnelError> {
        if bind_port == 0 {
            return Err(TunnelError::InvalidPort("bind port must be > 0".into()));
        }
        let bind_address = match bind_address {
            Some(a) => Some(validate_host_or_addr(a, false)?),
            None => None,
        };

        Ok(Self {
            forwarding_type: ForwardingType::Dynamic,
            bind_address,
            bind_port,
            target_host: None,
            target_port: None,
        })
    }

    /// Parse a forwarding specification string:
    /// - For Local/Remote: `[bind_addr:]bind_port:target_host:target_port`
    /// - For Dynamic: `[bind_addr:]bind_port`
    pub fn parse(spec: &str, forwarding_type: ForwardingType) -> Result<Self, TunnelError> {
        let parts: Vec<&str> = spec.split(':').collect();
        match forwarding_type {
            ForwardingType::Dynamic => match parts.len() {
                1 => {
                    let port = parse_port(parts[0])?;
                    Self::dynamic(None, port)
                }
                2 => {
                    let addr = parts[0];
                    let port = parse_port(parts[1])?;
                    Self::dynamic(Some(addr), port)
                }
                _ => Err(TunnelError::InvalidSpec(
                    "dynamic tunnel requires [bind_address:]port".into(),
                )),
            },
            ForwardingType::Local | ForwardingType::Remote => {
                match parts.len() {
                    3 => {
                        // bind_port:target_host:target_port
                        let bind_port = parse_port(parts[0])?;
                        let target_host = parts[1];
                        let target_port = parse_port(parts[2])?;
                        if forwarding_type == ForwardingType::Local {
                            Self::local(None, bind_port, target_host, target_port)
                        } else {
                            Self::remote(None, bind_port, target_host, target_port)
                        }
                    }
                    4 => {
                        // bind_address:bind_port:target_host:target_port
                        let bind_address = parts[0];
                        let bind_port = parse_port(parts[1])?;
                        let target_host = parts[2];
                        let target_port = parse_port(parts[3])?;
                        if forwarding_type == ForwardingType::Local {
                            Self::local(Some(bind_address), bind_port, target_host, target_port)
                        } else {
                            Self::remote(Some(bind_address), bind_port, target_host, target_port)
                        }
                    }
                    _ => Err(TunnelError::InvalidSpec(
                        "requires [bind_address:]bind_port:target_host:target_port".into(),
                    )),
                }
            }
        }
    }

    /// Returns `true` if the binding is exposed beyond localhost (e.g. `0.0.0.0`, `*`, or public interface).
    /// Used to generate mandatory security warnings before exposing services.
    pub fn is_exposed_beyond_localhost(&self) -> bool {
        if let Some(addr) = &self.bind_address {
            let lower = addr.to_lowercase();
            lower == "0.0.0.0"
                || lower == "::"
                || lower == "*"
                || (!lower.starts_with("127.") && lower != "localhost" && lower != "::1")
        } else {
            false
        }
    }

    /// Format as an OpenSSH command-line argument pair: `("-L", "127.0.0.1:8080:target:80")`.
    pub fn openssh_argument(&self) -> (&'static str, String) {
        let flag = self.forwarding_type.flag();
        let value = match self.forwarding_type {
            ForwardingType::Dynamic => {
                if let Some(addr) = &self.bind_address {
                    format!("{}:{}", addr, self.bind_port)
                } else {
                    format!("{}", self.bind_port)
                }
            }
            ForwardingType::Local | ForwardingType::Remote => {
                let target_host = self.target_host.as_deref().unwrap_or("localhost");
                let target_port = self.target_port.unwrap_or(80);
                if let Some(addr) = &self.bind_address {
                    format!(
                        "{}:{}:{}:{}",
                        addr, self.bind_port, target_host, target_port
                    )
                } else {
                    format!("{}:{}:{}", self.bind_port, target_host, target_port)
                }
            }
        };
        (flag, value)
    }
}

/// Validates a jump host string or comma-separated jump host chain (`-J`).
///
/// Accepts formats like:
/// - `jump.example.org`
/// - `admin@jump.example.org:2222`
/// - `bastion1.corp.internal,bastion2.dmz.internal:2222`
///
/// Rejects control characters, whitespace, leading hyphens, and unsafe characters.
pub fn validate_jump_host(jump: &str) -> Result<String, TunnelError> {
    let trimmed = jump.trim();
    if trimmed.is_empty() {
        return Err(TunnelError::InvalidJumpHost("must not be empty".into()));
    }
    if trimmed.starts_with('-') {
        return Err(TunnelError::InvalidJumpHost(
            "must not start with a hyphen".into(),
        ));
    }
    if trimmed.chars().any(|c| {
        c.is_control()
            || c.is_whitespace()
            || c == ';'
            || c == '&'
            || c == '|'
            || c == '`'
            || c == '$'
    }) {
        return Err(TunnelError::InvalidJumpHost(
            "contains prohibited shell or control characters".into(),
        ));
    }

    // Validate each hop in the chain
    for hop in trimmed.split(',') {
        let hop = hop.trim();
        if hop.is_empty() {
            return Err(TunnelError::InvalidJumpHost(
                "hop in chain must not be empty".into(),
            ));
        }
        if hop.starts_with('-') {
            return Err(TunnelError::InvalidJumpHost(
                "hop must not start with a hyphen".into(),
            ));
        }
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_formats_local_forwarding() {
        let rule = PortForwardRule::local(None, 8080, "internal-db", 5432).unwrap();
        assert_eq!(
            rule.openssh_argument(),
            ("-L", "8080:internal-db:5432".to_string())
        );
        assert!(!rule.is_exposed_beyond_localhost());

        let rule_bound =
            PortForwardRule::local(Some("127.0.0.1"), 8080, "internal-db", 5432).unwrap();
        assert_eq!(
            rule_bound.openssh_argument(),
            ("-L", "127.0.0.1:8080:internal-db:5432".to_string())
        );
        assert!(!rule_bound.is_exposed_beyond_localhost());

        let rule_exposed =
            PortForwardRule::local(Some("0.0.0.0"), 8080, "internal-db", 5432).unwrap();
        assert_eq!(
            rule_exposed.openssh_argument(),
            ("-L", "0.0.0.0:8080:internal-db:5432".to_string())
        );
        assert!(rule_exposed.is_exposed_beyond_localhost());
    }

    #[test]
    fn creates_and_formats_remote_forwarding() {
        let rule = PortForwardRule::remote(Some("0.0.0.0"), 9000, "localhost", 3000).unwrap();
        assert_eq!(
            rule.openssh_argument(),
            ("-R", "0.0.0.0:9000:localhost:3000".to_string())
        );
        assert!(rule.is_exposed_beyond_localhost());
    }

    #[test]
    fn creates_and_formats_dynamic_forwarding() {
        let rule = PortForwardRule::dynamic(None, 1080).unwrap();
        assert_eq!(rule.openssh_argument(), ("-D", "1080".to_string()));
        assert!(!rule.is_exposed_beyond_localhost());

        let rule_ip = PortForwardRule::dynamic(Some("127.0.0.1"), 1080).unwrap();
        assert_eq!(
            rule_ip.openssh_argument(),
            ("-D", "127.0.0.1:1080".to_string())
        );
        assert!(!rule_ip.is_exposed_beyond_localhost());

        let rule_pub = PortForwardRule::dynamic(Some("192.168.1.50"), 1080).unwrap();
        assert!(rule_pub.is_exposed_beyond_localhost());
    }

    #[test]
    fn parses_forwarding_specs() {
        let l1 = PortForwardRule::parse("8080:db.local:5432", ForwardingType::Local).unwrap();
        assert_eq!(l1.bind_port, 8080);
        assert_eq!(l1.target_host.as_deref(), Some("db.local"));
        assert_eq!(l1.target_port, Some(5432));
        assert_eq!(l1.bind_address, None);

        let l2 =
            PortForwardRule::parse("127.0.0.1:8080:db.local:5432", ForwardingType::Local).unwrap();
        assert_eq!(l2.bind_address.as_deref(), Some("127.0.0.1"));
        assert_eq!(l2.bind_port, 8080);

        let d1 = PortForwardRule::parse("1080", ForwardingType::Dynamic).unwrap();
        assert_eq!(d1.bind_port, 1080);
        assert_eq!(d1.bind_address, None);

        let d2 = PortForwardRule::parse("0.0.0.0:1080", ForwardingType::Dynamic).unwrap();
        assert_eq!(d2.bind_address.as_deref(), Some("0.0.0.0"));
        assert!(d2.is_exposed_beyond_localhost());
    }

    #[test]
    fn validates_jump_hosts_and_chains() {
        assert_eq!(
            validate_jump_host("jump.example.org").unwrap(),
            "jump.example.org"
        );
        assert_eq!(
            validate_jump_host("admin@jump.example.org:2222").unwrap(),
            "admin@jump.example.org:2222"
        );
        assert_eq!(
            validate_jump_host("bastion1.corp.internal,bastion2.dmz.internal:2222").unwrap(),
            "bastion1.corp.internal,bastion2.dmz.internal:2222"
        );

        // Rejections
        assert!(validate_jump_host("").is_err());
        assert!(validate_jump_host("  ").is_err());
        assert!(validate_jump_host("-badhost").is_err());
        assert!(validate_jump_host("hop1, -badhop").is_err());
        assert!(validate_jump_host("host; rm -rf /").is_err());
        assert!(validate_jump_host("host`id`").is_err());
        assert!(validate_jump_host("host$USER").is_err());
    }
}

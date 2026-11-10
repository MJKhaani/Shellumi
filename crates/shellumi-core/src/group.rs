//! Host group models and configuration inheritance.
//!
//! Hosts can inherit default username, default port, and tags from their parent group.

use crate::{DestinationError, SshDestination};
use std::fmt;
use std::num::NonZeroU16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostGroup {
    pub name: String,
    pub default_username: Option<String>,
    pub default_port: Option<NonZeroU16>,
    pub tags: Vec<String>,
    pub identity_id: Option<i64>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GroupError {
    InvalidName,
    InvalidUsername,
    InvalidPort,
    InvalidTag,
}

impl fmt::Display for GroupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName => {
                f.write_str("group name must not be empty or contain control characters")
            }
            Self::InvalidUsername => {
                f.write_str("default username must not contain whitespace or control characters")
            }
            Self::InvalidPort => f.write_str("default port must be between 1 and 65535"),
            Self::InvalidTag => f.write_str("tag must not contain commas or control characters"),
        }
    }
}

impl std::error::Error for GroupError {}

impl HostGroup {
    pub fn new(
        name: &str,
        default_username: Option<&str>,
        default_port: Option<u16>,
        tags: &[String],
    ) -> Result<Self, GroupError> {
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(GroupError::InvalidName);
        }

        let default_username = match default_username {
            Some(u) => {
                let u = u.trim();
                if u.is_empty() {
                    None
                } else if u.chars().any(|c| c.is_whitespace() || c.is_control()) {
                    return Err(GroupError::InvalidUsername);
                } else {
                    Some(u.to_string())
                }
            }
            None => None,
        };

        let default_port = match default_port {
            Some(p) => Some(NonZeroU16::new(p).ok_or(GroupError::InvalidPort)?),
            None => None,
        };

        let mut clean_tags = Vec::new();
        for tag in tags {
            let t = tag.trim();
            if t.is_empty() {
                continue;
            }
            if t.chars().any(|c| c == ',' || c.is_control()) {
                return Err(GroupError::InvalidTag);
            }
            clean_tags.push(t.to_string());
        }

        Ok(Self {
            name: name.to_string(),
            default_username,
            default_port,
            tags: clean_tags,
            identity_id: None,
        })
    }

    pub fn with_identity_id(mut self, identity_id: Option<i64>) -> Self {
        self.identity_id = identity_id;
        self
    }
}

/// Resolves effective connection parameters for a host, inheriting missing username
/// or port settings from its group when available.
pub fn resolve_effective_destination(
    host: &str,
    username: Option<&str>,
    port: Option<u16>,
    group: Option<&HostGroup>,
) -> Result<SshDestination, DestinationError> {
    resolve_effective_destination_with_identity(host, username, port, group, None)
}

/// Resolves effective connection parameters for a host, inheriting missing username
/// from its assigned identity or parent group.
pub fn resolve_effective_destination_with_identity(
    host: &str,
    username: Option<&str>,
    port: Option<u16>,
    group: Option<&HostGroup>,
    identity: Option<&crate::Identity>,
) -> Result<SshDestination, DestinationError> {
    let effective_user = match username {
        Some(u)
            if !u.trim().is_empty()
                && u.trim() != "default_placeholder"
                && u.trim() != "original" =>
        {
            u.trim()
        }
        _ => match identity {
            Some(ident) if !ident.username.trim().is_empty() => ident.username.trim(),
            _ => match group.and_then(|g| g.default_username.as_deref()) {
                Some(gu) => gu,
                None => match username {
                    Some(u) if !u.trim().is_empty() => u.trim(),
                    _ => return Err(DestinationError::InvalidUsername),
                },
            },
        },
    };

    let effective_port = match port {
        Some(p) if p > 0 => p,
        _ => match group.and_then(|g| g.default_port) {
            Some(gp) => gp.get(),
            None => 22,
        },
    };

    SshDestination::new(host, effective_user, effective_port)
}

/// Resolves the effective set of tags for a host by merging host-specific tags
/// with inherited group tags, deduplicating while preserving order.
pub fn resolve_effective_tags(host_tags: &[String], group: Option<&HostGroup>) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(group) = group {
        for tag in &group.tags {
            if !result.contains(tag) {
                result.push(tag.clone());
            }
        }
    }
    for tag in host_tags {
        if !result.contains(tag) {
            result.push(tag.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_valid_group_and_rejects_invalid_fields() {
        let group = HostGroup::new(
            "Production",
            Some("ubuntu"),
            Some(2200),
            &["cloud".into(), "aws".into()],
        )
        .unwrap();
        assert_eq!(group.name, "Production");
        assert_eq!(group.default_username.as_deref(), Some("ubuntu"));
        assert_eq!(group.default_port.map(|p| p.get()), Some(2200));
        assert_eq!(group.tags, vec!["cloud", "aws"]);

        assert_eq!(
            HostGroup::new("", None, None, &[]),
            Err(GroupError::InvalidName)
        );
        assert_eq!(
            HostGroup::new("Valid", Some("bad user"), None, &[]),
            Err(GroupError::InvalidUsername)
        );
        assert_eq!(
            HostGroup::new("Valid", None, Some(0), &[]),
            Err(GroupError::InvalidPort)
        );
        assert_eq!(
            HostGroup::new("Valid", None, None, &["bad,tag".into()]),
            Err(GroupError::InvalidTag)
        );
    }

    #[test]
    fn inherits_destination_parameters_from_group() {
        let group =
            HostGroup::new("Staging", Some("deploy"), Some(2222), &["internal".into()]).unwrap();

        // Host provides no username or port -> inherits both
        let d1 = resolve_effective_destination("10.0.0.1", None, None, Some(&group)).unwrap();
        assert_eq!(d1.username(), "deploy");
        assert_eq!(d1.port().get(), 2222);

        // Host provides specific username -> overrides group username, inherits port
        let d2 =
            resolve_effective_destination("10.0.0.1", Some("root"), None, Some(&group)).unwrap();
        assert_eq!(d2.username(), "root");
        assert_eq!(d2.port().get(), 2222);

        // Host provides specific port -> overrides group port, inherits username
        let d3 = resolve_effective_destination("10.0.0.1", None, Some(8022), Some(&group)).unwrap();
        assert_eq!(d3.username(), "deploy");
        assert_eq!(d3.port().get(), 8022);

        // No group, no username -> error
        assert_eq!(
            resolve_effective_destination("10.0.0.1", None, None, None),
            Err(DestinationError::InvalidUsername)
        );

        // No group, port defaults to 22
        let d4 = resolve_effective_destination("10.0.0.1", Some("alice"), None, None).unwrap();
        assert_eq!(d4.username(), "alice");
        assert_eq!(d4.port().get(), 22);

        // Host username takes precedence over identity username
        let ident = crate::Identity {
            id: 1,
            name: "id_ed25519".into(),
            username: "mjk".into(),
            auth_method: crate::AuthMethod::PublicKey {
                key_name: "id_ed25519".into(),
            },
            password_secret_ref: None,
            key_path: Some("/home/mjk/.ssh/id_ed25519".into()),
            passphrase_secret_ref: None,
            certificate_path: None,
            tags: vec![],
            comment: None,
            created_at: None,
            updated_at: None,
        };
        let d5 = resolve_effective_destination_with_identity(
            "oasis",
            Some("root"),
            None,
            None,
            Some(&ident),
        )
        .unwrap();
        assert_eq!(
            d5.username(),
            "root",
            "Host username must not be overridden by identity user"
        );

        // When host username is omitted, identity username is used as fallback
        let d6 =
            resolve_effective_destination_with_identity("oasis", None, None, None, Some(&ident))
                .unwrap();
        assert_eq!(d6.username(), "mjk");
    }

    #[test]
    fn merges_and_deduplicates_inherited_tags() {
        let group = HostGroup::new("Prod", None, None, &["global".into(), "eu".into()]).unwrap();
        let host_tags = vec!["web".into(), "eu".into(), "primary".into()];
        let effective = resolve_effective_tags(&host_tags, Some(&group));
        assert_eq!(effective, vec!["global", "eu", "web", "primary"]);
    }
}

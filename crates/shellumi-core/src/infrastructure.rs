//! Infrastructure integrations: Cloud providers (AWS, Azure, DigitalOcean) and Ansible inventory.
//!
//! Provides pure Rust domain models, response decoders, configuration builders,
//! and bidirectional Ansible inventory serialization with non-destructive merge logic.

mod ansible;
mod aws;
mod azure;
mod digitalocean;

pub use ansible::{AnsibleInventoryEngine, HostExportItem};
pub use aws::AwsEc2Parser;
pub use azure::AzureVmParser;
pub use digitalocean::DigitalOceanParser;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// Supported infrastructure and cloud discovery providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CloudProviderKind {
    /// Amazon Web Services (EC2 instances).
    Aws,
    /// Microsoft Azure (Compute Virtual Machines).
    Azure,
    /// DigitalOcean (Droplets).
    DigitalOcean,
    /// Ansible Inventory (INI or YAML).
    Ansible,
}

impl CloudProviderKind {
    /// Returns the canonical kebab-case identifier for this provider.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aws => "aws",
            Self::Azure => "azure",
            Self::DigitalOcean => "digitalocean",
            Self::Ansible => "ansible",
        }
    }

    /// Returns a human-friendly display label.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Aws => "Amazon Web Services",
            Self::Azure => "Microsoft Azure",
            Self::DigitalOcean => "DigitalOcean",
            Self::Ansible => "Ansible Inventory",
        }
    }

    /// Returns an emoji icon representing the provider.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Aws => "☁️",
            Self::Azure => "🔷",
            Self::DigitalOcean => "🌊",
            Self::Ansible => "🅰️",
        }
    }

    /// Returns the default SSH port for hosts discovered under this provider.
    pub fn default_port(&self) -> u16 {
        22
    }

    /// Returns the default fallback SSH username for this cloud provider.
    pub fn default_username(&self) -> &'static str {
        match self {
            Self::Aws => "ec2-user",
            Self::Azure => "azureuser",
            Self::DigitalOcean => "root",
            Self::Ansible => "root",
        }
    }
}

impl fmt::Display for CloudProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for CloudProviderKind {
    type Err = InfrastructureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "aws" | "ec2" | "amazon" => Ok(Self::Aws),
            "azure" | "msazure" | "microsoft" => Ok(Self::Azure),
            "digitalocean" | "do" | "droplet" | "droplets" => Ok(Self::DigitalOcean),
            "ansible" | "inventory" => Ok(Self::Ansible),
            other => Err(InfrastructureError::InvalidConfig(format!(
                "unsupported cloud provider '{other}'. Supported: aws, azure, digitalocean, ansible"
            ))),
        }
    }
}

/// Configuration settings for an infrastructure provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudProviderConfig {
    pub kind: CloudProviderKind,
    pub name: String,
    pub region: Option<String>,
    pub profile: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_key: Option<String>,
    pub session_token: Option<String>,
    pub subscription_id: Option<String>,
    pub resource_group: Option<String>,
    pub tenant_id: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub api_token: Option<String>,
    pub inventory_path: Option<String>,
    pub inventory_format: Option<String>,
    pub use_private_ip: bool,
    pub tag_filter: Option<String>,
    pub project_name: Option<String>,
}

impl CloudProviderConfig {
    /// Creates a new provider configuration with defaults.
    pub fn new(kind: CloudProviderKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            region: None,
            profile: None,
            access_key_id: None,
            secret_key: None,
            session_token: None,
            subscription_id: None,
            resource_group: None,
            tenant_id: None,
            client_id: None,
            client_secret: None,
            api_token: None,
            inventory_path: None,
            inventory_format: None,
            use_private_ip: false,
            tag_filter: None,
            project_name: None,
        }
    }

    /// Validates the configuration parameters according to provider requirements.
    pub fn validate(&self) -> Result<(), InfrastructureError> {
        if self.name.trim().is_empty() {
            return Err(InfrastructureError::InvalidConfig(
                "provider name cannot be empty".to_string(),
            ));
        }

        match self.kind {
            CloudProviderKind::Aws => {
                // Must specify either a profile, credentials, or allow ambient environment credentials
            }
            CloudProviderKind::Azure => {
                // If service principal provided, validate tuple
                if self.client_id.is_some() && self.client_secret.is_none() {
                    return Err(InfrastructureError::InvalidConfig(
                        "Azure client_secret must accompany client_id".to_string(),
                    ));
                }
            }
            CloudProviderKind::DigitalOcean => {
                // Token can be configured or provided via ambient environment
            }
            CloudProviderKind::Ansible => {
                if let Some(ref path) = self.inventory_path {
                    if path.trim().is_empty() {
                        return Err(InfrastructureError::InvalidConfig(
                            "Ansible inventory path cannot be empty".to_string(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns a redacted diagnostic summary ensuring secrets are never exposed in plaintext.
    pub fn redacted_summary(&self) -> String {
        let mut parts = vec![
            format!("name: {}", self.name),
            format!("kind: {}", self.kind),
        ];

        if let Some(ref reg) = self.region {
            parts.push(format!("region: {reg}"));
        }
        if let Some(ref prof) = self.profile {
            parts.push(format!("profile: {prof}"));
        }
        if let Some(ref key) = self.access_key_id {
            parts.push(format!("access_key: {}***", &key[..key.len().min(4)]));
        }
        if self.secret_key.is_some() {
            parts.push("secret_key: [REDACTED]".to_string());
        }
        if let Some(ref sub) = self.subscription_id {
            parts.push(format!("subscription_id: {sub}"));
        }
        if let Some(ref path) = self.inventory_path {
            parts.push(format!("inventory_path: {path}"));
        }
        if self.use_private_ip {
            parts.push("ip_mode: private".to_string());
        } else {
            parts.push("ip_mode: public".to_string());
        }

        parts.join(", ")
    }
}

/// A host instance discovered from a cloud provider or Ansible inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredHost {
    /// Unique provider-scoped instance identifier (e.g. `aws:i-0123456789abcdef0`).
    pub id: String,
    /// Friendly instance name.
    pub name: String,
    /// Source cloud provider kind.
    pub provider: CloudProviderKind,
    /// Hostname, FQDN, or IP address.
    pub hostname: String,
    /// SSH connection port.
    pub port: u16,
    /// Recommended SSH login username.
    pub username: Option<String>,
    /// Infrastructure state (e.g. "running", "stopped", "active").
    pub state: String,
    /// Cloud region or datacenter (e.g. "us-east-1", "nyc3").
    pub region: Option<String>,
    /// Cloud instance hardware size/flavor (e.g. "t3.medium", "Standard_B2s").
    pub instance_type: Option<String>,
    /// Hierarchical group path (e.g. "AWS/us-east-1/Production").
    pub group_path: Option<String>,
    /// Discovered key-value tags.
    pub tags: BTreeMap<String, String>,
    /// Additional provider-specific diagnostic metadata.
    pub extra_metadata: BTreeMap<String, String>,
}

impl DiscoveredHost {
    /// Returns true if the instance is currently running/active.
    pub fn is_running(&self) -> bool {
        let s = self.state.trim().to_lowercase();
        matches!(
            s.as_str(),
            "running" | "active" | "online" | "vm running" | "up"
        )
    }

    /// Returns a styled status indicator badge label.
    pub fn status_badge(&self) -> &'static str {
        if self.is_running() {
            "🟢 Running"
        } else if self.state.eq_ignore_ascii_case("stopped")
            || self.state.eq_ignore_ascii_case("vm stopped")
            || self.state.eq_ignore_ascii_case("terminated")
        {
            "🔴 Stopped"
        } else {
            "🟡 Pending"
        }
    }

    /// Formats the primary destination string (`user@host:port` or `host:port`).
    pub fn format_destination(&self) -> String {
        let base = if let Some(ref u) = self.username {
            format!("{u}@{}", self.hostname)
        } else {
            self.hostname.clone()
        };

        if self.port != 22 {
            format!("{base}:{}", self.port)
        } else {
            base
        }
    }
}

/// Simplified view of an existing saved host for non-destructive merge operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingHostRecord {
    pub id: String,
    pub name: String,
    pub hostname: String,
    pub port: u16,
    pub username: Option<String>,
    pub group_id: Option<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub startup_snippet_id: Option<String>,
    pub external_provider_id: Option<String>,
}

/// Policy choices for merging discovered infrastructure with existing saved hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergePolicy {
    /// Preserve custom user-added tags instead of replacing them completely.
    pub preserve_user_tags: bool,
    /// Preserve user-defined username overrides if already set.
    pub preserve_user_credentials: bool,
    /// Preserve user-written notes.
    pub preserve_notes: bool,
    /// Flag hosts that were previously imported but are now missing from the provider.
    pub detect_stale_hosts: bool,
}

impl Default for MergePolicy {
    fn default() -> Self {
        Self {
            preserve_user_tags: true,
            preserve_user_credentials: true,
            preserve_notes: true,
            detect_stale_hosts: true,
        }
    }
}

/// Action to be taken on an individual host during inventory synchronization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfraSyncAction {
    /// Create a new host record in the local database.
    Create(DiscoveredHost),
    /// Update dynamic properties of an existing host while preserving user customizations.
    Update {
        existing_id: String,
        discovered: DiscoveredHost,
        updated_hostname: bool,
        updated_state: bool,
        merged_tags: Vec<String>,
    },
    /// Host is already up to date; no changes required.
    Unchanged { existing_id: String, name: String },
    /// Host previously imported from this provider is no longer present in provider output.
    Stale {
        existing_id: String,
        name: String,
        external_id: String,
    },
}

/// Summary report of an inventory synchronization cycle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InfraSyncReport {
    pub created_count: usize,
    pub updated_count: usize,
    pub unchanged_count: usize,
    pub stale_count: usize,
    pub actions: Vec<InfraSyncAction>,
}

/// Engine that performs non-destructive merge operations between discovered infrastructure and local hosts.
pub struct InventoryMergeEngine;

impl InventoryMergeEngine {
    /// Evaluates discovered hosts against existing hosts under the specified merge policy.
    pub fn plan_merge(
        existing_hosts: &[ExistingHostRecord],
        discovered_hosts: &[DiscoveredHost],
        policy: MergePolicy,
    ) -> InfraSyncReport {
        let mut report = InfraSyncReport::default();
        let mut matched_existing_ids = std::collections::BTreeSet::new();

        for discovered in discovered_hosts {
            // Match first by explicit provider instance ID, then fallback to hostname + provider tag
            let matched = existing_hosts.iter().find(|h| {
                if let Some(ref ext_id) = h.external_provider_id {
                    ext_id == &discovered.id
                } else {
                    h.hostname.eq_ignore_ascii_case(&discovered.hostname)
                        || h.name.eq_ignore_ascii_case(&discovered.name)
                }
            });

            if let Some(existing) = matched {
                matched_existing_ids.insert(existing.id.clone());

                let hostname_changed =
                    !existing.hostname.eq_ignore_ascii_case(&discovered.hostname);
                let mut merged_tags = existing.tags.clone();

                if policy.preserve_user_tags {
                    for (k, v) in &discovered.tags {
                        let tag_str = if v.is_empty() {
                            k.clone()
                        } else {
                            format!("{k}:{v}")
                        };
                        if !merged_tags.iter().any(|t| t.eq_ignore_ascii_case(&tag_str)) {
                            merged_tags.push(tag_str);
                        }
                    }
                } else {
                    merged_tags = discovered
                        .tags
                        .iter()
                        .map(|(k, v)| {
                            if v.is_empty() {
                                k.clone()
                            } else {
                                format!("{k}:{v}")
                            }
                        })
                        .collect();
                }

                if hostname_changed || !discovered.is_running() {
                    report.updated_count += 1;
                    report.actions.push(InfraSyncAction::Update {
                        existing_id: existing.id.clone(),
                        discovered: discovered.clone(),
                        updated_hostname: hostname_changed,
                        updated_state: true,
                        merged_tags,
                    });
                } else {
                    report.unchanged_count += 1;
                    report.actions.push(InfraSyncAction::Unchanged {
                        existing_id: existing.id.clone(),
                        name: existing.name.clone(),
                    });
                }
            } else {
                report.created_count += 1;
                report
                    .actions
                    .push(InfraSyncAction::Create(discovered.clone()));
            }
        }

        // Check for stale hosts previously tagged with provider IDs from this provider kind
        if policy.detect_stale_hosts && !discovered_hosts.is_empty() {
            let provider_prefix = format!("{}:", discovered_hosts[0].provider.as_str());
            for existing in existing_hosts {
                if let Some(ref ext_id) = existing.external_provider_id {
                    if ext_id.starts_with(&provider_prefix)
                        && !matched_existing_ids.contains(&existing.id)
                    {
                        report.stale_count += 1;
                        report.actions.push(InfraSyncAction::Stale {
                            existing_id: existing.id.clone(),
                            name: existing.name.clone(),
                            external_id: ext_id.clone(),
                        });
                    }
                }
            }
        }

        report
    }
}

/// Errors occurring during infrastructure provider configuration, discovery, or inventory synchronization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfrastructureError {
    InvalidConfig(String),
    ParseError(String),
    MissingField(&'static str),
    MergeConflict(String),
}

impl fmt::Display for InfrastructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(msg) => write!(f, "infrastructure configuration error: {msg}"),
            Self::ParseError(msg) => write!(f, "infrastructure parsing error: {msg}"),
            Self::MissingField(field) => {
                write!(f, "required infrastructure field missing: {field}")
            }
            Self::MergeConflict(msg) => write!(f, "infrastructure merge conflict: {msg}"),
        }
    }
}

impl std::error::Error for InfrastructureError {}

#[cfg(test)]
mod tests;

//! Infrastructure integrations: Cloud providers (AWS, Azure, DigitalOcean) and Ansible inventory.
//!
//! Provides pure Rust domain models, response decoders, configuration builders,
//! and bidirectional Ansible inventory serialization with non-destructive merge logic.

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

/// Parser for AWS EC2 DescribeInstances JSON output.
pub struct AwsEc2Parser;

impl AwsEc2Parser {
    /// Decodes AWS EC2 DescribeInstances response JSON into standard DiscoveredHost models.
    pub fn parse_describe_instances_json(
        raw_json: &str,
        use_private_ip: bool,
    ) -> Result<Vec<DiscoveredHost>, InfrastructureError> {
        let val: serde_json::Value = serde_json::from_str(raw_json)
            .map_err(|e| InfrastructureError::ParseError(format!("invalid AWS EC2 JSON: {e}")))?;

        let reservations = val
            .get("Reservations")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                InfrastructureError::ParseError(
                    "missing 'Reservations' array in AWS EC2 output".to_string(),
                )
            })?;

        let mut results = Vec::new();

        for reservation in reservations {
            if let Some(instances) = reservation.get("Instances").and_then(|v| v.as_array()) {
                for inst in instances {
                    let instance_id =
                        inst.get("InstanceId")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                InfrastructureError::ParseError(
                                    "missing 'InstanceId' in EC2 instance".to_string(),
                                )
                            })?;

                    let state_name = inst
                        .get("State")
                        .and_then(|s| s.get("Name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    let instance_type = inst
                        .get("InstanceType")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);

                    let az = inst
                        .get("Placement")
                        .and_then(|p| p.get("AvailabilityZone"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    // Derive region by stripping the AZ letter suffix (e.g. us-east-1a -> us-east-1)
                    let region = if az.len() > 1 && az.chars().last().unwrap_or(' ').is_alphabetic()
                    {
                        let mut reg = az.to_string();
                        reg.pop();
                        Some(reg)
                    } else if !az.is_empty() {
                        Some(az.to_string())
                    } else {
                        None
                    };

                    let pub_ip = inst.get("PublicIpAddress").and_then(|v| v.as_str());
                    let priv_ip = inst.get("PrivateIpAddress").and_then(|v| v.as_str());
                    let pub_dns = inst.get("PublicDnsName").and_then(|v| v.as_str());

                    let chosen_ip = if use_private_ip {
                        priv_ip.or(pub_ip).or(pub_dns)
                    } else {
                        pub_ip.or(pub_dns).or(priv_ip)
                    };

                    let hostname = chosen_ip.unwrap_or("127.0.0.1").to_string();

                    let mut tags = BTreeMap::new();
                    let mut instance_name = String::new();

                    if let Some(tag_arr) = inst.get("Tags").and_then(|v| v.as_array()) {
                        for t in tag_arr {
                            let k = t.get("Key").and_then(|v| v.as_str()).unwrap_or("");
                            let v = t.get("Value").and_then(|v| v.as_str()).unwrap_or("");
                            if k.eq_ignore_ascii_case("name") {
                                instance_name = v.to_string();
                            }
                            if !k.is_empty() {
                                tags.insert(k.to_string(), v.to_string());
                            }
                        }
                    }

                    if instance_name.is_empty() {
                        instance_name = instance_id.to_string();
                    }

                    let mut extra = BTreeMap::new();
                    extra.insert("availability_zone".to_string(), az.to_string());
                    if let Some(priv_addr) = priv_ip {
                        extra.insert("private_ip".to_string(), priv_addr.to_string());
                    }
                    if let Some(pub_addr) = pub_ip {
                        extra.insert("public_ip".to_string(), pub_addr.to_string());
                    }
                    if let Some(key_name) = inst.get("KeyName").and_then(|v| v.as_str()) {
                        extra.insert("key_name".to_string(), key_name.to_string());
                    }

                    let group_path = if let Some(ref reg) = region {
                        if let Some(env) = tags.get("Environment").or_else(|| tags.get("env")) {
                            Some(format!("AWS/{reg}/{env}"))
                        } else {
                            Some(format!("AWS/{reg}"))
                        }
                    } else {
                        Some("AWS/Instances".to_string())
                    };

                    results.push(DiscoveredHost {
                        id: format!("aws:{instance_id}"),
                        name: instance_name,
                        provider: CloudProviderKind::Aws,
                        hostname,
                        port: 22,
                        username: Some(CloudProviderKind::Aws.default_username().to_string()),
                        state: state_name.to_string(),
                        region,
                        instance_type,
                        group_path,
                        tags,
                        extra_metadata: extra,
                    });
                }
            }
        }

        Ok(results)
    }
}

/// Parser for Azure Compute Virtual Machines REST/CLI JSON output.
pub struct AzureVmParser;

impl AzureVmParser {
    /// Decodes Azure VM list response JSON into standard DiscoveredHost models.
    pub fn parse_virtual_machines_json(
        raw_json: &str,
        use_private_ip: bool,
    ) -> Result<Vec<DiscoveredHost>, InfrastructureError> {
        let val: serde_json::Value = serde_json::from_str(raw_json)
            .map_err(|e| InfrastructureError::ParseError(format!("invalid Azure VM JSON: {e}")))?;

        // Support either top-level array or Azure resource list envelope (`{"value": [...]}`)
        let vm_list = if let Some(arr) = val.get("value").and_then(|v| v.as_array()) {
            arr
        } else if let Some(arr) = val.as_array() {
            arr
        } else {
            return Err(InfrastructureError::ParseError(
                "expected JSON array or 'value' array in Azure VM output".to_string(),
            ));
        };

        let mut results = Vec::new();

        for vm in vm_list {
            let id = vm
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown-azure-vm");

            let name = vm
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed VM")
                .to_string();

            let location = vm
                .get("location")
                .and_then(|v| v.as_str())
                .map(str::to_string);

            let properties = vm.get("properties");

            let vm_size = properties
                .and_then(|p| p.get("hardwareProfile"))
                .and_then(|h| h.get("vmSize"))
                .and_then(|v| v.as_str())
                .map(str::to_string);

            let admin_username = properties
                .and_then(|p| p.get("osProfile"))
                .and_then(|o| o.get("adminUsername"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| CloudProviderKind::Azure.default_username().to_string());

            let mut state = "running".to_string();
            if let Some(statuses) = properties
                .and_then(|p| p.get("instanceView"))
                .and_then(|iv| iv.get("statuses"))
                .and_then(|s| s.as_array())
            {
                for st in statuses {
                    if let Some(code) = st.get("code").and_then(|v| v.as_str()) {
                        if code.starts_with("PowerState/") {
                            state = code.trim_start_matches("PowerState/").to_string();
                            break;
                        }
                    }
                }
            }

            // Extract tags
            let mut tags = BTreeMap::new();
            if let Some(tag_map) = vm.get("tags").and_then(|v| v.as_object()) {
                for (k, v) in tag_map {
                    if let Some(v_str) = v.as_str() {
                        tags.insert(k.clone(), v_str.to_string());
                    }
                }
            }

            // Resolve IPs from network interfaces if embedded, or fallback to VM name
            let mut public_ip = None;
            let mut private_ip = None;

            if let Some(interfaces) = properties
                .and_then(|p| p.get("networkProfile"))
                .and_then(|n| n.get("networkInterfaces"))
                .and_then(|ni| ni.as_array())
            {
                for nic in interfaces {
                    if let Some(ip_configs) = nic
                        .get("properties")
                        .and_then(|p| p.get("ipConfigurations"))
                        .and_then(|c| c.as_array())
                    {
                        for cfg in ip_configs {
                            let priv_cand = cfg
                                .get("properties")
                                .and_then(|p| p.get("privateIPAddress"))
                                .and_then(|v| v.as_str());
                            if priv_cand.is_some() && private_ip.is_none() {
                                private_ip = priv_cand.map(str::to_string);
                            }

                            let pub_cand = cfg
                                .get("properties")
                                .and_then(|p| p.get("publicIPAddress"))
                                .and_then(|p| p.get("properties"))
                                .and_then(|p| p.get("ipAddress"))
                                .and_then(|v| v.as_str());
                            if pub_cand.is_some() && public_ip.is_none() {
                                public_ip = pub_cand.map(str::to_string);
                            }
                        }
                    }
                }
            }

            let hostname = if use_private_ip {
                private_ip.clone().or_else(|| public_ip.clone())
            } else {
                public_ip.clone().or_else(|| private_ip.clone())
            }
            .unwrap_or_else(|| name.clone());

            let mut extra = BTreeMap::new();
            extra.insert("azure_id".to_string(), id.to_string());
            if let Some(ref pub_addr) = public_ip {
                extra.insert("public_ip".to_string(), pub_addr.clone());
            }
            if let Some(ref priv_addr) = private_ip {
                extra.insert("private_ip".to_string(), priv_addr.clone());
            }

            let group_path = if let Some(ref loc) = location {
                Some(format!("Azure/{loc}"))
            } else {
                Some("Azure/VirtualMachines".to_string())
            };

            results.push(DiscoveredHost {
                id: format!("azure:{id}"),
                name,
                provider: CloudProviderKind::Azure,
                hostname,
                port: 22,
                username: Some(admin_username),
                state,
                region: location,
                instance_type: vm_size,
                group_path,
                tags,
                extra_metadata: extra,
            });
        }

        Ok(results)
    }
}

/// Parser for DigitalOcean Droplets API v2 JSON output.
pub struct DigitalOceanParser;

impl DigitalOceanParser {
    /// Decodes DigitalOcean Droplets list JSON into standard DiscoveredHost models.
    pub fn parse_droplets_json(
        raw_json: &str,
        use_private_ip: bool,
    ) -> Result<Vec<DiscoveredHost>, InfrastructureError> {
        let val: serde_json::Value = serde_json::from_str(raw_json).map_err(|e| {
            InfrastructureError::ParseError(format!("invalid DigitalOcean Droplet JSON: {e}"))
        })?;

        let droplets = val
            .get("droplets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                InfrastructureError::ParseError(
                    "missing 'droplets' array in DigitalOcean output".to_string(),
                )
            })?;

        let mut results = Vec::new();

        for drop in droplets {
            let id = drop.get("id").and_then(|v| v.as_i64()).ok_or_else(|| {
                InfrastructureError::ParseError("missing 'id' on DigitalOcean droplet".to_string())
            })?;

            let name = drop
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed-droplet")
                .to_string();

            let status = drop
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("active")
                .to_string();

            let region_slug = drop
                .get("region")
                .and_then(|r| r.get("slug"))
                .and_then(|v| v.as_str())
                .map(str::to_string);

            let memory = drop.get("memory").and_then(|v| v.as_i64()).unwrap_or(0);
            let vcpus = drop.get("vcpus").and_then(|v| v.as_i64()).unwrap_or(1);
            let instance_type = Some(format!("{vcpus}vCPU-{memory}MB"));

            // Parse IPv4 networks
            let mut public_ip = None;
            let mut private_ip = None;

            if let Some(v4_nets) = drop
                .get("networks")
                .and_then(|n| n.get("v4"))
                .and_then(|v| v.as_array())
            {
                for net in v4_nets {
                    let ip = net.get("ip_address").and_then(|v| v.as_str());
                    let net_type = net.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if net_type.eq_ignore_ascii_case("public") && public_ip.is_none() {
                        public_ip = ip.map(str::to_string);
                    } else if net_type.eq_ignore_ascii_case("private") && private_ip.is_none() {
                        private_ip = ip.map(str::to_string);
                    }
                }
            }

            let hostname = if use_private_ip {
                private_ip.clone().or_else(|| public_ip.clone())
            } else {
                public_ip.clone().or_else(|| private_ip.clone())
            }
            .unwrap_or_else(|| name.clone());

            let mut tags = BTreeMap::new();
            if let Some(tag_arr) = drop.get("tags").and_then(|v| v.as_array()) {
                for t in tag_arr {
                    if let Some(t_str) = t.as_str() {
                        tags.insert(t_str.to_string(), String::new());
                    }
                }
            }

            let mut extra = BTreeMap::new();
            extra.insert("droplet_id".to_string(), id.to_string());
            if let Some(ref pub_addr) = public_ip {
                extra.insert("public_ip".to_string(), pub_addr.clone());
            }
            if let Some(ref priv_addr) = private_ip {
                extra.insert("private_ip".to_string(), priv_addr.clone());
            }

            let group_path = if let Some(ref reg) = region_slug {
                Some(format!("DigitalOcean/{reg}"))
            } else {
                Some("DigitalOcean/Droplets".to_string())
            };

            results.push(DiscoveredHost {
                id: format!("do:{id}"),
                name,
                provider: CloudProviderKind::DigitalOcean,
                hostname,
                port: 22,
                username: Some(
                    CloudProviderKind::DigitalOcean
                        .default_username()
                        .to_string(),
                ),
                state: status,
                region: region_slug,
                instance_type,
                group_path,
                tags,
                extra_metadata: extra,
            });
        }

        Ok(results)
    }
}

/// Simplified host data for Ansible export.
#[derive(Debug, Clone)]
pub struct HostExportItem {
    pub name: String,
    pub hostname: String,
    pub port: u16,
    pub username: Option<String>,
    pub group_name: Option<String>,
    pub tags: Vec<String>,
}

/// Bidirectional Ansible inventory engine supporting INI and YAML parsing and export.
pub struct AnsibleInventoryEngine;

impl AnsibleInventoryEngine {
    /// Parses an Ansible inventory in INI format into DiscoveredHost records.
    pub fn parse_ini_inventory(content: &str) -> Result<Vec<DiscoveredHost>, InfrastructureError> {
        let mut results = Vec::new();
        let mut current_group = "all".to_string();
        let mut group_vars: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let section = &trimmed[1..trimmed.len() - 1].trim();
                current_group = section.to_string();
                continue;
            }

            // Handle [group:vars]
            if current_group.ends_with(":vars") {
                let grp_name = current_group.trim_end_matches(":vars").to_string();
                if let Some((k, v)) = trimmed.split_once('=') {
                    group_vars
                        .entry(grp_name)
                        .or_default()
                        .insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
                }
                continue;
            }

            // Skip [group:children] section headers in host parsing
            if current_group.ends_with(":children") {
                continue;
            }

            // Parse host line: `web1 ansible_host=192.168.1.10 ansible_user=deploy ansible_port=2222`
            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            let host_identifier = tokens[0];
            let mut hostname = host_identifier.to_string();
            let mut port = 22u16;
            let mut username = None;
            let mut tags = BTreeMap::new();
            let mut extra = BTreeMap::new();

            // Inherit group variables if any
            if let Some(vars) = group_vars.get(&current_group) {
                if let Some(h) = vars.get("ansible_host") {
                    hostname = h.clone();
                }
                if let Some(u) = vars.get("ansible_user") {
                    username = Some(u.clone());
                }
                if let Some(p) = vars.get("ansible_port").and_then(|p| p.parse::<u16>().ok()) {
                    port = p;
                }
            }

            for token in &tokens[1..] {
                if let Some((k, v)) = token.split_once('=') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('"');
                    match key {
                        "ansible_host" => hostname = val.to_string(),
                        "ansible_port" => {
                            if let Ok(p) = val.parse::<u16>() {
                                port = p;
                            }
                        }
                        "ansible_user" => username = Some(val.to_string()),
                        "ansible_ssh_private_key_file" => {
                            extra.insert("key_file".to_string(), val.to_string());
                        }
                        other => {
                            tags.insert(other.to_string(), val.to_string());
                        }
                    }
                }
            }

            let group_path = if current_group != "all" && !current_group.is_empty() {
                Some(format!("Ansible/{current_group}"))
            } else {
                Some("Ansible/Inventory".to_string())
            };

            results.push(DiscoveredHost {
                id: format!("ansible:{host_identifier}"),
                name: host_identifier.to_string(),
                provider: CloudProviderKind::Ansible,
                hostname,
                port,
                username,
                state: "active".to_string(),
                region: None,
                instance_type: None,
                group_path,
                tags,
                extra_metadata: extra,
            });
        }

        Ok(results)
    }

    /// Parses an Ansible inventory in YAML format into DiscoveredHost records.
    pub fn parse_yaml_inventory(content: &str) -> Result<Vec<DiscoveredHost>, InfrastructureError> {
        let val: serde_json::Value = serde_yaml_minimal_parse(content)?;
        let mut results = Vec::new();

        fn extract_hosts(
            val: &serde_json::Value,
            current_group: &str,
            out: &mut Vec<DiscoveredHost>,
        ) {
            if let Some(hosts_map) = val.get("hosts").and_then(|h| h.as_object()) {
                for (host_name, host_val) in hosts_map {
                    let mut hostname = host_name.clone();
                    let mut port = 22u16;
                    let mut username = None;
                    let mut tags = BTreeMap::new();
                    let extra = BTreeMap::new();

                    if let Some(host_obj) = host_val.as_object() {
                        if let Some(h) = host_obj.get("ansible_host").and_then(|v| v.as_str()) {
                            hostname = h.to_string();
                        }
                        if let Some(p) = host_obj.get("ansible_port").and_then(|v| v.as_u64()) {
                            port = p as u16;
                        }
                        if let Some(u) = host_obj.get("ansible_user").and_then(|v| v.as_str()) {
                            username = Some(u.to_string());
                        }
                        for (k, v) in host_obj {
                            if !k.starts_with("ansible_") {
                                if let Some(v_str) = v.as_str() {
                                    tags.insert(k.clone(), v_str.to_string());
                                }
                            }
                        }
                    }

                    let group_path = if !current_group.is_empty() && current_group != "all" {
                        Some(format!("Ansible/{current_group}"))
                    } else {
                        Some("Ansible/Inventory".to_string())
                    };

                    out.push(DiscoveredHost {
                        id: format!("ansible:{host_name}"),
                        name: host_name.clone(),
                        provider: CloudProviderKind::Ansible,
                        hostname,
                        port,
                        username,
                        state: "active".to_string(),
                        region: None,
                        instance_type: None,
                        group_path,
                        tags,
                        extra_metadata: extra,
                    });
                }
            }

            if let Some(children) = val.get("children").and_then(|c| c.as_object()) {
                for (child_name, child_val) in children {
                    extract_hosts(child_val, child_name, out);
                }
            }
        }

        if let Some(all_node) = val.get("all") {
            extract_hosts(all_node, "all", &mut results);
        } else {
            extract_hosts(&val, "all", &mut results);
        }

        Ok(results)
    }

    /// Serializes a set of hosts into Ansible INI inventory format.
    pub fn export_to_ini(hosts: &[HostExportItem]) -> String {
        let mut grouped: BTreeMap<String, Vec<&HostExportItem>> = BTreeMap::new();

        for host in hosts {
            let grp = host
                .group_name
                .as_deref()
                .unwrap_or("ungrouped")
                .to_string();
            grouped.entry(grp).or_default().push(host);
        }

        let mut out = String::new();
        out.push_str("# Shellumi Exported Ansible Inventory (INI format)\n\n");

        for (group, group_hosts) in grouped {
            out.push_str(&format!("[{group}]\n"));
            for h in group_hosts {
                let mut line = format!("{} ansible_host={}", h.name, h.hostname);
                if h.port != 22 {
                    line.push_str(&format!(" ansible_port={}", h.port));
                }
                if let Some(ref u) = h.username {
                    line.push_str(&format!(" ansible_user={u}"));
                }
                for tag in &h.tags {
                    if let Some((k, v)) = tag.split_once(':') {
                        line.push_str(&format!(" {k}={v}"));
                    }
                }
                out.push_str(&line);
                out.push('\n');
            }
            out.push('\n');
        }

        out
    }

    /// Serializes a set of hosts into Ansible YAML inventory format.
    pub fn export_to_yaml(hosts: &[HostExportItem]) -> String {
        let mut grouped: BTreeMap<String, Vec<&HostExportItem>> = BTreeMap::new();

        for host in hosts {
            let grp = host
                .group_name
                .as_deref()
                .unwrap_or("ungrouped")
                .to_string();
            grouped.entry(grp).or_default().push(host);
        }

        let mut out = String::new();
        out.push_str("# Shellumi Exported Ansible Inventory (YAML format)\n");
        out.push_str("---\n");
        out.push_str("all:\n");
        out.push_str("  children:\n");

        for (group, group_hosts) in grouped {
            out.push_str(&format!("    {group}:\n"));
            out.push_str("      hosts:\n");
            for h in group_hosts {
                out.push_str(&format!("        {}:\n", h.name));
                out.push_str(&format!("          ansible_host: \"{}\"\n", h.hostname));
                if h.port != 22 {
                    out.push_str(&format!("          ansible_port: {}\n", h.port));
                }
                if let Some(ref u) = h.username {
                    out.push_str(&format!("          ansible_user: \"{u}\"\n"));
                }
                for tag in &h.tags {
                    if let Some((k, v)) = tag.split_once(':') {
                        out.push_str(&format!("          {k}: \"{v}\"\n"));
                    }
                }
            }
        }

        out
    }
}

/// Minimal safe YAML parser supporting standard Ansible inventory structures without heavy external crates.
fn serde_yaml_minimal_parse(content: &str) -> Result<serde_json::Value, InfrastructureError> {
    let mut root = serde_json::Map::new();
    let mut all_map = serde_json::Map::new();
    let mut children_map = serde_json::Map::new();
    let mut current_child_group: Option<String> = None;
    let mut current_hosts_map = serde_json::Map::new();
    let mut current_host_name: Option<String> = None;
    let mut current_host_props = serde_json::Map::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("---") {
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        if indent <= 2 && (trimmed.starts_with("all:") || trimmed.starts_with("children:")) {
            continue;
        }

        // Group level (indent ~ 4)
        if indent == 4 && trimmed.ends_with(':') {
            if let Some(ch_name) = current_child_group.take() {
                if let Some(h_name) = current_host_name.take() {
                    current_hosts_map.insert(
                        h_name,
                        serde_json::Value::Object(current_host_props.clone()),
                    );
                    current_host_props.clear();
                }
                let mut grp_obj = serde_json::Map::new();
                grp_obj.insert(
                    "hosts".to_string(),
                    serde_json::Value::Object(current_hosts_map.clone()),
                );
                current_hosts_map.clear();
                children_map.insert(ch_name, serde_json::Value::Object(grp_obj));
            }
            let grp = trimmed.trim_end_matches(':').trim().to_string();
            current_child_group = Some(grp);
            continue;
        }

        // Host header (indent ~ 8)
        if indent == 8 && trimmed.ends_with(':') {
            if let Some(h_name) = current_host_name.take() {
                current_hosts_map.insert(
                    h_name,
                    serde_json::Value::Object(current_host_props.clone()),
                );
                current_host_props.clear();
            }
            let h = trimmed.trim_end_matches(':').trim().to_string();
            current_host_name = Some(h);
            continue;
        }

        // Host properties (indent >= 10)
        if indent >= 10 {
            if let Some((k, v)) = trimmed.split_once(':') {
                let key = k.trim();
                let val_str = v.trim().trim_matches('"').trim_matches('\'');
                if let Ok(num) = val_str.parse::<i64>() {
                    current_host_props
                        .insert(key.to_string(), serde_json::Value::Number(num.into()));
                } else {
                    current_host_props.insert(
                        key.to_string(),
                        serde_json::Value::String(val_str.to_string()),
                    );
                }
            }
        }
    }

    if let Some(ch_name) = current_child_group.take() {
        if let Some(h_name) = current_host_name.take() {
            current_hosts_map.insert(h_name, serde_json::Value::Object(current_host_props));
        }
        let mut grp_obj = serde_json::Map::new();
        grp_obj.insert(
            "hosts".to_string(),
            serde_json::Value::Object(current_hosts_map),
        );
        children_map.insert(ch_name, serde_json::Value::Object(grp_obj));
    }

    all_map.insert(
        "children".to_string(),
        serde_json::Value::Object(children_map),
    );
    root.insert("all".to_string(), serde_json::Value::Object(all_map));

    Ok(serde_json::Value::Object(root))
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
mod tests {
    use super::*;

    #[test]
    fn parses_aws_ec2_describe_instances_json_correctly() {
        let fixture = r#"{
            "Reservations": [
                {
                    "ReservationId": "r-0123456789",
                    "Instances": [
                        {
                            "InstanceId": "i-0a1b2c3d4e5f60718",
                            "InstanceType": "t3.medium",
                            "State": { "Name": "running" },
                            "PublicIpAddress": "54.210.10.20",
                            "PrivateIpAddress": "10.0.1.55",
                            "PublicDnsName": "ec2-54-210-10-20.compute-1.amazonaws.com",
                            "KeyName": "prod-ssh-bastion",
                            "Placement": { "AvailabilityZone": "us-east-1b" },
                            "Tags": [
                                { "Key": "Name", "Value": "api-gateway-01" },
                                { "Key": "Environment", "Value": "Production" },
                                { "Key": "Team", "Value": "Backend" }
                            ]
                        },
                        {
                            "InstanceId": "i-0987654321fedcba0",
                            "InstanceType": "c6i.xlarge",
                            "State": { "Name": "stopped" },
                            "PrivateIpAddress": "10.0.2.14",
                            "Placement": { "AvailabilityZone": "us-east-1c" },
                            "Tags": [
                                { "Key": "Name", "Value": "batch-worker-09" }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let hosts = AwsEc2Parser::parse_describe_instances_json(fixture, false).unwrap();
        assert_eq!(hosts.len(), 2);

        let h1 = &hosts[0];
        assert_eq!(h1.id, "aws:i-0a1b2c3d4e5f60718");
        assert_eq!(h1.name, "api-gateway-01");
        assert_eq!(h1.provider, CloudProviderKind::Aws);
        assert_eq!(h1.hostname, "54.210.10.20");
        assert_eq!(h1.username.as_deref(), Some("ec2-user"));
        assert!(h1.is_running());
        assert_eq!(h1.region.as_deref(), Some("us-east-1"));
        assert_eq!(h1.instance_type.as_deref(), Some("t3.medium"));
        assert_eq!(h1.group_path.as_deref(), Some("AWS/us-east-1/Production"));
        assert_eq!(h1.tags.get("Team").map(String::as_str), Some("Backend"));

        let h2 = &hosts[1];
        assert_eq!(h2.id, "aws:i-0987654321fedcba0");
        assert_eq!(h2.name, "batch-worker-09");
        assert_eq!(h2.hostname, "10.0.2.14"); // Fallback to private IP
        assert!(!h2.is_running());
        assert_eq!(h2.status_badge(), "🔴 Stopped");
    }

    #[test]
    fn parses_azure_vm_json_correctly() {
        let fixture = r#"{
            "value": [
                {
                    "id": "/subscriptions/sub-1/resourceGroups/rg-prod/providers/Microsoft.Compute/virtualMachines/vm-frontend-01",
                    "name": "vm-frontend-01",
                    "location": "eastus",
                    "tags": { "env": "prod", "tier": "web" },
                    "properties": {
                        "hardwareProfile": { "vmSize": "Standard_B2s" },
                        "osProfile": { "adminUsername": "azureuser" },
                        "instanceView": {
                            "statuses": [
                                { "code": "PowerState/running", "displayStatus": "VM running" }
                            ]
                        },
                        "networkProfile": {
                            "networkInterfaces": [
                                {
                                    "properties": {
                                        "ipConfigurations": [
                                            {
                                                "properties": {
                                                    "publicIPAddress": {
                                                        "properties": { "ipAddress": "20.50.100.8" }
                                                    },
                                                    "privateIPAddress": "10.1.0.4"
                                                }
                                            }
                                        ]
                                    }
                                }
                            ]
                        }
                    }
                }
            ]
        }"#;

        let hosts = AzureVmParser::parse_virtual_machines_json(fixture, false).unwrap();
        assert_eq!(hosts.len(), 1);

        let h = &hosts[0];
        assert_eq!(h.name, "vm-frontend-01");
        assert_eq!(h.provider, CloudProviderKind::Azure);
        assert_eq!(h.hostname, "20.50.100.8");
        assert_eq!(h.username.as_deref(), Some("azureuser"));
        assert_eq!(h.instance_type.as_deref(), Some("Standard_B2s"));
        assert_eq!(h.region.as_deref(), Some("eastus"));
        assert_eq!(h.tags.get("tier").map(String::as_str), Some("web"));
    }

    #[test]
    fn parses_digitalocean_droplets_json_correctly() {
        let fixture = r#"{
            "droplets": [
                {
                    "id": 4182910,
                    "name": "fra1-k8s-master-01",
                    "memory": 2048,
                    "vcpus": 2,
                    "disk": 50,
                    "region": { "slug": "fra1", "name": "Frankfurt 1" },
                    "status": "active",
                    "tags": ["kubernetes", "master", "europe"],
                    "networks": {
                        "v4": [
                            { "ip_address": "159.65.120.4", "type": "public" },
                            { "ip_address": "10.135.0.2", "type": "private" }
                        ]
                    }
                }
            ]
        }"#;

        let hosts = DigitalOceanParser::parse_droplets_json(fixture, false).unwrap();
        assert_eq!(hosts.len(), 1);

        let h = &hosts[0];
        assert_eq!(h.id, "do:4182910");
        assert_eq!(h.name, "fra1-k8s-master-01");
        assert_eq!(h.provider, CloudProviderKind::DigitalOcean);
        assert_eq!(h.hostname, "159.65.120.4");
        assert_eq!(h.username.as_deref(), Some("root"));
        assert_eq!(h.region.as_deref(), Some("fra1"));
        assert_eq!(h.instance_type.as_deref(), Some("2vCPU-2048MB"));
        assert!(h.tags.contains_key("kubernetes"));
    }

    #[test]
    fn parses_and_exports_ansible_ini_inventory() {
        let ini_fixture = r#"
# Production Ansible Inventory
[webservers]
web1.example.com ansible_host=192.168.1.10 ansible_port=2202 ansible_user=deployer tier=frontend
web2.example.com ansible_host=192.168.1.11 ansible_user=deployer

[dbservers]
db1.internal ansible_host=10.0.5.10 ansible_user=dbadmin role=primary

[webservers:vars]
http_port=80
"#;

        let hosts = AnsibleInventoryEngine::parse_ini_inventory(ini_fixture).unwrap();
        assert_eq!(hosts.len(), 3);

        let w1 = &hosts[0];
        assert_eq!(w1.name, "web1.example.com");
        assert_eq!(w1.hostname, "192.168.1.10");
        assert_eq!(w1.port, 2202);
        assert_eq!(w1.username.as_deref(), Some("deployer"));
        assert_eq!(w1.group_path.as_deref(), Some("Ansible/webservers"));
        assert_eq!(w1.tags.get("tier").map(String::as_str), Some("frontend"));

        // Roundtrip test export
        let items: Vec<HostExportItem> = hosts
            .iter()
            .map(|h| HostExportItem {
                name: h.name.clone(),
                hostname: h.hostname.clone(),
                port: h.port,
                username: h.username.clone(),
                group_name: Some("webservers".to_string()),
                tags: vec!["env:prod".to_string()],
            })
            .collect();

        let exported = AnsibleInventoryEngine::export_to_ini(&items);
        assert!(exported.contains("[webservers]"));
        assert!(exported.contains("ansible_host=192.168.1.10"));
        assert!(exported.contains("ansible_port=2202"));
    }

    #[test]
    fn parses_and_exports_ansible_yaml_inventory() {
        let yaml_fixture = r#"
---
all:
  children:
    appservers:
      hosts:
        app01:
          ansible_host: "10.0.1.100"
          ansible_port: 22
          ansible_user: "appuser"
          env: "staging"
"#;

        let hosts = AnsibleInventoryEngine::parse_yaml_inventory(yaml_fixture).unwrap();
        assert_eq!(hosts.len(), 1);

        let h = &hosts[0];
        assert_eq!(h.name, "app01");
        assert_eq!(h.hostname, "10.0.1.100");
        assert_eq!(h.username.as_deref(), Some("appuser"));
        assert_eq!(h.tags.get("env").map(String::as_str), Some("staging"));

        let export_items = vec![HostExportItem {
            name: "app01".to_string(),
            hostname: "10.0.1.100".to_string(),
            port: 22,
            username: Some("appuser".to_string()),
            group_name: Some("appservers".to_string()),
            tags: vec!["env:staging".to_string()],
        }];

        let exported_yaml = AnsibleInventoryEngine::export_to_yaml(&export_items);
        assert!(exported_yaml.contains("appservers:"));
        assert!(exported_yaml.contains("ansible_host: \"10.0.1.100\""));
    }

    #[test]
    fn merges_inventory_preserving_user_tags_and_detecting_stale_hosts() {
        let existing = vec![
            ExistingHostRecord {
                id: "host-1".to_string(),
                name: "prod-web-01".to_string(),
                hostname: "54.200.1.1".to_string(),
                port: 22,
                username: Some("admin".to_string()),
                group_id: None,
                tags: vec!["custom:vip".to_string(), "tier:web".to_string()],
                notes: Some("Crucial gateway".to_string()),
                startup_snippet_id: None,
                external_provider_id: Some("aws:i-1111".to_string()),
            },
            ExistingHostRecord {
                id: "host-2".to_string(),
                name: "decommissioned-worker".to_string(),
                hostname: "54.200.1.2".to_string(),
                port: 22,
                username: Some("ec2-user".to_string()),
                group_id: None,
                tags: vec![],
                notes: None,
                startup_snippet_id: None,
                external_provider_id: Some("aws:i-9999".to_string()),
            },
        ];

        let discovered = vec![
            // Updated host with new IP and new tags
            DiscoveredHost {
                id: "aws:i-1111".to_string(),
                name: "prod-web-01".to_string(),
                provider: CloudProviderKind::Aws,
                hostname: "54.200.1.100".to_string(), // New dynamic IP
                port: 22,
                username: Some("ec2-user".to_string()),
                state: "running".to_string(),
                region: Some("us-west-2".to_string()),
                instance_type: Some("t3.small".to_string()),
                group_path: Some("AWS/us-west-2".to_string()),
                tags: {
                    let mut m = BTreeMap::new();
                    m.insert("role".to_string(), "api".to_string());
                    m
                },
                extra_metadata: BTreeMap::new(),
            },
            // Brand new discovered host
            DiscoveredHost {
                id: "aws:i-2222".to_string(),
                name: "prod-db-01".to_string(),
                provider: CloudProviderKind::Aws,
                hostname: "10.0.5.20".to_string(),
                port: 22,
                username: Some("ec2-user".to_string()),
                state: "running".to_string(),
                region: Some("us-west-2".to_string()),
                instance_type: Some("r6i.large".to_string()),
                group_path: Some("AWS/us-west-2".to_string()),
                tags: BTreeMap::new(),
                extra_metadata: BTreeMap::new(),
            },
        ];

        let report =
            InventoryMergeEngine::plan_merge(&existing, &discovered, MergePolicy::default());

        assert_eq!(report.created_count, 1);
        assert_eq!(report.updated_count, 1);
        assert_eq!(report.stale_count, 1);

        // Verify custom tags preserved during update
        let update_action = report
            .actions
            .iter()
            .find(|a| matches!(a, InfraSyncAction::Update { .. }))
            .unwrap();
        if let InfraSyncAction::Update {
            ref merged_tags,
            updated_hostname,
            ..
        } = update_action
        {
            assert!(updated_hostname);
            assert!(merged_tags.contains(&"custom:vip".to_string()));
            assert!(merged_tags.contains(&"role:api".to_string()));
        } else {
            panic!("expected InfraSyncAction::Update");
        }
    }
}

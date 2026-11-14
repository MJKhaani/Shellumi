//! Digitalocean inventory support.
use super::*;

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

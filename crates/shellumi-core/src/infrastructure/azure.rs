//! Azure inventory support.
use super::*;

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

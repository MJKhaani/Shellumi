//! Ansible inventory support.
use super::*;

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

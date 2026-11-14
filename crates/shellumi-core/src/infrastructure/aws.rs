//! Aws inventory support.
use super::*;

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

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

    let report = InventoryMergeEngine::plan_merge(&existing, &discovered, MergePolicy::default());

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

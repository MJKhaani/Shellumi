#[test]
fn verifies_infrastructure_palette_actions_and_categories() {
    use shellumi_core::{PaletteAction, PaletteCategory, PaletteRegistry};

    let registry = PaletteRegistry::with_static_actions();
    let infra_commands: Vec<_> = registry
        .commands()
        .iter()
        .filter(|c| c.category == PaletteCategory::Infrastructure)
        .collect();

    assert_eq!(infra_commands.len(), 4);
    assert!(infra_commands
        .iter()
        .any(|c| c.action == PaletteAction::OpenInfrastructureManager));
    assert!(infra_commands
        .iter()
        .any(|c| c.action == PaletteAction::SyncCloudInfrastructure));
    assert!(infra_commands
        .iter()
        .any(|c| c.action == PaletteAction::ImportAnsibleInventory));
    assert!(infra_commands
        .iter()
        .any(|c| c.action == PaletteAction::ExportAnsibleInventory));

    assert_eq!(PaletteCategory::Infrastructure.as_str(), "Infrastructure");
    assert_eq!(PaletteCategory::Infrastructure.badge_icon(), "☁️");
}

#[test]
fn verifies_infrastructure_cloud_provider_and_discovered_host_models() {
    use shellumi_core::{CloudProviderConfig, CloudProviderKind, DiscoveredHost};
    use shellumi_storage::{CloudProviderRecord, DiscoveredHostRecord};
    use std::collections::BTreeMap;

    let mut config = CloudProviderConfig::new(CloudProviderKind::Aws, "Prod AWS");
    config.region = Some("us-west-2".into());
    config.access_key_id = Some("AKIAIOSFODNN7EXAMPLE".into());
    config.secret_key = Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into());

    assert!(config.validate().is_ok());
    let redacted = config.redacted_summary();
    assert!(redacted.contains("us-west-2"));
    assert!(!redacted.contains("wJalrXUtnFEMI"));

    let provider_rec = CloudProviderRecord {
        id: "1".into(),
        name: "Prod AWS".into(),
        provider_kind: "aws".into(),
        config_json: serde_json::to_string(&config).unwrap(),
        sync_interval_mins: 60,
        last_synced_at: Some(1730000000),
        auto_sync: true,
        created_at: 1730000000,
        updated_at: 1730000000,
    };
    assert_eq!(provider_rec.name, "Prod AWS");

    let disc_host = DiscoveredHost {
        id: "i-0123456789abcdef0".into(),
        name: "web-server-01".into(),
        provider: CloudProviderKind::Aws,
        hostname: "54.214.1.100".into(),
        port: 22,
        username: Some("ec2-user".into()),
        state: "running".into(),
        region: Some("us-west-2".into()),
        instance_type: Some("t3.medium".into()),
        group_path: Some("AWS/us-west-2".into()),
        tags: BTreeMap::new(),
        extra_metadata: BTreeMap::new(),
    };

    assert!(disc_host.is_running());
    assert_eq!(disc_host.status_badge(), "🟢 Running");
    assert_eq!(disc_host.format_destination(), "ec2-user@54.214.1.100");

    let disc_rec = DiscoveredHostRecord {
        id: "10".into(),
        provider_id: "1".into(),
        external_id: disc_host.id.clone(),
        name: disc_host.name.clone(),
        hostname: disc_host.hostname.clone(),
        port: disc_host.port as i64,
        username: disc_host.username.clone(),
        state: disc_host.state.clone(),
        region: disc_host.region.clone(),
        group_path: disc_host.group_path.clone(),
        tags_json: serde_json::to_string(&disc_host.tags).unwrap(),
        metadata_json: serde_json::to_string(&disc_host.extra_metadata).unwrap(),
        imported_host_id: None,
        last_seen_at: 1730000000,
    };
    assert_eq!(disc_rec.external_id, "i-0123456789abcdef0");
    assert!(disc_rec.imported_host_id.is_none());
}

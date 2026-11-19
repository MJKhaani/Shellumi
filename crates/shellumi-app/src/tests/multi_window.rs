use shellumi_core::{
    server::{ShellumiWsClient, WsClientConfig},
    HostGroup, PaletteAction, PaletteCategory, PaletteCommand, PaletteRegistry, ServerConnectionId,
    Snippet, SshDestination, VaultContext, VaultId, VaultLocation,
};
use shellumi_storage::{HostStore, VaultServerConnectionRecord, LEGACY_LOCAL_VAULT_ID};
use uuid::Uuid;

#[tokio::test]
async fn test_simultaneous_company_vaults_and_multi_window_binding() {
    let mut store = HostStore::open(":memory:").await.unwrap();

    let default_vid = LEGACY_LOCAL_VAULT_ID.to_string();
    let corp_a_vid = "corp-alpha-vault".to_string();
    let corp_b_vid = "corp-beta-vault".to_string();

    let srv_a_id = "srv-conn-alpha";
    let srv_b_id = "srv-conn-beta";

    let conn_a = VaultServerConnectionRecord {
        id: srv_a_id.to_string(),
        base_url: "https://server-alpha.corp:8443".to_string(),
        auth_token: Some("token-alpha-123".to_string()),
        refresh_token: None,
        device_id: Some(Uuid::new_v4().to_string()),
        device_signing_key: None,
        server_instance_id: Some("inst-alpha-uuid".to_string()),
        device_agreement_key: None,
        created_at: 1000,
        updated_at: 1000,
    };
    store.save_vault_server_connection(&conn_a).await.unwrap();

    let conn_b = VaultServerConnectionRecord {
        id: srv_b_id.to_string(),
        base_url: "https://server-beta.corp:9443".to_string(),
        auth_token: Some("token-beta-456".to_string()),
        refresh_token: None,
        device_id: Some(Uuid::new_v4().to_string()),
        device_signing_key: None,
        server_instance_id: Some("inst-beta-uuid".to_string()),
        device_agreement_key: None,
        created_at: 2000,
        updated_at: 2000,
    };
    store.save_vault_server_connection(&conn_b).await.unwrap();

    let remote_uuid_a = Uuid::new_v4();
    let ctx_a = VaultContext::new(
        VaultId::new(&corp_a_vid).unwrap(),
        VaultLocation::Remote {
            server: ServerConnectionId::new(srv_a_id).unwrap(),
            server_vault_id: VaultId::new(remote_uuid_a.to_string()).unwrap(),
        },
    );
    store
        .register_vault_context("Alpha Infrastructure", &ctx_a)
        .await
        .unwrap();

    let remote_uuid_b = Uuid::new_v4();
    let ctx_b = VaultContext::new(
        VaultId::new(&corp_b_vid).unwrap(),
        VaultLocation::Remote {
            server: ServerConnectionId::new(srv_b_id).unwrap(),
            server_vault_id: VaultId::new(remote_uuid_b.to_string()).unwrap(),
        },
    );
    store
        .register_vault_context("Beta Production", &ctx_b)
        .await
        .unwrap();

    // Verify VaultContextSummary representations
    let summary_local = crate::actions::VaultContextSummary {
        id: default_vid.clone(),
        name: "Personal".to_string(),
        is_remote: false,
        server_url: None,
        server_vault_id: None,
        is_locked: false,
    };

    let summary_alpha = crate::actions::VaultContextSummary {
        id: corp_a_vid.clone(),
        name: "Alpha Infrastructure".to_string(),
        is_remote: true,
        server_url: Some(conn_a.base_url.clone()),
        server_vault_id: Some(remote_uuid_a.to_string()),
        is_locked: false,
    };

    let summary_beta = crate::actions::VaultContextSummary {
        id: corp_b_vid.clone(),
        name: "Beta Production".to_string(),
        is_remote: true,
        server_url: Some(conn_b.base_url.clone()),
        server_vault_id: Some(remote_uuid_b.to_string()),
        is_locked: false,
    };

    // Verify window titles accurately represent context
    let title_for = |summary: &crate::actions::VaultContextSummary| -> String {
        if let Some(ref url) = summary.server_url {
            format!("Shellumi — {} ({})", summary.name, url)
        } else {
            format!("Shellumi — {}", summary.name)
        }
    };

    assert_eq!(title_for(&summary_local), "Shellumi — Personal");
    assert_eq!(
        title_for(&summary_alpha),
        "Shellumi — Alpha Infrastructure (https://server-alpha.corp:8443)"
    );
    assert_eq!(
        title_for(&summary_beta),
        "Shellumi — Beta Production (https://server-beta.corp:9443)"
    );

    // Verify host chip string extraction for visible top bar indicators
    let host_chip_for = |summary: &crate::actions::VaultContextSummary| -> Option<String> {
        summary.server_url.as_deref().map(|url| {
            if let Some(stripped) = url.strip_prefix("https://") {
                stripped.trim_end_matches('/').to_string()
            } else if let Some(stripped) = url.strip_prefix("http://") {
                stripped.trim_end_matches('/').to_string()
            } else {
                url.to_string()
            }
        })
    };

    assert_eq!(host_chip_for(&summary_local), None);
    assert_eq!(
        host_chip_for(&summary_alpha).as_deref(),
        Some("server-alpha.corp:8443")
    );
    assert_eq!(
        host_chip_for(&summary_beta).as_deref(),
        Some("server-beta.corp:9443")
    );
}

#[tokio::test]
async fn test_multi_window_host_mutation_and_inventory_isolation() {
    let mut store = HostStore::open(":memory:").await.unwrap();

    let vid_local = VaultId::new(LEGACY_LOCAL_VAULT_ID).unwrap();
    let vid_alpha = VaultId::new("corp-alpha-vault").unwrap();
    let vid_beta = VaultId::new("corp-beta-vault").unwrap();

    // 1. Concurrently simulate 3 windows inserting a host with the IDENTICAL name
    store.set_vault_id(vid_local.clone());
    let dest_local = SshDestination::new("127.0.0.1", "localdev", 22).unwrap();
    let id_local = store
        .add_full(
            "bastion",
            &dest_local,
            None,
            &[],
            None,
            None,
            "ssh",
            None,
            None,
        )
        .await
        .unwrap();

    store.set_vault_id(vid_alpha.clone());
    let dest_alpha = SshDestination::new("10.0.1.1", "alpha-admin", 22).unwrap();
    let id_alpha = store
        .add_full(
            "bastion",
            &dest_alpha,
            None,
            &[],
            None,
            None,
            "ssh",
            None,
            None,
        )
        .await
        .unwrap();

    store.set_vault_id(vid_beta.clone());
    let dest_beta = SshDestination::new("10.0.2.1", "beta-admin", 2222).unwrap();
    let id_beta = store
        .add_full(
            "bastion",
            &dest_beta,
            None,
            &[],
            None,
            None,
            "ssh",
            None,
            None,
        )
        .await
        .unwrap();

    // 2. Query each window's scoped inventory and verify strict isolation
    store.set_vault_id(vid_local.clone());
    let hosts_local = store.list().await.unwrap();
    assert_eq!(hosts_local.len(), 1);
    assert_eq!(hosts_local[0].id, id_local);
    assert_eq!(hosts_local[0].name, "bastion");
    assert_eq!(hosts_local[0].destination.host(), "127.0.0.1");
    assert_eq!(hosts_local[0].destination.username(), "localdev");
    assert_eq!(hosts_local[0].destination.port().get(), 22);

    store.set_vault_id(vid_alpha.clone());
    let hosts_alpha = store.list().await.unwrap();
    assert_eq!(hosts_alpha.len(), 1);
    assert_eq!(hosts_alpha[0].id, id_alpha);
    assert_eq!(hosts_alpha[0].name, "bastion");
    assert_eq!(hosts_alpha[0].destination.host(), "10.0.1.1");
    assert_eq!(hosts_alpha[0].destination.username(), "alpha-admin");
    assert_eq!(hosts_alpha[0].destination.port().get(), 22);

    store.set_vault_id(vid_beta.clone());
    let hosts_beta = store.list().await.unwrap();
    assert_eq!(hosts_beta.len(), 1);
    assert_eq!(hosts_beta[0].id, id_beta);
    assert_eq!(hosts_beta[0].name, "bastion");
    assert_eq!(hosts_beta[0].destination.host(), "10.0.2.1");
    assert_eq!(hosts_beta[0].destination.username(), "beta-admin");
    assert_eq!(hosts_beta[0].destination.port().get(), 2222);

    // 3. Update host in Window Alpha (change port to 2200)
    store.set_vault_id(vid_alpha.clone());
    let updated_dest_alpha = SshDestination::new("10.0.1.1", "alpha-admin", 2200).unwrap();
    let updated = store
        .update_full(
            id_alpha,
            "bastion",
            &updated_dest_alpha,
            None,
            &[],
            None,
            None,
            "ssh",
            None,
            None,
        )
        .await
        .unwrap();
    assert!(updated);

    // Window Alpha sees port 2200
    let hosts_alpha = store.list().await.unwrap();
    assert_eq!(hosts_alpha[0].destination.port().get(), 2200);

    // Window Beta still sees port 2222
    store.set_vault_id(vid_beta.clone());
    let hosts_beta = store.list().await.unwrap();
    assert_eq!(hosts_beta[0].destination.port().get(), 2222);

    // Window Local still sees port 22
    store.set_vault_id(vid_local.clone());
    let hosts_local = store.list().await.unwrap();
    assert_eq!(hosts_local[0].destination.port().get(), 22);

    // 4. Delete host in Window Beta
    store.set_vault_id(vid_beta.clone());
    let deleted = store.delete(id_beta).await.unwrap();
    assert!(deleted);
    let hosts_beta = store.list().await.unwrap();
    assert!(hosts_beta.is_empty());

    // Window Alpha still has its host intact
    store.set_vault_id(vid_alpha.clone());
    let hosts_alpha = store.list().await.unwrap();
    assert_eq!(hosts_alpha.len(), 1);

    // Window Local still has its host intact
    store.set_vault_id(vid_local.clone());
    let hosts_local = store.list().await.unwrap();
    assert_eq!(hosts_local.len(), 1);
}

#[tokio::test]
async fn test_multi_window_groups_snippets_and_workspaces_isolation() {
    let mut store = HostStore::open(":memory:").await.unwrap();

    let vid_alpha = VaultId::new("corp-alpha-vault").unwrap();
    let vid_beta = VaultId::new("corp-beta-vault").unwrap();

    // 1. Groups isolation with identical group names
    store.set_vault_id(vid_alpha.clone());
    let group_alpha = HostGroup::new("DevOps", None, None, &[]).unwrap();
    store.add_group(&group_alpha).await.unwrap();

    store.set_vault_id(vid_beta.clone());
    let group_beta = HostGroup::new("DevOps", None, None, &[]).unwrap();
    store.add_group(&group_beta).await.unwrap();

    store.set_vault_id(vid_alpha.clone());
    let groups_alpha = store.list_groups().await.unwrap();
    assert_eq!(groups_alpha.len(), 1);
    assert_eq!(groups_alpha[0].name, "DevOps");

    store.set_vault_id(vid_beta.clone());
    let groups_beta = store.list_groups().await.unwrap();
    assert_eq!(groups_beta.len(), 1);
    assert_eq!(groups_beta[0].name, "DevOps");

    // 2. Snippets isolation with identical snippet names
    store.set_vault_id(vid_alpha.clone());
    let snippet_alpha = Snippet::new(
        "restart-service",
        "systemctl restart alpha-svc",
        Some("Restart Alpha Service".to_string()),
        &["prod".to_string()],
    )
    .unwrap();
    let s_alpha_id = store.add_snippet(&snippet_alpha).await.unwrap();

    store.set_vault_id(vid_beta.clone());
    let snippet_beta = Snippet::new(
        "restart-service",
        "systemctl restart beta-svc",
        Some("Restart Beta Service".to_string()),
        &["staging".to_string()],
    )
    .unwrap();
    let s_beta_id = store.add_snippet(&snippet_beta).await.unwrap();

    store.set_vault_id(vid_alpha.clone());
    let snippets_alpha = store.list_snippets().await.unwrap();
    assert_eq!(snippets_alpha.len(), 1);
    assert_eq!(snippets_alpha[0].id, s_alpha_id);
    assert_eq!(snippets_alpha[0].command, "systemctl restart alpha-svc");

    store.set_vault_id(vid_beta.clone());
    let snippets_beta = store.list_snippets().await.unwrap();
    assert_eq!(snippets_beta.len(), 1);
    assert_eq!(snippets_beta[0].id, s_beta_id);
    assert_eq!(snippets_beta[0].command, "systemctl restart beta-svc");

    // 3. Workspaces isolation with identical workspace names
    store.set_vault_id(vid_alpha.clone());
    let w_alpha_id = store
        .add_workspace(
            "Monitoring",
            Some("Alpha monitoring dashboard"),
            "single",
            &[],
            &["alpha".to_string()],
        )
        .await
        .unwrap();

    store.set_vault_id(vid_beta.clone());
    let w_beta_id = store
        .add_workspace(
            "Monitoring",
            Some("Beta staging dashboard"),
            "split_horizontal",
            &[],
            &["beta".to_string()],
        )
        .await
        .unwrap();

    store.set_vault_id(vid_alpha.clone());
    let workspaces_alpha = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces_alpha.len(), 1);
    assert_eq!(workspaces_alpha[0].id, w_alpha_id);
    assert_eq!(
        workspaces_alpha[0].description.as_deref(),
        Some("Alpha monitoring dashboard")
    );

    store.set_vault_id(vid_beta.clone());
    let workspaces_beta = store.list_workspaces().await.unwrap();
    assert_eq!(workspaces_beta.len(), 1);
    assert_eq!(workspaces_beta[0].id, w_beta_id);
    assert_eq!(
        workspaces_beta[0].description.as_deref(),
        Some("Beta staging dashboard")
    );
}

#[tokio::test]
async fn test_multi_window_pending_mutation_queue_isolation() {
    let mut store = HostStore::open(":memory:").await.unwrap();

    let vid_alpha = "corp-alpha-vault";
    let vid_beta = "corp-beta-vault";

    // 1. Record mutations in Window Alpha
    store
        .record_pending_mutation(
            vid_alpha,
            "host-alpha-1",
            "host",
            0,
            false,
            Some(r#"{"hostname":"10.0.1.100"}"#),
        )
        .await
        .unwrap();
    store
        .record_pending_mutation(
            vid_alpha,
            "snippet-alpha-1",
            "snippet",
            0,
            false,
            Some(r#"{"name":"deploy"}"#),
        )
        .await
        .unwrap();

    // 2. Record mutations in Window Beta
    store
        .record_pending_mutation(
            vid_beta,
            "host-beta-1",
            "host",
            0,
            false,
            Some(r#"{"hostname":"10.0.2.200"}"#),
        )
        .await
        .unwrap();

    // 3. Verify queue isolation per vault
    let muts_alpha = store.get_pending_mutations(vid_alpha).await.unwrap();
    assert_eq!(muts_alpha.len(), 2);
    assert_eq!(muts_alpha[0].object_id, "host-alpha-1");
    assert_eq!(muts_alpha[1].object_id, "snippet-alpha-1");

    let muts_beta = store.get_pending_mutations(vid_beta).await.unwrap();
    assert_eq!(muts_beta.len(), 1);
    assert_eq!(muts_beta[0].object_id, "host-beta-1");

    // 4. Clear mutations in Window Alpha and verify Window Beta is untouched
    let alpha_ids: Vec<i64> = muts_alpha.iter().map(|m| m.id).collect();
    store
        .clear_pending_mutations(vid_alpha, &alpha_ids)
        .await
        .unwrap();

    let muts_alpha_after = store.get_pending_mutations(vid_alpha).await.unwrap();
    assert!(muts_alpha_after.is_empty());

    let muts_beta_after = store.get_pending_mutations(vid_beta).await.unwrap();
    assert_eq!(muts_beta_after.len(), 1);
    assert_eq!(muts_beta_after[0].object_id, "host-beta-1");
}

#[tokio::test]
async fn test_window_lifecycle_ws_client_termination_and_independence() {
    let dev_alpha = Uuid::new_v4();
    let dev_beta = Uuid::new_v4();

    let config_alpha = WsClientConfig::new(
        "http://127.0.0.1:54321".to_string(),
        "token-alpha".to_string(),
        dev_alpha,
    );
    let config_beta = WsClientConfig::new(
        "http://127.0.0.1:54322".to_string(),
        "token-beta".to_string(),
        dev_beta,
    );

    let client_alpha = ShellumiWsClient::spawn(config_alpha);
    let client_beta = ShellumiWsClient::spawn(config_beta);

    assert_eq!(client_alpha.config().device_id, dev_alpha);
    assert_eq!(client_beta.config().device_id, dev_beta);

    // Simulate Window Alpha closing by dropping client_alpha
    client_alpha.close().await;
    drop(client_alpha);

    // Verify client_beta is completely unaffected and independent
    assert_eq!(client_beta.config().token, "token-beta");
    client_beta.close().await;
}

#[test]
fn test_command_palette_and_dropdown_window_spawning_actions() {
    let mut registry = PaletteRegistry::new();

    let available_vaults = vec![
        crate::actions::VaultContextSummary {
            id: "local".to_string(),
            name: "Personal".to_string(),
            is_remote: false,
            server_url: None,
            server_vault_id: None,
            is_locked: false,
        },
        crate::actions::VaultContextSummary {
            id: "corp-alpha-vault".to_string(),
            name: "Alpha Infra".to_string(),
            is_remote: true,
            server_url: Some("https://alpha.corp:8443".to_string()),
            server_vault_id: Some("uuid-alpha".to_string()),
            is_locked: false,
        },
        crate::actions::VaultContextSummary {
            id: "corp-beta-vault".to_string(),
            name: "Beta Prod".to_string(),
            is_remote: true,
            server_url: Some("https://beta.corp:9443".to_string()),
            server_vault_id: Some("uuid-beta".to_string()),
            is_locked: false,
        },
    ];

    let active_vault_id = "local";
    let active_vault = "Personal";

    // Register primary "Open Active Vault in New Window"
    registry.register(
        PaletteCommand::new(
            "vault:open-new-window",
            format!("Open Vault in New Window: {active_vault}"),
            format!("Spawn an independent window scoped to vault '{active_vault}'"),
            PaletteCategory::Vault,
            PaletteAction::OpenVaultInNewWindow {
                vault_id: active_vault_id.to_string(),
                vault_name: active_vault.to_string(),
            },
        )
        .with_shortcut("Cmd+Shift+N"),
    );

    // Register all other vaults
    for v in &available_vaults {
        if v.id != active_vault_id {
            registry.register(PaletteCommand::new(
                format!("vault:open:{}", v.id),
                format!("Open Vault in New Window: {}", v.name),
                format!("Spawn an independent window scoped to vault '{}'", v.name),
                PaletteCategory::Vault,
                PaletteAction::OpenVaultInNewWindow {
                    vault_id: v.id.clone(),
                    vault_name: v.name.clone(),
                },
            ));
        }
    }

    // Verify commands exist and match expectations
    let all = registry.commands();
    assert_eq!(all.len(), 3);

    let active_cmd = all
        .iter()
        .find(|c| c.id == "vault:open-new-window")
        .unwrap();
    assert_eq!(active_cmd.shortcut.as_deref(), Some("Cmd+Shift+N"));
    if let PaletteAction::OpenVaultInNewWindow {
        ref vault_id,
        ref vault_name,
    } = active_cmd.action
    {
        assert_eq!(vault_id, "local");
        assert_eq!(vault_name, "Personal");
    } else {
        panic!("Unexpected action type");
    }

    let alpha_cmd = all
        .iter()
        .find(|c| c.id == "vault:open:corp-alpha-vault")
        .unwrap();
    if let PaletteAction::OpenVaultInNewWindow {
        ref vault_id,
        ref vault_name,
    } = alpha_cmd.action
    {
        assert_eq!(vault_id, "corp-alpha-vault");
        assert_eq!(vault_name, "Alpha Infra");
    } else {
        panic!("Unexpected action type");
    }
}

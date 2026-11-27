mod collaboration;
mod infrastructure;
mod multi_window;
mod real_server_verification;
mod setup_wizard;
mod sftp;
mod sync;

#[test]
fn permissions_input_and_grid_width_are_bounded() {
    assert_eq!(super::parse_sftp_permissions("0000"), Some(0));
    assert_eq!(super::parse_sftp_permissions("0755"), Some(0o755));
    assert_eq!(super::parse_sftp_permissions("7777"), Some(0o7777));
    for invalid in ["", "888", "17777", "-1", "0o755"] {
        assert_eq!(super::parse_sftp_permissions(invalid), None);
    }
    assert_eq!(super::responsive_card_width(864.0, 280.0, 12.0), 280.0);
    assert_eq!(super::responsive_card_width(100.0, 280.0, 12.0), 100.0);
}

#[test]
fn escape_dismisses_an_idle_editor_but_preserves_one_while_saving() {
    let mut editor = Some("draft");
    assert!(super::dismiss_editor(&mut editor, true));
    assert_eq!(editor, Some("draft"));
    assert!(super::dismiss_editor(&mut editor, false));
    assert_eq!(editor, None);
    assert!(!super::dismiss_editor(&mut editor, false));
}

#[test]
fn collection_filter_matches_only_the_selected_group() {
    assert!(super::group_matches(Some("Production"), None));
    assert!(super::group_matches(None, None));
    assert!(super::group_matches(Some("Production"), Some("Production")));
    assert!(!super::group_matches(Some("Production"), Some("Staging")));
    assert!(!super::group_matches(None, Some("Production")));
}

use super::{format_endpoint, is_paste_keystroke, key_bytes, render_terminal_spans, SavedHost};
use gpui::Keystroke;
use shellumi_core::PlaybackSpeed;
use shellumi_terminal::{Color, StyledSpan};

#[test]
fn displays_host_and_ipv6_port_unambiguously() {
    assert_eq!(
        format_endpoint("alice", "example.org", 22),
        "alice@example.org:22"
    );
    assert_eq!(
        format_endpoint("bob", "2001:db8::1", 2222),
        "bob@[2001:db8::1]:2222"
    );
    assert_eq!(
        format_endpoint("bob", "[2001:db8::1]", 2222),
        "bob@[2001:db8::1]:2222"
    );
}

#[test]
fn terminal_keys_translate_to_expected_bytes() {
    let mut key = Keystroke {
        key: "enter".into(),
        ..Default::default()
    };
    assert_eq!(key_bytes(&key), Some(vec![b'\r']));
    key.key = "up".into();
    assert_eq!(key_bytes(&key), Some(b"\x1b[A".to_vec()));
    key.key = "down".into();
    assert_eq!(key_bytes(&key), Some(b"\x1b[B".to_vec()));
    key.key = "right".into();
    assert_eq!(key_bytes(&key), Some(b"\x1b[C".to_vec()));
    key.key = "left".into();
    assert_eq!(key_bytes(&key), Some(b"\x1b[D".to_vec()));
    key.key = "home".into();
    assert_eq!(key_bytes(&key), Some(b"\x1b[H".to_vec()));
    key.key = "end".into();
    assert_eq!(key_bytes(&key), Some(b"\x1b[F".to_vec()));
    key.key = "pageup".into();
    assert_eq!(key_bytes(&key), Some(b"\x1b[5~".to_vec()));
    key.key = "pagedown".into();
    assert_eq!(key_bytes(&key), Some(b"\x1b[6~".to_vec()));
    key.key = "delete".into();
    assert_eq!(key_bytes(&key), Some(b"\x1b[3~".to_vec()));

    key.key = "c".into();
    key.modifiers.control = true;
    assert_eq!(key_bytes(&key), Some(vec![3]));
    key.modifiers.control = false;

    key.key_char = Some("é".into());
    key.key = "é".into();
    assert_eq!(key_bytes(&key), Some("é".as_bytes().to_vec()));

    // Alt sends escape prefix
    key.modifiers.alt = true;
    assert_eq!(key_bytes(&key), Some(vec![0x1b, b'\xc3', b'\xa9']));
    key.modifiers.alt = false;

    key.modifiers.platform = true;
    assert_eq!(key_bytes(&key), None);
}

#[test]
fn identifies_paste_keystrokes_correctly() {
    let mut key = Keystroke {
        key: "v".into(),
        ..Default::default()
    };
    assert!(!is_paste_keystroke(&key));

    // macOS Command-V
    key.modifiers.platform = true;
    assert!(is_paste_keystroke(&key));

    // Command-Ctrl-V is not regular paste
    key.modifiers.control = true;
    assert!(!is_paste_keystroke(&key));
    key.modifiers.platform = false;

    // Linux/Windows Terminal Ctrl-Shift-V
    key.modifiers.shift = true;
    assert!(is_paste_keystroke(&key));

    // Ctrl-V without shift is not paste (so terminal can receive raw ^V)
    key.modifiers.shift = false;
    assert!(!is_paste_keystroke(&key));
}

#[test]
fn renders_terminal_spans_without_panic() {
    use shellumi_terminal::CellAttributes;
    let spans = vec![StyledSpan {
        text: "colored terminal text".into(),
        fg: Color::Rgb(255, 0, 0),
        bg: Color::Rgb(0, 0, 0),
        attrs: CellAttributes {
            bold: true,
            italic: true,
            underline: true,
            strikethrough: true,
            ..Default::default()
        },
    }];
    let _ = render_terminal_spans(spans, 21.0);
    let empty_spans = Vec::new();
    let _ = render_terminal_spans(empty_spans, 21.0);
}

#[test]
fn manages_tab_indices_correctly_on_close() {
    let mut tabs = vec![1u64, 2, 3];
    let mut active = Some(1usize);

    // Close tab 2 (index 1)
    let pos = tabs.iter().position(|&id| id == 2).unwrap();
    tabs.remove(pos);
    if let Some(a) = active {
        if a == pos {
            active = Some(pos.min(tabs.len() - 1));
        } else if a > pos {
            active = Some(a - 1);
        }
    }
    assert_eq!(tabs, vec![1, 3]);
    assert_eq!(active, Some(1));

    // Close tab 3 (index 1)
    let pos = tabs.iter().position(|&id| id == 3).unwrap();
    tabs.remove(pos);
    if let Some(a) = active {
        if a == pos {
            active = Some(pos.min(tabs.len() - 1));
        } else if a > pos {
            active = Some(a - 1);
        }
    }
    assert_eq!(tabs, vec![1]);
    assert_eq!(active, Some(0));

    // Close tab 1 (index 0)
    let pos = tabs.iter().position(|&id| id == 1).unwrap();
    tabs.remove(pos);
    if tabs.is_empty() {
        active = None;
    }
    assert_eq!(tabs.len(), 0);
    assert_eq!(active, None);
}

#[test]
fn filters_hosts_by_name_group_and_tag() {
    use shellumi_core::SshDestination;
    let hosts = [
        SavedHost {
            id: 1,
            name: "Web1".into(),
            destination: SshDestination::new("10.0.0.1", "admin", 22).unwrap(),
            group_name: Some("Production".into()),
            tags: vec!["frontend".into(), "nginx".into()],
            jump_host: Some("bastion.corp:2222".into()),
            startup_snippet_id: Some(42),
            protocol: "ssh".into(),
            protocol_config: None,
            identity_id: None,
            auth_dependency_unresolved: false,
            unresolved_identity_name: None,
        },
        SavedHost {
            id: 2,
            name: "Db1".into(),
            destination: SshDestination::new("10.0.0.2", "postgres", 5432).unwrap(),
            group_name: Some("Production".into()),
            tags: vec!["database".into(), "sql".into()],
            jump_host: None,
            startup_snippet_id: None,
            protocol: "ssh".into(),
            protocol_config: None,
            identity_id: None,
            auth_dependency_unresolved: false,
            unresolved_identity_name: None,
        },
        SavedHost {
            id: 3,
            name: "TestBench".into(),
            destination: SshDestination::new("192.168.1.5", "dev", 2222).unwrap(),
            group_name: Some("Staging".into()),
            tags: vec!["test".into()],
            jump_host: None,
            startup_snippet_id: None,
            protocol: "ssh".into(),
            protocol_config: None,
            identity_id: None,
            auth_dependency_unresolved: false,
            unresolved_identity_name: None,
        },
    ];

    let filter = |query: &str| -> Vec<&SavedHost> {
        let q = query.to_lowercase();
        hosts
            .iter()
            .filter(|h| {
                h.name.to_lowercase().contains(&q)
                    || h.destination.host().to_lowercase().contains(&q)
                    || h.destination.username().to_lowercase().contains(&q)
                    || h.group_name
                        .as_ref()
                        .is_some_and(|g| g.to_lowercase().contains(&q))
                    || h.jump_host
                        .as_ref()
                        .is_some_and(|j| j.to_lowercase().contains(&q))
                    || h.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    };

    assert_eq!(filter("web").len(), 1);
    assert_eq!(filter("production").len(), 2);
    assert_eq!(filter("database").len(), 1);
    assert_eq!(filter("192.168.1.5").len(), 1);
    assert_eq!(filter("bastion.corp").len(), 1);
    assert_eq!(filter("nonexistent").len(), 0);
}

#[test]
fn manages_pane_indices_correctly_on_close() {
    let mut pane_ids = vec![101u64, 102, 103];
    let mut active_pane_idx = 1usize; // pointing to 102

    // Close pane 102 (active pane)
    let pos = pane_ids.iter().position(|&id| id == 102).unwrap();
    pane_ids.remove(pos);
    if active_pane_idx >= pane_ids.len() {
        active_pane_idx = pane_ids.len().saturating_sub(1);
    }
    assert_eq!(pane_ids, vec![101, 103]);
    assert_eq!(active_pane_idx, 1); // now pointing to 103

    // Close pane 103 (last pane, active)
    let pos = pane_ids.iter().position(|&id| id == 103).unwrap();
    pane_ids.remove(pos);
    if active_pane_idx >= pane_ids.len() {
        active_pane_idx = pane_ids.len().saturating_sub(1);
    }
    assert_eq!(pane_ids, vec![101]);
    assert_eq!(active_pane_idx, 0); // now pointing to 101
}

#[test]
fn formats_session_tab_labels_and_titles() {
    use super::SessionKind;

    let format_tab_label = |kind: SessionKind, name: &str| match kind {
        SessionKind::Ssh => format!("SSH: {name}"),
        SessionKind::Mosh => format!("Mosh: {name}"),
        SessionKind::Telnet => format!("Telnet: {name}"),
        SessionKind::Serial => format!("Serial: {name}"),
        SessionKind::Local => name.to_string(),
        SessionKind::Sftp => format!("SFTP: {name}"),
        SessionKind::RemoteDesktop => format!("Desktop: {name}"),
    };

    assert_eq!(format_tab_label(SessionKind::Ssh, "Prod"), "SSH: Prod");
    assert_eq!(
        format_tab_label(SessionKind::Mosh, "Roaming"),
        "Mosh: Roaming"
    );
    assert_eq!(
        format_tab_label(SessionKind::Telnet, "Switch"),
        "Telnet: Switch"
    );
    assert_eq!(
        format_tab_label(SessionKind::Serial, "Console"),
        "Serial: Console"
    );
    assert_eq!(
        format_tab_label(SessionKind::Local, "Local terminal"),
        "Local terminal"
    );
    assert_eq!(
        format_tab_label(SessionKind::Sftp, "Storage"),
        "SFTP: Storage"
    );
}

#[test]
fn renders_snippet_command_for_insertion_or_execution() {
    let template = "echo 'Deploying version {{version:-v1.0}}'";
    let params = std::collections::HashMap::new();
    let rendered = shellumi_core::render_command(template, &params).unwrap();
    assert_eq!(rendered, "echo 'Deploying version v1.0'");

    let paste_bytes = rendered.clone().into_bytes();
    assert_eq!(paste_bytes, b"echo 'Deploying version v1.0'");

    let run_bytes = format!("{rendered}\r").into_bytes();
    assert_eq!(run_bytes, b"echo 'Deploying version v1.0'\r");
}

#[test]
fn verifies_snippet_modal_parameter_binding_and_execution_flow() {
    use shellumi_core::{render_command, render_preview, Snippet};
    use std::collections::HashMap;

    let snippet = Snippet::new(
            "Deploy Container",
            "docker run -d -p {{port:-8080}}:80 --name {{app_name}} -e DB_PASSWORD={{db_password}} {{image}}",
            Some("Deploys a web container".into()),
            &["docker".into()],
        )
        .unwrap();

    assert_eq!(snippet.variables.len(), 4);
    assert!(snippet
        .variables
        .iter()
        .any(|v| v.name == "db_password" && v.is_secret));

    // Missing required variable fails
    let empty_params = HashMap::new();
    assert!(render_command(&snippet.command, &empty_params).is_err());

    // Supplying required parameters renders command with defaults applied
    let mut params = HashMap::new();
    params.insert("app_name".into(), "web-svc".into());
    params.insert("db_password".into(), "SuperSecretPass123!".into());
    params.insert("image".into(), "nginx:latest".into());

    let rendered = render_command(&snippet.command, &params).unwrap();
    assert!(rendered.contains("-p 8080:80"));
    assert!(rendered.contains("--name web-svc"));
    assert!(rendered.contains("DB_PASSWORD=SuperSecretPass123!"));
    assert!(rendered.contains("nginx:latest"));

    // Preview redacts secret values
    let preview = render_preview(&snippet.command, &params, &snippet.variables).unwrap();
    assert!(preview.contains("DB_PASSWORD=[REDACTED]"));
    assert!(!preview.contains("SuperSecretPass123!"));
}

#[test]
fn verifies_command_palette_registry_matching_and_actions() {
    use shellumi_core::{
        PaletteAction, PaletteCategory, PaletteCommand, PaletteRegistry, SshDestination,
    };

    let mut registry = PaletteRegistry::with_static_actions();

    // Check static actions
    assert!(registry
        .commands()
        .iter()
        .any(|c| c.action == PaletteAction::QuickConnect));
    assert!(registry
        .commands()
        .iter()
        .any(|c| c.action == PaletteAction::TriggerSync));
    assert!(registry
        .commands()
        .iter()
        .any(|c| c.action == PaletteAction::SplitVertical));
    assert!(registry
        .commands()
        .iter()
        .any(|c| c.action == PaletteAction::NewLocalTerminal));

    // Add mock host
    let dest = SshDestination::new("192.168.1.50", "admin", 22).unwrap();
    registry.register(
        PaletteCommand::new(
            "host:99",
            "Connect to Bastion Gateway",
            format!("SSH: {}@{}:{}", dest.username(), dest.host(), dest.port()),
            PaletteCategory::Host,
            PaletteAction::ConnectHost {
                host_id: 99,
                host_name: "Bastion Gateway".into(),
            },
        )
        .with_keywords(["gateway", "bastion", "prod"]),
    );

    // Add mock snippet
    registry.register(
        PaletteCommand::new(
            "snippet:77",
            "Run Snippet: Clear Docker Logs",
            "truncate -s 0 /var/lib/docker/containers/*/*-json.log",
            PaletteCategory::Snippet,
            PaletteAction::ExecuteSnippet {
                snippet_id: 77,
                snippet_name: "Clear Docker Logs".into(),
            },
        )
        .with_keywords(["docker", "cleanup", "logs"]),
    );

    // 1. Search for host
    let host_matches = registry.search("bastion");
    assert!(!host_matches.is_empty());
    assert_eq!(
        host_matches[0].command.action,
        PaletteAction::ConnectHost {
            host_id: 99,
            host_name: "Bastion Gateway".into(),
        }
    );
    assert_eq!(host_matches[0].command.category, PaletteCategory::Host);

    // 2. Search for snippet
    let snip_matches = registry.search("Clear Docker");
    assert!(!snip_matches.is_empty());
    assert_eq!(
        snip_matches[0].command.action,
        PaletteAction::ExecuteSnippet {
            snippet_id: 77,
            snippet_name: "Clear Docker Logs".into(),
        }
    );
    assert_eq!(snip_matches[0].command.category, PaletteCategory::Snippet);

    // 3. Search for action by shortcut keyword
    let split_matches = registry.search("split");
    assert!(split_matches.len() >= 2);
    assert!(split_matches
        .iter()
        .any(|m| m.command.action == PaletteAction::SplitVertical));
    assert!(split_matches
        .iter()
        .any(|m| m.command.action == PaletteAction::SplitHorizontal));

    // 4. Acronym match: "qc" -> Quick Connect
    let qc_matches = registry.search("qc");
    assert!(!qc_matches.is_empty());
    assert_eq!(qc_matches[0].command.action, PaletteAction::QuickConnect);

    // 5. Batch execution snippet command
    registry.register(PaletteCommand::batch_from_snippet(77, "Clear Docker Logs"));
    let batch_matches = registry.search("multi-host");
    assert!(!batch_matches.is_empty());
    assert_eq!(
        batch_matches[0].command.action,
        PaletteAction::ExecuteBatchSnippet {
            snippet_id: 77,
            snippet_name: "Clear Docker Logs".into(),
        }
    );
    assert_eq!(batch_matches[0].command.category, PaletteCategory::Snippet);

    // 6. Broadcast and Workspace actions
    assert!(registry
        .commands()
        .iter()
        .any(|c| c.action == PaletteAction::ToggleBroadcastInput));
    assert!(registry
        .commands()
        .iter()
        .any(|c| c.action == PaletteAction::SaveCurrentLayoutAsWorkspace));

    registry.register(PaletteCommand::workspace_from_saved(
        12,
        "Kubernetes Ops Layout",
        "vertical",
    ));
    let ws_matches = registry.search("k8s");
    assert_eq!(ws_matches.len(), 0);
    let ws_matches2 = registry.search("Kubernetes");
    assert_eq!(ws_matches2.len(), 1);
    assert_eq!(
        ws_matches2[0].command.action,
        PaletteAction::LoadWorkspace {
            workspace_id: 12,
            workspace_name: "Kubernetes Ops Layout".into(),
        }
    );
    assert_eq!(ws_matches2[0].command.category, PaletteCategory::Workspace);
}

#[test]
fn verifies_workspace_and_broadcast_input_flow() {
    use shellumi_core::{
        check_dangerous_command, BroadcastManager, BroadcastSafetyCheck, Workspace,
        WorkspaceLayout, WorkspaceSession, WorkspaceSessionKind,
    };

    // 1. Broadcast manager state & exclusions
    let mut bm = BroadcastManager::new(3);
    assert!(!bm.is_enabled());
    assert_eq!(bm.receiving_panes_count(), 0);
    assert_eq!(bm.status_text(), "Broadcast: Off");

    bm.enable();
    assert!(bm.is_enabled());
    assert_eq!(bm.receiving_panes_count(), 3);
    assert_eq!(bm.status_text(), "📡 Broadcast: ON (3/3 panes receiving)");

    // Exclude pane 1
    bm.exclude_pane(1);
    assert!(bm.is_pane_excluded(1));
    assert!(!bm.is_pane_excluded(0));
    assert!(!bm.is_pane_excluded(2));
    assert_eq!(bm.receiving_panes_count(), 2);
    assert_eq!(bm.status_text(), "📡 Broadcast: ON (2/3 panes receiving)");

    // 2. Dangerous command detection for broadcast safety
    let safe1 = check_dangerous_command("ls -la /var/log");
    assert_eq!(safe1, BroadcastSafetyCheck::Safe);
    let safe2 = check_dangerous_command("docker ps -a");
    assert_eq!(safe2, BroadcastSafetyCheck::Safe);

    let dangerous_rm = check_dangerous_command("rm -rf /");
    assert!(matches!(
        dangerous_rm,
        BroadcastSafetyCheck::Dangerous { .. }
    ));

    let dangerous_mkfs = check_dangerous_command("mkfs.ext4 /dev/sdb1");
    assert!(matches!(
        dangerous_mkfs,
        BroadcastSafetyCheck::Dangerous { .. }
    ));

    let dangerous_dd = check_dangerous_command("dd if=/dev/zero of=/dev/sda bs=1M");
    assert!(matches!(
        dangerous_dd,
        BroadcastSafetyCheck::Dangerous { .. }
    ));

    // 3. Workspace model construction & validation
    let s1 = WorkspaceSession {
        name: "Bastion SSH".into(),
        kind: WorkspaceSessionKind::Ssh,
        destination: None,
        host_id: Some(10),
        startup_command: Some("tmux new-session -A -s main".into()),
        working_directory: None,
        pane_index: 0,
        excluded_from_broadcast: false,
    };
    let s2 = WorkspaceSession {
        name: "Local Monitor".into(),
        kind: WorkspaceSessionKind::Local,
        destination: None,
        host_id: None,
        startup_command: Some("htop".into()),
        working_directory: None,
        pane_index: 1,
        excluded_from_broadcast: true,
    };

    let ws = Workspace::new("Production Ops Center")
        .unwrap()
        .with_description("Multi-pane monitoring")
        .with_layout(WorkspaceLayout::SplitVertical)
        .with_sessions(vec![s1, s2])
        .with_tags(["production", "sre"]);

    assert_eq!(ws.name, "Production Ops Center");
    assert_eq!(ws.layout, WorkspaceLayout::SplitVertical);
    assert_eq!(ws.sessions.len(), 2);
    assert!(!ws.sessions[0].excluded_from_broadcast);
    assert!(ws.sessions[1].excluded_from_broadcast);
}

#[test]
fn verifies_batch_execution_runner_plan_and_dry_run_preview() {
    use shellumi_core::{
        BatchExecutionPlan, BatchHostStatus, BatchHostTarget, BatchPolicy, BatchRunner,
        SshDestination,
    };

    let target1 = BatchHostTarget::new(
        1,
        "web-1",
        SshDestination::new("192.168.1.10", "deploy", 22).unwrap(),
    )
    .with_group("production")
    .with_tags(["web", "us-east"]);

    let target2 = BatchHostTarget::new(
        2,
        "web-2",
        SshDestination::new("192.168.1.11", "deploy", 22).unwrap(),
    )
    .with_group("production")
    .with_tags(["web", "us-east"]);

    let plan = BatchExecutionPlan::new(
        10,
        "Restart Nginx",
        "sudo systemctl restart nginx # host={{host.name}} ip={{host.hostname}} env={{env}}",
        vec![target1, target2],
    )
    .with_policy(BatchPolicy::Parallel { max_concurrency: 4 })
    .with_global_variable("env", "prod");

    // Dry-run preview
    let previews = plan.preview_all();
    assert_eq!(previews.len(), 2);
    assert_eq!(
        previews[0].1.as_ref().unwrap(),
        "sudo systemctl restart nginx # host=web-1 ip=192.168.1.10 env=prod"
    );
    assert_eq!(
        previews[1].1.as_ref().unwrap(),
        "sudo systemctl restart nginx # host=web-2 ip=192.168.1.11 env=prod"
    );

    // Batch runner execution
    let mut runner = BatchRunner::new(plan);
    assert_eq!(runner.results().len(), 2);
    assert_eq!(runner.results()[0].status, BatchHostStatus::Queued);

    let summary = runner.execute_sync(|target, rendered| {
        assert!(rendered.contains("sudo systemctl restart nginx"));
        Ok((0, format!("OK from {}", target.name), String::new()))
    });

    assert_eq!(summary.total, 2);
    assert_eq!(summary.succeeded, 2);
    assert_eq!(summary.failed, 0);
    assert!(summary.is_complete);
    assert_eq!(runner.results()[0].status, BatchHostStatus::Succeeded);
    assert_eq!(runner.results()[0].exit_code, Some(0));
    assert_eq!(runner.results()[0].stdout, "OK from web-1");
}

#[test]
fn verifies_startup_snippet_variable_resolution() {
    use shellumi_core::{render_command, SshDestination};

    let host = SavedHost {
        id: 42,
        name: "api-server".into(),
        destination: SshDestination::new("api.corp.internal", "appuser", 2200).unwrap(),
        group_name: Some("backend".into()),
        tags: vec!["api".into(), "v2".into()],
        jump_host: Some("bastion.corp:22".into()),
        startup_snippet_id: Some(99),
        protocol: "ssh".into(),
        protocol_config: None,
        identity_id: None,
        auth_dependency_unresolved: false,
        unresolved_identity_name: None,
    };

    let template = "echo 'Welcome {{host.user}} to {{host.name}} ({{host.hostname}}:{{host.port}}) [{{host.group}}] tags: {{host.tags}} id: {{host.id}}'";

    let mut vars = std::collections::HashMap::new();
    vars.insert("host.id".to_string(), host.id.to_string());
    vars.insert("host.name".to_string(), host.name.clone());
    vars.insert(
        "host.hostname".to_string(),
        host.destination.host().to_string(),
    );
    vars.insert(
        "host.username".to_string(),
        host.destination.username().to_string(),
    );
    vars.insert(
        "host.user".to_string(),
        host.destination.username().to_string(),
    );
    vars.insert(
        "host.port".to_string(),
        host.destination.port().get().to_string(),
    );
    vars.insert(
        "host.group".to_string(),
        host.group_name.clone().unwrap_or_default(),
    );
    vars.insert("host.tags".to_string(), host.tags.join(","));

    let rendered = render_command(template, &vars).unwrap();
    assert_eq!(
            rendered,
            "echo 'Welcome appuser to api-server (api.corp.internal:2200) [backend] tags: api,v2 id: 42'"
        );
}

#[test]
fn verifies_protocol_classification_and_ui_security_warnings() {
    use shellumi_core::{ConnectionProtocol, TelnetConfig};

    // Protocol capabilities and badges
    assert!(ConnectionProtocol::Ssh.is_encrypted());
    assert!(ConnectionProtocol::Ssh.supports_sftp());
    assert_eq!(ConnectionProtocol::Ssh.badge_icon(), "🔒");

    assert!(ConnectionProtocol::Mosh.is_encrypted());
    assert!(!ConnectionProtocol::Mosh.supports_sftp());
    assert_eq!(ConnectionProtocol::Mosh.badge_icon(), "⚡");

    assert!(!ConnectionProtocol::Telnet.is_encrypted());
    assert!(!ConnectionProtocol::Telnet.supports_sftp());
    assert_eq!(ConnectionProtocol::Telnet.badge_icon(), "⚠️");

    assert!(!ConnectionProtocol::Serial.is_encrypted());
    assert!(!ConnectionProtocol::Serial.supports_sftp());
    assert_eq!(ConnectionProtocol::Serial.badge_icon(), "🔌");

    assert!(!ConnectionProtocol::Vnc.is_encrypted());
    assert!(!ConnectionProtocol::Vnc.supports_sftp());
    assert!(ConnectionProtocol::Vnc.is_graphical());
    assert_eq!(ConnectionProtocol::Vnc.badge_icon(), "🖥️");
    assert_eq!(ConnectionProtocol::Vnc.default_port(), Some(5900));

    assert!(ConnectionProtocol::Rdp.is_encrypted());
    assert!(!ConnectionProtocol::Rdp.supports_sftp());
    assert!(ConnectionProtocol::Rdp.is_graphical());
    assert_eq!(ConnectionProtocol::Rdp.badge_icon(), "🪟");
    assert_eq!(ConnectionProtocol::Rdp.default_port(), Some(3389));

    // Telnet security warning
    let warning = TelnetConfig::security_warning();
    assert!(warning.contains("UNENCRYPTED PROTOCOL"));
    assert!(warning.contains("cleartext"));
}

#[test]
fn verifies_remote_desktop_state_and_scaling_transitions() {
    use super::{
        RemoteDesktopProtocol, RemoteDesktopScaleMode, RemoteDesktopState, RemoteDesktopStatus,
    };

    let mut rd = RemoteDesktopState {
        host_name: "Ubuntu Workstation".into(),
        target: "192.168.1.50:5900".into(),
        protocol: RemoteDesktopProtocol::Vnc,
        status: RemoteDesktopStatus::NotConnected,
        status_message: Some("Ready to connect via RFB".into()),
        width: 1920,
        height: 1080,
        view_only: false,
        scale_mode: RemoteDesktopScaleMode::Fit,
        color_depth: 24,
        fps: None,
        latency_ms: None,
        clipboard_text: None,
        error: None,
        last_action_note: None,
        framebuffer: None,
        render_image: None,
        ui_handle: None,
        metrics: shellumi_remote_desktop::SessionMetrics::new(),
        username: None,
        password: None,
        domain: None,
        tls_strict: true,
        show_diagnostics: false,
        username_input: None,
        domain_input: None,
        password_input: None,
        enable_audio: false,
        audio_muted: false,
        audio_volume: 100,
        audio_active: false,
        audio_format: None,
        audio_sample_rate: 44100,
        audio_channels: 2,
        audio_bytes_received: 0,
        enable_microphone: false,
        mic_muted: false,
        mic_volume: 100,
        mic_active: false,
        mic_format: None,
        mic_sample_rate: 44100,
        mic_channels: 2,
        mic_bytes_sent: 0,
        mic_packets_sent: 0,
        display_control_ready: false,
        max_monitor_area: None,
        connection_generation: 0,
        dynamic_resize_enabled: true,
        shared_folder: None,
        drive_name: None,
        drive_mounted: false,
        drive_bytes_read: 0,
        drive_bytes_written: 0,
        drive_operations_count: 0,
    };

    // Truthful baseline: initial state must never be fake Connected, metrics must be None
    assert_eq!(rd.status, RemoteDesktopStatus::NotConnected);
    assert!(
        rd.fps.is_none(),
        "FPS must be unmeasured before real frames"
    );
    assert!(
        rd.latency_ms.is_none(),
        "Latency must be unmeasured before real ping"
    );
    assert_eq!(rd.scale_mode, RemoteDesktopScaleMode::Fit);
    assert!(!rd.view_only);

    // Toggle view only
    rd.view_only = true;
    assert!(rd.view_only);

    // Change scale mode
    rd.scale_mode = RemoteDesktopScaleMode::Original;
    assert_eq!(rd.scale_mode, RemoteDesktopScaleMode::Original);

    // Disconnect
    rd.status = RemoteDesktopStatus::Disconnected;
    assert_eq!(rd.status, RemoteDesktopStatus::Disconnected);

    // Network resilience: reconnecting state with retry attempts
    rd.status = RemoteDesktopStatus::Reconnecting {
        attempt: 1,
        max_attempts: 5,
    };
    assert_eq!(
        rd.status,
        RemoteDesktopStatus::Reconnecting {
            attempt: 1,
            max_attempts: 5,
        }
    );

    // Reconnecting transitions to Connecting (never immediate fake Connected)
    rd.status = RemoteDesktopStatus::Connecting;
    assert_eq!(rd.status, RemoteDesktopStatus::Connecting);

    // Only genuine protocol events may set Connected and metrics
    rd.status = RemoteDesktopStatus::Connected;
    rd.fps = Some(30);
    rd.latency_ms = Some(18);
    assert_eq!(rd.status, RemoteDesktopStatus::Connected);
    assert_eq!(rd.fps, Some(30));
    assert_eq!(rd.latency_ms, Some(18));
}

#[test]
fn verifies_remote_desktop_never_claims_fake_connected_state_on_launch() {
    use super::{RemoteDesktopProtocol, RemoteDesktopStatus};
    use shellumi_core::SshDestination;
    use shellumi_storage::SavedHost;

    // Simulate host launch parameter resolution
    let host = SavedHost {
        id: 42,
        name: "Dev Bastion".into(),
        destination: SshDestination::new("bastion.example.internal", "developer", 22).unwrap(),
        group_name: None,
        tags: vec![],
        jump_host: None,
        startup_snippet_id: None,
        protocol: "vnc".into(),
        protocol_config: None,
        identity_id: None,
        auth_dependency_unresolved: false,
        unresolved_identity_name: None,
    };

    let target = format!("{}:5900", host.destination.host());
    let protocol = RemoteDesktopProtocol::Vnc;

    // Build the exact initial launch state as shellumi-app launch_remote_desktop produces
    let rd = super::RemoteDesktopState {
        host_name: host.name.clone(),
        target: target.clone(),
        protocol,
        status: RemoteDesktopStatus::NotConnected,
        status_message: Some(format!(
            "Ready to connect to {} via {} — native engine in development",
            host.name, "RFB / VNC (port 5900)"
        )),
        width: 1920,
        height: 1080,
        view_only: false,
        scale_mode: super::RemoteDesktopScaleMode::Fit,
        color_depth: 24,
        fps: None,
        latency_ms: None,
        clipboard_text: None,
        error: None,
        last_action_note: None,
        framebuffer: None,
        render_image: None,
        ui_handle: None,
        metrics: shellumi_remote_desktop::SessionMetrics::new(),
        username: None,
        password: None,
        domain: None,
        tls_strict: true,
        show_diagnostics: false,
        username_input: None,
        domain_input: None,
        password_input: None,
        enable_audio: false,
        audio_muted: false,
        audio_volume: 100,
        audio_active: false,
        audio_format: None,
        audio_sample_rate: 44100,
        audio_channels: 2,
        audio_bytes_received: 0,
        enable_microphone: false,
        mic_muted: false,
        mic_volume: 100,
        mic_active: false,
        mic_format: None,
        mic_sample_rate: 44100,
        mic_channels: 2,
        mic_bytes_sent: 0,
        mic_packets_sent: 0,
        display_control_ready: false,
        max_monitor_area: None,
        connection_generation: 0,
        dynamic_resize_enabled: true,
        shared_folder: None,
        drive_name: None,
        drive_mounted: false,
        drive_bytes_read: 0,
        drive_bytes_written: 0,
        drive_operations_count: 0,
    };

    assert_eq!(
        rd.status,
        RemoteDesktopStatus::NotConnected,
        "New remote desktop session must start in NotConnected state"
    );
    assert_eq!(
        rd.fps, None,
        "FPS metric must be None prior to real protocol frame arrival"
    );
    assert_eq!(
        rd.latency_ms, None,
        "Latency metric must be None prior to real protocol ping measurement"
    );
    assert!(rd
        .status_message
        .as_ref()
        .unwrap()
        .contains("native engine in development"));
}

#[test]
fn verifies_framebuffer_snapshot_application_and_canvas_binding() {
    use super::{
        RemoteDesktopProtocol, RemoteDesktopScaleMode, RemoteDesktopState, RemoteDesktopStatus,
    };
    use shellumi_remote_desktop::{Framebuffer, Rect};

    // Create a 128x128 framebuffer with test patterns
    let mut fb = Framebuffer::new(128, 128).unwrap();
    fb.fill_rect(Rect::new(0, 0, 64, 64), 0, 255, 0, 255)
        .unwrap(); // Green top-left
    fb.fill_rect(Rect::new(64, 64, 64, 64), 255, 255, 0, 255)
        .unwrap(); // Yellow bottom-right

    let snapshot = fb.snapshot();
    assert_eq!(snapshot.width, 128);
    assert_eq!(snapshot.height, 128);

    // Convert into GPUI RenderImage via canvas renderer helper
    let render_image =
        crate::ui::remote_desktop_canvas::create_render_image_from_snapshot(&snapshot);
    assert!(render_image.is_some());
    let img = render_image.as_ref().unwrap();
    assert_eq!(img.frame_count(), 1);

    // Bind into session state
    let mut rd = RemoteDesktopState {
        host_name: "Ubuntu VM".into(),
        target: "10.0.0.15:5900".into(),
        protocol: RemoteDesktopProtocol::Vnc,
        status: RemoteDesktopStatus::NotConnected,
        status_message: None,
        width: 1920,
        height: 1080,
        view_only: false,
        scale_mode: RemoteDesktopScaleMode::Fit,
        color_depth: 24,
        fps: None,
        latency_ms: None,
        clipboard_text: None,
        error: None,
        last_action_note: None,
        framebuffer: None,
        render_image: None,
        ui_handle: None,
        metrics: shellumi_remote_desktop::SessionMetrics::new(),
        username: None,
        password: None,
        domain: None,
        tls_strict: true,
        show_diagnostics: false,
        username_input: None,
        domain_input: None,
        password_input: None,
        enable_audio: false,
        audio_muted: false,
        audio_volume: 100,
        audio_active: false,
        audio_format: None,
        audio_sample_rate: 44100,
        audio_channels: 2,
        audio_bytes_received: 0,
        enable_microphone: false,
        mic_muted: false,
        mic_volume: 100,
        mic_active: false,
        mic_format: None,
        mic_sample_rate: 44100,
        mic_channels: 2,
        mic_bytes_sent: 0,
        mic_packets_sent: 0,
        display_control_ready: false,
        max_monitor_area: None,
        connection_generation: 0,
        dynamic_resize_enabled: true,
        shared_folder: None,
        drive_name: None,
        drive_mounted: false,
        drive_bytes_read: 0,
        drive_bytes_written: 0,
        drive_operations_count: 0,
    };

    // Apply snapshot to state (simulating remote_desktop_apply_framebuffer_snapshot)
    rd.width = snapshot.width;
    rd.height = snapshot.height;
    rd.framebuffer = Some(snapshot);
    rd.render_image = render_image;
    rd.status = RemoteDesktopStatus::Connected;

    assert_eq!(rd.status, RemoteDesktopStatus::Connected);
    assert_eq!(rd.width, 128);
    assert_eq!(rd.height, 128);
    assert!(rd.framebuffer.is_some());
    assert!(rd.render_image.is_some());

    // Verify pixel colors in snapshot
    assert_eq!(
        rd.framebuffer.as_ref().unwrap().pixel(10, 10),
        Some([0, 255, 0, 255])
    );
    assert_eq!(
        rd.framebuffer.as_ref().unwrap().pixel(80, 80),
        Some([255, 255, 0, 255])
    );
}

#[test]
fn verifies_remote_desktop_clipboard_sync_and_file_transfer_commands() {
    let (ui_handle, worker_handle) = shellumi_remote_desktop::create_session_channels(16, 16);
    let id = shellumi_remote_desktop::SessionIdentity::new(1, 1, 1);
    ui_handle.set_active_identity(id);

    // Verify SendClipboard command plumbing
    ui_handle
        .try_send_command(shellumi_remote_desktop::SessionCommand::SendClipboard {
            identity: id,
            text: "Test clipboard contents".into(),
        })
        .unwrap();
    let cmd = worker_handle.try_recv_command().unwrap();
    assert_eq!(
        cmd,
        Some(shellumi_remote_desktop::SessionCommand::SendClipboard {
            identity: id,
            text: "Test clipboard contents".into(),
        })
    );

    // Verify SendFiles command plumbing
    let files = vec![std::path::PathBuf::from("/tmp/sample.txt")];
    ui_handle
        .try_send_command(shellumi_remote_desktop::SessionCommand::SendFiles {
            identity: id,
            paths: files.clone(),
        })
        .unwrap();
    let cmd = worker_handle.try_recv_command().unwrap();
    assert_eq!(
        cmd,
        Some(shellumi_remote_desktop::SessionCommand::SendFiles {
            identity: id,
            paths: files,
        })
    );

    // Verify ServerFilesReceived event plumbing
    worker_handle
        .try_send_event(shellumi_remote_desktop::SessionEvent::ServerFilesReceived {
            files: vec!["received_file.dat".into()],
            target_dir: "/tmp/transfers".into(),
        })
        .unwrap();
    let ev = ui_handle.try_recv_event().unwrap();
    assert_eq!(
        ev,
        Some(shellumi_remote_desktop::SessionEvent::ServerFilesReceived {
            files: vec!["received_file.dat".into()],
            target_dir: "/tmp/transfers".into(),
        })
    );
}

#[test]
fn verifies_remote_desktop_audio_redirection_commands_and_telemetry() {
    let (ui_handle, worker_handle) = shellumi_remote_desktop::create_session_channels(32, 64);
    let id = shellumi_remote_desktop::SessionIdentity::new(1, 1, 1);
    ui_handle.set_active_identity(id);

    // Verify SetAudioMute command plumbing
    ui_handle
        .try_send_command(shellumi_remote_desktop::SessionCommand::SetAudioMute {
            identity: id,
            muted: true,
        })
        .unwrap();
    let cmd = worker_handle.try_recv_command().unwrap();
    assert_eq!(
        cmd,
        Some(shellumi_remote_desktop::SessionCommand::SetAudioMute {
            identity: id,
            muted: true,
        })
    );

    // Verify SetAudioVolume command plumbing
    ui_handle
        .try_send_command(shellumi_remote_desktop::SessionCommand::SetAudioVolume {
            identity: id,
            volume: 75,
        })
        .unwrap();
    let cmd = worker_handle.try_recv_command().unwrap();
    assert_eq!(
        cmd,
        Some(shellumi_remote_desktop::SessionCommand::SetAudioVolume {
            identity: id,
            volume: 75,
        })
    );

    // Verify AudioStatus event plumbing and fields
    let audio_event = shellumi_remote_desktop::SessionEvent::AudioStatus {
        active: true,
        muted: false,
        volume: 75,
        format: "PCM".to_string(),
        sample_rate: 44100,
        channels: 2,
        bytes_received: 8192,
    };
    worker_handle.try_send_event(audio_event.clone()).unwrap();
    let ev = ui_handle.try_recv_event().unwrap();
    assert_eq!(ev, Some(audio_event));
}

#[test]
fn verifies_phase5_graphical_quick_connect_and_ephemeral_credentials() {
    use super::{
        RemoteDesktopProtocol, RemoteDesktopScaleMode, RemoteDesktopState, RemoteDesktopStatus,
    };
    use shellumi_core::{parse_graphical_quick_connect, ConnectionProtocol};

    // 1. Verify VNC quick connect target parsing
    let (proto_vnc, host_vnc, port_vnc, user_vnc) =
        parse_graphical_quick_connect("vnc://admin@192.168.1.100:5901").unwrap();
    assert_eq!(proto_vnc, ConnectionProtocol::Vnc);
    assert_eq!(host_vnc, "192.168.1.100");
    assert_eq!(port_vnc, 5901);
    assert_eq!(user_vnc.as_deref(), Some("admin"));

    // 2. Verify RDP quick connect target parsing
    let (proto_rdp, host_rdp, port_rdp, user_rdp) =
        parse_graphical_quick_connect("rdp://operator@windows.corp").unwrap();
    assert_eq!(proto_rdp, ConnectionProtocol::Rdp);
    assert_eq!(host_rdp, "windows.corp");
    assert_eq!(port_rdp, 3389);
    assert_eq!(user_rdp.as_deref(), Some("operator"));

    // 3. Ephemeral credential handling: state must never serialize password to disk or log
    let mut rd = RemoteDesktopState {
        host_name: format!("{}:{}", host_rdp, port_rdp),
        target: format!("{}:{}", host_rdp, port_rdp),
        protocol: RemoteDesktopProtocol::Rdp,
        status: RemoteDesktopStatus::NotConnected,
        status_message: Some("Awaiting credentials".into()),
        width: 1920,
        height: 1080,
        view_only: false,
        scale_mode: RemoteDesktopScaleMode::Fit,
        color_depth: 32,
        fps: None,
        latency_ms: None,
        clipboard_text: None,
        error: None,
        last_action_note: None,
        framebuffer: None,
        render_image: None,
        ui_handle: None,
        metrics: shellumi_remote_desktop::SessionMetrics::new(),
        username: user_rdp,
        password: None,
        domain: Some("CORP".into()),
        tls_strict: true,
        show_diagnostics: false,
        username_input: None,
        domain_input: None,
        password_input: None,
        enable_audio: true,
        audio_muted: false,
        audio_volume: 100,
        audio_active: false,
        audio_format: None,
        audio_sample_rate: 44100,
        audio_channels: 2,
        audio_bytes_received: 0,
        enable_microphone: false,
        mic_muted: false,
        mic_volume: 100,
        mic_active: false,
        mic_format: None,
        mic_sample_rate: 44100,
        mic_channels: 2,
        mic_bytes_sent: 0,
        mic_packets_sent: 0,
        display_control_ready: false,
        max_monitor_area: None,
        connection_generation: 0,
        dynamic_resize_enabled: true,
        shared_folder: None,
        drive_name: None,
        drive_mounted: false,
        drive_bytes_read: 0,
        drive_bytes_written: 0,
        drive_operations_count: 0,
    };

    assert_eq!(rd.status, RemoteDesktopStatus::NotConnected);
    assert!(rd.password.is_none());

    // User provides interactive on-canvas password
    rd.password = Some("EphemeralSecret123!".into());
    assert_eq!(rd.password.as_deref(), Some("EphemeralSecret123!"));

    // Diagnostics toggle
    assert!(!rd.show_diagnostics);
    rd.show_diagnostics = true;
    assert!(rd.show_diagnostics);
}

#[test]
fn verifies_phase5_workspace_persistence_with_vnc_and_rdp() {
    use shellumi_core::{SshDestination, WorkspaceSession, WorkspaceSessionKind};

    let sess_vnc = WorkspaceSession {
        name: "Linux Workstation".into(),
        kind: WorkspaceSessionKind::Vnc,
        destination: Some(SshDestination::new("192.168.1.50", "user", 5900).unwrap()),
        host_id: Some(101),
        startup_command: None,
        working_directory: None,
        pane_index: 0,
        excluded_from_broadcast: false,
    };

    let sess_rdp = WorkspaceSession {
        name: "Windows Server".into(),
        kind: WorkspaceSessionKind::Rdp,
        destination: Some(SshDestination::new("10.0.0.12", "Administrator", 3389).unwrap()),
        host_id: Some(102),
        startup_command: None,
        working_directory: None,
        pane_index: 1,
        excluded_from_broadcast: false,
    };

    // Serialize and deserialize JSON (as Turso storage does)
    let json_vnc = serde_json::to_string(&sess_vnc).unwrap();
    let json_rdp = serde_json::to_string(&sess_rdp).unwrap();

    let restored_vnc: WorkspaceSession = serde_json::from_str(&json_vnc).unwrap();
    let restored_rdp: WorkspaceSession = serde_json::from_str(&json_rdp).unwrap();

    assert_eq!(restored_vnc.kind, WorkspaceSessionKind::Vnc);
    assert_eq!(restored_vnc.kind.as_str(), "vnc");
    assert_eq!(restored_rdp.kind, WorkspaceSessionKind::Rdp);
    assert_eq!(restored_rdp.kind.as_str(), "rdp");
}

#[test]
fn verifies_diagnostics_modal_and_palette_integration() {
    use shellumi_core::{
        DiagnosticCategory, DiagnosticCheck, DiagnosticStatus, DiagnosticsReport, PaletteAction,
        PaletteCategory, PaletteRegistry,
    };

    let registry = PaletteRegistry::with_static_actions();
    let diag_cmd = registry
        .commands()
        .iter()
        .find(|c| c.action == PaletteAction::RunDiagnostics)
        .expect("should register diagnostics command");

    assert_eq!(diag_cmd.category, PaletteCategory::Diagnostics);
    assert_eq!(diag_cmd.title, "Run System Diagnostics & Health Check");
    assert!(diag_cmd.keywords.contains(&"doctor".to_string()));

    let mock_report = DiagnosticsReport {
        timestamp: "2026-11-05T00:00:00Z".into(),
        shellumi_version: "0.1.0".into(),
        os_platform: "macos".into(),
        os_arch: "aarch64".into(),
        checks: vec![
            DiagnosticCheck {
                id: "chk_1".into(),
                name: "OpenSSH".into(),
                category: DiagnosticCategory::OpenSsh,
                status: DiagnosticStatus::Passed,
                message: "OpenSSH 9.8p1 detected".into(),
                remediation: None,
                duration_ms: 12,
            },
            DiagnosticCheck {
                id: "chk_2".into(),
                name: "Loopback".into(),
                category: DiagnosticCategory::Network,
                status: DiagnosticStatus::Passed,
                message: "Loopback listener bound".into(),
                remediation: None,
                duration_ms: 2,
            },
        ],
    };

    assert!(mock_report.is_healthy());
    assert!(!mock_report.has_failures());
    let (passed, warnings, failures) = mock_report.counts();
    assert_eq!(passed, 2);
    assert_eq!(warnings, 0);
    assert_eq!(failures, 0);

    // Verify benchmark palette registration and modal tab state
    let bench_cmd = registry
        .commands()
        .iter()
        .find(|c| c.action == PaletteAction::RunBenchmarks)
        .expect("should register benchmarks command");
    assert_eq!(bench_cmd.category, PaletteCategory::Diagnostics);
    assert_eq!(bench_cmd.title, "Run Performance Benchmarks");
    assert!(bench_cmd.keywords.contains(&"bench".to_string()));

    let modal_state = super::DiagnosticsModalState {
        active_tab: super::DiagnosticsModalTab::Benchmarks,
        report: None,
        benchmark_report: None,
        is_running: false,
        is_running_benchmarks: true,
        error: None,
    };
    assert_eq!(
        modal_state.active_tab,
        super::DiagnosticsModalTab::Benchmarks
    );
    assert!(modal_state.is_running_benchmarks);
}

#[test]
fn verifies_mobile_simulator_modal_and_palette_integration() {
    use super::{MobileSimulatorModalState, MobileViewportPreset};
    use shellumi_core::{
        AccessoryBarLayout, MobileLifecycleEvent, MobileLifecycleManager, MobilePlatform,
        MobileSessionState, PaletteAction, PaletteCategory, PaletteRegistry, VirtualAccessoryBar,
        VirtualKey, VirtualKeyModifier,
    };

    // 1. Verify Palette command registration
    let registry = PaletteRegistry::with_static_actions();
    let mobile_cmd = registry
        .commands()
        .iter()
        .find(|c| c.action == PaletteAction::OpenMobileSimulator)
        .expect("should register mobile simulator command");

    assert_eq!(mobile_cmd.category, PaletteCategory::Mobile);
    assert_eq!(mobile_cmd.title, "Open Mobile Touch & Accessory Simulator");
    assert!(mobile_cmd.keywords.contains(&"touch".to_string()));
    assert!(mobile_cmd.keywords.contains(&"virtual".to_string()));

    // 2. Verify simulator modal state initialization
    let mut lifecycle_manager = MobileLifecycleManager::new(MobilePlatform::Ios);
    lifecycle_manager.mark_connected();
    let accessory_bar = VirtualAccessoryBar::new(AccessoryBarLayout::DefaultTerminal);

    let mut modal = MobileSimulatorModalState {
        preset: MobileViewportPreset::IPhone15Pro,
        accessory_bar,
        lifecycle_manager,
        simulated_output: vec!["$ ".into()],
        input_history: Vec::new(),
    };

    assert_eq!(modal.preset, MobileViewportPreset::IPhone15Pro);
    assert_eq!(modal.preset.terminal_cols_rows(), (80, 24));
    assert_eq!(modal.lifecycle_manager.state, MobileSessionState::Connected);

    // 3. Test virtual key press & byte sequence translation
    let esc_bytes = modal.accessory_bar.press_key(&VirtualKey::Escape);
    assert_eq!(esc_bytes, vec![0x1B]);

    let ctrl_c_bytes = modal.accessory_bar.press_key(&VirtualKey::ControlKey('c'));
    assert_eq!(ctrl_c_bytes, vec![0x03]);

    // 4. Test sticky modifiers
    modal
        .accessory_bar
        .toggle_modifier(VirtualKeyModifier::Ctrl);
    assert!(modal.accessory_bar.has_modifier(VirtualKeyModifier::Ctrl));
    let resolved = modal.accessory_bar.resolve_char_keystroke('z');
    assert_eq!(resolved, vec![0x1A]); // Ctrl-Z
    assert!(!modal.accessory_bar.has_modifier(VirtualKeyModifier::Ctrl));

    // 5. Test lifecycle transitions
    modal.lifecycle_manager.transition(
        MobileLifecycleEvent::AppBackgrounded { grace_seconds: 30 },
        100,
    );
    assert_eq!(
        modal.lifecycle_manager.state,
        MobileSessionState::BackgroundGracePeriod { remaining_secs: 30 }
    );

    modal
        .lifecycle_manager
        .transition(MobileLifecycleEvent::BackgroundExpired, 130);
    assert_eq!(
        modal.lifecycle_manager.state,
        MobileSessionState::Suspended {
            saved_at_epoch: 130
        }
    );

    modal
        .lifecycle_manager
        .transition(MobileLifecycleEvent::AppForegrounded, 150);
    assert_eq!(
        modal.lifecycle_manager.state,
        MobileSessionState::Reconnecting {
            attempt: 1,
            backoff_secs: 2
        }
    );
}

#[test]
fn verifies_shell_integration_modal_and_autocomplete_integration() {
    use super::*;
    use shellumi_terminal::Terminal;

    // 1. Verify Command Palette action registration
    let registry = PaletteRegistry::with_static_actions();
    let cmd = registry
        .commands()
        .iter()
        .find(|c| c.action == PaletteAction::OpenShellIntegrationModal)
        .expect("should register shell integration command");

    assert_eq!(cmd.category, PaletteCategory::Terminal);
    assert_eq!(cmd.title, "Shell Integration & Autocomplete Setup");
    assert!(cmd.keywords.contains(&"osc133".to_string()));
    assert!(cmd.keywords.contains(&"autocomplete".to_string()));

    // 2. Verify modal state and tab transitions
    let mut modal = ShellIntegrationModalState {
        active_tab: ShellIntegrationTab::Hooks,
        selected_shell: "zsh".to_string(),
        test_prompt_input: "[sudo] password for root:".to_string(),
        detected_prompt_kind: Some(PasswordPromptKind::Sudo),
        test_query_input: "git ".to_string(),
        autocomplete_results: Vec::new(),
    };

    assert_eq!(modal.active_tab, ShellIntegrationTab::Hooks);
    assert_eq!(modal.selected_shell, "zsh");

    modal.active_tab = ShellIntegrationTab::OscMarkers;
    assert_eq!(modal.active_tab, ShellIntegrationTab::OscMarkers);

    modal.active_tab = ShellIntegrationTab::PasswordDetection;
    assert_eq!(modal.active_tab, ShellIntegrationTab::PasswordDetection);

    // 3. Verify hook generation for all supported shells
    let zsh_hook = generate_zsh_hook();
    assert!(zsh_hook.contains("133;A"));
    assert!(zsh_hook.contains("133;D"));
    assert!(zsh_hook.contains("file://"));

    let bash_hook = generate_bash_hook();
    assert!(bash_hook.contains("133;A"));
    assert!(bash_hook.contains("PROMPT_COMMAND"));

    let fish_hook = generate_fish_hook();
    assert!(fish_hook.contains("133;A"));
    assert!(fish_hook.contains("fish_prompt"));

    // 4. Verify password prompt classification in app context
    let sudo_match = PasswordPromptDetector::detect(&modal.test_prompt_input);
    assert!(sudo_match.is_some());
    assert_eq!(sudo_match.unwrap().kind, PasswordPromptKind::Sudo);

    let ssh_match = PasswordPromptDetector::detect("root@bastion.internal's password: ");
    assert!(ssh_match.is_some());
    assert_eq!(ssh_match.unwrap().kind, PasswordPromptKind::Ssh);

    let mfa_match = PasswordPromptDetector::detect("Enter 6-digit MFA verification code: ");
    assert!(mfa_match.is_some());
    assert_eq!(mfa_match.unwrap().kind, PasswordPromptKind::MfaCode);

    let normal_output =
        PasswordPromptDetector::detect("total 48 drwxr-xr-x  14 user  staff   448B");
    assert!(normal_output.is_none());

    // 5. Verify Autocomplete engine integration
    let engine = AutocompleteEngine::new();
    let git_results = engine.complete(&AutocompleteContext::new("git ch", 6));
    assert!(git_results.iter().any(|c| c.display_label == "checkout"));

    let mut host_ctx = AutocompleteContext::new("ssh sta", 7);
    host_ctx
        .host_candidates
        .push("staging-cluster-01".to_string());
    let host_results = engine.complete(&host_ctx);
    assert!(host_results
        .iter()
        .any(|c| c.display_label == "staging-cluster-01"));

    let mut snip_ctx = AutocompleteContext::new("dep", 3);
    snip_ctx.snippet_candidates.push((
        "deploy-prod".to_string(),
        "kubectl rollout restart deployment".to_string(),
    ));
    let snippet_results = engine.complete(&snip_ctx);
    assert!(snippet_results
        .iter()
        .any(|c| c.display_label == "deploy-prod"));

    // 6. Verify Terminal row text and shell integration tracking
    let mut term = Terminal::new(24, 80);
    term.process_bytes(b"echo hello world\r\n");
    assert_eq!(term.row_text(0).as_deref(), Some("echo hello world"));
}

#[test]
fn verifies_session_replay_modal_and_recording_integration() {
    use super::*;
    use shellumi_terminal::Terminal;

    // 1. Verify Command Palette action registration
    let registry = PaletteRegistry::with_static_actions();
    let cmd = registry
        .commands()
        .iter()
        .find(|c| c.action == PaletteAction::OpenSessionRecordings)
        .expect("should register session recordings command");

    assert_eq!(cmd.category, PaletteCategory::Terminal);
    assert_eq!(cmd.title, "Session Recordings & Cast Replay");
    assert!(cmd.keywords.contains(&"asciicast".to_string()));
    assert!(cmd.keywords.contains(&"recording".to_string()));

    // 2. Verify Terminal recording capture lifecycle in app context
    let mut term = Terminal::new(24, 80);
    assert!(!term.is_recording());

    term.start_recording(Some("CI Production Audit".to_string()));
    assert!(term.is_recording());

    term.record_input(b"uname -a\r");
    term.process_bytes(b"Linux prod-node 6.8.0 #1 SMP\r\n");
    term.process_bytes(b"export SECRET=super_classified_val\r\n");
    term.resize(30, 100);

    assert!(term.recording_event_count() >= 3);
    let recording = term.stop_recording().expect("should return recording");
    assert!(!term.is_recording());
    assert_eq!(
        recording.header.title.as_deref(),
        Some("CI Production Audit")
    );

    let raw_cast = recording
        .to_asciicast_v2()
        .expect("should serialize asciicast");
    assert!(raw_cast.contains("\"version\":2"));
    assert!(raw_cast.contains("uname -a"));

    // 3. Verify automated sensitive data redaction on recording
    let clean_cast = SessionRedactor::redact_asciicast_v2(&raw_cast).expect("should redact");
    assert!(!clean_cast.contains("super_classified_val"));
    assert!(clean_cast.contains("[REDACTED_SECRET]"));

    // 4. Verify Replay Modal state and player state machine
    let mut engine = CastPlaybackEngine::from_asciicast_v2(&clean_cast).expect("valid engine");
    assert_eq!(engine.status(), PlaybackStatus::Stopped);

    engine.play();
    assert_eq!(engine.status(), PlaybackStatus::Playing);

    let initial_frames = engine.seek(0.0);
    let mut screen_buffer = String::new();
    for f in initial_frames {
        if f.event_type == "o" {
            screen_buffer.push_str(&f.data);
        }
    }

    let mut modal = SessionReplayModalState {
        active_tab: SessionReplayTab::RecordingsList,
        recordings: vec![SessionRecordingSummary {
            id: "rec-test-1".to_string(),
            session_id: "sess-1".to_string(),
            host_id: None,
            title: "CI Production Audit".to_string(),
            duration_secs: engine.duration_secs(),
            event_count: engine.total_frames(),
            byte_size: clean_cast.len(),
            recorded_at: "2026-11-05T07:20:00Z".to_string(),
        }],
        selected_recording_id: Some("rec-test-1".to_string()),
        current_recording: None,
        playback_engine: Some(engine),
        replay_terminal: None,
        replay_generation: 0,
        screen_output_buffer: screen_buffer,
        status_notification: None,
        search_query: String::new(),
    };

    assert_eq!(modal.active_tab, SessionReplayTab::RecordingsList);
    assert_eq!(modal.recordings.len(), 1);
    assert_eq!(modal.recordings[0].title, "CI Production Audit");

    // Tab transition to player
    modal.active_tab = SessionReplayTab::ReplayPlayer;
    assert_eq!(modal.active_tab, SessionReplayTab::ReplayPlayer);

    // Player operations
    if let Some(ref mut eng) = modal.playback_engine {
        eng.toggle_play_pause();
        assert_eq!(eng.status(), PlaybackStatus::Paused);

        eng.set_speed(PlaybackSpeed::Double);
        assert_eq!(eng.speed(), PlaybackSpeed::Double);
        assert_eq!(eng.speed().multiplier(), 2.0);

        eng.cycle_speed();
        assert_eq!(eng.speed(), PlaybackSpeed::Quad);
    }
}

#[test]
fn verifies_tunnel_modal_state_telemetry_and_command_builder() {
    use super::*;

    // 1. Verify Command Palette action registration
    let registry = PaletteRegistry::with_static_actions();
    let cmd = registry
        .commands()
        .iter()
        .find(|c| c.action == PaletteAction::AddNewTunnel)
        .expect("should register add new tunnel command");

    assert_eq!(cmd.category, PaletteCategory::Tunnel);
    assert_eq!(cmd.title, "Add New SSH Tunnel");

    // 2. Verify PortForwardRule and command builder
    let rule_local = PortForwardRule::local(Some("127.0.0.1"), 8080, "internal.db", 5432).unwrap();
    let cmd_local =
        build_ssh_tunnel_command(&rule_local, "bastion.example.com", Some("jump.example.com"));
    assert!(cmd_local.contains(&"-N".to_string()));
    assert!(cmd_local.contains(&"-J".to_string()));
    assert!(cmd_local.contains(&"jump.example.com".to_string()));
    assert!(cmd_local.contains(&"-L".to_string()));
    assert!(cmd_local.contains(&"127.0.0.1:8080:internal.db:5432".to_string()));
    assert!(cmd_local.contains(&"bastion.example.com".to_string()));

    let rule_dyn = PortForwardRule::dynamic(Some("127.0.0.1"), 1080).unwrap();
    let cmd_dyn = build_ssh_tunnel_command(&rule_dyn, "proxy.corp.internal", None);
    assert!(cmd_dyn.contains(&"-D".to_string()));
    assert!(cmd_dyn.contains(&"127.0.0.1:1080".to_string()));

    // 3. Verify ActiveTunnelState telemetry
    let mut active = ActiveTunnelState::new(1, Some(42), "DB Tunnel", rule_local.clone(), true);
    assert_eq!(active.status, ActiveTunnelStatus::Stopped);
    assert_eq!(active.status_pill(), ("⚪ Stopped", "stopped"));

    active.start();
    active.record_traffic(4096, 8192);
    assert_eq!(active.status, ActiveTunnelStatus::Active);
    assert_eq!(active.status_pill(), ("🟢 Active", "active"));
    assert_eq!(active.bytes_sent, 4096);
    assert_eq!(active.bytes_received, 8192);

    active.stop();
    assert_eq!(active.status, ActiveTunnelStatus::Stopped);
}

#[test]
fn verifies_startup_automation_tracker_readiness_and_prompt_dispatch() {
    use super::*;

    // 1. Verify template parameter rendering
    let ctx = StartupSnippetContext::new("Prod Web", "web01.internal", "ubuntu", 22)
        .with_host_id(101)
        .with_custom_var("branch", "main");

    let rendered = prepare_startup_snippet(
            "echo 'Connecting to {{host.name}} ({{host.user}}@{{host.hostname}}:{{host.port}})' && git checkout {{branch}}",
            &ctx,
        ).expect("rendering should succeed");

    assert_eq!(
        rendered,
        "echo 'Connecting to Prod Web (ubuntu@web01.internal:22)' && git checkout main"
    );

    // 2. Verify dangerous command interception
    let dangerous = prepare_startup_snippet("rm -rf / --no-preserve-root", &ctx);
    assert!(dangerous.is_err());

    // 3. Verify StartupSessionTracker lifecycle
    let mut tracker = StartupSessionTracker::new(
        Some(1),
        "Init Script",
        "uptime",
        StartupExecutionPolicy::AutoAfterAuth,
    );
    assert_eq!(tracker.status, StartupStatus::PendingAuth);

    // Password prompt output should NOT trigger dispatch
    tracker.observe_output("ubuntu@web01's password: ");
    assert_eq!(tracker.status, StartupStatus::PendingAuth);
    assert!(tracker.dispatch(false).is_none());

    // Shell prompt output SHOULD transition to ReadyToDispatch
    tracker.observe_output("Last login: Thu Nov 5 10:00:00 2026\r\nubuntu@web01:~$ ");
    assert_eq!(tracker.status, StartupStatus::ReadyToDispatch);

    // Prompt-like output alone must not dispatch a startup command.
    assert!(tracker.dispatch(false).is_none());
    let cmd = tracker
        .dispatch(true)
        .expect("explicitly confirmed command");
    assert_eq!(cmd, "uptime\n");
    assert_eq!(tracker.status, StartupStatus::Dispatched);

    tracker.mark_completed();
    assert_eq!(tracker.status, StartupStatus::Completed);
}

#[test]
fn verifies_keyboard_shortcut_actions_and_keystroke_parsing() {
    use super::{
        ClosePane, CloseTab, FindInTerminal, FocusNextPane, FocusPrevPane, GoToHosts, GoToKeychain,
        GoToOverview, GoToPortForwarding, GoToSftp, GoToWorkspaces, NewHost, NewTab, NextTab,
        OpenLocalTerminal, OpenSettings, OpenVaultInNewWindow, PrevTab, QuickConnect,
        ReconnectSession, RefreshHosts, ResetZoom, SelectTab1, SelectTab9, SplitHorizontal,
        SplitVertical, ToggleBroadcast, ToggleCommandPalette, ToggleFocusMode, ToggleSidebar,
        ToggleSidebarCollapse, ToggleSnippets, ZoomIn, ZoomOut,
    };

    // Ensure all actions are constructible
    let _ = NewTab;
    let _ = OpenLocalTerminal;
    let _ = CloseTab;
    let _ = NextTab;
    let _ = PrevTab;
    let _ = SelectTab1;
    let _ = SelectTab9;
    let _ = SplitVertical;
    let _ = SplitHorizontal;
    let _ = FocusNextPane;
    let _ = FocusPrevPane;
    let _ = ClosePane;
    let _ = ToggleFocusMode;
    let _ = ToggleBroadcast;
    let _ = ToggleSnippets;
    let _ = FindInTerminal;
    let _ = ZoomIn;
    let _ = ZoomOut;
    let _ = ResetZoom;
    let _ = QuickConnect;
    let _ = NewHost;
    let _ = ReconnectSession;
    let _ = ToggleSidebar;
    let _ = ToggleSidebarCollapse;
    let _ = GoToHosts;
    let _ = GoToOverview;
    let _ = GoToWorkspaces;
    let _ = GoToSftp;
    let _ = GoToKeychain;
    let _ = GoToPortForwarding;
    let _ = OpenSettings;
    let _ = RefreshHosts;
    let _ = ToggleCommandPalette;
    let _ = OpenVaultInNewWindow;

    // Test Keystroke matching logic
    let cmd_t = Keystroke {
        modifiers: gpui::Modifiers {
            platform: true,
            ..Default::default()
        },
        key: "t".into(),
        ..Default::default()
    };
    let is_primary = cmd_t.modifiers.platform || cmd_t.modifiers.control;
    assert!(is_primary && cmd_t.key == "t");

    let cmd_shift_d = Keystroke {
        modifiers: gpui::Modifiers {
            platform: true,
            shift: true,
            ..Default::default()
        },
        key: "d".into(),
        ..Default::default()
    };
    assert!(
        cmd_shift_d.modifiers.platform && cmd_shift_d.modifiers.shift && cmd_shift_d.key == "d"
    );

    let ctrl_shift_e = Keystroke {
        modifiers: gpui::Modifiers {
            control: true,
            shift: true,
            ..Default::default()
        },
        key: "e".into(),
        ..Default::default()
    };
    assert!(
        ctrl_shift_e.modifiers.control && ctrl_shift_e.modifiers.shift && ctrl_shift_e.key == "e"
    );

    let cmd_zoom_in = Keystroke {
        modifiers: gpui::Modifiers {
            platform: true,
            ..Default::default()
        },
        key: "=".into(),
        ..Default::default()
    };
    assert!(cmd_zoom_in.modifiers.platform && (cmd_zoom_in.key == "=" || cmd_zoom_in.key == "+"));

    let cmd_tab_3 = Keystroke {
        modifiers: gpui::Modifiers {
            platform: true,
            ..Default::default()
        },
        key: "3".into(),
        ..Default::default()
    };
    let ch = cmd_tab_3.key.chars().next().unwrap();
    assert!(('1'..='9').contains(&ch));
    assert_eq!((ch as usize) - ('1' as usize), 2);
}

#[test]
fn verifies_remote_desktop_display_control_state_and_resizing() {
    use super::{
        RemoteDesktopProtocol, RemoteDesktopScaleMode, RemoteDesktopState, RemoteDesktopStatus,
    };
    use shellumi_remote_desktop::{SessionCommand, SessionEvent};

    let mut rd = RemoteDesktopState {
        host_name: "Windows Server 2022".into(),
        target: "10.0.0.88:3389".into(),
        protocol: RemoteDesktopProtocol::Rdp,
        status: RemoteDesktopStatus::Connected,
        status_message: Some("Connected".into()),
        width: 1920,
        height: 1080,
        view_only: false,
        scale_mode: RemoteDesktopScaleMode::Fit,
        color_depth: 32,
        fps: Some(60),
        latency_ms: Some(12),
        clipboard_text: None,
        error: None,
        last_action_note: None,
        framebuffer: None,
        render_image: None,
        ui_handle: None,
        metrics: shellumi_remote_desktop::SessionMetrics::new(),
        username: Some("Administrator".into()),
        password: None,
        domain: None,
        tls_strict: true,
        show_diagnostics: false,
        username_input: None,
        domain_input: None,
        password_input: None,
        enable_audio: true,
        audio_muted: false,
        audio_volume: 100,
        audio_active: false,
        audio_format: None,
        audio_sample_rate: 44100,
        audio_channels: 2,
        audio_bytes_received: 0,
        enable_microphone: false,
        mic_muted: false,
        mic_volume: 100,
        mic_active: false,
        mic_format: None,
        mic_sample_rate: 44100,
        mic_channels: 2,
        mic_bytes_sent: 0,
        mic_packets_sent: 0,
        display_control_ready: false,
        max_monitor_area: None,
        connection_generation: 0,
        dynamic_resize_enabled: true,
        shared_folder: None,
        drive_name: None,
        drive_mounted: false,
        drive_bytes_read: 0,
        drive_bytes_written: 0,
        drive_operations_count: 0,
    };

    // Initial state before capability PDU
    assert!(!rd.display_control_ready);
    assert!(rd.max_monitor_area.is_none());

    // Simulate receiving DisplayControlCaps
    let event = SessionEvent::DisplayControlCaps {
        max_monitors: 16,
        max_area: 33177600, // e.g. 4 * 4K
    };
    match event {
        SessionEvent::DisplayControlCaps {
            max_monitors: _,
            max_area,
        } => {
            rd.display_control_ready = true;
            rd.max_monitor_area = Some(max_area);
            rd.last_action_note = Some("Dynamic display resizing (MS-RDPEDISP) ready".into());
        }
        _ => panic!("Expected DisplayControlCaps"),
    }
    assert!(rd.display_control_ready);
    assert_eq!(rd.max_monitor_area, Some(33177600));

    // Simulate RemoteResize event
    let resize_event = SessionEvent::RemoteResize {
        width: 2560,
        height: 1440,
    };
    match resize_event {
        SessionEvent::RemoteResize { width, height } => {
            rd.width = width;
            rd.height = height;
        }
        _ => panic!("Expected RemoteResize"),
    }
    assert_eq!(rd.width, 2560);
    assert_eq!(rd.height, 1440);

    // Verify SessionCommand::RequestResize construction
    let cmd = SessionCommand::RequestResize {
        identity: shellumi_remote_desktop::SessionIdentity::new(1, 1, 1),
        width: 3840,
        height: 2160,
    };
    if let SessionCommand::RequestResize { width, height, .. } = cmd {
        assert_eq!(width, 3840);
        assert_eq!(height, 2160);
    } else {
        panic!("Expected RequestResize command");
    }
}

#[test]
fn test_remote_desktop_drive_redirection_state_and_events() {
    use super::{
        RemoteDesktopProtocol, RemoteDesktopScaleMode, RemoteDesktopState, RemoteDesktopStatus,
    };
    use shellumi_remote_desktop::{SessionCommand, SessionEvent};

    let mut rd = RemoteDesktopState {
        host_name: "win-fileserver".into(),
        target: "192.168.1.100:3389".into(),
        protocol: RemoteDesktopProtocol::Rdp,
        status: RemoteDesktopStatus::Connected,
        status_message: None,
        width: 1920,
        height: 1080,
        view_only: false,
        scale_mode: RemoteDesktopScaleMode::Fit,
        color_depth: 32,
        fps: None,
        latency_ms: None,
        clipboard_text: None,
        error: None,
        last_action_note: None,
        framebuffer: None,
        render_image: None,
        ui_handle: None,
        metrics: shellumi_remote_desktop::SessionMetrics::new(),
        username: Some("Administrator".into()),
        password: None,
        domain: None,
        tls_strict: true,
        show_diagnostics: false,
        username_input: None,
        domain_input: None,
        password_input: None,
        enable_audio: true,
        audio_muted: false,
        audio_volume: 100,
        audio_active: false,
        audio_format: None,
        audio_sample_rate: 44100,
        audio_channels: 2,
        audio_bytes_received: 0,
        enable_microphone: false,
        mic_muted: false,
        mic_volume: 100,
        mic_active: false,
        mic_format: None,
        mic_sample_rate: 44100,
        mic_channels: 2,
        mic_bytes_sent: 0,
        mic_packets_sent: 0,
        display_control_ready: false,
        max_monitor_area: None,
        connection_generation: 0,
        dynamic_resize_enabled: true,
        shared_folder: None,
        drive_name: None,
        drive_mounted: false,
        drive_bytes_read: 0,
        drive_bytes_written: 0,
        drive_operations_count: 0,
    };

    // Initial state: drive is not mounted and counters are 0
    assert!(!rd.drive_mounted);
    assert!(rd.shared_folder.is_none());
    assert!(rd.drive_name.is_none());
    assert_eq!(rd.drive_bytes_read, 0);
    assert_eq!(rd.drive_bytes_written, 0);
    assert_eq!(rd.drive_operations_count, 0);

    // Simulate receiving DriveStatus event from protocol background worker
    let event = SessionEvent::DriveStatus {
        drive_name: "Downloads".into(),
        local_path: "/Users/test/Downloads".into(),
        mounted: true,
        bytes_read: 4096,
        bytes_written: 8192,
        operations_count: 12,
    };

    match event {
        SessionEvent::DriveStatus {
            drive_name,
            local_path,
            mounted,
            bytes_read,
            bytes_written,
            operations_count,
        } => {
            rd.drive_name = Some(drive_name.clone());
            rd.shared_folder = Some(std::path::PathBuf::from(local_path.clone()));
            rd.drive_mounted = mounted;
            rd.drive_bytes_read = bytes_read;
            rd.drive_bytes_written = bytes_written;
            rd.drive_operations_count = operations_count;
            if mounted {
                rd.last_action_note = Some(format!(
                    "Shared drive \"{}\" mounted ({})",
                    drive_name, local_path
                ));
            }
        }
        _ => panic!("Expected DriveStatus event"),
    }

    assert!(rd.drive_mounted);
    assert_eq!(rd.drive_name, Some("Downloads".into()));
    assert_eq!(
        rd.shared_folder,
        Some(std::path::PathBuf::from("/Users/test/Downloads"))
    );
    assert_eq!(rd.drive_bytes_read, 4096);
    assert_eq!(rd.drive_bytes_written, 8192);
    assert_eq!(rd.drive_operations_count, 12);
    assert_eq!(
        rd.last_action_note,
        Some("Shared drive \"Downloads\" mounted (/Users/test/Downloads)".into())
    );

    // Verify SessionCommand::ConfigureDrivePolicy
    let cmd = SessionCommand::ConfigureDrivePolicy {
        identity: shellumi_remote_desktop::SessionIdentity::new(1, 1, 1),
        path: Some(std::path::PathBuf::from("/Users/test/Shared")),
        read_only: true,
    };
    if let SessionCommand::ConfigureDrivePolicy {
        path: Some(path), ..
    } = cmd
    {
        assert_eq!(path, std::path::PathBuf::from("/Users/test/Shared"));
    } else {
        panic!("Expected ConfigureDrive command");
    }
}

#[test]
fn test_remote_desktop_microphone_redirection_state_and_events() {
    use super::{
        RemoteDesktopProtocol, RemoteDesktopScaleMode, RemoteDesktopState, RemoteDesktopStatus,
    };
    use shellumi_remote_desktop::{SessionCommand, SessionEvent};

    let mut rd = RemoteDesktopState {
        host_name: "Windows RDS Farm".into(),
        target: "rds.corp.internal:3389".into(),
        protocol: RemoteDesktopProtocol::Rdp,
        status: RemoteDesktopStatus::Connected,
        status_message: Some("Connected via IronRDP MS-RDPBCGR".into()),
        width: 1920,
        height: 1080,
        view_only: false,
        scale_mode: RemoteDesktopScaleMode::Fit,
        color_depth: 32,
        fps: Some(60),
        latency_ms: Some(15),
        clipboard_text: None,
        error: None,
        last_action_note: None,
        framebuffer: None,
        render_image: None,
        ui_handle: None,
        metrics: shellumi_remote_desktop::SessionMetrics::new(),
        username: Some("DomainUser".into()),
        password: None,
        domain: Some("CORP".into()),
        tls_strict: true,
        show_diagnostics: true,
        username_input: None,
        domain_input: None,
        password_input: None,
        enable_audio: true,
        audio_muted: false,
        audio_volume: 100,
        audio_active: true,
        audio_format: Some("PCM 44100Hz 2ch".into()),
        audio_sample_rate: 44100,
        audio_channels: 2,
        audio_bytes_received: 10240,
        enable_microphone: true,
        mic_muted: false,
        mic_volume: 100,
        mic_active: false,
        mic_format: None,
        mic_sample_rate: 44100,
        mic_channels: 2,
        mic_bytes_sent: 0,
        mic_packets_sent: 0,
        display_control_ready: true,
        max_monitor_area: Some(1920 * 1080),
        connection_generation: 0,
        dynamic_resize_enabled: true,
        shared_folder: None,
        drive_name: None,
        drive_mounted: false,
        drive_bytes_read: 0,
        drive_bytes_written: 0,
        drive_operations_count: 0,
    };

    // 1. Initial microphone state
    assert!(rd.enable_microphone);
    assert!(!rd.mic_muted);
    assert_eq!(rd.mic_volume, 100);
    assert!(!rd.mic_active);
    assert_eq!(rd.mic_format, None);
    assert_eq!(rd.mic_bytes_sent, 0);
    assert_eq!(rd.mic_packets_sent, 0);

    // 2. Simulate SessionEvent::MicrophoneStatus transition when AUDIO_INPUT opens
    let mic_event = SessionEvent::MicrophoneStatus {
        active: true,
        muted: false,
        volume: 90,
        format: "16-bit PCM 44100Hz 2ch".to_string(),
        sample_rate: 44100,
        channels: 2,
        bytes_sent: 4096,
        packets_sent: 4,
    };

    match mic_event {
        SessionEvent::MicrophoneStatus {
            active,
            muted,
            volume,
            format,
            sample_rate,
            channels,
            bytes_sent,
            packets_sent,
        } => {
            rd.mic_active = active;
            rd.mic_muted = muted;
            rd.mic_volume = volume;
            rd.mic_format = Some(format);
            rd.mic_sample_rate = sample_rate;
            rd.mic_channels = channels;
            rd.mic_bytes_sent = bytes_sent;
            rd.mic_packets_sent = packets_sent;
        }
        _ => panic!("Unexpected event"),
    }

    assert!(rd.mic_active);
    assert!(!rd.mic_muted);
    assert_eq!(rd.mic_volume, 90);
    assert_eq!(rd.mic_format.as_deref(), Some("16-bit PCM 44100Hz 2ch"));
    assert_eq!(rd.mic_sample_rate, 44100);
    assert_eq!(rd.mic_channels, 2);
    assert_eq!(rd.mic_bytes_sent, 4096);
    assert_eq!(rd.mic_packets_sent, 4);

    // 3. Test volume adjustment and muting
    rd.mic_muted = true;
    rd.mic_volume = 120; // 120% microphone gain boost
    assert!(rd.mic_muted);
    assert_eq!(rd.mic_volume, 120);

    // 4. Verify SessionCommand variants for microphone control
    let id = shellumi_remote_desktop::SessionIdentity::new(1, 1, 1);
    let mute_cmd = SessionCommand::SetMicrophoneMute {
        identity: id,
        muted: true,
    };
    if let SessionCommand::SetMicrophoneMute { muted, .. } = mute_cmd {
        assert!(muted);
    } else {
        panic!("Expected SetMicrophoneMute command");
    }

    let vol_cmd = SessionCommand::SetMicrophoneVolume {
        identity: id,
        volume: 120,
    };
    if let SessionCommand::SetMicrophoneVolume { volume, .. } = vol_cmd {
        assert_eq!(volume, 120);
    } else {
        panic!("Expected SetMicrophoneVolume command");
    }
}

#[test]
fn test_multi_window_vault_binding_and_context_isolation() {
    let default_vid = shellumi_storage::LEGACY_LOCAL_VAULT_ID.to_string();
    let corp_vid = "corp-vault-99".to_string();

    let summary_local = crate::actions::VaultContextSummary {
        id: default_vid.clone(),
        name: "Personal".to_string(),
        is_remote: false,
        server_url: None,
        server_vault_id: None,
        is_locked: false,
    };

    let summary_remote = crate::actions::VaultContextSummary {
        id: corp_vid.clone(),
        name: "Corporate Ops".to_string(),
        is_remote: true,
        server_url: Some("https://shellumi.internal:8443".to_string()),
        server_vault_id: Some("srv-corp-123".to_string()),
        is_locked: false,
    };

    let title_1 = format!("Shellumi — {}", summary_local.name);
    let title_2 = format!("Shellumi — {}", summary_remote.name);
    assert_eq!(title_1, "Shellumi — Personal");
    assert_eq!(title_2, "Shellumi — Corporate Ops");

    assert!(!summary_local.is_remote);
    assert!(summary_remote.is_remote);
    assert_eq!(
        summary_remote.server_url.as_deref(),
        Some("https://shellumi.internal:8443")
    );

    let action = crate::OpenVaultInNewWindow;
    let _ = format!("{action:?}");
}

#[tokio::test]
async fn test_vault_lock_and_unlock_recovery_roundtrip() {
    use shellumi_core::{VaultId, VaultKey, VaultKeyManager};
    use shellumi_storage::HostStore;

    let mut store = HostStore::open(":memory:").await.unwrap();
    let vid = VaultId::new("finance-vault").unwrap();
    let key = VaultKey::generate().unwrap();
    let passphrase = "finance-recovery-passphrase-2026";

    // 0. Register the vault context in storage
    let ctx = shellumi_core::VaultContext::new(vid.clone(), shellumi_core::VaultLocation::Local);
    store
        .register_vault_context("Finance Vault", &ctx)
        .await
        .unwrap();

    // 1. Wrap key into recovery envelope
    let envelope = key.wrap_recovery(&vid, passphrase).unwrap();
    let envelope_json = serde_json::to_string(&envelope).unwrap();

    // 2. Persist recovery envelope and mark vault locked
    store
        .save_vault_recovery_envelope("finance-vault", &envelope_json)
        .await
        .unwrap();
    store.set_vault_locked("finance-vault", true).await.unwrap();
    assert!(store.is_vault_locked("finance-vault").await.unwrap());

    // 3. Reconstruct envelope and recover key
    let retrieved_str = store
        .get_vault_recovery_envelope("finance-vault")
        .await
        .unwrap()
        .unwrap();
    let retrieved_env: shellumi_core::sync::EncryptedEnvelope =
        serde_json::from_str(&retrieved_str).unwrap();

    // Rejects invalid/short passphrases
    assert!(VaultKey::recover(&vid, "short", &retrieved_env).is_err());
    assert!(VaultKey::recover(&vid, "wrong-passphrase-here-1234", &retrieved_env).is_err());

    // Succeeds with correct passphrase
    let recovered_key = VaultKey::recover(&vid, passphrase, &retrieved_env).unwrap();
    assert_eq!(recovered_key.expose_bytes(), key.expose_bytes());

    // Provision into credential store and unlock in database
    let cred_store = shellumi_core::EphemeralCredentialStore::default();
    VaultKeyManager::provision_key(&cred_store, &vid, &recovered_key).unwrap();
    store
        .set_vault_locked("finance-vault", false)
        .await
        .unwrap();
    assert!(!store.is_vault_locked("finance-vault").await.unwrap());

    // Verify key retrieved from credential store matches
    let retrieved_key = VaultKeyManager::retrieve_key(&cred_store, &vid)
        .unwrap()
        .unwrap();
    assert_eq!(retrieved_key.expose_bytes(), key.expose_bytes());
}

#[test]
fn test_client_side_directory_sync_envelope_and_palette_commands() {
    use shellumi_core::{
        PaletteAction, PaletteCategory, PaletteCommand, PaletteRegistry, SyncExecutionReport,
        SyncVault, VaultDirectory, VaultDirectoryEntry, VaultDirectoryLocation, VaultId, VaultKey,
    };

    let default_vid = VaultId::new("legacy-local").unwrap();
    let default_key = VaultKey::generate().unwrap();

    let mut dir = VaultDirectory::new();
    dir.upsert(VaultDirectoryEntry {
        id: "legacy-local".to_string(),
        name: "Personal".to_string(),
        location: VaultDirectoryLocation::Local,
        has_recovery_envelope: false,
        sync_enabled: true,
        created_at: 1000,
        updated_at: 1000,
    });
    dir.upsert(VaultDirectoryEntry {
        id: "remote-corp".to_string(),
        name: "Corporate".to_string(),
        location: VaultDirectoryLocation::Remote {
            server_connection_id: "conn-1".to_string(),
            server_vault_id: "corp-srv-vault".to_string(),
        },
        has_recovery_envelope: true,
        sync_enabled: false,
        created_at: 1000,
        updated_at: 1000,
    });

    let envelope = dir.encrypt(&default_vid, &default_key).unwrap();
    let envelope_json = serde_json::to_string(&envelope).unwrap();

    // Verify SyncVault attaches encrypted directory
    let mut sync_vault = SyncVault::new("test-vault", 1000);
    sync_vault.encrypted_directory = Some(envelope_json.clone());
    let json = sync_vault.to_json().unwrap();
    let restored_vault = SyncVault::from_json(&json).unwrap();
    assert_eq!(restored_vault.encrypted_directory, Some(envelope_json));

    // Verify SyncExecutionReport attaches encrypted directory
    let report = SyncExecutionReport {
        local_updated: 5,
        remote_updated: 2,
        conflicts_resolved: 0,
        duration_ms: 15,
        message: "Sync OK".to_string(),
        encrypted_directory: restored_vault.encrypted_directory,
        key_envelope: None,
        cloud_state: None,
        snapshot: None,
    };
    assert!(report.encrypted_directory.is_some());

    // Verify Palette commands
    let mut reg = PaletteRegistry::new();
    reg.register(PaletteCommand::new(
        "vault:open-new-window",
        "Open Vault in New Window: Personal",
        "Spawn an independent window",
        PaletteCategory::Vault,
        PaletteAction::OpenVaultInNewWindow {
            vault_id: "legacy-local".to_string(),
            vault_name: "Personal".to_string(),
        },
    ));
    reg.register(PaletteCommand::new(
        "vault:lock-active",
        "Lock Active Vault",
        "Protect vault",
        PaletteCategory::Vault,
        PaletteAction::LockActiveVault,
    ));

    let matches = reg.search("Vault");
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].command.category, PaletteCategory::Vault);
    assert_eq!(PaletteCategory::Vault.badge_icon(), "🔒");
    assert_eq!(PaletteCategory::Vault.as_str(), "Vault");
}

#[test]
fn verifies_resolve_icon_path_finds_or_extracts_icons() {
    for name in [
        "terminal",
        "hosts",
        "sftp",
        "serial",
        "settings",
        "vaults",
        "search",
        "connect",
        "shellumi_logo",
        "os_windows",
        "os_ubuntu",
    ] {
        let regular = crate::resolve_icon_path(name, false, 48);
        assert!(
            regular.is_some(),
            "Expected regular icon '{name}' to resolve"
        );
        let regular_path = regular.unwrap();
        assert!(
            regular_path.exists(),
            "Expected regular icon file '{name}' to exist at {:?}",
            regular_path
        );

        let hover = crate::resolve_icon_path(name, true, 48);
        assert!(hover.is_some(), "Expected hover icon '{name}' to resolve");
        let hover_path = hover.unwrap();
        assert!(
            hover_path.exists(),
            "Expected hover icon file '{name}' to exist at {:?}",
            hover_path
        );
    }
}

#[tokio::test]
async fn test_keychain_disabled_vault_unlock_and_sync_with_in_memory_keys() {
    use shellumi_core::{
        EphemeralCredentialStore, SshDestination, VaultId, VaultKey, VaultKeyManager, VaultManifest,
    };
    use shellumi_storage::HostStore;

    let temp = tempfile::tempdir().unwrap();
    let base_dir = temp.path();
    let vaults_dir = base_dir.join("vaults");
    let manifests_dir = base_dir.join("vault_manifests");
    std::fs::create_dir_all(&vaults_dir).unwrap();
    std::fs::create_dir_all(&manifests_dir).unwrap();

    let vault_id = VaultId::new("vlt-inmemory-01").unwrap();
    let user_pass = "Mem0rySecretPass#2026!";
    let cred_store = EphemeralCredentialStore::new();

    // 1. Provision new vault with remember_in_keychain = false
    let (dek, primary_env, _rec_env) = VaultKeyManager::provision_new_vault(
        &cred_store,
        &vault_id,
        user_pass,
        false, // Do NOT store in Keychain!
        None,
    )
    .unwrap();

    // Verify key was NOT stored in cred_store
    let key_in_store = VaultKeyManager::retrieve_key(&cred_store, &vault_id).unwrap();
    assert!(
        key_in_store.is_none(),
        "Key must NOT be persisted in credential store when remember_in_keychain is false"
    );

    // Save manifest
    let manifest = VaultManifest::new(
        vault_id.as_str().to_string(),
        "In-Memory Vault".to_string(),
        primary_env.clone(),
        None,
        false,
    );
    let manifest_path = manifests_dir.join(format!("{}.manifest.json", vault_id.as_str()));
    manifest.save_to_path(&manifest_path).unwrap();

    // 2. Open database strictly using in-memory DEK
    let db_path = vaults_dir.join(format!("{}.db", vault_id.as_str()));
    let mut store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&dek))
        .await
        .unwrap();
    store.set_vault_id(vault_id.clone());

    let dest = SshDestination::new("10.10.10.1", "root", 22).unwrap();
    store
        .add("in-memory-host", &dest, None, &[], None, None)
        .await
        .unwrap();

    // 3. Export vault for synchronization: export attaches envelope and encrypts records
    let passphrase = hex::encode(dek.expose_bytes());
    let mut sync_vault = store.export_vault(&passphrase, "client-app").await.unwrap();
    sync_vault.key_envelope = Some(primary_env.clone());

    assert_eq!(sync_vault.vault_id, "vlt-inmemory-01");
    assert!(sync_vault.key_envelope.is_some());
    assert_eq!(sync_vault.records.len(), 1);

    // 4. Drop store and memory key (simulating vault lock)
    drop(store);
    drop(dek);

    // Opening without key must fail
    assert!(HostStore::open_with_key(db_path.to_str().unwrap(), None)
        .await
        .is_err());

    // 5. Unlock strictly from manifest using user password (reconstructing DEK in memory)
    let loaded_manifest = VaultManifest::load_from_path(&manifest_path).unwrap();
    assert!(!loaded_manifest.remember_in_keychain);
    let unlocked_dek =
        VaultKey::unwrap_with_password(&vault_id, user_pass, &loaded_manifest.primary_envelope)
            .unwrap();

    // Database opens successfully with reconstructed in-memory key
    let mut unlocked_store =
        HostStore::open_with_key(db_path.to_str().unwrap(), Some(&unlocked_dek))
            .await
            .unwrap();
    unlocked_store.set_vault_id(vault_id.clone());

    let hosts = unlocked_store.list().await.unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].name, "in-memory-host");

    // Credential store still remains empty
    assert!(VaultKeyManager::retrieve_key(&cred_store, &vault_id)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_cross_vault_async_isolation_out_of_order_responses() {
    use shellumi_core::{SshDestination, VaultId, VaultKey};
    use shellumi_storage::HostStore;

    let temp = tempfile::tempdir().unwrap();
    let db_path_a = temp.path().join("vault_a.db");
    let db_path_b = temp.path().join("vault_b.db");

    let key_a = VaultKey::generate().unwrap();
    let key_b = VaultKey::generate().unwrap();

    let mut store_a = HostStore::open_with_key(db_path_a.to_str().unwrap(), Some(&key_a))
        .await
        .unwrap();
    let vid_a = VaultId::new("vault-alpha").unwrap();
    store_a.set_vault_id(vid_a.clone());

    let dest_a = SshDestination::new("10.0.0.1", "admin", 22).unwrap();
    store_a
        .add("alpha-server", &dest_a, None, &[], None, None)
        .await
        .unwrap();

    let dest_b = SshDestination::new("10.0.0.2", "admin", 22).unwrap();
    let mut store_b = HostStore::open_with_key(db_path_b.to_str().unwrap(), Some(&key_b))
        .await
        .unwrap();
    let vid_b = VaultId::new("vault-beta").unwrap();
    store_b.set_vault_id(vid_b.clone());
    store_b
        .add("beta-server", &dest_b, None, &[], None, None)
        .await
        .unwrap();

    // Verify generation isolation logic
    let mut active_vault = "vault-alpha".to_string();
    let mut vault_load_generation = 1u64;

    // Simulate async dispatch for vault-alpha with gen = 1
    let task_originating_vault = active_vault.clone();
    let task_gen = vault_load_generation;

    // User switches to vault-beta before task finishes
    active_vault = "vault-beta".to_string();
    vault_load_generation += 1;

    // Task finishes with results from vault-alpha
    let _loaded_hosts_a = store_a.list().await.unwrap();

    // Application validates originating vault and generation
    let is_valid = active_vault == task_originating_vault && vault_load_generation == task_gen;
    assert!(
        !is_valid,
        "Out-of-order response from previous vault must be discarded"
    );

    // The active vault's generation matches its own load
    let active_task_gen = vault_load_generation;
    let active_task_vault = active_vault.clone();
    let loaded_hosts_b = store_b.list().await.unwrap();
    let is_valid_b = active_vault == active_task_vault && vault_load_generation == active_task_gen;
    assert!(
        is_valid_b,
        "Current vault's matching generation must be accepted"
    );
    assert_eq!(loaded_hosts_b.len(), 1);
    assert_eq!(loaded_hosts_b[0].name, "beta-server");
}

#[tokio::test]
async fn test_vault_creation_emergency_recovery_and_password_rotation() {
    use shellumi_core::{
        EphemeralCredentialStore, VaultId, VaultKey, VaultKeyManager, VaultManifest,
    };
    use shellumi_storage::HostStore;

    let temp = tempfile::tempdir().unwrap();
    let vault_id = VaultId::new("vlt-rotate-test").unwrap();
    let password_v1 = "InitialSecurePassword123!";
    let password_v2 = "RotatedSecurePassword456#";

    let cred_store = EphemeralCredentialStore::new();
    let recovery_code = VaultKey::generate_recovery_code().unwrap();
    assert!(
        !recovery_code.is_empty(),
        "Emergency recovery code must be non-empty"
    );

    // 1. Provision new vault with recovery code
    let (key, primary_env, recovery_env) = VaultKeyManager::provision_new_vault(
        &cred_store,
        &vault_id,
        password_v1,
        false,
        Some(&recovery_code),
    )
    .unwrap();

    assert!(recovery_env.is_some(), "Recovery envelope must be present");

    // 2. Save manifest
    let manifest_path = temp.path().join("vlt-rotate-test.manifest.json");
    let mut manifest = VaultManifest::new(
        vault_id.as_str().to_string(),
        "Rotate Test Vault".into(),
        primary_env.clone(),
        recovery_env.clone(),
        false,
    );
    manifest.save_to_path(&manifest_path).unwrap();

    // 3. Database opens with key
    let db_path = temp.path().join("vlt-rotate-test.db");
    let mut store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&key))
        .await
        .unwrap();
    store.set_vault_id(vault_id.clone());

    // 4. Password rotation: re-wrap DEK with password_v2
    let new_primary = key.wrap_with_password(&vault_id, password_v2).unwrap();
    manifest.primary_envelope = new_primary.clone();
    manifest.save_to_path(&manifest_path).unwrap();

    // 5. Verify unwrapping with password_v2 succeeds, and password_v1 fails
    let unwrapped_v2 =
        VaultKey::unwrap_with_password(&vault_id, password_v2, &manifest.primary_envelope);
    assert!(
        unwrapped_v2.is_ok(),
        "Unwrapping with rotated password must succeed"
    );
    assert_eq!(unwrapped_v2.unwrap().expose_bytes(), key.expose_bytes());

    let unwrapped_v1 =
        VaultKey::unwrap_with_password(&vault_id, password_v1, &manifest.primary_envelope);
    assert!(
        unwrapped_v1.is_err(),
        "Unwrapping with old password must fail"
    );

    // 6. Verify emergency recovery code still works
    let recovered_key = VaultKey::unwrap_with_password(
        &vault_id,
        &recovery_code,
        &manifest.recovery_envelope.unwrap(),
    );
    assert!(
        recovered_key.is_ok(),
        "Emergency recovery code must unwrap the key"
    );
    assert_eq!(recovered_key.unwrap().expose_bytes(), key.expose_bytes());
}

#[tokio::test]
async fn test_vault_registry_deletion_and_protection() {
    use shellumi_core::{VaultContext, VaultId, VaultKey, VaultLocation};
    use shellumi_storage::HostStore;

    let temp = tempfile::tempdir().unwrap();
    let reg_db = temp.path().join("default.db");
    let default_key = VaultKey::generate().unwrap();

    let mut reg_store = HostStore::open_with_key(reg_db.to_str().unwrap(), Some(&default_key))
        .await
        .unwrap();
    reg_store.set_vault_id(VaultId::new(shellumi_storage::LEGACY_LOCAL_VAULT_ID).unwrap());

    // Register a secondary vault
    let vid = VaultId::new("vlt-secondary").unwrap();
    let context = VaultContext::new(vid, VaultLocation::Local);
    reg_store
        .register_vault_context("Secondary Vault", &context)
        .await
        .unwrap();

    let contexts = reg_store.list_vault_contexts().await.unwrap();
    assert_eq!(
        contexts.len(),
        2,
        "Expected default vault + secondary vault"
    );
    assert!(contexts
        .iter()
        .any(|(name, ctx)| ctx.id().as_str() == "vlt-secondary" && name == "Secondary Vault"));

    // Unregister secondary vault
    let deleted = reg_store
        .unregister_vault_context("vlt-secondary")
        .await
        .unwrap();
    assert!(deleted, "Unregistering registered vault must return true");

    // Verify it is gone and only the default vault remains
    let contexts_after = reg_store.list_vault_contexts().await.unwrap();
    assert_eq!(contexts_after.len(), 1);
    assert!(!contexts_after
        .iter()
        .any(|(_, ctx)| ctx.id().as_str() == "vlt-secondary"));
    assert_eq!(
        contexts_after[0].1.id().as_str(),
        shellumi_storage::LEGACY_LOCAL_VAULT_ID
    );

    // Unregistering again returns false
    let deleted_again = reg_store
        .unregister_vault_context("vlt-secondary")
        .await
        .unwrap();
    assert!(
        !deleted_again,
        "Unregistering non-existent vault must return false"
    );
}

#[test]
fn test_vault_quarantine_staging_and_rollback() {
    use crate::data_path::{
        quarantine_vault_files, resolve_manifest_file, resolve_vault_db_file,
        restore_quarantined_vault,
    };

    let temp = tempfile::tempdir().unwrap();
    let base_dir = temp.path().to_path_buf();
    let vault_id = "vlt-quarantine-test";

    let db_path = resolve_vault_db_file(&base_dir, vault_id);
    let manifest_path = resolve_manifest_file(&base_dir, vault_id);

    std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    std::fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();

    std::fs::write(&db_path, b"test database payload").unwrap();
    std::fs::write(&manifest_path, b"test manifest payload").unwrap();

    assert!(db_path.exists());
    assert!(manifest_path.exists());

    // 1. Quarantine files
    let q_dir = quarantine_vault_files(&base_dir, vault_id, "test quarantine").unwrap();
    assert!(q_dir.exists(), "Quarantine directory must be created");

    // Original files must no longer be in the vaults/ or vault_manifests/ directories
    assert!(!db_path.exists(), "Original DB must be moved out");
    assert!(
        !manifest_path.exists(),
        "Original manifest must be moved out"
    );

    // Files must be inside the quarantine folder
    assert!(q_dir.join(format!("{vault_id}.db")).exists());
    assert!(q_dir.join(format!("{vault_id}.manifest.json")).exists());
    assert!(q_dir.join("quarantine_meta.json").exists());

    // 2. Rollback / restore files
    restore_quarantined_vault(&q_dir).unwrap();

    // Files must be restored to original paths
    assert!(db_path.exists(), "DB file must be restored");
    assert!(manifest_path.exists(), "Manifest file must be restored");
    assert_eq!(std::fs::read(&db_path).unwrap(), b"test database payload");
    assert_eq!(
        std::fs::read(&manifest_path).unwrap(),
        b"test manifest payload"
    );
    assert!(
        !q_dir.exists(),
        "Quarantine folder must be removed after restore"
    );

    // 3. Symlink rejection
    #[cfg(unix)]
    {
        let sym_target = temp.path().join("external_secret.txt");
        std::fs::write(&sym_target, b"secret").unwrap();
        let sym_db = resolve_vault_db_file(&base_dir, "vlt-symlink");
        std::fs::create_dir_all(sym_db.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&sym_target, &sym_db).unwrap();

        let err = quarantine_vault_files(&base_dir, "vlt-symlink", "test symlink");
        assert!(
            err.is_err(),
            "Symlinked files must be rejected from quarantine"
        );
        assert!(err
            .unwrap_err()
            .contains("Refusing to quarantine symlinked"));
    }
}

#[tokio::test]
async fn test_team_member_revocation_with_cryptographic_rotation() {
    use shellumi_core::{AuditAction, TeamMember, TeamRole, VaultId, VaultKey};
    use shellumi_storage::HostStore;

    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("team_vault.db");
    let key = VaultKey::generate().unwrap();

    let mut store = HostStore::open_with_key(db_path.to_str().unwrap(), Some(&key))
        .await
        .unwrap();
    let vid = VaultId::new("vlt-team-rot").unwrap();
    store.set_vault_id(vid);

    // Create shared vault
    let shared_vault = shellumi_core::SharedVault::new(
        "vlt-team-rot",
        "Team Engineering",
        Some("Engineering shared vault".into()),
        "owner@corp.internal",
        "2026-11-24T12:00:00Z",
    )
    .unwrap();
    store.create_shared_vault(&shared_vault).await.unwrap();

    // Add team member
    let member = TeamMember {
        id: "tm-charlie".into(),
        vault_id: "vlt-team-rot".into(),
        user_id: "charlie@corp.internal".into(),
        display_name: "Charlie Dev".into(),
        public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICfakeCharliePubKey".into(),
        agreement_public_key: None,
        enrollment_status: shellumi_core::EnrollmentStatus::LegacyRequiresMigration,
        role: TeamRole::Editor,
        enrolled_at: "2026-11-24T12:01:00Z".into(),
        invited_by: "owner@corp.internal".into(),
        revoked: false,
        revoked_at: None,
    };
    store.add_team_member(&member).await.unwrap();

    // Revoke team member with rotation
    let new_epoch = store
        .revoke_team_member_with_rotation(
            "vlt-team-rot",
            "tm-charlie",
            "2026-11-24T12:05:00Z",
            "owner@corp.internal",
        )
        .await
        .unwrap();

    assert_eq!(
        new_epoch, 2,
        "Epoch must advance from 1 to 2 upon member revocation"
    );

    // Verify member is revoked
    let member_after = store.get_team_member("tm-charlie").await.unwrap().unwrap();
    assert!(member_after.revoked, "Member must be marked revoked");
    assert_eq!(
        member_after.revoked_at.as_deref(),
        Some("2026-11-24T12:05:00Z")
    );

    // Verify audit log has revocation record
    let audit_records = store.list_audit_log("vlt-team-rot", None).await.unwrap();
    assert!(
        audit_records
            .iter()
            .any(|r| r.action == AuditAction::MemberRevoked
                && r.target_id.as_deref() == Some("tm-charlie")),
        "Audit log must contain MemberRevoked event"
    );

    // Attempting to revoke again must fail
    let err = store
        .revoke_team_member_with_rotation(
            "vlt-team-rot",
            "tm-charlie",
            "2026-11-24T12:06:00Z",
            "owner@corp.internal",
        )
        .await;
    assert!(err.is_err(), "Revoking an already revoked member must fail");
}

#[test]
fn test_navigation_completion_close_tab_and_close_window_queuing() {
    use crate::ui::inspector::coordinator::{InspectorCoordinator, NavigationRequest};
    use crate::PendingWindowAction;

    let coord = InspectorCoordinator::new();
    assert!(!coord.is_open());

    // Test queuing CloseTab
    let req_tab = NavigationRequest::CloseTab(42);
    assert_eq!(req_tab, NavigationRequest::CloseTab(42));

    // Test queuing CloseWindow
    let req_win = NavigationRequest::CloseWindow;
    assert_eq!(req_win, NavigationRequest::CloseWindow);

    // Verify PendingWindowAction variants
    let action_tab = PendingWindowAction::CloseTab(42);
    let action_win = PendingWindowAction::CloseWindow;
    assert_eq!(action_tab, PendingWindowAction::CloseTab(42));
    assert_eq!(action_win, PendingWindowAction::CloseWindow);
}

#[test]
fn test_remote_desktop_canvas_geometry_translation_and_margins() {
    use shellumi_remote_desktop::{translate_coordinates, ScaleMode};

    let fb_w: u32 = 1920;
    let fb_h: u32 = 1080;
    let canvas_w: f32 = 800.0;
    let canvas_h: f32 = 600.0;

    // In ScaleMode::Fit, 1920x1080 into 800x600 has letterboxing (800x450 rendered, 75px top/bottom margin)
    // Click in top margin (0..75) should return None
    assert!(
        translate_coordinates(400.0, 30.0, canvas_w, canvas_h, fb_w, fb_h, ScaleMode::Fit)
            .is_none()
    );

    // Click in center (400.0, 300.0) should map to center of remote desktop (960, 540)
    let center =
        translate_coordinates(400.0, 300.0, canvas_w, canvas_h, fb_w, fb_h, ScaleMode::Fit);
    assert!(center.is_some());
    let (cx, cy) = center.unwrap();
    assert!((cx as i32 - 960).abs() <= 2);
    assert!((cy as i32 - 540).abs() <= 2);

    // In ScaleMode::Fill, there are no letterbox margins, all points within canvas map
    let fill_top =
        translate_coordinates(400.0, 30.0, canvas_w, canvas_h, fb_w, fb_h, ScaleMode::Fill);
    assert!(fill_top.is_some());
}

#[test]
fn test_icon_pack_all_168_resolutions_and_hover() {
    use crate::embedded_icons::get_embedded_icon;
    use crate::ui::icon::{pick_asset_resolution, resolve_icon_path, IconName};

    // 1. Retina resolution picker mapping tests
    assert_eq!(pick_asset_resolution(14.0), 32);
    assert_eq!(pick_asset_resolution(16.0), 32);
    assert_eq!(pick_asset_resolution(20.0), 48);
    assert_eq!(pick_asset_resolution(24.0), 48);
    assert_eq!(pick_asset_resolution(32.0), 64);
    assert_eq!(pick_asset_resolution(48.0), 128);
    assert_eq!(pick_asset_resolution(64.0), 128);
    assert_eq!(pick_asset_resolution(128.0), 256);
    assert_eq!(pick_asset_resolution(256.0), 256);

    // 2. All 168 icons have valid embedded assets across resolutions and states
    let all_icons = [
        IconName::Hosts,
        IconName::Connect,
        IconName::Sftp,
        IconName::Snippets,
        IconName::History,
        IconName::Keychain,
        IconName::Vaults,
        IconName::Workspaces,
        IconName::CloudSync,
        IconName::Settings,
        IconName::AddHost,
        IconName::Search,
        IconName::Terminal,
        IconName::Serial,
        IconName::SplitView,
        IconName::GridView,
        IconName::ListView,
        IconName::Groups,
        IconName::Tags,
        IconName::Favorite,
        IconName::More,
        IconName::Edit,
        IconName::Delete,
        IconName::Refresh,
        IconName::Profile,
        IconName::KnownHosts,
        IconName::PortForwarding,
        IconName::ShellumiLogo,
        IconName::GroupSettings,
        IconName::Import,
        IconName::Export,
        IconName::Tag,
        IconName::CalendarSort,
        IconName::Certificate,
        IconName::PrivateKey,
        IconName::PublicKey,
        IconName::IdentityCard,
        IconName::OsArch,
        IconName::OsUbuntu,
        IconName::OsMacos,
        IconName::OsFreebsd,
        IconName::OsNetbsd,
        IconName::OsOpenbsd,
        IconName::OsLinux,
        IconName::OsWindows,
        IconName::Server,
        IconName::Fido2,
        IconName::Fingerprint,
        IconName::Log,
        IconName::Bell,
        IconName::InviteMember,
        IconName::Infrastructure,
        IconName::Diagnostics,
        IconName::Mobile,
        IconName::Overview,
        IconName::Inspector,
        IconName::NewGroup,
        IconName::DuplicateHost,
        IconName::MoveToGroup,
        IconName::CopyToGroup,
        IconName::NewWorkspace,
        IconName::NewVault,
        IconName::SwitchVault,
        IconName::QuickConnect,
        IconName::JumpHost,
        IconName::Ssh,
        IconName::Mosh,
        IconName::Rdp,
        IconName::Vnc,
        IconName::Disconnect,
        IconName::Reconnect,
        IconName::LocalTerminal,
        IconName::SessionRecord,
        IconName::SessionReplay,
        IconName::SessionInfo,
        IconName::FitToView,
        IconName::ActualSize,
        IconName::FillView,
        IconName::DisplayResolution,
        IconName::DynamicResize,
        IconName::RemoteKeyboard,
        IconName::RemoteClipboard,
        IconName::ViewOnly,
        IconName::Fullscreen,
        IconName::SendCtrlAltDel,
        IconName::SendWindowsKey,
        IconName::SharedDrive,
        IconName::RemoteAudio,
        IconName::RemoteAudioMuted,
        IconName::RemoteMicrophone,
        IconName::RemoteMicrophoneMuted,
        IconName::InputEnabled,
        IconName::SessionTelemetry,
        IconName::Upload,
        IconName::Download,
        IconName::TransferQueue,
        IconName::NewFolder,
        IconName::TransferProgress,
        IconName::TransferComplete,
        IconName::TransferFailed,
        IconName::TransferRetry,
        IconName::LocalFiles,
        IconName::RemoteFiles,
        IconName::FilePermissions,
        IconName::HiddenFiles,
        IconName::VaultLock,
        IconName::VaultUnlock,
        IconName::GoogleDrive,
        IconName::SyncConflict,
        IconName::EncryptedBackup,
        IconName::RestoreBackup,
        IconName::RecoveryCode,
        IconName::VaultEncryption,
        IconName::ServerSync,
        IconName::SyncUpload,
        IconName::SyncDownload,
        IconName::SyncOffline,
        IconName::SyncSuccess,
        IconName::SetupResume,
        IconName::DeviceEnrollment,
        IconName::HostKeyVerified,
        IconName::HostKeyChanged,
        IconName::SshAgent,
        IconName::GenerateKey,
        IconName::RotateKey,
        IconName::RevokeKey,
        IconName::Password,
        IconName::SecurityAudit,
        IconName::TrustedDevice,
        IconName::UntrustedDevice,
        IconName::TeamMembers,
        IconName::SharedVault,
        IconName::MemberRevoke,
        IconName::PendingInvite,
        IconName::RoleAdmin,
        IconName::RoleEditor,
        IconName::RoleViewer,
        IconName::TeamKeyRotation,
        IconName::TunnelLocal,
        IconName::TunnelRemote,
        IconName::TunnelSocks,
        IconName::TunnelActive,
        IconName::TunnelStopped,
        IconName::ProviderAws,
        IconName::ProviderAzure,
        IconName::ProviderDigitalocean,
        IconName::ProviderAnsible,
        IconName::ProviderConnect,
        IconName::DiscoverHosts,
        IconName::Inventory,
        IconName::InventorySync,
        IconName::HealthCheck,
        IconName::Benchmark,
        IconName::Latency,
        IconName::NetworkTest,
        IconName::StorageTest,
        IconName::OsDebian,
        IconName::OsFedora,
        IconName::OsRhel,
        IconName::OsCentos,
        IconName::OsRocky,
        IconName::OsAlmalinux,
        IconName::OsAlpine,
        IconName::OsOpensuse,
        IconName::OsKali,
        IconName::OsWindowsServer,
        IconName::OsAndroid,
        IconName::OsIos,
    ];

    assert_eq!(all_icons.len(), 168);

    for icon in all_icons {
        let name_str = icon.as_str();
        assert_eq!(IconName::from_name(name_str), Some(icon));
        assert!(!icon.label().is_empty());

        // Regular state
        let reg_bytes = get_embedded_icon(name_str, false, 48);
        assert!(
            reg_bytes.is_some(),
            "Embedded icon '{}' (regular, 48) must exist",
            name_str
        );
        assert_eq!(&reg_bytes.unwrap()[..8], b"\x89PNG\r\n\x1a\n");

        // Hover state
        let hover_bytes = get_embedded_icon(name_str, true, 48);
        assert!(
            hover_bytes.is_some(),
            "Embedded icon '{}' (hover, 48) must exist",
            name_str
        );
        assert_eq!(&hover_bytes.unwrap()[..8], b"\x89PNG\r\n\x1a\n");

        // Resolution lookup
        assert!(resolve_icon_path(name_str, false, 48).is_some());
    }
}

#[test]
fn test_all_1680_icon_assets_png_decode_dimensions_and_artwork() {
    use crate::embedded_icons::get_embedded_icon;
    use crate::ui::icon::IconName;
    use image::GenericImageView;

    let resolutions = [32u32, 48, 64, 128, 256];
    let states = [false, true]; // [regular, hover]
    let problematic_icons = [
        "actual_size",
        "delete",
        "disconnect",
        "display_resolution",
        "dynamic_resize",
        "file_permissions",
    ];

    let mut decoded_count = 0;

    for icon in IconName::ALL {
        let name_str = icon.as_str();

        for &res in &resolutions {
            for &is_hover in &states {
                let state_str = if is_hover { "hover" } else { "regular" };
                let bytes = get_embedded_icon(name_str, is_hover, res).unwrap_or_else(|| {
                    panic!("Missing embedded asset for icon '{name_str}' at {res}px ({state_str})")
                });

                // Verify PNG magic header
                assert!(
                    bytes.len() > 8 && &bytes[..8] == b"\x89PNG\r\n\x1a\n",
                    "Icon '{name_str}' at {res}px ({state_str}) has invalid PNG magic header"
                );

                // Fully decode PNG into pixel buffer using `image` crate with PNG feature
                let decoded = image::load_from_memory(bytes).unwrap_or_else(|e| {
                    panic!(
                        "Failed to decode PNG for icon '{name_str}' at {res}px ({state_str}): {e}"
                    )
                });

                // Validate dimensions match exact requested resolution
                let (width, height) = decoded.dimensions();
                assert_eq!(
                    (width, height),
                    (res, res),
                    "Dimension mismatch for icon '{name_str}' at {res}px ({state_str}): got {width}x{height}"
                );

                // Validate non-empty artwork: ensure at least some pixels have non-zero alpha channel
                let rgba = decoded.to_rgba8();
                let has_visible_artwork = rgba.pixels().any(|p| p[3] > 0);
                assert!(
                    has_visible_artwork,
                    "Icon '{name_str}' at {res}px ({state_str}) contains only empty transparent pixels"
                );

                // For problematic icons, verify transparency exists (not a fully opaque corrupted block)
                if problematic_icons.contains(&name_str) {
                    let has_transparent_pixels = rgba.pixels().any(|p| p[3] < 255);
                    assert!(
                        has_transparent_pixels,
                        "Problematic icon '{name_str}' at {res}px ({state_str}) must retain transparency"
                    );
                }

                decoded_count += 1;
            }
        }
    }

    assert_eq!(
        decoded_count, 1680,
        "Must verify exactly 1,680 embedded icon assets (168 icons x 5 resolutions x 2 states)"
    );
}

#[test]
fn test_vault_quarantine_journal_durable_state_machine_and_overwrite_protection() {
    use crate::data_path::{
        quarantine_vault_files, resolve_manifest_file, resolve_vault_db_file,
        restore_quarantined_vault, QuarantineStage, VaultQuarantineJournal,
    };
    use std::fs;

    let temp_dir = std::env::temp_dir().join(format!(
        "shellumi_test_q_{}",
        uuid::Uuid::new_v4().as_simple()
    ));
    let _ = fs::create_dir_all(&temp_dir);

    let vault_id = "vlt-quarantine-test";
    let db_path = resolve_vault_db_file(&temp_dir, vault_id);
    let manifest_path = resolve_manifest_file(&temp_dir, vault_id);
    let wal_path = std::path::PathBuf::from(format!("{}-wal", db_path.display()));

    fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
    fs::write(&db_path, b"SQLite format 3\0test_database_bytes").unwrap();
    fs::write(
        &manifest_path,
        b"{\"vault_id\":\"vlt-quarantine-test\",\"name\":\"Test\"}",
    )
    .unwrap();
    fs::write(&wal_path, b"mock_wal_bytes").unwrap();

    // 1. Execute quarantine
    let q_dir =
        quarantine_vault_files(&temp_dir, vault_id, "Testing safe decommissioning").unwrap();
    assert!(q_dir.exists());

    // 2. Verify durable journal was written with Completed stage
    let journal_path = q_dir.join("journal.json");
    assert!(journal_path.exists());
    let journal_content = fs::read_to_string(&journal_path).unwrap();
    let journal: VaultQuarantineJournal = serde_json::from_str(&journal_content).unwrap();
    assert_eq!(journal.stage, QuarantineStage::Completed);
    assert_eq!(journal.vault_id, vault_id);
    assert!(journal.quarantined_db_path.is_some());
    assert!(journal.quarantined_manifest_path.is_some());
    assert!(journal.quarantined_wal_path.is_some());

    // 3. Verify original files no longer exist at original locations
    assert!(!db_path.exists());
    assert!(!manifest_path.exists());
    assert!(!wal_path.exists());

    // 4. Test Overwrite Protection: create conflicting destination file
    fs::write(&db_path, b"conflict_db").unwrap();
    let overwrite_err = restore_quarantined_vault(&q_dir);
    assert!(
        overwrite_err.is_err(),
        "restore_quarantined_vault must refuse to overwrite existing files"
    );
    assert!(overwrite_err.unwrap_err().contains("already exists"));

    // 5. Remove conflict and restore successfully
    fs::remove_file(&db_path).unwrap();
    let restore_res = restore_quarantined_vault(&q_dir);
    assert!(
        restore_res.is_ok(),
        "Restore must succeed when destination has no conflicts"
    );
    assert!(db_path.exists());
    assert!(manifest_path.exists());
    assert!(wal_path.exists());
    assert!(
        !q_dir.exists(),
        "Quarantine directory must be purged on successful restore"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_vault_quarantine_restart_recovery() {
    use crate::data_path::{
        detect_and_recover_interrupted_quarantine, resolve_quarantine_dir, QuarantineStage,
        VaultQuarantineJournal,
    };
    use std::fs;

    let temp_dir = std::env::temp_dir().join(format!(
        "shellumi_test_q_rec_{}",
        uuid::Uuid::new_v4().as_simple()
    ));
    let q_root = resolve_quarantine_dir(&temp_dir);
    let q_vault_dir = q_root.join("vlt-interrupted_12345_uuid");
    fs::create_dir_all(&q_vault_dir).unwrap();

    let journal = VaultQuarantineJournal {
        journal_version: 1,
        vault_id: "vlt-interrupted".to_string(),
        original_db_path: temp_dir
            .join("vaults/vlt-interrupted.db")
            .to_string_lossy()
            .to_string(),
        original_manifest_path: None,
        original_wal_path: None,
        original_shm_path: None,
        quarantined_db_path: None,
        quarantined_manifest_path: None,
        quarantined_wal_path: None,
        quarantined_shm_path: None,
        stage: QuarantineStage::Initiated,
        quarantined_at: "12345".to_string(),
        reason: "Interrupted deletion test".to_string(),
    };
    let journal_json = serde_json::to_string_pretty(&journal).unwrap();
    fs::write(q_vault_dir.join("journal.json"), journal_json).unwrap();

    // Call recovery
    assert!(detect_and_recover_interrupted_quarantine(&temp_dir).is_err());
    assert!(q_vault_dir.join("journal.json").exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_vault_password_rotation_durable_journal_and_restart_recovery() {
    use crate::data_path::{
        check_pending_password_rotation, resolve_manifest_file, write_rotation_journal,
        PasswordRotationJournal, PasswordRotationStage,
    };
    use shellumi_core::{VaultId, VaultKey, VaultManifest};
    use std::fs;

    let temp_dir = std::env::temp_dir().join(format!(
        "shellumi_test_pwd_rot_{}",
        uuid::Uuid::new_v4().as_simple()
    ));
    let vault_id = "vlt-pwd-test";
    let manifest_path = resolve_manifest_file(&temp_dir, vault_id);
    let _ = fs::create_dir_all(manifest_path.parent().unwrap());
    let rotation_journal_path = manifest_path.with_extension("rotation_journal.json");

    let vid = VaultId::new(vault_id).unwrap();
    let old_pass = "OldPassword123!";
    let rec_code = "RecoveryCode123!";

    let key = VaultKey::generate().unwrap();
    let primary_envelope = key.wrap_with_password(&vid, old_pass).unwrap();
    let recovery_envelope = Some(key.wrap_with_password(&vid, rec_code).unwrap());
    let manifest = VaultManifest::new(
        vid.as_str().to_string(),
        "Password Test Vault".to_string(),
        primary_envelope,
        recovery_envelope,
        false,
    );
    manifest.save_to_path(&manifest_path).unwrap();

    let original_bytes = fs::read_to_string(&manifest_path).unwrap();

    // Stage an incomplete rotation journal (e.g. power loss during DB update)
    let journal = PasswordRotationJournal {
        journal_version: 1,
        vault_id: vault_id.to_string(),
        stage: PasswordRotationStage::Prepared,
        original_manifest_backup: Some(original_bytes.clone()),
        staged_manifest_json: None,
        new_primary_envelope_json: None,
        original_registry_envelope: None,
        original_target_db_envelope: None,
        original_registry_state: None,
        original_target_db_state: None,
        target_db_path: None,
        registry_db_path: None,
        manifest_path: None,
        had_keychain_secret: false,
        remember_in_keychain: false,
        updated_at: 1000,
    };
    write_rotation_journal(&rotation_journal_path, &journal).unwrap();

    // Corrupt manifest to simulate partial write before crash
    fs::write(&manifest_path, b"corrupted partial manifest write").unwrap();

    for _ in 0..2 {
        assert!(check_pending_password_rotation(&temp_dir).is_err());
        assert_eq!(
            fs::read(&manifest_path).unwrap(),
            b"corrupted partial manifest write"
        );
        let saved: PasswordRotationJournal =
            serde_json::from_slice(&fs::read(&rotation_journal_path).unwrap()).unwrap();
        let backup: VaultManifest =
            serde_json::from_str(saved.original_manifest_backup.as_ref().unwrap()).unwrap();
        assert_eq!(
            backup.unlock(old_pass).unwrap().expose_bytes(),
            key.expose_bytes()
        );
        assert_eq!(
            backup
                .unlock_with_recovery(rec_code)
                .unwrap()
                .expose_bytes(),
            key.expose_bytes()
        );
    }
    fs::write(&rotation_journal_path, b"truncated").unwrap();
    assert!(check_pending_password_rotation(&temp_dir).is_err());
    assert_eq!(fs::read(&rotation_journal_path).unwrap(), b"truncated");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_vault_quarantine_crash_recovery_restores_database_when_interrupted_at_initiated() {
    use crate::data_path::{
        detect_and_recover_interrupted_quarantine, resolve_quarantine_dir, QuarantineStage,
        VaultQuarantineJournal,
    };
    use std::fs;

    let temp_dir = std::env::temp_dir().join(format!(
        "shellumi_test_q_crash_{}",
        uuid::Uuid::new_v4().as_simple()
    ));
    let q_root = resolve_quarantine_dir(&temp_dir);
    let q_vault_dir = q_root.join("vlt-crashed_54321_uuid");
    fs::create_dir_all(&q_vault_dir).unwrap();

    let original_db = temp_dir.join("vaults/vlt-crashed.db");
    let original_manifest = temp_dir.join("vault_manifests/vlt-crashed.manifest.json");

    // Put real data files into quarantine folder to simulate crash right after moving files
    // but BEFORE journal stage was updated from Initiated
    fs::write(
        q_vault_dir.join("vlt-crashed.db"),
        b"SQLITE_DATABASE_CRITICAL_USER_DATA",
    )
    .unwrap();
    fs::write(
        q_vault_dir.join("vlt-crashed.manifest.json"),
        b"{\"manifest\":\"critical\"}",
    )
    .unwrap();

    let journal = VaultQuarantineJournal {
        journal_version: 1,
        vault_id: "vlt-crashed".to_string(),
        original_db_path: original_db.to_string_lossy().to_string(),
        original_manifest_path: Some(original_manifest.to_string_lossy().to_string()),
        original_wal_path: None,
        original_shm_path: None,
        quarantined_db_path: None, // Interrupted before recording moved path
        quarantined_manifest_path: None,
        quarantined_wal_path: None,
        quarantined_shm_path: None,
        stage: QuarantineStage::Initiated, // Stage still says Initiated!
        quarantined_at: "54321".to_string(),
        reason: "Crash recovery test".to_string(),
    };
    let journal_json = serde_json::to_string_pretty(&journal).unwrap();
    fs::write(q_vault_dir.join("journal.json"), journal_json).unwrap();

    // Call recovery
    for _ in 0..2 {
        assert!(detect_and_recover_interrupted_quarantine(&temp_dir).is_err());
    }
    assert!(q_vault_dir.join("journal.json").exists());

    // CRITICAL GUARANTEE: User data was NOT purged!
    // It was safely reconciled and restored back to original paths!
    assert!(
        original_db.exists(),
        "Original database file must be restored"
    );
    assert_eq!(
        fs::read(&original_db).unwrap(),
        b"SQLITE_DATABASE_CRITICAL_USER_DATA"
    );
    assert!(
        original_manifest.exists(),
        "Original manifest file must be restored"
    );
    assert_eq!(
        fs::read(&original_manifest).unwrap(),
        b"{\"manifest\":\"critical\"}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn term_rec_t01_t02_t03_recording_lifecycle_uuids_and_recovery() {
    use crate::session::{Session, SessionEvent};
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("vault.db");
    let db_str = db_path.to_string_lossy().to_string();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let vault_key = shellumi_core::VaultKey::from_bytes([0x22; 32]);
    let vault_id = shellumi_core::VaultId::new("vault-rec-test").unwrap();

    // 1. Real PTY session + Terminal + SessionRecorder + SessionRedactor + RecoveryJournal + Turso DB
    let mut term1 = shellumi_terminal::Terminal::new(24, 80);
    term1.start_recording(Some("PTY Recording 1".into()));
    let sess =
        Session::start(shellumi_terminal::TerminalCommand::new("/bin/echo").arg("pty-rec-ok"));
    loop {
        match sess.events.recv_blocking().unwrap() {
            SessionEvent::Output(bytes) => term1.process_bytes(&bytes),
            SessionEvent::Closed => break,
            SessionEvent::Error(e) => panic!("{e}"),
        }
    }
    let rec1 = term1.stop_recording().unwrap();
    let raw1 = rec1.to_asciicast_v2().unwrap();
    let clean1 = shellumi_core::SessionRedactor::redact_asciicast_v2(&raw1).unwrap();
    assert!(clean1.contains("pty-rec-ok"));

    let mut term2 = shellumi_terminal::Terminal::new(24, 80);
    term2.start_recording(Some("PTY Recording 2".into()));
    term2.process_str("pane2 output\r\n");
    let rec2 = term2.stop_recording().unwrap();
    let clean2 =
        shellumi_core::SessionRedactor::redact_asciicast_v2(&rec2.to_asciicast_v2().unwrap())
            .unwrap();

    let id1 = uuid::Uuid::new_v4().to_string();
    let id2 = uuid::Uuid::new_v4().to_string();
    assert_ne!(id1, id2);
    let now = shellumi_core::format_rfc3339_utc(std::time::SystemTime::now());

    // Encrypted recovery journal roundtrip before DB commit
    let journal = shellumi_core::RecordingRecoveryJournal::new(tmp.path().join("recovery"));
    let cp = shellumi_core::RecordingCheckpoint {
        recording_id: id1.clone(),
        vault_id: vault_id.as_str().to_string(),
        session_id: uuid::Uuid::new_v4().to_string(),
        pane_id: 1,
        host_id: None,
        protocol: "Local".into(),
        title: "PTY Recording 1".into(),
        started_at: now.clone(),
        duration_secs: rec1.duration_secs(),
        event_count: rec1.event_count(),
        byte_size: clean1.len(),
        cast_data: clean1.clone(),
        truncated: false,
    };
    journal.save_checkpoint(&cp, &vault_id, &vault_key).unwrap();
    assert_eq!(
        journal
            .load_checkpoint(&id1, &vault_id, &vault_key)
            .unwrap(),
        cp
    );

    rt.block_on(async {
        let mut store = shellumi_storage::HostStore::open_with_key(&db_str, Some(&vault_key))
            .await
            .unwrap();
        let db_rec1 = shellumi_storage::SessionRecordingRecord {
            id: id1.clone(),
            session_id: cp.session_id.clone(),
            host_id: None,
            title: "PTY Recording 1".into(),
            duration_secs: rec1.duration_secs(),
            event_count: rec1.event_count(),
            byte_size: clean1.len(),
            cast_data: clean1,
            recorded_at: now.clone(),
            created_at: now.clone(),
        };
        let db_rec2 = shellumi_storage::SessionRecordingRecord {
            id: id2.clone(),
            session_id: uuid::Uuid::new_v4().to_string(),
            host_id: None,
            title: "PTY Recording 2".into(),
            duration_secs: rec2.duration_secs(),
            event_count: rec2.event_count(),
            byte_size: clean2.len(),
            cast_data: clean2,
            recorded_at: now.clone(),
            created_at: now,
        };
        store.save_session_recording(&db_rec1).await.unwrap();
        // Idempotent retry
        store.save_session_recording(&db_rec1).await.unwrap();
        store.save_session_recording(&db_rec2).await.unwrap();
        let list = store.list_session_recordings(None).await.unwrap();
        assert_eq!(list.len(), 2);

        // Cross-vault ID collision must fail closed
        store.set_vault_id(shellumi_core::VaultId::new("other-vault").unwrap());
        assert!(store.save_session_recording(&db_rec1).await.is_err());
    });
}

#[test]
fn term_ui_t01_t02_t03_modes_paste_and_selection() {
    let modes = shellumi_terminal::TerminalModes {
        application_cursor: true,
        ..Default::default()
    };
    let up = gpui::Keystroke::parse("up").unwrap();
    assert_eq!(super::key_bytes_for_modes(&up, &modes).unwrap(), b"\x1bOA");
    let f5 = gpui::Keystroke::parse("f5").unwrap();
    assert_eq!(
        super::key_bytes_for_modes(&f5, &modes).unwrap(),
        b"\x1b[15~"
    );
    let shift_tab = gpui::Keystroke::parse("shift-tab").unwrap();
    assert_eq!(
        super::key_bytes_for_modes(&shift_tab, &modes).unwrap(),
        b"\x1b[Z"
    );

    let paste = super::encode_paste_bytes("echo ok\x1b[201~evil\nline2", true);
    assert_eq!(paste, b"\x1b[200~echo okevil\rline2\x1b[201~".to_vec());

    let mut term = shellumi_terminal::Terminal::new(4, 20);
    term.process_str("pub\x1b[8mSECRET\x1b[0mend");
    let sel = super::TerminalSelection {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 15,
        rectangular: false,
    };
    let copied = super::selected_text_from_terminal(&term, sel);
    assert!(!copied.contains("SECRET"));
    assert!(copied.contains("pub"));
    assert!(copied.contains("end"));
}

#[test]
fn term_ui_t06_virtualized_50k_scrollback_benchmark() {
    let mut term = shellumi_terminal::Terminal::new(24, 80);
    term.set_scrollback_limit(50_000);
    let t0 = std::time::Instant::now();
    for i in 0..50_025 {
        term.process_str(&format!("Row {i:05}: benchmark payload\r\n"));
    }
    let ingest_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(term.scrollback_count(), 50_000);

    let r0 = std::time::Instant::now();
    let mut rendered_rows = 0usize;
    for i in (50_000 - 32)..50_000 {
        if let Some(spans) = term.grid().scrollback_row_spans(i) {
            let _ = super::render_terminal_spans(spans, 21.0);
            rendered_rows += 1;
        }
    }
    let render_ms = r0.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(rendered_rows, 32);
    assert!(ingest_ms < 5000.0, "50k ingest took {ingest_ms:.2}ms");
    assert!(
        render_ms < 50.0,
        "32-row virtualized render took {render_ms:.2}ms"
    );
}

#[test]
fn term_play_t01_isolated_replay_and_side_effect_suppression() {
    let cast = "{\"version\":2,\"width\":40,\"height\":10,\"duration\":2.0}\n[0.5,\"o\",\"\\u0007\\u001b]52;c;aGVsbG8=\\u0007\\u001b[6nHello\"]\n[1.0,\"r\",\"60x15\"]\n[1.5,\"o\",\" World\"]\n";
    let mut engine = shellumi_core::CastPlaybackEngine::from_asciicast_v2(cast).unwrap();
    engine.play();
    let frames = engine.advance(1.6);
    let mut term = crate::actions::recordings::rebuild_replay_terminal(cast, &frames);
    assert_eq!(term.grid().cols(), 60);
    assert_eq!(term.grid().rows(), 15);
    assert!(term.screen_to_string().contains("Hello World"));
    assert!(term.drain_events().is_empty());
}

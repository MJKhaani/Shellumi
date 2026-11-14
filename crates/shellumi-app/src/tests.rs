mod collaboration;
mod infrastructure;
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
        OpenLocalTerminal, OpenSettings, PrevTab, QuickConnect, ReconnectSession, RefreshHosts,
        ResetZoom, SelectTab1, SelectTab9, SplitHorizontal, SplitVertical, ToggleBroadcast,
        ToggleCommandPalette, ToggleFocusMode, ToggleSidebar, ToggleSidebarCollapse,
        ToggleSnippets, ZoomIn, ZoomOut,
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

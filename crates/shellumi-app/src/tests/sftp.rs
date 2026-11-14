#[test]
fn sftp_transfer_names_reject_paths_and_empty_selection() {
    assert!(super::super::sftp_valid_transfer_names(&[
        "report.txt".into(),
        "folder".into()
    ]));
    assert!(!super::super::sftp_valid_transfer_names(&[]));
    for name in ["../secret", "/tmp/file", "..", "a/b", "a\\b", "bad\nname"] {
        assert!(
            !super::super::sftp_valid_transfer_names(&[name.into()]),
            "{name:?}"
        );
    }
}

#[test]
fn verifies_sftp_dual_pane_state_and_navigation() {
    use super::super::*;
    use shellumi_core::{FileEntry, FileKind, PathUtils, TransferDirection, TransferQueue};

    let mut sftp_state = SftpBrowserState {
        listing_generation: 0,
        host_name: "prod-server".to_string(),
        connection_options: SftpConnectOptions::default(),
        destination: None,
        jump_host: None,
        loading: false,
        local_path: "/Users/test/workspace".to_string(),
        remote_path: "/var/www".to_string(),
        local_entries: vec![
            FileEntry {
                name: "index.html".to_string(),
                path: "/Users/test/workspace/index.html".to_string(),
                size: 2048,
                kind: FileKind::File,
                modified: Some(1700000000),
                permissions: Some(0o644),
            },
            FileEntry {
                name: "assets".to_string(),
                path: "/Users/test/workspace/assets".to_string(),
                size: 4096,
                kind: FileKind::Directory,
                modified: Some(1700000100),
                permissions: Some(0o755),
            },
        ],
        remote_entries: vec![FileEntry {
            name: "backup.tar.gz".to_string(),
            path: "/var/www/backup.tar.gz".to_string(),
            size: 1048576,
            kind: FileKind::File,
            modified: Some(1700000200),
            permissions: Some(0o600),
        }],
        selected_local: None,
        selected_remote: None,
        show_hidden: false,
        mode: SftpTabMode::DualPane,
        error: None,
        status_message: None,
        queue: TransferQueue::new(2),
    };

    // 1. Initial mode and selections
    assert_eq!(sftp_state.mode, SftpTabMode::DualPane);
    assert!(!sftp_state.show_hidden);
    assert_eq!(sftp_state.selected_local, None);
    assert_eq!(sftp_state.selected_remote, None);

    // 2. Select local file
    sftp_state.selected_local = Some("index.html".to_string());
    assert_eq!(sftp_state.selected_local.as_deref(), Some("index.html"));

    // 3. Queue upload
    let file = sftp_state
        .local_entries
        .iter()
        .find(|e| e.name == "index.html")
        .unwrap()
        .clone();
    let target_remote = PathUtils::join_path(&sftp_state.remote_path, &file.name);
    assert_eq!(target_remote, "/var/www/index.html");

    let tid = sftp_state.queue.enqueue(
        TransferDirection::Upload,
        &file.path,
        &target_remote,
        file.size,
    );
    assert_eq!(tid, 1);
    let dispatched = sftp_state.queue.dispatch_next();
    assert_eq!(dispatched, vec![1]);

    sftp_state.queue.mark_completed(tid);
    assert_eq!(sftp_state.queue.summary().completed_tasks, 1);
    assert_eq!(sftp_state.queue.summary().overall_percentage, 100.0);

    // 4. Select remote file and download
    sftp_state.selected_remote = Some("backup.tar.gz".to_string());
    let rfile = sftp_state
        .remote_entries
        .iter()
        .find(|e| e.name == "backup.tar.gz")
        .unwrap()
        .clone();
    let target_local = PathUtils::join_path(&sftp_state.local_path, &rfile.name);
    assert_eq!(target_local, "/Users/test/workspace/backup.tar.gz");

    let tid2 = sftp_state.queue.enqueue(
        TransferDirection::Download,
        &target_local,
        &rfile.path,
        rfile.size,
    );
    assert_eq!(tid2, 2);
    let dispatched2 = sftp_state.queue.dispatch_next();
    assert_eq!(dispatched2, vec![2]);
    sftp_state.queue.mark_completed(tid2);

    assert_eq!(sftp_state.queue.summary().completed_tasks, 2);
    assert_eq!(sftp_state.queue.summary().total_tasks, 2);

    // 5. Clear completed
    sftp_state.queue.clear_completed();
    assert_eq!(sftp_state.queue.summary().total_tasks, 0);

    // 6. Navigation breadcrumbs
    let crumbs = PathUtils::breadcrumbs(&sftp_state.remote_path);
    assert_eq!(
        crumbs,
        vec![
            ("/".to_string(), "/".to_string()),
            ("var".to_string(), "/var".to_string()),
            ("www".to_string(), "/var/www".to_string())
        ]
    );
    let parent = PathUtils::parent_path(&sftp_state.remote_path);
    assert_eq!(parent, Some("/var".to_string()));

    // 7. Toggle view mode
    sftp_state.mode = match sftp_state.mode {
        SftpTabMode::DualPane => SftpTabMode::Terminal,
        SftpTabMode::Terminal => SftpTabMode::DualPane,
    };
    assert_eq!(sftp_state.mode, SftpTabMode::Terminal);
}

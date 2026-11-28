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

#[test]
fn app_sftp_unified_transfer_and_editor_engine_integration() {
    use shellumi_core::{
        detect_language, format_document, BatchId, EditorBuffer, EditorLanguage, EndpointKind,
        EndpointPath, FormatOutcome, JobId, SymlinkPolicy, TransferIntent, TransferJobState,
        TransferRoute, UnifiedTransferManager, VaultScope,
    };

    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("sshd_config");
    let dst = dir.path().join("sshd_config.bak");
    std::fs::write(&src, b"Port 22\nPermitRootLogin no\n").unwrap();

    // 1. UnifiedTransferManager staged local-to-local copy
    let scope = VaultScope::new("test-vault", 1);
    let mgr = UnifiedTransferManager::new(2);
    let job_id = JobId::new();
    let intent = TransferIntent {
        job_id,
        batch_id: BatchId::new(),
        vault_scope: scope.clone(),
        route: TransferRoute::LocalToLocal,
        source_endpoint: EndpointKind::local(scope.clone()),
        destination_endpoint: EndpointKind::local(scope.clone()),
        source_path: EndpointPath::Local(src.clone()),
        destination_path: EndpointPath::Local(dst.clone()),
        source_options: None,
        destination_options: None,
        conflict_policy: shellumi_core::ConflictPolicy::Ask,
        symlink_policy: SymlinkPolicy::Skip,
        preserve_metadata: true,
        delete_source_after_commit: false,
        expected_bytes: Some(27),
    };
    mgr.enqueue(intent).unwrap();
    let ready = mgr.dispatch_ready(&scope);
    assert_eq!(ready.len(), 1);
    let res = mgr.execute_local_to_local(&ready[0].0, ready[0].1, &ready[0].2);
    mgr.complete_attempt(job_id, ready[0].1, res);
    assert_eq!(
        mgr.snapshot(job_id).unwrap().state,
        TransferJobState::Completed
    );
    assert_eq!(
        std::fs::read(&dst).unwrap(),
        b"Port 22\nPermitRootLogin no\n"
    );

    // 2. Editor language detection & conservative formatting
    let content = std::fs::read_to_string(&src).unwrap();
    assert_eq!(
        detect_language(src.to_str().unwrap(), &content),
        EditorLanguage::SshConfig
    );
    assert!(matches!(
        format_document(EditorLanguage::SshConfig, &content, 2),
        FormatOutcome::Unsupported(_)
    ));

    let buf = EditorBuffer::from_bytes(content.as_bytes()).unwrap();
    assert!(!buf.is_dirty());
}

#[test]
fn sftp_controller_enforces_vault_auth_gate_and_revocation() {
    use super::super::sftp_controller::SftpController;
    use shellumi_core::VaultScope;

    let v1 = VaultScope::new("vault-a", 1);
    let v2 = VaultScope::new("vault-b", 1);
    let controller = SftpController::new(v1.clone());

    let (op1, ctrl1) = controller.register_op(&v1).unwrap();
    assert!(controller.is_op_authorized(op1, &v1));
    assert!(controller.register_op(&v2).is_err());

    // Switch vault -> op1 must be cancelled and unauthorized
    controller.revoke_unauthorized(Some(&v2));
    assert!(ctrl1.is_cancelled());
    assert!(!controller.is_op_authorized(op1, &v1));
    assert!(controller.register_op(&v1).is_err());

    // Lock vault -> nothing authorized
    controller.revoke_unauthorized(None);
    assert!(controller.register_op(&v2).is_err());
}

#[test]
fn operation_lease_cleans_up_on_drop_and_utf16_roundtrip_is_safe() {
    use super::super::sftp_controller::SftpController;
    use super::super::sftp_editor::{
        byte_to_utf16, range_from_utf16, range_to_utf16, utf16_to_byte,
    };
    use shellumi_core::VaultScope;

    let v1 = VaultScope::new("vault-lease", 1);
    let controller = SftpController::new(v1.clone());
    let op_id = {
        let lease = controller.register_lease(&v1).unwrap();
        assert!(controller.is_op_authorized(lease.op_id, &v1));
        lease.op_id
    };
    // Dropped lease must immediately unregister from active_ops
    assert!(!controller.is_op_authorized(op_id, &v1));

    let s = "Aé中🙂سلام";
    for (byte_idx, _) in s.char_indices() {
        let u16_idx = byte_to_utf16(s, byte_idx);
        assert_eq!(utf16_to_byte(s, u16_idx), byte_idx);
    }
    let full = range_to_utf16(s, 0..s.len());
    assert_eq!(range_from_utf16(s, &full), 0..s.len());
}
